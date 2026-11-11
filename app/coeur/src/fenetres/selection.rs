//! Sélection (étape 5) : choisir une zone de l'écran.
//!
//! Une seule fenêtre Tauri transparente, préparée au démarrage et cachée. Montrée, elle
//! couvre tout le bureau virtuel juste au-dessus de l'écran figé (fenêtre native de
//! `systeme::gel`) et ne dessine que le voile et le rectangle. Elle ne reçoit jamais les
//! pixels de la capture et ne sait pas ce que deviendra la zone : elle rend deux coins en
//! pixels réels du bureau, ou une annulation, par `Signal`.
//!
//! # Ce que l'Application doit faire à l'assemblage
//!
//! 1. Déclarer les quatre commandes dans `invoke_handler` :
//!    `fenetres::selection::{selection_chargee, selection_prete, selection_terminee,
//!    selection_annulee}` (elles sont déjà permises à la seule fenêtre `selection` par
//!    `capabilities/selection.json` et `build.rs`).
//! 2. Au démarrage, sur le fil principal, après `systeme::gel::preparer_ecran_fige` :
//!    `Selection::creer(app, rappel)`, puis `app.manage(selection)` (les commandes
//!    trouvent la sélection par `State<Selection>`).
//! 3. Pour une capture de zone, sur le fil principal : figer et montrer l'écran figé, puis
//!    `selection.montrer(Demande { bureau: <rect de la photo>, ecrans: <rect de chaque
//!    écran>, dessous: systeme::gel::poignee_ecran_fige(), cote_min: capture::COTE_MIN })`.
//!    Sur `Err` : la fenêtre est déjà cachée ; cacher l'écran figé et revenir au Repos.
//! 4. Pendant que la sélection est montrée (de `montrer` jusqu'au signal de fin), réserver
//!    Échap en global (`raccourcis`) : à l'appui, `selection.cacher()` puis cacher l'écran
//!    figé et revenir au Repos (aucun signal ne vient de la sélection dans ce cas). Libérer
//!    Échap à chaque fin.
//! 5. Dans le rappel (fil principal pour les signaux de fin, dans le même tour que le
//!    masquage de la fenêtre) : `Zone` → cacher l'écran figé tout de suite, puis
//!    `capture::rect_entre(depart, arrivee)`, borner à l'écran du départ et découper hors
//!    du fil principal ; `Annulee`, `Muette`, `Impossible` → cacher l'écran figé, Repos
//!    (`Impossible` : ligne de journal déjà écrite ici, message si utile). `Prete` : rien
//!    d'obligatoire (le chrono est déjà au journal). Le rappel ne doit pas bloquer.
//! 6. À l'arrêt : `selection.fermer()`, avant `systeme::gel::detruire_ecran_fige()`.
//!
//! # Règles
//!
//! - Un seul signal de fin par affichage (`Zone`, `Annulee`, `Muette`, `Impossible`) : le
//!   premier gagne. Toute commande dont l'`id` n'est pas celui en cours est ignorée.
//! - Avant le rappel d'un signal de fin : le premier plan est rendu à la fenêtre notée
//!   (avant le masquage, tant que l'app a encore le droit de le donner), la fenêtre est
//!   cachée, puis `selection:cachee` part vers la page.
//! - Aucun verrou n'est tenu pendant un appel à Tauri, un `emit` ou le rappel.
//! - Sous Windows la fenêtre est montrée et cachée par `systeme`, jamais par tao : `show()`
//!   de tao réécrit les styles étendus (window_state.rs, `apply_diff`) et remettrait la
//!   fenêtre dans Alt+Tab ; `set_focus()` de tao ne fait rien sur une fenêtre qu'il croit
//!   cachée (window.rs, `set_focus`). Hors Windows, tout passe par Tauri.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

use serde::{Deserialize, Deserializer, Serialize};
use tauri::{AppHandle, Emitter, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use super::ARGUMENTS_NAVIGATEUR;
use crate::systeme::{self, Rect};

/// Étiquette de la fenêtre (aussi dans `capabilities/selection.json`).
pub const ETIQUETTE_SELECTION: &str = "selection";
/// Événement écouté par la page : `{ id, ecrans: [RectPage], cote_min }`.
pub const EVENEMENT_MONTRER: &str = "selection:montrer";
/// Événement écouté par la page : la fenêtre est cachée, retour au repos.
pub const EVENEMENT_CACHEE: &str = "selection:cachee";
/// Sans `selection_prete` dans ce délai, la sélection est cachée (`Signal::Muette`).
pub const DELAI_PRETE: Duration = Duration::from_secs(2);
/// Tolérance, en pixels réels, pour un point venu de la page juste au bord de la fenêtre.
const TOLERANCE: f64 = 1.0;

/// Ce que l'Application demande d'afficher.
#[derive(Debug, Clone, PartialEq)]
pub struct Demande {
    /// Le bureau virtuel à couvrir (celui de la photo), en pixels réels.
    pub bureau: Rect,
    /// Les rectangles des écrans, en pixels réels.
    pub ecrans: Vec<Rect>,
    /// Poignée de la fenêtre figée, à garder juste en dessous.
    pub dessous: Option<isize>,
    /// Côté minimal d'une zone, en pixels réels (`capture::COTE_MIN`).
    pub cote_min: i32,
}

/// Pourquoi la sélection a été annulée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    Echap,
    ClicDroit,
    FocusPerdu,
    /// Demande de fermeture de la fenêtre (Alt+F4) : refusée, l'affichage est annulé.
    Fermeture,
}

impl Cause {
    pub fn nom(self) -> &'static str {
        match self {
            Cause::Echap => "echap",
            Cause::ClicDroit => "clic_droit",
            Cause::FocusPerdu => "focus_perdu",
            Cause::Fermeture => "fermeture",
        }
    }
}

/// Ce que la Sélection signale à l'Application.
#[derive(Debug, Clone, PartialEq)]
pub enum Signal {
    /// La page est visible et peinte. `duree` : depuis l'appel à `montrer`.
    Prete {
        id: u64,
        duree: Duration,
    },
    /// Zone choisie : les deux coins du glisser, en pixels réels du bureau, **tous deux
    /// inclus** (le premier et le dernier pixel de la zone, dans le sens du glisser) :
    /// `capture::rect_entre(depart, arrivee)` donne exactement la zone dessinée.
    Zone {
        id: u64,
        depart: (i32, i32),
        arrivee: (i32, i32),
    },
    Annulee {
        id: u64,
        cause: Cause,
    },
    /// Pas de `selection_prete` dans les 2 s.
    Muette {
        id: u64,
    },
    /// Donnée invalide venue de la page, fenêtre détruite, événement impossible à envoyer…
    Impossible {
        id: u64,
        cause: String,
    },
}

