//! Décisions de la vignette (fonctions pures, ARCHITECTURE §11) : ce qu'elle montre, ce
//! que fait chaque bouton, ce que Supprimer efface, ce qu'on dit au bandeau et au journal.
//! Les gestes sont dans `application/mod.rs`, les fenêtres dans `fenetres/vignette.rs`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};

use super::etats::{Declencheur, Etat};
use super::zone::{message_annonce, DELAI_SORTIE};
use crate::fenetres::vignette::{ActionVignette, Coin, Place};
use crate::fenetres::{self, Genre, Message};
use crate::reglages::{self, Animations};
use crate::sortie::{self, Annonce, Bilan, Etape, EtapeFichier, Suppression};
use crate::systeme::{self, gel::Pixels, glisser::Depot};

/// Durée pendant laquelle Supprimer peut être annulé.
pub const DELAI_ANNULATION: Duration = Duration::from_secs(4);
/// Plus grand côté de l'aperçu envoyé aux pages, pixels.
pub const APERCU_COTE_MAX: u32 = 1600;
/// Sous-dossier du dossier local où sont posés les fichiers donnés au glisser.
pub const SOUS_DOSSIER_GLISSER: &str = "Glisser";

/// Identifiants des actions (aussi dans la page et l'essai 07).
pub const COPIER: &str = "copier";
pub const ENREGISTRER: &str = "enregistrer";
pub const SUPPRIMER: &str = "supprimer";
pub const FERMER: &str = "fermer";
pub const ANNULER: &str = "annuler";

/// La vignette se montre si le réglage le veut et si ses fenêtres existent.
pub fn doit_montrer(afficher: bool, vignette_creee: bool) -> bool {
    afficher && vignette_creee
}

/// Après la sortie : `ImagePrete` (souffle puis vignette) ou `ImagePreteSansVignette`
/// (bandeau, comme en 0.2) si la vignette ne doit pas ou ne peut pas se montrer.
pub fn issue(doit_montrer: bool, apercu_ok: bool) -> Declencheur {
    if doit_montrer && apercu_ok {
        Declencheur::ImagePrete
    } else {
        Declencheur::ImagePreteSansVignette
    }
}

/// `duree_s` du réglage ; 0 = toujours (`None`).
pub fn duree_affichage(duree_s: u32) -> Option<Duration> {
    (duree_s > 0).then(|| Duration::from_secs(u64::from(duree_s)))
}

/// Animations réduites : d'après le réglage, ou celui de Windows (`systeme`) s'il dit
/// « système ».
pub fn animations_reduites(reglage: Animations, systeme: bool) -> bool {
    match reglage {
        Animations::Systeme => systeme,
        Animations::Normales => false,
        Animations::Reduites => true,
    }
}

pub fn coin(reglage: reglages::Coin) -> Coin {
    match reglage {
        reglages::Coin::BasGauche => Coin::BasGauche,
        reglages::Coin::BasDroite => Coin::BasDroite,
        reglages::Coin::HautGauche => Coin::HautGauche,
        reglages::Coin::HautDroite => Coin::HautDroite,
    }
}

fn action(id: &str, libelle: &str, fait: Option<&str>, place: Place) -> ActionVignette {
    ActionVignette {
        id: id.into(),
        libelle: libelle.into(),
        libelle_fait: fait.map(Into::into),
        place,
        picto: id.into(),
    }
}

/// Les boutons de la carte, dans l'ordre de la page : seulement ce qui marche (ni Annoter
/// ni Épingler en 0.3).
pub fn actions() -> Vec<ActionVignette> {
    vec![
        action(COPIER, "Copier", Some("Copié"), Place::Principale),
        action(
            ENREGISTRER,
            "Enregistrer",
            Some("Enregistré"),
            Place::Principale,
        ),
        action(SUPPRIMER, "Supprimer", None, Place::Secondaire),
        action(FERMER, "Fermer", None, Place::Fermer),
        action(ANNULER, "Annuler", None, Place::Annulation),
    ]
}

