//! Fenêtre de contrôle d'un film (vidéo et GIF, contrat 0.7, §10, §11, §12) : une fenêtre
//! outil (`creer_fenetre_outil(app, "film", "film/film.html", taille, "film exclue")`)
//! cachée, sans focus, transparente, toujours au-dessus, exclue des captures (sauf
//! `BREEZYSHOT_ESSAI_CAPTURABLE=1`), clics, molette et clavier traversants sauf sur la
//! pilule quand la souris y est (la surveillance du bandeau, `place.rs`). Posée sur tout
//! l'écran de la zone (`poser_au_dessus`), à l'échelle de cet écran. Une seule ; créée à la
//! visée, par un tour suivant de la boucle (`plus_tard_nomme`), jamais dans un rappel ;
//! détruite à la fin du film. Elle porte le cadre, le voile et la pilule, pas le disque du
//! décompte (celui du bandeau).
//!
//! La page ne décide rien : elle pose ce qu'on lui dit, rapporte la place réelle de la
//! pilule et les clics par les commandes de `application::fenetre_film`. Le module crée,
//! place, montre, cache, prévient la page et détruit.
//!
//! Charges envoyées à la page (pixels réels relatifs au coin de la fenêtre) :
//! - `film:montrer` : `ChargeMontrer` ;
//! - `film:etat` : `ChargeEtat` (à chaque changement, et à chaque seconde de film) ;
//! - `film:cacher` : `ChargeCacher` ;
//! - `interface:theme` (`theme::envoyer_a`).
//!
//! Clics : la fenêtre part toujours traversée ; un fil `film-souris` suit la souris pendant
//! qu'elle est montrée et ne la rend cliquable que lorsque la souris est sur la pilule
//! (`pilule`), comme le bandeau et la fenêtre de la capture défilante.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use super::place::{INTERVALLE_LOIN, INTERVALLE_PROCHE};
use crate::systeme::{self, Rect};

/// Étiquette de la fenêtre (aussi dans `capabilities/film.json`).
pub const ETIQUETTE_FILM: &str = "film";
/// Page de la fenêtre, sous `app/interface/`.
pub const PAGE: &str = "film/film.html";
/// Libellé de l'exclusion au journal : `film exclue des captures: oui|non|non (essai)`.
pub const EXCLUSION: &str = "film exclue";

/// Événements écoutés par la page (§11).
pub const EVENEMENT_MONTRER: &str = "film:montrer";
pub const EVENEMENT_ETAT: &str = "film:etat";
pub const EVENEMENT_CACHER: &str = "film:cacher";

/// Taille logique de la fenêtre à sa création, avant d'être posée sur l'écran de la zone.
const TAILLE_INITIALE: (f64, f64) = (200.0, 200.0);

/// Taille de la pilule dessinée par la page, en pixels logiques, ombre non comprise, pour
/// le placement du cœur (`application::film::placer`) ; la page rapporte la place réelle par
/// `film_zone`. Jetons `film-*` : bord 1 + marge 3 + durée (9 + 58 + 8) + écart 2 + bouton
/// 28 + écart 2 + bouton 28 + marge 3 + bord 1, haute de 1 + 3 + 28 + 3 + 1. Avec « Micro »
/// (vidéo, PC avec un micro) : + écart 2 + trait (2 + 1 + 2) + écart 2 + bouton 28.
pub const TAILLE_PILULE_LOGIQUE: (f64, f64) = (143.0, 36.0);
pub const TAILLE_PILULE_MICRO_LOGIQUE: (f64, f64) = (180.0, 36.0);

/// La taille logique de la pilule, avec ou sans « Micro ».
pub fn taille_pilule_logique(micro: bool) -> (f64, f64) {
    if micro {
        TAILLE_PILULE_MICRO_LOGIQUE
    } else {
        TAILLE_PILULE_LOGIQUE
    }
}

/// Un rectangle de la page, en pixels réels relatifs au coin de la fenêtre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RectPage {
    pub x: i32,
    pub y: i32,
    pub largeur: u32,
    pub hauteur: u32,
}

/// Un point de la page, en pixels réels relatifs au coin de la fenêtre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PointPage {
    pub x: i32,
    pub y: i32,
}

