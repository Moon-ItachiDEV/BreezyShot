//! `PlaySoundW` sur un WAV en mémoire, en asynchrone : l'appel rend la main tout de suite.

use std::sync::OnceLock;

use ::windows::core::{w, PCWSTR};
use ::windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
use ::windows::Win32::System::LibraryLoader::{LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32};

/// Charge `winmm.dll` une seule fois, à la demande, depuis System32 seulement.
fn winmm_prete() -> bool {
    static PRETE: OnceLock<bool> = OnceLock::new();
    *PRETE.get_or_init(|| {
        // SAFETY : nom constant ; chargé au plus une fois par processus (jamais libéré).
        unsafe { LoadLibraryExW(w!("winmm.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) }.is_ok()
    })
}

pub fn jouer(wav: &'static [u8]) {
    // `winmm.dll` est chargée à la demande (build.rs) : d'abord depuis System32 seulement,
    // comme pour la minuterie fine ; si elle manque, pas de son (et pas d'appel, qui ferait
    // échouer le chargement différé).
    if !winmm_prete() {
        return;
    }
    // SAFETY : `wav` est `'static` (compilé dans l'exécutable), donc valide pendant toute la
    // lecture asynchrone ; avec SND_MEMORY le « nom » est l'adresse du fichier, pas un texte.
    // Un échec (pas de sortie audio) est ignoré : la capture n'en dépend pas.
    let _ = unsafe {
        PlaySoundW(
            PCWSTR(wav.as_ptr().cast()),
            None,
            SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
        )
    };
}
