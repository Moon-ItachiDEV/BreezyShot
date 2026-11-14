//! Vidéo et GIF, côté gestes (contrat 0.7, §4.2, §5, F1 à F37 côté cœur) : la visée en
//! mode film, la fenêtre de contrôle (par `plus_tard_nomme`, jamais dans un rappel), le
//! décompte du film, les fils `film`, `film-video`, `film-gif`, `film-son`, `film-sortie`
//! et `film-codeurs` (F37), la Finalisation, la sortie après la vignette (F36), l'arrêt
//! (F35). Les décisions sont dans `film.rs`.
//!
//! Points d'accroche de `mod.rs`, tous sur le fil principal :
//! - `capturer` : la séquence du mode une fois en Gel (`apres_gel`, modes `video`, `gif`) ;
//! - `selection_montree` : la Sélection est montrée en mode film (affichage `id`, Échap
//!   déjà réservé) ;
//! - `zone_choisie` : zone tracée en Sélection (F2), la capture encore rangée
//!   (`ranger_capture`) ; la Sélection reste montrée jusqu'à `Selection::cacher`
//!   (`garder_si_film`) ;
//! - `visee_abandonnee` : la visée d'un film a été abandonnée (Échap, annulée, écrans,
//!   muette…), écran figé et sélection déjà cachés ;
//! - `signal_selection`, `echap`, `bureau_change`, `action_bandeau` (croix du disque du
//!   décompte) : vrais s'ils ont été pris ici ; faux, le chemin commun suit ;
//! - `raccourci` : un raccourci de capture pressé ; vrai s'il a été pris ici (choix B1 :
//!   le raccourci du film en cours l'arrête, `film::arret_par_raccourci`) ;
//! - `fenetres_bloquees` : de la visée à la fin de la Finalisation (F30) ;
//! - `traiter` : ce que les fils postent (`Evenement::Film`) ;
//! - `lancer_recherche` : la recherche du codeur H.264, si `film::recherche_voulue` (F32) ;
//!   elle rend `Evenement::VideoDisponible` ; le nettoyage de `<local>/Films` au
//!   démarrage la précède, dans le même fil (F22) ;
//! - `lancer_action_vignette`, `lire` : Copier, Enregistrer et le rond de lecture (choix
//!   B2) de la carte d'un film (le glisser suit le chemin commun : `glisser_possible` y
//!   donne le fichier du film) ;
//! - `fermer_pour_arret` et `fin_de_session` : arrêt de l'app et fin de session Windows
//!   (F35), attente de la fermeture du fichier sur un canal, jamais par un `Evenement`.
//!
//! La Finalisation finie, la suite d'`ImagePrete` est `super::film_ferme` (vignette d'un
//! film, souffle depuis la zone), puis la sortie (`sortie::sortir_film`, fil
//! `film-sortie`).
//!
//! Les fils. `film` est la cadence (`Fil`, une machine écrite sur des traits pour être
//! essayée sous Linux avec des faux) : il prend l'image, lit le pointeur, pousse dans la
//! file bornée, surveille écrans, session, veille, disque et limites, tient le filet de la
//! page (F5) et fait l'aperçu ; il n'écrit jamais dans un fichier. `film-video` ou
//! `film-gif` (`ecrire`) crée, nourrit et ferme le fichier ; lui seul touche Media
//! Foundation. `film-son` est celui de `systeme::micro` ; le micro s'ouvre dans un petit
//! fil `film-micro`, l'ouverture pouvant attendre l'appareil. Un fil qui panique rend une
//! fin (`film: fil arrete (panique)`) : l'état Enregistrement n'est jamais laissé seul.

use std::fs::File;
use std::io::BufWriter;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime};

use chrono::{DateTime, Local};
use tauri::{AppHandle, Manager};

use super::etats::{Declencheur, Mode};
use super::film::{self, Annulation, Cadrage, Cadre, Cours, Fin, Geste, Mediane, Phase, Sorte, Vu};
use super::vignette::{self, Contenu, EtatSuppression};
use super::{
    defilante, zone, Apercu, Application, CaptureEnCours, Evenement, FinAction, FinFilm, GardeEcran,
};
use crate::capture;
use crate::fenetres::film::{
    self as controle, ChargeCacher, ChargeMontrer, Controle, PointPage, RectPage,
};
use crate::fenetres::selection::{self, Selection};
use crate::fenetres::{self, theme, Bandeau, Genre, Message};
use crate::film::{self as images, Calage, Gif, Horloge, Pas};
use crate::sortie::{self, BilanFilm, Etape, EtapeFichier};
use crate::systeme::gel::{self, Changement, EcranInfo, Pixels, Pointeur};
use crate::systeme::{self, micro, mp4, vivant, Rect};

/// Une source de film est-elle ouverte ? Une seule à la fois (`Jeton`).
static SOURCE: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

/// Le droit d'ouvrir la source d'un film : pris avant `vivant::ouvrir_source`, rendu avec
/// elle (dès la fin du film, avant la fermeture du fichier, ou à la fin du fil). Windows
/// refuse une seconde duplication du même écran (`0x80070057`) : sans ce jeton, le film
/// qui suit de près un film annulé partirait en BitBlt.
#[derive(Debug)]
struct Jeton(());

impl Jeton {
    /// Attend au plus `delai` que la source précédente soit rendue ; `None` si elle ne
    /// l'est pas (le film part quand même, sans jeton).
    fn prendre(delai: Duration) -> Option<Jeton> {
        let (tenue, rendue) = &SOURCE;
        let g = tenue.lock().unwrap_or_else(|e| e.into_inner());
        let (mut g, _) = rendue
            .wait_timeout_while(g, delai, |tenue| *tenue)
            .unwrap_or_else(|e| e.into_inner());
        if *g {
            return None;
        }
        *g = true;
        Some(Jeton(()))
    }
}

impl Drop for Jeton {
    fn drop(&mut self) {
        let (tenue, rendue) = &SOURCE;
        *tenue.lock().unwrap_or_else(|e| e.into_inner()) = false;
        rendue.notify_all();
    }
}

/// Dernier numéro de film donné.
static NUMERO: AtomicU64 = AtomicU64::new(0);

/// La sorte du film dont la visée commence (`capturer`, puis `selection_montree`).
static PROCHAINE: Mutex<Option<Sorte>> = Mutex::new(None);

/// La recherche du codeur est partie (une fois par lancement, F32).
static RECHERCHE_LANCEE: AtomicBool = AtomicBool::new(false);

/// Les fichiers de travail dont la sortie n'a pas rendu son bilan (F22 : jamais retirés par
/// le nettoyage de `<local>/Films`).
static SORTIES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Essais de création de la fenêtre de contrôle : juste après la destruction de celle du
/// film précédent, Tauri peut refuser l'étiquette encore prise.
const CREATIONS_MAX: u32 = 3;
const ENTRE_CREATIONS: Duration = Duration::from_millis(150);

/// Action de la croix du disque du décompte (`bandeau_action <id> annuler`, F8).
const ACTION_CROIX: &str = "annuler";

/// Ce que les fils du film postent au fil principal (`Evenement::Film`).
#[derive(Debug, Clone, PartialEq)]
pub enum Signal {
    /// La présence du micro (vidéo), lue par le fil `film` à son départ (F17).
    Micro { numero: u64, present: bool },
    /// Ce que le fil `film` a vu (préparation, décompte, secondes, fins sans geste).
    Vu { numero: u64, vu: Vu },
    /// Le filet de F5 : aucune pilule posée à temps (l'écran figé est déjà caché).
    PageEnRetard { numero: u64 },
    /// Le fichier fermé (F29), ou pourquoi il ne l'est pas.
    Ferme {
        numero: u64,
        issue: Result<Box<Fermeture>, &'static str>,
    },
    /// Le fil `film` s'est arrêté sur une panique.
    Panique { numero: u64 },
    /// Le bilan de la sortie (F36), hors de la table des états.
    Sortie(Box<SortieFaite>),
}

/// Un film fermé, rendu par le fil `film`.
#[derive(Debug, Clone, PartialEq)]
pub struct Fermeture {
    pub fichier: PathBuf,
    pub duree: Duration,
    /// La dernière image brute, sans le pointeur (F25).
    pub derniere: Arc<Pixels>,
    pub apercu: Option<Apercu>,
}

/// Le bilan de la sortie d'un film (fil `film-sortie`).
#[derive(Debug, Clone, PartialEq)]
pub struct SortieFaite {
    /// La carte du film (numéro de sortie).
    pub id: u64,
    pub fichier: PathBuf,
    pub sorte: Sorte,
    pub demande: sortie::Demande,
    pub deplacer: bool,
    /// La vignette a été montrée (sinon le bandeau annonce le bilan, F26).
    pub vignette: bool,
    pub bilan: BilanFilm,
    pub duree: Duration,
}

/// Ce que le fil principal ordonne au fil `film` (avec l'instant de l'ordre).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ordre {
    /// La fenêtre de contrôle est montrée : le filet de la page tombe.
    PageMontree,
    /// Le décompte finit à cet instant.
    Decompter(Instant),
    Demarrer(Instant),
    Suspendre(Instant),
    Reprendre(Instant),
    /// Ouvrir (vrai) ou couper le micro.
    Micro(bool),
    Finir(Fin, Instant),
    Annuler,
}

/// Le micro ouvert, vu du fil `film` (un faux dans les essais).
trait Ouvert: Send {
    fn perdu(&self) -> bool;
}

impl Ouvert for micro::Micro {
    fn perdu(&self) -> bool {
        micro::Micro::perdu(self)
    }
}

/// Ce que le fil `film` reçoit : les ordres du fil principal, les comptes du fil
/// d'écriture, le micro ouvert par `film-micro`.
enum Courrier {
    Ordre(Ordre),
    /// Le fichier de travail et le codeur sont prêts (F9).
    Pret {
        fichier: PathBuf,
        a: Instant,
    },
    /// La préparation a échoué, ou le fil d'écriture s'est arrêté (`fil`).
    Echec(&'static str),
    /// Une écriture refusée (F28, `disque`).
    Refus,
    /// Le fichier est fermé (F29).
    Ferme(Result<Ecrit, &'static str>),
    Micro(Result<Box<dyn Ouvert>, micro::Erreur>),
}

/// Ce que le fil d'écriture dit d'un fichier fermé.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Ecrit {
    octets: u64,
    /// Médiane du temps d'écriture d'une image, en millisecondes.
    ecriture: Option<f64>,
}

/// Ce que le fil `film` envoie au fil d'écriture.
#[derive(Debug)]
enum Envoi {
    Image {
        pixels: Pixels,
        instant: Duration,
    },
    /// Rien de neuf (F13) : la dernière image réécrite (vidéo).
    Meme {
        instant: Duration,
    },
    Finir {
        duree: Duration,
    },
    Abandonner,
}

/// Le film en cours (de la visée à la fin de la Finalisation), tenu par l'Application
/// (`super::film_en_cours`). Fil principal ; verrou jamais tenu pendant un appel à Tauri
/// ou à un module.
pub struct EnCours {
    numero: u64,
    sorte: Sorte,
    /// L'affichage de sélection.
    selection: u64,
    /// La fenêtre de contrôle, une fois créée (cachée).
    controle: Option<Arc<Controle>>,
    /// La demande de création, pour `film page prete en <ms> ms`.
    creation: Instant,
    /// La page a dit `film_pret`.
    page_prete: bool,
    /// Après le relâchement.
    lancee: Option<Lancee>,
}

impl std::fmt::Debug for EnCours {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnCours")
            .field("numero", &self.numero)
            .field("sorte", &self.sorte)
            .field("controle", &self.controle.is_some())
            .field("lancee", &self.lancee.as_ref().map(|l| l.cours.phase()))
            .finish()
    }
}

/// Un film lancé (zone choisie, état Enregistrement puis Finalisation).
struct Lancee {
    relache: Instant,
    cadre: Cadre,
    ecran: EcranInfo,
    cours: Cours,
    /// L'écran figé et la sélection, encore montrés jusqu'à la fenêtre de contrôle montrée.
    garde: Option<GardeEcran>,
    envoi: Sender<Courrier>,
    /// Les pas tenus par le fil `film` (images écrites).
    envoyees: Arc<AtomicU64>,
    /// Le fil d'écriture a fermé (ou abandonné) le fichier (F35).
    fermeture: Option<Receiver<()>>,
    /// L'`id` du disque du décompte dans le bandeau (0 : aucun).
    decompte: u64,
    /// `film:montrer` envoyé pour la page en cours.
    montrer_envoye: bool,
    /// La pilule est posée, la fenêtre de contrôle montrée.
    montree: bool,
    /// Le filet est tombé : une pilule tardive ne montre plus rien.
    en_retard: bool,
    /// `EnregistrementArrete` appliqué.
    finalisation: bool,
    /// `Finir` ou `Annuler` est parti au fil `film`.
    ordre_final: bool,
}

