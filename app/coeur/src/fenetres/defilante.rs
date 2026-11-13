//! Fenêtre de contrôle de la capture défilante (contrat 0.6, §10, §11, §18) : une fenêtre
//! outil (`creer_fenetre_outil`) cachée, sans focus, transparente, toujours au-dessus,
//! exclue des captures (sauf `BREEZYSHOT_ESSAI_CAPTURABLE=1`), clics et molette
//! traversants sauf sur la pilule quand la souris y est (comme le bandeau). Posée sur tout
//! l'écran de la zone (zone utile et barre des tâches), à l'échelle de cet écran. Une
//! seule ; créée au début de la visée (R1), par un tour suivant de la boucle, jamais dans
//! un rappel ; détruite à la fin de la capture.
//!
//! La page pose ce qu'on lui dit (voile, trait, aperçu, pilule, message) et rapporte la
//! place de la pilule et le clic sur « Terminer » par les commandes de
//! `application::fenetre_defilante`. Le module crée, place, montre, cache, prévient la page
//! et détruit ; il ne décide de rien.
//!
//! Charges envoyées à la page (pixels réels relatifs au coin de la fenêtre) :
//! - `defilante:montrer` : `ChargeMontrer` ;
//! - `defilante:apercu` : `ChargeApercu` (la page demande les octets par
//!   `defilante_apercu`) ;
//! - `defilante:message` : `ChargeMessage` (`texte` : le texte venu du cœur, ou `null`) ;
//! - `defilante:cacher` : `ChargeCacher` ;
//! - `interface:theme` (`theme::envoyer_a`).
//!
//! Clics : la fenêtre part toujours traversée (clics et molette vont à la page dessous,
//! mesure S9) ; un fil `defilante-souris` suit la souris pendant qu'elle est montrée et ne
//! la rend cliquable que lorsque la souris est sur la pilule (`pilule`), comme le bandeau.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use super::place::{INTERVALLE_LOIN, INTERVALLE_PROCHE};
use crate::systeme::{self, Rect};

/// Étiquette de la fenêtre (aussi dans `capabilities/defilante.json`).
pub const ETIQUETTE_DEFILANTE: &str = "defilante";
/// Page de la fenêtre, sous `app/interface/`.
pub const PAGE: &str = "defilante/defilante.html";
/// Libellé de l'exclusion au journal : `defilante exclue des captures: oui|non|non (essai)`.
pub const EXCLUSION: &str = "defilante exclue";

/// Événements écoutés par la page (§11).
pub const EVENEMENT_MONTRER: &str = "defilante:montrer";
pub const EVENEMENT_APERCU: &str = "defilante:apercu";
pub const EVENEMENT_MESSAGE: &str = "defilante:message";
pub const EVENEMENT_CACHER: &str = "defilante:cacher";

/// Un rectangle de la page, en pixels réels relatifs au coin de la fenêtre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RectPage {
    pub x: i32,
    pub y: i32,
    pub largeur: u32,
    pub hauteur: u32,
}

/// La place de l'aperçu en direct : la hauteur est la hauteur maximale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ApercuPage {
    pub x: i32,
    pub y: i32,
    pub largeur: u32,
    pub hauteur_max: u32,
}

/// Un point de la page, en pixels réels relatifs au coin de la fenêtre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PointPage {
    pub x: i32,
    pub y: i32,
}

/// `defilante:montrer` : la capture `id` commence ; tout ce que la page doit poser.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChargeMontrer {
    pub id: u64,
    /// L'échelle de l'écran de la zone.
    pub echelle: f64,
    pub zone: RectPage,
    pub apercu: ApercuPage,
    pub pilule: PointPage,
    /// « Terminer » (le texte vient du cœur).
    pub libelle: String,
    pub reduites: bool,
}

/// `defilante:apercu` : l'aperçu `numero` est prêt, haut de `hauteur` pixels réels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ChargeApercu {
    pub id: u64,
    pub numero: u64,
    pub hauteur: u32,
}

