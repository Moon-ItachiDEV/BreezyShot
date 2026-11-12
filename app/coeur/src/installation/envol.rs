//! Place de la fenêtre et trajectoire de l'envol vers la zone de notification (ARCHITECTURE
//! §12.3 et §12.4), en pixels réels.

use std::time::Duration;

use crate::systeme::Rect;

/// Carte de la fenêtre, en pixels logiques (le design : 720 × 480).
pub const CARTE: (f64, f64) = (720.0, 480.0);
/// Marge transparente autour de la carte, pour son ombre (jeton `installateur-marge`).
pub const MARGE: f64 = 40.0;
/// Durée de l'envol (jeton `duree-envol-fenetre`) et pas entre deux poses.
pub const DUREE_ENVOL: Duration = Duration::from_millis(460);
pub const PAS_ENVOL: Duration = Duration::from_millis(16);
/// Hauteur de l'arc au-dessus du départ, en pixels logiques (le design : 140).
pub const ARC: f64 = 140.0;

/// Fondu sans déplacement plutôt qu'envol : animations coupées par le système, sauf si un
/// essai force l'envol (`BREEZYSHOT_ESSAI_ANIMATIONS`).
pub fn envol_en_fondu(animations_reduites: bool, forcees_par_essai: bool) -> bool {
    animations_reduites && !forcees_par_essai
}

/// Rectangle de la fenêtre : carte et marge à l'échelle, bornées à la zone utile, centrées.
pub fn place_fenetre(zone_utile: Rect, echelle: f64) -> Rect {
    let e = if echelle.is_finite() && echelle > 0.0 {
        echelle
    } else {
        1.0
    };
    let largeur = (((CARTE.0 + 2.0 * MARGE) * e).round() as i32).min(zone_utile.largeur);
    let hauteur = (((CARTE.1 + 2.0 * MARGE) * e).round() as i32).min(zone_utile.hauteur);
    Rect {
        x: zone_utile.x + (zone_utile.largeur - largeur) / 2,
        y: zone_utile.y + (zone_utile.hauteur - hauteur) / 2,
        largeur,
        hauteur,
    }
}

/// D'où viser : le milieu du bord gauche de la zone de notification (là où Windows 11
/// range le menu « ^ » des icônes cachées), sinon le coin bas droit de la zone utile moins
/// 120 × 24 px logiques.
pub fn cible_envol(notification: Option<Rect>, zone_utile: Rect, echelle: f64) -> (i32, i32) {
    match notification.filter(|r| r.largeur > 0 && r.hauteur > 0) {
        Some(r) => (r.x + 12, r.y + r.hauteur / 2),
        None => (
            zone_utile.x + zone_utile.largeur - (120.0 * echelle).round() as i32,
            zone_utile.y + zone_utile.hauteur - (24.0 * echelle).round() as i32,
        ),
    }
}

/// Courbe `cubic-bezier(.5, 0, .2, 1)` (jeton `courbe-envol`) : avancement pour la part
/// de temps `t` (0 à 1).
pub fn courbe_envol(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    let f = |a: f64, b: f64, s: f64| {
        3.0 * a * s * (1.0 - s) * (1.0 - s) + 3.0 * b * s * s * (1.0 - s) + s * s * s
    };
    let (mut bas, mut haut, mut s) = (0.0, 1.0, t);
    for _ in 0..30 {
        s = (bas + haut) / 2.0;
        if f(0.5, 0.2, s) < t {
            bas = s;
        } else {
            haut = s;
        }
    }
    f(0.0, 1.0, s)
}