impl Drop for Lancee {
    /// Un film lâché sans ordre final est annulé : le fil `film` s'arrête, le fichier de
    /// travail est effacé.
    fn drop(&mut self) {
        if !self.ordre_final {
            let _ = self.envoi.send(Courrier::Ordre(Ordre::Annuler));
        }
    }
}

fn poster(app: &AppHandle, signal: Signal) {
    super::traiter(app, Evenement::Film(signal));
}

/// Le film en cours si c'est le `numero`, retiré (à détruire hors du verrou).
fn prendre(app: &AppHandle, numero: u64) -> Option<EnCours> {
    super::film_en_cours(app).take_if(|e| e.numero == numero)
}

/// Envoie `ordre` au fil du film `numero`.
fn ordonner(app: &AppHandle, numero: u64, ordre: Ordre) {
    let envoi = super::film_en_cours(app)
        .as_ref()
        .filter(|e| e.numero == numero)
        .and_then(|e| e.lancee.as_ref())
        .map(|l| l.envoi.clone());
    if let Some(envoi) = envoi {
        let _ = envoi.send(Courrier::Ordre(ordre));
    }
}

/// Un bandeau d'erreur au titre seul.
fn message(titre: &str) -> Message {
    Message {
        genre: Genre::Erreur,
        titre: titre.into(),
        detail: String::new(),
        actions: Vec::new(),
        duree_ms: None,
    }
}

/// Vrai de la visée à la fin de la Finalisation (F30) : le menu n'offre que « Quitter
/// BreezyShot », les raccourcis qui ouvrent une fenêtre et Annoter de l'Historique sont
/// ignorés.
pub fn fenetres_bloquees(app: &AppHandle) -> bool {
    super::film_en_cours(app).is_some()
}

/// La séquence du mode `mode` (`video` ou `gif`), en Gel : contrôles, photo sans pointeur,
/// écran figé, Sélection en mode film, Échap réservé (le chemin commun).
pub fn capturer(app: &AppHandle, mode: Mode, recu_a: Instant, bandeau_cache: bool) {
    *PROCHAINE.lock().unwrap_or_else(|e| e.into_inner()) = Sorte::depuis_mode(mode);
    super::capturer_avec_selection(app, recu_a, mode, bandeau_cache);
}

/// La Sélection est montrée en mode film (affichage `id`) : la fenêtre de contrôle est
/// créée cachée à un tour suivant de la boucle (F3).
pub fn selection_montree(app: &AppHandle, id: u64) {
    let Some(sorte) = PROCHAINE.lock().unwrap_or_else(|e| e.into_inner()).take() else {
        log::warn!("film: sorte inconnue a la visee");
        return;
    };
    let numero = NUMERO.fetch_add(1, Ordering::SeqCst) + 1;
    // Un menu de l'icône ouvert juste avant le départ montrerait encore ses actions.
    if crate::fenetres::menu::cacher(app) {
        log::info!("menu ferme: film");
    }
    let ancienne = super::film_en_cours(app).replace(EnCours {
        numero,
        sorte,
        selection: id,
        controle: None,
        creation: Instant::now(),
        page_prete: false,
        lancee: None,
    });
    if let Some(a) = ancienne {
        // Ne devrait pas arriver (une seule capture à la fois).
        log::warn!("film {}: ancien film retire", a.numero);
        fermer(app, a);
    }
    // Jamais ici (rappel d'un autre geste) : un tour suivant de la boucle.
    let postee = super::plus_tard_nomme(app, "film-poste", Duration::ZERO, move |app| {
        creer_controle(app, numero, 1)
    });
    if !postee {
        // Sans fenêtre de contrôle, le filet de la page finira le film au relâchement.
        log::warn!("film: fenetre non creee (poste)");
    }
}

/// Crée la fenêtre de contrôle (cachée) du film `numero`, s'il est encore en cours (essai
/// `essai`). Fil principal, tour de boucle à lui.
fn creer_controle(app: &AppHandle, numero: u64, essai: u32) {
    let voulue = super::film_en_cours(app)
        .as_ref()
        .is_some_and(|e| e.numero == numero && e.controle.is_none());
    if !voulue {
        return;
    }
    match controle::creer(app) {
        Ok(c) => {
            let c = Arc::new(c);
            let rangee = {
                let mut g = super::film_en_cours(app);
                match g
                    .as_mut()
                    .filter(|e| e.numero == numero && e.controle.is_none())
                {
                    Some(e) => {
                        e.controle = Some(Arc::clone(&c));
                        true
                    }
                    None => false,
                }
            };
            if rangee {
                envoyer_montrer(app, numero);
            } else {
                // Abandonné pendant la création : détruite tout de suite.
                detruire_controle(c);
            }
        }
        Err(e) => {
            log::warn!("film: fenetre non creee ({essai}): {e}");
            if essai >= CREATIONS_MAX {
                return;
            }
            let postee = super::plus_tard_nomme(app, "film-poste", ENTRE_CREATIONS, move |app| {
                creer_controle(app, numero, essai + 1)
            });
            if !postee {
                log::warn!("film: fenetre non creee (poste)");
            }
        }
    }
}

/// Cache puis détruit la fenêtre de contrôle (`Controle::detruire` écrit l'échec au
/// journal ; une copie encore tenue la détruit en tombant).
fn detruire_controle(c: Arc<Controle>) {
    match Arc::try_unwrap(c) {
        Ok(c) => {
            let _ = c.detruire();
        }
        Err(c) => c.cacher(),
    }
}

/// Abandon de la visée au relâchement : tout est caché, Repos par `declencheur`.
fn abandonner_visee(app: &AppHandle, mut garde: GardeEcran, declencheur: Declencheur) {
    garde.cacher_tout();
    drop(garde);
    super::appliquer(app, declencheur);
    visee_abandonnee(app);
}

/// Les noms des films à ne pas retirer de `<local>/Films` (F22) : celui de la vignette
/// montrée et ceux dont la sortie tourne encore.
fn films_proteges(app: &AppHandle) -> Vec<String> {
    let mut noms = SORTIES.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let montre = super::montree(app)
        .as_ref()
        .and_then(|c| c.contenu.film().map(|(f, _)| nom_de(f)));
    noms.extend(montre);
    noms
}

