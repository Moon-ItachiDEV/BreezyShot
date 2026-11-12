//! L'éditeur d'annotation, côté Application (contrat 0.4 « Éditeur », §6) : les huit
//! commandes de la page (fil principal, sans `async`), permises à la seule fenêtre
//! `editeur` (`capabilities/editeur.json`). Les décisions sont dans `editeur.rs`, les
//! gestes communs dans `mod.rs` ; ici, seulement des gestes.
//!
//! Un `id` qui n'est pas celui du document en cours est ignoré (`editeur <commande> <id>
//! ignoree`).

use std::sync::Arc;
use std::time::Instant;

use tauri::AppHandle;

use super::editeur::{self, ChargeEditeur, Fermeture, RaisonFermeture};
use super::{
    afficher, avec_editeur, exporter_editeur, fermer_editeur, fermer_ou_suivant,
    garder_preferences_editeur, preferences_editeur, reduites_editeur,
};
use crate::fenetres::editeur as fenetre;
use crate::fenetres::theme;

/// Réponse d'une commande pour un autre document que celui en cours.
const AUTRE_DOCUMENT: &str = "id";

fn ignoree(commande: &str, id: u64) {
    log::info!("editeur {commande} {id} ignoree");
}

/// La page est prête (thème posé, écoutes en place) : rend la charge, la fenêtre est
/// montrée au premier plan.
#[tauri::command]
pub fn editeur_pret(app: AppHandle) -> Result<ChargeEditeur, String> {
    let reduites = reduites_editeur(&app);
    let preferences = preferences_editeur(&app);
    let Some((charge, demande_a)) = avec_editeur(&app, None, |e| {
        e.page_prete = true;
        (editeur::charge(e, reduites, preferences), e.demande_a)
    }) else {
        ignoree("pret", 0);
        return Err(AUTRE_DOCUMENT.into());
    };
    theme::envoyer_a(&app, fenetre::ETIQUETTE_EDITEUR);
    fenetre::montrer(&app);
    log::info!(
        "editeur pret {} en {} ms",
        charge.id,
        demande_a.elapsed().as_millis()
    );
    Ok(charge)
}

/// Les pixels du document `id` : exactement `largeur × hauteur × 4` octets RGBA, bruts.
#[tauri::command]
pub fn editeur_image(app: AppHandle, id: u64) -> Result<tauri::ipc::Response, String> {
    let debut = Instant::now();
    let Some(pixels) = avec_editeur(&app, Some(id), |e| Arc::clone(&e.document.pixels)) else {
        ignoree("image", id);
        return Err(AUTRE_DOCUMENT.into());
    };
    let octets = pixels.rgba.clone();
    drop(pixels);
    log::info!(
        "editeur image envoyee {id} {} octets en {} ms",
        octets.len(),
        debut.elapsed().as_millis()
    );
    Ok(tauri::ipc::Response::new(octets))
}

/// Premier dessin de l'image (chrono d'ouverture, depuis le clic).
#[tauri::command]
pub fn editeur_affiche(app: AppHandle, id: u64) {
    let Some((premier, demande_a)) = avec_editeur(&app, Some(id), |e| {
        let premier = !e.affichee;
        e.affichee = true;
        (premier, e.demande_a)
    }) else {
        ignoree("affiche", id);
        return;
    };
    if premier {
        log::info!(
            "editeur image affichee {id} en {} ms",
            demande_a.elapsed().as_millis()
        );
    }
}

/// Fin d'un changement de vue : l'image dans la page, pixels CSS, et le zoom.
#[tauri::command]
pub fn editeur_vue(app: AppHandle, id: u64, x: f64, y: f64, largeur: f64, hauteur: f64, zoom: f64) {
    if avec_editeur(&app, Some(id), |_| ()).is_none() {
        ignoree("vue", id);
        return;
    }
    let Some((page, echelle)) = fenetre::rect_page(&app) else {
        return;
    };
    if let Some(vue) = editeur::vue_ecran(page, echelle, x, y, largeur, hauteur) {
        if zoom.is_finite() {
            log::info!("{}", editeur::ligne_vue(id, &vue, zoom));
        }
    }
}

/// Fin d'une modification du document (250 ms au plus entre deux, et la dernière).
#[tauri::command]
pub fn editeur_etat(app: AppHandle, id: u64, version: u64, vide: bool) {
    if avec_editeur(&app, Some(id), |e| {
        e.version = version;
        e.vide = vide;
    })
    .is_none()
    {
        ignoree("etat", id);
    }
}

/// Un export : corps brut RGBA, en-têtes `x-editeur-id`, `-version`, `-but`, `-largeur`,
/// `-hauteur`. Rend le numéro de l'export, ou le code du refus (`Refus::code`).
#[tauri::command]
pub fn editeur_exporter(app: AppHandle, requete: tauri::ipc::Request<'_>) -> Result<u64, String> {
    exporter_editeur(&app, &requete)
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
    garder_preferences_editeur(&app, preferences);
    let Some((repondue, affichee, decision)) = avec_editeur(&app, Some(id), |e| {
        e.version = version;
        e.vide = vide;
        let repondue = e.fermeture_demandee.take().is_some();
        (
            repondue,
            e.affichee,
            editeur::fermeture(version, vide, e.exportee),
        )
    }) else {
        ignoree("fermer", id);
        return Err(AUTRE_DOCUMENT.into());
    };
    if !repondue && !force {
        log::info!("editeur fermeture demandee {id} (page)");
    }
    // Une page qui ferme d'office avant d'avoir montré l'image l'a refusée (§4.2).
    if force && !affichee {
        log::warn!("editeur impossible {id}: image");
        fermer_editeur(&app, RaisonFermeture::Impossible);
        afficher(&app, editeur::message_impossible());
        return Ok("fermee".into());
    }
    if !force && decision == Fermeture::Demander {
        log::info!("editeur fermeture {id}: demander");
        return Ok("demander".into());
    }
    let raison = if force {
        RaisonFermeture::SansGarder
    } else {
        RaisonFermeture::Propre
    };
    fermer_ou_suivant(&app, raison);
    Ok("fermee".into())
}

/// « Continuer » du dialogue de fermeture : le document en attente (nouvelle capture) est
/// rendu.
#[tauri::command]
pub fn editeur_garder(app: AppHandle, id: u64) {
    let Some(abandonnee) = avec_editeur(&app, Some(id), |e| {
        e.fermeture_demandee = None;
        e.en_attente.take().map(|d| d.id)
    }) else {
        ignoree("garder", id);
        return;
    };
    if let Some(a) = abandonnee {
        log::info!("editeur: capture {a} abandonnee");
    }
}
