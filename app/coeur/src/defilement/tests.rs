//! Tests de l'assemblage (contrat 0.6, chantier C) : images faites par le test. Chaque cas
//! d'assemblage vérifie la hauteur finale exacte et que chaque ligne de la page y est une
//! fois (à sa place), et note la durée des mesures (`--nocapture`).

use std::time::{Duration, Instant};

use super::*;

const BLANC: [u8; 3] = [250, 250, 250];
const ENCRE: [u8; 3] = [30, 30, 40];

fn melange(a: u32, b: u32, c: u32) -> u32 {
    let mut x = a
        .wrapping_mul(0x9E37_79B1)
        .wrapping_add(b.wrapping_mul(0x85EB_CA77))
        .wrapping_add(c.wrapping_mul(0xC2B2_AE3D));
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^ (x >> 15)
}

/// Un hasard reproductible.
struct Hasard(u64);

impl Hasard {
    fn suivant(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn entre(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.suivant() % (hi - lo + 1) as u32) as i32
    }
}

fn vide(l: u32, h: u32) -> Pixels {
    Pixels {
        largeur: l,
        hauteur: h,
        rgba: vec![255; (l * h * 4) as usize],
    }
}

fn poser(p: &mut Pixels, x: u32, y: u32, c: [u8; 3]) {
    let i = ((y * p.largeur + x) * 4) as usize;
    p.rgba[i..i + 3].copy_from_slice(&c);
    p.rgba[i + 3] = 255;
}

fn pixel(p: &Pixels, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * p.largeur + x) * 4) as usize;
    [p.rgba[i], p.rgba[i + 1], p.rgba[i + 2]]
}

/// Une page de texte aux lignes numérotées : à gauche, le numéro de la ligne en binaire
/// (16 bits de 3 pixels) ; à droite, des lignes de « texte » de 16 pixels séparées de 6
/// pixels de fond, faites de caractères de 7 pixels tirés au hasard.
fn page_texte(l: u32, h: u32, graine: u32) -> Pixels {
    let mut p = vide(l, h);
    for y in 0..h {
        for x in 0..l {
            let c = if x < 48 {
                if (y >> (x / 3)) & 1 == 1 {
                    ENCRE
                } else {
                    BLANC
                }
            } else if x < 56 {
                BLANC
            } else {
                let (ligne, dans) = (y / 22, y % 22);
                let caractere = (x - 56) / 7;
                let encre = (3..19).contains(&dans)
                    && (x - 56) % 7 != 6
                    && !melange(ligne, caractere, graine).is_multiple_of(5)
                    && melange(ligne * 31 + dans, caractere * 7 + (x - 56) % 7, graine)
                        .is_multiple_of(3);
                if encre {
                    ENCRE
                } else {
                    BLANC
                }
            };
            poser(&mut p, x, y, c);
        }
    }
    p
}

/// Une page lisse (pour le décalage fractionnaire) : chaque canal varie peu d'une ligne à
/// la suivante.
fn page_lisse(l: u32, h: u32) -> Pixels {
    let mut p = vide(l, h);
    for y in 0..h {
        for x in 0..l {
            let (xf, yf) = (f64::from(x), f64::from(y));
            // Dent de scie repliée : aucun saut d'une ligne à la suivante.
            let v = (x + 3 * y) % 510;
            let r = if v < 256 { v } else { 509 - v } as u8;
            let g = (128.0 + 90.0 * (yf / 23.0 + xf / 50.0).sin()) as u8;
            let b = (128.0 + 90.0 * (yf / 41.0 - xf / 70.0).sin()) as u8;
            poser(&mut p, x, y, [r, g, b]);
        }
    }
    p
}

/// Une bande décorée (en-tête ou pied) : rayures de couleur, différente de la page.
fn bande(l: u32, h: u32, teinte: u8) -> Pixels {
    let mut p = vide(l, h);
    for y in 0..h {
        for x in 0..l {
            let c = if (x / 9 + y / 4) % 3 == 0 {
                [teinte, 40, 200]
            } else {
                [teinte / 2, 220, 90]
            };
            poser(&mut p, x, y, c);
        }
    }
    p
}

fn lignes(p: &Pixels, y0: u32, n: u32) -> &[u8] {
    let r = (p.largeur * 4) as usize;
    &p.rgba[y0 as usize * r..(y0 + n) as usize * r]
}

/// La zone quand la page est à `position` : en-tête, `h - en-tête - pied` lignes de la
/// page, pied.
fn zone(
    page: &Pixels,
    position: u32,
    h: u32,
    entete: Option<&Pixels>,
    pied: Option<&Pixels>,
) -> Pixels {
    let eh = entete.map_or(0, |e| e.hauteur);
    let ph = pied.map_or(0, |p| p.hauteur);
    let mut rgba = Vec::new();
    if let Some(e) = entete {
        rgba.extend_from_slice(&e.rgba);
    }
    rgba.extend_from_slice(lignes(page, position, h - eh - ph));
    if let Some(p) = pied {
        rgba.extend_from_slice(&p.rgba);
    }
    Pixels {
        largeur: page.largeur,
        hauteur: h,
        rgba,
    }
}