fn nom_de(fichier: &Path) -> String {
    fichier
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Zone tracée (en Sélection, capture encore rangée) : cadrée (F4), Enregistrement, l'écran
/// figé gardé jusqu'à la fenêtre de contrôle montrée, le fil `film` lancé (F5).
pub fn zone_choisie(app: &AppHandle, zone: Rect) {
    let relache = Instant::now();
    let Some(capture) = super::ranger_capture(app, None) else {
        if let Some(s) = app.try_state::<Selection>() {
            s.cacher();
        }
        return;
    };
    let CaptureEnCours { gel, garde, .. } = capture;
    let ecrans = gel.ecrans.clone();
    drop(gel);
    let visee = super::film_en_cours(app)
        .as_ref()
        .filter(|e| e.lancee.is_none())
        .map(|e| (e.numero, e.sorte));
    let (Some(ecran), Some((numero, sorte))) = (defilante::ecran_de(zone, &ecrans), visee) else {
        abandonner_visee(app, garde, Declencheur::SelectionAnnulee);
        return;
    };
    let cadre = match film::cadrer(zone, &ecran, sorte, mp4::taille_acceptee) {
        Cadrage::Bon(c) => c,
        Cadrage::Vide => {
            abandonner_visee(app, garde, Declencheur::SelectionAnnulee);
            return;
        }
        Cadrage::Refuse => {
            abandonner_visee(app, garde, Declencheur::SelectionImpossible);
            log::info!("{}", film::ligne_impossible("codeur"));
            super::afficher(app, zone::message_capture_impossible());
            return;
        }
    };
    if super::appliquer(app, Declencheur::EnregistrementChoisi).is_none() {
        abandonner_visee(app, garde, Declencheur::SelectionAnnulee);
        return;
    }
    log::info!("{}", film::ligne_zone(sorte, zone, &cadre));
    let secondes = film::decompte_s(super::reglages_lus(app).capture.retardateur_s);
    let (envoi, reception) = mpsc::channel();
    let (fini, fermeture) = mpsc::channel();
    let envoyees = Arc::new(AtomicU64::new(0));
    let lancee = Lancee {
        relache,
        cadre,
        ecran,
        cours: Cours::nouveau(sorte, secondes),
        garde: Some(garde),
        envoi: envoi.clone(),
        envoyees: Arc::clone(&envoyees),
        fermeture: Some(fermeture),
        decompte: 0,
        montrer_envoye: false,
        montree: false,
        en_retard: false,
        finalisation: false,
        ordre_final: false,
    };
    let rangee = {
        let mut g = super::film_en_cours(app);
        match g.as_mut().filter(|e| e.numero == numero) {
            Some(e) => {
                e.lancee = Some(lancee);
                None
            }
            None => Some(lancee),
        }
    };
    if let Some(lancee) = rangee {
        // Impossible sur ce fil (rien entre la lecture et ici) ; sûr quand même.
        drop(lancee);
        super::appliquer(app, Declencheur::EnregistrementImpossible);
        return;
    }
    let (_, glisser) = super::dossiers_vignette(app);
    let depart = Depart {
        numero,
        sorte,
        relache,
        cadre,
        ecran,
        ecrans,
        verrouillages: gel::verrouillages_de_session(),
        veilles: gel::mises_en_veille(),
        reception,
        envoi,
        envoyees,
        fini,
        dossier_films: super::dossier_films(app),
        dossier_glisser: glisser,
        proteges: films_proteges(app),
        maintenant: Local::now(),
    };
    if let Err(e) = lancer_fil(app, depart) {
        log::warn!("film: fil impossible ({e})");
        rendre_impossible(app, numero, "fil");
        return;
    }
    envoyer_montrer(app, numero);
}

/// Envoie `film:montrer` à la page quand tout est là : la fenêtre créée, la page prête, la
/// zone choisie et la présence du micro connue. Une fois par page.
fn envoyer_montrer(app: &AppHandle, numero: u64) {
    let reduites = super::reduites_editeur(app);
    let envoi = {
        let mut g = super::film_en_cours(app);
        g.as_mut()
            .filter(|e| e.numero == numero && e.page_prete)
            .and_then(|e| {
                let c = e.controle.clone()?;
                let l = e.lancee.as_mut()?;
                if l.montrer_envoye || !l.cours.micro_connu() || l.ordre_final {
                    return None;
                }
                l.montrer_envoye = true;
                Some((c, l.ecran, charge_montrer(numero, l, reduites)))
            })
    };
    let Some((c, ecran, charge)) = envoi else {
        return;
    };
    if let Err(e) = c.poser(ecran.rect) {
        log::warn!("film: fenetre non posee: {e}");
    }
    if let Err(e) = c.emettre(controle::EVENEMENT_MONTRER, charge) {
        // Sans réponse de la page, le filet finira le film.
        log::warn!("film: charge non envoyee: {e}");
    }
}

/// La charge de `film:montrer` : pixels réels relatifs au coin de la fenêtre (posée sur
/// tout l'écran de la zone).
fn charge_montrer(numero: u64, l: &Lancee, reduites: bool) -> ChargeMontrer {
    let (ox, oy) = (l.ecran.rect.x, l.ecran.rect.y);
    let z = l.cadre.zone;
    let taille = film::taille_pilule(
        controle::taille_pilule_logique(l.cours.micro().is_some()),
        &l.ecran,
    );
    let (px, py) = film::placer(z, &l.ecran, taille);
    ChargeMontrer {
        id: numero,
        echelle: l.ecran.echelle,
        zone: RectPage {
            x: z.x - ox,
            y: z.y - oy,
            largeur: z.largeur.max(0) as u32,
            hauteur: z.hauteur.max(0) as u32,
        },
        pilule: PointPage {
            x: px - ox,
            y: py - oy,
        },
        etat: film::charge_etat(numero, &l.cours),
        reduites,
    }
}

/// `film_pret` : la page écoute. Rend `reduites` ; `None` sans film en cours.
pub(super) fn page_prete(app: &AppHandle) -> Option<bool> {
    let reduites = super::reduites_editeur(app);
    let numero = {
        let mut g = super::film_en_cours(app);
        let e = g.as_mut()?;
        if !e.page_prete {
            log::info!("{}", film::ligne_page_prete(e.creation.elapsed()));
        }
        e.page_prete = true;
        // Une page rechargée reçoit de nouveau sa charge.
        if let Some(l) = e.lancee.as_mut().filter(|l| !l.montree) {
            l.montrer_envoye = false;
        }
        e.numero
    };
    theme::envoyer_a(app, controle::ETIQUETTE_FILM);
    envoyer_montrer(app, numero);
    Some(reduites)
}

/// `film_zone` : la pilule est posée (pixels CSS de la page), convertie avec l'origine et
/// l'échelle réelles de la fenêtre. La première fois, dans ce même tour : la fenêtre de
/// contrôle montrée, l'écran figé et la sélection cachés (F5), puis le décompte.
pub(super) fn pilule_posee(app: &AppHandle, id: u64, x: f64, y: f64, largeur: f64, hauteur: f64) {
    let pris = {
        let mut g = super::film_en_cours(app);
        let Some(e) = g.as_mut().filter(|e| e.numero == id) else {
            return;
        };
        let (Some(c), Some(l)) = (e.controle.clone(), e.lancee.as_mut()) else {
            return;
        };
        if l.en_retard || l.ordre_final {
            return;
        }
        let premiere = !l.montree;
        let garde = if premiere {
            l.montree = true;
            l.garde.take()
        } else {
            None
        };
        (c, l.ecran, premiere, garde, l.envoi.clone())
    };
    let (c, ecran, premiere, garde, envoi) = pris;
    let ecran = c.identifiant().and_then(systeme::rectangle_fenetre).map_or(
        ecran,
        |(x, y, _, _, echelle)| EcranInfo {
            rect: Rect { x, y, ..ecran.rect },
            echelle,
            ..ecran
        },
    );
    c.pilule(defilante::pilule_reelle(&ecran, x, y, largeur, hauteur));
    if !premiere {
        return;
    }
    // Le même tour : la fenêtre de contrôle paraît, l'écran figé et la sélection partent.
    let montree = c.montrer();
    gel::cacher_ecran_fige();
    if let Some(s) = app.try_state::<Selection>() {
        s.cacher();
    }
    if let Some(mut garde) = garde {
        // Déjà caché ici ; Échap reste réservé jusqu'à la fin du décompte.
        garde.fait = true;
    }
    if let Err(e) = montree {
        log::warn!("film: fenetre non montree: {e}");
        rendre_impossible(app, id, "page");
        return;
    }
    let _ = envoi.send(Courrier::Ordre(Ordre::PageMontree));
    vu(app, id, Vu::PageMontree);
}

/// `film_action` : un clic sur la pilule (`pause`, `arreter`, `micro`) ; un `id` périmé ou
/// une action inconnue sont ignorés.
pub(super) fn action(app: &AppHandle, id: u64, action: &str) {
    let vu_ = match action {
        "pause" => Vu::Pause,
        "arreter" => Vu::Arreter,
        "micro" => Vu::Micro,
        _ => return,
    };
    vu(app, id, vu_);
}

/// Ce que `vu` fait dans le cours du film `numero` (décision pure), puis les gestes.
fn vu(app: &AppHandle, numero: u64, vu: Vu) {
    let gestes = {
        let mut g = super::film_en_cours(app);
        match g
            .as_mut()
            .filter(|e| e.numero == numero)
            .and_then(|e| e.lancee.as_mut())
            .filter(|l| !l.ordre_final)
        {
            Some(l) => film::suite(&mut l.cours, vu),
            None => return,
        }
    };
    for geste in gestes {
        faire(app, numero, geste);
    }
}

/// Un geste du film `numero`.
fn faire(app: &AppHandle, numero: u64, geste: Geste) {
    match geste {
        Geste::Decompter(s) => decompter(app, numero, s),
        Geste::Demarrer => demarrer(app, numero),
        Geste::Suspendre => {
            log::info!("film pause");
            ordonner(app, numero, Ordre::Suspendre(Instant::now()));
        }
        Geste::Reprendre => {
            log::info!("film reprise");
            ordonner(app, numero, Ordre::Reprendre(Instant::now()));
        }
        Geste::OuvrirMicro => ordonner(app, numero, Ordre::Micro(true)),
        Geste::CouperMicro => {
            log::info!("{}", film::ligne_micro("coupe"));
            ordonner(app, numero, Ordre::Micro(false));
        }
        Geste::Etat => envoyer_etat(app, numero),
        Geste::Dire(titre) => {
            super::afficher(app, message(titre));
        }
        Geste::Finir(fin) => finir(app, numero, fin),
        Geste::Annuler(cause) => annuler(app, numero, cause),
        Geste::Impossible(cause) => rendre_impossible(app, numero, cause),
    }
}

/// Envoie `film:etat` à la page.
fn envoyer_etat(app: &AppHandle, numero: u64) {
    let envoi = super::film_en_cours(app)
        .as_ref()
        .filter(|e| e.numero == numero)
        .and_then(|e| {
            let c = e.controle.clone()?;
            let l = e.lancee.as_ref()?;
            Some((c, film::charge_etat(numero, &l.cours)))
        });
    if let Some((c, charge)) = envoi {
        if let Err(e) = c.emettre(controle::EVENEMENT_ETAT, charge) {
            log::warn!("film: charge non envoyee: {e}");
        }
    }
}

/// Le décompte (F8) : le disque du retardateur au centre de la zone, au-dessus de la
/// fenêtre de contrôle.
fn decompter(app: &AppHandle, numero: u64, secondes: u32) {
    let fin = Instant::now() + Duration::from_secs(u64::from(secondes));
    let reduites = super::reduites_editeur(app);
    let pris = super::film_en_cours(app)
        .as_ref()
        .filter(|e| e.numero == numero)
        .and_then(|e| {
            let l = e.lancee.as_ref()?;
            Some((
                l.cadre.zone,
                e.controle.as_ref().and_then(|c| c.identifiant()),
            ))
        });
    let Some((z, dessous)) = pris else {
        return;
    };
    log::info!("{}", film::ligne_decompte(secondes));
    ordonner(app, numero, Ordre::Decompter(fin));
    let Some(bandeau) = app.try_state::<Bandeau>() else {
        return;
    };
    let id = bandeau.montrer_decompte(fenetres::Decompte {
        numero,
        secondes,
        restant: Duration::from_secs(u64::from(secondes)),
        centre: (z.x + z.largeur / 2, z.y + z.hauteur / 2),
        cote: Some(z.largeur.min(z.hauteur)),
        reduites,
    });
    if id == 0 {
        return;
    }
    if let Some(l) = super::film_en_cours(app)
        .as_mut()
        .filter(|e| e.numero == numero)
        .and_then(|e| e.lancee.as_mut())
    {
        l.decompte = id;
    }
    if let (Some(dessous), Some(dessus)) = (dessous, bandeau.identifiant()) {
        if let Err(e) = systeme::placer_sous(dessous, dessus) {
            log::warn!("film: disque non place: {e}");
        }
    }
}

/// Retire l'`id` du disque du décompte du film `numero` (0 : aucun).
fn prendre_decompte(app: &AppHandle, numero: u64) -> u64 {
    super::film_en_cours(app)
        .as_mut()
        .filter(|e| e.numero == numero)
        .and_then(|e| e.lancee.as_mut())
        .map_or(0, |l| std::mem::take(&mut l.decompte))
}

fn cacher_decompte(app: &AppHandle, id: u64) {
    if id == 0 {
        return;
    }
    if let Some(b) = app.try_state::<Bandeau>() {
        b.cacher_decompte(id);
    }
}

/// Le départ (F11) : disque caché, Échap rendu (E4), l'horloge du film part.
fn demarrer(app: &AppHandle, numero: u64) {
    let id = prendre_decompte(app, numero);
    cacher_decompte(app, id);
    super::liberer_echap(app);
    ordonner(app, numero, Ordre::Demarrer(Instant::now()));
}

/// Une fin qui garde le film (F27, F28) : la fenêtre de contrôle part tout de suite, le
/// fil `film` ferme le fichier, Finalisation. Sans aucune image : impossible.
fn finir(app: &AppHandle, numero: u64, fin: Fin) {
    let a = Instant::now();
    let pris = {
        let mut g = super::film_en_cours(app);
        let Some(e) = g.as_mut().filter(|e| e.numero == numero) else {
            return;
        };
        let Some(l) = e.lancee.as_mut().filter(|l| !l.ordre_final) else {
            return;
        };
        if l.envoyees.load(Ordering::SeqCst) == 0 {
            None
        } else {
            l.ordre_final = true;
            l.finalisation = true;
            let _ = l.envoi.send(Courrier::Ordre(Ordre::Finir(fin, a)));
            Some((e.controle.take(), std::mem::take(&mut l.decompte)))
        }
    };
    let Some((c, decompte)) = pris else {
        rendre_impossible(app, numero, "source");
        return;
    };
    if let Some(c) = c {
        let _ = c.emettre(controle::EVENEMENT_CACHER, ChargeCacher { id: numero });
        c.pilule(None);
        detruire_controle(c);
    }
    cacher_decompte(app, decompte);
    super::liberer_echap(app);
    super::appliquer(app, Declencheur::EnregistrementArrete);
}

/// Tout ce que tient `e` est rendu : le disque caché, l'écran figé et la sélection cachés
/// s'ils l'étaient encore, le fil arrêté (`Lancee::drop`), la fenêtre de contrôle détruite,
/// Échap rendu. Hors du verrou.
fn fermer(app: &AppHandle, e: EnCours) {
    let EnCours {
        controle, lancee, ..
    } = e;
    if let Some(mut l) = lancee {
        cacher_decompte(app, std::mem::take(&mut l.decompte));
        // La garde (non faite) cache l'écran figé et la sélection en tombant.
        drop(l.garde.take());
        drop(l);
    }
    if let Some(c) = controle {
        detruire_controle(c);
    }
    super::liberer_echap(app);
}

/// L'attente ou le décompte annulés (F10) : rien n'est gardé, aucun message.
fn annuler(app: &AppHandle, numero: u64, cause: Annulation) {
    let Some(e) = prendre(app, numero) else {
        return;
    };
    let phase = e
        .lancee
        .as_ref()
        .map_or(Phase::Attente, |l| l.cours.phase());
    fermer(app, e);
    super::appliquer(app, Declencheur::EnregistrementAnnule);
    log::info!("{}", film::ligne_annule(phase, cause));
}

/// Le film finit sans rien garder (F9, F11, F28, F29) : Repos, bandeau d'erreur.
fn rendre_impossible(app: &AppHandle, numero: u64, cause: &'static str) {
    let Some(e) = prendre(app, numero) else {
        return;
    };
    let finalisation = e.lancee.as_ref().is_some_and(|l| l.finalisation);
    fermer(app, e);
    if finalisation {
        super::appliquer(app, Declencheur::FinalisationImpossible);
        log::info!("{}", film::ligne_impossible("fichier"));
        log::info!("film: cause {cause}");
        super::afficher(app, message(film::TITRE_FICHIER_IMPOSSIBLE));
    } else {
        super::appliquer(app, Declencheur::EnregistrementImpossible);
        log::info!("{}", film::ligne_impossible(cause));
        super::afficher(app, zone::message_capture_impossible());
    }
}

/// La visée d'un film est abandonnée (écran figé et sélection déjà cachés) : la fenêtre de
/// contrôle est détruite.
pub fn visee_abandonnee(app: &AppHandle) {
    // Pris sous le verrou, détruit hors du verrou.
    let ancien = super::film_en_cours(app).take();
    if let Some(e) = ancien {
        fermer(app, e);
    }
}

/// Le film lancé (pas encore en Finalisation) : son numéro et sa sorte.
fn lance(app: &AppHandle) -> Option<(u64, Sorte, u64, u64)> {
    super::film_en_cours(app).as_ref().and_then(|e| {
        let l = e.lancee.as_ref().filter(|l| !l.ordre_final)?;
        Some((e.numero, e.sorte, e.selection, l.decompte))
    })
}

/// Un signal de la Sélection : vrai s'il est pris ici. Une fois la zone choisie,
/// l'affichage gardé ne décide plus rien : ses signaux sont ignorés.
pub fn signal_selection(app: &AppHandle, signal: &selection::Signal) -> bool {
    super::film_en_cours(app)
        .as_ref()
        .is_some_and(|e| e.lancee.is_some() && e.selection == signal.id())
}

/// Échap global : vrai s'il est pris ici (film lancé : annulé pendant l'attente et le
/// décompte, ignoré ensuite).
pub fn echap(app: &AppHandle) -> bool {
    let Some((numero, ..)) = lance(app) else {
        return false;
    };
    vu(app, numero, Vu::Echap);
    true
}

/// Écrans changés ou session verrouillée pendant l'écran figé (phase `Attente`) : vrais
/// s'ils sont pris ici (F10).
pub fn bureau_change(app: &AppHandle, changement: Changement) -> bool {
    let Some((numero, ..)) = lance(app) else {
        return false;
    };
    vu(app, numero, Vu::Bureau(changement));
    true
}

/// Une action du bandeau (la croix du disque du décompte du film, F8) : vraie si elle est
/// prise ici.
pub fn action_bandeau(app: &AppHandle, id: u64, action: &str) -> bool {
    match lance(app) {
        Some((numero, _, _, decompte)) if decompte != 0 && decompte == id => {
            if action == ACTION_CROIX {
                vu(app, numero, Vu::Croix);
            }
            true
        }
        _ => false,
    }
}

/// Un raccourci de capture pressé (`mode`) : vrai s'il est pris ici. Choix B1 : pendant
/// un film, son propre raccourci l'arrête (`film::arret_par_raccourci`), les autres sont
/// laissés à la table, qui les ignore.
pub fn raccourci(app: &AppHandle, mode: Mode) -> bool {
    match lance(app) {
        Some((numero, sorte, ..)) if film::arret_par_raccourci(sorte, mode) => {
            vu(app, numero, Vu::Arreter);
            true
        }
        _ => false,
    }
}

/// Ce que les fils postent.
pub fn traiter(app: &AppHandle, signal: Signal) {
    match signal {
        Signal::Micro { numero, present } => {
            let connu = super::film_en_cours(app)
                .as_mut()
                .filter(|e| e.numero == numero)
                .and_then(|e| e.lancee.as_mut())
                .map(|l| l.cours.micro_present(present))
                .is_some();
            if connu {
                envoyer_montrer(app, numero);
            }
        }
        Signal::Vu { numero, vu: v } => vu(app, numero, v),
        Signal::PageEnRetard { numero } => rendre_impossible(app, numero, "page"),
        Signal::Ferme { numero, issue } => ferme(app, numero, issue),
        Signal::Panique { numero } => rendre_impossible(app, numero, "fil"),
        Signal::Sortie(s) => sortie_faite(app, *s),
    }
}

/// Le bilan de la Finalisation est-il attendu ? Seulement pour le film en Finalisation de
/// ce numéro (un bilan venu après l'échéance, ou après l'arrêt, est ignoré).
fn bilan_attendu(en_cours: Option<(u64, bool)>, numero: u64) -> bool {
    en_cours == Some((numero, true))
}

/// Le fichier fermé (F29) : `ImagePrete` (vignette) ou `ImagePreteSansVignette`, puis la
/// sortie (F36) ; ou `FinalisationImpossible`.
fn ferme(app: &AppHandle, numero: u64, issue: Result<Box<Fermeture>, &'static str>) {
    let attendu = {
        let g = super::film_en_cours(app);
        let en_cours = g
            .as_ref()
            .map(|e| (e.numero, e.lancee.as_ref().is_some_and(|l| l.finalisation)));
        bilan_attendu(en_cours, numero)
    };
    if !attendu {
        log::info!("film {numero}: bilan perime ignore");
        return;
    }
    let Some(e) = prendre(app, numero) else {
        return;
    };
    let f = match issue {
        Ok(f) => f,
        Err(cause) => {
            let _ = super::film_en_cours(app).replace(e);
            rendre_impossible(app, numero, cause);
            return;
        }
    };
    let (sorte, relache, zone) = match e.lancee.as_ref() {
        Some(l) => (l.cours.sorte(), l.relache, l.cadre.zone),
        None => return,
    };
    fermer(app, e);
    let (demande, deplacer) = film::demande_film(&super::demande_sortie(app, false));
    let id = app
        .state::<Application>()
        .derniere_sortie
        .fetch_add(1, Ordering::SeqCst)
        + 1;
    let Fermeture {
        fichier,
        duree,
        derniere,
        apercu,
    } = *f;
    let declencheur = super::film_ferme(
        app,
        FinFilm {
            numero: id,
            fichier: fichier.clone(),
            sorte,
            duree,
            zone,
            derniere,
            apercu,
            demande: demande.clone(),
            relache,
        },
    );
    lancer_sortie(
        app,
        SortieFaite {
            id,
            fichier,
            sorte,
            demande,
            deplacer,
            vignette: declencheur == Declencheur::ImagePrete,
            bilan: BilanFilm {
                copie: Etape::NonDemandee,
                enregistrement: EtapeFichier::NonDemande,
                lien: None,
            },
            duree: Duration::ZERO,
        },
    );
}

/// La sortie du film (F23, F36) dans le fil `film-sortie`, sans échéance ; le bilan revient
/// par `Signal::Sortie`. Sans rien à faire, l'annonce « Enregistrement terminé », tout de
/// suite (F26).
fn lancer_sortie(app: &AppHandle, mut s: SortieFaite) {
    if !s.demande.copier && !s.demande.enregistrer {
        if !s.vignette {
            super::afficher(
                app,
                zone::message_annonce(&sortie::Annonce::Reussite {
                    titre: film::TITRE_CARTE.into(),
                }),
            );
        }
        return;
    }
    let nom = nom_de(&s.fichier);
    SORTIES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(nom.clone());
    if s.vignette {
        let (copier, enregistrer) = (s.demande.copier, s.demande.enregistrer);
        super::avec_montree(app, s.id, |c| {
            let maintenant = Some(Instant::now());
            if copier {
                c.copie = maintenant;
            }
            if enregistrer {
                c.enregistrement = maintenant;
            }
        });
    }
    let pour = app.clone();
    let lance = std::thread::Builder::new()
        .name("film-sortie".into())
        .spawn(move || {
            let debut = Instant::now();
            s.bilan = sortir_ou_echec(&s.fichier, s.sorte, &s.demande, s.deplacer);
            s.duree = debut.elapsed();
            poster(&pour, Signal::Sortie(Box::new(s)));
        });
    if let Err(e) = lance {
        log::warn!("film: sortie impossible ({e})");
        SORTIES
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|n| *n != nom);
    }
}

