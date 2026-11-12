//! La vignette et son souffle (étape 9, ARCHITECTURE §11).
//!
//! Deux fenêtres « outil » préparées au démarrage et cachées (`creer_fenetre_outil`, comme
//! le bandeau) : `souffle` joue le flash et l'envol, `vignette` montre la carte. Le module
//! pose, montre, cache, tient la minuterie (en pause au survol et pendant un glisser) et
//! rapporte ce que fait la page ; il ne décide d'aucune action : tout remonte à
//! l'Application par `Signal`.
//!
//! Déroulé : `preparer` (fenêtre posée cachée, charge envoyée) → la page donne sa carte
//! (`vignette_zone`) → `Signal::Prete` → `jouer_souffle` → `souffle_termine` →
//! `Signal::SouffleFini` → `entrer` → `vignette_entree` (souffle caché) → … → `partir` →
//! `vignette_terminee`. Chaque étape a son filet (délais `DELAI_*`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

use super::place::{self, cliquable, intervalle, zone_ecran};
use super::{
    borner_zone_dans, cacher, creer_fenetre_outil, ecran_sous, ecrans, place_reelle, poser, souris,
    theme, traverser, Ecran, Rect, ZoneCss,
};
use crate::systeme;

/// Étiquettes des deux fenêtres (aussi dans `capabilities/`).
pub const ETIQUETTE_VIGNETTE: &str = "vignette";
pub const ETIQUETTE_SOUFFLE: &str = "souffle";

/// Taille de la fenêtre de la vignette, pixels logiques : la carte (jetons), sa marge, son
/// ombre, ses infobulles et la place de l'entrée. La page borne la carte à la fenêtre.
pub const TAILLE_LOGIQUE_VIGNETTE: (f64, f64) = (340.0, 280.0);
/// De combien la fenêtre peut dépasser la zone utile vers le bord de l'écran (l'ombre de
/// la carte passe sur la barre des tâches), pixels logiques.
pub const DEBORD_LOGIQUE: f64 = 48.0;
/// Marge autour de l'envol (l'arc de 40 px et l'ombre), pixels logiques.
pub const MARGE_SOUFFLE_LOGIQUE: f64 = 48.0;

/// Filets de sécurité (ARCHITECTURE §11).
pub const DELAI_ZONE: Duration = Duration::from_secs(2);
pub const DELAI_SOUFFLE: Duration = Duration::from_millis(1500);
pub const DELAI_ENTREE: Duration = Duration::from_millis(500);
pub const DELAI_DEPART: Duration = Duration::from_secs(1);

/// Coin de l'écran où se pose la carte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Coin {
    BasGauche,
    BasDroite,
    HautGauche,
    HautDroite,
}

impl Coin {
    pub fn nom(self) -> &'static str {
        match self {
            Coin::BasGauche => "bas_gauche",
            Coin::BasDroite => "bas_droite",
            Coin::HautGauche => "haut_gauche",
            Coin::HautDroite => "haut_droite",
        }
    }
    fn en_bas(self) -> bool {
        matches!(self, Coin::BasGauche | Coin::BasDroite)
    }
    fn a_gauche(self) -> bool {
        matches!(self, Coin::BasGauche | Coin::HautGauche)
    }
}

/// Où la page range un bouton.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Place {
    /// Pilule en bas au centre (la première est sur l'accent).
    Principale,
    /// Bouton rond en haut à droite.
    Secondaire,
    /// La croix.
    Fermer,
    /// Montré seulement quand la carte est « supprimée ».
    Annulation,
}

/// Un bouton de la carte, fourni par l'Application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActionVignette {
    pub id: String,
    pub libelle: String,
    pub libelle_fait: Option<String>,
    pub place: Place,
    pub picto: String,
}

/// Ce que l'Application demande de montrer.
#[derive(Debug, Clone, PartialEq)]
pub struct Demande {
    pub id: u64,
    /// Le rectangle capturé, pixels réels du bureau : départ du souffle, et son écran est
    /// celui de la vignette.
    pub capture: Rect,
    pub coin: Coin,
    pub actions: Vec<ActionVignette>,
    /// `None` : la vignette reste (« toujours »).
    pub duree: Option<Duration>,
    /// Durée d'annulation de Supprimer.
    pub duree_annulation: Duration,
    pub glisser: bool,
    pub reduites: bool,
    /// L'aperçu en PNG et sa taille.
    pub apercu: Arc<Vec<u8>>,
    pub apercu_taille: (u32, u32),
}

/// Ce que le module signale à l'Application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    /// La carte est posée dans la page (elle attend `entrer`).
    Prete { id: u64 },
    /// L'envol est fini (ou son filet est tombé).
    SouffleFini { id: u64, d_office: bool },
    /// La carte a commencé à entrer.
    Entree { id: u64 },
    /// Clic sur un bouton.
    Action { id: u64, action: String },
    /// Appui sur l'image et déplacement : demande de glisser.
    Glisser { id: u64 },
    /// La minuterie est arrivée au bout.
    DelaiEcoule { id: u64 },
    /// La carte est partie (fenêtre cachée).
    Terminee { id: u64 },
    /// La page n'a pas posé la carte à temps.
    Impossible { id: u64, cause: &'static str },
}

type Rappel = Box<dyn Fn(Signal) + Send + Sync + 'static>;
/// Un geste posté sur le fil principal par la surveillance.
type Geste = Box<dyn FnOnce(&Vignette) + Send>;

