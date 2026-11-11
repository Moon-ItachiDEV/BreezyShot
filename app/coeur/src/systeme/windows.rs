//! Gestes propres à Windows. Lecture seule du registre : l'app n'y écrit jamais.

use ::windows::core::{w, PCWSTR};
use ::windows::Win32::Foundation::{CloseHandle, ERROR_SUCCESS, HWND, WAIT_OBJECT_0};
use ::windows::Win32::System::Registry::{
    RegCloseKey, RegGetValueW, RegNotifyChangeKeyValue, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER,
    HKEY_LOCAL_MACHINE, KEY_NOTIFY, KEY_READ, REG_NOTIFY_CHANGE_LAST_SET, RRF_RT_REG_DWORD,
    RRF_RT_REG_SZ,
};
use ::windows::Win32::System::Threading::{
    CreateEventW, GetCurrentProcess, WaitForSingleObject, INFINITE,
};
use ::windows::Win32::UI::HiDpi::{
    AreDpiAwarenessContextsEqual, GetDpiAwarenessContextForProcess,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use ::windows::Win32::UI::WindowsAndMessaging::{
    GetWindowDisplayAffinity, GetWindowLongW, SetWindowDisplayAffinity, SetWindowLongW,
    SetWindowPos, ShowWindow, SystemParametersInfoW, GWL_EXSTYLE, HWND_TOPMOST,
    SPI_GETCLIENTAREAANIMATION, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SWP_SHOWWINDOW, SW_SHOWNOACTIVATE, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    WDA_EXCLUDEFROMCAPTURE, WS_EX_LAYERED, WS_EX_TRANSPARENT,
};

pub type RappelTheme = Box<dyn Fn(super::Theme) + Send + 'static>;

const CLE_PERSONNALISER: PCWSTR =
    w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");

/// Lit une valeur DWORD. `None` si la clé ou la valeur est absente.
fn lire_dword(racine: HKEY, cle: PCWSTR, valeur: PCWSTR) -> Option<u32> {
    let mut donnee: u32 = 0;
    let mut taille = std::mem::size_of::<u32>() as u32;
    // SAFETY : tampon de 4 octets dont la taille est passée à l'API.
    let resultat = unsafe {
        RegGetValueW(
            racine,
            cle,
            valeur,
            RRF_RT_REG_DWORD,
            None,
            Some(&mut donnee as *mut u32 as *mut core::ffi::c_void),
            Some(&mut taille),
        )
    };
    (resultat == ERROR_SUCCESS).then_some(donnee)
}

/// Lit une valeur texte courte (au plus 255 caractères).
fn lire_texte(racine: HKEY, cle: PCWSTR, valeur: PCWSTR) -> Option<String> {
    let mut tampon = [0u16; 256];
    let mut taille = std::mem::size_of_val(&tampon) as u32;
    // SAFETY : tampon local dont la taille en octets est passée à l'API.
    let resultat = unsafe {
        RegGetValueW(
            racine,
            cle,
            valeur,
            RRF_RT_REG_SZ,
            None,
            Some(tampon.as_mut_ptr() as *mut core::ffi::c_void),
            Some(&mut taille),
        )
    };
    if resultat != ERROR_SUCCESS {
        return None;
    }
    let longueur = tampon.iter().position(|&c| c == 0).unwrap_or(tampon.len());
    Some(String::from_utf16_lossy(&tampon[..longueur]))
}

pub fn valeur_theme_barre() -> Option<u32> {
    lire_dword(
        HKEY_CURRENT_USER,
        CLE_PERSONNALISER,
        w!("SystemUsesLightTheme"),
    )
}

/// Fil de surveillance de la clé Personalize. À chaque tour : se réabonner d'abord, puis
/// relire le thème, puis attendre. Un changement survenu pendant la relecture réveille
/// donc le tour suivant : aucun n'est manqué. On ne prévient que si le thème a changé.
pub fn surveiller_theme_barre(rappel: RappelTheme) -> Result<(), String> {
    std::thread::Builder::new()
        .name("theme-barre".into())
        .spawn(move || {
            let mut cle = HKEY::default();
            // SAFETY : `cle` reçoit une clé ouverte, fermée en fin de fil.
            let ouverte = unsafe {
                RegOpenKeyExW(
                    HKEY_CURRENT_USER,
                    CLE_PERSONNALISER,
                    None,
                    KEY_NOTIFY | KEY_READ,
                    &mut cle,
                )
            };
            if ouverte != ERROR_SUCCESS {
                log::warn!("surveillance du theme impossible: cle introuvable");
                return;
            }
            // SAFETY : événement à réinitialisation automatique, fermé en fin de fil.
            let evenement = match unsafe { CreateEventW(None, false, false, PCWSTR::null()) } {
                Ok(e) => e,
                Err(e) => {
                    log::warn!("surveillance du theme impossible: {e}");
                    // SAFETY : clé ouverte plus haut.
                    unsafe {
                        let _ = RegCloseKey(cle);
                    }
                    return;
                }
            };
            let mut actuel = super::theme_barre();
            loop {
                // SAFETY : abonnement asynchrone sur une clé ouverte ; ce fil reste vivant
                // tant que l'abonnement sert.
                let r = unsafe {
                    RegNotifyChangeKeyValue(
                        cle,
                        false,
                        REG_NOTIFY_CHANGE_LAST_SET,
                        Some(evenement),
                        true,
                    )
                };
                if r != ERROR_SUCCESS {
                    log::warn!("surveillance du theme arretee: erreur {}", r.0);
                    break;
                }
                let nouveau = super::theme_barre();
                if nouveau != actuel {
                    actuel = nouveau;
                    rappel(nouveau);
                }
                // SAFETY : attente sur un événement valide.
                if unsafe { WaitForSingleObject(evenement, INFINITE) } != WAIT_OBJECT_0 {
                    log::warn!("surveillance du theme arretee: attente interrompue");
                    break;
                }
            }
            // SAFETY : clé et événement ouverts plus haut.
            unsafe {
                let _ = RegCloseKey(cle);
                let _ = CloseHandle(evenement);
            }
        })
        .map(|_| ())
        .map_err(|e| format!("fil de surveillance du theme: {e}"))
}

pub fn echelle_par_ecran_v2() -> Option<bool> {
    // SAFETY : pseudo-handle du processus courant, valable sans fermeture.
    let egal = unsafe {
        let contexte = GetDpiAwarenessContextForProcess(GetCurrentProcess());
        AreDpiAwarenessContextsEqual(contexte, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
    };
    Some(egal.as_bool())
}

pub fn version_windows() -> Option<u32> {
    lire_texte(
        HKEY_LOCAL_MACHINE,
        w!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion"),
        w!("CurrentBuildNumber"),
    )?
    .trim()
    .parse()
    .ok()
}

pub fn lecture_impr_ecran() -> Option<(Option<u32>, Option<u32>)> {
    let valeur = lire_dword(
        HKEY_CURRENT_USER,
        w!(r"Control Panel\Keyboard"),
        w!("PrintScreenKeyForSnippingEnabled"),
    );
    Some((valeur, version_windows()))
}

pub fn animations_reduites() -> Option<bool> {
    let mut actives = ::windows::core::BOOL(1);
    // SAFETY : SPI_GETCLIENTAREAANIMATION écrit un BOOL dans le tampon fourni.
    let lu = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some(&mut actives as *mut ::windows::core::BOOL as *mut core::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    lu.ok().map(|_| !actives.as_bool())
}

pub fn valeur_transparence() -> Option<u32> {
    lire_dword(
        HKEY_CURRENT_USER,
        CLE_PERSONNALISER,
        w!("EnableTransparency"),
    )
}

pub fn systeme() -> super::Systeme {
    super::Systeme::Windows
}

/// Le handle Win32 d'une fenêtre de l'interface.
fn hwnd(fenetre: &tauri::WebviewWindow) -> Result<HWND, String> {
    let h = fenetre
        .hwnd()
        .map_err(|e| format!("handle de fenetre: {e}"))?;
    Ok(HWND(h.0))
}

/// Montre sans activer. On ne passe pas par `show()` de Tauri : tao y appelle
/// `ShowWindow(SW_SHOW)`, qui active la fenêtre (window_state.rs, `set_window_flags`).
/// Le drapeau « visible » de tao reste donc faux : son `hide()` appelle quand même
/// `ShowWindow(SW_HIDE)` (même fonction), ce qui suffit pour cacher.
pub fn montrer_sans_activer(fenetre: &tauri::WebviewWindow) -> Result<bool, String> {
    let h = hwnd(fenetre)?;
    // SAFETY : handle valide d'une fenêtre de ce processus ; aucun tampon.
    unsafe {
        let _ = ShowWindow(h, SW_SHOWNOACTIVATE);
        SetWindowPos(
            h,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )
        .map_err(|e| format!("SetWindowPos: {e}"))?;
    }
    Ok(true)
}

/// Mêmes styles que `set_ignore_cursor_events` de tao (window_state.rs :
/// `WS_EX_TRANSPARENT | WS_EX_LAYERED`), posés directement : passer par tao réappliquerait
/// tous les styles et, comme tao croit la fenêtre cachée, la cacherait.
pub fn traverser_clics(fenetre: &tauri::WebviewWindow, traverser: bool) -> Result<bool, String> {
    let h = hwnd(fenetre)?;
    let bits = (WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0) as i32;
    // SAFETY : lecture et écriture du style étendu d'une fenêtre de ce processus.
    unsafe {
        let actuel = GetWindowLongW(h, GWL_EXSTYLE);
        let voulu = if traverser {
            actuel | bits
        } else {
            actuel & !bits
        };
        if voulu != actuel {
            SetWindowLongW(h, GWL_EXSTYLE, voulu);
            SetWindowPos(
                h,
                None,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            )
            .map_err(|e| format!("SetWindowPos: {e}"))?;
        }
    }
    Ok(true)
}

/// `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` (Windows 10 2004 et plus), relu par
/// `GetWindowDisplayAffinity`.
pub fn exclure_des_captures(fenetre: &tauri::WebviewWindow) -> bool {
    let Ok(h) = hwnd(fenetre) else {
        return false;
    };
    let mut affinite = 0u32;
    // SAFETY : handle valide ; `affinite` reçoit un DWORD.
    unsafe {
        let _ = SetWindowDisplayAffinity(h, WDA_EXCLUDEFROMCAPTURE);
        if GetWindowDisplayAffinity(h, &mut affinite).is_err() {
            return false;
        }
    }
    affinite == WDA_EXCLUDEFROMCAPTURE.0
}