/// `sortie::sortir_film`, et un bilan d'échec si elle panique.
fn sortir_ou_echec(
    fichier: &Path,
    sorte: Sorte,
    demande: &sortie::Demande,
    deplacer: bool,
) -> BilanFilm {
    panic::catch_unwind(AssertUnwindSafe(|| {
        sortie::sortir_film(fichier, sorte.sortie(), demande, deplacer)
    }))
    .unwrap_or_else(|_| {
        log::error!("film-sortie: fil interrompu (panique)");
        let cause = zone::CAUSE_SORTIE_INTERROMPUE.to_string();
        BilanFilm {
            copie: if demande.copier {
                Etape::Echec(cause.clone())
            } else {
                Etape::NonDemandee
            },
            enregistrement: if demande.enregistrer {
                EtapeFichier::Echec {
                    dossier: demande.dossier_enregistrement.clone(),
                    cause,
                }
            } else {
                EtapeFichier::NonDemande
            },
            lien: None,
        }
    })
}

/// Le bilan de la sortie (F36) : journal ; la carte apprend le fichier enregistré (déplacé,
/// choix U4 : Copier, Lire et le glisser passent ensuite le fichier enregistré) et le
/// lien ; « Enregistrer » cliqué pendant la sortie dit « Enregistrée » ; une carte supprimée
/// entre-temps efface aussi ce qui vient d'arriver ; puis l'annonce (F26) ou l'erreur.
fn sortie_faite(app: &AppHandle, s: SortieFaite) {
    let nom = nom_de(&s.fichier);
    SORTIES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|n| *n != nom);
    log::info!("{}", film::ligne_sortie(&s.bilan, s.duree));
    let annonce = sortie::annonce_film(&s.bilan, &s.demande, s.sorte.sortie());
    log::info!("{}", zone::ligne_annonce(&annonce));
    let enregistre = match &s.bilan.enregistrement {
        EtapeFichier::Fait(p) => Some(p.clone()),
        _ => None,
    };
    let etat = super::avec_montree(app, s.id, |c| {
        c.copie = None;
        c.enregistrement = None;
        if c.suppression != EtatSuppression::Lancee {
            c.liens.extend(s.bilan.lien.clone());
            if let Some(p) = &enregistre {
                c.enregistre = Some(p.clone());
                if s.deplacer {
                    if let Contenu::Film { fichier, .. } = &mut c.contenu {
                        fichier.clone_from(p);
                    }
                }
            }
        }
        (
            c.suppression,
            !c.partie,
            std::mem::take(&mut c.enregistrer_attendu),
        )
    });
    let retenue = etat.is_none() && {
        let etat_app = app.state::<Application>();
        let mut retenues = etat_app.retenues.lock().unwrap_or_else(|e| e.into_inner());
        let avant = retenues.len();
        retenues.retain(|i| *i != s.id);
        retenues.len() != avant
    };
    let supprimee = match etat {
        Some((suppression, ..)) => suppression == EtatSuppression::Lancee,
        None => retenue,
    };
    if supprimee {
        let tardive = sortie::Suppression {
            recent: None,
            enregistre: enregistre.clone(),
            liens: s.bilan.lien.iter().cloned().collect(),
            film: None,
        };
        if tardive.enregistre.is_some() || !tardive.liens.is_empty() {
            super::lancer_suppression(app, s.id, tardive, true);
        }
        return;
    }
    if let Some((_, true, true)) = etat {
        if let Some(v) = app.try_state::<super::Vignette>() {
            v.confirmer(s.id, vignette::ENREGISTRER, enregistre.is_some());
        }
    }
    let message = if s.vignette {
        vignette::message_bandeau(&annonce, true)
    } else {
        Some(zone::message_annonce(&annonce))
    };
    if let Some(m) = message {
        super::afficher(app, m);
    }
}

/// La recherche du codeur H.264 (F32, fil `film-codeurs`), une fois par lancement, et
/// seulement si `film::recherche_voulue` : elle rend `Evenement::VideoDisponible`. Avant,
/// aucune DLL de Media Foundation n'est chargée (F31). Le même fil nettoie d'abord
/// `<local>/Films` (F22, au démarrage) : sans licence, rien n'y est touché.
pub fn lancer_recherche(app: &AppHandle) {
    if !film::recherche_voulue(super::pro(app)) || RECHERCHE_LANCEE.swap(true, Ordering::SeqCst) {
        return;
    }
    let pour = app.clone();
    let films = super::dossier_films(app);
    let (_, glisser) = super::dossiers_vignette(app);
    let proteges = films_proteges(app);
    let lance = std::thread::Builder::new()
        .name("film-codeurs".into())
        .spawn(move || {
            nettoyer(&films, &glisser, &proteges);
            let present = mp4::codeur_h264_present();
            super::traiter(&pour, Evenement::VideoDisponible(present));
        });
    if let Err(e) = lance {
        log::warn!("codeur video non cherche: {e}");
    }
}

/// Nettoie `<local>/Films` (F22) ; journal s'il a retiré quelque chose.
fn nettoyer(films: &Path, glisser: &Path, proteges: &[String]) {
    if films.as_os_str().is_empty() {
        return;
    }
    let (n, octets) = sortie::nettoyer_films(films, glisser, SystemTime::now(), proteges);
    if n > 0 {
        log::info!("{}", film::ligne_nettoyes(n, octets));
    }
}

/// Copier (`copier`) ou Enregistrer depuis la carte d'un film (F25) : le fichier, dans le
/// fil `vignette-action`, puis le chemin commun d'`ActionFaite`.
pub fn lancer_action_vignette(app: &AppHandle, id: u64, copier: bool) {
    let base = super::demande_sortie(app, false);
    let Some((fichier, sorte, demande)) = super::avec_montree(app, id, |c| {
        let (f, sorte) = c.contenu.film()?;
        let (f, sorte) = (f.to_path_buf(), sorte);
        let maintenant = Some(Instant::now());
        if copier {
            c.copie = maintenant;
        } else {
            c.enregistrement = maintenant;
        }
        Some((f, sorte, vignette::demande_action(&base, copier, c)))
    })
    .flatten() else {
        return;
    };
    let action = if copier {
        vignette::COPIER
    } else {
        vignette::ENREGISTRER
    };
    let pour = app.clone();
    let lance = std::thread::Builder::new()
        .name("vignette-action".into())
        .spawn(move || {
            let (bilan, lien) = action_sur_fichier(&fichier, sorte, &demande, copier);
            let fin = Box::new(FinAction {
                id,
                action: action.into(),
                bilan,
            });
            let pour_traiter = pour.clone();
            let rendu = pour.run_on_main_thread(move || {
                if let Some(l) = lien {
                    super::avec_montree(&pour_traiter, id, |c| c.liens.push(l));
                }
                super::traiter(&pour_traiter, Evenement::ActionFaite(fin));
            });
            if let Err(e) = rendu {
                log::warn!("resultat de l'action non rendu: {e}");
            }
        });
    if let Err(e) = lance {
        log::warn!("fil de l'action impossible ({e})");
        super::avec_montree(app, id, |c| {
            c.copie = None;
            c.enregistrement = None;
        });
    }
}