/// Les pixels réels qui séparent les bords de la fenêtre et ceux de la zone utile, du côté
/// du coin (la page pose la carte à `retrait + vignette-marge`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct Retrait {
    pub haut: u32,
    pub bas: u32,
    pub gauche: u32,
    pub droite: u32,
}

fn echelle_sure(e: f64) -> f64 {
    if e.is_finite() && e > 0.0 {
        e
    } else {
        1.0
    }
}

/// Place de la fenêtre de la vignette (fonction pure) : son coin sur celui de la zone utile,
/// repoussé vers le bord de l'écran d'au plus `debord` pixels logiques, sans sortir de
/// l'écran.
pub fn placer_vignette(
    ecran: &Ecran,
    coin: Coin,
    taille: (f64, f64),
    debord: f64,
) -> (Rect, Retrait) {
    let e = echelle_sure(ecran.echelle);
    let l = (taille.0 * e).round().max(1.0) as i64;
    let h = (taille.1 * e).round().max(1.0) as i64;
    let d = (debord * e).round().max(0.0) as i64;
    let (zu, r) = (ecran.zone_utile, ecran.rect);
    let (zx0, zy0) = (i64::from(zu.x), i64::from(zu.y));
    let (zx1, zy1) = (zx0 + i64::from(zu.largeur), zy0 + i64::from(zu.hauteur));
    let (rx0, ry0) = (i64::from(r.x), i64::from(r.y));
    let (rx1, ry1) = (rx0 + i64::from(r.largeur), ry0 + i64::from(r.hauteur));
    let mut retrait = Retrait::default();
    let x = if coin.a_gauche() {
        let g = (zx0 - d).max(rx0).min(zx0);
        retrait.gauche = (zx0 - g) as u32;
        g
    } else {
        let dr = (zx1 + d).min(rx1).max(zx1);
        retrait.droite = (dr - zx1) as u32;
        dr - l
    };
    let y = if coin.en_bas() {
        let b = (zy1 + d).min(ry1).max(zy1);
        retrait.bas = (b - zy1) as u32;
        b - h
    } else {
        let hh = (zy0 - d).max(ry0).min(zy0);
        retrait.haut = (zy0 - hh) as u32;
        hh
    };
    (
        Rect {
            x: x as i32,
            y: y as i32,
            largeur: l as u32,
            hauteur: h as u32,
        },
        retrait,
    )
}

/// Fenêtre du souffle (fonction pure) : l'union du départ et de l'arrivée, élargie de
/// `marge` pixels réels, coupée à l'écran. `None` si rien ne reste.
pub fn place_souffle(depart: Rect, arrivee: Rect, ecran: Rect, marge: u32) -> Option<Rect> {
    let m = i64::from(marge);
    let bords = |r: Rect| {
        let (x, y) = (i64::from(r.x), i64::from(r.y));
        (x, y, x + i64::from(r.largeur), y + i64::from(r.hauteur))
    };
    let (a, b, c) = (bords(depart), bords(arrivee), bords(ecran));
    let x0 = (a.0.min(b.0) - m).max(c.0);
    let y0 = (a.1.min(b.1) - m).max(c.1);
    let x1 = (a.2.max(b.2) + m).min(c.2);
    let y1 = (a.3.max(b.3) + m).min(c.3);
    (x1 > x0 && y1 > y0).then(|| Rect {
        x: x0 as i32,
        y: y0 as i32,
        largeur: (x1 - x0) as u32,
        hauteur: (y1 - y0) as u32,
    })
}

/// La minuterie de la vignette (pure) : compte le temps qui passe hors pause.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Minuterie {
    total: Option<Duration>,
    restant: Duration,
    en_marche: bool,
    finie: bool,
}

impl Minuterie {
    /// Arrêtée, pleine. `None` : jamais de fin (« toujours »).
    pub fn nouvelle(total: Option<Duration>) -> Minuterie {
        Minuterie {
            total,
            restant: total.unwrap_or_default(),
            en_marche: false,
            finie: false,
        }
    }
    pub fn demarrer(&mut self) {
        self.en_marche = true;
    }
    /// Repart à plein sur `total`.
    pub fn remettre(&mut self, total: Option<Duration>) {
        let marche = self.en_marche;
        *self = Minuterie::nouvelle(total);
        self.en_marche = marche;
    }
    pub fn total(&self) -> Option<Duration> {
        self.total
    }
    pub fn restant(&self) -> Duration {
        self.restant
    }
    pub fn en_marche(&self) -> bool {
        self.en_marche
    }
    /// Fait passer `dt` (sauf en pause). Vrai une seule fois, quand elle arrive au bout.
    pub fn avancer(&mut self, dt: Duration, pause: bool) -> bool {
        if !self.en_marche || pause || self.finie || self.total.is_none() {
            return false;
        }
        self.restant = self.restant.saturating_sub(dt);
        if self.restant.is_zero() {
            self.finie = true;
            return true;
        }
        false
    }
}

/// Où en est la carte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Charge envoyée (ou en attente d'une page prête), carte pas encore posée.
    Preparee,
    /// Carte posée, en attente de l'Application.
    Prete,
    Souffle,
    Entree,
    Montree,
    Partie,
}

