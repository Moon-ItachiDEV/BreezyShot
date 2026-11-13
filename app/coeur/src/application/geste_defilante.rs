//! Capture défilante, côté gestes (contrat 0.6, §5, §18) : la visée en mode défilante, la
//! fenêtre de contrôle créée au début de la visée (R1, par `plus_tard`, jamais dans un
//! rappel), les pages proposées (fil `defilante-pages`), le défilement et l'assemblage
//! (fil `defilante`), l'écoute de la molette, Entrée et Échap en global, la fin vers la
//! sortie commune. Les décisions sont dans `defilante.rs`.
//!
//! Points d'accroche de `mod.rs`, tous sur le fil principal :
//! - `capturer` : la séquence du mode une fois en Gel (`apres_gel`) ;
//! - `selection_montree` : la Sélection est montrée en mode défilante (affichage `id`,
//!   Échap déjà réservé) ; les pages se proposent par `super::proposer_pages` ;
//! - `zone_choisie` : zone tracée (`page` : `None`) ou page cliquée (`page` : son indice
//!   dans la liste proposée), en Sélection, la capture encore rangée (`ranger_capture`) ;
//! - `visee_abandonnee` : la capture en mode défilante a été abandonnée pendant la visée
//!   (Échap, annulée, écrans, muette…), écran figé et sélection déjà cachés ;
//! - `signal_selection`, `echap`, `bureau_change` : vrais s'ils ont été pris ici (en
//!   Défilement) ; faux, le chemin commun de la Sélection et du décompte suit ;
//! - `entree` : Entrée globale (`raccourcis::Signal::Entree`) ;
//! - `traiter` : ce que les fils postent (`Evenement::Defilante`) ;
//! - `arreter` : arrêt de l'app (`defilante annulee arret`).
//!
//! La fin passe par `super::lancer_sortie(app, pixels, zone, duree, relache, false, true)`
//! (aperçu du haut, D16), après `DefilementFini`.
//!
//! Socle : corps inertes (chantier D) ; rien n'est appelé tant que la défilante n'est pas
//! une action construite.

use std::time::Instant;

use tauri::AppHandle;

use super::etats::{Declencheur, Mode};
use crate::fenetres::selection;
use crate::systeme::gel::Changement;
use crate::systeme::Rect;

/// Ce que les fils de la défilante postent au fil principal (`Evenement::Defilante`).
#[derive(Debug, Clone, PartialEq)]
pub enum Signal {}

/// La capture défilante en cours (de la visée à la fin), tenue par l'Application
/// (`super::defilante_en_cours`). Fil principal ; verrou jamais tenu pendant un appel à
/// Tauri ou à un module.
#[derive(Debug, Default)]
pub struct EnCours {}

/// La séquence du mode, en Gel : contrôles, photo sans pointeur (D1), écran figé,
/// Sélection en mode défilante, Échap réservé (le chemin commun).
pub fn capturer(app: &AppHandle, recu_a: Instant, bandeau_cache: bool) {
    super::capturer_avec_selection(app, recu_a, Mode::Defilante, bandeau_cache);
}

/// La Sélection est montrée en mode défilante (affichage `id`). Socle : rien (les pages
/// se proposeront par `super::proposer_pages`).
pub fn selection_montree(app: &AppHandle, id: u64) {
    let _ = (app, id, super::proposer_pages);
}

/// Zone tracée ou page cliquée (en Sélection, capture encore rangée). Socle : la capture
/// est abandonnée sans message.
pub fn zone_choisie(app: &AppHandle, zone: Rect, page: Option<usize>) {
    let _ = (zone, page);
    super::abandonner(app, Declencheur::SelectionAnnulee, None);
}

/// La visée en mode défilante est abandonnée (écran figé et sélection déjà cachés).
pub fn visee_abandonnee(app: &AppHandle) {
    // Pris sous le verrou, détruit hors du verrou.
    let _ancienne = super::defilante_en_cours(app).take();
}

/// Un signal de la Sélection : vrai s'il est pris ici.
pub fn signal_selection(app: &AppHandle, signal: &selection::Signal) -> bool {
    let _ = (app, signal);
    false
}

/// Échap global : vrai s'il est pris ici.
pub fn echap(app: &AppHandle) -> bool {
    let _ = app;
    false
}

/// Entrée globale (= « Terminer »).
pub fn entree(app: &AppHandle) {
    let _ = app;
}

/// Écrans changés ou session verrouillée : vrai s'ils sont pris ici.
pub fn bureau_change(app: &AppHandle, changement: Changement) -> bool {
    let _ = (app, changement);
    false
}

/// Ce que les fils postent.
pub fn traiter(app: &AppHandle, signal: Signal) {
    let _ = app;
    match signal {}
}

/// Arrêt de l'app : la défilante en cours est annulée, sa fenêtre détruite.
pub fn arreter(app: &AppHandle) {
    // Pris sous le verrou, détruit hors du verrou.
    let _ancienne = super::defilante_en_cours(app).take();
}