/// Le message du bandeau après la sortie : avec la vignette, seulement une erreur (la
/// vignette confirme les réussites) ; sans, comme en 0.2.
pub fn message_bandeau(annonce: &Annonce, avec_vignette: bool) -> Option<Message> {
    match annonce {
        Annonce::Reussite { .. } if avec_vignette => None,
        _ => Some(message_annonce(annonce)),
    }
}

/// Le rectangle capturé, pour les fenêtres (côtés jamais négatifs).
pub fn rect_fenetres(r: systeme::Rect) -> fenetres::Rect {
    fenetres::Rect {
        x: r.x,
        y: r.y,
        largeur: r.largeur.max(0) as u32,
        hauteur: r.hauteur.max(0) as u32,
    }
}

/// Où en est Supprimer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatSuppression {
    Aucune,
    /// Carte « supprimée », rien d'effacé encore.
    EnAttente,
    /// Effacement lancé (fin du délai, nouvelle capture ou arrêt).
    Lancee,
}

/// La capture que montre la vignette, tenue par l'Application (une seule à la fois).
#[derive(Debug, Clone)]
pub struct CaptureMontree {
    /// Le numéro de la sortie, aussi celui de la carte.
    pub id: u64,
    /// Pixels pleine taille, gardés tant que la vignette est là.
    pub pixels: Arc<Pixels>,
    pub transparence: bool,
    /// L'heure de la capture (nom du fichier enregistré).
    pub maintenant: DateTime<Local>,
    /// Fichier de « Récents », s'il a été écrit.
    pub recent: Option<PathBuf>,
    /// Fichier enregistré pour cette capture (réglage ou bouton).
    pub enregistre: Option<PathBuf>,
    /// Fichiers posés pour le glisser.
    pub liens: Vec<PathBuf>,
    pub suppression: EtatSuppression,
    /// Copie et enregistrement lancés, depuis quand.
    pub copie: Option<Instant>,
    pub enregistrement: Option<Instant>,
    pub glisser: bool,
    pub reduites: bool,
    /// La carte est partie (sa fermeture est déjà au journal).
    pub partie: bool,
    /// L'annonce de la sortie (montrée au bandeau si la vignette ne peut pas l'être).
    pub annonce: Annonce,
    /// Relâchement de la souris (chrono), et le moment où le souffle part.
    pub relache: Instant,
    pub souffle_a: Option<Instant>,
}

/// Ce que fait un clic sur un bouton.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Geste {
    Copier,
    Enregistrer,
    DejaEnregistree,
    Supprimer,
    Annuler,
    Fermer,
    Ignorer,
}

fn en_cours(depuis: Option<Instant>, maintenant: Instant) -> bool {
    depuis.is_some_and(|d| maintenant.saturating_duration_since(d) < DELAI_SORTIE)
}

/// Ce que fait l'action `action` sur `capture` : pendant l'état « supprimée », seul
/// Annuler compte ; une action qui tourne encore (moins de `DELAI_SORTIE`) est ignorée.
pub fn decider(capture: &CaptureMontree, action: &str, maintenant: Instant) -> Geste {
    if capture.suppression != EtatSuppression::Aucune {
        return match (action, capture.suppression) {
            (ANNULER, EtatSuppression::EnAttente) => Geste::Annuler,
            _ => Geste::Ignorer,
        };
    }
    match action {
        COPIER if en_cours(capture.copie, maintenant) => Geste::Ignorer,
        COPIER => Geste::Copier,
        ENREGISTRER if capture.enregistre.is_some() => Geste::DejaEnregistree,
        ENREGISTRER if en_cours(capture.enregistrement, maintenant) => Geste::Ignorer,
        ENREGISTRER => Geste::Enregistrer,
        SUPPRIMER if capture.glisser => Geste::Ignorer,
        SUPPRIMER => Geste::Supprimer,
        FERMER => Geste::Fermer,
        _ => Geste::Ignorer,
    }
}