/// Copier ou Enregistrer un film (une panique : un échec).
fn action_sur_fichier(
    fichier: &Path,
    sorte: Sorte,
    demande: &sortie::Demande,
    copier: bool,
) -> (sortie::Bilan, Option<PathBuf>) {
    let fait = panic::catch_unwind(AssertUnwindSafe(|| {
        if copier {
            let (copie, lien) = sortie::copier_film(fichier, sorte.sortie(), demande);
            (
                sortie::Bilan {
                    copie,
                    recent: None,
                    enregistrement: EtapeFichier::NonDemande,
                },
                lien,
            )
        } else {
            (
                sortie::Bilan {
                    copie: Etape::NonDemandee,
                    recent: None,
                    enregistrement: sortie::enregistrer_film(fichier, sorte.sortie(), demande),
                },
                None,
            )
        }
    }));
    fait.unwrap_or_else(|_| {
        log::error!("vignette-action: fil interrompu (panique)");
        (
            zone::bilan_d_echec(demande, zone::CAUSE_SORTIE_INTERROMPUE),
            None,
        )
    })
}

/// Le rond de lecture de la carte `id` (choix B2) : le film s'ouvre dans le lecteur de
/// Windows (`systeme::ouvrir_fichier`, jamais sur le fil principal).
pub fn lire(app: &AppHandle, id: u64) {
    let Some(fichier) =
        super::avec_montree(app, id, |c| c.contenu.film().map(|(f, _)| f.to_path_buf())).flatten()
    else {
        return;
    };
    let lance = std::thread::Builder::new()
        .name("film-lire".into())
        .spawn(move || {
            if let Err(e) = systeme::ouvrir_fichier(&fichier) {
                log::warn!("vignette lire {id} refusee ({e})");
            }
        });
    if let Err(e) = lance {
        log::warn!("vignette lire {id} refusee ({e})");
    }
}

/// Attend la fermeture du fichier sur `fermeture`, `delai` au plus : vrai si elle est
/// faite (ou si personne ne peut plus l'écrire : le fil d'écriture est parti).
fn attendre_fermeture(fermeture: &Receiver<()>, delai: Duration) -> bool {
    !matches!(
        fermeture.recv_timeout(delai),
        Err(RecvTimeoutError::Timeout)
    )
}

/// Arrêt de l'app (F35) : demande la fin au fil `film`, puis attend la fermeture du
/// fichier sur un canal, `delai` au plus. Vrai si rien n'était en cours ou si le fichier
/// est fermé ; faux sinon (`arreter` écrit alors `film: fichier non ferme a l'arret`).
pub fn fermer_pour_arret(app: &AppHandle, delai: Duration) -> bool {
    // Pris sous le verrou, détruit hors du verrou.
    let ancien = super::film_en_cours(app).take();
    match ancien {
        Some(e) => fermer_en_arret(app, e, delai),
        None => true,
    }
}

/// La fin de session Windows (F35, `gel::quand_session_finit`) : le même chemin que
/// l'arrêt. Appelé dans la procédure de la fenêtre de l'écran figé, peut-être deux fois
/// (`WM_QUERYENDSESSION` puis `WM_ENDSESSION`) : un verrou pris ailleurs n'est pas attendu.
pub fn fin_de_session(app: &AppHandle) {
    let etat = app.state::<Application>();
    let pris = match etat.film.try_lock() {
        Ok(mut g) => g.take(),
        Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner().take(),
        Err(std::sync::TryLockError::WouldBlock) => {
            log::warn!("film: fin de session pendant un geste");
            return;
        }
    };
    let Some(e) = pris else {
        return;
    };
    log::info!("film: fin de session");
    if !fermer_en_arret(app, e, film::DELAI_ARRET) {
        log::warn!("film: fichier non ferme a l'arret");
    }
}

/// Une seule chose est faite : fermer le fichier (F35). En marche, `Finir(arret)` (le film
/// reste dans `<local>/Films`, ni copie, ni vignette, ni annonce) ; avant, l'annulation.
/// Puis l'attente de la fermeture sur le canal. L'automate revient au Repos (une fin de
/// session refusée laisse l'app en marche).
fn fermer_en_arret(app: &AppHandle, mut e: EnCours, delai: Duration) -> bool {
    let Some(l) = e.lancee.as_mut() else {
        fermer(app, e);
        return true;
    };
    let a = Instant::now();
    let phase = l.cours.phase();
    let images = l.envoyees.load(Ordering::SeqCst);
    let gardee = if l.finalisation {
        true
    } else if !phase.avant() && images > 0 {
        l.ordre_final = true;
        l.finalisation = true;
        let _ = l.envoi.send(Courrier::Ordre(Ordre::Finir(Fin::Arret, a)));
        super::appliquer(app, Declencheur::EnregistrementArrete);
        true
    } else if !phase.avant() {
        super::appliquer(app, Declencheur::EnregistrementImpossible);
        log::info!("{}", film::ligne_impossible("source"));
        false
    } else {
        super::appliquer(app, Declencheur::EnregistrementAnnule);
        log::info!("{}", film::ligne_annule(phase, Annulation::Arret));
        false
    };
    let fermeture = l.fermeture.take();
    fermer(app, e);
    let ferme = fermeture.is_none_or(|f| attendre_fermeture(&f, delai));
    if gardee {
        if ferme {
            log::info!("{}", film::ligne_arret_ferme(a.elapsed()));
            super::appliquer(app, Declencheur::ImagePreteSansVignette);
        } else {
            super::appliquer(app, Declencheur::FinalisationImpossible);
        }
    }
    ferme
}

// ---------------------------------------------------------------------------------------
// Les fils du film.

/// Ce que le fil `film` reçoit au départ.
struct Depart {
    numero: u64,
    sorte: Sorte,
    relache: Instant,
    cadre: Cadre,
    ecran: EcranInfo,
    /// Les écrans au relâchement (F10).
    ecrans: Vec<EcranInfo>,
    verrouillages: u64,
    veilles: u64,
    reception: Receiver<Courrier>,
    envoi: Sender<Courrier>,
    envoyees: Arc<AtomicU64>,
    /// Prévient `fermer_pour_arret` que le fichier est fermé.
    fini: Sender<()>,
    dossier_films: PathBuf,
    dossier_glisser: PathBuf,
    proteges: Vec<String>,
    maintenant: DateTime<Local>,
}

/// Lance le fil `film`. Une panique dans le fil finit le film (`fil`).
fn lancer_fil(app: &AppHandle, depart: Depart) -> std::io::Result<()> {
    let numero = depart.numero;
    let pour = app.clone();
    std::thread::Builder::new()
        .name("film".into())
        .spawn(move || {
            let pour_fil = pour.clone();
            let sauf = sans_panique(move || filmer(depart, pour_fil));
            if !sauf {
                poster(&pour, Signal::Panique { numero });
            }
        })
        .map(|_| ())
}

/// `f`, une panique arrêtée et écrite (`film: fil arrete (panique)`). Vrai sans panique.
fn sans_panique(f: impl FnOnce()) -> bool {
    let sauf = panic::catch_unwind(AssertUnwindSafe(f)).is_ok();
    if !sauf {
        log::error!("film: fil arrete (panique)");
    }
    sauf
}

/// D'où viennent les images (`vivant::Source` ; un faux dans les essais).
trait Prise {
    fn prendre(&mut self, dest: &mut Pixels) -> Result<(vivant::Image, Duration), String>;
    fn replier(&mut self) -> Result<(), String>;
    /// Le moyen et la raison d'un changement de moyen depuis le dernier appel.
    fn repli(&mut self) -> Option<(&'static str, String)>;
    fn moyen(&self) -> &'static str;
}

impl Prise for vivant::Source {
    fn prendre(&mut self, dest: &mut Pixels) -> Result<(vivant::Image, Duration), String> {
        vivant::Source::prendre(self, dest)
    }

    fn replier(&mut self) -> Result<(), String> {
        vivant::Source::replier(self)
    }

    fn repli(&mut self) -> Option<(&'static str, String)> {
        vivant::Source::repli(self).map(|(m, r)| (m.nom(), r))
    }

    fn moyen(&self) -> &'static str {
        vivant::Source::moyen(self).nom()
    }
}

/// Le fil `film` : le micro (vidéo), le fil d'écriture, la source, puis la cadence.
fn filmer(d: Depart, app: AppHandle) {
    let numero = d.numero;
    let sortie_app = app.clone();
    let son = d.sorte == Sorte::Video && micro::present();
    if d.sorte == Sorte::Video {
        poster(
            &app,
            Signal::Micro {
                numero,
                present: son,
            },
        );
    }
    let calage = Arc::new(Mutex::new(Calage::default()));
    let ouvert = Arc::new(AtomicBool::new(false));
    let (envoi_ecriture, envois) = mpsc::channel();
    let (retour, retours) = mpsc::channel();
    let en_file = Arc::new(AtomicUsize::new(0));
    let ecriture = Ecriture {
        sorte: d.sorte,
        dossier_films: d.dossier_films.clone(),
        dossier_glisser: d.dossier_glisser,
        proteges: d.proteges,
        maintenant: d.maintenant,
        rapport: d.envoi.clone(),
        envois,
        retour,
        en_file: Arc::clone(&en_file),
        fini: d.fini,
    };
    let (l, h) = d.cadre.image;
    let fabrique: Fabrique = match d.sorte {
        Sorte::Video => {
            let (calage, ouvert) = (Arc::clone(&calage), Arc::clone(&ouvert));
            Box::new(move |chemin: &Path| creer_video(chemin, (l, h), son, calage, ouvert))
        }
        Sorte::Gif => Box::new(move |chemin: &Path| creer_gif(chemin, (l, h))),
    };
    let nom = match d.sorte {
        Sorte::Video => "film-video",
        Sorte::Gif => "film-gif",
    };
    let pour_ecriture = d.envoi.clone();
    let lance = std::thread::Builder::new()
        .name(nom.into())
        .spawn(move || ecrire_sans_panique(ecriture, fabrique));
    if let Err(e) = lance {
        log::warn!("film: fil d'ecriture impossible ({e})");
        let _ = pour_ecriture.send(Courrier::Echec("fil"));
    }
    let attendu = Instant::now();
    let jeton = Jeton::prendre(film::SOURCE_RENDUE_MAX);
    if jeton.is_none() {
        log::warn!(
            "film: source precedente non rendue apres {} ms",
            attendu.elapsed().as_millis()
        );
    }
    let source = vivant::ouvrir_source(d.cadre.zone, d.ecran.rect);
    let films = d.dossier_films;
    let bureau = Bureau {
        ecrans: Box::new(gel::ecrans),
        verrouillages: Box::new(gel::verrouillages_de_session),
        veilles: Box::new(gel::mises_en_veille),
        place: Box::new(move || systeme::installation::espace_libre(&films)),
        taille: Box::new(|f: &Path| std::fs::metadata(f).ok().map(|m| m.len())),
        pointeur: Box::new(|| gel::lire_pointeur().ok().flatten()),
        ouvrir_micro: Box::new(ouvrir_micro),
        apercu: |p| super::faire_apercu(p, false),
    };
    let pour_filet = app.clone();
    let filet: Box<dyn FnOnce() + Send> = Box::new(move || filet_de_la_page(&pour_filet, numero));
    let mut fil = Fil::nouveau(
        Lancement {
            numero,
            sorte: d.sorte,
            relache: d.relache,
            cadre: d.cadre,
            ecrans: d.ecrans,
            verrouillages: d.verrouillages,
            veilles: d.veilles,
            envoyees: d.envoyees,
            moi: d.envoi,
        },
        source,
        Liens {
            sortie: Box::new(move |s| poster(&sortie_app, s)),
            ecriture: envoi_ecriture,
            en_file,
            retours,
            calage,
            ouvert,
        },
        bureau,
        Delais::du_film(d.sorte),
        Some(filet),
    );
    fil.tenir(jeton);
    boucler(fil, d.reception);
}