#[derive(Debug, Clone, Serialize)]
struct ChargeVignette<'a> {
    id: u64,
    coin: Coin,
    retrait: Retrait,
    actions: &'a [ActionVignette],
    duree_ms: Option<u64>,
    glisser: bool,
    reduites: bool,
    apercu: Taille,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct Taille {
    largeur: u32,
    hauteur: u32,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct RectCharge {
    x: i32,
    y: i32,
    largeur: u32,
    hauteur: u32,
}

impl RectCharge {
    /// `r` relatif à l'origine `o` (pixels réels).
    fn relatif(r: Rect, o: Rect) -> RectCharge {
        RectCharge {
            x: r.x.saturating_sub(o.x),
            y: r.y.saturating_sub(o.y),
            largeur: r.largeur,
            hauteur: r.hauteur,
        }
    }
}

/// Un bouton tel que la page le décrit (`vignette_zone`), pixels CSS.
#[derive(Debug, Clone, Deserialize)]
pub struct BoutonCss {
    pub action: String,
    pub x: f64,
    pub y: f64,
    pub largeur: f64,
    pub hauteur: f64,
}

#[derive(Debug, Clone)]
struct EnCours {
    demande: Demande,
    envoi: u64,
    phase: Phase,
    depuis: Instant,
    /// Fenêtre de la vignette, échelle et écran.
    place: Option<(Rect, f64, Ecran)>,
    /// La carte, pixels CSS de la page.
    carte: Option<ZoneCss>,
    minuterie: Minuterie,
    survol: bool,
    glisser: bool,
    supprimee: bool,
    recoit_clics: bool,
    souffle_visible: bool,
    signale_delai: bool,
}

impl EnCours {
    fn id(&self) -> u64 {
        self.demande.id
    }
}

#[derive(Default)]
struct Etat {
    en_cours: Option<EnCours>,
    page_prete: bool,
    souffle_pret: bool,
    dernier_envoi: u64,
}

/// La vignette et son souffle. Règle des verrous : `etat` n'est jamais tenu pendant un
/// appel à Tauri ni un `emit`. Les gestes sur les fenêtres se font sur le fil principal.
pub struct Vignette {
    app: AppHandle,
    rappel: Rappel,
    etat: Mutex<Etat>,
    fil_principal: ThreadId,
    vivante: Arc<AtomicBool>,
    identifiant: Option<isize>,
    identifiant_souffle: Option<isize>,
}

impl Vignette {
    /// Crée les deux fenêtres, cachées (fil principal).
    pub fn creer<F>(app: &AppHandle, rappel: F) -> Result<Vignette, String>
    where
        F: Fn(Signal) + Send + Sync + 'static,
    {
        let souffle = creer_fenetre_outil(
            app,
            ETIQUETTE_SOUFFLE,
            "vignette/souffle.html",
            (200.0, 200.0),
            "souffle exclu",
        )?;
        let carte = creer_fenetre_outil(
            app,
            ETIQUETTE_VIGNETTE,
            "vignette/vignette.html",
            TAILLE_LOGIQUE_VIGNETTE,
            "vignette exclue",
        )?;
        Ok(Vignette {
            app: app.clone(),
            rappel: Box::new(rappel),
            etat: Mutex::new(Etat::default()),
            fil_principal: std::thread::current().id(),
            vivante: carte.vivante,
            identifiant: carte.identifiant,
            identifiant_souffle: souffle.identifiant,
        })
    }

    fn etat(&self) -> Option<MutexGuard<'_, Etat>> {
        match self.etat.lock() {
            Ok(e) => Some(e),
            Err(_) => {
                log::error!("vignette: etat inaccessible");
                None
            }
        }
    }

    fn fenetre(&self) -> Option<WebviewWindow> {
        self.app.get_webview_window(ETIQUETTE_VIGNETTE)
    }

    fn fenetre_souffle(&self) -> Option<WebviewWindow> {
        self.app.get_webview_window(ETIQUETTE_SOUFFLE)
    }

    /// Les identifiants système des deux fenêtres : `[vignette, souffle]`.
    pub fn identifiants(&self) -> [Option<isize>; 2] {
        [self.identifiant, self.identifiant_souffle]
    }

    /// Lance `f(self)` sur le fil principal (tout de suite si on y est).
    fn sur_fil_principal(&self, f: impl FnOnce(&Vignette) + Send + 'static) {
        if std::thread::current().id() == self.fil_principal {
            f(self);
            return;
        }
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || {
            if let Some(v) = app.try_state::<Vignette>() {
                f(&v);
            }
        });
    }

    /// L'écran de la capture (celui qui contient son centre).
    fn ecran_de(&self, capture: Rect) -> Option<Ecran> {
        let centre = (
            f64::from(capture.x) + f64::from(capture.largeur) / 2.0,
            f64::from(capture.y) + f64::from(capture.hauteur) / 2.0,
        );
        ecran_sous(Some(centre), &ecrans(&self.app))
    }

    /// Prépare la carte `demande` (fil principal) : elle remplace la précédente.
    pub fn preparer(&self, demande: Demande) {
        let id = demande.id;
        let prete = {
            let Some(mut etat) = self.etat() else {
                return;
            };
            let minuterie = Minuterie::nouvelle(demande.duree);
            etat.en_cours = Some(EnCours {
                demande,
                envoi: 0,
                phase: Phase::Preparee,
                depuis: Instant::now(),
                place: None,
                carte: None,
                minuterie,
                survol: false,
                glisser: false,
                supprimee: false,
                recoit_clics: false,
                souffle_visible: false,
                signale_delai: false,
            });
            etat.page_prete
        };
        if prete {
            self.envoyer();
        } else {
            // Page pas encore prête : le filet de `DELAI_ZONE` court quand même (envoi 0,
            // remplacé par la surveillance de `envoyer` si la page arrive à temps).
            self.surveiller(id, 0);
        }
    }

    /// Pose la fenêtre (cachée) et envoie la charge de la carte en cours.
    fn envoyer(&self) {
        let Some(fenetre) = self.fenetre() else {
            return;
        };
        let (demande, envoi) = {
            let Some(mut etat) = self.etat() else {
                return;
            };
            etat.dernier_envoi += 1;
            let envoi = etat.dernier_envoi;
            let Some(c) = etat.en_cours.as_mut() else {
                return;
            };
            c.envoi = envoi;
            c.phase = Phase::Preparee;
            c.depuis = Instant::now();
            c.carte = None;
            (c.demande.clone(), envoi)
        };
        let ecran = self.ecran_de(demande.capture);
        let place = ecran.map(|e| {
            let (r, retrait) =
                placer_vignette(&e, demande.coin, TAILLE_LOGIQUE_VIGNETTE, DEBORD_LOGIQUE);
            (r, retrait, e)
        });
        let retrait = place.map(|p| p.1).unwrap_or_default();
        if let Some((r, _, e)) = place {
            poser(&fenetre, r);
            log::info!(
                "vignette preparee {} coin {} ecran {},{} {}x{} echelle {} fenetre {},{} {}x{} \
                 duree {} actions {} glisser {}",
                demande.id,
                demande.coin.nom(),
                e.rect.x,
                e.rect.y,
                e.rect.largeur,
                e.rect.hauteur,
                e.echelle,
                r.x,
                r.y,
                r.largeur,
                r.hauteur,
                demande
                    .duree
                    .map(|d| d.as_secs().to_string())
                    .unwrap_or_else(|| "toujours".into()),
                demande
                    .actions
                    .iter()
                    .map(|a| a.id.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
                if demande.glisser { "oui" } else { "non" }
            );
        }
        traverser(&fenetre, true);
        if let Some(mut etat) = self.etat() {
            if let Some(c) = etat.en_cours.as_mut().filter(|c| c.envoi == envoi) {
                c.place = place.map(|(r, _, e)| (r, e.echelle, e));
            }
        }
        let charge = ChargeVignette {
            id: demande.id,
            coin: demande.coin,
            retrait,
            actions: &demande.actions,
            duree_ms: demande.duree.map(|d| d.as_millis() as u64),
            glisser: demande.glisser,
            reduites: demande.reduites,
            apercu: Taille {
                largeur: demande.apercu_taille.0,
                hauteur: demande.apercu_taille.1,
            },
        };
        if let Err(e) = self
            .app
            .emit_to(ETIQUETTE_VIGNETTE, "vignette:preparer", charge)
        {
            log::warn!("vignette: charge non envoyee: {e}");
        }
        self.surveiller(demande.id, envoi);
    }

    /// Joue le souffle de la carte `id` (fil principal). Faux s'il ne peut pas l'être
    /// (fenêtre absente, page pas prête, carte inconnue) : l'Application passe alors à
    /// l'entrée.
    pub fn jouer_souffle(&self, id: u64) -> bool {
        let Some(fenetre) = self.fenetre_souffle() else {
            return false;
        };
        let (depart, arrivee, ecran) = {
            let Some(etat) = self.etat() else {
                return false;
            };
            if !etat.souffle_pret {
                return false;
            }
            let Some(c) = etat.en_cours.as_ref().filter(|c| c.id() == id) else {
                return false;
            };
            let (Some((fen, echelle, ecran)), Some(carte)) = (c.place, c.carte) else {
                return false;
            };
            (
                c.demande.capture,
                zone_ecran(&carte, echelle, (fen.x, fen.y)),
                ecran,
            )
        };
        let marge = (MARGE_SOUFFLE_LOGIQUE * echelle_sure(ecran.echelle)).round() as u32;
        let Some(r) = place_souffle(depart, arrivee, ecran.rect, marge) else {
            return false;
        };
        poser(&fenetre, r);
        traverser(&fenetre, true);
        montrer(&fenetre);
        log::info!(
            "souffle joue {id} depart {},{} {}x{} arrivee {},{} {}x{}",
            depart.x,
            depart.y,
            depart.largeur,
            depart.hauteur,
            arrivee.x,
            arrivee.y,
            arrivee.largeur,
            arrivee.hauteur
        );
        if let Some(mut etat) = self.etat() {
            if let Some(c) = etat.en_cours.as_mut().filter(|c| c.id() == id) {
                c.phase = Phase::Souffle;
                c.depuis = Instant::now();
                c.souffle_visible = true;
            }
        }
        let charge = serde_json::json!({
            "id": id,
            "depart": RectCharge::relatif(depart, r),
            "arrivee": RectCharge::relatif(arrivee, r),
        });
        if let Err(e) = self.app.emit_to(ETIQUETTE_SOUFFLE, "souffle:jouer", charge) {
            log::warn!("souffle: charge non envoyee: {e}");
        }
        true
    }

    /// Fin du souffle (page ou filet) : signalée une fois.
    fn souffle_fini(&self, id: u64, d_office: bool) {
        let ok = self.etat().is_some_and(|mut e| {
            match e
                .en_cours
                .as_mut()
                .filter(|c| c.id() == id && c.phase == Phase::Souffle)
            {
                Some(c) => {
                    c.phase = Phase::Prete;
                    true
                }
                None => false,
            }
        });
        if !ok {
            return;
        }
        if d_office {
            log::warn!("souffle termine d'office {id}");
        } else {
            log::info!("souffle termine {id}");
        }
        (self.rappel)(Signal::SouffleFini { id, d_office });
    }

    /// Montre la carte et lance la minuterie (fil principal).
    pub fn entrer(&self, id: u64) {
        let Some(fenetre) = self.fenetre() else {
            return;
        };
        let ok =
            self.etat()
                .is_some_and(|mut e| match e.en_cours.as_mut().filter(|c| c.id() == id) {
                    Some(c) => {
                        c.phase = Phase::Entree;
                        c.depuis = Instant::now();
                        c.minuterie.demarrer();
                        true
                    }
                    None => false,
                });
        if !ok {
            return;
        }
        traverser(&fenetre, true);
        montrer(&fenetre);
        let _ = self.app.emit_to(
            ETIQUETTE_VIGNETTE,
            "vignette:entrer",
            serde_json::json!({ "id": id }),
        );
        self.envoyer_minuterie(id);
    }

    /// La page a commencé l'entrée : le souffle peut disparaître.
    fn entree(&self, id: u64) {
        let souffle = self.etat().and_then(|mut e| {
            let c = e
                .en_cours
                .as_mut()
                .filter(|c| c.id() == id && c.phase == Phase::Entree)?;
            c.phase = Phase::Montree;
            Some(std::mem::replace(&mut c.souffle_visible, false))
        });
        let Some(souffle_visible) = souffle else {
            return;
        };
        if souffle_visible {
            self.cacher_souffle(id);
        }
        log::info!("vignette entree {id}");
        (self.rappel)(Signal::Entree { id });
    }

    /// Envoie l'état de la minuterie à la page.
    fn envoyer_minuterie(&self, id: u64) {
        let m = self.etat().and_then(|e| {
            let c = e.en_cours.as_ref().filter(|c| c.id() == id)?;
            Some((c.minuterie, c.survol || c.glisser))
        });
        let Some((m, pause)) = m else {
            return;
        };
        let charge = serde_json::json!({
            "id": id,
            "restant_ms": m.restant().as_millis() as u64,
            "total_ms": m.total().map(|t| t.as_millis() as u64),
            "en_marche": m.en_marche() && !pause,
        });
        let _ = self
            .app
            .emit_to(ETIQUETTE_VIGNETTE, "vignette:minuterie", charge);
    }

    /// Résultat d'une action pour la page ; une réussite remet la minuterie à plein.
    pub fn confirmer(&self, id: u64, action: &str, reussi: bool) {
        let present =
            self.etat()
                .is_some_and(|mut e| match e.en_cours.as_mut().filter(|c| c.id() == id) {
                    Some(c) => {
                        if reussi && !c.supprimee {
                            c.minuterie.remettre(c.demande.duree);
                        }
                        true
                    }
                    None => false,
                });
        if !present {
            return;
        }
        let charge = serde_json::json!({ "id": id, "action": action, "reussi": reussi });
        let _ = self
            .app
            .emit_to(ETIQUETTE_VIGNETTE, "vignette:confirmer", charge);
        self.envoyer_minuterie(id);
    }

    /// Carte « supprimée » (minuterie sur la durée d'annulation) ou rétablie (à plein).
    pub fn marquer_supprimee(&self, id: u64, oui: bool) {
        let present =
            self.etat()
                .is_some_and(|mut e| match e.en_cours.as_mut().filter(|c| c.id() == id) {
                    Some(c) => {
                        c.supprimee = oui;
                        let total = if oui {
                            Some(c.demande.duree_annulation)
                        } else {
                            c.demande.duree
                        };
                        c.minuterie.remettre(total);
                        c.signale_delai = false;
                        true
                    }
                    None => false,
                });
        if !present {
            return;
        }
        let charge = serde_json::json!({ "id": id, "oui": oui });
        let _ = self
            .app
            .emit_to(ETIQUETTE_VIGNETTE, "vignette:supprimee", charge);
        self.envoyer_minuterie(id);
    }

    /// Un glisser commence ou finit : minuterie en pause, fenêtre laissée telle quelle.
    pub fn glisser_en_cours(&self, id: u64, oui: bool) {
        let present =
            self.etat()
                .is_some_and(|mut e| match e.en_cours.as_mut().filter(|c| c.id() == id) {
                    Some(c) => {
                        c.glisser = oui;
                        true
                    }
                    None => false,
                });
        if !present {
            return;
        }
        log::info!(
            "vignette minuterie {id} {}",
            if oui { "pause glisser" } else { "reprise" }
        );
        let charge = serde_json::json!({ "id": id, "en_cours": oui });
        let _ = self
            .app
            .emit_to(ETIQUETTE_VIGNETTE, "vignette:glisser", charge);
        self.envoyer_minuterie(id);
    }

    /// La carte part : en coup de vent (`animee`), ou tout de suite.
    pub fn partir(&self, id: u64, animee: bool) {
        let present =
            self.etat()
                .is_some_and(|mut e| match e.en_cours.as_mut().filter(|c| c.id() == id) {
                    Some(c) => {
                        c.phase = Phase::Partie;
                        c.depuis = Instant::now();
                        c.recoit_clics = false;
                        true
                    }
                    None => false,
                });
        if !present {
            return;
        }
        if let Some(f) = self.fenetre() {
            traverser(&f, true);
        }
        if !animee {
            self.cacher();
            return;
        }
        let charge = serde_json::json!({ "id": id, "animee": animee });
        let _ = self
            .app
            .emit_to(ETIQUETTE_VIGNETTE, "vignette:partir", charge);
    }

    /// Cache tout de suite la carte et le souffle, sans animation (avant une photo, à
    /// l'arrêt). Rend vrai s'il y avait une carte en cours.
    pub fn cacher(&self) -> bool {
        let id = self
            .etat()
            .and_then(|mut e| e.en_cours.take().map(|c| c.id()));
        if let Some(f) = self.fenetre() {
            cacher(&f);
        }
        if let Some(id) = id {
            self.cacher_souffle(id);
            let _ = self.app.emit_to(
                ETIQUETTE_VIGNETTE,
                "vignette:cachee",
                serde_json::json!({ "id": id }),
            );
        }
        id.is_some()
    }

    /// Cache la fenêtre du souffle, puis la page se vide (rien de l'envol au prochain
    /// affichage).
    fn cacher_souffle(&self, id: u64) {
        if let Some(f) = self.fenetre_souffle() {
            cacher(&f);
        }
        let charge = serde_json::json!({ "id": id });
        let _ = self
            .app
            .emit_to(ETIQUETTE_SOUFFLE, "souffle:effacer", charge);
    }

    /// La page a fini le départ : fenêtre cachée.
    fn terminee(&self, id: u64) {
        let present = self.etat().is_some_and(|mut e| {
            if e.en_cours
                .as_ref()
                .is_some_and(|c| c.id() == id && c.phase == Phase::Partie)
            {
                e.en_cours = None;
                true
            } else {
                false
            }
        });
        if !present {
            return;
        }
        if let Some(f) = self.fenetre() {
            cacher(&f);
        }
        log::info!("vignette terminee {id}");
        (self.rappel)(Signal::Terminee { id });
    }

    /// À l'arrêt : tout caché puis détruit.
    pub fn fermer(&self) {
        self.vivante.store(false, Ordering::SeqCst);
        self.cacher();
        for f in [self.fenetre(), self.fenetre_souffle()]
            .into_iter()
            .flatten()
        {
            let _ = f.destroy();
        }
    }

    /// Fil de surveillance de la carte (envoi `envoi`) : filets, souris, minuterie.
    fn surveiller(&self, id: u64, envoi: u64) {
        let app = self.app.clone();
        let vivante = Arc::clone(&self.vivante);
        let identifiant = self.identifiant;
        let lance = std::thread::Builder::new()
            .name("vignette-souris".into())
            .spawn(move || {
                let mut attente = place::INTERVALLE_PROCHE;
                let mut avant = Instant::now();
                while vivante.load(Ordering::SeqCst) {
                    std::thread::sleep(attente);
                    let maintenant = Instant::now();
                    let dt = maintenant.saturating_duration_since(avant);
                    avant = maintenant;
                    let Some(v) = app.try_state::<Vignette>() else {
                        break;
                    };
                    let pos = souris(&app);
                    let reelle = identifiant.and_then(place_reelle);
                    let Some(mut etat) = v.etat() else {
                        break;
                    };
                    let Some(c) = etat
                        .en_cours
                        .as_mut()
                        .filter(|c| c.id() == id && c.envoi == envoi)
                    else {
                        break;
                    };
                    let depuis = maintenant.saturating_duration_since(c.depuis);
                    match c.phase {
                        Phase::Preparee if depuis >= DELAI_ZONE => {
                            drop(etat);
                            (v.rappel)(Signal::Impossible {
                                id,
                                cause: "page muette",
                            });
                            break;
                        }
                        Phase::Souffle if depuis >= DELAI_SOUFFLE => {
                            drop(etat);
                            v.sur_fil_principal(move |v| {
                                v.cacher_souffle(id);
                                v.souffle_fini(id, true);
                            });
                            continue;
                        }
                        Phase::Entree if depuis >= DELAI_ENTREE => {
                            drop(etat);
                            v.sur_fil_principal(move |v| v.entree(id));
                            continue;
                        }
                        Phase::Partie if depuis >= DELAI_DEPART => {
                            etat.en_cours = None;
                            drop(etat);
                            v.sur_fil_principal(move |v| {
                                if let Some(f) = v.fenetre() {
                                    cacher(&f);
                                }
                                log::warn!("vignette cachee d'office {id}");
                                (v.rappel)(Signal::Terminee { id });
                            });
                            break;
                        }
                        Phase::Entree | Phase::Montree => {}
                        _ => continue,
                    }
                    // Souris : la fenêtre ne reçoit les clics que sur la carte.
                    let (fen, echelle) = match reelle.or(c.place.map(|(r, e, _)| (r, e))) {
                        Some((r, e)) => ((r.x, r.y), e),
                        None => continue,
                    };
                    let carte = c.carte.map(|z| zone_ecran(&z, echelle, fen));
                    let dedans = cliquable(true, c.carte.as_ref(), echelle, fen, pos);
                    attente = intervalle(pos, carte.as_ref());
                    let mut gestes: Vec<Geste> = Vec::new();
                    if !c.glisser && dedans != c.recoit_clics {
                        c.recoit_clics = dedans;
                        gestes.push(Box::new(move |v: &Vignette| v.geste_clics(id, dedans)));
                    }
                    if dedans != c.survol && !c.glisser {
                        c.survol = dedans;
                        log::info!(
                            "vignette minuterie {id} {}",
                            if dedans { "pause survol" } else { "reprise" }
                        );
                        gestes.push(Box::new(move |v: &Vignette| {
                            let charge = serde_json::json!({ "id": id, "dedans": dedans });
                            let _ = v.app.emit_to(ETIQUETTE_VIGNETTE, "vignette:survol", charge);
                            v.envoyer_minuterie(id);
                        }));
                    }
                    let pause = c.survol || c.glisser;
                    if c.minuterie.avancer(dt, pause) && !c.signale_delai {
                        c.signale_delai = true;
                        drop(etat);
                        (v.rappel)(Signal::DelaiEcoule { id });
                    } else {
                        drop(etat);
                    }
                    for g in gestes {
                        v.sur_fil_principal(g);
                    }
                }
            });
        if let Err(e) = lance {
            log::warn!("vignette: surveillance impossible: {e}");
        }
    }

    /// Geste posté par la surveillance (fil principal) : relu sous le verrou.
    fn geste_clics(&self, id: u64, voulu: bool) {
        let cliquable = self.etat().is_some_and(|e| {
            e.en_cours.as_ref().is_some_and(|c| {
                c.id() == id && c.recoit_clics == voulu && c.phase != Phase::Partie
            }) && voulu
        });
        if let Some(f) = self.fenetre() {
            traverser(&f, !cliquable);
        }
    }

    fn page_prete(&self) {
        // Une carte partie ne reçoit plus sa charge (la page rechargée reste vide).
        let en_cours = self.etat().is_some_and(|mut e| {
            e.page_prete = true;
            e.en_cours
                .as_ref()
                .is_some_and(|c| c.phase != Phase::Partie)
        });
        log::info!("vignette prete");
        if en_cours {
            self.envoyer();
        }
    }
}

