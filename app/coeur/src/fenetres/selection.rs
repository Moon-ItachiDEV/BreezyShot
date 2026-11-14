//! Sélection (étapes 5 et 8) : choisir une zone de l'écran, ou une fenêtre.
//!
//! Une seule fenêtre Tauri transparente, préparée au démarrage et cachée. Montrée, elle
//! couvre tout le bureau virtuel juste au-dessus de l'écran figé (fenêtre native de
//! `systeme::gel`) et ne dessine que le voile et le rectangle (mode zone), ou le contour de
//! la fenêtre sous la souris (mode fenêtre). Elle ne reçoit jamais les pixels de la capture
//! et ne sait pas ce que deviendra le choix : elle rend deux coins (`Signal::Zone`) ou une
//! fenêtre de la liste et le point cliqué (`Signal::Fenetre`), en pixels réels du bureau, ou
//! une annulation, par `Signal`.
//!
//! # Ce que l'Application doit faire à l'assemblage
//!
//! 1. Déclarer les cinq commandes dans `invoke_handler` :
//!    `fenetres::selection::{selection_chargee, selection_prete, selection_terminee,
//!    selection_cliquee, selection_annulee}` (elles sont déjà permises à la seule fenêtre
//!    `selection` par `capabilities/selection.json` et `build.rs`).
//! 2. Au démarrage, sur le fil principal, après `systeme::gel::preparer_ecran_fige` :
//!    `Selection::creer(app, rappel)`, puis `app.manage(selection)` (les commandes
//!    trouvent la sélection par `State<Selection>`).
//! 3. Pour une capture de zone, sur le fil principal : figer et montrer l'écran figé, puis
//!    `selection.montrer(Demande { bureau: <rect de la photo>, ecrans: <rect de chaque
//!    écran>, dessous: systeme::gel::poignee_ecran_fige(), cote_min: capture::COTE_MIN,
//!    mode: Mode::Zone, fenetres: Vec::new() })`. Pour une capture de fenêtre : `mode:
//!    Mode::Fenetre` et `fenetres` : les fenêtres du premier plan vers le fond, chacune avec
//!    son rectangle et son rayon de coins (pixels réels).
//!    Sur `Err` : la fenêtre est déjà cachée ; cacher l'écran figé et revenir au Repos.
//! 4. Pendant que la sélection est montrée (de `montrer` jusqu'au signal de fin), réserver
//!    Échap en global (`raccourcis`) : à l'appui, `selection.cacher()` puis cacher l'écran
//!    figé et revenir au Repos (aucun signal ne vient de la sélection dans ce cas). Libérer
//!    Échap à chaque fin.
//! 5. Dans le rappel (fil principal pour les signaux de fin, dans le même tour que le
//!    masquage de la fenêtre) : `Zone` → cacher l'écran figé tout de suite, puis
//!    `capture::rect_entre(depart, arrivee)`, borner à l'écran du départ et découper hors
//!    du fil principal ; `Fenetre { indice, point }` → de même, avec la fenêtre `indice` de
//!    la demande coupée à l'écran de `point` ; `Annulee`, `Muette`, `Impossible` → cacher l'écran figé, Repos
//!    (`Impossible` : ligne de journal déjà écrite ici, message si utile). `Prete` : rien
//!    d'obligatoire (le chrono est déjà au journal). Le rappel ne doit pas bloquer.
//! 6. À l'arrêt : `selection.fermer()`, avant `systeme::gel::detruire_ecran_fige()`.
//!
//! # Règles
//!
//! - Un seul signal de fin par affichage (`Zone`, `Fenetre`, `Annulee`, `Muette`,
//!   `Impossible`) : le
//!   premier gagne. Toute commande dont l'`id` n'est pas celui en cours est ignorée.
//! - Avant le rappel d'un signal de fin : le premier plan est rendu à la fenêtre notée
//!   (avant le masquage, tant que l'app a encore le droit de le donner), la fenêtre est
//!   cachée, puis `selection:cachee` part vers la page.
//! - Mode texte (0.5) : la zone choisie laisse la fenêtre montrée (le premier plan n'est
//!   pas encore rendu) pour le balayage (`balayer`) ; `cacher` la cache ensuite.
//! - Mode défilante (0.6) : tracer (`Signal::Zone`) **ou** cliquer sur une page proposée
//!   (`Signal::Fenetre`, indice dans la liste de `proposer`). Les pages arrivent après
//!   `montrer` (ou jamais) : `proposer` les pose sur l'affichage en cours, même numéro de
//!   charge (le repère ne change pas : un geste compté avant reste valable), et la page les
//!   prend sans perdre son geste. L'une ou l'autre fin laisse la fenêtre montrée, comme le
//!   mode texte (`garder_si_defilante`), jusqu'à `cacher`.
//! - Mode film (0.7, vidéo et GIF) : exactement le mode zone (seul `selection_terminee`
//!   est permis, aucune fenêtre proposée), avec son nom accessible ; la zone choisie laisse
//!   la fenêtre montrée (`garder_si_film`) jusqu'à `cacher`, le temps que la fenêtre de
//!   contrôle du film paraisse.
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
/// Événement écouté par la page : `{ id, numero, largeur, hauteur, ecrans: [RectFenetre],
/// cote_min, mode, fenetres: [{ x, y, largeur, hauteur, rayon }], souris: { x, y } | null,
/// suite? }`, tout en pixels réels relatifs au coin de la fenêtre. `suite: true` (mode
/// défilante, `proposer`) : la même charge, pages ajoutées ; la page garde son geste.
pub const EVENEMENT_MONTRER: &str = "selection:montrer";
/// Événement écouté par la page : la fenêtre est cachée, retour au repos.
pub const EVENEMENT_CACHEE: &str = "selection:cachee";
/// Événement écouté par la page (Copier le texte, 0.5) : `{ id, x, y, largeur, hauteur,
/// reduites }`, la zone lue en pixels réels relatifs au coin de la fenêtre.
pub const EVENEMENT_BALAYER: &str = "selection:balayer";
/// Événement écouté par la page (Copier le texte, 13/11) : `{ id, lignes: [RectFenetre],
/// reduites, trace, pas, tenue, sortie }`, les lignes d'un texte copié en pixels réels
/// relatifs au coin de la fenêtre, et le déroulé de leur surlignage (`Surlignage`).
pub const EVENEMENT_SURLIGNER: &str = "selection:surligner";
/// Sans `selection_prete` dans ce délai, la sélection est cachée (`Signal::Muette`).
pub const DELAI_PRETE: Duration = Duration::from_secs(2);
/// Tolérance, en pixels réels, pour un point venu de la page juste au bord de la fenêtre.
const TOLERANCE: f64 = 1.0;
/// Nombre de fenêtres proposées au plus : au-delà, la liste est tronquée (les premières,
/// celles du premier plan, sont gardées).
pub const FENETRES_MAX: usize = 512;

/// Ce que l'utilisateur choisit : une zone (glisser) ou une fenêtre (clic). Propre à la
/// sélection, sans lien avec les modes de l'Application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Zone,
    Fenetre,
    /// Copier le texte (0.5) : une zone, comme `Zone`, avec le nom et le voile du texte.
    Texte,
    /// Capture défilante (0.6) : tracer une zone, comme `Zone`, **ou** cliquer sur une page
    /// proposée en retard (`Selection::proposer`), comme `Fenetre`.
    Defilante,
    /// Vidéo et GIF (0.7) : une zone, comme `Zone`, avec le nom « Enregistrer l'écran :
    /// sélectionnez une zone » ; la zone choisie reste montrée jusqu'à `cacher`.
    Film,
}

impl Mode {
    /// Nom écrit au journal et envoyé à la page : `zone`, `fenetre`.
    pub fn nom(self) -> &'static str {
        match self {
            Mode::Zone => "zone",
            Mode::Fenetre => "fenetre",
            Mode::Texte => "texte",
            Mode::Defilante => "defilante",
            Mode::Film => "film",
        }
    }
}

/// Une fenêtre proposée en mode fenêtre, en pixels réels du bureau.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FenetreProposee {
    /// Le vrai bord de la fenêtre (sans l'ombre). Peut déborder du bureau.
    pub rect: Rect,
    /// Rayon de ses coins arrondis, en pixels réels (0 : coins droits).
    pub rayon: i32,
}

/// Ce que l'Application demande d'afficher.
#[derive(Debug, Clone, PartialEq)]
pub struct Demande {
    /// Le bureau virtuel à couvrir (celui de la photo), en pixels réels.
    pub bureau: Rect,
    /// Les rectangles des écrans, en pixels réels.
    pub ecrans: Vec<Rect>,
    /// Poignée de la fenêtre figée, à garder juste en dessous.
    pub dessous: Option<isize>,
    /// Côté minimal d'une zone, en pixels réels (`capture::COTE_MIN`). En mode fenêtre :
    /// côté minimal de la fenêtre coupée à l'écran de la souris.
    pub cote_min: i32,
    pub mode: Mode,
    /// Du premier plan vers le fond. Vide en mode zone (ignorée si elle ne l'est pas) ;
    /// tronquée à `FENETRES_MAX`.
    pub fenetres: Vec<FenetreProposee>,
}

/// Les fenêtres retenues d'une demande : aucune en mode zone, les `FENETRES_MAX` premières
/// en mode fenêtre. `Signal::Fenetre::indice` est un indice de cette liste, qui est aussi
/// celui de `Demande::fenetres`.
fn fenetres_de(demande: &Demande) -> Vec<FenetreProposee> {
    match demande.mode {
        // Défilante : les pages arrivent après, par `Selection::proposer`.
        Mode::Zone | Mode::Texte | Mode::Defilante | Mode::Film => Vec::new(),
        Mode::Fenetre => demande
            .fenetres
            .iter()
            .take(FENETRES_MAX)
            .copied()
            .collect(),
    }
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
    /// Fenêtre choisie (mode fenêtre) : `indice` dans `Demande::fenetres`, et le pixel
    /// cliqué, en pixels réels du bureau (il désigne l'écran où couper la fenêtre).
    Fenetre {
        id: u64,
        indice: usize,
        point: (i32, i32),
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
            | Signal::Fenetre { id, .. }
            | Signal::Annulee { id, .. }
            | Signal::Muette { id }
            | Signal::Impossible { id, .. } => *id,
        }
    }
}

