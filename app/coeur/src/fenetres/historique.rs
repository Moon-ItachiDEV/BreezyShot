//! Fenêtre de l'historique (contrat 0.5, §4.1, §6.2) : une fenêtre ordinaire sans cadre
//! Windows, comme l'éditeur (même comportement de tao, voir `editeur.rs`) ; une seule,
//! créée cachée à la demande, montrée à `historique_pret`, détruite à la fermeture,
//! jamais gardée cachée. La croix de la barre des tâches et Alt+F4 la ferment
//! (`CloseRequested` laissé faire).
//!
//! Le module crée, place, montre et prévient la page ; il ne décide de rien.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use super::place::ecran_sous;
use super::theme::{self, ThemePage};
use super::{ecrans, souris, Ecran, Rect, ARGUMENTS_NAVIGATEUR};
use crate::systeme;

/// Étiquette de la fenêtre (aussi dans `capabilities/historique.json`).
pub const ETIQUETTE_HISTORIQUE: &str = "historique";
/// Titre de la fenêtre (barre des tâches, Alt+Tab).
pub const TITRE: &str = "Historique";
/// Page de la fenêtre, sous `app/interface/`.
pub const PAGE: &str = "historique/historique.html";
/// Taille logique voulue, et minimale.
pub const TAILLE: (f64, f64) = (760.0, 560.0);
pub const TAILLE_MIN: (f64, f64) = (480.0, 360.0);
/// Part maximale de la zone utile prise par la fenêtre.
pub const PART_MAX: f64 = 0.9;

/// Événements écoutés par la page (§6.3).
pub const EVENEMENT_LISTE: &str = "historique:liste";
pub const EVENEMENT_APERCU: &str = "historique:apercu";
pub const EVENEMENT_RESULTAT: &str = "historique:resultat";
pub const EVENEMENT_ATTENTE: &str = "historique:attente";
pub const EVENEMENT_GLISSER: &str = "historique:glisser";

fn fenetre(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(ETIQUETTE_HISTORIQUE)
}

/// Échelle utilisable : une valeur absurde vaut 1.
fn echelle_sure(echelle: f64) -> f64 {
    if echelle.is_finite() && echelle > 0.0 {
        echelle
    } else {
        1.0
    }
}

/// Place de la fenêtre sur `ecran` (pure, pixels réels) : `TAILLE` à l'échelle de l'écran,
/// bornée à `TAILLE_MIN` et à 90 % de la zone utile, centrée dans la zone utile.
pub fn placer(ecran: &Ecran) -> Rect {
    let e = echelle_sure(ecran.echelle);
    let zone = ecran.zone_utile;
    let cote = |voulu: f64, min: f64, zone: u32| {
        let max = (f64::from(zone) * PART_MAX).floor().max(1.0);
        (voulu * e).max(min * e).min(max).round().max(1.0) as u32
    };
    let largeur = cote(TAILLE.0, TAILLE_MIN.0, zone.largeur);
    let hauteur = cote(TAILLE.1, TAILLE_MIN.1, zone.hauteur);
    Rect {
        x: (i64::from(zone.x) + (i64::from(zone.largeur) - i64::from(largeur)) / 2) as i32,
        y: (i64::from(zone.y) + (i64::from(zone.hauteur) - i64::from(hauteur)) / 2) as i32,
        largeur,
        hauteur,
    }
}

/// L'écran sous la souris ; à défaut le premier.
pub fn ecran_souris(app: &AppHandle) -> Option<Ecran> {
    ecran_sous(souris(app), &ecrans(app))
}

/// Crée la fenêtre cachée à `place` (fil principal, jamais dans un rappel : toujours
/// postée à un tour suivant de la boucle). `Destroyed` appelle `detruite`. Rend
/// l'identifiant système, s'il est connu. Une fenêtre qui existe encore : une erreur.
pub fn ouvrir<F: Fn() + Send + Sync + 'static>(
    app: &AppHandle,
    place: Rect,
    detruite: F,
) -> Result<Option<isize>, String> {
    if fenetre(app).is_some() {
        return Err("encore ouverte".into());
    }
    let (l, h) = TAILLE;
    let (lmin, hmin) = TAILLE_MIN;
    let f = WebviewWindowBuilder::new(app, ETIQUETTE_HISTORIQUE, WebviewUrl::App(PAGE.into()))
        .title(TITRE)
        .inner_size(l, h)
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
    f.on_window_event(move |e| {
        if let tauri::WindowEvent::Destroyed = e {
            detruite();
        }
    });
    // Comme l'éditeur : sur l'écran de `place`, la zone cliente à sa taille, puis décalée
    // des bords invisibles de l'ombre.
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
    Ok(systeme::identifiant_fenetre(&f))
}

