//! Hors Windows : pas de sélecteur de dossier ni de dialogue « Enregistrer sous » natifs
//! pour l'instant.

use std::path::{Path, PathBuf};

pub fn choisir_dossier(
    _proprietaire: Option<isize>,
    _depart: Option<PathBuf>,
) -> Result<Option<PathBuf>, String> {
    Err("non disponible".into())
}

pub fn choisir_fichier_png(
    _proprietaire: Option<isize>,
    _depart: Option<&Path>,
    _nom: &str,
) -> Result<Option<PathBuf>, String> {
    // Hors Windows : comme un dialogue annulé (contrat 0.4 « Éditeur », §7.3, §14).
    Ok(None)
}
