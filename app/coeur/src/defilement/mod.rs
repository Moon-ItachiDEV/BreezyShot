//! Assemblage de la capture défilante (contrat 0.6, §13.2, D10 à D13) : mesurer le
//! décalage entre deux images de la zone, reconnaître les bandes fixes (en-tête, pied),
//! poser chaque image gardée à sa place, rendre l'image finale. Module pur : il n'utilise
//! que `systeme` (le type `Pixels`), ne lit ni n'écrit rien, ne connaît ni Tauri ni le temps.
//!
//! **Ce qui compte.** Une ligne n'est jugée que sur ses pixels qui portent de l'encre :
//! ceux qui s'écartent de plus de 3 × `canal` du fond de la ligne (la couleur la plus
//! fréquente). Deux lignes sont égales (D12) si 90 % des pixels d'encre de l'une ou de
//! l'autre (et au moins `DENOMINATEUR_MIN`) le sont à ± `canal` : une ligne presque
//! blanche n'est donc plus égale à une ligne de texte clairsemé. Une ligne est
//! **porteuse** si elle a de l'encre et que cette encre change de plus de 3 × `canal` (24)
//! à 3 lignes au-dessus ou au-dessous (information verticale ; un bruit ou une trame de
//! ± `canal` n'en est pas) : une ligne vide traversée par une barre latérale, tramée ou
//! non, un filet vertical ou le milieu d'un fût ne votent pour aucun décalage. Seules les
//! paires où l'une des deux lignes est porteuse comptent pour l'appui et la note.
//! Une bande à droite (`bande_droite`, la barre de défilement et son pouce, qui bougent
//! seuls) n'est jamais comparée.
//!
//! **Décision.** Tous les décalages permis (0 compris) sont classés sur les mêmes
//! critères, en trois passes de plus en plus fines : un profil d'une valeur par ligne
//! (tous les décalages), un résumé de 16 blocs (les `CANDIDATS_PROFIL` meilleurs, sur tout
//! le recouvrement et sur son milieu seul), puis pixel à pixel (0, les meilleurs, l'attendu
//! et quelques décalages à trop peu de lignes porteuses). Un décalage **passe** s'il a au
//! moins `PORTEUSES_MIN` lignes porteuses égales, au plus 3 % de lignes utiles différentes
//! (D12) et au plus 10 % de lignes porteuses différentes. Sa **note** est (écart + une
//! demi-ligne) / (appui + 1). Le meilleur qui passe n'est rendu que si chaque autre
//! décalage jugé, à plus de 2 lignes de lui, en est **séparé** : un quart de ligne d'écart
//! de plus et une note au moins double, ou bien il ne passe pas et a moins de la moitié de
//! son appui. Sinon `Ambigue`. Dans le groupe des voisins du meilleur (à 2 lignes l'un de
//! l'autre : page lisse, écarts de rendu type WPF), 0 l'emporte s'il passe : `Immobile`.
//! Rien ne passe : `Perdue`, ou `Ambigue` si un décalage n'est contredit par rien (trop
//! peu de lignes porteuses, bande teintée trop grande, recouvrements blancs non jugés).
//!
//! **Attendu.** Seule une position **lue** par l'application (`Attendu::Lu`) est passée,
//! jamais le pas commandé. Deux images égales à la même place donnent `Immobile` quel que
//! soit le lu, sauf entièrement unies (chaque ligne d'une couleur à `canal` près) : le lu
//! est alors rendu (dans la portée, sinon `Perdue`). Sinon, si le recouvrement au lu (à
//! `ATTENDU_A_PEU_PRES` lignes près s'il y a assez de lignes porteuses, sinon le lu seul ;
//! le lu d'abord) est cohérent (il passe, ou n'a aucune ligne différente : un blanc), il est
//! rendu même si d'autres décalages passent, sauf si l'un d'eux le bat nettement
//! (`Ambigue`, ou `Immobile` si c'est 0) ; sinon la mesure sans attendu, et `Ambigue` si
//! elle trouve une autre avance que le lu.
//!
//! **Ce qui est excusé**, pour un décalage donné, sans compter ni pour ni contre :
//! - les lignes des suites immobiles du haut et du bas (égales à la même place dans les
//!   deux images) qui ne suivent pas ce décalage : en-tête et pied, connus ou non ;
//! - une bande **teintée** (ligne survolée ou sélectionnée, ombre sous un en-tête) : le
//!   fond de la ligne change mais l'encre reste à sa place ; au plus deux suites de
//!   `BANDE_MAX` lignes qui sont la même bande vue dans les deux images (à place fixe : à
//!   `d` l'une de l'autre, ou ses deux bouts si elle se recouvre elle-même ; alignée sur
//!   les lignes d'une liste : à `d` près de moins que sa hauteur), la bande au plus 1/8
//!   du recouvrement ;
//! - au bord du recouvrement, une suite de lignes différentes (en-tête collant qui
//!   apparaît, bannière qui part) d'au plus `BORD_MAX` lignes, comptée à demi dans
//!   l'écart.
//!
//! Les bandes fixes de l'assemblage (D11) se déterminent après la mesure, sur les lignes
//! immobiles qui ne suivent pas le décalage trouvé (`fixes`). Au placement, une ligne déjà
//! posée que la nouvelle image ne montre que teintée (survol à place fixe) est gardée, et
//! une ligne posée teintée est remplacée : la bande ne reste qu'une fois au plus.
//!
//! **Budget** (15 ms de médiane jusqu'à 2 560 × 1 300) : au-delà de `COLONNES_VISEES`
//! colonnes, une colonne sur `pas` seulement est lue (2 à 1 024 de large, 3 à 1 920, 4 à
//! 2 560), jamais des suites de colonnes voisines (elles cacheraient le chiffre qui change
//! d'une ligne de liste à l'autre). Les calculs se font par paquets de 64 octets, sans
//! branche ; les tampons des images échantillonnées sont gardés d'une mesure à l'autre
//! dans le fil qui mesure (`TAMPONS_GARDES`).

mod assemblage;

use std::borrow::Cow;
use std::cell::RefCell;

pub use assemblage::Assemblage;

use crate::systeme::gel::Pixels;

/// La comparaison tolérante de deux images (D12).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tolerance {
    /// Écart permis par canal (rouge, vert, bleu).
    pub canal: u8,
    /// Part des pixels d'encre d'une ligne qui doivent être égaux pour que la ligne le
    /// soit.
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

/// Lignes porteuses égales qu'un recouvrement doit avoir pour qu'un décalage passe (D10) ;
/// moins : `Ambigue`.
const PORTEUSES_MIN: u32 = 16;

/// Lignes de chaque bord du recouvrement qui ne comptent pas (§18 : 1 à 4 lignes entières
/// très différentes au bord, échelles 125 et 150 %).
const LIGNES_DE_BORD: usize = 4;

/// Hauteur maximale d'une bande teintée à place fixe (ligne survolée de l'Explorateur :
/// 17 lignes à 100 %, environ 26 à 150 %, 34 à 200 %).
const BANDE_MAX: usize = 48;

/// Hauteur maximale d'une suite différente excusée au bord du recouvrement (en-tête
/// collant de 56 px à 150 % : 84 lignes ; 64 px à 250 % : 160).
const BORD_MAX: usize = 160;

/// Un pixel porte de l'encre s'il s'écarte du fond de sa ligne de plus de ce multiple de
/// l'écart permis (un bruit de ± l'écart permis n'en fait pas).
const ENCRE_FOIS_CANAL: u16 = 3;

/// Canaux d'encre (lus) qu'une ligne porteuse doit avoir (2 pixels gris).
const ENCRE_MIN: u32 = 6;

/// Canaux d'encre qui changent à ± `ECART_VERTICAL` lignes pour qu'une ligne soit
/// porteuse.
const CHANGE_MIN: u32 = 4;

/// Distance, en lignes, à laquelle l'information verticale est cherchée.
const ECART_VERTICAL: usize = 3;

/// Dénominateur minimal d'une ligne (canaux d'encre) : une ligne de 2 pixels d'encre
/// contre une ligne vide est différente.
const DENOMINATEUR_MIN: u32 = 8;

/// Part maximale (sur 100) de lignes porteuses différentes d'un décalage qui passe.
const PORTEUSES_FAUSSES_MAX: u32 = 10;

/// L'attendu choisit parmi des candidats inséparables celui qui est à tant de lignes de
/// lui, s'il est seul.
const ATTENDU_A_PEU_PRES: i32 = 3;

/// Étendue maximale d'un groupe de voisins qui passent (page lisse) ; au-delà, `Ambigue`.
const GROUPE_MAX: i32 = 6;

/// Décalages gardés après le profil, puis après les résumés.
const CANDIDATS_PROFIL: usize = 40;
const CANDIDATS_RESUME: usize = 6;
/// Décalages gardés en plus, classés sur le milieu du recouvrement seul (voir `milieu`).
const CANDIDATS_MILIEU: usize = 2;
/// Décalages à trop peu de lignes porteuses jugés aussi (voir `classer`).
const CANDIDATS_FAIBLES: usize = 3;

/// Blocs du résumé d'une ligne.
const BLOCS: usize = 16;

/// Colonnes lues au plus par ligne (environ) ; au-delà, une sur `pas`.
const COLONNES_VISEES: usize = 768;

