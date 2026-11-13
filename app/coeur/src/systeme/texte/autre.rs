//! Hors Windows : pas de moteur de lecture (macOS : `macos.rs` viendra, par Vision).

use super::{Disponibilite, ErreurMoteur, Place, Reconnu};

pub const FIN_DE_LIGNE: &str = "\n";

pub fn disponibilite() -> Disponibilite {
    Disponibilite::NonPrisEnCharge
}

pub fn dimension() -> Result<u32, ErreurMoteur> {
    Err(ErreurMoteur::AucuneLangue)
}

pub fn reconnaitre(
    _bgra: &[u8],
    _largeur: u32,
    _hauteur: u32,
    _place: Place<'static>,
) -> Result<Reconnu, ErreurMoteur> {
    Err(ErreurMoteur::AucuneLangue)
}
