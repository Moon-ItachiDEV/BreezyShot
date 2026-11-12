//! Hors Windows : pas de presse-papiers natif ici (`sortie` passe par `arboard`).

use super::ErreurPressePapiers;

pub const DISPONIBLE: bool = false;

pub struct Preparee;

pub fn preparer(
    _png: &[u8],
    _largeur: u32,
    _hauteur: u32,
    _rgba_sur_blanc: &[u8],
) -> Result<Preparee, ErreurPressePapiers> {
    Err(ErreurPressePapiers::NonDisponible)
}

pub fn poser(_image: &mut Preparee) -> Result<(), ErreurPressePapiers> {
    Err(ErreurPressePapiers::NonDisponible)
}

#[cfg(test)]
pub fn lire_dib() -> Option<Vec<u8>> {
    None
}