impl Signal {
    /// L'affichage auquel le signal se rapporte.
    pub fn id(&self) -> u64 {
        match self {
            Signal::Prete { id, .. }
            | Signal::Zone { id, .. }
            | Signal::Annulee { id, .. }
            | Signal::Muette { id }
            | Signal::Impossible { id, .. } => *id,
        }
    }
}

/// Fin d'un affichage telle qu'écrite au journal : `zone`, `annulee echap`, `muette`,
/// `impossible <cause>`. `None` pour `Prete`, qui n'est pas une fin.
pub fn decrire_fin(signal: &Signal) -> Option<String> {
    match signal {
        Signal::Prete { .. } => None,
        Signal::Zone { .. } => Some("zone".into()),
        Signal::Annulee { cause, .. } => Some(format!("annulee {}", cause.nom())),
        Signal::Muette { .. } => Some("muette".into()),
        Signal::Impossible { cause, .. } => Some(format!("impossible {cause}")),
    }
}

/// Rectangle en pixels CSS de la page.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RectPage {
    pub x: f64,
    pub y: f64,
    pub largeur: f64,
    pub hauteur: f64,
}

/// Point envoyé par la page, en pixels CSS. Une coordonnée absente ou qui n'est pas un
/// nombre (`null` pour un `NaN` passé par JSON) devient `NaN` : la commande arrive quand
/// même, et `vers_bureau` la refuse (signal `Impossible`).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Point {
    #[serde(default = "pas_un_nombre", deserialize_with = "nombre_ou_nan")]
    pub x: f64,
    #[serde(default = "pas_un_nombre", deserialize_with = "nombre_ou_nan")]
    pub y: f64,
}

fn pas_un_nombre() -> f64 {
    f64::NAN
}

fn nombre_ou_nan<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(v.as_f64().unwrap_or(f64::NAN))
}

/// Échelle utilisable : un nombre fini strictement positif.
fn echelle_valide(echelle: f64) -> bool {
    echelle.is_finite() && echelle > 0.0
}

/// Pixel CSS de la page → pixel réel du bureau. `None` si la donnée n'est pas un nombre fini
/// ou sort de la fenêtre (tolérance : 1 pixel réel). Le résultat est borné au bureau.
///
/// `bureau` est le rectangle réel de la fenêtre (le coin (0, 0) de la page est son coin
/// haut gauche). Un coin peut valoir le bord droit ou bas (exclus) : c'est un bord de zone.
pub fn vers_bureau(point: (f64, f64), echelle: f64, bureau: Rect) -> Option<(i32, i32)> {
    let (x, y) = point;
    if !echelle_valide(echelle) || bureau.est_vide() || !x.is_finite() || !y.is_finite() {
        return None;
    }
    let (rx, ry) = (x * echelle, y * echelle);
    let (l, h) = (f64::from(bureau.largeur), f64::from(bureau.hauteur));
    if rx < -TOLERANCE || ry < -TOLERANCE || rx > l + TOLERANCE || ry > h + TOLERANCE {
        return None;
    }
    let bx = (rx.round().clamp(0.0, l) + f64::from(bureau.x)) as i64;
    let by = (ry.round().clamp(0.0, h) + f64::from(bureau.y)) as i64;
    let bx = bx.clamp(i64::from(bureau.x), bureau.droite());
    let by = by.clamp(i64::from(bureau.y), bureau.bas());
    Some((i32::try_from(bx).ok()?, i32::try_from(by).ok()?))
}

/// Rectangle réel du bureau → rectangle en pixels CSS de la page (pour envoyer les écrans).
/// Une échelle invalide compte pour 1.
pub fn vers_page(rect: Rect, echelle: f64, bureau: Rect) -> RectPage {
    let e = if echelle_valide(echelle) {
        echelle
    } else {
        1.0
    };
    RectPage {
        x: (f64::from(rect.x) - f64::from(bureau.x)) / e,
        y: (f64::from(rect.y) - f64::from(bureau.y)) / e,
        largeur: f64::from(rect.largeur.max(0)) / e,
        hauteur: f64::from(rect.hauteur.max(0)) / e,
    }
}

/// Cause d'annulation envoyée par la page (`echap`, `clic_droit`). `focus_perdu` ne vient
/// jamais de la page.
pub fn cause_depuis_nom(nom: &str) -> Option<Cause> {
    match nom {
        "echap" => Some(Cause::Echap),
        "clic_droit" => Some(Cause::ClicDroit),
        _ => None,
    }
}

/// Ce que vaut un `selection_terminee` pour l'affichage en cours.
#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    /// Une vraie zone : les deux coins en pixels réels, inclus (voir `Signal::Zone`).
    Zone((i32, i32), (i32, i32)),
    /// Moins de `cote_min` de côté : pas une zone, la sélection continue.
    TropPetite,
    /// Point non fini ou hors de la fenêtre.
    Invalide(String),
}

/// Décide d'un `selection_terminee` (fonction pure). `fenetre` : rectangle réel de la page.
///
/// La page envoie des **bords** : la zone dessinée couvre `[min, max)` sur chaque axe. Le
/// seuil se compte sur ces bords (`|a − b| >= cote_min`) ; la zone rendue donne des coins
/// de pixels inclus : sur chaque axe, le côté max perd 1, le sens du glisser est gardé.
pub fn decider_zone(
    depart: (f64, f64),
    arrivee: (f64, f64),
    echelle: f64,
    fenetre: Rect,
    cote_min: i32,
) -> Decision {
    let (Some(a), Some(b)) = (
        vers_bureau(depart, echelle, fenetre),
        vers_bureau(arrivee, echelle, fenetre),
    ) else {
        return Decision::Invalide("point invalide".into());
    };
    let largeur = (i64::from(a.0) - i64::from(b.0)).abs();
    let hauteur = (i64::from(a.1) - i64::from(b.1)).abs();
    let min = i64::from(cote_min.max(0));
    if largeur < min || hauteur < min {
        return Decision::TropPetite;
    }
    // Bords → pixels inclus : celui des deux qui est le bord max recule d'un pixel.
    let inclus = |u: i32, v: i32| -> (i32, i32) {
        match u.cmp(&v) {
            std::cmp::Ordering::Less => (u, v - 1),
            std::cmp::Ordering::Greater => (u - 1, v),
            std::cmp::Ordering::Equal => (u, v),
        }
    };
    let (ax, bx) = inclus(a.0, b.0);
    let (ay, by) = inclus(a.1, b.1);
    Decision::Zone((ax, ay), (bx, by))
}

/// Ce que reçoit la page avec `selection:montrer`.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct ChargeMontrer {
    id: u64,
    ecrans: Vec<RectPage>,
    /// En pixels CSS.
    cote_min: f64,
    /// Taille attendue de la page, en pixels CSS (rectangle réel / échelle).
    largeur: f64,
    hauteur: f64,
    /// Échelle attendue (`devicePixelRatio` de la page).
    echelle: f64,
}

