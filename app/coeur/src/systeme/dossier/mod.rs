//! Le sélecteur de dossier du système (« Parcourir… » de la fenêtre Réglages, étape 10) et
//! le dialogue « Enregistrer sous » (éditeur, 0.4).
//!
//! `choisir_dossier` et `choisir_fichier_png` bloquent jusqu'à la réponse de l'utilisateur :
//! l'appelant les lance dans un fil à part, jamais sur le fil principal (un raccourci de
//! capture ne doit pas tourner dans la boucle du dialogue). Windows : `IFileOpenDialog` en
//! mode dossier, `IFileSaveDialog` pour le PNG, modaux à la fenêtre donnée. Hors Windows :
//! « non disponible » pour le dossier, `Ok(None)` (annulé) pour le PNG.

use std::path::{Path, PathBuf};

#[cfg(windows)]
#[path = "windows.rs"]
mod plateforme;

#[cfg(not(windows))]
#[path = "autre.rs"]
mod plateforme;

/// Montre le sélecteur de dossier. `proprietaire` : la fenêtre qu'il bloque (HWND), ou
/// rien. `depart` : le dossier montré d'abord, s'il existe. `Ok(None)` : annulé.
pub fn choisir_dossier(
    proprietaire: Option<isize>,
    depart: Option<PathBuf>,
) -> Result<Option<PathBuf>, String> {
    plateforme::choisir_dossier(proprietaire, depart)
}

/// Montre le dialogue « Enregistrer sous » du système pour un PNG : modal à `proprietaire`,
/// ouvert sur `depart` s'il existe, nom proposé `nom`, filtre PNG, extension `.png`
/// imposée, remplacement confirmé par le système. `Ok(None)` : annulé.
pub fn choisir_fichier_png(
    proprietaire: Option<isize>,
    depart: Option<&Path>,
    nom: &str,
) -> Result<Option<PathBuf>, String> {
    plateforme::choisir_fichier_png(proprietaire, depart, nom)
}
