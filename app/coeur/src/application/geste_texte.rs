//! Copier le texte, côté gestes (contrat 0.5, §5) : la lecture dans le fil `texte`, son
//! échéance, le résultat. Les décisions sont dans `texte.rs`.
//!
//! Points d'accroche de `mod.rs` : `lancer_sonde` (démarrage, fil `texte-langues`, qui
//! poste `Evenement::TexteDisponible`), `zone_choisie` (zone choisie en mode texte),
//! `texte_lu` et `echu` (`Evenement::TexteLu`, `Evenement::TexteEchu`), `lire_export`
//! (bouton « Texte » de l'éditeur, export de but `texte`), `raccourci_reserve`, `arreter`.
//!
//! Socle : inerte (chantier E).

use tauri::AppHandle;

use super::editeur::EnTete;
use super::texte::FinLecture;
use crate::raccourcis::ChoixAction;
use crate::systeme::gel::Pixels;
use crate::systeme::Rect;

/// Démarrage : cherche les langues de lecture dans un petit fil, puis poste
/// `Evenement::TexteDisponible`. Socle : rien (la fonction reste absente).
pub fn lancer_sonde(_app: &AppHandle) {}

/// Le raccourci de `texte` vient d'être réservé (ou non) à l'arrivée des langues de
/// lecture, après le démarrage : l'annonce « Aucun raccourci pour le texte » éventuelle.
pub fn raccourci_reserve(_app: &AppHandle, _choix: &ChoixAction) {}

/// Zone choisie en mode texte (fil principal, état Sélection, capture encore rangée).
/// Socle : la capture est abandonnée sans message.
pub fn zone_choisie(app: &AppHandle, _zone: Rect) {
    super::abandonner(app, super::etats::Declencheur::SelectionAnnulee, None);
}

/// Le résultat d'une lecture (fil principal).
pub fn texte_lu(_app: &AppHandle, _fin: FinLecture) {}

/// L'échéance de la lecture `numero` est tombée (fil principal).
pub fn echu(_app: &AppHandle, _numero: u64) {}

/// L'image composée de l'éditeur à lire (export `texte` accepté, fil principal).
pub fn lire_export(_app: &AppHandle, _numero: u64, _entete: EnTete, _pixels: Pixels) {}

/// Arrêt de l'app : la lecture en cours est oubliée.
pub fn arreter(app: &AppHandle) {
    super::lecture_en_cours(app).take();
}
