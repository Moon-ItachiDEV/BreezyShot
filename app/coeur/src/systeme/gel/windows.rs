//! Le Gel sous Windows : GDI, écrans, fenêtres visibles, fenêtre native de l'écran figé.
//!
//! Deux verrous : `IMAGE` (la photo du bureau) et `ECRAN` (la fenêtre native).
//! Règle : on ne garde jamais un verrou pendant un appel qui peut envoyer un message à une
//! fenêtre (`SetWindowPos`, `ShowWindow`, `CreateWindowExW`, `DestroyWindow`…). La
//! procédure de la fenêtre prend `IMAGE` pour peindre : un verrou gardé à ce moment-là
//! bloquerait le fil. Un verrou empoisonné (fil qui a paniqué) est repris, jamais déballé.

use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use ::windows::core::{w, BOOL, PCWSTR};
use ::windows::Win32::Foundation::{
    GetLastError, ERROR_CLASS_ALREADY_EXISTS, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM,
};
use ::windows::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DwmSetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
    DWMWA_TRANSITIONS_FORCEDISABLED, DWMWA_WINDOW_CORNER_PREFERENCE,
};
use ::windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateDIBSection, DeleteDC,
    DeleteObject, EndPaint, EnumDisplayMonitors, GdiFlush, GetDC, GetDIBits, GetMonitorInfoW,
    GetObjectW, ReleaseDC, SelectObject, ValidateRect, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, CAPTUREBLT, DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ, HMONITOR, MONITORINFO, PAINTSTRUCT,
    SRCCOPY,
};
use ::windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use ::windows::Win32::System::RemoteDesktop::{
    WTSRegisterSessionNotification, WTSUnRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION,
};
use ::windows::Win32::System::Threading::{GetCurrentProcessId, GetCurrentThreadId};
use ::windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use ::windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DrawIconEx, EnumWindows, GetClassNameW,
    GetCursorInfo, GetIconInfo, GetSystemMetrics, GetWindowLongW, GetWindowRect, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible, IsZoomed, LoadCursorW,
    PostMessageW, RegisterClassW, SendMessageTimeoutW, SetWindowPos, ShowWindow, ShowWindowAsync,
    CURSORINFO, DI_NORMAL, GWL_EXSTYLE, GWL_STYLE, HICON, HWND_TOPMOST, ICONINFO, IDC_CROSS,
    MA_NOACTIVATE, MONITORINFOF_PRIMARY, SMTO_ABORTIFHUNG, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE, WM_CLOSE,
    WM_DESTROY, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_ERASEBKGND, WM_MOUSEACTIVATE, WM_NULL,
    WM_PAINT, WM_WTSSESSION_CHANGE, WNDCLASSW, WNDPROC, WS_CAPTION, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP, WS_THICKFRAME,
    WTS_SESSION_LOCK,
};

