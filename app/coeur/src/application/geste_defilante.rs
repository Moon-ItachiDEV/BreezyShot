//! Capture défilante, côté gestes (contrat 0.6, §5, §18) : la visée en mode défilante, la
//! fenêtre de contrôle créée au début de la visée (R1, par `plus_tard_nomme`, jamais dans
//! un rappel), les pages proposées (fil `defilante-pages`), le défilement et l'assemblage
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
//! Déroulé (R1) : à la Sélection montrée, la fenêtre de contrôle est créée cachée (un tour
//! suivant de la boucle) et les pages cherchées dans un fil. Au relâchement, Entrée est
//! réservée et l'écran figé reste montré (sa garde passe dans `Lancee`) ; dès la page prête
//! (`defilante_pret`), `defilante:montrer` part ; quand la page a posé la pilule
//! (`defilante_zone`), la fenêtre est montrée et l'écran figé et la sélection cachés dans
//! le même tour, puis le fil `defilante` part. Filet (fil `defilante-filet`) : sans fenêtre
//! montrée `DELAI_PAGE` après le relâchement, l'écran figé est caché d'ici, puis la capture
//! abandonnée (« Impossible de lancer la capture défilante »).
//!
//! Le fil `defilante` tient tout ce qui est lent (prise, mesure, défilement, assemblage) ;
//! il ne parle au fil principal que par `Evenement::Defilante`. Le verrou de `EnCours`
//! n'est jamais tenu pendant un appel à Tauri ou à un module.

use std::collections::VecDeque;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use super::defilante::{
    self as regles, Annulation, Conduite, Course, Fin, Geste, Impossible, Placement, Vu,
};
use super::etats::{Declencheur, Mode};
use super::{zone, CaptureEnCours, Evenement, GardeEcran};
use crate::capture;
use crate::defilement::{self, Ajout, Assemblage, Fixes, Mesure, TOLERANCE};
use crate::fenetres::defilante::{
    self as controle, ApercuPage, ChargeApercu, ChargeCacher, ChargeMessage, ChargeMontrer,
    Controle, PointPage, RectPage,
};
use crate::fenetres::selection::{self, FenetreProposee, Selection};
use crate::fenetres::theme;
use crate::raccourcis;
use crate::sortie;
use crate::systeme::defiler::{self, Envoi};
use crate::systeme::gel::{self, Changement, EcranInfo, Pixels};
use crate::systeme::{self, vivant, Rect};

/// Dernier numéro de capture défilante donné.
static NUMERO: AtomicU64 = AtomicU64::new(0);

/// Sans rien à faire, le fil `defilante` regarde le temps aussi souvent.
const TICK: Duration = Duration::from_millis(50);

/// Les écrans sont relus aussi souvent pendant le défilement (R9) : l'écran figé caché, la
/// surveillance de `gel` ne prévient plus.
const SURVEILLANCE_ECRANS: Duration = Duration::from_millis(500);

/// Ce que les fils de la défilante postent au fil principal (`Evenement::Defilante`).
#[derive(Debug, Clone, PartialEq)]
pub enum Signal {
    /// Les pages trouvées (fil `defilante-pages`), pour l'affichage `selection`.
    Pages {
        numero: u64,
        selection: u64,
        pages: Vec<(Rect, isize)>,
        duree: Duration,
    },
    /// Le filet : aucune fenêtre de contrôle montrée à temps (l'écran figé est déjà caché).
    PageEnRetard { numero: u64 },
    /// « Trop vite » à montrer, ou à cacher (`None`).
    Message {
        numero: u64,
        texte: Option<&'static str>,
    },
    /// L'aperçu en direct numéro `apercu` est prêt.
    Apercu {
        numero: u64,
        apercu: u64,
        hauteur: u32,
    },
    /// La fin est décidée : la fenêtre de contrôle part, Entrée et Échap sont libérés.
    Finit { numero: u64 },
    /// L'image assemblée ; `relache` : la fin décidée.
    Assemblee {
        numero: u64,
        pixels: Arc<Pixels>,
        relache: Instant,
        duree: Duration,
    },
    /// Annulée par le fil (écrans changés).
    Annulee { numero: u64, cause: Annulation },
    /// Impossible (D19).
    Impossible { numero: u64, cause: Impossible },
}

/// Ce que le fil principal envoie au fil `defilante`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Commande {
    Molette(defiler::Molette),
    Terminer,
    Entree,
    Arret,
}

/// L'aperçu en direct le plus récent : son numéro et ses pixels (lu par
/// `defilante_apercu`, hors du fil principal).
type PlaceApercu = Arc<Mutex<Option<(u64, Arc<Pixels>)>>>;

/// La capture défilante en cours (de la visée à la fin), tenue par l'Application
/// (`super::defilante_en_cours`). Fil principal ; verrou jamais tenu pendant un appel à
/// Tauri ou à un module.
pub struct EnCours {
    numero: u64,
    /// L'affichage de sélection.
    selection: u64,
    /// La fenêtre de contrôle, une fois créée (cachée).
    controle: Option<Arc<Controle>>,
    /// La demande de création, pour `defilante page prete en <ms> ms`.
    creation: Instant,
    /// La création a échoué (ou n'a pas pu être postée).
    creation_ratee: bool,
    /// `defilante_pret` reçu.
    prete: bool,
    /// La fenêtre (HWND) de chaque page proposée, dans l'ordre de la liste.
    pages: Vec<isize>,
    /// Les écrans de la photo.
    ecrans: Vec<EcranInfo>,
    /// Après le relâchement.
    lancee: Option<Lancee>,
}

impl std::fmt::Debug for EnCours {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnCours")
            .field("numero", &self.numero)
            .field("selection", &self.selection)
            .field("controle", &self.controle.is_some())
            .field("prete", &self.prete)
            .field("lancee", &self.lancee.is_some())
            .finish()
    }
}

/// Une défilante lancée (zone choisie, état Défilement).
struct Lancee {
    relache: Instant,
    zone: Rect,
    ecran: EcranInfo,
    placement: Placement,
    fenetre: Option<isize>,
    /// L'écran figé et la sélection, encore montrés jusqu'à la fenêtre de contrôle montrée.
    garde: Option<GardeEcran>,
    /// La fenêtre de contrôle est montrée (et l'écran figé caché).
    montree: bool,
    /// Le filet a pris la garde : la capture est abandonnée.
    abandonnee: bool,
    /// `defilante:montrer` envoyé.
    montrer_envoye: bool,
    envoi: Sender<Commande>,
    /// Pris au départ du fil.
    reception: Option<Receiver<Commande>>,
    arret: Arc<AtomicBool>,
    apercu: PlaceApercu,
    /// Entrée est réservée.
    entree: bool,
    /// La fin est décidée (`Signal::Finit`).
    finie: bool,
}

/// Ce que le fil `defilante` reçoit au départ.
struct Depart {
    app: AppHandle,
    numero: u64,
    zone: Rect,
    fenetre: Option<isize>,
    relache: Instant,
    ecrans: Vec<EcranInfo>,
    apercu: PlaceApercu,
    apercu_taille: (u32, u32),
    reception: Receiver<Commande>,
    envoi: Sender<Commande>,
    arret: Arc<AtomicBool>,
}

