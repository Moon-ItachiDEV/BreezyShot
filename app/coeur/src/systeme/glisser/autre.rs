//! Hors Windows : pas de glisser-déposer natif pour l'instant (macOS viendra ensuite).

use std::path::Path;

use super::Depot;

pub fn glisser_fichier(_fenetre: isize, _chemin: &Path) -> Result<Depot, String> {
    Err("glisser non disponible".into())
}

pub fn bouton_gauche_enfonce() -> bool {
    false
}

pub fn fenetre_sous(_point: (i32, i32)) -> Option<isize> {
    None
}
