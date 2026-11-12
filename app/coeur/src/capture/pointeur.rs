//! Le pointeur posé dans les pixels découpés (0.4) : fonctions pures.
//!
//! Le pointeur est lu deux fois par `systeme::gel::lire_pointeur`, peint sur du noir puis
//! sur du blanc ; la comparaison des deux dit, pixel par pixel, ce qu'il fait à l'image :
//! rien, une couleur opaque, un mélange (ombre, bord adouci) ou une inversion (curseur
//! texte). Rien n'est écrit au journal ici : `ligne_lecture` et `ligne_pose` donnent les
//! lignes, l'Application les écrit (jamais la forme ni le nom du pointeur).

use std::time::Duration;

use crate::systeme::gel::{Pixels, Pointeur};
use crate::systeme::Rect;

/// Ce qu'un pixel du pointeur fait au pixel de l'image dessous.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Couche {
    /// Rien : le pixel de l'image reste tel quel.
    Transparent,
    /// Le pixel prend cette couleur.
    Opaque([u8; 3]),
    /// Cette couleur, mêlée à l'image avec cette opacité.
    Mele { rgb: [u8; 3], alpha: u8 },
    /// Le pixel de l'image est inversé (XOR).
    Inverse,
}

const NOIR: [u8; 3] = [0, 0, 0];
const BLANC: [u8; 3] = [255, 255, 255];

/// La couche d'un pixel, d'après sa couleur sur noir et sur blanc :
/// - égales → opaque ;
/// - noir sur noir et blanc sur blanc → rien ;
/// - plus clair (ou égal) sur blanc dans chaque canal → mêlé : opacité `255 - écart` (le
///   plus grand écart des trois canaux), couleur `noir × 255 / opacité` ; une opacité nulle
///   → rien. C'est le cas d'un gris semi-transparent (50 sur noir, 205 sur blanc), même si
///   `noir ^ blanc` vaut 255 ;
/// - un canal plus sombre sur blanc que sur noir, et chaque canal inversé de l'un à l'autre
///   (`noir ^ blanc == 255`, dont 255 sur noir et 0 sur blanc) → inverse (un curseur XOR
///   coloré est approché par l'inversion) ;
/// - tout autre cas (impossible pour un pointeur peint par Windows) → opaque, couleur sur
///   noir.
pub fn couche(noir: [u8; 3], blanc: [u8; 3]) -> Couche {
    if noir == blanc {
        return Couche::Opaque(noir);
    }
    if noir == NOIR && blanc == BLANC {
        return Couche::Transparent;
    }
    if (0..3).all(|c| blanc[c] >= noir[c]) {
        let ecart = (0..3).map(|c| blanc[c] - noir[c]).max().unwrap_or(0);
        let alpha = 255 - ecart;
        if alpha == 0 {
            return Couche::Transparent;
        }
        let mut rgb = [0u8; 3];
        for c in 0..3 {
            let v = (noir[c] as u32 * 255 + alpha as u32 / 2) / alpha as u32;
            rgb[c] = v.min(255) as u8;
        }
        return Couche::Mele { rgb, alpha };
    }
    if (0..3).all(|c| noir[c] ^ blanc[c] == 255) {
        return Couche::Inverse;
    }
    Couche::Opaque(noir)
}

/// Applique une couche à un pixel RGB de l'image (l'alpha de l'image n'est pas touché).
fn appliquer(c: Couche, image: &mut [u8]) {
    match c {
        Couche::Transparent => {}
        Couche::Opaque(rgb) => image[..3].copy_from_slice(&rgb),
        Couche::Mele { rgb, alpha } => {
            let a = alpha as u32;
            for k in 0..3 {
                let v = (rgb[k] as u32 * a + image[k] as u32 * (255 - a) + 127) / 255;
                image[k] = v as u8;
            }
        }
        Couche::Inverse => {
            for v in &mut image[..3] {
                *v = 255 - *v;
            }
        }
    }
}

