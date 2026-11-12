//! Fenêtre des Réglages (étape 10) : une fenêtre Windows ordinaire (cadre, barre des
//! tâches, Alt+Tab), une seule à la fois, créée à la demande et détruite à la fermeture.
//!
//! Le module crée, ramène devant et prévient la page ; il ne décide de rien : les commandes
//! de la page sont dans l'Application, qui seule connaît les réglages et les raccourcis.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use super::theme::{self, ThemePage};
use super::ARGUMENTS_NAVIGATEUR;
use crate::systeme;

/// Étiquette de la fenêtre (aussi dans `capabilities/reglages.json`).
pub const ETIQUETTE_REGLAGES: &str = "reglages";
/// Événement écouté par la page : un nouvel état complet à afficher.
pub const EVENEMENT_ETAT: &str = "reglages:etat";
/// Titre de la fenêtre (barre de titre, barre des tâches).
pub const TITRE: &str = "Réglages de BreezyShot";
/// Taille d'ouverture et taille minimale, en pixels logiques.
pub const TAILLE: (f64, f64) = (780.0, 540.0);
pub const TAILLE_MIN: (f64, f64) = (560.0, 420.0);

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
    .center()
    .resizable(true)
    .theme(theme::pour_cadre(theme::actuel()))
    .additional_browser_args(ARGUMENTS_NAVIGATEUR)
    .build()
    .map_err(|e| format!("fenetre des reglages: {e}"))?;
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
