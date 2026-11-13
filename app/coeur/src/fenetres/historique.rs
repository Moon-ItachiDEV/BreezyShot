//! Fenêtre de l'historique (contrat 0.5, §4.1, §6.2) : une fenêtre ordinaire sans cadre
//! Windows, comme l'éditeur ; une seule, créée cachée à la demande, montrée à
//! `historique_pret`, détruite à la fermeture, jamais gardée cachée.
//!
//! Le module crée, place, montre et prévient la page ; il ne décide de rien.
//!
//! Socle : signatures (chantier D pour les corps).

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use super::{Ecran, Rect};
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

/// Événements écoutés par la page (§6.3).
pub const EVENEMENT_LISTE: &str = "historique:liste";
pub const EVENEMENT_APERCU: &str = "historique:apercu";
pub const EVENEMENT_RESULTAT: &str = "historique:resultat";
pub const EVENEMENT_ATTENTE: &str = "historique:attente";
pub const EVENEMENT_GLISSER: &str = "historique:glisser";

fn fenetre(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(ETIQUETTE_HISTORIQUE)
}

/// Place de la fenêtre sur `ecran` (pure, pixels réels) : `TAILLE` à l'échelle de l'écran,
/// bornée à `TAILLE_MIN` et à 90 % de la zone utile, centrée dans la zone utile.
pub fn placer(ecran: &Ecran) -> Rect {
    ecran.zone_utile
}

/// Crée la fenêtre cachée à `place` (fil principal, jamais dans un rappel : toujours
/// postée à un tour suivant de la boucle). `Destroyed` appelle `detruite`. Rend
/// l'identifiant système, s'il est connu.
pub fn ouvrir<F: Fn() + Send + Sync + 'static>(
    _app: &AppHandle,
    _place: Rect,
    _detruite: F,
) -> Result<Option<isize>, String> {
    Err("historique non construit".into())
}

/// Montre la fenêtre au premier plan. Faux si elle n'existe pas.
pub fn montrer(_app: &AppHandle) -> bool {
    false
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

/// Détruit la fenêtre si elle existe.
pub fn fermer(app: &AppHandle) {
    if let Some(f) = fenetre(app) {
        if let Err(e) = f.destroy() {
            log::warn!("historique: fenetre non detruite: {e}");
        }
    }
}

/// Vrai si la fenêtre existe.
pub fn ouverte(app: &AppHandle) -> bool {
    fenetre(app).is_some()
}

/// Identifiant système de la fenêtre (Windows : le HWND), pour le glisser.
pub fn identifiant(app: &AppHandle) -> Option<isize> {
    fenetre(app).and_then(|f| systeme::identifiant_fenetre(&f))
}