/// Pose le pointeur dans `pixels`, qui sont ceux de `zone` (pixels réels du bureau, même
/// largeur et même hauteur) ; seule la partie dans la zone est posée, l'alpha des pixels est
/// gardé (fenêtre aux coins arrondis). Rend vrai si le pointeur recoupe la zone. Des tailles
/// incohérentes (pixels qui ne sont pas ceux de la zone, images du pointeur trop courtes)
/// ne touchent à rien et rendent faux.
pub fn poser_pointeur(pixels: &mut Pixels, zone: Rect, p: &Pointeur) -> bool {
    if zone.est_vide()
        || pixels.largeur != zone.largeur as u32
        || pixels.hauteur != zone.hauteur as u32
        || pixels.rgba.len() < pixels.largeur as usize * pixels.hauteur as usize * 4
    {
        return false;
    }
    let taille = p.largeur as usize * p.hauteur as usize * 4;
    if p.largeur == 0 || p.hauteur == 0 || p.sur_noir.len() < taille || p.sur_blanc.len() < taille {
        return false;
    }
    let rect = Rect {
        x: p.origine.0,
        y: p.origine.1,
        largeur: p.largeur.min(i32::MAX as u32) as i32,
        hauteur: p.hauteur.min(i32::MAX as u32) as i32,
    };
    let Some(commun) = rect.intersection(zone) else {
        return false;
    };
    let largeur_image = pixels.largeur as usize;
    for y in commun.y as i64..commun.bas() {
        for x in commun.x as i64..commun.droite() {
            let i = ((y - rect.y as i64) as usize * p.largeur as usize
                + (x - rect.x as i64) as usize)
                * 4;
            let noir = [p.sur_noir[i], p.sur_noir[i + 1], p.sur_noir[i + 2]];
            let blanc = [p.sur_blanc[i], p.sur_blanc[i + 1], p.sur_blanc[i + 2]];
            let j =
                ((y - zone.y as i64) as usize * largeur_image + (x - zone.x as i64) as usize) * 4;
            appliquer(couche(noir, blanc), &mut pixels.rgba[j..j + 4]);
        }
    }
    true
}

/// Ce que `capture::figer(true)` a lu du pointeur, pour le journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LecturePointeur {
    /// Durée de `lire_pointeur` seule.
    pub duree: Duration,
    /// L'erreur de la lecture (texte sans chemin ni titre), `None` si elle a réussi.
    pub erreur: Option<String>,
}

/// La ligne du journal après la lecture : `pointeur pris <l>x<h> en <ms> ms`,
/// `pointeur absent (cache)` ou `pointeur absent (<erreur>)`.
pub fn ligne_lecture(lecture: &LecturePointeur, pointeur: Option<&Pointeur>) -> String {
    match (&lecture.erreur, pointeur) {
        (Some(e), _) => format!("pointeur absent ({e})"),
        (None, Some(p)) => format!(
            "pointeur pris {}x{} en {} ms",
            p.largeur,
            p.hauteur,
            lecture.duree.as_millis()
        ),
        (None, None) => "pointeur absent (cache)".into(),
    }
}

