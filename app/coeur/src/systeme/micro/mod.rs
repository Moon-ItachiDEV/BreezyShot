//! Le micro (contrat 0.7, §13.1, F17, F18) : WASAPI, le point de capture par défaut du rôle
//! `eConsole` (celui de « Son › Entrée »), en mode partagé, converti par Windows en PCM
//! 48 kHz mono 16 bits. L'appareil n'est ouvert que pendant que « Micro » est actif ; le
//! son du système n'est jamais pris.
//!
//! Façade commune : `windows.rs` la réalise, `autre.rs` dit « absent » ailleurs.
//!
//! Socle : signatures ; `windows.rs` ne fait encore rien (chantier C).

#[cfg(windows)]
#[path = "windows.rs"]
mod plateforme;

#[cfg(not(windows))]
#[path = "autre.rs"]
mod plateforme;

/// Vrai si le PC a un point de capture actif (le bouton « Micro » est montré).
pub fn present() -> bool {
    plateforme::present()
}

/// Pourquoi le micro ne s'ouvre pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Erreur {
    /// Aucun point de capture.
    Absent,
    /// Refusé par Windows (confidentialité) ou pris en exclusif.
    Refuse,
    Autre(String),
}

/// Le micro ouvert. À sa destruction, le fil `film-son` s'arrête et l'appareil est rendu.
#[derive(Debug)]
pub struct Micro {
    interne: plateforme::Micro,
}

/// Ouvre le micro : `rappel` reçoit du PCM 48 kHz mono 16 bits, depuis le fil `film-son` ;
/// il ne fait que ranger (`film::Calage`).
pub fn ouvrir(rappel: impl FnMut(&[i16]) + Send + 'static) -> Result<Micro, Erreur> {
    plateforme::ouvrir(Box::new(rappel)).map(|interne| Micro { interne })
}

impl Micro {
    /// Vrai si l'appareil a été débranché ou repris par Windows depuis l'ouverture.
    pub fn perdu(&self) -> bool {
        plateforme::perdu(&self.interne)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sans micro (Linux ; Windows tant que le chantier C n'est pas fait) : rien ne
    /// s'ouvre, sans panique.
    #[test]
    fn absent_sans_panique() {
        if !present() {
            assert!(ouvrir(|_| {}).is_err());
        }
    }
}
