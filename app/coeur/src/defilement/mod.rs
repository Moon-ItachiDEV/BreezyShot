//! Assemblage de la capture défilante (contrat 0.6, §13.2, D10 à D13) : mesurer le
//! décalage entre deux images de la zone, reconnaître les bandes fixes (en-tête, pied),
//! ajouter les lignes nouvelles, rendre l'image finale. Module pur : il n'utilise que
//! `systeme` (le type `Pixels`), ne lit ni n'écrit rien, ne connaît ni Tauri ni le temps.
//!
//! Tolérance retenue après la sonde (§18) : ± 8 par canal, une ligne est égale si 90 % de
//! ses pixels le sont, un décalage est accepté si 97 % des lignes du recouvrement le sont ;
//! 1 à 4 lignes entières très différentes sont tolérées au bord du recouvrement (échelles
//! 125 et 150 %).
//!
//! En plus, pour l'Explorateur (sonde S3, 0 paire sur 4) : une bande d'au plus
//! `BANDE_MAX` lignes **à place fixe dans la zone** qui change de couleur (la ligne
//! survolée sous le pointeur, une ligne sélectionnée, un carré animé) est tolérée. Elle se
//! voit, dans le recouvrement, à la place `r` (dans la nouvelle image) et à `r - d` (dans
//! l'ancienne) : seules ces deux fenêtres sont retirées du compte, jamais des lignes
//! éparses.
//!
//! Comment la mesure reste dans son budget (15 ms pour 1 024 × 700) : chaque ligne est
//! résumée par 16 moyennes de gris ; tous les décalages permis sont classés sur ces
//! résumés (quelques millions d'opérations sur des octets), puis seuls les
//! `CANDIDATS` meilleurs sont comparés pixel à pixel selon la tolérance. Avec un décalage
//! attendu, la recherche se borne d'abord autour de lui (et de 0) ; elle ne s'élargit à
//! tout le recouvrement permis que si rien n'y est retenu.

mod assemblage;

pub use assemblage::Assemblage;

use crate::systeme::gel::Pixels;

/// La comparaison tolérante de deux images (D12).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tolerance {
    /// Écart permis par canal (rouge, vert, bleu).
    pub canal: u8,
    /// Part des pixels d'une ligne qui doivent être égaux pour que la ligne le soit.
    pub part_ligne: f32,
    /// Part des lignes utiles du recouvrement qui doivent être égales pour retenir un
    /// décalage.
    pub part_lignes: f32,
}

/// La tolérance de la capture défilante (D12, §18).
pub const TOLERANCE: Tolerance = Tolerance {
    canal: 8,
    part_ligne: 0.90,
    part_lignes: 0.97,
};

/// Recouvrement minimal entre deux images pour mesurer un décalage, en pixels (D10) ; un
/// quart de la hauteur utile si c'est moins.
pub const RECOUVREMENT_MIN: u32 = 48;

/// Lignes « qui portent quelque chose » qu'un recouvrement doit avoir, égales, pour qu'un
/// décalage soit retenu (D10) ; moins : `Ambigue`.
const PORTEUSES_MIN: u32 = 16;

/// Une bande fixe a au moins ce nombre de lignes (D11).
const FIXES_MIN: u32 = 8;

/// Lignes de chaque bord du recouvrement qui ne comptent pas (§18 : 1 à 4 lignes entières
/// très différentes au bord, échelles 125 et 150 %).
const LIGNES_DE_BORD: usize = 4;

/// Hauteur maximale d'une bande à place fixe qui change de couleur (ligne survolée de
/// l'Explorateur : 17 lignes à 100 %, environ 33 à 150 % ; marge comprise).
const BANDE_MAX: usize = 64;

/// Décalages comparés pixel à pixel après le classement sur les résumés.
const CANDIDATS: usize = 8;

/// Décalages comparés pixel à pixel parmi ceux dont le recouvrement est presque uni.
const CANDIDATS_UNIS: usize = 4;

