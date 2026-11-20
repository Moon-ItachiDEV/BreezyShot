//! Pipette de couleur : lecture d'un pixel ou d'un voisinage carré dans l'image figée et
//! mise en forme en HEX, RGB ou HSL. Fonctions pures (hormis `ImageFigee::lire`), sans OS.

use crate::systeme::Rect;

use super::{Erreur, Gel};

/// Demi-côté minimal, par défaut et maximal du voisinage de la loupe (`2 * r + 1` pixels).
pub const RAYON_MIN: u32 = 1;
pub const RAYON_DEFAUT: u32 = 7;
pub const RAYON_MAX: u32 = 15;

/// Format de couleur copié par la pipette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FormatCouleur {
    #[default]
    Hex,
    Rgb,
    Hsl,
}

impl FormatCouleur {
    pub fn depuis_nom(nom: &str) -> Option<FormatCouleur> {
        match nom {
            "hex" => Some(FormatCouleur::Hex),
            "rgb" => Some(FormatCouleur::Rgb),
            "hsl" => Some(FormatCouleur::Hsl),
            _ => None,
        }
    }

    pub fn nom(self) -> &'static str {
        match self {
            FormatCouleur::Hex => "hex",
            FormatCouleur::Rgb => "rgb",
            FormatCouleur::Hsl => "hsl",
        }
    }
}

/// Convertit `(r, g, b)` en `(teinte 0..=359, saturation 0..=100, luminosite 0..=100)`.
/// Fonction pure.
pub fn rgb_vers_hsl(r: u8, g: u8, b: u8) -> (u16, u8, u8) {
    let rf = f64::from(r) / 255.0;
    let gf = f64::from(g) / 255.0;
    let bf = f64::from(b) / 255.0;
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let delta = max - min;
    let l = (max + min) / 2.0;

    let (h, s) = if delta == 0.0 {
        (0.0, 0.0)
    } else {
        let s = delta / (1.0 - (2.0 * l - 1.0).abs());
        let mut h = if (max - rf).abs() < f64::EPSILON {
            ((gf - bf) / delta).rem_euclid(6.0)
        } else if (max - gf).abs() < f64::EPSILON {
            (bf - rf) / delta + 2.0
        } else {
            (rf - gf) / delta + 4.0
        };
        h *= 60.0;
        if h < 0.0 {
            h += 360.0;
        }
        (h, s)
    };

    let h_arrondi = (h.round() as i32).rem_euclid(360) as u16;
    let s_arrondi = (s * 100.0).round().clamp(0.0, 100.0) as u8;
    let l_arrondi = (l * 100.0).round().clamp(0.0, 100.0) as u8;
    (h_arrondi, s_arrondi, l_arrondi)
}

/// Met en forme la couleur `(r, g, b)` dans le format demandé. Fonction pure.
pub fn formater(r: u8, g: u8, b: u8, format: FormatCouleur) -> String {
    match format {
        FormatCouleur::Hex => format!("#{r:02X}{g:02X}{b:02X}"),
        FormatCouleur::Rgb => format!("rgb({r}, {g}, {b})"),
        FormatCouleur::Hsl => {
            let (h, s, l) = rgb_vers_hsl(r, g, b);
            format!("hsl({h}, {s}%, {l}%)")
        }
    }
}

/// Lit le pixel exact `(x, y)` (en pixels réels du bureau) dans `gel`. Si `(x, y)` touche
/// le bord exclusif du bureau, il est ramené au dernier pixel valide du bureau.
pub fn lire_pixel(gel: &Gel, x: i32, y: i32) -> Result<(u8, u8, u8), Erreur> {
    if gel.bureau.est_vide() {
        return Err(Erreur::ZoneHorsBureau);
    }
    let px = x.clamp(gel.bureau.x, (gel.bureau.droite() - 1) as i32);
    let py = y.clamp(gel.bureau.y, (gel.bureau.bas() - 1) as i32);
    let rect = Rect {
        x: px,
        y: py,
        largeur: 1,
        hauteur: 1,
    };
    let pixels = gel.image.lire(rect).map_err(Erreur::Lecture)?;
    if pixels.rgba.len() < 3 {
        return Err(Erreur::Lecture("pixel absent".into()));
    }
    Ok((pixels.rgba[0], pixels.rgba[1], pixels.rgba[2]))
}

