//! L'historique, côté décisions (contrat 0.5, §4.2) : fonctions pures, sans Tauri ni
//! système. Les gestes sont dans `geste_historique.rs`, les commandes de la page dans
//! `fenetre_historique.rs`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::{Duration, Instant};

use serde::Serialize;

use super::editeur;
use super::zone::DELAI_SORTIE;
use crate::sortie::{self, Bilan, EntreeRecente, Etape, EtapeFichier};

/// Vrai quand l'historique est construit (chantier D) : seulement alors l'action
/// `historique` est construite (raccourci, menu), et seulement avec une licence (`droits`).
pub const CONSTRUIT: bool = true;

/// Délai d'« Annuler » après Supprimer.
pub const DELAI_ANNULER: Duration = Duration::from_secs(5);

/// Sans `historique_pret` de la page dans ce délai, la fenêtre est détruite.
pub const DELAI_PAGE: Duration = Duration::from_secs(30);

/// Dossier des aperçus, à côté de « Récents » (`<local>/Apercus`) : celui de `sortie`.
pub const SOUS_DOSSIER_APERCUS: &str = crate::sortie::SOUS_DOSSIER_APERCUS;

/// Messages de la page (`historique:resultat`).
pub const MESSAGE_COPIER: &str = "Impossible de copier";
pub const MESSAGE_ENREGISTRER: &str = "Impossible d'enregistrer";
pub const MESSAGE_SUPPRIMER: &str = "Impossible de supprimer";

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
    /// Numéro de la fenêtre : un `Destroyed` d'une fenêtre précédente ne compte pas.
    pub numero: u64,
    pub page_prete: bool,
    /// La demande d'ouverture (chronos `pret` et `affiche`).
    pub ouverte_a: Instant,
    /// `historique affiche` déjà écrit.
    pub affichee: bool,
    /// La dernière liste lue sur le disque (la plus récente d'abord).
    pub liste: Vec<EntreeRecente>,
    /// Noms dont la suppression attend la fin du délai d'Annuler.
    pub en_attente: Vec<String>,
    /// Noms pris par l'échéance, la fermeture ou « Tout effacer », dont l'effacement est en
    /// cours : jamais remontrés, toute action refusée.
    pub en_effacement: BTreeSet<String>,
    pub echeance: Option<Instant>,
    /// Noms enregistrés depuis l'historique pendant la session.
    pub enregistrees: BTreeSet<String>,
    /// Copier, Enregistrer ou Annoter en cours, par nom et action (`cle_action`).
    pub actions: BTreeMap<String, Instant>,
    pub glisser: bool,
}

impl EtatHistorique {
    pub fn nouveau(numero: u64, ouverte_a: Instant) -> EtatHistorique {
        EtatHistorique {
            numero,
            page_prete: false,
            ouverte_a,
            affichee: false,
            liste: Vec::new(),
            en_attente: Vec::new(),
            en_effacement: BTreeSet::new(),
            echeance: None,
            enregistrees: BTreeSet::new(),
            actions: BTreeMap::new(),
            glisser: false,
        }
    }

    /// Les cases montrées.
    pub fn vues(&self) -> Vec<EntreePage> {
        let caches: Vec<String> = self
            .en_attente
            .iter()
            .chain(&self.en_effacement)
            .cloned()
            .collect();
        entrees_vues(&self.liste, &caches)
    }

    /// Vrai si `nom` a le format exact, est dans la liste gardée et n'est pas en cours
    /// d'effacement (une commande sur un autre nom est refusée).
    pub fn connu(&self, nom: &str) -> bool {
        nom_valide(nom)
            && !self.en_effacement.contains(nom)
            && self.liste.iter().any(|e| e.nom == nom)
    }

    /// L'effacement de `noms` est fini : ils quittent la liste gardée et l'ensemble en
    /// effacement.
    pub fn effaces(&mut self, noms: &[String]) {
        self.liste.retain(|e| !noms.contains(&e.nom));
        for n in noms {
            self.en_effacement.remove(n);
        }
    }