/// Fin d'un affichage telle qu'écrite au journal : `zone`, `fenetre`, `annulee echap`, `muette`,
/// `impossible <cause>`. `None` pour `Prete`, qui n'est pas une fin.
pub fn decrire_fin(signal: &Signal) -> Option<String> {
    match signal {
        Signal::Prete { .. } => None,
        Signal::Zone { .. } => Some("zone".into()),
        Signal::Fenetre { .. } => Some("fenetre".into()),
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
/// Le résultat est arrondi au pixel et borné aux pixels de la fenêtre
/// (`[0, largeur − 1] × [0, hauteur − 1]`).
///
/// `fenetre` est le rectangle réel de la fenêtre.
pub fn vers_bureau(point: (f64, f64), fenetre: Rect) -> Option<(i32, i32)> {
    let (x, y) = point;
    if fenetre.est_vide() || !x.is_finite() || !y.is_finite() {
        return None;
    }
    let (dx, dy) = (
        f64::from(fenetre.largeur) - 1.0,
        f64::from(fenetre.hauteur) - 1.0,
    );
    if x < -TOLERANCE || y < -TOLERANCE || x > dx + TOLERANCE || y > dy + TOLERANCE {
        return None;
    }
    let bx = x.round().clamp(0.0, dx) as i64 + i64::from(fenetre.x);
    let by = y.round().clamp(0.0, dy) as i64 + i64::from(fenetre.y);
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

/// Contrôle de cohérence de `selection_prete` (fonction pure). La page envoie sa taille CSS
/// `page` et l'échelle `echelle = (kx, ky)` qu'elle a retenue (pixels réels par pixel CSS :
/// `devicePixelRatio` dans le cas normal, sinon la taille réelle divisée par sa taille CSS
/// non arrondie). Accepté si tout est fini et non nul, si `k` est dans [0,5 ; 5], si `kx`
/// et `ky` diffèrent de 2 % au plus, et si l'échelle retenue explique la taille réelle de la
/// fenêtre à 1 pixel CSS près sur chaque axe. Rend `(kx, ky)`, ou la cause
/// « page <l>x<h> pour fenetre <L>x<H> ».
pub fn controler_page(
    page: (f64, f64),
    echelle: (f64, f64),
    fenetre: Rect,
) -> Result<(f64, f64), String> {
    let (l, h) = page;
    let (kx, ky) = echelle;
    let cause = || {
        format!(
            "page {l}x{h} pour fenetre {}x{}",
            fenetre.largeur, fenetre.hauteur
        )
    };
    let positif = |v: f64| v.is_finite() && v > 0.0;
    if ![l, h, kx, ky].into_iter().all(positif) || fenetre.est_vide() {
        return Err(cause());
    }
    let dans = |k: f64| (ECHELLE_MIN..=ECHELLE_MAX).contains(&k);
    if !dans(kx) || !dans(ky) || (kx - ky).abs() > ECART_ECHELLES * kx.max(ky) {
        return Err(cause());
    }
    let explique = |reel: i32, css: f64, k: f64| (f64::from(reel) / k - css).abs() <= TOLERANCE;
    if !explique(fenetre.largeur, l, kx) || !explique(fenetre.hauteur, h, ky) {
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
/// La page envoie des **pixels**, inclus : celui sous le curseur à l'appui et celui sous le
/// curseur au relâchement (la zone dessinée couvre `[min, max + 1[` sur chaque axe). Un
/// côté vaut `|a − b| + 1`, comparé à `cote_min` ; les coins sont rendus tels quels, dans
/// le sens du glisser.
pub fn decider_zone(
    depart: (f64, f64),
    arrivee: (f64, f64),
    fenetre: Rect,
    cote_min: i32,
) -> Decision {
    let (Some(a), Some(b)) = (vers_bureau(depart, fenetre), vers_bureau(arrivee, fenetre)) else {
        return Decision::Invalide("point invalide".into());
    };
    let largeur = (i64::from(a.0) - i64::from(b.0)).abs() + 1;
    let hauteur = (i64::from(a.1) - i64::from(b.1)).abs() + 1;
    let min = i64::from(cote_min.max(0));
    if largeur < min || hauteur < min {
        return Decision::TropPetite;
    }
    Decision::Zone(a, b)
}

/// Ce que vaut un `selection_cliquee` pour l'affichage en cours.
#[derive(Debug, Clone, PartialEq)]
pub enum DecisionFenetre {
    /// La fenêtre `indice` de la liste, cliquée au pixel `point` (pixels réels du bureau).
    Fenetre { indice: usize, point: (i32, i32) },
    /// Aucune fenêtre sous le point, ou une fenêtre trop petite une fois coupée à l'écran
    /// du point : rien n'est choisi, la sélection continue.
    Rien,
    /// Point non fini ou hors de la fenêtre de sélection.
    Invalide(String),
}

/// Décide d'un `selection_cliquee` (fonction pure). `point` : pixel réel relatif à la
/// fenêtre de sélection, dont `fenetre_selection` est le rectangle réel ; `fenetres` : du
/// premier plan vers le fond, en pixels réels du bureau.
///
/// Le cœur ne croit pas la page : il reçoit un point, pas une fenêtre. La fenêtre choisie
/// est la première de la liste qui contient le point ; coupée à l'écran du point, elle doit
/// garder `cote_min` de côté (sinon `Rien`, sans chercher plus loin dans la liste, comme la
/// page). Un point sur aucun écran ou sur aucune fenêtre : `Rien`.
pub fn decider_fenetre(
    point: (f64, f64),
    fenetre_selection: Rect,
    fenetres: &[FenetreProposee],
    ecrans: &[Rect],
    cote_min: i32,
) -> DecisionFenetre {
    let Some(p) = vers_bureau(point, fenetre_selection) else {
        return DecisionFenetre::Invalide("point invalide".into());
    };
    let Some(indice) = fenetres.iter().position(|f| f.rect.contient(p)) else {
        return DecisionFenetre::Rien;
    };
    let Some(ecran) = ecrans.iter().find(|e| e.contient(p)) else {
        return DecisionFenetre::Rien;
    };
    let min = cote_min.max(0);
    match fenetres[indice].rect.intersection(*ecran) {
        Some(c) if c.largeur >= min && c.hauteur >= min => {
            DecisionFenetre::Fenetre { indice, point: p }
        }
        _ => DecisionFenetre::Rien,
    }
}

/// Garde-fou : une commande de fin qui ne va pas avec le mode de l'affichage (fonction
/// pure). `Err` porte la cause du signal `Impossible`.
fn garde_mode(mode: Mode, attendu: Mode) -> Result<(), String> {
    match (mode, attendu) {
        (Mode::Zone, Mode::Fenetre) => Err("clic en mode zone".into()),
        (Mode::Fenetre, Mode::Zone) => Err("zone en mode fenetre".into()),
        (Mode::Texte, Mode::Fenetre) => Err("clic en mode texte".into()),
        (Mode::Film, Mode::Fenetre) => Err("clic en mode film".into()),
        _ => Ok(()),
    }
}

/// Une fenêtre proposée, telle que la reçoit la page : pixels réels relatifs au coin de la
/// fenêtre de sélection (elle peut en déborder, coordonnées négatives comprises).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
struct FenetrePage {
    x: i32,
    y: i32,
    largeur: i32,
    hauteur: i32,
    rayon: i32,
}

/// Pixel réel relatif au coin de la fenêtre de sélection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
struct PointPage {
    x: i32,
    y: i32,
}

/// La position de la souris (pixels réels du bureau, `systeme::position_souris`) → le pixel
/// relatif à la fenêtre `fenetre` ; `None` si elle est inconnue ou hors de la fenêtre.
fn souris_pour(position: Option<(f64, f64)>, fenetre: Rect) -> Option<PointPage> {
    let (x, y) = position?;
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    let (px, py) = (x.floor(), y.floor());
    let dans_i32 = |v: f64| v >= f64::from(i32::MIN) && v <= f64::from(i32::MAX);
    if !dans_i32(px) || !dans_i32(py) {
        return None;
    }
    let p = (px as i32, py as i32);
    if !fenetre.contient(p) {
        return None;
    }
    Some(PointPage {
        x: borner_i32(i64::from(p.0) - i64::from(fenetre.x)),
        y: borner_i32(i64::from(p.1) - i64::from(fenetre.y)),
    })
}

/// Ce que reçoit la page avec `selection:montrer` : tout en pixels réels, relatifs au coin
/// haut gauche de la fenêtre. Aucune échelle : la page la mesure.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct ChargeMontrer {
    id: u64,
    /// Numéro de la charge, croissant à chaque charge (fenêtre relue). La page le renvoie
    /// avec `selection_prete`, `selection_terminee` et `selection_annulee`.
    numero: u64,
    /// Taille réelle de la fenêtre (rectangle relu auprès du système).
    largeur: i32,
    hauteur: i32,
    ecrans: Vec<RectFenetre>,
    cote_min: i32,
    mode: Mode,
    /// Vide en mode zone.
    fenetres: Vec<FenetrePage>,
    /// La souris relue à chaque envoi (`avec_souris`), pour entourer la bonne fenêtre avant
    /// tout mouvement ; `null` si elle est inconnue ou hors de la fenêtre.
    souris: Option<PointPage>,
    /// Vrai seulement pour l'envoi de `proposer` (mode défilante) : même affichage, même
    /// numéro, seules les fenêtres (les pages) changent ; la page ne revient pas au repos.
    /// Jamais gardé dans la charge courante (un renvoi remet la page au repos). Absent du
    /// JSON quand il est faux.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    suite: bool,
    /// Le rectangle réel de la fenêtre, repère de cette charge (non envoyé).
    #[serde(skip)]
    repere: Rect,
}

impl ChargeMontrer {
    /// La même charge, avec la position de la souris `position` (pixels réels du bureau).
    fn avec_souris(mut self, position: Option<(f64, f64)>) -> ChargeMontrer {
        self.souris = souris_pour(position, self.repere);
        self
    }
}

/// La charge d'un affichage pour la fenêtre `fenetre` (fonction pure). La souris n'y est
/// pas encore : elle est relue à chaque envoi.
fn charge_pour(
    id: u64,
    numero: u64,
    fenetre: Rect,
    ecrans: &[Rect],
    cote_min: i32,
    mode: Mode,
    fenetres: &[FenetreProposee],
) -> ChargeMontrer {
    ChargeMontrer {
        id,
        numero,
        largeur: fenetre.largeur.max(0),
        hauteur: fenetre.hauteur.max(0),
        ecrans: ecrans.iter().map(|r| vers_page(*r, fenetre)).collect(),
        cote_min: cote_min.max(0),
        mode,
        fenetres: fenetres
            .iter()
            .map(|f| {
                let r = vers_page(f.rect, fenetre);
                FenetrePage {
                    x: r.x,
                    y: r.y,
                    largeur: r.largeur,
                    hauteur: r.hauteur,
                    rayon: f.rayon.max(0),
                }
            })
            .collect(),
        souris: None,
        suite: false,
        repere: fenetre,
    }
}

/// Ce que la page envoie avec `selection_prete`.
#[derive(Debug, Clone, Copy)]
struct PagePrete {
    numero: u64,
    /// Taille CSS sur laquelle l'échelle a été retenue.
    largeur: f64,
    hauteur: f64,
    /// `devicePixelRatio`, pour le journal.
    dpr: f64,
    /// Échelle retenue par la page (pixels réels par pixel CSS).
    kx: f64,
    ky: f64,
    /// Envoyé après un `resize` : la page n'est plus prête et attend une charge.
    attente: bool,
}

/// Ce que le cœur fait d'un `selection_prete` (décision pure, `Etat::repondre_prete`).
#[derive(Debug, Clone, PartialEq)]
enum ReponsePrete {
    /// Numéro d'une ancienne charge : rien (la page recevra la suivante).
    Ancienne,
    Suite {
        /// Rectangle réel retenu pour le contrôle.
        fenetre: Rect,
        /// Charge à envoyer à la page : nouvelle (fenêtre changée, numéro suivant) ou, pour
        /// une page en attente, la charge courante avec le même numéro.
        renvoi: Option<ChargeMontrer>,
        /// L'ancien rectangle, si la fenêtre a changé.
        changee: Option<Rect>,
        /// Faux pour une page en attente : elle renverra `selection_prete` après la charge.
        controler: bool,
    },
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
    mode: Mode,
    /// Fenêtres retenues (`fenetres_de`), en pixels réels du bureau.
    fenetres: Vec<FenetreProposee>,
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
            mode: demande.mode,
            fenetres: fenetres_de(demande),
            montre_a: maintenant,
            avant,
            prete: false,
            a_eu_focus: false,
            charge: None,
        }
    }
}

/// Un affichage fini et gardé à l'écran jusqu'à `cacher` : mode texte fini par sa zone (le
/// balayage, Copier le texte, 0.5), ou mode défilante fini par sa zone ou sa page (la
/// sélection reste le temps que la fenêtre de contrôle soit montrée, 0.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Gardee {
    id: u64,
    /// Seul un affichage gardé du mode texte reçoit le balayage et le surlignage.
    mode: Mode,
    /// Rectangle réel de la fenêtre (repère de la zone envoyée à la page).
    fenetre: Rect,
    /// La fenêtre au premier plan avant l'affichage, à qui le rendre.
    avant: Option<isize>,
}

/// Ce que reçoit la page avec `selection:balayer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
struct ChargeBalayer {
    id: u64,
    x: i32,
    y: i32,
    largeur: i32,
    hauteur: i32,
    reduites: bool,
}

/// Le déroulé du surlignage des lignes d'un texte copié, en millisecondes : la ligne de rang
/// `r` (à partir de 0) se tire de gauche à droite de `r × pas` à `r × pas + trace` ; toutes
/// restent `tenue`, puis la sélection s'efface en `sortie`. `pas` à 0 : toutes ensemble.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Surlignage {
    pub trace: u32,
    pub pas: u32,
    pub tenue: u32,
    pub sortie: u32,
}

