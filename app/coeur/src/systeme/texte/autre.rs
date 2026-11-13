//! Hors Windows : pas de moteur de lecture (macOS : `macos.rs` viendra, par Vision).

use std::time::Duration;

use super::{Disponibilite, ErreurMoteur, Reconnu};

pub const FIN_DE_LIGNE: &str = "\n";

pub fn disponibilite() -> Disponibilite {
    Disponibilite::NonPrisEnCharge
}

pub fn dimension_max() -> u32 {
    0
}

pub fn reconnaitre(_bgra: &[u8], _largeur: u32, _hauteur: u32) -> Result<Reconnu, ErreurMoteur> {
    Err(ErreurMoteur::AucuneLangue)
}

pub fn derniere_creation() -> Option<Duration> {
    None
}
