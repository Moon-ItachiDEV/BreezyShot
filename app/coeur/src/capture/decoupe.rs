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

/// Finesse de l'adoucissement : chaque pixel du bord est mesuré en
/// `ECHANTILLONS × ECHANTILLONS` points.
const ECHANTILLONS: u32 = 16;

/// Part du pixel `(i, j)` (compté depuis le coin haut gauche) qui tombe dans le quart de
/// cercle de rayon `rayon` centré en `(rayon, rayon)`, de 0 à 255.
fn couverture(i: u32, j: u32, rayon: f64) -> u8 {
    let distance2 = |x: f64, y: f64| (rayon - x).powi(2) + (rayon - y).powi(2);
    let (x0, y0) = (i as f64, j as f64);
    let r2 = rayon * rayon;
    // Coin du pixel le plus loin du centre dedans : couvert entier.
    if distance2(x0, y0) <= r2 {
        return 255;
    }
    // Coin le plus proche dehors : rien.
    if distance2(x0 + 1.0, y0 + 1.0) >= r2 {
        return 0;
    }
    let mut dedans = 0u32;
    for a in 0..ECHANTILLONS {
        for b in 0..ECHANTILLONS {
            let x = x0 + (a as f64 + 0.5) / ECHANTILLONS as f64;
            let y = y0 + (b as f64 + 0.5) / ECHANTILLONS as f64;
            if distance2(x, y) < r2 {
                dedans += 1;
            }
        }
    }
    let total = ECHANTILLONS * ECHANTILLONS;
    ((dedans * 255 + total / 2) / total) as u8
}

/// Arrondit les coins demandés (haut gauche, haut droit, bas gauche, bas droit) : hors du
/// quart de cercle de rayon `rayon`, l'alpha tombe à 0 ; sur le bord, il vaut la part du
/// pixel couverte (bord adouci), multipliée par l'alpha déjà là. Les couleurs ne changent
/// pas. Le pixel même du coin n'est à 0 qu'à partir d'un rayon de 4 : en dessous, le
/// cercle en couvre vraiment une part (π/4 pour un rayon de 1), qui est gardée. Rayon nul
/// ou négatif : rien. Des pixels incohérents ou plus petits que le rayon : jamais de
/// panique.
pub fn arrondir(pixels: &mut Pixels, rayon: i32, coins: [bool; 4]) {
    let (largeur, hauteur) = (pixels.largeur as usize, pixels.hauteur as usize);
    let attendu = largeur.checked_mul(hauteur).and_then(|n| n.checked_mul(4));
    if rayon <= 0 || attendu != Some(pixels.rgba.len()) {
        return;
    }
    // Le carré du coin, borné à l'image.
    let cote = rayon as u32;
    let (n_x, n_y) = (
        (cote as usize).min(largeur) as u32,
        (cote as usize).min(hauteur) as u32,
    );
    let mut masque = Vec::with_capacity(n_x as usize * n_y as usize);
    for j in 0..n_y {
        for i in 0..n_x {
            masque.push(couverture(i, j, rayon as f64));
        }
    }
    for (numero, &demande) in coins.iter().enumerate() {
        if !demande {
            continue;
        }
        let (a_droite, en_bas) = (numero % 2 == 1, numero >= 2);
        for j in 0..n_y as usize {
            for i in 0..n_x as usize {
                let c = masque[j * n_x as usize + i] as u32;
                if c == 255 {
                    continue;
                }
                let x = if a_droite { largeur - 1 - i } else { i };
                let y = if en_bas { hauteur - 1 - j } else { j };
                let alpha = &mut pixels.rgba[(y * largeur + x) * 4 + 3];
                *alpha = ((*alpha as u32 * c + 127) / 255) as u8;
            }
        }
    }
}

/// Taille que l'aperçu de l'historique doit couvrir (une case 16:10).
const APERCU_HISTORIQUE: (u32, u32) = (400, 250);