    /// Rang et taille de la case `nom` parmi les cases montrées (journal).
    pub fn rang(&self, nom: &str) -> Option<String> {
        self.vues()
            .iter()
            .position(|e| e.nom == nom)
            .map(|i| nom_court(i, &self.vues()[i]))
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

/// Vrai si `nom` a le format exact d'un fichier de « Récents » (la règle de `sortie`).
pub fn nom_valide(nom: &str) -> bool {
    sortie::chemin_recent(Path::new("."), nom).is_some()
}

/// Les cases montrées : la liste sans les noms en attente de suppression, 50 au plus.
pub fn entrees_vues(liste: &[EntreeRecente], en_attente: &[String]) -> Vec<EntreePage> {
    liste
        .iter()
        .filter(|e| !en_attente.contains(&e.nom))
        .take(sortie::RECENTS_GARDES)
        .map(|e| EntreePage {
            nom: e.nom.clone(),
            horodatage_ms: e.horodatage.timestamp_millis(),
            largeur: e.largeur,
            hauteur: e.hauteur,
            annotable: editeur::annotable(e.largeur, e.hauteur),
        })
        .collect()
}

/// La clé d'une action en cours dans `EtatHistorique::actions`.
pub fn cle_action(nom: &str, action: ActionCase) -> String {
    format!("{} {nom}", action.nom())
}

/// Ce que fait `action` sur la case `nom` : ignorée pour un nom qui n'est pas montré (en
/// attente de suppression, en effacement, inconnu), une image que l'éditeur refuse, ou une
/// copie (un enregistrement, une ouverture dans l'éditeur) de la même case encore en cours
/// depuis moins de `DELAI_SORTIE`.
pub fn decider(etat: &EtatHistorique, nom: &str, action: ActionCase, maintenant: Instant) -> Geste {
    let Some(entree) = etat.vues().into_iter().find(|e| e.nom == nom) else {
        return Geste::Ignore;
    };
    let en_cours = etat
        .actions
        .get(&cle_action(nom, action))
        .is_some_and(|debut| maintenant.saturating_duration_since(*debut) < DELAI_SORTIE);
    match action {
        _ if en_cours => Geste::Ignore,
        ActionCase::Copier => Geste::Copier,
        ActionCase::Enregistrer => Geste::Enregistrer {
            deja_fait: etat.enregistrees.contains(nom),
        },
        ActionCase::Annoter if !entree.annotable => Geste::Ignore,
        ActionCase::Annoter => Geste::Annoter,
        ActionCase::Supprimer => Geste::Supprimer,
    }
}

/// Met `nom` en attente de suppression (l'échéance repart : `DELAI_ANNULER` après la
/// dernière) ; rend le nombre en attente.
pub fn ajouter_suppression(etat: &mut EtatHistorique, nom: String, maintenant: Instant) -> usize {
    if !etat.en_attente.contains(&nom) {
        etat.en_attente.push(nom);
    }
    etat.echeance = Some(maintenant + DELAI_ANNULER);
    etat.en_attente.len()
}

/// Annule toute la série en attente ; rend les noms remis.
pub fn annuler_suppressions(etat: &mut EtatHistorique) -> Vec<String> {
    etat.echeance = None;
    std::mem::take(&mut etat.en_attente)
}

/// Prend toute la série en attente pour l'effacer (échéance, fermeture, arrêt) : ses noms
/// passent en effacement (jamais remontrés, même par une liste relue avant la fin).
pub fn prendre_suppressions(etat: &mut EtatHistorique) -> Vec<String> {
    let noms = annuler_suppressions(etat);
    etat.en_effacement.extend(noms.iter().cloned());
    noms
}

/// « Tout effacer » : la série en attente et toutes les cases de la liste passent en
/// effacement (une liste relue avant la fin du vidage ne les remontre pas).
pub fn prendre_tout(etat: &mut EtatHistorique) -> Vec<String> {
    let mut noms = prendre_suppressions(etat);
    for e in &etat.liste {
        if !noms.contains(&e.nom) {
            noms.push(e.nom.clone());
        }
    }
    etat.en_effacement.extend(noms.iter().cloned());
    noms
}

/// Une capture fait-elle son aperçu à la sortie ? Si l'historique est une action en
/// vigueur (licence), ou si sa fenêtre est ouverte (licence retirée pendant qu'elle l'est :
/// elle reste utilisable jusqu'à sa fermeture). Sinon rien n'est écrit dans `Apercus`.
pub fn apercu_a_la_sortie(actions_en_vigueur: &[&str], fenetre_ouverte: bool) -> bool {
    fenetre_ouverte || actions_en_vigueur.contains(&super::ACTION_HISTORIQUE)
}

/// Ce que fait une demande d'ouverture de l'historique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ouverture {
    /// Ouvert et prêt : ramené devant.
    Ramener,
    /// Ouvert, page pas encore prête : elle se montrera à `historique_pret`.
    Rien,
    /// Fenêtre encore en train d'être détruite : nouvel essai plus tard.
    Attendre,
    /// Plus d'essai : abandon.
    Abandonner,
    Creer,
}

/// `page_prete` : `None` sans historique ouvert ; `fenetre` : la fenêtre existe encore.
pub fn ouverture(page_prete: Option<bool>, fenetre: bool, essais: u32) -> Ouverture {
    match (page_prete, fenetre) {
        (Some(true), _) => Ouverture::Ramener,
        (Some(false), _) => Ouverture::Rien,
        (None, true) if essais == 0 => Ouverture::Abandonner,
        (None, true) => Ouverture::Attendre,
        (None, false) => Ouverture::Creer,
    }
}

/// Le filet tombe : la fenêtre `numero` est toujours là, sa page n'a jamais dit prête.
pub fn page_muette(etat: &EtatHistorique, numero: u64) -> bool {
    etat.numero == numero && !etat.page_prete
}

/// La vignette se ferme si elle montre (non partie) une capture de `noms` (`None` : toutes,
/// « Tout effacer ») ; `recent` : son fichier de « Récents ».
pub fn vignette_a_fermer(recent: Option<&Path>, partie: bool, noms: Option<&[String]>) -> bool {
    !partie
        && recent
            .and_then(nom_du_chemin)
            .is_some_and(|n| noms.is_none_or(|l| l.contains(&n)))
}

/// Vrai `DELAI_ANNULER` après la dernière suppression de la série.
pub fn suppressions_echues(etat: &EtatHistorique, maintenant: Instant) -> bool {
    !etat.en_attente.is_empty() && etat.echeance.is_some_and(|e| maintenant >= e)
}

/// Le message de la page pour une action en échec, ou rien (réussie, ou sans message).
pub fn message_echec(action: ActionCase, bilan: &Bilan) -> Option<&'static str> {
    match action {
        ActionCase::Copier => (bilan.copie != Etape::Faite).then_some(MESSAGE_COPIER),
        ActionCase::Enregistrer => {
            (!matches!(bilan.enregistrement, EtapeFichier::Fait(_))).then_some(MESSAGE_ENREGISTRER)
        }
        ActionCase::Annoter | ActionCase::Supprimer => None,
    }
}