/// Un affichage en cours, vu du cœur.
#[derive(Debug, Clone)]
struct Affichage {
    id: u64,
    /// Rectangle réel de la fenêtre (repère des conversions de cet affichage).
    fenetre: Rect,
    echelle: f64,
    cote_min: i32,
    montre_a: Instant,
    /// La fenêtre au premier plan avant l'affichage, à qui le rendre.
    avant: Option<isize>,
    prete: bool,
    a_eu_focus: bool,
    /// Ce qui a été envoyé à la page, renvoyé si elle se recharge ou si une zone trop
    /// petite lui est refusée (elle revient alors au repos).
    charge: Option<ChargeMontrer>,
}

impl Affichage {
    fn nouveau(id: u64, demande: &Demande, avant: Option<isize>, maintenant: Instant) -> Self {
        Affichage {
            id,
            fenetre: demande.bureau,
            echelle: 1.0,
            cote_min: demande.cote_min,
            montre_a: maintenant,
            avant,
            prete: false,
            a_eu_focus: false,
            charge: None,
        }
    }
}

/// L'état de la sélection : une petite machine pure, testée sans fenêtre.
#[derive(Debug, Default)]
struct Etat {
    dernier_id: u64,
    en_cours: Option<Affichage>,
    page_chargee: bool,
}

impl Etat {
    /// Commence un affichage (le précédent, s'il y en a un, est oublié sans signal). La
    /// fenêtre à qui rendre le premier plan est `devant`, sauf si c'est la sélection
    /// elle-même : on garde alors celle de l'affichage oublié. Rend l'`id` et l'`id` oublié.
    fn commencer(
        &mut self,
        demande: &Demande,
        devant: Option<isize>,
        nous: Option<isize>,
        maintenant: Instant,
    ) -> (u64, Option<u64>) {
        self.dernier_id += 1;
        let id = self.dernier_id;
        let ancien = self.en_cours.take();
        let avant = if devant.is_some() && devant == nous {
            ancien.as_ref().and_then(|a| a.avant)
        } else {
            devant
        };
        self.en_cours = Some(Affichage::nouveau(id, demande, avant, maintenant));
        (id, ancien.map(|a| a.id))
    }

    fn courant(&self, id: u64) -> Option<&Affichage> {
        self.en_cours.as_ref().filter(|a| a.id == id)
    }

    fn courant_mut(&mut self, id: u64) -> Option<&mut Affichage> {
        self.en_cours.as_mut().filter(|a| a.id == id)
    }

    /// Le premier signal de fin pour `id` gagne : l'affichage est retiré et rendu. Ensuite,
    /// et pour tout autre `id`, `None`.
    fn terminer(&mut self, id: u64) -> Option<Affichage> {
        self.courant(id)?;
        self.en_cours.take()
    }

    /// `selection_prete` : la durée depuis `montrer`, la première fois seulement.
    fn prete(&mut self, id: u64, maintenant: Instant) -> Option<Duration> {
        let a = self.courant_mut(id)?;
        if a.prete {
            return None;
        }
        a.prete = true;
        Some(maintenant.saturating_duration_since(a.montre_a))
    }

    /// Fin du délai `DELAI_PRETE` pour `id` : vrai s'il est toujours en cours sans être prêt.
    fn muette(&self, id: u64) -> bool {
        self.courant(id).is_some_and(|a| !a.prete)
    }

    /// Changement de focus de la fenêtre. Rend l'`id` à annuler (`FocusPerdu`) quand la
    /// fenêtre perd le focus après l'avoir eu pendant cet affichage.
    fn focus(&mut self, focus: bool) -> Option<u64> {
        let a = self.en_cours.as_mut()?;
        if focus {
            a.a_eu_focus = true;
            None
        } else {
            a.a_eu_focus.then_some(a.id)
        }
    }
}

type Rappel = Box<dyn Fn(Signal) + Send + Sync + 'static>;

/// Le cœur de la sélection, partagé avec les fils du délai et les événements de fenêtre :
/// ces filets de sécurité marchent même si l'Application n'a pas confié la sélection à
/// Tauri.
struct Interne {
    app: AppHandle,
    rappel: Rappel,
    etat: Mutex<Etat>,
    fil_principal: ThreadId,
    /// Faux après `fermer` ou la destruction de la fenêtre.
    vivante: AtomicBool,
    /// Identifiant système de la fenêtre (Windows : le HWND), gardé dès la création.
    identifiant: Option<isize>,
}

/// La fenêtre de sélection, préchargée et cachée. À confier à Tauri (`app.manage`).
pub struct Selection {
    interne: Arc<Interne>,
}

impl Selection {
    /// Crée la fenêtre cachée et charge la page (fil principal, au démarrage).
    pub fn creer<F>(app: &AppHandle, rappel: F) -> Result<Selection, String>
    where
        F: Fn(Signal) + Send + Sync + 'static,
    {
        let capturable = systeme::essai_capturable();
        let fenetre = WebviewWindowBuilder::new(
            app,
            ETIQUETTE_SELECTION,
            WebviewUrl::App("selection/selection.html".into()),
        )
        .title("BreezyShot")
        .inner_size(320.0, 200.0)
        .visible(false)
        .focused(false)
        .focusable(true)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .content_protected(!capturable)
        .additional_browser_args(ARGUMENTS_NAVIGATEUR)
        .build()
        .map_err(|e| format!("fenetre de selection: {e}"))?;
        if let Err(e) = systeme::preparer_fenetre_outil_active(&fenetre) {
            log::warn!("selection: style outil non pose: {e}");
        }
        let identifiant = systeme::identifiant_fenetre(&fenetre);
        if let Some(h) = identifiant {
            log::info!("selection fenetre {h}");
        }
        if capturable {
            log::info!("selection exclue des captures: non (essai)");
        } else {
            let exclue = systeme::exclure_des_captures(&fenetre);
            log::info!(
                "selection exclue des captures: {}",
                if exclue { "oui" } else { "non" }
            );
        }
        let interne = Arc::new(Interne {
            app: app.clone(),
            rappel: Box::new(rappel),
            etat: Mutex::new(Etat::default()),
            fil_principal: std::thread::current().id(),
            vivante: AtomicBool::new(true),
            identifiant,
        });
        let pour_evenements = Arc::clone(&interne);
        fenetre.on_window_event(move |e| match e {
            tauri::WindowEvent::Focused(focus) => {
                // Traité au tour suivant de la boucle, hors du rappel de Tauri.
                let i = Arc::clone(&pour_evenements);
                let focus = *focus;
                let _ = pour_evenements
                    .app
                    .run_on_main_thread(move || i.changer_focus(focus));
            }
            tauri::WindowEvent::CloseRequested { api, .. } => {
                // La fenêtre n'est détruite que par `fermer` : Alt+F4 annule l'affichage.
                api.prevent_close();
                let i = Arc::clone(&pour_evenements);
                let _ = pour_evenements
                    .app
                    .run_on_main_thread(move || i.fermeture_demandee());
            }
            tauri::WindowEvent::Destroyed => {
                pour_evenements.vivante.store(false, Ordering::SeqCst);
                let i = Arc::clone(&pour_evenements);
                let _ = pour_evenements
                    .app
                    .run_on_main_thread(move || i.fenetre_detruite());
            }
            _ => {}
        });
        Ok(Selection { interne })
    }

