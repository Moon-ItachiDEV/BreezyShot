//! Barre de capture « Galet » (design `barre-capture`, piste A, choisie le 16/11/2026) : une petite fenêtre web sans
//! cadre, toujours au premier plan, centrée en haut de la zone utile de l'écran sous la
//! souris, à `HAUT` px du haut. Relancer l'app déjà ouverte l'ouvre (0.7.2). Comme le menu
//! de l'icône (`menu.rs`, le modèle) : créée cachée à la première ouverture, puis gardée
//! cachée et réutilisée.
//!
//! Le module crée, place, montre et cache ; il ne décide de rien. L'Application lui donne
//! les boutons et reçoit le choix (`barre_choisi`). La largeur de la barre dépend des
//! boutons et de la police : la page la mesure et la rend (`barre_taille`), la fenêtre est
//! alors reposée sur le même centre ; d'ici là (première ouverture), une largeur estimée.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use super::place::ecran_sous;
use super::{ecrans, theme, Ecran, Rect, ARGUMENTS_NAVIGATEUR};
use crate::systeme;

/// Étiquette de la fenêtre (aussi dans `capabilities/barre.json`).
pub const ETIQUETTE_BARRE: &str = "barre";
/// Page de la fenêtre, sous `app/interface/`.
pub const PAGE: &str = "barre/barre.html";
/// Événements écoutés par la page : les boutons à montrer, à chaque ouverture ; vider la
/// barre (cachée : la prochaine ouverture ne montre jamais les anciens boutons).
pub const EVENEMENT_OUVRIR: &str = "barre:ouvrir";
pub const EVENEMENT_VIDER: &str = "barre:vider";
/// Pas de la surveillance des clics hors de la barre.
pub const PAS_SURVEILLANCE: Duration = Duration::from_millis(30);

/// Mesures en pixels logiques, les mêmes que `barre/barre.css`. Écart entre le haut de la
/// zone utile et la carte (la fenêtre commence au haut de la zone utile). L'ombre
/// `--bc-ombre` (12 px de décalage, 28 px de flou) déborde de 16 px en haut : 2 px de sa
/// queue, invisibles, passent au-dessus de la fenêtre.
pub const HAUT: f64 = 14.0;
/// Place de l'ombre à gauche et à droite de la carte (28 px de flou ; le nom de Réglages,
/// dernier bouton, déborde en plus de 7 px avec sa propre ombre de 14 px, soit 21 px).
pub const MARGE: f64 = 28.0;
/// Place sous la carte, réservée en permanence : l'ombre (12 + 28 = 40 px) et l'étiquette du
/// nom d'un bouton (8 px d'écart, 26 px de haut, ombre de 20 px, soit 54 px).
pub const MARGE_BAS: f64 = 56.0;
/// Hauteur de la carte, étage du retardateur fermé : boutons de 44 px, retrait de 6 px et
/// bord de 1 px en haut et en bas.
pub const HAUTEUR_BARRE: f64 = 58.0;
/// Écart minimal entre la carte et les bords gauche et droit de la zone utile.
pub const ECART: f64 = 8.0;
/// Largeur estimée (première ouverture, avant la mesure de la page) : le cadre (retrait de
/// 6 px et bord de 1 px de chaque côté), puis chaque bouton de 44 px (2 px d'écart), chaque
/// filet (1 px et 7 px de chaque côté) et la puce « Pro ». Un peu large plutôt qu'étroite :
/// la page, centrée, ne déborde pas.
pub const LARGEUR_CADRE: f64 = 14.0;
pub const LARGEUR_BOUTON: f64 = 46.0;
pub const LARGEUR_FILET: f64 = 17.0;
pub const LARGEUR_PUCE: f64 = 44.0;
/// Largeur de la carte à la création de la fenêtre, avant que `ouvrir` la pose à sa mesure.
pub const LARGEUR_INITIALE: f64 = 520.0;
/// Bornes d'une taille rendue par la page (une valeur absurde est refusée).
pub const TAILLE_MAX: f64 = 4000.0;

/// Ce que reçoit la page à chaque ouverture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ouverture {
    /// Identifiants des entrées du menu de l'icône que la barre montre, dans son ordre.
    pub boutons: Vec<String>,
    /// Le retardateur des réglages, en secondes ; `None` : pas de bouton Retardateur.
    pub retardateur_s: Option<u32>,
    /// Les choix de l'étage du retardateur (0 = aucun).
    pub retardateurs: Vec<u32>,
    /// Ceux de `boutons` que la licence n'ouvre pas : montrés éteints, un clic ouvre
    /// Réglages › Licence.
    pub verrouilles: Vec<String>,
}