/// Coin haut gauche de la fenêtre à la part de temps `t` : son centre suit l'arc du
/// design (courbe de Bézier quadratique, point de contrôle à 45 % de l'écart horizontal et
/// `ARC` au-dessus) jusqu'à `cible`.
pub fn position_envol(depart: Rect, cible: (i32, i32), t: f64, echelle: f64) -> (i32, i32) {
    let centre = (
        depart.x as f64 + depart.largeur as f64 / 2.0,
        depart.y as f64 + depart.hauteur as f64 / 2.0,
    );
    let dx = cible.0 as f64 - centre.0;
    let dy = cible.1 as f64 - centre.1;
    let (cx, cy) = (dx * 0.45, dy.min(0.0) - ARC * echelle);
    let u = courbe_envol(t);
    let x = 2.0 * (1.0 - u) * u * cx + u * u * dx;
    let y = 2.0 * (1.0 - u) * u * cy + u * u * dy;
    (
        (depart.x as f64 + x).round() as i32,
        (depart.y as f64 + y).round() as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZONE: Rect = Rect {
        x: 0,
        y: 0,
        largeur: 1024,
        hauteur: 728,
    };

    #[test]
    fn place_centree_et_bornee() {
        let r = place_fenetre(ZONE, 1.0);
        assert_eq!((r.largeur, r.hauteur), (800, 560));
        assert_eq!((r.x, r.y), (112, 84));
        let petite = Rect {
            x: 1920,
            y: 0,
            largeur: 1366,
            hauteur: 720,
        };
        let r = place_fenetre(petite, 1.5);
        assert_eq!((r.largeur, r.hauteur), (1200, 720));
        assert_eq!((r.x, r.y), (1920 + 83, 0));
        assert_eq!(place_fenetre(ZONE, f64::NAN).largeur, 800);
    }

    #[test]
    fn cible() {
        let zone_notif = Rect {
            x: 900,
            y: 728,
            largeur: 100,
            hauteur: 40,
        };
        assert_eq!(cible_envol(Some(zone_notif), ZONE, 1.0), (912, 748));
        assert_eq!(cible_envol(None, ZONE, 1.0), (904, 704));
        assert_eq!(
            cible_envol(
                Some(Rect {
                    largeur: 0,
                    ..zone_notif
                }),
                ZONE,
                2.0
            ),
            (784, 680)
        );
    }

    #[test]
    fn fondu_ou_envol() {
        assert!(envol_en_fondu(true, false));
        assert!(!envol_en_fondu(true, true));
        assert!(!envol_en_fondu(false, false));
        assert!(!envol_en_fondu(false, true));
    }

    #[test]
    fn courbe() {
        assert!(courbe_envol(0.0).abs() < 1e-6);
        assert!((courbe_envol(1.0) - 1.0).abs() < 1e-6);
        let mut avant = 0.0;
        for i in 1..=20 {
            let u = courbe_envol(i as f64 / 20.0);
            assert!(u >= avant - 1e-9);
            avant = u;
        }
    }

    /// Valeur d'un jeton de `jetons.css`, en nombre (unité retirée).
    fn jeton(nom: &str) -> f64 {
        let css = include_str!("../../../interface/commun/jetons.css");
        let debut = format!("--{nom}:");
        let ligne = css
            .lines()
            .map(str::trim)
            .find(|l| l.starts_with(&debut))
            .unwrap_or_else(|| panic!("jeton {nom} absent"));
        let valeur = ligne[debut.len()..].trim().trim_end_matches(';');
        valeur
            .trim_end_matches("ms")
            .trim_end_matches("px")
            .parse()
            .unwrap_or_else(|_| panic!("jeton {nom}: {valeur}"))
    }

    #[test]
    fn constantes_egales_aux_jetons() {
        assert_eq!(DUREE_ENVOL.as_millis() as f64, jeton("duree-envol-fenetre"));
        assert_eq!(MARGE, jeton("installateur-marge"));
        assert_eq!(CARTE.0, jeton("installateur-carte-largeur"));
        assert_eq!(CARTE.1, jeton("installateur-carte-hauteur"));
    }

    #[test]
    fn trajectoire() {
        let depart = place_fenetre(ZONE, 1.0);
        let cible = (904, 704);
        assert_eq!(
            position_envol(depart, cible, 0.0, 1.0),
            (depart.x, depart.y)
        );
        let fin = position_envol(depart, cible, 1.0, 1.0);
        // À la fin, le centre de la fenêtre est sur la cible.
        assert_eq!((fin.0 + 400, fin.1 + 280), cible);
        // L'arc monte d'abord au-dessus du départ.
        let tot = position_envol(depart, cible, 0.25, 1.0);
        assert!(tot.1 < depart.y, "{tot:?}");
    }
}
