//! L'historique, côté gestes (contrat 0.5, §3 et §4) : ouvrir la fenêtre, tenir son état,
//! faire les actions des cases dans des fils. Les décisions sont dans `historique.rs`.
//!
//! Points d'accroche de `mod.rs` : `ouvrir` (raccourci, menu), `traiter`
//! (`Evenement::Historique`), `apres_sortie` (fil de sortie et fil d'export de l'éditeur,
//! une fois « Récents » écrit), `rafraichir` (après une suppression de la vignette),
//! `arreter` (arrêt de l'app).
//!
//! Socle : inerte (chantier D).

use tauri::AppHandle;

use crate::sortie;
use crate::systeme::gel::Pixels;

/// Ce qui revient au fil principal pour l'historique (`Evenement::Historique`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    /// La fenêtre est détruite.
    Detruite,
}

/// Raccourci ou menu : ouvre la fenêtre, ou la ramène devant. Jamais créée ici : toujours
/// postée à un tour suivant de la boucle (`super::plus_tard_nomme`, fil
/// `historique-poste`).
pub fn ouvrir(_app: &AppHandle) {}

/// Un signal de l'historique (fil principal).
pub fn traiter(_app: &AppHandle, _signal: Signal) {}

/// Après une sortie qui a rangé un fichier dans « Récents » (`bilan.recent`) : son aperçu,
/// puis la liste à jour si la fenêtre est ouverte. Appelé dans le fil de la sortie, jamais
/// sur le fil principal.
pub fn apres_sortie(_app: &AppHandle, _pixels: &Pixels, _bilan: &sortie::Bilan) {}

/// « Récents » a changé (suppression de la vignette) : la liste à jour si la fenêtre est
/// ouverte. N'importe quel fil.
pub fn rafraichir(_app: &AppHandle) {}

/// Arrêt de l'app : suppressions en attente faites, fenêtre détruite, état oublié.
pub fn arreter(app: &AppHandle) {
    super::historique_ouvert(app).take();
}