impl Ouverture {
    /// Clé d'une forme de barre : mêmes boutons, même largeur (la puce « Pro » compte).
    fn cle(&self) -> String {
        format!(
            "{}|{}|{}",
            self.boutons.join(","),
            self.retardateur_s.is_some(),
            !self.verrouilles.is_empty()
        )
    }
}

/// L'ouverture en cours : son écran, la place de la fenêtre (surveillance des clics,
/// redimensionnement), ses boutons (seul l'un d'eux peut être choisi) et sa forme.
struct EnCours {
    ecran: Ecran,
    place: Rect,
    boutons: Vec<String>,
    /// Les boutons montrés éteints (Pro sans licence).
    verrouilles: Vec<String>,
    /// Le bouton Retardateur est montré.
    retardateur: bool,
    forme: String,
}
static EN_COURS: Mutex<Option<EnCours>> = Mutex::new(None);
/// Charge en attente de la page (première ouverture), une seule fois.
static EN_ATTENTE: Mutex<Option<Ouverture>> = Mutex::new(None);
/// Dernière largeur mesurée par la page, étage du retardateur fermé, pour une forme de barre.
static LARGEUR_MESUREE: Mutex<Option<(String, f64)>> = Mutex::new(None);
/// Heure de la dernière ouverture (`dans_le_double_clic`).
static OUVERTE_A: Mutex<Option<Instant>> = Mutex::new(None);
/// Vrai tant que la barre est montrée (évite un appel inter-fils `is_visible()` toutes les 30 ms).
static VISIBLE: AtomicBool = AtomicBool::new(false);
/// Numéro de l'ouverture : une surveillance des clics d'une ouverture passée s'arrête.
static NUMERO: AtomicU64 = AtomicU64::new(0);

fn fenetre(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(ETIQUETTE_BARRE)
}

/// Identifiant système de la fenêtre si elle existe (HWND sous Windows).
pub fn identifiant(app: &AppHandle) -> Option<isize> {
    fenetre(app).and_then(|f| systeme::identifiant_fenetre(&f))
}

/// Échelle utilisable : une valeur absurde vaut 1.
fn echelle_sure(echelle: f64) -> f64 {
    if echelle.is_finite() && echelle > 0.0 {
        echelle
    } else {
        1.0
    }
}

/// Largeur estimée de la carte (pixels logiques) pour ces boutons, avant la mesure de la
/// page. Fonction pure.
pub fn largeur_estimee(o: &Ouverture) -> f64 {
    let a_libelle = ["capturer_zone", "capturer_plein_ecran", "capturer_fenetre"];
    let films = ["capturer_defilante", "capturer_video", "capturer_gif"];
    let n_captures = o
        .boutons
        .iter()
        .filter(|b| a_libelle.contains(&b.as_str()))
        .count();
    let n_films = o
        .boutons
        .iter()
        .filter(|b| films.contains(&b.as_str()))
        .count();
    let n_reglages = o.boutons.iter().filter(|b| *b == "reglages").count();
    let boutons = n_captures + n_films + n_reglages + usize::from(o.retardateur_s.is_some());
    // Les groupes non vides : captures (avec le retardateur), films, Réglages.
    let groupes = usize::from(n_captures > 0 || o.retardateur_s.is_some())
        + usize::from(n_films > 0)
        + usize::from(n_reglages > 0);
    let filets = groupes.saturating_sub(1);
    let puce = if o.verrouilles.is_empty() {
        0.0
    } else {
        LARGEUR_PUCE
    };
    LARGEUR_CADRE + boutons as f64 * LARGEUR_BOUTON + filets as f64 * LARGEUR_FILET + puce
}

/// Plafond du délai pendant lequel la barre ignore un clic ailleurs : `GetDoubleClickTime`
/// peut aller jusqu'à 5 s, et la barre ne doit pas rester sourde aussi longtemps.
pub const DOUBLE_CLIC_MAX: Duration = Duration::from_secs(1);

