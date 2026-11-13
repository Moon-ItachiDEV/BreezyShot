//! Fenêtre de l'éditeur d'annotation (contrat 0.4 « Éditeur », §4.1 ; avenant « Éditeur
//! épuré », §3) : une fenêtre sans cadre Windows (la barre du haut de la page la déplace
//! par `data-tauri-drag-region`), avec l'ombre et les coins arrondis du système,
//! redimensionnable par ses bords, dans la barre des tâches et Alt+Tab ; une seule à la
//! fois, créée cachée à la demande, montrée à `editeur_pret`, détruite à la fermeture.
//!
//! Sans cadre, sous Windows (tao 0.37) : le style garde `WS_CAPTION` et `WS_SIZEBOX` (menu
//! système, Alt+F4, redimensionnement) ; `WM_NCCALCSIZE` rend toute la fenêtre à la zone
//! cliente, sauf, avec l'ombre, des bords invisibles (épaisseur du cadre à gauche, à
//! droite et en bas ; en haut `round(dpi / 96)` sous Windows 11, soit 1 px à 100 % et
//! 2 px à 150 %, et 0 sous Windows 10) qui portent le redimensionnement.
//!
//! Le module crée, place, montre et prévient la page ; il ne décide de rien : les
//! commandes de la page et les décisions sont dans l'Application.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use super::place::ecran_sous;
use super::theme::{self, ThemePage};
use super::{ecrans, souris, Ecran, Rect, ARGUMENTS_NAVIGATEUR};
use crate::systeme;

/// Étiquette de la fenêtre (aussi dans `capabilities/editeur.json`).
pub const ETIQUETTE_EDITEUR: &str = "editeur";
/// Titre de la fenêtre (barre des tâches, Alt+Tab ; aucune barre de titre affichée).
pub const TITRE: &str = "Annoter la capture";
/// Page de la fenêtre, sous `app/interface/`.
pub const PAGE: &str = "editeur/editeur.html";
/// Taille intérieure minimale, pixels logiques.
pub const TAILLE_MIN: (f64, f64) = (640.0, 440.0);
/// Largeur intérieure à l'ouverture, au moins, pixels logiques, quand la zone utile le
/// permet (jeton `editeur-ouverture-largeur`) : la barre d'annotation y tient sans défiler,
/// quel que soit l'outil déplié (la plus large, Forme dépliée, fait 1071 px, plus le retrait
/// de 16 px). Réduite ensuite jusqu'à `TAILLE_MIN`, la fenêtre fait défiler les outils.
pub const LARGEUR_OUVERTURE: f64 = 1088.0;
/// Ce que la page garde autour de l'image, pixels logiques (avenant §3, barre C) : 36 px de
/// chaque côté ; barre du haut 52, réserve haute 14, barre d'annotation 80 et sa réserve 34
/// en bas.
pub const MARGES: (f64, f64) = (72.0, 180.0);
/// Part maximale de la zone utile prise par la fenêtre.
pub const PART_MAX: f64 = 0.9;

/// Événements écoutés par la page (§6).
pub const EVENEMENT_CHARGER: &str = "editeur:charger";
pub const EVENEMENT_FERMER_DEMANDE: &str = "editeur:fermer_demande";
pub const EVENEMENT_RESULTAT: &str = "editeur:resultat";
pub const EVENEMENT_GLISSER: &str = "editeur:glisser";

/// Ce qu'a fait `ouvrir`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ouverture {
    /// Fenêtre créée, cachée (identifiant système, s'il est connu).
    Creee { identifiant: Option<isize> },
    /// Elle existait déjà.
    Existante,
}

