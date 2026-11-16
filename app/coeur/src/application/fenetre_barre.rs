//! La barre de capture côté Application (0.7.2) : l'ouvrir quand l'app est relancée
//! (`Commande::Demarrer`), et les commandes de la page `barre/barre.html`. La fenêtre
//! elle-même est dans `fenetres::barre`.
//!
//! Ses boutons sont tirés des entrées du menu de l'icône (`menu_actuel`), jamais d'une
//! autre règle : licence, système, codeur H.264 suivent donc le menu (GIF sans Vidéo
//! quand Media Foundation manque, rien de Pro sans licence).
//!
//! Contrat avec la page :
//! - `barre_pret` : la page écoute `barre:ouvrir` et le thème ; rend la charge de la
//!   première ouverture (ou `null`).
//! - `barre:ouvrir` { boutons: [id], retardateur_s: n | null, retardateurs: [n] } à
//!   chaque ouverture suivante ; `barre:vider` quand la barre est cachée.
//! - `barre_taille` { largeur, hauteur } : la carte mesurée (pixels logiques, menu volant
//!   ouvert compris) ; la fenêtre est reposée à cette taille.
//! - `barre_choisi` { id } : un bouton choisi ; la barre se cache, l'entrée du menu de même
//!   identifiant est traitée au tour suivant de la boucle (`Evenement::Menu`).
//! - `barre_retardateur` { secondes } : le retardateur des Réglages changé, par le même
//!   chemin que la page des Réglages ; rend la valeur en vigueur. La barre reste ouverte.
//! - `barre_fermer` : Échap ; la barre se cache.

use tauri::AppHandle;

use super::decompte::RETARDATEURS_PROPOSES;
use super::etats::{Etat, Mode};
use super::icone::EntreeMenu;
use super::page_reglages::{lignes_montrees, Changement};
use super::{
    etat_automate, fenetre_reglages, geste_defilante, geste_film, id_menu, menu_actuel,
    poster_plus_tard, reglages_lus, Evenement, MENU_REGLAGES, VIGNETTE_CONSTRUITE,
};
use crate::fenetres::barre::{self, Ouverture};
use crate::fenetres::{menu, theme};
use crate::systeme::gel;

/// Les modes que la barre peut montrer, dans son ordre : Zone, Plein écran, Fenêtre |
/// Capture défilante, Vidéo et GIF (un bouton et son menu).
pub const MODES_BARRE: [Mode; 6] = [
    Mode::Zone,
    Mode::PleinEcran,
    Mode::Fenetre,
    Mode::Defilante,
    Mode::Video,
    Mode::Gif,
];

/// Les boutons de la barre d'après les entrées du menu de l'icône : chaque mode de
/// `MODES_BARRE` présent au menu, puis Réglages. Fonction pure.
pub fn boutons(entrees: &[EntreeMenu]) -> Vec<String> {
    let ids: Vec<&str> = entrees
        .iter()
        .filter_map(|e| match e {
            EntreeMenu::Action { id, .. } => Some(id.as_str()),
            EntreeMenu::Separateur => None,
        })
        .collect();
    let mut b: Vec<String> = MODES_BARRE
        .iter()
        .map(|m| id_menu(*m))
        .filter(|id| ids.contains(&id.as_str()))
        .collect();
    if ids.contains(&MENU_REGLAGES) {
        b.push(MENU_REGLAGES.into());
    }
    b
}

/// La charge de la page : les boutons, et le retardateur s'il a un effet (`retardateur_s`
/// des réglages ; `None` quand Réglages › Capture ne le montre pas). Fonction pure.
pub fn ouverture(entrees: &[EntreeMenu], retardateur_s: Option<u32>) -> Ouverture {
    Ouverture {
        boutons: boutons(entrees),
        retardateur_s,
        retardateurs: RETARDATEURS_PROPOSES.to_vec(),
    }
}

/// Pourquoi la barre ne s'ouvre pas, d'après l'état (ligne du journal), `None` si elle
/// peut s'ouvrir : au repos ou vignette montrée seulement. Fonction pure.
pub fn refus_etat(defilante: bool, film: bool, etat: Option<Etat>) -> Option<&'static str> {
    if defilante {
        Some("barre non ouverte: defilante en cours")
    } else if film {
        Some("barre non ouverte: film en cours")
    } else if !matches!(etat, Some(Etat::Repos | Etat::Vignette)) {
        Some("barre non ouverte: capture en cours")
    } else {
        None
    }
}

/// Ouvre la barre (relance de l'app), sauf pendant une capture défilante, un film ou une
/// autre capture. Déjà ouverte, elle est reposée sur l'écran de la souris et remise à jour.
pub fn ouvrir(app: &AppHandle) {
    let refus = refus_etat(
        geste_defilante::fenetres_bloquees(app),
        geste_film::fenetres_bloquees(app),
        etat_automate(app),
    );
    if let Some(ligne) = refus {
        log::info!("{ligne}");
        return;
    }
    let Some(entrees) = menu_actuel(app) else {
        log::warn!("barre non ouverte: application pas prete");
        return;
    };
    // Le menu de l'icône et la barre ne sont jamais montrés ensemble.
    let _ = menu::cacher(app);
    let retardateur = lignes_montrees(VIGNETTE_CONSTRUITE, gel::disponible())
        .contains(&"retardateur")
        .then(|| reglages_lus(app).capture.retardateur_s);
    barre::ouvrir(app, ouverture(&entrees, retardateur));
}