/// `defilante:message` : le message à montrer en haut de l'écran, ou `None` pour le cacher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChargeMessage {
    pub id: u64,
    pub texte: Option<String>,
}

/// `defilante:cacher` : la capture `id` est finie ; la page efface tout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ChargeCacher {
    pub id: u64,
}

/// Taille logique de la fenêtre à sa création, avant d'être posée sur l'écran de la zone.
const TAILLE_INITIALE: (f64, f64) = (200.0, 200.0);

/// Taille de la pilule « Terminer » dessinée par la page, en pixels logiques, ombre non
/// comprise (bord 1 + marge 4 + bouton 90 au moins × 32 + marge 4 + bord 1, jetons
/// `defilante-*`). Pour le placement du cœur : la page rapporte la place réelle par
/// `defilante_zone`.
pub const TAILLE_PILULE_LOGIQUE: (f64, f64) = (100.0, 42.0);

/// `r` en rectangle de `place` (pixels réels du bureau), pour `contient` et `intervalle`.
fn rect_place(r: Rect) -> super::Rect {
    super::Rect {
        x: r.x,
        y: r.y,
        largeur: r.largeur.max(0) as u32,
        hauteur: r.hauteur.max(0) as u32,
    }
}

/// La fenêtre doit-elle recevoir les clics ? Seulement montrée, avec une pilule, et quand
/// la souris est dessus.
fn cliquable(montree: bool, pilule: Option<Rect>, souris: Option<(f64, f64)>) -> bool {
    match (montree, pilule, souris) {
        (true, Some(r), Some((x, y))) => !r.est_vide() && rect_place(r).contient(x, y),
        _ => false,
    }
}

/// La pilule retenue pour la surveillance : un rectangle vide n'en est pas une.
fn pilule_retenue(rect: Option<Rect>) -> Option<Rect> {
    rect.filter(|r| !r.est_vide())
}

/// Le fil de surveillance de l'affichage `affichage` continue-t-il ? Non dès que la
/// fenêtre est détruite, cachée, ou qu'un autre affichage a commencé (`courant`).
fn fil_continue(vivante: bool, courant: u64, affichage: u64, montree: bool) -> bool {
    vivante && montree && courant == affichage
}

/// Le geste « cliquable » posté au fil principal s'applique-t-il encore ? Seulement s'il
/// le demande, que la fenêtre est toujours montrée avec une pilule, et que son affichage
/// (`affichage_geste`) est l'affichage en cours (`courant`) ; sinon les clics traversent.
fn recoit_apres_geste(
    voulu: bool,
    montree: bool,
    pilule: bool,
    courant: u64,
    affichage_geste: u64,
) -> bool {
    voulu && montree && pilule && courant == affichage_geste
}

/// Rythme de la surveillance : proche de la pilule, plus serré.
fn intervalle(pilule: Option<Rect>, souris: Option<(f64, f64)>) -> Duration {
    let (Some(r), Some(p)) = (pilule, souris) else {
        return INTERVALLE_LOIN;
    };
    super::intervalle(Some(p), Some(&rect_place(r)))
}

/// Ce que partagent la fenêtre de contrôle et son fil de surveillance.
#[derive(Debug, Default)]
struct Souris {
    /// Vrai entre `montrer` et `cacher`.
    montree: bool,
    /// Rectangle de la pilule, pixels réels du bureau.
    pilule: Option<Rect>,
    /// Ce que la fenêtre fait des clics : vrai = elle les reçoit.
    recoit: bool,
}

#[derive(Debug, Default)]
struct Partage {
    souris: Mutex<Souris>,
    /// Numéro de l'affichage en cours : un fil d'un affichage passé s'arrête.
    affichage: AtomicU64,
}