/// Le « vrai » de l'application : mesurer contre la dernière image gardée, ajouter,
/// finir. Rend l'image, les mesures et leurs durées.
fn derouler(
    images: &[Pixels],
    attendus: &[Option<i32>],
    limite: Limite,
) -> (Pixels, Vec<Mesure>, Vec<Duration>) {
    let mut a = Assemblage::nouveau(&images[0], limite).unwrap();
    let mut gardee = images[0].clone();
    let (mut mesures, mut durees) = (Vec::new(), Vec::new());
    for (i, image) in images.iter().enumerate().skip(1) {
        let debut = Instant::now();
        let m = mesurer(
            &gardee,
            image,
            a.fixes(),
            attendus.get(i).copied().flatten(),
            &TOLERANCE,
        );
        durees.push(debut.elapsed());
        mesures.push(m);
        match a.ajouter(image, &m) {
            Ajout::Ajoutees(_) | Ajout::Limite(_) => gardee = image.clone(),
            Ajout::Rien => {}
        }
    }
    (a.finir(&gardee).unwrap(), mesures, durees)
}

fn sans_limite() -> Limite {
    Limite {
        hauteur: 16_384,
        pixels: 25_000_000,
    }
}

fn noter(cas: &str, durees: &[Duration]) {
    let mut d: Vec<_> = durees.to_vec();
    d.sort();
    let mediane = d.get(d.len() / 2).copied().unwrap_or_default();
    let max = d.last().copied().unwrap_or_default();
    eprintln!(
        "defilement {cas} : {} mesures, mediane {mediane:?}, max {max:?}",
        d.len()
    );
}

/// Les positions d'une page de `p` lignes vue par une zone utile de `vue` lignes, par pas
/// de `pas`, la dernière collée en bas.
fn positions(p: u32, vue: u32, pas: u32) -> Vec<u32> {
    let mut v: Vec<u32> = (0..)
        .map(|k| k * pas)
        .take_while(|y| *y < p - vue)
        .collect();
    v.push(p - vue);
    v
}

/// L'image que doit rendre l'assemblage si chaque mesure est juste : l'en-tête et le haut
/// de la première image, les lignes nouvelles du bas de chaque image, le pied de la
/// dernière.
fn attendue(images: &[Pixels], pos: &[u32], eh: u32, ph: u32) -> Pixels {
    let h = images[0].hauteur;
    let mut rgba = lignes(&images[0], 0, h - ph).to_vec();
    for k in 1..images.len() {
        let d = pos[k] - pos[k - 1];
        rgba.extend_from_slice(lignes(&images[k], h - ph - d, d));
    }
    rgba.extend_from_slice(lignes(&images[images.len() - 1], h - ph, ph));
    let _ = eh;
    Pixels {
        largeur: images[0].largeur,
        hauteur: (rgba.len() / (images[0].largeur as usize * 4)) as u32,
        rgba,
    }
}

