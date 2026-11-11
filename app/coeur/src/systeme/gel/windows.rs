//! Le Gel sous Windows : GDI, écrans, fenêtres visibles, fenêtre native de l'écran figé.
//!
//! Deux verrous : `IMAGE` (la photo du bureau) et `ECRAN` (la fenêtre native).
//! Règle : on ne garde jamais un verrou pendant un appel qui peut envoyer un message à une
//! fenêtre (`SetWindowPos`, `ShowWindow`, `CreateWindowExW`, `DestroyWindow`…). La
//! procédure de la fenêtre prend `IMAGE` pour peindre : un verrou gardé à ce moment-là
//! bloquerait le fil. Un verrou empoisonné (fil qui a paniqué) est repris, jamais déballé.

use std::sync::{Mutex, MutexGuard};

use ::windows::core::{w, BOOL, PCWSTR};
use ::windows::Win32::Foundation::{
    GetLastError, ERROR_CLASS_ALREADY_EXISTS, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM,
};
use ::windows::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DwmSetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
    DWMWA_TRANSITIONS_FORCEDISABLED,
};
use ::windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject,
    EndPaint, EnumDisplayMonitors, GdiFlush, GetDC, GetDIBits, GetMonitorInfoW, ReleaseDC,
    SelectObject, ValidateRect, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT, DIB_RGB_COLORS,
    HBITMAP, HDC, HGDIOBJ, HMONITOR, MONITORINFO, PAINTSTRUCT, SRCCOPY,
};
use ::windows::Win32::System::LibraryLoader::GetModuleHandleW;
use ::windows::Win32::System::Threading::{GetCurrentProcessId, GetCurrentThreadId};
use ::windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use ::windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, EnumWindows, GetSystemMetrics, GetWindowRect,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow,
    IsWindowVisible, LoadCursorW, PostMessageW, RegisterClassW, SetWindowPos, ShowWindow,
    ShowWindowAsync, HWND_TOPMOST, IDC_CROSS, MA_NOACTIVATE, MONITORINFOF_PRIMARY,
    SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SWP_NOACTIVATE,
    SWP_SHOWWINDOW, SW_HIDE, WM_CLOSE, WM_ERASEBKGND, WM_MOUSEACTIVATE, WM_PAINT, WNDCLASSW,
    WNDPROC, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use super::{
    bgra_en_rgba, echelle_depuis_dpi, garder_fenetre, EcranInfo, EtatFenetre, FenetreInfo, Pixels,
    Rect,
};

pub const DISPONIBLE: bool = true;

/// Prend un verrou ; s'il est empoisonné, reprend la valeur au lieu de paniquer.
fn verrouiller<T>(verrou: &Mutex<T>) -> MutexGuard<'_, T> {
    verrou.lock().unwrap_or_else(|e| e.into_inner())
}

fn rect_de(r: RECT) -> Rect {
    Rect::depuis_bords(r.left, r.top, r.right, r.bottom)
}

/// Le bureau virtuel : tous les écrans, en pixels réels.
fn bureau_virtuel() -> Result<Rect, String> {
    // SAFETY : lecture de constantes du système, sans tampon.
    let (x, y, largeur, hauteur) = unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    };
    if largeur <= 0 || hauteur <= 0 {
        return Err(format!("bureau virtuel invalide ({largeur}x{hauteur})"));
    }
    Ok(Rect {
        x,
        y,
        largeur,
        hauteur,
    })
}

// ---------------------------------------------------------------------------------------
// Ressources GDI : chaque poignée a un propriétaire qui la rend dans son `Drop`.

/// Un DC prêté par `GetDC` (écran entier si `fenetre` est `None`), rendu par `ReleaseDC`.
struct DcPrete {
    fenetre: Option<HWND>,
    dc: HDC,
}

impl DcPrete {
    fn prendre(fenetre: Option<HWND>) -> Result<DcPrete, String> {
        // SAFETY : `fenetre` est `None` (écran) ou une fenêtre de ce processus.
        let dc = unsafe { GetDC(fenetre) };
        if dc.is_invalid() {
            return Err("GetDC a echoue".into());
        }
        Ok(DcPrete { fenetre, dc })
    }
}

impl Drop for DcPrete {
    fn drop(&mut self) {
        // SAFETY : DC obtenu par `GetDC` avec la même fenêtre.
        unsafe {
            ReleaseDC(self.fenetre, self.dc);
        }
    }
}

/// Un DC en mémoire (`CreateCompatibleDC`), détruit par `DeleteDC`.
struct DcMemoire(HDC);

impl Drop for DcMemoire {
    fn drop(&mut self) {
        // SAFETY : DC créé par `CreateCompatibleDC`, détruit une seule fois.
        unsafe {
            let _ = DeleteDC(self.0);
        }
    }
}

/// Un bitmap GDI, détruit par `DeleteObject`.
struct Bitmap(HBITMAP);

impl Drop for Bitmap {
    fn drop(&mut self) {
        // SAFETY : bitmap créé par `CreateCompatibleBitmap`, détruit une seule fois, après
        // avoir été retiré de son DC (voir `ImageGdi::drop`).
        unsafe {
            let _ = DeleteObject(self.0.into());
        }
    }
}

/// Une image GDI en mémoire : un DC et son bitmap, sélectionné dedans.
///
/// Ordre de destruction : `drop` remet l'ancien objet dans le DC (le bitmap n'y est plus
/// sélectionné), puis les champs sont détruits dans l'ordre : le DC, puis le bitmap. Même
/// si la remise échouait, détruire le DC d'abord libère le bitmap.
struct ImageGdi {
    dc: DcMemoire,
    bitmap: Bitmap,
    ancien: HGDIOBJ,
    largeur: i32,
    hauteur: i32,
}