/// Le message d'une action en échec avant `sortir` (capture illisible).
pub fn message_illisible(action: ActionCase) -> Option<&'static str> {
    match action {
        ActionCase::Copier => Some(MESSAGE_COPIER),
        ActionCase::Enregistrer => Some(MESSAGE_ENREGISTRER),
        ActionCase::Annoter | ActionCase::Supprimer => None,
    }
}

/// Le nom d'une case au journal : son rang dans la liste montrée et sa taille
/// (`0 1024x768`), jamais son nom de fichier.
pub fn nom_court(rang: usize, entree: &EntreePage) -> String {
    format!("{rang} {}x{}", entree.largeur, entree.hauteur)
}

/// La ligne de journal d'une action (jamais le nom complet : le rang et la taille).
pub fn ligne_action(nom_court: &str, action: ActionCase) -> String {
    format!("historique action {nom_court} {}", action.nom())
}

/// La ligne de journal d'une action finie (`historique copie <rang> faite|echec`…).
pub fn ligne_resultat(nom_court: &str, action: ActionCase, etat: EtatResultat) -> String {
    let issue = match (action, etat) {
        (ActionCase::Copier, EtatResultat::Fait) => "faite",
        (ActionCase::Enregistrer, EtatResultat::Fait) => "fait",
        (_, EtatResultat::DejaFait) => "deja fait",
        (_, _) => "echec",
    };
    let quoi = match action {
        ActionCase::Copier => "copie",
        ActionCase::Enregistrer => "enregistrement",
        ActionCase::Annoter => "annoter",
        ActionCase::Supprimer => "suppression",
    };
    format!("historique {quoi} {nom_court} {issue}")
}