fn poster(app: &AppHandle, signal: Signal) {
    super::traiter(app, Evenement::Defilante(signal));
}

/// La défilante en cours si c'est la `numero`, retirée (à détruire hors du verrou).
fn prendre(app: &AppHandle, numero: u64) -> Option<EnCours> {
    super::defilante_en_cours(app).take_if(|e| e.numero == numero)
}

/// Le bandeau de R1 : la page de contrôle n'a pas été prête à temps.
fn message_lancement_impossible() -> crate::fenetres::Message {
    crate::fenetres::Message {
        genre: crate::fenetres::Genre::Erreur,
        titre: regles::TITRE_LANCEMENT_IMPOSSIBLE.into(),
        detail: String::new(),
        actions: Vec::new(),
        duree_ms: None,
    }
}

/// La séquence du mode, en Gel : contrôles, photo sans pointeur (D1), écran figé,
/// Sélection en mode défilante, Échap réservé (le chemin commun).
pub fn capturer(app: &AppHandle, recu_a: Instant, bandeau_cache: bool) {
    super::capturer_avec_selection(app, recu_a, Mode::Defilante, bandeau_cache);
}

/// La Sélection est montrée en mode défilante (affichage `id`) : la fenêtre de contrôle
/// est créée cachée à un tour suivant de la boucle (R1), les pages sont cherchées dans le
/// fil `defilante-pages` (D2).
pub fn selection_montree(app: &AppHandle, id: u64) {
    let numero = NUMERO.fetch_add(1, Ordering::SeqCst) + 1;
    let ecrans = super::capture_en_cours(app)
        .as_ref()
        .map(|c| c.gel.ecrans.clone())
        .unwrap_or_default();
    let ancienne = super::defilante_en_cours(app).replace(EnCours {
        numero,
        selection: id,
        controle: None,
        creation: Instant::now(),
        creation_ratee: false,
        prete: false,
        pages: Vec::new(),
        ecrans: ecrans.clone(),
        lancee: None,
    });
    if let Some(a) = ancienne {
        // Ne devrait pas arriver (une seule capture à la fois) : rien n'en reste.
        log::warn!("defilante {}: ancienne capture retiree", a.numero);
        fermer(app, a);
    }
    // Jamais ici (rappel d'un autre geste) : un tour suivant de la boucle.
    let postee = super::plus_tard_nomme(app, "defilante-poste", Duration::ZERO, move |app| {
        creer_controle(app, numero, 1)
    });
    if !postee {
        if let Some(e) = super::defilante_en_cours(app)
            .as_mut()
            .filter(|e| e.numero == numero)
        {
            e.creation_ratee = true;
        }
    }
    chercher_pages(app, numero, id, ecrans);
}

/// Essais de création de la fenêtre de contrôle : juste après la destruction de celle de
/// la capture précédente, Tauri peut refuser l'étiquette encore prise.
const CREATIONS_MAX: u32 = 3;
const ENTRE_CREATIONS: Duration = Duration::from_millis(150);

/// Crée la fenêtre de contrôle (cachée) de la défilante `numero`, si elle est encore en
/// cours (essai `essai`). Fil principal, tour de boucle à lui.
fn creer_controle(app: &AppHandle, numero: u64, essai: u32) {
    let voulue = super::defilante_en_cours(app)
        .as_ref()
        .is_some_and(|e| e.numero == numero && e.controle.is_none());
    if !voulue {
        return;
    }
    let cree = controle::creer(app);
    let (rangee, montrer, echec) = {
        let mut g = super::defilante_en_cours(app);
        match (g.as_mut().filter(|e| e.numero == numero), cree) {
            (Some(e), Ok(c)) if e.controle.is_none() => {
                e.controle = Some(Arc::new(c));
                let montrer = e.prete && e.lancee.is_some();
                (None, montrer, false)
            }
            (Some(e), Err(err)) => {
                log::warn!("defilante: fenetre non creee ({essai}): {err}");
                let reessai = essai < CREATIONS_MAX
                    && super::plus_tard_nomme(
                        app,
                        "defilante-poste",
                        ENTRE_CREATIONS,
                        move |app| creer_controle(app, numero, essai + 1),
                    );
                if reessai {
                    (None, false, false)
                } else {
                    e.creation_ratee = true;
                    (None, false, e.lancee.is_some())
                }
            }
            // Abandonnée pendant la création (ou déjà créée) : détruite tout de suite.
            (_, Ok(c)) => (Some(c), false, false),
            (None, Err(_)) => (None, false, false),
        }
    };
    if let Some(c) = rangee {
        detruire_controle(Arc::new(c));
    }
    if montrer {
        envoyer_montrer(app, numero);
    }
    if echec {
        echouer_page(app, numero);
    }
}

/// Le fil `defilante-pages` : les pages qui défilent dans les premières fenêtres (S4),
/// coupées à leur écran (D2), puis `Signal::Pages`. Jamais attendu.
fn chercher_pages(app: &AppHandle, numero: u64, selection: u64, ecrans: Vec<EcranInfo>) {
    let pour = app.clone();
    let lance = std::thread::Builder::new()
        .name("defilante-pages".into())
        .spawn(move || {
            let debut = Instant::now();
            let fenetres = capture::fenetres();
            let trouvees = defiler::pages(&fenetres, regles::FENETRES_FOUILLEES);
            let pages = regles::pages_proposees(&trouvees, &ecrans);
            poster(
                &pour,
                Signal::Pages {
                    numero,
                    selection,
                    pages,
                    duree: debut.elapsed(),
                },
            );
        });
    if let Err(e) = lance {
        // Sans pages, le clic ne vise rien : on trace (D2).
        log::warn!("defilante: pages non cherchees: {e}");
    }
}

/// Pages trouvées : proposées à la Sélection si la visée est encore la même.
fn pages_trouvees(
    app: &AppHandle,
    numero: u64,
    selection_id: u64,
    pages: Vec<(Rect, isize)>,
    duree: Duration,
) {
    log::info!("{}", regles::ligne_pages(pages.len(), duree));
    let visee = super::defilante_en_cours(app)
        .as_ref()
        .is_some_and(|e| e.numero == numero && e.lancee.is_none());
    if !visee {
        return;
    }
    let proposees = pages
        .iter()
        .map(|(rect, _)| FenetreProposee {
            rect: *rect,
            rayon: 0,
        })
        .collect();
    if super::proposer_pages(app, selection_id, proposees) {
        if let Some(e) = super::defilante_en_cours(app)
            .as_mut()
            .filter(|e| e.numero == numero)
        {
            e.pages = pages.iter().map(|(_, f)| *f).collect();
        }
    }
}

/// Abandon de la visée au relâchement (rien de valide) : tout est caché, Repos, rien dit.
fn abandonner_visee(app: &AppHandle, mut garde: GardeEcran) {
    garde.cacher_tout();
    drop(garde);
    super::appliquer(app, Declencheur::SelectionAnnulee);
    visee_abandonnee(app);
}

