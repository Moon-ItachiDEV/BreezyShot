//! Fenêtre des Réglages (étape 10) : une fenêtre Windows ordinaire (cadre, barre des
//! tâches, Alt+Tab), une seule à la fois, créée à la demande et détruite à la fermeture.
//!
//! Le module crée, ramène devant et prévient la page ; il ne décide de rien : les commandes
//! de la page sont dans l'Application, qui seule connaît les réglages et les raccourcis.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use super::place::{centrer, ecran_sous, Ecran};
use super::theme::{self, ThemePage};
use super::{ecrans, souris, ARGUMENTS_NAVIGATEUR};
use crate::systeme;

/// Étiquette de la fenêtre (aussi dans `capabilities/reglages.json`).
pub const ETIQUETTE_REGLAGES: &str = "reglages";
/// Événement écouté par la page : un nouvel état complet à afficher.
pub const EVENEMENT_ETAT: &str = "reglages:etat";
/// Titre de la fenêtre (barre de titre, barre des tâches).
pub const TITRE: &str = "Réglages de BreezyShot";
/// Taille d'ouverture et taille minimale, en pixels logiques.
pub const TAILLE: (f64, f64) = (780.0, 520.0);
pub const TAILLE_MIN: (f64, f64) = (600.0, 420.0);

/// Part de la zone utile que la fenêtre peut prendre au plus : la place du cadre (barre de
/// titre, bords) reste dans la zone utile, au-dessus de la barre des tâches.
pub const PART_MAX: f64 = 0.9;

/// Taille d'ouverture en pixels logiques sur `ecran` : `TAILLE`, bornée à `PART_MAX` de la
/// zone utile (à l'échelle de l'écran), jamais sous `TAILLE_MIN`. Fonction pure.
pub fn taille_sur(ecran: &Ecran) -> (f64, f64) {
    let e = if ecran.echelle.is_finite() && ecran.echelle > 0.0 {
        ecran.echelle
    } else {
        1.0
    };
    let cote = |voulu: f64, min: f64, zone: u32| {
        let max = (f64::from(zone) * PART_MAX / e).floor();
        voulu.min(max).max(min)
    };
    (
        cote(TAILLE.0, TAILLE_MIN.0, ecran.zone_utile.largeur),
        cote(TAILLE.1, TAILLE_MIN.1, ecran.zone_utile.hauteur),
    )
}

/// Ce qu'a fait `ouvrir`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ouverture {
    /// Fenêtre créée (identifiant système, s'il est connu).
    Creee { identifiant: Option<isize> },
    /// Elle existait : ramenée devant.
    RameneeDevant,
}

/// La fenêtre, si elle est ouverte.
pub fn fenetre(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(ETIQUETTE_REGLAGES)
}

/// Identifiant système de la fenêtre ouverte (HWND sous Windows).
pub fn identifiant(app: &AppHandle) -> Option<isize> {
    fenetre(app).and_then(|f| systeme::identifiant_fenetre(&f))
}

/// Ouvre la fenêtre, ou la ramène devant si elle existe (fil principal). `fermee` est
/// appelé quand elle est détruite.
pub fn ouvrir<F>(app: &AppHandle, fermee: F) -> Result<Ouverture, String>
where
    F: Fn() + Send + Sync + 'static,
{
    if let Some(f) = fenetre(app) {
        let _ = f.unminimize();
        let _ = f.show();
        if !matches!(systeme::prendre_premier_plan(&f), Ok(true)) {
            let _ = f.set_focus();
        }
        return Ok(Ouverture::RameneeDevant);
    }
    let (largeur, hauteur) = TAILLE;
    let (lmin, hmin) = TAILLE_MIN;
    let f = WebviewWindowBuilder::new(
        app,
        ETIQUETTE_REGLAGES,
        WebviewUrl::App("reglages/reglages.html".into()),
    )
    .title(TITRE)
    .inner_size(largeur, hauteur)
    .min_inner_size(lmin, hmin)
    .visible(false)
    .resizable(true)
    .theme(theme::pour_cadre(theme::actuel()))
    .additional_browser_args(ARGUMENTS_NAVIGATEUR)
    .build()
    .map_err(|e| format!("fenetre des reglages: {e}"))?;
    // Centrée sur l'écran de la souris (pixels réels, échelle de cet écran), puis montrée.
    if let Some(ecran) = ecran_sous(souris(app), &ecrans(app)) {
        let r = centrer(&ecran, taille_sur(&ecran));
        let _ = f.set_size(tauri::PhysicalSize::new(r.largeur, r.hauteur));
        let _ = f.set_position(tauri::PhysicalPosition::new(r.x, r.y));
    } else {
        let _ = f.center();
    }
    let _ = f.show();
    f.on_window_event(move |e| {
        if matches!(e, tauri::WindowEvent::Destroyed) {
            fermee();
        }
    });
    let _ = f.set_focus();
    Ok(Ouverture::Creee {
        identifiant: systeme::identifiant_fenetre(&f),
    })
}

/// Envoie un état complet à la page, si la fenêtre est ouverte.
pub fn emettre_etat<S: Serialize + Clone>(app: &AppHandle, etat: S) {
    if fenetre(app).is_some() {
        if let Err(e) = app.emit_to(ETIQUETTE_REGLAGES, EVENEMENT_ETAT, etat) {
            log::warn!("reglages: etat non envoye: {e}");
        }
    }
}

/// Le thème du cadre de la fenêtre ouverte.
pub fn regler_cadre(app: &AppHandle, theme_page: Option<ThemePage>) {
    if let Some(f) = fenetre(app) {
        let _ = f.set_theme(theme::pour_cadre(theme_page));
    }
}

/// Ferme la fenêtre si elle est ouverte (arrêt).
pub fn fermer(app: &AppHandle) {
    if let Some(f) = fenetre(app) {
        let _ = f.destroy();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fenetres::Rect;

    fn ecran(l: u32, h: u32, utile_h: u32, echelle: f64) -> Ecran {
        Ecran {
            rect: Rect {
                x: 0,
                y: 0,
                largeur: l,
                hauteur: h,
            },
            zone_utile: Rect {
                x: 0,
                y: 0,
                largeur: l,
                hauteur: utile_h,
            },
            echelle,
        }
    }

    #[test]
    fn taille_voulue_sur_un_grand_ecran() {
        assert_eq!(taille_sur(&ecran(1920, 1080, 1032, 1.0)), TAILLE);
        assert_eq!(taille_sur(&ecran(2560, 1440, 1392, 1.5)), TAILLE);
    }

    #[test]
    fn portable_1366_a_125_pour_cent_cadre_dans_la_zone_utile() {
        // Zone utile de 720 px réels : la fenêtre en prend au plus 90 %, le cadre tient.
        let (l, h) = taille_sur(&ecran(1366, 768, 720, 1.25));
        assert_eq!(l, TAILLE.0);
        assert!(h < TAILLE.1 && h * 1.25 <= 720.0 * PART_MAX, "{h}");
    }

    #[test]
    fn jamais_sous_la_taille_minimale() {
        assert_eq!(
            taille_sur(&ecran(800, 480, 440, 1.0)),
            (720.0, TAILLE_MIN.1)
        );
        assert_eq!(taille_sur(&ecran(1920, 1080, 1032, f64::NAN)), TAILLE);
    }
}