/// L'issue d'une action pour la page (`historique:resultat.etat`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatResultat {
    Fait,
    DejaFait,
    Echec,
}

impl EtatResultat {
    pub fn nom(self) -> &'static str {
        match self {
            EtatResultat::Fait => "fait",
            EtatResultat::DejaFait => "deja_fait",
            EtatResultat::Echec => "echec",
        }
    }
}

/// Pourquoi la fenêtre s'est fermée (`historique ferme (page|cadre|arret)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaisonFermeture {
    Page,
    Cadre,
    Arret,
}

pub fn ligne_fermeture(raison: RaisonFermeture) -> &'static str {
    match raison {
        RaisonFermeture::Page => "historique ferme (page)",
        RaisonFermeture::Cadre => "historique ferme (cadre)",
        RaisonFermeture::Arret => "historique ferme (arret)",
    }
}

/// Les noms partis de « Récents » entre deux listes (leur aperçu peut partir aussi).
pub fn noms_partis(avant: &BTreeSet<String>, apres: &[EntreeRecente]) -> Vec<String> {
    let presents: BTreeSet<&str> = apres.iter().map(|e| e.nom.as_str()).collect();
    avant
        .iter()
        .filter(|n| !presents.contains(n.as_str()))
        .cloned()
        .collect()
}