/// Zone tracée ou page cliquée (en Sélection, capture encore rangée) : agrandie (D3),
/// Défilement, Entrée réservée, l'écran figé gardé jusqu'à la fenêtre de contrôle montrée
/// (R1), filet de `DELAI_PAGE`.
pub fn zone_choisie(app: &AppHandle, zone: Rect, page: Option<usize>) {
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
    let Some(ecran) = regles::ecran_de(zone, &ecrans) else {
        abandonner_visee(app, garde);
        return;
    };
    let (zone, agrandie) = regles::agrandir(zone, &ecran);
    let visee = super::defilante_en_cours(app)
        .as_ref()
        .map(|e| (e.numero, page.and_then(|i| e.pages.get(i).copied())));
    let Some((numero, fenetre)) = visee else {
        abandonner_visee(app, garde);
        return;
    };
    if super::appliquer(app, Declencheur::DefilementChoisi).is_none() {
        abandonner_visee(app, garde);
        return;
    }
    log::info!("{}", regles::ligne_zone(zone, page.is_some(), agrandie));
    let entree = reserver_entree(app);
    let placement = regles::placer(zone, &ecran);
    let (envoi, reception) = mpsc::channel();
    let (prete, ratee) = {
        let mut g = super::defilante_en_cours(app);
        match g.as_mut().filter(|e| e.numero == numero) {
            Some(e) => {
                e.lancee = Some(Lancee {
                    relache,
                    zone,
                    ecran,
                    placement,
                    fenetre,
                    garde: Some(garde),
                    montree: false,
                    abandonnee: false,
                    montrer_envoye: false,
                    envoi,
                    reception: Some(reception),
                    arret: Arc::new(AtomicBool::new(false)),
                    apercu: Arc::new(Mutex::new(None)),
                    entree,
                    finie: false,
                });
                (e.prete && e.controle.is_some(), e.creation_ratee)
            }
            None => {
                // Impossible sur ce fil (rien entre la lecture et ici) ; sûr quand même.
                drop(g);
                let mut garde = garde;
                garde.cacher_tout();
                liberer_entree(app, entree);
                super::appliquer(app, Declencheur::DefilementImpossible);
                return;
            }
        }
    };
    if ratee {
        echouer_page(app, numero);
        return;
    }
    if prete {
        envoyer_montrer(app, numero);
    }
    armer_filet(app, numero, relache);
}

/// Réserve Entrée en global (= « Terminer », R5). Vrai si elle l'est.
fn reserver_entree(app: &AppHandle) -> bool {
    let pour = app.clone();
    match raccourcis::reserver_entree(app, move |s| super::traiter(&pour, Evenement::Raccourci(s)))
    {
        Ok(()) => {
            log::info!("entree globale reservee");
            true
        }
        Err(e) => {
            // « Terminer » reste.
            log::warn!("entree globale non reservee: {e}");
            false
        }
    }
}

/// Libère Entrée si `reservee`.
fn liberer_entree(app: &AppHandle, reservee: bool) {
    if !reservee {
        return;
    }
    match raccourcis::liberer_entree(app) {
        Ok(()) => log::info!("entree globale liberee"),
        Err(e) => log::warn!("entree globale non liberee: {e}"),
    }
}

/// Le filet de R1 : sans fenêtre de contrôle montrée `DELAI_PAGE` après le relâchement,
/// l'écran figé est caché depuis ce fil (même si la boucle est prise), puis la capture est
/// abandonnée sur le fil principal.
fn armer_filet(app: &AppHandle, numero: u64, relache: Instant) {
    let pour = app.clone();
    let lance = std::thread::Builder::new()
        .name("defilante-filet".into())
        .spawn(move || {
            let reste = (relache + regles::DELAI_PAGE).saturating_duration_since(Instant::now());
            std::thread::sleep(reste);
            let garde = {
                let mut g = super::defilante_en_cours(&pour);
                g.as_mut()
                    .filter(|e| e.numero == numero)
                    .and_then(|e| e.lancee.as_mut())
                    .filter(|l| !l.montree && !l.abandonnee)
                    .map(|l| {
                        l.abandonnee = true;
                        l.garde.take()
                    })
            };
            let Some(garde) = garde else {
                return;
            };
            // Hors du verrou : la garde cache l'écran figé et la sélection.
            drop(garde);
            poster(&pour, Signal::PageEnRetard { numero });
        });
    if let Err(e) = lance {
        log::warn!("defilante: filet non arme ({e})");
        let montree = super::defilante_en_cours(app)
            .as_ref()
            .and_then(|e| e.lancee.as_ref())
            .is_some_and(|l| l.montree);
        if !montree {
            // Sans filet, l'écran figé ne doit pas risquer de rester : abandon tout de suite.
            echouer_page(app, numero);
        }
    }
}

/// Envoie `defilante:montrer` à la page (une fois), la fenêtre posée sur l'écran de la zone.
fn envoyer_montrer(app: &AppHandle, numero: u64) {
    let reduites = super::reduites_editeur(app);
    let envoi = {
        let mut g = super::defilante_en_cours(app);
        let Some(e) = g.as_mut().filter(|e| e.numero == numero) else {
            return;
        };
        let Some(c) = e.controle.clone() else {
            return;
        };
        let Some(l) = e
            .lancee
            .as_mut()
            .filter(|l| !l.montrer_envoye && !l.abandonnee)
        else {
            return;
        };
        l.montrer_envoye = true;
        (c, l.ecran, charge_montrer(numero, l, reduites))
    };
    let (c, ecran, charge) = envoi;
    if let Err(e) = c.poser(ecran.rect) {
        log::warn!("defilante: fenetre non posee: {e}");
    }
    if let Err(e) = c.emettre(controle::EVENEMENT_MONTRER, charge) {
        // Sans réponse de la page, le filet abandonnera.
        log::warn!("defilante: charge non envoyee: {e}");
    }
}

/// La charge de `defilante:montrer` : pixels réels relatifs au coin de la fenêtre (posée
/// sur tout l'écran de la zone).
fn charge_montrer(numero: u64, l: &Lancee, reduites: bool) -> ChargeMontrer {
    let (ox, oy) = (l.ecran.rect.x, l.ecran.rect.y);
    let p = l.placement;
    ChargeMontrer {
        id: numero,
        echelle: l.ecran.echelle,
        zone: RectPage {
            x: l.zone.x - ox,
            y: l.zone.y - oy,
            largeur: l.zone.largeur.max(0) as u32,
            hauteur: l.zone.hauteur.max(0) as u32,
        },
        apercu: ApercuPage {
            x: p.apercu.x - ox,
            y: p.apercu.y - oy,
            largeur: p.apercu.largeur.max(0) as u32,
            hauteur_max: p.apercu.hauteur.max(0) as u32,
        },
        pilule: PointPage {
            x: p.pilule.0 - ox,
            y: p.pilule.1 - oy,
        },
        libelle: regles::LIBELLE_TERMINER.into(),
        reduites,
    }
}