/// Vrai si un clic ailleurs ou une perte du focus, `ecoule` après l'ouverture, tombe dans
/// le délai du double-clic du système (`delai`, plafonné à `DOUBLE_CLIC_MAX`) : il est
/// ignoré. Un double-clic sur l'icône épinglée ouvre la barre au premier clic ; le second,
/// sur la barre des tâches, ne doit pas la refermer. Fonction pure.
pub fn dans_le_double_clic(ecoule: Duration, delai: Duration) -> bool {
    ecoule < delai.min(DOUBLE_CLIC_MAX)
}

/// Vrai si la dernière ouverture date de moins que le délai du double-clic.
fn vient_d_ouvrir() -> bool {
    OUVERTE_A
        .lock()
        .ok()
        .and_then(|o| *o)
        .is_some_and(|t| dans_le_double_clic(t.elapsed(), systeme::delai_double_clic()))
}

/// Une taille rendue par la page, si elle est sensée.
pub fn taille_sensee(largeur: f64, hauteur: f64) -> Option<(f64, f64)> {
    let ok = |v: f64| (1.0..=TAILLE_MAX).contains(&v);
    (ok(largeur) && ok(hauteur)).then_some((largeur, hauteur))
}

/// Place de la fenêtre (pure, pixels réels) pour une carte de `taille_logique` (barre et
/// étage ouvert) : centrée en haut de la zone utile de `ecran`, la carte à `HAUT` du
/// haut ; la fenêtre comprend la place de l'ombre (`MARGE` à gauche et à droite,
/// `MARGE_BAS` en bas).
/// Plus large que la zone utile, la carte reste à `ECART` de son bord gauche.
pub fn placer(ecran: &Ecran, taille_logique: (f64, f64)) -> Rect {
    let e = echelle_sure(ecran.echelle);
    let px = |v: f64| (v * e).round() as i64;
    let (haut, marge, marge_bas, ecart) = (px(HAUT), px(MARGE), px(MARGE_BAS), px(ECART));
    let (lc, hc) = (px(taille_logique.0).max(1), px(taille_logique.1).max(1));
    let z = ecran.zone_utile;
    let (zg, zh) = (i64::from(z.x), i64::from(z.y));
    let zd = zg + i64::from(z.largeur);
    // La carte centrée dans la zone utile, puis bornée (le bord gauche gagne).
    let gauche = zg + (i64::from(z.largeur) - lc) / 2;
    let gauche = gauche.min(zd - ecart - lc).max(zg + ecart);
    Rect {
        x: (gauche - marge) as i32,
        y: zh as i32,
        largeur: (lc + 2 * marge).max(1) as u32,
        hauteur: (haut + hc + marge_bas).max(1) as u32,
    }
}

/// Vrai si la barre est montrée.
pub fn ouverte(app: &AppHandle) -> bool {
    fenetre(app).is_some_and(|f| f.is_visible().unwrap_or(false))
}

