//! Images vivantes sous Windows (chantier A, 0.6) : `BitBlt` du DC de l'écran sur le seul
//! rectangle de la zone, `SRCCOPY` sans `CAPTUREBLT`, dans une section DIB réutilisée (R2).
//!
//! La prise garde un DC en mémoire et sa section DIB, de la taille de la zone, de
//! `ouvrir` jusqu'à sa destruction : une prise n'alloue rien (ni objet GDI, ni mémoire,
//! sauf la première fois pour une destination vide). Le DC de l'écran est prêté par
//! `GetDC` le temps d'une prise, comme pour la photo du Gel.
//!
//! Garde de l'écran figé : `gel::ecran_fige_montre` (la garde de `gel::figer_bureau` :
//! marque « montré » ou fenêtre encore visible), lue avant ET après la copie, celle-ci
//! vidée par `GdiFlush` avant la seconde lecture.

use std::time::{Duration, Instant};

use ::windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GdiFlush, GetDC,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC,
    HGDIOBJ, SRCCOPY,
};

use super::{couper_au_bureau, preparer_destination, Pixels, Rect};
use crate::systeme::gel;

pub fn disponible() -> bool {
    true
}

// ---------------------------------------------------------------------------------------
// Ressources GDI : chaque poignée a un propriétaire qui la rend dans son `Drop`.

/// Le DC de l'écran prêté par `GetDC(None)`, rendu par `ReleaseDC`.
struct DcEcran(HDC);

impl DcEcran {
    fn prendre() -> Result<DcEcran, String> {
        // SAFETY : DC de tout l'écran.
        let dc = unsafe { GetDC(None) };
        if dc.is_invalid() {
            return Err("GetDC a echoue".into());
        }
        Ok(DcEcran(dc))
    }
}

impl Drop for DcEcran {
    fn drop(&mut self) {
        // SAFETY : DC obtenu par `GetDC(None)`.
        unsafe {
            ReleaseDC(None, self.0);
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

/// La section DIB, détruite par `DeleteObject`.
struct Bitmap(HBITMAP);

impl Drop for Bitmap {
    fn drop(&mut self) {
        // SAFETY : bitmap créé par `CreateDIBSection`, détruit une seule fois, après avoir
        // été retiré de son DC (voir `Dib::drop`).
        unsafe {
            let _ = DeleteObject(self.0.into());
        }
    }
}

/// Une section DIB 32 bits, de haut en bas, sélectionnée dans un DC en mémoire : ses
/// octets (BGRA) se lisent directement, sans `GetDIBits`.
///
/// Ordre de destruction : `drop` remet l'ancien objet dans le DC, puis les champs sont
/// détruits dans l'ordre : le DC, puis le bitmap (comme `gel`).
struct Dib {
    dc: DcMemoire,
    _bitmap: Bitmap,
    ancien: HGDIOBJ,
    bits: *const u8,
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
            bits: bits.cast_const().cast(),
            largeur,
            hauteur,
        })
    }

    /// Les octets de l'image (BGRA), après avoir fini les dessins GDI en attente.
    fn octets(&self) -> &[u8] {
        // SAFETY : `bits` couvre largeur × hauteur × 4 octets (32 bits, aucune ligne n'est
        // complétée) tant que le bitmap vit ; seul GDI y écrit, pendant `BitBlt`, qui
        // demande `&mut Prise` : aucune écriture pendant cet emprunt.
        unsafe {
            let _ = GdiFlush();
            std::slice::from_raw_parts(self.bits, self.largeur as usize * self.hauteur as usize * 4)
        }
    }
}

impl Drop for Dib {
    fn drop(&mut self) {
        // SAFETY : remet dans le DC l'objet qu'il avait à sa création.
        unsafe {
            SelectObject(self.dc.0, self.ancien);
        }
    }
}

// ---------------------------------------------------------------------------------------
// La prise.

/// Le tampon de la prise : un DC en mémoire et sa section DIB, de la taille de la zone.
pub struct Prise {
    zone: Rect,
    dib: Dib,
}

// SAFETY : les objets GDI ne sont pas liés au fil qui les a créés ; `&mut self` sur
// `prendre` garantit qu'un seul fil s'en sert à la fois. `Prise` n'est pas `Sync`.
unsafe impl Send for Prise {}

impl std::fmt::Debug for Prise {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prise").field("zone", &self.zone).finish()
    }
}