    /// Couvre le bureau, prend le clavier, prévient la page. Fil principal. Rend l'`id`.
    /// Toute erreur laisse la fenêtre cachée.
    pub fn montrer(&self, demande: Demande) -> Result<u64, String> {
        self.interne.montrer(demande)
    }

    /// Cache la fenêtre et rend le premier plan. Sûr à tout moment, de n'importe quel fil,
    /// même si rien n'est montré. N'émet aucun signal.
    pub fn cacher(&self) {
        self.interne.cacher();
    }

    /// Vrai entre `montrer` et la fin de l'affichage.
    pub fn visible(&self) -> bool {
        self.interne.etat().en_cours.is_some()
    }

    /// À l'arrêt de l'app : cache puis détruit la fenêtre. Aucun signal.
    pub fn fermer(&self) {
        self.interne.vivante.store(false, Ordering::SeqCst);
        self.interne.cacher();
        if let Some(f) = self.interne.fenetre() {
            let _ = f.destroy();
        }
    }
}

/// Prend le verrou de l'état. Un verrou empoisonné (panique d'un autre fil pendant qu'il le
/// tenait) est repris tel quel, comme dans `systeme::gel` : `finir`, `cacher` et le délai
/// doivent toujours pouvoir cacher la fenêtre.
fn prendre(etat: &Mutex<Etat>) -> MutexGuard<'_, Etat> {
    match etat.lock() {
        Ok(e) => e,
        Err(empoisonne) => {
            log::error!("selection: etat repris apres une panique");
            etat.clear_poison();
            empoisonne.into_inner()
        }
    }
}

/// Nombre de placements au plus quand le rectangle réel diffère du bureau voulu.
const PLACEMENTS_MAX: usize = 3;

impl Interne {
    fn fenetre(&self) -> Option<WebviewWindow> {
        tauri::Manager::get_webview_window(&self.app, ETIQUETTE_SELECTION)
    }