/// Échelle des notes (une ligne fausse vaut `UNITE`).
const UNITE: u64 = 1024;

/// Ce qui est ajouté à l'écart avant de diviser par l'appui (une demi-ligne : à écart
/// nul, le plus d'appui l'emporte).
const REGULARISATION: u64 = UNITE / 2;

/// Un rival n'est séparé du meilleur que s'il a au moins tant d'écart de plus (et une
/// note double) : à écart nul de part et d'autre (motif répété à l'identique), rien ne
/// sépare.
const MARGE_ECART: u64 = UNITE / 4;

/// Ce que vaut une image contre la dernière image gardée (D10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mesure {
    /// La page est descendue de `n` lignes : `n` lignes nouvelles en bas.
    Avance(u32),
    /// Rien n'a bougé.
    Immobile,
    /// La page est remontée de `n` lignes : rien n'est ajouté.
    Recul(u32),
    /// Aucun décalage permis n'accorde les deux images : la page est allée trop loin
    /// (« Trop vite »).
    Perdue,
    /// Pas assez de lignes porteuses, ou plusieurs décalages que rien ne sépare (page
    /// unie, motif qui se répète) : rien n'est ajouté, l'image suivante décidera.
    Ambigue,
}

/// Ce que l'application sait du décalage, avant de mesurer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attendu {
    /// Rien de sûr (le pas commandé n'est qu'un indice : il n'est pas passé).
    Aucun,
    /// Le décalage lu par l'application (position de défilement lue, en pixels), depuis
    /// la dernière image gardée : fiable, il peut traverser un blanc (voir `mesurer`).
    Lu(i32),
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

/// Des pixels cohérents.
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
}

/// Colonnes de droite jamais comparées (barre de défilement et pouce, qui bougent seuls).
fn bande_droite(l: usize) -> usize {
    (l / 24).min(48)
}

/// Une colonne lue sur `pas` (au plus `COLONNES_VISEES` colonnes lues).
fn pas_des_colonnes(lc: usize) -> usize {
    lc.div_ceil(COLONNES_VISEES).max(1)
}

/// Les colonnes lues d'une image : `l` pixels par ligne, rangée de `rangee` octets.
struct Image<'a> {
    l: usize,
    h: usize,
    rangee: usize,
    rgba: Cow<'a, [u8]>,
}

impl Image<'_> {
    fn ligne(&self, y: usize) -> &[u8] {
        &self.rgba[y * self.rangee..y * self.rangee + self.l * 4]
    }
}

/// Tampons des images échantillonnées, gardés d'une mesure à l'autre dans le fil qui
/// mesure (deux au plus) : une allocation neuve de plusieurs mégaoctets coûte plus que
/// sa copie (pages à fournir).
const TAMPONS_GARDES: usize = 2;

thread_local! {
    static TAMPONS: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
}

fn tampon(n: usize) -> Vec<u8> {
    let mut v = TAMPONS
        .try_with(|t| t.try_borrow_mut().ok().and_then(|mut t| t.pop()))
        .ok()
        .flatten()
        .unwrap_or_default();
    // Chaque octet est réécrit par l'échantillonnage : le contenu précédent ne compte pas.
    v.resize(n, 0);
    v.truncate(n);
    v
}

impl Drop for Image<'_> {
    fn drop(&mut self) {
        if let Cow::Owned(v) = std::mem::take(&mut self.rgba) {
            let _ = TAMPONS.try_with(|t| {
                if let Ok(mut t) = t.try_borrow_mut() {
                    if t.len() < TAMPONS_GARDES {
                        t.push(v);
                    }
                }
            });
        }
    }
}

fn echantillonner(v: Vue<'_>) -> Image<'_> {
    let lc = (v.l - bande_droite(v.l)).max(1);
    let pas = pas_des_colonnes(lc);
    if pas == 1 {
        return Image {
            l: lc,
            h: v.h,
            rangee: v.l * 4,
            rgba: Cow::Borrowed(v.rgba),
        };
    }
    let l = lc.div_ceil(pas);
    let mut rgba = tampon(l * v.h * 4);
    for (vers, ligne) in rgba
        .chunks_exact_mut(l * 4)
        .zip(v.rgba.chunks_exact(v.l * 4))
    {
        copier_les_colonnes(vers, &ligne[..lc * 4], pas);
    }
    Image {
        l,
        h: v.h,
        rangee: l * 4,
        rgba: Cow::Owned(rgba),
    }
}

/// Écart maximal entre deux pixels sur les trois canaux.
#[inline]
fn ecart(p: &[u8], q: &[u8]) -> u8 {
    p[0].abs_diff(q[0])
        .max(p[1].abs_diff(q[1]))
        .max(p[2].abs_diff(q[2]))
}

/// Octets par paquet des calculs : 16 pixels. Les calculs comptent des **canaux** (rouge,
/// vert, bleu ; l'alpha jamais) : un pixel gris compte trois fois, des deux côtés d'un
/// rapport, qui ne change donc pas. Écrits sans branche sur des paquets de taille fixe,
/// ils deviennent des instructions vectorielles.
const PAQUET: usize = 64;

/// 1 pour les canaux de couleur, 0 pour l'alpha.
const COULEUR: [u8; PAQUET] = {
    let mut m = [1u8; PAQUET];
    let mut k = 3;
    while k < PAQUET {
        m[k] = 0;
        k += 4;
    }
    m
};

#[inline(always)]
fn dif(a: u8, b: u8) -> u8 {
    a.saturating_sub(b) | b.saturating_sub(a)
}

/// 1 si `x > s`, sans branche ni comparaison signée.
#[inline(always)]
fn plus(x: u8, s: u8) -> u8 {
    u8::from(x.saturating_sub(s) != 0)
}

/// Un paquet de la couleur `c`.
fn paquet_de(c: [u8; 3]) -> [u8; PAQUET] {
    let mut p = [0u8; PAQUET];
    for k in 0..PAQUET / 4 {
        p[4 * k..4 * k + 3].copy_from_slice(&c);
    }
    p
}

/// Appelle `f` sur les paquets de même place de `N` lignes de même longueur ; le dernier
/// paquet, incomplet, est complété par `bourre`. `f` rend faux pour arrêter.
#[inline(always)]
fn pour_paquets<const N: usize>(
    lignes: [&[u8]; N],
    bourre: [&[u8; PAQUET]; N],
    mut f: impl FnMut([&[u8; PAQUET]; N]) -> bool,
) {
    let len = lignes[0].len();
    let entiers = len / PAQUET;
    for k in 0..entiers {
        let p: [&[u8; PAQUET]; N] = std::array::from_fn(|i| {
            lignes[i][k * PAQUET..(k + 1) * PAQUET]
                .try_into()
                .unwrap_or(bourre[i])
        });
        if !f(p) {
            return;
        }
    }
    if !len.is_multiple_of(PAQUET) {
        let reste: [[u8; PAQUET]; N] = std::array::from_fn(|i| {
            let mut p = *bourre[i];
            p[..len % PAQUET].copy_from_slice(&lignes[i][entiers * PAQUET..]);
            p
        });
        f(std::array::from_fn(|i| &reste[i]));
    }
}

/// Un morceau de `PAQUET` octets vu comme un paquet.
#[inline(always)]
fn paquet(x: &[u8]) -> &[u8; PAQUET] {
    x.try_into().unwrap_or(&COULEUR)
}

/// Le paquet `k` d'une ligne, ou `reste` (le dernier, complété) au-delà des paquets entiers.
#[inline(always)]
fn paquet_n<'a>(ligne: &'a [u8], k: usize, reste: &'a [u8; PAQUET]) -> &'a [u8; PAQUET] {
    match ligne.get(k * PAQUET..(k + 1) * PAQUET) {
        Some(x) => paquet(x),
        None => reste,
    }
}

/// Canaux d'encre d'un paquet (écart au fond de plus de `seuil`).
#[inline(always)]
fn encre_du_paquet(p: &[u8; PAQUET], f: &[u8; PAQUET], seuil: u8) -> u32 {
    let mut m = [0u8; PAQUET];
    for j in 0..PAQUET {
        m[j] = plus(dif(p[j], f[j]), seuil) & COULEUR[j];
    }
    m.iter().map(|v| u32::from(*v)).sum()
}

/// Ce que `decrire` tire d'un paquet, ajouté aux compteurs `acc` (encre, encre qui
/// change de plus de `seuil` à 3 lignes au-dessus, au-dessous : un bruit ou une trame de
/// ± l'écart permis ne fait pas une ligne porteuse ; un compteur par octet, 255 paquets
/// au plus).
#[inline(always)]
fn decrire_le_paquet(
    acc: &mut [[u8; PAQUET]; 3],
    p: &[u8; PAQUET],
    f: &[u8; PAQUET],
    dessus: &[u8; PAQUET],
    dessous: &[u8; PAQUET],
    seuil: u8,
) {
    for j in 0..PAQUET {
        let encre = plus(dif(p[j], f[j]), seuil) & COULEUR[j];
        acc[0][j] += encre;
        acc[1][j] += encre & plus(dif(p[j], dessus[j]), seuil);
        acc[2][j] += encre & plus(dif(p[j], dessous[j]), seuil);
    }
}

fn somme(m: &[u8; PAQUET]) -> u32 {
    m.iter().map(|v| u32::from(*v)).sum()
}

/// Canaux qui diffèrent de plus de `canal` et portent de l'encre dans l'un ou l'autre
/// paquet.
#[inline(always)]
fn paquet_mauvais(
    a: &[u8; PAQUET],
    fa: &[u8; PAQUET],
    b: &[u8; PAQUET],
    fb: &[u8; PAQUET],
    canal: u8,
    seuil: u8,
) -> u32 {
    let mut m = [0u8; PAQUET];
    for j in 0..PAQUET {
        let encre = plus(dif(a[j], fa[j]), seuil) | plus(dif(b[j], fb[j]), seuil);
        m[j] = plus(dif(a[j], b[j]), canal) & encre & COULEUR[j];
    }
    m.iter().map(|v| u32::from(*v)).sum()
}

/// Le fond d'une ligne : la couleur la plus fréquente parmi 64 pixels pris à égale
/// distance (gris par tranches de 8, deux tranches voisines réunies). Il **domine** s'il
/// couvre au moins la moitié de ces pixels (sinon, une image ou un dégradé : sa couleur
/// ne veut rien dire et n'est pas comparée).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fond {
    c: [u8; 3],
    domine: bool,
}

