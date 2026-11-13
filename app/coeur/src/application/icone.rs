//! Icône de la zone de notification et son menu (§3.5).
//!
//! Le menu est une liste d'entrées assemblée par l'Application (`super::entrees_menu`) :
//! un module ou un lot futur ajoute une entrée à la liste, jamais un cas écrit ici.
//!
//! Sous Windows et macOS, le menu est la fenêtre web `fenetres::menu` (piste A « Liste ») :
//! un clic gauche ou droit sur l'icône appelle `au_clic` avec le rectangle de l'icône, et
//! le moulinet fait un quart de tour (`tourner`). Sous Linux, où Tauri ne rapporte pas les
//! clics sur l'icône, le menu natif reste.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Wry};

use crate::fenetres::Rect;
use crate::systeme::Theme;

/// Vrai là où le menu est la fenêtre web (les clics sur l'icône sont rapportés).
pub const MENU_WEB: bool = !cfg!(target_os = "linux");

/// L'image posée en ce moment (thème de la barre, taille), pour le quart de tour.
static IMAGE_ACTUELLE: Mutex<Option<(Theme, u32)>> = Mutex::new(None);
/// Vrai pendant un quart de tour (un second clic ne le relance pas).
static TOURNE: AtomicBool = AtomicBool::new(false);

/// Durée du quart de tour (jeton `--duree-envol`, 420 ms) et nombre d'images.
pub const DUREE_TOUR: Duration = Duration::from_millis(420);
pub const IMAGES_TOUR: u32 = 10;

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

/// Le rectangle d'un clic sur l'icône, en pixels réels.
fn rect_physique(r: &tauri::Rect) -> Rect {
    let p = r.position.to_physical::<i32>(1.0);
    let t = r.size.to_physical::<u32>(1.0);
    Rect {
        x: p.x,
        y: p.y,
        largeur: t.width.max(1),
        hauteur: t.height.max(1),
    }
}

/// Installe l'icône. `au_menu` reçoit l'identifiant de l'entrée choisie dans le menu natif
/// (Linux) ; `au_clic`, le rectangle de l'icône à chaque clic gauche ou droit relâché
/// (Windows, macOS : le menu web).
pub fn installer<F, C>(
    app: &AppHandle,
    theme: Theme,
    taille: u32,
    entrees: &[EntreeMenu],
    au_menu: F,
    au_clic: C,
) -> Result<(), String>
where
    F: Fn(&AppHandle, String) + Send + Sync + 'static,
    C: Fn(&AppHandle, Rect) + Send + Sync + 'static,
{
    let image = Image::from_bytes(image_png(theme, taille)).map_err(|e| format!("image: {e}"))?;
    let menu = if MENU_WEB {
        None
    } else {
        Some(construire_menu(app, entrees).map_err(|e| format!("menu: {e}"))?)
    };
    // Sous Linux sans zone de notification, la bibliothèque peut paniquer : on l'écrit au
    // journal au lieu de planter.
    let construction = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut b = TrayIconBuilder::with_id(ID_ICONE)
            .icon(image)
            .tooltip(INFO_BULLE)
            .show_menu_on_left_click(false)
            .on_menu_event(move |app, evenement| au_menu(app, evenement.id().0.clone()))
            .on_tray_icon_event(move |icone, evenement| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left | MouseButton::Right,
                    button_state: MouseButtonState::Up,
                    rect,
                    ..
                } = evenement
                {
                    au_clic(icone.app_handle(), rect_physique(&rect));
                }
            });
        if let Some(menu) = &menu {
            b = b.menu(menu);
        }
        b.build(app)
    }));
    match construction {
        Ok(Ok(_)) => {
            if let Ok(mut i) = IMAGE_ACTUELLE.lock() {
                *i = Some((theme, taille));
            }
            Ok(())
        }
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
    icone.set_icon(Some(image)).map_err(|e| e.to_string())?;
    if let Ok(mut i) = IMAGE_ACTUELLE.lock() {
        *i = Some((theme, taille));
    }
    Ok(())
}

/// Avancement du quart de tour à l'instant `t` (0 à 1) : la courbe « ressort » du design
/// (léger dépassement, puis retour). Fonction pure.
pub fn avancement_ressort(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0) - 1.0;
    let c1 = 1.70158;
    1.0 + (c1 + 1.0) * t * t * t + c1 * t * t
}

