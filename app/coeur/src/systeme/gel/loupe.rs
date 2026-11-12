//! La loupe de la sélection de zone (0.4) : mesures, place et image, en fonctions pures.
//!
//! Aucun dessin de la loupe n'est validé : toutes ses mesures sont les constantes ci-dessous,
//! à reprendre d'un dessin sans toucher au reste. Les longueurs « logiques » sont multipliées
//! par l'échelle de l'écran de la pointe ; tout le reste est en pixels réels.
//!
//! `windows.rs` lit les `PIXELS_VUS × PIXELS_VUS` pixels de l'image figée autour de la
//! pointe, `composer` les agrandit et y peint l'anneau et la case du pixel visé, puis la
//! fenêtre de la loupe est posée à la place que donne `placer`.

use super::{EcranInfo, Rect};

/// Pixels de l'image vus de chaque côté du carré (impair : le pixel visé est au centre).
pub const PIXELS_VUS: i32 = 15;
/// Côté d'un pixel agrandi, en pixels logiques (la loupe fait `PIXELS_VUS × GROSSISSEMENT`,
/// 120 pixels logiques).
pub const GROSSISSEMENT: i32 = 8;
/// Écart entre la pointe et le coin de la loupe, en pixels logiques (en bas à droite, de
/// l'autre côté près d'un bord).
pub const DECALAGE: i32 = 24;
/// Épaisseur de l'anneau clair, en pixels logiques.
pub const ANNEAU_CLAIR: f64 = 2.0;
/// Épaisseur du trait sombre qui double l'anneau au bord, en pixels logiques.
pub const TRAIT_SOMBRE: f64 = 1.0;
/// Couleur de l'anneau clair (gris, donc la même en RGB et en BGR).
pub const COULEUR_ANNEAU: u8 = 255;
/// Couleur du trait sombre.
pub const COULEUR_TRAIT: u8 = 40;
/// Épaisseur de chacun des deux filets de la case du pixel visé (clair contre la case,
/// sombre autour), en pixels logiques. Ils sont posés dehors : la case garde sa couleur.
pub const CASE_FILET: f64 = 1.0;
/// Couleur du filet clair de la case.
pub const COULEUR_CASE_CLAIRE: u8 = 255;
/// Couleur du filet sombre de la case.
pub const COULEUR_CASE_SOMBRE: u8 = 0;

/// Une longueur logique en pixels réels, jamais moins d'un pixel.
fn reel(logique: f64, echelle: f64) -> i32 {
    ((logique * sure(echelle)).round() as i32).max(1)
}

/// L'échelle, ou 1 si elle est absurde (nulle, négative, infinie).
fn sure(echelle: f64) -> f64 {
    if echelle.is_finite() && echelle > 0.0 {
        echelle
    } else {
        1.0
    }
}

/// Côté d'un pixel agrandi, en pixels réels.
pub fn case(echelle: f64) -> i32 {
    reel(GROSSISSEMENT as f64, echelle)
}

/// Côté de la loupe, en pixels réels : `PIXELS_VUS × round(GROSSISSEMENT × échelle)`.
pub fn cote(echelle: f64) -> i32 {
    PIXELS_VUS * case(echelle)
}

/// Les pixels de l'image vus par la loupe : le carré centré sur la pointe.
pub fn source(pointe: (i32, i32)) -> Rect {
    let demi = PIXELS_VUS / 2;
    Rect {
        x: pointe.0.saturating_sub(demi),
        y: pointe.1.saturating_sub(demi),
        largeur: PIXELS_VUS,
        hauteur: PIXELS_VUS,
    }
}

/// L'écran de la pointe ; à défaut (pointe hors de tout écran), le principal, puis le premier.
pub fn ecran_de(pointe: (i32, i32), ecrans: &[EcranInfo]) -> Option<&EcranInfo> {
    ecrans
        .iter()
        .find(|e| e.rect.contient(pointe))
        .or_else(|| ecrans.iter().find(|e| e.principal))
        .or_else(|| ecrans.first())
}

/// Une coordonnée de la loupe sur un axe : après la pointe, ou avant si elle déborderait,
/// puis bornée à l'écran.
fn axe(pointe: i32, decalage: i32, cote: i32, debut: i32, fin: i64) -> i32 {
    let apres = pointe as i64 + decalage as i64;
    let v = if apres + cote as i64 > fin {
        pointe as i64 - decalage as i64 - cote as i64
    } else {
        apres
    };
    let max = (fin - cote as i64).max(debut as i64);
    v.clamp(debut as i64, max) as i32
}