    fn etat(&self) -> MutexGuard<'_, Etat> {
        prendre(&self.etat)
    }

    fn sur_fil_principal(&self) -> bool {
        std::thread::current().id() == self.fil_principal
    }

    fn montrer(self: &Arc<Self>, demande: Demande) -> Result<u64, String> {
        if !self.sur_fil_principal() {
            return Err("selection: montrer hors du fil principal".into());
        }
        if !self.vivante.load(Ordering::SeqCst) {
            return Err("selection: fenetre fermee".into());
        }
        if demande.bureau.est_vide() {
            return Err("selection: bureau vide".into());
        }
        let fenetre = self.fenetre().ok_or("selection: fenetre absente")?;
        // 1. La fenêtre au premier plan, pour la lui rendre à la fin.
        let devant = systeme::fenetre_premier_plan();
        let (id, oublie) =
            self.etat()
                .commencer(&demande, devant, self.identifiant, Instant::now());
        if let Some(ancien) = oublie {
            log::warn!("selection {ancien} remplacee par {id} sans fin");
        }
        match self.montrer_etapes(&fenetre, id, &demande) {
            Ok(()) => Ok(id),
            Err(e) => {
                let avant = self.etat().terminer(id).and_then(|a| a.avant);
                self.masquer(avant);
                log::warn!("selection montree {id} impossible: {e}");
                Err(e)
            }
        }
    }

    /// Étapes 2 à 6 de `montrer` (l'état de l'affichage `id` existe déjà).
    fn montrer_etapes(
        self: &Arc<Self>,
        fenetre: &WebviewWindow,
        id: u64,
        demande: &Demande,
    ) -> Result<(), String> {
        let voulu = demande.bureau;
        // 2. Poser, montrer, donner le premier plan et le clavier. Un premier plan refusé
        // n'arrête rien : un clic active la fenêtre, le clic droit annule, et l'Application
        // réserve Échap en global.
        let par_systeme = systeme::poser_au_dessus(fenetre, voulu)?;
        if par_systeme {
            if !systeme::prendre_premier_plan(fenetre)? {
                log::warn!("selection sans premier plan {id}");
            }
        } else {
            poser_par_tauri(fenetre, voulu)?;
            fenetre.show().map_err(|e| format!("show: {e}"))?;
            if let Err(e) = fenetre.set_focus() {
                log::warn!("selection sans premier plan {id}: {e}");
            }
        }
        // 3. L'écran figé juste en dessous.
        self.placer_dessous(demande.dessous)?;
        // 4. Relire le rectangle réel ; le reposer (3 fois au plus) s'il diffère. S'il
        // diffère encore, on continue avec le rectangle réel, repère des conversions.
        let mut reel = self.identifiant.and_then(rect_reel);
        let mut placements = 1;
        while placements < PLACEMENTS_MAX && reel.is_some_and(|r| r != voulu) {
            systeme::poser_au_dessus(fenetre, voulu)?;
            self.placer_dessous(demande.dessous)?;
            reel = self.identifiant.and_then(rect_reel);
            placements += 1;
        }
        let echelle = fenetre
            .scale_factor()
            .map_err(|e| format!("echelle: {e}"))?;
        if !echelle_valide(echelle) {
            return Err(format!("echelle invalide {echelle}"));
        }
        let repere = reel.unwrap_or(voulu);
        log::info!(
            "selection montree {id} voulu {} reel {} echelle {echelle} placements {placements}",
            texte_rect(voulu),
            reel.map(texte_rect).unwrap_or_else(|| "inconnu".into())
        );
        // 5. Prévenir la page, avec les écrans dans son repère et la taille attendue.
        let charge = ChargeMontrer {
            id,
            ecrans: demande
                .ecrans
                .iter()
                .map(|r| vers_page(*r, echelle, repere))
                .collect(),
            cote_min: f64::from(demande.cote_min.max(0)) / echelle,
            largeur: f64::from(repere.largeur.max(0)) / echelle,
            hauteur: f64::from(repere.hauteur.max(0)) / echelle,
            echelle,
        };
        {
            let mut etat = self.etat();
            let a = etat.courant_mut(id).ok_or("affichage remplace")?;
            a.fenetre = repere;
            a.echelle = echelle;
            a.charge = Some(charge.clone());
        }
        self.app
            .emit_to(ETIQUETTE_SELECTION, EVENEMENT_MONTRER, charge)
            .map_err(|e| format!("evenement non envoye: {e}"))?;
        // 6. Le filet de sécurité : sans page prête à temps, on cache.
        self.armer_delai(id)
    }

    fn placer_dessous(&self, dessous: Option<isize>) -> Result<(), String> {
        if let (Some(bas), Some(haut)) = (dessous, self.identifiant) {
            systeme::placer_sous(bas, haut).map_err(|e| format!("ecran fige non place: {e}"))?;
        }
        Ok(())
    }

    fn armer_delai(self: &Arc<Self>, id: u64) -> Result<(), String> {
        let interne = Arc::clone(self);
        std::thread::Builder::new()
            .name("selection-delai".into())
            .spawn(move || {
                std::thread::sleep(DELAI_PRETE);
                let pour = Arc::clone(&interne);
                let poste = interne.app.run_on_main_thread(move || {
                    let muette = pour.etat().muette(id);
                    if muette {
                        pour.finir(Signal::Muette { id });
                    }
                });
                if poste.is_err() {
                    // Boucle arrêtée : au moins ne pas laisser l'écran couvert.
                    if let Some(h) = interne.identifiant {
                        systeme::cacher_fenetre_async(h);
                    }
                }
            })
            .map(|_| ())
            .map_err(|e| format!("delai non arme: {e}"))
    }

    /// Rend le premier plan à `avant` (tant que l'app a le droit de le donner), cache la
    /// fenêtre, puis prévient la page. Fil principal.
    fn masquer(&self, avant: Option<isize>) {
        if let Some(a) = avant.filter(|a| Some(*a) != self.identifiant) {
            systeme::rendre_premier_plan(a);
        }
        match self.fenetre() {
            Some(f) => {
                let fait = match systeme::cacher_sans_tao(&f) {
                    Ok(true) => Ok(()),
                    Ok(false) => f.hide().map_err(|e| e.to_string()),
                    Err(e) => Err(e),
                };
                if let Err(e) = fait {
                    log::warn!("selection: fenetre non cachee: {e}");
                    if let Some(h) = self.identifiant {
                        systeme::cacher_fenetre_async(h);
                    }
                }
                if let Err(e) = self.app.emit_to(ETIQUETTE_SELECTION, EVENEMENT_CACHEE, ()) {
                    log::warn!("selection: page non prevenue du masquage: {e}");
                }
            }
            None => {
                if let Some(h) = self.identifiant {
                    systeme::cacher_fenetre_async(h);
                }
            }
        }
    }

    /// Signal de fin : le premier pour cet `id` gagne. Fenêtre cachée et premier plan rendu
    /// avant le rappel, dans le même tour, sur le fil principal.
    fn finir(self: &Arc<Self>, signal: Signal) {
        if !self.sur_fil_principal() {
            let i = Arc::clone(self);
            let poste = self.app.run_on_main_thread(move || i.finir(signal));
            if poste.is_err() {
                if let Some(h) = self.identifiant {
                    systeme::cacher_fenetre_async(h);
                }
            }
            return;
        }
        let id = signal.id();
        let Some(affichage) = self.etat().terminer(id) else {
            return;
        };
        self.masquer(affichage.avant);
        if let Some(fin) = decrire_fin(&signal) {
            log::info!("selection fin {id} {fin}");
        }
        (self.rappel)(signal);
    }

    /// `cacher` public : sans signal. Hors du fil principal, tout part sur le fil principal
    /// (un masquage posté directement à Windows pourrait cacher l'affichage suivant) ; la
    /// fenêtre n'est cachée sans attendre que si cet envoi échoue (boucle arrêtée).
    fn cacher(self: &Arc<Self>) {
        let avant = self.etat().en_cours.take().and_then(|a| a.avant);
        if self.sur_fil_principal() {
            self.masquer(avant);
            return;
        }
        let i = Arc::clone(self);
        let poste = self.app.run_on_main_thread(move || {
            // Un nouvel affichage commencé entre-temps n'est pas touché.
            let libre = i.etat().en_cours.is_none();
            if libre {
                i.masquer(avant);
            }
        });
        if poste.is_err() {
            if let Some(h) = self.identifiant {
                systeme::cacher_fenetre_async(h);
            }
        }
    }

    fn changer_focus(self: &Arc<Self>, focus: bool) {
        let perdu = self.etat().focus(focus);
        if let Some(id) = perdu {
            self.finir(Signal::Annulee {
                id,
                cause: Cause::FocusPerdu,
            });
        }
    }

    fn fermeture_demandee(self: &Arc<Self>) {
        let id = self.etat().en_cours.as_ref().map(|a| a.id);
        if let Some(id) = id {
            self.finir(Signal::Annulee {
                id,
                cause: Cause::Fermeture,
            });
        }
    }

    fn fenetre_detruite(self: &Arc<Self>) {
        let id = self.etat().en_cours.as_ref().map(|a| a.id);
        if let Some(id) = id {
            self.finir(Signal::Impossible {
                id,
                cause: "fenetre detruite".into(),
            });
        }
    }

    /// Renvoie à la page la demande de l'affichage en cours : elle revient au repos (son
    /// `selection_prete` suivant est ignoré s'il l'était déjà).
    fn renvoyer(&self, charge: ChargeMontrer) {
        if let Err(e) = self
            .app
            .emit_to(ETIQUETTE_SELECTION, EVENEMENT_MONTRER, charge)
        {
            log::warn!("selection: demande non renvoyee: {e}");
        }
    }

    fn page_chargee(&self) {
        let renvoi = {
            let mut etat = self.etat();
            etat.page_chargee = true;
            etat.en_cours.as_ref().and_then(|a| a.charge.clone())
        };
        log::info!("selection chargee");
        // Page (re)chargée pendant un affichage : elle reçoit de nouveau sa demande.
        if let Some(charge) = renvoi {
            self.renvoyer(charge);
        }
    }

    fn prete(&self, id: u64, page: (f64, f64, f64)) {
        let duree = self.etat().prete(id, Instant::now());
        if let Some(duree) = duree {
            let (largeur, hauteur, echelle) = page;
            log::info!(
                "chrono selection prete {} page {largeur}x{hauteur} echelle {echelle}",
                duree.as_millis()
            );
            (self.rappel)(Signal::Prete { id, duree });
        }
    }

    fn terminee(self: &Arc<Self>, id: u64, depart: Point, arrivee: Point) {
        let Some((fenetre, echelle, cote_min, charge)) = self
            .etat()
            .courant(id)
            .map(|a| (a.fenetre, a.echelle, a.cote_min, a.charge.clone()))
        else {
            return;
        };
        match decider_zone(
            (depart.x, depart.y),
            (arrivee.x, arrivee.y),
            echelle,
            fenetre,
            cote_min,
        ) {
            Decision::Zone(depart, arrivee) => self.finir(Signal::Zone {
                id,
                depart,
                arrivee,
            }),
            Decision::TropPetite => {
                // La page attend qu'on la cache : on la remet au repos, la sélection continue.
                log::info!("selection {id}: zone trop petite ignoree");
                if let Some(charge) = charge {
                    self.renvoyer(charge);
                }
            }
            Decision::Invalide(cause) => self.finir(Signal::Impossible { id, cause }),
        }
    }

    fn annulee(self: &Arc<Self>, id: u64, nom: &str) {
        if self.etat().courant(id).is_none() {
            return;
        }
        let signal = match cause_depuis_nom(nom) {
            Some(cause) => Signal::Annulee { id, cause },
            None => Signal::Impossible {
                id,
                cause: "cause inconnue".into(),
            },
        };
        self.finir(signal);
    }
}