impl Partage {
    fn souris(&self) -> MutexGuard<'_, Souris> {
        // Un fil qui a paniqué en tenant le verrou ne laisse que des valeurs simples.
        self.souris.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// La fenêtre de contrôle créée (cachée). À garder par l'Application le temps d'une
/// capture, puis `detruire`. Ses gestes se font sur le fil principal.
#[derive(Debug)]
pub struct Controle {
    app: AppHandle,
    /// Identifiant système (Windows : le HWND), rendu par `identifiant()` (journal,
    /// conversion des places par le cœur).
    identifiant: Option<isize>,
    /// Faux une fois la fenêtre détruite.
    vivante: Arc<AtomicBool>,
    partage: Arc<Partage>,
    /// L'écran où la fenêtre est posée (pixels réels du bureau).
    ecran: Mutex<Option<Rect>>,
    /// Vrai après `detruire` : `Drop` n'a plus rien à faire.
    detruite: AtomicBool,
}

/// Crée la fenêtre de contrôle, cachée (fil principal, jamais dans un rappel de page :
/// toujours postée à un tour suivant de la boucle). Journal : `defilante fenetre <hwnd>`,
/// `defilante exclue des captures: …`. Une fenêtre qui existe encore : une erreur.
pub fn creer(app: &AppHandle) -> Result<Controle, String> {
    if app.get_webview_window(ETIQUETTE_DEFILANTE).is_some() {
        return Err("fenetre de controle deja creee".into());
    }
    let outil =
        super::creer_fenetre_outil(app, ETIQUETTE_DEFILANTE, PAGE, TAILLE_INITIALE, EXCLUSION)?;
    Ok(Controle {
        app: app.clone(),
        identifiant: outil.identifiant,
        vivante: outil.vivante,
        partage: Arc::new(Partage::default()),
        ecran: Mutex::new(None),
        detruite: AtomicBool::new(false),
    })
}

impl Controle {
    fn fenetre(&self) -> Option<WebviewWindow> {
        self.app.get_webview_window(ETIQUETTE_DEFILANTE)
    }

