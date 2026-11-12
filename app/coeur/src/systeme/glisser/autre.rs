//! Hors Windows : pas de glisser-déposer natif pour l'instant (macOS viendra ensuite).

use std::path::Path;

use super::{BoutonPhysique, Depot};

pub fn glisser_fichier(_fenetre: isize, _chemin: &Path) -> Result<Depot, String> {
    Err("glisser non disponible".into())
}

pub fn boutons_inverses() -> bool {
    false
}

pub fn bouton_enfonce(_bouton: BoutonPhysique) -> bool {
    false
}

pub fn fenetre_sous(_point: (i32, i32)) -> Option<isize> {
    None
}
