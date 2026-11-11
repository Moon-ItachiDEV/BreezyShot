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
/// Événement écouté par la page : `{ id, largeur, hauteur, ecrans: [RectFenetre], cote_min }`,
/// tout en pixels réels relatifs au coin de la fenêtre.
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

/// Rectangle en pixels réels, relatif au coin haut gauche de la fenêtre (ce que reçoit la
/// page).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RectFenetre {
    pub x: i32,
    pub y: i32,
    pub largeur: i32,
    pub hauteur: i32,
}

/// Point envoyé par la page, en pixels réels relatifs au coin haut gauche de la fenêtre.
/// Une coordonnée absente ou qui n'est pas un nombre (`null` pour un `NaN` passé par JSON)
/// devient `NaN` : la commande arrive quand même, et `vers_bureau` la refuse (signal
/// `Impossible`).
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

/// Borne un calcul fait en `i64` dans les `i32`.
fn borner_i32(v: i64) -> i32 {
    v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// Pixel réel relatif à la fenêtre (envoyé par la page) → pixel réel du bureau. `None` si
/// la donnée n'est pas un nombre fini ou sort de la fenêtre (tolérance : 1 pixel réel).
/// Le résultat est arrondi au pixel et borné à la fenêtre.
///
/// `fenetre` est le rectangle réel de la fenêtre. Un coin peut valoir le bord droit ou bas
/// (exclus) : c'est un bord de zone.
pub fn vers_bureau(point: (f64, f64), fenetre: Rect) -> Option<(i32, i32)> {
    let (x, y) = point;
    if fenetre.est_vide() || !x.is_finite() || !y.is_finite() {
        return None;
    }
    let (l, h) = (f64::from(fenetre.largeur), f64::from(fenetre.hauteur));
    if x < -TOLERANCE || y < -TOLERANCE || x > l + TOLERANCE || y > h + TOLERANCE {
        return None;
    }
    let bx = x.round().clamp(0.0, l) as i64 + i64::from(fenetre.x);
    let by = y.round().clamp(0.0, h) as i64 + i64::from(fenetre.y);
    Some((i32::try_from(bx).ok()?, i32::try_from(by).ok()?))
}

/// Rectangle réel du bureau → le même, relatif au coin de la fenêtre (simple changement
/// d'origine, pour envoyer les écrans à la page).
pub fn vers_page(rect: Rect, fenetre: Rect) -> RectFenetre {
    RectFenetre {
        x: borner_i32(i64::from(rect.x) - i64::from(fenetre.x)),
        y: borner_i32(i64::from(rect.y) - i64::from(fenetre.y)),
        largeur: rect.largeur.max(0),
        hauteur: rect.hauteur.max(0),
    }
}

/// Cause d'annulation envoyée par la page (`echap`, `clic_droit`). `focus_perdu` et
/// `fermeture` ne viennent jamais de la page.
pub fn cause_depuis_nom(nom: &str) -> Option<Cause> {
    match nom {
        "echap" => Some(Cause::Echap),
        "clic_droit" => Some(Cause::ClicDroit),
        _ => None,
    }
}

/// Échelle mesurée la plus petite et la plus grande admises.
const ECHELLE_MIN: f64 = 0.5;
const ECHELLE_MAX: f64 = 5.0;
/// Écart relatif admis entre l'échelle horizontale et la verticale.
const ECART_ECHELLES: f64 = 0.02;

/// Contrôle de cohérence de `selection_prete` (fonction pure) : l'échelle mesurée
/// `k = taille réelle de la fenêtre / taille de la page en pixels CSS`, sur chaque axe.
/// Rend `(kx, ky)`, ou la cause « page <l>x<h> pour fenetre <L>x<H> » si une valeur n'est
/// pas finie ou est nulle, si `k` sort de [0,5 ; 5], ou si `kx` et `ky` diffèrent de plus
/// de 2 %.
pub fn controler_page(page: (f64, f64), fenetre: Rect) -> Result<(f64, f64), String> {
    let (l, h) = page;
    let cause = || {
        format!(
            "page {l}x{h} pour fenetre {}x{}",
            fenetre.largeur, fenetre.hauteur
        )
    };
    if !(l.is_finite() && h.is_finite() && l > 0.0 && h > 0.0) || fenetre.est_vide() {
        return Err(cause());
    }
    let kx = f64::from(fenetre.largeur) / l;
    let ky = f64::from(fenetre.hauteur) / h;
    let dans = |k: f64| (ECHELLE_MIN..=ECHELLE_MAX).contains(&k);
    if !dans(kx) || !dans(ky) || (kx - ky).abs() > ECART_ECHELLES * kx.max(ky) {
        return Err(cause());
    }
    Ok((kx, ky))
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

/// Décide d'un `selection_terminee` (fonction pure). `fenetre` : rectangle réel de la
/// fenêtre ; `depart`, `arrivee` : pixels réels relatifs à la fenêtre.
///
/// La page envoie des **bords** : la zone dessinée couvre `[min, max)` sur chaque axe. Le
/// seuil se compte sur ces bords (`|a − b| >= cote_min`) ; la zone rendue donne des coins
/// de pixels inclus : sur chaque axe, le côté max perd 1, le sens du glisser est gardé.
pub fn decider_zone(
    depart: (f64, f64),
    arrivee: (f64, f64),
    fenetre: Rect,
    cote_min: i32,
) -> Decision {
    let (Some(a), Some(b)) = (vers_bureau(depart, fenetre), vers_bureau(arrivee, fenetre)) else {
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

/// Ce que reçoit la page avec `selection:montrer` : tout en pixels réels, relatifs au coin
/// haut gauche de la fenêtre. Aucune échelle : la page la mesure.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct ChargeMontrer {
    id: u64,
    /// Taille réelle de la fenêtre (rectangle relu auprès du système).
    largeur: i32,
    hauteur: i32,
    ecrans: Vec<RectFenetre>,
    cote_min: i32,
}

/// La charge d'un affichage pour la fenêtre `fenetre` (fonction pure).
fn charge_pour(id: u64, fenetre: Rect, ecrans: &[Rect], cote_min: i32) -> ChargeMontrer {
    ChargeMontrer {
        id,
        largeur: fenetre.largeur.max(0),
        hauteur: fenetre.hauteur.max(0),
        ecrans: ecrans.iter().map(|r| vers_page(*r, fenetre)).collect(),
        cote_min: cote_min.max(0),
    }
}

/// Un affichage en cours, vu du cœur.
#[derive(Debug, Clone)]
struct Affichage {
    id: u64,
    /// Rectangle réel de la fenêtre (repère des coins de cet affichage).
    fenetre: Rect,
    /// Écrans demandés, en pixels réels du bureau (pour refaire la charge).
    ecrans: Vec<Rect>,
    /// Échelle annoncée par tao, seulement pour le journal.
    echelle_tao: f64,
    cote_min: i32,
    montre_a: Instant,
    /// La fenêtre au premier plan avant l'affichage, à qui le rendre.
    avant: Option<isize>,
    prete: bool,
    a_eu_focus: bool,
    /// Ce qui a été envoyé à la page, renvoyé si elle se recharge, si une zone trop petite
    /// lui est refusée ou si la fenêtre a changé de taille (elle revient alors au repos).
    charge: Option<ChargeMontrer>,
}

impl Affichage {
    fn nouveau(id: u64, demande: &Demande, avant: Option<isize>, maintenant: Instant) -> Self {
        Affichage {
            id,
            fenetre: demande.bureau,
            ecrans: demande.ecrans.clone(),
            echelle_tao: f64::NAN,
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
        // L'échelle de tao n'est que notée : celle qui sert est mesurée par la page.
        let echelle_tao = fenetre.scale_factor().unwrap_or(f64::NAN);
        let repere = reel.filter(|r| !r.est_vide()).unwrap_or(voulu);
        log::info!(
            "selection montree {id} voulu {} reel {} echelle {echelle_tao} placements {placements}",
            texte_rect(voulu),
            reel.map(texte_rect).unwrap_or_else(|| "inconnu".into())
        );
        // 5. Prévenir la page : tout en pixels réels relatifs au coin de la fenêtre.
        let charge = charge_pour(id, repere, &demande.ecrans, demande.cote_min);
        {
            let mut etat = self.etat();
            let a = etat.courant_mut(id).ok_or("affichage remplace")?;
            a.fenetre = repere;
            a.echelle_tao = echelle_tao;
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

    /// `selection_prete` de l'affichage en cours, même répété (la page le renvoie après un
    /// changement de taille). La fenêtre est relue : si elle a changé, elle est retenue et
    /// la page reçoit la nouvelle charge. Puis le contrôle de cohérence : un échec termine
    /// l'affichage (`Impossible`), sauf juste après un changement de la fenêtre (la page,
    /// remise au repos, renverra `selection_prete`). `Prete` n'est signalé qu'une fois.
    fn prete(self: &Arc<Self>, id: u64, page: (f64, f64, f64)) {
        let Some((avant, ecrans, cote_min, tao)) = self
            .etat()
            .courant(id)
            .map(|a| (a.fenetre, a.ecrans.clone(), a.cote_min, a.echelle_tao))
        else {
            return;
        };
        let nouvelle = self
            .identifiant
            .and_then(rect_reel)
            .filter(|r| !r.est_vide() && *r != avant);
        let fenetre = nouvelle.unwrap_or(avant);
        if let Some(r) = nouvelle {
            let charge = charge_pour(id, r, &ecrans, cote_min);
            {
                let mut etat = self.etat();
                let Some(a) = etat.courant_mut(id) else {
                    return;
                };
                a.fenetre = r;
                a.charge = Some(charge.clone());
            }
            log::info!(
                "selection {id}: fenetre changee {} -> {}",
                texte_rect(avant),
                texte_rect(r)
            );
            self.renvoyer(charge);
        }
        let (largeur, hauteur, dpr) = page;
        match controler_page((largeur, hauteur), fenetre) {
            Ok((kx, _)) => {
                let duree = self.etat().prete(id, Instant::now());
                if let Some(duree) = duree {
                    log::info!(
                        "chrono selection prete {} page {largeur}x{hauteur} echelle {kx} page-dpr {dpr} tao {tao}",
                        duree.as_millis()
                    );
                    (self.rappel)(Signal::Prete { id, duree });
                }
            }
            Err(cause) if nouvelle.is_some() => {
                log::info!("selection {id}: page a recontroler ({cause})");
            }
            Err(cause) => self.finir(Signal::Impossible { id, cause }),
        }
    }

    fn terminee(self: &Arc<Self>, id: u64, depart: Point, arrivee: Point) {
        let Some((fenetre, cote_min, charge)) = self
            .etat()
            .courant(id)
            .map(|a| (a.fenetre, a.cote_min, a.charge.clone()))
        else {
            return;
        };
        match decider_zone(
            (depart.x, depart.y),
            (arrivee.x, arrivee.y),
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

/// La page est visible et peinte, et sa taille est stable depuis deux images. `largeur`,
/// `hauteur` : `innerWidth`, `innerHeight` (pixels CSS), d'où l'échelle mesurée
/// (`controler_page`) ; `echelle` : `devicePixelRatio`, seulement écrit au journal.
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

/// Zone choisie : les deux bords finals, en pixels réels relatifs au coin de la fenêtre.
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

    /// Ce que la page a dessiné : `[min, max)` des bords envoyés (pixels réels relatifs),
    /// placé dans le bureau.
    fn dessinee(depart: (f64, f64), arrivee: (f64, f64), fenetre: Rect) -> Rect {
        let b = |v: f64| v.round() as i32;
        Rect::depuis_bords(
            fenetre.x + b(depart.0.min(arrivee.0)),
            fenetre.y + b(depart.1.min(arrivee.1)),
            fenetre.x + b(depart.0.max(arrivee.0)),
            fenetre.y + b(depart.1.max(arrivee.1)),
        )
    }

    fn zone(d: Decision) -> Rect {
        match d {
            Decision::Zone(a, b) => rect_entre(a, b),
            autre => panic!("pas une zone : {autre:?}"),
        }
    }

    #[test]
    fn vers_bureau_change_l_origine() {
        let f = r(-1920, -200, 3840, 1280);
        assert_eq!(vers_bureau((0.0, 0.0), f), Some((-1920, -200)));
        assert_eq!(vers_bureau((1920.0, 200.0), f), Some((0, 0)));
        assert_eq!(vers_bureau((3840.0, 1280.0), f), Some((1920, 1080)));
        assert_eq!(vers_bureau((10.4, 10.6), f), Some((-1910, -189)));
    }

    #[test]
    fn vers_bureau_bornes_et_tolerance() {
        let f = r(10, 20, 100, 50);
        // Les bords exclus sont des coins valides.
        assert_eq!(vers_bureau((100.0, 50.0), f), Some((110, 70)));
        // Jusqu'à 1 pixel réel dehors : ramené au bord.
        assert_eq!(vers_bureau((-1.0, -0.5), f), Some((10, 20)));
        assert_eq!(vers_bureau((101.0, 51.0), f), Some((110, 70)));
        // Au-delà : refusé.
        assert_eq!(vers_bureau((-1.01, 0.0), f), None);
        assert_eq!(vers_bureau((0.0, 51.01), f), None);
    }

    #[test]
    fn vers_bureau_refuse_ce_qui_n_est_pas_un_nombre() {
        let f = r(0, 0, 100, 100);
        assert_eq!(vers_bureau((f64::NAN, 1.0), f), None);
        assert_eq!(vers_bureau((1.0, f64::INFINITY), f), None);
        assert_eq!(vers_bureau((f64::NEG_INFINITY, 1.0), f), None);
        assert_eq!(vers_bureau((1.0, 1.0), r(0, 0, 0, 10)), None);
        assert_eq!(vers_bureau((1e300, 1.0), f), None);
    }

    #[test]
    fn vers_page_et_aller_retour() {
        let f = r(-1920, 0, 5760, 1080);
        for ecran in [
            r(-1920, 0, 1920, 1080),
            r(0, 0, 1920, 1080),
            r(1920, 0, 1920, 1080),
        ] {
            let p = vers_page(ecran, f);
            let (x, y) = (f64::from(p.x), f64::from(p.y));
            let (l, h) = (f64::from(p.largeur), f64::from(p.hauteur));
            assert_eq!(vers_bureau((x, y), f), Some((ecran.x, ecran.y)));
            assert_eq!(
                vers_bureau((x + l, y + h), f),
                Some((ecran.x + ecran.largeur, ecran.y + ecran.hauteur))
            );
        }
        assert_eq!(
            vers_page(r(0, 0, 1920, 1080), f),
            RectFenetre {
                x: 1920,
                y: 0,
                largeur: 1920,
                hauteur: 1080
            }
        );
    }

    #[test]
    fn echelle_mesuree_et_coins_exacts() {
        // La page mesure k = taille réelle / taille CSS, arrondit au pixel réel et envoie des
        // pixels réels : la zone découpée est celle dessinée, à tout k (même non rond).
        for k in [1.0_f64, 1.25, 1.5, 2.0, 1.1] {
            let (lc, hc): (f64, f64) = (1280.0, 400.0);
            let f = r(-500, 30, (lc * k).round() as i32, (hc * k).round() as i32);
            let (kx, ky) = controler_page((lc, hc), f).expect("page coherente");
            assert!((kx - k).abs() < 1e-9 && (ky - k).abs() < 1e-9, "k {k}");
            let reel = |css: f64, k: f64| (css * k).round();
            for (d, a) in [
                ((100.3, 50.6), (301.7, 250.2)),
                ((301.7, 250.2), (100.3, 50.6)),
                ((0.0, 0.0), (lc, hc)),
            ] {
                let depart = (reel(d.0, kx), reel(d.1, ky));
                let arrivee = (reel(a.0, kx), reel(a.1, ky));
                assert_eq!(
                    zone(decider_zone(depart, arrivee, f, 3)),
                    dessinee(depart, arrivee, f),
                    "k {k}"
                );
            }
        }
    }

    #[test]
    fn controle_de_coherence_de_la_page() {
        let f = r(0, 0, 1600, 500);
        assert!(controler_page((1280.0, 400.0), f).is_ok());
        // Une page de taille CSS quelconque donne son échelle, quel que soit le DPR.
        assert!(controler_page((1066.0, 333.0), f).is_ok());
        // kx et ky à moins de 2 % : accepté ; au-delà : refusé.
        assert!(controler_page((1280.0, 395.0), f).is_ok());
        assert_eq!(
            controler_page((1280.0, 380.0), f),
            Err("page 1280x380 pour fenetre 1600x500".into())
        );
        for page in [
            (0.0, 400.0),
            (1280.0, 0.0),
            (-1280.0, 400.0),
            (f64::NAN, 400.0),
            (1280.0, f64::INFINITY),
            (100.0, 31.25),   // k = 16
            (6400.0, 2000.0), // k = 0,25
        ] {
            assert!(controler_page(page, f).is_err(), "{page:?}");
        }
        assert!(controler_page((1280.0, 400.0), r(0, 0, 0, 500)).is_err());
        // Bornes de k incluses.
        assert!(controler_page((3200.0, 1000.0), f).is_ok());
        assert!(controler_page((320.0, 100.0), f).is_ok());
    }

    #[test]
    fn decision_d_une_zone() {
        let f = r(0, 0, 1920, 1080);
        assert_eq!(
            decider_zone((10.0, 10.0), (110.0, 60.0), f, 3),
            Decision::Zone((10, 10), (109, 59))
        );
        // Sens inverse : le sens est gardé, le bord max (ici le départ) recule d'un pixel.
        assert_eq!(
            decider_zone((110.0, 60.0), (10.0, 10.0), f, 3),
            Decision::Zone((109, 59), (10, 10))
        );
        assert_eq!(
            decider_zone((10.0, 10.0), (12.0, 200.0), f, 3),
            Decision::TropPetite
        );
        assert_eq!(
            decider_zone((10.0, 10.0), (10.0, 10.0), f, 3),
            Decision::TropPetite
        );
        assert!(matches!(
            decider_zone((f64::NAN, 10.0), (100.0, 100.0), f, 3),
            Decision::Invalide(_)
        ));
        assert!(matches!(
            decider_zone((10.0, 10.0), (5000.0, 100.0), f, 3),
            Decision::Invalide(_)
        ));
    }

    #[test]
    fn zone_dessinee_egale_zone_decoupee() {
        // Contre-essai de la relecture : un rectangle dessiné de 200 × 150 était découpé en
        // 201 × 151.
        let f = r(-300, 40, 1920, 1080);
        let (g, h, d, b) = (100.0, 100.0, 300.0, 250.0);
        for (depart, arrivee) in [
            ((g, h), (d, b)),
            ((d, b), (g, h)),
            ((d, h), (g, b)),
            ((g, b), (d, h)),
        ] {
            assert_eq!(
                zone(decider_zone(depart, arrivee, f, 3)),
                dessinee(depart, arrivee, f)
            );
        }
        assert_eq!(
            zone(decider_zone((g, h), (d, b), r(0, 0, 1920, 1080), 3)),
            r(100, 100, 200, 150)
        );
    }

    #[test]
    fn coin_a_coin_d_un_ecran_donne_ses_pixels() {
        let f = r(-1920, 0, 3840, 1080);
        for ecran in [r(-1920, 0, 1920, 1080), r(0, 0, 1920, 1080)] {
            let p = vers_page(ecran, f);
            let (x0, y0) = (f64::from(p.x), f64::from(p.y));
            let (x1, y1) = (x0 + f64::from(p.largeur), y0 + f64::from(p.hauteur));
            for (depart, arrivee) in [((x0, y0), (x1, y1)), ((x1, y1), (x0, y0))] {
                let Decision::Zone(a, b) = decider_zone(depart, arrivee, f, 3) else {
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
        let Decision::Zone(a, b) =
            decider_zone((640.0, 100.0), (540.0, 200.0), r(0, 0, 1280, 400), 3)
        else {
            panic!("pas une zone");
        };
        assert_eq!(a, (639, 100));
        assert!(r(0, 0, 640, 400).contient(a));
        assert_eq!(rect_entre(a, b), r(540, 100, 100, 100));
    }

    #[test]
    fn seuil_compte_sur_les_bords() {
        let f = r(0, 0, 100, 100);
        assert_eq!(
            decider_zone((10.0, 10.0), (13.0, 13.0), f, 3),
            Decision::Zone((10, 10), (12, 12))
        );
        assert_eq!(
            decider_zone((10.0, 10.0), (12.0, 13.0), f, 3),
            Decision::TropPetite
        );
    }

    #[test]
    fn charge_envoyee_a_la_page() {
        let c = charge_pour(3, r(-1920, 0, 3520, 1080), &[r(0, 0, 1600, 900)], 3);
        assert_eq!(
            serde_json::to_string(&c).unwrap(),
            concat!(
                r#"{"id":3,"largeur":3520,"hauteur":1080,"#,
                r#""ecrans":[{"x":1920,"y":0,"largeur":1600,"hauteur":900}],"cote_min":3}"#
            )
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