/// Le filet de F5, depuis le fil `film` : sans pilule posée, l'écran figé et la sélection
/// sont cachés tout de suite (même si la boucle est prise), puis le film finit sur le fil
/// principal (`PageEnRetard`).
fn filet_de_la_page(app: &AppHandle, numero: u64) {
    let garde = {
        let mut g = super::film_en_cours(app);
        g.as_mut()
            .filter(|e| e.numero == numero)
            .and_then(|e| e.lancee.as_mut())
            .filter(|l| !l.montree && !l.ordre_final)
            .map(|l| {
                l.en_retard = true;
                l.garde.take()
            })
    };
    let Some(garde) = garde else {
        return;
    };
    // Hors du verrou : la garde cache l'écran figé et la sélection.
    drop(garde);
    poster(app, Signal::PageEnRetard { numero });
}

/// Ouvre le micro dans le fil `film-micro` (l'ouverture peut attendre l'appareil) ; le
/// résultat revient au fil `film`.
fn ouvrir_micro(calage: Arc<Mutex<Calage>>, rapport: Sender<Courrier>) {
    let pour = rapport.clone();
    let lance = std::thread::Builder::new()
        .name("film-micro".into())
        .spawn(move || {
            let ouvert = micro::ouvrir(move |pcm| {
                calage.lock().unwrap_or_else(|e| e.into_inner()).ranger(pcm)
            });
            let ouvert = ouvert.map(|m| Box::new(m) as Box<dyn Ouvert>);
            let _ = pour.send(Courrier::Micro(ouvert));
        });
    if lance.is_err() {
        let _ = rapport.send(Courrier::Micro(Err(micro::Erreur::Autre("fil".into()))));
    }
}