impl Surlignage {
    /// Du premier trait à la fin de la sortie, pour `lignes` lignes.
    pub fn duree(&self, lignes: usize) -> Duration {
        let ecarts = u64::try_from(lignes.saturating_sub(1)).unwrap_or(u64::MAX);
        let ms = u64::from(self.trace)
            .saturating_add(u64::from(self.pas).saturating_mul(ecarts))
            .saturating_add(u64::from(self.tenue))
            .saturating_add(u64::from(self.sortie));
        Duration::from_millis(ms)
    }
}

/// Ce que reçoit la page avec `selection:surligner`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ChargeSurligner {
    id: u64,
    lignes: Vec<RectFenetre>,
    reduites: bool,
    #[serde(flatten)]
    deroule: Surlignage,
}

/// L'état de la sélection : une petite machine pure, testée sans fenêtre.
#[derive(Debug, Default)]
struct Etat {
    dernier_id: u64,
    dernier_numero: u64,
    en_cours: Option<Affichage>,
    /// Mode texte (balayage) ou défilante : l'affichage fini et encore montré.
    gardee: Option<Gardee>,
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
        let gardee = self.gardee.take();
        let avant = if devant.is_some() && devant == nous {
            ancien
                .as_ref()
                .and_then(|a| a.avant)
                .or(gardee.and_then(|g| g.avant))
        } else {
            devant
        };
        self.en_cours = Some(Affichage::nouveau(id, demande, avant, maintenant));
        (id, ancien.map(|a| a.id))
    }

    /// Nouvelle charge de l'affichage `id` pour la fenêtre `fenetre` (relue) : numéro
    /// suivant ; la fenêtre et la charge sont retenues. `None` si `id` n'est plus en cours.
    fn nouvelle_charge(&mut self, id: u64, fenetre: Rect) -> Option<ChargeMontrer> {
        self.dernier_numero += 1;
        let numero = self.dernier_numero;
        let a = self.en_cours.as_mut().filter(|a| a.id == id)?;
        let charge = charge_pour(
            id,
            numero,
            fenetre,
            &a.ecrans,
            a.cote_min,
            a.mode,
            &a.fenetres,
        );
        a.fenetre = fenetre;
        a.charge = Some(charge.clone());
        Some(charge)
    }

    /// Le message de la page porte-t-il le numéro de la dernière charge de `id` ? Un ancien
    /// numéro vient d'une page qui n'a pas encore reçu la charge suivante.
    fn charge_courante(&self, id: u64, numero: u64) -> bool {
        self.courant(id)
            .and_then(|a| a.charge.as_ref())
            .is_some_and(|c| c.numero == numero)
    }

    /// Réponse à un `selection_prete` de `id` portant `numero`, le rectangle réel relu
    /// étant `relu`. Une page en attente (après un `resize`) reçoit toujours une charge.
    fn repondre_prete(
        &mut self,
        id: u64,
        numero: u64,
        attente: bool,
        relu: Option<Rect>,
    ) -> Option<ReponsePrete> {
        let avant = self.courant(id)?.fenetre;
        if !self.charge_courante(id, numero) {
            return Some(ReponsePrete::Ancienne);
        }
        let nouvelle = relu.filter(|r| !r.est_vide() && *r != avant);
        let renvoi = match nouvelle {
            Some(r) => Some(self.nouvelle_charge(id, r)?),
            None if attente => self.courant(id)?.charge.clone(),
            None => None,
        };
        Some(ReponsePrete::Suite {
            fenetre: nouvelle.unwrap_or(avant),
            renvoi,
            changee: nouvelle.map(|_| avant),
            controler: !attente,
        })
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

    /// Fin d'un affichage (déjà retiré par `terminer`) : vrai s'il reste montré, gardé pour
    /// le balayage (mode texte, zone choisie) ; sinon il est à cacher tout de suite.
    fn garder_si_texte(&mut self, affichage: &Affichage, signal: &Signal) -> bool {
        let garder = affichage.mode == Mode::Texte && matches!(signal, Signal::Zone { .. });
        if garder {
            self.garder(affichage);
        }
        garder
    }

    /// Fin d'un affichage (déjà retiré par `terminer`) : vrai s'il reste montré, gardé
    /// jusqu'à `cacher` (mode défilante, zone tracée ou page cliquée : D4, la sélection
    /// reste le temps que la fenêtre de contrôle paraisse) ; sinon il est à cacher tout de
    /// suite. Une annulation n'est jamais gardée.
    fn garder_si_defilante(&mut self, affichage: &Affichage, signal: &Signal) -> bool {
        let garder = affichage.mode == Mode::Defilante
            && matches!(signal, Signal::Zone { .. } | Signal::Fenetre { .. });
        if garder {
            self.garder(affichage);
        }
        garder
    }

    /// Fin d'un affichage (déjà retiré par `terminer`) : vrai s'il reste montré, gardé
    /// jusqu'à `cacher` (mode film, zone choisie : F2, la sélection reste le temps que la
    /// fenêtre de contrôle paraisse) ; sinon il est à cacher tout de suite. Une annulation
    /// n'est jamais gardée.
    fn garder_si_film(&mut self, affichage: &Affichage, signal: &Signal) -> bool {
        let garder = affichage.mode == Mode::Film && matches!(signal, Signal::Zone { .. });
        if garder {
            self.garder(affichage);
        }
        garder
    }

    fn garder(&mut self, affichage: &Affichage) {
        self.gardee = Some(Gardee {
            id: affichage.id,
            mode: affichage.mode,
            fenetre: affichage.fenetre,
            avant: affichage.avant,
        });
    }

    /// `proposer` (mode défilante) : les `pages` (pixels réels du bureau, les
    /// `FENETRES_MAX` premières) remplacent la liste de l'affichage `id`, et la charge
    /// courante est refaite avec elles, **même numéro** (le repère ne change pas). Rend la
    /// charge à envoyer (`suite` vrai), ou `None` : `id` n'est plus en cours, n'est pas en
    /// mode défilante, ou n'a pas encore de charge (la première, faite ensuite, portera les
    /// pages).
    /// Vrai si `id` est en cours, en mode défilante, sans charge encore faite : `proposer`
    /// retient alors la liste pour la première charge.
    fn sans_charge(&mut self, id: u64) -> bool {
        self.courant_mut(id)
            .is_some_and(|a| a.mode == Mode::Defilante && a.charge.is_none())
    }

    fn proposer(&mut self, id: u64, pages: Vec<FenetreProposee>) -> Option<ChargeMontrer> {
        let a = self.courant_mut(id).filter(|a| a.mode == Mode::Defilante)?;
        a.fenetres = pages.into_iter().take(FENETRES_MAX).collect();
        let (numero, repere) = a.charge.as_ref().map(|c| (c.numero, c.repere))?;
        let charge = charge_pour(
            id,
            numero,
            repere,
            &a.ecrans,
            a.cote_min,
            a.mode,
            &a.fenetres,
        );
        a.charge = Some(charge.clone());
        Some(ChargeMontrer {
            suite: true,
            ..charge
        })
    }

    /// `cacher` : l'affichage en cours ou gardé est oublié ; rend la fenêtre à qui rendre
    /// le premier plan.
    fn tout_oublier(&mut self) -> Option<isize> {
        let en_cours = self.en_cours.take().and_then(|a| a.avant);
        let gardee = self.gardee.take().and_then(|g| g.avant);
        en_cours.or(gardee)
    }

    /// La charge du balayage de `zone` (pixels réels du bureau) sur l'affichage gardé ;
    /// `None` s'il n'y en a pas.
    fn charge_balayer(&self, zone: Rect, reduites: bool) -> Option<ChargeBalayer> {
        let g = self.gardee.filter(|g| g.mode == Mode::Texte)?;
        let r = vers_page(zone, g.fenetre);
        Some(ChargeBalayer {
            id: g.id,
            x: r.x,
            y: r.y,
            largeur: r.largeur,
            hauteur: r.hauteur,
            reduites,
        })
    }

    /// La charge du surlignage de `lignes` (pixels réels du bureau) sur l'affichage gardé ;
    /// `None` s'il n'y en a pas, ou sans ligne.
    fn charge_surligner(
        &self,
        lignes: &[Rect],
        reduites: bool,
        deroule: Surlignage,
    ) -> Option<ChargeSurligner> {
        let g = self.gardee.filter(|g| g.mode == Mode::Texte)?;
        if lignes.is_empty() {
            return None;
        }
        Some(ChargeSurligner {
            id: g.id,
            lignes: lignes.iter().map(|&r| vers_page(r, g.fenetre)).collect(),
            reduites,
            deroule,
        })
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
                // `run_on_main_thread` depuis le fil principal s'exécute tout de suite
                // (tauri-runtime-wry 2.12.3, lib.rs:263-273) : ce n'est pas un report. Les
                // événements de focus arrivent déjà par la file de la boucle (WebView2
                // GotFocus/LostFocus → `proxy.send_event`, lib.rs:5364, 5398) ; aucun verrou
                // n'est tenu pendant un appel au système ou à Tauri.
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

    /// Copier le texte (0.5) : joue le balayage sur `zone` (pixels réels du bureau) une fois
    /// la zone choisie (`selection:balayer`). En mode texte, la zone choisie laisse la
    /// fenêtre montrée jusqu'à `cacher`. Fil principal. Sans affichage gardé : rien.
    pub fn balayer(&self, zone: Rect, reduites: bool) {
        self.interne.balayer(zone, reduites);
    }

    /// Copier le texte (13/11) : surligne les `lignes` d'un texte copié (pixels réels du
    /// bureau) selon `deroule` (`selection:surligner`) ; la page efface la sélection à la
    /// fin, le cœur la cache ensuite. Fil principal. Faux sans affichage gardé, sans ligne,
    /// ou si l'événement n'a pas pu partir : rien n'est joué.
    pub fn surligner(&self, lignes: &[Rect], reduites: bool, deroule: Surlignage) -> bool {
        self.interne.surligner(lignes, reduites, deroule)
    }

    /// Capture défilante (0.6) : propose à l'affichage `id` les pages qui défilent (pixels
    /// réels du bureau, du premier plan vers le fond), calculées après la photo : nouvelle
    /// charge, même affichage, même numéro de charge (le repère ne change pas) ; la page
    /// garde son geste en cours. `Signal::Fenetre::indice` est alors un indice de `pages`
    /// (les `FENETRES_MAX` premières). Fil principal (posté sinon). Un `id` qui n'est pas
    /// celui en cours, ou pas en mode défilante : rien. Peut venir plusieurs fois par
    /// affichage (pages d'UI Automation venues en retard) : chaque liste prolonge la
    /// précédente sans changer ses indices, si bien qu'un clic décidé sur une liste vise
    /// la même page dans la suivante.
    pub fn proposer(&self, id: u64, pages: Vec<FenetreProposee>) {
        self.interne.proposer(id, pages);
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
            "selection montree {id} voulu {} reel {} echelle {echelle_tao} placements {placements} mode {} fenetres {}",
            texte_rect(voulu),
            reel.map(texte_rect).unwrap_or_else(|| "inconnu".into()),
            demande.mode.nom(),
            fenetres_de(demande).len()
        );
        // 5. Prévenir la page : tout en pixels réels relatifs au coin de la fenêtre.
        let charge = {
            let mut etat = self.etat();
            let a = etat.courant_mut(id).ok_or("affichage remplace")?;
            a.echelle_tao = echelle_tao;
            etat.nouvelle_charge(id, repere)
                .ok_or("affichage remplace")?
        };
        self.emettre_charge(charge)
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
        // Mode texte, zone choisie : la fenêtre reste pour le balayage ; mode défilante, zone
        // ou page choisie, et mode film, zone choisie : elle reste jusqu'à la fenêtre de
        // contrôle. `cacher` la cache.
        let gardee = {
            let mut etat = self.etat();
            etat.garder_si_texte(&affichage, &signal)
                || etat.garder_si_defilante(&affichage, &signal)
                || etat.garder_si_film(&affichage, &signal)
        };
        if !gardee {
            self.masquer(affichage.avant);
        }
        if let Some(fin) = decrire_fin(&signal) {
            log::info!("selection fin {id} {fin}");
        }
        (self.rappel)(signal);
    }

    /// `cacher` public : sans signal. Hors du fil principal, tout part sur le fil principal
    /// (un masquage posté directement à Windows pourrait cacher l'affichage suivant) ; la
    /// fenêtre n'est cachée sans attendre que si cet envoi échoue (boucle arrêtée).
    fn cacher(self: &Arc<Self>) {
        let avant = self.etat().tout_oublier();
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

    fn balayer(&self, zone: Rect, reduites: bool) {
        let Some(charge) = self.etat().charge_balayer(zone, reduites) else {
            log::info!("selection balayage sans affichage");
            return;
        };
        match self
            .app
            .emit_to(ETIQUETTE_SELECTION, EVENEMENT_BALAYER, charge)
        {
            Ok(()) => log::info!("selection balayage {}", charge.id),
            Err(e) => log::warn!("selection balayage {} non envoye: {e}", charge.id),
        }
    }

    fn surligner(&self, lignes: &[Rect], reduites: bool, deroule: Surlignage) -> bool {
        if lignes.is_empty() {
            // Un texte dont aucun mot n'a de surface : rien à surligner.
            log::info!("selection surlignage sans ligne");
            return false;
        }
        let Some(charge) = self.etat().charge_surligner(lignes, reduites, deroule) else {
            log::info!("selection surlignage sans affichage");
            return false;
        };
        let (id, n) = (charge.id, charge.lignes.len());
        match self
            .app
            .emit_to(ETIQUETTE_SELECTION, EVENEMENT_SURLIGNER, charge)
        {
            Ok(()) => {
                log::info!("selection surlignage {id} {n} lignes");
                true
            }
            Err(e) => {
                log::warn!("selection surlignage {id} non envoye: {e}");
                false
            }
        }
    }

    /// `proposer` public. Hors du fil principal, posté sur le fil principal (l'état et
    /// l'envoi suivent l'ordre des charges).
    fn proposer(self: &Arc<Self>, id: u64, pages: Vec<FenetreProposee>) {
        if !self.sur_fil_principal() {
            let i = Arc::clone(self);
            if let Err(e) = self.app.run_on_main_thread(move || i.proposer(id, pages)) {
                log::warn!("selection {id}: pages non proposees: {e}");
            }
            return;
        }
        let n = pages.len().min(FENETRES_MAX);
        let retenue = self.etat().sans_charge(id);
        let Some(charge) = self.etat().proposer(id, pages) else {
            if retenue {
                log::info!("selection {id}: pages retenues pour la premiere charge");
            } else {
                log::info!("selection {id}: pages sans affichage en cours");
            }
            return;
        };
        match self.emettre_charge(charge) {
            Ok(()) => log::info!("selection {id}: pages proposees {n}"),
            Err(e) => log::warn!("selection {id}: pages non envoyees: {e}"),
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

    /// Envoie une charge à la page, avec la souris relue à l'instant.
    fn emettre_charge(&self, charge: ChargeMontrer) -> tauri::Result<()> {
        let charge = charge.avec_souris(systeme::position_souris());
        self.app
            .emit_to(ETIQUETTE_SELECTION, EVENEMENT_MONTRER, charge)
    }

    /// Renvoie à la page la demande de l'affichage en cours : elle revient au repos (son
    /// `selection_prete` suivant est ignoré s'il l'était déjà).
    fn renvoyer(&self, charge: ChargeMontrer) {
        if let Err(e) = self.emettre_charge(charge) {
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
    /// changement de taille). Un ancien numéro de charge est ignoré : la page n'a pas encore
    /// reçu la charge suivante et renverra `selection_prete` (le délai `Muette` reste armé si
    /// l'affichage n'était pas prêt). Sinon, la fenêtre est relue : si elle a changé, elle
    /// est retenue et la page reçoit une nouvelle charge. Puis le contrôle de cohérence : un
    /// échec termine l'affichage (`Impossible`), sauf juste après un changement de la
    /// fenêtre (la page, remise au repos, renverra `selection_prete`). `Prete` n'est signalé
    /// qu'une fois.
    fn prete(self: &Arc<Self>, id: u64, page: PagePrete) {
        let relu = self.identifiant.and_then(rect_reel);
        let reponse = self
            .etat()
            .repondre_prete(id, page.numero, page.attente, relu);
        let Some(reponse) = reponse else {
            return;
        };
        let (fenetre, renvoi, changee, controler) = match reponse {
            ReponsePrete::Ancienne => {
                log::info!(
                    "selection {id}: prete d'une ancienne charge {} ignoree",
                    page.numero
                );
                return;
            }
            ReponsePrete::Suite {
                fenetre,
                renvoi,
                changee,
                controler,
            } => (fenetre, renvoi, changee, controler),
        };
        if let Some(avant) = changee {
            log::info!(
                "selection {id}: fenetre changee {} -> {}",
                texte_rect(avant),
                texte_rect(fenetre)
            );
        }
        if let Some(charge) = renvoi {
            self.renvoyer(charge);
        }
        if !controler {
            return;
        }
        let tao = self.etat().courant(id).map_or(f64::NAN, |a| a.echelle_tao);
        let PagePrete {
            largeur,
            hauteur,
            dpr,
            kx,
            ky,
            ..
        } = page;
        match controler_page((largeur, hauteur), (kx, ky), fenetre) {
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
            Err(cause) if changee.is_some() => {
                log::info!("selection {id}: page a recontroler ({cause})");
            }
            Err(cause) => self.finir(Signal::Impossible { id, cause }),
        }
    }

    /// Une zone d'un ancien numéro de charge est ignorée, sans fin : ses coins sont comptés
    /// dans une fenêtre qui a changé, et la charge suivante, déjà envoyée, remet la page au
    /// repos.
    fn terminee(self: &Arc<Self>, id: u64, numero: u64, depart: Point, arrivee: Point) {
        let Some((fenetre, cote_min, charge, courante, mode)) = ({
            let etat = self.etat();
            etat.courant(id).map(|a| {
                (
                    a.fenetre,
                    a.cote_min,
                    a.charge.clone(),
                    etat.charge_courante(id, numero),
                    a.mode,
                )
            })
        }) else {
            return;
        };
        if let Err(cause) = garde_mode(mode, Mode::Zone) {
            self.finir(Signal::Impossible { id, cause });
            return;
        }
        if !courante {
            log::info!("selection {id}: zone d'une ancienne charge {numero} ignoree");
            return;
        }
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

    /// Clic en mode fenêtre. Mêmes règles que `terminee` : un ancien numéro de charge est
    /// ignoré ; un clic sur rien (la page et le cœur ne sont pas d'accord) remet la page au
    /// repos et la sélection continue ; un point invalide termine l'affichage.
    fn cliquee(self: &Arc<Self>, id: u64, numero: u64, point: (f64, f64)) {
        let Some((a, courante)) = ({
            let etat = self.etat();
            etat.courant(id)
                .map(|a| (a.clone(), etat.charge_courante(id, numero)))
        }) else {
            return;
        };
        if let Err(cause) = garde_mode(a.mode, Mode::Fenetre) {
            self.finir(Signal::Impossible { id, cause });
            return;
        }
        if !courante {
            log::info!("selection {id}: clic d'une ancienne charge {numero} ignore");
            return;
        }
        match decider_fenetre(point, a.fenetre, &a.fenetres, &a.ecrans, a.cote_min) {
            DecisionFenetre::Fenetre { indice, point } => {
                self.finir(Signal::Fenetre { id, indice, point })
            }
            DecisionFenetre::Rien => {
                log::info!("selection {id}: clic sur aucune fenetre ignore");
                if let Some(charge) = a.charge {
                    self.renvoyer(charge);
                }
            }
            DecisionFenetre::Invalide(cause) => self.finir(Signal::Impossible { id, cause }),
        }
    }

    /// Une annulation d'un ancien numéro de charge reste une annulation : l'utilisateur veut
    /// sortir.
    fn annulee(self: &Arc<Self>, id: u64, numero: u64, nom: &str) {
        let Some(courante) = ({
            let etat = self.etat();
            etat.courant(id).map(|_| etat.charge_courante(id, numero))
        }) else {
            return;
        };
        if !courante {
            log::info!("selection {id}: annulation d'une ancienne charge {numero}");
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

/// La page est visible et peinte, et sa taille est stable depuis deux images. `numero` :
/// celui de la charge reçue. `largeur`, `hauteur` : la taille CSS sur laquelle l'échelle a
/// été retenue ; `kx`, `ky` : l'échelle retenue (`controler_page`) ; `echelle` :
/// `devicePixelRatio`, seulement écrit au journal ; `attente` : envoyé après un `resize`, la
/// page attend une charge (toujours renvoyée, même numéro si la fenêtre n'a pas changé).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn selection_prete(
    selection: State<'_, Selection>,
    id: u64,
    numero: u64,
    largeur: f64,
    hauteur: f64,
    echelle: f64,
    kx: f64,
    ky: f64,
    attente: bool,
) {
    selection.interne.prete(
        id,
        PagePrete {
            numero,
            largeur,
            hauteur,
            dpr: echelle,
            kx,
            ky,
            attente,
        },
    );
}

/// Zone choisie : les deux pixels finals (inclus), en pixels réels relatifs au coin de la fenêtre ;
/// `numero` : celui de la charge sur laquelle ils sont comptés.
#[tauri::command]
pub fn selection_terminee(
    selection: State<'_, Selection>,
    id: u64,
    numero: u64,
    depart: Point,
    arrivee: Point,
) {
    selection.interne.terminee(id, numero, depart, arrivee);
}

/// Clic en mode fenêtre : le pixel réel sous la pointe au relâchement, relatif au coin de la
/// fenêtre ; `numero` : celui de la charge sur laquelle il est compté. Une coordonnée qui
/// n'est pas un nombre (ou absente) devient `NaN` et donne `Impossible`.
#[tauri::command]
pub fn selection_cliquee(
    selection: State<'_, Selection>,
    id: u64,
    numero: u64,
    x: Option<serde_json::Value>,
    y: Option<serde_json::Value>,
) {
    selection
        .interne
        .cliquee(id, numero, (nombre_ou_rien(x), nombre_ou_rien(y)));
}

/// Un argument de commande lu comme un nombre ; `NaN` s'il est absent ou n'en est pas un.
fn nombre_ou_rien(v: Option<serde_json::Value>) -> f64 {
    v.and_then(|v| v.as_f64()).unwrap_or(f64::NAN)
}

/// Annulation par la page : `cause` vaut `"echap"` ou `"clic_droit"`.
#[tauri::command]
pub fn selection_annulee(selection: State<'_, Selection>, id: u64, numero: u64, cause: String) {
    selection.interne.annulee(id, numero, &cause);
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
            mode: Mode::Zone,
            fenetres: Vec::new(),
        }
    }

    /// Ce que la page a dessiné : `[min, max + 1[` des pixels envoyés (réels, relatifs),
    /// placé dans le bureau.
    fn dessinee(depart: (f64, f64), arrivee: (f64, f64), fenetre: Rect) -> Rect {
        let b = |v: f64| v.round() as i32;
        Rect::depuis_bords(
            fenetre.x + b(depart.0.min(arrivee.0)),
            fenetre.y + b(depart.1.min(arrivee.1)),
            fenetre.x + b(depart.0.max(arrivee.0)) + 1,
            fenetre.y + b(depart.1.max(arrivee.1)) + 1,
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
        assert_eq!(vers_bureau((3839.0, 1279.0), f), Some((1919, 1079)));
        assert_eq!(vers_bureau((10.4, 10.6), f), Some((-1910, -189)));
    }

    #[test]
    fn vers_bureau_bornes_et_tolerance() {
        let f = r(10, 20, 100, 50);
        // Le dernier pixel de la fenêtre est un coin valide.
        assert_eq!(vers_bureau((99.0, 49.0), f), Some((109, 69)));
        // Jusqu'à 1 pixel réel dehors : ramené au dernier pixel.
        assert_eq!(vers_bureau((-1.0, -0.5), f), Some((10, 20)));
        assert_eq!(vers_bureau((100.0, 50.0), f), Some((109, 69)));
        // Au-delà : refusé.
        assert_eq!(vers_bureau((-1.01, 0.0), f), None);
        assert_eq!(vers_bureau((0.0, 50.01), f), None);
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
                vers_bureau((x + l - 1.0, y + h - 1.0), f),
                Some((ecran.x + ecran.largeur - 1, ecran.y + ecran.hauteur - 1))
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
        // La page retient k, prend le pixel réel sous le curseur et envoie des pixels réels :
        // la zone découpée est celle dessinée, à tout k (même non rond).
        for k in [1.0_f64, 1.25, 1.5, 2.0, 1.1] {
            let (lc, hc): (f64, f64) = (1280.0, 400.0);
            let f = r(-500, 30, (lc * k).round() as i32, (hc * k).round() as i32);
            let (kx, ky) = controler_page((lc, hc), (k, k), f).expect("page coherente");
            assert!((kx - k).abs() < 1e-9 && (ky - k).abs() < 1e-9, "k {k}");
            let reel = |css: f64, k: f64| (css * k + 1e-3).floor();
            for (d, a) in [
                ((100.3, 50.6), (301.7, 250.2)),
                ((301.7, 250.2), (100.3, 50.6)),
                ((0.0, 0.0), (lc - 1.0 / k, hc - 1.0 / k)),
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
        let k = (1.25, 1.25);
        assert!(controler_page((1280.0, 400.0), k, f).is_ok());
        // Largeur CSS non entière (1366 px à 125 % : 1092,8 px CSS, innerWidth 1093) :
        // devicePixelRatio retenu, à 1 px CSS près.
        assert!(controler_page((1093.0, 401.0), k, r(0, 0, 1366, 501)).is_ok());
        // 2560 × 1440 à 150 % : 1706,67 px CSS, innerWidth 1707.
        assert!(controler_page((1707.0, 960.0), (1.5, 1.5), r(0, 0, 2560, 1440)).is_ok());
        // Repli mesuré (taille CSS non arrondie) : accepté s'il explique la fenêtre.
        let m = (1366.0 / 1092.8, 501.0 / 400.8);
        assert!(controler_page((1092.8, 400.8), m, r(0, 0, 1366, 501)).is_ok());
        // Échelle qui n'explique pas la fenêtre (plus d'un pixel CSS d'écart) : refusée.
        assert_eq!(
            controler_page((1280.0, 400.0), (1.5, 1.5), f),
            Err("page 1280x400 pour fenetre 1600x500".into())
        );
        assert!(controler_page((1282.0, 400.0), k, f).is_err());
        // kx et ky à plus de 2 % l'un de l'autre : refusé, même s'ils expliquent la fenêtre.
        assert_eq!(
            controler_page((1280.0, 380.0), (1.25, 500.0 / 380.0), f),
            Err("page 1280x380 pour fenetre 1600x500".into())
        );
        assert!(controler_page((1280.0, 395.0), (1.25, 500.0 / 395.0), f).is_ok());
        for (page, k) in [
            ((0.0, 400.0), k),
            ((1280.0, 0.0), k),
            ((-1280.0, 400.0), k),
            ((f64::NAN, 400.0), k),
            ((1280.0, f64::INFINITY), k),
            ((1280.0, 400.0), (f64::NAN, 1.25)),
            ((1280.0, 400.0), (1.25, 0.0)),
            ((100.0, 31.25), (16.0, 16.0)),
            ((6400.0, 2000.0), (0.25, 0.25)),
        ] {
            assert!(controler_page(page, k, f).is_err(), "{page:?} {k:?}");
        }
        assert!(controler_page((1280.0, 400.0), k, r(0, 0, 0, 500)).is_err());
        // Bornes de k incluses.
        assert!(controler_page((3200.0, 1000.0), (0.5, 0.5), f).is_ok());
        assert!(controler_page((320.0, 100.0), (5.0, 5.0), f).is_ok());
    }

    #[test]
    fn page_en_attente_recoit_toujours_une_charge() {
        let mut e = Etat::default();
        let (id, _) = e.commencer(&demande(), None, None, Instant::now());
        let f = r(0, 0, 1600, 500);
        let c = e.nouvelle_charge(id, f).expect("charge");
        // Rectangle inchangé, page en attente : même charge, même numéro ; pas de contrôle.
        assert_eq!(
            e.repondre_prete(id, c.numero, true, Some(f)),
            Some(ReponsePrete::Suite {
                fenetre: f,
                renvoi: Some(c.clone()),
                changee: None,
                controler: false,
            })
        );
        assert!(e.charge_courante(id, c.numero), "numero inchange");
        // Rectangle inchangé, page prête : rien à renvoyer, contrôle.
        assert_eq!(
            e.repondre_prete(id, c.numero, false, Some(f)),
            Some(ReponsePrete::Suite {
                fenetre: f,
                renvoi: None,
                changee: None,
                controler: true,
            })
        );
        // Rectangle illisible : traité comme inchangé.
        let rep = e.repondre_prete(id, c.numero, true, None);
        assert!(matches!(rep, Some(ReponsePrete::Suite { renvoi: Some(ref x), .. }) if *x == c));
        // Rectangle changé : nouvelle charge, numéro suivant, en attente ou non.
        let g = r(0, 0, 1250, 475);
        let Some(ReponsePrete::Suite {
            fenetre,
            renvoi: Some(c2),
            changee,
            controler,
        }) = e.repondre_prete(id, c.numero, true, Some(g))
        else {
            panic!("suite attendue");
        };
        assert_eq!((fenetre, changee, controler), (g, Some(f), false));
        assert!(c2.numero > c.numero && c2.largeur == 1250);
        // L'ancien numéro est ignoré ; un autre affichage aussi.
        assert_eq!(
            e.repondre_prete(id, c.numero, true, Some(g)),
            Some(ReponsePrete::Ancienne)
        );
        assert_eq!(e.repondre_prete(id + 1, c2.numero, true, Some(g)), None);
    }

    #[test]
    fn numero_de_charge_courant_ou_ancien() {
        let mut e = Etat::default();
        let (id, _) = e.commencer(&demande(), None, None, Instant::now());
        assert!(!e.charge_courante(id, 0), "pas encore de charge");
        let c1 = e.nouvelle_charge(id, r(0, 0, 1600, 500)).expect("charge");
        assert!(e.charge_courante(id, c1.numero));
        assert!(!e.charge_courante(id + 1, c1.numero), "autre affichage");
        // Fenêtre relue : nouvelle charge, l'ancien numéro n'est plus courant.
        let c2 = e.nouvelle_charge(id, r(0, 0, 1250, 475)).expect("charge");
        assert!(c2.numero > c1.numero);
        assert!(!e.charge_courante(id, c1.numero));
        assert!(e.charge_courante(id, c2.numero));
        let a = e.courant(id).expect("en cours");
        assert_eq!(
            (a.fenetre, a.charge.as_ref().map(|c| c.largeur)),
            (r(0, 0, 1250, 475), Some(1250))
        );
        // Les numéros continuent d'un affichage à l'autre ; un affichage fini n'en a plus.
        let (id2, _) = e.commencer(&demande(), None, None, Instant::now());
        assert!(!e.charge_courante(id, c2.numero) && !e.charge_courante(id2, c2.numero));
        let c3 = e.nouvelle_charge(id2, r(0, 0, 1600, 500)).expect("charge");
        assert!(c3.numero > c2.numero && e.charge_courante(id2, c3.numero));
        assert!(
            e.nouvelle_charge(id, r(0, 0, 1, 1)).is_none(),
            "affichage oublie"
        );
        e.terminer(id2);
        assert!(!e.charge_courante(id2, c3.numero));
    }

    #[test]
    fn decision_d_une_zone() {
        let f = r(0, 0, 1920, 1080);
        assert_eq!(
            decider_zone((10.0, 10.0), (109.0, 59.0), f, 3),
            Decision::Zone((10, 10), (109, 59))
        );
        // Sens inverse : le sens est gardé, rien n'est retiré.
        assert_eq!(
            decider_zone((109.0, 59.0), (10.0, 10.0), f, 3),
            Decision::Zone((109, 59), (10, 10))
        );
        assert_eq!(
            decider_zone((10.0, 10.0), (11.0, 200.0), f, 3),
            Decision::TropPetite
        );
        // Un clic sans bouger : 1 × 1, pas une zone, la sélection continue.
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
    fn glisser_de_412_a_611_donne_200() {
        // Essai Windows (1024 × 768 à 100 %) : appui en (412, 40), relâché en (611, 239) ;
        // les deux pixels sous le curseur sont inclus.
        let f = r(0, 0, 1024, 768);
        let z = zone(decider_zone((412.0, 40.0), (611.0, 239.0), f, 3));
        assert_eq!(z, r(412, 40, 200, 200));
        // Jusqu'au dernier pixel (le curseur ne va pas plus loin) : dernière colonne et
        // dernière ligne comprises, dans les deux sens.
        let z = zone(decider_zone((412.0, 40.0), (1023.0, 767.0), f, 3));
        assert_eq!((z.droite(), z.bas()), (1024_i64, 768_i64));
        let z = zone(decider_zone((1023.0, 767.0), (0.0, 0.0), f, 3));
        assert_eq!(z, f);
    }

    #[test]
    fn zone_dessinee_egale_zone_decoupee() {
        let f = r(-300, 40, 1920, 1080);
        let (g, h, d, b) = (100.0, 100.0, 299.0, 249.0);
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
            let (x1, y1) = (
                x0 + f64::from(p.largeur) - 1.0,
                y0 + f64::from(p.hauteur) - 1.0,
            );
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
        // La page borne aux pixels de l'écran du départ : le dernier pixel de l'écran de
        // gauche est 639.
        let Decision::Zone(a, b) =
            decider_zone((639.0, 100.0), (540.0, 199.0), r(0, 0, 1280, 400), 3)
        else {
            panic!("pas une zone");
        };
        assert_eq!(a, (639, 100));
        assert!(r(0, 0, 640, 400).contient(a));
        assert_eq!(rect_entre(a, b), r(540, 100, 100, 100));
    }

    #[test]
    fn seuil_compte_en_pixels_inclus() {
        let f = r(0, 0, 100, 100);
        assert_eq!(
            decider_zone((10.0, 10.0), (12.0, 12.0), f, 3),
            Decision::Zone((10, 10), (12, 12))
        );
        assert_eq!(
            decider_zone((12.0, 12.0), (10.0, 11.0), f, 3),
            Decision::TropPetite
        );
    }

    #[test]
    fn charge_envoyee_a_la_page() {
        let c = charge_pour(
            3,
            8,
            r(-1920, 0, 3520, 1080),
            &[r(0, 0, 1600, 900)],
            3,
            Mode::Zone,
            &[],
        );
        assert_eq!(
            serde_json::to_string(&c).unwrap(),
            concat!(
                r#"{"id":3,"numero":8,"largeur":3520,"hauteur":1080,"#,
                r#""ecrans":[{"x":1920,"y":0,"largeur":1600,"hauteur":900}],"cote_min":3,"#,
                r#""mode":"zone","fenetres":[],"souris":null}"#
            )
        );
    }

    fn fp(x: i32, y: i32, largeur: i32, hauteur: i32, rayon: i32) -> FenetreProposee {
        FenetreProposee {
            rect: r(x, y, largeur, hauteur),
            rayon,
        }
    }

    #[test]
    fn charge_du_mode_fenetre() {
        // Fenêtres relatives à la fenêtre de sélection, débordements et rayon négatif borné.
        let f = r(-1920, -100, 3840, 1180);
        let c = charge_pour(
            1,
            2,
            f,
            &[r(-1920, -100, 1920, 1080), r(0, 0, 1920, 1080)],
            3,
            Mode::Fenetre,
            &[fp(-2000, -150, 500, 400, 8), fp(100, 200, 300, 200, -4)],
        )
        .avec_souris(Some((-1900.0, -90.5)));
        assert_eq!(
            serde_json::to_string(&c).unwrap(),
            concat!(
                r#"{"id":1,"numero":2,"largeur":3840,"hauteur":1180,"ecrans":["#,
                r#"{"x":0,"y":0,"largeur":1920,"hauteur":1080},"#,
                r#"{"x":1920,"y":100,"largeur":1920,"hauteur":1080}],"cote_min":3,"#,
                r#""mode":"fenetre","fenetres":["#,
                r#"{"x":-80,"y":-50,"largeur":500,"hauteur":400,"rayon":8},"#,
                r#"{"x":2020,"y":300,"largeur":300,"hauteur":200,"rayon":0}],"#,
                r#""souris":{"x":20,"y":9}}"#
            )
        );
    }

    #[test]
    fn souris_relue_relative_a_la_fenetre() {
        let f = r(-1920, 0, 3840, 1080);
        let p = |x, y| Some(PointPage { x, y });
        assert_eq!(souris_pour(Some((-1920.0, 0.0)), f), p(0, 0));
        assert_eq!(souris_pour(Some((1919.9, 1079.9)), f), p(3839, 1079));
        assert_eq!(souris_pour(Some((-1920.5, 0.0)), f), None, "hors a gauche");
        assert_eq!(souris_pour(Some((1920.0, 5.0)), f), None, "hors a droite");
        assert_eq!(souris_pour(Some((f64::NAN, 5.0)), f), None);
        assert_eq!(souris_pour(Some((1e300, 5.0)), f), None);
        assert_eq!(souris_pour(None, f), None);
    }

    #[test]
    fn mode_zone_sans_fenetres_et_liste_tronquee() {
        let mut d = demande();
        d.fenetres = vec![fp(0, 0, 10, 10, 0)];
        assert!(fenetres_de(&d).is_empty(), "vide en mode zone");
        d.mode = Mode::Fenetre;
        d.fenetres = (0..600).map(|i| fp(i, 0, 10, 10, 0)).collect();
        let retenues = fenetres_de(&d);
        assert_eq!(retenues.len(), FENETRES_MAX);
        assert_eq!(retenues[0], d.fenetres[0]);
        assert_eq!(retenues[511], d.fenetres[511]);
        // La charge porte la liste tronquée.
        let mut e = Etat::default();
        let (id, _) = e.commencer(&d, None, None, Instant::now());
        let c = e.nouvelle_charge(id, r(0, 0, 1920, 1080)).expect("charge");
        assert_eq!((c.mode, c.fenetres.len()), (Mode::Fenetre, FENETRES_MAX));
        assert_eq!(Mode::Zone.nom(), "zone");
        assert_eq!(Mode::Fenetre.nom(), "fenetre");
        assert_eq!(Mode::default(), Mode::Zone);
    }

    #[test]
    fn decider_fenetre_premiere_de_la_liste() {
        let f = r(0, 0, 1920, 1080);
        let ecrans = [f];
        // Deux fenêtres qui se recouvrent : la première de la liste (premier plan) gagne.
        let liste = [fp(100, 100, 400, 300, 8), fp(300, 200, 600, 500, 8)];
        assert_eq!(
            decider_fenetre((350.0, 250.0), f, &liste, &ecrans, 3),
            DecisionFenetre::Fenetre {
                indice: 0,
                point: (350, 250)
            }
        );
        // Sous la seconde seulement.
        assert_eq!(
            decider_fenetre((600.0, 600.0), f, &liste, &ecrans, 3),
            DecisionFenetre::Fenetre {
                indice: 1,
                point: (600, 600)
            }
        );
        // Bords : le dernier pixel de la fenêtre en est, le suivant non.
        assert_eq!(
            decider_fenetre((499.0, 399.0), f, &liste[..1], &ecrans, 3),
            DecisionFenetre::Fenetre {
                indice: 0,
                point: (499, 399)
            }
        );
        assert_eq!(
            decider_fenetre((500.0, 250.0), f, &liste[..1], &ecrans, 3),
            DecisionFenetre::Rien
        );
        // Sur le bureau, sur rien.
        assert_eq!(
            decider_fenetre((50.0, 50.0), f, &liste, &ecrans, 3),
            DecisionFenetre::Rien
        );
        assert_eq!(
            decider_fenetre((50.0, 50.0), f, &[], &ecrans, 3),
            DecisionFenetre::Rien
        );
    }

    #[test]
    fn decider_fenetre_coupee_a_l_ecran_du_point() {
        // Deux écrans, celui de gauche à coordonnées négatives ; fenêtre de sélection sur
        // tout le bureau.
        let sel = r(-1920, -200, 3840, 1280);
        let ecrans = [r(-1920, -200, 1920, 1080), r(0, 0, 1920, 1080)];
        // Fenêtre à cheval : 2 pixels à droite de la frontière, le reste à gauche.
        let liste = [fp(-1000, 100, 1002, 400, 8)];
        // Point à gauche (pixel réel du bureau -500, 300 → relatif 1420, 500) : elle garde
        // 1000 × 400.
        assert_eq!(
            decider_fenetre((1420.0, 500.0), sel, &liste, &ecrans, 3),
            DecisionFenetre::Fenetre {
                indice: 0,
                point: (-500, 300)
            }
        );
        // Point à droite (pixel 0, 300) : coupée à l'écran de droite, 2 × 400 : trop petite.
        assert_eq!(
            decider_fenetre((1920.0, 500.0), sel, &liste, &ecrans, 3),
            DecisionFenetre::Rien
        );
        // Avec 3 pixels à droite : assez grande.
        let liste = [fp(-1000, 100, 1003, 400, 8)];
        assert_eq!(
            decider_fenetre((1922.0, 500.0), sel, &liste, &ecrans, 3),
            DecisionFenetre::Fenetre {
                indice: 0,
                point: (2, 300)
            }
        );
        // La première trop petite ne laisse pas passer celle d'en dessous.
        let liste = [fp(-2, 100, 1000, 400, 0), fp(-1500, 0, 3000, 900, 0)];
        assert_eq!(
            decider_fenetre((1919.0, 500.0), sel, &liste, &ecrans, 3),
            DecisionFenetre::Rien
        );
        // Point dans le bureau mais sur aucun écran (sous l'écran de gauche, pixel -500,
        // 1000) : rien.
        let grande = [fp(-1920, -200, 3840, 1280, 0)];
        assert_eq!(
            decider_fenetre((1420.0, 1200.0), sel, &grande, &ecrans, 3),
            DecisionFenetre::Rien
        );
    }

    #[test]
    fn decider_fenetre_refuse_un_point_invalide() {
        let f = r(0, 0, 1920, 1080);
        let liste = [fp(0, 0, 1920, 1080, 0)];
        for p in [
            (f64::NAN, 1.0),
            (1.0, f64::INFINITY),
            (-5.0, 10.0),
            (10.0, 1081.5),
        ] {
            assert!(
                matches!(
                    decider_fenetre(p, f, &liste, &[f], 3),
                    DecisionFenetre::Invalide(_)
                ),
                "{p:?}"
            );
        }
        assert_eq!(nombre_ou_rien(Some(serde_json::json!(12))), 12.0);
        assert!(nombre_ou_rien(Some(serde_json::Value::Null)).is_nan());
        assert!(nombre_ou_rien(Some(serde_json::json!("3"))).is_nan());
        assert!(nombre_ou_rien(None).is_nan());
    }

    #[test]
    fn garde_fous_de_mode() {
        assert_eq!(garde_mode(Mode::Zone, Mode::Zone), Ok(()));
        assert_eq!(garde_mode(Mode::Fenetre, Mode::Fenetre), Ok(()));
        assert_eq!(
            garde_mode(Mode::Zone, Mode::Fenetre),
            Err("clic en mode zone".into())
        );
        assert_eq!(
            garde_mode(Mode::Fenetre, Mode::Zone),
            Err("zone en mode fenetre".into())
        );
    }

    #[test]
    fn numero_de_charge_en_mode_fenetre() {
        let mut d = demande();
        d.mode = Mode::Fenetre;
        d.fenetres = vec![fp(10, 10, 500, 500, 8)];
        let mut e = Etat::default();
        let (id, _) = e.commencer(&d, None, None, Instant::now());
        let c1 = e.nouvelle_charge(id, r(0, 0, 1920, 1080)).expect("charge");
        let c2 = e.nouvelle_charge(id, r(0, 0, 1600, 900)).expect("charge");
        assert!(!e.charge_courante(id, c1.numero) && e.charge_courante(id, c2.numero));
        // La charge relue garde le mode et les fenêtres, relatives au nouveau repère.
        assert_eq!(c2.mode, Mode::Fenetre);
        assert_eq!(c2.fenetres[0].x, 10);
        assert_eq!(c2.repere, r(0, 0, 1600, 900));
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
            decrire_fin(&Signal::Fenetre {
                id: 1,
                indice: 0,
                point: (5, 5)
            })
            .as_deref(),
            Some("fenetre")
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

    #[test]
    fn mode_texte_garde_la_fenetre_pour_le_balayage() {
        let mut e = Etat::default();
        let mut d = demande();
        d.bureau = r(-1920, 0, 3840, 1080);
        d.mode = Mode::Texte;
        let (id, _) = e.commencer(&d, Some(77), Some(5), Instant::now());
        let zone = Signal::Zone {
            id,
            depart: (-100, 10),
            arrivee: (99, 59),
        };
        let a = e.terminer(id).expect("affichage");
        assert!(e.garder_si_texte(&a, &zone), "zone en mode texte : gardee");
        // La zone en pixels de la page (origine au coin de la fenêtre).
        assert_eq!(
            e.charge_balayer(r(-100, 10, 200, 50), true),
            Some(ChargeBalayer {
                id,
                x: 1820,
                y: 10,
                largeur: 200,
                hauteur: 50,
                reduites: true,
            })
        );
        // `cacher` oublie l'affichage gardé et rend le premier plan à celle d'avant.
        assert_eq!(e.tout_oublier(), Some(77));
        assert_eq!(e.charge_balayer(r(0, 0, 10, 10), false), None);
        assert_eq!(e.tout_oublier(), None);
    }

    #[test]
    fn surlignage_des_lignes_sur_l_affichage_garde() {
        let mut e = Etat::default();
        let mut d = demande();
        d.bureau = r(-1920, 0, 3840, 1080);
        d.mode = Mode::Texte;
        let deroule = Surlignage {
            trace: 260,
            pas: 40,
            tenue: 220,
            sortie: 140,
        };
        let lignes = [r(-90, 12, 120, 14), r(-90, 32, 80, 14)];
        // Sans affichage gardé : rien.
        assert_eq!(e.charge_surligner(&lignes, false, deroule), None);
        let (id, _) = e.commencer(&d, Some(77), Some(5), Instant::now());
        let zone = Signal::Zone {
            id,
            depart: (-100, 10),
            arrivee: (99, 59),
        };
        let a = e.terminer(id).expect("affichage");
        assert!(e.garder_si_texte(&a, &zone));
        // Les lignes en pixels de la page (origine au coin de la fenêtre), dans l'ordre.
        let charge = e.charge_surligner(&lignes, false, deroule).expect("charge");
        assert_eq!(
            charge,
            ChargeSurligner {
                id,
                lignes: vec![
                    RectFenetre {
                        x: 1830,
                        y: 12,
                        largeur: 120,
                        hauteur: 14
                    },
                    RectFenetre {
                        x: 1830,
                        y: 32,
                        largeur: 80,
                        hauteur: 14
                    },
                ],
                reduites: false,
                deroule,
            }
        );
        // Le déroulé à plat, à côté des lignes : ce que lit la page.
        let json = serde_json::to_value(&charge).expect("json");
        assert_eq!(json["trace"], 260);
        assert_eq!(json["pas"], 40);
        assert_eq!(json["tenue"], 220);
        assert_eq!(json["sortie"], 140);
        assert_eq!(json["reduites"], false);
        assert_eq!(json["lignes"][1]["largeur"], 80);
        assert!(json.get("deroule").is_none());
        // Sans ligne : rien à jouer.
        assert_eq!(e.charge_surligner(&[], false, deroule), None);
        assert_eq!(e.tout_oublier(), Some(77));
        assert_eq!(e.charge_surligner(&lignes, false, deroule), None);
    }

    #[test]
    fn duree_du_surlignage() {
        let d = Surlignage {
            trace: 260,
            pas: 40,
            tenue: 220,
            sortie: 140,
        };
        let ms = Duration::from_millis;
        assert_eq!(d.duree(0), ms(620));
        assert_eq!(d.duree(1), ms(620));
        assert_eq!(d.duree(5), ms(780));
        let enorme = Surlignage {
            trace: u32::MAX,
            pas: u32::MAX,
            tenue: u32::MAX,
            sortie: u32::MAX,
        };
        assert!(enorme.duree(usize::MAX) > ms(u64::from(u32::MAX)));
    }

    #[test]
    fn hors_mode_texte_ou_annulation_rien_n_est_garde() {
        for (mode, annule) in [(Mode::Zone, false), (Mode::Texte, true)] {
            let mut e = Etat::default();
            let mut d = demande();
            d.mode = mode;
            let (id, _) = e.commencer(&d, None, None, Instant::now());
            let signal = if annule {
                Signal::Annulee {
                    id,
                    cause: Cause::Echap,
                }
            } else {
                Signal::Zone {
                    id,
                    depart: (0, 0),
                    arrivee: (9, 9),
                }
            };
            let a = e.terminer(id).expect("affichage");
            assert!(!e.garder_si_texte(&a, &signal), "{mode:?}");
            assert_eq!(e.charge_balayer(r(0, 0, 10, 10), false), None);
        }
    }

    #[test]
    fn un_nouvel_affichage_oublie_l_affichage_garde() {
        let mut e = Etat::default();
        let mut d = demande();
        d.mode = Mode::Texte;
        let (id, _) = e.commencer(&d, Some(77), Some(5), Instant::now());
        let a = e.terminer(id).expect("affichage");
        let zone = Signal::Zone {
            id,
            depart: (0, 0),
            arrivee: (9, 9),
        };
        assert!(e.garder_si_texte(&a, &zone));
        // La sélection elle-même au premier plan : la fenêtre d'avant est reprise.
        let (id2, _) = e.commencer(&demande(), Some(5), Some(5), Instant::now());
        assert_eq!(e.charge_balayer(r(0, 0, 10, 10), false), None);
        assert_eq!(e.terminer(id2).and_then(|a| a.avant), Some(77));
    }

    #[test]
    fn mode_defilante_permet_les_deux_signaux() {
        assert_eq!(garde_mode(Mode::Defilante, Mode::Zone), Ok(()));
        assert_eq!(garde_mode(Mode::Defilante, Mode::Fenetre), Ok(()));
        // Les autres modes ne changent pas.
        assert_eq!(
            garde_mode(Mode::Texte, Mode::Fenetre),
            Err("clic en mode texte".into())
        );
        assert_eq!(garde_mode(Mode::Texte, Mode::Zone), Ok(()));
        assert!(garde_mode(Mode::Zone, Mode::Fenetre).is_err());
        assert!(garde_mode(Mode::Fenetre, Mode::Zone).is_err());
        assert_eq!(Mode::Defilante.nom(), "defilante");
        assert_eq!(
            serde_json::to_string(&Mode::Defilante).unwrap(),
            r#""defilante""#
        );
        // Sans pages proposées, la demande n'en porte aucune (elles viennent après).
        let mut d = demande();
        d.mode = Mode::Defilante;
        d.fenetres = vec![fp(0, 0, 500, 500, 0)];
        assert!(fenetres_de(&d).is_empty());
    }

    #[test]
    fn proposer_nouvelle_charge_meme_affichage() {
        let mut d = demande();
        d.bureau = r(-1920, 0, 3840, 1080);
        d.ecrans = vec![r(-1920, 0, 1920, 1080), r(0, 0, 1920, 1080)];
        d.mode = Mode::Defilante;
        let mut e = Etat::default();
        let (id, _) = e.commencer(&d, Some(77), Some(5), Instant::now());
        // Avant la première charge : rien à envoyer, mais la liste est retenue et la première
        // charge la porte.
        assert_eq!(e.proposer(id, vec![fp(-1800, 100, 900, 700, 0)]), None);
        let c1 = e.nouvelle_charge(id, d.bureau).expect("charge");
        assert_eq!(c1.fenetres.len(), 1);
        assert!(!c1.suite);
        // Pages arrivées après : même affichage, même numéro, même repère, pages relatives.
        let pages = vec![fp(-1800, 100, 900, 700, 0), fp(200, 50, 1000, 900, 8)];
        let c2 = e.proposer(id, pages.clone()).expect("charge des pages");
        assert_eq!((c2.id, c2.numero, c2.repere), (id, c1.numero, c1.repere));
        assert!(c2.suite);
        assert_eq!(c2.mode, Mode::Defilante);
        assert_eq!(c2.ecrans, c1.ecrans);
        assert_eq!(
            c2.fenetres,
            vec![
                FenetrePage {
                    x: 120,
                    y: 100,
                    largeur: 900,
                    hauteur: 700,
                    rayon: 0
                },
                FenetrePage {
                    x: 2120,
                    y: 50,
                    largeur: 1000,
                    hauteur: 900,
                    rayon: 8
                },
            ]
        );
        // Un geste compté sur la charge d'avant reste celui de la charge courante.
        assert!(e.charge_courante(id, c1.numero));
        // La charge courante n'est pas une suite : un renvoi remet la page au repos.
        let courante = e
            .courant(id)
            .and_then(|a| a.charge.clone())
            .expect("charge");
        assert!(!courante.suite);
        assert_eq!(courante.fenetres, c2.fenetres);
        // `suite` n'est écrit que vrai.
        let json = serde_json::to_value(&c2).expect("json");
        assert_eq!(json["suite"], true);
        assert_eq!(json["mode"], "defilante");
        assert!(serde_json::to_value(&courante)
            .expect("json")
            .get("suite")
            .is_none());
        // Le clic se décide sur les pages proposées : indice dans `pages`.
        let a = e.courant(id).expect("affichage").clone();
        assert_eq!(a.fenetres, pages);
        assert_eq!(
            decider_fenetre((2300.0, 400.0), a.fenetre, &a.fenetres, &a.ecrans, 3),
            DecisionFenetre::Fenetre {
                indice: 1,
                point: (380, 400)
            }
        );
        assert_eq!(
            decider_fenetre((10.0, 10.0), a.fenetre, &a.fenetres, &a.ecrans, 3),
            DecisionFenetre::Rien
        );
        // Une liste vide efface les pages.
        let c3 = e.proposer(id, Vec::new()).expect("charge");
        assert!(c3.fenetres.is_empty() && c3.numero == c1.numero);
        // Liste trop longue : tronquée.
        let longue = vec![fp(0, 0, 200, 200, 0); FENETRES_MAX + 3];
        let c4 = e.proposer(id, longue).expect("charge");
        assert_eq!(c4.fenetres.len(), FENETRES_MAX);
        // Un autre id, ou après la fin : rien.
        assert_eq!(e.proposer(id + 1, pages.clone()), None);
        let fin = e.terminer(id).expect("affichage");
        assert!(e.garder_si_defilante(
            &fin,
            &Signal::Zone {
                id,
                depart: (0, 0),
                arrivee: (300, 300)
            }
        ));
        assert_eq!(e.proposer(id, pages), None);
    }

    #[test]
    fn proposer_des_pages_tardives_garde_les_indices_et_le_numero() {
        // Les pages venues après le budget prolongent la liste (`defiler::chercher_pages`) :
        // même charge, et un point vise la même page, par le même indice, avant et après.
        let mut d = demande();
        d.mode = Mode::Defilante;
        let mut e = Etat::default();
        let (id, _) = e.commencer(&d, None, None, Instant::now());
        let c1 = e.nouvelle_charge(id, d.bureau).expect("charge");
        let premiere = vec![fp(100, 100, 600, 400, 0)];
        let c2 = e.proposer(id, premiere.clone()).expect("pages");
        let a = e.courant(id).expect("affichage").clone();
        let avant = decider_fenetre((300.0, 300.0), a.fenetre, &a.fenetres, &a.ecrans, 3);
        let mut grandie = premiere;
        grandie.push(fp(900, 100, 400, 400, 0));
        let c3 = e.proposer(id, grandie).expect("pages tardives");
        assert_eq!((c2.numero, c3.numero), (c1.numero, c1.numero));
        assert!(c2.suite && c3.suite);
        assert!(e.charge_courante(id, c1.numero));
        let a = e.courant(id).expect("affichage").clone();
        assert_eq!(
            decider_fenetre((300.0, 300.0), a.fenetre, &a.fenetres, &a.ecrans, 3),
            avant
        );
        assert!(matches!(avant, DecisionFenetre::Fenetre { indice: 0, .. }));
        assert!(matches!(
            decider_fenetre((1000.0, 300.0), a.fenetre, &a.fenetres, &a.ecrans, 3),
            DecisionFenetre::Fenetre { indice: 1, .. }
        ));
    }

    #[test]
    fn proposer_hors_mode_defilante_ne_change_rien() {
        for mode in [Mode::Zone, Mode::Fenetre, Mode::Texte, Mode::Film] {
            let mut d = demande();
            d.mode = mode;
            d.fenetres = vec![fp(10, 10, 500, 500, 8)];
            let mut e = Etat::default();
            let (id, _) = e.commencer(&d, None, None, Instant::now());
            let c = e.nouvelle_charge(id, d.bureau).expect("charge");
            assert_eq!(
                e.proposer(id, vec![fp(0, 0, 900, 900, 0)]),
                None,
                "{mode:?}"
            );
            let a = e.courant(id).expect("affichage");
            assert_eq!(a.fenetres, fenetres_de(&d), "{mode:?}");
            assert_eq!(a.charge.as_ref(), Some(&c), "{mode:?}");
        }
    }

    #[test]
    fn garder_si_defilante_zone_ou_page() {
        let zone = |id| Signal::Zone {
            id,
            depart: (-100, 10),
            arrivee: (299, 259),
        };
        let page = |id| Signal::Fenetre {
            id,
            indice: 0,
            point: (10, 10),
        };
        for signal in [zone as fn(u64) -> Signal, page] {
            let mut e = Etat::default();
            let mut d = demande();
            d.bureau = r(-1920, 0, 3840, 1080);
            d.mode = Mode::Defilante;
            let (id, _) = e.commencer(&d, Some(77), Some(5), Instant::now());
            let a = e.terminer(id).expect("affichage");
            let s = signal(id);
            assert!(!e.garder_si_texte(&a, &s), "pas le mode texte");
            assert!(e.garder_si_defilante(&a, &s), "{s:?} : gardee");
            // Ni balayage ni surlignage sur une défilante gardée.
            assert_eq!(e.charge_balayer(r(0, 0, 10, 10), false), None);
            let deroule = Surlignage {
                trace: 1,
                pas: 1,
                tenue: 1,
                sortie: 1,
            };
            assert_eq!(e.charge_surligner(&[r(0, 0, 9, 9)], false, deroule), None);
            // `cacher` l'oublie et rend le premier plan à celle d'avant.
            assert_eq!(e.tout_oublier(), Some(77));
            assert_eq!(e.tout_oublier(), None);
        }
        // Un affichage suivant, la sélection au premier plan, reprend la fenêtre d'avant.
        let mut e = Etat::default();
        let mut d = demande();
        d.mode = Mode::Defilante;
        let (id, _) = e.commencer(&d, Some(77), Some(5), Instant::now());
        let a = e.terminer(id).expect("affichage");
        assert!(e.garder_si_defilante(&a, &page(id)));
        let (id2, _) = e.commencer(&demande(), Some(5), Some(5), Instant::now());
        assert_eq!(e.terminer(id2).and_then(|a| a.avant), Some(77));
    }

    #[test]
    fn hors_mode_defilante_ou_annulation_rien_n_est_garde() {
        let fins = |id| {
            [
                Signal::Annulee {
                    id,
                    cause: Cause::Echap,
                },
                Signal::Muette { id },
                Signal::Impossible {
                    id,
                    cause: "x".into(),
                },
            ]
        };
        let mut e = Etat::default();
        let mut d = demande();
        d.mode = Mode::Defilante;
        let (id, _) = e.commencer(&d, None, None, Instant::now());
        let a = e.terminer(id).expect("affichage");
        for s in fins(id) {
            assert!(!e.garder_si_defilante(&a, &s), "{s:?}");
        }
        assert_eq!(e.tout_oublier(), None);
        for mode in [Mode::Zone, Mode::Fenetre, Mode::Texte, Mode::Film] {
            let mut e = Etat::default();
            let mut d = demande();
            d.mode = mode;
            let (id, _) = e.commencer(&d, Some(3), None, Instant::now());
            let a = e.terminer(id).expect("affichage");
            for s in [
                Signal::Zone {
                    id,
                    depart: (0, 0),
                    arrivee: (9, 9),
                },
                Signal::Fenetre {
                    id,
                    indice: 0,
                    point: (0, 0),
                },
            ] {
                assert!(!e.garder_si_defilante(&a, &s), "{mode:?} {s:?}");
            }
            assert_eq!(e.tout_oublier(), None, "{mode:?}");
        }
    }

    /// Mode film (0.7) : le mode zone, seul `selection_terminee` permis (un clic est un
    /// garde-fou `Impossible`), aucune fenêtre proposée, son nom au journal et à la page.
    #[test]
    fn mode_film_comme_la_zone() {
        assert_eq!(garde_mode(Mode::Film, Mode::Zone), Ok(()));
        assert_eq!(
            garde_mode(Mode::Film, Mode::Fenetre),
            Err("clic en mode film".into())
        );
        // Les autres modes ne changent pas.
        assert!(garde_mode(Mode::Zone, Mode::Fenetre).is_err());
        assert!(garde_mode(Mode::Fenetre, Mode::Zone).is_err());
        assert_eq!(garde_mode(Mode::Defilante, Mode::Fenetre), Ok(()));
        assert_eq!(Mode::Film.nom(), "film");
        assert_eq!(serde_json::to_string(&Mode::Film).unwrap(), r#""film""#);
        let mut d = demande();
        d.mode = Mode::Film;
        d.fenetres = vec![fp(0, 0, 500, 500, 0)];
        assert!(fenetres_de(&d).is_empty());
        let mut e = Etat::default();
        let (id, _) = e.commencer(&d, None, None, Instant::now());
        let c = e.nouvelle_charge(id, d.bureau).expect("charge");
        assert_eq!(c.mode, Mode::Film);
        assert!(c.fenetres.is_empty());
    }

    /// Mode film : la zone choisie garde l'affichage jusqu'à `cacher` ; une annulation,
    /// une page muette ou impossible ne le gardent pas ; ni balayage ni surlignage.
    #[test]
    fn garder_si_film_zone_seulement() {
        let mut e = Etat::default();
        let mut d = demande();
        d.bureau = r(-1920, 0, 3840, 1080);
        d.mode = Mode::Film;
        let (id, _) = e.commencer(&d, Some(77), Some(5), Instant::now());
        let a = e.terminer(id).expect("affichage");
        let zone = Signal::Zone {
            id,
            depart: (-100, 10),
            arrivee: (299, 259),
        };
        assert!(!e.garder_si_texte(&a, &zone), "pas le mode texte");
        assert!(!e.garder_si_defilante(&a, &zone), "pas le mode defilante");
        assert!(e.garder_si_film(&a, &zone));
        assert_eq!(e.charge_balayer(r(0, 0, 10, 10), false), None);
        let deroule = Surlignage {
            trace: 1,
            pas: 1,
            tenue: 1,
            sortie: 1,
        };
        assert_eq!(e.charge_surligner(&[r(0, 0, 9, 9)], false, deroule), None);
        // `cacher` l'oublie et rend le premier plan à celle d'avant.
        assert_eq!(e.tout_oublier(), Some(77));
        assert_eq!(e.tout_oublier(), None);
        // Les fins sans zone ne gardent rien.
        let (id, _) = e.commencer(&d, None, None, Instant::now());
        let a = e.terminer(id).expect("affichage");
        for s in [
            Signal::Annulee {
                id,
                cause: Cause::Echap,
            },
            Signal::Muette { id },
            Signal::Impossible {
                id,
                cause: "x".into(),
            },
        ] {
            assert!(!e.garder_si_film(&a, &s), "{s:?}");
        }
        assert_eq!(e.tout_oublier(), None);
        // Les autres modes ne sont jamais gardés comme un film.
        for mode in [Mode::Zone, Mode::Fenetre, Mode::Texte, Mode::Defilante] {
            let mut e = Etat::default();
            let mut d = demande();
            d.mode = mode;
            let (id, _) = e.commencer(&d, Some(3), None, Instant::now());
            let a = e.terminer(id).expect("affichage");
            let zone = Signal::Zone {
                id,
                depart: (0, 0),
                arrivee: (9, 9),
            };
            assert!(!e.garder_si_film(&a, &zone), "{mode:?}");
        }
    }
}