/// L'image `rgba` (carrée, `cote` px) tournée de `degres` autour de son centre, sens des
/// aiguilles d'une montre, interpolation bilinéaire (couleurs pondérées par l'opacité).
/// Fonction pure.
pub fn tourner_rgba(rgba: &[u8], cote: u32, degres: f64) -> Vec<u8> {
    let n = cote as i64;
    let mut sortie = vec![0u8; rgba.len()];
    let (s, c) = degres.to_radians().sin_cos();
    let centre = f64::from(cote) / 2.0;
    let pixel = |x: i64, y: i64| -> [f64; 4] {
        if x < 0 || y < 0 || x >= n || y >= n {
            return [0.0; 4];
        }
        let i = ((y * n + x) * 4) as usize;
        let a = f64::from(rgba[i + 3]) / 255.0;
        [
            f64::from(rgba[i]) * a,
            f64::from(rgba[i + 1]) * a,
            f64::from(rgba[i + 2]) * a,
            a,
        ]
    };
    for y in 0..n {
        for x in 0..n {
            // Rotation inverse : d'où vient ce pixel.
            let (dx, dy) = (x as f64 + 0.5 - centre, y as f64 + 0.5 - centre);
            let sx = c * dx + s * dy + centre - 0.5;
            let sy = -s * dx + c * dy + centre - 0.5;
            let (x0, y0) = (sx.floor() as i64, sy.floor() as i64);
            let (fx, fy) = (sx - sx.floor(), sy - sy.floor());
            let mut acc = [0.0f64; 4];
            for (px, py, w) in [
                (x0, y0, (1.0 - fx) * (1.0 - fy)),
                (x0 + 1, y0, fx * (1.0 - fy)),
                (x0, y0 + 1, (1.0 - fx) * fy),
                (x0 + 1, y0 + 1, fx * fy),
            ] {
                let p = pixel(px, py);
                for k in 0..4 {
                    acc[k] += p[k] * w;
                }
            }
            let i = ((y * n + x) * 4) as usize;
            let a = acc[3];
            if a > 0.0 {
                for k in 0..3 {
                    sortie[i + k] = (acc[k] / a).round().clamp(0.0, 255.0) as u8;
                }
                sortie[i + 3] = (a * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    sortie
}

/// Le moulinet de l'icône fait un quart de tour (ouverture du menu). Le moulinet est
/// symétrique d'un quart de tour : la dernière image est l'image d'origine. Un fil à part
/// pose les images ; rien si un tour est déjà en cours.
pub fn tourner(app: &AppHandle) {
    let Some((theme, taille)) = IMAGE_ACTUELLE.lock().ok().and_then(|i| *i) else {
        return;
    };
    if TOURNE.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    let lance = std::thread::Builder::new()
        .name("moulinet".into())
        .spawn(move || {
            let origine = image_png(theme, taille);
            if let (Ok(image), Some(icone)) = (Image::from_bytes(origine), app.tray_by_id(ID_ICONE))
            {
                let (cote, rgba) = (image.width(), image.rgba().to_vec());
                let pas = DUREE_TOUR / IMAGES_TOUR;
                for k in 1..IMAGES_TOUR {
                    let angle = 90.0 * avancement_ressort(f64::from(k) / f64::from(IMAGES_TOUR));
                    let tournee = Image::new_owned(tourner_rgba(&rgba, cote, angle), cote, cote);
                    let _ = icone.set_icon(Some(tournee));
                    std::thread::sleep(pas);
                }
                // L'image du moment (le thème a pu changer pendant le tour).
                let (t, n) = IMAGE_ACTUELLE
                    .lock()
                    .ok()
                    .and_then(|i| *i)
                    .unwrap_or((theme, taille));
                if let Ok(fin) = Image::from_bytes(image_png(t, n)) {
                    let _ = icone.set_icon(Some(fin));
                }
            }
            TOURNE.store(false, Ordering::SeqCst);
        });
    if lance.is_err() {
        TOURNE.store(false, Ordering::SeqCst);
    }
}

/// Refait le menu natif de l'icône (un raccourci a changé dans les Réglages). Sans objet
/// pour le menu web, qui relit ses entrées à chaque ouverture.
pub fn changer_menu(app: &AppHandle, entrees: &[EntreeMenu]) -> Result<(), String> {
    if MENU_WEB {
        return Ok(());
    }
    let Some(icone) = app.tray_by_id(ID_ICONE) else {
        return Err("icone absente".into());
    };
    let menu = construire_menu(app, entrees).map_err(|e| format!("menu: {e}"))?;
    icone.set_menu(Some(menu)).map_err(|e| e.to_string())
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
    fn ressort_part_de_zero_et_finit_a_un() {
        assert!(avancement_ressort(0.0).abs() < 1e-9);
        assert!((avancement_ressort(1.0) - 1.0).abs() < 1e-9);
        // Léger dépassement avant la fin.
        assert!(avancement_ressort(0.8) > 1.0);
    }

    #[test]
    fn quart_de_tour_d_un_moulinet() {
        // Un seul pixel opaque en haut à gauche : après un quart de tour, en haut à droite.
        let mut rgba = vec![0u8; 4 * 4 * 4];
        rgba[0..4].copy_from_slice(&[255, 0, 0, 255]);
        let t = tourner_rgba(&rgba, 4, 90.0);
        let i = 3 * 4;
        assert_eq!(&t[i..i + 4], &[255, 0, 0, 255]);
        assert_eq!(t[3], 0);
        // Zéro degré : l'image telle quelle.
        assert_eq!(tourner_rgba(&rgba, 4, 0.0), rgba);
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
