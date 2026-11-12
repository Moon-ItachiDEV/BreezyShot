//! L'éditeur d'annotation, côté Application (contrat 0.4 « Éditeur », §6) : les huit
//! commandes de la page (fil principal, sans `async`), permises à la seule fenêtre
//! `editeur` (`capabilities/editeur.json`). Les décisions sont dans `editeur.rs`.
//!
//! Un `id` qui n'est pas celui du document en cours est ignoré (`editeur <commande> <id>
//! ignoree`).
//!
//! SOCLE : commandes inertes ; chaque corps « Provisoire (A) » est remplacé par le
//! chantier A.

use tauri::AppHandle;

use super::editeur::ChargeEditeur;

/// Réponse des commandes tant que l'éditeur n'est pas construit.
const PAS_CONSTRUIT: &str = "pas construit";

/// La page est prête (thème posé, écoutes en place) : rend la charge, la fenêtre est
/// montrée au premier plan.
#[tauri::command]
pub fn editeur_pret(app: AppHandle) -> Result<ChargeEditeur, String> {
    // Provisoire (A) : refusée.
    let _ = app;
    Err(PAS_CONSTRUIT.into())
}

/// Les pixels du document `id` : exactement `largeur × hauteur × 4` octets RGBA, bruts.
#[tauri::command]
pub fn editeur_image(app: AppHandle, id: u64) -> Result<tauri::ipc::Response, String> {
    // Provisoire (A) : refusée.
    let _ = (app, id);
    Err(PAS_CONSTRUIT.into())
}

/// Premier dessin de l'image (chrono d'ouverture).
#[tauri::command]
pub fn editeur_affiche(app: AppHandle, id: u64) {
    // Provisoire (A) : ignorée.
    let _ = (app, id);
}

/// Fin d'un changement de vue : l'image dans la page, pixels CSS, et le zoom.
#[tauri::command]
pub fn editeur_vue(app: AppHandle, id: u64, x: f64, y: f64, largeur: f64, hauteur: f64, zoom: f64) {
    // Provisoire (A) : ignorée.
    let _ = (app, id, x, y, largeur, hauteur, zoom);
}

/// Fin d'une modification du document (250 ms au plus entre deux, et la dernière).
#[tauri::command]
pub fn editeur_etat(app: AppHandle, id: u64, version: u64, vide: bool) {
    // Provisoire (A) : ignorée.
    let _ = (app, id, version, vide);
}

/// Un export : corps brut RGBA, en-têtes `x-editeur-id`, `-version`, `-but`, `-largeur`,
/// `-hauteur`. Rend le numéro de l'export, ou le code du refus (`Refus::code`).
#[tauri::command]
pub fn editeur_exporter(app: AppHandle, requete: tauri::ipc::Request<'_>) -> Result<u64, String> {
    // Provisoire (A) : refusé.
    let _ = (app, requete);
    Err(PAS_CONSTRUIT.into())
}

/// La page veut fermer : rend "fermee" (et ferme) ou "demander" ; `force` ferme sans
/// demander. `preferences` : JSON opaque de la page, gardé pour la session (R9).
#[tauri::command]
pub fn editeur_fermer(
    app: AppHandle,
    id: u64,
    version: u64,
    vide: bool,
    force: bool,
    preferences: Option<String>,
) -> Result<String, String> {
    // Provisoire (A) : refusée.
    let _ = (app, id, version, vide, force, preferences);
    Err(PAS_CONSTRUIT.into())
}

/// « Continuer » du dialogue ouvert pour une nouvelle capture : le document en attente est
/// rendu.
#[tauri::command]
pub fn editeur_garder(app: AppHandle, id: u64) {
    // Provisoire (A) : ignorée.
    let _ = (app, id);
}