/// Moyennes par ligne du résumé.
const BLOCS: usize = 16;

/// Une ligne porte quelque chose si un de ses canaux varie de plus de ce multiple de
/// l'écart permis (un bruit de ± l'écart permis ne suffit pas).
const PORTEUSE_FOIS_CANAL: u16 = 3;

/// Ce que vaut une image contre la dernière image gardée (D10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mesure {
    /// La page est descendue de `n` lignes : `n` lignes nouvelles en bas.
    Avance(u32),
    /// Rien n'a bougé.
    Immobile,
    /// La page est remontée de `n` lignes : rien n'est ajouté.
    Recul(u32),
    /// Aucun recouvrement suffisant : la page est allée trop loin (« Trop vite »).
    Perdue,
    /// Le recouvrement ne porte pas assez de lignes distinctes (page unie) : rien n'est
    /// ajouté, l'image suivante décidera.
    Ambigue,
}

/// Les bandes fixes de la zone (D11) : lignes du haut et du bas qui ne défilent pas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Fixes {
    pub haut: u32,
    pub bas: u32,
}

/// Taille maximale de l'image assemblée (D13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limite {
    /// Hauteur maximale, en pixels.
    pub hauteur: u32,
    /// Nombre maximal de pixels (largeur × hauteur).
    pub pixels: u64,
}

/// Ce qu'a fait un ajout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ajout {
    /// `n` lignes ajoutées.
    Ajoutees(u32),
    /// Rien d'ajouté (immobile, recul, perdue, ambiguë).
    Rien,
    /// La limite est atteinte : les `n` lignes qui tenaient sont ajoutées, la capture finit.
    Limite(u32),
}

/// Des pixels cohérents, vus ligne à ligne.
#[derive(Clone, Copy)]
struct Vue<'a> {
    l: usize,
    h: usize,
    rgba: &'a [u8],
}

impl<'a> Vue<'a> {
    fn de(p: &'a Pixels) -> Option<Vue<'a>> {
        let (l, h) = (p.largeur as usize, p.hauteur as usize);
        let attendu = l.checked_mul(h).and_then(|n| n.checked_mul(4));
        (attendu == Some(p.rgba.len()) && l > 0 && h > 0).then_some(Vue {
            l,
            h,
            rgba: &p.rgba,
        })
    }

    fn ligne(&self, y: usize) -> &'a [u8] {
        &self.rgba[y * self.l * 4..(y + 1) * self.l * 4]
    }
}

/// Le résumé des lignes d'une image.
struct Lignes {
    /// 16 moyennes de gris par ligne.
    resume: Vec<[u8; BLOCS]>,
    /// La ligne porte quelque chose (pas unie).
    porteuse: Vec<bool>,
    /// Les numéros des lignes porteuses, dans l'ordre.
    porteuses: Vec<u32>,
}

fn decrire(v: Vue, t: &Tolerance) -> Lignes {
    let blocs = BLOCS.min(v.l);
    let seuil = u16::from(t.canal) * PORTEUSE_FOIS_CANAL;
    let mut resume = Vec::with_capacity(v.h);
    let mut porteuse = Vec::with_capacity(v.h);
    let mut porteuses = Vec::new();
    for y in 0..v.h {
        let ligne = v.ligne(y);
        let mut r = [0u8; BLOCS];
        let (mut bas, mut haut) = ([255u8; 3], [0u8; 3]);
        for (k, case) in r.iter_mut().enumerate().take(blocs) {
            let (x0, x1) = (k * v.l / blocs, (k + 1) * v.l / blocs);
            let mut somme = 0u32;
            for p in ligne[x0 * 4..x1 * 4].chunks_exact(4) {
                somme += u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2]);
                for c in 0..3 {
                    bas[c] = bas[c].min(p[c]);
                    haut[c] = haut[c].max(p[c]);
                }
            }
            *case = (somme / (3 * (x1 - x0).max(1) as u32)) as u8;
        }
        resume.push(r);
        let porte = (0..3).any(|c| u16::from(haut[c] - bas[c]) > seuil);
        porteuse.push(porte);
        if porte {
            porteuses.push(y as u32);
        }
    }
    Lignes {
        resume,
        porteuse,
        porteuses,
    }
}