fn creer(app: &AppHandle) -> Result<WebviewWindow, String> {
    let f = WebviewWindowBuilder::new(app, ETIQUETTE_BARRE, WebviewUrl::App(PAGE.into()))
        .title("BreezyShot")
        .inner_size(
            LARGEUR_INITIALE + 2.0 * MARGE,
            HAUT + HAUTEUR_BARRE + MARGE_BAS,
        )
        .visible(false)
        .focused(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .theme(theme::pour_cadre(theme::actuel()))
        .additional_browser_args(ARGUMENTS_NAVIGATEUR)
        .build()
        .map_err(|e| format!("creation: {e}"))?;
    if let Err(e) = systeme::preparer_fenetre_outil_active(&f) {
        log::warn!("barre: style outil non pose: {e}");
    }
    let pour_focus = app.clone();
    f.on_window_event(move |e| match e {
        // Un clic ailleurs (ou Alt+Tab) : la barre se ferme, sauf dans le délai du
        // double-clic après l'ouverture.
        tauri::WindowEvent::Focused(false) => {
            if vient_d_ouvrir() {
                log::info!("barre: perte du focus ignoree (double-clic)");
            } else {
                let _ = cacher(&pour_focus);
            }
        }
        tauri::WindowEvent::Destroyed => {
            VISIBLE.store(false, Ordering::SeqCst);
        }
        _ => {}
    });
    match systeme::identifiant_fenetre(&f) {
        Some(h) => log::info!("barre fenetre {h}"),
        None => log::info!("barre fenetre creee"),
    }
    Ok(f)
}

/// Ouvre la barre sur l'écran de la souris (déjà ouverte : reposée et remise à jour). La
/// première fois, la fenêtre est créée et la page envoie `barre_pret` : la charge lui est
/// donnée à ce moment-là. Jamais exclue des captures : elle ne s'ouvre ni pendant une
/// défilante ni pendant un film, et chaque capture la ferme avant sa photo. Rend vrai si
/// la barre est montrée.
pub fn ouvrir(app: &AppHandle, charge: Ouverture) -> bool {
    let Some(ecran) = ecran_sous(super::souris(app), &ecrans(app)) else {
        log::warn!("barre non ouverte: aucun ecran");
        return false;
    };
    let cle = charge.cle();
    let largeur = LARGEUR_MESUREE
        .lock()
        .ok()
        .and_then(|m| m.as_ref().filter(|(c, _)| *c == cle).map(|(_, l)| *l))
        .unwrap_or_else(|| largeur_estimee(&charge));
    let place = placer(&ecran, (largeur, HAUTEUR_BARRE));
    let existante = fenetre(app);
    if existante.is_none() {
        // La page n'écoute pas encore : elle prendra la charge à `barre_pret` (posée avant
        // la création, que WebView2 peut faire tourner dans une boucle imbriquée).
        if let Ok(mut a) = EN_ATTENTE.lock() {
            *a = Some(charge.clone());
        }
    }
    let (f, nouvelle) = match existante {
        Some(f) => (f, false),
        None => match creer(app) {
            Ok(f) => (f, true),
            Err(e) => {
                let _ = prendre_en_attente();
                log::warn!("barre non ouverte: {e}");
                return false;
            }
        },
    };
    if let Ok(mut c) = EN_COURS.lock() {
        *c = Some(EnCours {
            ecran,
            place,
            boutons: charge.boutons.clone(),
            verrouilles: charge.verrouilles.clone(),
            retardateur: charge.retardateur_s.is_some(),
            forme: cle,
        });
    }
    let _ = f.set_position(tauri::PhysicalPosition::new(place.x, place.y));
    let _ = f.set_size(tauri::PhysicalSize::new(place.largeur, place.hauteur));
    if !nouvelle {
        theme::envoyer_a(app, ETIQUETTE_BARRE);
        if let Err(e) = app.emit_to(ETIQUETTE_BARRE, EVENEMENT_OUVRIR, charge) {
            log::warn!("barre: boutons non envoyes: {e}");
        }
    }
    if let Ok(mut o) = OUVERTE_A.lock() {
        *o = Some(Instant::now());
    }
    VISIBLE.store(true, Ordering::SeqCst);
    let _ = f.show();
    if !matches!(systeme::prendre_premier_plan(&f), Ok(true)) {
        let _ = f.set_focus();
    }
    log::info!("barre ouverte");
    let numero = NUMERO.fetch_add(1, Ordering::SeqCst) + 1;
    surveiller_clics(app, numero);
    true
}

/// La page a mesuré sa carte (pixels logiques, étage ouvert compris) : la fenêtre
/// est reposée à cette taille, centrée de même. La largeur, étage fermé, est retenue pour
/// les prochaines ouvertures de la même forme.
pub fn redimensionner(app: &AppHandle, largeur: f64, hauteur: f64) {
    let Some((l, h)) = taille_sensee(largeur, hauteur) else {
        log::warn!("barre: taille refusee");
        return;
    };
    let Some(f) = fenetre(app) else {
        return;
    };
    let (place, forme) = {
        let Ok(mut c) = EN_COURS.lock() else {
            return;
        };
        let Some(c) = c.as_mut() else {
            return;
        };
        c.place = placer(&c.ecran, (l, h));
        (c.place, c.forme.clone())
    };
    if (h - HAUTEUR_BARRE).abs() < 0.5 {
        if let Ok(mut m) = LARGEUR_MESUREE.lock() {
            *m = Some((forme, l));
        }
    }
    let _ = f.set_position(tauri::PhysicalPosition::new(place.x, place.y));
    let _ = f.set_size(tauri::PhysicalSize::new(place.largeur, place.hauteur));
}

/// Ferme la barre au premier clic hors d'elle, même si elle n'a jamais reçu le focus
/// (Windows peut refuser le premier plan : `Focused(false)` n'arriverait jamais). Un fil
/// regarde les boutons de la souris tant que la barre de cette ouverture est montrée.
fn surveiller_clics(app: &AppHandle, numero: u64) {
    let app = app.clone();
    let lance = std::thread::Builder::new()
        .name("barre-clics".into())
        .spawn(move || {
            // Un bouton encore enfoncé à l'ouverture ne compte pas.
            let mut relache = !systeme::bouton_souris_enfonce();
            loop {
                std::thread::sleep(PAS_SURVEILLANCE);
                if NUMERO.load(Ordering::SeqCst) != numero || !VISIBLE.load(Ordering::SeqCst) {
                    return;
                }
                let enfonce = systeme::bouton_souris_enfonce();
                if !enfonce {
                    relache = true;
                    continue;
                }
                if !relache {
                    continue;
                }
                // Le second clic d'un double-clic sur l'icône épinglée : ignoré.
                if vient_d_ouvrir() {
                    relache = false;
                    continue;
                }
                let place = EN_COURS
                    .lock()
                    .ok()
                    .and_then(|c| c.as_ref().map(|c| c.place));
                let dedans = match (place, systeme::position_souris()) {
                    (Some(r), Some((x, y))) => r.contient(x, y),
                    _ => true,
                };
                if !dedans {
                    let pour = app.clone();
                    let _ = app.run_on_main_thread(move || {
                        cacher(&pour);
                    });
                    return;
                }
                relache = false;
            }
        });
    if let Err(e) = lance {
        log::warn!("barre: surveillance des clics impossible: {e}");
    }
}

/// La charge en attente de la page (première ouverture), une seule fois.
pub fn prendre_en_attente() -> Option<Ouverture> {
    EN_ATTENTE.lock().ok().and_then(|mut a| a.take())
}

/// Vrai si `id` est un bouton de l'ouverture en cours.
pub fn bouton_montre(id: &str) -> bool {
    EN_COURS.lock().is_ok_and(|c| {
        c.as_ref()
            .is_some_and(|c| c.boutons.iter().any(|x| x == id))
    })
}

/// Vrai si `id` est un bouton éteint de l'ouverture en cours (Pro sans licence).
pub fn bouton_verrouille(id: &str) -> bool {
    EN_COURS.lock().is_ok_and(|c| {
        c.as_ref()
            .is_some_and(|c| c.verrouilles.iter().any(|x| x == id))
    })
}

/// Vrai si l'ouverture en cours montre le bouton Retardateur.
pub fn retardateur_montre() -> bool {
    EN_COURS
        .lock()
        .is_ok_and(|c| c.as_ref().is_some_and(|c| c.retardateur))
}

/// Cache la barre (choix fait, Échap, clic ailleurs, capture qui démarre) et vide la page.
/// Rend vrai si elle était montrée.
pub fn cacher(app: &AppHandle) -> bool {
    let Some(f) = fenetre(app) else {
        VISIBLE.store(false, Ordering::SeqCst);
        return false;
    };
    if !f.is_visible().unwrap_or(false) {
        VISIBLE.store(false, Ordering::SeqCst);
        return false;
    }
    VISIBLE.store(false, Ordering::SeqCst);
    let _ = f.hide();
    if let Ok(mut c) = EN_COURS.lock() {
        if let Some(c) = c.as_mut() {
            c.boutons.clear();
            c.verrouilles.clear();
            c.retardateur = false;
        }
    }
    if let Err(e) = app.emit_to(ETIQUETTE_BARRE, EVENEMENT_VIDER, ()) {
        log::warn!("barre: page non videe: {e}");
    }
    log::info!("barre fermee");
    true
}

/// Ferme la fenêtre si elle existe (arrêt de l'application).
pub fn fermer(app: &AppHandle) {
    VISIBLE.store(false, Ordering::SeqCst);
    NUMERO.fetch_add(1, Ordering::SeqCst);
    if let Some(f) = fenetre(app) {
        let _ = f.destroy();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Écran `l` × `h` à `echelle`, en (x, y), barre des tâches de 48 px (logiques) en bas.
    fn ecran(x: i32, y: i32, l: u32, h: u32, echelle: f64) -> Ecran {
        let barre = (48.0 * echelle) as u32;
        Ecran {
            rect: Rect {
                x,
                y,
                largeur: l,
                hauteur: h,
            },
            zone_utile: Rect {
                x,
                y,
                largeur: l,
                hauteur: h - barre,
            },
            echelle,
        }
    }

    fn ouverture(boutons: &[&str], retardateur: bool) -> Ouverture {
        Ouverture {
            boutons: boutons.iter().map(|b| b.to_string()).collect(),
            retardateur_s: retardateur.then_some(0),
            retardateurs: vec![0, 3, 5, 10],
            verrouilles: vec![],
        }
    }

    /// Le centre horizontal de la carte (sans l'ombre) et le haut de la carte.
    fn carte(p: &Rect, echelle: f64) -> (i32, i32) {
        let haut = (HAUT * echelle).round() as i32;
        (p.x + p.largeur as i32 / 2, p.y + haut)
    }

    #[test]
    fn centree_en_haut_de_la_zone_utile() {
        let e = ecran(0, 0, 1920, 1080, 1.0);
        let p = placer(&e, (500.0, HAUTEUR_BARRE));
        assert_eq!(carte(&p, 1.0), (960, 14));
        assert_eq!(p.y, 0);
        assert_eq!(p.largeur, (500.0 + 2.0 * MARGE) as u32);
        assert_eq!(p.hauteur, (HAUT + HAUTEUR_BARRE + MARGE_BAS) as u32);
    }

    #[test]
    fn a_l_echelle_de_l_ecran() {
        let e = ecran(0, 0, 2880, 1620, 1.5);
        let p = placer(&e, (500.0, HAUTEUR_BARRE));
        assert_eq!(carte(&p, 1.5), (1440, 21));
        assert_eq!(p.largeur, ((500.0 + 2.0 * MARGE) * 1.5).round() as u32);
        assert_eq!(
            p.hauteur,
            (HAUT * 1.5).round() as u32
                + (HAUTEUR_BARRE * 1.5).round() as u32
                + (MARGE_BAS * 1.5).round() as u32
        );
    }

    #[test]
    fn menu_volant_ouvert_la_fenetre_grandit_vers_le_bas() {
        let e = ecran(0, 0, 1920, 1080, 1.25);
        let fermee = placer(&e, (500.0, HAUTEUR_BARRE));
        let ouverte = placer(&e, (500.0, HAUTEUR_BARRE + 80.0));
        assert_eq!(
            (fermee.x, fermee.y, fermee.largeur),
            (ouverte.x, ouverte.y, ouverte.largeur)
        );
        assert_eq!(ouverte.hauteur - fermee.hauteur, 100);
    }

    #[test]
    fn ecran_a_gauche_coordonnees_negatives() {
        let e = ecran(-1920, -200, 1920, 1080, 1.0);
        let p = placer(&e, (500.0, HAUTEUR_BARRE));
        assert_eq!(carte(&p, 1.0), (-960, -186));
        assert_eq!(p.y, -200);
    }

    #[test]
    fn barre_des_taches_en_haut() {
        let mut e = ecran(0, 0, 1920, 1080, 1.0);
        e.zone_utile.y = 48;
        e.zone_utile.hauteur = 1032;
        let p = placer(&e, (500.0, HAUTEUR_BARRE));
        assert_eq!(carte(&p, 1.0), (960, 62));
    }

    #[test]
    fn barre_des_taches_a_gauche_centree_dans_la_zone_utile() {
        let mut e = ecran(0, 0, 1920, 1080, 1.0);
        e.zone_utile.x = 64;
        e.zone_utile.largeur = 1856;
        e.zone_utile.hauteur = 1080;
        let p = placer(&e, (500.0, HAUTEUR_BARRE));
        assert_eq!(carte(&p, 1.0).0, 64 + 928);
    }

    #[test]
    fn plus_large_que_la_zone_le_bord_gauche_gagne() {
        let e = ecran(100, 0, 400, 600, 1.0);
        let p = placer(&e, (500.0, HAUTEUR_BARRE));
        assert_eq!(p.x + MARGE as i32, 100 + ECART as i32);
    }

    #[test]
    fn largeur_estimee_selon_les_boutons() {
        let base = [
            "capturer_zone",
            "capturer_plein_ecran",
            "capturer_fenetre",
            "reglages",
        ];
        // Trois captures, Réglages, le retardateur : cinq boutons, un filet.
        let sans = largeur_estimee(&ouverture(&base, true));
        assert_eq!(sans, LARGEUR_CADRE + 5.0 * LARGEUR_BOUTON + LARGEUR_FILET);
        let mut pro = base.to_vec();
        pro.extend(["capturer_defilante", "capturer_video", "capturer_gif"]);
        // Défilante, Vidéo et GIF : trois boutons directs et un filet de plus.
        let avec = largeur_estimee(&ouverture(&pro, true));
        assert_eq!(avec - sans, 3.0 * LARGEUR_BOUTON + LARGEUR_FILET);
        // Sans licence : les mêmes boutons éteints, et la puce « Pro ».
        let mut eteints = ouverture(&pro, true);
        eteints.verrouilles = vec!["capturer_defilante".into(), "capturer_video".into()];
        assert_eq!(largeur_estimee(&eteints) - avec, LARGEUR_PUCE);
        // Sans retardateur : un bouton de moins.
        assert_eq!(
            sans - largeur_estimee(&ouverture(&base, false)),
            LARGEUR_BOUTON
        );
    }

    #[test]
    fn la_carte_estimee_tient_dans_la_largeur_initiale() {
        let pro = [
            "capturer_zone",
            "capturer_plein_ecran",
            "capturer_fenetre",
            "capturer_pipette",
            "capturer_defilante",
            "capturer_video",
            "capturer_gif",
            "reglages",
        ];
        let mut o = ouverture(&pro, true);
        o.verrouilles = vec!["capturer_defilante".into()];
        // La barre la plus large (tout, avec la puce) : environ 506 px, la fenêtre créée.
        assert!((largeur_estimee(&o) - LARGEUR_INITIALE).abs() < 20.0);
    }

    #[test]
    fn taille_de_la_page_bornee() {
        assert_eq!(taille_sensee(520.0, 58.0), Some((520.0, 58.0)));
        assert_eq!(taille_sensee(f64::NAN, 58.0), None);
        assert_eq!(taille_sensee(520.0, 0.0), None);
        assert_eq!(taille_sensee(1e9, 58.0), None);
        assert_eq!(taille_sensee(-3.0, 58.0), None);
    }

    #[test]
    fn delai_du_double_clic() {
        let d = Duration::from_millis(500);
        assert!(dans_le_double_clic(Duration::from_millis(0), d));
        assert!(dans_le_double_clic(Duration::from_millis(499), d));
        assert!(!dans_le_double_clic(Duration::from_millis(500), d));
        assert!(!dans_le_double_clic(Duration::from_secs(3), d));
        // Plafonné à 1 s : le délai réglé au plus long dans Windows (5 s) n'y change rien.
        assert_eq!(DOUBLE_CLIC_MAX, Duration::from_secs(1));
        let long = Duration::from_secs(5);
        assert!(dans_le_double_clic(Duration::from_millis(999), long));
        assert!(!dans_le_double_clic(Duration::from_millis(1000), long));
        assert!(!dans_le_double_clic(Duration::from_secs(3), long));
        let pile = Duration::from_secs(1);
        assert!(dans_le_double_clic(Duration::from_millis(999), pile));
        assert!(!dans_le_double_clic(pile, pile));
    }

    #[test]
    fn noms_du_contrat() {
        assert_eq!(ETIQUETTE_BARRE, "barre");
        assert_eq!(PAGE, "barre/barre.html");
        assert_eq!(EVENEMENT_OUVRIR, "barre:ouvrir");
        assert_eq!(EVENEMENT_VIDER, "barre:vider");
        let capacites: serde_json::Value =
            serde_json::from_str(include_str!("../../capabilities/barre.json")).unwrap();
        assert_eq!(capacites["windows"], serde_json::json!([ETIQUETTE_BARRE]));
        for commande in ["pret", "taille", "choisi", "retardateur", "fermer"] {
            let droit = serde_json::json!(format!("allow-barre-{commande}"));
            assert!(
                capacites["permissions"]
                    .as_array()
                    .is_some_and(|d| d.contains(&droit)),
                "{droit}"
            );
        }
        let page = concat!(env!("CARGO_MANIFEST_DIR"), "/../interface/");
        assert!(std::path::Path::new(page).join(PAGE).is_file());
    }

    #[test]
    fn forme_de_la_barre() {
        let a = ouverture(&["capturer_zone", "reglages"], true);
        let b = ouverture(&["capturer_zone", "reglages"], false);
        assert_ne!(a.cle(), b.cle());
        let mut c = a.clone();
        c.retardateur_s = Some(5);
        assert_eq!(a.cle(), c.cle());
        // La puce « Pro » change la largeur : une autre forme.
        let mut d = a.clone();
        d.verrouilles = vec!["capturer_gif".into()];
        assert_ne!(a.cle(), d.cle());
    }
}