/// La boucle du fil `film` : un tour, puis le courrier jusqu'au tour suivant.
fn boucler<P: Prise>(mut fil: Fil<P>, reception: Receiver<Courrier>) {
    loop {
        let Some(attente) = fil.tour(Instant::now()) else {
            return;
        };
        match reception.recv_timeout(attente) {
            Ok(c) => {
                fil.recevoir(c, Instant::now());
                while let Ok(c) = reception.try_recv() {
                    fil.recevoir(c, Instant::now());
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                fil.recevoir(Courrier::Ordre(Ordre::Annuler), Instant::now());
                return;
            }
        }
    }
}

/// Les délais du fil `film` (ceux du contrat ; plus courts dans les essais).
#[derive(Debug, Clone, Copy)]
struct Delais {
    page: Duration,
    pret: Duration,
    fermeture: Duration,
    perdue_max: Duration,
    repli_max: Duration,
    saut_veille: Duration,
    tour: Duration,
    ecrans: Duration,
    disque: Duration,
    duree_max: Duration,
    pause_max: Duration,
}

impl Delais {
    fn du_film(sorte: Sorte) -> Delais {
        Delais {
            page: film::DELAI_PAGE,
            pret: film::DELAI_PRET,
            fermeture: film::DELAI_FERMETURE,
            perdue_max: film::SOURCE_PERDUE_MAX,
            repli_max: film::SOURCE_REPLI_MAX,
            saut_veille: film::SAUT_VEILLE,
            tour: film::TOUR,
            ecrans: film::SURVEILLANCE_ECRANS,
            disque: film::RELECTURE_DISQUE,
            duree_max: sorte.duree_max(),
            pause_max: film::PAUSE_MAX,
        }
    }
}

/// La taille d'un fichier (`None` s'il est illisible).
type Taille = Box<dyn FnMut(&Path) -> Option<u64> + Send>;
/// Ouvre le micro ; le résultat revient au fil `film` (`Courrier::Micro`).
type OuvrirMicro = Box<dyn FnMut(Arc<Mutex<Calage>>, Sender<Courrier>) + Send>;

/// Ce que le fil `film` lit du bureau (des faux dans les essais).
struct Bureau {
    ecrans: Box<dyn FnMut() -> Vec<EcranInfo> + Send>,
    verrouillages: Box<dyn FnMut() -> u64 + Send>,
    veilles: Box<dyn FnMut() -> u64 + Send>,
    /// La place libre du volume de `<local>/Films`.
    place: Box<dyn FnMut() -> Option<u64> + Send>,
    taille: Taille,
    pointeur: Box<dyn FnMut() -> Option<Pointeur> + Send>,
    ouvrir_micro: OuvrirMicro,
    apercu: fn(&Pixels) -> Option<Apercu>,
}

/// Ce qui décrit le film au fil `film`.
struct Lancement {
    numero: u64,
    sorte: Sorte,
    relache: Instant,
    cadre: Cadre,
    ecrans: Vec<EcranInfo>,
    verrouillages: u64,
    veilles: u64,
    envoyees: Arc<AtomicU64>,
    /// Sa propre boîte (le micro y rend son ouverture).
    moi: Sender<Courrier>,
}

/// Les liens du fil `film` avec le fil principal et le fil d'écriture.
struct Liens {
    sortie: Box<dyn FnMut(Signal) + Send>,
    ecriture: Sender<Envoi>,
    en_file: Arc<AtomicUsize>,
    /// Les tampons rendus par le fil d'écriture, réutilisés.
    retours: Receiver<Pixels>,
    calage: Arc<Mutex<Calage>>,
    /// Le micro est ouvert (sinon la piste reçoit du silence).
    ouvert: Arc<AtomicBool>,
}

/// Où en est le fil `film`.
enum Moment {
    Attente,
    Decompte(Instant),
    Marche,
    Pause(Instant),
    Fermeture {
        a: Instant,
        limite: Instant,
        duree: Duration,
        derniere: Arc<Pixels>,
        apercu: Option<Apercu>,
    },
    Finie,
}

/// Le fil `film` : une machine menée par `tour` (le temps) et `recevoir` (le courrier).
struct Fil<P: Prise> {
    l: Lancement,
    source: Option<P>,
    /// Rendu avec la source (`rendre_source`), jamais avant.
    jeton: Option<Jeton>,
    liens: Liens,
    bureau: Bureau,
    delais: Delais,
    filet: Option<Box<dyn FnOnce() + Send>>,
    file_max: usize,
    etape: Moment,
    /// Le fil d'écriture est prêt depuis cet instant (aucune image avant, F11).
    pret: Option<Instant>,
    fichier: Option<PathBuf>,
    pret_dit: bool,
    fin_decompte: Option<Instant>,
    decompte_dit: bool,
    echu_dit: bool,
    echec_dit: bool,
    horloge: Option<Horloge>,
    /// La dernière image brute de la source, sans pointeur (F14).
    brute: Pixels,
    a_image: bool,
    /// Pour poser le pointeur avant la réduction de moitié.
    tampon: Pixels,
    pointeur: Option<Pointeur>,
    sautees: u64,
    premiere_dite: bool,
    seconde: u64,
    perdue: Option<Instant>,
    repli: Option<Instant>,
    prochains_ecrans: Option<Instant>,
    prochain_disque: Option<Instant>,
    taille_avant: Option<(u64, Instant)>,
    dernier_tour: Option<Instant>,
    /// Une fin a été vue par ce fil : plus de surveillance ni d'image.
    vue: bool,
    /// Le temps du film s'arrête là (veille, limite, écrans…).
    gel_a: Option<Instant>,
    micro: Option<Box<dyn Ouvert>>,
    micro_voulu: bool,
    prises: Mediane,
    file_vue: usize,
}

impl<P: Prise> Fil<P> {
    fn nouveau(
        l: Lancement,
        source: Result<P, String>,
        liens: Liens,
        bureau: Bureau,
        delais: Delais,
        filet: Option<Box<dyn FnOnce() + Send>>,
    ) -> Fil<P> {
        let (iw, ih) = l.cadre.image;
        let octets = u64::from(iw) * u64::from(ih) * 4;
        let mut fil = Fil {
            source: None,
            jeton: None,
            liens,
            bureau,
            delais,
            filet,
            file_max: film::file_max(octets),
            etape: Moment::Attente,
            pret: None,
            fichier: None,
            pret_dit: false,
            fin_decompte: None,
            decompte_dit: false,
            echu_dit: false,
            echec_dit: false,
            horloge: None,
            brute: vide(),
            a_image: false,
            tampon: vide(),
            pointeur: None,
            sautees: 0,
            premiere_dite: false,
            seconde: 0,
            perdue: None,
            repli: None,
            prochains_ecrans: None,
            prochain_disque: None,
            taille_avant: None,
            dernier_tour: None,
            vue: false,
            gel_a: None,
            micro: None,
            micro_voulu: false,
            prises: Mediane::default(),
            file_vue: 0,
            l,
        };
        match source {
            Ok(mut s) => {
                let (moyen, raison) = match s.repli() {
                    Some((m, r)) => (m, Some(r)),
                    None => (s.moyen(), None),
                };
                log::info!("{}", film::ligne_source(moyen, raison.as_deref()));
                fil.source = Some(s);
            }
            Err(e) => {
                log::warn!("film: source refusee ({e})");
                fil.echouer("source");
            }
        }
        fil
    }

    /// Le jeton de la source (`Jeton::prendre`) ; rendu tout de suite s'il n'y a pas de
    /// source.
    fn tenir(&mut self, jeton: Option<Jeton>) {
        if self.source.is_some() {
            self.jeton = jeton;
        }
    }

    /// Plus d'image à prendre : la source est fermée, puis son jeton rendu (le film
    /// suivant peut ouvrir la sienne).
    fn rendre_source(&mut self) {
        self.source = None;
        self.jeton = None;
    }

    fn poster(&mut self, signal: Signal) {
        (self.liens.sortie)(signal);
    }

    fn dire(&mut self, vu: Vu) {
        let numero = self.l.numero;
        self.poster(Signal::Vu { numero, vu });
    }

    /// Une fin vue par ce fil (une seule) : le temps du film s'arrête à `a`.
    fn fin_vue(&mut self, vu: Vu, a: Instant) {
        if self.vue {
            return;
        }
        self.vue = true;
        self.gel_a.get_or_insert(a);
        self.dire(vu);
    }

    fn echouer(&mut self, cause: &'static str) {
        if !self.echec_dit {
            self.echec_dit = true;
            self.vue = true;
            self.dire(Vu::Echec(cause));
        }
    }

    fn en_marche(&self) -> bool {
        matches!(self.etape, Moment::Marche | Moment::Pause(_))
    }

    /// Un tour du fil à `a` : surveillance, décompte, images. Rend l'attente jusqu'au tour
    /// suivant ; `None` : le fil a fini.
    fn tour(&mut self, a: Instant) -> Option<Duration> {
        match &self.etape {
            Moment::Finie => return None,
            Moment::Fermeture { limite, .. } => {
                let limite = *limite;
                if a < limite {
                    return Some(limite.saturating_duration_since(a).min(self.delais.tour));
                }
                // Échéance de F29 : le fichier n'est pas fermé à temps.
                log::warn!(
                    "film: fichier non ferme en {} ms",
                    self.delais.fermeture.as_millis()
                );
                let numero = self.l.numero;
                self.poster(Signal::Ferme {
                    numero,
                    issue: Err("delai"),
                });
                self.etape = Moment::Finie;
                return None;
            }
            _ => {}
        }
        // Deux tours trop éloignés : une veille que le système n'a pas annoncée (F34).
        if let Some(avant) = self.dernier_tour {
            if a.saturating_duration_since(avant) > self.delais.saut_veille {
                self.fin_vue(Vu::Veille, avant);
            }
        }
        self.dernier_tour = Some(a);
        if a >= self.l.relache + self.delais.page {
            if let Some(filet) = self.filet.take() {
                filet();
            }
        }
        if self.micro.as_ref().is_some_and(|m| m.perdu()) {
            self.micro = None;
            self.micro_voulu = false;
            self.liens.ouvert.store(false, Ordering::SeqCst);
            log::info!("{}", film::ligne_micro("indisponible"));
            self.dire(Vu::MicroPerdu);
        }
        if !self.vue {
            self.surveiller(a);
        }
        let mut attente = self.delais.tour;
        match self.etape {
            Moment::Attente => self.amorcer(),
            Moment::Decompte(fin) => {
                if a >= fin && !self.decompte_dit {
                    self.decompte_dit = true;
                    self.dire(Vu::DecompteFini);
                }
                let echeance = fin + self.delais.pret;
                if a >= echeance && !self.pret_dit && !self.echu_dit {
                    self.echu_dit = true;
                    self.dire(Vu::PretEchu);
                }
                for borne in [fin, echeance] {
                    if borne > a {
                        attente = attente.min(borne - a);
                    }
                }
                self.amorcer();
            }
            Moment::Marche => attente = self.cadencer(a),
            Moment::Pause(depuis) => {
                if !self.vue && a.saturating_duration_since(depuis) >= self.delais.pause_max {
                    self.fin_vue(Vu::Limite, depuis);
                }
            }
            Moment::Fermeture { .. } | Moment::Finie => {}
        }
        if self.filet.is_some() {
            let borne = self.l.relache + self.delais.page;
            if borne > a {
                attente = attente.min(borne - a);
            }
        }
        Some(attente)
    }

    /// Écrans, session, veille et disque (F10, F28, F34).
    fn surveiller(&mut self, a: Instant) {
        if (self.bureau.verrouillages)() != self.l.verrouillages {
            self.fin_vue(Vu::Session, a);
            return;
        }
        if (self.bureau.veilles)() != self.l.veilles {
            self.fin_vue(Vu::Veille, a);
            return;
        }
        if self.prochains_ecrans.is_none_or(|p| a >= p) {
            self.prochains_ecrans = Some(a + self.delais.ecrans);
            let ecrans = (self.bureau.ecrans)();
            // Une liste vide est une lecture ratée, pas un changement.
            if !ecrans.is_empty() && ecrans != self.l.ecrans {
                self.fin_vue(Vu::Ecrans, a);
                return;
            }
        }
        if self.en_marche() && self.prochain_disque.is_none_or(|p| a >= p) {
            self.prochain_disque = Some(a + self.delais.disque);
            let taille = match &self.fichier {
                Some(f) => (self.bureau.taille)(f).unwrap_or(0),
                None => 0,
            };
            let debit = match self.taille_avant {
                Some((avant, depuis)) => {
                    let s = a.saturating_duration_since(depuis).as_secs_f64().max(1.0);
                    (taille.saturating_sub(avant) as f64 / s) as u64
                }
                None => 0,
            };
            self.taille_avant = Some((taille, a));
            if (self.bureau.place)().is_some_and(|p| p < film::marge_disque(debit)) {
                self.fin_vue(Vu::Disque, a);
            }
        }
    }

    /// Pendant l'attente et le décompte, la source est prise sans rien pousser : la
    /// première image est prête au départ (aucune image n'est jamais poussée avant).
    fn amorcer(&mut self) {
        let Some(s) = self.source.as_mut() else {
            return;
        };
        if let Ok((vivant::Image::Nouvelle, _)) = s.prendre(&mut self.brute) {
            self.a_image = self.taille_juste();
        }
    }

    /// L'image brute a la taille de la zone filmée.
    fn taille_juste(&self) -> bool {
        let z = self.l.cadre.zone;
        i64::from(self.brute.largeur) == i64::from(z.largeur)
            && i64::from(self.brute.hauteur) == i64::from(z.hauteur)
            && self.brute.rgba.len()
                == self.brute.largeur as usize * self.brute.hauteur as usize * 4
    }

    /// En marche : les pas échus de la grille (F13), la limite, les secondes.
    fn cadencer(&mut self, a: Instant) -> Duration {
        let Some(horloge) = self.horloge.as_mut() else {
            return self.delais.tour;
        };
        let t = horloge.temps(a);
        if !self.vue && t >= self.delais.duree_max {
            let depasse = t - self.delais.duree_max;
            self.fin_vue(Vu::Limite, a.checked_sub(depasse).unwrap_or(a));
        }
        let secondes = t.as_secs();
        if secondes > self.seconde && !self.vue {
            self.seconde = secondes;
            self.dire(Vu::Seconde(Duration::from_secs(secondes)));
        }
        if self.vue {
            return self.delais.tour;
        }
        let Some(horloge) = self.horloge.as_mut() else {
            return self.delais.tour;
        };
        match horloge.pas(a) {
            Pas::Prendre {
                instant, sautes, ..
            } => {
                self.sautees += u64::from(sautes);
                self.pousser(instant, a);
                // Le pas suivant est peut-être déjà proche : on repasse vite.
                match self.horloge.as_mut().map(|h| h.pas(a)) {
                    Some(Pas::Attendre(d)) => d.min(self.delais.tour),
                    _ => Duration::ZERO,
                }
            }
            Pas::Attendre(d) => d.min(self.delais.tour),
        }
    }

    /// Le pas du temps `instant` du film : prise, pointeur, et l'image (ou « la même »)
    /// poussée dans la file bornée (F13, F14, F20). Jamais avant `pret` (F11).
    fn pousser(&mut self, instant: Duration, a: Instant) {
        if self.pret.is_none() {
            self.sautees += 1;
            return;
        }
        if self.liens.en_file.load(Ordering::SeqCst) >= self.file_max {
            self.sautees += 1;
            return;
        }
        let mut nouvelle = false;
        if let Some(s) = self.source.as_mut() {
            match s.prendre(&mut self.brute) {
                Ok((vivant::Image::Nouvelle, d)) => {
                    self.prises.ajouter(d);
                    nouvelle = true;
                    self.a_image = self.taille_juste();
                    self.reprise(a);
                }
                Ok((vivant::Image::Inchangee, d)) => {
                    self.prises.ajouter(d);
                    self.reprise(a);
                }
                Ok((vivant::Image::Perdue, _)) => self.perdue(a),
                // L'écran figé encore montré : rien de neuf.
                Err(_) => {}
            }
        }
        if !self.a_image {
            return;
        }
        let zone = self.l.cadre.zone;
        let pointeur = (self.bureau.pointeur)().filter(|p| sur_la_zone(p, zone));
        let premiere = self.l.envoyees.load(Ordering::SeqCst) == 0;
        let change = nouvelle || premiere || pointeur != self.pointeur;
        let envoi = if change {
            let pixels = self.composer(pointeur.as_ref());
            self.pointeur = pointeur;
            Some(Envoi::Image { pixels, instant })
        } else if self.l.sorte == Sorte::Video {
            Some(Envoi::Meme { instant })
        } else {
            // GIF : rien d'écrit, l'image précédente s'allonge (F19).
            None
        };
        if let Some(envoi) = envoi {
            if self.liens.ecriture.send(envoi).is_err() {
                return;
            }
            let n = self.liens.en_file.fetch_add(1, Ordering::SeqCst) + 1;
            self.file_vue = self.file_vue.max(n);
        }
        self.l.envoyees.fetch_add(1, Ordering::SeqCst);
        if !self.premiere_dite {
            self.premiere_dite = true;
            let pret = self.pret.unwrap_or(a);
            let fin = self.fin_decompte.unwrap_or(a);
            log::info!("{}", film::ligne_debut(self.l.relache, pret, fin, a));
        }
    }

    /// L'image à écrire : une copie de la brute où le pointeur est posé, réduite de moitié
    /// si le cadre le dit (F4, F14). Le tampon vient du fil d'écriture quand il en rend un.
    fn composer(&mut self, pointeur: Option<&Pointeur>) -> Pixels {
        let mut image = self.liens.retours.try_recv().unwrap_or_else(|_| vide());
        let zone = self.l.cadre.zone;
        if self.l.cadre.reduite {
            copier_dans(&self.brute, &mut self.tampon);
            if let Some(p) = pointeur {
                capture::poser_pointeur(&mut self.tampon, zone, p);
            }
            images::moitie(&self.tampon, &mut image);
        } else {
            copier_dans(&self.brute, &mut image);
            if let Some(p) = pointeur {
                capture::poser_pointeur(&mut image, zone, p);
            }
        }
        image
    }

    /// La source ne rend rien (F33) : la dernière image est reprise ; après
    /// `SOURCE_PERDUE_MAX`, `BitBlt` est essayé ; s'il ne rend rien non plus pendant
    /// `SOURCE_REPLI_MAX`, fin `source`.
    fn perdue(&mut self, a: Instant) {
        if !self.a_image {
            return;
        }
        let depuis = *self.perdue.get_or_insert_with(|| {
            log::info!("film source perdue");
            a
        });
        match self.repli {
            None if a.saturating_duration_since(depuis) >= self.delais.perdue_max => {
                self.repli = Some(a);
                if let Some(s) = self.source.as_mut() {
                    if let Err(e) = s.replier() {
                        log::warn!("film: repli refuse ({e})");
                    }
                    if let Some((m, r)) = s.repli() {
                        log::info!("{}", film::ligne_source(m, Some(&r)));
                    }
                }
            }
            Some(r) if a.saturating_duration_since(r) >= self.delais.repli_max => {
                self.fin_vue(Vu::SourceFinie, a);
            }
            _ => {}
        }
    }

    /// Une image après une perte (F33).
    fn reprise(&mut self, a: Instant) {
        if let Some(depuis) = self.perdue.take() {
            self.repli = None;
            log::info!(
                "{}",
                film::ligne_reprise(a.saturating_duration_since(depuis))
            );
        }
    }

    fn recevoir(&mut self, c: Courrier, a: Instant) {
        match c {
            Courrier::Ordre(o) => self.ordre(o, a),
            Courrier::Pret { fichier, a: t } => {
                self.pret = Some(t);
                self.fichier = Some(fichier);
                if self.source.is_some() && !self.pret_dit && !self.echec_dit {
                    self.pret_dit = true;
                    self.dire(Vu::Pret);
                }
            }
            Courrier::Echec(cause) => match self.etape {
                Moment::Fermeture { .. } => self.fermer(Err(cause), a),
                Moment::Finie => {}
                _ => self.echouer(cause),
            },
            Courrier::Refus => {
                if self.en_marche() {
                    self.fin_vue(Vu::Disque, a);
                } else if !matches!(self.etape, Moment::Fermeture { .. } | Moment::Finie) {
                    self.echouer("disque");
                }
            }
            Courrier::Ferme(issue) => {
                if matches!(self.etape, Moment::Fermeture { .. }) {
                    self.fermer(issue, a);
                }
            }
            Courrier::Micro(Ok(m)) => {
                if self.micro_voulu
                    && !matches!(self.etape, Moment::Fermeture { .. } | Moment::Finie)
                {
                    self.micro = Some(m);
                    self.liens.ouvert.store(true, Ordering::SeqCst);
                    log::info!("{}", film::ligne_micro("ouvert"));
                }
            }
            Courrier::Micro(Err(_)) => {
                if std::mem::take(&mut self.micro_voulu) {
                    log::info!("{}", film::ligne_micro("indisponible"));
                    self.dire(Vu::MicroPerdu);
                }
            }
        }
    }

    fn ordre(&mut self, o: Ordre, a: Instant) {
        match o {
            Ordre::PageMontree => self.filet = None,
            Ordre::Decompter(fin) => {
                if matches!(self.etape, Moment::Attente) {
                    self.filet = None;
                    self.etape = Moment::Decompte(fin);
                    self.fin_decompte = Some(fin);
                }
            }
            Ordre::Demarrer(t) => {
                if matches!(self.etape, Moment::Attente | Moment::Decompte(_)) {
                    self.etape = Moment::Marche;
                    self.horloge = Some(Horloge::nouvelle(self.l.sorte.cadence(), t));
                    self.fin_decompte.get_or_insert(t);
                    self.prochain_disque = Some(t);
                }
            }
            Ordre::Suspendre(t) => {
                if let (Moment::Marche, Some(h)) = (&self.etape, self.horloge.as_mut()) {
                    h.pause(t);
                    self.etape = Moment::Pause(t);
                }
            }
            Ordre::Reprendre(t) => {
                if let (Moment::Pause(_), Some(h)) = (&self.etape, self.horloge.as_mut()) {
                    h.reprise(t);
                    self.etape = Moment::Marche;
                }
            }
            Ordre::Micro(true) => {
                if self.micro.is_none() && !self.micro_voulu {
                    self.micro_voulu = true;
                    let calage = Arc::clone(&self.liens.calage);
                    (self.bureau.ouvrir_micro)(calage, self.l.moi.clone());
                }
            }
            Ordre::Micro(false) => {
                self.micro_voulu = false;
                self.liens.ouvert.store(false, Ordering::SeqCst);
                self.micro = None;
            }
            Ordre::Finir(fin, t) => self.finir(fin, t),
            Ordre::Annuler => {
                if !matches!(self.etape, Moment::Finie) {
                    let _ = self.liens.ecriture.send(Envoi::Abandonner);
                }
                self.couper_micro();
                self.rendre_source();
                self.etape = Moment::Finie;
            }
        }
        let _ = a;
    }

    fn couper_micro(&mut self) {
        self.micro = None;
        self.micro_voulu = false;
        self.liens.ouvert.store(false, Ordering::SeqCst);
    }

    /// La fin gardée (F27 à F29) : la durée du film arrêtée là (ou au gel), le fichier
    /// fermé par le fil d'écriture, l'aperçu fait ici pendant ce temps.
    fn finir(&mut self, fin: Fin, t: Instant) {
        if !self.en_marche() {
            return;
        }
        let jusqu_a = self.gel_a.map_or(t, |g| g.min(t));
        let duree = self
            .horloge
            .as_ref()
            .map_or(Duration::ZERO, |h| h.temps(jusqu_a))
            .min(self.delais.duree_max);
        let images = self.l.envoyees.load(Ordering::SeqCst);
        log::info!("{}", film::ligne_fin(fin, duree, images, self.sautees));
        let _ = self.liens.ecriture.send(Envoi::Finir { duree });
        self.couper_micro();
        self.rendre_source();
        let derniere = Arc::new(std::mem::replace(&mut self.brute, vide()));
        let apercu = (self.bureau.apercu)(&derniere);
        self.etape = Moment::Fermeture {
            a: t,
            limite: t + self.delais.fermeture,
            duree,
            derniere,
            apercu,
        };
    }

    /// Le fil d'écriture a fermé le fichier (ou n'a pas pu).
    fn fermer(&mut self, issue: Result<Ecrit, &'static str>, recu: Instant) {
        let Moment::Fermeture {
            a,
            duree,
            derniere,
            apercu,
            ..
        } = std::mem::replace(&mut self.etape, Moment::Finie)
        else {
            return;
        };
        let numero = self.l.numero;
        let issue = match (issue, self.fichier.clone()) {
            (Ok(ecrit), Some(fichier)) => {
                let en = recu.saturating_duration_since(a);
                let images = self.l.envoyees.load(Ordering::SeqCst);
                log::info!("{}", film::ligne_fichier(self.l.sorte, ecrit.octets, en));
                log::info!(
                    "{}",
                    film::ligne_chrono(
                        images,
                        duree,
                        self.prises.valeur(),
                        ecrit.ecriture,
                        self.file_vue,
                        en
                    )
                );
                Ok(Box::new(Fermeture {
                    fichier,
                    duree,
                    derniere,
                    apercu,
                }))
            }
            (Ok(_), None) => Err("fichier"),
            (Err(cause), _) => {
                log::warn!("film: fichier non ferme ({cause})");
                Err("fichier")
            }
        };
        self.poster(Signal::Ferme { numero, issue });
    }
}

impl<P: Prise> Drop for Fil<P> {
    /// La fin du fil (annulation, échec, panique) : la source d'abord, puis le jeton.
    fn drop(&mut self) {
        self.rendre_source();
    }
}

/// Le pointeur touche-t-il la zone ?
fn sur_la_zone(p: &Pointeur, zone: Rect) -> bool {
    let r = Rect {
        x: p.origine.0,
        y: p.origine.1,
        largeur: p.largeur.min(i32::MAX as u32) as i32,
        hauteur: p.hauteur.min(i32::MAX as u32) as i32,
    };
    r.intersection(zone).is_some()
}

fn vide() -> Pixels {
    Pixels {
        largeur: 0,
        hauteur: 0,
        rgba: Vec::new(),
    }
}

/// `source` copié dans `dest`, dont la mémoire est réutilisée.
fn copier_dans(source: &Pixels, dest: &mut Pixels) {
    dest.largeur = source.largeur;
    dest.hauteur = source.hauteur;
    dest.rgba.clear();
    dest.rgba.extend_from_slice(&source.rgba);
}

// ---------------------------------------------------------------------------------------
// Le fil d'écriture (`film-video`, `film-gif`).

/// Ce qui écrit le fichier (le MP4 de `systeme::mp4`, le GIF de `film::gif`, ou un faux).
trait Ecrivain {
    /// Rend le temps passé à écrire cette image.
    fn image(&mut self, pixels: &Pixels, instant: Duration) -> Result<Duration, String>;
    fn meme(&mut self, instant: Duration) -> Result<Duration, String>;
    fn finir(self: Box<Self>, duree: Duration) -> Result<(), String>;
    fn abandonner(self: Box<Self>);
}

/// Crée l'écrivain du fichier donné (dans le fil d'écriture : l'écrivain MP4 n'est pas
/// `Send`). Erreur : la cause de `film impossible` (`codeur`, `disque`).
type Fabrique = Box<dyn FnOnce(&Path) -> Result<Box<dyn Ecrivain>, &'static str> + Send>;

/// Ce que le fil d'écriture reçoit au départ.
struct Ecriture {
    sorte: Sorte,
    dossier_films: PathBuf,
    dossier_glisser: PathBuf,
    proteges: Vec<String>,
    maintenant: DateTime<Local>,
    rapport: Sender<Courrier>,
    envois: Receiver<Envoi>,
    retour: Sender<Pixels>,
    en_file: Arc<AtomicUsize>,
    fini: Sender<()>,
}

/// `ecrire` ; une panique y devient `Echec("fil")` (F37), et la fermeture est dite (F35 :
/// l'arrêt n'attend pas un fil parti).
fn ecrire_sans_panique(ecriture: Ecriture, fabrique: Fabrique) {
    let rapport = ecriture.rapport.clone();
    let fini = ecriture.fini.clone();
    if !sans_panique(move || ecrire(ecriture, fabrique)) {
        let _ = rapport.send(Courrier::Echec("fil"));
        let _ = fini.send(());
    }
}

/// Le fil d'écriture : nettoyage de `<local>/Films` (F22), place, fichier de travail,
/// écrivain, `pret` ; puis les images de la file jusqu'à la fin ou l'abandon. Prévient
/// `fini` dès que le fichier est fermé (F35).
fn ecrire(e: Ecriture, fabrique: Fabrique) {
    let Ecriture {
        sorte,
        dossier_films,
        dossier_glisser,
        proteges,
        maintenant,
        rapport,
        envois,
        retour,
        en_file,
        fini,
    } = e;
    nettoyer(&dossier_films, &dossier_glisser, &proteges);
    let chemin = match sortie::chemin_film(&dossier_films, maintenant, sorte.sortie()) {
        Ok(c) => c,
        Err(_) => {
            let _ = rapport.send(Courrier::Echec("disque"));
            let _ = fini.send(());
            return;
        }
    };
    if systeme::installation::espace_libre(&dossier_films).is_some_and(|p| p < film::PLACE_MIN) {
        let _ = rapport.send(Courrier::Echec("disque"));
        let _ = fini.send(());
        return;
    }
    let mut ecrivain = match fabrique(&chemin) {
        Ok(e) => e,
        Err(cause) => {
            sortie::effacer_film(&chemin);
            let _ = rapport.send(Courrier::Echec(cause));
            let _ = fini.send(());
            return;
        }
    };
    let _ = rapport.send(Courrier::Pret {
        fichier: chemin.clone(),
        a: Instant::now(),
    });
    let mut ecriture = Mediane::default();
    let mut refuse = false;
    let noter = |r: Result<Duration, String>, ecriture: &mut Mediane, refuse: &mut bool| match r {
        Ok(d) => ecriture.ajouter(d),
        Err(e) => {
            *refuse = true;
            log::warn!("film: ecriture refusee ({e})");
            let _ = rapport.send(Courrier::Refus);
        }
    };
    loop {
        match envois.recv() {
            Ok(Envoi::Image { pixels, instant }) => {
                if !refuse {
                    let r = ecrivain.image(&pixels, instant);
                    noter(r, &mut ecriture, &mut refuse);
                }
                en_file.fetch_sub(1, Ordering::SeqCst);
                let _ = retour.send(pixels);
            }
            Ok(Envoi::Meme { instant }) => {
                if !refuse {
                    let r = ecrivain.meme(instant);
                    noter(r, &mut ecriture, &mut refuse);
                }
                en_file.fetch_sub(1, Ordering::SeqCst);
            }
            Ok(Envoi::Finir { duree }) => {
                let issue = match ecrivain.finir(duree) {
                    Ok(()) => Ok(Ecrit {
                        octets: std::fs::metadata(&chemin).map_or(0, |m| m.len()),
                        ecriture: ecriture.valeur(),
                    }),
                    Err(e) => {
                        log::warn!("film: fermeture refusee ({e})");
                        Err("fichier")
                    }
                };
                let _ = fini.send(());
                let _ = rapport.send(Courrier::Ferme(issue));
                return;
            }
            Ok(Envoi::Abandonner) | Err(_) => {
                ecrivain.abandonner();
                sortie::effacer_film(&chemin);
                let _ = fini.send(());
                return;
            }
        }
    }
}

/// L'écrivain MP4 (F15 à F18) : images et son, tirés du calage à chaque image (silence
/// tant que le micro est coupé).
struct EcrivainVideo {
    ecrivain: mp4::Ecrivain,
    son: Option<(Arc<Mutex<Calage>>, Arc<AtomicBool>)>,
    pcm: Vec<i16>,
}

impl EcrivainVideo {
    fn nourrir_son(&mut self, jusqu_a: Duration) -> Result<(), String> {
        let Some((calage, ouvert)) = &self.son else {
            return Ok(());
        };
        self.pcm.clear();
        let debut = calage.lock().unwrap_or_else(|e| e.into_inner()).tirer(
            jusqu_a,
            ouvert.load(Ordering::SeqCst),
            &mut self.pcm,
        );
        if self.pcm.is_empty() {
            return Ok(());
        }
        self.ecrivain.son(&self.pcm, debut)
    }
}

impl Ecrivain for EcrivainVideo {
    fn image(&mut self, pixels: &Pixels, instant: Duration) -> Result<Duration, String> {
        let d = self.ecrivain.image(pixels, instant)?;
        self.nourrir_son(instant)?;
        Ok(d)
    }

    fn meme(&mut self, instant: Duration) -> Result<Duration, String> {
        let d = self.ecrivain.meme_image(instant)?;
        self.nourrir_son(instant)?;
        Ok(d)
    }

    fn finir(mut self: Box<Self>, duree: Duration) -> Result<(), String> {
        self.nourrir_son(duree)?;
        self.ecrivain.finir().map(|_| ())
    }

    fn abandonner(self: Box<Self>) {
        self.ecrivain.abandonner();
    }
}

/// Crée l'écrivain MP4 : `film codeur …`, `film son …`.
fn creer_video(
    chemin: &Path,
    (largeur, hauteur): (u32, u32),
    son: bool,
    calage: Arc<Mutex<Calage>>,
    ouvert: Arc<AtomicBool>,
) -> Result<Box<dyn Ecrivain>, &'static str> {
    let p = mp4::Parametres {
        largeur,
        hauteur,
        cadence: film::CADENCE_VIDEO,
        debit_bit_s: film::debit(largeur, hauteur),
        son,
        materiel: true,
    };
    match mp4::creer(chemin, &p) {
        Ok(ecrivain) => {
            log::info!(
                "{}",
                film::ligne_codeur(ecrivain.codeur().nom(), (largeur, hauteur), p.debit_bit_s)
            );
            log::info!("{}", film::ligne_son(son));
            Ok(Box::new(EcrivainVideo {
                ecrivain,
                son: son.then_some((calage, ouvert)),
                pcm: Vec::new(),
            }))
        }
        Err(e) => {
            log::warn!("film: ecrivain refuse ({e})");
            Err("codeur")
        }
    }
}

