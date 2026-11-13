//! L'historique : le dossier « Recents » lu, et ses apercus (contrat 0.5, §3.1).
//!
//! Le dossier fait foi, sans index. Une entree est un fichier au format exact des noms de
//! « Recents » ; un autre nom n'est jamais montre ni efface.
//!
//! Socle : signatures (chantier B pour les corps).

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use crate::systeme::gel::Pixels;

/// Une capture de « Recents ».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntreeRecente {
    pub nom: String,
    /// Lu dans le nom (UTC, millisecondes).
    pub horodatage: DateTime<Utc>,
    /// Lues dans l'en-tete IHDR.
    pub largeur: u32,
    pub hauteur: u32,
}

/// Les entrees du dossier, la plus recente d'abord ; un fichier illisible est ecarte.
pub fn lister_recents(_dossier: &Path) -> Vec<EntreeRecente> {
    Vec::new()
}

/// Le chemin de `nom` dans `dossier` si `nom` a le format exact, sinon `None`.
pub fn chemin_recent(_dossier: &Path, _nom: &str) -> Option<PathBuf> {
    None
}

/// Relit un PNG de « Recents » (RGBA 8 bits) ; le booleen dit s'il a un alpha < 255.
pub fn lire_recent(_chemin: &Path) -> Result<(Pixels, bool), String> {
    Err("non construit".into())
}

/// Ecrit l'apercu de `nom` dans `dossier_apercus`.
pub fn ranger_apercu(_dossier_apercus: &Path, _nom: &str, _apercu: &Pixels) -> Result<(), String> {
    Err("non construit".into())
}

/// Le chemin de l'apercu de `nom` s'il a le format exact, sinon `None`.
pub fn chemin_apercu(_dossier_apercus: &Path, _nom: &str) -> Option<PathBuf> {
    None
}

/// Efface le PNG de « Recents » et son apercu.
pub fn supprimer_recent(
    _dossier_recents: &Path,
    _dossier_apercus: &Path,
    _nom: &str,
) -> Result<(), String> {
    Err("non construit".into())
}

/// Ce que « Tout effacer » a fait.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BilanVidage {
    pub effaces: usize,
    pub echecs: usize,
}

/// Efface tout ce qui a le format de « Recents » et des apercus, et les `.png` de
/// `dossier_glisser`. Rien d'autre.
pub fn vider(
    _dossier_recents: &Path,
    _dossier_apercus: &Path,
    _dossier_glisser: &Path,
) -> BilanVidage {
    BilanVidage::default()
}