fn fond(ligne: &[u8]) -> Fond {
    let n = ligne.len() / 4;
    let m = n.min(64);
    // Les pixels `k × n / m` (sans division par pixel), leur gris par tranches de 8.
    let mut pris = [[0u8; 4]; 64];
    let mut tranches = [0u16; 33];
    let (q, r) = (n / m.max(1), n % m.max(1));
    let (mut x, mut reste) = (0usize, 0usize);
    for p in pris.iter_mut().take(m) {
        let px = &ligne[x * 4..x * 4 + 3];
        let g = ((u16::from(px[0]) + 2 * u16::from(px[1]) + u16::from(px[2])) / 4) >> 3;
        *p = [px[0], px[1], px[2], g as u8];
        tranches[g as usize] += 1;
        x += q;
        reste += r;
        if reste >= m {
            reste -= m;
            x += 1;
        }
    }
    let mut i = 0;
    for j in 1..32 {
        if tranches[j] + tranches[j + 1] > tranches[i] + tranches[i + 1] {
            i = j;
        }
    }
    let (mut s, mut c) = ([0u32; 3], 0u32);
    for p in &pris[..m] {
        let g = usize::from(p[3]);
        if g == i || g == i + 1 {
            for k in 0..3 {
                s[k] += u32::from(p[k]);
            }
            c += 1;
        }
    }
    let domine = 2 * c as usize >= m;
    let c = c.max(1);
    Fond {
        c: [(s[0] / c) as u8, (s[1] / c) as u8, (s[2] / c) as u8],
        domine,
    }
}

/// Ce que dit chaque ligne d'une image.
struct Lignes {
    fond: Vec<Fond>,
    /// Pixels d'encre lus.
    encre: Vec<u32>,
    /// Porteuse : de l'encre qui change à ± `ECART_VERTICAL` lignes.
    porteuse: Vec<u8>,
    /// Profil : 0 si non porteuse, sinon le nombre de pixels d'encre en quarts d'octave.
    profil: Vec<u8>,
    /// Gris du fond.
    gris: Vec<u8>,
    /// Écart moyen au fond, par bloc.
    resume: Vec<[u8; BLOCS]>,
}

fn quart_d_octave(n: u32) -> u8 {
    if n == 0 {
        return 0;
    }
    let e = 31 - n.leading_zeros();
    let f = if e >= 2 {
        (n >> (e - 2)) & 3
    } else {
        (n << (2 - e)) & 3
    };
    (1 + e * 4 + f).min(255) as u8
}

fn decrire(im: &Image, t: &Tolerance) -> Lignes {
    let seuil = seuil_encre(t);
    let n = (im.l * 4).div_ceil(PAQUET);
    let blocs = BLOCS.min(n);
    // Les paquets de chaque bloc, et ce qui ramène ses canaux d'encre à 0..255 (calculés
    // une fois : pas de division par ligne).
    let bornes: [usize; BLOCS + 1] = std::array::from_fn(|k| k.min(blocs) * n / blocs);
    let echelle: [u64; BLOCS] = std::array::from_fn(|k| {
        let canaux = (bornes[k + 1] - bornes[k]) * PAQUET * 3 / 4;
        (255u64 << 16) / canaux.max(1) as u64
    });
    let mut d = Lignes {
        fond: Vec::with_capacity(im.h),
        encre: Vec::with_capacity(im.h),
        porteuse: Vec::with_capacity(im.h),
        profil: Vec::with_capacity(im.h),
        gris: Vec::with_capacity(im.h),
        resume: Vec::with_capacity(im.h),
    };
    for y in 0..im.h {
        let ligne = im.ligne(y);
        let f = fond(ligne);
        let pf = paquet_de(f.c);
        // Sans ligne à 3 au-dessus (au-dessous), la ligne elle-même : rien ne change.
        let dessus = if y >= ECART_VERTICAL {
            im.ligne(y - ECART_VERTICAL)
        } else {
            ligne
        };
        let dessous = if y + ECART_VERTICAL < im.h {
            im.ligne(y + ECART_VERTICAL)
        } else {
            ligne
        };
        let (mut encre, mut haut, mut bas) = (0u32, 0u32, 0u32);
        let mut r = [0u8; BLOCS];
        let mut par_bloc = [0u32; BLOCS];
        let entiers = ligne.len() / PAQUET;
        let reste = |x: &[u8]| {
            let mut q = pf;
            q[..x.len() - entiers * PAQUET].copy_from_slice(&x[entiers * PAQUET..]);
            q
        };
        let restes = [reste(ligne), reste(dessus), reste(dessous)];

        for (k, total) in par_bloc.iter_mut().enumerate().take(blocs) {
            let mut acc = [[0u8; PAQUET]; 3];
            for j in bornes[k]..bornes[k + 1] {
                decrire_le_paquet(
                    &mut acc,
                    paquet_n(ligne, j, &restes[0]),
                    &pf,
                    paquet_n(dessus, j, &restes[1]),
                    paquet_n(dessous, j, &restes[2]),
                    seuil,
                );
            }
            let e = somme(&acc[0]);
            *total = e;
            encre += e;
            haut += somme(&acc[1]);
            bas += somme(&acc[2]);
        }
        for k in 0..blocs {
            r[k] = ((u64::from(par_bloc[k]) * echelle[k]) >> 16).min(255) as u8;
        }
        let porte = encre >= ENCRE_MIN && haut.max(bas) >= CHANGE_MIN;
        d.fond.push(f);
        d.encre.push(encre);
        d.porteuse.push(u8::from(porte));
        d.profil.push(if porte { quart_d_octave(encre) } else { 0 });
        d.gris
            .push(((u16::from(f.c[0]) + 2 * u16::from(f.c[1]) + u16::from(f.c[2])) / 4) as u8);
        d.resume.push(r);
    }
    d
}

fn seuil_encre(t: &Tolerance) -> u8 {
    (u16::from(t.canal) * ENCRE_FOIS_CANAL).min(255) as u8
}

/// Pixels d'encre (dans l'une ou l'autre ligne, chacune contre son fond `fa`, `fb`) qui
/// diffèrent de plus de `canal`, comptés jusqu'à `plafond` au moins. Deux fonds différents
/// rendent `plafond` : toute la ligne a changé. Les pixels de fond des deux lignes ne
/// sont pas comparés (un écart de fond de moins de 3 × `canal`, comme WPF, ne compte pas).
fn differents(a: &[u8], fa: Fond, b: &[u8], fb: Fond, t: &Tolerance, plafond: u32) -> u32 {
    if fonds_differents(fa, fb, t.canal) {
        return plafond;
    }
    let seuil = seuil_encre(t);
    let (pa, pb) = (paquet_de(fa.c), paquet_de(fb.c));
    let (mut n, mut k) = (0u32, 0u32);
    pour_paquets([a, b], [&pa, &pb], |[ca, cb]| {
        n += paquet_mauvais(ca, &pa, cb, &pb, t.canal, seuil);
        k += 1;
        k % 4 != 0 || n < plafond
    });
    n
}

/// Les canaux d'encre d'une ligne contre son fond.
fn encre_de(ligne: &[u8], f: Fond, t: &Tolerance) -> u32 {
    let (seuil, pf) = (seuil_encre(t), paquet_de(f.c));
    let mut n = 0;
    pour_paquets([ligne], [&pf], |[p]| {
        n += encre_du_paquet(p, &pf, seuil);
        true
    });
    n
}

/// L'encre de deux lignes est-elle à la même place (chaque ligne jugée contre son propre
/// fond) ? Seule une petite part des pixels d'encre de l'une ou de l'autre n'en a que
/// dans une : 1/5 si le fond a changé (survol, sélection, ombre : l'anticrénelage
/// change), 1/20 sinon (couleur changée sur place), et 2 pixels au moins.
fn encre_en_place(a: &[u8], fa: Fond, b: &[u8], fb: Fond, t: &Tolerance) -> bool {
    let seuil = seuil_encre(t);
    let (mut ou, mut seule) = (0u32, 0u32);
    // Par pixel (un canal suffit) : une teinte qui multiplie les couleurs fait passer un
    // canal sous le seuil sans déplacer l'encre.
    for (p, q) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        let ea = (0..3).any(|c| dif(p[c], fa.c[c]) > seuil);
        let eb = (0..3).any(|c| dif(q[c], fb.c[c]) > seuil);
        ou += u32::from(ea | eb);
        seule += u32::from(ea ^ eb);
    }
    let part = if fonds_differents(fa, fb, t.canal) {
        5
    } else {
        20
    };
    seule <= (ou / part).max(2)
}

