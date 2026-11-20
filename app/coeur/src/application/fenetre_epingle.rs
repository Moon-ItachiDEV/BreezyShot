//! Commandes Tauri des fenêtres de captures épinglées (`epingle-*`).

use tauri::{AppHandle, WebviewWindow};

use super::epingle::{ChargeEpingle, TailleEpingle};
use super::geste_epingle;
use crate::fenetres::epingle::PREFIXE_ETIQUETTE;

/// Extrait l'identifiant numérique depuis l'étiquette `epingle-{id}` de la fenêtre appelante.
/// Empêche une fenêtre épinglée d'agir sur une autre.
fn id_de_fenetre(fenetre: &WebviewWindow) -> Result<u64, String> {
    fenetre
        .label()
        .strip_prefix(PREFIXE_ETIQUETTE)
        .and_then(|s| s.parse::<u64>().ok())
        .ok_or_else(|| "fenetre epingle invalide".to_string())
}

#[tauri::command]
pub fn epingle_pret(app: AppHandle, fenetre: WebviewWindow) -> Result<ChargeEpingle, String> {
    let id = id_de_fenetre(&fenetre)?;
    geste_epingle::pret(&app, id)
}

#[tauri::command]
pub fn epingle_image(
    app: AppHandle,
    fenetre: WebviewWindow,
) -> Result<tauri::ipc::Response, String> {
    let id = id_de_fenetre(&fenetre)?;
    geste_epingle::image(&app, id).map(tauri::ipc::Response::new)
}

#[tauri::command]
pub fn epingle_redimensionner(
    app: AppHandle,
    fenetre: WebviewWindow,
    facteur: f64,
) -> Result<TailleEpingle, String> {
    let id = id_de_fenetre(&fenetre)?;
    geste_epingle::redimensionner(&app, id, facteur)
}

#[tauri::command]
pub fn epingle_opacite(
    app: AppHandle,
    fenetre: WebviewWindow,
    opacite: f64,
) -> Result<f64, String> {
    let id = id_de_fenetre(&fenetre)?;
    geste_epingle::opacite(&app, id, opacite)
}

#[tauri::command]
pub async fn epingle_copier(app: AppHandle, fenetre: WebviewWindow) -> Result<(), String> {
    let id = id_de_fenetre(&fenetre)?;
    let (pixels, demande) = geste_epingle::preparer_copie(&app, id)?;
    tauri::async_runtime::spawn_blocking(move || {
        geste_epingle::copier_pixels(id, &pixels, &demande)
    })
    .await
    .map_err(|e| format!("tache copie: {e}"))?
}

#[tauri::command]
pub fn epingle_fermer(app: AppHandle, fenetre: WebviewWindow) {
    if let Ok(id) = id_de_fenetre(&fenetre) {
        geste_epingle::fermer(&app, id);
    }
}