// SAFETY : les objets GDI ne sont pas liés au fil qui les a créés ; le verrou `IMAGE` (ou
// la propriété exclusive, pour une image locale) garantit qu'un seul fil s'en sert à la fois.
unsafe impl Send for ImageGdi {}

impl ImageGdi {
    /// Crée une image vide compatible avec `reference` (l'écran, ou une autre image).
    fn nouvelle(reference: HDC, largeur: i32, hauteur: i32) -> Result<ImageGdi, String> {
        if largeur <= 0 || hauteur <= 0 {
            return Err(format!("taille d'image invalide ({largeur}x{hauteur})"));
        }
        // SAFETY : `reference` est un DC valide pendant l'appel.
        let dc = unsafe { CreateCompatibleDC(Some(reference)) };
        if dc.is_invalid() {
            return Err("CreateCompatibleDC a echoue".into());
        }
        let dc = DcMemoire(dc);
        // SAFETY : idem ; le bitmap a le format de ce qui est affiché par `reference`.
        let bitmap = unsafe { CreateCompatibleBitmap(reference, largeur, hauteur) };
        if bitmap.is_invalid() {
            return Err(format!(
                "CreateCompatibleBitmap {largeur}x{hauteur} a echoue (memoire ?)"
            ));
        }
        let bitmap = Bitmap(bitmap);
        // SAFETY : DC et bitmap valides, possédés ici.
        let ancien = unsafe { SelectObject(dc.0, bitmap.0.into()) };
        if ancien.is_invalid() {
            return Err("SelectObject du bitmap a echoue".into());
        }
        Ok(ImageGdi {
            dc,
            bitmap,
            ancien,
            largeur,
            hauteur,
        })
    }