    fn ecran(&self) -> Option<Rect> {
        *self.ecran.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Identifiant système de la fenêtre (Windows : le HWND).
    pub fn identifiant(&self) -> Option<isize> {
        self.identifiant
    }

    /// Pose la fenêtre sur tout `ecran` (pixels réels du bureau), sans la montrer : la page
    /// prend la taille de l'écran avant `defilante:montrer`. `montrer` la repose au-dessus
    /// de tout au même endroit.
    pub fn poser(&self, ecran: Rect) -> Result<(), String> {
        if ecran.est_vide() {
            return Err("ecran vide".into());
        }
        let fenetre = self.fenetre().ok_or("fenetre de controle absente")?;
        *self.ecran.lock().unwrap_or_else(|e| e.into_inner()) = Some(ecran);
        super::poser(
            &fenetre,
            super::Rect {
                x: ecran.x,
                y: ecran.y,
                largeur: ecran.largeur as u32,
                hauteur: ecran.hauteur as u32,
            },
        );
        Ok(())
    }

    /// Montre la fenêtre sans l'activer (jamais le focus ni le clavier), au-dessus de tout,
    /// clics traversants ; la surveillance de la souris part.
    pub fn montrer(&self) -> Result<(), String> {
        if !self.vivante.load(Ordering::SeqCst) {
            return Err("fenetre de controle detruite".into());
        }
        let fenetre = self.fenetre().ok_or("fenetre de controle absente")?;
        let par_systeme = match self.ecran() {
            Some(e) => systeme::poser_au_dessus(&fenetre, e)?,
            None => false,
        };
        if !par_systeme && !systeme::montrer_sans_activer(&fenetre)? {
            fenetre.show().map_err(|e| format!("show: {e}"))?;
        }
        super::traverser(&fenetre, true);
        let affichage = {
            let mut s = self.partage.souris();
            s.montree = true;
            s.recoit = false;
            self.partage.affichage.fetch_add(1, Ordering::SeqCst) + 1
        };
        self.surveiller(affichage);
        Ok(())
    }

    /// Cache la fenêtre (clics traversants) ; la surveillance s'arrête.
    pub fn cacher(&self) {
        {
            let mut s = self.partage.souris();
            s.montree = false;
            s.recoit = false;
            s.pilule = None;
        }
        self.partage.affichage.fetch_add(1, Ordering::SeqCst);
        if let Some(f) = self.fenetre() {
            super::cacher(&f);
        }
    }

    /// La pilule « Terminer » : son rectangle en pixels réels du bureau (`None` : aucune) ;
    /// la fenêtre n'est cliquable que là, quand la souris y est.
    pub fn pilule(&self, rect: Option<Rect>) {
        self.partage.souris().pilule = pilule_retenue(rect);
    }

    /// Envoie `charge` à la page sous le nom `evenement`.
    pub fn emettre<S: Serialize + Clone>(&self, evenement: &str, charge: S) -> Result<(), String> {
        self.app
            .emit_to(ETIQUETTE_DEFILANTE, evenement, charge)
            .map_err(|e| e.to_string())
    }

    /// Cache puis détruit la fenêtre. Journal en WARN si elle ne l'est pas :
    /// `defilante: fenetre non detruite: <e>`.
    pub fn detruire(self) -> Result<(), String> {
        self.detruite.store(true, Ordering::SeqCst);
        self.cacher();
        let Some(f) = self.fenetre() else {
            return Ok(());
        };
        match f.destroy() {
            Ok(()) => Ok(()),
            Err(e) => {
                log::warn!("defilante: fenetre non detruite: {e}");
                Err(e.to_string())
            }
        }
    }

    /// Fil de surveillance de l'affichage `affichage` : rend la fenêtre cliquable quand la
    /// souris est sur la pilule, traversée sinon. S'arrête à `cacher`, à un nouvel
    /// affichage, ou à la destruction de la fenêtre. Aucune méthode de la fenêtre Tauri ici
    /// (chacune attend le fil principal) : les gestes y sont postés.
    fn surveiller(&self, affichage: u64) {
        let app = self.app.clone();
        let vivante = Arc::clone(&self.vivante);
        let partage = Arc::clone(&self.partage);
        let lancement = std::thread::Builder::new()
            .name("defilante-souris".into())
            .spawn(move || {
                let mut attente = INTERVALLE_PROCHE;
                loop {
                    std::thread::sleep(attente);
                    let pos = super::souris(&app);
                    let mut s = partage.souris();
                    let continuer = fil_continue(
                        vivante.load(Ordering::SeqCst),
                        partage.affichage.load(Ordering::SeqCst),
                        affichage,
                        s.montree,
                    );
                    if !continuer {
                        break;
                    }
                    let voulu = cliquable(s.montree, s.pilule, pos);
                    attente = intervalle(s.pilule, pos);
                    if voulu == s.recoit {
                        continue;
                    }
                    s.recoit = voulu;
                    drop(s);
                    let pour = app.clone();
                    let partage = Arc::clone(&partage);
                    let _ = app.run_on_main_thread(move || {
                        geste_clics(&pour, &partage, affichage, voulu);
                    });
                }
            });
        if let Err(e) = lancement {
            log::warn!("defilante: surveillance impossible: {e}");
        }
    }
}

/// Filet : un `Controle` lâché sans `detruire` (chemin d'erreur) arrête son fil et fait
/// détruire sa fenêtre sur le fil principal, si c'est toujours la sienne (`vivante` : une
/// fenêtre détruite entre-temps n'est pas confondue avec une nouvelle de même étiquette).
impl Drop for Controle {
    fn drop(&mut self) {
        if self.detruite.load(Ordering::SeqCst) {
            return;
        }
        {
            let mut s = self.partage.souris();
            s.montree = false;
            s.recoit = false;
            s.pilule = None;
        }
        self.partage.affichage.fetch_add(1, Ordering::SeqCst);
        if !self.vivante.load(Ordering::SeqCst) {
            return;
        }
        log::warn!("defilante: fenetre lachee sans detruire");
        let app = self.app.clone();
        let vivante = Arc::clone(&self.vivante);
        let poste = self.app.run_on_main_thread(move || {
            if !vivante.load(Ordering::SeqCst) {
                return;
            }
            if let Some(f) = app.get_webview_window(ETIQUETTE_DEFILANTE) {
                super::cacher(&f);
                if let Err(e) = f.destroy() {
                    log::warn!("defilante: fenetre non detruite: {e}");
                }
            }
        });
        if let Err(e) = poste {
            log::warn!("defilante: fenetre non detruite: {e}");
        }
    }
}

/// Geste posté par la surveillance (fil principal) : l'état est relu sous le verrou ; la
/// fenêtre ne reçoit les clics que si l'affichage est toujours le même et montré.
fn geste_clics(app: &AppHandle, partage: &Partage, affichage: u64, voulu: bool) {
    let recoit = {
        let mut s = partage.souris();
        let ok = recoit_apres_geste(
            voulu,
            s.montree,
            s.pilule.is_some(),
            partage.affichage.load(Ordering::SeqCst),
            affichage,
        );
        s.recoit = ok;
        ok
    };
    if let Some(f) = app.get_webview_window(ETIQUETTE_DEFILANTE) {
        super::traverser(&f, !recoit);
    }
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

    #[test]
    fn cliquable_seulement_sur_la_pilule_montree() {
        let p = r(100, 200, 150, 63);
        assert!(cliquable(true, Some(p), Some((100.0, 200.0))));
        assert!(cliquable(true, Some(p), Some((249.9, 262.9))));
        // Bord droit et bas exclus.
        assert!(!cliquable(true, Some(p), Some((250.0, 230.0))));
        assert!(!cliquable(true, Some(p), Some((120.0, 263.0))));
        assert!(!cliquable(true, Some(p), Some((99.9, 230.0))));
        // Cachée, sans pilule, souris inconnue, pilule vide : traversée.
        assert!(!cliquable(false, Some(p), Some((120.0, 230.0))));
        assert!(!cliquable(true, None, Some((120.0, 230.0))));
        assert!(!cliquable(true, Some(p), None));
        assert!(!cliquable(
            true,
            Some(r(100, 200, 0, 63)),
            Some((100.0, 230.0))
        ));
    }

    #[test]
    fn cliquable_sur_un_ecran_a_gauche() {
        let p = r(-1700, -300, 150, 63);
        assert!(cliquable(true, Some(p), Some((-1650.0, -280.0))));
        assert!(!cliquable(true, Some(p), Some((1650.0, 280.0))));
    }

    #[test]
    fn rythme_selon_la_distance() {
        let p = r(1000, 800, 150, 63);
        assert_eq!(
            intervalle(Some(p), Some((1050.0, 820.0))),
            INTERVALLE_PROCHE
        );
        assert_eq!(intervalle(Some(p), Some((10.0, 10.0))), INTERVALLE_LOIN);
        assert_eq!(intervalle(None, Some((1050.0, 820.0))), INTERVALLE_LOIN);
        assert_eq!(intervalle(Some(p), None), INTERVALLE_LOIN);
    }

    #[test]
    fn le_fil_s_arrete_detruite_cachee_ou_remplacee() {
        assert!(fil_continue(true, 3, 3, true));
        // Fenêtre détruite (`Destroyed`).
        assert!(!fil_continue(false, 3, 3, true));
        // `cacher`, `detruire` ou `Drop` : montree faux et numéro avancé.
        assert!(!fil_continue(true, 3, 3, false));
        assert!(!fil_continue(true, 4, 3, true));
        // Un nouveau `montrer` : le fil de l'affichage 3 laisse la place à celui du 4.
        assert!(!fil_continue(true, 4, 3, true));
        assert!(fil_continue(true, 4, 4, true));
    }

    #[test]
    fn le_geste_cliquable_ne_vaut_que_pour_son_affichage() {
        assert!(recoit_apres_geste(true, true, true, 5, 5));
        // Un geste « traversée » reste traversée.
        assert!(!recoit_apres_geste(false, true, true, 5, 5));
        // Affichage périmé : caché puis remontré pendant que le geste était en route.
        assert!(!recoit_apres_geste(true, true, true, 6, 5));
        // Fenêtre cachée.
        assert!(!recoit_apres_geste(true, false, true, 5, 5));
        // Pilule retirée.
        assert!(!recoit_apres_geste(true, true, false, 5, 5));
    }

    #[test]
    fn la_pilule_vide_est_oubliee() {
        assert_eq!(pilule_retenue(Some(r(5, 5, 0, 4))), None);
        assert_eq!(pilule_retenue(Some(r(5, 5, 4, -1))), None);
        assert_eq!(pilule_retenue(None), None);
        assert_eq!(pilule_retenue(Some(r(5, 5, 4, 4))), Some(r(5, 5, 4, 4)));
    }

    #[test]
    fn charges_au_format_de_la_page() {
        let montrer = ChargeMontrer {
            id: 4,
            echelle: 1.25,
            zone: RectPage {
                x: 100,
                y: 80,
                largeur: 600,
                hauteur: 450,
            },
            apercu: ApercuPage {
                x: 715,
                y: 80,
                largeur: 105,
                hauteur_max: 800,
            },
            pilule: PointPage { x: 338, y: 543 },
            libelle: "Terminer".into(),
            reduites: false,
        };
        assert_eq!(
            serde_json::to_value(&montrer).unwrap(),
            serde_json::json!({
                "id": 4, "echelle": 1.25,
                "zone": { "x": 100, "y": 80, "largeur": 600, "hauteur": 450 },
                "apercu": { "x": 715, "y": 80, "largeur": 105, "hauteur_max": 800 },
                "pilule": { "x": 338, "y": 543 },
                "libelle": "Terminer", "reduites": false
            })
        );
        assert_eq!(
            serde_json::to_value(ChargeApercu {
                id: 4,
                numero: 7,
                hauteur: 312
            })
            .unwrap(),
            serde_json::json!({ "id": 4, "numero": 7, "hauteur": 312 })
        );
        assert_eq!(
            serde_json::to_value(ChargeMessage { id: 4, texte: None }).unwrap(),
            serde_json::json!({ "id": 4, "texte": null })
        );
        assert_eq!(
            serde_json::to_value(ChargeMessage {
                id: 4,
                texte: Some("Trop vite. Remontez un peu.".into())
            })
            .unwrap(),
            serde_json::json!({ "id": 4, "texte": "Trop vite. Remontez un peu." })
        );
        assert_eq!(
            serde_json::to_value(ChargeCacher { id: 4 }).unwrap(),
            serde_json::json!({ "id": 4 })
        );
    }

    #[test]
    fn noms_du_contrat() {
        assert_eq!(ETIQUETTE_DEFILANTE, "defilante");
        assert_eq!(PAGE, "defilante/defilante.html");
        assert_eq!(EVENEMENT_MONTRER, "defilante:montrer");
        assert_eq!(EVENEMENT_APERCU, "defilante:apercu");
        assert_eq!(EVENEMENT_MESSAGE, "defilante:message");
        assert_eq!(EVENEMENT_CACHER, "defilante:cacher");
        let capacites: serde_json::Value =
            serde_json::from_str(include_str!("../../capabilities/defilante.json")).unwrap();
        assert_eq!(
            capacites["windows"],
            serde_json::json!([ETIQUETTE_DEFILANTE])
        );
        let page = concat!(env!("CARGO_MANIFEST_DIR"), "/../interface/");
        assert!(std::path::Path::new(page).join(PAGE).is_file());
    }
}