/// Lit un voisinage carré `(2*r + 1) × (2*r + 1)` centré sur `(x, y)` en RGBA brut.
/// Les pixels hors du bureau virtuel valent `[0, 0, 0, 0]` : le pixel `(r, r)` est toujours
/// celui sous `(x, y)`.
pub fn lire_patch(gel: &Gel, x: i32, y: i32, rayon: u32) -> Result<Vec<u8>, Erreur> {
    let r = rayon.clamp(RAYON_MIN, RAYON_MAX) as i32;
    let cote = (2 * r + 1) as usize;
    let mut sortie = vec![0u8; cote * cote * 4];
    let demande = Rect {
        x: x.saturating_sub(r),
        y: y.saturating_sub(r),
        largeur: cote as i32,
        hauteur: cote as i32,
    };
    let Some(inter) = demande.intersection(gel.bureau) else {
        return Ok(sortie);
    };
    let pixels = gel.image.lire(inter).map_err(Erreur::Lecture)?;
    let dx = (inter.x - demande.x) as usize;
    let dy = (inter.y - demande.y) as usize;
    let l = inter.largeur as usize;
    let h = inter.hauteur as usize;
    for ligne in 0..h {
        let src_debut = ligne * l * 4;
        let dst_debut = ((dy + ligne) * cote + dx) * 4;
        if let (Some(src), Some(dst)) = (
            pixels.rgba.get(src_debut..src_debut + l * 4),
            sortie.get_mut(dst_debut..dst_debut + l * 4),
        ) {
            dst.copy_from_slice(src);
        }
    }
    Ok(sortie)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_depuis_nom() {
        assert_eq!(FormatCouleur::depuis_nom("hex"), Some(FormatCouleur::Hex));
        assert_eq!(FormatCouleur::depuis_nom("rgb"), Some(FormatCouleur::Rgb));
        assert_eq!(FormatCouleur::depuis_nom("hsl"), Some(FormatCouleur::Hsl));
        assert_eq!(FormatCouleur::depuis_nom("HEX"), None);
        assert_eq!(FormatCouleur::depuis_nom("cmyk"), None);
        assert_eq!(FormatCouleur::Hex.nom(), "hex");
        assert_eq!(FormatCouleur::Rgb.nom(), "rgb");
        assert_eq!(FormatCouleur::Hsl.nom(), "hsl");
    }

    #[test]
    fn formater_hex_rgb_hsl() {
        assert_eq!(formater(0, 0, 0, FormatCouleur::Hex), "#000000");
        assert_eq!(formater(0, 0, 0, FormatCouleur::Rgb), "rgb(0, 0, 0)");
        assert_eq!(formater(0, 0, 0, FormatCouleur::Hsl), "hsl(0, 0%, 0%)");

        assert_eq!(formater(255, 255, 255, FormatCouleur::Hex), "#FFFFFF");
        assert_eq!(
            formater(255, 255, 255, FormatCouleur::Hsl),
            "hsl(0, 0%, 100%)"
        );

        assert_eq!(formater(255, 0, 0, FormatCouleur::Hsl), "hsl(0, 100%, 50%)");
        assert_eq!(
            formater(0, 255, 0, FormatCouleur::Hsl),
            "hsl(120, 100%, 50%)"
        );
        assert_eq!(
            formater(0, 0, 255, FormatCouleur::Hsl),
            "hsl(240, 100%, 50%)"
        );
        assert_eq!(
            formater(0, 255, 255, FormatCouleur::Hsl),
            "hsl(180, 100%, 50%)"
        );
        assert_eq!(
            formater(255, 255, 0, FormatCouleur::Hsl),
            "hsl(60, 100%, 50%)"
        );
        assert_eq!(
            formater(255, 0, 255, FormatCouleur::Hsl),
            "hsl(300, 100%, 50%)"
        );

        assert_eq!(formater(255, 126, 107, FormatCouleur::Hex), "#FF7E6B");
        assert_eq!(
            formater(255, 126, 107, FormatCouleur::Rgb),
            "rgb(255, 126, 107)"
        );
        assert_eq!(
            formater(255, 126, 107, FormatCouleur::Hsl),
            "hsl(8, 100%, 71%)"
        );

        // Gris neutre : saturation 0, teinte 0.
        assert_eq!(rgb_vers_hsl(128, 128, 128), (0, 0, 50));
    }
}