/// La demande de sortie d'un bouton : `base` (réglages du moment) pour Copier ou
/// Enregistrer seulement, jamais « Récents », à l'heure de la capture.
pub fn demande_action(
    base: &sortie::Demande,
    copier: bool,
    capture: &CaptureMontree,
) -> sortie::Demande {
    sortie::Demande {
        copier,
        enregistrer: !copier,
        ranger_recents: false,
        maintenant: capture.maintenant,
        transparence: capture.transparence,
        ..base.clone()
    }
}

/// Après une action : réussie ou non, et le message d'erreur du bandeau ; le fichier
/// enregistré s'il y en a un.
pub fn apres_action(action: &str, bilan: &Bilan) -> (bool, Option<Message>, Option<PathBuf>) {
    let erreur = |titre: &str, cause: &str| Message {
        genre: Genre::Erreur,
        titre: titre.into(),
        detail: cause.into(),
        actions: Vec::new(),
    };
    match action {
        COPIER => match &bilan.copie {
            Etape::Faite => (true, None, None),
            Etape::Echec(c) => (false, Some(erreur("Impossible de copier", c)), None),
            Etape::NonDemandee => (false, None, None),
        },
        _ => match &bilan.enregistrement {
            EtapeFichier::Fait(p) => (true, None, Some(p.clone())),
            EtapeFichier::Echec { cause, .. } => {
                (false, Some(erreur("Impossible d'enregistrer", cause)), None)
            }
            EtapeFichier::NonDemande => (false, None, None),
        },
    }
}

/// Les fichiers de cette capture que Supprimer efface (`sortie::a_effacer` refait le tri).
pub fn a_supprimer(capture: &CaptureMontree) -> Suppression {
    Suppression {
        recent: capture.recent.clone(),
        enregistre: capture.enregistre.clone(),
        liens: capture.liens.clone(),
    }
}

/// La carte se dit posée : le souffle part seulement si l'automate l'attend (état
/// Souffle) et la carte n'est pas partie ; une page rechargée plus tard ne le rejoue pas.
pub fn prete_acceptee(etat: Option<Etat>, partie: bool) -> bool {
    etat == Some(Etat::Souffle) && !partie
}

/// Nombre de captures supprimées retenues pour un Enregistrer qui tourne encore.
pub const RETENUES_MAX: usize = 8;

/// La capture quitte l'Application (carte partie, nouvelle capture, arrêt) : retenue si
/// son effacement est lancé alors qu'un Enregistrer tourne encore (son fichier arrivera
/// après).
pub fn a_retenir(capture: &CaptureMontree) -> bool {
    capture.suppression == EtatSuppression::Lancee && capture.enregistrement.is_some()
}

/// Ajoute `id` aux captures retenues, les plus anciennes oubliées au-delà de `RETENUES_MAX`.
pub fn retenir(retenues: &mut Vec<u64>, id: u64) {
    if !retenues.contains(&id) {
        retenues.push(id);
    }
    let trop = retenues.len().saturating_sub(RETENUES_MAX);
    retenues.drain(..trop);
}

/// Le fichier d'un Enregistrer arrive : à effacer (seul, `sortie::a_effacer` refait le tri)
/// si l'effacement de sa capture est déjà lancé, la carte encore là (`etat`) ou déjà
/// partie mais retenue (`retenue`). Sinon gardé : la carte l'ajoute à sa suppression.
pub fn enregistrement_tardif(
    etat: Option<EtatSuppression>,
    retenue: bool,
    fichier: Option<&Path>,
) -> Option<Suppression> {
    let lancee = match etat {
        Some(e) => e == EtatSuppression::Lancee,
        None => retenue,
    };
    let fichier = fichier.filter(|_| lancee)?;
    Some(Suppression {
        recent: None,
        enregistre: Some(fichier.to_path_buf()),
        liens: Vec::new(),
    })
}

/// Une suppression en attente au départ de la carte : la nouvelle capture ou l'arrêt la
/// font tout de suite ; à l'arrêt sur le fil même (la boucle d'événements s'arrête).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuppressionAuDepart {
    Aucune,
    DansUnFil,
    SurCeFil,
}

