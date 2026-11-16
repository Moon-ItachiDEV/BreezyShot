//! `timeBeginPeriod(1)` et `timeEndPeriod(1)` : la résolution de la minuterie du système.

use ::windows::Win32::Media::{timeBeginPeriod, timeEndPeriod};

/// 1 ms demandée si Windows l'accepte ; rendue à la destruction.
#[derive(Debug)]
pub struct Fine {
    prise: bool,
}

impl Fine {
    pub fn demander() -> Fine {
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