/// Deux lignes de même largeur sont égales (D12) : `part_ligne` de leurs pixels au moins
/// le sont à ± `canal` sur chaque canal de couleur.
fn egales(a: &[u8], b: &[u8], t: &Tolerance) -> bool {
    let n = a.len() / 4;
    let requis = ((f64::from(t.part_ligne) * n as f64).ceil() as usize).min(n);
    let permis = n - requis;
    let c = t.canal;
    let mut mauvais = 0usize;
    for (ca, cb) in a.chunks(256).zip(b.chunks(256)) {
        mauvais += ca
            .chunks_exact(4)
            .zip(cb.chunks_exact(4))
            .filter(|(p, q)| {
                p[0].abs_diff(q[0]) > c || p[1].abs_diff(q[1]) > c || p[2].abs_diff(q[2]) > c
            })
            .count();
        if mauvais > permis {
            return false;
        }
    }
    true
}

/// Les deux images comparées et leurs résumés.
struct Contexte<'a> {
    a: Vue<'a>,
    b: Vue<'a>,
    la: &'a Lignes,
    lb: &'a Lignes,
    t: &'a Tolerance,
}

/// Le résultat de la comparaison pixel à pixel d'un décalage retenu.
#[derive(Debug, Clone, Copy)]
struct Verdict {
    d: i32,
    /// Lignes différentes (bords exclus).
    mauvaises: usize,
    /// Lignes comptées (recouvrement moins les bords).
    utiles: usize,
    /// Lignes porteuses égales.
    porteuses: u32,
    /// Écart moyen des résumés des lignes porteuses (départage des voisins, page lisse).
    note: u64,
}

/// Les lignes de `b` (la nouvelle image) qui recouvrent `a` décalée de `d`, hors des
/// bandes `haut` et `bas` : la ligne `y` de `b` montre la ligne `y + d` de `a`.
fn recouvrement(h: usize, haut: u32, bas: u32, d: i32) -> (i64, i64) {
    let (h, haut, bas, d) = (h as i64, i64::from(haut), i64::from(bas), i64::from(d));
    (haut.max(haut - d), (h - bas).min(h - bas - d))
}

fn recouvrement_min(hauteur_utile: u32) -> u32 {
    RECOUVREMENT_MIN.min(hauteur_utile / 4).max(1)
}

/// Compare `b` à `a` décalée de `d`, ligne à ligne (D12, bords et bande fixe tolérés).
/// `None` : décalage refusé ou recouvrement trop court.
fn verifier(cx: &Contexte, haut: u32, bas: u32, d: i32, rec: u32) -> Option<Verdict> {
    let (y0, y1) = recouvrement(cx.b.h, haut, bas, d);
    if y1 - y0 < i64::from(rec) || y1 <= y0 {
        return None;
    }
    let n = (y1 - y0) as usize;
    let bord = if n >= 2 * LIGNES_DE_BORD + PORTEUSES_MIN as usize {
        LIGNES_DE_BORD
    } else {
        0
    };
    let (i0, i1) = (y0 as usize + bord, y1 as usize - bord);
    let utiles = i1 - i0;
    let permises = ((1.0 - f64::from(cx.t.part_lignes)) * utiles as f64 + 1e-6).floor() as usize;
    let sauvables = (2 * (BANDE_MAX + 4)).min(utiles / 4);
    let mut masque = vec![false; utiles];
    let (mut mauvaises, mut porteuses) = (0usize, 0u32);
    for (i, y) in (i0..i1).enumerate() {
        let ya = (y as i64 + i64::from(d)) as usize;
        if egales(cx.b.ligne(y), cx.a.ligne(ya), cx.t) {
            porteuses += u32::from(cx.lb.porteuse[y]);
        } else {
            masque[i] = true;
            mauvaises += 1;
            if mauvaises > permises + sauvables {
                return None;
            }
        }
    }
    let v = Verdict {
        d,
        mauvaises,
        utiles,
        porteuses,
        note: noter(cx, y0, y1, d).0,
    };
    if mauvaises <= permises || bande_a_place_fixe(&masque, d, permises, utiles / 4) {
        Some(v)
    } else {
        None
    }
}