/// Place de la fenêtre pour une image de `image` pixels sur `ecran` (pure, pixels réels) :
/// image à la taille réelle + `MARGES`, au moins `LARGEUR_OUVERTURE` × `TAILLE_MIN.1`, au
/// plus `PART_MAX` de la zone utile, centrée dans la zone utile. Le rectangle est celui de
/// la zone cliente (la page) ; `ouvrir` met la zone cliente exactement dessus (les bords
/// invisibles de l'ombre débordent).
pub fn placer(image: (u32, u32), ecran: &Ecran) -> Rect {
    let e = echelle_sure(ecran.echelle);
    let zone = ecran.zone_utile;
    let max = (
        (f64::from(zone.largeur) * PART_MAX).floor().max(1.0),
        (f64::from(zone.hauteur) * PART_MAX).floor().max(1.0),
    );
    // Taille réelle : un pixel de l'image par pixel de l'écran, plus la marge et la barre.
    let cote = |image: u32, marge: f64, min: f64, max: f64| {
        let voulu = f64::from(image) + marge * e;
        voulu.max(min * e).min(max).round().max(1.0) as u32
    };
    let largeur = cote(image.0, MARGES.0, LARGEUR_OUVERTURE, max.0);
    let hauteur = cote(image.1, MARGES.1, TAILLE_MIN.1, max.1);
    Rect {
        x: (i64::from(zone.x) + (i64::from(zone.largeur) - i64::from(largeur)) / 2) as i32,
        y: (i64::from(zone.y) + (i64::from(zone.hauteur) - i64::from(hauteur)) / 2) as i32,
        largeur,
        hauteur,
    }
}

/// Échelle utilisable : une valeur absurde vaut 1.
fn echelle_sure(echelle: f64) -> f64 {
    if echelle.is_finite() && echelle > 0.0 {
        echelle
    } else {
        1.0
    }
}

/// La fenêtre, si elle est ouverte.
fn fenetre(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(ETIQUETTE_EDITEUR)
}

/// Vrai tant que la fenêtre existe (y compris entre `destroy()` et `Destroyed`).
pub fn existe(app: &AppHandle) -> bool {
    fenetre(app).is_some()
}

/// L'écran sous la souris (le clic Annoter est sur la vignette, donc sur l'écran de la
/// capture) ; à défaut le premier.
pub fn ecran_souris(app: &AppHandle) -> Option<Ecran> {
    ecran_sous(souris(app), &ecrans(app))
}

/// Crée la fenêtre cachée à `place` (fil principal). `CloseRequested` est empêché et
/// appelle `fermeture_demandee` ; `Destroyed` appelle `detruite`. `destroy()` (fermeture
/// décidée, arrêt) ne passe pas par `CloseRequested` : rien ne peut bloquer l'arrêt.
pub fn ouvrir<F, G>(
    app: &AppHandle,
    place: Rect,
    detruite: F,
    fermeture_demandee: G,
) -> Result<Ouverture, String>
where
    F: Fn() + Send + Sync + 'static,
    G: Fn() + Send + Sync + 'static,
{
    if fenetre(app).is_some() {
        return Ok(Ouverture::Existante);
    }
    let (lmin, hmin) = TAILLE_MIN;
    let f = WebviewWindowBuilder::new(app, ETIQUETTE_EDITEUR, WebviewUrl::App(PAGE.into()))
        .title(TITRE)
        .inner_size(lmin, hmin)
        .min_inner_size(lmin, hmin)
        .visible(false)
        .resizable(true)
        .decorations(false)
        .shadow(true)
        .skip_taskbar(false)
        .always_on_top(false)
        .theme(theme::pour_cadre(theme::actuel()))
        .additional_browser_args(ARGUMENTS_NAVIGATEUR)
        .build()
        .map_err(|e| format!("creation: {e}"))?;
    f.on_window_event(move |e| match e {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            fermeture_demandee();
        }
        tauri::WindowEvent::Destroyed => detruite(),
        _ => {}
    });
    // D'abord sur l'écran de `place` (l'échelle de la taille est celle de cet écran),
    // puis la zone cliente à la taille de `place` (tao ajoute les bords invisibles), puis
    // la fenêtre décalée de ces bords : la zone cliente tombe exactement sur `place`.
    let _ = f.set_position(tauri::PhysicalPosition::new(place.x, place.y));
    let _ = f.set_size(tauri::PhysicalSize::new(place.largeur, place.hauteur));
    let (dx, dy) = match (f.inner_position(), f.outer_position()) {
        (Ok(i), Ok(o)) => (
            i64::from(i.x) - i64::from(o.x),
            i64::from(i.y) - i64::from(o.y),
        ),
        _ => (0, 0),
    };
    let x = i64::from(place.x) - dx;
    let y = i64::from(place.y) - dy;
    let _ = f.set_position(tauri::PhysicalPosition::new(x as i32, y as i32));
    Ok(Ouverture::Creee {
        identifiant: systeme::identifiant_fenetre(&f),
    })
}

