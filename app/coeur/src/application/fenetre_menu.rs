//! Le menu de l'icône côté Application (piste A « Liste ») : ouvrir au clic sur l'icône,
//! et les commandes de la page `menu/menu.html`. La fenêtre elle-même est dans
//! `fenetres::menu`.
//!
//! Contrat avec la page :
//! - `menu_pret` : la page écoute `menu:ouvrir` et le thème ; rend les lignes de la
//!   première ouverture (ou `null`).
//! - `menu:ouvrir` { lignes: [{ id, libelle, raccourci, separateur }] } à chaque
//!   ouverture suivante ; `menu:vider` quand le menu est caché.
//! - `menu_choisi` { id } : une ligne choisie ; le menu se cache, l'entrée est traitée au
//!   tour suivant de la boucle (jamais une fenêtre créée dans la commande d'une page).
//! - `menu_fermer` : Échap ou Tab ; le menu se cache.

use tauri::AppHandle;

use super::icone::{self, EntreeMenu};
use super::{entrees_pendant_defilante, geste_defilante, menu_actuel, poster_plus_tard, Evenement};
use crate::fenetres::menu::{self, Ligne, Ouverture};
use crate::fenetres::{theme, Rect};

/// Les lignes de la page d'après les entrées du menu : le texte avant la tabulation est le
/// libellé, après elle le raccourci. Fonction pure.
pub fn lignes(entrees: &[EntreeMenu]) -> Vec<Ligne> {
    entrees
        .iter()
        .map(|e| match e {
            EntreeMenu::Action { id, texte } => {
                let (libelle, raccourci) = match texte.split_once('\t') {
                    Some((l, r)) => (l.to_string(), Some(r.to_string())),
                    None => (texte.clone(), None),
                };
                Ligne {
                    id: id.clone(),
                    libelle,
                    raccourci,
                    separateur: false,
                }
            }
            EntreeMenu::Separateur => Ligne {
                id: String::new(),
                libelle: String::new(),
                raccourci: None,
                separateur: true,
            },
        })
        .collect()
}

/// Clic sur l'icône : ouvre le menu au-dessus d'elle, ou le ferme s'il est ouvert. Un clic
/// qui vient de le fermer (le menu perd le focus au même clic) ne le rouvre pas.
pub fn clic_icone(app: &AppHandle, rect: Rect) {
    log::info!("icone: clic");
    if menu::ouvert(app) {
        let _ = menu::cacher(app);
        return;
    }
    if menu::vient_de_fermer() {
        return;
    }
    let entrees = if geste_defilante::fenetres_bloquees(app) {
        // Pendant une défilante, aucune entrée qui ouvrirait une fenêtre : Quitter seul.
        log::info!("menu pendant la defilante: quitter seulement");
        entrees_pendant_defilante()
    } else {
        let Some(entrees) = menu_actuel(app) else {
            log::warn!("menu non ouvert: application pas prete");
            return;
        };
        entrees
    };
    if menu::ouvrir(app, Some(rect), lignes(&entrees)) {
        icone::tourner(app);
    }
}

#[tauri::command]
pub fn menu_pret(app: AppHandle) -> Option<Ouverture> {
    log::info!("menu page prete");
    theme::envoyer_a(&app, menu::ETIQUETTE_MENU);
    menu::prendre_en_attente()
}

#[tauri::command]
pub fn menu_choisi(app: AppHandle, id: String) {
    let _ = menu::cacher(&app);
    poster_plus_tard(&app, Evenement::Menu(id));
}

#[tauri::command]
pub fn menu_fermer(app: AppHandle) {
    let _ = menu::cacher(&app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn libelle_et_raccourci_separes() {
        let l = lignes(&[
            EntreeMenu::Action {
                id: "capturer_zone".into(),
                texte: "Capturer une zone\tCtrl + Maj + 1".into(),
            },
            EntreeMenu::Separateur,
            EntreeMenu::Action {
                id: "reglages".into(),
                texte: "Réglages".into(),
            },
        ]);
        assert_eq!(l[0].libelle, "Capturer une zone");
        assert_eq!(l[0].raccourci.as_deref(), Some("Ctrl + Maj + 1"));
        assert!(l[1].separateur && l[1].id.is_empty());
        assert_eq!(
            (l[2].libelle.as_str(), l[2].raccourci.as_deref()),
            ("Réglages", None)
        );
    }
}