/// Les lignes différentes (`masque`, recouvrement vu depuis la nouvelle image)
/// s'expliquent-elles, à `permises` près, par une seule bande d'au plus `BANDE_MAX`
/// lignes à place fixe dans la zone ? Elle se voit à `r` (nouvelle image) et à `r - d`
/// (ancienne image), et ne retire pas plus de `max_retirees` lignes.
fn bande_a_place_fixe(masque: &[bool], d: i32, permises: usize, max_retirees: usize) -> bool {
    let n = masque.len();
    let mut cumul = Vec::with_capacity(n + 1);
    cumul.push(0usize);
    for &m in masque {
        cumul.push(cumul.last().copied().unwrap_or(0) + usize::from(m));
    }
    let total = cumul[n];
    let compte = |a: i64, b: i64| {
        let (a, b) = (a.clamp(0, n as i64) as usize, b.clamp(0, n as i64) as usize);
        if b > a {
            cumul[b] - cumul[a]
        } else {
            0
        }
    };
    let d = i64::from(d);
    let mut i = 0;
    while i < n {
        if !masque[i] {
            i += 1;
            continue;
        }
        let s = i;
        while i < n && masque[i] {
            i += 1;
        }
        let long = i - s;
        if long > BANDE_MAX {
            continue;
        }
        // La suite vue est l'image de la bande dans la nouvelle image (r = s) ou dans
        // l'ancienne (r - d = s).
        for r in [s as i64, s as i64 + d] {
            let (a1, b1) = (r - 2, r + long as i64 + 2);
            let (a2, b2) = (a1 - d, b1 - d);
            let mut retirees = compte(a1, b1) + compte(a2, b2);
            let (ia, ib) = (a1.max(a2), b1.min(b2));
            if ia < ib {
                retirees -= compte(ia, ib);
            }
            if retirees <= max_retirees && total - retirees <= permises {
                return true;
            }
        }
    }
    false
}

/// Les suites de lignes égales à la même place, en haut et en bas, qui comptent comme
/// bandes fixes : au moins `FIXES_MIN` lignes, pas toutes unies.
fn bandes_immobiles(cx: &Contexte, rogner_bas: bool) -> (u32, u32) {
    let h = cx.b.h;
    let meme = |y: usize| egales(cx.b.ligne(y), cx.a.ligne(y), cx.t);
    let mut haut = 0;
    while haut < h && meme(haut) {
        haut += 1;
    }
    let mut bas = 0;
    while bas < h - haut && meme(h - 1 - bas) {
        bas += 1;
    }
    // Une ligne prouve la bande si elle porte quelque chose dans les deux images (une
    // ligne presque unie peut passer la tolérance contre une ligne unie sans être fixe).
    let preuve = |y: &usize| cx.la.porteuse[*y] && cx.lb.porteuse[*y];
    // L'en-tête s'arrête à sa dernière ligne prouvée (moins d'en-tête ne perd rien : les
    // lignes nouvelles entrent par le bas).
    let haut = (0..haut).rev().find(preuve).map_or(0, |y| y + 1);
    // Le pied commence à sa première ligne prouvée pour la mesure ; pour l'assemblage il
    // garde ses lignes unies du haut (un pied trop court se répéterait à chaque pas).
    let bas = match (h - bas..h).find(preuve) {
        Some(y) if rogner_bas => h - y,
        Some(_) => bas,
        None => 0,
    };
    let garde = |n: usize| if n >= FIXES_MIN as usize { n as u32 } else { 0 };
    (garde(haut), garde(bas))
}