/// Le bouton « Pause » / « Reprendre » de la pilule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BoutonPause {
    /// « Pause » ou « Reprendre » (le texte vient du cœur).
    pub libelle: String,
    /// Faux pendant le décompte.
    pub actif: bool,
}

/// Le bouton « Micro » de la pilule (vidéo, PC avec un micro).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BoutonMicro {
    /// « Micro » ou « Micro activé ».
    pub libelle: String,
    pub ouvert: bool,
}

/// `film:etat` : ce que la pilule montre.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChargeEtat {
    pub id: u64,
    /// `decompte`, `marche` ou `pause`.
    pub phase: &'static str,
    /// « 0:07 » (`application::film::texte_duree`).
    pub duree: String,
    pub pause: BoutonPause,
    /// « Arrêter ».
    pub arreter: String,
    /// `None` : pas de bouton (GIF, ou PC sans micro) ni le trait qui le précède.
    pub micro: Option<BoutonMicro>,
    /// Nom accessible du groupe : « Enregistrement ».
    pub groupe: String,
}

/// `film:montrer` : le film `id` commence ; tout ce que la page doit poser.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChargeMontrer {
    pub id: u64,
    /// L'échelle de l'écran de la zone.
    pub echelle: f64,
    pub zone: RectPage,
    /// Le coin haut gauche de la pilule (`application::film::placer`).
    pub pilule: PointPage,
    pub etat: ChargeEtat,
    pub reduites: bool,
}

/// `film:cacher` : le film `id` est fini ; la page efface tout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ChargeCacher {
    pub id: u64,
}

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
/// est l'affichage en cours ; sinon les clics traversent.
fn recoit_apres_geste(
    voulu: bool,
    montree: bool,
    pilule: bool,
    courant: u64,
    affichage_geste: u64,
) -> bool {
    voulu && montree && pilule && courant == affichage_geste
}

/// Rythme de la surveillance : 30 ms près de la pilule, 60 ms loin d'elle (§10).
fn intervalle(pilule: Option<Rect>, souris: Option<(f64, f64)>) -> Duration {
    let (Some(r), Some(p)) = (pilule, souris) else {
        return INTERVALLE_LOIN;
    };
    super::intervalle(Some(p), Some(&rect_place(r)))
}

/// `Drop` d'un `Controle` lâché sans `detruire` : la fenêtre n'est détruite que si elle vit
/// encore.
fn drop_detruit(detruite: bool, vivante: bool) -> bool {
    !detruite && vivante
}