/// `defilante_pret` : la page écoute. Rend `reduites` ; `None` sans défilante en cours.
pub(super) fn page_prete(app: &AppHandle) -> Option<bool> {
    let reduites = super::reduites_editeur(app);
    let (numero, montrer) = {
        let mut g = super::defilante_en_cours(app);
        let e = g.as_mut()?;
        if !e.prete {
            e.prete = true;
            log::info!("{}", regles::ligne_page_prete(e.creation.elapsed()));
        }
        // Une page rechargée avant d'être montrée reçoit de nouveau sa charge.
        let montrer = match e.lancee.as_mut() {
            Some(l) if !l.montree && !l.abandonnee => {
                l.montrer_envoye = false;
                true
            }
            _ => false,
        };
        (e.numero, montrer && e.controle.is_some())
    };
    theme::envoyer_a(app, controle::ETIQUETTE_DEFILANTE);
    if montrer {
        envoyer_montrer(app, numero);
    }
    Some(reduites)
}

/// `defilante_zone` : la pilule est posée (pixels CSS de la page). La première fois, dans
/// ce même tour : la fenêtre de contrôle montrée, l'écran figé et la sélection cachés
/// (D4), puis le fil `defilante`.
pub(super) fn pilule_posee(app: &AppHandle, id: u64, x: f64, y: f64, largeur: f64, hauteur: f64) {
    let pris = {
        let mut g = super::defilante_en_cours(app);
        let Some(e) = g.as_mut().filter(|e| e.numero == id) else {
            return;
        };
        let Some(c) = e.controle.clone() else {
            return;
        };
        let Some(l) = e.lancee.as_mut().filter(|l| !l.abandonnee && !l.finie) else {
            return;
        };
        let ecran = l.ecran;
        let depart = if l.montree {
            None
        } else {
            l.montree = true;
            let reception = l.reception.take();
            reception.map(|reception| {
                (
                    l.garde.take(),
                    Depart {
                        app: app.clone(),
                        numero: id,
                        zone: l.zone,
                        fenetre: l.fenetre,
                        relache: l.relache,
                        ecrans: e.ecrans.clone(),
                        apercu: Arc::clone(&l.apercu),
                        apercu_taille: (
                            l.placement.apercu.largeur.max(1) as u32,
                            l.placement.apercu.hauteur.max(1) as u32,
                        ),
                        reception,
                        envoi: l.envoi.clone(),
                        arret: Arc::clone(&l.arret),
                    },
                )
            })
        };
        (c, ecran, depart)
    };
    let (c, ecran, depart) = pris;
    // L'origine et l'échelle réelles de la fenêtre posée (pas celles de la charge) : une
    // page sur un écran d'une autre échelle rapporte en pixels CSS de son écran.
    let ecran = c.identifiant().and_then(systeme::rectangle_fenetre).map_or(
        ecran,
        |(x, y, _, _, echelle)| EcranInfo {
            rect: Rect { x, y, ..ecran.rect },
            echelle,
            ..ecran
        },
    );
    let rect = regles::pilule_reelle(&ecran, x, y, largeur, hauteur);
    c.pilule(rect);
    let Some((garde, depart)) = depart else {
        return;
    };
    // Le même tour : la fenêtre de contrôle paraît, l'écran figé et la sélection partent.
    let montree = c.montrer();
    gel::cacher_ecran_fige();
    if let Some(s) = app.try_state::<Selection>() {
        s.cacher();
    }
    if let Some(mut garde) = garde {
        // Déjà caché ici ; Échap reste réservé le temps du défilement.
        garde.fait = true;
    }
    if let Err(e) = montree {
        log::warn!("defilante: fenetre non montree: {e}");
        echouer_page(app, id);
        return;
    }
    lancer_fil(app, depart);
}

/// `defilante_terminer` : fin `bouton`.
pub(super) fn terminer(app: &AppHandle, id: u64) {
    envoyer(app, Some(id), Commande::Terminer);
}

/// Envoie `commande` au fil de la défilante en cours (`numero` : seulement celle-là).
fn envoyer(app: &AppHandle, numero: Option<u64>, commande: Commande) {
    let envoi = super::defilante_en_cours(app)
        .as_ref()
        .filter(|e| numero.is_none_or(|n| n == e.numero))
        .and_then(|e| e.lancee.as_ref())
        .filter(|l| !l.finie && !l.abandonnee)
        .map(|l| l.envoi.clone());
    if let Some(envoi) = envoi {
        let _ = envoi.send(commande);
    }
}

/// `defilante_apercu` : les octets PNG de l'aperçu `numero` de la capture `id` (hors du fil
/// principal) ; refus s'il n'est pas (ou plus) celui en cours.
pub(super) fn apercu_png(app: &AppHandle, id: u64, numero: u64) -> Result<Vec<u8>, String> {
    let place = super::defilante_en_cours(app)
        .as_ref()
        .filter(|e| e.numero == id)
        .and_then(|e| e.lancee.as_ref())
        .map(|l| Arc::clone(&l.apercu))
        .ok_or_else(|| "defilante".to_string())?;
    let pixels = match &*place.lock().unwrap_or_else(|e| e.into_inner()) {
        Some((n, p)) if *n == numero => Arc::clone(p),
        _ => return Err("defilante".into()),
    };
    sortie::encoder_png(&pixels)
}

/// La visée en mode défilante est abandonnée (écran figé et sélection déjà cachés) : la
/// fenêtre de contrôle est détruite.
pub fn visee_abandonnee(app: &AppHandle) {
    // Pris sous le verrou, détruit hors du verrou.
    let ancienne = super::defilante_en_cours(app).take();
    if let Some(e) = ancienne {
        fermer(app, e);
    }
}

/// Tout ce que tient `e` est rendu : le fil arrêté, l'écran figé caché s'il l'était encore,
/// Entrée et Échap libérés, la fenêtre de contrôle détruite. Hors du verrou.
fn fermer(app: &AppHandle, e: EnCours) {
    let EnCours {
        controle, lancee, ..
    } = e;
    if let Some(l) = lancee {
        l.arret.store(true, Ordering::SeqCst);
        let _ = l.envoi.send(Commande::Arret);
        if let Some(mut garde) = l.garde {
            garde.cacher_tout();
        }
        liberer_entree(app, l.entree);
    }
    if let Some(c) = controle {
        detruire_controle(c);
    }
    super::liberer_echap(app);
}

/// Cache puis détruit la fenêtre de contrôle.
fn detruire_controle(c: Arc<Controle>) {
    match Arc::try_unwrap(c) {
        Ok(c) => {
            if let Err(e) = c.detruire() {
                log::warn!("defilante: fenetre non detruite: {e}");
            }
        }
        Err(c) => {
            c.cacher();
            log::warn!("defilante: fenetre non detruite: encore tenue");
        }
    }
}