    /// Lit toute l'image en BGRA 32 bits, de haut en bas (`GetDIBits`).
    fn lire_tout(&self) -> Result<Vec<u8>, String> {
        let taille = (self.largeur as usize)
            .checked_mul(self.hauteur as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or("zone trop grande")?;
        let mut bgra = vec![0u8; taille];
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: self.largeur,
                // Hauteur négative : lignes de haut en bas.
                biHeight: -self.hauteur,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        // `GetDIBits` veut un bitmap sélectionné dans aucun DC : on le retire le temps de
        // la lecture, puis on le remet.
        // SAFETY : DC et objets possédés par `self`.
        let retire = unsafe { SelectObject(self.dc.0, self.ancien) };
        if retire.is_invalid() {
            return Err("SelectObject avant GetDIBits a echoue".into());
        }
        // SAFETY : le tampon fait largeur × hauteur × 4 octets, exactement ce que décrit
        // `info` (32 bits par pixel, sans table de couleurs).
        let lignes = unsafe {
            GetDIBits(
                self.dc.0,
                self.bitmap.0,
                0,
                self.hauteur as u32,
                Some(bgra.as_mut_ptr().cast()),
                &mut info,
                DIB_RGB_COLORS,
            )
        };
        // SAFETY : remet le bitmap dans son DC.
        let remis = unsafe { SelectObject(self.dc.0, self.bitmap.0.into()) };
        if lignes != self.hauteur {
            return Err(format!(
                "GetDIBits a lu {lignes} lignes sur {}",
                self.hauteur
            ));
        }
        if remis.is_invalid() {
            return Err("SelectObject apres GetDIBits a echoue".into());
        }
        Ok(bgra)
    }
}

impl Drop for ImageGdi {
    fn drop(&mut self) {
        // SAFETY : remet dans le DC l'objet qu'il avait à sa création.
        unsafe {
            SelectObject(self.dc.0, self.ancien);
        }
    }
}

// ---------------------------------------------------------------------------------------
// La photo du bureau.

/// La photo gardée en mémoire, réutilisée tant que la taille du bureau ne change pas.
struct Photo {
    gdi: ImageGdi,
    /// Le bureau virtuel au moment de la photo.
    rect: Rect,
    /// Numéro de la photo actuellement dans `gdi`.
    numero: u64,
}

struct EtatImage {
    photo: Option<Photo>,
    dernier_numero: u64,
}

static IMAGE: Mutex<EtatImage> = Mutex::new(EtatImage {
    photo: None,
    dernier_numero: 0,
});

/// Ce que l'appelant garde d'une photo : sa place et son numéro. Les pixels restent dans
/// `IMAGE`.
#[derive(Debug)]
pub struct Image {
    rect: Rect,
    numero: u64,
}

pub fn rect_image(image: &Image) -> Rect {
    image.rect
}

pub fn figer_bureau() -> Result<Image, String> {
    let bureau = bureau_virtuel()?;
    let ecran = DcPrete::prendre(None)?;
    let mut etat = verrouiller(&IMAGE);
    let a_refaire = etat
        .photo
        .as_ref()
        .is_none_or(|p| p.gdi.largeur != bureau.largeur || p.gdi.hauteur != bureau.hauteur);
    if a_refaire {
        // L'ancienne image est rendue avant d'allouer la nouvelle.
        etat.photo = None;
        let gdi = ImageGdi::nouvelle(ecran.dc, bureau.largeur, bureau.hauteur)?;
        etat.photo = Some(Photo {
            gdi,
            rect: bureau,
            numero: 0,
        });
    }
    // Le numéro change avant la copie : même si elle échoue, une ancienne `Image` ne
    // pourra pas lire un mélange de deux photos.
    etat.dernier_numero = etat.dernier_numero.wrapping_add(1);
    let numero = etat.dernier_numero;
    let Some(photo) = etat.photo.as_mut() else {
        return Err("image du bureau absente".into());
    };
    photo.numero = numero;
    photo.rect = bureau;
    // Si le bureau a changé de taille depuis la lecture des mesures, la copie reste dans
    // l'image (au pire, une bande noire) ; la photo suivante réallouera.
    // SAFETY : les deux DC sont valides ; la copie tient dans l'image (même taille).
    unsafe {
        BitBlt(
            photo.gdi.dc.0,
            0,
            0,
            bureau.largeur,
            bureau.hauteur,
            Some(ecran.dc),
            bureau.x,
            bureau.y,
            SRCCOPY | CAPTUREBLT,
        )
    }
    .map_err(|e| format!("BitBlt du bureau: {e}"))?;
    Ok(Image {
        rect: bureau,
        numero,
    })
}

pub fn lire(image: &Image, zone: Rect) -> Result<Pixels, String> {
    let coupee = zone
        .intersection(image.rect)
        .ok_or("zone vide ou hors du bureau")?;
    let etat = verrouiller(&IMAGE);
    let photo = etat
        .photo
        .as_ref()
        .filter(|p| p.numero == image.numero)
        .ok_or("image remplacee par une photo plus recente, ou rendue")?;
    // Copie de la zone seule dans une petite image, puis lecture de celle-ci.
    let tampon = ImageGdi::nouvelle(photo.gdi.dc.0, coupee.largeur, coupee.hauteur)?;
    // SAFETY : DC valides ; la zone source est dans la photo (coupée au bureau photographié).
    unsafe {
        BitBlt(
            tampon.dc.0,
            0,
            0,
            coupee.largeur,
            coupee.hauteur,
            Some(photo.gdi.dc.0),
            coupee.x - image.rect.x,
            coupee.y - image.rect.y,
            SRCCOPY,
        )
    }
    .map_err(|e| format!("BitBlt de la zone: {e}"))?;
    drop(etat);
    let mut rgba = tampon.lire_tout()?;
    bgra_en_rgba(&mut rgba);
    Ok(Pixels {
        largeur: coupee.largeur as u32,
        hauteur: coupee.hauteur as u32,
        rgba,
    })
}

pub fn liberer_image() {
    let photo = verrouiller(&IMAGE).photo.take();
    drop(photo);
}

/// Peint la photo actuelle dans `dc`, à l'origine. Faux s'il n'y a rien à peindre.
fn peindre_image(dc: HDC) -> bool {
    let etat = verrouiller(&IMAGE);
    let Some(photo) = etat.photo.as_ref() else {
        return false;
    };
    // SAFETY : DC valides ; aucun message n'est envoyé pendant la copie.
    unsafe {
        BitBlt(
            dc,
            0,
            0,
            photo.gdi.largeur,
            photo.gdi.hauteur,
            Some(photo.gdi.dc.0),
            0,
            0,
            SRCCOPY,
        )
    }
    .is_ok()
}

// ---------------------------------------------------------------------------------------
// Écrans.

/// Appelée par `EnumDisplayMonitors` pour chaque écran.
unsafe extern "system" fn rappel_ecran(
    moniteur: HMONITOR,
    _dc: HDC,
    _rect: *mut RECT,
    donnee: LPARAM,
) -> BOOL {
    // SAFETY : `donnee` est l'adresse du `Vec` de `ecrans()`, vivant pendant l'énumération.
    let liste = unsafe { &mut *(donnee.0 as *mut Vec<EcranInfo>) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY : `info` est un MONITORINFO dont la taille est renseignée.
    if unsafe { GetMonitorInfoW(moniteur, &mut info) }.as_bool() {
        let (mut dpi_x, mut dpi_y) = (0u32, 0u32);
        // SAFETY : deux u32 en sortie.
        let lu = unsafe { GetDpiForMonitor(moniteur, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
        let dpi = if lu.is_ok() { dpi_x } else { 0 };
        liste.push(EcranInfo {
            rect: rect_de(info.rcMonitor),
            zone_utile: rect_de(info.rcWork),
            echelle: echelle_depuis_dpi(dpi),
            principal: info.dwFlags & MONITORINFOF_PRIMARY != 0,
        });
    }
    BOOL(1)
}

pub fn ecrans() -> Vec<EcranInfo> {
    let mut liste: Vec<EcranInfo> = Vec::new();
    // SAFETY : le rappel ne garde pas l'adresse de `liste` au-delà de l'appel.
    let _ = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(rappel_ecran),
            LPARAM(&mut liste as *mut Vec<EcranInfo> as isize),
        )
    };
    liste
}

// ---------------------------------------------------------------------------------------
// Fenêtres visibles.

struct Collecte {
    liste: Vec<FenetreInfo>,
    /// Processus dont on ne veut pas les fenêtres (BreezyShot lui-même).
    exclure: Option<u32>,
}

/// Le vrai bord de la fenêtre, sans l'ombre ; à défaut, son rectangle.
fn bord_reel(hwnd: HWND) -> Option<Rect> {
    let mut r = RECT::default();
    // SAFETY : `r` est un RECT dont la taille est passée.
    let lu = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut r as *mut RECT as *mut core::ffi::c_void,
            std::mem::size_of::<RECT>() as u32,
        )
    };
    if lu.is_ok() {
        return Some(rect_de(r));
    }
    // SAFETY : idem.
    unsafe { GetWindowRect(hwnd, &mut r) }.ok()?;
    Some(rect_de(r))
}

fn titre(hwnd: HWND) -> String {
    // SAFETY : lecture du titre dans un tampon dont la longueur est passée. Pour une
    // fenêtre d'un autre processus, Windows lit le titre sans lui envoyer de message.
    unsafe {
        let n = GetWindowTextLengthW(hwnd);
        if n <= 0 {
            return String::new();
        }
        let mut tampon = vec![0u16; n as usize + 1];
        let lu = GetWindowTextW(hwnd, &mut tampon);
        let lu = (lu.max(0) as usize).min(tampon.len());
        String::from_utf16_lossy(&tampon[..lu])
    }
}

