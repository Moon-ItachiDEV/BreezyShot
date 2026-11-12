//! Hors Windows : pas de sélecteur de dossier natif pour l'instant.

use std::path::PathBuf;

pub fn choisir_dossier(
    _proprietaire: Option<isize>,
    _depart: Option<PathBuf>,
) -> Result<Option<PathBuf>, String> {
    Err("non disponible".into())
}
