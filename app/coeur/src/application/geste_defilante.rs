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

//!
//! Le fil `defilante` sert à chaque tour d'abord les ordres reçus (Terminer, Entrée,
//! molette, arrêt), puis l'aperçu et la relecture des écrans, puis une seule action lente
//! (un pas avec sa page posée, ou une prise). Terminer et Entrée interrompent l'attente
//! de page posée. Il est écrit sur trois traits (`Source`, `Pilote`, `Atelier`) pour être
//! essayé sous Linux avec des faux.

use std::collections::VecDeque;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use super::defilante::{
    self as regles, Annulation, Conduite, Course, Etape, Fin, Geste, Impossible, Lancement,
    Placement, Raison, Vu,
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
use crate::systeme::defiler::{self, Envoi, Moyen, Position};
use crate::systeme::gel::{self, Changement, EcranInfo, Pixels};
use crate::systeme::{self, vivant, Rect};

/// Dernier numéro de capture défilante donné.
static NUMERO: AtomicU64 = AtomicU64::new(0);

/// Sans rien à faire, le fil `defilante` regarde le temps aussi souvent.
const TICK: Duration = Duration::from_millis(50);

/// Les écrans sont relus aussi souvent pendant le défilement (R9) : l'écran figé caché, la
/// surveillance de `gel` ne prévient plus.
const SURVEILLANCE_ECRANS: Duration = Duration::from_millis(500);

/// Essais de création de la fenêtre de contrôle : juste après la destruction de celle de
/// la capture précédente, Tauri peut refuser l'étiquette encore prise.
const CREATIONS_MAX: u32 = 3;
const ENTRE_CREATIONS: Duration = Duration::from_millis(150);

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
    /// L'image assemblée ; `relache` : le clic sur « Terminer », Entrée, ou la fin décidée.
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

/// Ce que le fil principal envoie au fil `defilante` (avec l'instant de l'ordre).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Commande {
    Molette(defiler::Molette),
    Terminer(Instant),
    Entree(Instant),
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
    /// Où en est le lancement (pur, `defilante::Lancement`).
    etat: Lancement,
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
            .field("etat", &self.etat)
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
    envoi: Sender<Commande>,
    /// Pris au départ du fil.
    reception: Option<Receiver<Commande>>,
    arret: Arc<AtomicBool>,
    apercu: PlaceApercu,
    /// Entrée est réservée.
    entree: bool,
    /// Un « Terminer » ou une Entrée est déjà parti au fil.
    fin_demandee: bool,
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

/// Vrai de la visée à la fin d'une capture défilante : le menu de l'icône et les raccourcis
/// qui ouvriraient une fenêtre de l'app (Réglages, Historique) doivent être ignorés, sauf
/// Quitter (une fenêtre ouverte sur la zone entrerait dans l'image, R2). À brancher dans
/// `mod.rs` (accroche du socle).
pub fn fenetres_bloquees(app: &AppHandle) -> bool {
    super::defilante_en_cours(app).is_some()
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
        etat: Lancement::default(),
        pages: Vec::new(),
        ecrans: ecrans.clone(),
        lancee: None,
    });
    if let Some(a) = ancienne {
        // Ne devrait pas arriver (une seule capture à la fois). L'Échap réservé est celui
        // de la nouvelle visée : il reste.
        log::warn!("defilante {}: ancienne capture retiree", a.numero);
        fermer(app, a, false);
    }
    // Jamais ici (rappel d'un autre geste) : un tour suivant de la boucle.
    let postee = super::plus_tard_nomme(app, "defilante-poste", Duration::ZERO, move |app| {
        creer_controle(app, numero, 1)
    });
    if !postee {
        creation_ratee(app, numero);
    }
    chercher_pages(app, numero, id, ecrans);
}

/// La création de la fenêtre de contrôle a échoué pour de bon.
fn creation_ratee(app: &AppHandle, numero: u64) {
    let etape = super::defilante_en_cours(app)
        .as_mut()
        .filter(|e| e.numero == numero)
        .map(|e| e.etat.creation_ratee());
    if etape == Some(Etape::Echouer) {
        rendre_impossible(app, numero, Impossible::Page);
    }
}

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
    let err = match cree {
        Ok(c) => {
            let c = Arc::new(c);
            let etape = {
                let mut g = super::defilante_en_cours(app);
                match g
                    .as_mut()
                    .filter(|e| e.numero == numero && e.controle.is_none())
                {
                    Some(e) => {
                        e.controle = Some(Arc::clone(&c));
                        Some(e.etat.controle_cree())
                    }
                    None => None,
                }
            };
            match etape {
                Some(Etape::Montrer) => envoyer_montrer(app, numero),
                Some(_) => {}
                // Abandonnée pendant la création : détruite tout de suite.
                None => detruire_controle(c),
            }
            return;
        }
        Err(e) => e,
    };
    log::warn!("defilante: fenetre non creee ({essai}): {err}");
    if essai >= CREATIONS_MAX {
        creation_ratee(app, numero);
        return;
    }
    // Une fenêtre `defilante` restée d'une capture précédente (destruction en retard) :
    // détruite, puis un nouvel essai.
    if let Some(vieille) = app.get_webview_window(controle::ETIQUETTE_DEFILANTE) {
        if let Err(e) = vieille.destroy() {
            log::warn!("defilante: ancienne fenetre non detruite: {e}");
        }
    }
    let postee = super::plus_tard_nomme(app, "defilante-poste", ENTRE_CREATIONS, move |app| {
        creer_controle(app, numero, essai + 1)
    });
    if !postee {
        creation_ratee(app, numero);
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
        .is_some_and(|e| e.numero == numero && !e.etat.lancee);
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
    let lancee = Lancee {
        relache,
        zone,
        ecran,
        placement,
        fenetre,
        garde: Some(garde),
        envoi,
        reception: Some(reception),
        arret: Arc::new(AtomicBool::new(false)),
        apercu: Arc::new(Mutex::new(None)),
        entree,
        fin_demandee: false,
    };
    let etape = {
        let mut g = super::defilante_en_cours(app);
        match g.as_mut().filter(|e| e.numero == numero) {
            Some(e) => {
                e.lancee = Some(lancee);
                Ok(e.etat.relache())
            }
            None => Err(lancee),
        }
    };
    match etape {
        Ok(Etape::Montrer) => envoyer_montrer(app, numero),
        Ok(Etape::Echouer) => {
            rendre_impossible(app, numero, Impossible::Page);
            return;
        }
        Ok(_) => {}
        Err(lancee) => {
            // Impossible sur ce fil (rien entre la lecture et ici) ; sûr quand même.
            if let Some(mut garde) = lancee.garde {
                garde.cacher_tout();
            }
            liberer_entree(app, entree);
            super::appliquer(app, Declencheur::DefilementImpossible);
            return;
        }
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
                    .and_then(|e| {
                        if e.etat.filet() {
                            e.lancee.as_mut()
                        } else {
                            None
                        }
                    })
                    .map(|l| l.garde.take())
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
        // Sans filet, l'écran figé ne doit pas risquer de rester : abandon tout de suite.
        let abandon = super::defilante_en_cours(app)
            .as_mut()
            .filter(|e| e.numero == numero)
            .is_some_and(|e| e.etat.filet());
        if abandon {
            rendre_impossible(app, numero, Impossible::Page);
        }
    }
}

