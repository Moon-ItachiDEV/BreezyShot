//! Fenêtres flottantes de captures épinglées à l'écran : sans cadre, transparentes,
//! toujours au premier plan, hors de la barre des tâches, déplaçables et redimensionnables.
//!
//! Chaque capture épinglée possède sa propre fenêtre Tauri (`epingle-{id}`). Ce module crée,
//! place, montre, redimensionne et détruit ; les décisions sont dans `application/epingle.rs`
//! et l'état dans `application/geste_epingle.rs`.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use super::place::ecran_sous;
use super::{ecrans, theme, Ecran, Rect, ARGUMENTS_NAVIGATEUR};
use crate::systeme;

/// Préfixe d'étiquette des fenêtres épinglées (aussi dans `capabilities/epingle.json`).
pub const PREFIXE_ETIQUETTE: &str = "epingle-";
/// Page de la fenêtre, sous `app/interface/`.
pub const PAGE: &str = "epingle/epingle.html";
/// Marge transparente (pixels logiques) autour de la carte pour laisser respirer l'ombre.
pub const MARGE_OMBRE: f64 = 16.0;
/// Décalage (pixels logiques) entre deux fenêtres épinglées ouvertes à la suite.
const DECALAGE_CASCADE: f64 = 28.0;
/// Écart minimal (pixels logiques) entre la carte et les bords de la zone utile.
const ECART_BORD: f64 = 12.0;

/// Étiquette Tauri de la fenêtre épinglée `id`.
pub fn etiquette(id: u64) -> String {
    format!("{PREFIXE_ETIQUETTE}{id}")
}

fn fenetre(app: &AppHandle, id: u64) -> Option<WebviewWindow> {
    app.get_webview_window(&etiquette(id))
}

/// Écran par défaut de secours si la lecture système ne rend aucun écran.
fn ecran_de_secours() -> Ecran {
    let rect = Rect {
        x: 0,
        y: 0,
        largeur: 1920,
        hauteur: 1080,
    };
    Ecran {
        rect,
        zone_utile: rect,
        echelle: 1.0,
    }
}

/// L'écran sous la fenêtre épinglée `id` (ou sous la souris si `id` est `None`).
pub fn ecran_courant(app: &AppHandle, id: Option<u64>) -> Ecran {
    let liste = ecrans(app);
    let point = id
        .and_then(|i| centre_fenetre(app, i))
        .map(|(x, y)| (f64::from(x), f64::from(y)))
        .or_else(|| super::souris(app));
    ecran_sous(point, &liste)
        .or_else(|| liste.into_iter().next())
        .unwrap_or_else(ecran_de_secours)
}

/// Centre physique actuel de la fenêtre épinglée `id`, si elle existe.
fn centre_fenetre(app: &AppHandle, id: u64) -> Option<(i32, i32)> {
    let f = fenetre(app, id)?;
    let pos = f.outer_position().ok()?;
    let taille = f.outer_size().ok()?;
    Some((
        pos.x.saturating_add((taille.width / 2) as i32),
        pos.y.saturating_add((taille.height / 2) as i32),
    ))
}

/// Rectangle physique de la fenêtre (carte + `MARGE_OMBRE` de chaque côté) centrée sur
/// `ecran.zone_utile` avec un léger décalage en cascade selon `rang`, borné à la zone utile.
pub fn placer_initial(ecran: &Ecran, largeur_log: u32, hauteur_log: u32, rang: usize) -> Rect {
    let k = if ecran.echelle.is_finite() && ecran.echelle > 0.0 {
        ecran.echelle
    } else {
        1.0
    };
    let z = ecran.zone_utile;
    let pas = ((rang % 6) as f64 - 2.0) * DECALAGE_CASCADE * k;
    let cx = i64::from(z.x) + i64::from(z.largeur) / 2 + pas.round() as i64;
    let cy = i64::from(z.y) + i64::from(z.hauteur) / 2 + pas.round() as i64;
    placer_autour_centre(cx as i32, cy as i32, ecran, largeur_log, hauteur_log)
}

/// Rectangle physique de la fenêtre (carte + `MARGE_OMBRE`) centrée sur `(centre_x, centre_y)`
/// et ramenée dans `ecran.zone_utile`.
pub fn placer_autour_centre(
    centre_x: i32,
    centre_y: i32,
    ecran: &Ecran,
    largeur_log: u32,
    hauteur_log: u32,
) -> Rect {
    let k = if ecran.echelle.is_finite() && ecran.echelle > 0.0 {
        ecran.echelle
    } else {
        1.0
    };
    let marge = (MARGE_OMBRE * k).round() as i64;
    let ecart = (ECART_BORD * k).round() as i64;
    let lc = (f64::from(largeur_log.max(1)) * k).round() as i64;
    let hc = (f64::from(hauteur_log.max(1)) * k).round() as i64;
    let z = ecran.zone_utile;
    let zg = i64::from(z.x);
    let zh = i64::from(z.y);
    let zd = zg + i64::from(z.largeur);
    let zb = zh + i64::from(z.hauteur);

    let gauche = (i64::from(centre_x) - lc / 2)
        .min(zd - ecart - lc)
        .max(zg + ecart);
    let haut = (i64::from(centre_y) - hc / 2)
        .min(zb - ecart - hc)
        .max(zh + ecart);

    Rect {
        x: (gauche - marge) as i32,
        y: (haut - marge) as i32,
        largeur: (lc + 2 * marge).max(1) as u32,
        hauteur: (hc + 2 * marge).max(1) as u32,
    }
}

