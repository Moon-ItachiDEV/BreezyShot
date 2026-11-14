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

    // --- Source d'un film (0.7, chantier A) ---

    /// Tests Windows de `Source` : même verrou, mêmes fenêtres d'essai que ceux de `Prise`.
    /// Chaque moyen disponible sur la machine est joué ; un test propre à la duplication le
    /// dit et ne fait rien si elle est refusée (repli pris à l'ouverture).
    mod source_film {
        use super::*;
        use crate::systeme::vivant::{Image, Moyen, Source};
        use ::windows::Win32::Graphics::Gdi::{
            ChangeDisplaySettingsExW, EnumDisplaySettingsW, InvalidateRect, CDS_TYPE, DEVMODEW,
            DM_PELSHEIGHT, DM_PELSWIDTH, ENUM_CURRENT_SETTINGS,
        };
        use ::windows::Win32::System::Threading::GetProcessHandleCount;
        use ::windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, SetCursorPos};

        fn ecran_principal() -> Rect {
            gel::ecrans()
                .into_iter()
                .find(|e| e.principal)
                .expect("un ecran principal")
                .rect
        }

        fn ouvrir_sur(zone: Rect) -> Source {
            let mut source = facade::ouvrir_source(zone, ecran_principal()).unwrap();
            eprintln!(
                "vivant: source {:?} ouverte, moyen {} ({:?})",
                source.zone(),
                source.moyen().nom(),
                source.repli()
            );
            source
        }

        /// Prend jusqu'à voir les quatre carrés exacts (au plus 2 s). Rend le temps
        /// d'attente et ce que la dernière prise a rendu.
        fn attendre_carres_source(
            source: &mut Source,
            dest: &mut Pixels,
        ) -> Result<(Duration, Image), String> {
            let debut = Instant::now();
            let mut dernier = String::new();
            while debut.elapsed() < Duration::from_secs(2) {
                match source.prendre(dest) {
                    Ok((Image::Perdue, _)) => dernier = "perdue".into(),
                    Ok((image, _)) => match ecarts_aux_carres(dest, source.zone()) {
                        Ok((0, _)) => return Ok((debut.elapsed(), image)),
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

        /// Au pixel près ; `Inchangee` sur un écran immobile, sans toucher la destination ;
        /// le pointeur qui bouge sur la zone ne fait aucune image neuve ; la fenêtre cachée
        /// se voit. Durées notées.
        fn verifier_moyen(source: &mut Source, essai: &FenetreEssai) {
            let nom = source.moyen().nom();
            let mut px = vide();
            let (attente, _) = attendre_carres_source(source, &mut px).unwrap();
            eprintln!(
                "vivant: {nom} : carres vus apres {:.0} ms",
                attente.as_secs_f64() * 1000.0
            );
            let adresse = px.rgba.as_ptr();
            let mut durees = Vec::new();
            let mut inchangees = 0;
            let mut nouvelles = 0;
            for _ in 0..32 {
                let (image, duree) = source.prendre(&mut px).unwrap();
                durees.push(duree);
                match image {
                    Image::Inchangee => inchangees += 1,
                    Image::Nouvelle => nouvelles += 1,
                    Image::Perdue => panic!("{nom} : perdue sur un ecran immobile"),
                }
                assert_eq!(px.rgba.as_ptr(), adresse, "destination reallouee");
                laisser_composer();
            }
            let (n, premier) = ecarts_aux_carres(&px, source.zone()).unwrap();
            assert_eq!(n, 0, "{nom} : {n} pixels faux, premier {premier:?}");
            eprintln!(
                "vivant: {nom} : ecran immobile, {inchangees} inchangees et {nouvelles} \
                 nouvelles sur 32 ; prise mediane {:.2} ms (construction de test)",
                mediane(durees)
            );
            // Une image identique n'est jamais « nouvelle » : rien ne bouge dans la zone.
            assert!(
                inchangees >= 28,
                "{nom} : seulement {inchangees} inchangees sur 32"
            );

            // Le pointeur posé et bougé sur la zone : noté (D5 : la duplication et BitBlt ne
            // le posent pas ; sur la machine GitHub il est caché faute de souris).
            let mut avant = ::windows::Win32::Foundation::POINT::default();
            // SAFETY : un POINT en sortie ; le pointeur est remis à sa place ensuite.
            let pointeur = unsafe { GetCursorPos(&mut avant) }.is_ok();
            if pointeur {
                let mut pixels_du_pointeur = 0usize;
                for i in 0..8 {
                    // SAFETY : position dans la zone, en pixels réels.
                    unsafe {
                        let _ =
                            SetCursorPos(essai.rect.x + 20 + i * 15, essai.rect.y + 30 + i * 10);
                    }
                    laisser_composer();
                    if let Ok((image, _)) = source.prendre(&mut px) {
                        if image != Image::Perdue {
                            let (n, _) = ecarts_aux_carres(&px, source.zone()).unwrap();
                            pixels_du_pointeur = pixels_du_pointeur.max(n);
                        }
                    }
                }
                // SAFETY : retour à la position lue.
                unsafe {
                    let _ = SetCursorPos(avant.x, avant.y);
                }
                laisser_composer();
                eprintln!(
                    "vivant: {nom} : pointeur bouge sur la zone : au plus {pixels_du_pointeur} \
                     pixels differents des carres"
                );
            }

            // La source suit l'écran : fenêtre cachée, une image neuve sans les carrés.
            essai.cacher();
            let debut = Instant::now();
            let partie = loop {
                let (image, _) = source.prendre(&mut px).unwrap();
                let (n, _) = ecarts_aux_carres(&px, source.zone()).unwrap();
                if image == Image::Nouvelle && n > 0 {
                    break true;
                }
                if debut.elapsed() > Duration::from_secs(2) {
                    break false;
                }
                laisser_composer();
            };
            assert!(
                partie,
                "{nom} : image figee, les carres restent apres la fenetre"
            );
        }

        /// Chaque moyen : la duplication (si la machine l'accepte), puis `BitBlt` par
        /// `replier`, qui le dit une fois.
        #[test]
        fn source_au_pixel_pres_par_chaque_moyen() {
            let _s = Seance::ouvrir();
            let essai = FenetreEssai::carres();
            let mut source = ouvrir_sur(essai.rect);
            assert_eq!(source.zone(), essai.rect);
            verifier_moyen(&mut source, &essai);
            drop(essai);

            let premier = source.moyen();
            source.replier().unwrap();
            assert_eq!(source.moyen(), Moyen::BitBlt);
            if premier == Moyen::Duplication {
                assert_eq!(
                    source.repli(),
                    Some((Moyen::BitBlt, "acces perdu".to_string()))
                );
            }
            assert_eq!(source.repli(), None);
            // Un second repli ne change rien et ne dit rien.
            source.replier().unwrap();
            assert_eq!(source.repli(), None);
            let essai = FenetreEssai::carres();
            verifier_moyen(&mut source, &essai);
        }

        /// Une zone coupée au bureau ; une zone vide ou dehors refusée ; une zone à cheval
        /// sur le bord ouvre quand même (repli si la duplication ne la voit pas entière).
        #[test]
        fn ouverture_bornee() {
            let _s = Seance::ouvrir();
            let bureau = gel::bureau_virtuel().unwrap();
            let cheval = Rect {
                x: bureau.x - 5,
                y: bureau.y - 4,
                largeur: 10,
                hauteur: 10,
            };
            let mut source = ouvrir_sur(cheval);
            assert_eq!(
                source.zone(),
                Rect {
                    x: bureau.x,
                    y: bureau.y,
                    largeur: 5,
                    hauteur: 6
                }
            );
            let mut px = vide();
            let debut = Instant::now();
            while let Ok((Image::Perdue, _)) | Err(_) = source.prendre(&mut px) {
                assert!(debut.elapsed() < Duration::from_secs(2), "aucune image");
                laisser_composer();
            }
            assert_eq!((px.largeur, px.hauteur), (5, 6));
            assert!(px.rgba.chunks_exact(4).all(|p| p[3] == 255), "alpha 255");
            let ecran = ecran_principal();
            assert!(facade::ouvrir_source(
                Rect {
                    x: bureau.x - 50,
                    ..cheval
                },
                ecran
            )
            .is_err());
            assert!(facade::ouvrir_source(
                Rect {
                    largeur: 0,
                    ..cheval
                },
                ecran
            )
            .is_err());
        }

        /// F6, S1 (a) : une fenêtre exclue des captures posée sur la zone n'est pas dans
        /// l'image de la duplication (exigé), ni dans celle de `BitBlt` (noté : une fenêtre
        /// NATIVE exclue y reste, une WebView2 exclue non, R2 de la 0.6).
        #[test]
        fn fenetre_exclue_absente() {
            let _s = Seance::ouvrir();
            let essai = FenetreEssai::carres();
            let mut source = ouvrir_sur(essai.rect);
            let mut px = vide();
            attendre_carres_source(&mut source, &mut px).unwrap();
            let voile = Rect {
                x: essai.rect.x + COTE / 2,
                y: essai.rect.y + COTE / 2,
                largeur: COTE,
                hauteur: COTE,
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
                // SAFETY : fenêtre de premier niveau de ce processus.
                unsafe { SetWindowDisplayAffinity(fenetre.hwnd, WDA_EXCLUDEFROMCAPTURE) }.unwrap();
                // Le plus d'écarts vu en une demi-seconde : la fenêtre ne doit jamais paraître.
                let debut = Instant::now();
                let mut plus = 0usize;
                while debut.elapsed() < Duration::from_millis(500) {
                    laisser_composer();
                    if let Ok((image, _)) = source.prendre(&mut px) {
                        if image != Image::Perdue {
                            let (n, _) = ecarts_aux_carres(&px, source.zone()).unwrap();
                            plus = plus.max(n);
                        }
                    }
                }
                eprintln!(
                    "vivant: {} : fenetre native exclue {nom} : au plus {plus} pixels differents",
                    source.moyen().nom()
                );
                if source.moyen() == Moyen::Duplication {
                    assert_eq!(
                        plus, 0,
                        "fenetre exclue {nom} dans l'image de la duplication"
                    );
                }
                drop(fenetre);
            }
            attendre_carres_source(&mut source, &mut px).unwrap();
        }

        /// La garde de l'écran figé, lue après la copie, écarte l'image sans toucher la
        /// destination ; la prise suivante rend de nouveau les carrés.
        #[test]
        fn seconde_lecture_de_la_garde_source() {
            let _s = Seance::ouvrir();
            let essai = FenetreEssai::carres();
            let mut source = ouvrir_sur(essai.rect);
            let mut px = vide();
            attendre_carres_source(&mut source, &mut px).unwrap();
            let gardee = px.rgba.clone();
            // Fait repeindre la fenêtre : la duplication a une image neuve à copier.
            // SAFETY : fenêtre de ce fil.
            unsafe {
                let _ = InvalidateRect(Some(essai.hwnd), None, true);
            }
            laisser_composer();
            let mut lectures = 0;
            let mut apres_la_copie = || {
                lectures += 1;
                lectures == 2
            };
            let r = source::prendre_source_garde(&mut source.interne, &mut px, &mut apres_la_copie);
            eprintln!(
                "vivant: {} : garde apres la copie : {r:?} ({lectures} lectures)",
                source.moyen().nom()
            );
            if lectures == 2 {
                assert!(r.unwrap_err().contains("ecran fige"));
            }
            assert_eq!(px.rgba, gardee, "destination touchee");
            // Montré dès la première lecture : refus sans rien prendre.
            let mut lectures = 0;
            let mut avant = || {
                lectures += 1;
                true
            };
            assert!(
                source::prendre_source_garde(&mut source.interne, &mut px, &mut avant).is_err()
            );
            assert_eq!(lectures, 1);
            // Jamais montré : les carrés.
            let (_, image) = attendre_carres_source(&mut source, &mut px).unwrap();
            assert_ne!(image, Image::Perdue);
        }

        /// F33 : une perte simulée (la duplication rendue, comme après
        /// `DXGI_ERROR_ACCESS_LOST`) est refaite tout de suite, sans changer de moyen, et
        /// la première image de la duplication refaite arrive sur un écran immobile.
        #[test]
        fn perte_simulee_puis_reprise() {
            let _s = Seance::ouvrir();
            let essai = FenetreEssai::carres();
            let mut source = ouvrir_sur(essai.rect);
            if source.moyen() != Moyen::Duplication {
                eprintln!("vivant: duplication refusee sur cette machine : perte non jouee");
                return;
            }
            let mut px = vide();
            attendre_carres_source(&mut source, &mut px).unwrap();
            let gardee = px.rgba.clone();
            for essai_n in 0..3 {
                source::perdre_pour_essai(&mut source.interne);
                let debut = Instant::now();
                let mut perdues = 0;
                let image = loop {
                    let (image, _) = source.prendre(&mut px).unwrap();
                    if image != Image::Perdue {
                        break image;
                    }
                    perdues += 1;
                    assert_eq!(px.rgba, gardee, "destination touchee pendant la perte");
                    assert!(
                        debut.elapsed() < Duration::from_millis(600),
                        "pas reprise en 600 ms"
                    );
                    std::thread::sleep(Duration::from_millis(5));
                };
                eprintln!(
                    "vivant: perte simulee {essai_n} : reprise apres {:.1} ms ({perdues} prises perdues), {image:?}",
                    debut.elapsed().as_secs_f64() * 1000.0
                );
                let (n, premier) = ecarts_aux_carres(&px, source.zone()).unwrap();
                assert_eq!(
                    n, 0,
                    "{n} pixels faux apres la reprise, premier {premier:?}"
                );
                assert_eq!(source.moyen(), Moyen::Duplication);
                assert_eq!(source.repli(), None);
            }
        }

        /// Le mode d'affichage du moment, remis à la fin même si le test échoue.
        struct ModeRemis;

        impl Drop for ModeRemis {
            fn drop(&mut self) {
                // SAFETY : sans mode : retour au mode enregistré.
                let code = unsafe { ChangeDisplaySettingsExW(None, None, None, CDS_TYPE(0), None) };
                eprintln!("vivant: mode remis (code {})", code.0);
                laisser_composer();
            }
        }

        fn changer_mode(largeur: u32, hauteur: u32) -> i32 {
            // SAFETY : DEVMODEW dont la taille est renseignée ; écran principal ; mode posé
            // sans l'écrire au registre.
            unsafe {
                let mut d = DEVMODEW {
                    dmSize: std::mem::size_of::<DEVMODEW>() as u16,
                    ..Default::default()
                };
                if !EnumDisplaySettingsW(None, ENUM_CURRENT_SETTINGS, &mut d).as_bool() {
                    return -100;
                }
                d.dmPelsWidth = largeur;
                d.dmPelsHeight = hauteur;
                d.dmFields = DM_PELSWIDTH | DM_PELSHEIGHT;
                ChangeDisplaySettingsExW(None, Some(&d), None, CDS_TYPE(0), None).0
            }
        }

        /// F33, S1 point 6 : un changement de mode (800 × 600 puis retour) fait perdre
        /// l'accès ; la source rend `Perdue` sans toucher la destination, se refait
        /// d'elle-même et reprend, sans changer de moyen.
        #[test]
        fn perdue_puis_reprise_apres_un_changement_de_mode() {
            let _s = Seance::ouvrir();
            let essai = FenetreEssai::carres();
            let mut source = ouvrir_sur(essai.rect);
            if source.moyen() != Moyen::Duplication {
                eprintln!("vivant: duplication refusee sur cette machine : perte non jouee");
                return;
            }
            let mut px = vide();
            attendre_carres_source(&mut source, &mut px).unwrap();
            let mut gardee = px.rgba.clone();
            let code = changer_mode(800, 600);
            if code != 0 {
                eprintln!("vivant: mode 800x600 refuse (code {code}) : perte non jouee");
                return;
            }
            let remis = ModeRemis;
            let debut = Instant::now();
            let mut perdue_a = None;
            let mut prises = 0u32;
            while debut.elapsed() < Duration::from_secs(2) {
                if let Ok((image, _)) = source.prendre(&mut px) {
                    prises += 1;
                    if image == Image::Perdue {
                        perdue_a.get_or_insert(debut.elapsed());
                        assert_eq!(px.rgba, gardee, "destination touchee pendant la perte");
                    } else {
                        // Refaite pendant le mode 800 × 600 : l'image peut changer.
                        gardee.clone_from(&px.rgba);
                    }
                }
                laisser_composer();
            }
            drop(remis);
            let remis_a = Instant::now();
            let reprise = loop {
                if let Ok((image, _)) = source.prendre(&mut px) {
                    if image != Image::Perdue
                        && ecarts_aux_carres(&px, source.zone()).is_ok_and(|(n, _)| n == 0)
                    {
                        break remis_a.elapsed();
                    }
                }
                assert!(
                    remis_a.elapsed() < Duration::from_secs(3),
                    "pas reprise 3 s apres le mode remis"
                );
                laisser_composer();
            };
            eprintln!(
                "vivant: changement de mode : perdue apres {perdue_a:?} ({prises} prises), \
                 carres revus {:.0} ms apres le mode remis",
                reprise.as_secs_f64() * 1000.0
            );
            assert!(
                perdue_a.is_some(),
                "acces jamais perdu pendant le changement de mode"
            );
            assert_eq!(source.moyen(), Moyen::Duplication);
            assert_eq!(source.repli(), None);
        }

        #[test]
        fn refus_pendant_l_ecran_fige_source() {
            let _s = Seance::ouvrir();
            dans_un_processus_a_part("source_film::refus_pendant_l_ecran_fige_source_enfant");
        }

        #[test]
        #[ignore = "joue dans un processus a part par refus_pendant_l_ecran_fige_source"]
        fn refus_pendant_l_ecran_fige_source_enfant() {
            if !dans_l_enfant() {
                return;
            }
            let _s = Seance::ouvrir();
            let _rendu = EcranFigeRendu;
            let essai = FenetreEssai::carres();
            let mut source = ouvrir_sur(essai.rect);
            let mut px = vide();
            attendre_carres_source(&mut source, &mut px).unwrap();
            gel::preparer_ecran_fige().unwrap();
            let image = gel::figer_bureau().unwrap();
            gel::montrer_ecran_fige(&image).unwrap();
            pomper();
            let e = source.prendre(&mut px).unwrap_err();
            assert!(e.contains("ecran fige"), "{e}");
            // Une source ouverte pendant l'écran figé (le cas d'un film) refuse aussi.
            let mut autre = ouvrir_sur(essai.rect);
            assert!(autre.prendre(&mut px).is_err());
            gel::cacher_ecran_fige();
            laisser_composer();
            attendre_carres_source(&mut source, &mut px).unwrap();
            // Celle ouverte pendant l'écran figé rend l'écran, pas l'image figée.
            attendre_carres_source(&mut autre, &mut px).unwrap();
        }

        fn poignees() -> u32 {
            let mut n = 0u32;
            // SAFETY : pseudo-poignée du processus courant ; un u32 en sortie.
            unsafe { GetProcessHandleCount(GetCurrentProcess(), &mut n) }.unwrap();
            n
        }

        /// 3 000 prises par la duplication (la fenêtre repeinte à chaque fois : la zone est
        /// copiée), puis 1 000 par `BitBlt`, puis 20 ouvertures, sans poignée ni objet GDI
        /// de plus. Dans un processus à part, comme `mille_prises_sans_fuite_gdi`.
        #[test]
        fn trois_mille_prises_sans_fuite() {
            let _s = Seance::ouvrir();
            dans_un_processus_a_part("source_film::trois_mille_prises_sans_fuite_enfant");
        }

        #[test]
        #[ignore = "joue dans un processus a part par trois_mille_prises_sans_fuite"]
        fn trois_mille_prises_sans_fuite_enfant() {
            if !dans_l_enfant() {
                return;
            }
            let _s = Seance::ouvrir();
            let essai = FenetreEssai::carres();
            let mut px = vide();
            // Mise en train : les premiers appels chargent DXGI et Direct3D.
            for _ in 0..3 {
                let mut s = ouvrir_sur(essai.rect);
                let _ = attendre_carres_source(&mut s, &mut px);
            }
            let (poignees_avant, gdi_avant) = (poignees(), objets_gdi());
            let mut source = ouvrir_sur(essai.rect);
            let moyen = source.moyen();
            // 3 000 prises par le premier moyen, puis 1 000 par `BitBlt` (25 à 30 ms chacune
            // sur la machine GitHub ; `mille_prises_sans_fuite_gdi` le prouve déjà pour
            // `Prise`).
            for (tour, nombre) in [3000usize, 1000].into_iter().enumerate() {
                let mut durees = Vec::with_capacity(nombre);
                let (mut nouvelles, mut inchangees, mut perdues) = (0, 0, 0);
                for _ in 0..nombre {
                    // SAFETY : fenêtre de ce fil.
                    unsafe {
                        let _ = InvalidateRect(Some(essai.hwnd), None, false);
                    }
                    pomper();
                    let (image, duree) = source.prendre(&mut px).unwrap();
                    durees.push(duree);
                    match image {
                        Image::Nouvelle => nouvelles += 1,
                        Image::Inchangee => inchangees += 1,
                        Image::Perdue => perdues += 1,
                    }
                }
                eprintln!(
                    "vivant: {} : {nombre} prises, {nouvelles} nouvelles, {inchangees} \
                     inchangees, {perdues} perdues ; mediane {:.2} ms ; poignees {} -> {}, \
                     objets GDI {} -> {}",
                    source.moyen().nom(),
                    mediane(durees),
                    poignees_avant,
                    poignees(),
                    gdi_avant,
                    objets_gdi()
                );
                assert!(perdues <= 3, "{perdues} prises perdues sur un ecran stable");
                assert!(poignees() <= poignees_avant + 16, "fuite de poignees");
                assert!(objets_gdi() <= gdi_avant + 4, "fuite d'objets GDI");
                if tour == 0 {
                    source.replier().unwrap();
                }
            }
            drop(source);
            for _ in 0..20 {
                let mut s = ouvrir_sur(essai.rect);
                let _ = s.prendre(&mut px);
            }
            let (poignees_apres, gdi_apres) = (poignees(), objets_gdi());
            eprintln!(
                "vivant: {} puis bitblt : apres 20 ouvertures, poignees {poignees_avant} -> \
                 {poignees_apres}, objets GDI {gdi_avant} -> {gdi_apres}",
                moyen.nom()
            );
            assert!(
                poignees_apres <= poignees_avant + 16,
                "fuite a l'ouverture : {poignees_avant} -> {poignees_apres}"
            );
            assert!(
                gdi_apres <= gdi_avant + 2,
                "fuite GDI a l'ouverture : {gdi_avant} -> {gdi_apres}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// Source d'un film (0.7, F12, F13, F33) : la duplication de bureau de l'écran de la zone,
// `BitBlt` en repli. La logique (choix de la sortie, rectangles, copie, reprise) est pure,
// dans `vivant/mod.rs` ; ici, seulement les appels à DXGI, Direct3D 11 et GDI.
//
// Duplication : le périphérique Direct3D est créé sur la carte qui porte la sortie de la
// zone (on parcourt les cartes et leurs sorties) ; `AcquireNextFrame` à délai nul ; seule
// la zone est copiée (`CopySubresourceRegion` vers une texture lisible de sa taille), et
// seulement si un rectangle sale ou déplacé la touche ; la copie vers `dest` dit encore si
// un pixel a changé. Le pointeur n'est jamais dans l'image de la duplication (Windows le
// donne à part) : une image où seul le pointeur a bougé ne coûte rien. Un accès perdu
// (changement de mode, bureau sécurisé, plein écran exclusif) rend `Perdue`, et la
// duplication est refaite à la prise suivante, puis au plus toutes les 500 ms (F33).
//
// `dxgi.dll` et `d3d11.dll` sont chargées à la demande (`build.rs`) : la première
// ouverture d'une source les charge ; elles existent sur tout Windows 11 (F31, sans garde).

pub use source::{
    moyen_source, ouvrir_source, prendre_source, repli_source, replier_source, zone_source, Source,
};

mod source {
    use std::time::{Duration, Instant};

    use ::windows::core::Interface;
    use ::windows::Win32::Foundation::{HMODULE, RECT};
    use ::windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
    use ::windows::Win32::Graphics::Direct3D11::{
        D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_BOX,
        D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE,
        D3D11_MAP_READ, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
    };
    use ::windows::Win32::Graphics::Dxgi::Common::{
        DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_MODE_ROTATION_IDENTITY, DXGI_MODE_ROTATION_UNSPECIFIED,
        DXGI_SAMPLE_DESC,
    };
    use ::windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIAdapter, IDXGIFactory1, IDXGIOutput1, IDXGIOutputDuplication,
        IDXGIResource, DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO,
        DXGI_OUTDUPL_MOVE_RECT,
    };
    use ::windows::Win32::Graphics::Gdi::{BitBlt, GdiFlush, SRCCOPY};

    use super::{couper_au_bureau, DcEcran, Dib, Pixels, Rect};
    use crate::systeme::gel;
    use crate::systeme::vivant::{
        choisir_sortie, code_hex, copier_bgra, zone_dans_la_sortie, zone_touchee, ChoixSortie,
        Etape, Image, Moyen, Serie, SortieVue, Suivi, RAISON_ACCES_PERDU,
    };

    fn rect_de(r: RECT) -> Rect {
        Rect::depuis_bords(r.left, r.top, r.right, r.bottom)
    }

    /// Ce qui a empêché d'ouvrir la duplication : la raison au journal (`film source
    /// bitblt (<raison>)`), sans accent ni chemin.
    type Refus = String;

    /// Une étape refusée par le système : « <étape> 0x887A0004 ».
    fn refus(etape: &str, e: &::windows::core::Error) -> Refus {
        format!("{etape} {}", code_hex(e.code().0))
    }

    // --- La duplication d'une sortie ---

    /// La duplication de la sortie qui porte la zone, et la texture lisible de la taille
    /// de la zone. Champs détruits dans l'ordre : la duplication avant le périphérique.
    struct Duplication {
        dupl: IDXGIOutputDuplication,
        lisible: ID3D11Texture2D,
        contexte: ID3D11DeviceContext,
        _appareil: ID3D11Device,
        /// La zone, en coordonnées de la sortie.
        dans_sortie: Rect,
    }

    /// Parcourt les cartes et leurs sorties (fabrique neuve : à jour après un changement de
    /// mode), choisit celle qui porte la zone (`choisir_sortie`), crée le périphérique sur
    /// sa carte et la duplication. Rien n'est acquis ici : à l'ouverture d'un film, l'écran
    /// figé peut encore être montré.
    fn ouvrir_duplication(zone: Rect, ecran: Rect) -> Result<Duplication, Refus> {
        // SAFETY : énumération DXGI ; chaque objet COM est rendu par son compte de
        // références ; les descriptions sont copiées.
        let (carte, sortie, bureau) = unsafe {
            let fabrique: IDXGIFactory1 = CreateDXGIFactory1().map_err(|e| refus("dxgi", &e))?;
            let mut vues = Vec::new();
            let mut objets = Vec::new();
            let mut i = 0;
            while let Ok(carte) = fabrique.EnumAdapters1(i) {
                let mut j = 0;
                while let Ok(sortie) = carte.EnumOutputs(j) {
                    if let Ok(d) = sortie.GetDesc() {
                        if d.AttachedToDesktop.as_bool() {
                            vues.push(SortieVue {
                                bureau: rect_de(d.DesktopCoordinates),
                                tournee: d.Rotation != DXGI_MODE_ROTATION_IDENTITY
                                    && d.Rotation != DXGI_MODE_ROTATION_UNSPECIFIED,
                            });
                            objets.push((carte.clone(), sortie));
                        }
                    }
                    j += 1;
                }
                i += 1;
            }
            match choisir_sortie(zone, ecran, &vues) {
                ChoixSortie::Sortie(k) => {
                    let (carte, sortie) = objets.swap_remove(k);
                    (carte, sortie, vues[k].bureau)
                }
                autre => return Err(autre.raison().unwrap_or("sortie").to_string()),
            }
        };
        let dans_sortie = zone_dans_la_sortie(zone, bureau).ok_or("zone hors des sorties")?;
        let carte: IDXGIAdapter = carte.cast().map_err(|e| refus("carte", &e))?;
        let sortie: IDXGIOutput1 = sortie.cast().map_err(|e| refus("sortie", &e))?;
        let mut appareil = None;
        let mut contexte = None;
        // SAFETY : création d'un périphérique sur la carte de la sortie ; les sorties sont
        // des `Option` remplies par l'appel.
        unsafe {
            D3D11CreateDevice(
                &carte,
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut appareil),
                None,
                Some(&mut contexte),
            )
        }
        .map_err(|e| refus("d3d11", &e))?;
        let appareil = appareil.ok_or("d3d11 sans peripherique")?;
        let contexte = contexte.ok_or("d3d11 sans contexte")?;
        // SAFETY : sortie et périphérique vivants, de la même carte.
        let dupl = unsafe { sortie.DuplicateOutput(&appareil) }
            .map_err(|e| refus("duplication refusee", &e))?;
        let desc = D3D11_TEXTURE2D_DESC {
            Width: dans_sortie.largeur as u32,
            Height: dans_sortie.hauteur as u32,
            MipLevels: 1,
            ArraySize: 1,
            // `DuplicateOutput` rend toujours ce format, quel que soit le mode.
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        let mut lisible = None;
        // SAFETY : description valide ; la texture est rendue par son compte de références.
        unsafe { appareil.CreateTexture2D(&desc, None, Some(&mut lisible)) }
            .map_err(|e| refus("texture", &e))?;
        Ok(Duplication {
            dupl,
            lisible: lisible.ok_or("texture absente")?,
            contexte,
            _appareil: appareil,
            dans_sortie,
        })
    }

    /// Une image acquise, rendue par `ReleaseFrame` quoi qu'il arrive.
    struct ImageAcquise<'a>(&'a IDXGIOutputDuplication);

    impl Drop for ImageAcquise<'_> {
        fn drop(&mut self) {
            // SAFETY : une image a été acquise par cette duplication et pas encore rendue.
            unsafe {
                let _ = self.0.ReleaseFrame();
            }
        }
    }

    /// La texture lisible ouverte en lecture, refermée par `Unmap` quoi qu'il arrive.
    struct Lecture<'a> {
        contexte: &'a ID3D11DeviceContext,
        texture: &'a ID3D11Texture2D,
        carte: D3D11_MAPPED_SUBRESOURCE,
    }

    impl Drop for Lecture<'_> {
        fn drop(&mut self) {
            // SAFETY : la sous-ressource 0 a été ouverte par `Map` et pas encore refermée.
            unsafe {
                self.contexte.Unmap(self.texture, 0);
            }
        }
    }

    /// Tampons des rectangles d'une image, gardés d'une prise à l'autre.
    #[derive(Default)]
    struct Rectangles {
        deplaces_dxgi: Vec<DXGI_OUTDUPL_MOVE_RECT>,
        sales_dxgi: Vec<RECT>,
        deplaces: Vec<(Rect, Rect)>,
        sales: Vec<Rect>,
    }

    impl Rectangles {
        /// Un rectangle sale ou déplacé de l'image acquise touche-t-il la zone ? `None` :
        /// non lus (pas de métadonnées, ou lecture refusée) : l'image est alors copiée.
        fn zone_touchee(
            &mut self,
            dupl: &IDXGIOutputDuplication,
            info: &DXGI_OUTDUPL_FRAME_INFO,
            zone: Rect,
        ) -> Option<bool> {
            let taille = info.TotalMetadataBufferSize as usize;
            if taille == 0 {
                return None;
            }
            let n = taille / std::mem::size_of::<DXGI_OUTDUPL_MOVE_RECT>() + 1;
            self.deplaces_dxgi
                .resize(n, DXGI_OUTDUPL_MOVE_RECT::default());
            let mut requis = 0u32;
            // SAFETY : tampon de `n` rectangles dont la taille en octets est passée ; image
            // acquise et pas encore rendue.
            unsafe {
                dupl.GetFrameMoveRects(
                    (n * std::mem::size_of::<DXGI_OUTDUPL_MOVE_RECT>()) as u32,
                    self.deplaces_dxgi.as_mut_ptr(),
                    &mut requis,
                )
            }
            .ok()?;
            let lus = (requis as usize / std::mem::size_of::<DXGI_OUTDUPL_MOVE_RECT>()).min(n);
            self.deplaces.clear();
            self.deplaces
                .extend(self.deplaces_dxgi[..lus].iter().map(|m| {
                    let d = rect_de(m.DestinationRect);
                    let source = Rect {
                        x: m.SourcePoint.x,
                        y: m.SourcePoint.y,
                        ..d
                    };
                    (source, d)
                }));
            let n = taille / std::mem::size_of::<RECT>() + 1;
            self.sales_dxgi.resize(n, RECT::default());
            // SAFETY : idem.
            unsafe {
                dupl.GetFrameDirtyRects(
                    (n * std::mem::size_of::<RECT>()) as u32,
                    self.sales_dxgi.as_mut_ptr(),
                    &mut requis,
                )
            }
            .ok()?;
            let lus = (requis as usize / std::mem::size_of::<RECT>()).min(n);
            self.sales.clear();
            self.sales
                .extend(self.sales_dxgi[..lus].iter().map(|r| rect_de(*r)));
            Some(zone_touchee(zone, &self.deplaces, &self.sales))
        }
    }

    /// Ce qu'a donné une acquisition.
    enum Acquis {
        /// Délai dépassé : rien de neuf sur l'écran.
        Rien,
        /// Une image, mais rien à copier (pointeur seul, zone intacte).
        PasACopier,
        /// L'image de la zone a été copiée ; vrai si un pixel a changé.
        Copiee(bool),
        /// L'écran figé est montré : l'image est écartée.
        Garde,
        /// Échec : la duplication est à refaire. Le code au journal ; vrai pour un accès
        /// perdu (attendu : changement de mode, bureau sécurisé), qui n'est pas écrit.
        Perte(String, bool),
    }

    /// Acquiert une image (délai nul) et copie la zone si `suivi` le demande.
    fn acquerir(
        d: &Duplication,
        suivi: &Suivi,
        rectangles: &mut Rectangles,
        dest: &mut Pixels,
        ecran_fige_montre: &mut dyn FnMut() -> bool,
    ) -> Acquis {
        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut ressource: Option<IDXGIResource> = None;
        // SAFETY : duplication vivante ; l'image acquise est rendue par `ImageAcquise`.
        let acquise = unsafe { d.dupl.AcquireNextFrame(0, &mut info, &mut ressource) };
        match acquise {
            Ok(()) => {}
            Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => return Acquis::Rien,
            Err(e) => {
                let perdu = e.code() == DXGI_ERROR_ACCESS_LOST;
                return Acquis::Perte(code_hex(e.code().0), perdu);
            }
        }
        let _rendue = ImageAcquise(&d.dupl);
        let ecran_neuf = info.LastPresentTime != 0;
        let touchee = if ecran_neuf && !suivi.forcer {
            rectangles.zone_touchee(&d.dupl, &info, d.dans_sortie)
        } else {
            None
        };
        if !suivi.a_copier(ecran_neuf, touchee) {
            return Acquis::PasACopier;
        }
        let Some(ressource) = ressource else {
            return Acquis::Perte("image absente".into(), false);
        };
        let texture: ID3D11Texture2D = match ressource.cast() {
            Ok(t) => t,
            Err(e) => return Acquis::Perte(code_hex(e.code().0), false),
        };
        let z = d.dans_sortie;
        let boite = D3D11_BOX {
            left: z.x as u32,
            top: z.y as u32,
            front: 0,
            right: (z.x + z.largeur) as u32,
            bottom: (z.y + z.hauteur) as u32,
            back: 1,
        };
        let mut carte = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY : textures du même périphérique, de même format ; la boîte est dans
        // l'image de la sortie (`zone_dans_la_sortie`) et a la taille de la texture
        // lisible. `Map` attend la fin de la copie.
        let ouverte = unsafe {
            d.contexte
                .CopySubresourceRegion(&d.lisible, 0, 0, 0, 0, &texture, 0, Some(&boite));
            d.contexte
                .Map(&d.lisible, 0, D3D11_MAP_READ, 0, Some(&mut carte))
        };
        if let Err(e) = ouverte {
            return Acquis::Perte(code_hex(e.code().0), false);
        }
        let lecture = Lecture {
            contexte: &d.contexte,
            texture: &d.lisible,
            carte,
        };
        // L'écran figé a pu paraître pendant la copie : l'image serait la sienne.
        if ecran_fige_montre() {
            return Acquis::Garde;
        }
        let (largeur, hauteur) = (z.largeur as u32, z.hauteur as u32);
        let pas = lecture.carte.RowPitch as usize;
        let octets = pas * (hauteur as usize - 1) + largeur as usize * 4;
        if lecture.carte.pData.is_null() {
            return Acquis::Perte("lecture vide".into(), false);
        }
        // SAFETY : `Map` rend `hauteur` lignes de `RowPitch` octets, la dernière au moins
        // de la largeur de la texture ; lues pendant que la texture est ouverte.
        let source =
            unsafe { std::slice::from_raw_parts(lecture.carte.pData as *const u8, octets) };
        match copier_bgra(source, pas, largeur, hauteur, dest) {
            Ok(change) => Acquis::Copiee(change),
            Err(e) => Acquis::Perte(e, false),
        }
    }

    // --- La source ---

    /// Le moyen en service et son état.
    enum Interne {
        Duplication {
            duplication: Option<Duplication>,
            suivi: Suivi,
            rectangles: Rectangles,
        },
        BitBlt {
            dib: Dib,
        },
    }

    /// La source d'un film.
    pub struct Source {
        zone: Rect,
        ecran: Rect,
        interne: Interne,
        /// Changement de moyen pas encore lu par `repli`.
        changement: Option<(Moyen, String)>,
        /// Échecs de prise (autres que l'accès perdu) en série : un WARN par série.
        serie: Serie,
    }

    // SAFETY : le périphérique Direct3D 11 et la duplication peuvent servir depuis
    // n'importe quel fil ; le contexte immédiat, la section DIB et son DC ne servent qu'à
    // un fil à la fois, ce que garantit `&mut self` sur toutes les méthodes qui y touchent.
    // `Source` n'est pas `Sync`.
    unsafe impl Send for Source {}

    impl std::fmt::Debug for Source {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("Source")
                .field("zone", &self.zone)
                .field("moyen", &moyen_source(self))
                .finish()
        }
    }

    pub fn ouvrir_source(zone: Rect, ecran: Rect) -> Result<Source, String> {
        let zone = couper_au_bureau(zone, gel::bureau_virtuel()?)?;
        let (interne, changement) = match ouvrir_duplication(zone, ecran) {
            Ok(duplication) => (
                Interne::Duplication {
                    duplication: Some(duplication),
                    suivi: Suivi::nouveau(),
                    rectangles: Rectangles::default(),
                },
                None,
            ),
            Err(raison) => (
                Interne::BitBlt {
                    dib: Dib::nouvelle(zone.largeur, zone.hauteur)?,
                },
                Some((Moyen::BitBlt, raison)),
            ),
        };
        Ok(Source {
            zone,
            ecran,
            interne,
            changement,
            serie: Serie::default(),
        })
    }

    pub fn moyen_source(source: &Source) -> Moyen {
        match source.interne {
            Interne::Duplication { .. } => Moyen::Duplication,
            Interne::BitBlt { .. } => Moyen::BitBlt,
        }
    }

    pub fn zone_source(source: &Source) -> Rect {
        source.zone
    }

    pub fn prendre_source(
        source: &mut Source,
        dest: &mut Pixels,
    ) -> Result<(Image, Duration), String> {
        prendre_source_garde(source, dest, &mut gel::ecran_fige_montre)
    }

    /// Écrit le premier échec d'une série (§8 : une ligne par série, pas par image).
    fn noter_echec(serie: &mut Serie, code: &str) {
        if serie.echec() {
            log::warn!("film: prise en echec ({code})");
        }
    }

    /// `prendre_source`, la garde de l'écran figé donnée par l'appelant (les tests la
    /// simulent). Lue avant la prise et après la copie en mémoire, avant d'écrire `dest`.
    pub(super) fn prendre_source_garde(
        source: &mut Source,
        dest: &mut Pixels,
        ecran_fige_montre: &mut dyn FnMut() -> bool,
    ) -> Result<(Image, Duration), String> {
        if ecran_fige_montre() {
            return Err("ecran fige montre".into());
        }
        let debut = Instant::now();
        let image = match &mut source.interne {
            Interne::Duplication {
                duplication,
                suivi,
                rectangles,
            } => {
                match suivi.etape(duplication.is_some(), debut) {
                    Etape::Acquerir => {}
                    Etape::Attendre => return Ok((suivi.sans_copie(), debut.elapsed())),
                    Etape::Refaire => match ouvrir_duplication(source.zone, source.ecran) {
                        Ok(d) => {
                            *duplication = Some(d);
                            suivi.refaite(true, debut);
                        }
                        Err(_) => {
                            // Attendu tant que le mode change ou que le bureau sécurisé est
                            // montré : pas écrit (le film écrit `film source perdue`).
                            suivi.refaite(false, debut);
                            return Ok((suivi.sans_copie(), debut.elapsed()));
                        }
                    },
                }
                let acquis = match duplication.as_ref() {
                    Some(d) => acquerir(d, suivi, rectangles, dest, ecran_fige_montre),
                    None => Acquis::Rien,
                };
                match acquis {
                    Acquis::Rien => suivi.sans_copie(),
                    Acquis::PasACopier => {
                        source.serie.succes();
                        suivi.sans_copie()
                    }
                    Acquis::Copiee(change) => {
                        source.serie.succes();
                        suivi.copiee(change, Instant::now()).0
                    }
                    Acquis::Garde => {
                        suivi.ecartee();
                        return Err("ecran fige montre".into());
                    }
                    Acquis::Perte(code, acces_perdu) => {
                        if !acces_perdu {
                            noter_echec(&mut source.serie, &code);
                        }
                        // La duplication (et l'image acquise) sont rendues ici.
                        *duplication = None;
                        suivi.perdue(Instant::now());
                        Image::Perdue
                    }
                }
            }
            Interne::BitBlt { dib } => {
                match bitblt(dib, source.zone) {
                    Ok(()) => {}
                    Err(code) => {
                        noter_echec(&mut source.serie, &code);
                        return Ok((Image::Perdue, debut.elapsed()));
                    }
                }
                if ecran_fige_montre() {
                    return Err("ecran fige montre".into());
                }
                let (largeur, hauteur) = (source.zone.largeur as u32, source.zone.hauteur as u32);
                let change =
                    copier_bgra(dib.octets(), largeur as usize * 4, largeur, hauteur, dest)?;
                source.serie.succes();
                if change {
                    Image::Nouvelle
                } else {
                    Image::Inchangee
                }
            }
        };
        Ok((image, debut.elapsed()))
    }

    /// Le `BitBlt` de la zone dans la section DIB (celui de `Prise`). Erreur : le code.
    fn bitblt(dib: &Dib, zone: Rect) -> Result<(), String> {
        let ecran = DcEcran::prendre().map_err(|_| "GetDC".to_string())?;
        // SAFETY : les deux DC sont valides ; la copie tient dans la section DIB (même
        // taille que la zone). Pas de `CAPTUREBLT` (R2).
        unsafe {
            BitBlt(
                dib.dc.0,
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
        .map_err(|e| code_hex(e.code().0))?;
        // La copie est faite avant de rendre le DC de l'écran et avant la seconde lecture
        // de la garde. SAFETY : sans paramètre ; vide le lot de ce fil.
        unsafe {
            let _ = GdiFlush();
        }
        Ok(())
    }

    pub fn replier_source(source: &mut Source) -> Result<(), String> {
        if matches!(source.interne, Interne::BitBlt { .. }) {
            return Ok(());
        }
        let dib = Dib::nouvelle(source.zone.largeur, source.zone.hauteur)?;
        // La duplication est rendue ici.
        source.interne = Interne::BitBlt { dib };
        source.changement = Some((Moyen::BitBlt, RAISON_ACCES_PERDU.to_string()));
        source.serie.succes();
        Ok(())
    }

    pub fn repli_source(source: &mut Source) -> Option<(Moyen, String)> {
        source.changement.take()
    }

    /// Pour les tests : une perte simulée (comme `DXGI_ERROR_ACCESS_LOST`).
    #[cfg(test)]
    pub(super) fn perdre_pour_essai(source: &mut Source) {
        if let Interne::Duplication {
            duplication, suivi, ..
        } = &mut source.interne
        {
            *duplication = None;
            suivi.perdue(Instant::now());
        }
    }
}
