//! Fenêtre de l'éditeur d'annotation (contrat 0.4 « Éditeur », §4.1) : une fenêtre Windows
//! ordinaire (cadre du système, barre des tâches, Alt+Tab), une seule à la fois, créée
//! cachée à la demande, montrée à `editeur_pret`, détruite à la fermeture.
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
/// Titre de la fenêtre (barre de titre, barre des tâches).
pub const TITRE: &str = "Annoter la capture";
/// Page de la fenêtre, sous `app/interface/`.
pub const PAGE: &str = "editeur/editeur.html";
/// Taille intérieure minimale, pixels logiques.
pub const TAILLE_MIN: (f64, f64) = (640.0, 440.0);
/// Marge et barre de la page autour de l'image, pixels logiques.
pub const MARGES: (f64, f64) = (40.0, 84.0);
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
/// image à la taille réelle + `MARGES`, bornée à `TAILLE_MIN` et à `PART_MAX` de la zone
/// utile, centrée dans la zone utile. Le rectangle est celui de la zone intérieure (la
/// page) ; `ouvrir` centre le cadre du système sur le même centre.
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
    let largeur = cote(image.0, MARGES.0, TAILLE_MIN.0, max.0);
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
        .decorations(true)
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
    // Zone intérieure à `place`, puis le cadre centré sur le même centre.
    let _ = f.set_size(tauri::PhysicalSize::new(place.largeur, place.hauteur));
    let (dl, dh) = match (f.outer_size(), f.inner_size()) {
        (Ok(o), Ok(i)) => (
            i64::from(o.width.saturating_sub(i.width)),
            i64::from(o.height.saturating_sub(i.height)),
        ),
        _ => (0, 0),
    };
    let x = i64::from(place.x) - dl / 2;
    let y = i64::from(place.y) - dh / 2;
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

/// Le thème du cadre de la fenêtre ouverte.
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

    #[test]
    fn image_petite_juste_trop_grande() {
        let e = ecran(0, 0, 1920, 1080, 1.0);
        // Petite : la taille minimale, centrée.
        let r = placer((100, 80), &e);
        assert_eq!((r.largeur, r.hauteur), (640, 440));
        assert_eq!((r.x, r.y), (640, 296));
        // Juste : l'image à la taille réelle, plus la marge et la barre.
        let r = placer((1000, 600), &e);
        assert_eq!((r.largeur, r.hauteur), (1040, 684));
        // Trop grande : 90 % de la zone utile.
        let r = placer((3000, 2000), &e);
        assert_eq!((r.largeur, r.hauteur), (1728, 928));
        assert_eq!((r.x, r.y), (96, 52));
        assert!(dans(&r, &e.zone_utile));
    }

    #[test]
    fn echelles_de_100_a_200() {
        for (echelle, attendu) in [
            (1.0, (1040, 684)),
            (1.25, (1050, 705)),
            (1.5, (1060, 726)),
            (2.0, (1280, 880)),
        ] {
            let e = ecran(0, 0, 3840, 2160, echelle);
            let r = placer((1000, 600), &e);
            assert_eq!((r.largeur, r.hauteur), attendu, "{echelle}");
            assert!(dans(&r, &e.zone_utile));
        }
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
        assert_eq!((r.x, r.y), (-1920 + 640, -200 + 296));
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