/// L'aperçu d'une case de l'historique (contrat 0.5, §3.1) : la partie 16:10 du haut à
/// gauche (ce que montre la case), réduite pour couvrir 400 × 250, jamais agrandie. Pur.
/// Une image vide ou incohérente est rendue telle quelle.
pub fn apercu_historique(pixels: &Pixels) -> Pixels {
    let (l, h) = (pixels.largeur, pixels.hauteur);
    let coherent = pixels.rgba.len() as u64 == u64::from(l) * u64::from(h) * 4;
    if !coherent || l == 0 || h == 0 {
        return pixels.clone();
    }
    // La plus grande partie 16:10 calée en haut à gauche (arrondie au pixel).
    let (pl, ph) = if u64::from(l) * 10 >= u64::from(h) * 16 {
        let pl = (u64::from(h) * 16 + 5) / 10;
        (pl.clamp(1, u64::from(l)) as u32, h)
    } else {
        let ph = (u64::from(l) * 10 + 8) / 16;
        (l, ph.clamp(1, u64::from(h)) as u32)
    };
    let partie = if (pl, ph) == (l, h) {
        pixels.clone()
    } else {
        let rangee = pl as usize * 4;
        let mut rgba = Vec::with_capacity(rangee * ph as usize);
        for ligne in pixels.rgba.chunks_exact(l as usize * 4).take(ph as usize) {
            rgba.extend_from_slice(&ligne[..rangee]);
        }
        Pixels {
            largeur: pl,
            hauteur: ph,
            rgba,
        }
    };
    // Couvrir 400 × 250 : le plus petit facteur qui garde les deux côtés au moins à la
    // cible. Déjà plus petite d'un côté : jamais agrandie, rendue telle quelle.
    let (cl, ch) = APERCU_HISTORIQUE;
    if pl <= cl || ph <= ch {
        return partie;
    }
    let k = (f64::from(cl) / f64::from(pl)).max(f64::from(ch) / f64::from(ph));
    let nl = ((f64::from(pl) * k).round() as u32).clamp(cl, pl);
    let nh = ((f64::from(ph) * k).round() as u32).clamp(ch, ph);
    reduire_a(&partie, nl, nh)
}

/// Réduit l'image pour qu'aucun côté ne dépasse `cote_max` (aperçu de la vignette) :
/// chaque pixel d'arrivée est la moyenne des pixels de départ qu'il couvre (boîtes),
/// couleurs pondérées par l'alpha, alpha gardé. Une image déjà assez petite, vide ou
/// incohérente est rendue telle quelle.
pub fn reduire(pixels: &Pixels, cote_max: u32) -> Pixels {
    let (l, h) = (pixels.largeur, pixels.hauteur);
    let coherent = pixels.rgba.len() as u64 == u64::from(l) * u64::from(h) * 4;
    if cote_max == 0 || l.max(h) <= cote_max || !coherent || l == 0 || h == 0 {
        return pixels.clone();
    }
    let k = f64::from(cote_max) / f64::from(l.max(h));
    let nl = ((f64::from(l) * k).round() as u32).clamp(1, cote_max);
    let nh = ((f64::from(h) * k).round() as u32).clamp(1, cote_max);
    reduire_a(pixels, nl, nh)
}