use super::{
    bgra_en_rgba, coins_de, echelle_depuis_dpi, est_classe_du_bureau, garder_fenetre,
    pointeur_montre, taille_pointeur, Changement, Coins, EcranInfo, EtatCoins, EtatFenetre,
    FenetreInfo, Pixels, Pointeur, Rect,
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
pub fn bureau_virtuel() -> Result<Rect, String> {
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
    /// Numéro de la photo que l'écran figé a le droit de peindre. Posé par
    /// `montrer_ecran_fige` sous ce verrou, au moment où il vérifie le numéro ; retiré dès
    /// qu'on demande de cacher l'écran figé (tous les chemins : `cacher_ecran_fige`,
    /// masquage automatique, peinture impossible, `liberer_image`, destruction).
    /// Tant qu'il est posé, `figer_bureau` refuse : l'image affichée n'est jamais remplacée.
    montre: Option<u64>,
}

static IMAGE: Mutex<EtatImage> = Mutex::new(EtatImage {
    photo: None,
    dernier_numero: 0,
    montre: None,
});

/// Retire la marque « montrée » : l'écran figé ne peindra plus aucune photo.
fn retirer_marque() {
    verrouiller(&IMAGE).montre = None;
}

/// Ce que l'appelant garde d'une photo : sa place, son numéro et la durée de la copie.
/// Les pixels restent dans `IMAGE`.
#[derive(Debug)]
pub struct Image {
    rect: Rect,
    numero: u64,
    duree: Duration,
}

pub fn rect_image(image: &Image) -> Rect {
    image.rect
}

pub fn duree_image(image: &Image) -> Duration {
    image.duree
}

/// Copie tout le bureau (`bureau`, lu depuis l'écran `ecran`) dans `dest`, qui a sa taille.
/// Rend la durée du `BitBlt` seul.
fn photographier(dest: &ImageGdi, ecran: HDC, bureau: Rect) -> Result<Duration, String> {
    let debut = Instant::now();
    // Si le bureau a changé de taille depuis la lecture des mesures, la copie reste dans
    // l'image (au pire, une bande noire) ; la photo suivante réallouera.
    // SAFETY : les deux DC sont valides ; la copie tient dans l'image (même taille).
    unsafe {
        BitBlt(
            dest.dc.0,
            0,
            0,
            bureau.largeur,
            bureau.hauteur,
            Some(ecran),
            bureau.x,
            bureau.y,
            SRCCOPY | CAPTUREBLT,
        )
    }
    .map_err(|e| format!("BitBlt du bureau: {e}"))?;
    Ok(debut.elapsed())
}

/// Vrai si l'écran figé est à l'écran (lecture du style, aucun message envoyé).
fn ecran_fige_visible() -> bool {
    fenetre_figee().is_some_and(|(hwnd, _)| {
        // SAFETY : simple lecture, possible depuis n'importe quel fil.
        unsafe { IsWindowVisible(hwnd) }.as_bool()
    })
}

pub fn ecran_fige_montre() -> bool {
    // La marque d'abord (posée avant l'affichage), puis la fenêtre (masquage posté).
    verrouiller(&IMAGE).montre.is_some() || ecran_fige_visible()
}

pub fn figer_bureau() -> Result<Image, String> {
    // Fenêtre encore visible (par exemple un masquage posté depuis un autre fil, pas
    // encore traité) : la photo contiendrait l'écran figé lui-même. On refuse ; le refus
    // dure tant que le fil de la fenêtre n'a pas traité le masquage. C'est le choix sûr :
    // une photo fausse serait pire qu'un refus, et un fil bloqué est l'affaire de la garde.
    if ecran_fige_visible() {
        return Err("ecran fige deja montre".into());
    }
    let bureau = bureau_virtuel()?;
    let ecran = DcPrete::prendre(None)?;
    let mut etat = verrouiller(&IMAGE);
    // Vérifié sous le verrou : aucune course avec `montrer_ecran_fige`.
    if etat.montre.is_some() {
        return Err("ecran fige deja montre".into());
    }
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
    let duree = photographier(&photo.gdi, ecran.dc, bureau)?;
    Ok(Image {
        rect: bureau,
        numero,
        duree,
    })
}

/// Copie brute de `zone` (coordonnées du bureau) depuis `source`, qui couvre `origine`,
/// dans une petite image à part. Seule étape à faire sous le verrou `IMAGE`.
fn copier_zone(source: &ImageGdi, origine: Rect, zone: Rect) -> Result<ImageGdi, String> {
    let coupee = zone
        .intersection(origine)
        .ok_or("zone vide ou hors du bureau")?;
    let tampon = ImageGdi::nouvelle(source.dc.0, coupee.largeur, coupee.hauteur)?;
    // SAFETY : DC valides ; la zone source est dans l'image (coupée à `origine`).
    unsafe {
        BitBlt(
            tampon.dc.0,
            0,
            0,
            coupee.largeur,
            coupee.hauteur,
            Some(source.dc.0),
            coupee.x - origine.x,
            coupee.y - origine.y,
            SRCCOPY,
        )
    }
    .map_err(|e| format!("BitBlt de la zone: {e}"))?;
    Ok(tampon)
}

/// Pixels d'une petite image : `GetDIBits`, puis BGRA -> RGBA (hors de tout verrou).
fn pixels_de(tampon: &ImageGdi) -> Result<Pixels, String> {
    let mut rgba = tampon.lire_tout()?;
    bgra_en_rgba(&mut rgba);
    Ok(Pixels {
        largeur: tampon.largeur as u32,
        hauteur: tampon.hauteur as u32,
        rgba,
    })
}

pub fn lire(image: &Image, zone: Rect) -> Result<Pixels, String> {
    if zone.intersection(image.rect).is_none() {
        return Err("zone vide ou hors du bureau".into());
    }
    let tampon = {
        let etat = verrouiller(&IMAGE);
        let photo = etat
            .photo
            .as_ref()
            .filter(|p| p.numero == image.numero)
            .ok_or("image remplacee par une photo plus recente, ou rendue")?;
        copier_zone(&photo.gdi, image.rect, zone)?
    };
    pixels_de(&tampon)
}

/// Pour les tests : une photo du bureau dans une image à part, qui ne touche ni l'image de
/// l'écran figé ni son numéro. Rendue quand l'appelant la lâche.
#[cfg(test)]
fn photo_a_part() -> Result<(ImageGdi, Rect), String> {
    let bureau = bureau_virtuel()?;
    let ecran = DcPrete::prendre(None)?;
    let gdi = ImageGdi::nouvelle(ecran.dc, bureau.largeur, bureau.hauteur)?;
    photographier(&gdi, ecran.dc, bureau)?;
    Ok((gdi, bureau))
}

/// Pour les tests : lit une zone d'une image à part.
#[cfg(test)]
fn lire_zone(source: &ImageGdi, origine: Rect, zone: Rect) -> Result<Pixels, String> {
    pixels_de(&copier_zone(source, origine, zone)?)
}

pub fn liberer_image() {
    // L'écran figé ne doit jamais rester montré sans son image (retire aussi la marque).
    cacher_ecran_fige();
    let photo = verrouiller(&IMAGE).photo.take();
    drop(photo);
}

/// Peint dans `dc`, à l'origine, la photo marquée « montrée » et elle seule. Faux s'il
/// n'y a rien à peindre (pas de marque, photo remplacée ou rendue) : l'appelant cache.
fn peindre_image(dc: HDC) -> bool {
    let etat = verrouiller(&IMAGE);
    let Some(photo) = etat
        .photo
        .as_ref()
        .filter(|p| etat.montre == Some(p.numero))
    else {
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

/// Le titre, coupé à 511 caractères. Un seul appel, `GetWindowTextW` : pour une fenêtre
/// d'un autre processus, il lit le titre gardé par le système sans lui envoyer de message
/// (rien de tel n'est promis pour `GetWindowTextLengthW`, qui n'est donc pas appelé).
fn titre(hwnd: HWND) -> String {
    let mut tampon = [0u16; 512];
    // SAFETY : tampon dont la longueur est passée.
    let lu = unsafe { GetWindowTextW(hwnd, &mut tampon) };
    let lu = (lu.max(0) as usize).min(tampon.len());
    String::from_utf16_lossy(&tampon[..lu])
}

/// Nom de la classe de la fenêtre (lu sans message).
fn classe(hwnd: HWND) -> String {
    let mut tampon = [0u16; 64];
    // SAFETY : tampon dont la longueur est passée.
    let n = unsafe { GetClassNameW(hwnd, &mut tampon) };
    let n = (n.max(0) as usize).min(tampon.len());
    String::from_utf16_lossy(&tampon[..n])
}

type FonctionAncree = unsafe extern "system" fn(HWND) -> BOOL;

/// `IsWindowArranged`, cherchée une fois dans user32 par `GetProcAddress`, comme le
/// demande Microsoft (pas de bibliothèque d'import ; Windows 10 1903 et après). Un lien
/// direct empêcherait l'app de démarrer sur un Windows qui ne l'a pas.
fn fonction_ancree() -> Option<FonctionAncree> {
    static FONCTION: std::sync::OnceLock<Option<FonctionAncree>> = std::sync::OnceLock::new();
    *FONCTION.get_or_init(|| {
        // SAFETY : user32 est déjà chargé dans un processus graphique ; le nom est une
        // chaîne constante terminée par un zéro.
        unsafe {
            let module = GetModuleHandleW(w!("user32.dll")).ok()?;
            let adresse = GetProcAddress(module, ::windows::core::s!("IsWindowArranged"))?;
            // SAFETY : signature documentée : BOOL IsWindowArranged(HWND).
            Some(std::mem::transmute::<
                unsafe extern "system" fn() -> isize,
                FonctionAncree,
            >(adresse))
        }
    })
}

/// Build de Windows, lue une fois.
fn build_windows() -> Option<u32> {
    static BUILD: std::sync::OnceLock<Option<u32>> = std::sync::OnceLock::new();
    *BUILD.get_or_init(crate::systeme::version_windows)
}

/// `DWMWA_WINDOW_CORNER_PREFERENCE`. Microsoft ne documente cet attribut que pour
/// `DwmSetWindowAttribute` (Windows 11 build 22000) : sa lecture peut échouer, et
/// l'échec vaut « inconnu » (`None`).
fn preference_coins(hwnd: HWND) -> Option<i32> {
    let mut valeur = 0i32;
    // SAFETY : `valeur` est un i32 (taille de DWM_WINDOW_CORNER_PREFERENCE) dont la taille
    // est passée.
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &mut valeur as *mut i32 as *mut core::ffi::c_void,
            std::mem::size_of::<i32>() as u32,
        )
    }
    .ok()
    .map(|()| valeur)
}

/// Forme approchée des coins d'une fenêtre gardée.
fn coins(hwnd: HWND, style: u32) -> Coins {
    // SAFETY : lectures d'état d'une fenêtre, sans message.
    let agrandie = unsafe { IsZoomed(hwnd) }.as_bool();
    // SAFETY : fonction trouvée dans user32, appelée avec sa signature.
    let ancree = fonction_ancree().is_some_and(|f| unsafe { f(hwnd) }.as_bool());
    coins_de(EtatCoins {
        build: build_windows(),
        agrandie,
        ancree,
        preference: preference_coins(hwnd),
        a_un_cadre: style & WS_CAPTION.0 == WS_CAPTION.0 || style & WS_THICKFRAME.0 != 0,
    })
}

fn decrire_fenetre(hwnd: HWND, exclure: Option<u32>) -> Option<FenetreInfo> {
    // SAFETY : `hwnd` vient d'`EnumWindows` ; ces lectures n'envoient aucun message.
    let (visible, reduite, processus, style, style_etendu) = unsafe {
        let mut processus = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut processus));
        (
            IsWindowVisible(hwnd).as_bool(),
            IsIconic(hwnd).as_bool(),
            processus,
            GetWindowLongW(hwnd, GWL_STYLE) as u32,
            GetWindowLongW(hwnd, GWL_EXSTYLE) as u32,
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
    let etat = EtatFenetre {
        visible,
        reduite,
        masquee,
        de_breezyshot,
        calque: style_etendu & WS_EX_LAYERED.0 != 0,
        transparente: style_etendu & WS_EX_TRANSPARENT.0 != 0,
        bureau: est_classe_du_bureau(&classe(hwnd)),
    };
    if !garder_fenetre(etat, rect) {
        return None;
    }
    Some(FenetreInfo {
        rect,
        titre: titre(hwnd),
        coins: coins(hwnd, style),
        fenetre: hwnd.0 as isize,
    })
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
        // Les écrans ont changé : l'image ne correspond plus, l'écran figé se cache.
        WM_DISPLAYCHANGE => {
            cacher_et_prevenir(hwnd, Changement::Ecrans);
            LRESULT(0)
        }
        WM_WTSSESSION_CHANGE => {
            if wparam.0 == WTS_SESSION_LOCK as usize {
                cacher_et_prevenir(hwnd, Changement::SessionVerrouillee);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            // SAFETY : notre fenêtre, sur son fil ; sans effet si elle n'était pas inscrite.
            unsafe {
                let _ = WTSUnRegisterSessionNotification(hwnd);
            }
            let _ = SESSION_SURVEILLEE.compare_exchange(
                hwnd.0 as isize,
                0,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
            LRESULT(0)
        }
        // La fenêtre garde sa taille en pixels réels : Windows ne la redimensionne jamais.
        WM_DPICHANGED => LRESULT(0),
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            // SAFETY : peinture de cette fenêtre, sur son fil, entre BeginPaint et EndPaint.
            let peinte = unsafe {
                let dc = BeginPaint(hwnd, &mut ps);
                let peinte = !dc.is_invalid() && peindre_image(dc);
                let _ = EndPaint(hwnd, &ps);
                peinte
            };
            if !peinte {
                // Plus d'image à montrer : jamais un écran figé vide ou faux.
                retirer_marque();
                // SAFETY : notre fenêtre, sur son fil.
                unsafe {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
            }
            LRESULT(0)
        }
        // SAFETY : traitement par défaut de Windows.
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

pub type RappelChangement = Arc<dyn Fn(Changement) + Send + Sync + 'static>;

static RAPPEL_CHANGEMENT: Mutex<Option<RappelChangement>> = Mutex::new(None);

/// Poignée de la fenêtre inscrite au verrouillage de session (0 : aucune).
static SESSION_SURVEILLEE: AtomicIsize = AtomicIsize::new(0);

pub fn quand_bureau_change(rappel: RappelChangement) {
    let ancien = verrouiller(&RAPPEL_CHANGEMENT).replace(rappel);
    drop(ancien);
}

pub fn surveillance_session() -> bool {
    let inscrite = SESSION_SURVEILLEE.load(Ordering::SeqCst);
    inscrite != 0 && fenetre_figee().is_some_and(|(hwnd, _)| hwnd.0 as isize == inscrite)
}

/// Cache l'écran figé et, s'il était montré, prévient l'Application (sans verrou tenu).
fn cacher_et_prevenir(hwnd: HWND, changement: Changement) {
    retirer_marque();
    // SAFETY : notre fenêtre, sur son fil.
    let etait_visible = unsafe {
        let visible = IsWindowVisible(hwnd).as_bool();
        let _ = ShowWindow(hwnd, SW_HIDE);
        visible
    };
    if !etait_visible {
        return;
    }
    let rappel = verrouiller(&RAPPEL_CHANGEMENT).clone();
    if let Some(rappel) = rappel {
        rappel(changement);
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
    // SAFETY : notre fenêtre ; désinscrite à sa destruction (WM_DESTROY).
    match unsafe { WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) } {
        Ok(()) => SESSION_SURVEILLEE.store(hwnd.0 as isize, Ordering::SeqCst),
        // On continue sans : `surveillance_session()` le dit.
        Err(e) => log::warn!("ecran fige: verrouillage de session non suivi: {e}"),
    }
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
        // Vérification et marque sous le même verrou : dès ici, `figer_bureau` refuse et
        // la fenêtre ne peindra que ce numéro.
        let mut etat = verrouiller(&IMAGE);
        if !etat
            .photo
            .as_ref()
            .is_some_and(|p| p.numero == image.numero)
        {
            return Err("image remplacee par une photo plus recente, ou rendue".into());
        }
        etat.montre = Some(image.numero);
    }
    // La garde surveille avant même l'affichage : un blocage pendant `SetWindowPos` ou
    // la peinture ne passe pas inaperçu.
    reveiller_garde();
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
    reveiller_garde();
    Ok(())
}

pub fn cacher_ecran_fige() {
    // La marque est retirée dès la demande, même postée : la fenêtre ne peindra plus
    // cette photo (une peinture sans marque cache la fenêtre).
    retirer_marque();
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
    retirer_marque();
    let fenetre = verrouiller(&ECRAN).take();
    // Hors du verrou : `DestroyWindow` envoie des messages.
    drop(fenetre);
}

pub fn poignee_ecran_fige() -> Option<isize> {
    fenetre_figee().map(|(hwnd, _)| hwnd.0 as isize)
}

// ---------------------------------------------------------------------------------------
// Une section DIB 32 bits, de haut en bas, sélectionnée dans un DC en mémoire : ses octets
// (BGRA) se lisent et s'écrivent directement. Pointeur (0.4).

struct Dib {
    dc: DcMemoire,
    _bitmap: Bitmap,
    ancien: HGDIOBJ,
    bits: *mut u8,
    largeur: i32,
    hauteur: i32,
}

impl Dib {
    fn nouvelle(largeur: i32, hauteur: i32) -> Result<Dib, String> {
        if largeur <= 0 || hauteur <= 0 {
            return Err(format!("taille d'image invalide ({largeur}x{hauteur})"));
        }
        // SAFETY : DC en mémoire compatible avec l'écran.
        let dc = unsafe { CreateCompatibleDC(None) };
        if dc.is_invalid() {
            return Err("CreateCompatibleDC a echoue".into());
        }
        let dc = DcMemoire(dc);
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: largeur,
                // Hauteur négative : lignes de haut en bas.
                biHeight: -hauteur,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        // SAFETY : `info` décrit une image 32 bits sans table de couleurs ; `bits` reçoit
        // l'adresse des pixels, valide tant que le bitmap existe.
        let bitmap =
            unsafe { CreateDIBSection(Some(dc.0), &info, DIB_RGB_COLORS, &mut bits, None, 0) }
                .map_err(|e| format!("CreateDIBSection {largeur}x{hauteur}: {e}"))?;
        let bitmap = Bitmap(bitmap);
        if bits.is_null() {
            return Err("CreateDIBSection sans pixels".into());
        }
        // SAFETY : DC et bitmap valides, possédés ici.
        let ancien = unsafe { SelectObject(dc.0, bitmap.0.into()) };
        if ancien.is_invalid() {
            return Err("SelectObject de la section DIB a echoue".into());
        }
        Ok(Dib {
            dc,
            _bitmap: bitmap,
            ancien,
            bits: bits.cast(),
            largeur,
            hauteur,
        })
    }

    /// Les octets de l'image, après avoir fini les dessins GDI en attente.
    fn octets(&mut self) -> &mut [u8] {
        // SAFETY : `bits` couvre largeur × hauteur × 4 octets (32 bits, aucune ligne n'est
        // complétée) tant que le bitmap vit ; `&mut self` empêche tout autre accès.
        unsafe {
            let _ = GdiFlush();
            std::slice::from_raw_parts_mut(
                self.bits,
                self.largeur as usize * self.hauteur as usize * 4,
            )
        }
    }
}

impl Drop for Dib {
    fn drop(&mut self) {
        // SAFETY : remet dans le DC l'objet qu'il avait ; puis le DC et le bitmap sont
        // détruits dans cet ordre (ordre des champs), comme pour `ImageGdi`.
        unsafe {
            SelectObject(self.dc.0, self.ancien);
        }
    }
}

// ---------------------------------------------------------------------------------------
// Le pointeur (0.4).

/// Un bitmap rendu par `GetIconInfo`, qui peut être absent (pointeur monochrome).
struct BitmapRecu(HBITMAP);

impl Drop for BitmapRecu {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY : bitmap donné par `GetIconInfo`, à rendre par l'appelant, une fois.
            unsafe {
                let _ = DeleteObject(self.0.into());
            }
        }
    }
}

/// Le côté d'un bitmap (`GetObjectW`), `None` s'il est absent ou illisible.
fn taille_bitmap(bitmap: HBITMAP) -> Option<(i32, i32)> {
    if bitmap.is_invalid() {
        return None;
    }
    let mut b = BITMAP::default();
    // SAFETY : `b` est un BITMAP dont la taille est passée.
    let lu = unsafe {
        GetObjectW(
            bitmap.into(),
            std::mem::size_of::<BITMAP>() as i32,
            Some((&mut b as *mut BITMAP).cast()),
        )
    };
    (lu != 0).then_some((b.bmWidth, b.bmHeight))
}

/// Peint le pointeur sur un fond uni (`fond` : 0 noir, 255 blanc) et rend ses pixels RGBA.
fn peindre_pointeur(dib: &mut Dib, icone: HICON, fond: u8) -> Result<Vec<u8>, String> {
    dib.octets().fill(fond);
    // SAFETY : DC et pointeur valides ; le dessin tient dans l'image (même taille).
    unsafe {
        DrawIconEx(
            dib.dc.0,
            0,
            0,
            icone,
            dib.largeur,
            dib.hauteur,
            0,
            None,
            DI_NORMAL,
        )
    }
    .map_err(|e| format!("DrawIconEx: {e}"))?;
    let mut rgba = dib.octets().to_vec();
    bgra_en_rgba(&mut rgba);
    Ok(rgba)
}

pub fn lire_pointeur() -> Result<Option<Pointeur>, String> {
    let mut info = CURSORINFO {
        cbSize: std::mem::size_of::<CURSORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY : `info` est un CURSORINFO dont la taille est renseignée.
    unsafe { GetCursorInfo(&mut info) }.map_err(|e| format!("GetCursorInfo: {e}"))?;
    if !pointeur_montre(info.flags.0) || info.hCursor.is_invalid() {
        return Ok(None);
    }
    let icone = HICON(info.hCursor.0);
    let mut icone_info = ICONINFO::default();
    // SAFETY : `icone_info` reçoit deux bitmaps, rendus par les gardes ci-dessous.
    unsafe { GetIconInfo(icone, &mut icone_info) }.map_err(|e| format!("GetIconInfo: {e}"))?;
    let masque = BitmapRecu(icone_info.hbmMask);
    let couleurs = BitmapRecu(icone_info.hbmColor);
    let (largeur, hauteur) = taille_bitmap(masque.0)
        .and_then(|m| taille_pointeur(m, taille_bitmap(couleurs.0)))
        .ok_or("taille du pointeur illisible")?;
    let mut dib = Dib::nouvelle(largeur as i32, hauteur as i32)?;
    let sur_noir = peindre_pointeur(&mut dib, icone, 0)?;
    let sur_blanc = peindre_pointeur(&mut dib, icone, 255)?;
    Ok(Some(Pointeur {
        origine: (
            info.ptScreenPos
                .x
                .saturating_sub(icone_info.xHotspot as i32),
            info.ptScreenPos
                .y
                .saturating_sub(icone_info.yHotspot as i32),
        ),
        largeur,
        hauteur,
        sur_noir,
        sur_blanc,
    }))
}

// ---------------------------------------------------------------------------------------
// Fil de garde : si le fil de l'écran figé ne répond plus, prévenir l'Application.

/// Cadence des vérifications pendant que l'écran figé est montré.
const CADENCE_GARDE: Duration = Duration::from_millis(500);
/// Attente maximale d'une réponse à `WM_NULL`, en millisecondes.
const ATTENTE_REPONSE_MS: u32 = 1000;

pub type RappelBlocage = Box<dyn Fn() + Send + 'static>;

struct Reglage {
    delai: Duration,
    au_blocage: Arc<Mutex<RappelBlocage>>,
}

struct EtatGarde {
    reglage: Option<Reglage>,
    /// Compte les affichages de l'écran figé : le fil de garde se réveille quand il change.
    affichages: u64,
    lance: bool,
}

static GARDE: Mutex<EtatGarde> = Mutex::new(EtatGarde {
    reglage: None,
    affichages: 0,
    lance: false,
});
static REVEIL_GARDE: Condvar = Condvar::new();

/// Le réglage actuel, copié hors du verrou.
fn reglage_garde() -> Option<(Duration, Arc<Mutex<RappelBlocage>>)> {
    verrouiller(&GARDE)
        .reglage
        .as_ref()
        .map(|r| (r.delai, Arc::clone(&r.au_blocage)))
}

/// Prévient le fil de garde que l'écran figé vient d'être montré.
fn reveiller_garde() {
    let mut garde = verrouiller(&GARDE);
    garde.affichages = garde.affichages.wrapping_add(1);
    drop(garde);
    REVEIL_GARDE.notify_all();
}

pub fn garder_ecran_fige(delai: Duration, au_blocage: RappelBlocage) {
    let (lancer, vus) = {
        let mut garde = verrouiller(&GARDE);
        garde.reglage = Some(Reglage {
            delai,
            au_blocage: Arc::new(Mutex::new(au_blocage)),
        });
        (!std::mem::replace(&mut garde.lance, true), garde.affichages)
    };
    if !lancer {
        REVEIL_GARDE.notify_all();
        return;
    }
    let lance = std::thread::Builder::new()
        .name("garde-ecran-fige".into())
        .spawn(move || boucle_garde(vus));
    if let Err(e) = lance {
        verrouiller(&GARDE).lance = false;
        log::warn!("fil de garde de l'ecran fige impossible: {e}");
    }
}

/// Dort tant que l'écran figé n'est pas montré ; le surveille pendant qu'il l'est.
/// `vus` : le nombre d'affichages déjà connus au lancement (pris sous le même verrou que
/// le réglage, pour ne pas manquer un affichage pendant le démarrage du fil). Un écran
/// figé déjà montré au lancement est aussi surveillé.
fn boucle_garde(mut vus: u64) {
    loop {
        {
            // Ordre des verrous : GARDE puis ECRAN (jamais l'inverse).
            let mut garde = verrouiller(&GARDE);
            // Sans réglage, on dort sans fin (`garder_ecran_fige` réveille). Avec un réglage,
            // on revérifie aussi la visibilité deux fois par seconde : un affichage dont le
            // réveil serait manqué (fil bloqué au mauvais moment) est quand même surveillé.
            loop {
                if garde.reglage.is_none() {
                    garde = REVEIL_GARDE.wait(garde).unwrap_or_else(|e| e.into_inner());
                    continue;
                }
                if garde.affichages != vus || ecran_fige_visible() {
                    break;
                }
                garde = REVEIL_GARDE
                    .wait_timeout(garde, CADENCE_GARDE)
                    .unwrap_or_else(|e| e.into_inner())
                    .0;
            }
            vus = garde.affichages;
        }
        surveiller_ecran_fige();
    }
}

/// Vérifie toutes les 500 ms que le fil de l'écran figé répond, jusqu'à ce que l'écran
/// figé soit caché. Après `delai` sans réponse, appelle le rappel, une fois par blocage.
fn surveiller_ecran_fige() {
    let mut derniere_reponse = Instant::now();
    let mut signale = false;
    loop {
        let Some((hwnd, _)) = fenetre_figee() else {
            return;
        };
        if !ecran_fige_visible() {
            return;
        }
        let Some((delai, au_blocage)) = reglage_garde() else {
            return;
        };
        let mut resultat = 0usize;
        // SAFETY : `WM_NULL` ne fait rien ; l'appel rend la main après 1 s au plus, ou
        // tout de suite si Windows juge déjà le fil bloqué.
        let repondu = unsafe {
            SendMessageTimeoutW(
                hwnd,
                WM_NULL,
                WPARAM(0),
                LPARAM(0),
                SMTO_ABORTIFHUNG,
                ATTENTE_REPONSE_MS,
                Some(&mut resultat),
            )
        }
        .0 != 0;
        if repondu {
            derniere_reponse = Instant::now();
            signale = false;
        } else if !signale && derniere_reponse.elapsed() >= delai && ecran_fige_visible() {
            signale = true;
            let rappel = au_blocage.lock().unwrap_or_else(|e| e.into_inner());
            rappel();
        }
        std::thread::sleep(CADENCE_GARDE);
    }
}

/// Pour les tests : retire le rappel de garde (le fil de garde ne prévient plus personne).
#[cfg(test)]
fn oublier_garde() {
    verrouiller(&GARDE).reglage = None;
    let ancien = verrouiller(&RAPPEL_CHANGEMENT).take();
    drop(ancien);
}

// ---------------------------------------------------------------------------------------
// Tests sur l'écran réel (machine Windows de GitHub). Ils se suivent un par un.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Once;
    use std::time::{Duration, Instant};

    use ::windows::Win32::Foundation::{COLORREF, POINT};
    use ::windows::Win32::Graphics::Dwm::{
        DwmFlush, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWCP_DEFAULT, DWMWCP_DONOTROUND,
        DWMWCP_ROUNDSMALL, DWM_WINDOW_CORNER_PREFERENCE,
    };
    use ::windows::Win32::Graphics::Gdi::{CreateSolidBrush, FillRect, GetPixel, UpdateWindow};
    use ::windows::Win32::System::Threading::{GetCurrentProcess, GetGuiResources, GR_GDIOBJECTS};
    use ::windows::Win32::UI::HiDpi::{
        SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    };
    use ::windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, FindWindowW, GetCursorPos, GetForegroundWindow, PeekMessageW,
        SendMessageW, SetCursorPos, SetLayeredWindowAttributes, TranslateMessage, LWA_ALPHA, MSG,
        PM_REMOVE, SWP_NOMOVE, SWP_NOSIZE, SW_SHOWMINNOACTIVE, WINDOW_EX_STYLE, WINDOW_STYLE,
        WS_OVERLAPPEDWINDOW, WTS_SESSION_UNLOCK,
    };

    use crate::systeme::gel as facade;

    /// Style étendu attendu de la fenêtre de l'écran figé.
    const STYLE_ECRAN_FIGE: WINDOW_EX_STYLE =
        WINDOW_EX_STYLE(WS_EX_TOPMOST.0 | WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0);

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
                _verrou: verrouiller(&facade::VERROU_ECRAN),
            }
        }
    }

    impl Drop for Seance {
        fn drop(&mut self) {
            oublier_garde();
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
            FenetreEssai::creer_avec(titre, CLASSE_ESSAI, WINDOW_EX_STYLE(0))
        }

        /// Comme `creer`, avec une autre classe et des styles étendus en plus.
        fn creer_avec(
            titre: PCWSTR,
            classe: PCWSTR,
            style_etendu: WINDOW_EX_STYLE,
        ) -> FenetreEssai {
            FenetreEssai::creer_complete(titre, classe, style_etendu, WS_POPUP, DWMWCP_DONOTROUND)
        }

        /// Comme `creer_avec`, avec un autre style et une autre préférence de coins.
        fn creer_complete(
            titre: PCWSTR,
            classe: PCWSTR,
            style_etendu: WINDOW_EX_STYLE,
            style: WINDOW_STYLE,
            coins: DWM_WINDOW_CORNER_PREFERENCE,
        ) -> FenetreEssai {
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
            let instance = enregistrer_classe(classe, Some(procedure_essai)).unwrap();
            // SAFETY : classe inscrite ; fenêtre de ce fil, détruite par `Drop`.
            unsafe {
                let hwnd = CreateWindowExW(
                    WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | style_etendu,
                    classe,
                    titre,
                    style,
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
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_WINDOW_CORNER_PREFERENCE,
                    &coins as *const _ as *const core::ffi::c_void,
                    std::mem::size_of_val(&coins) as u32,
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
    /// `a_part` : photo dans une image à part (permis pendant que l'écran figé est montré),
    /// sinon par `figer_bureau`.
    fn attendre_carres(zone: Rect, a_part: bool) -> Result<Duration, String> {
        let debut = Instant::now();
        let mut dernier = String::new();
        while debut.elapsed() < Duration::from_secs(2) {
            let lu = if a_part {
                photo_a_part().and_then(|(gdi, bureau)| lire_zone(&gdi, bureau, zone))
            } else {
                figer_bureau().and_then(|image| lire(&image, zone))
            };
            match lu.and_then(|px| comparer_carres(&px, zone)) {
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
        let attente = attendre_carres(essai.rect, false).unwrap();
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

    /// Les valeurs recopiées pour la règle pure des coins sont celles de dwmapi.h.
    #[test]
    fn preferences_de_coins_recopiees() {
        use ::windows::Win32::Graphics::Dwm::DWMWCP_ROUND;
        assert_eq!(facade::PREFERENCE_DEFAUT, DWMWCP_DEFAULT.0);
        assert_eq!(facade::PREFERENCE_NE_PAS_ARRONDIR, DWMWCP_DONOTROUND.0);
        assert_eq!(facade::PREFERENCE_ARRONDIR, DWMWCP_ROUND.0);
        assert_eq!(facade::PREFERENCE_ARRONDIR_PEU, DWMWCP_ROUNDSMALL.0);
    }

    #[test]
    fn fenetre_d_essai_vue_avec_son_rectangle_et_ses_coins() {
        let _s = Seance::ouvrir();
        let titre = "BreezyShot essai coins";
        let essai = FenetreEssai::creer(w!("BreezyShot essai coins"));
        let toutes = lister_fenetres(None);
        let trouvee = toutes.iter().find(|f| f.titre == titre);
        // Sans cadre et réglée « ne pas arrondir » : coins droits.
        assert_eq!(
            trouvee.map(|f| (f.rect, f.coins)),
            Some((essai.rect, Coins::Droits)),
            "liste : {toutes:?}"
        );
        println!(
            "build {:?}, IsWindowArranged {}, preference lue {:?}",
            build_windows(),
            if fonction_ancree().is_some() {
                "trouvee"
            } else {
                "absente"
            },
            preference_coins(essai.hwnd)
        );
    }

    #[test]
    fn coins_d_une_fenetre_avec_cadre() {
        let _s = Seance::ouvrir();
        let titre = "BreezyShot essai cadre";
        let essai = FenetreEssai::creer_complete(
            w!("BreezyShot essai cadre"),
            CLASSE_ESSAI,
            WINDOW_EX_STYLE(0),
            WS_OVERLAPPEDWINDOW,
            DWMWCP_DEFAULT,
        );
        let toutes = lister_fenetres(None);
        let coins = toutes.iter().find(|f| f.titre == titre).map(|f| f.coins);
        let attendu = if build_windows().is_some_and(|b| b >= facade::BUILD_WINDOWS_11) {
            Coins::Arrondis
        } else {
            Coins::Droits
        };
        assert_eq!(coins, Some(attendu), "build {:?}", build_windows());
        drop(essai);
    }

    #[test]
    fn coins_d_une_fenetre_reglee_petits_arrondis() {
        let _s = Seance::ouvrir();
        let titre = "BreezyShot essai petits arrondis";
        let essai = FenetreEssai::creer_complete(
            w!("BreezyShot essai petits arrondis"),
            CLASSE_ESSAI,
            WINDOW_EX_STYLE(0),
            WS_OVERLAPPEDWINDOW,
            DWMWCP_ROUNDSMALL,
        );
        let lue = preference_coins(essai.hwnd);
        let windows_11 = build_windows().is_some_and(|b| b >= facade::BUILD_WINDOWS_11);
        if lue.is_some() && windows_11 {
            assert_eq!(lue, Some(DWMWCP_ROUNDSMALL.0));
            let toutes = lister_fenetres(None);
            let coins = toutes.iter().find(|f| f.titre == titre).map(|f| f.coins);
            assert_eq!(coins, Some(Coins::PetitsArrondis));
            println!("preference de coins lue : petits arrondis prouves.");
        } else {
            println!(
                "preference de coins non lue ici ({lue:?}, build {:?}) : petits arrondis NON prouves.",
                build_windows()
            );
        }
        drop(essai);
    }

    #[test]
    fn fenetres_ecartees_reduite_traversee_et_bureau() {
        let _s = Seance::ouvrir();
        let liste_a = |titre: &str| lister_fenetres(None).iter().any(|f| f.titre == titre);

        // Réduite.
        let reduite = FenetreEssai::creer(w!("BreezyShot essai reduite"));
        assert!(
            liste_a("BreezyShot essai reduite"),
            "vue avant d'etre reduite"
        );
        // SAFETY : fenêtre de ce fil.
        unsafe {
            let _ = ShowWindow(reduite.hwnd, SW_SHOWMINNOACTIVE);
        }
        laisser_composer();
        // SAFETY : fenêtre de ce fil.
        assert!(unsafe { IsIconic(reduite.hwnd) }.as_bool(), "pas reduite");
        assert!(decrire_fenetre(reduite.hwnd, None).is_none());
        assert!(!liste_a("BreezyShot essai reduite"));
        drop(reduite);

        // Traversée par les clics.
        let voile = FenetreEssai::creer_avec(
            w!("BreezyShot essai voile"),
            CLASSE_ESSAI,
            WS_EX_LAYERED | WS_EX_TRANSPARENT,
        );
        // Une fenêtre `WS_EX_LAYERED` reste invisible tant qu'on ne pose pas son opacité.
        // SAFETY : fenêtre de ce fil.
        unsafe { SetLayeredWindowAttributes(voile.hwnd, COLORREF(0), 255, LWA_ALPHA) }.unwrap();
        laisser_composer();
        // SAFETY : fenêtre de ce fil.
        assert!(unsafe { IsWindowVisible(voile.hwnd) }.as_bool());
        assert!(decrire_fenetre(voile.hwnd, None).is_none());
        assert!(!liste_a("BreezyShot essai voile"));
        drop(voile);

        // `WS_EX_TRANSPARENT` seul ne rend pas une fenêtre de premier niveau
        // traversable : elle reste proposée.
        let transparente = FenetreEssai::creer_avec(
            w!("BreezyShot essai transparente seule"),
            CLASSE_ESSAI,
            WS_EX_TRANSPARENT,
        );
        assert!(decrire_fenetre(transparente.hwnd, None).is_some());
        assert!(liste_a("BreezyShot essai transparente seule"));
        drop(transparente);

        // Une fenêtre de la classe du bureau (inscrite ici, dans ce processus).
        let faux_bureau = FenetreEssai::creer_avec(
            w!("BreezyShot essai bureau"),
            w!("WorkerW"),
            WINDOW_EX_STYLE(0),
        );
        // SAFETY : fenêtre de ce fil.
        assert!(unsafe { IsWindowVisible(faux_bureau.hwnd) }.as_bool());
        assert!(decrire_fenetre(faux_bureau.hwnd, None).is_none());
        assert!(!liste_a("BreezyShot essai bureau"));
        drop(faux_bureau);

        // Le vrai bureau, s'il existe sur cette machine.
        for nom in [w!("Progman"), w!("WorkerW")] {
            // SAFETY : recherche par nom de classe, sans tampon.
            if let Ok(hwnd) = unsafe { FindWindowW(nom, PCWSTR::null()) } {
                assert!(decrire_fenetre(hwnd, None).is_none(), "{}", classe(hwnd));
            }
        }

        // Témoin : une fenêtre ordinaire est bien vue.
        let temoin = FenetreEssai::creer(w!("BreezyShot essai temoin"));
        assert!(decrire_fenetre(temoin.hwnd, None).is_some());
    }

    #[test]
    fn fenetres_du_premier_plan_vers_le_fond() {
        let _s = Seance::ouvrir();
        let a = FenetreEssai::creer(w!("BreezyShot essai ordre A"));
        let b = FenetreEssai::creer(w!("BreezyShot essai ordre B"));
        let rang = |titre: &str| {
            lister_fenetres(None)
                .iter()
                .position(|f| f.titre == titre)
                .unwrap_or_else(|| panic!("{titre} absente"))
        };
        // B, créée après, est au-dessus de A.
        assert!(rang("BreezyShot essai ordre B") < rang("BreezyShot essai ordre A"));
        // A remonte au-dessus de B.
        // SAFETY : fenêtre de ce fil.
        unsafe {
            SetWindowPos(
                a.hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        }
        .unwrap();
        laisser_composer();
        assert!(rang("BreezyShot essai ordre A") < rang("BreezyShot essai ordre B"));
        drop(b);
    }

    #[test]
    fn ecran_fige_couvre_sans_activer_et_se_cache() {
        let _s = Seance::ouvrir();
        let essai = FenetreEssai::creer(w!("BreezyShot essai ecran fige"));
        let zone = essai.rect;
        attendre_carres(zone, false).unwrap();

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

        // On ne remplace jamais l'image affichée.
        assert!(figer_bureau().is_err(), "photo acceptee ecran fige montre");
        comparer_carres(&lire(&image, zone).unwrap(), zone).unwrap();

        // Preuve que l'image figée est peinte : les carrés reviennent à l'écran, alors que
        // la fenêtre d'essai est cachée (photo dans une image à part).
        attendre_carres(zone, true).unwrap();
        // SAFETY : sans paramètre.
        assert_eq!(unsafe { GetForegroundWindow() }, premier_plan, "focus pris");

        cacher_ecran_fige();
        // SAFETY : lecture sur notre fenêtre.
        assert!(!unsafe { IsWindowVisible(hwnd) }.as_bool());

        // Cacher depuis un autre fil : la demande est postée, ce fil la distribue.
        let image = figer_bureau().unwrap();
        montrer_ecran_fige(&image).unwrap();
        std::thread::spawn(cacher_ecran_fige).join().unwrap();
        // La marque est retirée dès la demande ; la fenêtre, pas encore cachée (ce fil
        // n'a pas distribué le masquage), interdit encore la photo : elle y serait.
        assert_eq!(verrouiller(&IMAGE).montre, None);
        assert!(figer_bureau().is_err());
        // SAFETY : lecture sur notre fenêtre.
        assert!(attendre(|| !unsafe { IsWindowVisible(hwnd) }.as_bool()));

        // Rendre l'image cache d'abord l'écran figé.
        let image = figer_bureau().unwrap();
        montrer_ecran_fige(&image).unwrap();
        liberer_image();
        // SAFETY : lecture sur notre fenêtre.
        assert!(!unsafe { IsWindowVisible(hwnd) }.as_bool());

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

    /// Délai court de la garde dans les tests.
    const DELAI_GARDE: Duration = Duration::from_millis(1500);

    /// Prépare l'écran figé, le montre et rend sa poignée et son image.
    fn montrer_un_ecran_fige() -> (HWND, Image) {
        preparer_ecran_fige().unwrap();
        let image = figer_bureau().unwrap();
        montrer_ecran_fige(&image).unwrap();
        pomper();
        (fenetre_figee().unwrap().0, image)
    }

    /// Distribue les messages de ce fil pendant `duree`, par courtes pauses bornées, sans
    /// `DwmFlush` (qui pourrait attendre une composition sur une machine sans affichage
    /// actif et faire croire la garde à un blocage).
    fn pomper_pendant(duree: Duration) {
        let debut = Instant::now();
        while debut.elapsed() < duree {
            pomper();
            std::thread::sleep(Duration::from_millis(10));
        }
        pomper();
    }

    fn compteur_de_garde() -> Arc<AtomicUsize> {
        let appels = Arc::new(AtomicUsize::new(0));
        let a = Arc::clone(&appels);
        garder_ecran_fige(
            DELAI_GARDE,
            Box::new(move || {
                a.fetch_add(1, Ordering::SeqCst);
            }),
        );
        appels
    }

    #[test]
    fn garde_previent_quand_le_fil_ne_repond_plus() {
        let _s = Seance::ouvrir();
        let appels = compteur_de_garde();
        let (hwnd, _image) = montrer_un_ecran_fige();
        // Le fil du test ne distribue plus ses messages : il est « bloqué ». Avec une
        // vérification toutes les 500 ms et 1 s d'attente chacune, l'alerte tombe vers
        // 2,5 s ; on attend le délai et 2,5 s de marge.
        std::thread::sleep(DELAI_GARDE + Duration::from_millis(2500));
        assert_eq!(appels.load(Ordering::SeqCst), 1, "une alerte, une seule");
        // Il répond de nouveau : pas de nouvelle alerte.
        pomper_pendant(Duration::from_millis(1500));
        assert_eq!(appels.load(Ordering::SeqCst), 1);
        cacher_ecran_fige();
        // SAFETY : lecture sur notre fenêtre.
        assert!(!unsafe { IsWindowVisible(hwnd) }.as_bool());
    }

    #[test]
    fn garde_silencieuse_si_le_fil_repond_ou_si_l_ecran_est_cache() {
        let _s = Seance::ouvrir();
        let appels = compteur_de_garde();
        let (_hwnd, _image) = montrer_un_ecran_fige();
        pomper_pendant(DELAI_GARDE + Duration::from_millis(2000));
        assert_eq!(
            appels.load(Ordering::SeqCst),
            0,
            "alerte alors que le fil repond"
        );
        cacher_ecran_fige();
        std::thread::sleep(DELAI_GARDE + Duration::from_millis(2000));
        assert_eq!(appels.load(Ordering::SeqCst), 0, "alerte ecran cache");
    }

    #[test]
    fn ecran_fige_se_cache_et_previent_quand_le_bureau_change() {
        let _s = Seance::ouvrir();
        let recus: Arc<Mutex<Vec<Changement>>> = Arc::new(Mutex::new(Vec::new()));
        let r = Arc::clone(&recus);
        quand_bureau_change(Arc::new(move |c| verrouiller(&r).push(c)));
        let (hwnd, image) = montrer_un_ecran_fige();
        println!("verrouillage de session suivi : {}", surveillance_session());
        let envoyer = |message: u32, wparam: usize| {
            // SAFETY : message envoyé à notre fenêtre, sur son fil.
            unsafe {
                SendMessageW(hwnd, message, Some(WPARAM(wparam)), Some(LPARAM(0)));
            }
        };
        // SAFETY : lecture sur notre fenêtre.
        let visible = || unsafe { IsWindowVisible(hwnd) }.as_bool();

        envoyer(WM_DISPLAYCHANGE, 32);
        assert!(!visible());
        assert_eq!(*verrouiller(&recus), vec![Changement::Ecrans]);

        montrer_ecran_fige(&image).unwrap();
        envoyer(WM_WTSSESSION_CHANGE, WTS_SESSION_UNLOCK as usize);
        assert!(visible(), "deverrouillage : rien a faire");
        envoyer(WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK as usize);
        assert!(!visible());
        assert_eq!(
            *verrouiller(&recus),
            vec![Changement::Ecrans, Changement::SessionVerrouillee]
        );

        // Écran figé caché : aucun rappel.
        envoyer(WM_DISPLAYCHANGE, 32);
        envoyer(WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK as usize);
        assert_eq!(verrouiller(&recus).len(), 2);
    }

    // --- Pointeur (0.4) ---

    fn objets_gdi() -> u32 {
        // SAFETY : pseudo-poignée du processus courant.
        unsafe { GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS) }
    }

    #[test]
    fn drapeaux_du_pointeur_recopies() {
        use ::windows::Win32::UI::WindowsAndMessaging::{CURSOR_SHOWING, CURSOR_SUPPRESSED};
        assert_eq!(facade::POINTEUR_MONTRE, CURSOR_SHOWING.0);
        assert_eq!(facade::POINTEUR_SUPPRIME, CURSOR_SUPPRESSED.0);
    }

    /// Un vrai mouvement de souris (1 px puis retour), qui fait montrer le pointeur par
    /// Windows quand il était caché faute de mouvement.
    fn bouger_la_souris() {
        use ::windows::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_MOVE, MOUSEINPUT,
        };
        let mouvement = |dx: i32| INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx,
                    dy: 0,
                    dwFlags: MOUSEEVENTF_MOVE,
                    ..Default::default()
                },
            },
        };
        let entrees = [mouvement(1), mouvement(-1)];
        // SAFETY : deux entrées dont la taille est passée.
        unsafe {
            SendInput(&entrees, std::mem::size_of::<INPUT>() as i32);
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    fn drapeaux_du_pointeur() -> u32 {
        let mut info = CURSORINFO {
            cbSize: std::mem::size_of::<CURSORINFO>() as u32,
            ..Default::default()
        };
        // SAFETY : CURSORINFO dont la taille est renseignée.
        unsafe { GetCursorInfo(&mut info) }.unwrap();
        info.flags.0
    }

    /// Le pointeur est d'abord rendu visible (pointe posée, vrai mouvement). S'il reste
    /// caché (session sans souris), le test vérifie ce qui reste vrai (`Ok(None)`, aucune
    /// fuite) et le dit ; la preuve du pointeur dans l'image appartient à l'essai 11.
    #[test]
    fn pointeur_lu_sans_fuite_gdi() {
        let _s = Seance::ouvrir();
        let bureau = bureau_virtuel().unwrap();
        // SAFETY : sans tampon.
        let _ =
            unsafe { SetCursorPos(bureau.x + bureau.largeur / 2, bureau.y + bureau.hauteur / 2) };
        bouger_la_souris();
        let drapeaux = drapeaux_du_pointeur();
        let avant_lecture = |n: usize| {
            for _ in 0..n {
                let _ = lire_pointeur().unwrap();
            }
        };
        if !pointeur_montre(drapeaux) {
            println!(
                "pointeur non montre sur cette machine (drapeaux {drapeaux:#x}) : seules \
                 l'absence de pointeur et l'absence de fuite sont verifiees"
            );
            assert_eq!(lire_pointeur(), Ok(None));
            avant_lecture(5);
            let avant = objets_gdi();
            avant_lecture(200);
            let apres = objets_gdi();
            assert!(avant > 0, "GetGuiResources a echoue");
            assert!(apres <= avant + 4, "fuite GDI : {avant} -> {apres}");
            return;
        }
        let p = lire_pointeur()
            .unwrap()
            .expect("pointeur montre mais non lu");
        assert!(p.largeur > 0 && p.hauteur > 0);
        let taille = (p.largeur * p.hauteur * 4) as usize;
        assert_eq!(p.sur_noir.len(), taille);
        assert_eq!(p.sur_blanc.len(), taille);
        // La pointe (point chaud) est dans l'image du pointeur.
        let mut pointe = POINT::default();
        // SAFETY : un POINT en sortie.
        unsafe { GetCursorPos(&mut pointe) }.unwrap();
        let rect = Rect {
            x: p.origine.0,
            y: p.origine.1,
            largeur: p.largeur as i32,
            hauteur: p.hauteur as i32,
        };
        assert!(rect.contient((pointe.x, pointe.y)), "{rect:?} {pointe:?}");
        // Quelque chose est peint : au moins un pixel n'est pas transparent.
        let peint = (0..taille / 4).any(|i| {
            let n = &p.sur_noir[i * 4..i * 4 + 3];
            let b = &p.sur_blanc[i * 4..i * 4 + 3];
            n != [0, 0, 0] || b != [255, 255, 255]
        });
        assert!(peint, "pointeur entierement transparent");
        avant_lecture(5);
        let avant = objets_gdi();
        avant_lecture(200);
        let apres = objets_gdi();
        println!("objets GDI : avant {avant}, apres 200 pointeurs {apres}");
        assert!(avant > 0, "GetGuiResources a echoue");
        assert!(apres <= avant + 4, "fuite GDI : {avant} -> {apres}");
    }
}