fn distance(a: &[u8; BLOCS], b: &[u8; BLOCS]) -> u32 {
    a.iter()
        .zip(b)
        .map(|(x, y)| u32::from(x.abs_diff(*y)))
        .sum()
}

/// Garde les `n` plus petits `(note, d)`.
fn garder(liste: &mut Vec<(u64, i32)>, n: usize, note: u64, d: i32) {
    if liste.len() == n && liste.last().is_some_and(|(pire, _)| *pire <= note) {
        return;
    }
    let place = liste.partition_point(|(x, _)| *x <= note);
    liste.insert(place, (note, d));
    liste.truncate(n);
}

/// L'écart moyen (× 1 024) des résumés des lignes porteuses de `b` dans `y0..y1` contre
/// celles de `a` décalées de `d`, et leur nombre.
fn noter(cx: &Contexte, y0: i64, y1: i64, d: i32) -> (u64, u64) {
    const PLAFOND: u32 = 24 * BLOCS as u32;
    let porteuses = &cx.lb.porteuses;
    let debut = porteuses.partition_point(|&y| i64::from(y) < y0);
    let fin = porteuses.partition_point(|&y| i64::from(y) < y1);
    let mut somme = 0u64;
    for &y in &porteuses[debut..fin] {
        let ya = (i64::from(y) + i64::from(d)) as usize;
        somme += u64::from(distance(&cx.lb.resume[y as usize], &cx.la.resume[ya]).min(PLAFOND));
    }
    let np = (fin - debut) as u64;
    ((somme * 1024).checked_div(np).unwrap_or(0), np)
}

/// Ce qu'une recherche a vu.
#[derive(Default)]
struct Recherche {
    retenus: Vec<Verdict>,
    /// Un décalage au moins avait assez de lignes porteuses pour juger.
    jugeable: bool,
    /// Un décalage presque uni passe la tolérance : on ne peut pas dire « perdue ».
    unie_possible: bool,
}

/// Cherche les décalages de `lo..=hi` (0 et `sauf` exclus) qui passent la tolérance.
#[allow(clippy::too_many_arguments)]
fn chercher(
    cx: &Contexte,
    haut: u32,
    bas: u32,
    rec: u32,
    (lo, hi): (i32, i32),
    sauf: Option<(i32, i32)>,
    attendu: Option<i32>,
    vu: &mut Recherche,
) {
    let mut meilleurs: Vec<(u64, i32)> = Vec::with_capacity(CANDIDATS + 1);
    let mut unis: Vec<(u64, i32)> = Vec::with_capacity(CANDIDATS_UNIS + 1);
    for d in lo..=hi {
        if d == 0 || sauf.is_some_and(|(a, b)| (a..=b).contains(&d)) {
            continue;
        }
        let (y0, y1) = recouvrement(cx.b.h, haut, bas, d);
        if y1 - y0 < i64::from(rec) {
            continue;
        }
        let (note, np) = noter(cx, y0, y1, d);
        if np >= u64::from(PORTEUSES_MIN) {
            garder(&mut meilleurs, CANDIDATS, note, d);
        } else {
            garder(&mut unis, CANDIDATS_UNIS, note, d);
        }
    }
    if let Some(e) = attendu.filter(|e| *e != 0 && (lo..=hi).contains(e)) {
        if !meilleurs.iter().any(|(_, d)| *d == e) && !unis.iter().any(|(_, d)| *d == e) {
            meilleurs.push((0, e));
        }
    }
    for &(_, d) in &meilleurs {
        match verifier(cx, haut, bas, d, rec) {
            Some(v) if v.porteuses >= PORTEUSES_MIN => {
                vu.jugeable = true;
                vu.retenus.push(v);
            }
            Some(_) => vu.unie_possible = true,
            None => vu.jugeable = true,
        }
    }
    for &(_, d) in &unis {
        if verifier(cx, haut, bas, d, rec).is_some() {
            vu.unie_possible = true;
        }
    }
}