/// Réduit des pixels cohérents à `nl × nh` (au plus leur taille, au moins 1), par la
/// moyenne des boîtes de `reduire`.
fn reduire_a(pixels: &Pixels, nl: u32, nh: u32) -> Pixels {
    let (l, h) = (pixels.largeur, pixels.hauteur);
    let (nl, nh) = (nl.clamp(1, l.max(1)), nh.clamp(1, h.max(1)));
    let mut rgba = vec![0u8; nl as usize * nh as usize * 4];
    for y in 0..nh {
        let y0 = (u64::from(y) * u64::from(h) / u64::from(nh)) as u32;
        let y1 = ((u64::from(y + 1) * u64::from(h)).div_ceil(u64::from(nh)) as u32).max(y0 + 1);
        for x in 0..nl {
            let x0 = (u64::from(x) * u64::from(l) / u64::from(nl)) as u32;
            let x1 = ((u64::from(x + 1) * u64::from(l)).div_ceil(u64::from(nl)) as u32).max(x0 + 1);
            let mut somme = [0u64; 4];
            let mut n = 0u64;
            for sy in y0..y1.min(h) {
                let ligne = sy as usize * l as usize;
                for sx in x0..x1.min(l) {
                    let i = (ligne + sx as usize) * 4;
                    let a = u64::from(pixels.rgba[i + 3]);
                    for (s, v) in somme.iter_mut().zip(&pixels.rgba[i..i + 3]) {
                        *s += u64::from(*v) * a;
                    }
                    somme[3] += a;
                    n += 1;
                }
            }
            let o = (y as usize * nl as usize + x as usize) * 4;
            let poids = somme[3];
            for (d, s) in rgba[o..o + 3].iter_mut().zip(&somme[..3]) {
                *d = (s + poids / 2).checked_div(poids).unwrap_or(0) as u8;
            }
            rgba[o + 3] = ((somme[3] + n / 2) / n.max(1)) as u8;
        }
    }
    Pixels {
        largeur: nl,
        hauteur: nh,
        rgba,
    }
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
    /// Image unie dont chaque pixel dit sa place dans ses couleurs, alpha donné.
    fn unie(largeur: u32, hauteur: u32, alpha: u8) -> Pixels {
        let mut rgba = Vec::new();
        for y in 0..hauteur {
            for x in 0..largeur {
                rgba.extend_from_slice(&[x as u8, y as u8, 7, alpha]);
            }
        }
        Pixels {
            largeur,
            hauteur,
            rgba,
        }
    }

    fn alpha(p: &Pixels, x: u32, y: u32) -> u8 {
        p.rgba[((y * p.largeur + x) * 4 + 3) as usize]
    }

    #[test]
    fn arrondir_les_quatre_coins() {
        let (l, h, rayon) = (40u32, 30u32, 8);
        let avant = unie(l, h, 255);
        let mut p = avant.clone();
        arrondir(&mut p, rayon, [true; 4]);
        // Le pixel de chaque coin : transparent.
        for (x, y) in [(0, 0), (l - 1, 0), (0, h - 1), (l - 1, h - 1)] {
            assert_eq!(alpha(&p, x, y), 0, "coin ({x}, {y})");
        }
        // À `rayon` du coin, sur le bord : opaque.
        let r = rayon as u32;
        for (x, y) in [(r, 0), (0, r), (l - 1 - r, 0), (l - 1, h - 1 - r)] {
            assert_eq!(alpha(&p, x, y), 255, "bord ({x}, {y})");
        }
        // Centre intact.
        assert_eq!(alpha(&p, l / 2, h / 2), 255);
        // Les couleurs ne changent pas, nulle part.
        for (a, b) in p.rgba.chunks_exact(4).zip(avant.rgba.chunks_exact(4)) {
            assert_eq!(a[..3], b[..3]);
        }
        // Bord adouci : des alphas entre 0 et 255 le long de la diagonale du coin.
        let intermediaires = (0..r)
            .map(|i| alpha(&p, i, i))
            .filter(|a| *a > 0 && *a < 255)
            .count();
        assert!(intermediaires >= 1, "aucun pixel adouci");
        // Hors du carré des coins : rien ne change.
        for y in 0..h {
            for x in 0..l {
                let dans_un_coin = (x < r || x >= l - r) && (y < r || y >= h - r);
                if !dans_un_coin {
                    assert_eq!(alpha(&p, x, y), 255, "({x}, {y})");
                }
            }
        }
    }

    #[test]
    fn arrondir_petits_rayons_pixel_du_coin() {
        let coin = |rayon| {
            let mut p = unie(20, 20, 255);
            arrondir(&mut p, rayon, [true; 4]);
            alpha(&p, 0, 0)
        };
        // Rayon 1 : le quart de cercle couvre π/4 du pixel (≈ 200 sur 255).
        assert!((195..=206).contains(&coin(1)), "rayon 1 : {}", coin(1));
        // Rayons 2 et 3 : encore une part, de plus en plus petite.
        assert!(coin(2) > 0 && coin(2) < coin(1), "rayon 2 : {}", coin(2));
        assert!(coin(3) > 0 && coin(3) < coin(2), "rayon 3 : {}", coin(3));
        // À partir de 4 : transparent.
        for rayon in 4..=20 {
            assert_eq!(coin(rayon), 0, "rayon {rayon}");
        }
    }

    #[test]
    fn arrondir_symetrique() {
        let (l, h) = (23u32, 17u32);
        let mut p = unie(l, h, 255);
        arrondir(&mut p, 6, [true; 4]);
        for y in 0..h {
            for x in 0..l {
                let a = alpha(&p, x, y);
                assert_eq!(a, alpha(&p, l - 1 - x, y), "({x}, {y}) gauche droite");
                assert_eq!(a, alpha(&p, x, h - 1 - y), "({x}, {y}) haut bas");
                // Le quart de cercle est symétrique par rapport à sa diagonale.
                if x < 6 && y < 6 {
                    assert_eq!(a, alpha(&p, y, x), "({x}, {y}) diagonale");
                }
            }
        }
        // L'alpha monte en s'éloignant du coin, le long du bord et de la diagonale.
        for i in 1..6 {
            assert!(alpha(&p, i, 0) >= alpha(&p, i - 1, 0));
            assert!(alpha(&p, i, i) >= alpha(&p, i - 1, i - 1));
        }
    }

    #[test]
    fn arrondir_seulement_les_coins_demandes() {
        let (l, h) = (20u32, 20u32);
        let mut p = unie(l, h, 255);
        // Haut droit et bas gauche seulement.
        arrondir(&mut p, 8, [false, true, true, false]);
        assert_eq!(alpha(&p, 0, 0), 255);
        assert_eq!(alpha(&p, l - 1, 0), 0);
        assert_eq!(alpha(&p, 0, h - 1), 0);
        assert_eq!(alpha(&p, l - 1, h - 1), 255);
        let mut q = unie(l, h, 255);
        arrondir(&mut q, 8, [false; 4]);
        assert_eq!(q, unie(l, h, 255));
    }

    #[test]
    fn arrondir_multiplie_l_alpha_deja_la() {
        let mut p = unie(20, 20, 128);
        arrondir(&mut p, 8, [true; 4]);
        assert_eq!(alpha(&p, 0, 0), 0);
        assert_eq!(alpha(&p, 10, 10), 128);
        assert_eq!(alpha(&p, 8, 0), 128);
    }

    #[test]
    fn arrondir_rayon_nul_ou_negatif_ne_fait_rien() {
        for rayon in [0, -1, i32::MIN] {
            let mut p = unie(10, 10, 255);
            arrondir(&mut p, rayon, [true; 4]);
            assert_eq!(p, unie(10, 10, 255));
        }
    }

    #[test]
    fn arrondir_image_plus_petite_que_le_rayon_sans_panique() {
        for (l, h) in [(3u32, 2u32), (1, 1), (1, 40), (5, 5)] {
            for rayon in [8, 100, i32::MAX] {
                let mut p = unie(l, h, 255);
                arrondir(&mut p, rayon, [true; 4]);
                assert_eq!(alpha(&p, 0, 0), 0, "{l}x{h} rayon {rayon}");
            }
        }
        // Image vide ou incohérente : rien, sans panique.
        let mut vide = Pixels {
            largeur: 0,
            hauteur: 0,
            rgba: Vec::new(),
        };
        arrondir(&mut vide, 8, [true; 4]);
        let mut abimee = unie(4, 4, 255);
        abimee.rgba.pop();
        let copie = abimee.clone();
        arrondir(&mut abimee, 2, [true; 4]);
        assert_eq!(abimee, copie);
    }

    #[test]
    fn reduire_garde_les_proportions_et_les_couleurs() {
        // 4 × 2 : moitié gauche rouge, moitié droite bleue, opaque.
        let mut rgba = Vec::new();
        for _y in 0..2 {
            for x in 0..4 {
                rgba.extend_from_slice(if x < 2 {
                    &[255, 0, 0, 255]
                } else {
                    &[0, 0, 255, 255]
                });
            }
        }
        let p = Pixels {
            largeur: 4,
            hauteur: 2,
            rgba,
        };
        let r = reduire(&p, 2);
        assert_eq!((r.largeur, r.hauteur), (2, 1));
        assert_eq!(r.rgba, [255, 0, 0, 255, 0, 0, 255, 255]);
        // Déjà assez petite : rendue telle quelle.
        assert_eq!(reduire(&p, 4).rgba, p.rgba);
        // Un pixel transparent ne salit pas la couleur, l'alpha est moyenné.
        let p = Pixels {
            largeur: 2,
            hauteur: 1,
            rgba: vec![10, 20, 30, 255, 200, 200, 200, 0],
        };
        let r = reduire(&p, 1);
        assert_eq!(r.rgba, [10, 20, 30, 128]);
        // Incohérente : rendue telle quelle.
        let p = Pixels {
            largeur: 10,
            hauteur: 10,
            rgba: vec![0; 4],
        };
        assert_eq!(reduire(&p, 2).largeur, 10);
    }

    /// Image dont chaque pixel dit sa place : (x mod 256, y mod 256, 7, 255).
    fn grille(l: u32, h: u32) -> Pixels {
        let mut rgba = Vec::with_capacity((l * h * 4) as usize);
        for y in 0..h {
            for x in 0..l {
                rgba.extend_from_slice(&[(x % 256) as u8, (y % 256) as u8, 7, 255]);
            }
        }
        Pixels {
            largeur: l,
            hauteur: h,
            rgba,
        }
    }

    #[test]
    fn apercu_historique_ecran_1080p() {
        // 1920 × 1080 : partie 1728 × 1080, puis 400 × 250.
        let a = apercu_historique(&grille(1920, 1080));
        assert_eq!((a.largeur, a.hauteur), (400, 250));
        assert_eq!(a.rgba.len(), 400 * 250 * 4);
        // Rouge = x : le dernier pixel vient de la colonne 1727 au plus (partie 16:10, pas
        // l'image entière réduite, qui irait jusqu'à 1919 mod 256 = 127 sur la fin).
        let dernier = &a.rgba[(399 * 4) as usize..(400 * 4) as usize];
        let attendu = ((1728 - 3)..1728).map(|x| (x % 256) as u32).sum::<u32>() / 3;
        assert!(
            u32::from(dernier[0]).abs_diff(attendu) <= 40,
            "{dernier:?} {attendu}"
        );
    }

    #[test]
    fn apercu_historique_partie_du_haut_a_gauche() {
        // Très large : 5760 × 1080 (trois écrans) → partie 1728 × 1080 → 400 × 250.
        let large = grille(5760, 1080);
        let a = apercu_historique(&large);
        assert_eq!((a.largeur, a.hauteur), (400, 250));
        // Même résultat que la partie 1728 × 1080 seule : la droite n'y entre pas.
        assert_eq!(a, apercu_historique(&grille(1728, 1080)));
        // Très haute : 400 × 4000 → partie 400 × 250, sans réduction : les pixels mêmes du
        // haut de l'image.
        let haute = grille(400, 4000);
        let a = apercu_historique(&haute);
        assert_eq!((a.largeur, a.hauteur), (400, 250));
        assert_eq!(a.rgba[..], haute.rgba[..400 * 250 * 4]);
    }

    #[test]
    fn apercu_historique_jamais_agrandi() {
        // Plus petite que 400 × 250 : coupée en 16:10, jamais agrandie.
        let a = apercu_historique(&grille(300, 200));
        assert_eq!((a.largeur, a.hauteur), (300, 188));
        let a = apercu_historique(&grille(320, 200));
        assert_eq!(a, grille(320, 200));
        // Un côté sous la cible : la partie telle quelle.
        let a = apercu_historique(&grille(1000, 240));
        assert_eq!((a.largeur, a.hauteur), (384, 240));
        // Cas limites sans panique.
        for (l, h) in [(1u32, 1u32), (1, 5000), (5000, 1), (2, 1)] {
            let a = apercu_historique(&grille(l, h));
            assert!(a.largeur >= 1 && a.hauteur >= 1, "{l}x{h}");
            assert_eq!(a.rgba.len() as u32, a.largeur * a.hauteur * 4, "{l}x{h}");
        }
        // Vide ou incohérente : rendue telle quelle.
        let vide = Pixels {
            largeur: 0,
            hauteur: 0,
            rgba: Vec::new(),
        };
        assert_eq!(apercu_historique(&vide), vide);
        let mut abimee = grille(800, 500);
        abimee.rgba.pop();
        assert_eq!(apercu_historique(&abimee), abimee);
    }

    #[test]
    fn apercu_historique_couvre_toujours_la_case() {
        // Quelle que soit la taille au-delà de 400 × 250, les deux côtés couvrent la case et
        // la forme reste 16:10 au pixel près.
        for (l, h) in [
            (1001u32, 1000u32),
            (641, 401),
            (4096, 2160),
            (2560, 1600),
            (799, 3000),
        ] {
            let a = apercu_historique(&grille(l, h));
            assert!(
                a.largeur >= 400 && a.hauteur >= 250,
                "{l}x{h} → {}x{}",
                a.largeur,
                a.hauteur
            );
            assert!(
                a.largeur <= 401 && a.hauteur <= 251,
                "{l}x{h} → {}x{}",
                a.largeur,
                a.hauteur
            );
        }
    }
}