pub fn suppression_au_depart(etat: EtatSuppression, arret: bool) -> SuppressionAuDepart {
    match (etat, arret) {
        (EtatSuppression::EnAttente, true) => SuppressionAuDepart::SurCeFil,
        (EtatSuppression::EnAttente, false) => SuppressionAuDepart::DansUnFil,
        _ => SuppressionAuDepart::Aucune,
    }
}

/// Fin des secondes d'affichage (ou des 4 s d'Annuler) : la carte part, et sa suppression
/// est lancée si elle attendait.
pub fn au_delai(etat: EtatSuppression) -> (Declencheur, &'static str, bool) {
    if etat == EtatSuppression::EnAttente {
        (Declencheur::CaptureSupprimee, "supprimer", true)
    } else {
        (Declencheur::DelaiEcoule, "delai", false)
    }
}

/// Le glisser part vraiment : possible pour la carte, une fenêtre pour le lancer, et le
/// bouton principal encore enfoncé ; sinon la raison écrite au journal.
pub fn glisser_pret(
    possible: bool,
    fenetre: Option<isize>,
    bouton_enfonce: bool,
) -> Result<isize, &'static str> {
    match (possible, fenetre) {
        (false, _) => Err("indisponible"),
        (true, None) => Err("refuse (fenetre absente)"),
        (true, Some(_)) if !bouton_enfonce => Err("annule (bouton relache)"),
        (true, Some(f)) => Ok(f),
    }
}

/// Un glisser peut partir : un fichier « Récents », pas de suppression, pas de glisser en
/// cours. Rend le fichier de « Récents ».
pub fn glisser_possible(capture: &CaptureMontree) -> Option<&Path> {
    if capture.suppression != EtatSuppression::Aucune || capture.glisser {
        return None;
    }
    capture.recent.as_deref()
}

/// Fin d'un glisser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueGlisser {
    Depose,
    Annule,
    Refuse(String),
}

impl IssueGlisser {
    /// Le déclencheur de l'automate : seul un dépôt ferme la vignette.
    pub fn declencheur(&self) -> Option<Declencheur> {
        matches!(self, IssueGlisser::Depose).then_some(Declencheur::GlisserReussi)
    }
}

/// Fin d'un glisser : un dépôt sur une fenêtre de BreezyShot (`cible_a_nous`) compte comme
/// annulé.
pub fn depot(resultat: &Result<Depot, String>, cible_a_nous: bool) -> IssueGlisser {
    match resultat {
        Ok(Depot::Depose) if !cible_a_nous => IssueGlisser::Depose,
        Ok(_) => IssueGlisser::Annule,
        Err(e) => IssueGlisser::Refuse(e.clone()),
    }
}

/// Une demande de capture est acceptée, sauf pendant un glisser (la boucle du glisser
/// tient le fil principal).
pub fn capture_acceptee(glisser_en_cours: bool) -> bool {
    !glisser_en_cours
}

// Lignes de journal (jamais de chemin).

pub fn ligne_glisser(id: u64, issue: &IssueGlisser) -> String {
    match issue {
        IssueGlisser::Depose => format!("vignette glisser {id} depose"),
        IssueGlisser::Annule => format!("vignette glisser {id} annule"),
        IssueGlisser::Refuse(c) => format!("vignette glisser {id} refuse ({c})"),
    }
}

pub fn ligne_action_faite(id: u64, action: &str, reussi: bool) -> String {
    match (action, reussi) {
        (COPIER, true) => format!("vignette copie {id} faite"),
        (COPIER, false) => format!("vignette copie {id} echec"),
        (_, true) => format!("vignette enregistrement {id} fait"),
        (_, false) => format!("vignette enregistrement {id} echec"),
    }
}