/// Vérifie qu'une image finale de hauteur `en_tete + page + pied` montre chaque ligne de
/// la page une fois, à sa place, à `ecart` près, hors des pixels que `masque(x, y_page)`
/// écarte.
fn chaque_ligne_une_fois(
    finale: &Pixels,
    page: &Pixels,
    eh: u32,
    ecart: u8,
    masque: impl Fn(u32, u32) -> bool,
) {
    for y in 0..page.hauteur {
        for x in 0..page.largeur {
            if masque(x, y) {
                continue;
            }
            let (a, b) = (pixel(finale, x, y + eh), pixel(page, x, y));
            assert!(
                a.iter().zip(b).all(|(p, q)| p.abs_diff(q) <= ecart),
                "ligne {y} de la page, x {x} : {a:?} contre {b:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// Mesures simples

#[test]
fn immobile_avance_recul_perdue() {
    let page = page_texte(400, 1200, 1);
    let a = zone(&page, 300, 300, None, None);
    let t = &TOLERANCE;
    assert_eq!(mesurer(&a, &a, Fixes::default(), None, t), Mesure::Immobile);
    for d in [1u32, 7, 48, 150, 252] {
        let b = zone(&page, 300 + d, 300, None, None);
        assert_eq!(
            mesurer(&a, &b, Fixes::default(), None, t),
            Mesure::Avance(d),
            "avance {d}"
        );
        let c = zone(&page, 300 - d, 300, None, None);
        assert_eq!(
            mesurer(&a, &c, Fixes::default(), None, t),
            Mesure::Recul(d),
            "recul {d}"
        );
    }
    // Moins de 48 lignes de recouvrement (sur 300 : 1/4 = 75, donc 48) : perdue.
    let b = zone(&page, 300 + 260, 300, None, None);
    assert_eq!(mesurer(&a, &b, Fixes::default(), None, t), Mesure::Perdue);
    let b = zone(&page, 900, 300, None, None);
    assert_eq!(mesurer(&a, &b, Fixes::default(), None, t), Mesure::Perdue);
    // Un attendu faux ne trompe pas : la recherche s'élargit.
    let b = zone(&page, 300 + 100, 300, None, None);
    assert_eq!(
        mesurer(&a, &b, Fixes::default(), Some(30), t),
        Mesure::Avance(100)
    );
    // Tailles différentes ou incohérentes : perdue, sans panique.
    let petite = zone(&page, 0, 200, None, None);
    assert_eq!(
        mesurer(&a, &petite, Fixes::default(), None, t),
        Mesure::Perdue
    );
    let mut abimee = a.clone();
    abimee.rgba.pop();
    assert_eq!(
        mesurer(&a, &abimee, Fixes::default(), None, t),
        Mesure::Perdue
    );
}

#[test]
fn trop_vite_puis_reprise_contre_la_meme_image() {
    // En `Main` : une image perdue n'est pas gardée ; la suivante, mesurée contre la même
    // dernière image gardée, la retrouve.
    let page = page_texte(400, 2000, 2);
    let h = 300;
    let images: Vec<Pixels> = [0u32, 150, 600, 400, 500]
        .iter()
        .map(|p| zone(&page, *p, h, None, None))
        .collect();
    let mut a = Assemblage::nouveau(&images[0], sans_limite()).unwrap();
    let mut gardee = images[0].clone();
    let mut vues = Vec::new();
    for image in &images[1..] {
        let m = mesurer(&gardee, image, a.fixes(), None, &TOLERANCE);
        vues.push(m);
        if let Ajout::Ajoutees(_) = a.ajouter(image, &m) {
            gardee = image.clone();
        }
    }
    assert_eq!(
        vues,
        [
            Mesure::Avance(150),
            Mesure::Perdue,
            Mesure::Avance(250),
            Mesure::Avance(100)
        ]
    );
    let finale = a.finir(&gardee).unwrap();
    assert_eq!(finale.hauteur, 800);
    assert_eq!(finale.rgba, lignes(&page, 0, 800));
}

// ---------------------------------------------------------------------------------------
// Cas d'assemblage du contrat

#[test]
fn lignes_numerotees() {
    let page = page_texte(400, 1800, 3);
    let h = 300;
    let pos = positions(page.hauteur, h, 150);
    let images: Vec<Pixels> = pos.iter().map(|p| zone(&page, *p, h, None, None)).collect();
    let (finale, mesures, durees) = derouler(&images, &[], sans_limite());
    noter("lignes numerotees", &durees);
    for (k, m) in mesures.iter().enumerate() {
        assert_eq!(*m, Mesure::Avance(pos[k + 1] - pos[k]));
    }
    assert_eq!((finale.largeur, finale.hauteur), (400, 1800));
    assert_eq!(finale, page);
}

#[test]
fn entete_et_pied_fixes() {
    let page = page_texte(400, 2000, 4);
    let (entete, pied) = (bande(400, 40, 200), bande(400, 30, 90));
    let h = 300;
    let vue = h - 40 - 30;
    let pos = positions(page.hauteur, vue, 115);
    let images: Vec<Pixels> = pos
        .iter()
        .map(|p| zone(&page, *p, h, Some(&entete), Some(&pied)))
        .collect();
    let mut a = Assemblage::nouveau(&images[0], sans_limite()).unwrap();
    let debut = Instant::now();
    let m = mesurer(&images[0], &images[1], a.fixes(), None, &TOLERANCE);
    let premiere = debut.elapsed();
    assert_eq!(m, Mesure::Avance(115));
    assert_eq!(a.ajouter(&images[1], &m), Ajout::Ajoutees(115));
    assert_eq!(a.fixes(), Fixes { haut: 40, bas: 30 });
    assert_eq!(a.hauteur(), 300 + 115);
    let (finale, mesures, mut durees) = derouler(&images, &[], sans_limite());
    durees.push(premiere);
    noter("en-tete et pied fixes", &durees);
    assert!(
        mesures.iter().all(|m| matches!(m, Mesure::Avance(_))),
        "{mesures:?}"
    );
    // En-tête une fois en haut, toute la page, pied une fois en bas.
    assert_eq!(finale.hauteur, 40 + 2000 + 30);
    assert_eq!(lignes(&finale, 0, 40), &entete.rgba[..]);
    assert_eq!(lignes(&finale, 40, 2000), &page.rgba[..]);
    assert_eq!(lignes(&finale, 2040, 30), &pied.rgba[..]);
    // Avec les bandes connues, la mesure suivante les met de côté.
    assert_eq!(
        mesurer(
            &images[1],
            &images[2],
            Fixes { haut: 40, bas: 30 },
            None,
            &TOLERANCE
        ),
        Mesure::Avance(115)
    );
}

#[test]
fn fixes_d11() {
    let page = page_texte(400, 1000, 5);
    let t = &TOLERANCE;
    let (entete, pied) = (bande(400, 40, 200), bande(400, 30, 90));
    let a = zone(&page, 0, 300, Some(&entete), Some(&pied));
    let b = zone(&page, 100, 300, Some(&entete), Some(&pied));
    assert_eq!(fixes(&a, &b, 100, t), Fixes { haut: 40, bas: 30 });
    // Sans bandes : rien.
    let a = zone(&page, 0, 300, None, None);
    let b = zone(&page, 100, 300, None, None);
    assert_eq!(fixes(&a, &b, 100, t), Fixes::default());
    // Moins de 8 lignes : pas une bande.
    let petite = bande(400, 7, 200);
    let a = zone(&page, 0, 300, Some(&petite), None);
    let b = zone(&page, 100, 300, Some(&petite), None);
    assert_eq!(fixes(&a, &b, 100, t), Fixes::default());
    // Toutes unies : pas une bande.
    let mut unie = vide(400, 20);
    for y in 0..20 {
        for x in 0..400 {
            poser(&mut unie, x, y, [200, 210, 220]);
        }
    }
    let a = zone(&page, 0, 300, Some(&unie), None);
    let b = zone(&page, 100, 300, Some(&unie), None);
    assert_eq!(fixes(&a, &b, 100, t), Fixes::default());
}

#[test]
fn zones_unies_rien_d_invente() {
    let t = &TOLERANCE;
    let mut unie = vide(400, 300);
    for y in 0..300 {
        for x in 0..400 {
            poser(&mut unie, x, y, [240, 240, 240]);
        }
    }
    let debut = Instant::now();
    assert_eq!(
        mesurer(&unie, &unie, Fixes::default(), None, t),
        Mesure::Ambigue
    );
    noter("zones unies", &[debut.elapsed()]);
    // Avec un léger bruit (± 4) : toujours ambiguë.
    let mut h = Hasard(7);
    let mut bruitee = unie.clone();
    for c in bruitee.rgba.chunks_exact_mut(4) {
        for v in &mut c[..3] {
            *v = (i32::from(*v) + h.entre(-4, 4)) as u8;
        }
    }
    assert_eq!(
        mesurer(&unie, &bruitee, Fixes::default(), None, t),
        Mesure::Ambigue
    );
    // Une page dont une partie est unie, plus haute que la zone : dans la partie unie, la
    // mesure est ambiguë et rien n'est ajouté.
    let mut page = page_texte(400, 1500, 6);
    for y in 400..900 {
        for x in 0..400 {
            poser(&mut page, x, y, BLANC);
        }
    }
    let a = zone(&page, 450, 300, None, None);
    let b = zone(&page, 550, 300, None, None);
    assert_eq!(mesurer(&a, &b, Fixes::default(), None, t), Mesure::Ambigue);
    let mut asm = Assemblage::nouveau(&a, sans_limite()).unwrap();
    assert_eq!(asm.ajouter(&b, &Mesure::Ambigue), Ajout::Rien);
    assert_eq!(asm.hauteur(), 300);
    // Une seule ligne de texte (moins de 16 lignes porteuses) : ambiguë aussi.
    let mut presque = unie.clone();
    for y in 100..110 {
        for x in 0..400 {
            if (x / 5) % 2 == 0 {
                poser(&mut presque, x, y, ENCRE);
            }
        }
    }
    let mut descendue = unie.clone();
    for y in 40..50 {
        for x in 0..400 {
            if (x / 5) % 2 == 0 {
                poser(&mut descendue, x, y, ENCRE);
            }
        }
    }
    assert_eq!(
        mesurer(&presque, &descendue, Fixes::default(), None, t),
        Mesure::Ambigue
    );
    // En `Auto`, le décalage attendu départage (D10) : il passe la tolérance, il est pris.
    let a = zone(&page, 450, 300, None, None);
    let b = zone(&page, 550, 300, None, None);
    assert_eq!(
        mesurer(&a, &b, Fixes::default(), Some(100), t),
        Mesure::Avance(100)
    );
}

#[test]
fn page_unie_au_milieu_traversee_avec_l_attendu() {
    let mut page = page_texte(400, 1500, 7);
    for y in 400..900 {
        for x in 0..400 {
            poser(&mut page, x, y, BLANC);
        }
    }
    let h = 300;
    let pos = positions(page.hauteur, h, 100);
    let images: Vec<Pixels> = pos.iter().map(|p| zone(&page, *p, h, None, None)).collect();
    let attendus: Vec<Option<i32>> = (0..pos.len())
        .map(|k| (k > 0).then(|| (pos[k] - pos[k - 1]) as i32))
        .collect();
    let (finale, _, durees) = derouler(&images, &attendus, sans_limite());
    noter("page unie au milieu (attendu)", &durees);
    assert_eq!(finale, page);
}

#[test]
fn carre_anime_a_place_fixe() {
    // Un carré de 120 × 30 (30 % de la largeur), à place fixe dans la zone, change de
    // couleur à chaque image.
    let page = page_texte(400, 1800, 8);
    let h = 300;
    let pos = positions(page.hauteur, h, 150);
    let images: Vec<Pixels> = pos
        .iter()
        .enumerate()
        .map(|(k, p)| {
            let mut z = zone(&page, *p, h, None, None);
            for y in 40..70 {
                for x in 100..220 {
                    poser(
                        &mut z,
                        x,
                        y,
                        [(k * 70 % 256) as u8, 120, (255 - k * 40 % 256) as u8],
                    );
                }
            }
            z
        })
        .collect();
    let (finale, mesures, durees) = derouler(&images, &[], sans_limite());
    noter("carre anime a place fixe", &durees);
    assert!(
        mesures.iter().all(|m| matches!(m, Mesure::Avance(_))),
        "{mesures:?}"
    );
    assert_eq!(finale.hauteur, 1800);
    assert_eq!(finale, attendue(&images, &pos, 0, 0));
    // Le carré n'est que dans la première image : ailleurs, la page.
    chaque_ligne_une_fois(&finale, &page, 0, 0, |x, y| {
        (40..70).contains(&y) && (100..220).contains(&x)
    });
}

#[test]
fn carre_anime_qui_suit_la_page() {
    // Un carré animé dans la page (il défile avec elle) : 30 lignes sur 30 % de la
    // largeur, qui changent entre deux images.
    let page = page_texte(400, 1800, 9);
    let h = 300;
    let pos = positions(page.hauteur, h, 150);
    let (cy0, cy1) = (700u32, 730u32);
    let images: Vec<Pixels> = pos
        .iter()
        .enumerate()
        .map(|(k, p)| {
            let mut z = zone(&page, *p, h, None, None);
            for yp in cy0..cy1 {
                if yp >= *p && yp < *p + h {
                    for x in 100..220 {
                        poser(&mut z, x, yp - p, [(k * 70 % 256) as u8, 30, 200]);
                    }
                }
            }
            z
        })
        .collect();
    let (finale, mesures, durees) = derouler(&images, &[], sans_limite());
    noter("carre anime dans la page", &durees);
    assert!(
        mesures.iter().all(|m| matches!(m, Mesure::Avance(_))),
        "{mesures:?}"
    );
    assert_eq!(finale.hauteur, 1800);
    assert_eq!(finale, attendue(&images, &pos, 0, 0));
    chaque_ligne_une_fois(&finale, &page, 0, 0, |x, y| {
        (cy0..cy1).contains(&y) && (100..220).contains(&x)
    });
}

#[test]
fn pouce_de_barre_qui_bouge() {
    let page = page_texte(400, 2400, 10);
    let h = 300;
    let pos = positions(page.hauteur, h, 150);
    let barre = 14u32;
    let images: Vec<Pixels> = pos
        .iter()
        .map(|p| {
            let mut z = zone(&page, *p, h, None, None);
            let (debut, haut) = (p * h / page.hauteur, h * h / page.hauteur);
            for y in 0..h {
                for x in 400 - barre..400 {
                    let c = if (debut..debut + haut).contains(&y) {
                        [120, 120, 120]
                    } else {
                        [235, 235, 235]
                    };
                    poser(&mut z, x, y, c);
                }
            }
            z
        })
        .collect();
    let (finale, mesures, durees) = derouler(&images, &[], sans_limite());
    noter("pouce de barre", &durees);
    for (k, m) in mesures.iter().enumerate() {
        assert_eq!(*m, Mesure::Avance(pos[k + 1] - pos[k]));
    }
    assert_eq!(finale.hauteur, 2400);
    assert_eq!(finale, attendue(&images, &pos, 0, 0));
    chaque_ligne_une_fois(&finale, &page, 0, 0, |x, _| x >= 400 - barre);
}

/// La zone à `position + 1/2` : chaque ligne est la moyenne de deux lignes de la page.
fn zone_et_demie(page: &Pixels, position: u32, h: u32) -> Pixels {
    let a = zone(page, position, h, None, None);
    let b = zone(page, position + 1, h, None, None);
    let rgba = a
        .rgba
        .iter()
        .zip(&b.rgba)
        .map(|(x, y)| ((u16::from(*x) + u16::from(*y)) / 2) as u8)
        .collect();
    Pixels {
        largeur: page.largeur,
        hauteur: h,
        rgba,
    }
}

#[test]
fn decalage_fractionnaire_par_moyenne_de_deux_lignes() {
    let page = page_lisse(400, 1500);
    let h = 300;
    // Positions 0, 150, 300,5, 450, 600,5, 750, … : une image sur deux à mi-ligne.
    let pos = positions(page.hauteur, h, 150);
    let images: Vec<Pixels> = pos
        .iter()
        .enumerate()
        .map(|(k, p)| {
            if k % 2 == 0 || *p + h + 1 > page.hauteur {
                zone(&page, *p, h, None, None)
            } else {
                zone_et_demie(&page, *p, h)
            }
        })
        .collect();
    // Sans attendu, la mesure tombe sur l'une des deux lignes.
    let m = mesurer(&images[0], &images[1], Fixes::default(), None, &TOLERANCE);
    assert!(
        matches!(m, Mesure::Avance(150) | Mesure::Avance(151)),
        "{m:?}"
    );
    // Avec le pas attendu (`Auto`), la hauteur est exacte.
    let attendus: Vec<Option<i32>> = (0..pos.len())
        .map(|k| (k > 0).then(|| (pos[k] - pos[k - 1]) as i32))
        .collect();
    let (finale, mesures, durees) = derouler(&images, &attendus, sans_limite());
    noter("decalage fractionnaire", &durees);
    for (k, m) in mesures.iter().enumerate() {
        assert_eq!(*m, Mesure::Avance(pos[k + 1] - pos[k]));
    }
    assert_eq!(finale.hauteur, 1500);
    // Les lignes à mi-chemin sont à moins de ± 8 de la page (pente au plus de 5 par ligne).
    chaque_ligne_une_fois(&finale, &page, 0, 8, |_, _| false);
}

#[test]
fn bruit_de_plus_ou_moins_8() {
    let page = page_texte(400, 1800, 11);
    let h = 300;
    let pos = positions(page.hauteur, h, 150);
    let mut hasard = Hasard(99);
    // Une image sur deux reçoit un bruit de ± 8 par canal : deux images successives
    // diffèrent de 8 au plus partout.
    let images: Vec<Pixels> = pos
        .iter()
        .enumerate()
        .map(|(k, p)| {
            let mut z = zone(&page, *p, h, None, None);
            if k % 2 == 1 {
                for c in z.rgba.chunks_exact_mut(4) {
                    for v in &mut c[..3] {
                        *v = (i32::from(*v) + hasard.entre(-8, 8)).clamp(0, 255) as u8;
                    }
                }
            }
            z
        })
        .collect();
    let (finale, mesures, durees) = derouler(&images, &[], sans_limite());
    noter("bruit de 8", &durees);
    for (k, m) in mesures.iter().enumerate() {
        assert_eq!(*m, Mesure::Avance(pos[k + 1] - pos[k]));
    }
    assert_eq!(finale.hauteur, 1800);
    assert_eq!(finale, attendue(&images, &pos, 0, 0));
    chaque_ligne_une_fois(&finale, &page, 0, 8, |_, _| false);
}

#[test]
fn comme_wpf_et_edge_a_l_echelle() {
    // WPF à 100 % : petites zones à 14 d'écart dans presque chaque paire. Edge forcé à 125
    // et 150 % : une ou deux lignes entières très différentes, dont une au bord du
    // recouvrement.
    let page = page_texte(400, 1800, 12);
    let h = 300;
    let pos = positions(page.hauteur, h, 150);
    let mut hasard = Hasard(5);
    let images: Vec<Pixels> = pos
        .iter()
        .enumerate()
        .map(|(k, p)| {
            let mut z = zone(&page, *p, h, None, None);
            if k == 0 {
                return z;
            }
            for _ in 0..3 {
                let (x0, y0) = (hasard.entre(56, 360) as u32, hasard.entre(0, 290) as u32);
                for y in y0..y0 + 8 {
                    for x in x0..x0 + 16 {
                        let c = pixel(&z, x, y).map(|v| v.saturating_sub(14));
                        poser(&mut z, x, y, c);
                    }
                }
            }
            // Une ligne entière fausse dans le recouvrement, et la dernière ligne du
            // recouvrement (bord).
            let y = hasard.entre(10, 140) as u32;
            for yy in [y, 149] {
                for x in 0..400 {
                    poser(
                        &mut z,
                        x,
                        yy,
                        [(x * 7 % 256) as u8, 255 - (x % 256) as u8, 0],
                    );
                }
            }
            z
        })
        .collect();
    let (finale, mesures, durees) = derouler(&images, &[], sans_limite());
    noter("comme WPF et Edge a l'echelle", &durees);
    for (k, m) in mesures.iter().enumerate() {
        assert_eq!(*m, Mesure::Avance(pos[k + 1] - pos[k]));
    }
    assert_eq!(finale.hauteur, 1800);
    assert_eq!(finale, attendue(&images, &pos, 0, 0));
}

/// La ligne survolée de l'Explorateur : à place fixe dans la zone (sous le pointeur), le
/// fond et l'encre s'inversent sur `hauteur` lignes à partir de `y0`.
fn survoler(z: &mut Pixels, y0: u32, hauteur: u32) {
    for y in y0..y0 + hauteur {
        for x in 0..z.largeur - 17 {
            let c = pixel(z, x, y).map(|v| 255 - v);
            poser(z, x, y, c);
        }
    }
}

#[test]
fn bande_qui_change_de_couleur_explorateur() {
    // Comme la sonde : 524 × 552, une ligne de 17 pixels à y 280 (sous le pointeur, au
    // centre) qui change de couleur ; sans la tolérance de la bande, 34 lignes sur 276
    // (12 %) seraient différentes.
    let page = page_texte(524, 2400, 13);
    let h = 552;
    let pos = positions(page.hauteur, h, 276);
    let images: Vec<Pixels> = pos
        .iter()
        .map(|p| {
            let mut z = zone(&page, *p, h, None, None);
            survoler(&mut z, 280, 17);
            z
        })
        .collect();
    let (finale, mesures, durees) = derouler(&images, &[], sans_limite());
    noter(
        "bande qui change de couleur (haut de la zone utile)",
        &durees,
    );
    for (k, m) in mesures.iter().enumerate() {
        assert_eq!(*m, Mesure::Avance(pos[k + 1] - pos[k]), "paire {k}");
    }
    assert_eq!(finale.hauteur, 2400);
    assert_eq!(finale, attendue(&images, &pos, 0, 0));
    // La bande est dans les lignes nouvelles (y 280 ≥ 552 − 276) : elle se voit une fois
    // par image (couture, D12), ailleurs la page est exacte.
    let collee: Vec<u32> = pos.iter().map(|p| p + 280).collect();
    chaque_ligne_une_fois(&finale, &page, 0, 0, |x, y| {
        x < 524 - 17 && collee.iter().any(|c| (*c..*c + 17).contains(&y))
    });
    // Petit pas (la bande se recouvre elle-même) et page immobile avec le survol qui passe
    // à la ligne voisine.
    let a = images[1].clone();
    for d in [10u32, 17, 30, 100] {
        let mut b = zone(&page, pos[1] + d, h, None, None);
        survoler(&mut b, 280, 17);
        assert_eq!(
            mesurer(&a, &b, Fixes::default(), None, &TOLERANCE),
            Mesure::Avance(d),
            "pas {d}"
        );
    }
    let mut b = zone(&page, pos[1], h, None, None);
    survoler(&mut b, 297, 17);
    assert_eq!(
        mesurer(&a, &b, Fixes::default(), None, &TOLERANCE),
        Mesure::Immobile
    );
}

#[test]
fn bande_qui_change_de_couleur_hors_des_lignes_nouvelles() {
    // La même bande plus haut (y 100), dans la partie qui ne fait que recouvrir : elle
    // n'entre qu'avec la première image.
    let page = page_texte(524, 2400, 14);
    let h = 552;
    let pos = positions(page.hauteur, h, 276);
    let images: Vec<Pixels> = pos
        .iter()
        .map(|p| {
            let mut z = zone(&page, *p, h, None, None);
            survoler(&mut z, 100, 20);
            z
        })
        .collect();
    let (finale, mesures, durees) = derouler(&images, &[], sans_limite());
    noter("bande qui change de couleur (recouvrement)", &durees);
    assert!(
        mesures.iter().all(|m| matches!(m, Mesure::Avance(_))),
        "{mesures:?}"
    );
    assert_eq!(finale.hauteur, 2400);
    chaque_ligne_une_fois(&finale, &page, 0, 0, |x, y| {
        x < 524 - 17 && (100..120).contains(&y)
    });
}

#[test]
fn deux_bandes_eparses_ne_passent_pas() {
    // La tolérance de la bande ne couvre qu'une bande à place fixe : des lignes fausses
    // éparses (15 % du recouvrement) ne passent pas, et aucun autre décalage n'est
    // inventé : perdue.
    let page = page_texte(400, 1200, 15);
    let a = zone(&page, 300, 300, None, None);
    let mut b = zone(&page, 400, 300, None, None);
    // 30 lignes fausses éparses (une sur cinq) : 15 % du recouvrement.
    for k in 0..30 {
        let y = 5 + k * 6;
        for x in 0..400 {
            poser(&mut b, x, y, [(x % 256) as u8, 0, 255]);
        }
    }
    assert_eq!(
        mesurer(&a, &b, Fixes::default(), None, &TOLERANCE),
        Mesure::Perdue
    );
}

// ---------------------------------------------------------------------------------------
// Limite, mémoire, aperçu

#[test]
fn limite_d13_coupe_le_dernier_ajout() {
    let page = page_texte(400, 2000, 16);
    let (entete, pied) = (bande(400, 40, 200), bande(400, 30, 90));
    let h = 300;
    let pos = positions(page.hauteur, h - 70, 115);
    let images: Vec<Pixels> = pos
        .iter()
        .map(|p| zone(&page, *p, h, Some(&entete), Some(&pied)))
        .collect();
    // Hauteur : 1 000 ; pixels : 400 × 1 200 (la hauteur gagne).
    let limite = Limite {
        hauteur: 1000,
        pixels: 400 * 1200,
    };
    let mut a = Assemblage::nouveau(&images[0], limite).unwrap();
    let mut gardee = images[0].clone();
    let mut derniers = Vec::new();
    for image in &images[1..] {
        let m = mesurer(&gardee, image, a.fixes(), None, &TOLERANCE);
        let r = a.ajouter(image, &m);
        derniers.push(r);
        if !matches!(r, Ajout::Rien) {
            gardee = image.clone();
        }
        assert!(a.hauteur() <= 1000);
    }
    let coupe = derniers
        .iter()
        .position(|r| matches!(r, Ajout::Limite(_)))
        .unwrap();
    assert!(matches!(derniers[coupe], Ajout::Limite(n) if n < 115));
    assert!(derniers[coupe + 1..].iter().all(|r| *r == Ajout::Limite(0)));
    let finale = a.finir(&gardee).unwrap();
    assert_eq!(finale.hauteur, 1000);
    assert_eq!(lignes(&finale, 0, 40), &entete.rgba[..]);
    assert_eq!(lignes(&finale, 40, 930), lignes(&page, 0, 930));
    assert_eq!(lignes(&finale, 970, 30), &pied.rgba[..]);
    // Par les pixels : 400 × 700.
    let limite = Limite {
        hauteur: 16_384,
        pixels: 400 * 700,
    };
    let images: Vec<Pixels> = positions(2000, 300, 150)
        .iter()
        .map(|p| zone(&page, *p, 300, None, None))
        .collect();
    let (finale, _, _) = derouler(&images, &[], limite);
    assert_eq!(finale.hauteur, 700);
    assert_eq!(finale.rgba, lignes(&page, 0, 700));
    // Une zone plus grande que la limite : refusée.
    assert!(Assemblage::nouveau(
        &images[0],
        Limite {
            hauteur: 200,
            pixels: 1 << 30
        }
    )
    .is_err());
    assert!(Assemblage::nouveau(&vide(0, 0), sans_limite()).is_err());
}

#[test]
fn aucune_reallocation_et_une_copie_a_finir() {
    // 8 000 lignes : 8 bandes de 1 024 ; aucune bande n'est déplacée ni agrandie par les
    // ajouts suivants.
    let page = page_texte(400, 8000, 17);
    let h = 300;
    let pos = positions(page.hauteur, h, 150);
    let images: Vec<Pixels> = pos.iter().map(|p| zone(&page, *p, h, None, None)).collect();
    let mut a = Assemblage::nouveau(&images[0], sans_limite()).unwrap();
    let mut vues: Vec<(*const u8, usize)> = Vec::new();
    for k in 1..images.len() {
        let m = mesurer(&images[k - 1], &images[k], a.fixes(), None, &TOLERANCE);
        assert!(matches!(a.ajouter(&images[k], &m), Ajout::Ajoutees(_)));
        let maintenant = a.adresses();
        assert_eq!(&maintenant[..vues.len()], &vues[..], "une bande a bougé");
        for (_, capacite) in &maintenant {
            assert_eq!(*capacite, 1024 * 400 * 4);
        }
        vues = maintenant;
    }
    assert_eq!(vues.len(), 8);
    let finale = a.finir(images.last().unwrap()).unwrap();
    assert_eq!(finale, page);
    assert_eq!(finale.rgba.capacity(), finale.rgba.len());
}

#[test]
fn le_pied_de_la_premiere_image_quitte_le_flux_au_bord_d_une_bande() {
    // La première image fait 1 030 lignes (plus d'une bande) : couper son pied repasse
    // sous la première bande.
    let page = page_texte(200, 4000, 18);
    let (entete, pied) = (bande(200, 20, 120), bande(200, 30, 60));
    let h = 1030;
    let pos = positions(page.hauteur, h - 50, 400);
    let images: Vec<Pixels> = pos
        .iter()
        .map(|p| zone(&page, *p, h, Some(&entete), Some(&pied)))
        .collect();
    let (finale, _, durees) = derouler(&images, &[], sans_limite());
    noter("zone de 1 030 lignes", &durees);
    assert_eq!(finale.hauteur, 20 + 4000 + 30);
    assert_eq!(lignes(&finale, 20, 4000), &page.rgba[..]);
    assert_eq!(lignes(&finale, 4020, 30), &pied.rgba[..]);
}

#[test]
fn apercu_le_bas_reduit() {
    let page = page_texte(400, 1800, 19);
    let pied = bande(400, 30, 90);
    let h = 300;
    let pos = positions(page.hauteur, h - 30, 135);
    let images: Vec<Pixels> = pos
        .iter()
        .map(|p| zone(&page, *p, h, None, Some(&pied)))
        .collect();
    let mut a = Assemblage::nouveau(&images[0], sans_limite()).unwrap();
    for k in 1..images.len() {
        let m = mesurer(&images[k - 1], &images[k], a.fixes(), None, &TOLERANCE);
        a.ajouter(&images[k], &m);
    }
    assert_eq!(a.hauteur(), 1830);
    // Pleine largeur : le bas exact, pied compris.
    let debut = Instant::now();
    let p = a.apercu(400, 200);
    noter("apercu 400 x 200", &[debut.elapsed()]);
    assert_eq!((p.largeur, p.hauteur), (400, 200));
    assert_eq!(lignes(&p, 170, 30), &pied.rgba[..]);
    assert_eq!(lignes(&p, 0, 170), lignes(&page, 1800 - 170, 170));
    // Réduit de moitié : 200 de large, 300 lignes au plus, venues des 600 du bas.
    let p = a.apercu(200, 300);
    assert_eq!((p.largeur, p.hauteur), (200, 300));
    assert_eq!(p.rgba.len(), 200 * 300 * 4);
    // Le bas de l'aperçu est le pied réduit.
    let attendu = pixel(&pied, 0, 28);
    let vu = pixel(&p, 0, 299);
    assert!(
        attendu.iter().zip(vu).all(|(x, y)| x.abs_diff(y) <= 100),
        "{attendu:?} {vu:?}"
    );
    // Plus haut que l'image : toute l'image, jamais agrandie.
    let p = a.apercu(800, 5000);
    assert_eq!((p.largeur, p.hauteur), (400, 1830));
    assert_eq!(a.apercu(0, 10).hauteur, 0);
}

#[test]
fn finir_refuse_une_autre_taille() {
    let page = page_texte(400, 600, 20);
    let a = Assemblage::nouveau(&zone(&page, 0, 300, None, None), sans_limite()).unwrap();
    assert!(a.finir(&zone(&page, 0, 200, None, None)).is_err());
}

// ---------------------------------------------------------------------------------------
// Budget (contrat §1.7, §9) : à jouer en release.

#[test]
#[ignore = "budget : cargo test --release -- --ignored budget_de_la_mesure --nocapture"]
fn budget_de_la_mesure() {
    let page = page_texte(1024, 6000, 21);
    let h = 700;
    let mut durees_sans = Vec::new();
    let mut durees_avec = Vec::new();
    let mut durees_immobile = Vec::new();
    for k in 0..30u32 {
        let p = k * 150;
        let a = zone(&page, p, h, None, None);
        let d = 200 + (k * 37) % 250;
        let b = zone(&page, p + d, h, None, None);
        let debut = Instant::now();
        let m = mesurer(&a, &b, Fixes::default(), None, &TOLERANCE);
        durees_sans.push(debut.elapsed());
        assert_eq!(m, Mesure::Avance(d));
        let debut = Instant::now();
        let m = mesurer(&a, &b, Fixes::default(), Some(d as i32), &TOLERANCE);
        durees_avec.push(debut.elapsed());
        assert_eq!(m, Mesure::Avance(d));
        let debut = Instant::now();
        let m = mesurer(&a, &a, Fixes::default(), None, &TOLERANCE);
        durees_immobile.push(debut.elapsed());
        assert_eq!(m, Mesure::Immobile);
    }
    // Le pire : perdue (tout est comparé).
    let mut durees_perdue = Vec::new();
    for k in 0..10u32 {
        let a = zone(&page, k * 300, h, None, None);
        let b = zone(&page, k * 300 + 690, h, None, None);
        let debut = Instant::now();
        assert_eq!(
            mesurer(&a, &b, Fixes::default(), None, &TOLERANCE),
            Mesure::Perdue
        );
        durees_perdue.push(debut.elapsed());
    }
    noter("1024x700 sans attendu", &durees_sans);
    noter("1024x700 avec attendu", &durees_avec);
    noter("1024x700 immobile", &durees_immobile);
    noter("1024x700 perdue", &durees_perdue);
    let mut d = durees_sans.clone();
    d.sort();
    let mediane = d[d.len() / 2];
    assert!(
        cfg!(debug_assertions) || mediane <= Duration::from_millis(15),
        "mediane {mediane:?}"
    );
}