/// Annule la défilante lancée (D18) : rien n'est gardé, aucun message. Faux s'il n'y en a
/// pas (visée, ou fin déjà décidée).
fn annuler(app: &AppHandle, numero: Option<u64>, cause: Annulation) -> bool {
    let prise = super::defilante_en_cours(app).take_if(|e| {
        numero.is_none_or(|n| n == e.numero) && e.lancee.as_ref().is_some_and(|l| !l.finie)
    });
    let Some(e) = prise else {
        return false;
    };
    fermer(app, e);
    super::appliquer(app, Declencheur::DefilementAnnule);
    log::info!("{}", regles::ligne_annulee(cause));
    true
}

/// La défilante finit sans image (D19) : Repos, bandeau d'erreur.
fn rendre_impossible(app: &AppHandle, numero: u64, cause: Impossible) {
    let Some(e) = prendre(app, numero) else {
        return;
    };
    fermer(app, e);
    super::appliquer(app, Declencheur::DefilementImpossible);
    log::info!("{}", regles::ligne_impossible(cause));
    log::info!("capture impossible: defilante");
    let message = if cause == Impossible::Page {
        message_lancement_impossible()
    } else {
        zone::message_capture_impossible()
    };
    super::afficher(app, message);
}

/// La fenêtre de contrôle n'a pas été prête (ou montrée) à temps : abandon sans image (R1).
fn echouer_page(app: &AppHandle, numero: u64) {
    let lancee = super::defilante_en_cours(app)
        .as_ref()
        .is_some_and(|e| e.numero == numero && e.lancee.is_some());
    if lancee {
        rendre_impossible(app, numero, Impossible::Page);
    }
}

/// Un signal de la Sélection : vrai s'il est pris ici. En Défilement, l'affichage gardé
/// (zone choisie) ne décide plus rien : ses signaux sont ignorés.
pub fn signal_selection(app: &AppHandle, signal: &selection::Signal) -> bool {
    super::defilante_en_cours(app)
        .as_ref()
        .is_some_and(|e| e.lancee.is_some() && e.selection == signal.id())
}

/// Échap global : vrai s'il est pris ici (en Défilement : annulée).
pub fn echap(app: &AppHandle) -> bool {
    annuler(app, None, Annulation::Echap)
}

/// Entrée globale (= « Terminer »).
pub fn entree(app: &AppHandle) {
    envoyer(app, None, Commande::Entree);
}

/// Écrans changés ou session verrouillée : vrai s'ils sont pris ici (R9 : annulée, sans
/// message).
pub fn bureau_change(app: &AppHandle, changement: Changement) -> bool {
    let cause = match changement {
        Changement::Ecrans => Annulation::Ecrans,
        Changement::SessionVerrouillee => Annulation::Session,
    };
    annuler(app, None, cause)
}

/// Ce que les fils postent.
pub fn traiter(app: &AppHandle, signal: Signal) {
    match signal {
        Signal::Pages {
            numero,
            selection,
            pages,
            duree,
        } => pages_trouvees(app, numero, selection, pages, duree),
        Signal::PageEnRetard { numero } => echouer_page(app, numero),
        Signal::Message { numero, texte } => emettre(
            app,
            numero,
            controle::EVENEMENT_MESSAGE,
            ChargeMessage {
                id: numero,
                texte: texte.map(String::from),
            },
        ),
        Signal::Apercu {
            numero,
            apercu,
            hauteur,
        } => emettre(
            app,
            numero,
            controle::EVENEMENT_APERCU,
            ChargeApercu {
                id: numero,
                numero: apercu,
                hauteur,
            },
        ),
        Signal::Finit { numero } => fin_decidee(app, numero),
        Signal::Assemblee {
            numero,
            pixels,
            relache,
            duree,
        } => assemblee(app, numero, pixels, relache, duree),
        Signal::Annulee { numero, cause } => {
            annuler(app, Some(numero), cause);
        }
        Signal::Impossible { numero, cause } => rendre_impossible(app, numero, cause),
    }
}

/// Envoie une charge à la page de contrôle de la défilante `numero`.
fn emettre<S: serde::Serialize + Clone>(app: &AppHandle, numero: u64, evenement: &str, charge: S) {
    let c = super::defilante_en_cours(app)
        .as_ref()
        .filter(|e| e.numero == numero && e.lancee.as_ref().is_some_and(|l| !l.finie))
        .and_then(|e| e.controle.clone());
    if let Some(c) = c {
        if let Err(e) = c.emettre(evenement, charge) {
            log::warn!("defilante: charge non envoyee: {e}");
        }
    }
}

/// La fin est décidée : le voile, le trait, l'aperçu et la pilule partent d'un coup ;
/// Entrée et Échap libérés (D14).
fn fin_decidee(app: &AppHandle, numero: u64) {
    let pris = {
        let mut g = super::defilante_en_cours(app);
        let Some(e) = g.as_mut().filter(|e| e.numero == numero) else {
            return;
        };
        let Some(l) = e.lancee.as_mut().filter(|l| !l.finie) else {
            return;
        };
        l.finie = true;
        let entree = std::mem::replace(&mut l.entree, false);
        (e.controle.clone(), entree)
    };
    let (c, entree) = pris;
    if let Some(c) = c {
        if let Err(e) = c.emettre(controle::EVENEMENT_CACHER, ChargeCacher { id: numero }) {
            log::warn!("defilante: charge non envoyee: {e}");
        }
        c.pilule(None);
        c.cacher();
    }
    liberer_entree(app, entree);
    super::liberer_echap(app);
}

/// L'image assemblée : `DefilementFini`, puis la sortie commune (aperçu du haut, D16).
fn assemblee(app: &AppHandle, numero: u64, pixels: Arc<Pixels>, relache: Instant, duree: Duration) {
    let Some(e) = prendre(app, numero) else {
        log::info!("defilante {numero}: assemblage perime ignore");
        return;
    };
    let zone = e.lancee.as_ref().map(|l| l.zone).unwrap_or_default();
    fermer(app, e);
    if super::appliquer(app, Declencheur::DefilementFini).is_none() {
        log::warn!("defilante {numero}: fin hors Defilement, image jetee");
        return;
    }
    let pixels = Arc::try_unwrap(pixels).unwrap_or_else(|p| (*p).clone());
    super::lancer_sortie(app, pixels, zone, duree, relache, false, true);
}

/// Arrêt de l'app : la défilante en cours est annulée, sa fenêtre détruite.
pub fn arreter(app: &AppHandle) {
    if annuler(app, None, Annulation::Arret) {
        return;
    }
    // Pris sous le verrou, détruit hors du verrou.
    let ancienne = super::defilante_en_cours(app).take();
    if let Some(e) = ancienne {
        fermer(app, e);
    }
}