/// `vignette suppression <id> <faite|tardive> recent <ok|echec|aucun> fichier
/// <ok|echec|aucun>` (`tardive` : le fichier d'un Enregistrer arrivé après l'effacement),
/// et le message d'erreur du bandeau si un fichier n'a pas pu être effacé.
pub fn bilan_suppression(
    id: u64,
    s: &Suppression,
    resultats: &[(PathBuf, Result<(), String>)],
    tardive: bool,
) -> (String, Option<Message>) {
    let etat = |chemin: &Option<PathBuf>| match chemin
        .as_ref()
        .and_then(|c| resultats.iter().find(|(p, _)| p == c))
    {
        Some((_, Ok(()))) => "ok",
        Some((_, Err(_))) => "echec",
        None => "aucun",
    };
    let ligne = format!(
        "vignette suppression {id} {} recent {} fichier {}",
        if tardive { "tardive" } else { "faite" },
        etat(&s.recent),
        etat(&s.enregistre)
    );
    let message = resultats
        .iter()
        .find_map(|(_, r)| r.as_ref().err())
        .map(|c| Message {
            genre: Genre::Erreur,
            titre: "Impossible de supprimer".into(),
            detail: c.clone(),
            actions: Vec::new(),
        });
    (ligne, message)
}

/// `chrono vignette relache-souffle <ms> relache-entree <ms>` (souffle absent : `-`).
pub fn ligne_chrono(souffle: Option<Duration>, entree: Duration) -> String {
    let souffle = souffle.map_or_else(|| "-".into(), |d| d.as_millis().to_string());
    format!(
        "chrono vignette relache-souffle {souffle} relache-entree {}",
        entree.as_millis()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capture() -> CaptureMontree {
        CaptureMontree {
            id: 7,
            pixels: Arc::new(Pixels {
                largeur: 1,
                hauteur: 1,
                rgba: vec![0, 0, 0, 255],
            }),
            transparence: false,
            maintenant: Local::now(),
            recent: Some(PathBuf::from("/l/Recents/20261111-194012-123.png")),
            enregistre: None,
            liens: Vec::new(),
            suppression: EtatSuppression::Aucune,
            copie: None,
            enregistrement: None,
            glisser: false,
            reduites: false,
            partie: false,
            annonce: Annonce::Reussite {
                titre: "Copié dans le presse-papiers".into(),
            },
            relache: Instant::now(),
            souffle_a: None,
        }
    }

    #[test]
    fn montrer_issue_duree_animations_coin() {
        assert!(doit_montrer(true, true));
        assert!(!doit_montrer(true, false) && !doit_montrer(false, true));
        assert_eq!(issue(true, true), Declencheur::ImagePrete);
        assert_eq!(issue(true, false), Declencheur::ImagePreteSansVignette);
        assert_eq!(issue(false, true), Declencheur::ImagePreteSansVignette);
        assert_eq!(duree_affichage(0), None);
        assert_eq!(duree_affichage(6), Some(Duration::from_secs(6)));
        assert!(animations_reduites(Animations::Systeme, true));
        assert!(!animations_reduites(Animations::Systeme, false));
        assert!(!animations_reduites(Animations::Normales, true));
        assert!(animations_reduites(Animations::Reduites, false));
        assert_eq!(coin(reglages::Coin::HautDroite), Coin::HautDroite);
        assert_eq!(coin(reglages::Coin::BasGauche), Coin::BasGauche);
    }

    #[test]
    fn actions_construites_seulement() {
        let a = actions();
        let ids: Vec<&str> = a.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, [COPIER, ENREGISTRER, SUPPRIMER, FERMER, ANNULER]);
        assert_eq!(a[0].libelle_fait.as_deref(), Some("Copié"));
        assert_eq!(a[1].libelle_fait.as_deref(), Some("Enregistré"));
        assert!(!ids.contains(&"annoter") && !ids.contains(&"epingler"));
    }

    #[test]
    fn bandeau_ne_dit_que_les_erreurs_avec_la_vignette() {
        let r = Annonce::Reussite {
            titre: "Copié dans le presse-papiers".into(),
        };
        let e = Annonce::Erreur {
            titre: "Impossible de copier".into(),
            cause: "Le presse-papiers est occupé.".into(),
        };
        assert_eq!(message_bandeau(&r, true), None);
        assert_eq!(
            message_bandeau(&r, false).map(|m| m.genre),
            Some(Genre::Reussite)
        );
        assert_eq!(
            message_bandeau(&e, true).map(|m| m.genre),
            Some(Genre::Erreur)
        );
    }

    #[test]
    fn chaque_bouton() {
        let t = Instant::now();
        let mut c = capture();
        assert_eq!(decider(&c, COPIER, t), Geste::Copier);
        assert_eq!(decider(&c, ENREGISTRER, t), Geste::Enregistrer);
        assert_eq!(decider(&c, SUPPRIMER, t), Geste::Supprimer);
        assert_eq!(decider(&c, FERMER, t), Geste::Fermer);
        assert_eq!(decider(&c, ANNULER, t), Geste::Ignorer);
        assert_eq!(decider(&c, "annoter", t), Geste::Ignorer);
        // En cours : ignoré, puis plus « en cours » après DELAI_SORTIE.
        c.copie = Some(t);
        assert_eq!(decider(&c, COPIER, t), Geste::Ignorer);
        assert_eq!(decider(&c, ENREGISTRER, t), Geste::Enregistrer);
        assert_eq!(decider(&c, COPIER, t + DELAI_SORTIE), Geste::Copier);
        // Déjà enregistrée : pas de doublon.
        c.enregistre = Some(PathBuf::from("/i/a.png"));
        assert_eq!(decider(&c, ENREGISTRER, t), Geste::DejaEnregistree);
        // Supprimée : seul Annuler.
        c.suppression = EtatSuppression::EnAttente;
        for a in [COPIER, ENREGISTRER, SUPPRIMER, FERMER] {
            assert_eq!(decider(&c, a, t), Geste::Ignorer, "{a}");
        }
        assert_eq!(decider(&c, ANNULER, t), Geste::Annuler);
        c.suppression = EtatSuppression::Lancee;
        assert_eq!(decider(&c, ANNULER, t), Geste::Ignorer);
    }

    #[test]
    fn demande_d_un_bouton() {
        let c = capture();
        let base = sortie::Demande {
            copier: false,
            enregistrer: true,
            dossier_enregistrement: PathBuf::from("/i"),
            dossier_affiche: "Images".into(),
            dossier_recents: PathBuf::from("/l/Recents"),
            ranger_recents: true,
            modele_nom: "BreezyShot {date} à {heure}".into(),
            maintenant: Local::now(),
            transparence: true,
        };
        let d = demande_action(&base, true, &c);
        assert!(d.copier && !d.enregistrer && !d.ranger_recents && !d.transparence);
        assert_eq!(d.maintenant, c.maintenant);
        let d = demande_action(&base, false, &c);
        assert!(!d.copier && d.enregistrer && !d.ranger_recents);
        assert_eq!(d.dossier_enregistrement, PathBuf::from("/i"));
    }

    #[test]
    fn resultat_d_un_bouton() {
        let bilan = |copie, enregistrement| Bilan {
            copie,
            recent: None,
            enregistrement,
        };
        let b = bilan(Etape::Faite, EtapeFichier::NonDemande);
        assert_eq!(apres_action(COPIER, &b), (true, None, None));
        let b = bilan(Etape::Echec("occupé".into()), EtapeFichier::NonDemande);
        let (ok, m, _) = apres_action(COPIER, &b);
        assert!(!ok);
        assert_eq!(m.map(|m| m.titre), Some("Impossible de copier".into()));
        let p = PathBuf::from("/i/a.png");
        let b = bilan(Etape::NonDemandee, EtapeFichier::Fait(p.clone()));
        assert_eq!(apres_action(ENREGISTRER, &b), (true, None, Some(p)));
        let b = bilan(
            Etape::NonDemandee,
            EtapeFichier::Echec {
                dossier: PathBuf::from("/x"),
                cause: "plein".into(),
            },
        );
        let (ok, m, f) = apres_action(ENREGISTRER, &b);
        assert!(!ok && f.is_none());
        assert_eq!(m.map(|m| m.detail), Some("plein".into()));
    }

    #[test]
    fn suppression_et_glisser() {
        let mut c = capture();
        c.enregistre = Some(PathBuf::from("/i/a.png"));
        c.liens = vec![PathBuf::from("/l/Glisser/a.png")];
        let s = a_supprimer(&c);
        assert_eq!(s.recent, c.recent);
        assert_eq!(s.enregistre, c.enregistre);
        assert_eq!(s.liens, c.liens);
        assert!(glisser_possible(&c).is_some());
        c.glisser = true;
        assert!(glisser_possible(&c).is_none());
        c.glisser = false;
        c.suppression = EtatSuppression::EnAttente;
        assert!(glisser_possible(&c).is_none());
        c.suppression = EtatSuppression::Aucune;
        c.recent = None;
        assert!(
            glisser_possible(&c).is_none(),
            "sans « Récents », rien à glisser"
        );

        let r = |p: &str, ok: bool| {
            let e = if ok {
                Ok(())
            } else {
                Err("Le fichier est protégé ou ouvert.".into())
            };
            (PathBuf::from(p), e)
        };
        let s = Suppression {
            recent: Some(PathBuf::from("/l/Recents/x.png")),
            enregistre: Some(PathBuf::from("/i/a.png")),
            liens: vec![],
        };
        let (l, m) = bilan_suppression(
            3,
            &s,
            &[r("/l/Recents/x.png", true), r("/i/a.png", true)],
            false,
        );
        assert_eq!(l, "vignette suppression 3 faite recent ok fichier ok");
        assert!(m.is_none());
        let (l, m) = bilan_suppression(
            3,
            &s,
            &[r("/l/Recents/x.png", true), r("/i/a.png", false)],
            false,
        );
        assert_eq!(l, "vignette suppression 3 faite recent ok fichier echec");
        assert_eq!(m.map(|m| m.titre), Some("Impossible de supprimer".into()));
        let (l, _) = bilan_suppression(3, &Suppression::default(), &[], false);
        assert_eq!(l, "vignette suppression 3 faite recent aucun fichier aucun");
        let tard = Suppression {
            enregistre: Some(PathBuf::from("/i/a.png")),
            ..Suppression::default()
        };
        let (l, _) = bilan_suppression(3, &tard, &[r("/i/a.png", true)], true);
        assert_eq!(l, "vignette suppression 3 tardive recent aucun fichier ok");
    }

    #[test]
    fn enregistrer_tardif_contre_supprimer() {
        use EtatSuppression::*;
        let f = Path::new("/i/a.png");
        // Retenue au départ seulement si l'effacement est lancé et Enregistrer tourne.
        let mut c = capture();
        c.suppression = Lancee;
        assert!(!a_retenir(&c), "pas d'Enregistrer en cours");
        c.enregistrement = Some(Instant::now());
        assert!(a_retenir(&c));
        c.suppression = EnAttente;
        assert!(!a_retenir(&c), "en attente : la suppression le prendra");
        // Le fichier qui arrive.
        let seul = enregistrement_tardif(Some(Lancee), false, Some(f)).expect("effacé");
        assert_eq!(seul.enregistre.as_deref(), Some(f));
        assert_eq!((seul.recent, seul.liens.len()), (None, 0));
        assert!(
            enregistrement_tardif(None, true, Some(f)).is_some(),
            "carte partie, retenue"
        );
        assert!(
            enregistrement_tardif(None, false, Some(f)).is_none(),
            "jamais supprimée"
        );
        assert!(enregistrement_tardif(Some(EnAttente), true, Some(f)).is_none());
        assert!(enregistrement_tardif(Some(Aucune), false, Some(f)).is_none());
        assert!(
            enregistrement_tardif(Some(Lancee), false, None).is_none(),
            "échec : rien"
        );
        // Mêmes gardes que Supprimer : un chemin relatif ou pas PNG n'est pas effacé.
        let relatif = enregistrement_tardif(None, true, Some(Path::new("a.png"))).unwrap();
        assert!(sortie::a_effacer(&relatif, Path::new("/l/R"), Path::new("/l/G")).is_empty());
        // Retenues bornées, sans doublon.
        let mut r = Vec::new();
        for i in 0..(RETENUES_MAX as u64 + 3) {
            retenir(&mut r, i);
        }
        retenir(&mut r, RETENUES_MAX as u64 + 2);
        assert_eq!(r.len(), RETENUES_MAX);
        assert_eq!(r[0], 3);
    }

    #[test]
    fn depart_delai_glisser_et_page_prete() {
        use EtatSuppression::*;
        assert_eq!(
            suppression_au_depart(EnAttente, true),
            SuppressionAuDepart::SurCeFil
        );
        assert_eq!(
            suppression_au_depart(EnAttente, false),
            SuppressionAuDepart::DansUnFil
        );
        assert_eq!(
            suppression_au_depart(Aucune, true),
            SuppressionAuDepart::Aucune
        );
        assert_eq!(
            suppression_au_depart(Lancee, false),
            SuppressionAuDepart::Aucune
        );
        assert_eq!(
            au_delai(EnAttente),
            (Declencheur::CaptureSupprimee, "supprimer", true)
        );
        assert_eq!(au_delai(Aucune), (Declencheur::DelaiEcoule, "delai", false));
        assert_eq!(au_delai(Lancee), (Declencheur::DelaiEcoule, "delai", false));
        assert_eq!(glisser_pret(true, Some(5), true), Ok(5));
        assert_eq!(glisser_pret(false, Some(5), true), Err("indisponible"));
        assert_eq!(
            glisser_pret(true, None, true),
            Err("refuse (fenetre absente)")
        );
        assert_eq!(
            glisser_pret(true, Some(5), false),
            Err("annule (bouton relache)")
        );
        assert!(prete_acceptee(Some(Etat::Souffle), false));
        assert!(!prete_acceptee(Some(Etat::Souffle), true), "carte partie");
        assert!(
            !prete_acceptee(Some(Etat::Vignette), false),
            "souffle déjà joué"
        );
        assert!(!prete_acceptee(Some(Etat::Repos), false), "après la croix");
        assert!(!prete_acceptee(None, false));
    }

    #[test]
    fn issue_d_un_glisser_et_capture_pendant() {
        assert_eq!(depot(&Ok(Depot::Depose), false), IssueGlisser::Depose);
        assert_eq!(depot(&Ok(Depot::Depose), true), IssueGlisser::Annule);
        assert_eq!(depot(&Ok(Depot::Annule), false), IssueGlisser::Annule);
        assert_eq!(
            depot(&Err("x".into()), false),
            IssueGlisser::Refuse("x".into())
        );
        assert_eq!(
            IssueGlisser::Depose.declencheur(),
            Some(Declencheur::GlisserReussi)
        );
        assert_eq!(IssueGlisser::Annule.declencheur(), None);
        assert!(capture_acceptee(false) && !capture_acceptee(true));
        assert_eq!(
            ligne_glisser(2, &IssueGlisser::Refuse("x".into())),
            "vignette glisser 2 refuse (x)"
        );
    }

    #[test]
    fn lignes() {
        assert_eq!(
            ligne_action_faite(1, COPIER, true),
            "vignette copie 1 faite"
        );
        assert_eq!(
            ligne_action_faite(1, ENREGISTRER, false),
            "vignette enregistrement 1 echec"
        );
        let ms = Duration::from_millis;
        assert_eq!(
            ligne_chrono(Some(ms(120)), ms(560)),
            "chrono vignette relache-souffle 120 relache-entree 560"
        );
        assert_eq!(
            ligne_chrono(None, ms(90)),
            "chrono vignette relache-souffle - relache-entree 90"
        );
        let r = rect_fenetres(systeme::Rect {
            x: -5,
            y: 2,
            largeur: 10,
            hauteur: -1,
        });
        assert_eq!((r.x, r.y, r.largeur, r.hauteur), (-5, 2, 10, 0));
    }
}
