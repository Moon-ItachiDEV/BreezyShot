//! La licence, côté Application (avenant 0.5 « Licence », §3.3) : lue au démarrage,
//! activée ou retirée par la fenêtre Réglages (trois commandes, permises à la seule fenêtre
//! `reglages`). Après un changement : `Evenement::LicenceChangee`, qui refait le menu et les
//! raccourcis. Le texte de la licence n'est jamais écrit au journal : seuls l'`id` (borné,
//! ASCII imprimable) et le code du refus y vont.

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
pub fn charger(app: &AppHandle) {
    // Un fichier illisible ou une licence refusée : pas de licence, le fichier reste.
    let licence = match crate::sortie::lire_licence() {
        None => {
            log::info!("licence absente");
            None
        }
        Some(texte) => match crate::licence::verifier(&texte) {
            Ok(licence) => {
                log::info!("licence active {}", licence.id);
                Some(licence)
            }
            Err(refus) => {
                log::info!("licence refusee {}", refus.code());
                None
            }
        },
    };
    *super::licence_en_vigueur(app) = licence;
}

/// Vérifie et range `texte` ; l'erreur est le code du refus.
pub fn activer(app: &AppHandle, texte: &str) -> Result<EtatLicence, &'static str> {
    let licence = crate::licence::verifier(texte).map_err(|refus| {
        log::info!("licence refusee {}", refus.code());
        refus.code()
    })?;
    // Rangée telle que collée. Si le disque refuse, la licence vaut pour cette session.
    if let Err(e) = crate::sortie::ecrire_licence(texte) {
        log::warn!("licence non rangee: {e}");
    }
    log::info!("licence active {}", licence.id);
    super::poser_licence(app, Some(licence));
    Ok(etat(app))
}

/// Retire la licence : texte rangé effacé, plus de Pro, listes refaites.
pub fn retirer(app: &AppHandle) -> EtatLicence {
    if let Err(e) = crate::sortie::effacer_licence() {
        log::warn!("licence non effacee: {e}");
    }
    log::info!("licence retiree");
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