/// Montre sans activer (sous Windows par `systeme`, sinon par Tauri).
fn montrer(fenetre: &WebviewWindow) {
    match systeme::montrer_sans_activer(fenetre) {
        Ok(true) => {}
        Ok(false) => {
            if let Err(e) = fenetre.show() {
                log::warn!("{}: affichage impossible: {e}", fenetre.label());
            }
        }
        Err(e) => log::warn!("{}: affichage impossible: {e}", fenetre.label()),
    }
}

// Commandes des deux pages. Un `id` qui n'est pas celui en cours est ignoré.

/// La page de la vignette écoute ses événements.
#[tauri::command]
pub fn vignette_pret(vignette: State<'_, Vignette>) {
    theme::envoyer_a(&vignette.app, ETIQUETTE_VIGNETTE);
    vignette.page_prete();
}

/// La page du souffle écoute ses événements.
#[tauri::command]
pub fn souffle_pret(vignette: State<'_, Vignette>) {
    theme::envoyer_a(&vignette.app, ETIQUETTE_SOUFFLE);
    if let Some(mut e) = vignette.etat() {
        e.souffle_pret = true;
    }
    log::info!("souffle pret");
}

/// L'aperçu de la carte `id`, en PNG.
#[tauri::command]
pub fn vignette_image(
    vignette: State<'_, Vignette>,
    id: u64,
    quoi: String,
) -> Result<tauri::ipc::Response, String> {
    if quoi != "apercu" {
        return Err("image inconnue".into());
    }
    let octets = vignette
        .etat()
        .and_then(|e| {
            e.en_cours
                .as_ref()
                .filter(|c| c.id() == id)
                .map(|c| Arc::clone(&c.demande.apercu))
        })
        .ok_or("vignette inconnue")?;
    Ok(tauri::ipc::Response::new(octets.as_ref().clone()))
}