pub fn ouvrir(zone: Rect) -> Result<Prise, String> {
    let zone = couper_au_bureau(zone, gel::bureau_virtuel()?)?;
    let dib = Dib::nouvelle(zone.largeur, zone.hauteur)?;
    Ok(Prise { zone, dib })
}

pub fn zone(prise: &Prise) -> Rect {
    prise.zone
}

pub fn prendre(prise: &mut Prise, dest: &mut Pixels) -> Result<Duration, String> {
    prendre_garde(prise, dest, &mut gel::ecran_fige_montre)
}

/// `prendre`, la garde de l'écran figé donnée par l'appelant (les tests la simulent).
fn prendre_garde(
    prise: &mut Prise,
    dest: &mut Pixels,
    ecran_fige_montre: &mut dyn FnMut() -> bool,
) -> Result<Duration, String> {
    if ecran_fige_montre() {
        return Err("ecran fige montre".into());
    }
    let debut = Instant::now();
    let zone = prise.zone;
    {
        let ecran = DcEcran::prendre()?;
        // Si le bureau a rétréci depuis `ouvrir`, la partie dehors reste noire.
        // SAFETY : les deux DC sont valides ; la copie tient dans la section DIB (même
        // taille que la zone). Pas de `CAPTUREBLT` (R2).
        unsafe {
            BitBlt(
                prise.dib.dc.0,
                0,
                0,
                zone.largeur,
                zone.hauteur,
                Some(ecran.0),
                zone.x,
                zone.y,
                SRCCOPY,
            )
        }
        .map_err(|e| format!("BitBlt de la zone: {e}"))?;
        // GDI peut mettre la copie en lot : elle est faite ici, avant de rendre le DC de
        // l'écran et avant la seconde lecture de la garde, qui l'encadre donc vraiment.
        // SAFETY : sans paramètre ; vide le lot de ce fil.
        unsafe {
            let _ = GdiFlush();
        }
    }
    // L'écran figé a pu paraître pendant la copie : l'image serait la sienne.
    if ecran_fige_montre() {
        return Err("ecran fige montre".into());
    }
    let octets = preparer_destination(dest, zone.largeur as u32, zone.hauteur as u32)?;
    let source = prise.dib.octets();
    if source.len() != octets {
        return Err("taille de la section DIB inattendue".into());
    }
    dest.rgba.copy_from_slice(source);
    // Même ordre des canaux que le reste de la capture : RGBA, alpha 255.
    gel::bgra_en_rgba(&mut dest.rgba);
    Ok(debut.elapsed())
}

#[cfg(test)]
mod tests {
    //! Tests Windows. Un seul à la fois avec ceux de `gel` (`Seance`, verrou partagé
    //! `gel::VERROU_ECRAN`). Ceux qui montrent l'écran figé ou comptent les objets GDI se
    //! jouent en plus dans un processus à part (`dans_un_processus_a_part`), le parent
    //! tenant le verrou partagé pendant que l'enfant tourne : le compte des objets GDI n'y
    //! voit que la prise, et l'écran figé de l'enfant ne paraît jamais pendant un test de
    //! `gel`.

    use super::*;
    use std::sync::{MutexGuard, Once};