/// L'écrivain du GIF (F19), au fil de l'eau.
struct EcrivainGif {
    gif: Gif<BufWriter<File>>,
}

impl Ecrivain for EcrivainGif {
    fn image(&mut self, pixels: &Pixels, instant: Duration) -> Result<Duration, String> {
        let debut = Instant::now();
        self.gif.image(pixels, instant)?;
        Ok(debut.elapsed())
    }

    fn meme(&mut self, _instant: Duration) -> Result<Duration, String> {
        Ok(Duration::ZERO)
    }

    fn finir(self: Box<Self>, duree: Duration) -> Result<(), String> {
        let tampon = self.gif.finir(duree)?;
        let fichier = tampon.into_inner().map_err(|e| e.error().to_string())?;
        // Un disque qui ne sait pas vider (réseau) n'est pas une faute.
        let _ = fichier.sync_all();
        Ok(())
    }

    fn abandonner(self: Box<Self>) {}
}

fn creer_gif(
    chemin: &Path,
    (largeur, hauteur): (u32, u32),
) -> Result<Box<dyn Ecrivain>, &'static str> {
    let fichier = File::create(chemin).map_err(|_| "disque")?;
    let (l, h) = (
        u16::try_from(largeur).map_err(|_| "codeur")?,
        u16::try_from(hauteur).map_err(|_| "codeur")?,
    );
    let gif =
        Gif::nouveau(BufWriter::new(fichier), l, h, film::VITESSE_GIF).map_err(|_| "codeur")?;
    Ok(Box::new(EcrivainGif { gif }))
}

#[cfg(test)]
mod tests;
