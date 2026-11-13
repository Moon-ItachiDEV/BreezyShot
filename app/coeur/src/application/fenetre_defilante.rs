//! Capture défilante, côté Application : les quatre commandes de la page de contrôle
//! (contrat 0.6, §11), permises à la seule fenêtre `defilante`
//! (`capabilities/defilante.json`). Fil principal, sauf `defilante_apercu`. Un `id` qui
//! n'est pas celui de la capture en cours est ignoré. Ici, seulement des gestes : les
//! décisions sont dans `defilante.rs`.

use serde::Serialize;
use tauri::AppHandle;

/// Réponse d'une commande refusée (aucune défilante en cours, `id` périmé).
const REFUS: &str = "defilante";

/// Ce que `defilante_pret` rend à la page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChargeDefilante {
    pub reduites: bool,
}

/// La page est prête (thème posé, écoutes en place) : rend la charge ; `defilante:montrer`
/// suit quand la zone est choisie.
#[tauri::command]
pub fn defilante_pret(app: AppHandle) -> Result<ChargeDefilante, String> {
    super::geste_defilante::page_prete(&app)
        .map(|reduites| ChargeDefilante { reduites })
        .ok_or_else(|| REFUS.into())
}

/// La pilule est posée : son rectangle réel, en pixels CSS de la page (rendu cliquable
/// quand la souris y est). La première fois, la fenêtre de contrôle est montrée.
#[tauri::command]
pub fn defilante_zone(app: AppHandle, id: u64, x: f64, y: f64, largeur: f64, hauteur: f64) {
    super::geste_defilante::pilule_posee(&app, id, x, y, largeur, hauteur);
}

/// « Terminer » : fin `bouton` (D14).
#[tauri::command]
pub fn defilante_terminer(app: AppHandle, id: u64) {
    super::geste_defilante::terminer(&app, id);
}

/// Les octets PNG de l'aperçu en direct `numero` (fait hors du fil principal) ; refus s'il
/// n'est pas (ou plus) celui en cours.
#[tauri::command]
pub async fn defilante_apercu(
    app: AppHandle,
    id: u64,
    numero: u64,
) -> Result<tauri::ipc::Response, String> {
    super::geste_defilante::apercu_png(&app, id, numero)
        .map(tauri::ipc::Response::new)
        .map_err(|_| REFUS.into())
}