/// La ligne du journal à la découpe : `pointeur pose` ou `pointeur hors zone`.
pub fn ligne_pose(pose: bool) -> &'static str {
    if pose {
        "pointeur pose"
    } else {
        "pointeur hors zone"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROUGE: [u8; 3] = [200, 30, 40];

    fn r(x: i32, y: i32, largeur: i32, hauteur: i32) -> Rect {
        Rect {
            x,
            y,
            largeur,
            hauteur,
        }
    }

    /// Une image unie de la couleur donnée, alpha 255.
    fn image(largeur: u32, hauteur: u32, rgb: [u8; 3]) -> Pixels {
        let mut rgba = Vec::new();
        for _ in 0..largeur * hauteur {
            rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
        Pixels {
            largeur,
            hauteur,
            rgba,
        }
    }

    fn px(p: &Pixels, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * p.largeur + x) * 4) as usize;
        [p.rgba[i], p.rgba[i + 1], p.rgba[i + 2], p.rgba[i + 3]]
    }

    /// Un pointeur dont chaque pixel est donné par (sur noir, sur blanc).
    fn pointeur(
        origine: (i32, i32),
        largeur: u32,
        f: impl Fn(u32, u32) -> ([u8; 3], [u8; 3]),
    ) -> Pointeur {
        let hauteur = largeur;
        let (mut sur_noir, mut sur_blanc) = (Vec::new(), Vec::new());
        for y in 0..hauteur {
            for x in 0..largeur {
                let (n, b) = f(x, y);
                sur_noir.extend_from_slice(&[n[0], n[1], n[2], 255]);
                sur_blanc.extend_from_slice(&[b[0], b[1], b[2], 255]);
            }
        }
        Pointeur {
            origine,
            largeur,
            hauteur,
            sur_noir,
            sur_blanc,
        }
    }

    #[test]
    fn couches() {
        assert_eq!(couche(NOIR, BLANC), Couche::Transparent);
        assert_eq!(couche(NOIR, NOIR), Couche::Opaque(NOIR));
        assert_eq!(couche(BLANC, BLANC), Couche::Opaque(BLANC));
        assert_eq!(couche(ROUGE, ROUGE), Couche::Opaque(ROUGE));
        assert_eq!(couche(BLANC, NOIR), Couche::Inverse);
        assert_eq!(couche([0, 255, 15], [255, 0, 240]), Couche::Inverse);
        // Ombre noire à demi-opacité : 0 sur noir, 127 sur blanc.
        assert_eq!(
            couche(NOIR, [127, 127, 127]),
            Couche::Mele {
                rgb: NOIR,
                alpha: 128
            }
        );
        // Bord adouci d'un pixel blanc à opacité 51 : 51 sur noir, 255 sur blanc.
        assert_eq!(
            couche([51, 51, 51], BLANC),
            Couche::Mele {
                rgb: BLANC,
                alpha: 51
            }
        );
        // Gris moyen semi-transparent (couleur 128, opacité 100) : 50 sur noir, 205 sur
        // blanc. `50 ^ 205 == 255`, mais c'est un mélange, pas une inversion.
        assert_eq!(
            couche([50, 50, 50], [205, 205, 205]),
            Couche::Mele {
                rgb: [128, 128, 128],
                alpha: 100
            }
        );
        // Bords adoucis d'une flèche ordinaire : contour noir à opacité 180, corps blanc à
        // opacité 120.
        assert_eq!(
            couche(NOIR, [75, 75, 75]),
            Couche::Mele {
                rgb: NOIR,
                alpha: 180
            }
        );
        assert_eq!(
            couche([120, 120, 120], BLANC),
            Couche::Mele {
                rgb: BLANC,
                alpha: 120
            }
        );
        // Cas impossible pour Windows (plus sombre sur blanc, sans inversion) : opaque.
        assert_eq!(
            couche([100, 0, 0], [50, 255, 255]),
            Couche::Opaque([100, 0, 0])
        );
    }

    /// Flèche réduite : un pixel noir, un pixel blanc, le reste transparent.
    #[test]
    fn fleche_opaque_et_transparent_intact() {
        let mut img = image(10, 10, ROUGE);
        let p = pointeur((2, 3), 3, |x, y| match (x, y) {
            (0, 0) => (NOIR, NOIR),
            (1, 0) => (BLANC, BLANC),
            _ => (NOIR, BLANC),
        });
        assert!(poser_pointeur(&mut img, r(0, 0, 10, 10), &p));
        assert_eq!(px(&img, 2, 3), [0, 0, 0, 255]);
        assert_eq!(px(&img, 3, 3), [255, 255, 255, 255]);
        let mut changes = 0;
        for y in 0..10 {
            for x in 0..10 {
                if px(&img, x, y) != [ROUGE[0], ROUGE[1], ROUGE[2], 255] {
                    changes += 1;
                }
            }
        }
        assert_eq!(changes, 2, "seuls les deux pixels opaques changent");
    }

    #[test]
    fn curseur_texte_inverse() {
        let mut img = image(4, 4, [10, 100, 250]);
        let p = pointeur((1, 1), 1, |_, _| (BLANC, NOIR));
        assert!(poser_pointeur(&mut img, r(0, 0, 4, 4), &p));
        assert_eq!(px(&img, 1, 1), [245, 155, 5, 255]);
        assert_eq!(px(&img, 0, 0), [10, 100, 250, 255]);
    }

    /// Le mélange redonne exactement le pointeur sur noir et sur blanc, et la moitié sur gris.
    #[test]
    fn ombre_a_demi_alpha() {
        let p = pointeur((0, 0), 1, |_, _| (NOIR, [127, 127, 127]));
        for (fond, attendu) in [(NOIR, 0u8), (BLANC, 127), ([200, 200, 200], 100)] {
            let mut img = image(1, 1, fond);
            assert!(poser_pointeur(&mut img, r(0, 0, 1, 1), &p));
            assert_eq!(px(&img, 0, 0), [attendu, attendu, attendu, 255], "{fond:?}");
        }
        let p = pointeur((0, 0), 1, |_, _| ([51, 51, 51], BLANC));
        for (fond, attendu) in [(NOIR, 51u8), (BLANC, 255)] {
            let mut img = image(1, 1, fond);
            assert!(poser_pointeur(&mut img, r(0, 0, 1, 1), &p));
            assert_eq!(px(&img, 0, 0), [attendu, attendu, attendu, 255], "{fond:?}");
        }
    }

    /// Pointeur à cheval sur le bord gauche et haut de la zone : seule la partie dedans.
    #[test]
    fn a_cheval_sur_le_bord() {
        let mut img = image(4, 4, ROUGE);
        let zone = r(100, 200, 4, 4);
        let p = pointeur((98, 198), 4, |_, _| (NOIR, NOIR));
        assert!(poser_pointeur(&mut img, zone, &p));
        for y in 0..4 {
            for x in 0..4 {
                let dedans = x < 2 && y < 2;
                let attendu = if dedans {
                    [0, 0, 0, 255]
                } else {
                    [200, 30, 40, 255]
                };
                assert_eq!(px(&img, x, y), attendu, "({x}, {y})");
            }
        }
    }

    #[test]
    fn hors_zone_rien_ne_change() {
        let mut img = image(4, 4, ROUGE);
        let avant = img.clone();
        let p = pointeur((4, 0), 4, |_, _| (NOIR, NOIR));
        assert!(!poser_pointeur(&mut img, r(0, 0, 4, 4), &p));
        assert_eq!(img, avant);
        let p = pointeur((-10, -10), 4, |_, _| (NOIR, NOIR));
        assert!(!poser_pointeur(&mut img, r(0, 0, 4, 4), &p));
        assert_eq!(img, avant);
    }

    /// Zone sur un écran à gauche du principal (coordonnées négatives).
    #[test]
    fn zone_au_bord_negatif() {
        let mut img = image(10, 10, ROUGE);
        let zone = r(-1920, -40, 10, 10);
        let p = pointeur((-1915, -38), 2, |_, _| (BLANC, BLANC));
        assert!(poser_pointeur(&mut img, zone, &p));
        assert_eq!(px(&img, 5, 2), [255, 255, 255, 255]);
        assert_eq!(px(&img, 6, 3), [255, 255, 255, 255]);
        assert_eq!(px(&img, 4, 2), [200, 30, 40, 255]);
    }

    /// Une fenêtre aux coins arrondis : l'alpha des pixels reste celui de l'image.
    #[test]
    fn alpha_garde() {
        let mut img = image(2, 1, ROUGE);
        img.rgba[3] = 0;
        img.rgba[7] = 90;
        let p = pointeur((0, 0), 2, |_, _| (NOIR, NOIR));
        assert!(poser_pointeur(&mut img, r(0, 0, 2, 1), &p));
        assert_eq!(px(&img, 0, 0), [0, 0, 0, 0]);
        assert_eq!(px(&img, 1, 0), [0, 0, 0, 90]);
    }

    #[test]
    fn tailles_incoherentes_refusees() {
        let mut img = image(4, 4, ROUGE);
        let avant = img.clone();
        let p = pointeur((0, 0), 2, |_, _| (NOIR, NOIR));
        assert!(!poser_pointeur(&mut img, r(0, 0, 5, 4), &p));
        let mut court = p.clone();
        court.sur_blanc.truncate(3);
        assert!(!poser_pointeur(&mut img, r(0, 0, 4, 4), &court));
        assert_eq!(img, avant);
    }

    #[test]
    fn lignes_du_journal() {
        let p = pointeur((0, 0), 32, |_, _| (NOIR, BLANC));
        let ok = LecturePointeur {
            duree: Duration::from_micros(1_700),
            erreur: None,
        };
        assert_eq!(ligne_lecture(&ok, Some(&p)), "pointeur pris 32x32 en 1 ms");
        assert_eq!(ligne_lecture(&ok, None), "pointeur absent (cache)");
        let ko = LecturePointeur {
            duree: Duration::ZERO,
            erreur: Some("GetIconInfo a echoue".into()),
        };
        assert_eq!(
            ligne_lecture(&ko, None),
            "pointeur absent (GetIconInfo a echoue)"
        );
        assert_eq!(ligne_pose(true), "pointeur pose");
        assert_eq!(ligne_pose(false), "pointeur hors zone");
    }
}