/// Lance le fil `defilante`. Une panique dans le fil finit la capture (`fil`) : jamais un
/// état Défilement sans fin (D19).
fn lancer_fil(app: &AppHandle, depart: Depart) {
    let numero = depart.numero;
    let pour = app.clone();
    let lance = std::thread::Builder::new()
        .name("defilante".into())
        .spawn(move || {
            let issue = panic::catch_unwind(AssertUnwindSafe(|| defiler_et_assembler(depart)));
            if issue.is_err() {
                log::error!("defilante: fil arrete (panique)");
                poster(
                    &pour,
                    Signal::Impossible {
                        numero,
                        cause: Impossible::Fil,
                    },
                );
            }
        });
    if let Err(e) = lance {
        log::warn!("defilante: fil impossible ({e})");
        rendre_impossible(app, numero, Impossible::Fil);
    }
}

/// Le corps du fil `defilante` : première image, puis la course jusqu'à la fin, l'arrêt ou
/// l'impossible. Rend toujours un signal sauf à l'arrêt demandé.
fn defiler_et_assembler(d: Depart) {
    if !systeme::attendre_composition(super::ATTENTE_COMPOSITION) {
        log::warn!("defilante: composition non attendue");
    }
    let mut prise = match vivant::ouvrir(d.zone) {
        // Une zone coupée au bord du bureau ne serait plus celle placée et montrée.
        Ok(p) if p.zone() != d.zone => {
            log::warn!("defilante: prise en echec (zone coupee)");
            impossible(&d, Impossible::Prise);
            return;
        }
        Ok(p) => p,
        Err(e) => {
            log::warn!("defilante: prise en echec ({e})");
            impossible(&d, Impossible::Prise);
            return;
        }
    };
    let mut image = Pixels {
        largeur: 0,
        hauteur: 0,
        rgba: Vec::new(),
    };
    let mut premiere = None;
    for essai in 0..regles::PRISES_EN_ECHEC_MAX {
        if d.arret.load(Ordering::SeqCst) {
            return;
        }
        match prise.prendre(&mut image) {
            Ok(duree) => {
                premiere = Some(duree);
                break;
            }
            Err(e) => {
                if essai == 0 {
                    log::warn!("defilante: prise en echec ({e})");
                }
                std::thread::sleep(regles::PRISE_POSEE);
            }
        }
    }
    let Some(duree_prise) = premiere else {
        impossible(&d, Impossible::Prise);
        return;
    };
    let relache_premiere = d.relache.elapsed();
    log::info!(
        "{}",
        regles::ligne_premiere_image(image.largeur, image.hauteur, duree_prise, relache_premiere)
    );
    let assemblage = match Assemblage::nouveau(&image, regles::limite(image.largeur)) {
        Ok(a) => a,
        Err(e) => {
            log::warn!("defilante: assemblage refuse ({e})");
            impossible(&d, Impossible::Memoire);
            return;
        }
    };
    let defileur = match defiler::preparer(d.zone, d.fenetre) {
        Ok(def) => Some(def),
        Err(e) => {
            log::warn!("defilante: defilement impossible ({e})");
            None
        }
    };
    log::info!(
        "{}",
        regles::ligne_moyen(defileur.as_ref().and_then(|def| def.moyen()))
    );
    let pour_molette = d.envoi.clone();
    let _ecoute = match defiler::ecouter_molette(move |m| {
        let _ = pour_molette.send(Commande::Molette(m));
    }) {
        Ok(e) => Some(e),
        Err(e) => {
            log::warn!("defilante: crochet de la molette impossible ({e})");
            None
        }
    };
    let maintenant = Instant::now();
    let mut course = Course::nouvelle(maintenant);
    course.regler_pas(regles::hauteur_utile(image.hauteur, Fixes::default()));
    if defileur.as_ref().and_then(|def| def.moyen()).is_none() {
        course.sans_moyen(maintenant);
    }
    let mut fil = Fil {
        prise,
        defileur,
        assemblage: Some(assemblage),
        reference: image,
        image: Pixels {
            largeur: 0,
            hauteur: 0,
            rgba: Vec::new(),
        },
        precedente: Pixels {
            largeur: 0,
            hauteur: 0,
            rgba: Vec::new(),
        },
        course,
        commandes: VecDeque::new(),
        a_voir: VecDeque::new(),
        prochaine: None,
        pas_en_attente: None,
        apercu_voulu: true,
        dernier_apercu: None,
        numero_apercu: 0,
        images: 1,
        gardees: 1,
        perdues: 0,
        echecs: 0,
        echec_dit: false,
        durees_pas: Vec::new(),
        durees_mesure: Vec::new(),
        fixes: Fixes::default(),
        surveillance: maintenant,
        relache_premiere,
        d,
    };
    fil.courir();
    // L'écoute vit jusqu'ici : son crochet est retiré à la fin du fil.
}

/// Poste `Impossible` sauf si l'arrêt a été demandé.
fn impossible(d: &Depart, cause: Impossible) {
    if !d.arret.load(Ordering::SeqCst) {
        poster(
            &d.app,
            Signal::Impossible {
                numero: d.numero,
                cause,
            },
        );
    }
}

/// L'état du fil `defilante`.
struct Fil {
    d: Depart,
    prise: vivant::Prise,
    defileur: Option<defiler::Defileur>,
    /// Pris à la fin.
    assemblage: Option<Assemblage>,
    /// La dernière image gardée : chaque image est mesurée contre elle (D10).
    reference: Pixels,
    /// Tampon de prise.
    image: Pixels,
    /// L'image d'avant, pendant qu'on attend la page posée (D6).
    precedente: Pixels,
    course: Course,
    /// Commandes reçues pendant une attente de page posée.
    commandes: VecDeque<Commande>,
    /// Ce qui reste à montrer à la course.
    a_voir: VecDeque<Vu>,
    /// La prochaine image demandée (`Geste::Prendre`).
    prochaine: Option<Instant>,
    /// Un pas retenu par la garde de la molette (D7) : retenté à ce moment.
    pas_en_attente: Option<(i32, Instant)>,
    apercu_voulu: bool,
    dernier_apercu: Option<Instant>,
    numero_apercu: u64,
    images: u32,
    gardees: u32,
    perdues: u32,
    /// Prises en échec de suite.
    echecs: u32,
    /// La série d'échecs en cours est déjà au journal.
    echec_dit: bool,
    durees_pas: Vec<Duration>,
    durees_mesure: Vec<Duration>,
    fixes: Fixes,
    surveillance: Instant,
    relache_premiere: Duration,
}

impl Fil {
    fn arrete(&self) -> bool {
        self.d.arret.load(Ordering::SeqCst)
    }

    fn poster(&self, signal: Signal) {
        if !self.arrete() {
            poster(&self.d.app, signal);
        }
    }