fn mesure_de(d: i32) -> Mesure {
    match d {
        0 => Mesure::Immobile,
        d if d > 0 => Mesure::Avance(d as u32),
        d => Mesure::Recul(d.unsigned_abs()),
    }
}

/// Mesure `apres` contre `avant` (même taille), hors des bandes `fixes` ; `attendu` : le
/// décalage attendu du pas, s'il est connu (départage une mesure ambiguë).
///
/// Sans bandes données (avant la première `Avance`), les lignes du haut et du bas égales
/// à la même place dans les deux images (au moins 8, pas toutes unies) sont mises de côté
/// pour la mesure, comme des bandes fixes. Des images de tailles différentes ou
/// incohérentes : `Perdue`.
pub fn mesurer(
    avant: &Pixels,
    apres: &Pixels,
    fixes: Fixes,
    attendu: Option<i32>,
    t: &Tolerance,
) -> Mesure {
    let (Some(a), Some(b)) = (Vue::de(avant), Vue::de(apres)) else {
        return Mesure::Perdue;
    };
    if a.l != b.l || a.h != b.h {
        return Mesure::Perdue;
    }
    let h = b.h as u32;
    let fh = fixes.haut.min(h);
    let fb = fixes.bas.min(h - fh);
    let (la, lb) = (decrire(a, t), decrire(b, t));
    let cx = Contexte {
        a,
        b,
        la: &la,
        lb: &lb,
        t,
    };
    // Immobile : tout ce qui n'est pas une bande donnée est à la même place.
    let rec0 = recouvrement_min(h - fh - fb);
    if let Some(v) = verifier(&cx, fh, fb, 0, rec0) {
        if v.porteuses >= PORTEUSES_MIN {
            return Mesure::Immobile;
        }
    }
    let (ih, ib) = bandes_immobiles(&cx, true);
    let haut = fh.max(ih);
    let bas = fb.max(ib).min(h - haut);
    let utile = h - haut - bas;
    let rec = recouvrement_min(utile);
    // En dernier recours (rien de jugeable), le décalage attendu départage s'il passe la
    // tolérance (D10) ; sinon rien n'est inventé.
    let par_attendu = |e: Option<i32>| match e {
        Some(e) if verifier(&cx, haut, bas, e, rec).is_some() => mesure_de(e),
        _ => Mesure::Ambigue,
    };
    if utile <= rec || (lb.porteuses.len() as u32) < PORTEUSES_MIN {
        return par_attendu(attendu);
    }
    let dmax = (utile - rec) as i32;
    let tout = (-dmax, dmax);
    let fenetre = attendu.map(|e| {
        let marge = (e.abs() / 2).max(16);
        ((e.min(0) - marge).max(-dmax), (e.max(0) + marge).min(dmax))
    });
    let mut vu = Recherche::default();
    if let Some(f) = fenetre {
        chercher(&cx, haut, bas, rec, f, None, attendu, &mut vu);
    }
    if vu.retenus.is_empty() {
        chercher(&cx, haut, bas, rec, tout, fenetre, attendu, &mut vu);
    }
    if vu.retenus.is_empty() {
        return if vu.jugeable && !vu.unie_possible {
            Mesure::Perdue
        } else {
            par_attendu(attendu)
        };
    }
    decider(vu.retenus, attendu)
}