/// Le nom de fichier d'un chemin de « Récents », s'il a le format exact.
pub fn nom_du_chemin(chemin: &Path) -> Option<String> {
    let nom = chemin.file_name()?.to_str()?;
    nom_valide(nom).then(|| nom.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use std::path::PathBuf;

    fn entree(nom: &str, l: u32, h: u32) -> EntreeRecente {
        EntreeRecente {
            nom: nom.into(),
            horodatage: Utc.with_ymd_and_hms(2026, 11, 13, 10, 0, 0).unwrap(),
            largeur: l,
            hauteur: h,
        }
    }

    const A: &str = "20261113-100000-000.png";
    const B: &str = "20261113-095959-999_2.png";
    const C: &str = "20261113-090000-000.png";

    fn etat() -> (EtatHistorique, Instant) {
        let t = Instant::now();
        let mut e = EtatHistorique::nouveau(1, t);
        e.liste = vec![entree(A, 1024, 768), entree(B, 800, 600), entree(C, 1, 1)];
        (e, t)
    }

    #[test]
    fn noms_au_format_exact() {
        assert!(nom_valide(A));
        assert!(nom_valide(B));
        assert!(!nom_valide("20261113-100000-000_1.png"));
        assert!(!nom_valide("20261113-100000-000_02.png"));
        assert!(!nom_valide("../20261113-100000-000.png"));
        assert!(!nom_valide("..\\20261113-100000-000.png"));
        assert!(!nom_valide("20261113-100000-000.PNG"));
        assert!(!nom_valide("2026111a-100000-000.png"));
        assert!(!nom_valide("capture.png"));
        assert!(!nom_valide(""));
        assert_eq!(
            nom_du_chemin(&PathBuf::from("/l/Recents").join(A)),
            Some(A.to_string())
        );
        assert_eq!(nom_du_chemin(Path::new("/l/Recents/x.png")), None);
    }

    #[test]
    fn actions_par_nom() {
        for a in ActionCase::TOUTES {
            assert_eq!(ActionCase::depuis_nom(a.nom()), Some(a));
        }
        assert_eq!(ActionCase::depuis_nom("Copier"), None);
        assert_eq!(ActionCase::depuis_nom("epingler"), None);
    }

    #[test]
    fn vues_sans_les_suppressions_en_attente() {
        let (e, _) = etat();
        let vues = entrees_vues(&e.liste, &[B.to_string()]);
        assert_eq!(
            vues.iter().map(|v| v.nom.as_str()).collect::<Vec<_>>(),
            [A, C]
        );
        assert_eq!(vues[0].horodatage_ms, 1_794_564_000_000);
        assert!(vues[0].annotable);
        let json = serde_json::to_string(&vues[1]).unwrap();
        assert_eq!(
            json,
            format!(
                "{{\"nom\":\"{C}\",\"horodatage_ms\":1794564000000,\"largeur\":1,\
                 \"hauteur\":1,\"annotable\":true}}"
            )
        );
        // 50 au plus.
        let longue: Vec<EntreeRecente> = (0..60).map(|_| entree(A, 10, 10)).collect();
        assert_eq!(entrees_vues(&longue, &[]).len(), sortie::RECENTS_GARDES);
        // Trop grande pour l'éditeur : pas annotable.
        let geante = [entree(A, 100_000, 10)];
        assert!(!entrees_vues(&geante, &[])[0].annotable);
    }

    #[test]
    fn decisions() {
        let (mut e, t) = etat();
        assert_eq!(decider(&e, A, ActionCase::Copier, t), Geste::Copier);
        assert_eq!(
            decider(&e, A, ActionCase::Enregistrer, t),
            Geste::Enregistrer { deja_fait: false }
        );
        assert_eq!(decider(&e, A, ActionCase::Annoter, t), Geste::Annoter);
        assert_eq!(decider(&e, A, ActionCase::Supprimer, t), Geste::Supprimer);
        // Inconnu, ou au format mais absent de la liste.
        assert_eq!(decider(&e, "x.png", ActionCase::Copier, t), Geste::Ignore);
        let absent = "20200101-000000-000.png";
        assert_eq!(decider(&e, absent, ActionCase::Copier, t), Geste::Ignore);
        // Action en cours : ignorée moins de DELAI_SORTIE ; l'autre action non.
        e.actions.insert(cle_action(A, ActionCase::Copier), t);
        assert_eq!(decider(&e, A, ActionCase::Copier, t), Geste::Ignore);
        assert_eq!(
            decider(&e, A, ActionCase::Enregistrer, t),
            Geste::Enregistrer { deja_fait: false }
        );
        assert_eq!(
            decider(&e, A, ActionCase::Copier, t + DELAI_SORTIE),
            Geste::Copier
        );
        // Annoter en cours : une seconde ouverture est ignorée (Entrée tenue).
        e.actions.insert(cle_action(A, ActionCase::Annoter), t);
        assert_eq!(decider(&e, A, ActionCase::Annoter, t), Geste::Ignore);
        assert_eq!(decider(&e, A, ActionCase::Supprimer, t), Geste::Supprimer);
        e.actions.remove(&cle_action(A, ActionCase::Annoter));
        assert_eq!(decider(&e, A, ActionCase::Annoter, t), Geste::Annoter);
        // Déjà enregistrée pendant la session.
        e.enregistrees.insert(B.to_string());
        assert_eq!(
            decider(&e, B, ActionCase::Enregistrer, t),
            Geste::Enregistrer { deja_fait: true }
        );
        // En attente de suppression : tout est ignoré.
        ajouter_suppression(&mut e, B.to_string(), t);
        for a in ActionCase::TOUTES {
            assert_eq!(decider(&e, B, a, t), Geste::Ignore);
        }
        // Image refusée par l'éditeur : pas d'Annoter.
        e.liste[2] = entree(C, 100_000, 10);
        assert_eq!(decider(&e, C, ActionCase::Annoter, t), Geste::Ignore);
        assert_eq!(decider(&e, C, ActionCase::Copier, t), Geste::Copier);
    }

    #[test]
    fn suppressions_en_serie() {
        let (mut e, t) = etat();
        assert!(!suppressions_echues(&e, t + DELAI_ANNULER));
        assert_eq!(ajouter_suppression(&mut e, A.into(), t), 1);
        assert_eq!(ajouter_suppression(&mut e, A.into(), t), 1);
        let t2 = t + Duration::from_secs(3);
        assert_eq!(ajouter_suppression(&mut e, B.into(), t2), 2);
        // 5 s après la dernière, pas après la première.
        assert!(!suppressions_echues(&e, t + DELAI_ANNULER));
        assert!(!suppressions_echues(
            &e,
            t2 + DELAI_ANNULER - Duration::from_millis(1)
        ));
        assert!(suppressions_echues(&e, t2 + DELAI_ANNULER));
        assert_eq!(e.rang(C).as_deref(), Some("0 1x1"));
        // Annuler remet toute la série.
        assert_eq!(annuler_suppressions(&mut e), vec![A.to_string(), B.into()]);
        assert!(e.en_attente.is_empty() && e.echeance.is_none());
        assert!(!suppressions_echues(&e, t2 + DELAI_ANNULER));
        assert_eq!(e.rang(B).as_deref(), Some("1 800x600"));
        // Fermeture : tout de suite, sans attendre l'échéance.
        ajouter_suppression(&mut e, C.into(), t);
        assert_eq!(prendre_suppressions(&mut e), vec![C.to_string()]);
        assert!(e.en_attente.is_empty());
        assert_eq!(e.rang(C), None);
        assert!(!e.connu(C));
        // Une liste relue avant la fin de l'effacement ne la remontre pas.
        e.liste = vec![entree(A, 1024, 768), entree(B, 800, 600), entree(C, 1, 1)];
        assert_eq!(e.vues().len(), 2);
        for a in ActionCase::TOUTES {
            assert_eq!(decider(&e, C, a, t), Geste::Ignore);
        }
        // Effacement fini : partie de la liste et de l'ensemble.
        e.effaces(&[C.to_string()]);
        assert!(e.en_effacement.is_empty());
        assert_eq!(e.liste.len(), 2);
        assert!(e.connu(A) && !e.connu(C) && !e.connu("x.png"));
    }

    #[test]
    fn messages() {
        let bilan = |copie, enregistrement| Bilan {
            copie,
            recent: None,
            enregistrement,
        };
        let fait = bilan(Etape::Faite, EtapeFichier::Fait(PathBuf::from("/a.png")));
        let rate = bilan(
            Etape::Echec("x".into()),
            EtapeFichier::Echec {
                dossier: PathBuf::new(),
                cause: "x".into(),
            },
        );
        assert_eq!(message_echec(ActionCase::Copier, &fait), None);
        assert_eq!(message_echec(ActionCase::Enregistrer, &fait), None);
        assert_eq!(
            message_echec(ActionCase::Copier, &rate),
            Some("Impossible de copier")
        );
        assert_eq!(
            message_echec(ActionCase::Enregistrer, &rate),
            Some("Impossible d'enregistrer")
        );
        assert_eq!(message_echec(ActionCase::Annoter, &rate), None);
        assert_eq!(message_illisible(ActionCase::Copier), Some(MESSAGE_COPIER));
        assert_eq!(MESSAGE_SUPPRIMER, "Impossible de supprimer");
    }

    #[test]
    fn lignes_du_journal_sans_nom() {
        let (e, _) = etat();
        let court = e.rang(A).unwrap();
        assert_eq!(court, "0 1024x768");
        let lignes = [
            ligne_action(&court, ActionCase::Supprimer),
            ligne_resultat(&court, ActionCase::Copier, EtatResultat::Fait),
            ligne_resultat(&court, ActionCase::Copier, EtatResultat::Echec),
            ligne_resultat(&court, ActionCase::Enregistrer, EtatResultat::Fait),
            ligne_resultat(&court, ActionCase::Enregistrer, EtatResultat::DejaFait),
        ];
        assert_eq!(
            lignes,
            [
                "historique action 0 1024x768 supprimer",
                "historique copie 0 1024x768 faite",
                "historique copie 0 1024x768 echec",
                "historique enregistrement 0 1024x768 fait",
                "historique enregistrement 0 1024x768 deja fait",
            ]
        );
        for l in &lignes {
            assert!(!l.contains(".png") && !l.contains("2026"), "{l}");
        }
        assert_eq!(
            ligne_fermeture(RaisonFermeture::Cadre),
            "historique ferme (cadre)"
        );
        assert_eq!(EtatResultat::DejaFait.nom(), "deja_fait");
    }

    #[test]
    fn decision_d_ouverture() {
        assert_eq!(ouverture(Some(true), true, 20), Ouverture::Ramener);
        assert_eq!(ouverture(Some(false), true, 20), Ouverture::Rien);
        assert_eq!(ouverture(None, true, 20), Ouverture::Attendre);
        assert_eq!(ouverture(None, true, 1), Ouverture::Attendre);
        assert_eq!(ouverture(None, true, 0), Ouverture::Abandonner);
        assert_eq!(ouverture(None, false, 0), Ouverture::Creer);
    }

    #[test]
    fn filet_de_la_page() {
        let (mut e, _) = etat();
        assert!(page_muette(&e, 1));
        assert!(!page_muette(&e, 2));
        e.page_prete = true;
        assert!(!page_muette(&e, 1));
    }

    #[test]
    fn vignette_concernee() {
        let recent = PathBuf::from("/l/Recents").join(A);
        let noms = [A.to_string()];
        let autres = [B.to_string()];
        assert!(vignette_a_fermer(Some(&recent), false, Some(&noms)));
        assert!(!vignette_a_fermer(Some(&recent), false, Some(&autres)));
        assert!(vignette_a_fermer(Some(&recent), false, None));
        assert!(!vignette_a_fermer(Some(&recent), true, None));
        assert!(!vignette_a_fermer(None, false, None));
        let autre = Path::new("/l/Recents/capture.png");
        assert!(!vignette_a_fermer(Some(autre), false, None));
    }

    #[test]
    fn noms_partis_entre_deux_listes() {
        let avant: BTreeSet<String> = [A, B, C].iter().map(|s| s.to_string()).collect();
        let apres = [entree(A, 1, 1), entree(C, 1, 1)];
        assert_eq!(noms_partis(&avant, &apres), vec![B.to_string()]);
        assert!(noms_partis(&BTreeSet::new(), &apres).is_empty());
    }

    #[test]
    fn tout_effacer_prend_toutes_les_cases() {
        let (mut e, t) = etat();
        ajouter_suppression(&mut e, B.to_string(), t);
        let mut pris = prendre_tout(&mut e);
        pris.sort();
        let mut attendus = vec![A.to_string(), B.to_string(), C.to_string()];
        attendus.sort();
        assert_eq!(pris, attendus);
        assert!(e.en_attente.is_empty());
        // Une liste relue avant la fin du vidage ne remontre aucune case.
        e.liste = vec![entree(A, 1024, 768), entree(B, 800, 600), entree(C, 1, 1)];
        assert!(e.vues().is_empty());
        assert!(!e.connu(A));
        e.effaces(&pris);
        assert!(e.liste.is_empty() && e.en_effacement.is_empty());
    }

    #[test]
    fn apercu_seulement_avec_l_historique() {
        let sans = ["zone", "plein_ecran", "fenetre", "derniere_zone"];
        assert!(apercu_a_la_sortie(&["zone", "historique"], false));
        // Sans licence et fenêtre fermée : aucun aperçu, comme la 0.4.
        assert!(!apercu_a_la_sortie(&sans, false));
        // Licence retirée, fenêtre encore ouverte : l'aperçu est fait.
        assert!(apercu_a_la_sortie(&sans, true));
        assert!(apercu_a_la_sortie(
            &crate::application::actions_construites(true, false),
            false
        ));
        assert!(!apercu_a_la_sortie(
            &crate::application::actions_construites(false, true),
            false
        ));
    }
}