fn fonds_differents(a: Fond, b: Fond, canal: u8) -> bool {
    a.domine && b.domine && ecart(&a.c, &b.c) > canal
}

/// Deux lignes comparées seules (sans description) : égales selon D12, sur l'encre.
fn lignes_egales(a: &[u8], b: &[u8], t: &Tolerance) -> bool {
    let (fa, fb) = (fond(a), fond(b));
    let den = encre_de(a, fa, t)
        .max(encre_de(b, fb, t))
        .max(DENOMINATEUR_MIN);
    let permis = permis_ligne(den, t);
    differents(a, fa, b, fb, t, permis + 1) <= permis
}

fn permis_ligne(den: u32, t: &Tolerance) -> u32 {
    ((1.0 - f64::from(t.part_ligne)) * f64::from(den) + 1e-6).floor() as u32
}

/// Les lignes de `b` (la nouvelle image) qui recouvrent `a` décalée de `d`, hors des
/// bandes `haut` et `bas` : la ligne `y` de `b` montre la ligne `y + d` de `a`.
fn recouvrement(h: usize, haut: usize, bas: usize, d: i32) -> (i64, i64) {
    let (h, haut, bas, d) = (h as i64, haut as i64, bas as i64, i64::from(d));
    (haut.max(haut - d), (h - bas).min(h - bas - d))
}

fn recouvrement_min(hauteur_utile: usize) -> usize {
    (RECOUVREMENT_MIN as usize).min(hauteur_utile / 4).max(1)
}

/// Les deux images comparées, leurs descriptions et les suites immobiles.
struct Contexte<'a> {
    a: &'a Image<'a>,
    b: &'a Image<'a>,
    la: &'a Lignes,
    lb: &'a Lignes,
    t: &'a Tolerance,
    /// Bandes données (D11).
    haut: usize,
    bas: usize,
    rec: usize,
    /// Fin de la suite immobile du haut, début de celle du bas.
    fixe_haut: usize,
    fixe_bas: usize,
}

/// Ce qu'est une ligne du recouvrement pour un décalage.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Etat {
    /// Égale, non porteuse (ne dit rien).
    Neutre,
    /// Égale et porteuse.
    Appui,
    /// Différente ; vrai si porteuse.
    Fausse(bool),
    /// Différente, mais l'encre est en place (fond ou couleur changés ; à valider).
    Teinte,
    /// Différente, dans une suite immobile (en-tête, pied) : excusée.
    Fixe,
}

/// Le jugement d'un décalage.
#[derive(Debug, Clone, Copy)]
struct Jugement {
    d: i32,
    /// Jugé sur tout le recouvrement (sinon arrêté : `note` est un minorant).
    complet: bool,
    passe: bool,
    /// Rien ne le contredit (passerait avec plus d'appui).
    sans_contradiction: bool,
    /// Accordé selon D12 sans aucune ligne différente (hors bandes fixes et teintées) :
    /// un recouvrement blanc égal ligne pour ligne.
    sans_ligne_fausse: bool,
    note: u64,
    /// Écart pondéré (lignes × `UNITE`).
    ecart: u64,
    appui: u32,
}