/// La carte est posée (pixels CSS) ; ses boutons servent au journal et aux essais.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn vignette_zone(
    vignette: State<'_, Vignette>,
    id: u64,
    x: f64,
    y: f64,
    largeur: f64,
    hauteur: f64,
    boutons: Vec<BoutonCss>,
) {
    let Some(zone) = borner_zone_dans(x, y, largeur, hauteur, TAILLE_LOGIQUE_VIGNETTE) else {
        log::warn!("vignette zone {id} refusee (valeur non finie)");
        return;
    };
    let (place, premiere) = {
        let Some(mut e) = vignette.etat() else {
            return;
        };
        let Some(c) = e.en_cours.as_mut().filter(|c| c.id() == id) else {
            return;
        };
        c.carte = Some(zone);
        let premiere = c.phase == Phase::Preparee;
        if premiere {
            c.phase = Phase::Prete;
        }
        (c.place, premiere)
    };
    if let Some((p, echelle, _)) = place {
        let r = zone_ecran(&zone, echelle, (p.x, p.y));
        log::info!(
            "vignette zone {id} {},{} {}x{}",
            r.x,
            r.y,
            r.largeur,
            r.hauteur
        );
        for b in &boutons {
            if let Some(z) =
                borner_zone_dans(b.x, b.y, b.largeur, b.hauteur, TAILLE_LOGIQUE_VIGNETTE)
            {
                let r = zone_ecran(&z, echelle, (p.x, p.y));
                let nom: String = b
                    .action
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .take(32)
                    .collect();
                log::info!(
                    "vignette bouton {id} {nom} {},{} {}x{}",
                    r.x,
                    r.y,
                    r.largeur,
                    r.hauteur
                );
            }
        }
    }
    if premiere {
        (vignette.rappel)(Signal::Prete { id });
    }
}

