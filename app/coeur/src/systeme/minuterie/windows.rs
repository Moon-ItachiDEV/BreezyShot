//! `timeBeginPeriod(1)` et `timeEndPeriod(1)` : la résolution de la minuterie du système.

use std::sync::OnceLock;

use ::windows::core::w;
use ::windows::Win32::Media::{timeBeginPeriod, timeEndPeriod};
use ::windows::Win32::System::LibraryLoader::{LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32};

/// Charge `winmm.dll` une seule fois, à la demande, depuis System32 seulement.
fn winmm_prete() -> bool {
    static PRETE: OnceLock<bool> = OnceLock::new();
    *PRETE.get_or_init(|| {
        // SAFETY : nom constant ; chargé au plus une fois par processus (jamais libéré).
        unsafe { LoadLibraryExW(w!("winmm.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) }.is_ok()
    })
}

/// 1 ms demandée si Windows l'accepte ; rendue à la destruction.
#[derive(Debug)]
pub struct Fine {
    prise: bool,
}

impl Fine {
    pub fn demander() -> Fine {
        // `winmm.dll` est chargée à la demande (build.rs) : d'abord depuis System32 seulement,
        // comme les DLL du film ; si elle manque, pas de minuterie fine (et pas d'appel).
        if !winmm_prete() {
            return Fine { prise: false };
        }
        // SAFETY : aucun tampon ; 0 (`TIMERR_NOERROR`) dit que la demande est prise.
        let prise = unsafe { timeBeginPeriod(1) } == 0;
        Fine { prise }
    }
}

impl Drop for Fine {
    fn drop(&mut self) {
        if self.prise {
            // SAFETY : rend la demande faite par `demander`.
            unsafe { timeEndPeriod(1) };
        }
    }
}

#[cfg(test)]
mod tests {
    /// Demandée puis rendue, deux fois de suite, sans panique.
    #[test]
    fn demander_et_rendre() {
        drop(crate::systeme::minuterie::Fine::demander());
        let _a = crate::systeme::minuterie::Fine::demander();
        let _b = crate::systeme::minuterie::Fine::demander();
    }
}
