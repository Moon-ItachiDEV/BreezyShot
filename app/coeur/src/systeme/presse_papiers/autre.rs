//! Hors Windows : pas de presse-papiers natif ici (`sortie` passe par `arboard`).

use super::ErreurPressePapiers;

pub const DISPONIBLE: bool = false;

pub fn poser_image(
    _png: &[u8],
    _largeur: u32,
    _hauteur: u32,
    _rgba_sur_blanc: &[u8],
) -> Result<(), ErreurPressePapiers> {
    Err(ErreurPressePapiers::NonDisponible)
}