#[tauri::command]
pub fn barre_pret(app: AppHandle) -> Option<Ouverture> {
    log::info!("barre page prete");
    theme::envoyer_a(&app, barre::ETIQUETTE_BARRE);
    barre::prendre_en_attente()
}

#[tauri::command]
pub fn barre_taille(app: AppHandle, largeur: f64, hauteur: f64) {
    barre::redimensionner(&app, largeur, hauteur);
}

#[tauri::command]
pub fn barre_choisi(app: AppHandle, id: String) {
    // Seul un bouton montré peut être choisi (jamais « Quitter », par exemple).
    if !barre::bouton_montre(&id) {
        log::warn!("barre: choix inconnu");
        return;
    }
    log::info!("barre choisi {id}");
    let _ = barre::cacher(&app);
    poster_plus_tard(&app, Evenement::Menu(id));
}

#[tauri::command]
pub fn barre_retardateur(app: AppHandle, secondes: u32) -> u32 {
    // Seulement barre ouverte, bouton Retardateur montré, et l'un des choix proposés.
    if !barre::ouverte(&app)
        || !barre::retardateur_montre()
        || !RETARDATEURS_PROPOSES.contains(&secondes)
    {
        log::warn!("barre: retardateur refuse");
    } else {
        log::info!("barre retardateur {secondes}");
        let r = fenetre_reglages::changer(&app, Changement::Retardateur(secondes));
        if r.refus.is_none() {
            // La page des Réglages, ouverte, suit.
            fenetre_reglages::envoyer_vue(&app);
        }
    }
    reglages_lus(&app).capture.retardateur_s
}

#[tauri::command]
pub fn barre_fermer(app: AppHandle) {
    let _ = barre::cacher(&app);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{actions_selon, entrees_menu, entrees_pendant_defilante};

    /// Les entrées du menu : licence, codeur H.264 (vidéo), système (défilante, GIF).
    fn menu(pro: bool, video: bool, vivant: bool) -> Vec<EntreeMenu> {
        let actions = actions_selon(pro, true, true, vivant, vivant && video, vivant);
        entrees_menu(&actions, |_| Some("Ctrl + Maj + 1".into()))
    }

    #[test]
    fn sans_licence_les_trois_captures_et_reglages() {
        assert_eq!(
            boutons(&menu(false, true, true)),
            [
                "capturer_zone",
                "capturer_plein_ecran",
                "capturer_fenetre",
                "reglages"
            ]
        );
    }

    #[test]
    fn avec_licence_defilante_video_et_gif() {
        assert_eq!(
            boutons(&menu(true, true, true)),
            [
                "capturer_zone",
                "capturer_plein_ecran",
                "capturer_fenetre",
                "capturer_defilante",
                "capturer_video",
                "capturer_gif",
                "reglages"
            ]
        );
    }

    #[test]
    fn sans_media_foundation_gif_sans_video() {
        let b = boutons(&menu(true, false, true));
        assert!(b.contains(&"capturer_gif".to_string()));
        assert!(!b.contains(&"capturer_video".to_string()));
        assert!(b.contains(&"capturer_defilante".to_string()));
    }

    #[test]
    fn sans_images_vivantes_rien_de_pro() {
        assert_eq!(boutons(&menu(true, true, false)).len(), 4);
    }

    #[test]
    fn jamais_ce_qui_n_est_pas_un_bouton() {
        // Dernière zone, Copier le texte, Historique, Quitter : au menu, pas dans la barre.
        let b = boutons(&menu(true, true, true));
        for absent in [
            "capturer_derniere_zone",
            "capturer_texte",
            "historique",
            "quitter",
        ] {
            assert!(!b.contains(&absent.to_string()), "{absent}");
        }
        // Le menu d'une défilante (Quitter seul) : aucun bouton.
        assert!(boutons(&entrees_pendant_defilante()).is_empty());
    }

    #[test]
    fn charge_avec_le_retardateur() {
        let o = ouverture(&menu(false, false, false), Some(5));
        assert_eq!(o.retardateur_s, Some(5));
        assert_eq!(o.retardateurs, [0, 3, 5, 10]);
        assert_eq!(
            ouverture(&menu(false, false, false), None).retardateur_s,
            None
        );
    }

    #[test]
    fn ouverte_au_repos_ou_vignette_montree_seulement() {
        assert_eq!(refus_etat(false, false, Some(Etat::Repos)), None);
        assert_eq!(refus_etat(false, false, Some(Etat::Vignette)), None);
        assert_eq!(
            refus_etat(true, false, Some(Etat::Repos)),
            Some("barre non ouverte: defilante en cours")
        );
        assert_eq!(
            refus_etat(false, true, Some(Etat::Repos)),
            Some("barre non ouverte: film en cours")
        );
        for e in [
            Etat::Gel,
            Etat::Selection,
            Etat::Decompte,
            Etat::Enregistrement,
        ] {
            assert_eq!(
                refus_etat(false, false, Some(e)),
                Some("barre non ouverte: capture en cours")
            );
        }
        assert!(refus_etat(false, false, None).is_some());
    }
}