/// Envoie `defilante:montrer` à la page (le lancement l'a voulu), la fenêtre posée sur
/// l'écran de la zone.
fn envoyer_montrer(app: &AppHandle, numero: u64) {
    let reduites = super::reduites_editeur(app);
    let envoi = {
        let g = super::defilante_en_cours(app);
        g.as_ref().filter(|e| e.numero == numero).and_then(|e| {
            let c = e.controle.clone()?;
            let l = e.lancee.as_ref()?;
            Some((c, l.ecran, charge_montrer(numero, l, reduites)))
        })
    };
    let Some((c, ecran, charge)) = envoi else {
        return;
    };
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
    let (numero, etape) = {
        let mut g = super::defilante_en_cours(app);
        let e = g.as_mut()?;
        if !e.etat.prete {
            log::info!("{}", regles::ligne_page_prete(e.creation.elapsed()));
        }
        (e.numero, e.etat.page_prete())
    };
    theme::envoyer_a(app, controle::ETIQUETTE_DEFILANTE);
    if etape == Etape::Montrer {
        envoyer_montrer(app, numero);
    }
    Some(reduites)
}

/// `defilante_zone` : la pilule est posée (pixels CSS de la page), chaque fois convertie
/// avec l'origine et l'échelle réelles de la fenêtre. La première fois, dans ce même tour :
/// la fenêtre de contrôle montrée, l'écran figé et la sélection cachés (D4), puis le fil
/// `defilante`.
pub(super) fn pilule_posee(app: &AppHandle, id: u64, x: f64, y: f64, largeur: f64, hauteur: f64) {
    let pris = {
        let mut g = super::defilante_en_cours(app);
        let Some(e) = g.as_mut().filter(|e| e.numero == id) else {
            return;
        };
        let etape = e.etat.pilule();
        let (Some(c), Some(l)) = (e.controle.clone(), e.lancee.as_mut()) else {
            return;
        };
        // `Partir` n'arrive qu'une fois, la réception est encore là (sinon le filet ne
        // jouerait plus).
        debug_assert!(etape != Etape::Partir || l.reception.is_some());
        let depart = match etape {
            Etape::Partir => l.reception.take().map(|reception| {
                (
                    l.garde.take(),
                    Depart {
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
            }),
            Etape::Placer => None,
            _ => return,
        };
        (c, l.ecran, depart)
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
        let compte = super::defilante_en_cours(app)
            .as_mut()
            .filter(|e| e.numero == id)
            .is_some_and(|e| e.etat.echouer());
        if compte {
            rendre_impossible(app, id, Impossible::Page);
        }
        return;
    }
    lancer_fil(app, depart);
}

/// `defilante_terminer` : fin `bouton` ; un second « Terminer » est ignoré.
pub(super) fn terminer(app: &AppHandle, id: u64) {
    envoyer_fin(app, Some(id), Commande::Terminer(Instant::now()));
}

/// Envoie un ordre de fin au fil de la défilante en cours, une fois par capture.
fn envoyer_fin(app: &AppHandle, numero: Option<u64>, commande: Commande) {
    let envoi = {
        let mut g = super::defilante_en_cours(app);
        g.as_mut()
            .filter(|e| numero.is_none_or(|n| n == e.numero) && e.etat.ouverte())
            .and_then(|e| e.lancee.as_mut())
            .filter(|l| !l.fin_demandee)
            .map(|l| {
                l.fin_demandee = true;
                l.envoi.clone()
            })
    };
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
        fermer(app, e, true);
    }
}

/// Tout ce que tient `e` est rendu : le fil arrêté, l'écran figé caché s'il l'était encore,
/// Entrée libérée (et Échap si `echap`), la fenêtre de contrôle détruite. Hors du verrou.
fn fermer(app: &AppHandle, e: EnCours, echap: bool) {
    let EnCours {
        controle, lancee, ..
    } = e;
    if let Some(l) = lancee {
        l.arret.store(true, Ordering::SeqCst);
        let _ = l.envoi.send(Commande::Arret);
        if let Some(mut garde) = l.garde {
            if echap {
                garde.cacher_tout();
            } else {
                gel::cacher_ecran_fige();
                if let Some(s) = app.try_state::<Selection>() {
                    s.cacher();
                }
                garde.fait = true;
            }
        }
        liberer_entree(app, l.entree);
    }
    if let Some(c) = controle {
        detruire_controle(c);
    }
    if echap {
        super::liberer_echap(app);
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

/// Annule la défilante lancée (D18) : rien n'est gardé, aucun message. Faux s'il n'y en a
/// pas (visée, ou fin déjà décidée).
fn annuler(app: &AppHandle, numero: Option<u64>, cause: Annulation) -> bool {
    let prise = super::defilante_en_cours(app)
        .take_if(|e| numero.is_none_or(|n| n == e.numero) && e.etat.lancee && !e.etat.finie);
    let Some(e) = prise else {
        return false;
    };
    fermer(app, e, true);
    super::appliquer(app, Declencheur::DefilementAnnule);
    log::info!("{}", regles::ligne_annulee(cause));
    true
}

/// La défilante finit sans image (D19) : Repos, bandeau d'erreur.
fn rendre_impossible(app: &AppHandle, numero: u64, cause: Impossible) {
    let Some(e) = prendre(app, numero) else {
        return;
    };
    let lancee = e.etat.lancee;
    fermer(app, e, true);
    if !lancee {
        return;
    }
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

/// Un signal de la Sélection : vrai s'il est pris ici. En Défilement, l'affichage gardé
/// (zone choisie) ne décide plus rien : ses signaux sont ignorés.
pub fn signal_selection(app: &AppHandle, signal: &selection::Signal) -> bool {
    super::defilante_en_cours(app)
        .as_ref()
        .is_some_and(|e| e.etat.lancee && e.selection == signal.id())
}

/// Échap global : vrai s'il est pris ici (en Défilement : annulée).
pub fn echap(app: &AppHandle) -> bool {
    annuler(app, None, Annulation::Echap)
}

/// Entrée globale (= « Terminer »).
pub fn entree(app: &AppHandle) {
    envoyer_fin(app, None, Commande::Entree(Instant::now()));
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
        Signal::PageEnRetard { numero } => rendre_impossible(app, numero, Impossible::Page),
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
        .filter(|e| e.numero == numero && e.etat.ouverte())
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
        if !e.etat.finir() {
            return;
        }
        let entree = e
            .lancee
            .as_mut()
            .is_some_and(|l| std::mem::replace(&mut l.entree, false));
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
    fermer(app, e, true);
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
        fermer(app, e, true);
    }
}

/// Ce que le fil `defilante` reçoit au départ.
struct Depart {
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

/// Lance le fil `defilante`. Une panique dans le fil finit la capture (`fil`) : jamais un
/// état Défilement sans fin (D19).
fn lancer_fil(app: &AppHandle, depart: Depart) {
    let numero = depart.numero;
    let pour = app.clone();
    let lance = std::thread::Builder::new()
        .name("defilante".into())
        .spawn(move || {
            let pour_fil = pour.clone();
            let issue = panic::catch_unwind(AssertUnwindSafe(move || {
                defiler_et_assembler(depart, move |s| poster(&pour_fil, s))
            }));
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

/// D'où viennent les images (`vivant::Prise` ; un faux dans les essais).
trait Source {
    fn prendre(&mut self, dest: &mut Pixels) -> Result<Duration, String>;
}

impl Source for vivant::Prise {
    fn prendre(&mut self, dest: &mut Pixels) -> Result<Duration, String> {
        vivant::Prise::prendre(self, dest)
    }
}

/// Ce qui fait défiler (`defiler::Defileur` ; un faux dans les essais).
trait Pilote {
    fn moyen(&self) -> Option<Moyen>;
    fn abandonner(&mut self) -> Option<Moyen>;
    fn descendre(&mut self, pixels: i32) -> Result<Envoi, String>;
    fn position(&self) -> Option<Position>;
    /// L'avance vue après un pas parti, 0 compris (`Defileur::constater`, B).
    fn constater(&mut self, avance: u32);
    /// Ce qu'un cran du moyen en cours fait au plus (`Defileur::avance_par_cran`, B).
    fn avance_par_cran(&self) -> Option<u32>;
}

impl Pilote for defiler::Defileur {
    fn moyen(&self) -> Option<Moyen> {
        defiler::Defileur::moyen(self)
    }
    fn abandonner(&mut self) -> Option<Moyen> {
        defiler::Defileur::abandonner(self)
    }
    fn descendre(&mut self, pixels: i32) -> Result<Envoi, String> {
        defiler::Defileur::descendre(self, pixels)
    }
    fn position(&self) -> Option<Position> {
        defiler::Defileur::position(self)
    }
    // À l'assemblage avec `0318bde` (B) : `defiler::Defileur::constater(self, avance)`.
    fn constater(&mut self, _avance: u32) {}
    // À l'assemblage avec `0318bde` (B) : `defiler::Defileur::avance_par_cran(self)`.
    fn avance_par_cran(&self) -> Option<u32> {
        None
    }
}

/// Mesure et assemblage (`defilement` ; un faux dans les essais).
trait Atelier {
    fn fixes(&self) -> Fixes;
    fn mesurer(&self, avant: &Pixels, apres: &Pixels, fixes: Fixes, attendu: Option<i32>)
        -> Mesure;
    fn ajouter(&mut self, image: &Pixels, mesure: &Mesure) -> Ajout;
    fn apercu(&self, largeur: u32, hauteur_max: u32) -> Pixels;
    /// Une seule fois.
    fn finir(&mut self, derniere: &Pixels) -> Result<Pixels, String>;
}

/// L'assemblage de `defilement`.
struct VraiAtelier(Option<Assemblage>);

impl Atelier for VraiAtelier {
    fn fixes(&self) -> Fixes {
        self.0.as_ref().map(|a| a.fixes()).unwrap_or_default()
    }
    fn mesurer(
        &self,
        avant: &Pixels,
        apres: &Pixels,
        fixes: Fixes,
        attendu: Option<i32>,
    ) -> Mesure {
        defilement::mesurer(avant, apres, fixes, attendu, &TOLERANCE)
    }
    fn ajouter(&mut self, image: &Pixels, mesure: &Mesure) -> Ajout {
        match self.0.as_mut() {
            Some(a) => a.ajouter(image, mesure),
            None => Ajout::Rien,
        }
    }
    fn apercu(&self, largeur: u32, hauteur_max: u32) -> Pixels {
        match self.0.as_ref() {
            Some(a) => a.apercu(largeur, hauteur_max),
            None => vide(),
        }
    }
    fn finir(&mut self, derniere: &Pixels) -> Result<Pixels, String> {
        match self.0.take() {
            Some(a) => a.finir(derniere),
            None => Err("deja fini".into()),
        }
    }
}

fn vide() -> Pixels {
    Pixels {
        largeur: 0,
        hauteur: 0,
        rgba: Vec::new(),
    }
}

/// Le corps du fil `defilante` : compositeur, prise, première image, assemblage,
/// défileur, écoute de la molette, puis la course (`Fil::courir`). Rend toujours un
/// signal par `sortie`, sauf à l'arrêt demandé.
fn defiler_et_assembler(d: Depart, sortie: impl FnMut(Signal) + Send + 'static) {
    let mut sortie = Sortie {
        arret: Arc::clone(&d.arret),
        poster: Box::new(sortie),
    };
    let numero = d.numero;
    // L'écran figé vient d'être caché : la dernière image composée doit être la vraie page.
    if !systeme::attendre_composition(super::ATTENTE_COMPOSITION) {
        log::warn!("defilante: composition non attendue");
    }
    let mut prise = match vivant::ouvrir(d.zone) {
        // Une zone coupée au bord du bureau ne serait plus celle placée et montrée.
        Ok(p) if p.zone() != d.zone => {
            log::warn!("defilante: prise en echec (zone coupee)");
            sortie.envoyer(Signal::Impossible {
                numero,
                cause: Impossible::Prise,
            });
            return;
        }
        Ok(p) => p,
        Err(e) => {
            log::warn!("defilante: prise en echec ({e})");
            sortie.envoyer(Signal::Impossible {
                numero,
                cause: Impossible::Prise,
            });
            return;
        }
    };
    let mut image = vide();
    let mut premiere = None;
    for essai in 0..regles::PRISES_EN_ECHEC_MAX {
        if d.arret.load(Ordering::SeqCst) {
            return;
        }
        match Source::prendre(&mut prise, &mut image) {
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
        sortie.envoyer(Signal::Impossible {
            numero,
            cause: Impossible::Prise,
        });
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
            sortie.envoyer(Signal::Impossible {
                numero,
                cause: Impossible::Memoire,
            });
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
    let pour_molette = d.envoi.clone();
    // Gardée toute la capture ; son crochet tombe à la fin du fil (jamais sur le fil
    // principal : sa destruction peut attendre).
    let _ecoute = match defiler::ecouter_molette(move |m| {
        let _ = pour_molette.send(Commande::Molette(m));
    }) {
        Ok(e) => Some(e),
        Err(e) => {
            log::warn!("defilante: crochet de la molette impossible ({e})");
            None
        }
    };
    let mut fil = Fil::nouveau(
        d,
        sortie,
        prise,
        defileur,
        VraiAtelier(Some(assemblage)),
        image,
        relache_premiere,
        gel::ecrans,
    );
    fil.courir();
}

/// Où le fil poste : rien une fois l'arrêt demandé.
struct Sortie {
    arret: Arc<AtomicBool>,
    poster: Box<dyn FnMut(Signal) + Send>,
}

impl Sortie {
    fn envoyer(&mut self, signal: Signal) {
        if !self.arret.load(Ordering::SeqCst) {
            (self.poster)(signal);
        }
    }
}

/// Comment finit une attente de page posée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Posee {
    /// Une image est prise (dans `image`).
    Prise,
    /// Aucune image n'a pu être prise.
    Aucune,
    /// Terminer, Entrée ou l'arrêt sont arrivés : rien n'est mesuré.
    Interrompue,
}

/// L'état du fil `defilante`.
struct Fil<S: Source, P: Pilote, A: Atelier> {
    numero: u64,
    zone: Rect,
    ecrans: Vec<EcranInfo>,
    lire_ecrans: fn() -> Vec<EcranInfo>,
    apercu: PlaceApercu,
    apercu_taille: (u32, u32),
    reception: Receiver<Commande>,
    arret: Arc<AtomicBool>,
    sortie: Sortie,
    source: S,
    pilote: Option<P>,
    atelier: A,
    fini: bool,
    /// La dernière image gardée : chaque image est mesurée contre elle (D10).
    reference: Pixels,
    /// Tampon de prise.
    image: Pixels,
    /// L'image d'avant, pendant qu'on attend la page posée (D6).
    precedente: Pixels,
    course: Course,
    /// Ordres reçus pendant une attente, servis au tour suivant.
    commandes: VecDeque<Commande>,
    /// Une image mesurée, à montrer à la course au tour suivant.
    vu_en_attente: Option<Vu>,
    /// Le pas demandé par la course, fait au tour suivant.
    pas_demande: Option<i32>,
    /// Un pas retenu par la garde de la molette (D7) : pas avant ce moment.
    pas_apres: Option<Instant>,
    /// La prochaine image demandée (`Geste::Prendre`).
    prochaine: Option<Instant>,
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
    /// La position lue à la dernière image gardée.
    position_gardee: Option<Position>,
    /// Les pixels commandés depuis la dernière image gardée.
    commande_depuis_gardee: i32,
    /// Le décalage observé (positions lues) au dernier pas, depuis la dernière image
    /// gardée : il vaut encore pour une image reprise sans pas entre-temps.
    observe: Option<i32>,
    /// L'instant de l'ordre de fin (clic, Entrée).
    fin_recue: Option<Instant>,
}

impl<S: Source, P: Pilote, A: Atelier> Fil<S, P, A> {
    #[allow(clippy::too_many_arguments)]
    fn nouveau(
        d: Depart,
        sortie: Sortie,
        source: S,
        pilote: Option<P>,
        atelier: A,
        premiere: Pixels,
        relache_premiere: Duration,
        lire_ecrans: fn() -> Vec<EcranInfo>,
    ) -> Fil<S, P, A> {
        let maintenant = Instant::now();
        let mut course = Course::nouvelle(maintenant);
        course.regler_pas(regles::hauteur_utile(premiere.hauteur, Fixes::default()));
        let moyen = pilote.as_ref().and_then(|p| p.moyen());
        log::info!("{}", regles::ligne_moyen(moyen));
        let mut prochaine = None;
        if moyen.is_none() {
            log::info!("{}", regles::ligne_repli(None, None, Raison::SansEffet));
            // Aucun moyen : la molette de l'utilisateur, et des images (D8).
            for g in course.sans_moyen(maintenant) {
                if let Geste::Prendre(delai) = g {
                    prochaine = Some(maintenant + delai);
                }
            }
        }
        let position_gardee = pilote.as_ref().and_then(|p| p.position());
        Fil {
            numero: d.numero,
            zone: d.zone,
            ecrans: d.ecrans,
            lire_ecrans,
            apercu: d.apercu,
            apercu_taille: d.apercu_taille,
            reception: d.reception,
            arret: d.arret,
            sortie,
            source,
            pilote,
            atelier,
            fini: false,
            reference: premiere,
            image: vide(),
            precedente: vide(),
            course,
            commandes: VecDeque::new(),
            vu_en_attente: None,
            pas_demande: None,
            pas_apres: None,
            prochaine,
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
            position_gardee,
            commande_depuis_gardee: 0,
            observe: None,
            fin_recue: None,
        }
    }

    fn arrete(&self) -> bool {
        self.arret.load(Ordering::SeqCst)
    }

    /// La boucle du fil. Chaque tour : les ordres reçus, la fin, l'aperçu, les écrans,
    /// puis une seule action (une image à montrer, un pas, une prise), sinon l'attente du
    /// prochain ordre ou de la prochaine échéance.
    fn courir(&mut self) {
        loop {
            if self.arrete() || self.servir_ordres() {
                return;
            }
            if self.echecs >= regles::PRISES_EN_ECHEC_MAX && self.course.fin().is_none() {
                // Des images sont déjà gardées : la capture finit avec ce qui est pris (D19).
                log::warn!("defilante: prises en echec, fin avec ce qui est pris");
                self.course.finir(Fin::Limite);
            }
            if let Some(fin) = self.course.fin() {
                self.finir(fin);
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
            if maintenant.saturating_duration_since(self.surveillance) >= SURVEILLANCE_ECRANS {
                self.surveillance = maintenant;
                let ecrans = (self.lire_ecrans)();
                // Une liste vide est une lecture ratée, pas un bureau sans écran.
                if !ecrans.is_empty() && regles::ecrans_changes(&self.ecrans, &ecrans) {
                    self.sortie.envoyer(Signal::Annulee {
                        numero: self.numero,
                        cause: Annulation::Ecrans,
                    });
                    return;
                }
            }
            if let Some(vu) = self.vu_en_attente.take() {
                self.voir(vu);
                continue;
            }
            if let Some(pas) = self.pas_demande {
                if self.pas_apres.is_none_or(|t| maintenant >= t) {
                    self.pas_demande = None;
                    self.pas_apres = None;
                    self.descendre(pas);
                    continue;
                }
            }
            if self.prochaine.is_some_and(|t| maintenant >= t) {
                self.prochaine = None;
                self.prendre_une();
                continue;
            }
            self.voir(Vu::Temps(maintenant));
            if self.course.fin().is_some()
                || self.vu_en_attente.is_some()
                || (self.pas_demande.is_some() && self.pas_apres.is_none())
            {
                continue;
            }
            let echeance = [
                self.prochaine,
                self.pas_demande.and(self.pas_apres),
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
            match self
                .reception
                .recv_timeout(echeance.saturating_duration_since(Instant::now()))
            {
                Ok(c) => self.commandes.push_back(c),
                Err(RecvTimeoutError::Timeout) => {}
                // Plus personne pour envoyer : l'app s'arrête.
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    /// Sert tous les ordres reçus, sans attendre. Vrai à l'arrêt.
    fn servir_ordres(&mut self) -> bool {
        while let Ok(c) = self.reception.try_recv() {
            self.commandes.push_back(c);
        }
        while let Some(c) = self.commandes.pop_front() {
            let vu = match c {
                Commande::Arret => return true,
                Commande::Molette(m) => Vu::Molette {
                    dans_zone: self.zone.contient(m.point),
                },
                Commande::Terminer(a) => {
                    self.fin_recue.get_or_insert(a);
                    Vu::Terminer
                }
                Commande::Entree(a) => {
                    self.fin_recue.get_or_insert(a);
                    Vu::Entree
                }
            };
            self.voir(vu);
        }
        self.arrete()
    }

    /// Les ordres arrivés pendant une attente, gardés pour le tour suivant ; vrai si l'un
    /// d'eux doit l'interrompre (Terminer, Entrée, arrêt).
    fn relever(&mut self) -> bool {
        while let Ok(c) = self.reception.try_recv() {
            self.commandes.push_back(c);
        }
        self.arrete()
            || self.commandes.iter().any(|c| {
                matches!(
                    c,
                    Commande::Arret | Commande::Terminer(_) | Commande::Entree(_)
                )
            })
    }

    /// Plus aucun moyen : la molette de l'utilisateur, et des images (D8).
    fn sans_moyen(&mut self) {
        self.pas_demande = None;
        self.pas_apres = None;
        let maintenant = Instant::now();
        for g in self.course.sans_moyen(maintenant) {
            if let Geste::Prendre(delai) = g {
                self.demander_image(maintenant + delai);
            }
        }
    }

    fn demander_image(&mut self, au_plus_tard: Instant) {
        self.prochaine = Some(self.prochaine.map_or(au_plus_tard, |t| t.min(au_plus_tard)));
    }

    /// Montre `vu` à la course et fait ses gestes (sans rien de lent : un pas demandé est
    /// fait au tour suivant).
    fn voir(&mut self, vu: Vu) {
        let maintenant = Instant::now();
        let avant = self.course.conduite();
        let gestes = regles::suite(&mut self.course, vu, maintenant);
        if avant != Conduite::Main && self.course.conduite() == Conduite::Main {
            self.pas_demande = None;
            self.pas_apres = None;
            if matches!(vu, Vu::Molette { .. }) {
                log::info!("{}", regles::LIGNE_MAIN);
            } else {
                // La page saute trop loin même à petit pas : la molette, sans rien dire.
                log::info!("{}", regles::ligne_repli(None, None, Raison::SansEffet));
            }
        }
        for g in gestes {
            if self.arrete() {
                return;
            }
            match g {
                Geste::Descendre(pas) => {
                    if self.course.conduite() == Conduite::Auto {
                        self.pas_demande = Some(pas);
                    }
                }
                Geste::Prendre(delai) => self.demander_image(maintenant + delai),
                Geste::Message(texte) => {
                    log::info!(
                        "{}",
                        if texte.is_some() {
                            regles::LIGNE_TROP_VITE
                        } else {
                            regles::LIGNE_REPRIS
                        }
                    );
                    self.sortie.envoyer(Signal::Message {
                        numero: self.numero,
                        texte,
                    });
                }
                Geste::Apercu => self.apercu_voulu = true,
                Geste::Repli(raison) => {
                    let (ancien, nouveau) = match self.pilote.as_mut() {
                        Some(p) => {
                            let ancien = p.moyen();
                            (ancien, p.abandonner())
                        }
                        None => (None, None),
                    };
                    log::info!("{}", regles::ligne_repli(ancien, nouveau, raison));
                    if nouveau.is_none() {
                        self.sans_moyen();
                    }
                }
                Geste::Finir(_) => {}
            }
        }
    }

    /// Un pas, puis les images jusqu'à la page posée (D6), puis la dernière, mesurée et
    /// rangée pour le tour suivant.
    fn descendre(&mut self, pas: i32) {
        let Some(p) = self.pilote.as_mut() else {
            self.sans_moyen();
            return;
        };
        let avant = p.moyen();
        let cran = p.avance_par_cran();
        let utile = regles::hauteur_utile(self.reference.hauteur, self.fixes);
        if regles::cran_trop_grand(avant, cran, utile) {
            // Chaque pas de ce moyen perdrait des lignes : le suivant (B), sans pas.
            let nouveau = p.abandonner();
            log::info!(
                "{}",
                regles::ligne_repli(avant, nouveau, Raison::CranTropGrand)
            );
            self.course.moyen_change();
            if nouveau.is_none() {
                self.sans_moyen();
            } else {
                self.pas_demande = Some(pas);
            }
            return;
        }
        let envoi = p.descendre(pas);
        let apres = p.moyen();
        if apres != avant {
            // Le défileur est passé de lui-même à un autre moyen.
            log::info!("{}", regles::ligne_repli(avant, apres, Raison::Impossible));
            self.course.moyen_change();
        }
        match envoi {
            Ok(Envoi::Saute) if apres.is_none() => {
                if avant.is_some() {
                    // Déjà écrit ci-dessus.
                } else {
                    log::info!("{}", regles::ligne_repli(None, None, Raison::Impossible));
                }
                self.sans_moyen();
                return;
            }
            Ok(Envoi::Saute) => {
                // Retenu par B (touche ou bouton tenu, fenêtre devant la zone, menu,
                // recherche d'UI Automation en cours…) : retenté au tour suivant, jamais
                // compté comme un pas sans effet du moyen.
                self.pas_demande = Some(pas);
                self.pas_apres = Some(Instant::now() + regles::PRISE_POSEE);
                return;
            }
            Ok(Envoi::Envoye) => {}
            // Compté comme sans effet : l'image le dira.
            Err(ref e) => log::warn!("defilante: pas refuse ({e})"),
        }
        if apres.is_none() {
            self.sans_moyen();
        }
        let parti = matches!(envoi, Ok(Envoi::Envoye));
        // L'image d'avant ce pas est la dernière gardée : la mesure dira l'avance du pas.
        let depuis_gardee = self.commande_depuis_gardee == 0;
        let commande = regles::avance_commandee(pas, cran, avant == Some(Moyen::Message));
        self.commande_depuis_gardee = self.commande_depuis_gardee.saturating_add(commande);
        let debut = Instant::now();
        let posee = self.attendre_posee();
        self.durees_pas.push(debut.elapsed());
        match posee {
            Posee::Interrompue => return,
            Posee::Aucune => {
                self.demander_image(Instant::now() + regles::PRISE_POSEE);
                return;
            }
            Posee::Prise => {}
        }
        if self.relever() {
            return;
        }
        // Une seule lecture de la position par pas (elle peut être lente).
        let position = self.pilote.as_ref().and_then(|p| p.position());
        let utile = regles::hauteur_utile(self.reference.hauteur, self.fixes);
        self.observe = regles::decalage_attendu(self.position_gardee, position, utile);
        let attendu = regles::attendu(
            self.position_gardee,
            position,
            utile,
            self.commande_depuis_gardee,
        );
        let mesure = self.mesurer(attendu, position);
        if parti && pas > 0 && depuis_gardee {
            if let (Some(avance), Some(p)) =
                (regles::avance_constatee(mesure), self.pilote.as_mut())
            {
                p.constater(avance);
            }
        }
        self.vu_en_attente = Some(Vu::Image(mesure, position));
    }

    /// Images toutes les `PRISE_POSEE` jusqu'à deux égales de suite ou
    /// `ATTENTE_POSEE_MAX` ; la dernière reste dans `image`. Terminer, Entrée et l'arrêt
    /// l'interrompent.
    fn attendre_posee(&mut self) -> Posee {
        let debut = Instant::now();
        let mut prise = false;
        loop {
            std::thread::sleep(regles::PRISE_POSEE);
            if self.relever() {
                return Posee::Interrompue;
            }
            let ecoule = debut.elapsed() >= regles::ATTENTE_POSEE_MAX;
            if !self.prendre() {
                if prise {
                    // Le tampon a pu être touché : la dernière bonne image y revient.
                    self.image.clone_from(&self.precedente);
                }
                if ecoule {
                    return if prise { Posee::Prise } else { Posee::Aucune };
                }
                continue;
            }
            if prise {
                let m = self
                    .atelier
                    .mesurer(&self.precedente, &self.image, self.fixes, None);
                if m == Mesure::Immobile {
                    return Posee::Prise;
                }
            }
            prise = true;
            if ecoule {
                return Posee::Prise;
            }
            self.precedente.clone_from(&self.image);
        }
    }

    /// Une image dans `image` ; vrai si elle est prise. Les échecs sont comptés (D19).
    fn prendre(&mut self) -> bool {
        match self.source.prendre(&mut self.image) {
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

    /// Une image demandée (`Geste::Prendre`), mesurée et rangée pour le tour suivant. La
    /// position n'est lue que si elle sert (image immobile ou ambiguë : « en bas »).
    fn prendre_une(&mut self) {
        if !self.prendre() {
            self.demander_image(Instant::now() + regles::PRISE_POSEE);
            return;
        }
        let attendu = match self.course.conduite() {
            Conduite::Main => None,
            _ => self
                .observe
                .or((self.commande_depuis_gardee != 0).then_some(self.commande_depuis_gardee)),
        };
        let mesure = self.mesurer(attendu, None);
        let position = match mesure {
            Mesure::Immobile | Mesure::Ambigue => self.pilote.as_ref().and_then(|p| p.position()),
            _ => None,
        };
        self.vu_en_attente = Some(Vu::Image(mesure, position));
    }

    /// Mesure `image` contre la dernière image gardée et l'ajoute (D10 à D13).
    fn mesurer(&mut self, attendu: Option<i32>, position: Option<Position>) -> Mesure {
        if self.fini {
            return Mesure::Immobile;
        }
        let debut = Instant::now();
        let fixes = self.atelier.fixes();
        let mesure = self
            .atelier
            .mesurer(&self.reference, &self.image, fixes, attendu);
        self.durees_mesure.push(debut.elapsed());
        self.images += 1;
        // La dernière image gardée est celle dont des lignes sont entrées (D10).
        let gardee = match self.atelier.ajouter(&self.image, &mesure) {
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
            self.position_gardee = position;
            self.commande_depuis_gardee = 0;
            self.observe = None;
        }
        if mesure == Mesure::Perdue {
            self.perdues += 1;
        }
        let fixes = self.atelier.fixes();
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
        self.apercu_voulu = false;
        self.dernier_apercu = Some(Instant::now());
        if self.fini {
            return;
        }
        let (largeur, hauteur_max) = self.apercu_taille;
        let pixels = self.atelier.apercu(largeur, hauteur_max);
        if pixels.largeur == 0 || pixels.hauteur == 0 {
            return;
        }
        self.numero_apercu += 1;
        let hauteur = pixels.hauteur;
        *self.apercu.lock().unwrap_or_else(|e| e.into_inner()) =
            Some((self.numero_apercu, Arc::new(pixels)));
        self.sortie.envoyer(Signal::Apercu {
            numero: self.numero,
            apercu: self.numero_apercu,
            hauteur,
        });
    }

    /// La fin (D14) : la fenêtre part tout de suite, puis l'image assemblée.
    fn finir(&mut self, fin: Fin) {
        if std::mem::replace(&mut self.fini, true) {
            log::error!("defilante {}: fin deja rendue", self.numero);
            return;
        }
        let decidee = Instant::now();
        let relache = self.fin_recue.unwrap_or(decidee);
        self.sortie.envoyer(Signal::Finit {
            numero: self.numero,
        });
        log::info!(
            "{}",
            regles::ligne_fin(fin, self.images, self.gardees, self.perdues)
        );
        match self.atelier.finir(&self.reference) {
            Ok(pixels) => {
                let duree = decidee.elapsed();
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
                        relache.elapsed()
                    )
                );
                self.sortie.envoyer(Signal::Assemblee {
                    numero: self.numero,
                    pixels: Arc::new(pixels),
                    relache,
                    duree,
                });
            }
            Err(e) => {
                log::warn!("defilante: assemblage impossible ({e})");
                self.sortie.envoyer(Signal::Impossible {
                    numero: self.numero,
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
            envoi,
            reception: None,
            arret: Arc::new(AtomicBool::new(false)),
            apercu: Arc::new(Mutex::new(None)),
            entree: false,
            fin_demandee: false,
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

    // --- Le fil `defilante` avec des faux ------------------------------------------------
    //
    // Une page de `bas` pixels de parcours ; chaque image est un pixel qui porte la
    // position de la page ; l'atelier mesure la différence des positions.

    const ZONE: Rect = Rect {
        x: 0,
        y: 0,
        largeur: 100,
        hauteur: 100,
    };
    /// Au-delà, l'atelier dit « perdue ».
    const RECOUVREMENT: i32 = 90;
    const ATTENTE: Duration = Duration::from_secs(10);

    #[derive(Default)]
    struct Page {
        position: i32,
        bas: i32,
        /// Prises à rater.
        echecs: u32,
        pas: u32,
        abandons: u32,
        prises: u32,
        /// La page bouge d'elle-même à chaque image (jamais posée).
        animee: bool,
    }

    type Partagee = Arc<Mutex<Page>>;

    fn page(bas: i32) -> Partagee {
        Arc::new(Mutex::new(Page {
            bas,
            ..Page::default()
        }))
    }

    fn image(position: i32) -> Pixels {
        Pixels {
            largeur: 1,
            hauteur: 1,
            rgba: position.to_le_bytes().to_vec(),
        }
    }

    fn position_de(p: &Pixels) -> i32 {
        i32::from_le_bytes(p.rgba[..4].try_into().unwrap())
    }

    struct FausseSource(Partagee);

    impl Source for FausseSource {
        fn prendre(&mut self, dest: &mut Pixels) -> Result<Duration, String> {
            let mut p = self.0.lock().unwrap();
            p.prises += 1;
            if p.echecs > 0 {
                p.echecs -= 1;
                return Err("faux echec".into());
            }
            if p.animee {
                p.position += 1;
            }
            *dest = image(p.position);
            Ok(Duration::from_millis(1))
        }
    }

    struct FauxPilote {
        page: Partagee,
        moyens: Vec<Moyen>,
        /// Un pas fait défiler la page.
        effet: bool,
        /// Le premier pas retire le dernier moyen et rend `Saute`.
        perd_au_pas: bool,
        /// Pas retenus (`Saute`) avant de partir, comme une recherche d'UI Automation.
        sautes: u32,
        cran: Option<u32>,
        constats: Arc<Mutex<Vec<u32>>>,
        /// La part visible de la page (`Position::vue`) ; `None` : position illisible.
        vue: Option<f64>,
    }

    impl Pilote for FauxPilote {
        fn moyen(&self) -> Option<Moyen> {
            self.moyens.first().copied()
        }
        fn abandonner(&mut self) -> Option<Moyen> {
            self.page.lock().unwrap().abandons += 1;
            if !self.moyens.is_empty() {
                self.moyens.remove(0);
            }
            self.moyen()
        }
        fn descendre(&mut self, pixels: i32) -> Result<Envoi, String> {
            let mut p = self.page.lock().unwrap();
            p.pas += 1;
            if self.perd_au_pas {
                self.moyens.clear();
                return Ok(Envoi::Saute);
            }
            if self.sautes > 0 {
                self.sautes -= 1;
                return Ok(Envoi::Saute);
            }
            if self.effet {
                p.position = (p.position + pixels).clamp(0, p.bas);
            }
            Ok(Envoi::Envoye)
        }
        fn constater(&mut self, avance: u32) {
            self.constats.lock().unwrap().push(avance);
        }
        fn avance_par_cran(&self) -> Option<u32> {
            self.cran
        }
        fn position(&self) -> Option<Position> {
            let vue = self.vue?;
            let p = self.page.lock().unwrap();
            Some(Position {
                pour_cent: f64::from(p.position) * 100.0 / f64::from(p.bas.max(1)),
                vue,
            })
        }
    }

    struct FauxAtelier {
        lignes: u32,
        limite: u32,
        attendus: Arc<Mutex<Vec<i32>>>,
    }

    impl Atelier for FauxAtelier {
        fn fixes(&self) -> Fixes {
            Fixes::default()
        }
        fn mesurer(
            &self,
            avant: &Pixels,
            apres: &Pixels,
            _: Fixes,
            attendu: Option<i32>,
        ) -> Mesure {
            // L'attente de page posée mesure sans attendu : seuls ceux transmis comptent.
            if let Some(a) = attendu {
                self.attendus.lock().unwrap().push(a);
            }
            let d = position_de(apres) - position_de(avant);
            match d {
                0 => Mesure::Immobile,
                d if d < 0 => Mesure::Recul(d.unsigned_abs()),
                d if d > RECOUVREMENT => Mesure::Perdue,
                d => Mesure::Avance(d as u32),
            }
        }
        fn ajouter(&mut self, _: &Pixels, mesure: &Mesure) -> Ajout {
            let Mesure::Avance(n) = *mesure else {
                return Ajout::Rien;
            };
            let n = n.min(self.limite - self.lignes);
            self.lignes += n;
            if self.lignes >= self.limite {
                Ajout::Limite(n)
            } else {
                Ajout::Ajoutees(n)
            }
        }
        fn apercu(&self, _: u32, _: u32) -> Pixels {
            image(0)
        }
        fn finir(&mut self, _: &Pixels) -> Result<Pixels, String> {
            Ok(Pixels {
                largeur: 1,
                hauteur: 1 + self.lignes,
                rgba: Vec::new(),
            })
        }
    }

    struct Essai {
        page: Partagee,
        envoi: Sender<Commande>,
        signaux: Receiver<Signal>,
        arret: Arc<AtomicBool>,
        fil: std::thread::JoinHandle<()>,
        /// Les attendus reçus par l'atelier.
        attendus: Arc<Mutex<Vec<i32>>>,
    }

    fn lancer(page: Partagee, pilote: Option<FauxPilote>, limite: u32) -> Essai {
        let (envoi, reception) = mpsc::channel();
        let (vers_essai, signaux) = mpsc::channel();
        let arret = Arc::new(AtomicBool::new(false));
        let premiere = image(page.lock().unwrap().position);
        let d = Depart {
            numero: 1,
            zone: ZONE,
            fenetre: None,
            relache: Instant::now(),
            ecrans: Vec::new(),
            apercu: Arc::new(Mutex::new(None)),
            apercu_taille: (84, 200),
            reception,
            envoi: envoi.clone(),
            arret: Arc::clone(&arret),
        };
        let sortie = Sortie {
            arret: Arc::clone(&arret),
            poster: Box::new(move |s| {
                let _ = vers_essai.send(s);
            }),
        };
        let source = FausseSource(Arc::clone(&page));
        let attendus = Arc::new(Mutex::new(Vec::new()));
        let pour_essai = Arc::clone(&attendus);
        let fil = std::thread::spawn(move || {
            let mut f = Fil::nouveau(
                d,
                sortie,
                source,
                pilote,
                FauxAtelier {
                    lignes: 0,
                    limite,
                    attendus: Arc::clone(&attendus),
                },
                premiere,
                Duration::ZERO,
                Vec::new,
            );
            f.courir();
        });
        Essai {
            page,
            envoi,
            signaux,
            arret,
            fil,
            attendus: pour_essai,
        }
    }

    fn pilote(page: &Partagee, moyens: &[Moyen], effet: bool) -> FauxPilote {
        FauxPilote {
            page: Arc::clone(page),
            moyens: moyens.to_vec(),
            effet,
            perd_au_pas: false,
            sautes: 0,
            cran: None,
            constats: Arc::new(Mutex::new(Vec::new())),
            vue: Some(50.0),
        }
    }

    impl Essai {
        /// Les signaux jusqu'à `Assemblee` (ou un signal de fin d'une autre sorte).
        fn jusqu_a_la_fin(&self) -> Vec<Signal> {
            let mut vus = Vec::new();
            loop {
                let s = self
                    .signaux
                    .recv_timeout(ATTENTE)
                    .expect("le fil doit finir");
                let fin = matches!(
                    s,
                    Signal::Assemblee { .. } | Signal::Impossible { .. } | Signal::Annulee { .. }
                );
                vus.push(s);
                if fin {
                    return vus;
                }
            }
        }

        fn attendre(&self, quoi: impl Fn(&Page) -> bool) {
            let fin = Instant::now() + ATTENTE;
            while !quoi(&self.page.lock().unwrap()) {
                assert!(Instant::now() < fin, "attente trop longue");
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        fn molette(&self) {
            self.envoi
                .send(Commande::Molette(defiler::Molette {
                    point: (50, 50),
                    crans: 1,
                }))
                .unwrap();
        }

        /// L'utilisateur fait défiler la page lui-même.
        fn rouler(&self, pixels: i32) {
            {
                let mut p = self.page.lock().unwrap();
                p.position = (p.position + pixels).clamp(0, p.bas);
            }
            self.molette();
        }

        fn terminer(self) -> (Vec<Signal>, Instant) {
            let a = Instant::now();
            self.envoi.send(Commande::Terminer(a)).unwrap();
            let s = self.jusqu_a_la_fin();
            self.fil.join().unwrap();
            (s, a)
        }
    }

    fn hauteur_assemblee(signaux: &[Signal]) -> u32 {
        match signaux.last() {
            Some(Signal::Assemblee { pixels, .. }) => pixels.hauteur,
            autre => panic!("pas d'image assemblee : {autre:?}"),
        }
    }

    fn finit_avant_assemblee(signaux: &[Signal]) {
        let n = signaux.len();
        assert!(n >= 2);
        assert_eq!(signaux[n - 2], Signal::Finit { numero: 1 });
    }

    #[test]
    fn fil_auto_jusqu_en_bas() {
        let p = page(400);
        let pi = pilote(&p, &[Moyen::Message], true);
        let e = lancer(p, Some(pi), 100_000);
        let s = e.jusqu_a_la_fin();
        e.fil.join().unwrap();
        finit_avant_assemblee(&s);
        assert_eq!(hauteur_assemblee(&s), 401);
        assert!(s.iter().any(|s| matches!(s, Signal::Apercu { .. })));
    }

    #[test]
    fn fil_terminer_pendant_l_auto_rend_l_instant_du_clic() {
        let p = page(1_000_000);
        let pi = pilote(&p, &[Moyen::Message], true);
        let e = lancer(p, Some(pi), 1_000_000);
        e.attendre(|p| p.pas >= 3);
        let (s, a) = e.terminer();
        let Some(Signal::Assemblee { relache, .. }) = s.last() else {
            panic!("pas d'image assemblee");
        };
        assert_eq!(*relache, a);
        assert!(a.elapsed() < Duration::from_secs(1));
        finit_avant_assemblee(&s);
    }

    #[test]
    fn fil_terminer_interrompt_le_pas() {
        let p = page(1_000_000);
        let pi = pilote(&p, &[Moyen::Message], true);
        let e = lancer(p, Some(pi), 1_000_000);
        e.attendre(|p| p.pas >= 2);
        let a = Instant::now();
        e.envoi.send(Commande::Terminer(a)).unwrap();
        loop {
            let s = e.signaux.recv_timeout(ATTENTE).unwrap();
            if s == (Signal::Finit { numero: 1 }) {
                break;
            }
        }
        // Le budget de 150 ms se mesure à l'essai 14 ; ici, l'attente est interrompue.
        assert!(
            a.elapsed() < Duration::from_millis(400),
            "{:?}",
            a.elapsed()
        );
        e.fil.join().unwrap();
    }

    #[test]
    fn fil_entree_pendant_l_attente_de_page_posee() {
        let p = page(1_000_000);
        p.lock().unwrap().animee = true;
        let pi = pilote(&p, &[Moyen::Message], true);
        let e = lancer(p, Some(pi), 1_000_000);
        e.attendre(|p| p.pas >= 1);
        // La page ne se pose jamais : le fil attend jusqu'à `ATTENTE_POSEE_MAX`.
        std::thread::sleep(Duration::from_millis(100));
        let a = Instant::now();
        e.envoi.send(Commande::Entree(a)).unwrap();
        loop {
            let s = e.signaux.recv_timeout(ATTENTE).unwrap();
            if s == (Signal::Finit { numero: 1 }) {
                break;
            }
        }
        // Le budget de 150 ms se mesure à l'essai 14 ; ici, l'attente est interrompue.
        assert!(
            a.elapsed() < Duration::from_millis(400),
            "{:?}",
            a.elapsed()
        );
        let s = e.jusqu_a_la_fin();
        let Some(Signal::Assemblee { relache, .. }) = s.last() else {
            panic!("pas d'image assemblee");
        };
        assert_eq!(*relache, a);
        e.fil.join().unwrap();
    }

    #[test]
    fn fil_la_molette_reprend_la_main() {
        let p = page(1_000_000);
        let pi = pilote(&p, &[Moyen::Message], true);
        let e = lancer(p, Some(pi), 1_000_000);
        e.attendre(|p| p.pas >= 2);
        e.molette();
        std::thread::sleep(Duration::from_millis(500));
        let (pas, depart) = {
            let p = e.page.lock().unwrap();
            (p.pas, p.position)
        };
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(e.page.lock().unwrap().pas, pas, "plus aucun pas en Main");
        for _ in 0..3 {
            e.rouler(30);
            std::thread::sleep(Duration::from_millis(150));
        }
        let (s, _) = e.terminer();
        assert_eq!(hauteur_assemblee(&s), 1 + depart as u32 + 90);
    }

    #[test]
    fn fil_sans_defileur_les_images_continuent() {
        let p = page(1_000);
        let e = lancer(p, None, 100_000);
        for _ in 0..4 {
            e.rouler(20);
            std::thread::sleep(Duration::from_millis(150));
        }
        let (s, _) = e.terminer();
        assert_eq!(hauteur_assemblee(&s), 81);
    }

    #[test]
    fn fil_replis_jusqu_a_aucun_moyen_puis_la_main() {
        let p = page(1_000);
        let pi = pilote(&p, &Moyen::ORDRE, false);
        let e = lancer(p, Some(pi), 100_000);
        e.attendre(|p| p.abandons >= 3);
        std::thread::sleep(Duration::from_millis(200));
        let pas = e.page.lock().unwrap().pas;
        // Deux pas sans effet par moyen.
        assert_eq!(pas, 6);
        e.rouler(40);
        std::thread::sleep(Duration::from_millis(200));
        e.rouler(40);
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(e.page.lock().unwrap().pas, pas);
        assert_eq!(e.page.lock().unwrap().abandons, 3);
        let (s, _) = e.terminer();
        assert_eq!(hauteur_assemblee(&s), 81);
    }

    #[test]
    fn fil_saute_sans_moyen_passe_a_la_main() {
        let p = page(1_000);
        let mut pi = pilote(&p, &[Moyen::Molette], true);
        pi.perd_au_pas = true;
        let e = lancer(p, Some(pi), 100_000);
        e.attendre(|p| p.pas >= 1);
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(e.page.lock().unwrap().pas, 1);
        e.rouler(50);
        std::thread::sleep(Duration::from_millis(200));
        let (s, _) = e.terminer();
        assert_eq!(hauteur_assemblee(&s), 51);
    }

    #[test]
    fn fil_constate_chaque_pas_parti() {
        let p = page(200);
        let pi = pilote(&p, &[Moyen::Message], true);
        let constats = Arc::clone(&pi.constats);
        let e = lancer(p, Some(pi), 100_000);
        let s = e.jusqu_a_la_fin();
        e.fil.join().unwrap();
        assert_eq!(hauteur_assemblee(&s), 201);
        let c = constats.lock().unwrap().clone();
        // Cinq pas de 40 qui avancent, puis des pas immobiles en bas : 0.
        assert_eq!(&c[..5], &[40, 40, 40, 40, 40]);
        assert!(c.len() > 5 && c[5..].iter().all(|&a| a == 0), "{c:?}");
    }

    #[test]
    fn fil_saute_n_est_pas_un_echec_du_moyen() {
        let p = page(200);
        let mut pi = pilote(&p, &[Moyen::Automation, Moyen::Molette], true);
        // Plus qu'il n'en faut pour deux pas « sans effet » s'ils comptaient.
        pi.sautes = 10;
        let e = lancer(p, Some(pi), 100_000);
        let s = e.jusqu_a_la_fin();
        e.fil.join().unwrap();
        assert_eq!(e.page.lock().unwrap().abandons, 0);
        assert_eq!(hauteur_assemblee(&s), 201);
    }

    #[test]
    fn fil_cran_trop_grand_laisse_le_moyen() {
        let p = page(1_000);
        let mut pi = pilote(&p, &[Moyen::Molette], true);
        // L'image d'essai a une ligne : tout cran est trop grand.
        pi.cran = Some(120);
        let e = lancer(p, Some(pi), 100_000);
        e.attendre(|p| p.abandons >= 1);
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(e.page.lock().unwrap().pas, 0);
        e.rouler(30);
        std::thread::sleep(Duration::from_millis(200));
        let (s, _) = e.terminer();
        assert_eq!(hauteur_assemblee(&s), 31);
    }

    #[test]
    fn fil_attendu_commande_sans_position() {
        let p = page(200);
        let mut pi = pilote(&p, &[Moyen::Message], true);
        pi.vue = None;
        let e = lancer(p, Some(pi), 100_000);
        let s = e.jusqu_a_la_fin();
        e.fil.join().unwrap();
        assert_eq!(hauteur_assemblee(&s), 201);
        let a = e.attendus.lock().unwrap().clone();
        // Les pixels commandés depuis l'image gardée : 40 à chaque pas, puis cumulés en
        // bas, où plus rien n'est gardé.
        assert_eq!(&a[..7], &[40, 40, 40, 40, 40, 40, 80], "{a:?}");
    }

    #[test]
    fn fil_attendu_observe_d_apres_la_position() {
        // Une ligne de zone, une vue de 0,1 % : 999 pixels de parcours, lus exactement.
        let p = page(999);
        let mut pi = pilote(&p, &[Moyen::Message], true);
        pi.vue = Some(0.1);
        let e = lancer(p, Some(pi), 100_000);
        let s = e.jusqu_a_la_fin();
        e.fil.join().unwrap();
        assert_eq!(hauteur_assemblee(&s), 1_000);
        let a = e.attendus.lock().unwrap().clone();
        assert_eq!(&a[..3], &[40, 40, 40], "{a:?}");
        // En bas, la position ne bouge plus : 0, et non le pas commandé.
        assert_eq!(a.last(), Some(&0), "{a:?}");
    }

    #[test]
    fn fil_limite() {
        let p = page(1_000_000);
        let pi = pilote(&p, &[Moyen::Message], true);
        let e = lancer(p, Some(pi), 100);
        let s = e.jusqu_a_la_fin();
        e.fil.join().unwrap();
        finit_avant_assemblee(&s);
        assert_eq!(hauteur_assemblee(&s), 101);
    }

    #[test]
    fn fil_prises_en_echec_fin_avec_ce_qui_est_pris() {
        let p = page(1_000);
        p.lock().unwrap().echecs = u32::MAX;
        let e = lancer(p, None, 100_000);
        let s = e.jusqu_a_la_fin();
        e.fil.join().unwrap();
        finit_avant_assemblee(&s);
        assert_eq!(hauteur_assemblee(&s), 1);
        assert!(e.page.lock().unwrap().prises >= regles::PRISES_EN_ECHEC_MAX);
    }

    #[test]
    fn fil_arret_sans_signal_de_fin() {
        let p = page(1_000_000);
        let pi = pilote(&p, &[Moyen::Message], true);
        let e = lancer(p, Some(pi), 1_000_000);
        e.attendre(|p| p.pas >= 1);
        e.arret.store(true, Ordering::SeqCst);
        e.envoi.send(Commande::Arret).unwrap();
        e.fil.join().unwrap();
        let fins = e.signaux.try_iter().filter(|s| {
            matches!(
                s,
                Signal::Finit { .. } | Signal::Assemblee { .. } | Signal::Impossible { .. }
            )
        });
        assert_eq!(fins.count(), 0);
    }
}