    /// La boucle du fil, jusqu'à la fin, l'arrêt ou l'annulation.
    fn courir(&mut self) {
        loop {
            if self.arrete() {
                return;
            }
            let maintenant = Instant::now();
            if maintenant.saturating_duration_since(self.surveillance) >= SURVEILLANCE_ECRANS {
                self.surveillance = maintenant;
                let ecrans = gel::ecrans();
                // Une liste vide est une lecture ratée, pas un bureau sans écran.
                if !ecrans.is_empty() && regles::ecrans_changes(&self.d.ecrans, &ecrans) {
                    self.poster(Signal::Annulee {
                        numero: self.d.numero,
                        cause: Annulation::Ecrans,
                    });
                    return;
                }
            }
            let echeance = [
                self.prochaine,
                self.pas_en_attente.map(|(_, t)| t),
                self.apercu_voulu.then(|| {
                    self.dernier_apercu
                        .map_or(maintenant, |t| t + regles::APERCU_INTERVALLE)
                }),
                Some(maintenant + TICK),
            ]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(maintenant + TICK);
            let mut vu = false;
            match self.commande(echeance.saturating_duration_since(maintenant)) {
                Some(Commande::Arret) => return,
                Some(Commande::Molette(m)) => {
                    self.a_voir.push_back(Vu::Molette {
                        dans_zone: self.d.zone.contient(m.point),
                    });
                    vu = true;
                }
                Some(Commande::Terminer) => {
                    self.a_voir.push_back(Vu::Terminer);
                    vu = true;
                }
                Some(Commande::Entree) => {
                    self.a_voir.push_back(Vu::Entree);
                    vu = true;
                }
                None => {}
            }
            if self.arrete() {
                return;
            }
            let maintenant = Instant::now();
            if self.apercu_voulu
                && self
                    .dernier_apercu
                    .is_none_or(|t| maintenant >= t + regles::APERCU_INTERVALLE)
            {
                self.publier_apercu();
            }
            // Les échéances passent même sous un flot de crans (molette rapide en `Main`).
            if let Some((pas, t)) = self.pas_en_attente {
                if maintenant >= t {
                    self.pas_en_attente = None;
                    if self.course.conduite() == Conduite::Auto {
                        self.descendre(pas);
                    }
                }
            }
            if self.prochaine.is_some_and(|t| maintenant >= t) {
                self.prochaine = None;
                self.prendre_une();
            }
            if !vu && self.a_voir.is_empty() {
                self.a_voir.push_back(Vu::Temps(maintenant));
            }
            while let Some(v) = self.a_voir.pop_front() {
                self.voir(v);
                if self.course.fin().is_some() || self.arrete() {
                    break;
                }
            }
            if self.arrete() {
                return;
            }
            if self.echecs >= regles::PRISES_EN_ECHEC_MAX && self.course.fin().is_none() {
                // Des images sont déjà gardées : la capture finit avec ce qui est pris (D19).
                log::info!("{}", regles::ligne_impossible(Impossible::Prise));
                self.course.finir(Fin::Limite);
            }
            if let Some(fin) = self.course.fin() {
                self.finir(fin);
                return;
            }
        }
    }

    /// La prochaine commande, attendue au plus `attente`.
    fn commande(&mut self, attente: Duration) -> Option<Commande> {
        if let Some(c) = self.commandes.pop_front() {
            return Some(c);
        }
        match self.d.reception.recv_timeout(attente) {
            Ok(c) => Some(c),
            Err(RecvTimeoutError::Timeout) => None,
            // Plus personne pour envoyer : l'app s'arrête.
            Err(RecvTimeoutError::Disconnected) => Some(Commande::Arret),
        }
    }

    /// Les commandes arrivées pendant une attente, gardées pour la boucle ; vrai à l'arrêt.
    fn relever(&mut self) -> bool {
        while let Ok(c) = self.d.reception.try_recv() {
            if c == Commande::Arret {
                return true;
            }
            self.commandes.push_back(c);
        }
        self.arrete()
    }

    /// Montre `vu` à la course et fait ses gestes.
    fn voir(&mut self, vu: Vu) {
        let avant = self.course.conduite();
        let gestes = regles::suite(&mut self.course, vu, Instant::now());
        let apres = self.course.conduite();
        if avant != Conduite::Main && apres == Conduite::Main {
            self.pas_en_attente = None;
            if matches!(vu, Vu::Molette { .. }) {
                log::info!("{}", regles::LIGNE_MAIN);
            } else if !gestes.contains(&Geste::Repli) {
                // La page saute trop loin même à petit pas : la molette, sans rien dire.
                log::info!("{}", regles::ligne_repli(None, None));
            }
        }
        for g in gestes {
            if self.arrete() {
                return;
            }
            match g {
                Geste::Descendre(pas) => self.descendre(pas),
                Geste::Prendre(delai) => self.prochaine = Some(Instant::now() + delai),
                Geste::Message(texte) => {
                    log::info!(
                        "{}",
                        if texte.is_some() {
                            regles::LIGNE_TROP_VITE
                        } else {
                            regles::LIGNE_REPRIS
                        }
                    );
                    self.poster(Signal::Message {
                        numero: self.d.numero,
                        texte,
                    });
                }
                Geste::Apercu => self.apercu_voulu = true,
                Geste::Repli => {
                    let (ancien, nouveau) = match self.defileur.as_mut() {
                        Some(def) => {
                            let ancien = def.moyen();
                            (ancien, def.abandonner())
                        }
                        None => (None, None),
                    };
                    log::info!("{}", regles::ligne_repli(ancien, nouveau));
                }
                Geste::Finir(_) => {}
            }
        }
    }

    /// Un pas, puis les images jusqu'à la page posée (D6), puis la dernière, mesurée.
    fn descendre(&mut self, pas: i32) {
        let Some(def) = self.defileur.as_mut() else {
            self.prochaine = Some(Instant::now() + regles::PRISE_POSEE);
            return;
        };
        let avant = def.position();
        match def.descendre(pas) {
            Ok(Envoi::Saute) => {
                // Le point visé n'est pas sur la page (D7) : retenté au tour suivant.
                self.pas_en_attente = Some((pas, Instant::now() + regles::PRISE_POSEE));
                return;
            }
            Ok(Envoi::Envoye) => {}
            // Compté comme sans effet : l'image le dira.
            Err(e) => log::warn!("defilante: pas refuse ({e})"),
        }
        let debut = Instant::now();
        let posee = self.attendre_posee();
        self.durees_pas.push(debut.elapsed());
        if !posee {
            // Aucune image prise : on retente une image plus tard.
            self.prochaine = Some(Instant::now() + regles::PRISE_POSEE);
            return;
        }
        let apres = self.defileur.as_ref().and_then(|d| d.position());
        let utile = regles::hauteur_utile(self.reference.hauteur, self.fixes);
        // Le décalage lu par UI Automation, sinon le pas demandé.
        let attendu = regles::decalage_attendu(avant, apres, utile).or(Some(pas));
        let mesure = self.mesurer(attendu);
        self.a_voir.push_back(Vu::Image(mesure, apres));
    }

