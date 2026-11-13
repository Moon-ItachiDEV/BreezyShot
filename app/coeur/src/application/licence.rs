//! La licence, côté Application (avenant 0.5 « Licence », §3.3) : lue au démarrage,
//! activée ou retirée par la fenêtre Réglages (trois commandes, permises à la seule fenêtre
//! `reglages`). Après un changement : `Evenement::LicenceChangee`, qui refait le menu et les
//! raccourcis. Le texte de la licence n'est jamais écrit au journal.
//!
//! Socle : inerte, aucune licence (chantier L pour les corps).

use serde::Serialize;
use tauri::AppHandle;

/// Ce que les commandes rendent à la page (`{"active":true}`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EtatLicence {
    pub active: bool,
}

/// Démarrage, avant la réservation des raccourcis : `sortie::lire_licence` puis
/// `licence::verifier`, rangée par `*super::licence_en_vigueur(app) = …` (sans événement :
/// les listes ne sont pas encore faites). Après le démarrage, `super::poser_licence`.
pub fn charger(_app: &AppHandle) {}

/// Vérifie et range `texte` ; l'erreur est le code du refus.
pub fn activer(_app: &AppHandle, _texte: &str) -> Result<EtatLicence, &'static str> {
    Err(crate::licence::Refus::Signature.code())
}

/// Retire la licence : texte rangé effacé, plus de Pro, listes refaites.
pub fn retirer(app: &AppHandle) -> EtatLicence {
    if let Err(e) = crate::sortie::effacer_licence() {
        log::warn!("licence non effacee: {e}");
    }
    super::poser_licence(app, None);
    etat(app)
}

/// L'état de la licence en vigueur.
pub fn etat(app: &AppHandle) -> EtatLicence {
    EtatLicence {
        active: super::pro(app),
    }
}

#[tauri::command]
pub fn licence_etat(app: AppHandle) -> EtatLicence {
    etat(&app)
}

#[tauri::command]
pub fn licence_activer(app: AppHandle, texte: String) -> Result<EtatLicence, String> {
    activer(&app, &texte).map_err(String::from)
}

#[tauri::command]
pub fn licence_retirer(app: AppHandle) -> EtatLicence {
    retirer(&app)
}
