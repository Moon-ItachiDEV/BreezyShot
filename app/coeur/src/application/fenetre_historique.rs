//! L'historique, côté Application : les huit commandes de la page (contrat 0.5, §6.3),
//! permises à la seule fenêtre `historique` (`capabilities/historique.json`). Fil
//! principal, sauf `historique_apercu` (lecture de fichier). Un `nom` qui n'a pas le
//! format exact ou qui n'est plus dans la liste est refusé : la page ne donne jamais un
//! chemin.
//!
//! Socle : inertes (chantier D).

use serde::Serialize;
use tauri::AppHandle;

/// Réponse d'une commande que le socle ne fait pas.
const NON_CONSTRUIT: &str = "historique";

/// Ce que `historique_pret` rend à la page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChargeHistorique {
    pub reduites: bool,
}

fn refusee(commande: &str) {
    log::info!("historique {commande} refusee");
}

/// La page est prête : rend la charge ; la fenêtre est montrée au premier plan.
#[tauri::command]
pub fn historique_pret(app: AppHandle) -> Result<ChargeHistorique, String> {
    let _ = app;
    refusee("pret");
    Err(NON_CONSTRUIT.into())
}

/// Les octets PNG de l'aperçu de `nom` (lecture hors du fil principal).
#[tauri::command]
pub async fn historique_apercu(
    app: AppHandle,
    nom: String,
) -> Result<tauri::ipc::Response, String> {
    let _ = (app, nom);
    refusee("apercu");
    Err(NON_CONSTRUIT.into())
}

/// Copier, Enregistrer, Annoter ou Supprimer sur la case `nom`.
#[tauri::command]
pub fn historique_action(app: AppHandle, nom: String, action: String) {
    let _ = (app, nom, action);
    refusee("action");
}

/// Annule les suppressions en attente.
#[tauri::command]
pub fn historique_annuler(app: AppHandle) {
    let _ = app;
    refusee("annuler");
}

/// « Tout effacer », confirmé par la page.
#[tauri::command]
pub fn historique_tout_effacer(app: AppHandle) {
    let _ = app;
    refusee("tout_effacer");
}

/// Glisser de la case `nom` vers une autre appli.
#[tauri::command]
pub fn historique_glisser(app: AppHandle, nom: String) {
    let _ = (app, nom);
    refusee("glisser");
}

/// La première rangée de `n` cases est peinte.
#[tauri::command]
pub fn historique_affiche(app: AppHandle, n: u32) {
    let _ = (app, n);
    refusee("affiche");
}

/// Bouton Fermer de la page.
#[tauri::command]
pub fn historique_fermer(app: AppHandle) {
    let _ = app;
    refusee("fermer");
}
