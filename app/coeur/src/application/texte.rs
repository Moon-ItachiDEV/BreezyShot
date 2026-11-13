//! Copier le texte, côté décisions (contrat 0.5, §5.4 à §5.6) : fonctions pures, sans
//! Tauri ni système. Les gestes sont dans `geste_texte.rs`.
//!
//! Socle : signatures (chantier E pour les corps).

use std::time::{Duration, Instant};

use crate::sortie::Etape;
use crate::texte::Issue;

/// Vrai quand Copier le texte est construit (chantier E) : seulement alors l'action
/// `texte` est construite (si une langue existe). Faux au socle : rien n'en paraît.
pub const CONSTRUIT: bool = false;

/// Le balayage sur la zone lue, et sa version réduite.
pub const BALAYAGE: Duration = Duration::from_millis(400);
pub const BALAYAGE_REDUIT: Duration = Duration::from_millis(120);

/// L'écran figé n'est jamais gardé plus longtemps après le relâcher.
pub const ECRAN_FIGE_MAX_LECTURE: Duration = Duration::from_millis(1500);

/// L'action du bandeau « Aucun texte trouvé ».
pub const ACTION_REESSAYER: &str = "reessayer";

/// La lecture en cours (état Lecture), du relâcher au résultat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LectureEnCours {
    pub numero: u64,
    pub relache: Instant,
}

/// Ce que le fil `texte` rend au fil principal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinLecture {
    pub numero: u64,
    pub issue: Issue,
    /// La copie du texte assemblé, si un texte a été lu.
    pub copie: Option<Etape>,
    pub duree: Duration,
}