/// Donne le premier plan et le clavier (repli `set_focus` si Windows le refuse).
fn devant(f: &WebviewWindow) {
    let _ = f.unminimize();
    let _ = f.show();
    if !matches!(systeme::prendre_premier_plan(f), Ok(true)) {
        log::info!("editeur premier plan refuse: repli");
        let _ = f.set_focus();
    }
}

/// Montre la fenêtre au premier plan (à `editeur_pret`) ; faux si elle n'existe pas.
pub fn montrer(app: &AppHandle) -> bool {
    fenetre(app).map(|f| devant(&f)).is_some()
}

/// Ramène la fenêtre devant (nouvel Annoter) ; faux si elle n'existe pas.
pub fn ramener_devant(app: &AppHandle) -> bool {
    montrer(app)
}

/// Envoie `evenement` à la page, si la fenêtre est ouverte.
pub fn emettre<S: Serialize + Clone>(app: &AppHandle, evenement: &str, charge: S) {
    if fenetre(app).is_some() {
        if let Err(e) = app.emit_to(ETIQUETTE_EDITEUR, evenement, charge) {
            log::warn!("editeur: {evenement} non envoye: {e}");
        }
    }
}

/// La zone de la page en pixels réels d'écran, et l'échelle de la fenêtre.
pub fn rect_page(app: &AppHandle) -> Option<(Rect, f64)> {
    let f = fenetre(app)?;
    let p = f.inner_position().ok()?;
    let t = f.inner_size().ok()?;
    let e = f.scale_factor().ok()?;
    Some((
        Rect {
            x: p.x,
            y: p.y,
            largeur: t.width,
            hauteur: t.height,
        },
        e,
    ))
}

/// Le thème de la fenêtre ouverte. Sans cadre, il sert encore : tao le passe à WebView2
/// (`PreferredColorScheme`, donc `prefers-color-scheme` de la page) et au mode sombre de
/// DWM (liseré, menu système d'Alt+Espace).
pub fn regler_cadre(app: &AppHandle, theme_page: Option<ThemePage>) {
    if let Some(f) = fenetre(app) {
        if let Err(e) = f.set_theme(theme::pour_cadre(theme_page)) {
            log::warn!("editeur: theme du cadre non regle: {e}");
        }
    }
}

/// Détruit la fenêtre si elle est ouverte (arrêt, fermeture décidée) : moteur de rendu,
/// page et toiles sont rendus (jamais seulement cachée).
pub fn fermer(app: &AppHandle) {
    if let Some(f) = fenetre(app) {
        if let Err(e) = f.destroy() {
            log::warn!("editeur: fenetre non detruite: {e}");
        }
    }
}