/// La place de la loupe : son coin haut gauche à `DECALAGE` (logique) en bas à droite de
/// la pointe ; de l'autre côté quand elle sortirait de l'écran (à droite ou en bas), et
/// toujours dans l'écran de la pointe. Le côté est celui de `cote(écran.echelle)`.
pub fn placer(pointe: (i32, i32), ecran: &EcranInfo) -> Rect {
    let c = cote(ecran.echelle);
    let d = reel(DECALAGE as f64, ecran.echelle);
    let r = ecran.rect;
    Rect {
        x: axe(pointe.0, d, c, r.x, r.droite()),
        y: axe(pointe.1, d, c, r.y, r.bas()),
        largeur: c,
        hauteur: c,
    }
}

/// Distance du centre du pixel `(x, y)` au centre d'un carré de côté `cote`.
fn distance(x: i32, y: i32, cote: i32) -> f64 {
    let centre = cote as f64 / 2.0;
    let dx = x as f64 + 0.5 - centre;
    let dy = y as f64 + 0.5 - centre;
    (dx * dx + dy * dy).sqrt()
}

/// L'opacité de chaque pixel d'un disque inscrit dans un carré de côté `cote`, ligne par
/// ligne : 255 dedans, 0 dehors, le bord adouci sur un pixel.
pub fn couverture_ronde(cote: i32) -> Vec<u8> {
    let cote = cote.max(0);
    let rayon = cote as f64 / 2.0;
    let mut alpha = Vec::with_capacity((cote * cote) as usize);
    for y in 0..cote {
        for x in 0..cote {
            let c = (rayon - distance(x, y, cote) + 0.5).clamp(0.0, 1.0);
            alpha.push((c * 255.0).round() as u8);
        }
    }
    alpha
}

/// L'image de la loupe, prête pour une fenêtre en calque : 4 octets par pixel, `cote ×
/// cote`, couleur prémultipliée par l'opacité (`UpdateLayeredWindow`). `source` : les
/// `PIXELS_VUS × PIXELS_VUS` pixels vus, 4 octets chacun, dont seuls les trois premiers
/// comptent (l'ordre des couleurs est gardé tel quel). `None` si `source` n'a pas la taille.
/// Pratique pour les tests ; la loupe écrit, elle, dans son image par `composer_dans`.
pub fn composer(source: &[u8], echelle: f64) -> Option<Vec<u8>> {
    let c = cote(echelle);
    let couverture = couverture_ronde(c);
    let mut sortie = vec![0u8; (c * c * 4) as usize];
    composer_dans(source, echelle, &couverture, &mut sortie).then_some(sortie)
}