/// La ligne de `Drop` d'une fenêtre lâchée sans `detruire`, avec la dernière erreur
/// connue. Fonction pure.
pub fn ligne_lachee(cause: Option<&str>) -> String {
    match cause {
        Some(c) => format!("film: fenetre lachee sans detruire (cause: {c})"),
        None => "film: fenetre lachee sans detruire (cause inconnue)".to_string(),
    }
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

/// La fenêtre de contrôle créée (cachée). À garder par l'Application le temps d'un film,
/// puis `detruire`. Ses gestes se font sur le fil principal.
#[derive(Debug)]
pub struct Controle {
    app: AppHandle,
    /// Identifiant système (Windows : le HWND), gardé pour la surveillance de la souris.
    identifiant: Option<isize>,
    /// Faux une fois la fenêtre détruite.
    vivante: Arc<AtomicBool>,
    partage: Arc<Partage>,
    /// L'écran où la fenêtre est posée (pixels réels du bureau).
    ecran: Mutex<Option<Rect>>,
    /// Vrai après `detruire` : `Drop` n'a plus rien à faire.
    detruite: AtomicBool,
    /// La dernière erreur de `poser` ou `montrer` : `Drop` l'écrit.
    erreur: Mutex<Option<String>>,
}

/// Crée la fenêtre de contrôle, cachée (fil principal, jamais dans un rappel de page :
/// toujours postée à un tour suivant de la boucle). Journal : `film fenetre <hwnd>`,
/// `film exclue des captures: …`. Une fenêtre qui existe encore : une erreur.
pub fn creer(app: &AppHandle) -> Result<Controle, String> {
    if app.get_webview_window(ETIQUETTE_FILM).is_some() {
        return Err("fenetre du film deja creee".into());
    }
    let outil = super::creer_fenetre_outil(app, ETIQUETTE_FILM, PAGE, TAILLE_INITIALE, EXCLUSION)?;
    Ok(Controle {
        app: app.clone(),
        identifiant: outil.identifiant,
        vivante: outil.vivante,
        partage: Arc::new(Partage::default()),
        ecran: Mutex::new(None),
        detruite: AtomicBool::new(false),
        erreur: Mutex::new(None),
    })
}

impl Controle {
    fn fenetre(&self) -> Option<WebviewWindow> {
        self.app.get_webview_window(ETIQUETTE_FILM)
    }

    fn ecran(&self) -> Option<Rect> {
        *self.ecran.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Identifiant système de la fenêtre (Windows : le HWND).
    pub fn identifiant(&self) -> Option<isize> {
        self.identifiant
    }

    /// Garde la dernière erreur pour `Drop` (`ligne_lachee`).
    fn noter<T>(&self, r: Result<T, String>) -> Result<T, String> {
        if let Err(e) = &r {
            *self.erreur.lock().unwrap_or_else(|p| p.into_inner()) = Some(e.clone());
        }
        r
    }

    /// Pose la fenêtre sur tout `ecran` (pixels réels du bureau), sans la montrer : la page
    /// prend la taille de l'écran avant `film:montrer`. `montrer` la repose au-dessus de
    /// tout au même endroit.
    pub fn poser(&self, ecran: Rect) -> Result<(), String> {
        let r = self.poser_sans_noter(ecran);
        self.noter(r)
    }

    fn poser_sans_noter(&self, ecran: Rect) -> Result<(), String> {
        if ecran.est_vide() {
            return Err("ecran vide".into());
        }
        let fenetre = self.fenetre().ok_or("fenetre du film absente")?;
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
        let r = self.montrer_sans_noter();
        self.noter(r)
    }

    fn montrer_sans_noter(&self) -> Result<(), String> {
        if !self.vivante.load(Ordering::SeqCst) {
            return Err("fenetre du film detruite".into());
        }
        let fenetre = self.fenetre().ok_or("fenetre du film absente")?;
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

    /// La pilule : son rectangle en pixels réels du bureau (`None` : aucune) ; la fenêtre
    /// n'est cliquable que là, quand la souris y est.
    pub fn pilule(&self, rect: Option<Rect>) {
        self.partage.souris().pilule = pilule_retenue(rect);
    }

    /// Envoie `charge` à la page sous le nom `evenement`.
    pub fn emettre<S: Serialize + Clone>(&self, evenement: &str, charge: S) -> Result<(), String> {
        self.app
            .emit_to(ETIQUETTE_FILM, evenement, charge)
            .map_err(|e| e.to_string())
    }

    /// Cache puis détruit la fenêtre. Journal en WARN si elle ne l'est pas :
    /// `film: fenetre non detruite: <e>`.
    pub fn detruire(self) -> Result<(), String> {
        self.detruite.store(true, Ordering::SeqCst);
        self.cacher();
        let Some(f) = self.fenetre() else {
            return Ok(());
        };
        match f.destroy() {
            Ok(()) => Ok(()),
            Err(e) => {
                log::warn!("film: fenetre non detruite: {e}");
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
            .name("film-souris".into())
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
            log::warn!("film: surveillance impossible: {e}");
        }
    }
}

/// Filet : un `Controle` lâché sans `detruire` (chemin d'erreur) arrête son fil et fait
/// détruire sa fenêtre sur le fil principal, si elle vit encore (`vivante` : une fenêtre
/// détruite entre-temps n'est pas confondue avec une nouvelle de même étiquette).
impl Drop for Controle {
    fn drop(&mut self) {
        {
            let mut s = self.partage.souris();
            s.montree = false;
            s.recoit = false;
            s.pilule = None;
        }
        self.partage.affichage.fetch_add(1, Ordering::SeqCst);
        if !drop_detruit(
            self.detruite.load(Ordering::SeqCst),
            self.vivante.load(Ordering::SeqCst),
        ) {
            return;
        }
        let cause = self
            .erreur
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        log::warn!("{}", ligne_lachee(cause.as_deref()));
        let app = self.app.clone();
        let vivante = Arc::clone(&self.vivante);
        let poste = self.app.run_on_main_thread(move || {
            if !vivante.load(Ordering::SeqCst) {
                return;
            }
            if let Some(f) = app.get_webview_window(ETIQUETTE_FILM) {
                super::cacher(&f);
                if let Err(e) = f.destroy() {
                    log::warn!("film: fenetre non detruite: {e}");
                }
            }
        });
        if let Err(e) = poste {
            log::warn!("film: fenetre non detruite: {e}");
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
    if let Some(f) = app.get_webview_window(ETIQUETTE_FILM) {
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
    fn fenetre_lachee_dit_la_cause() {
        assert_eq!(
            ligne_lachee(Some("show: WebView2 indisponible")),
            "film: fenetre lachee sans detruire (cause: show: WebView2 indisponible)"
        );
        assert_eq!(
            ligne_lachee(None),
            "film: fenetre lachee sans detruire (cause inconnue)"
        );
    }

    #[test]
    fn cliquable_seulement_sur_la_pilule_montree() {
        let p = r(100, 200, 180, 36);
        assert!(cliquable(true, Some(p), Some((100.0, 200.0))));
        assert!(cliquable(true, Some(p), Some((279.9, 235.9))));
        // Bord droit et bas exclus.
        assert!(!cliquable(true, Some(p), Some((280.0, 220.0))));
        assert!(!cliquable(true, Some(p), Some((120.0, 236.0))));
        assert!(!cliquable(true, Some(p), Some((99.9, 220.0))));
        // Cachée, sans pilule, souris inconnue, pilule vide : traversée.
        assert!(!cliquable(false, Some(p), Some((120.0, 220.0))));
        assert!(!cliquable(true, None, Some((120.0, 220.0))));
        assert!(!cliquable(true, Some(p), None));
        assert!(!cliquable(
            true,
            Some(r(100, 200, 0, 36)),
            Some((100.0, 220.0))
        ));
        // Un écran à gauche du principal.
        let g = r(-1700, -300, 143, 36);
        assert!(cliquable(true, Some(g), Some((-1650.0, -280.0))));
        assert!(!cliquable(true, Some(g), Some((1650.0, 280.0))));
    }

    #[test]
    fn rythme_selon_la_distance() {
        let p = r(1000, 800, 180, 36);
        assert_eq!(
            intervalle(Some(p), Some((1050.0, 820.0))),
            INTERVALLE_PROCHE
        );
        assert_eq!(intervalle(Some(p), Some((10.0, 10.0))), INTERVALLE_LOIN);
        assert_eq!(intervalle(None, Some((1050.0, 820.0))), INTERVALLE_LOIN);
        assert_eq!(intervalle(Some(p), None), INTERVALLE_LOIN);
        assert_eq!(INTERVALLE_PROCHE, Duration::from_millis(30));
        assert_eq!(INTERVALLE_LOIN, Duration::from_millis(60));
    }

    #[test]
    fn le_fil_s_arrete_detruite_cachee_ou_remplacee() {
        assert!(fil_continue(true, 3, 3, true));
        assert!(!fil_continue(false, 3, 3, true));
        assert!(!fil_continue(true, 3, 3, false));
        assert!(!fil_continue(true, 4, 3, true));
        assert!(fil_continue(true, 4, 4, true));
    }

    #[test]
    fn le_geste_cliquable_ne_vaut_que_pour_son_affichage() {
        assert!(recoit_apres_geste(true, true, true, 5, 5));
        assert!(!recoit_apres_geste(false, true, true, 5, 5));
        assert!(!recoit_apres_geste(true, true, true, 6, 5));
        assert!(!recoit_apres_geste(true, false, true, 5, 5));
        assert!(!recoit_apres_geste(true, true, false, 5, 5));
    }

    #[test]
    fn detruite_par_drop_seulement_si_elle_vit_encore() {
        assert!(drop_detruit(false, true));
        assert!(!drop_detruit(true, true));
        assert!(!drop_detruit(false, false));
    }

    #[test]
    fn la_pilule_vide_est_oubliee() {
        assert_eq!(pilule_retenue(Some(r(5, 5, 0, 4))), None);
        assert_eq!(pilule_retenue(Some(r(5, 5, 4, -1))), None);
        assert_eq!(pilule_retenue(None), None);
        assert_eq!(pilule_retenue(Some(r(5, 5, 4, 4))), Some(r(5, 5, 4, 4)));
    }

    #[test]
    fn taille_de_la_pilule_avec_et_sans_micro() {
        assert_eq!(taille_pilule_logique(false), (143.0, 36.0));
        assert_eq!(taille_pilule_logique(true), (180.0, 36.0));
    }

    fn etat(phase: &'static str, micro: Option<BoutonMicro>) -> ChargeEtat {
        ChargeEtat {
            id: 4,
            phase,
            duree: "0:07".into(),
            pause: BoutonPause {
                libelle: "Pause".into(),
                actif: phase != "decompte",
            },
            arreter: "Arrêter".into(),
            micro,
            groupe: "Enregistrement".into(),
        }
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
            pilule: PointPage { x: 475, y: 543 },
            etat: etat(
                "decompte",
                Some(BoutonMicro {
                    libelle: "Micro".into(),
                    ouvert: false,
                }),
            ),
            reduites: false,
        };
        assert_eq!(
            serde_json::to_value(&montrer).unwrap(),
            serde_json::json!({
                "id": 4, "echelle": 1.25,
                "zone": { "x": 100, "y": 80, "largeur": 600, "hauteur": 450 },
                "pilule": { "x": 475, "y": 543 },
                "etat": {
                    "id": 4, "phase": "decompte", "duree": "0:07",
                    "pause": { "libelle": "Pause", "actif": false },
                    "arreter": "Arrêter",
                    "micro": { "libelle": "Micro", "ouvert": false },
                    "groupe": "Enregistrement"
                },
                "reduites": false
            })
        );
        // GIF, ou PC sans micro : `micro` est null (ni bouton ni trait).
        assert_eq!(
            serde_json::to_value(etat("marche", None)).unwrap()["micro"],
            serde_json::Value::Null
        );
        assert_eq!(
            serde_json::to_value(ChargeCacher { id: 4 }).unwrap(),
            serde_json::json!({ "id": 4 })
        );
    }

    #[test]
    fn noms_du_contrat() {
        assert_eq!(ETIQUETTE_FILM, "film");
        assert_eq!(PAGE, "film/film.html");
        assert_eq!(EXCLUSION, "film exclue");
        assert_eq!(EVENEMENT_MONTRER, "film:montrer");
        assert_eq!(EVENEMENT_ETAT, "film:etat");
        assert_eq!(EVENEMENT_CACHER, "film:cacher");
        let capacites: serde_json::Value =
            serde_json::from_str(include_str!("../../capabilities/film.json")).unwrap();
        assert_eq!(capacites["windows"], serde_json::json!([ETIQUETTE_FILM]));
        let page = concat!(env!("CARGO_MANIFEST_DIR"), "/../interface/");
        assert!(std::path::Path::new(page).join(PAGE).is_file());
    }

    /// La taille de la pilule donnée au placement est celle que les jetons dessinent.
    #[test]
    fn taille_de_la_pilule_comme_les_jetons() {
        let css = include_str!("../../../interface/commun/jetons.css");
        let px = |nom: &str| -> f64 {
            let debut = format!("--{nom}: ");
            let ligne = css
                .lines()
                .find_map(|l| l.trim().strip_prefix(debut.as_str()))
                .unwrap_or_else(|| panic!("jeton {nom} absent"));
            ligne
                .trim_end_matches(';')
                .trim_end_matches("px")
                .parse()
                .unwrap()
        };
        let bord = 1.0;
        let marge = px("film-pilule-marge");
        let ecart = px("film-pilule-ecart");
        let bouton = px("film-bouton");
        let duree = px("film-duree-marge-gauche")
            + px("film-duree-largeur")
            + px("film-duree-marge-droite");
        let separateur = px("film-trait-marge") * 2.0 + 1.0;
        let sans = 2.0 * (bord + marge) + duree + 2.0 * (ecart + bouton);
        let avec = sans + 2.0 * ecart + separateur + bouton;
        let haut = 2.0 * (bord + marge) + bouton;
        assert_eq!(TAILLE_PILULE_LOGIQUE, (sans, haut));
        assert_eq!(TAILLE_PILULE_MICRO_LOGIQUE, (avec, haut));
    }
}