/// La carte commence à entrer.
#[tauri::command]
pub fn vignette_entree(vignette: State<'_, Vignette>, id: u64) {
    vignette.entree(id);
}

/// L'envol est fini.
#[tauri::command]
pub fn souffle_termine(vignette: State<'_, Vignette>, id: u64) {
    // La dernière image reste jusqu'à l'entrée de la carte : rien à cacher ici.
    vignette.souffle_fini(id, false);
}

/// Clic sur un bouton de la carte.
#[tauri::command]
pub fn vignette_action(vignette: State<'_, Vignette>, id: u64, action: String) {
    let valide = vignette.etat().is_some_and(|e| {
        e.en_cours
            .as_ref()
            .is_some_and(|c| c.id() == id && matches!(c.phase, Phase::Entree | Phase::Montree))
    });
    if valide {
        (vignette.rappel)(Signal::Action { id, action });
    }
}

/// Appui sur l'image puis déplacement : l'Application lancera le glisser.
#[tauri::command]
pub fn vignette_glisser(vignette: State<'_, Vignette>, id: u64) {
    let valide = vignette.etat().is_some_and(|e| {
        e.en_cours
            .as_ref()
            .is_some_and(|c| c.id() == id && matches!(c.phase, Phase::Entree | Phase::Montree))
    });
    if valide {
        (vignette.rappel)(Signal::Glisser { id });
    }
}