/// Choisit parmi les décalages retenus. Plusieurs groupes séparés (motif qui se répète) :
/// le décalage attendu départage, sinon `Ambigue`. Dans un groupe : le décalage attendu
/// s'il en est, sinon le plus petit écart des résumés (une page lisse passe la tolérance
/// à une ligne près), puis le moins de lignes différentes, puis le plus proche de
/// l'attendu, puis le plus petit.
fn decider(mut retenus: Vec<Verdict>, attendu: Option<i32>) -> Mesure {
    retenus.sort_by_key(|v| v.d);
    let mut groupes: Vec<Vec<Verdict>> = Vec::new();
    for v in retenus {
        match groupes.last_mut() {
            Some(g) if g.last().is_some_and(|p| v.d - p.d <= 2) => g.push(v),
            _ => groupes.push(vec![v]),
        }
    }
    let ecart = |g: &[Verdict], e: i32| {
        let (lo, hi) = (g[0].d, g[g.len() - 1].d);
        if e < lo {
            lo - e
        } else if e > hi {
            e - hi
        } else {
            0
        }
    };
    let groupe = match (groupes.len(), attendu) {
        (1, _) => &groupes[0],
        (_, Some(e)) => match groupes.iter().min_by_key(|g| ecart(g, e)) {
            Some(g) => g,
            None => return Mesure::Ambigue,
        },
        _ => return Mesure::Ambigue,
    };
    if let Some(e) = attendu {
        if groupe.iter().any(|v| v.d == e) {
            return mesure_de(e);
        }
    }
    groupe
        .iter()
        .min_by_key(|v| {
            (
                v.note,
                v.mauvaises * 1_000_000 / v.utiles.max(1),
                attendu.map_or(0, |e| (v.d - e).unsigned_abs()),
                v.d.unsigned_abs(),
            )
        })
        .map_or(Mesure::Ambigue, |v| mesure_de(v.d))
}

/// Les bandes fixes de deux images séparées par une `avance` (D11) : au moins 8 lignes
/// égales à la même place, pas toutes unies. Elles ne peuvent pas couvrir l'avance.
pub fn fixes(avant: &Pixels, apres: &Pixels, avance: u32, t: &Tolerance) -> Fixes {
    let (Some(a), Some(b)) = (Vue::de(avant), Vue::de(apres)) else {
        return Fixes::default();
    };
    if a.l != b.l || a.h != b.h {
        return Fixes::default();
    }
    let (la, lb) = (decrire(a, t), decrire(b, t));
    let cx = Contexte {
        a,
        b,
        la: &la,
        lb: &lb,
        t,
    };
    let (haut, bas) = bandes_immobiles(&cx, false);
    if u64::from(haut) + u64::from(bas) + u64::from(avance) >= b.h as u64 {
        return Fixes::default();
    }
    Fixes { haut, bas }
}

/// Les bandes après une nouvelle avance `d` (D11 : elles ne peuvent que rétrécir). Une
/// ligne quitte une bande quand elle n'est plus à la même place mais suit la page (égale à
/// la ligne `y + d` de l'image d'avant) ; une ligne qui change sans suivre la page (une
/// animation dans le pied) reste dans la bande.
fn retrecir(avant: &Pixels, apres: &Pixels, d: u32, f: Fixes, t: &Tolerance) -> Fixes {
    let (Some(a), Some(b)) = (Vue::de(avant), Vue::de(apres)) else {
        return f;
    };
    if a.l != b.l || a.h != b.h {
        return f;
    }
    let (h, d) = (b.h, d as usize);
    let bouge = |y: usize| {
        y + d < h && !egales(b.ligne(y), a.ligne(y), t) && egales(b.ligne(y), a.ligne(y + d), t)
    };
    let haut = (f.haut as usize).min(h);
    let bas = (f.bas as usize).min(h - haut);
    let nouveau_haut = (0..haut).find(|&y| bouge(y)).unwrap_or(haut);
    let nouveau_bas = (h - bas..h)
        .rev()
        .find(|&y| bouge(y))
        .map_or(bas, |y| h - 1 - y);
    Fixes {
        haut: nouveau_haut as u32,
        bas: nouveau_bas as u32,
    }
}

#[cfg(test)]
mod tests;