    /// Images toutes les `PRISE_POSEE` jusqu'à deux égales de suite ou
    /// `ATTENTE_POSEE_MAX` ; la dernière reste dans `image`. Faux si aucune n'a été prise.
    fn attendre_posee(&mut self) -> bool {
        let debut = Instant::now();
        let mut prise = false;
        loop {
            std::thread::sleep(regles::PRISE_POSEE);
            if self.relever() {
                return prise;
            }
            let ecoule = debut.elapsed() >= regles::ATTENTE_POSEE_MAX;
            if !self.prendre() {
                if prise {
                    // Le tampon a pu être touché : la dernière bonne image y revient.
                    self.image.clone_from(&self.precedente);
                }
                if ecoule {
                    return prise;
                }
                continue;
            }
            if prise {
                let m = defilement::mesurer(
                    &self.precedente,
                    &self.image,
                    self.fixes,
                    None,
                    &TOLERANCE,
                );
                if m == Mesure::Immobile {
                    return true;
                }
            }
            prise = true;
            if ecoule {
                return true;
            }
            self.precedente.clone_from(&self.image);
        }
    }

    /// Une image dans `image` ; vrai si elle est prise. Les échecs sont comptés (D19).
    fn prendre(&mut self) -> bool {
        match self.prise.prendre(&mut self.image) {
            Ok(_) => {
                self.echecs = 0;
                self.echec_dit = false;
                true
            }
            Err(e) => {
                self.echecs += 1;
                if !self.echec_dit {
                    self.echec_dit = true;
                    log::warn!("defilante: prise en echec ({e})");
                }
                false
            }
        }
    }

    /// Une image demandée (`Geste::Prendre`), mesurée et montrée à la course.
    fn prendre_une(&mut self) {
        if !self.prendre() {
            self.prochaine = Some(Instant::now() + regles::PRISE_POSEE);
            return;
        }
        let position = self.defileur.as_ref().and_then(|d| d.position());
        let mesure = self.mesurer(None);
        self.a_voir.push_back(Vu::Image(mesure, position));
    }

    /// Mesure `image` contre la dernière image gardée et l'ajoute (D10 à D13).
    fn mesurer(&mut self, attendu: Option<i32>) -> Mesure {
        let Some(assemblage) = self.assemblage.as_mut() else {
            return Mesure::Immobile;
        };
        let debut = Instant::now();
        let mesure = defilement::mesurer(
            &self.reference,
            &self.image,
            assemblage.fixes(),
            attendu,
            &TOLERANCE,
        );
        self.durees_mesure.push(debut.elapsed());
        self.images += 1;
        // La dernière image gardée est celle dont des lignes sont entrées (D10).
        let gardee = match assemblage.ajouter(&self.image, &mesure) {
            Ajout::Ajoutees(_) => true,
            Ajout::Limite(_) => {
                self.course.finir(Fin::Limite);
                true
            }
            Ajout::Rien => false,
        };
        if gardee {
            self.gardees += 1;
            std::mem::swap(&mut self.reference, &mut self.image);
        }
        if mesure == Mesure::Perdue {
            self.perdues += 1;
        }
        let fixes = assemblage.fixes();
        if fixes != self.fixes {
            self.fixes = fixes;
            log::info!("{}", regles::ligne_fixes(fixes));
            self.course
                .regler_pas(regles::hauteur_utile(self.reference.hauteur, fixes));
        }
        mesure
    }

    /// L'aperçu en direct : le bas de l'image assemblée, rangé pour `defilante_apercu`.
    fn publier_apercu(&mut self) {
        let Some(assemblage) = self.assemblage.as_ref() else {
            return;
        };
        let (largeur, hauteur_max) = self.d.apercu_taille;
        let pixels = assemblage.apercu(largeur, hauteur_max);
        self.apercu_voulu = false;
        self.dernier_apercu = Some(Instant::now());
        if pixels.largeur == 0 || pixels.hauteur == 0 {
            return;
        }
        self.numero_apercu += 1;
        let hauteur = pixels.hauteur;
        *self.d.apercu.lock().unwrap_or_else(|e| e.into_inner()) =
            Some((self.numero_apercu, Arc::new(pixels)));
        self.poster(Signal::Apercu {
            numero: self.d.numero,
            apercu: self.numero_apercu,
            hauteur,
        });
    }

    /// La fin (D14) : la fenêtre part tout de suite, puis l'image assemblée.
    fn finir(&mut self, fin: Fin) {
        let Some(assemblage) = self.assemblage.take() else {
            log::error!("defilante {}: fin deja rendue", self.d.numero);
            return;
        };
        let relache = Instant::now();
        self.poster(Signal::Finit {
            numero: self.d.numero,
        });
        log::info!(
            "{}",
            regles::ligne_fin(fin, self.images, self.gardees, self.perdues)
        );
        match assemblage.finir(&self.reference) {
            Ok(pixels) => {
                let duree = relache.elapsed();
                log::info!(
                    "{}",
                    regles::ligne_assemblee(pixels.largeur, pixels.hauteur, duree)
                );
                log::info!(
                    "{}",
                    regles::ligne_chrono(
                        self.relache_premiere,
                        regles::mediane(&self.durees_pas),
                        regles::mediane(&self.durees_mesure),
                        duree
                    )
                );
                self.poster(Signal::Assemblee {
                    numero: self.d.numero,
                    pixels: Arc::new(pixels),
                    relache,
                    duree,
                });
            }
            Err(e) => {
                log::warn!("defilante: assemblage impossible ({e})");
                self.poster(Signal::Impossible {
                    numero: self.d.numero,
                    cause: Impossible::Memoire,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bandeau_du_lancement_impossible() {
        let m = message_lancement_impossible();
        assert_eq!(m.titre, "Impossible de lancer la capture défilante");
        assert_eq!(m.genre, crate::fenetres::Genre::Erreur);
        assert!(m.detail.is_empty());
        assert!(m.actions.is_empty());
        assert_eq!(m.duree_ms, None);
    }

    #[test]
    fn charge_montrer_relative_a_l_ecran() {
        let ecran = EcranInfo {
            rect: Rect {
                x: -1920,
                y: 0,
                largeur: 1920,
                hauteur: 1080,
            },
            zone_utile: Rect {
                x: -1920,
                y: 0,
                largeur: 1920,
                hauteur: 1032,
            },
            echelle: 1.0,
            principal: false,
        };
        let zone = Rect {
            x: -1520,
            y: 200,
            largeur: 600,
            hauteur: 450,
        };
        let placement = regles::placer(zone, &ecran);
        let (envoi, _reception) = mpsc::channel();
        let l = Lancee {
            relache: Instant::now(),
            zone,
            ecran,
            placement,
            fenetre: None,
            garde: None,
            montree: false,
            abandonnee: false,
            montrer_envoye: false,
            envoi,
            reception: None,
            arret: Arc::new(AtomicBool::new(false)),
            apercu: Arc::new(Mutex::new(None)),
            entree: false,
            finie: false,
        };
        let c = charge_montrer(7, &l, true);
        assert_eq!(c.id, 7);
        assert_eq!(
            c.zone,
            RectPage {
                x: 400,
                y: 200,
                largeur: 600,
                hauteur: 450
            }
        );
        assert_eq!(c.apercu.x, 1012);
        assert_eq!(c.apercu.largeur, 84);
        assert_eq!(c.pilule, PointPage { x: 645, y: 660 });
        assert_eq!(c.libelle, "Terminer");
        assert!(c.reduites);
    }
}