/// Identifiant système de la fenêtre ouverte (HWND sous Windows).
pub fn identifiant(app: &AppHandle) -> Option<isize> {
    fenetre(app).and_then(|f| systeme::identifiant_fenetre(&f))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ecran(x: i32, y: i32, l: u32, h: u32, e: f64) -> Ecran {
        let r = Rect {
            x,
            y,
            largeur: l,
            hauteur: h,
        };
        Ecran {
            rect: r,
            zone_utile: Rect {
                hauteur: h - (48.0 * e) as u32,
                ..r
            },
            echelle: e,
        }
    }

    fn dans(r: &Rect, zone: &Rect) -> bool {
        r.x >= zone.x
            && r.y >= zone.y
            && i64::from(r.x) + i64::from(r.largeur) <= i64::from(zone.x) + i64::from(zone.largeur)
            && i64::from(r.y) + i64::from(r.hauteur) <= i64::from(zone.y) + i64::from(zone.hauteur)
    }

    fn jeton(nom: &str) -> f64 {
        let css = include_str!("../../../interface/commun/jetons.css");
        let debut = format!("--{nom}:");
        let ligne = css
            .lines()
            .map(str::trim)
            .find(|l| l.starts_with(&debut))
            .unwrap_or_else(|| panic!("jeton {nom} absent"));
        let valeur = ligne[debut.len()..].trim().trim_end_matches(';');
        valeur
            .trim_end_matches("px")
            .parse()
            .unwrap_or_else(|_| panic!("jeton {nom} illisible"))
    }

    #[test]
    fn largeur_d_ouverture_egale_au_jeton() {
        assert_eq!(LARGEUR_OUVERTURE, jeton("editeur-ouverture-largeur"));
        assert!(LARGEUR_OUVERTURE >= TAILLE_MIN.0);
    }

    #[test]
    fn image_petite_juste_trop_grande() {
        let e = ecran(0, 0, 1920, 1080, 1.0);
        // Petite : la largeur d'ouverture (la barre dépliée tient) et la hauteur minimale,
        // centrées.
        let r = placer((100, 80), &e);
        assert_eq!((r.largeur, r.hauteur), (1088, 440));
        assert_eq!((r.x, r.y), (416, 296));
        // Juste : l'image à la taille réelle, plus les réserves de la page.
        let r = placer((1100, 600), &e);
        assert_eq!((r.largeur, r.hauteur), (1172, 780));
        // Trop grande : 90 % de la zone utile.
        let r = placer((3000, 2000), &e);
        assert_eq!((r.largeur, r.hauteur), (1728, 928));
        assert_eq!((r.x, r.y), (96, 52));
        assert!(dans(&r, &e.zone_utile));
    }

    #[test]
    fn echelles_de_100_a_200() {
        for (echelle, attendu) in [
            (1.0, (2472, 780)),
            (1.25, (2490, 825)),
            (1.5, (2508, 870)),
            (2.0, (2544, 960)),
        ] {
            let e = ecran(0, 0, 3840, 2160, echelle);
            let r = placer((2400, 600), &e);
            assert_eq!((r.largeur, r.hauteur), attendu, "{echelle}");
            assert!(dans(&r, &e.zone_utile));
        }
        // Petite image : la largeur d'ouverture suit l'échelle (1088 px logiques).
        for (echelle, largeur) in [(1.0, 1088), (1.25, 1360), (1.5, 1632), (2.0, 2176)] {
            let e = ecran(0, 0, 3840, 2160, echelle);
            assert_eq!(placer((300, 200), &e).largeur, largeur, "{echelle}");
        }
        // 1366 × 768 à 125 % : 1360 ne tient pas dans 90 % de la zone utile, 1229 oui.
        let e = ecran(0, 0, 1366, 768, 1.25);
        let r = placer((300, 200), &e);
        assert_eq!(r.largeur, 1229);
        assert!(dans(&r, &e.zone_utile));
        // 1024 × 768 à 100 % : la capture plein écran tient dans 90 % de la zone utile.
        let e = ecran(0, 0, 1024, 768, 1.0);
        let r = placer((1024, 768), &e);
        assert_eq!((r.largeur, r.hauteur), (921, 648));
        assert!(dans(&r, &e.zone_utile));
    }

    #[test]
    fn ecran_negatif_et_barre_a_gauche() {
        let e = ecran(-1920, -200, 1920, 1080, 1.0);
        let r = placer((400, 300), &e);
        assert!(dans(&r, &e.zone_utile));
        // 400 + 72 sous la largeur d'ouverture ; 300 + 180 = 480 de haut.
        assert_eq!((r.largeur, r.hauteur), (1088, 480));
        assert_eq!((r.x, r.y), (-1920 + 416, -200 + 276));
        // Barre des tâches à gauche : la zone utile commence à 48.
        let gauche = Ecran {
            rect: Rect {
                x: 0,
                y: 0,
                largeur: 1920,
                hauteur: 1080,
            },
            zone_utile: Rect {
                x: 48,
                y: 0,
                largeur: 1872,
                hauteur: 1080,
            },
            echelle: 1.0,
        };
        let r = placer((5000, 5000), &gauche);
        assert_eq!(r.x, 48 + (1872 - 1684) / 2);
        assert!(dans(&r, &gauche.zone_utile));
    }

    #[test]
    fn toujours_dans_la_zone_utile() {
        // Petit écran où même la taille minimale ne tient pas : 90 % l'emporte.
        let petit = ecran(0, 0, 600, 400, 1.0);
        let r = placer((10, 10), &petit);
        assert!(dans(&r, &petit.zone_utile));
        for echelle in [1.0, 1.25, 1.5, 1.75, 2.0, 0.0, f64::NAN] {
            for image in [
                (1, 1),
                (640, 480),
                (1920, 1080),
                (5760, 1080),
                (16_384, 1525),
            ] {
                let e = ecran(-1280, 0, 1280, 1024, echelle);
                let r = placer(image, &e);
                assert!(dans(&r, &e.zone_utile), "{echelle} {image:?} {r:?}");
                assert!(r.largeur >= 1 && r.hauteur >= 1);
            }
        }
    }
}