/// Montre la fenêtre au premier plan avec le clavier (repli `set_focus` si Windows le
/// refuse). Faux si elle n'existe pas.
pub fn montrer(app: &AppHandle) -> bool {
    let Some(f) = fenetre(app) else {
        return false;
    };
    let _ = f.unminimize();
    let _ = f.show();
    if !matches!(systeme::prendre_premier_plan(&f), Ok(true)) {
        log::info!("historique premier plan refuse: repli");
        let _ = f.set_focus();
    }
    true
}

/// Ramène la fenêtre ouverte devant. Faux si elle n'existe pas.
pub fn ramener_devant(app: &AppHandle) -> bool {
    montrer(app)
}

/// Envoie `evenement` à la page, si la fenêtre est ouverte.
pub fn emettre<S: Serialize + Clone>(app: &AppHandle, evenement: &str, charge: S) {
    if fenetre(app).is_some() {
        if let Err(e) = app.emit_to(ETIQUETTE_HISTORIQUE, evenement, charge) {
            log::warn!("historique: {evenement} non envoye: {e}");
        }
    }
}

/// Le thème du cadre de la fenêtre ouverte (WebView2 et DWM le suivent, voir l'éditeur).
pub fn regler_cadre(app: &AppHandle, theme_page: Option<ThemePage>) {
    if let Some(f) = fenetre(app) {
        if let Err(e) = f.set_theme(theme::pour_cadre(theme_page)) {
            log::warn!("historique: theme du cadre non regle: {e}");
        }
    }
}

/// Détruit la fenêtre si elle existe : moteur de rendu et page sont rendus.
pub fn fermer(app: &AppHandle) {
    if let Some(f) = fenetre(app) {
        if let Err(e) = f.destroy() {
            log::warn!("historique: fenetre non detruite: {e}");
        }
    }
}

/// Vrai si la fenêtre existe (y compris entre `destroy()` et `Destroyed`).
pub fn ouverte(app: &AppHandle) -> bool {
    fenetre(app).is_some()
}

/// Identifiant système de la fenêtre (Windows : le HWND), pour le glisser.
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
    fn taille_voulue_centree() {
        let e = ecran(0, 0, 1920, 1080, 1.0);
        let r = placer(&e);
        assert_eq!((r.largeur, r.hauteur), (760, 560));
        assert_eq!((r.x, r.y), (580, 236));
        // 1024 × 768 à 100 % (machine d'essai) : la taille voulue tient.
        let e = ecran(0, 0, 1024, 768, 1.0);
        let r = placer(&e);
        assert_eq!((r.largeur, r.hauteur), (760, 560));
        assert!(dans(&r, &e.zone_utile));
    }

    #[test]
    fn echelles_de_100_a_200() {
        for (echelle, attendu) in [
            (1.0, (760, 560)),
            (1.25, (950, 700)),
            (1.5, (1140, 840)),
            (2.0, (1520, 1120)),
        ] {
            let e = ecran(0, 0, 3840, 2160, echelle);
            let r = placer(&e);
            assert_eq!((r.largeur, r.hauteur), attendu, "{echelle}");
            assert!(dans(&r, &e.zone_utile));
        }
        // 1920 × 1080 à 200 % : 90 % de la zone utile l'emporte.
        let e = ecran(0, 0, 1920, 1080, 2.0);
        let r = placer(&e);
        assert_eq!((r.largeur, r.hauteur), (1520, 885));
        assert!(dans(&r, &e.zone_utile));
    }

    #[test]
    fn ecran_negatif_et_barre_a_gauche() {
        let e = ecran(-1920, -200, 1920, 1080, 1.0);
        let r = placer(&e);
        assert_eq!((r.x, r.y), (-1920 + 580, -200 + 236));
        assert!(dans(&r, &e.zone_utile));
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
        let r = placer(&gauche);
        assert_eq!(r.x, 48 + (1872 - 760) / 2);
        assert!(dans(&r, &gauche.zone_utile));
    }

    #[test]
    fn toujours_dans_la_zone_utile() {
        for echelle in [1.0, 1.25, 1.5, 1.75, 2.0, 0.0, f64::NAN, -1.0] {
            for (l, h) in [(600, 400), (800, 600), (1280, 1024), (5760, 1080)] {
                let e = ecran(-1280, 0, l, h, echelle);
                let r = placer(&e);
                assert!(dans(&r, &e.zone_utile), "{echelle} {l}x{h} {r:?}");
                assert!(r.largeur >= 1 && r.hauteur >= 1);
            }
        }
    }
}
