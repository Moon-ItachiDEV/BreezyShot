//! Gestes propres à Windows. Lecture seule du registre : l'app n'y écrit jamais.

use ::windows::core::{w, PCWSTR};
use ::windows::Win32::Foundation::ERROR_SUCCESS;
use ::windows::Win32::System::Registry::{
    RegCloseKey, RegGetValueW, RegNotifyChangeKeyValue, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER,
    HKEY_LOCAL_MACHINE, KEY_NOTIFY, KEY_READ, REG_NOTIFY_CHANGE_LAST_SET, RRF_RT_REG_DWORD,
    RRF_RT_REG_SZ,
};
use ::windows::Win32::System::Threading::GetCurrentProcess;
use ::windows::Win32::UI::HiDpi::{
    AreDpiAwarenessContextsEqual, GetDpiAwarenessContextForProcess,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use ::windows::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
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

/// Fil de surveillance : attend une modification de la clé Personalize, relit le thème,
/// et prévient seulement s'il a changé.
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
            let mut actuel = super::theme_barre();
            loop {
                // SAFETY : appel synchrone sur une clé ouverte ; bloque jusqu'au changement.
                let r = unsafe {
                    RegNotifyChangeKeyValue(cle, false, REG_NOTIFY_CHANGE_LAST_SET, None, false)
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
            }
            // SAFETY : clé ouverte plus haut.
            unsafe {
                let _ = RegCloseKey(cle);
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