impl Contexte<'_> {
    fn ligne_cout(&self, y: usize, ya: usize, d0: Option<&[u32]>) -> (u32, u32) {
        let den = self.lb.encre[y]
            .max(self.la.encre[ya])
            .max(DENOMINATEUR_MIN);
        let mauvais = match d0 {
            Some(c) => c[y],
            None => differents(
                self.b.ligne(y),
                self.lb.fond[y],
                self.a.ligne(ya),
                self.la.fond[ya],
                self.t,
                den,
            ),
        };
        (mauvais.min(den), den)
    }

    fn excusable_fixe(&self, y: usize) -> bool {
        y < self.fixe_haut || y >= self.fixe_bas
    }

    /// Juge le décalage `d`. `seuil` : (note, écart) du meilleur ; arrêter dès que ce
    /// décalage en est sûrement séparé. Arrêter aussi dès qu'il a plus de lignes fausses
    /// que rien ne peut excuser. `d0` : les pixels différents à 0, déjà comptés.
    fn juger(&self, d: i32, seuil: Option<(u64, u64)>, d0: Option<&[u32]>) -> Option<Jugement> {
        let h = self.b.h;
        let (y0, y1) = recouvrement(h, self.haut, self.bas, d);
        if y1 - y0 < self.rec as i64 || y1 <= y0 {
            return None;
        }
        let n = (y1 - y0) as usize;
        let bord = if n >= 2 * LIGNES_DE_BORD + PORTEUSES_MIN as usize {
            LIGNES_DE_BORD
        } else {
            0
        };
        let (i0, i1) = (y0 as usize + bord, y1 as usize - bord);
        let long = i1 - i0;
        let borne = if d == 0 { 0 } else { BORD_MAX.min(long / 4) };
        let teintes_max = long / 8;
        let permises_max =
            ((1.0 - f64::from(self.t.part_lignes)) * long as f64 + 1e-6).floor() as usize;
        let mut etats = Vec::with_capacity(long);
        let mut couts = Vec::with_capacity(long);
        let (mut ecart_appui, mut appui) = (0u64, 0u32);
        let (mut milieu, mut cout_milieu, mut cout_bord) = (0usize, 0u64, 0u64);
        for y in i0..i1 {
            let i = y - i0;
            let ya = (y as i64 + i64::from(d)) as usize;
            let (mauvais, den) = self.ligne_cout(y, ya, d0);
            let cout = u64::from(mauvais) * UNITE / u64::from(den);
            let porte = self.lb.porteuse[y] | self.la.porteuse[ya] != 0;
            let etat = if mauvais <= permis_ligne(den, self.t) {
                if porte {
                    appui += 1;
                    ecart_appui += cout;
                    Etat::Appui
                } else {
                    Etat::Neutre
                }
            } else if d != 0 && (self.excusable_fixe(y) || self.excusable_fixe(ya)) {
                Etat::Fixe
            } else {
                let au_bord = i < borne || long - i <= borne;
                if au_bord {
                    cout_bord += if porte { cout } else { 0 };
                } else {
                    milieu += 1;
                    cout_milieu += if porte { cout } else { 0 };
                }
                Etat::Fausse(porte)
            };
            etats.push(etat);
            couts.push(cout);
            if i % 16 == 15 {
                let arret = |note: u64, ecart: u64| Jugement {
                    d,
                    complet: false,
                    passe: false,
                    sans_contradiction: false,
                    sans_ligne_fausse: false,
                    note,
                    ecart,
                    appui,
                };
                // Plus de lignes fausses au milieu que les bandes teintées et D12 n'en
                // permettent : contredit.
                if milieu > teintes_max + permises_max {
                    return Some(arret(u64::MAX, u64::MAX));
                }
                if let Some((note, ecart)) = seuil {
                    let restantes = (i1 - y - 1) as u64;
                    let minimum = ecart_appui
                        + (cout_milieu + cout_bord / 2).saturating_sub(teintes_max as u64 * UNITE);
                    let note_min =
                        (minimum + REGULARISATION) * UNITE / (u64::from(appui) + restantes + 1);
                    if minimum >= ecart + MARGE_ECART && note_min >= note.saturating_mul(2) {
                        return Some(arret(note_min, minimum));
                    }
                }
            }
        }
        Some(self.conclure(d, &mut etats, &couts, i0, borne))
    }

    /// Reconnaît les bandes teintées et les bords, puis compte.
    fn conclure(
        &self,
        d: i32,
        etats: &mut [Etat],
        couts: &[u64],
        i0: usize,
        borne: usize,
    ) -> Jugement {
        let long = etats.len();
        let permises_max =
            ((1.0 - f64::from(self.t.part_lignes)) * long as f64 + 1e-6).floor() as usize;
        // Les lignes fausses dont l'encre est en place (fond changé ou couleur changée sur
        // place) : cherchées seulement si elles peuvent changer le jugement.
        let fausses = etats
            .iter()
            .filter(|e| matches!(e, Etat::Fausse(_)))
            .count();
        if fausses <= long / 8 + permises_max + 2 * borne {
            for (i, e) in etats.iter_mut().enumerate() {
                let (y, ya) = (i0 + i, (i0 + i) as i64 + i64::from(d));
                if matches!(e, Etat::Fausse(_))
                    && encre_en_place(
                        self.b.ligne(y),
                        self.lb.fond[y],
                        self.a.ligne(ya as usize),
                        self.la.fond[ya as usize],
                        self.t,
                    )
                {
                    *e = Etat::Teinte;
                }
            }
        }
        // Bandes teintées : au plus deux suites (trous de 2 lignes au plus), chacune de
        // `BANDE_MAX` lignes au plus, la bande au plus 1/8 des lignes ; pour un décalage
        // non nul, deux suites sont la même bande vue dans les deux images (à `d`).
        let mut suites: Vec<(usize, usize)> = Vec::new();
        for (i, e) in etats.iter().enumerate() {
            if *e == Etat::Teinte {
                match suites.last_mut() {
                    Some((_, fin)) if i <= *fin + 2 => *fin = i,
                    _ => suites.push((i, i)),
                }
            }
        }
        // Deux suites séparées par des lignes égales seulement (le texte d'une ligne de
        // liste teintée, égal à son propre fond près), jamais fausses, n'en font qu'une si
        // elle reste de moins de `BANDE_MAX` lignes.
        let mut jointes: Vec<(usize, usize)> = Vec::with_capacity(suites.len());
        for (a, b) in suites {
            match jointes.last_mut() {
                Some((debut, fin))
                    if b - *debut < BANDE_MAX
                        && !etats[*fin..a].iter().any(|e| matches!(e, Etat::Fausse(_))) =>
                {
                    *fin = b
                }
                _ => jointes.push((a, b)),
            }
        }
        let suites = jointes;
        let total: usize = suites.iter().map(|(a, b)| b - a + 1).sum();
        // Deux suites : la bande de hauteur H vue à la même place dans les deux images, donc à
        // `d` lignes l'une de l'autre (H lignes chacune) ; si elle se recouvre elle-même
        // (`d` < H), seules ses lignes qui ne se recouvrent pas changent : deux suites de
        // `d` lignes, à H lignes l'une de l'autre.
        let pres = |x: usize, y: usize| (x as i64 - y as i64).abs() <= 2;
        let dd = d.unsigned_abs() as usize;
        // (Une suite coupée par le bord des lignes jugées garde sa fin ou son début.)
        // Une bande qui suit les lignes d'une liste (la ligne sous le pointeur) change de
        // place à l'écran de moins que sa hauteur : ses deux vues sont à `d` lignes près de
        // moins que cette hauteur.
        let meme_bande = |(a0, a1): (usize, usize), (b0, b1): (usize, usize)| {
            let haut = (a1 - a0 + 1).max(b1 - b0 + 1);
            pres(b0 - a0, dd)
                || pres(b1.saturating_sub(a1), dd)
                || (pres(a1 - a0 + 1, dd) && pres(b1 - b0 + 1, dd) && b0 - a0 <= BANDE_MAX + 2)
                || (b0 - a0).abs_diff(dd) < haut
        };
        let deux_vues = d != 0 && suites.len() == 2 && meme_bande(suites[0], suites[1]);
        // (La même bande vue deux fois n'est comptée qu'une fois.)
        let bande = if deux_vues {
            suites.iter().map(|(a, b)| b - a + 1).max().unwrap_or(0)
        } else {
            total
        };
        let valide = suites.len() <= 2
            && suites.iter().all(|(a, b)| b - a < BANDE_MAX)
            && bande * 8 <= long
            && (d == 0 || suites.len() < 2 || deux_vues);
        let mut refusees = vec![false; long];
        if !valide {
            for (i, e) in etats.iter_mut().enumerate() {
                if *e == Etat::Teinte {
                    let ya = (i0 + i) as i64 + i64::from(d);
                    let porte = self.lb.porteuse[i0 + i] | self.la.porteuse[ya as usize] != 0;
                    *e = Etat::Fausse(porte);
                    refusees[i] = true;
                }
            }
        }
        // Bords : une suite de lignes fausses depuis chaque bout, jusqu'à 4 lignes
        // porteuses égales de suite, d'au plus `borne` lignes.
        let mut demi = vec![false; long];
        if borne > 0 {
            for depuis_le_bas in [false, true] {
                let place = |k: usize| if depuis_le_bas { long - 1 - k } else { k };
                let (mut derniere, mut suite) = (None, 0);
                for k in 0..long.min(borne + 1) {
                    match etats[place(k)] {
                        Etat::Fausse(_) => {
                            derniere = Some(k);
                            suite = 0;
                        }
                        Etat::Appui => {
                            suite += 1;
                            if suite >= 4 {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                if let Some(k) = derniere.filter(|k| *k < borne) {
                    for j in 0..=k {
                        if matches!(etats[place(j)], Etat::Fausse(_)) {
                            demi[place(j)] = true;
                        }
                    }
                }
            }
        }
        let compter = |sans_refusees: bool| {
            let (mut utiles, mut fausses, mut portees, mut appui) = (0u32, 0u32, 0u32, 0u32);
            let mut ecart = 0u64;
            for (i, e) in etats.iter().enumerate() {
                match *e {
                    Etat::Neutre => utiles += 1,
                    Etat::Appui => {
                        utiles += 1;
                        appui += 1;
                        ecart += couts[i];
                    }
                    Etat::Fausse(porte) if demi[i] => {
                        if porte {
                            ecart += couts[i] / 2;
                        }
                    }
                    Etat::Fausse(_) if sans_refusees && refusees[i] => {}
                    Etat::Fausse(porte) => {
                        utiles += 1;
                        fausses += 1;
                        if porte {
                            portees += 1;
                            ecart += couts[i];
                        }
                    }
                    Etat::Teinte | Etat::Fixe => {}
                }
            }
            let permises =
                ((1.0 - f64::from(self.t.part_lignes)) * f64::from(utiles) + 1e-6).floor() as u32;
            let accorde = 2 * utiles as usize >= self.rec
                && fausses <= permises
                && portees * 100 <= PORTEUSES_FAUSSES_MAX * (appui + portees);
            (accorde, ecart, appui)
        };
        let (accorde, ecart, appui) = compter(false);
        // Refusé seulement pour des bandes teintées trop grandes : l'encre est à sa place,
        // ce n'est pas « trop vite ».
        let sans_contradiction = accorde || (!valide && total > 0 && compter(true).0);
        // (Une ligne excusée comme fixe est une ligne différente.)
        let aucune_fausse = !etats
            .iter()
            .any(|e| matches!(e, Etat::Fausse(_) | Etat::Fixe));
        Jugement {
            d,
            complet: true,
            passe: accorde && appui >= PORTEUSES_MIN,
            sans_contradiction,
            sans_ligne_fausse: accorde && aucune_fausse,
            note: (ecart + REGULARISATION) * UNITE / (u64::from(appui) + 1),
            ecart,
            appui,
        }
    }

    /// Somme des écarts (départage des voisins sur une page lisse).
    fn somme_des_ecarts(&self, d: i32) -> u64 {
        let (y0, y1) = recouvrement(self.b.h, self.haut, self.bas, d);
        let mut s = 0u64;
        for y in (y0.max(0) as usize..y1.max(0) as usize).step_by(2) {
            let ya = (y as i64 + i64::from(d)) as usize;
            for (p, q) in self.b.ligne(y).iter().zip(self.a.ligne(ya)) {
                s += u64::from(dif(*p, *q));
            }
        }
        s
    }
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

fn mesure_de(d: i32) -> Mesure {
    match d {
        0 => Mesure::Immobile,
        d if d > 0 => Mesure::Avance(d as u32),
        d => Mesure::Recul(d.unsigned_abs()),
    }
}

/// Mesure `apres` contre `avant` (même taille : la dernière image gardée), hors des
/// bandes `fixes` connues (D11).
///
/// Sans attendu, l'image seule décide : un décalage qui passe et que rien ne sépare des
/// autres, sinon `Ambigue` (ou `Perdue` si tout est contredit).
///
/// Avec un attendu **lu** `e` :
/// - deux images égales à la même place (sans pixel d'encre différent à ± `canal`)
///   donnent `Immobile` si elles ont des lignes porteuses, quel que soit `e` (contenu
///   fixe, ou page pas encore peinte) ; entièrement unies, `e` est rendu (`Perdue`
///   au-delà de la portée, `Immobile` pour 0) : le blanc est cohérent avec le mouvement ;
/// - si le recouvrement à `e` (ou à 3 lignes près, `e` d'abord) est cohérent (il passe, ou
///   il n'a aucune ligne différente, même sans ligne porteuse : un blanc), `e` est rendu
///   même si d'autres décalages passent aussi ; si un autre décalage le contredit
///   nettement, `Ambigue` (`Immobile` si c'est 0) ;
/// - sinon, la mesure sans attendu, où `e` départage seulement des candidats qui passent
///   tous ; une autre avance que `e` donne `Ambigue`.
///
/// Des images de tailles différentes ou incohérentes : `Perdue`.
pub fn mesurer(
    avant: &Pixels,
    apres: &Pixels,
    fixes: Fixes,
    attendu: Attendu,
    t: &Tolerance,
) -> Mesure {
    let (Some(va), Some(vb)) = (Vue::de(avant), Vue::de(apres)) else {
        return Mesure::Perdue;
    };
    if va.l != vb.l || va.h != vb.h {
        return Mesure::Perdue;
    }
    let lu = match attendu {
        Attendu::Aucun => None,
        Attendu::Lu(e) => Some(e),
    };
    let (a, b) = (echantillonner(va), echantillonner(vb));
    let (la, lb) = (decrire(&a, t), decrire(&b, t));
    let h = b.h;
    let haut = (fixes.haut as usize).min(h);
    let bas = (fixes.bas as usize).min(h - haut);
    let utile = h - haut - bas;
    let rec = recouvrement_min(utile);
    if utile <= rec {
        return Mesure::Ambigue;
    }
    // Les différences à la même place : les suites immobiles du haut et du bas.
    let mut d0 = vec![0u32; h];
    let mut statique = vec![false; h];
    for y in haut..h - bas {
        let den = lb.encre[y].max(la.encre[y]).max(DENOMINATEUR_MIN);
        d0[y] = differents(b.ligne(y), lb.fond[y], a.ligne(y), la.fond[y], t, den).min(den);
        statique[y] = d0[y] <= permis_ligne(den, t);
    }
    let porteuses = |l: &Lignes| {
        l.porteuse[haut..h - bas]
            .iter()
            .filter(|p| **p != 0)
            .count()
    };
    // Deux images égales à la même place, à la tolérance des valeurs près (pas à celle des
    // lignes, qui accepterait une ligne de liste pour sa voisine). Avec du contenu : rien
    // n'a bougé, ou la page n'est pas encore peinte. Une vue entièrement unie (chaque
    // ligne à `canal` près, voir `toute_unie`) est cohérente avec tout mouvement : le lu
    // décide, dans la portée.
    if let Some(e) = lu {
        if d0[haut..h - bas].iter().all(|n| *n == 0) {
            // (Égales : il suffit de regarder l'une.)
            let unie = toute_unie(&b, haut, h - bas, t.canal);
            return if !unie || e == 0 {
                Mesure::Immobile
            } else if e.unsigned_abs() as usize > utile - rec {
                Mesure::Perdue
            } else {
                mesure_de(e)
            };
        }
    }
    let fixe_haut = (haut..h - bas).find(|y| !statique[*y]).unwrap_or(h - bas);
    let fixe_bas = (haut..h - bas)
        .rev()
        .find(|y| !statique[*y])
        .map_or(haut, |y| y + 1);
    let cx = Contexte {
        a: &a,
        b: &b,
        la: &la,
        lb: &lb,
        t,
        haut,
        bas,
        rec,
        fixe_haut,
        fixe_bas,
    };
    // Assez de lignes porteuses dans les deux images, sinon rien ne peut passer : seul un
    // attendu lu peut alors décider.
    let assez =
        porteuses(&la) >= PORTEUSES_MIN as usize && porteuses(&lb) >= PORTEUSES_MIN as usize;
    let (normale, juges) = if assez {
        decider(&cx, &d0, lu)
    } else {
        (Mesure::Ambigue, Vec::new())
    };
    match lu {
        Some(e) => avec_le_lu(&cx, &d0, e, assez, normale, &juges),
        None => normale,
    }
}

/// La décision avec un attendu lu `e` (voir `mesurer`), après la mesure sans lui.
fn avec_le_lu(
    cx: &Contexte,
    d0: &[u32],
    e: i32,
    assez: bool,
    normale: Mesure,
    juges: &[Jugement],
) -> Mesure {
    let dmax = (cx.b.h - cx.haut - cx.bas - cx.rec) as i32;
    if e.abs() > dmax {
        // Au-delà du recouvrement permis : rien à vérifier.
        return match normale {
            Mesure::Immobile => Mesure::Immobile,
            Mesure::Ambigue | Mesure::Perdue => Mesure::Perdue,
            _ => Mesure::Ambigue,
        };
    }
    let mut pres: Vec<Jugement> = Vec::new();
    // Les voisins à 3 lignes près seulement si des lignes porteuses peuvent les départager :
    // sinon (une zone lisse) une pente compense un écart de rendu, `e` seul est jugé.
    let pres_de = if assez { ATTENDU_A_PEU_PRES } else { 0 };
    for d in (e - pres_de).max(-dmax)..=(e + pres_de).min(dmax) {
        let deja = juges.iter().find(|j| j.d == d && j.complet).copied();
        if let Some(j) = deja.or_else(|| cx.juger(d, None, (d == 0).then_some(d0))) {
            pres.push(j);
        }
    }
    let coherent = |j: &Jugement| j.complet && (j.passe || j.sans_ligne_fausse);
    let choisi = pres
        .iter()
        .filter(|j| coherent(j))
        .min_by_key(|j| (j.d != e, j.ecart, (j.d - e).unsigned_abs()));

    let Some(c) = choisi.copied() else {
        // Les images contredisent le lu : elles décident seules, sauf une autre avance.
        return match normale {
            Mesure::Avance(_) | Mesure::Recul(_) => {
                let v = match normale {
                    Mesure::Avance(n) => n as i32,
                    Mesure::Recul(n) => -(n as i32),
                    _ => 0,
                };
                if (v - e).abs() <= ATTENDU_A_PEU_PRES {
                    normale
                } else {
                    Mesure::Ambigue
                }
            }
            m => m,
        };
    };
    // Un autre décalage qui passe et bat nettement le lu le contredit : plus juste, ou
    // deux fois plus d'appui pour une note deux fois meilleure.
    let contredit = juges
        .iter()
        .filter(|r| r.complet && r.passe && (r.d - c.d).abs() > ATTENDU_A_PEU_PRES + 2)
        .filter(|r| {
            let deux_fois = c.note >= r.note.saturating_mul(2);
            (c.ecart >= r.ecart + MARGE_ECART && deux_fois)
                || (2 * c.appui < r.appui && (deux_fois || !c.passe))
        })
        .min_by_key(|r| r.note);
    match contredit {
        None => mesure_de(c.d),
        Some(r) if r.d == 0 => Mesure::Immobile,
        Some(_) => Mesure::Ambigue,
    }
}

/// Un décalage classé : (note, décalage).
type Classe = (u64, i32);

/// Le milieu d'un recouvrement `y0..y1` pour le classement : sans les suites du bord qu'un
/// jugement peut excuser (en-tête qui apparaît, bannière qui part), qui pèseraient sur le
/// vrai décalage plus que sur un faux.
fn milieu(y0: usize, y1: usize, d: i32) -> (usize, usize) {
    let bord = if d == 0 {
        0
    } else {
        BORD_MAX.min((y1 - y0) / 4)
    };
    (y0 + bord, y1 - bord)
}

/// Chaque ligne `y0..y1` est unie : sur chaque canal, ses points lus ne s'écartent pas de
/// plus de `canal` les uns des autres (un dégradé ou une photo floue ne l'est pas).
fn toute_unie(im: &Image, y0: usize, y1: usize, canal: u8) -> bool {
    (y0..y1).all(|y| {
        let (mut bas, mut haut) = ([255u8; 3], [0u8; 3]);
        for p in im.ligne(y).chunks_exact(4) {
            for c in 0..3 {
                bas[c] = bas[c].min(p[c]);
                haut[c] = haut[c].max(p[c]);
            }
        }
        (0..3).all(|c| haut[c].saturating_sub(bas[c]) <= canal)
    })
}

/// Les candidats classés en deux passes grossières (profil, résumés), et quelques
/// décalages dont le recouvrement porte trop peu pour juger (pour ne pas dire « perdue »
/// quand un recouvrement vide s'accorderait) ; vrai s'il y en avait plus, au profil
/// accordé, que ceux gardés (une barre latérale tramée : le bon n'est peut-être pas jugé).
fn classer(cx: &Contexte) -> (Vec<Classe>, Vec<Classe>, bool) {
    let h = cx.b.h;
    let utile = h - cx.haut - cx.bas;
    let dmax = (utile - cx.rec) as i32;
    let mut profil: Vec<(u64, i32)> = Vec::with_capacity(CANDIDATS_PROFIL + 1);
    let mut profil_milieu: Vec<(u64, i32)> = Vec::with_capacity(CANDIDATS_PROFIL + 1);
    let mut faibles: Vec<(u64, i32)> = Vec::with_capacity(CANDIDATS_FAIBLES + 1);
    let mut faibles_nuls = 0usize;
    for d in -dmax..=dmax {
        let (y0, y1) = recouvrement(h, cx.haut, cx.bas, d);
        if y1 - y0 < cx.rec as i64 {
            continue;
        }
        let (y0, y1) = (y0 as usize, y1 as usize);
        let (m0, m1) = milieu(y0, y1, d);
        let (c0, n0) = cout_du_profil(cx, d, y0, m0);
        let (cm, nm) = cout_du_profil(cx, d, m0, m1);
        let (c1, n1) = cout_du_profil(cx, d, m1, y1);
        let (cout, nb) = (c0 + cm + c1, n0 + nm + n1);
        if nb < u64::from(PORTEUSES_MIN) {
            faibles_nuls += usize::from(cout == 0);
            garder(&mut faibles, CANDIDATS_FAIBLES, cout * UNITE / nb.max(1), d);
            continue;
        }
        garder(&mut profil, CANDIDATS_PROFIL, cout * UNITE / nb, d);
        if nm >= u64::from(PORTEUSES_MIN) {
            garder(&mut profil_milieu, CANDIDATS_PROFIL, cm * UNITE / nm, d);
        }
    }
    let affiner = |liste: &[(u64, i32)], au_milieu: bool, n: usize| {
        let mut resume: Vec<(u64, i32)> = Vec::with_capacity(n + 1);
        for &(_, d) in liste {
            let (y0, y1) = recouvrement(h, cx.haut, cx.bas, d);
            let (y0, y1) = (y0 as usize, y1 as usize);
            let (y0, y1) = if au_milieu {
                milieu(y0, y1, d)
            } else {
                (y0, y1)
            };
            garder(&mut resume, n, note_du_resume(cx, d, y0, y1), d);
        }
        resume
    };
    let mut classes = affiner(&profil, false, CANDIDATS_RESUME);
    classes.extend(affiner(&profil_milieu, true, CANDIDATS_MILIEU));
    (classes, faibles, faibles_nuls > CANDIDATS_FAIBLES)
}

/// Coût du profil des lignes `y0..y1` de la nouvelle image décalées de `d`, et nombre de
/// paires où l'une des lignes est porteuse.
fn cout_du_profil(cx: &Contexte, d: i32, y0: usize, y1: usize) -> (u64, u64) {
    const PLAFOND_PROFIL: u32 = 8;
    let (pb, pa) = (&cx.lb.porteuse, &cx.la.porteuse);
    let (qb, qa) = (&cx.lb.profil, &cx.la.profil);
    let (gb, ga) = (&cx.lb.gris, &cx.la.gris);
    let canal = cx.t.canal;
    let (z0, z1) = (
        (y0 as i64 + i64::from(d)) as usize,
        (y1 as i64 + i64::from(d)) as usize,
    );
    let (mut cout, mut nb) = (0u32, 0u32);
    for ((((p, q), g), (pp, qq)), gg) in pb[y0..y1]
        .iter()
        .zip(&qb[y0..y1])
        .zip(&gb[y0..y1])
        .zip(pa[z0..z1].iter().zip(&qa[z0..z1]))
        .zip(&ga[z0..z1])
    {
        let j = u32::from(*p | *pp);
        let fonds = if g.abs_diff(*gg) > canal {
            PLAFOND_PROFIL
        } else {
            0
        };
        let c = (u32::from(q.abs_diff(*qq)) + fonds).min(PLAFOND_PROFIL);
        cout += j * c;
        nb += j;
    }
    (u64::from(cout), u64::from(nb))
}

/// Note des résumés des lignes `y0..y1` de la nouvelle image décalées de `d` (paires où
/// l'une des lignes est porteuse).
fn note_du_resume(cx: &Contexte, d: i32, y0: usize, y1: usize) -> u64 {
    const PLAFOND_RESUME: u32 = 24 * BLOCS as u32;
    let ya0 = (y0 as i64 + i64::from(d)) as usize;
    let n = y1 - y0;
    let (mut cout, mut nb) = (0u64, 0u64);
    for (((p, q), rb), ra) in cx.lb.porteuse[y0..y1]
        .iter()
        .zip(&cx.la.porteuse[ya0..ya0 + n])
        .zip(&cx.lb.resume[y0..y1])
        .zip(&cx.la.resume[ya0..ya0 + n])
    {
        if p | q == 0 {
            continue;
        }
        let dist: u32 = rb
            .iter()
            .zip(ra)
            .map(|(x, z)| u32::from(x.abs_diff(*z)))
            .sum();
        cout += u64::from(dist.min(PLAFOND_RESUME));
        nb += 1;
    }
    cout * UNITE / nb.max(1)
}

/// Juge 0, les meilleurs candidats et l'attendu lu, puis décide (voir la doc du module) ;
/// rend aussi les jugements.
fn decider(cx: &Contexte, d0: &[u32], attendu: Option<i32>) -> (Mesure, Vec<Jugement>) {
    let mut juges: Vec<Jugement> = Vec::new();
    let m = decider_dans(cx, d0, attendu, &mut juges);
    (m, juges)
}

fn decider_dans(
    cx: &Contexte,
    d0: &[u32],
    attendu: Option<i32>,
    juges: &mut Vec<Jugement>,
) -> Mesure {
    let h = cx.b.h;
    let dmax = (h - cx.haut - cx.bas - cx.rec) as i32;
    let mut ordre: Vec<i32> = vec![0];
    let (classes, faibles, non_vus) = classer(cx);
    for (_, d) in classes {
        if !ordre.contains(&d) {
            ordre.push(d);
        }
    }
    if let Some(e) = attendu.filter(|e| e.abs() <= dmax) {
        if !ordre.contains(&e) {
            ordre.push(e);
        }
    }
    for (_, d) in faibles {
        if !ordre.contains(&d) {
            ordre.push(d);
        }
    }
    let mut meilleure: Option<(u64, u64)> = None;
    for &d in &ordre {
        let j = cx.juger(d, meilleure, (d == 0).then_some(d0));
        if let Some(j) = j {
            if j.passe && meilleure.is_none_or(|(n, _)| j.note < n) {
                meilleure = Some((j.note, j.ecart));
            }
            juges.push(j);
        }
    }
    let Some(meilleur) = juges
        .iter()
        .filter(|j| j.passe)
        .min_by_key(|j| (j.note, j.d.unsigned_abs()))
        .copied()
    else {
        // Des recouvrements sans information n'ont pas tous été jugés : pas « perdue ».
        return if non_vus || juges.iter().any(|j| j.sans_contradiction) {
            Mesure::Ambigue
        } else {
            Mesure::Perdue
        };
    };
    // Le groupe du meilleur : la suite des décalages qui passent à 2 lignes l'un de
    // l'autre autour de lui (page lisse, décalage fractionnaire), sur `GROUPE_MAX` lignes
    // au plus.
    let (mut lo, mut hi) = (meilleur.d, meilleur.d);
    loop {
        let avant = (lo, hi);
        for j in juges.iter().filter(|j| j.passe && j.complet) {
            if j.d < lo && lo - j.d <= 2 {
                lo = j.d;
            }
            if j.d > hi && j.d - hi <= 2 {
                hi = j.d;
            }
        }
        if (lo, hi) == avant {
            break;
        }
    }
    if hi - lo > GROUPE_MAX {
        return Mesure::Ambigue;
    }
    let voisin = |d: i32| d >= lo - 2 && d <= hi + 2;
    let groupe: Vec<Jugement> = juges
        .iter()
        .filter(|j| voisin(j.d) && j.passe && j.complet)
        .copied()
        .collect();
    let choisi = choisir_dans_le_groupe(cx, &groupe, attendu);
    // Un rival est séparé s'il est nettement pire (écart et note), ou s'il ne passe pas
    // et a moitié moins d'appui (moins de preuves, pas une autre explication).
    let separe = |r: &Jugement| {
        !r.complet
            || (r.ecart >= meilleur.ecart + MARGE_ECART
                && r.note >= meilleur.note.saturating_mul(2))
            || (!r.passe && 2 * r.appui < meilleur.appui)
    };
    let rivaux: Vec<Jugement> = juges.iter().filter(|j| !voisin(j.d)).copied().collect();
    if rivaux.iter().all(&separe) {
        return mesure_de(choisi);
    }
    // Inséparables : l'attendu choisit, si tous passent, si un seul groupe est près de
    // lui, et jamais une avance contre 0.
    let Some(e) = attendu else {
        return Mesure::Ambigue;
    };
    let mut proches: Vec<Jugement> = rivaux.iter().filter(|r| !separe(r)).copied().collect();
    proches.extend(groupe.iter().copied());
    // Les décalages à 3 lignes de l'attendu qui n'ont pas été jugés le sont maintenant.
    for d in e - ATTENDU_A_PEU_PRES..=e + ATTENDU_A_PEU_PRES {
        if d.abs() <= dmax && !juges.iter().any(|j| j.d == d) {
            if let Some(j) = cx.juger(d, None, (d == 0).then_some(d0)) {
                juges.push(j);
                if !separe(&j) {
                    proches.push(j);
                }
            }
        }
    }
    if proches.iter().any(|j| !j.passe || !j.complet) {
        return Mesure::Ambigue;
    }
    let pres: Vec<&Jugement> = proches
        .iter()
        .filter(|j| (j.d - e).abs() <= ATTENDU_A_PEU_PRES)
        .collect();
    let (Some(lo), Some(hi)) = (
        pres.iter().map(|j| j.d).min(),
        pres.iter().map(|j| j.d).max(),
    ) else {
        return Mesure::Ambigue;
    };
    if hi - lo > 2 * ATTENDU_A_PEU_PRES || (proches.iter().any(|j| j.d == 0) && lo > 0) {
        return Mesure::Ambigue;
    }
    let pres: Vec<Jugement> = pres.into_iter().copied().collect();
    mesure_de(choisir_dans_le_groupe(cx, &pres, attendu))
}

/// Dans un groupe de voisins : 0 s'il y est (une page immobile aux écarts de rendu, comme
/// WPF, ne glisse pas de deux lignes) ; sinon le plus petit écart ; à égalité, l'attendu,
/// puis la plus petite somme des écarts pixel à pixel, puis le plus petit décalage.
fn choisir_dans_le_groupe(cx: &Contexte, groupe: &[Jugement], attendu: Option<i32>) -> i32 {
    if groupe.iter().any(|j| j.d == 0) {
        return 0;
    }
    let Some(min) = groupe.iter().map(|j| j.ecart).min() else {
        return 0;
    };
    let egaux: Vec<i32> = groupe
        .iter()
        .filter(|j| j.ecart == min)
        .map(|j| j.d)
        .collect();
    if egaux.len() == 1 {
        return egaux[0];
    }
    if let Some(e) = attendu.filter(|e| egaux.contains(e)) {
        return e;
    }
    egaux
        .iter()
        .copied()
        .min_by_key(|d| (cx.somme_des_ecarts(*d), d.unsigned_abs()))
        .unwrap_or(0)
}

/// Deux images comparées ligne à ligne sans description (assemblage, bandes fixes).
struct Paire<'a> {
    a: Vue<'a>,
    b: Vue<'a>,
    t: &'a Tolerance,
}

impl<'a> Paire<'a> {
    fn de(avant: &'a Pixels, apres: &'a Pixels, t: &'a Tolerance) -> Option<Paire<'a>> {
        let (a, b) = (Vue::de(avant)?, Vue::de(apres)?);
        (a.l == b.l && a.h == b.h).then_some(Paire { a, b, t })
    }

    /// La ligne `yb` de l'image d'après est égale à la ligne `ya` de l'image d'avant (les
    /// colonnes lues par la mesure, échantillonnées ligne par ligne : peu de lignes sont
    /// comparées).
    fn egales(&self, yb: usize, ya: usize) -> bool {
        lignes_egales(&ligne_lue(self.b, yb), &ligne_lue(self.a, ya), self.t)
    }

    fn encre(&self, yb: usize) -> u32 {
        let l = ligne_lue(self.b, yb);
        encre_de(&l, fond(&l), self.t)
    }

    /// Les lignes `y` de l'image d'après qui ne diffèrent de la ligne `y + d` d'avant que
    /// par une teinte (survol, sélection, ombre : fond changé, encre à sa place), et
    /// laquelle des deux est teintée. Une suite de telles lignes (trous de 2 lignes au
    /// plus) est jugée d'un bloc : est teintée la version dont le fond n'est pas celui des
    /// lignes voisines, égales dans les deux images (vote des lignes au fond net : une
    /// ligne de texte dense n'a pas de fond qui domine). Un drapeau par ligne de l'image.
    fn teintes(&self, d: usize) -> Vec<Teinte> {
        let h = self.b.h;
        let n = h.saturating_sub(d);
        let mut sortie = vec![Teinte::Non; h];
        let lc = (self.b.l - bande_droite(self.b.l)).max(1) * 4;
        let brute = |v: Vue<'a>, y: usize| -> &'a [u8] { &v.rgba[y * v.l * 4..y * v.l * 4 + lc] };
        // Les fonds (64 points) des deux versions de la ligne `y`.
        let fonds = |y: usize| (fond(brute(self.b, y)), fond(brute(self.a, y + d)));
        let canal = self.t.canal;
        let mut cand = vec![false; n];
        for (y, c) in cand.iter_mut().enumerate() {
            // 32 points égaux : pas une teinte (la plupart des lignes s'arrêtent là).
            let (rb, ra) = (brute(self.b, y), brute(self.a, y + d));
            let pas = (lc / 4 / 32).max(1) * 4;
            let pareilles = rb
                .chunks(pas)
                .zip(ra.chunks(pas))
                .all(|(p, q)| p.len() < 3 || q.len() < 3 || ecart(p, q) <= canal);
            if pareilles {
                continue;
            }
            let (b, a) = fonds(y);
            if ecart(&b.c, &a.c) > canal {
                let (lb, la) = (ligne_lue(self.b, y), ligne_lue(self.a, y + d));
                let (b, a) = (fond(&lb), fond(&la));
                *c = encre_en_place(&lb, b, &la, a, self.t);
            }
        }
        let mut y = 0;
        while y < n {
            if !cand[y] {
                y += 1;
                continue;
            }
            let (debut, mut fin) = (y, y + 1);
            let mut k = y + 1;
            while k < n && k < fin + 2 {
                if cand[k] {
                    fin = k + 1;
                }
                k += 1;
            }
            // Le fond de référence : la ligne voisine la plus proche, hors de la suite,
            // égale dans les deux images et au fond net.
            let net = |k: usize| {
                let (b, a) = fonds(k);
                (!cand[k] && b.domine && ecart(&a.c, &b.c) <= canal).then_some(b)
            };
            let apres = (fin..(fin + 8).min(n)).find_map(net);
            let avant = (debut.saturating_sub(8)..debut).rev().find_map(net);
            if let Some(r) = apres.or(avant) {
                let (mut neuve, mut ancienne) = (0u32, 0u32);
                for k in debut..fin {
                    let (b, a) = fonds(k);
                    let bt = b.domine && ecart(&b.c, &r.c) > canal;
                    let at = a.domine && ecart(&a.c, &r.c) > canal;
                    neuve += u32::from(bt && !at);
                    ancienne += u32::from(at && !bt);
                }
                let t = if neuve > ancienne {
                    Teinte::Neuve
                } else if ancienne > neuve {
                    Teinte::Ancienne
                } else {
                    Teinte::Non
                };
                sortie[debut..fin].fill(t);
            }
            y = fin;
        }
        sortie
    }
}

/// Une ligne de l'image d'après comparée à la ligne déjà posée qu'elle recouvre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Teinte {
    /// Égales, ou différentes autrement que par une teinte.
    Non,
    /// La nouvelle est teintée : garder la ligne posée.
    Neuve,
    /// La ligne posée est teintée : la remplacer.
    Ancienne,
}

/// Les colonnes lues de la ligne `y` (les mêmes qu'`echantillonner`).
fn ligne_lue(v: Vue<'_>, y: usize) -> Cow<'_, [u8]> {
    let lc = (v.l - bande_droite(v.l)).max(1);
    let pas = pas_des_colonnes(lc);
    let ligne = &v.rgba[y * v.l * 4..y * v.l * 4 + lc * 4];
    if pas == 1 {
        return Cow::Borrowed(ligne);
    }
    let mut vers = vec![0u8; lc.div_ceil(pas) * 4];
    copier_les_colonnes(&mut vers, ligne, pas);
    Cow::Owned(vers)
}

/// Une colonne sur `pas` de `ligne` (pixels entiers) dans `vers`.
fn copier_les_colonnes(vers: &mut [u8], ligne: &[u8], pas: usize) {
    for (q, p) in vers.chunks_exact_mut(4).zip(ligne.chunks(4 * pas)) {
        q.copy_from_slice(&p[..4]);
    }
}

/// Les bandes fixes de deux images séparées par une `avance` (D11), déterminées après la
/// mesure : les suites du haut et du bas égales à la même place dans les deux images, et
/// jusqu'à leur dernière ligne qui ne suit pas l'avance (unies comprises, sans minimum :
/// un pied manqué perdrait des lignes). Elles ne peuvent pas couvrir l'avance.
pub fn fixes(avant: &Pixels, apres: &Pixels, avance: u32, t: &Tolerance) -> Fixes {
    Paire::de(avant, apres, t).map_or_else(Fixes::default, |p| p.fixes(avance))
}

impl Paire<'_> {
    /// `fixes` sur une paire.
    fn fixes(&self, avance: u32) -> Fixes {
        let (h, d) = (self.b.h, avance as usize);
        if d == 0 || d >= h {
            return Fixes::default();
        }
        // En haut : la ligne y d'après ne montre pas la ligne y + d d'avant.
        let mut haut = 0;
        for y in 0..h - d {
            if !self.egales(y, y) {
                break;
            }
            if !self.egales(y, y + d) {
                haut = y + 1;
            }
        }
        // En bas : la ligne y d'avant ne se retrouve pas à y - d après.
        let mut bas = 0;
        for y in (d..h).rev() {
            if !self.egales(y, y) {
                break;
            }
            if !self.egales(y - d, y) {
                bas = h - y;
            }
        }
        if haut + bas + d >= h {
            return Fixes::default();
        }
        Fixes {
            haut: haut as u32,
            bas: bas as u32,
        }
    }

    /// Les lignes du haut de l'image d'après qu'il ne faut pas poser après une avance
    /// `d` : jusqu'à la dernière ligne qui ne montre pas la ligne `y + d` d'avant, avant
    /// quatre lignes d'encre égales de suite (en-tête fixe, en-tête qui apparaît, ombre
    /// sous l'en-tête). Une ligne qui ne diffère que par une teinte ne compte pas.
    fn haut_a_garder(&self, d: u32, teintes: &[Teinte]) -> u32 {
        let (h, d) = (self.b.h, d as usize);
        let (mut garder, mut suite) = (0usize, 0);
        for y in 0..h.saturating_sub(d).min(h / 2 + BORD_MAX) {
            if teintes.get(y).is_some_and(|t| *t != Teinte::Non) {
                // La même ligne, teintée dans l'une : pas un en-tête (`teintes` décide).
                suite = 0;
            } else if self.egales(y, y + d) {
                if self.encre(y) >= ENCRE_MIN {
                    suite += 1;
                    if suite >= 4 {
                        break;
                    }
                }
            } else {
                garder = y + 1;
                suite = 0;
            }
        }
        garder as u32
    }

    /// Les bandes après une nouvelle avance `d` (D11 : elles ne peuvent que rétrécir). Une
    /// ligne quitte une bande quand elle n'est plus à la même place mais suit la page
    /// (égale à la ligne `y + d` de l'image d'avant) ; une ligne qui change sans suivre la
    /// page (une animation dans le pied) reste dans la bande.
    fn retrecir(&self, d: u32, f: Fixes) -> Fixes {
        let (h, d) = (self.b.h, d as usize);
        let bouge = |y: usize| y + d < h && !self.egales(y, y) && self.egales(y, y + d);
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
}

#[cfg(test)]
mod tests;
