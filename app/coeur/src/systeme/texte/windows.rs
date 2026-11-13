//! `Windows.Media.Ocr` (chantier A). Socle : réponses inertes, la fonction reste absente.

use super::{Disponibilite, ErreurMoteur, Reconnu};

pub const FIN_DE_LIGNE: &str = "\r\n";

pub fn disponibilite() -> Disponibilite {
    Disponibilite::NonPrisEnCharge
}

pub fn dimension_max() -> u32 {
    0
}

pub fn reconnaitre(_bgra: &[u8], _largeur: u32, _hauteur: u32) -> Result<Reconnu, ErreurMoteur> {
    Err(ErreurMoteur::AucuneLangue)
}
