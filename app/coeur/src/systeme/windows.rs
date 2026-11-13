//! Gestes propres à Windows. Lecture seule du registre : l'app n'y écrit jamais.

use ::windows::core::{w, PCWSTR};
use ::windows::Win32::Foundation::{CloseHandle, ERROR_SUCCESS, HWND, POINT, RECT, WAIT_OBJECT_0};
use ::windows::Win32::System::Registry::{
    RegCloseKey, RegGetValueW, RegNotifyChangeKeyValue, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER,
    HKEY_LOCAL_MACHINE, KEY_NOTIFY, KEY_READ, REG_NOTIFY_CHANGE_LAST_SET, RRF_RT_REG_DWORD,
    RRF_RT_REG_SZ,
};
use ::windows::Win32::System::Threading::{
    CreateEventW, GetCurrentProcess, WaitForSingleObject, INFINITE,
};
use ::windows::Win32::UI::HiDpi::{
    AreDpiAwarenessContextsEqual, GetDpiAwarenessContextForProcess, GetDpiForWindow,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use ::windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetWindowDisplayAffinity, GetWindowLongW, GetWindowRect,
    SetWindowDisplayAffinity, SetWindowLongW, SetWindowPos, ShowWindow, SystemParametersInfoW,
    GWL_EXSTYLE, HWND_TOPMOST, SPI_GETCLIENTAREAANIMATION, SWP_FRAMECHANGED, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_HIDE, SW_SHOWNOACTIVATE,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WDA_EXCLUDEFROMCAPTURE, WS_EX_APPWINDOW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
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
/// `ShowWindow(SW_SHOW)`, qui active la fenêtre (window_state.rs, `apply_diff`). Le
/// drapeau « visible » de tao reste donc faux, et son `hide()` ne fait plus rien
/// (`apply_diff` sort tout de suite quand rien ne change) : on cache par
/// `cacher_sans_tao`.
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

/// Cache par `ShowWindow(SW_HIDE)`, symétrique de `montrer_sans_activer`.
pub fn cacher_sans_tao(fenetre: &tauri::WebviewWindow) -> Result<bool, String> {
    let h = hwnd(fenetre)?;
    // SAFETY : handle valide d'une fenêtre de ce processus.
    unsafe {
        let _ = ShowWindow(h, SW_HIDE);
    }
    Ok(true)
}

/// Fenêtre « outil » : ni barre des tâches ni Alt+Tab, jamais activée. tao pose
/// `WS_EX_APPWINDOW` pour une fenêtre sans parent (window.rs, `ON_TASKBAR`) et ne le
/// réapplique qu'en changeant ses drapeaux, ce qu'on ne fait plus sous Windows.
pub fn preparer_fenetre_outil(fenetre: &tauri::WebviewWindow) -> Result<bool, String> {
    let h = hwnd(fenetre)?;
    // SAFETY : lecture et écriture du style étendu d'une fenêtre de ce processus.
    unsafe {
        let actuel = GetWindowLongW(h, GWL_EXSTYLE);
        let voulu = (actuel & !(WS_EX_APPWINDOW.0 as i32))
            | (WS_EX_TOOLWINDOW.0 as i32)
            | (WS_EX_NOACTIVATE.0 as i32);
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

/// Le handle de la fenêtre, pour le journal (les essais le relisent).
pub fn identifiant_fenetre(fenetre: &tauri::WebviewWindow) -> Option<isize> {
    hwnd(fenetre).ok().map(|h| h.0 as isize)
}

/// Position de la souris en pixels réels (`GetCursorPos`), sans passer par le fil principal.
/// Vrai si le bouton gauche ou droit de la souris est enfoncé en ce moment.
pub fn bouton_souris_enfonce() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};
    // SAFETY : simple lecture de l'état des boutons.
    unsafe {
        (GetAsyncKeyState(i32::from(VK_LBUTTON.0)) as u16 & 0x8000) != 0
            || (GetAsyncKeyState(i32::from(VK_RBUTTON.0)) as u16 & 0x8000) != 0
    }
}

pub fn position_souris() -> Option<(f64, f64)> {
    let mut p = POINT::default();
    // SAFETY : `p` reçoit la position.
    unsafe { GetCursorPos(&mut p) }.ok()?;
    Some((f64::from(p.x), f64::from(p.y)))
}

/// Rectangle réel de la fenêtre et échelle de son écran (DPI de la fenêtre / 96).
/// `identifiant` : le HWND gardé à la création (sans aller-retour vers le fil principal).
pub fn rectangle_fenetre(identifiant: isize) -> Option<(i32, i32, u32, u32, f64)> {
    if identifiant == 0 {
        return None;
    }
    let h = HWND(identifiant as *mut core::ffi::c_void);
    let mut r = RECT::default();
    // SAFETY : handle valide ; `r` reçoit le rectangle.
    unsafe { GetWindowRect(h, &mut r) }.ok()?;
    // SAFETY : handle valide.
    let dpi = unsafe { GetDpiForWindow(h) };
    let echelle = if dpi == 0 { 1.0 } else { f64::from(dpi) / 96.0 };
    Some((
        r.left,
        r.top,
        (r.right - r.left).max(0) as u32,
        (r.bottom - r.top).max(0) as u32,
        echelle,
    ))
}

// ---------------------------------------------------------------------------------------
// Sélection (étape 5) : une fenêtre outil qui prend le focus, posée sur tout le bureau.
// Montrée et cachée sans tao, comme le bandeau : `show()` de tao réécrit tous les styles
// étendus (window_state.rs, `apply_diff`), ce qui remettrait `WS_EX_APPWINDOW` et
// retirerait `WS_EX_TOOLWINDOW` (la fenêtre entrerait dans Alt+Tab).

use ::windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_TRANSITIONS_FORCEDISABLED};
use ::windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, IsWindow, SetForegroundWindow, ShowWindowAsync,
};