/// Comme `composer`, sans allocation : écrit tous les octets de `sortie` (`cote × cote ×
/// 4`), avec `couverture` = `couverture_ronde(cote(echelle))`, calculée une fois par côté
/// par l'appelant. Faux, sans rien écrire, si une taille ne va pas.
pub fn composer_dans(source: &[u8], echelle: f64, couverture: &[u8], sortie: &mut [u8]) -> bool {
    let vus = PIXELS_VUS as usize;
    let t = case(echelle);
    let c = PIXELS_VUS * t;
    let n = (c * c) as usize;
    if source.len() != vus * vus * 4 || couverture.len() != n || sortie.len() != n * 4 {
        return false;
    }
    // La case du pixel visé et ses deux filets, posés dehors.
    let filet = reel(CASE_FILET, echelle);
    let debut_case = (PIXELS_VUS / 2) * t;
    let fin_case = debut_case + t;
    let dans = |v: i32, marge: i32| v >= debut_case - marge && v < fin_case + marge;
    // L'anneau, depuis le bord.
    let rayon = c as f64 / 2.0;
    let sombre = TRAIT_SOMBRE * sure(echelle);
    let anneau = sombre + ANNEAU_CLAIR * sure(echelle);
    for y in 0..c {
        for x in 0..c {
            let i = (y * c + x) as usize;
            let o = i * 4;
            let a = couverture[i];
            if a == 0 {
                sortie[o..o + 4].fill(0);
                continue;
            }
            let d = rayon - distance(x, y, c);
            let gris = if d < sombre {
                Some(COULEUR_TRAIT)
            } else if d < anneau {
                Some(COULEUR_ANNEAU)
            } else if dans(x, 0) && dans(y, 0) {
                None
            } else if dans(x, filet) && dans(y, filet) {
                Some(COULEUR_CASE_CLAIRE)
            } else if dans(x, 2 * filet) && dans(y, 2 * filet) {
                Some(COULEUR_CASE_SOMBRE)
            } else {
                None
            };
            let rgb = match gris {
                Some(g) => [g, g, g],
                None => {
                    let j = (((y / t) as usize) * vus + (x / t) as usize) * 4;
                    [source[j], source[j + 1], source[j + 2]]
                }
            };
            for k in 0..3 {
                sortie[o + k] = ((rgb[k] as u32 * a as u32 + 127) / 255) as u8;
            }
            sortie[o + 3] = a;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ecran(x: i32, y: i32, largeur: i32, hauteur: i32, echelle: f64) -> EcranInfo {
        let rect = Rect {
            x,
            y,
            largeur,
            hauteur,
        };
        EcranInfo {
            rect,
            zone_utile: rect,
            echelle,
            principal: x == 0 && y == 0,
        }
    }

    #[test]
    fn mesures_selon_l_echelle() {
        assert_eq!(cote(1.0), 120);
        assert_eq!(cote(1.25), 150);
        assert_eq!(cote(1.5), 180);
        assert_eq!(cote(2.0), 240);
        // Échelle absurde : comme 100 %.
        assert_eq!(cote(0.0), 120);
        assert_eq!(cote(f64::NAN), 120);
    }

    #[test]
    fn source_centree() {
        let s = source((100, 50));
        assert_eq!((s.x, s.y, s.largeur, s.hauteur), (93, 43, 15, 15));
        assert!(s.contient((100, 50)));
        assert_eq!(s.x + PIXELS_VUS / 2, 100);
        let s = source((-1920, -5));
        assert_eq!((s.x, s.y), (-1927, -12));
    }

    #[test]
    fn placee_en_bas_a_droite_au_milieu() {
        for (echelle, d, c) in [
            (1.0, 24, 120),
            (1.25, 30, 150),
            (1.5, 36, 180),
            (2.0, 48, 240),
        ] {
            let e = ecran(0, 0, 3840, 2160, echelle);
            let p = placer((1000, 800), &e);
            assert_eq!(
                (p.x, p.y, p.largeur, p.hauteur),
                (1000 + d, 800 + d, c, c),
                "{echelle}"
            );
        }
    }

    #[test]
    fn aux_quatre_bords() {
        let e = ecran(0, 0, 1024, 768, 1.0);
        // Bord droit : à gauche de la pointe.
        let p = placer((1000, 300), &e);
        assert_eq!((p.x, p.y), (1000 - 24 - 120, 324));
        // Bord bas : au-dessus.
        let p = placer((300, 760), &e);
        assert_eq!((p.x, p.y), (324, 760 - 24 - 120));
        // Coin bas droit : les deux.
        let p = placer((1023, 767), &e);
        assert_eq!((p.x, p.y), (1023 - 144, 767 - 144));
        // Bords gauche et haut : en bas à droite, comme au milieu.
        let p = placer((0, 0), &e);
        assert_eq!((p.x, p.y), (24, 24));
        // Toujours dans l'écran.
        for pointe in [
            (0, 0),
            (1023, 0),
            (0, 767),
            (1023, 767),
            (512, 384),
            (880, 640),
        ] {
            let p = placer(pointe, &e);
            assert!(e.rect.englobe(p), "{pointe:?} -> {p:?}");
        }
        // Écran plus petit que la loupe : collée au coin haut gauche.
        let petit = ecran(0, 0, 100, 100, 1.0);
        let p = placer((50, 50), &petit);
        assert_eq!((p.x, p.y), (0, 0));
    }

    #[test]
    fn ecran_a_coordonnees_negatives() {
        let e = ecran(-1920, -1080, 1920, 1080, 1.5);
        let p = placer((-1910, -1070), &e);
        assert_eq!((p.x, p.y), (-1910 + 36, -1070 + 36));
        let p = placer((-1, -1), &e);
        assert_eq!((p.x, p.y), (-1 - 36 - 180, -1 - 36 - 180));
        assert!(e.rect.englobe(p));
    }

    #[test]
    fn ecran_de_la_pointe() {
        let ecrans = [
            ecran(-1280, 0, 1280, 1024, 1.0),
            ecran(0, 0, 1920, 1080, 1.5),
        ];
        assert_eq!(ecran_de((-5, 10), &ecrans).unwrap().rect.x, -1280);
        assert_eq!(ecran_de((5, 10), &ecrans).unwrap().rect.x, 0);
        // Hors de tout écran : le principal.
        assert_eq!(ecran_de((5, 5000), &ecrans).unwrap().rect.x, 0);
        assert!(ecran_de((0, 0), &[]).is_none());
    }

    #[test]
    fn couverture_ronde_symetrique() {
        for c in [120, 150, 180, 240] {
            let a = couverture_ronde(c);
            assert_eq!(a.len(), (c * c) as usize);
            let at = |x: i32, y: i32| a[(y * c + x) as usize];
            for (x, y) in [(0, 0), (c - 1, 0), (0, c - 1), (c - 1, c - 1)] {
                assert_eq!(at(x, y), 0, "coin {x},{y}");
            }
            assert_eq!(at(c / 2, c / 2), 255);
            assert!(at(c / 2, 0) > 200, "haut du disque");
            for y in 0..c {
                for x in 0..c {
                    assert_eq!(at(x, y), at(c - 1 - x, y));
                    assert_eq!(at(x, y), at(x, c - 1 - y));
                    assert_eq!(at(x, y), at(y, x));
                }
            }
            // Le bord est adouci : des valeurs entre 0 et 255.
            assert!(a.iter().any(|&v| v > 0 && v < 255));
        }
    }

    /// Une source de deux couleurs : à gauche du pixel visé (colonnes 0 à 7) rouge, à
    /// droite vert, comme une pointe sur la frontière de deux carrés.
    fn source_deux_carres() -> Vec<u8> {
        let mut s = Vec::new();
        for _y in 0..PIXELS_VUS {
            for x in 0..PIXELS_VUS {
                if x <= PIXELS_VUS / 2 {
                    s.extend_from_slice(&[220, 10, 10, 0]);
                } else {
                    s.extend_from_slice(&[10, 200, 10, 0]);
                }
            }
        }
        s
    }

    #[test]
    fn image_agrandie_avec_anneau_et_case() {
        for echelle in [1.0, 1.25, 1.5, 2.0] {
            let t = case(echelle);
            let c = cote(echelle);
            let img = composer(&source_deux_carres(), echelle).unwrap();
            assert_eq!(img.len(), (c * c * 4) as usize);
            let at = |x: i32, y: i32| {
                let i = ((y * c + x) * 4) as usize;
                [img[i], img[i + 1], img[i + 2], img[i + 3]]
            };
            // La case du pixel visé, au centre, entière et à sa couleur.
            let debut = 7 * t;
            for y in debut..debut + t {
                for x in debut..debut + t {
                    assert_eq!(at(x, y), [220, 10, 10, 255], "{echelle} case {x},{y}");
                }
            }
            // Ses filets : clair contre la case, sombre autour.
            let f = reel(CASE_FILET, echelle);
            assert_eq!(at(debut - 1, debut), [255, 255, 255, 255]);
            assert_eq!(at(debut - f - 1, debut), [0, 0, 0, 255]);
            assert_eq!(at(debut + t + f, debut), [0, 0, 0, 255]);
            // Le centre de la case voisine (à droite) : la couleur du carré voisin.
            assert_eq!(at(8 * t + t / 2, debut + t / 2), [10, 200, 10, 255]);
            // Loin de la case : la source agrandie.
            assert_eq!(at(3 * t, 7 * t), [220, 10, 10, 255]);
            assert_eq!(at(11 * t, 7 * t), [10, 200, 10, 255]);
            // Coins transparents (couleur prémultipliée : nulle).
            assert_eq!(at(0, 0), [0, 0, 0, 0]);
            // Le trait sombre au bord haut (presque opaque : le bord est adouci), puis
            // l'anneau clair juste dedans.
            let bord = at(c / 2, 0);
            assert!(bord[3] > 200, "{echelle} {bord:?}");
            let gris = (COULEUR_TRAIT as u32 * bord[3] as u32 + 127) / 255;
            assert_eq!(bord[..3], [gris as u8; 3]);
            let dedans = reel(TRAIT_SOMBRE, echelle) + 1;
            assert_eq!(
                at(c / 2, dedans),
                [COULEUR_ANNEAU, COULEUR_ANNEAU, COULEUR_ANNEAU, 255]
            );
        }
    }

    #[test]
    fn source_de_mauvaise_taille_refusee() {
        assert!(composer(&[0u8; 10], 1.0).is_none());
        assert!(composer(&[], 1.0).is_none());
        // Mauvaise couverture ou mauvaise sortie : rien n'est écrit.
        let source = source_deux_carres();
        let mut sortie = vec![7u8; 120 * 120 * 4];
        assert!(!composer_dans(
            &source,
            1.0,
            &couverture_ronde(150),
            &mut sortie
        ));
        assert!(sortie.iter().all(|&v| v == 7));
        let mut courte = vec![7u8; 10];
        assert!(!composer_dans(
            &source,
            1.0,
            &couverture_ronde(120),
            &mut courte
        ));
        // Sortie réutilisée (pleine d'anciens octets) : tous réécrits, comme une neuve.
        assert!(composer_dans(
            &source,
            1.0,
            &couverture_ronde(120),
            &mut sortie
        ));
        assert_eq!(Some(sortie), composer(&source, 1.0));
    }
}
