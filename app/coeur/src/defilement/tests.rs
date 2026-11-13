//! Tests de l'assemblage (contrat 0.6, chantier C). Les images sont faites par le test et
//! imitent des pages réelles : texte anticrénelé clairsemé (fûts, barres, lignes de
//! longueur variable, paragraphes), listes aux noms proches, page de réglages aux grands
//! blancs, barre latérale, en-tête collant, pied qui apparaît, survol de l'Explorateur,
//! zones de WPF, lignes d'Edge à l'échelle, motif qui se répète. Une mesure est juste ou
//! dit `Ambigue` ou `Perdue`, jamais une autre avance ; un assemblage est comparé ligne à
//! ligne à l'image attendue (`par_placement` : chaque image posée à sa place, la plus
//! récente l'emporte).

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

fn unie(l: u32, h: u32, c: [u8; 3]) -> Pixels {
    let mut rgba = Vec::with_capacity((l * h * 4) as usize);
    for _ in 0..l * h {
        rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
    }
    Pixels {
        largeur: l,
        hauteur: h,
        rgba,
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

fn rectangle(p: &mut Pixels, x0: u32, y0: u32, l: u32, h: u32, c: [u8; 3]) {
    for y in y0..(y0 + h).min(p.hauteur) {
        for x in x0..(x0 + l).min(p.largeur) {
            poser(p, x, y, c);
        }
    }
}

/// Glyphes de 7 × 12 : fûts verticaux continus, barres horizontales, bords anticrénelés
/// (intensité de 0 à 255).
type Glyphe = [[u8; 7]; 12];

fn glyphes(graine: u32) -> Vec<Glyphe> {
    let mut h = Hasard(u64::from(graine) * 7919 + 13);
    (0..40)
        .map(|_| {
            let mut g = [[0u8; 7]; 12];
            for c in 0..6 {
                if h.suivant().is_multiple_of(2) {
                    let a = h.entre(0, 4) as usize;
                    let b = h.entre(7, 12) as usize;
                    for rang in g.iter_mut().take(b).skip(a) {
                        rang[c] = 255;
                    }
                }
            }
            for r in [0usize, 5, 11] {
                if h.suivant().is_multiple_of(3) {
                    for v in &mut g[r][..6] {
                        *v = (*v).max(200);
                    }
                }
            }
            let copie = g;
            for r in 0..12 {
                for c in 0..7 {
                    let voisin = (c > 0 && copie[r][c - 1] > 0) || (c < 6 && copie[r][c + 1] > 0);
                    if copie[r][c] == 0 && voisin {
                        g[r][c] = 90;
                    }
                }
            }
            g
        })
        .collect()
}

/// Écrit des glyphes (indices ; 40 ou plus : une espace) à partir de `(x0, y0)`, fondus
/// dans ce qui est déjà là.
fn ecrire(p: &mut Pixels, g: &[Glyphe], x0: u32, y0: u32, texte: &[usize], encre: [u8; 3]) {
    for (k, gi) in texte.iter().enumerate() {
        let Some(gl) = g.get(*gi) else { continue };
        for (r, rang) in gl.iter().enumerate() {
            for (c, v) in rang.iter().enumerate() {
                let (x, y) = (x0 + k as u32 * 7 + c as u32, y0 + r as u32);
                if *v == 0 || x >= p.largeur || y >= p.hauteur {
                    continue;
                }
                let v = u32::from(*v);
                let f = pixel(p, x, y);
                let m = |i: usize| {
                    ((u32::from(f[i]) * (255 - v) + u32::from(encre[i]) * v) / 255) as u8
                };
                poser(p, x, y, [m(0), m(1), m(2)]);
            }
        }
    }
}

/// Une page de texte : colonne de `x0` à `x1`, lignes de 22 px (glyphes de 12 lignes),
/// paragraphes de 6 lignes séparés d'une ligne vide, dernière ligne de paragraphe plus
/// courte, mots séparés d'espaces.
fn page_texte(l: u32, h: u32, x0: u32, x1: u32, graine: u32) -> Pixels {
    let g = glyphes(graine);
    let mut p = unie(l, h, BLANC);
    let cellules = (x1 - x0) / 7;
    for ligne in 0..h.div_ceil(22) {
        if ligne % 7 == 6 {
            continue;
        }
        let longueur = if ligne % 7 == 5 {
            cellules * (melange(ligne, 1, graine) % 80 + 10) / 100
        } else {
            cellules - melange(ligne, 2, graine) % 6
        };
        let texte: Vec<usize> = (0..longueur)
            .map(|k| {
                let m = melange(ligne, k, graine + 1);
                if m.is_multiple_of(6) {
                    99
                } else {
                    (m / 7) as usize % g.len()
                }
            })
            .collect();
        ecrire(&mut p, &g, x0, ligne * 22 + 5, &texte, ENCRE);
    }
    p
}

/// Une liste de l'Explorateur : lignes de 22 px, icône, nom au préfixe commun dont seuls
/// deux chiffres changent, date et type identiques d'une ligne à l'autre.
fn page_liste(l: u32, n: u32, graine: u32) -> Pixels {
    let g = glyphes(graine);
    let mut p = unie(l, n * 22, [255, 255, 255]);
    for i in 0..n {
        let y0 = i * 22;
        rectangle(&mut p, 8, y0 + 3, 16, 16, [230, 190, 60]);
        let mut texte: Vec<usize> = (0..14).map(|k| (k * 7 + 3) % 40).collect();
        texte.extend([(i / 10 % 10) as usize, (i % 10) as usize, 12, 25, 31]);
        texte.extend([99; 5]);
        texte.extend((0..16).map(|k| (k * 11 + 1) % 40));
        texte.extend([99; 3]);
        texte.extend((0..8).map(|k| (k * 5 + 2) % 40));
        ecrire(&mut p, &g, 32, y0 + 5, &texte, [20, 20, 20]);
    }
    p
}

/// Une page de réglages : par section, un titre, quatre à six réglages de 44 px (libellé,
/// interrupteur à droite, filet entre deux), puis un grand blanc de 80 à 260 lignes.
fn page_reglages(l: u32, h: u32, graine: u32) -> Pixels {
    let g = glyphes(graine);
    let mut p = unie(l, h, [249, 249, 249]);
    let mut y = 20;
    let mut s = 0;
    while y + 40 < h {
        let titre: Vec<usize> = (0..10 + melange(s, 0, graine) % 12)
            .map(|k| (melange(s, k, graine) % 40) as usize)
            .collect();
        ecrire(&mut p, &g, 40, y, &titre, [10, 10, 10]);
        y += 36;
        for r in 0..4 + melange(s, 1, graine) % 3 {
            if y + 44 >= h {
                break;
            }
            let libelle: Vec<usize> = (0..8 + melange(s, r + 2, graine) % 30)
                .map(|k| {
                    let m = melange(s * 31 + r, k, graine);
                    if m.is_multiple_of(7) {
                        99
                    } else {
                        (m % 40) as usize
                    }
                })
                .collect();
            ecrire(&mut p, &g, 56, y + 16, &libelle, [30, 30, 30]);
            let actif = melange(s, r, graine + 9).is_multiple_of(2);
            let c = if actif {
                [0, 103, 192]
            } else {
                [140, 140, 140]
            };
            rectangle(&mut p, l - 120, y + 12, 40, 20, c);
            rectangle(
                &mut p,
                if actif { l - 96 } else { l - 116 },
                y + 16,
                12,
                12,
                [255, 255, 255],
            );
            rectangle(&mut p, 40, y + 43, l - 120, 1, [225, 225, 225]);
            y += 44;
        }
        y += 80 + melange(s, 7, graine) % 180;
        s += 1;
    }
    p
}

/// Une page lisse (décalage fractionnaire) : chaque canal varie peu d'une ligne à la
/// suivante.
fn page_lisse(l: u32, h: u32) -> Pixels {
    let mut p = unie(l, h, BLANC);
    for y in 0..h {
        for x in 0..l {
            let (xf, yf) = (f64::from(x), f64::from(y));
            let v = (x + 9 * y) % 510;
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
    let mut p = unie(l, h, BLANC);
    for y in 0..h {
        for x in 0..l {
            let c = if (x / 9 + y / 4).is_multiple_of(3) {
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

fn z(page: &Pixels, position: u32, h: u32) -> Pixels {
    zone(page, position, h, None, None)
}

/// Pose `haut` (les `haut.hauteur` premières lignes) sur une image.
fn couvrir(im: &mut Pixels, y0: u32, bande: &Pixels) {
    let r = (im.largeur * 4) as usize;
    im.rgba[y0 as usize * r..(y0 + bande.hauteur) as usize * r].copy_from_slice(&bande.rgba);
}

/// Ce que fait l'application : mesurer contre la dernière image gardée, ajouter, finir.
/// Rend l'image, les mesures et leurs durées.
fn derouler(
    images: &[Pixels],
    attendus: &[Attendu],
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
            attendus.get(i).copied().unwrap_or(Attendu::Aucun),
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

fn mediane(durees: &[Duration]) -> Duration {
    let mut d = durees.to_vec();
    d.sort();
    d.get(d.len() / 2).copied().unwrap_or_default()
}

fn noter(cas: &str, durees: &[Duration]) {
    let max = durees.iter().max().copied().unwrap_or_default();
    eprintln!(
        "defilement {cas} : {} mesures, mediane {:?}, max {max:?}",
        durees.len(),
        mediane(durees)
    );
}

/// Les positions d'une page de `p` lignes vue par `vue` lignes, par pas de `pas`, la
/// dernière collée en bas.
fn positions(p: u32, vue: u32, pas: u32) -> Vec<u32> {
    let mut v: Vec<u32> = (0..)
        .map(|k| k * pas)
        .take_while(|y| *y < p - vue)
        .collect();
    v.push(p - vue);
    v
}

/// Les attendus lus d'un déroulé : le vrai pas, décalé de `faux`.
fn attendus(pos: &[u32], faux: i32) -> Vec<Attendu> {
    (0..pos.len())
        .map(|k| {
            if k == 0 {
                Attendu::Aucun
            } else {
                Attendu::Lu((pos[k] - pos[k - 1]) as i32 + faux)
            }
        })
        .collect()
}

/// L'image attendue d'un assemblage par placement : chaque image posée à sa position, ses
/// lignes à partir de `garde[k]` réécrivant celles déjà posées.
fn par_placement(images: &[Pixels], pos: &[u32], garde: &[u32]) -> Pixels {
    let (l, h) = (images[0].largeur, images[0].hauteur);
    let total = pos[pos.len() - 1] - pos[0] + h;
    let r = (l * 4) as usize;
    let mut rgba = vec![0u8; total as usize * r];
    for (k, im) in images.iter().enumerate() {
        let g = garde.get(k).copied().unwrap_or(0) as usize;
        let y = (pos[k] - pos[0]) as usize;
        rgba[(y + g) * r..(y + h as usize) * r].copy_from_slice(&im.rgba[g * r..]);
    }
    Pixels {
        largeur: l,
        hauteur: total,
        rgba,
    }
}

/// Deux images égales ligne à ligne ; sinon, la première ligne qui diffère.
fn egales(vu: &Pixels, attendu: &Pixels, cas: &str) {
    assert_eq!(
        (vu.largeur, vu.hauteur),
        (attendu.largeur, attendu.hauteur),
        "{cas} : taille"
    );
    let r = (vu.largeur * 4) as usize;
    if let Some(y) = (0..vu.hauteur as usize)
        .find(|y| vu.rgba[y * r..(y + 1) * r] != attendu.rgba[y * r..(y + 1) * r])
    {
        panic!("{cas} : la ligne {y} diffère");
    }
}

/// Chaque ligne de l'image finale est cette ligne de la page vue par l'une des images qui
/// la couvrent (à sa place) : aucune ligne étrangère, aucune ligne décalée.
fn chaque_ligne_d_une_image(vu: &Pixels, images: &[Pixels], pos: &[u32], cas: &str) {
    let h = images[0].hauteur;
    assert_eq!(
        vu.hauteur,
        pos[pos.len() - 1] - pos[0] + h,
        "{cas} : hauteur"
    );
    let r = (vu.largeur * 4) as usize;
    for y in 0..vu.hauteur {
        let ligne = &vu.rgba[y as usize * r..(y as usize + 1) * r];
        let trouvee = images.iter().zip(pos).any(|(im, p)| {
            let p = p - pos[0];
            (p..p + h).contains(&y) && lignes(im, y - p, 1) == ligne
        });
        assert!(trouvee, "{cas} : la ligne {y} ne vient d'aucune image");
    }
}

/// Chaque mesure est la vraie avance, ou `Ambigue`, ou `Perdue` (jamais une autre) ; rend
/// le nombre de justes.
fn jamais_fausse(m: Mesure, vrai: i64, cas: &str) -> bool {
    let vue = match m {
        Mesure::Avance(d) => i64::from(d),
        Mesure::Recul(d) => -i64::from(d),
        Mesure::Immobile => 0,
        Mesure::Ambigue | Mesure::Perdue => return false,
    };
    assert_eq!(vue, vrai, "{cas} : {m:?} pour un vrai décalage de {vrai}");
    true
}

// ---------------------------------------------------------------------------------------
// Mesures

#[test]
fn immobile_avance_recul_perdue() {
    let t = &TOLERANCE;
    let page = page_texte(600, 4000, 20, 580, 11);
    let h = 400;
    let a = z(&page, 1000, h);
    assert_eq!(
        mesurer(&a, &a, Fixes::default(), Attendu::Aucun, t),
        Mesure::Immobile
    );
    for d in [1u32, 2, 3, 22, 37, 100, 150, 300] {
        let b = z(&page, 1000 + d, h);
        assert_eq!(
            mesurer(&a, &b, Fixes::default(), Attendu::Aucun, t),
            Mesure::Avance(d),
            "avance {d}"
        );
        let c = z(&page, 1000 - d, h);
        assert_eq!(
            mesurer(&a, &c, Fixes::default(), Attendu::Aucun, t),
            Mesure::Recul(d),
            "recul {d}"
        );
    }
    // Moins de 48 lignes de recouvrement, ou plus de recouvrement du tout : perdue.
    for d in [360u32, 400, 1500] {
        let b = z(&page, 1000 + d, h);
        assert_eq!(
            mesurer(&a, &b, Fixes::default(), Attendu::Aucun, t),
            Mesure::Perdue,
            "{d}"
        );
    }
    // 49 lignes de recouvrement, deux lignes de texte : trop peu pour décider.
    let b = z(&page, 1351, h);
    assert!(matches!(
        mesurer(&a, &b, Fixes::default(), Attendu::Aucun, t),
        Mesure::Ambigue | Mesure::Avance(351)
    ));
    // Tailles différentes ou incohérentes : perdue, sans panique.
    assert_eq!(
        mesurer(&a, &z(&page, 0, 200), Fixes::default(), Attendu::Aucun, t),
        Mesure::Perdue
    );
    let mut abimee = a.clone();
    abimee.rgba.pop();
    assert_eq!(
        mesurer(&a, &abimee, Fixes::default(), Attendu::Aucun, t),
        Mesure::Perdue
    );
}

#[test]
fn texte_etroit_dans_une_zone_large() {
    // Une colonne de lecture de 640 px dans une zone de 800 à 2 560 de large : au-delà de
    // 768 colonnes, une sur deux, trois ou quatre seulement est lue.
    let t = &TOLERANCE;
    for (l, x0) in [(800u32, 80u32), (1600, 480), (2560, 960)] {
        let page = page_texte(l, 2400, x0, x0 + 640, 3);
        let h = 600;
        let a = z(&page, 1000, h);
        for d in [0u32, 1, 11, 22, 150, 300, 540] {
            let b = z(&page, 1000 + d, h);
            let m = mesurer(&a, &b, Fixes::default(), Attendu::Aucun, t);
            assert_eq!(
                m,
                if d == 0 {
                    Mesure::Immobile
                } else {
                    Mesure::Avance(d)
                },
                "{l} : {d}"
            );
        }
        for d in [560u32, 800] {
            let b = z(&page, 1000 + d, h);
            assert_eq!(
                mesurer(&a, &b, Fixes::default(), Attendu::Aucun, t),
                Mesure::Perdue,
                "{l} : {d}"
            );
        }
    }
}

#[test]
fn l_attendu_lu() {
    // D passe la position lue par l'application. Exacte (ou à 3 lignes près), elle est
    // rendue ; fausse, l'image la contredit : jamais une autre avance inventée, au pire
    // `Ambigue`. Deux images égales : `Immobile`, quel que soit le lu.
    let t = &TOLERANCE;
    let page = page_texte(600, 3000, 20, 580, 5);
    let a = z(&page, 900, 400);
    let b = z(&page, 1050, 400);
    for e in [
        Attendu::Aucun,
        Attendu::Lu(150),
        Attendu::Lu(148),
        Attendu::Lu(153),
    ] {
        assert_eq!(
            mesurer(&a, &b, Fixes::default(), e, t),
            Mesure::Avance(150),
            "{e:?}"
        );
    }
    for e in [120, 180, 400, -150, 0, 5000] {
        let m = mesurer(&a, &b, Fixes::default(), Attendu::Lu(e), t);
        assert!(
            matches!(m, Mesure::Avance(150) | Mesure::Ambigue),
            "{e} {m:?}"
        );
    }
    for e in [
        Attendu::Aucun,
        Attendu::Lu(0),
        Attendu::Lu(150),
        Attendu::Lu(-40),
        Attendu::Lu(5000),
    ] {
        assert_eq!(
            mesurer(&a, &a, Fixes::default(), e, t),
            Mesure::Immobile,
            "{e:?}"
        );
    }
    // Une page unie, ou une seule ligne de texte : sans lu, rien ne décide ; avec un lu,
    // deux images égales sont immobiles (l'application ment ou n'a pas encore peint).
    let vide = unie(400, 300, [240, 240, 240]);
    assert_eq!(
        mesurer(&vide, &vide, Fixes::default(), Attendu::Aucun, t),
        Mesure::Ambigue
    );
    let mut une = vide.clone();
    couvrir(&mut une, 100, &page_texte(400, 22, 10, 390, 9));
    for im in [&vide, &une] {
        for e in [Attendu::Lu(0), Attendu::Lu(150)] {
            assert_eq!(
                mesurer(im, im, Fixes::default(), e, t),
                Mesure::Immobile,
                "{e:?}"
            );
        }
    }
    assert_eq!(
        mesurer(&une, &une, Fixes::default(), Attendu::Aucun, t),
        Mesure::Ambigue
    );
    // Le bas d'une liste : trois lignes de texte puis du blanc, immobile.
    let mut bas = vide.clone();
    couvrir(&mut bas, 0, &page_texte(400, 66, 10, 390, 9));
    for e in [Attendu::Aucun, Attendu::Lu(150)] {
        assert_eq!(
            mesurer(&bas, &bas, Fixes::default(), e, t),
            Mesure::Immobile
        );
    }
}

#[test]
fn zones_unies_rien_d_invente() {
    let t = &TOLERANCE;
    let vide = unie(400, 300, [240, 240, 240]);
    // Un bruit de ± 4 n'est pas de l'encre.
    let mut h = Hasard(7);
    let mut bruitee = vide.clone();
    for c in bruitee.rgba.chunks_exact_mut(4) {
        for v in &mut c[..3] {
            *v = (i32::from(*v) + h.entre(-4, 4)) as u8;
        }
    }
    assert_eq!(
        mesurer(&vide, &bruitee, Fixes::default(), Attendu::Aucun, t),
        Mesure::Ambigue
    );
    assert_eq!(
        mesurer(&vide, &bruitee, Fixes::default(), Attendu::Lu(100), t),
        Mesure::Immobile
    );
    // Une page dont la partie unie est plus haute que la zone : deux images égales. Sans
    // lu, ambiguë et rien d'ajouté ; avec un lu, immobile (rien d'inventé non plus).
    let mut page = page_texte(400, 1500, 10, 390, 6);
    rectangle(&mut page, 0, 400, 400, 500, BLANC);
    let a = z(&page, 450, 300);
    let b = z(&page, 550, 300);
    assert_eq!(
        mesurer(&a, &b, Fixes::default(), Attendu::Aucun, t),
        Mesure::Ambigue
    );
    assert_eq!(
        mesurer(&a, &b, Fixes::default(), Attendu::Lu(100), t),
        Mesure::Immobile
    );
    let mut asm = Assemblage::nouveau(&a, sans_limite()).unwrap();
    assert_eq!(asm.ajouter(&b, &Mesure::Ambigue), Ajout::Rien);
    assert_eq!(asm.hauteur(), 300);
    // Une ligne de texte qui monte dans une zone unie (moins de 16 lignes porteuses) :
    // sans lu, ambiguë ; le lu exact traverse le blanc ; un lu faux ne passe pas.
    let ligne = page_texte(400, 22, 10, 390, 2);
    let (mut avant, mut apres) = (vide.clone(), vide.clone());
    couvrir(&mut avant, 100, &ligne);
    couvrir(&mut apres, 40, &ligne);
    for (e, m) in [
        (Attendu::Aucun, Mesure::Ambigue),
        (Attendu::Lu(60), Mesure::Avance(60)),
        (Attendu::Lu(61), Mesure::Avance(60)),
        (Attendu::Lu(90), Mesure::Ambigue),
        (Attendu::Lu(0), Mesure::Ambigue),
    ] {
        assert_eq!(mesurer(&avant, &apres, Fixes::default(), e, t), m, "{e:?}");
    }
}

#[test]
fn motif_qui_se_repete() {
    // Un bloc de trois lignes de texte répété à l'identique (période 66) : les décalages
    // d, d ± 66… s'accordent tous. Sans attendu : ambiguë ; l'attendu à 3 lignes près
    // choisit ; un attendu loin de tous : ambiguë.
    let t = &TOLERANCE;
    let bloc = page_texte(500, 66, 20, 480, 8);
    let mut page = unie(500, 1200, BLANC);
    for k in 0..1200 / 66 {
        couvrir(&mut page, k * 66, &bloc);
    }
    let a = z(&page, 100, 400);
    let b = z(&page, 200, 400);
    assert_eq!(
        mesurer(&a, &b, Fixes::default(), Attendu::Aucun, t),
        Mesure::Ambigue
    );
    assert_eq!(
        mesurer(&a, &b, Fixes::default(), Attendu::Lu(100), t),
        Mesure::Avance(100)
    );
    assert_eq!(
        mesurer(&a, &b, Fixes::default(), Attendu::Lu(102), t),
        Mesure::Avance(100)
    );
    assert_eq!(
        mesurer(&a, &b, Fixes::default(), Attendu::Lu(166), t),
        Mesure::Avance(166)
    );
    assert_eq!(
        mesurer(&a, &b, Fixes::default(), Attendu::Lu(120), t),
        Mesure::Ambigue
    );
    // Deux images identiques d'un motif : sans lu, 0 n'est pas seul, ambiguë ; avec un
    // lu, immobile. Jamais une avance.
    assert_eq!(
        mesurer(&a, &a, Fixes::default(), Attendu::Aucun, t),
        Mesure::Ambigue
    );
    assert_eq!(
        mesurer(&a, &a, Fixes::default(), Attendu::Lu(66), t),
        Mesure::Immobile
    );
}

#[test]
fn liste_aux_noms_proches() {
    // Explorateur, 524 × 552 : lignes de 22 px dont seuls deux chiffres changent.
    let t = &TOLERANCE;
    let page = page_liste(524, 200, 7);
    let h = 552;
    let a = z(&page, 440, h);
    for (d, e) in [
        (0u32, Attendu::Aucun),
        (0, Attendu::Lu(84)),
        (22, Attendu::Aucun),
        (84, Attendu::Aucun),
        (84, Attendu::Lu(84)),
        (88, Attendu::Aucun),
        (132, Attendu::Lu(132)),
    ] {
        let b = z(&page, 440 + d, h);
        let m = mesurer(&a, &b, Fixes::default(), e, t);
        assert_eq!(
            m,
            if d == 0 {
                Mesure::Immobile
            } else {
                Mesure::Avance(d)
            },
            "{d} {e:?}"
        );
    }
    // Un lu faux d'une ou plusieurs lignes de liste : pas une ligne voisine.
    for e in [28, 62, 106, 64] {
        let b = z(&page, 440 + 84, h);
        let m = mesurer(&a, &b, Fixes::default(), Attendu::Lu(e), t);
        assert!(
            matches!(m, Mesure::Avance(84) | Mesure::Ambigue),
            "{e} {m:?}"
        );
    }
    for d in [528u32, 540, 600, 1000] {
        let b = z(&page, 440 + d, h);
        for e in [Attendu::Aucun, Attendu::Lu(d as i32)] {
            assert!(!jamais_fausse(
                mesurer(&a, &b, Fixes::default(), e, t),
                i64::from(d),
                "liste"
            ));
        }
    }
    // Déroulé par pas de la molette (84) : la liste exacte.
    let pos = positions(page.hauteur, h, 84);
    let images: Vec<Pixels> = pos.iter().map(|p| z(&page, *p, h)).collect();
    for att in [Vec::new(), attendus(&pos, 0), attendus(&pos, 2)] {
        let (finale, _, durees) = derouler(&images, &att, sans_limite());
        noter("liste aux noms proches", &durees);
        egales(&finale, &page, "liste");
    }
}

#[test]
fn barre_laterale_et_contenu_clairseme() {
    // Barre latérale grise à gauche (chaque ligne a de l'encre, aucune ne change) et un
    // bloc de texte de 30 lignes toutes les 260 lignes : la barre ne vote pour rien.
    let t = &TOLERANCE;
    let texte = page_texte(800, 4000, 200, 780, 21);
    let mut page = unie(800, 4000, [255, 255, 255]);
    rectangle(&mut page, 0, 0, 160, 4000, [200, 205, 215]);
    for y in 0..4000 {
        if y % 260 < 30 {
            let r = 3200usize;
            page.rgba[y as usize * r + 800..(y as usize + 1) * r]
                .copy_from_slice(&texte.rgba[y as usize * r + 800..(y as usize + 1) * r]);
        }
    }
    let h = 552;
    for p0 in [100u32, 400, 1000] {
        let a = z(&page, p0, h);
        for d in [0u32, 130, 260, 400, 510, 520, 530, 560, 700, 1040] {
            let b = z(&page, p0 + d, h);
            let juste = jamais_fausse(
                mesurer(&a, &b, Fixes::default(), Attendu::Aucun, t),
                i64::from(d),
                "barre latérale",
            );
            assert!(juste || d > 130, "{p0} {d}");
        }
    }
    // Déroulé : ce qui est assemblé est exactement le haut de la page (une image qui ne
    // recouvre qu'un bloc, une ligne de texte, ne décide pas).
    let pos = positions(page.hauteur, h, 130);
    let images: Vec<Pixels> = pos.iter().map(|p| z(&page, *p, h)).collect();
    let (finale, _, _) = derouler(&images, &[], sans_limite());
    assert!(finale.hauteur > h);
    egales(&finale, &z(&page, 0, finale.hauteur), "barre latérale");
}

#[test]
fn page_de_reglages_aux_grands_blancs() {
    // Comme les pages de réglages : des blancs de 80 à 260 lignes. Enchaîné comme D le
    // fait (contre la dernière image gardée) : sans lu, une mesure est juste ou dit
    // qu'elle ne sait pas ; avec le lu exact, chaque mesure est juste et la page entière
    // est assemblée.
    let t = &TOLERANCE;
    let page = page_reglages(900, 5000, 4);
    for (h, pas) in [
        (600u32, 300u32),
        (500, 120),
        (400, 84),
        (700, 200),
        (400, 340),
    ] {
        let pos = positions(page.hauteur, h, pas);
        let images: Vec<Pixels> = pos.iter().map(|p| z(&page, *p, h)).collect();
        for avec_lu in [false, true] {
            let mut a = Assemblage::nouveau(&images[0], sans_limite()).unwrap();
            let (mut gardee, mut pg) = (images[0].clone(), pos[0]);
            let mut justes = 0;
            for k in 1..images.len() {
                let vrai = i64::from(pos[k]) - i64::from(pg);
                let e = if avec_lu {
                    Attendu::Lu(vrai as i32)
                } else {
                    Attendu::Aucun
                };
                let m = mesurer(&gardee, &images[k], a.fixes(), e, t);
                justes += usize::from(jamais_fausse(m, vrai, "réglages"));
                if avec_lu {
                    assert_eq!(m, mesure_de(vrai as i32), "{h} {pas} {k}");
                }
                if !matches!(a.ajouter(&images[k], &m), Ajout::Rien) {
                    gardee = images[k].clone();
                    pg = pos[k];
                }
            }
            eprintln!(
                "reglages zone {h} pas {pas} lu {avec_lu} : {justes} justes sur {}",
                images.len() - 1
            );
            // Ce qui est assemblé est exactement le haut de la page.
            let finale = a.finir(&gardee).unwrap();
            egales(&finale, &z(&page, 0, finale.hauteur), "réglages");
            if avec_lu {
                assert_eq!(finale.hauteur, page.hauteur, "{h} {pas}");
            }
        }
    }
}

#[test]
fn defilement_horizontal_jamais_une_avance() {
    let t = &TOLERANCE;
    let page = page_texte(600, 3000, 20, 580, 9);
    let a = z(&page, 500, 400);
    for dx in [1u32, 2, 5] {
        let b0 = z(&page, 600, 400);
        let mut b = b0.clone();
        for y in 0..400 {
            for x in 0..600 {
                poser(
                    &mut b,
                    x,
                    y,
                    if x >= dx {
                        pixel(&b0, x - dx, y)
                    } else {
                        BLANC
                    },
                );
            }
        }
        assert!(!jamais_fausse(
            mesurer(&a, &b, Fixes::default(), Attendu::Aucun, t),
            100,
            "horizontal"
        ));
    }
}

#[test]
fn lignes_fausses_eparses_ne_passent_pas() {
    // 30 lignes fausses éparses (15 % du recouvrement) : aucun décalage n'est inventé.
    let page = page_texte(400, 1200, 10, 390, 15);
    let a = z(&page, 300, 300);
    let mut b = z(&page, 400, 300);
    for k in 0..30 {
        for x in 0..400 {
            poser(&mut b, x, 5 + k * 6, [(x % 256) as u8, 0, 255]);
        }
    }
    assert!(!jamais_fausse(
        mesurer(&a, &b, Fixes::default(), Attendu::Aucun, &TOLERANCE),
        100,
        "éparses"
    ));
}

// ---------------------------------------------------------------------------------------
// Assemblages

#[test]
fn deroule_texte_realiste() {
    let page = page_texte(600, 4000, 20, 580, 11);
    for (h, pas) in [(400u32, 180u32), (300, 37), (552, 276)] {
        let pos = positions(page.hauteur, h, pas);
        let images: Vec<Pixels> = pos.iter().map(|p| z(&page, *p, h)).collect();
        for faux in [None, Some(0), Some(2), Some(-3)] {
            let att = faux.map_or_else(Vec::new, |f| attendus(&pos, f));
            let (finale, mesures, durees) = derouler(&images, &att, sans_limite());
            noter(&format!("texte {h} pas {pas} attendu {faux:?}"), &durees);
            for (k, m) in mesures.iter().enumerate() {
                assert_eq!(
                    *m,
                    Mesure::Avance(pos[k + 1] - pos[k]),
                    "{h} {pas} {faux:?} : {k}"
                );
            }
            egales(&finale, &page, "texte");
        }
    }
}

#[test]
fn trop_vite_puis_reprise_contre_la_meme_image() {
    // Une image perdue n'est pas gardée ; la suivante, mesurée contre la même dernière
    // image gardée, la retrouve.
    let page = page_texte(400, 2000, 10, 390, 2);
    let images: Vec<Pixels> = [0u32, 150, 600, 400, 500]
        .iter()
        .map(|p| z(&page, *p, 300))
        .collect();
    let mut a = Assemblage::nouveau(&images[0], sans_limite()).unwrap();
    let mut gardee = images[0].clone();
    let mut vues = Vec::new();
    for image in &images[1..] {
        let m = mesurer(&gardee, image, a.fixes(), Attendu::Aucun, &TOLERANCE);
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
    egales(&a.finir(&gardee).unwrap(), &z(&page, 0, 800), "reprise");
}

#[test]
fn entete_collant_et_pied_fixes() {
    let page = page_texte(400, 2000, 10, 390, 4);
    let (entete, pied) = (bande(400, 40, 200), bande(400, 30, 90));
    let h = 300;
    let pos = positions(page.hauteur, h - 70, 115);
    let images: Vec<Pixels> = pos
        .iter()
        .map(|p| zone(&page, *p, h, Some(&entete), Some(&pied)))
        .collect();
    let mut a = Assemblage::nouveau(&images[0], sans_limite()).unwrap();
    let m = mesurer(
        &images[0],
        &images[1],
        a.fixes(),
        Attendu::Aucun,
        &TOLERANCE,
    );
    assert_eq!(m, Mesure::Avance(115));
    assert_eq!(a.ajouter(&images[1], &m), Ajout::Ajoutees(115));
    assert_eq!(a.fixes(), Fixes { haut: 40, bas: 30 });
    assert_eq!(a.hauteur(), 300 + 115);
    let (finale, mesures, durees) = derouler(&images, &[], sans_limite());
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
    // Avec les bandes connues, la mesure les met de côté.
    let f = Fixes { haut: 40, bas: 30 };
    assert_eq!(
        mesurer(&images[1], &images[2], f, Attendu::Aucun, &TOLERANCE),
        Mesure::Avance(115)
    );
}

#[test]
fn fixes_d11() {
    let t = &TOLERANCE;
    let page = page_texte(400, 1000, 10, 390, 5);
    let (entete, pied) = (bande(400, 40, 200), bande(400, 30, 90));
    let a = zone(&page, 0, 300, Some(&entete), Some(&pied));
    let b = zone(&page, 100, 300, Some(&entete), Some(&pied));
    assert_eq!(fixes(&a, &b, 100, t), Fixes { haut: 40, bas: 30 });
    assert_eq!(
        fixes(&z(&page, 0, 300), &z(&page, 100, 300), 100, t),
        Fixes::default()
    );
    // Sans minimum : 7 lignes, ou une bande unie, sont des bandes (sinon elles se
    // répéteraient dans l'image).
    for e in [bande(400, 7, 200), unie(400, 20, [40, 90, 160])] {
        let a = zone(&page, 0, 300, Some(&e), None);
        let b = zone(&page, 100, 300, Some(&e), None);
        assert_eq!(
            fixes(&a, &b, 100, t),
            Fixes {
                haut: e.hauteur,
                bas: 0
            }
        );
    }
}

#[test]
fn en_tete_qui_apparait_en_route() {
    // Un en-tête collant de 60 ou 84 lignes (56 px à 150 %) apparaît à la deuxième image
    // et reste : il n'est jamais posé, la page est exacte.
    let page = page_texte(600, 3000, 20, 580, 4);
    let h = 500;
    for eh in [60u32, 84] {
        let ent = bande(600, eh, 200);
        let pos = positions(page.hauteur, h, 150);
        let images: Vec<Pixels> = pos
            .iter()
            .enumerate()
            .map(|(k, p)| {
                let mut im = z(&page, *p, h);
                if k >= 1 {
                    couvrir(&mut im, 0, &ent);
                }
                im
            })
            .collect();
        let (finale, mesures, _) = derouler(&images, &[], sans_limite());
        for (k, m) in mesures.iter().enumerate() {
            assert_eq!(*m, Mesure::Avance(pos[k + 1] - pos[k]), "{eh} : {k}");
        }
        egales(&finale, &page, "en-tête qui apparaît");
    }
}

#[test]
fn pied_qui_apparait_en_route() {
    // Une bannière (cookies) apparaît en bas à la troisième image et reste : elle n'est
    // qu'une fois en bas, les lignes de la page qu'elle cache toujours (les 30 dernières)
    // ne peuvent pas être là.
    let page = page_texte(400, 2000, 10, 390, 4);
    let ban = bande(400, 30, 90);
    let h = 300;
    let pos = positions(page.hauteur, h, 120);
    let images: Vec<Pixels> = pos
        .iter()
        .enumerate()
        .map(|(k, p)| {
            let mut im = z(&page, *p, h);
            if k >= 2 {
                couvrir(&mut im, h - 30, &ban);
            }
            im
        })
        .collect();
    let (finale, mesures, _) = derouler(&images, &[], sans_limite());
    for (k, m) in mesures.iter().enumerate() {
        assert_eq!(*m, Mesure::Avance(pos[k + 1] - pos[k]), "{k}");
    }
    assert_eq!(finale.hauteur, 2000);
    assert_eq!(lignes(&finale, 0, 1970), lignes(&page, 0, 1970));
    assert_eq!(lignes(&finale, 1970, 30), &ban.rgba[..]);
}

#[test]
fn pied_uni_ou_court() {
    for (hp, uni) in [(30u32, true), (6, false)] {
        let page = page_texte(400, 2000, 10, 390, 4);
        let pied = if uni {
            unie(400, hp, [40, 90, 160])
        } else {
            bande(400, hp, 90)
        };
        let h = 300;
        let pos = positions(page.hauteur, h - hp, 120);
        let images: Vec<Pixels> = pos
            .iter()
            .map(|p| zone(&page, *p, h, None, Some(&pied)))
            .collect();
        let (finale, mesures, _) = derouler(&images, &[], sans_limite());
        assert!(
            mesures.iter().all(|m| matches!(m, Mesure::Avance(_))),
            "{mesures:?}"
        );
        assert_eq!(finale.hauteur, 2000 + hp);
        assert_eq!(lignes(&finale, 0, 2000), &page.rgba[..]);
        assert_eq!(lignes(&finale, 2000, hp), &pied.rgba[..]);
    }
}

fn zone_et_demie(page: &Pixels, position: u32, h: u32) -> Pixels {
    let (a, b) = (z(page, position, h), z(page, position + 1, h));
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
    // Une image sur deux à mi-ligne.
    let pos = positions(page.hauteur, h, 150);
    let images: Vec<Pixels> = pos
        .iter()
        .enumerate()
        .map(|(k, p)| {
            if k.is_multiple_of(2) || *p + h + 1 > page.hauteur {
                z(&page, *p, h)
            } else {
                zone_et_demie(&page, *p, h)
            }
        })
        .collect();
    // Sans attendu, la mesure tombe sur l'une des deux lignes.
    let m = mesurer(
        &images[0],
        &images[1],
        Fixes::default(),
        Attendu::Aucun,
        &TOLERANCE,
    );
    assert!(
        matches!(m, Mesure::Avance(150) | Mesure::Avance(151)),
        "{m:?}"
    );
    // Avec le pas attendu, elle le prend.
    let (finale, mesures, _) = derouler(&images, &attendus(&pos, 0), sans_limite());
    for (k, m) in mesures.iter().enumerate() {
        assert_eq!(*m, Mesure::Avance(pos[k + 1] - pos[k]));
    }
    assert_eq!(finale.hauteur, 1500);
    // Les lignes à mi-chemin sont à moins de ± 8 de la page.
    for (p, q) in finale.rgba.iter().zip(&page.rgba) {
        assert!(p.abs_diff(*q) <= 8);
    }
}

#[test]
fn bruit_de_plus_ou_moins_8() {
    let page = page_texte(400, 1800, 10, 390, 11);
    let h = 300;
    let pos = positions(page.hauteur, h, 150);
    let mut hasard = Hasard(99);
    let images: Vec<Pixels> = pos
        .iter()
        .enumerate()
        .map(|(k, p)| {
            let mut im = z(&page, *p, h);
            if k % 2 == 1 {
                for c in im.rgba.chunks_exact_mut(4) {
                    for v in &mut c[..3] {
                        *v = (i32::from(*v) + hasard.entre(-8, 8)).clamp(0, 255) as u8;
                    }
                }
            }
            im
        })
        .collect();
    let (finale, mesures, _) = derouler(&images, &[], sans_limite());
    for (k, m) in mesures.iter().enumerate() {
        assert_eq!(*m, Mesure::Avance(pos[k + 1] - pos[k]));
    }
    egales(&finale, &par_placement(&images, &pos, &[]), "bruit");
}

#[test]
fn comme_wpf_et_edge_a_l_echelle() {
    // WPF à 100 % : des zones à 14 d'écart dans presque chaque paire. Edge à 125 et 150 % :
    // une ou deux lignes entières très différentes, dont une au bord du recouvrement.
    let page = page_texte(400, 1800, 10, 390, 12);
    let h = 300;
    let pos = positions(page.hauteur, h, 150);
    let mut hasard = Hasard(5);
    let images: Vec<Pixels> = pos
        .iter()
        .enumerate()
        .map(|(k, p)| {
            let mut im = z(&page, *p, h);
            if k == 0 {
                return im;
            }
            for _ in 0..3 {
                let (x0, y0) = (hasard.entre(0, 360) as u32, hasard.entre(0, 290) as u32);
                for y in y0..y0 + 8 {
                    for x in x0..x0 + 16 {
                        let c = pixel(&im, x, y).map(|v| v.saturating_sub(14));
                        poser(&mut im, x, y, c);
                    }
                }
            }
            for y in [hasard.entre(60, 140) as u32, 149] {
                for x in 0..400 {
                    poser(
                        &mut im,
                        x,
                        y,
                        [(x * 7 % 256) as u8, 255 - (x % 256) as u8, 0],
                    );
                }
            }
            im
        })
        .collect();
    let (finale, mesures, _) = derouler(&images, &[], sans_limite());
    for (k, m) in mesures.iter().enumerate() {
        assert_eq!(*m, Mesure::Avance(pos[k + 1] - pos[k]));
    }
    egales(&finale, &par_placement(&images, &pos, &[]), "WPF et Edge");
}

/// La ligne survolée de l'Explorateur : à place fixe dans la zone (sous le pointeur), le
/// fond et l'encre s'inversent sur `hauteur` lignes à partir de `y0`.
fn survoler(im: &mut Pixels, y0: u32, hauteur: u32) {
    for y in y0..y0 + hauteur {
        for x in 0..im.largeur - 17 {
            let c = pixel(im, x, y).map(|v| 255 - v);
            poser(im, x, y, c);
        }
    }
}

/// La ligne sélectionnée de Windows 11 : fond bleu pâle sur `hauteur` lignes, l'encre
/// reste.
fn teinter(im: &mut Pixels, y0: u32, hauteur: u32) {
    for y in y0..y0 + hauteur {
        for x in 0..im.largeur - 17 {
            let c = pixel(im, x, y);
            let t = [c[0].saturating_sub(30), c[1].saturating_sub(12), c[2]];
            poser(im, x, y, t);
        }
    }
}

#[test]
fn survol_et_selection_de_l_explorateur() {
    // Comme la sonde : 524 × 552, une ligne de 17 px sous le pointeur (y 280) qui change de
    // couleur, ou une ligne sélectionnée à place fixe ; ailleurs, la page. Placement : la
    // bande vient de l'image la plus récente qui couvre ces lignes.
    for (y0, inverser) in [(280u32, true), (100, true), (280, false)] {
        let page = page_liste(524, 120, 13);
        let h = 552;
        let pos = positions(page.hauteur, h, 276);
        let images: Vec<Pixels> = pos
            .iter()
            .map(|p| {
                let mut im = z(&page, *p, h);
                if inverser {
                    survoler(&mut im, y0, 17)
                } else {
                    teinter(&mut im, y0, 22)
                }
                im
            })
            .collect();
        let (finale, mesures, _) = derouler(&images, &[], sans_limite());
        for (k, m) in mesures.iter().enumerate() {
            assert_eq!(*m, Mesure::Avance(pos[k + 1] - pos[k]), "{y0} paire {k}");
        }
        chaque_ligne_d_une_image(&finale, &images, &pos, "survol");
        // Petit pas (la bande se recouvre elle-même), et page immobile avec le survol qui
        // passe à la ligne voisine.
        let a = images[1].clone();
        for d in [10u32, 17, 30, 100] {
            let mut b = z(&page, pos[1] + d, h);
            survoler(&mut b, y0, 17);
            if inverser {
                assert_eq!(
                    mesurer(&a, &b, Fixes::default(), Attendu::Aucun, &TOLERANCE),
                    Mesure::Avance(d),
                    "pas {d}"
                );
            }
        }
        if inverser {
            let mut b = z(&page, pos[1], h);
            survoler(&mut b, y0 + 22, 17);
            assert_eq!(
                mesurer(&a, &b, Fixes::default(), Attendu::Aucun, &TOLERANCE),
                Mesure::Immobile
            );
        }
    }
}

#[test]
fn animation_jamais_une_fausse_avance() {
    // Un carré de 120 × 30 qui change à chaque image, à place fixe ou dans la page : plus
    // de 3 % du recouvrement change, la mesure ne peut pas passer ; elle ne dit jamais une
    // autre avance.
    let page = page_texte(400, 1800, 10, 390, 8);
    let h = 300;
    for fixe in [true, false] {
        let a = z(&page, 450, h);
        let mut b = z(&page, 600, h);
        let y = if fixe { 40 } else { 700 - 600 };
        rectangle(&mut b, 100, y, 120, 30, [210, 120, 30]);
        let mut a2 = a.clone();
        rectangle(
            &mut a2,
            100,
            if fixe { 40 } else { 700 - 450 },
            120,
            30,
            [20, 120, 230],
        );
        let m = mesurer(&a2, &b, Fixes::default(), Attendu::Aucun, &TOLERANCE);
        jamais_fausse(m, 150, "animation");
        eprintln!("animation (a place fixe : {fixe}) : {m:?}");
    }
}

#[test]
fn pouce_de_barre_qui_bouge() {
    let page = page_texte(400, 2400, 10, 380, 10);
    let h = 300;
    let pos = positions(page.hauteur, h, 150);
    let images: Vec<Pixels> = pos
        .iter()
        .map(|p| {
            let mut im = z(&page, *p, h);
            let (debut, haut) = (p * h / page.hauteur, h * h / page.hauteur);
            rectangle(&mut im, 386, 0, 14, h, [235, 235, 235]);
            rectangle(&mut im, 386, debut, 14, haut, [120, 120, 120]);
            im
        })
        .collect();
    let (finale, mesures, _) = derouler(&images, &[], sans_limite());
    for (k, m) in mesures.iter().enumerate() {
        assert_eq!(*m, Mesure::Avance(pos[k + 1] - pos[k]));
    }
    egales(&finale, &par_placement(&images, &pos, &[]), "pouce");
}

// ---------------------------------------------------------------------------------------
// Limite, mémoire, aperçu

#[test]
fn limite_d13_coupe_le_dernier_ajout() {
    let page = page_texte(400, 2000, 10, 390, 16);
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
        let m = mesurer(&gardee, image, a.fixes(), Attendu::Aucun, &TOLERANCE);
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
        .map(|p| z(&page, *p, 300))
        .collect();
    let (finale, _, _) = derouler(&images, &[], limite);
    egales(&finale, &z(&page, 0, 700), "limite en pixels");
    // Une zone plus grande que la limite, ou vide : refusée.
    let petite = Limite {
        hauteur: 200,
        pixels: 1 << 30,
    };
    assert!(Assemblage::nouveau(&images[0], petite).is_err());
    assert!(Assemblage::nouveau(&unie(0, 0, BLANC), sans_limite()).is_err());
}

#[test]
fn aucune_reallocation_et_une_copie_a_finir() {
    // 8 000 lignes : 8 bandes de 1 024, aucune déplacée ni agrandie par les ajouts.
    let page = page_texte(400, 8000, 10, 390, 17);
    let h = 300;
    let pos = positions(page.hauteur, h, 150);
    let images: Vec<Pixels> = pos.iter().map(|p| z(&page, *p, h)).collect();
    let mut a = Assemblage::nouveau(&images[0], sans_limite()).unwrap();
    let mut vues: Vec<(*const u8, usize)> = Vec::new();
    for k in 1..images.len() {
        let m = mesurer(
            &images[k - 1],
            &images[k],
            a.fixes(),
            Attendu::Aucun,
            &TOLERANCE,
        );
        assert!(
            matches!(a.ajouter(&images[k], &m), Ajout::Ajoutees(_)),
            "{k} {m:?}"
        );
        let maintenant = a.adresses();
        assert_eq!(&maintenant[..vues.len()], &vues[..], "une bande a bougé");
        assert!(maintenant.iter().all(|(_, c)| *c == 1024 * 400 * 4));
        vues = maintenant;
    }
    assert_eq!(vues.len(), 8);
    let finale = a.finir(images.last().unwrap()).unwrap();
    egales(&finale, &page, "8 000 lignes");
    assert_eq!(finale.rgba.capacity(), finale.rgba.len());
}

#[test]
fn le_pied_de_la_premiere_image_quitte_le_flux_au_bord_d_une_bande() {
    // La zone fait 1 030 lignes (plus d'une bande).
    let page = page_texte(200, 4000, 10, 190, 18);
    let (entete, pied) = (bande(200, 20, 120), bande(200, 30, 60));
    let h = 1030;
    let pos = positions(page.hauteur, h - 50, 400);
    let images: Vec<Pixels> = pos
        .iter()
        .map(|p| zone(&page, *p, h, Some(&entete), Some(&pied)))
        .collect();
    let (finale, _, _) = derouler(&images, &[], sans_limite());
    assert_eq!(finale.hauteur, 20 + 4000 + 30);
    assert_eq!(lignes(&finale, 0, 20), &entete.rgba[..]);
    assert_eq!(lignes(&finale, 20, 4000), &page.rgba[..]);
    assert_eq!(lignes(&finale, 4020, 30), &pied.rgba[..]);
}

#[test]
fn apercu_le_bas_reduit() {
    let page = page_texte(400, 1800, 10, 390, 19);
    let pied = bande(400, 30, 90);
    let h = 300;
    let pos = positions(page.hauteur, h - 30, 135);
    let images: Vec<Pixels> = pos
        .iter()
        .map(|p| zone(&page, *p, h, None, Some(&pied)))
        .collect();
    let mut a = Assemblage::nouveau(&images[0], sans_limite()).unwrap();
    for k in 1..images.len() {
        let m = mesurer(
            &images[k - 1],
            &images[k],
            a.fixes(),
            Attendu::Aucun,
            &TOLERANCE,
        );
        a.ajouter(&images[k], &m);
    }
    assert_eq!(a.hauteur(), 1830);
    // Pleine largeur : le bas exact, pied compris.
    let p = a.apercu(400, 200);
    assert_eq!((p.largeur, p.hauteur), (400, 200));
    assert_eq!(lignes(&p, 170, 30), &pied.rgba[..]);
    assert_eq!(lignes(&p, 0, 170), lignes(&page, 1800 - 170, 170));
    // Réduit de moitié : 200 de large, 300 lignes au plus, venues des 600 du bas.
    let p = a.apercu(200, 300);
    assert_eq!((p.largeur, p.hauteur), (200, 300));
    let (attendu, vu) = (pixel(&pied, 0, 28), pixel(&p, 0, 299));
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
    let page = page_texte(400, 600, 10, 390, 20);
    let a = Assemblage::nouveau(&z(&page, 0, 300), sans_limite()).unwrap();
    assert!(a.finir(&z(&page, 0, 200)).is_err());
}

// ---------------------------------------------------------------------------------------
// Budget (contrat §9) : à jouer en release.

#[test]
#[ignore = "budget : cargo test --release --lib -- --ignored budget --nocapture"]
fn budget_de_la_mesure_et_de_l_apercu() {
    let t = &TOLERANCE;
    for (l, h) in [(1024u32, 700u32), (1920, 1000), (2560, 1300)] {
        let page = page_texte(l, 7000, l / 10, l - l / 10, 21);
        let (mut avance, mut avec, mut immobile, mut perdue, mut ajout) =
            (vec![], vec![], vec![], vec![], vec![]);
        for k in 0..15u32 {
            let a = z(&page, k * 300, h);
            let d = 200 + (k * 37) % 250;
            let b = z(&page, k * 300 + d, h);
            let debut = Instant::now();
            assert_eq!(
                mesurer(&a, &b, Fixes::default(), Attendu::Aucun, t),
                Mesure::Avance(d)
            );
            avance.push(debut.elapsed());
            let debut = Instant::now();
            assert_eq!(
                mesurer(&a, &b, Fixes::default(), Attendu::Lu(d as i32), t),
                Mesure::Avance(d)
            );
            avec.push(debut.elapsed());
            let debut = Instant::now();
            assert_eq!(
                mesurer(&a, &a, Fixes::default(), Attendu::Aucun, t),
                Mesure::Immobile
            );
            immobile.push(debut.elapsed());
            let c = z(&page, k * 300 + h + 10, h);
            let debut = Instant::now();
            assert_eq!(
                mesurer(&a, &c, Fixes::default(), Attendu::Aucun, t),
                Mesure::Perdue
            );
            perdue.push(debut.elapsed());
            let mut asm = Assemblage::nouveau(&a, sans_limite()).unwrap();
            let debut = Instant::now();
            asm.ajouter(&b, &Mesure::Avance(d));
            ajout.push(debut.elapsed());
        }
        noter(&format!("{l}x{h} avance"), &avance);
        noter(&format!("{l}x{h} avance, attendu lu"), &avec);
        noter(&format!("{l}x{h} immobile"), &immobile);
        noter(&format!("{l}x{h} perdue"), &perdue);
        noter(&format!("{l}x{h} ajouter"), &ajout);
        for d in [&avance, &avec, &immobile, &perdue] {
            assert!(
                cfg!(debug_assertions) || mediane(d) <= Duration::from_millis(15),
                "{l}x{h}"
            );
        }
    }
    // Aperçu d'un assemblage de 2 560 de large.
    let page = page_texte(2560, 6000, 256, 2304, 22);
    let pos = positions(6000, 1300, 600);
    let images: Vec<Pixels> = pos.iter().map(|p| z(&page, *p, 1300)).collect();
    let mut a = Assemblage::nouveau(&images[0], sans_limite()).unwrap();
    for k in 1..images.len() {
        a.ajouter(&images[k], &Mesure::Avance(pos[k] - pos[k - 1]));
    }
    for (al, ah) in [(400u32, 200u32), (320, 480), (600, 900)] {
        let durees: Vec<Duration> = (0..5)
            .map(|_| {
                let debut = Instant::now();
                std::hint::black_box(a.apercu(al, ah));
                debut.elapsed()
            })
            .collect();
        noter(&format!("apercu {al}x{ah} de 2560"), &durees);
        assert!(cfg!(debug_assertions) || mediane(&durees) <= Duration::from_millis(10));
    }
}

#[test]
fn liste_large_lue_une_colonne_sur_pas() {
    // La même liste dans une zone large : une colonne sur deux, trois ou quatre est lue
    // (jamais des suites de colonnes, qui cacheraient le chiffre qui change) ; un lu faux
    // d'une ligne de liste ne choisit pas une ligne voisine.
    let t = &TOLERANCE;
    for l in [1100u32, 1600, 2560] {
        let page = page_liste(l, 200, 7);
        let a = z(&page, 440, 552);
        for (d, e) in [
            (84u32, Attendu::Aucun),
            (84, Attendu::Lu(84)),
            (66, Attendu::Lu(66)),
            (66, Attendu::Aucun),
        ] {
            let b = z(&page, 440 + d, 552);
            assert_eq!(
                mesurer(&a, &b, Fixes::default(), e, t),
                Mesure::Avance(d),
                "{l} {d} {e:?}"
            );
        }
        for (d, e) in [(84u32, 28), (84, 62), (84, 106), (84, 64), (66, 88)] {
            let b = z(&page, 440 + d, 552);
            let m = mesurer(&a, &b, Fixes::default(), Attendu::Lu(e), t);
            assert!(
                m == Mesure::Avance(d) || m == Mesure::Ambigue,
                "{l} {d} {e} {m:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// Seconde relecture (R1 à R5)

/// Enchaîné comme D (`suite_auto`) : chaque image mesurée contre `derniere()`, avec le
/// lu exact (la position vraie depuis la dernière image gardée) ou sans attendu. Rend
/// l'image finale et les mesures (vrai, mesure).
fn comme_d(images: &[Pixels], pos: &[u32], avec_lu: bool) -> (Pixels, Vec<(i64, Mesure)>) {
    let mut a = Assemblage::nouveau(&images[0], sans_limite()).unwrap();
    let mut pg = pos[0];
    let mut vues = Vec::new();
    for k in 1..images.len() {
        let vrai = i64::from(pos[k]) - i64::from(pg);
        let e = if avec_lu {
            Attendu::Lu(vrai as i32)
        } else {
            Attendu::Aucun
        };
        let m = mesurer(a.derniere(), &images[k], a.fixes(), e, &TOLERANCE);
        vues.push((vrai, m));
        if !matches!(a.ajouter(&images[k], &m), Ajout::Rien) {
            pg = pos[k];
        }
    }
    let derniere = a.derniere().clone();
    (a.finir(&derniere).unwrap(), vues)
}

/// Les lignes de `vu` qui diffèrent de la page (même largeur).
fn lignes_differentes(vu: &Pixels, page: &Pixels) -> usize {
    let r = (vu.largeur * 4) as usize;
    (0..vu.hauteur.min(page.hauteur) as usize)
        .filter(|y| vu.rgba[y * r..(y + 1) * r] != page.rgba[y * r..(y + 1) * r])
        .count()
}

#[test]
fn r1_page_clairsemee_de_bout_en_bout() {
    // Réglages aux grands blancs, plus un blanc de 700 lignes (plus haut que la zone) :
    // avec le lu exact, la page entière, chaque mesure juste (deux images égales dans le
    // blanc : `Immobile`, la suivante rattrape) ; sans lu, jamais une mesure fausse, et
    // ce qui est assemblé est le haut de la page. Limite : après un `Immobile` dans le
    // blanc, la dernière image gardée reste ; si le décalage lu depuis elle dépasse la
    // portée (zone 400, pas 200 : 400 > 352), `Perdue` (D doit réduire le pas).
    let mut page = page_reglages(900, 5000, 6);
    rectangle(&mut page, 0, 2000, 900, 700, [249, 249, 249]);
    for (h, pas, complet) in [
        (400u32, 150u32, true),
        (600, 300, true),
        (500, 120, true),
        (400, 340, true),
        (400, 200, false),
    ] {
        let pos = positions(page.hauteur, h, pas);
        let images: Vec<Pixels> = pos.iter().map(|p| z(&page, *p, h)).collect();
        for avec_lu in [true, false] {
            let (finale, vues) = comme_d(&images, &pos, avec_lu);
            let mut justes = 0;
            for (k, (vrai, m)) in vues.iter().enumerate() {
                // Deux images égales (le blanc plus haut que la zone) : `Immobile` est juste.
                let pareilles = images[k + 1] == z(&page, pos[k + 1] - *vrai as u32, h);
                if pareilles && *m == Mesure::Immobile {
                    continue;
                }
                justes += usize::from(jamais_fausse(*m, *vrai, "page clairsemée"));
                if avec_lu && *vrai <= i64::from(h - RECOUVREMENT_MIN) {
                    assert_eq!(*m, mesure_de(*vrai as i32), "{h} {pas} {k}");
                }
            }
            eprintln!(
                "r1 zone {h} pas {pas} lu {avec_lu} : {justes} avances justes sur {}, hauteur {}",
                vues.len(),
                finale.hauteur
            );
            egales(&finale, &z(&page, 0, finale.hauteur), "page clairsemée");
            if avec_lu {
                assert_eq!(finale.hauteur == page.hauteur, complet, "{h} {pas}");
            }
        }
    }
}

/// Une barre latérale (160 px) d'un gris presque uni, et un bloc de texte de 30 lignes
/// toutes les 260 lignes ; `trame` : ± `trame` sur la barre, fixe dans la page.
fn page_barre(trame: i32) -> Pixels {
    let texte = page_texte(800, 4000, 200, 780, 21);
    let mut page = unie(800, 4000, BLANC);
    let mut hz = Hasard(9);
    for y in 0..4000 {
        for x in 0..800 {
            let c = if x < 160 {
                let v = if trame > 0 {
                    hz.entre(-trame, trame)
                } else {
                    0
                };
                [200, 205, 215].map(|c: i32| (c + v) as u8)
            } else if y % 260 < 30 && x >= 200 {
                pixel(&texte, x, y)
            } else {
                BLANC
            };
            poser(&mut page, x, y, c);
        }
    }
    page
}

#[test]
fn r2_bruit_et_trame_sur_une_barre_laterale() {
    // Un bruit de ± 8 sur la nouvelle image, ou une trame de ± 6 sur la barre, ne fait
    // pas des lignes porteuses : jamais une fausse avance, avec ou sans lu exact.
    let t = &TOLERANCE;
    let h = 500;
    for trame in [0, 6] {
        let page = page_barre(trame);
        for p in (0..3400u32).step_by(519) {
            for d in [0i64, 7, 60, 150, 260, 400, 440, 480, 900, -100] {
                let q = i64::from(p) + d;
                if q < 0 || q + i64::from(h) > 4000 {
                    continue;
                }
                let a = z(&page, p, h);
                let mut b = z(&page, q as u32, h);
                if trame == 0 {
                    let mut hz = Hasard((i64::from(p) * 7 + d + 1000) as u64);
                    for c in b.rgba.chunks_exact_mut(4) {
                        for v in &mut c[..3] {
                            *v = (i32::from(*v) + hz.entre(-8, 8)).clamp(0, 255) as u8;
                        }
                    }
                }
                for e in [Attendu::Aucun, Attendu::Lu(d as i32)] {
                    jamais_fausse(mesurer(&a, &b, Fixes::default(), e, t), d, "barre");
                }
            }
        }
    }
    // La trame seule : l'avance vraie est trouvée, et un recouvrement blanc n'est pas
    // « perdu » (la barre tramée s'accorderait).
    let page = page_barre(6);
    let m = |p: u32, d: u32, e: Attendu| {
        mesurer(&z(&page, p, h), &z(&page, p + d, h), Fixes::default(), e, t)
    };
    assert_eq!(m(346, 150, Attendu::Aucun), Mesure::Avance(150));
    assert_eq!(m(0, 400, Attendu::Aucun), Mesure::Ambigue);
    assert_eq!(m(0, 400, Attendu::Lu(400)), Mesure::Avance(400));
}

#[test]
fn r3_liste_hors_de_portee() {
    // Une liste aux noms proches dont le vrai décalage dépasse la portée : jamais une copie
    // du motif (ni avance ni recul), sans attendu ou avec le lu (hors de portée lui aussi).
    let t = &TOLERANCE;
    for l in [524u32, 2560] {
        let page = page_liste(l, 180, 7);
        let h = 500;
        for p in (0..page.hauteur - 1500).step_by(346) {
            let a = z(&page, p, h);
            for d in [460u32, 480, 900, 1400] {
                if p + d + h > page.hauteur {
                    continue;
                }
                let b = z(&page, p + d, h);
                for e in [Attendu::Aucun, Attendu::Lu(d as i32)] {
                    let m = mesurer(&a, &b, Fixes::default(), e, t);
                    assert!(
                        matches!(m, Mesure::Ambigue | Mesure::Perdue),
                        "{l} {p} {d} {e:?} : {m:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn r4_page_immobile_aux_ecarts_wpf() {
    // Quatre zones de 32 × 8 assombries de 14 sur une page immobile : `Immobile`, sans
    // attendu et avec le lu 0 (jamais une avance de 1 ou 2 lignes).
    let t = &TOLERANCE;
    for graine in 0..3u32 {
        let page = page_texte(800, 3000, 40, 760, 30 + graine);
        for p in (0..2400u32).step_by(317) {
            let a = z(&page, p, 500);
            let mut b = a.clone();
            let mut hz = Hasard(u64::from(p + graine));
            for _ in 0..4 {
                let (x0, y0) = (hz.entre(0, 760) as u32, hz.entre(0, 490) as u32);
                for y in y0..y0 + 8 {
                    for x in x0..x0 + 32 {
                        let c = pixel(&b, x, y).map(|v| v.saturating_sub(14));
                        poser(&mut b, x, y, c);
                    }
                }
            }
            for e in [Attendu::Aucun, Attendu::Lu(0)] {
                assert_eq!(
                    mesurer(&a, &b, Fixes::default(), e, t),
                    Mesure::Immobile,
                    "{graine} {p} {e:?}"
                );
            }
        }
    }
}

#[test]
fn r5_la_ligne_survolee_au_plus_une_fois() {
    // Le pointeur reste sur la liste : la ligne survolée (17 lignes inversées) ou
    // sélectionnée (22 lignes teintées) à place fixe dans chaque image. L'image finale
    // est la page, sauf au plus une bande.
    let page = page_texte(524, 2400, 20, 500, 4);
    let h = 552;
    for (y0, pas) in [
        (100u32, 150u32),
        (40, 84),
        (280, 276),
        (100, 276),
        (300, 200),
    ] {
        for inverser in [true, false] {
            let pos = positions(page.hauteur, h, pas);
            let images: Vec<Pixels> = pos
                .iter()
                .map(|p| {
                    let mut im = z(&page, *p, h);
                    if inverser {
                        survoler(&mut im, y0, 17);
                    } else {
                        teinter(&mut im, y0, 22);
                    }
                    im
                })
                .collect();
            let (finale, mesures, _) = derouler(&images, &[], sans_limite());
            let avances = mesures
                .iter()
                .filter(|m| matches!(m, Mesure::Avance(_)))
                .count();
            let n = lignes_differentes(&finale, &page);
            eprintln!(
                "r5 y {y0} pas {pas} inverser {inverser} : {avances} avances sur {}, {n} lignes différentes de la page",
                mesures.len()
            );
            assert_eq!(finale.hauteur, page.hauteur, "{y0} {pas} {inverser}");
            assert!(
                n <= if inverser { 17 } else { 22 },
                "{y0} {pas} {inverser} : {n}"
            );
        }
    }
}
