//! L'historique, côté Application : les huit commandes de la page (contrat 0.5, §6.3),
//! permises à la seule fenêtre `historique` (`capabilities/historique.json`). Fil
//! principal, sauf `historique_apercu` (lecture de fichier). Un `nom` qui n'a pas le
//! format exact ou qui n'est plus dans la liste est refusé : la page ne donne jamais un
//! chemin. Ici, seulement des gestes : les décisions sont dans `historique.rs`.

use std::time::Instant;

use serde::Serialize;
use tauri::AppHandle;

use super::geste_historique as geste;
use super::historique::{self, ActionCase, RaisonFermeture};
use crate::fenetres::historique as fenetre;
use crate::fenetres::theme;
use crate::sortie;

/// Réponse d'une commande refusée (historique fermé, nom refusé).
const REFUS: &str = "historique";
/// Réponse de `historique_apercu` quand l'aperçu n'est pas (encore) fait.
const ABSENT: &str = "absent";
/// Taille au plus d'un aperçu lu (400 × 250 en PNG en est très loin).
const APERCU_OCTETS_MAX: u64 = 8 << 20;

/// Ce que `historique_pret` rend à la page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChargeHistorique {
    pub reduites: bool,
}

fn refusee(commande: &str) {
    log::info!("historique {commande} refusee");
}

/// La page est prête (thème posé, écoutes en place) : rend la charge ; la fenêtre est
/// montrée au premier plan, puis `historique:liste` suit.
#[tauri::command]
pub fn historique_pret(app: AppHandle) -> Result<ChargeHistorique, String> {
    let reduites = super::reduites_editeur(&app);
    let pret = super::historique_ouvert(&app).as_mut().map(|e| {
        e.page_prete = true;
        (e.numero, e.ouverte_a)
    });
    let Some((numero, ouverte_a)) = pret else {
        refusee("pret");
        return Err(REFUS.into());
    };
    geste::desarmer_filet();
    theme::envoyer_a(&app, fenetre::ETIQUETTE_HISTORIQUE);
    fenetre::montrer(&app);
    log::info!("historique pret en {} ms", ouverte_a.elapsed().as_millis());
    geste::lancer_liste(&app, numero);
    Ok(ChargeHistorique { reduites })
}

/// Les octets PNG de l'aperçu de `nom` (lecture hors du fil principal) ; refus s'il
/// manque (la page attend alors `historique:apercu`).
#[tauri::command]
pub async fn historique_apercu(
    app: AppHandle,
    nom: String,
) -> Result<tauri::ipc::Response, String> {
    let Some(chemin) = chemin_apercu(&app, &nom) else {
        refusee("apercu");
        return Err(REFUS.into());
    };
    let lu = tauri::async_runtime::spawn_blocking(move || {
        let taille = std::fs::metadata(&chemin).ok()?.len();
        (taille <= APERCU_OCTETS_MAX)
            .then(|| std::fs::read(&chemin).ok())
            .flatten()
    })
    .await
    .ok()
    .flatten();
    lu.map(tauri::ipc::Response::new)
        .ok_or_else(|| ABSENT.into())
}

/// Le chemin de l'aperçu d'une case connue (verrou rendu avant toute attente).
fn chemin_apercu(app: &AppHandle, nom: &str) -> Option<std::path::PathBuf> {
    if !geste::connue(app, nom) {
        return None;
    }
    let (recents, _) = super::dossiers_vignette(app);
    let local = recents.parent()?;
    sortie::chemin_apercu(&local.join(historique::SOUS_DOSSIER_APERCUS), nom)
}

/// Copier, Enregistrer, Annoter ou Supprimer sur la case `nom`.
#[tauri::command]
pub fn historique_action(app: AppHandle, nom: String, action: String) {
    match ActionCase::depuis_nom(&action) {
        Some(a) if geste::connue(&app, &nom) => geste::action(&app, nom, a),
        _ => refusee("action"),
    }
}

/// Annule les suppressions en attente.
#[tauri::command]
pub fn historique_annuler(app: AppHandle) {
    geste::annuler(&app);
}

/// « Tout effacer », confirmé par la page.
#[tauri::command]
pub fn historique_tout_effacer(app: AppHandle) {
    geste::tout_effacer(&app);
}

/// Glisser de la case `nom` vers une autre appli.
#[tauri::command]
pub fn historique_glisser(app: AppHandle, nom: String) {
    if geste::connue(&app, &nom) {
        geste::glisser(&app, nom);
    } else {
        refusee("glisser");
    }
}

/// La première rangée de `n` cases est peinte (chrono d'ouverture, écrit une fois).
#[tauri::command]
pub fn historique_affiche(app: AppHandle, n: u32) {
    let premier = super::historique_ouvert(&app).as_mut().map(|e| {
        let premier = !e.affichee;
        e.affichee = true;
        (premier, e.ouverte_a)
    });
    match premier {
        Some((true, ouverte_a)) => log::info!(
            "historique affiche {n} en {} ms",
            Instant::now()
                .saturating_duration_since(ouverte_a)
                .as_millis()
        ),
        Some((false, _)) => {}
        None => refusee("affiche"),
    }
}

/// Bouton Fermer de la page.
#[tauri::command]
pub fn historique_fermer(app: AppHandle) {
    geste::fermer(&app, RaisonFermeture::Page);
}
