//! Raccourcis globaux (étape 3) : notation, choix au démarrage, annonces, traduction vers
//! les types du système. Tout est pur et testé sans écran, sauf `reservation.rs` (mince).
//!
//! Câblage : `reservation.rs` reçoit le `GlobalShortcut` de `tauri-plugin-global-shortcut`
//! (par l'`AppHandle`) et une fonction de rappel `Fn(Signal)` donnée par l'Application. Il
//! appelle `choisir` avec les candidats des réglages, `systeme::impr_ecran_reserve()` (passé
//! par l'Application) et un `essayer` qui fait `vers_systeme` puis la réservation ; à chaque
//! appui, il retrouve l'action par `Bilan::action_de` et envoie `Signal::Presse(action)`.
//! L'Application lit la dernière situation dans `<local>/etat.json`, appelle
//! `decider_annonce`, montre l'annonce éventuelle dans le bandeau, écrit ou efface la
//! situation, et met `Bilan::libelle_menu("zone")` dans le menu.

mod annonces;
mod choix;
mod conversion;
mod notation;
mod reservation;

pub use annonces::{decider_annonce, Annonce, Decision, Situation};
pub use choix::{choisir, Bilan, ChoixAction, Ecarte, Raison, ACTIONS_PRIORITAIRES};
pub use conversion::vers_systeme;
pub use notation::{
    analyser, ErreurNotation, Modificateurs, Raccourci, Touche, RESERVES_AU_SYSTEME,
};
pub use reservation::{liberer, lignes_journal, reserver, texte_raison};

/// Ce que le module signale à l'Application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Signal {
    /// Le raccourci de l'action nommée (`zone`, `plein_ecran`, `fenetre`…) a été pressé.
    Presse(String),
}
