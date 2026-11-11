//! Découpe : vérifier une zone et extraire ses pixels, en fonctions pures.

use super::geometrie::zone_valide;
use super::Erreur;
use crate::systeme::gel::Pixels;
use crate::systeme::Rect;

/// Vérifie une zone demandée et la coupe au bureau.
/// Trop petite (avant ou après la coupe) : `ZoneVide` ; sans rien de commun avec le
/// bureau : `ZoneHorsBureau`.
pub fn verifier_zone(zone: Rect, bureau: Rect) -> Result<Rect, Erreur> {
    if !zone_valide(zone) {
        return Err(Erreur::ZoneVide);
    }
    let coupee = zone.intersection(bureau).ok_or(Erreur::ZoneHorsBureau)?;
    if !zone_valide(coupee) {
        return Err(Erreur::ZoneVide);
    }
    Ok(coupee)
}

/// Extrait `zone` de `pixels`, qui couvrent `origine` (même repère). La zone est coupée à
/// `origine` ; des pixels dont la taille ne correspond pas sont une erreur, jamais une
/// panique.
pub fn extraire(pixels: &Pixels, origine: Rect, zone: Rect) -> Result<Pixels, Erreur> {
    let attendu = (pixels.largeur as usize)
        .checked_mul(pixels.hauteur as usize)
        .and_then(|n| n.checked_mul(4));
    if origine.largeur as i64 != pixels.largeur as i64
        || origine.hauteur as i64 != pixels.hauteur as i64
        || attendu != Some(pixels.rgba.len())
    {
        return Err(Erreur::Lecture(format!(
            "pixels {}x{} ({} octets) pour une origine {}x{}",
            pixels.largeur,
            pixels.hauteur,
            pixels.rgba.len(),
            origine.largeur,
            origine.hauteur
        )));
    }
    let coupee = zone.intersection(origine).ok_or(Erreur::ZoneHorsBureau)?;
    let largeur_source = pixels.largeur as usize;
    let (dx, dy) = (
        (coupee.x as i64 - origine.x as i64) as usize,
        (coupee.y as i64 - origine.y as i64) as usize,
    );
    let (l, h) = (coupee.largeur as usize, coupee.hauteur as usize);
    let mut rgba = Vec::with_capacity(l * h * 4);
    for ligne in dy..dy + h {
        let debut = (ligne * largeur_source + dx) * 4;
        rgba.extend_from_slice(&pixels.rgba[debut..debut + l * 4]);
    }
    Ok(Pixels {
        largeur: l as u32,
        hauteur: h as u32,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i32, y: i32, largeur: i32, hauteur: i32) -> Rect {
        Rect {
            x,
            y,
            largeur,
            hauteur,
        }
    }

    /// Image 4 × 3 dont chaque pixel dit sa place : (x, y, 7, 255).
    fn connue() -> Pixels {
        let mut rgba = Vec::new();
        for y in 0..3u8 {
            for x in 0..4u8 {
                rgba.extend_from_slice(&[x, y, 7, 255]);
            }
        }
        Pixels {
            largeur: 4,
            hauteur: 3,
            rgba,
        }
    }

    #[test]
    fn extrait_le_bon_morceau() {
        // L'image couvre (-2, -1)…(2, 2) : un bureau avec un écran à gauche.
        let origine = r(-2, -1, 4, 3);
        let p = extraire(&connue(), origine, r(-1, 0, 2, 2)).unwrap();
        assert_eq!((p.largeur, p.hauteur), (2, 2));
        assert_eq!(
            p.rgba,
            vec![1, 1, 7, 255, 2, 1, 7, 255, 1, 2, 7, 255, 2, 2, 7, 255]
        );
        // Tout.
        assert_eq!(extraire(&connue(), origine, origine).unwrap(), connue());
    }

    #[test]
    fn coupe_au_bord_et_refuse_dehors() {
        let origine = r(0, 0, 4, 3);
        let p = extraire(&connue(), origine, r(3, 2, 10, 10)).unwrap();
        assert_eq!((p.largeur, p.hauteur), (1, 1));
        assert_eq!(p.rgba, vec![3, 2, 7, 255]);
        assert_eq!(
            extraire(&connue(), origine, r(4, 0, 2, 2)),
            Err(Erreur::ZoneHorsBureau)
        );
    }

    #[test]
    fn pixels_incoherents_sans_panique() {
        let mut p = connue();
        p.rgba.pop();
        assert!(matches!(
            extraire(&p, r(0, 0, 4, 3), r(0, 0, 2, 2)),
            Err(Erreur::Lecture(_))
        ));
        assert!(matches!(
            extraire(&connue(), r(0, 0, 5, 3), r(0, 0, 2, 2)),
            Err(Erreur::Lecture(_))
        ));
    }

    #[test]
    fn verification_des_zones() {
        let bureau = r(-1920, 0, 3840, 1080);
        assert_eq!(
            verifier_zone(r(-10, 5, 20, 20), bureau),
            Ok(r(-10, 5, 20, 20))
        );
        assert_eq!(
            verifier_zone(r(1900, 1070, 100, 100), bureau),
            Ok(r(1900, 1070, 20, 10))
        );
        assert_eq!(verifier_zone(r(0, 0, 2, 50), bureau), Err(Erreur::ZoneVide));
        assert_eq!(
            verifier_zone(r(5000, 0, 50, 50), bureau),
            Err(Erreur::ZoneHorsBureau)
        );
        // À peine dedans : ce qui reste est trop petit.
        assert_eq!(
            verifier_zone(r(1918, 0, 50, 50), bureau),
            Err(Erreur::ZoneVide)
        );
    }
}
