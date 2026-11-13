//! L'historique, côté décisions (contrat 0.5, §4.2) : fonctions pures, sans Tauri ni
//! système. Les gestes sont dans `geste_historique.rs`, les commandes de la page dans
//! `fenetre_historique.rs`.
//!
//! Socle : signatures (chantier D pour les corps).

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::sortie::{Bilan, EntreeRecente};

/// Vrai quand l'historique est construit (chantier D) : seulement alors l'action
/// `historique` est construite (raccourci, menu). Faux au socle : rien n'en paraît.
pub const CONSTRUIT: bool = false;

/// Délai d'« Annuler » après Supprimer.
pub const DELAI_ANNULER: Duration = Duration::from_secs(5);

/// Taille couverte par l'aperçu d'une case.
pub const APERCU: (u32, u32) = (400, 250);

/// Les actions d'une case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionCase {
    Copier,
    Enregistrer,
    Annoter,
    Supprimer,
}

impl ActionCase {
    pub const TOUTES: [ActionCase; 4] = [
        ActionCase::Copier,
        ActionCase::Enregistrer,
        ActionCase::Annoter,
        ActionCase::Supprimer,
    ];

    /// Le nom de la commande `historique_action` et du journal.
    pub fn nom(self) -> &'static str {
        match self {
            ActionCase::Copier => "copier",
            ActionCase::Enregistrer => "enregistrer",
            ActionCase::Annoter => "annoter",
            ActionCase::Supprimer => "supprimer",
        }
    }

    /// L'action d'un nom exact ; `None` sinon.
    pub fn depuis_nom(nom: &str) -> Option<ActionCase> {
        ActionCase::TOUTES.into_iter().find(|a| a.nom() == nom)
    }
}

/// L'historique ouvert (fil principal ; oublié à la destruction de la fenêtre).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EtatHistorique {
    pub page_prete: bool,
    pub ouverte_a: Instant,
    /// Noms dont la suppression attend la fin du délai d'Annuler.
    pub en_attente: Vec<String>,
    pub echeance: Option<Instant>,
    /// Noms enregistrés depuis l'historique pendant la session.
    pub enregistrees: BTreeSet<String>,
    /// Copier ou Enregistrer en cours, par nom.
    pub actions: BTreeMap<String, Instant>,
    pub glisser: bool,
}

impl EtatHistorique {
    pub fn nouveau(ouverte_a: Instant) -> EtatHistorique {
        EtatHistorique {
            page_prete: false,
            ouverte_a,
            en_attente: Vec::new(),
            echeance: None,
            enregistrees: BTreeSet::new(),
            actions: BTreeMap::new(),
            glisser: false,
        }
    }
}

/// Une case telle que la page la reçoit (`historique:liste`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EntreePage {
    pub nom: String,
    pub horodatage_ms: i64,
    pub largeur: u32,
    pub hauteur: u32,
    pub annotable: bool,
}

/// Ce que fait une action sur une case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Geste {
    Ignore,
    Copier,
    Enregistrer { deja_fait: bool },
    Annoter,
    Supprimer,
}

/// Vrai si `nom` a le format exact d'un fichier de « Récents ».
pub fn nom_valide(_nom: &str) -> bool {
    false
}

/// Les cases montrées : la liste sans les noms en attente de suppression.
pub fn entrees_vues(_liste: &[EntreeRecente], _en_attente: &[String]) -> Vec<EntreePage> {
    Vec::new()
}

/// Ce que fait `action` sur la case `nom`.
pub fn decider(
    _etat: &EtatHistorique,
    _nom: &str,
    _action: ActionCase,
    _maintenant: Instant,
) -> Geste {
    Geste::Ignore
}

/// Met `nom` en attente de suppression ; rend le nombre en attente.
pub fn ajouter_suppression(etat: &mut EtatHistorique, _nom: String, _maintenant: Instant) -> usize {
    etat.en_attente.len()
}

/// Annule toute la série en attente ; rend les noms remis.
pub fn annuler_suppressions(_etat: &mut EtatHistorique) -> Vec<String> {
    Vec::new()
}

/// Vrai `DELAI_ANNULER` après la dernière suppression de la série.
pub fn suppressions_echues(_etat: &EtatHistorique, _maintenant: Instant) -> bool {
    false
}

/// Le message de la page pour une action en échec, ou rien.
pub fn message_echec(_action: ActionCase, _bilan: &Bilan) -> Option<&'static str> {
    None
}

/// La ligne de journal d'une action (jamais le nom complet : le rang et la taille).
pub fn ligne_action(nom_court: &str, action: ActionCase) -> String {
    format!("historique action {nom_court} {}", action.nom())
}
