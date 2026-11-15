//! Ce qui change d'une image à l'autre (F19) et la réduction de moitié d'une image trop
//! grande pour le codeur (F4, point 3). Pixels RGBA du cœur (`gel::Pixels`).

use crate::systeme::gel::{Pixels, Rect};

/// Vrai si `p` a la forme qu'il annonce (`largeur × hauteur × 4` octets).
fn bien_forme(p: &Pixels) -> bool {
    (p.largeur as usize)
        .checked_mul(p.hauteur as usize)
        .and_then(|n| n.checked_mul(4))
        == Some(p.rgba.len())
}

/// Le plus petit rectangle (pixels de l'image) qui contient les pixels changés entre
/// `avant` et `apres`, de même taille ; `None` : rien n'a changé. Tailles différentes (ou
/// `avant` mal formé) : toute l'image `apres`. Les quatre octets d'un pixel comptent.
pub fn rectangle_change(avant: &Pixels, apres: &Pixels) -> Option<Rect> {
    let (l, h) = (apres.largeur as usize, apres.hauteur as usize);
    if !bien_forme(apres) || l == 0 || h == 0 {
        return None;
    }
    if avant.largeur != apres.largeur || avant.hauteur != apres.hauteur || !bien_forme(avant) {
        return Some(tout(apres));
    }
    let ligne = l * 4;
    // Bornes en pixels : x0..x1, y0..y1 (x1 et y1 exclus).
    let (mut x0, mut x1, mut y0, mut y1) = (l, 0usize, h, 0usize);
    for (y, (a, b)) in avant
        .rgba
        .chunks_exact(ligne)
        .zip(apres.rgba.chunks_exact(ligne))
        .enumerate()
    {
        if a == b {
            continue;
        }
        if y0 == h {
            y0 = y;
        }
        y1 = y + 1;
        // Le premier pixel changé, cherché seulement à gauche de ce qu'on a déjà.
        if x0 > 0 {
            if let Some(x) = premier_change(&a[..x0 * 4], &b[..x0 * 4]) {
                x0 = x;
            }
        }
        // Le dernier, cherché seulement à droite.
        if x1 < l {
            if let Some(x) = dernier_change(&a[x1 * 4..], &b[x1 * 4..]) {
                x1 = x1 + x + 1;
            }
        }
    }
    if y0 == h {
        return None;
    }
    Some(Rect {
        x: x0 as i32,
        y: y0 as i32,
        largeur: (x1 - x0) as i32,
        hauteur: (y1 - y0) as i32,
    })
}

fn premier_change(a: &[u8], b: &[u8]) -> Option<usize> {
    a.chunks_exact(4)
        .zip(b.chunks_exact(4))
        .position(|(p, q)| p != q)
}

fn dernier_change(a: &[u8], b: &[u8]) -> Option<usize> {
    a.chunks_exact(4)
        .zip(b.chunks_exact(4))
        .rposition(|(p, q)| p != q)
}

fn tout(p: &Pixels) -> Rect {
    Rect {
        x: 0,
        y: 0,
        largeur: p.largeur as i32,
        hauteur: p.hauteur as i32,
    }
}

/// `r` ramené dans l'image `l × h` : (x, y, largeur, hauteur) en pixels, vide si hors.
fn dans(r: Rect, l: usize, h: usize) -> (usize, usize, usize, usize) {
    let borne = |v: i32, max: usize| (v.max(0) as usize).min(max);
    let x0 = borne(r.x, l);
    let y0 = borne(r.y, h);
    let x1 = borne(r.x.saturating_add(r.largeur), l).max(x0);
    let y1 = borne(r.y.saturating_add(r.hauteur), h).max(y0);
    (x0, y0, x1 - x0, y1 - y0)
}

/// Les pixels de `r` dans `apres`, en RGBA ; alpha 0 pour un pixel égal à celui d'`avant`.
/// `r` est ramené dans l'image ; `avant` d'une autre taille : aucun pixel transparent.
pub fn sous_image(avant: &Pixels, apres: &Pixels, r: Rect) -> Vec<u8> {
    let mut sortie = Vec::new();
    sous_image_dans(avant, apres, r, &mut sortie);
    sortie
}

/// `sous_image`, écrite dans `sortie` (réutilisée, vidée d'abord).
pub fn sous_image_dans(avant: &Pixels, apres: &Pixels, r: Rect, sortie: &mut Vec<u8>) {
    sortie.clear();
    let (l, h) = (apres.largeur as usize, apres.hauteur as usize);
    if !bien_forme(apres) {
        return;
    }
    let (x0, y0, rl, rh) = dans(r, l, h);
    let comparer = avant.largeur == apres.largeur && avant.hauteur == apres.hauteur;
    let comparer = comparer && bien_forme(avant);
    sortie.reserve(rl * rh * 4);
    for y in y0..y0 + rh {
        let debut = (y * l + x0) * 4;
        let fin = debut + rl * 4;
        let b = &apres.rgba[debut..fin];
        if !comparer {
            sortie.extend_from_slice(b);
            continue;
        }
        let a = &avant.rgba[debut..fin];
        for (p, q) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
            if p == q {
                sortie.extend_from_slice(&[q[0], q[1], q[2], 0]);
            } else {
                sortie.extend_from_slice(q);
            }
        }
    }
}