fn decrire_fenetre(hwnd: HWND, exclure: Option<u32>) -> Option<FenetreInfo> {
    // SAFETY : `hwnd` vient d'`EnumWindows` ; ces lectures n'envoient aucun message.
    let (visible, reduite, processus) = unsafe {
        let mut processus = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut processus));
        (
            IsWindowVisible(hwnd).as_bool(),
            IsIconic(hwnd).as_bool(),
            processus,
        )
    };
    let de_breezyshot = exclure.is_some_and(|p| p == processus);
    // Sortie rapide (et pas de lecture du titre de nos propres fenêtres, qui enverrait un
    // message à un autre fil de l'app).
    if !visible || reduite || de_breezyshot {
        return None;
    }
    let mut masque = 0u32;
    // SAFETY : `masque` est un u32 dont la taille est passée.
    let masquee = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut masque as *mut u32 as *mut core::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        )
    }
    .is_ok()
        && masque != 0;
    let rect = bord_reel(hwnd)?;
    let titre = titre(hwnd);
    let etat = EtatFenetre {
        visible,
        reduite,
        masquee,
        a_un_titre: !titre.is_empty(),
        de_breezyshot,
    };
    garder_fenetre(etat, rect).then_some(FenetreInfo { rect, titre })
}

/// Appelée par `EnumWindows` pour chaque fenêtre de premier niveau, du premier plan vers
/// le fond.
unsafe extern "system" fn rappel_fenetre(hwnd: HWND, donnee: LPARAM) -> BOOL {
    // SAFETY : `donnee` est l'adresse de la `Collecte` de `lister_fenetres`, vivante
    // pendant l'énumération.
    let collecte = unsafe { &mut *(donnee.0 as *mut Collecte) };
    if let Some(f) = decrire_fenetre(hwnd, collecte.exclure) {
        collecte.liste.push(f);
    }
    BOOL(1)
}

fn lister_fenetres(exclure: Option<u32>) -> Vec<FenetreInfo> {
    let mut collecte = Collecte {
        liste: Vec::new(),
        exclure,
    };
    // SAFETY : le rappel ne garde pas l'adresse de `collecte` au-delà de l'appel.
    let _ = unsafe {
        EnumWindows(
            Some(rappel_fenetre),
            LPARAM(&mut collecte as *mut Collecte as isize),
        )
    };
    collecte.liste
}

pub fn fenetres_visibles() -> Vec<FenetreInfo> {
    // SAFETY : sans paramètre.
    let moi = unsafe { GetCurrentProcessId() };
    lister_fenetres(Some(moi))
}

// ---------------------------------------------------------------------------------------
// Fenêtre native de l'écran figé.

const CLASSE_ECRAN_FIGE: PCWSTR = w!("BreezyShotEcranFige");

/// Inscrit une classe de fenêtre (une fois par processus ; une seconde fois ne fait rien).
fn enregistrer_classe(nom: PCWSTR, procedure: WNDPROC) -> Result<HINSTANCE, String> {
    // SAFETY : module du programme courant ; curseur prédéfini du système.
    unsafe {
        let instance: HINSTANCE = GetModuleHandleW(PCWSTR::null())
            .map_err(|e| format!("GetModuleHandleW: {e}"))?
            .into();
        let classe = WNDCLASSW {
            lpfnWndProc: procedure,
            hInstance: instance,
            lpszClassName: nom,
            hCursor: LoadCursorW(None, IDC_CROSS).unwrap_or_default(),
            ..Default::default()
        };
        if RegisterClassW(&classe) == 0 {
            let erreur = GetLastError();
            if erreur != ERROR_CLASS_ALREADY_EXISTS {
                return Err(format!("RegisterClassW: erreur {}", erreur.0));
            }
        }
        Ok(instance)
    }
}

/// Coupe le fondu de Windows à l'apparition et à la disparition.
fn sans_fondu(hwnd: HWND) -> Result<(), String> {
    let oui = BOOL(1);
    // SAFETY : un BOOL dont la taille est passée.
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_TRANSITIONS_FORCEDISABLED,
            &oui as *const BOOL as *const core::ffi::c_void,
            std::mem::size_of::<BOOL>() as u32,
        )
    }
    .map_err(|e| format!("DWMWA_TRANSITIONS_FORCEDISABLED: {e}"))
}