/// Une poignée gardée sous forme de nombre ; `None` pour 0.
fn poignee(identifiant: isize) -> Option<HWND> {
    (identifiant != 0).then_some(HWND(identifiant as *mut core::ffi::c_void))
}

/// Fenêtre outil qui peut être activée : `WS_EX_TOOLWINDOW`, sans `WS_EX_APPWINDOW` (posé
/// par tao pour une fenêtre sans parent, window.rs `ON_TASKBAR`) ni `WS_EX_NOACTIVATE`.
/// Les fondus d'apparition de Windows sont coupés (aucune animation d'entrée).
pub fn preparer_fenetre_outil_active(fenetre: &tauri::WebviewWindow) -> Result<bool, String> {
    let h = hwnd(fenetre)?;
    // SAFETY : lecture et écriture du style étendu d'une fenêtre de ce processus.
    unsafe {
        let actuel = GetWindowLongW(h, GWL_EXSTYLE);
        let voulu = (actuel & !(WS_EX_APPWINDOW.0 as i32) & !(WS_EX_NOACTIVATE.0 as i32))
            | (WS_EX_TOOLWINDOW.0 as i32);
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
    let oui = ::windows::core::BOOL(1);
    // SAFETY : un BOOL dont la taille est passée.
    let fondu = unsafe {
        DwmSetWindowAttribute(
            h,
            DWMWA_TRANSITIONS_FORCEDISABLED,
            &oui as *const ::windows::core::BOOL as *const core::ffi::c_void,
            std::mem::size_of::<::windows::core::BOOL>() as u32,
        )
    };
    if let Err(e) = fondu {
        log::warn!("selection: fondu de Windows non coupe: {e}");
    }
    Ok(true)
}

/// Pose la fenêtre sur `rect` (pixels réels), au-dessus de tout, et la montre, sans
/// l'activer (l'activation est `prendre_premier_plan`). `SetWindowPos` synchrone : tao
/// passe `SWP_ASYNCWINDOWPOS` (window.rs, `set_outer_position`), on ne pourrait pas relire.
pub fn poser_au_dessus(fenetre: &tauri::WebviewWindow, rect: super::Rect) -> Result<bool, String> {
    let h = hwnd(fenetre)?;
    // SAFETY : handle valide d'une fenêtre de ce processus ; aucun tampon.
    unsafe {
        SetWindowPos(
            h,
            Some(HWND_TOPMOST),
            rect.x,
            rect.y,
            rect.largeur,
            rect.hauteur,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )
    }
    .map_err(|e| format!("SetWindowPos: {e}"))?;
    Ok(true)
}

/// Donne le premier plan et le clavier à la fenêtre. Windows lui envoie alors
/// `WM_SETFOCUS`, que wry fait suivre à la page (webview2/mod.rs, `parent_subclass_proc`,
/// `MoveFocus`). `SetForegroundWindow` réussit après un raccourci global (le processus a
/// reçu la dernière entrée). Aucun autre essai s'il est refusé : rattacher notre file
/// d'entrée à celle d'une application figée (`AttachThreadInput`) pourrait bloquer le fil
/// principal pendant que l'écran est couvert. Rend vrai si la fenêtre est au premier plan.
pub fn prendre_premier_plan(fenetre: &tauri::WebviewWindow) -> Result<bool, String> {
    let h = hwnd(fenetre)?;
    // SAFETY : handle d'une fenêtre de ce processus ; aucun tampon.
    unsafe {
        if GetForegroundWindow() != h {
            let _ = SetForegroundWindow(h);
        }
        Ok(GetForegroundWindow() == h)
    }
}

/// Place `dessous` juste en dessous de `dessus` dans l'ordre des fenêtres, sans l'activer
/// ni la déplacer. Les deux sont « toujours au-dessus » : `dessous` reste dans ce groupe.
pub fn placer_sous(dessous: isize, dessus: isize) -> Result<bool, String> {
    let (Some(bas), Some(haut)) = (poignee(dessous), poignee(dessus)) else {
        return Err("poignee nulle".into());
    };
    // SAFETY : simples poignées ; `IsWindow` évite d'agir sur une fenêtre disparue.
    unsafe {
        if !IsWindow(Some(bas)).as_bool() {
            return Err("fenetre du dessous disparue".into());
        }
        SetWindowPos(
            bas,
            Some(haut),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
    }
    .map_err(|e| format!("SetWindowPos: {e}"))?;
    Ok(true)
}

/// La fenêtre au premier plan (`GetForegroundWindow`), `None` s'il n'y en a pas.
pub fn fenetre_premier_plan() -> Option<isize> {
    // SAFETY : sans paramètre.
    let h = unsafe { GetForegroundWindow() };
    (!h.0.is_null()).then_some(h.0 as isize)
}

/// Remet `identifiant` au premier plan s'il désigne encore une fenêtre (`IsWindow`).
pub fn rendre_premier_plan(identifiant: isize) -> bool {
    let Some(h) = poignee(identifiant) else {
        return false;
    };
    // SAFETY : simple poignée, vérifiée par `IsWindow`.
    unsafe { IsWindow(Some(h)).as_bool() && SetForegroundWindow(h).as_bool() }
}

/// Cache une fenêtre depuis n'importe quel fil sans attendre son fil
/// (`ShowWindowAsync(SW_HIDE)`).
pub fn cacher_fenetre_async(identifiant: isize) -> bool {
    let Some(h) = poignee(identifiant) else {
        return false;
    };
    // SAFETY : simple poignée, vérifiée par `IsWindow` ; l'appel ne fait que poster.
    unsafe { IsWindow(Some(h)).as_bool() && ShowWindowAsync(h, SW_HIDE).as_bool() }
}