/// `pixels` réduits de moitié sur chaque côté (moyenne de quatre pixels, arrondie) dans
/// `dest`, réutilisé (F4, point 3). Un côté impair perd sa dernière ligne ou colonne ; un
/// côté de 1 reste à 1. Image mal formée : `dest` vidé.
pub fn moitie(pixels: &Pixels, dest: &mut Pixels) {
    dest.rgba.clear();
    let (l, h) = (pixels.largeur as usize, pixels.hauteur as usize);
    if !bien_forme(pixels) || l == 0 || h == 0 {
        dest.largeur = 0;
        dest.hauteur = 0;
        return;
    }
    let (dl, dh) = ((l / 2).max(1), (h / 2).max(1));
    dest.largeur = dl as u32;
    dest.hauteur = dh as u32;
    dest.rgba.reserve(dl * dh * 4);
    let src = &pixels.rgba;
    for y in 0..dh {
        let ya = 2 * y;
        let yb = (2 * y + 1).min(h - 1);
        for x in 0..dl {
            let xa = 2 * x;
            let xb = (2 * x + 1).min(l - 1);
            let i = [
                (ya * l + xa) * 4,
                (ya * l + xb) * 4,
                (yb * l + xa) * 4,
                (yb * l + xb) * 4,
            ];
            for c in 0..4 {
                let s: u32 = i.iter().map(|&k| u32::from(src[k + c])).sum();
                dest.rgba.push(((s + 2) / 4) as u8);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn uni(l: u32, h: u32, c: [u8; 4]) -> Pixels {
        Pixels {
            largeur: l,
            hauteur: h,
            rgba: c.repeat((l * h) as usize),
        }
    }

    fn poser(p: &mut Pixels, x: u32, y: u32, c: [u8; 4]) {
        let i = ((y * p.largeur + x) * 4) as usize;
        p.rgba[i..i + 4].copy_from_slice(&c);
    }

    fn rect(x: i32, y: i32, largeur: i32, hauteur: i32) -> Rect {
        Rect {
            x,
            y,
            largeur,
            hauteur,
        }
    }

    #[test]
    fn rien_puis_un_pixel_puis_deux_coins() {
        let a = uni(600, 450, [10, 20, 30, 255]);
        let mut b = a.clone();
        assert_eq!(rectangle_change(&a, &b), None);
        poser(&mut b, 17, 33, [0, 0, 0, 255]);
        assert_eq!(rectangle_change(&a, &b), Some(rect(17, 33, 1, 1)));
        // Seul l'alpha change : c'est un changement.
        let mut c = a.clone();
        poser(&mut c, 5, 6, [10, 20, 30, 254]);
        assert_eq!(rectangle_change(&a, &c), Some(rect(5, 6, 1, 1)));
        // Deux coins opposés : toute l'image.
        poser(&mut b, 0, 0, [1, 1, 1, 255]);
        poser(&mut b, 599, 449, [1, 1, 1, 255]);
        assert_eq!(rectangle_change(&a, &b), Some(rect(0, 0, 600, 450)));
        // Un L : la colonne de gauche vient d'une ligne, la droite d'une autre.
        let mut d = a.clone();
        poser(&mut d, 100, 10, [0; 4]);
        poser(&mut d, 300, 20, [0; 4]);
        poser(&mut d, 50, 30, [0; 4]);
        assert_eq!(rectangle_change(&a, &d), Some(rect(50, 10, 251, 21)));
    }

    #[test]
    fn tailles_differentes_toute_l_image() {
        let a = uni(4, 4, [0; 4]);
        let b = uni(5, 3, [1, 1, 1, 255]);
        assert_eq!(rectangle_change(&a, &b), Some(rect(0, 0, 5, 3)));
        // Une sous-image sans comparaison possible : rien de transparent.
        let s = sous_image(&a, &b, rect(0, 0, 5, 3));
        assert_eq!(s.len(), 5 * 3 * 4);
        assert!(s.chunks_exact(4).all(|p| p == [1, 1, 1, 255]));
        // Image mal formée : rien.
        let abime = Pixels {
            largeur: 3,
            hauteur: 3,
            rgba: vec![0; 7],
        };
        assert_eq!(rectangle_change(&a, &abime), None);
        assert!(sous_image(&a, &abime, rect(0, 0, 3, 3)).is_empty());
    }

    #[test]
    fn sous_image_transparente_ou_inchangee() {
        let a = uni(8, 6, [1, 2, 3, 255]);
        let mut b = a.clone();
        poser(&mut b, 2, 1, [9, 9, 9, 255]);
        poser(&mut b, 4, 3, [8, 8, 8, 255]);
        let r = rectangle_change(&a, &b).expect("changé");
        assert_eq!(r, rect(2, 1, 3, 3));
        let s = sous_image(&a, &b, r);
        assert_eq!(s.len(), 3 * 3 * 4);
        // Coin haut gauche : changé, opaque ; coin bas droit : changé ; le reste
        // transparent, mais avec la couleur de l'image (pour la palette).
        assert_eq!(&s[0..4], &[9, 9, 9, 255]);
        assert_eq!(&s[(2 * 3 + 2) * 4..(2 * 3 + 3) * 4], &[8, 8, 8, 255]);
        let transparents = s.chunks_exact(4).filter(|p| p[3] == 0).count();
        assert_eq!(transparents, 7);
        assert!(s
            .chunks_exact(4)
            .filter(|p| p[3] == 0)
            .all(|p| p[..3] == [1, 2, 3]));
        // Un rectangle qui déborde est ramené dans l'image.
        let s = sous_image(&a, &b, rect(6, 4, 10, 10));
        assert_eq!(s.len(), 2 * 2 * 4);
        assert!(sous_image(&a, &b, rect(20, 20, 3, 3)).is_empty());
        assert!(sous_image(&a, &b, rect(-5, -5, 3, 3)).is_empty());
    }

    #[test]
    fn pire_cas_et_zone_fixe_restent_bon_marche() {
        // Pire cas : tout change à chaque image (600 × 450) ; zone fixe : rien. Le temps
        // est écrit (debug ou release selon la commande) ; aucune borne serrée ici.
        let a = uni(600, 450, [0, 0, 0, 255]);
        let mut b = a.clone();
        for (i, o) in b.rgba.iter_mut().enumerate() {
            if i % 4 != 3 {
                *o = (i % 251) as u8 + 1;
            }
        }
        let t = Instant::now();
        let r = rectangle_change(&a, &b);
        let mut s = Vec::new();
        sous_image_dans(&a, &b, r.expect("tout"), &mut s);
        let pire = t.elapsed();
        assert_eq!(r, Some(rect(0, 0, 600, 450)));
        assert!(s.chunks_exact(4).all(|p| p[3] == 255));
        let t = Instant::now();
        assert_eq!(rectangle_change(&a, &a.clone()), None);
        let fixe = t.elapsed();
        println!("difference 600x450 : pire cas {pire:?}, zone fixe {fixe:?}");
    }

    #[test]
    fn moitie_moyenne_et_cotes_impairs() {
        let mut p = uni(4, 2, [0, 0, 0, 255]);
        poser(&mut p, 0, 0, [100, 0, 0, 255]);
        poser(&mut p, 1, 0, [100, 0, 0, 255]);
        poser(&mut p, 0, 1, [0, 100, 0, 255]);
        poser(&mut p, 1, 1, [0, 101, 0, 255]);
        let mut d = Pixels {
            largeur: 99,
            hauteur: 99,
            rgba: vec![7; 12],
        };
        moitie(&p, &mut d);
        assert_eq!((d.largeur, d.hauteur), (2, 1));
        assert_eq!(d.rgba, vec![50, 50, 0, 255, 0, 0, 0, 255]);
        // Côtés impairs : la dernière colonne et la dernière ligne sont perdues.
        let q = uni(5, 3, [40, 40, 40, 255]);
        moitie(&q, &mut d);
        assert_eq!((d.largeur, d.hauteur, d.rgba.len()), (2, 1, 8));
        // Un côté de 1 reste à 1.
        let r = uni(1, 7, [9, 9, 9, 9]);
        moitie(&r, &mut d);
        assert_eq!((d.largeur, d.hauteur), (1, 3));
        assert!(d.rgba.iter().all(|&o| o == 9));
        // Mal formée : vidé.
        let abime = Pixels {
            largeur: 2,
            hauteur: 2,
            rgba: vec![0; 3],
        };
        moitie(&abime, &mut d);
        assert_eq!((d.largeur, d.hauteur, d.rgba.len()), (0, 0, 0));
        // `dest` est réutilisé : sa capacité ne grandit pas d'une image à l'autre.
        let grande = uni(600, 450, [1, 2, 3, 255]);
        moitie(&grande, &mut d);
        let cap = d.rgba.capacity();
        for _ in 0..5 {
            moitie(&grande, &mut d);
        }
        assert_eq!(d.rgba.capacity(), cap);
    }
}
