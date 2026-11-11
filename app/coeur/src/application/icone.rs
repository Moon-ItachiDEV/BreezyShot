//! Icône de la zone de notification et son menu (§3.5).
//!
//! Le menu est une liste d'entrées assemblée par l'Application (`super::entrees_menu`) :
//! un module ou un lot futur ajoute une entrée à la liste, jamais un cas écrit ici.

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Wry};

use crate::systeme::Theme;

pub const ID_ICONE: &str = "breezyshot";
pub const INFO_BULLE: &str = "BreezyShot";

/// Une entrée du menu de l'icône.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntreeMenu {
    /// Une action : son identifiant (reçu dans `Evenement::Menu`) et son texte visible.
    Action {
        id: String,
        texte: String,
    },
    Separateur,
}

/// Taille de l'icône calée au pixel d'après l'échelle de l'écran principal :
/// 100 % → 16, 125 à 150 % → 24, au-delà → 32.
pub fn taille_icone(echelle: f64) -> u32 {
    if !echelle.is_finite() || echelle < 1.25 {
        16
    } else if echelle <= 1.5 {
        24
    } else {
        32
    }
}

/// Image monochrome : blanche sur barre sombre, sombre sur barre claire.
/// Jamais la version couleur dans la zone de notification.
pub fn image_png(theme: Theme, taille: u32) -> &'static [u8] {
    match (theme, taille) {
        (Theme::Sombre, 16) => include_bytes!("../../icons/notif-blanc-16.png"),
        (Theme::Sombre, 24) => include_bytes!("../../icons/notif-blanc-24.png"),
        (Theme::Sombre, _) => include_bytes!("../../icons/notif-blanc-32.png"),
        (Theme::Clair, 16) => include_bytes!("../../icons/notif-sombre-16.png"),
        (Theme::Clair, 24) => include_bytes!("../../icons/notif-sombre-24.png"),
        (Theme::Clair, _) => include_bytes!("../../icons/notif-sombre-32.png"),
    }
}

/// Échelle de l'écran principal (1.0 si inconnue).
pub fn echelle_ecran_principal(app: &AppHandle) -> f64 {
    match app.primary_monitor() {
        Ok(Some(ecran)) => ecran.scale_factor(),
        _ => 1.0,
    }
}

fn construire_menu(app: &AppHandle, entrees: &[EntreeMenu]) -> tauri::Result<Menu<Wry>> {
    let menu = Menu::new(app)?;
    for entree in entrees {
        match entree {
            EntreeMenu::Action { id, texte } => {
                menu.append(&MenuItem::with_id(app, id, texte, true, None::<&str>)?)?
            }
            EntreeMenu::Separateur => menu.append(&PredefinedMenuItem::separator(app)?)?,
        }
    }
    Ok(menu)
}

/// Installe l'icône. `au_menu` reçoit l'identifiant de l'entrée choisie.
pub fn installer<F>(
    app: &AppHandle,
    theme: Theme,
    taille: u32,
    entrees: &[EntreeMenu],
    au_menu: F,
) -> Result<(), String>
where
    F: Fn(&AppHandle, String) + Send + Sync + 'static,
{
    let image = Image::from_bytes(image_png(theme, taille)).map_err(|e| format!("image: {e}"))?;
    let menu = construire_menu(app, entrees).map_err(|e| format!("menu: {e}"))?;
    // Sous Linux sans zone de notification, la bibliothèque peut paniquer : on l'écrit au
    // journal au lieu de planter.
    let construction = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        TrayIconBuilder::with_id(ID_ICONE)
            .icon(image)
            .tooltip(INFO_BULLE)
            .menu(&menu)
            .show_menu_on_left_click(false)
            .on_menu_event(move |app, evenement| au_menu(app, evenement.id().0.clone()))
            .build(app)
    }));
    match construction {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("la zone de notification a refuse l'icone".into()),
    }
}

/// L'icône est-elle réellement dans la zone de notification ? Vrai si le système donne
/// son rectangle, de taille non nulle. Sous Linux, Tauri ne le donne jamais : faux.
pub fn visible(app: &AppHandle) -> bool {
    let Some(icone) = app.tray_by_id(ID_ICONE) else {
        return false;
    };
    match icone.rect() {
        Ok(Some(rect)) => {
            let taille = rect.size.to_physical::<u32>(1.0);
            taille.width > 0 && taille.height > 0
        }
        _ => false,
    }
}

/// Change l'image de l'icône (thème de la barre changé).
pub fn changer_image(app: &AppHandle, theme: Theme, taille: u32) -> Result<(), String> {
    let Some(icone) = app.tray_by_id(ID_ICONE) else {
        return Err("icone absente".into());
    };
    let image = Image::from_bytes(image_png(theme, taille)).map_err(|e| format!("image: {e}"))?;
    icone.set_icon(Some(image)).map_err(|e| e.to_string())
}

/// Retire l'icône de la zone de notification. Rend vrai si elle y était.
pub fn retirer(app: &AppHandle) -> bool {
    app.remove_tray_by_id(ID_ICONE).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taille_selon_l_echelle() {
        assert_eq!(taille_icone(1.0), 16);
        assert_eq!(taille_icone(1.1), 16);
        assert_eq!(taille_icone(1.25), 24);
        assert_eq!(taille_icone(1.5), 24);
        assert_eq!(taille_icone(1.75), 32);
        assert_eq!(taille_icone(3.0), 32);
        assert_eq!(taille_icone(f64::NAN), 16);
    }

    #[test]
    fn images_lisibles_a_la_bonne_taille() {
        for theme in [Theme::Clair, Theme::Sombre] {
            for taille in [16, 24, 32] {
                let image = Image::from_bytes(image_png(theme, taille)).unwrap();
                assert_eq!((image.width(), image.height()), (taille, taille));
            }
        }
    }
}