/// Pose la fenêtre par Tauri (hors Windows), en pixels réels. La position est redonnée
/// après la taille, comme pour le bandeau.
fn poser_par_tauri(fenetre: &WebviewWindow, r: Rect) -> Result<(), String> {
    let position = tauri::PhysicalPosition::new(r.x, r.y);
    let taille = tauri::PhysicalSize::new(r.largeur.max(1) as u32, r.hauteur.max(1) as u32);
    fenetre
        .set_position(position)
        .and_then(|_| fenetre.set_size(taille))
        .and_then(|_| fenetre.set_position(position))
        .map_err(|e| format!("placement: {e}"))
}

/// Rectangle réel de la fenêtre, relu auprès du système (`None` hors Windows).
fn rect_reel(identifiant: isize) -> Option<Rect> {
    let (x, y, l, h, _) = systeme::rectangle_fenetre(identifiant)?;
    Some(Rect {
        x,
        y,
        largeur: i32::try_from(l).ok()?,
        hauteur: i32::try_from(h).ok()?,
    })
}

fn texte_rect(r: Rect) -> String {
    format!("{},{},{},{}", r.x, r.y, r.largeur, r.hauteur)
}

// Commandes appelées par la page (les seules permises à la fenêtre `selection`).
// Un `id` qui n'est pas celui de l'affichage en cours est ignoré.

/// La page écoute `selection:montrer` et `selection:cachee`.
#[tauri::command]
pub fn selection_chargee(selection: State<'_, Selection>) {
    selection.interne.page_chargee();
}

/// La page est visible et peinte, à la taille et à l'échelle attendues (deux images de
/// suite après `selection:montrer`). `largeur`, `hauteur` : `innerWidth`, `innerHeight` ;
/// `echelle` : `devicePixelRatio`. Écrits au journal.
#[tauri::command]
pub fn selection_prete(
    selection: State<'_, Selection>,
    id: u64,
    largeur: f64,
    hauteur: f64,
    echelle: f64,
) {
    selection.interne.prete(id, (largeur, hauteur, echelle));
}

/// Zone choisie : les deux coins finals, en pixels CSS de la page.
#[tauri::command]
pub fn selection_terminee(selection: State<'_, Selection>, id: u64, depart: Point, arrivee: Point) {
    selection.interne.terminee(id, depart, arrivee);
}

