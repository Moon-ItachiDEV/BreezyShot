//! Le sélecteur de dossier du système (« Parcourir… » de la fenêtre Réglages, étape 10).
//!
//! `choisir_dossier` bloque jusqu'à la réponse de l'utilisateur : l'appelant le lance dans
//! un fil à part, jamais sur le fil principal (un raccourci de capture ne doit pas tourner
//! dans la boucle du dialogue). Windows : `IFileOpenDialog` en mode dossier, modal à la
//! fenêtre donnée. Hors Windows : « non disponible ».

use std::path::PathBuf;

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