    use ::windows::core::{w, PCWSTR};
    use ::windows::Win32::Foundation::{
        GetLastError, SetLastError, COLORREF, ERROR_CLASS_ALREADY_EXISTS, ERROR_SUCCESS, HINSTANCE,
        HWND, LPARAM, LRESULT, RECT, WPARAM,
    };
    use ::windows::Win32::Graphics::Dwm::{
        DwmFlush, DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE,
        DWMWA_TRANSITIONS_FORCEDISABLED, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
    };
    use ::windows::Win32::Graphics::Gdi::{
        BeginPaint, CreateSolidBrush, EndPaint, FillRect, UpdateWindow, PAINTSTRUCT,
    };
    use ::windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use ::windows::Win32::System::Threading::{GetCurrentProcess, GetGuiResources, GR_GDIOBJECTS};
    use ::windows::Win32::UI::HiDpi::{
        SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    };
    use ::windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetWindowDisplayAffinity,
        IsWindowVisible, PeekMessageW, RegisterClassW, SetLayeredWindowAttributes,
        SetWindowDisplayAffinity, SetWindowPos, ShowWindow, TranslateMessage, HWND_TOPMOST,
        LWA_ALPHA, MSG, PM_REMOVE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE, WDA_EXCLUDEFROMCAPTURE,
        WINDOW_EX_STYLE, WM_ERASEBKGND, WM_PAINT, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
    };

    use crate::systeme::vivant as facade;

    static ECHELLE: Once = Once::new();

    /// Un test qui touche à l'écran : un seul à la fois, avec ceux de `gel` (même verrou).
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
                _verrou: gel::VERROU_ECRAN.lock().unwrap_or_else(|e| e.into_inner()),
            }
        }
    }

    impl Drop for Seance {
        fn drop(&mut self) {
            pomper();
        }
    }

    /// Dans le processus à part seulement : à la fin, même en échec, l'écran figé est
    /// caché et détruit et son image rendue.
    struct EcranFigeRendu;

    impl Drop for EcranFigeRendu {
        fn drop(&mut self) {
            gel::cacher_ecran_fige();
            gel::detruire_ecran_fige();
            gel::liberer_image();
            pomper();
        }
    }

    /// Variable posée pour le processus à part : sans elle, un test « enfant » ne fait rien.
    const ENFANT: &str = "BREEZYSHOT_VIVANT_ENFANT";

    /// Joue le test ignoré `nom` de ce module dans un processus à part (le même exécutable
    /// de tests), relaie sa sortie et échoue s'il échoue ou s'il n'a pas été joué.
    fn dans_un_processus_a_part(nom: &str) {
        let module = module_path!()
            .split_once("::")
            .map(|(_, reste)| reste)
            .unwrap_or(module_path!());
        let chemin = format!("{module}::{nom}");
        let sortie = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                chemin.as_str(),
                "--exact",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(ENFANT, "1")
            .output()
            .unwrap();
        let texte = String::from_utf8_lossy(&sortie.stdout);
        let erreurs = String::from_utf8_lossy(&sortie.stderr);
        eprintln!("--- {chemin} (processus a part) ---\n{texte}{erreurs}---");
        assert!(sortie.status.success(), "{chemin} a echoue");
        assert!(texte.contains("1 passed"), "{chemin} non joue");
    }

    /// Vrai dans le processus à part ; sinon le dit (test lancé à la main par `--ignored`).
    fn dans_l_enfant() -> bool {
        let oui = std::env::var_os(ENFANT).is_some();
        if !oui {
            eprintln!("a jouer par son test parent (processus a part) : rien fait");
        }
        oui
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

    /// Objets GDI du processus. Zéro est un vrai compte (processus de tests sans fenêtre,
    /// prises rendues) : seul un zéro avec une erreur Windows fait échouer.
    fn objets_gdi() -> u32 {
        // SAFETY : pseudo-poignée du processus courant ; l'erreur est remise à zéro avant.
        unsafe {
            SetLastError(ERROR_SUCCESS);
            let n = GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS);
            if n == 0 {
                let erreur = GetLastError();
                assert_eq!(
                    erreur, ERROR_SUCCESS,
                    "GetGuiResources a echoue : {erreur:?}"
                );
            }
            n
        }
    }

    fn vide() -> Pixels {
        Pixels {
            largeur: 0,
            hauteur: 0,
            rgba: Vec::new(),
        }
    }

    fn mediane(mut durees: Vec<Duration>) -> f64 {
        durees.sort();
        durees[durees.len() / 2].as_secs_f64() * 1000.0
    }

    /// Une prise, réessayée au plus 2 s (filet : sous le verrou partagé, aucun test de `gel`
    /// ne montre l'écran figé, mais une autre app de la machine pourrait gêner un instant).
    fn prendre_patient(prise: &mut facade::Prise, dest: &mut Pixels) -> Duration {
        let debut = Instant::now();
        loop {
            match prise.prendre(dest) {
                Ok(d) => return d,
                Err(e) => {
                    assert!(debut.elapsed() < Duration::from_secs(2), "prise : {e}");
                    laisser_composer();
                }
            }
        }
    }

    // --- Fenêtres d'essai ---

    const CLASSE_CARRES: PCWSTR = w!("BreezyShotEssaiVivantCarres");
    const CLASSE_VOILE: PCWSTR = w!("BreezyShotEssaiVivantVoile");
    /// Côté d'un carré ; la fenêtre en a quatre (2 × 2).
    const COTE: i32 = 80;
    /// Couleurs pures, exactes même en 16 bits : haut gauche, haut droite, bas gauche,
    /// bas droite (rouge, vert, bleu, jaune).
    const COULEURS: [[u8; 3]; 4] = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0]];
    /// Couleur de la fenêtre posée par-dessus (aucun carré ne l'a).
    const VOILE: [u8; 3] = [255, 0, 255];
    /// Décalage depuis le coin haut gauche de la zone utile : en haut à gauche, à côté des
    /// fenêtres d'essai de `gel` (60, 60, 160 × 160) et loin de celles des tests de
    /// `defiler` (en bas à droite) ; la fenêtre et son voile tiennent dans 160 × 160.
    const DECALAGE: (i32, i32) = (260, 40);

    fn couleur_attendue(dx: i32, dy: i32) -> [u8; 3] {
        COULEURS[((dy / COTE) * 2 + dx / COTE) as usize]
    }

    fn colorref([r, v, b]: [u8; 3]) -> COLORREF {
        COLORREF(r as u32 | (v as u32) << 8 | (b as u32) << 16)
    }

    unsafe extern "system" fn procedure_carres(
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
                    for (i, couleur) in COULEURS.iter().enumerate() {
                        let i = i as i32;
                        let pinceau = CreateSolidBrush(colorref(*couleur));
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

    unsafe extern "system" fn procedure_voile(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_ERASEBKGND => LRESULT(1),
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                // SAFETY : peinture de la fenêtre d'essai sur son fil.
                unsafe {
                    let dc = BeginPaint(hwnd, &mut ps);
                    let pinceau = CreateSolidBrush(colorref(VOILE));
                    FillRect(dc, &ps.rcPaint, pinceau);
                    let _ = DeleteObject(pinceau.into());
                    let _ = EndPaint(hwnd, &ps);
                }
                LRESULT(0)
            }
            // SAFETY : traitement par défaut.
            _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
        }
    }

    fn enregistrer_classe(
        nom: PCWSTR,
        procedure: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
    ) -> HINSTANCE {
        // SAFETY : module du programme courant.
        unsafe {
            let instance: HINSTANCE = GetModuleHandleW(PCWSTR::null()).unwrap().into();
            let classe = WNDCLASSW {
                lpfnWndProc: Some(procedure),
                hInstance: instance,
                lpszClassName: nom,
                ..Default::default()
            };
            if RegisterClassW(&classe) == 0 {
                let erreur = GetLastError();
                assert_eq!(erreur, ERROR_CLASS_ALREADY_EXISTS, "RegisterClassW");
            }
            instance
        }
    }

    /// Une fenêtre d'essai toujours au-dessus, sans activation, sans coins arrondis, sans
    /// liseré ni fondu. Détruite par `Drop`, même si le test échoue.
    struct FenetreEssai {
        hwnd: HWND,
        rect: Rect,
    }

    impl FenetreEssai {
        fn creer(
            titre: PCWSTR,
            classe: PCWSTR,
            procedure: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
            style_etendu: WINDOW_EX_STYLE,
            rect: Rect,
        ) -> FenetreEssai {
            let instance = enregistrer_classe(classe, procedure);
            // SAFETY : classe inscrite ; fenêtre de ce fil, détruite par `Drop`.
            unsafe {
                let hwnd = CreateWindowExW(
                    WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | style_etendu,
                    classe,
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
                let coins = DWMWCP_DONOTROUND;
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_WINDOW_CORNER_PREFERENCE,
                    &coins as *const _ as *const core::ffi::c_void,
                    std::mem::size_of_val(&coins) as u32,
                );
                let sans_bord = DWMWA_COLOR_NONE;
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_BORDER_COLOR,
                    &sans_bord as *const u32 as *const core::ffi::c_void,
                    std::mem::size_of::<u32>() as u32,
                );
                let oui = ::windows::core::BOOL(1);
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_TRANSITIONS_FORCEDISABLED,
                    &oui as *const _ as *const core::ffi::c_void,
                    std::mem::size_of_val(&oui) as u32,
                );
                if style_etendu.0 & WS_EX_LAYERED.0 != 0 {
                    // Calque opaque : comme une fenêtre de contrôle native transparente.
                    SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA).unwrap();
                }
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

        /// Les quatre carrés, sur l'écran principal.
        fn carres() -> FenetreEssai {
            let principal = gel::ecrans()
                .into_iter()
                .find(|e| e.principal)
                .expect("un ecran principal");
            let rect = Rect {
                x: principal.zone_utile.x + DECALAGE.0,
                y: principal.zone_utile.y + DECALAGE.1,
                largeur: 2 * COTE,
                hauteur: 2 * COTE,
            };
            FenetreEssai::creer(
                w!("BreezyShot essai vivant"),
                CLASSE_CARRES,
                procedure_carres,
                WINDOW_EX_STYLE(0),
                rect,
            )
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

    /// Nombre de pixels qui ne sont pas les quatre carrés ; `Err` si la taille est fausse.
    /// Avec le premier écart, pour le message.
    fn ecarts_aux_carres(px: &Pixels, zone: Rect) -> Result<(usize, Option<String>), String> {
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
        Ok((ecarts, premier))
    }

    /// Prend jusqu'à voir les quatre carrés (au plus 2 s : une fenêtre d'un autre test peut
    /// passer). Rend le temps d'attente.
    fn attendre_carres(prise: &mut facade::Prise, dest: &mut Pixels) -> Result<Duration, String> {
        let debut = Instant::now();
        let mut dernier = String::new();
        while debut.elapsed() < Duration::from_secs(2) {
            match prise.prendre(dest) {
                Ok(_) => match ecarts_aux_carres(dest, prise.zone()) {
                    Ok((0, _)) => return Ok(debut.elapsed()),
                    Ok((n, premier)) => {
                        dernier = format!("{n} pixels faux, premier {}", premier.unwrap())
                    }
                    Err(e) => dernier = e,
                },
                Err(e) => dernier = e,
            }
            laisser_composer();
        }
        Err(format!("carres jamais vus en 2 s : {dernier}"))
    }

    // --- Tests ---

    #[test]
    fn ouvrir_coupe_au_bureau_et_refuse_le_vide() {
        let _s = Seance::ouvrir();
        let bureau = gel::bureau_virtuel().unwrap();
        let cheval = Rect {
            x: bureau.x - 5,
            y: bureau.y - 4,
            largeur: 10,
            hauteur: 10,
        };
        let mut prise = facade::ouvrir(cheval).unwrap();
        assert_eq!(
            prise.zone(),
            Rect {
                x: bureau.x,
                y: bureau.y,
                largeur: 5,
                hauteur: 6
            }
        );
        let mut px = vide();
        prendre_patient(&mut prise, &mut px);
        assert_eq!((px.largeur, px.hauteur), (5, 6));
        assert_eq!(px.rgba.len(), 5 * 6 * 4);
        assert!(px.rgba.chunks_exact(4).all(|p| p[3] == 255), "alpha 255");
        let dehors = Rect {
            x: bureau.x - 50,
            ..cheval
        };
        assert!(facade::ouvrir(dehors).is_err());
        assert!(facade::ouvrir(Rect {
            largeur: 0,
            ..cheval
        })
        .is_err());
    }

    /// Au pixel près, sur la fenêtre des quatre carrés ; la destination est réutilisée
    /// (même tampon d'une prise à l'autre) ; durée notée.
    #[test]
    fn prise_au_pixel_pres() {
        let _s = Seance::ouvrir();
        let essai = FenetreEssai::carres();
        let mut prise = facade::ouvrir(essai.rect).unwrap();
        assert_eq!(prise.zone(), essai.rect);
        let mut px = vide();
        let attente = attendre_carres(&mut prise, &mut px).unwrap();
        eprintln!(
            "vivant: carres vus apres {:.0} ms",
            attente.as_secs_f64() * 1000.0
        );
        let adresse = px.rgba.as_ptr();
        let capacite = px.rgba.capacity();
        let mut durees = Vec::new();
        for _ in 0..32 {
            durees.push(prendre_patient(&mut prise, &mut px));
            assert_eq!(px.rgba.as_ptr(), adresse, "destination reallouee");
            assert_eq!(px.rgba.capacity(), capacite);
        }
        // Les dernières prises sont encore les carrés, au pixel près.
        let (n, premier) = ecarts_aux_carres(&px, prise.zone()).unwrap();
        assert_eq!(n, 0, "{n} pixels faux, premier {premier:?}");
        eprintln!(
            "vivant: prise {}x{} : mediane {:.1} ms sur 32 (construction de test ; le budget de 40 ms se mesure en release, essai 14)",
            essai.rect.largeur,
            essai.rect.hauteur,
            mediane(durees)
        );

        // La prise suit l'écran : fenêtre cachée, les carrés ne sont plus là.
        essai.cacher();
        let debut = Instant::now();
        let partie = loop {
            prendre_patient(&mut prise, &mut px);
            let (n, _) = ecarts_aux_carres(&px, prise.zone()).unwrap();
            if n > 0 || debut.elapsed() > Duration::from_secs(2) {
                break n;
            }
            laisser_composer();
        };
        assert!(
            partie > 0,
            "image figee : les carres restent apres la fenetre"
        );
    }

    /// Note, sans échouer, ce que laisse dans l'image une fenêtre NATIVE exclue des
    /// captures (`WDA_EXCLUDEFROMCAPTURE`) posée sur les carrés : la sonde (S2 (a)) en a vu
    /// 138 072 pixels en `BitBlt`, alors qu'une WebView2 exclue n'en laisse aucun (R2). Deux
    /// variantes : une fenêtre opaque simple et une fenêtre en calque traversée par les
    /// clics (comme une fenêtre de contrôle native). Le compte sans l'exclusion est noté
    /// aussi : il prouve que la fenêtre est bien à l'écran.
    #[test]
    fn fenetre_native_exclue_notee() {
        let _s = Seance::ouvrir();
        let essai = FenetreEssai::carres();
        let mut prise = facade::ouvrir(essai.rect).unwrap();
        let mut px = vide();
        attendre_carres(&mut prise, &mut px).unwrap();
        // Le voile couvre le centre : un quart de chaque carré.
        let voile = Rect {
            x: essai.rect.x + COTE / 2,
            y: essai.rect.y + COTE / 2,
            largeur: COTE,
            hauteur: COTE,
        };
        // Le moins d'écarts vu en une demi-seconde (la composition peut traîner).
        let mut compter = |prise: &mut facade::Prise| -> usize {
            let debut = Instant::now();
            let mut moins = usize::MAX;
            while debut.elapsed() < Duration::from_millis(500) {
                laisser_composer();
                if prise.prendre(&mut px).is_ok() {
                    let (n, _) = ecarts_aux_carres(&px, prise.zone()).unwrap();
                    moins = moins.min(n);
                }
            }
            moins
        };
        for (nom, style) in [
            ("opaque", WINDOW_EX_STYLE(0)),
            ("calque traversant", WS_EX_LAYERED | WS_EX_TRANSPARENT),
        ] {
            let fenetre = FenetreEssai::creer(
                w!("BreezyShot essai vivant voile"),
                CLASSE_VOILE,
                procedure_voile,
                style,
                voile,
            );
            let sans_exclusion = compter(&mut prise);
            // SAFETY : fenêtre de premier niveau de ce processus ; un u32 en sortie.
            let (pose, relue) = unsafe {
                let pose = SetWindowDisplayAffinity(fenetre.hwnd, WDA_EXCLUDEFROMCAPTURE);
                let mut affinite = 0u32;
                let relue =
                    GetWindowDisplayAffinity(fenetre.hwnd, &mut affinite).map(|()| affinite);
                (pose, relue)
            };
            let avec_exclusion = compter(&mut prise);
            eprintln!(
                "vivant: fenetre native {nom} {}x{} : sans exclusion {sans_exclusion} pixels \
                 differents, avec exclusion {avec_exclusion} (pose {pose:?}, affinite relue \
                 {relue:?}, attendue {:#x})",
                voile.largeur, voile.hauteur, WDA_EXCLUDEFROMCAPTURE.0
            );
            drop(fenetre);
        }
        // Le voile parti, les carrés reviennent : la prise n'est pas abîmée.
        attendre_carres(&mut prise, &mut px).unwrap();
    }

    /// La seconde lecture de la garde (après la copie) refuse aussi, et la destination
    /// reste alors intacte. La garde est simulée : montrer l'écran figé pendant un `BitBlt`
    /// ne se provoque pas.
    #[test]
    fn seconde_lecture_de_la_garde() {
        let _s = Seance::ouvrir();
        let bureau = gel::bureau_virtuel().unwrap();
        let zone = Rect {
            x: bureau.x,
            y: bureau.y,
            largeur: 8,
            hauteur: 4,
        };
        let mut prise = facade::ouvrir(zone).unwrap();
        let mut px = Pixels {
            largeur: 1,
            hauteur: 1,
            rgba: vec![7, 7, 7, 7],
        };
        // Montré seulement à la seconde lecture : refus, destination intacte.
        let mut lectures = 0;
        let mut apres_la_copie = || {
            lectures += 1;
            lectures == 2
        };
        let e = prendre_garde(&mut prise.interne, &mut px, &mut apres_la_copie).unwrap_err();
        assert!(e.contains("ecran fige"), "{e}");
        assert_eq!(lectures, 2);
        assert_eq!(
            (px.largeur, px.hauteur, px.rgba.as_slice()),
            (1, 1, &[7u8; 4][..])
        );
        // Montré à la première : refus avant toute copie (une seule lecture).
        let mut lectures = 0;
        let mut avant = || {
            lectures += 1;
            true
        };
        assert!(prendre_garde(&mut prise.interne, &mut px, &mut avant).is_err());
        assert_eq!(lectures, 1);
        // Jamais montré : deux lectures, l'image est rendue.
        let mut lectures = 0;
        let mut jamais = || {
            lectures += 1;
            false
        };
        prendre_garde(&mut prise.interne, &mut px, &mut jamais).unwrap();
        assert_eq!(lectures, 2);
        assert_eq!((px.largeur, px.hauteur), (8, 4));
    }

    #[test]
    fn refus_pendant_l_ecran_fige() {
        // Le verrou est tenu pendant l'enfant : ses fenêtres ne gênent pas les autres tests
        // de ce module (même place à l'écran).
        let _s = Seance::ouvrir();
        dans_un_processus_a_part("refus_pendant_l_ecran_fige_enfant");
    }

    #[test]
    #[ignore = "joue dans un processus a part par refus_pendant_l_ecran_fige"]
    fn refus_pendant_l_ecran_fige_enfant() {
        if !dans_l_enfant() {
            return;
        }
        let _s = Seance::ouvrir();
        let _rendu = EcranFigeRendu;
        let essai = FenetreEssai::carres();
        let mut prise = facade::ouvrir(essai.rect).unwrap();
        let mut px = vide();
        attendre_carres(&mut prise, &mut px).unwrap();

        gel::preparer_ecran_fige().unwrap();
        let hwnd = HWND(gel::poignee_ecran_fige().unwrap() as *mut core::ffi::c_void);
        assert!(!gel::ecran_fige_montre(), "prepare mais cache");
        let image = gel::figer_bureau().unwrap();
        // Préparé mais caché : la prise marche.
        prise.prendre(&mut px).unwrap();

        gel::montrer_ecran_fige(&image).unwrap();
        pomper();
        // SAFETY : lecture sur la fenêtre de l'écran figé.
        assert!(unsafe { IsWindowVisible(hwnd) }.as_bool());
        let e = prise.prendre(&mut px).unwrap_err();
        assert!(e.contains("ecran fige"), "{e}");
        // Une prise ouverte pendant l'écran figé refuse aussi.
        let mut autre = facade::ouvrir(essai.rect).unwrap();
        assert!(autre.prendre(&mut px).is_err());

        // Caché depuis ce fil : la prise reprend.
        gel::cacher_ecran_fige();
        laisser_composer();
        prise.prendre(&mut px).unwrap();

        // Caché depuis un autre fil : tant que ce fil n'a pas distribué le masquage, la
        // fenêtre est visible et la prise refuse (comme `figer_bureau`).
        let image = gel::figer_bureau().unwrap();
        gel::montrer_ecran_fige(&image).unwrap();
        std::thread::spawn(gel::cacher_ecran_fige).join().unwrap();
        // Ce fil n'a rien distribué pendant `join` : la fenêtre est forcément visible.
        // SAFETY : lecture sur la fenêtre de l'écran figé.
        assert!(unsafe { IsWindowVisible(hwnd) }.as_bool());
        assert!(gel::ecran_fige_montre());
        assert!(prise.prendre(&mut px).is_err());
        let debut = Instant::now();
        // SAFETY : idem.
        while unsafe { IsWindowVisible(hwnd) }.as_bool() {
            assert!(debut.elapsed() < Duration::from_secs(2), "ecran fige reste");
            laisser_composer();
        }
        attendre_carres(&mut prise, &mut px).unwrap();
        drop(autre);
    }

    /// 1 000 prises, et 50 ouvertures et fermetures, sans objet GDI de plus ; durées
    /// notées. Dans un processus à part : aucun autre test n'y crée d'objets GDI.
    #[test]
    fn mille_prises_sans_fuite_gdi() {
        // Le verrou est tenu pendant l'enfant : ses fenêtres ne gênent pas les autres tests
        // de ce module (même place à l'écran).
        let _s = Seance::ouvrir();
        dans_un_processus_a_part("mille_prises_sans_fuite_gdi_enfant");
    }

    #[test]
    #[ignore = "joue dans un processus a part par mille_prises_sans_fuite_gdi"]
    fn mille_prises_sans_fuite_gdi_enfant() {
        if !dans_l_enfant() {
            return;
        }
        let _s = Seance::ouvrir();
        let principal = gel::ecrans()
            .into_iter()
            .find(|e| e.principal)
            .expect("un ecran principal");
        // 600 × 450 : la taille du budget « Prise » (§9), coupée à l'écran au besoin.
        let zone = Rect {
            x: principal.rect.x,
            y: principal.rect.y,
            largeur: 600.min(principal.rect.largeur),
            hauteur: 450.min(principal.rect.hauteur),
        };
        let mut px = vide();
        // Mise en train : les premiers appels peuvent créer des objets que GDI garde.
        for _ in 0..3 {
            let mut prise = facade::ouvrir(zone).unwrap();
            prise.prendre(&mut px).unwrap();
        }
        let avant = objets_gdi();

        let mut prise = facade::ouvrir(zone).unwrap();
        let ouverte = objets_gdi();
        let adresse = px.rgba.as_ptr();
        let mut durees = Vec::with_capacity(1000);
        for _ in 0..1000 {
            durees.push(prise.prendre(&mut px).unwrap());
        }
        assert_eq!(px.rgba.as_ptr(), adresse, "destination reallouee");
        let pendant = objets_gdi();
        // La prise ouverte tient un DC et une section DIB : un compteur qui ne les voit pas
        // rendrait les contrôles de fuite ci-dessous muets.
        assert!(pendant > avant, "compteur GDI muet : {avant} -> {pendant}");
        drop(prise);
        let rendue = objets_gdi();
        for _ in 0..50 {
            let mut p = facade::ouvrir(zone).unwrap();
            p.prendre(&mut px).unwrap();
        }
        let apres = objets_gdi();
        let p95 = {
            let mut d = durees.clone();
            d.sort();
            d[d.len() * 95 / 100].as_secs_f64() * 1000.0
        };
        eprintln!(
            "vivant: objets GDI avant {avant}, prise ouverte {ouverte}, apres 1000 prises \
             {pendant}, prise rendue {rendue}, apres 50 ouvertures {apres} ; prise {}x{} : \
             mediane {:.1} ms, 95e centile {p95:.1} ms (construction de test ; le budget de 40 ms se mesure en release, essai 14)",
            zone.largeur,
            zone.hauteur,
            mediane(durees)
        );
        // Les objets de la prise (un DC, une section DIB) existent pendant, pas après.
        // Une fuite par prise donnerait 1 000 objets de plus, une prise non rendue au moins
        // 2 (DC et section), 50 ouvertures qui fuient au moins 50 : la marge n'en cache aucune.
        assert!(
            pendant <= ouverte + 2,
            "fuite pendant les prises : {ouverte} -> {pendant}"
        );
        assert!(
            rendue <= avant + 1,
            "objets non rendus : {avant} -> {rendue}"
        );
        assert!(
            apres <= avant + 2,
            "fuite a l'ouverture : {avant} -> {apres}"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Source d'un film (0.7). Socle : pas encore construite (chantier A).

/// Aucune source ne peut exister ici : le type n'a aucune valeur.
#[derive(Debug)]
pub enum Source {}

pub fn ouvrir_source(_zone: Rect, _ecran: Rect) -> Result<Source, String> {
    Err("source du film non construite".into())
}

pub fn moyen_source(source: &Source) -> super::Moyen {
    match *source {}
}

pub fn zone_source(source: &Source) -> Rect {
    match *source {}
}

pub fn prendre_source(
    source: &mut Source,
    _dest: &mut Pixels,
) -> Result<(super::Image, Duration), String> {
    match *source {}
}

pub fn replier_source(source: &mut Source) -> Result<(), String> {
    match *source {}
}

pub fn repli_source(source: &mut Source) -> Option<(super::Moyen, String)> {
    match *source {}
}