/// Crée une nouvelle fenêtre épinglée cachée, posée sur `ecran`, prête à charger `PAGE`.
/// Rend son identifiant natif (`HWND` sous Windows) s'il est disponible.
pub fn creer(
    app: &AppHandle,
    id: u64,
    ecran: &Ecran,
    largeur_log: u32,
    hauteur_log: u32,
    rang: usize,
    sur_destruction: impl Fn(u64) + Send + Sync + 'static,
) -> Result<Option<isize>, String> {
    let nom = etiquette(id);
    let place = placer_initial(ecran, largeur_log, hauteur_log, rang);
    let f = WebviewWindowBuilder::new(app, &nom, WebviewUrl::App(PAGE.into()))
        .title("BreezyShot — Épinglé")
        .inner_size(
            f64::from(largeur_log) + 2.0 * MARGE_OMBRE,
            f64::from(hauteur_log) + 2.0 * MARGE_OMBRE,
        )
        .visible(false)
        .focused(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .theme(theme::pour_cadre(theme::actuel()))
        .additional_browser_args(ARGUMENTS_NAVIGATEUR)
        .build()
        .map_err(|e| format!("creation epingle {id}: {e}"))?;
    if let Err(e) = systeme::preparer_fenetre_outil_active(&f) {
        log::warn!("epingle {id}: style outil non pose: {e}");
    }
    let _ = f.set_position(tauri::PhysicalPosition::new(place.x, place.y));
    let _ = f.set_size(tauri::PhysicalSize::new(place.largeur, place.hauteur));
    f.on_window_event(move |ev| {
        if matches!(ev, tauri::WindowEvent::Destroyed) {
            sur_destruction(id);
        }
    });
    let h = systeme::identifiant_fenetre(&f);
    match h {
        Some(hwnd) => log::info!("epingle fenetre {id} {hwnd}"),
        None => log::info!("epingle fenetre {id} creee"),
    }
    Ok(h)
}

/// Montre la fenêtre épinglée `id` au premier plan (appelé une fois la page prête).
pub fn montrer(app: &AppHandle, id: u64) {
    let Some(f) = fenetre(app, id) else {
        return;
    };
    let _ = f.show();
    if !matches!(systeme::prendre_premier_plan(&f), Ok(true)) {
        let _ = f.set_focus();
    }
}

/// Redimensionne la fenêtre épinglée `id` à `(largeur_log, hauteur_log)` (pixels logiques
/// de la carte, hors `MARGE_OMBRE`) en gardant son centre immobile et dans son écran.
pub fn redimensionner(app: &AppHandle, id: u64, largeur_log: u32, hauteur_log: u32) -> bool {
    let Some(f) = fenetre(app, id) else {
        return false;
    };
    let ecran = ecran_courant(app, Some(id));
    let (cx, cy) = centre_fenetre(app, id).unwrap_or((
        ecran.zone_utile.x + (ecran.zone_utile.largeur / 2) as i32,
        ecran.zone_utile.y + (ecran.zone_utile.hauteur / 2) as i32,
    ));
    let place = placer_autour_centre(cx, cy, &ecran, largeur_log, hauteur_log);
    let _ = f.set_position(tauri::PhysicalPosition::new(place.x, place.y));
    let _ = f.set_size(tauri::PhysicalSize::new(place.largeur, place.hauteur));
    true
}

/// Cache puis détruit la fenêtre épinglée `id`.
pub fn detruire(app: &AppHandle, id: u64) {
    let Some(f) = fenetre(app, id) else {
        return;
    };
    let _ = systeme::cacher_sans_tao(&f);
    if let Err(e) = f.destroy() {
        log::warn!("epingle {id}: destruction impossible: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ecran(l: u32, h: u32, echelle: f64) -> Ecran {
        let rect = Rect {
            x: 0,
            y: 0,
            largeur: l,
            hauteur: h,
        };
        Ecran {
            rect,
            zone_utile: rect,
            echelle,
        }
    }

    #[test]
    fn etiquette_formatee() {
        assert_eq!(etiquette(1), "epingle-1");
        assert_eq!(etiquette(42), "epingle-42");
    }

    #[test]
    fn placement_autour_du_centre_reste_dans_l_ecran() {
        let e = ecran(1920, 1080, 1.0);
        let r = placer_autour_centre(960, 540, &e, 400, 300);
        assert_eq!(r.largeur, 400 + 2 * MARGE_OMBRE as u32);
        assert_eq!(r.hauteur, 300 + 2 * MARGE_OMBRE as u32);
        // Coin très à gauche : la carte ne sort pas de la zone utile.
        let bord = placer_autour_centre(0, 0, &e, 400, 300);
        assert_eq!(bord.x + MARGE_OMBRE as i32, ECART_BORD as i32);
        assert_eq!(bord.y + MARGE_OMBRE as i32, ECART_BORD as i32);
    }

    #[test]
    fn cascade_decale_les_epingles_successives() {
        let e = ecran(1920, 1080, 1.0);
        let a = placer_initial(&e, 400, 300, 0);
        let b = placer_initial(&e, 400, 300, 1);
        assert_eq!(b.x - a.x, DECALAGE_CASCADE as i32);
        assert_eq!(b.y - a.y, DECALAGE_CASCADE as i32);
    }
}
