//! Vidéo et GIF, côté Application : les trois commandes de la page de contrôle du film
//! (contrat 0.7, §11), permises à la seule fenêtre `film` (`capabilities/film.json`). Fil
//! principal. Un `id` qui n'est pas celui du film en cours, une action inconnue, ou
//! `pause` pendant le décompte sont ignorés. Ici, seulement des gestes : les décisions
//! sont dans `film.rs`.
//!
//! Socle : corps inertes (chantier F) ; rien n'appelle ces commandes tant que la fenêtre
//! de contrôle n'existe pas.

use serde::Serialize;
use tauri::AppHandle;

/// Réponse d'une commande refusée (aucun film en cours).
const REFUS: &str = "film";

/// Ce que `film_pret` rend à la page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChargeFilm {
    pub reduites: bool,
}

/// La page est prête (thème posé, écoutes en place) : rend la charge ; `film:montrer`
/// suit aussitôt.
#[tauri::command]
pub fn film_pret(app: AppHandle) -> Result<ChargeFilm, String> {
    let _ = app;
    Err(REFUS.into())
}

/// La pilule est posée : son rectangle réel, en pixels CSS de la page (rendu cliquable
/// quand la souris y est). La première fois, la fenêtre de contrôle est montrée.
#[tauri::command]
pub fn film_zone(app: AppHandle, id: u64, x: f64, y: f64, largeur: f64, hauteur: f64) {
    let _ = (app, id, x, y, largeur, hauteur);
}

/// Un clic sur la pilule : `pause`, `arreter` ou `micro`.
#[tauri::command]
pub fn film_action(app: AppHandle, id: u64, action: String) {
    let _ = (app, id, action);
}