/// Le départ est fini.
#[tauri::command]
pub fn vignette_terminee(vignette: State<'_, Vignette>, id: u64) {
    vignette.terminee(id);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, largeur: u32, hauteur: u32) -> Rect {
        Rect {
            x,
            y,
            largeur,
            hauteur,
        }
    }

    fn ecran(r: Rect, utile: Rect, echelle: f64) -> Ecran {
        Ecran {
            rect: r,
            zone_utile: utile,
            echelle,
        }
    }

    #[test]
    fn place_aux_quatre_coins_a_100() {
        // 1024 × 768, barre des tâches de 48 px en bas.
        let e = ecran(rect(0, 0, 1024, 768), rect(0, 0, 1024, 720), 1.0);
        let t = TAILLE_LOGIQUE_VIGNETTE;
        let (r, re) = placer_vignette(&e, Coin::BasGauche, t, DEBORD_LOGIQUE);
        assert_eq!(r, rect(0, 768 - 280, 340, 280));
        assert_eq!(
            re,
            Retrait {
                bas: 48,
                ..Retrait::default()
            }
        );
        let (r, re) = placer_vignette(&e, Coin::BasDroite, t, DEBORD_LOGIQUE);
        assert_eq!(r, rect(1024 - 340, 488, 340, 280));
        assert_eq!(
            re,
            Retrait {
                bas: 48,
                ..Retrait::default()
            }
        );
        let (r, re) = placer_vignette(&e, Coin::HautGauche, t, DEBORD_LOGIQUE);
        assert_eq!(r, rect(0, 0, 340, 280));
        assert_eq!(re, Retrait::default());
        let (r, _) = placer_vignette(&e, Coin::HautDroite, t, DEBORD_LOGIQUE);
        assert_eq!(r, rect(684, 0, 340, 280));
    }

    #[test]
    fn place_a_150_et_barre_a_gauche_sur_un_ecran_a_gauche() {
        // Écran à gauche du principal, barre des tâches de 72 px réels à gauche.
        let e = ecran(rect(-2880, 0, 2880, 1800), rect(-2808, 0, 2808, 1800), 1.5);
        let (r, re) = placer_vignette(&e, Coin::BasGauche, (340.0, 280.0), 48.0);
        // Débord : 72 réels, l'écran n'arrête rien ; en bas, la zone utile touche l'écran.
        assert_eq!(r, rect(-2880, 1800 - 420, 510, 420));
        assert_eq!(
            re,
            Retrait {
                gauche: 72,
                ..Retrait::default()
            }
        );
        // Grande barre (200 réels) : le débord s'arrête à 72 (48 × 1,5).
        let e = ecran(rect(0, 0, 2880, 1800), rect(0, 0, 2880, 1600), 1.5);
        let (r, re) = placer_vignette(&e, Coin::BasDroite, (340.0, 280.0), 48.0);
        assert_eq!(re.bas, 72);
        assert_eq!((r.x, r.y + r.hauteur as i32), (2880 - 510, 1672));
    }

    #[test]
    fn souffle_union_marge_et_ecran() {
        let e = rect(0, 0, 1024, 768);
        let depart = rect(100, 100, 300, 200);
        let arrivee = rect(12, 550, 260, 162);
        assert_eq!(
            place_souffle(depart, arrivee, e, 48),
            Some(rect(0, 52, 448, 708))
        );
        // Plein écran : coupé à l'écran.
        assert_eq!(place_souffle(e, arrivee, e, 48), Some(e));
        // Rien sur l'écran.
        assert_eq!(
            place_souffle(rect(5000, 0, 10, 10), rect(5000, 0, 10, 10), e, 0),
            None
        );
    }

    #[test]
    fn minuterie_pause_reprise_et_toujours() {
        let s = Duration::from_millis;
        let mut m = Minuterie::nouvelle(Some(s(3000)));
        assert!(
            !m.avancer(s(5000), false),
            "arrêtée tant qu'elle n'a pas démarré"
        );
        m.demarrer();
        assert!(!m.avancer(s(1000), false));
        assert!(!m.avancer(s(5000), true), "en pause");
        assert_eq!(m.restant(), s(2000));
        assert!(m.avancer(s(2000), false));
        assert!(!m.avancer(s(10), false), "une seule fois");
        m.remettre(Some(s(3000)));
        assert_eq!(m.restant(), s(3000));
        assert!(m.en_marche());
        let mut t = Minuterie::nouvelle(None);
        t.demarrer();
        assert!(!t.avancer(Duration::from_secs(3600), false));
        assert_eq!(t.total(), None);
    }

    #[test]
    fn coin_et_charge() {
        assert_eq!(Coin::HautDroite.nom(), "haut_droite");
        let v = serde_json::to_value(Coin::BasGauche).unwrap();
        assert_eq!(v, serde_json::json!("bas_gauche"));
        assert_eq!(
            serde_json::to_value(Place::Annulation).unwrap(),
            "annulation"
        );
        let r = RectCharge::relatif(rect(10, 20, 5, 6), rect(4, 4, 100, 100));
        assert_eq!((r.x, r.y, r.largeur, r.hauteur), (6, 16, 5, 6));
    }
}