/// Procédure de la fenêtre de l'écran figé. Ne panique jamais (une panique ici
/// arrêterait le processus).
unsafe extern "system" fn procedure_ecran_fige(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        // Le fond n'est jamais effacé : l'image couvre tout.
        WM_ERASEBKGND => LRESULT(1),
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            // SAFETY : peinture de cette fenêtre, sur son fil, entre BeginPaint et EndPaint.
            unsafe {
                let dc = BeginPaint(hwnd, &mut ps);
                if !dc.is_invalid() {
                    peindre_image(dc);
                }
                let _ = EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        // SAFETY : traitement par défaut de Windows.
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

/// La fenêtre de l'écran figé et le fil qui l'a créée (le seul qui peut la détruire).
struct FenetreNative {
    hwnd: HWND,
    fil: u32,
}

// SAFETY : depuis un autre fil que le sien, on ne fait sur la poignée que ce que Windows
// permet depuis n'importe quel fil (`IsWindow`, `ShowWindowAsync`, `PostMessageW`) ;
// `DestroyWindow` n'est appelé que sur son fil.
unsafe impl Send for FenetreNative {}

impl Drop for FenetreNative {
    fn drop(&mut self) {
        // SAFETY : poignée de notre fenêtre ; hors de son fil, on poste la fermeture
        // (`WM_CLOSE`, que Windows traite par `DestroyWindow` sur le bon fil).
        unsafe {
            if GetCurrentThreadId() == self.fil {
                let _ = DestroyWindow(self.hwnd);
            } else {
                let _ = PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
    }
}

static ECRAN: Mutex<Option<FenetreNative>> = Mutex::new(None);

/// Poignée et fil de la fenêtre de l'écran figé, copiés hors du verrou.
fn fenetre_figee() -> Option<(HWND, u32)> {
    verrouiller(&ECRAN).as_ref().map(|f| (f.hwnd, f.fil))
}

pub fn preparer_ecran_fige() -> Result<(), String> {
    // SAFETY : sans paramètre.
    let fil = unsafe { GetCurrentThreadId() };
    {
        let mut garde = verrouiller(&ECRAN);
        if let Some(f) = garde.as_ref() {
            // SAFETY : simple vérification, aucun message envoyé.
            if unsafe { IsWindow(Some(f.hwnd)) }.as_bool() {
                return if f.fil == fil {
                    Ok(())
                } else {
                    Err("ecran fige deja cree sur un autre fil".into())
                };
            }
        }
        // Fenêtre disparue (son fil s'est terminé) : on oublie sa poignée sans rien lui
        // envoyer, elle pourrait déjà désigner une autre fenêtre.
        if let Some(morte) = garde.take() {
            std::mem::forget(morte);
        }
    }
    let instance = enregistrer_classe(CLASSE_ECRAN_FIGE, Some(procedure_ecran_fige))?;
    // SAFETY : classe inscrite ci-dessus ; fenêtre sans parent, créée sur ce fil.
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            CLASSE_ECRAN_FIGE,
            w!("BreezyShot"),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(instance),
            None,
        )
    }
    .map_err(|e| format!("CreateWindowExW de l'ecran fige: {e}"))?;
    let nouvelle = FenetreNative { hwnd, fil };
    if let Err(e) = sans_fondu(hwnd) {
        // Sans gravité : l'écran figé apparaîtrait avec le fondu de Windows.
        log::warn!("ecran fige: {e}");
    }
    let ancienne = verrouiller(&ECRAN).replace(nouvelle);
    // Course improbable (deux appels en même temps) : l'autre fenêtre est détruite ici,
    // hors du verrou.
    drop(ancienne);
    Ok(())
}

pub fn montrer_ecran_fige(image: &Image) -> Result<(), String> {
    let (hwnd, fil) = fenetre_figee().ok_or("ecran fige non prepare")?;
    // SAFETY : sans paramètre.
    if unsafe { GetCurrentThreadId() } != fil {
        return Err("montrer_ecran_fige appele hors du fil de la fenetre".into());
    }
    {
        let etat = verrouiller(&IMAGE);
        if !etat
            .photo
            .as_ref()
            .is_some_and(|p| p.numero == image.numero)
        {
            return Err("image remplacee par une photo plus recente, ou rendue".into());
        }
    }
    let r = image.rect;
    // SAFETY : notre fenêtre, sur son fil ; aucun verrou n'est gardé.
    let place = unsafe {
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            r.x,
            r.y,
            r.largeur,
            r.hauteur,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )
    };
    if let Err(e) = place {
        cacher_ecran_fige();
        return Err(format!("SetWindowPos de l'ecran fige: {e}"));
    }
    // Peinture immédiate, sans attendre WM_PAINT.
    let peinte = match DcPrete::prendre(Some(hwnd)) {
        Ok(dc) => peindre_image(dc.dc),
        Err(_) => false,
    };
    if !peinte {
        // Jamais un écran figé vide ou faux : on le cache.
        cacher_ecran_fige();
        return Err("peinture de l'ecran fige impossible".into());
    }
    // SAFETY : notre fenêtre ; vide la file de GDI pour que la composition suive.
    unsafe {
        let _ = ValidateRect(Some(hwnd), None);
        let _ = GdiFlush();
    }
    Ok(())
}

pub fn cacher_ecran_fige() {
    let Some((hwnd, fil)) = fenetre_figee() else {
        return;
    };
    // SAFETY : notre fenêtre. Sur son fil, on la cache tout de suite ; ailleurs, la
    // demande est postée et l'appel n'attend pas (le fil principal peut être occupé).
    unsafe {
        if GetCurrentThreadId() == fil {
            let _ = ShowWindow(hwnd, SW_HIDE);
        } else {
            let _ = ShowWindowAsync(hwnd, SW_HIDE);
        }
    }
}

pub fn detruire_ecran_fige() {
    let fenetre = verrouiller(&ECRAN).take();
    // Hors du verrou : `DestroyWindow` envoie des messages.
    drop(fenetre);
}

pub fn poignee_ecran_fige() -> Option<isize> {
    fenetre_figee().map(|(hwnd, _)| hwnd.0 as isize)
}

// ---------------------------------------------------------------------------------------
// Tests sur l'écran réel (machine Windows de GitHub). Ils se suivent un par un.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;
    use std::time::{Duration, Instant};

    use ::windows::Win32::Foundation::COLORREF;
    use ::windows::Win32::Graphics::Dwm::{
        DwmFlush, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWA_WINDOW_CORNER_PREFERENCE,
        DWMWCP_DONOTROUND,
    };
    use ::windows::Win32::Graphics::Gdi::{CreateSolidBrush, FillRect, GetPixel, UpdateWindow};
    use ::windows::Win32::System::Threading::{GetCurrentProcess, GetGuiResources, GR_GDIOBJECTS};
    use ::windows::Win32::UI::HiDpi::{
        SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    };
    use ::windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetForegroundWindow, GetWindowLongW, PeekMessageW, TranslateMessage,
        GWL_EXSTYLE, MSG, PM_REMOVE, WINDOW_EX_STYLE,
    };

    use crate::systeme::gel as facade;

    /// Style étendu attendu de la fenêtre de l'écran figé.
    const STYLE_ECRAN_FIGE: WINDOW_EX_STYLE =
        WINDOW_EX_STYLE(WS_EX_TOPMOST.0 | WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0);

    static VERROU_ECRAN: Mutex<()> = Mutex::new(());
    static ECHELLE: Once = Once::new();

    /// Un test qui touche à l'écran : un seul à la fois. À la fin, même en échec, l'écran
    /// figé est caché et détruit et l'image rendue (le verrou n'est relâché qu'après).
    struct Seance {
        _verrou: MutexGuard<'static, ()>,
    }

    impl Seance {
        fn ouvrir() -> Seance {
            ECHELLE.call_once(|| {
                // Comme l'app (manifeste) : sinon les coordonnées seraient virtualisées.
                // Un échec veut dire que c'est déjà posé.
                // SAFETY : sans tampon.
                let _ = unsafe {
                    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
                };
            });
            Seance {
                _verrou: verrouiller(&VERROU_ECRAN),
            }
        }
    }

    impl Drop for Seance {
        fn drop(&mut self) {
            cacher_ecran_fige();
            detruire_ecran_fige();
            liberer_image();
            pomper();
        }
    }

    /// Distribue les messages en attente des fenêtres de ce fil.
    fn pomper() {
        let mut msg = MSG::default();
        // SAFETY : file de messages de ce fil.
        unsafe {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    /// Une courte pause qui laisse composer Windows (au plus une image d'écran).
    fn laisser_composer() {
        pomper();
        // SAFETY : sans paramètre ; attend la prochaine composition.
        let _ = unsafe { DwmFlush() };
        std::thread::sleep(Duration::from_millis(16));
        pomper();
    }

    const CLASSE_ESSAI: PCWSTR = w!("BreezyShotEssaiCarres");
    /// Côté d'un carré ; la fenêtre en a quatre (2 × 2).
    const COTE: i32 = 80;
    /// Couleurs pures, exactes même en 16 bits : haut gauche, haut droite, bas gauche,
    /// bas droite (rouge, vert, bleu).
    const COULEURS: [[u8; 3]; 4] = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0]];

    fn couleur_attendue(dx: i32, dy: i32) -> [u8; 3] {
        COULEURS[((dy / COTE) * 2 + dx / COTE) as usize]
    }

    unsafe extern "system" fn procedure_essai(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_ERASEBKGND => LRESULT(1),
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                // SAFETY : peinture de la fenêtre d'essai sur son fil ; chaque pinceau est
                // rendu tout de suite.
                unsafe {
                    let dc = BeginPaint(hwnd, &mut ps);
                    for (i, [r, v, b]) in COULEURS.iter().enumerate() {
                        let i = i as i32;
                        let pinceau = CreateSolidBrush(COLORREF(
                            *r as u32 | (*v as u32) << 8 | (*b as u32) << 16,
                        ));
                        let carre = RECT {
                            left: (i % 2) * COTE,
                            top: (i / 2) * COTE,
                            right: (i % 2 + 1) * COTE,
                            bottom: (i / 2 + 1) * COTE,
                        };
                        FillRect(dc, &carre, pinceau);
                        let _ = DeleteObject(pinceau.into());
                    }
                    let _ = EndPaint(hwnd, &ps);
                }
                LRESULT(0)
            }
            // SAFETY : traitement par défaut.
            _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
        }
    }

    /// La fenêtre d'essai : quatre carrés de couleurs connues, toujours au-dessus, sans
    /// coins arrondis ni fondu. Détruite par `Drop`, même si le test échoue.
    struct FenetreEssai {
        hwnd: HWND,
        rect: Rect,
    }

    impl FenetreEssai {
        fn creer(titre: PCWSTR) -> FenetreEssai {
            let principal = facade::ecrans()
                .into_iter()
                .find(|e| e.principal)
                .expect("un ecran principal");
            let rect = Rect {
                x: principal.zone_utile.x + 60,
                y: principal.zone_utile.y + 60,
                largeur: 2 * COTE,
                hauteur: 2 * COTE,
            };
            let instance = enregistrer_classe(CLASSE_ESSAI, Some(procedure_essai)).unwrap();
            // SAFETY : classe inscrite ; fenêtre de ce fil, détruite par `Drop`.
            unsafe {
                let hwnd = CreateWindowExW(
                    WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    CLASSE_ESSAI,
                    titre,
                    WS_POPUP,
                    rect.x,
                    rect.y,
                    rect.largeur,
                    rect.hauteur,
                    None,
                    None,
                    Some(instance),
                    None,
                )
                .unwrap();
                let essai = FenetreEssai { hwnd, rect };
                let carre = DWMWCP_DONOTROUND;
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_WINDOW_CORNER_PREFERENCE,
                    &carre as *const _ as *const core::ffi::c_void,
                    std::mem::size_of_val(&carre) as u32,
                );
                // Pas de liseré de couleur de Windows 11 autour de la fenêtre.
                let sans_bord = DWMWA_COLOR_NONE;
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_BORDER_COLOR,
                    &sans_bord as *const u32 as *const core::ffi::c_void,
                    std::mem::size_of::<u32>() as u32,
                );
                let _ = sans_fondu(hwnd);
                SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    rect.x,
                    rect.y,
                    rect.largeur,
                    rect.hauteur,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                )
                .unwrap();
                let _ = UpdateWindow(hwnd);
                laisser_composer();
                essai
            }
        }

        fn cacher(&self) {
            // SAFETY : fenêtre de ce fil.
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
            }
            laisser_composer();
        }
    }

    impl Drop for FenetreEssai {
        fn drop(&mut self) {
            // SAFETY : fenêtre de ce fil, détruite une fois.
            unsafe {
                let _ = DestroyWindow(self.hwnd);
            }
            laisser_composer();
        }
    }

    /// Compare des pixels lus aux quatre carrés. `Err` dit le premier écart.
    fn comparer_carres(px: &Pixels, zone: Rect) -> Result<(), String> {
        if px.largeur != zone.largeur as u32 || px.hauteur != zone.hauteur as u32 {
            return Err(format!("taille lue {}x{}", px.largeur, px.hauteur));
        }
        let mut ecarts = 0usize;
        let mut premier = None;
        for dy in 0..zone.hauteur {
            for dx in 0..zone.largeur {
                let i = ((dy * zone.largeur + dx) * 4) as usize;
                let lu = &px.rgba[i..i + 4];
                let [r, v, b] = couleur_attendue(dx, dy);
                if lu != [r, v, b, 255] {
                    ecarts += 1;
                    premier.get_or_insert(format!("({dx}, {dy}) : lu {lu:?}, attendu {r},{v},{b}"));
                }
            }
        }
        match premier {
            None => Ok(()),
            Some(p) => Err(format!("{ecarts} pixels faux, premier {p}")),
        }
    }

    /// Photographie jusqu'à voir les quatre carrés dans `zone` (au plus 2 s).
    fn attendre_carres(zone: Rect) -> Result<Duration, String> {
        let debut = Instant::now();
        let mut dernier = String::new();
        while debut.elapsed() < Duration::from_secs(2) {
            let image = figer_bureau()?;
            match lire(&image, zone).and_then(|px| comparer_carres(&px, zone)) {
                Ok(()) => return Ok(debut.elapsed()),
                Err(e) => dernier = e,
            }
            laisser_composer();
        }
        Err(format!("carres jamais vus en 2 s : {dernier}"))
    }

    /// Couleur d'un pixel de l'écran, lue directement (sans passer par la photo).
    fn pixel_ecran(x: i32, y: i32) -> [u8; 3] {
        let ecran = DcPrete::prendre(None).unwrap();
        // SAFETY : DC de l'écran valide.
        let c = unsafe { GetPixel(ecran.dc, x, y) }.0;
        [
            (c & 0xff) as u8,
            (c >> 8 & 0xff) as u8,
            (c >> 16 & 0xff) as u8,
        ]
    }

    /// Vrai si le centre de chaque carré a sa couleur à l'écran.
    fn carres_a_l_ecran(zone: Rect) -> bool {
        (0..4).all(|i| {
            let (dx, dy) = ((i % 2) * COTE + COTE / 2, (i / 2) * COTE + COTE / 2);
            pixel_ecran(zone.x + dx, zone.y + dy) == couleur_attendue(dx, dy)
        })
    }

    fn attendre(condition: impl Fn() -> bool) -> bool {
        let debut = Instant::now();
        while debut.elapsed() < Duration::from_secs(2) {
            if condition() {
                return true;
            }
            laisser_composer();
        }
        condition()
    }

    #[test]
    fn photo_couvre_le_bureau_virtuel() {
        let _s = Seance::ouvrir();
        let bureau = bureau_virtuel().unwrap();
        // Durées : la première alloue l'image, les suivantes la réutilisent.
        liberer_image();
        let mut durees = Vec::new();
        for _ in 0..10 {
            let t = Instant::now();
            figer_bureau().unwrap();
            durees.push(t.elapsed());
        }
        let premiere = durees[0];
        durees.sort();
        println!(
            "chrono gel photo {}x{} : premiere {:.1} ms, mediane {:.1} ms",
            bureau.largeur,
            bureau.hauteur,
            premiere.as_secs_f64() * 1000.0,
            durees[durees.len() / 2].as_secs_f64() * 1000.0
        );

        let image = figer_bureau().unwrap();
        assert_eq!(rect_image(&image), bureau);
        let zone = Rect {
            x: bureau.x + 3,
            y: bureau.y + 2,
            largeur: 10,
            hauteur: 7,
        };
        let px = lire(&image, zone).unwrap();
        assert_eq!((px.largeur, px.hauteur), (10, 7));
        assert_eq!(px.rgba.len(), 10 * 7 * 4);
        assert!(px.rgba.chunks_exact(4).all(|p| p[3] == 255));

        // Zone à cheval sur le bord : coupée au bureau.
        let cheval = Rect {
            x: bureau.x - 5,
            y: bureau.y - 4,
            largeur: 10,
            hauteur: 10,
        };
        let px = lire(&image, cheval).unwrap();
        assert_eq!((px.largeur, px.hauteur), (5, 6));
        // Zone dehors ou vide : erreur, pas de panique.
        let dehors = Rect {
            x: bureau.x - 50,
            y: bureau.y,
            largeur: 10,
            hauteur: 10,
        };
        assert!(lire(&image, dehors).is_err());
        assert!(lire(&image, Rect { largeur: 0, ..zone }).is_err());

        // Une image dépassée ne se lit plus.
        let suivante = figer_bureau().unwrap();
        assert!(lire(&image, zone).is_err());
        assert!(lire(&suivante, zone).is_ok());
        liberer_image();
        assert!(lire(&suivante, zone).is_err());
    }

    #[test]
    fn photo_au_pixel_pres() {
        let _s = Seance::ouvrir();
        let essai = FenetreEssai::creer(w!("BreezyShot essai pixels"));
        let attente = attendre_carres(essai.rect).unwrap();
        println!(
            "fenetre d'essai vue apres {:.0} ms",
            attente.as_secs_f64() * 1000.0
        );
    }

    #[test]
    fn ecrans_coherents() {
        let _s = Seance::ouvrir();
        let liste = facade::ecrans();
        let bureau = bureau_virtuel().unwrap();
        assert!(!liste.is_empty(), "aucun ecran");
        assert_eq!(liste.iter().filter(|e| e.principal).count(), 1);
        let principal = liste.iter().find(|e| e.principal).unwrap();
        assert_eq!((principal.rect.x, principal.rect.y), (0, 0));
        for e in &liste {
            println!("ecran {:?}", e);
            assert!(
                e.rect.englobe(e.zone_utile),
                "zone utile hors de l'ecran {e:?}"
            );
            assert!(bureau.englobe(e.rect), "ecran hors du bureau {e:?}");
            assert!(e.echelle >= 1.0, "echelle {e:?}");
        }
        let ordre: Vec<(i32, i32)> = liste.iter().map(|e| (e.rect.x, e.rect.y)).collect();
        let mut trie = ordre.clone();
        trie.sort();
        assert_eq!(ordre, trie);
    }

    #[test]
    fn fenetres_visibles_voit_la_fenetre_du_test() {
        let _s = Seance::ouvrir();
        let titre = "BreezyShot essai fenetres";
        let essai = FenetreEssai::creer(w!("BreezyShot essai fenetres"));
        let rect = essai.rect;
        // Sans l'exclusion de BreezyShot (la fenêtre d'essai est de ce processus).
        let toutes = lister_fenetres(None);
        let trouvee = toutes.iter().find(|f| f.titre == titre);
        assert_eq!(trouvee.map(|f| f.rect), Some(rect), "liste : {toutes:?}");
        // Avec l'exclusion : les fenêtres de BreezyShot n'y sont pas.
        assert!(!fenetres_visibles().iter().any(|f| f.titre == titre));
        drop(essai);
        assert!(!lister_fenetres(None).iter().any(|f| f.titre == titre));
    }

    #[test]
    fn ecran_fige_couvre_sans_activer_et_se_cache() {
        let _s = Seance::ouvrir();
        let essai = FenetreEssai::creer(w!("BreezyShot essai ecran fige"));
        let zone = essai.rect;
        attendre_carres(zone).unwrap();

        preparer_ecran_fige().unwrap();
        preparer_ecran_fige().unwrap();
        let (hwnd, _) = fenetre_figee().unwrap();
        assert_eq!(poignee_ecran_fige(), Some(hwnd.0 as isize));
        // SAFETY : lecture du style de notre fenêtre.
        let style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
        assert_eq!(style & STYLE_ECRAN_FIGE.0, STYLE_ECRAN_FIGE.0);
        // SAFETY : sans paramètre.
        assert!(!unsafe { IsWindowVisible(hwnd) }.as_bool(), "creee cachee");

        // La photo contient les carrés ; puis on retire la fenêtre d'essai de l'écran.
        let image = figer_bureau().unwrap();
        essai.cacher();
        assert!(
            attendre(|| !carres_a_l_ecran(zone)),
            "la fenetre d'essai reste a l'ecran"
        );

        // Montrer depuis un autre fil est refusé.
        let ailleurs = std::thread::scope(|s| s.spawn(|| montrer_ecran_fige(&image)).join());
        assert!(matches!(ailleurs, Ok(Err(_))));

        // SAFETY : sans paramètre.
        let premier_plan = unsafe { GetForegroundWindow() };
        montrer_ecran_fige(&image).unwrap();
        pomper();
        // SAFETY : lectures sur notre fenêtre.
        unsafe {
            assert!(IsWindowVisible(hwnd).as_bool());
            let mut r = RECT::default();
            GetWindowRect(hwnd, &mut r).unwrap();
            assert_eq!(rect_de(r), rect_image(&image), "couvre le bureau");
            assert_eq!(GetForegroundWindow(), premier_plan, "focus pris");
        }

        // Preuve que l'image figée est peinte : les carrés reviennent à l'écran, alors que
        // la fenêtre d'essai est cachée.
        attendre_carres(zone).unwrap();
        // SAFETY : sans paramètre.
        assert_eq!(unsafe { GetForegroundWindow() }, premier_plan, "focus pris");

        cacher_ecran_fige();
        // SAFETY : lecture sur notre fenêtre.
        assert!(!unsafe { IsWindowVisible(hwnd) }.as_bool());

        // Cacher depuis un autre fil : la demande est postée, ce fil la distribue.
        let image = figer_bureau().unwrap();
        montrer_ecran_fige(&image).unwrap();
        std::thread::spawn(cacher_ecran_fige).join().unwrap();
        // SAFETY : lecture sur notre fenêtre.
        assert!(attendre(|| !unsafe { IsWindowVisible(hwnd) }.as_bool()));

        detruire_ecran_fige();
        assert_eq!(poignee_ecran_fige(), None);
        // SAFETY : vérification seule.
        assert!(!unsafe { IsWindow(Some(hwnd)) }.as_bool());
        cacher_ecran_fige();
    }

    #[test]
    fn deux_cents_photos_sans_fuite_gdi() {
        let _s = Seance::ouvrir();
        let bureau = bureau_virtuel().unwrap();
        let zone = Rect {
            x: bureau.x,
            y: bureau.y,
            largeur: 64,
            hauteur: 64,
        };
        let objets = || {
            // SAFETY : pseudo-poignée du processus courant.
            unsafe { GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS) }
        };
        // Mise en train : les premiers appels peuvent créer des objets que GDI garde.
        for _ in 0..5 {
            let image = figer_bureau().unwrap();
            lire(&image, zone).unwrap();
        }
        let avant = objets();
        for i in 0..200 {
            if i % 50 == 25 {
                // Passe aussi par la libération et la réallocation de l'image.
                liberer_image();
            }
            let image = figer_bureau().unwrap();
            lire(&image, zone).unwrap();
        }
        let apres = objets();
        println!("objets GDI : avant {avant}, apres 200 photos {apres}");
        assert!(avant > 0, "GetGuiResources a echoue");
        // Une fuite par photo donnerait au moins 200 objets de plus ; la marge couvre un
        // objet créé ailleurs dans le processus pendant ce temps.
        assert!(apres <= avant + 4, "fuite GDI : {avant} -> {apres}");
    }
}