/// Annulation par la page : `cause` vaut `"echap"` ou `"clic_droit"`.
#[tauri::command]
pub fn selection_annulee(selection: State<'_, Selection>, id: u64, cause: String) {
    selection.interne.annulee(id, &cause);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i32, y: i32, largeur: i32, hauteur: i32) -> Rect {
        Rect {
            x,
            y,
            largeur,
            hauteur,
        }
    }

    fn demande() -> Demande {
        Demande {
            bureau: r(0, 0, 1920, 1080),
            ecrans: vec![r(0, 0, 1920, 1080)],
            dessous: None,
            cote_min: 3,
        }
    }

    #[test]
    fn vers_bureau_aux_echelles_courantes() {
        let bureau = r(0, 0, 3840, 3840);
        for e in [1.0, 1.25, 1.5, 2.0] {
            // Un pixel réel entier, exprimé en CSS, revient exactement.
            for reel in [0, 1, 7, 333, 1919, 3840] {
                let css = f64::from(reel) / e;
                assert_eq!(
                    vers_bureau((css, css), e, bureau),
                    Some((reel, reel)),
                    "echelle {e}, pixel {reel}"
                );
            }
        }
        assert_eq!(vers_bureau((100.0, 40.0), 1.25, bureau), Some((125, 50)));
        assert_eq!(vers_bureau((100.0, 40.0), 1.5, bureau), Some((150, 60)));
        assert_eq!(vers_bureau((100.0, 40.0), 2.0, bureau), Some((200, 80)));
    }

    #[test]
    fn vers_bureau_avec_origine_negative() {
        // Un écran à gauche de l'écran principal : le bureau commence à x = -1920.
        let bureau = r(-1920, -200, 3840, 1280);
        assert_eq!(vers_bureau((0.0, 0.0), 1.0, bureau), Some((-1920, -200)));
        assert_eq!(vers_bureau((1920.0, 200.0), 1.0, bureau), Some((0, 0)));
        assert_eq!(vers_bureau((1536.0, 160.0), 1.25, bureau), Some((0, 0)));
        assert_eq!(
            vers_bureau((2560.0, 800.0), 1.5, bureau),
            Some((1920, 1000))
        );
    }

    #[test]
    fn vers_bureau_bornes_et_tolerance() {
        let bureau = r(10, 20, 100, 50);
        // Les bords exclus sont des coins valides.
        assert_eq!(vers_bureau((100.0, 50.0), 1.0, bureau), Some((110, 70)));
        // Jusqu'à 1 pixel réel dehors : ramené au bord.
        assert_eq!(vers_bureau((-1.0, -0.5), 1.0, bureau), Some((10, 20)));
        assert_eq!(vers_bureau((101.0, 51.0), 1.0, bureau), Some((110, 70)));
        assert_eq!(vers_bureau((-0.8, 0.0), 1.25, bureau), Some((10, 20)));
        // Au-delà : refusé.
        assert_eq!(vers_bureau((-1.01, 0.0), 1.0, bureau), None);
        assert_eq!(vers_bureau((0.0, 51.01), 1.0, bureau), None);
        assert_eq!(vers_bureau((81.0, 0.0), 1.25, bureau), None);
    }

    #[test]
    fn vers_bureau_refuse_ce_qui_n_est_pas_un_nombre() {
        let bureau = r(0, 0, 100, 100);
        assert_eq!(vers_bureau((f64::NAN, 1.0), 1.0, bureau), None);
        assert_eq!(vers_bureau((1.0, f64::INFINITY), 1.0, bureau), None);
        assert_eq!(vers_bureau((f64::NEG_INFINITY, 1.0), 1.0, bureau), None);
        assert_eq!(vers_bureau((1.0, 1.0), f64::NAN, bureau), None);
        assert_eq!(vers_bureau((1.0, 1.0), 0.0, bureau), None);
        assert_eq!(vers_bureau((1.0, 1.0), -1.0, bureau), None);
        assert_eq!(vers_bureau((1.0, 1.0), 1.0, r(0, 0, 0, 10)), None);
        assert_eq!(vers_bureau((1e300, 1.0), 1.0, bureau), None);
    }

    #[test]
    fn vers_page_et_aller_retour() {
        let bureau = r(-1920, 0, 5760, 1080);
        let ecrans = [
            r(-1920, 0, 1920, 1080),
            r(0, 0, 1920, 1080),
            r(1920, 0, 1920, 1080),
        ];
        for e in [1.0, 1.25, 1.5, 2.0] {
            for ecran in ecrans {
                let p = vers_page(ecran, e, bureau);
                assert_eq!(
                    vers_bureau((p.x, p.y), e, bureau),
                    Some((ecran.x, ecran.y)),
                    "echelle {e}"
                );
                assert_eq!(
                    vers_bureau((p.x + p.largeur, p.y + p.hauteur), e, bureau),
                    Some((ecran.x + ecran.largeur, ecran.y + ecran.hauteur)),
                    "echelle {e}"
                );
            }
        }
        assert_eq!(
            vers_page(r(0, 0, 1920, 1080), 1.25, bureau),
            RectPage {
                x: 1536.0,
                y: 0.0,
                largeur: 1536.0,
                hauteur: 864.0
            }
        );
        // Échelle invalide : comptée pour 1.
        assert_eq!(
            vers_page(r(10, 0, 5, 5), f64::NAN, r(0, 0, 100, 100)).x,
            10.0
        );
    }

    #[test]
    fn causes_de_la_page() {
        assert_eq!(cause_depuis_nom("echap"), Some(Cause::Echap));
        assert_eq!(cause_depuis_nom("clic_droit"), Some(Cause::ClicDroit));
        assert_eq!(cause_depuis_nom("focus_perdu"), None);
        assert_eq!(cause_depuis_nom("Echap"), None);
        assert_eq!(cause_depuis_nom(""), None);
    }

    #[test]
    fn decision_d_une_zone() {
        let f = r(0, 0, 1920, 1080);
        assert_eq!(
            decider_zone((10.0, 10.0), (110.0, 60.0), 1.0, f, 3),
            Decision::Zone((10, 10), (109, 59))
        );
        // Sens inverse : le sens est gardé, le bord max (ici le départ) recule d'un pixel.
        assert_eq!(
            decider_zone((110.0, 60.0), (10.0, 10.0), 1.25, f, 3),
            Decision::Zone((137, 74), (13, 13))
        );
        // Moins de 3 pixels réels de côté.
        assert_eq!(
            decider_zone((10.0, 10.0), (12.0, 200.0), 1.0, f, 3),
            Decision::TropPetite
        );
        assert_eq!(
            decider_zone((10.0, 10.0), (10.0, 10.0), 1.0, f, 3),
            Decision::TropPetite
        );
        // 2,4 pixels CSS à 125 % = 3 pixels réels : une zone.
        assert!(matches!(
            decider_zone((10.0, 10.0), (12.4, 12.4), 1.25, f, 3),
            Decision::Zone(..)
        ));
        assert!(matches!(
            decider_zone((f64::NAN, 10.0), (100.0, 100.0), 1.0, f, 3),
            Decision::Invalide(_)
        ));
        assert!(matches!(
            decider_zone((10.0, 10.0), (5000.0, 100.0), 1.0, f, 3),
            Decision::Invalide(_)
        ));
    }

    #[test]
    fn un_seul_signal_de_fin_par_affichage() {
        let mut e = Etat::default();
        let t0 = Instant::now();
        let (id, oublie) = e.commencer(&demande(), Some(42), Some(7), t0);
        assert_eq!((id, oublie), (1, None));
        assert!(e.terminer(id + 1).is_none(), "autre id ignore");
        let a = e.terminer(id).expect("premiere fin");
        assert_eq!(a.avant, Some(42));
        assert!(e.terminer(id).is_none(), "seconde fin ignoree");
        assert!(e.prete(id, t0).is_none(), "prete apres la fin ignore");
        assert!(!e.muette(id));
        assert_eq!(e.focus(false), None);
    }

    #[test]
    fn un_nouvel_affichage_garde_la_fenetre_a_rendre() {
        let mut e = Etat::default();
        let t0 = Instant::now();
        let (a, _) = e.commencer(&demande(), Some(42), Some(7), t0);
        // La sélection elle-même est au premier plan : on garde 42.
        let (b, oublie) = e.commencer(&demande(), Some(7), Some(7), t0);
        assert_eq!(oublie, Some(a));
        assert!(e.terminer(a).is_none());
        assert_eq!(e.terminer(b).and_then(|x| x.avant), Some(42));
        // Rien au premier plan.
        let (c, _) = e.commencer(&demande(), None, Some(7), t0);
        assert_eq!(e.terminer(c).and_then(|x| x.avant), None);
    }

    #[test]
    fn prete_une_seule_fois_et_delai_muette() {
        let mut e = Etat::default();
        let t0 = Instant::now();
        let (id, _) = e.commencer(&demande(), None, None, t0);
        assert!(e.muette(id), "sans prete, le delai cache");
        assert!(!e.muette(id + 1));
        assert_eq!(
            e.prete(id, t0 + Duration::from_millis(120)),
            Some(Duration::from_millis(120))
        );
        assert_eq!(e.prete(id, t0 + Duration::from_millis(300)), None);
        assert!(!e.muette(id), "prete a temps : pas muette");
        // Un affichage suivant a son propre délai.
        let (id2, _) = e.commencer(&demande(), None, None, t0);
        assert!(!e.muette(id) && e.muette(id2));
    }

    #[test]
    fn focus_perdu_seulement_apres_l_avoir_eu() {
        let mut e = Etat::default();
        assert_eq!(e.focus(false), None, "rien en cours");
        let (id, _) = e.commencer(&demande(), None, None, Instant::now());
        assert_eq!(e.focus(false), None, "jamais eu le focus");
        assert_eq!(e.focus(true), None);
        assert_eq!(e.focus(false), Some(id));
        // Un nouvel affichage repart sans focus.
        let (id2, _) = e.commencer(&demande(), None, None, Instant::now());
        assert_eq!(e.focus(false), None);
        assert_eq!(e.focus(true), None);
        assert_eq!(e.focus(false), Some(id2));
    }

    #[test]
    fn fin_ecrite_au_journal() {
        assert_eq!(
            decrire_fin(&Signal::Prete {
                id: 1,
                duree: Duration::ZERO
            }),
            None
        );
        assert_eq!(
            decrire_fin(&Signal::Zone {
                id: 1,
                depart: (0, 0),
                arrivee: (5, 5)
            })
            .as_deref(),
            Some("zone")
        );
        assert_eq!(
            decrire_fin(&Signal::Annulee {
                id: 1,
                cause: Cause::FocusPerdu
            })
            .as_deref(),
            Some("annulee focus_perdu")
        );
        assert_eq!(
            decrire_fin(&Signal::Muette { id: 1 }).as_deref(),
            Some("muette")
        );
        assert_eq!(
            decrire_fin(&Signal::Impossible {
                id: 1,
                cause: "point invalide".into()
            })
            .as_deref(),
            Some("impossible point invalide")
        );
    }

    #[test]
    fn point_de_la_page_toujours_lisible() {
        let p: Point = serde_json::from_str(r#"{"x": 1.5, "y": 2}"#).unwrap();
        assert_eq!((p.x, p.y), (1.5, 2.0));
        let p: Point = serde_json::from_str(r#"{"x": null, "y": "a"}"#).unwrap();
        assert!(p.x.is_nan() && p.y.is_nan());
        let p: Point = serde_json::from_str("{}").unwrap();
        assert!(p.x.is_nan() && p.y.is_nan());
    }

    #[test]
    fn charge_envoyee_a_la_page() {
        let c = ChargeMontrer {
            id: 3,
            ecrans: vec![vers_page(r(0, 0, 1920, 1080), 1.5, r(0, 0, 1920, 1080))],
            cote_min: 2.0,
            largeur: 1280.0,
            hauteur: 720.0,
            echelle: 1.5,
        };
        assert_eq!(
            serde_json::to_string(&c).unwrap(),
            concat!(
                r#"{"id":3,"ecrans":[{"x":0.0,"y":0.0,"largeur":1280.0,"hauteur":720.0}],"#,
                r#""cote_min":2.0,"largeur":1280.0,"hauteur":720.0,"echelle":1.5}"#
            )
        );
    }

    #[test]
    fn arguments_identiques_et_complets() {
        assert!(ARGUMENTS_NAVIGATEUR.starts_with(
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,CalculateNativeWinOcclusion"
        ));
    }

    /// Même règle que `capture::rect_entre` (coins inclus), recopiée ici : `fenetres`
    /// n'importe pas `capture`, même dans ses tests.
    fn rect_entre(a: (i32, i32), b: (i32, i32)) -> Rect {
        Rect::depuis_bords(
            a.0.min(b.0),
            a.1.min(b.1),
            a.0.max(b.0) + 1,
            a.1.max(b.1) + 1,
        )
    }

    /// Ce que la page a dessiné, en pixels réels : `[min, max)` des bords envoyés.
    fn dessinee(depart: (f64, f64), arrivee: (f64, f64), e: f64) -> Rect {
        let b = |v: f64| (v * e).round() as i32;
        Rect::depuis_bords(
            b(depart.0.min(arrivee.0)),
            b(depart.1.min(arrivee.1)),
            b(depart.0.max(arrivee.0)),
            b(depart.1.max(arrivee.1)),
        )
    }

    #[test]
    fn zone_dessinee_egale_zone_decoupee() {
        // Contre-essai de la relecture : un rectangle dessiné de 200 × 150 était découpé en
        // 201 × 151.
        let f = r(0, 0, 1920, 1080);
        for e in [1.0, 1.25, 1.5] {
            let (g, h, d, b) = (100.0, 100.0, 300.0, 250.0);
            for (depart, arrivee) in [
                ((g, h), (d, b)),
                ((d, b), (g, h)),
                ((d, h), (g, b)),
                ((g, b), (d, h)),
            ] {
                let Decision::Zone(a, z) = decider_zone(depart, arrivee, e, f, 3) else {
                    panic!("pas une zone a {e}");
                };
                assert_eq!(
                    rect_entre(a, z),
                    dessinee(depart, arrivee, e),
                    "echelle {e}, {depart:?} -> {arrivee:?}"
                );
            }
        }
        assert_eq!(
            match decider_zone((100.0, 100.0), (300.0, 250.0), 1.0, f, 3) {
                Decision::Zone(a, b) => rect_entre(a, b),
                _ => Rect::default(),
            },
            r(100, 100, 200, 150)
        );
    }

    #[test]
    fn coin_a_coin_d_un_ecran_donne_ses_pixels() {
        // Bureau de deux écrans, celui de gauche à x négatif ; échelle 1,25.
        let bureau = r(-1920, 0, 3840, 1080);
        for ecran in [r(-1920, 0, 1920, 1080), r(0, 0, 1920, 1080)] {
            let p = vers_page(ecran, 1.25, bureau);
            for (depart, arrivee) in [
                ((p.x, p.y), (p.x + p.largeur, p.y + p.hauteur)),
                ((p.x + p.largeur, p.y + p.hauteur), (p.x, p.y)),
            ] {
                let Decision::Zone(a, b) = decider_zone(depart, arrivee, 1.25, bureau, 3) else {
                    panic!("pas une zone");
                };
                assert_eq!(rect_entre(a, b), ecran);
                assert!(ecran.contient(a) && ecran.contient(b), "coins sur l'ecran");
            }
        }
    }

    #[test]
    fn espace_le_depart_reste_sur_son_ecran() {
        // Contre-essai A : départ envoyé sur le bord droit de l'écran de gauche (640), qui
        // est le premier pixel de l'écran voisin.
        let ecran_gauche = r(0, 0, 640, 400);
        let Decision::Zone(a, b) =
            decider_zone((640.0, 100.0), (540.0, 200.0), 1.0, r(0, 0, 1280, 400), 3)
        else {
            panic!("pas une zone");
        };
        assert_eq!(a, (639, 100));
        assert!(ecran_gauche.contient(a));
        assert_eq!(rect_entre(a, b), r(540, 100, 100, 100));
    }

    #[test]
    fn seuil_compte_sur_les_bords() {
        let f = r(0, 0, 100, 100);
        assert!(matches!(
            decider_zone((10.0, 10.0), (13.0, 13.0), 1.0, f, 3),
            Decision::Zone((10, 10), (12, 12))
        ));
        assert_eq!(
            decider_zone((10.0, 10.0), (12.0, 13.0), 1.0, f, 3),
            Decision::TropPetite
        );
    }

    #[test]
    fn verrou_empoisonne_repris() {
        // Contre-essai de la relecture : un verrou empoisonné empêchait `finir`, `cacher` et
        // le délai de cacher la fenêtre.
        let m = Arc::new(Mutex::new(Etat::default()));
        let (id, _) = prendre(&m).commencer(&demande(), None, None, Instant::now());
        let m2 = Arc::clone(&m);
        let _ = std::thread::spawn(move || {
            let _g = m2.lock();
            panic!("panique sous verrou (voulue par le test)");
        })
        .join();
        assert!(m.is_poisoned());
        assert!(prendre(&m).terminer(id).is_some(), "la fin passe encore");
        assert!(!m.is_poisoned());
    }

    #[test]
    fn fermeture_nommee() {
        assert_eq!(Cause::Fermeture.nom(), "fermeture");
        assert_eq!(
            cause_depuis_nom("fermeture"),
            None,
            "jamais envoyee par la page"
        );
        assert_eq!(
            decrire_fin(&Signal::Annulee {
                id: 2,
                cause: Cause::Fermeture
            })
            .as_deref(),
            Some("annulee fermeture")
        );
    }

    #[test]
    fn la_selection_peut_etre_confiee_a_tauri() {
        fn partageable<T: Send + Sync + 'static>() {}
        partageable::<Selection>();
        partageable::<Signal>();
    }
}
