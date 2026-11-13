//! Texte : la partie commune de « Copier le texte » (contrat 0.5, §5.3), au-dessus du
//! moteur du système (`systeme::texte`). Pur, sauf `lire`. N'importe que `crate::systeme`.
//!
//! Le texte lu ne va jamais au journal ni à une page : seules les boîtes des mots sortent
//! de `Lu` ; le texte n'en sort que par `assembler` et `extrait`, vers le presse-papiers.

use std::fmt;
use std::time::Duration;

use crate::systeme::gel::Pixels;
use crate::systeme::texte::{self as moteur, ErreurMoteur, LigneBrute, Reconnu};

#[cfg(test)]
mod tests;

/// Au-delà, la lecture est abandonnée (« Impossible de lire le texte »).
pub const DELAI_LECTURE: Duration = Duration::from_secs(10);

/// En pixels : une image plus petite est agrandie ×2 avant la lecture.
pub const PETITE_IMAGE: u64 = 1_000_000;

/// L'image telle que le moteur la lit : BGRA, alpha composé sur blanc, et le facteur
/// appliqué à l'image d'origine.
#[derive(Debug, Clone, PartialEq)]
pub struct Preparee {
    pub bgra: Vec<u8>,
    pub largeur: u32,
    pub hauteur: u32,
    pub facteur: f32,
}

/// Prépare `pixels` pour le moteur (alpha sur blanc, ×2 sous `PETITE_IMAGE` si le double
/// tient dans `dimension_max`, réduite si un côté le dépasse). `None` si l'image est vide
/// (ou ses octets ne font pas `largeur × hauteur × 4`, ou `dimension_max` vaut 0).
///
/// Le ×2 (plus proche voisin) : le moteur lit mal le texte d'écran de 11 à 12 px à sa
/// taille (sonde) et ne lit pas une image de moins de 40 px de haut environ. La réduction
/// fait la moyenne des pixels couverts (le texte reste lisible).
pub fn preparer(pixels: &Pixels, dimension_max: u32) -> Option<Preparee> {
    let (l, h) = (pixels.largeur, pixels.hauteur);
    let attendu = (l as usize).checked_mul(h as usize)?.checked_mul(4)?;
    if l == 0 || h == 0 || dimension_max == 0 || pixels.rgba.len() != attendu {
        return None;
    }
    let blanc = sur_blanc(&pixels.rgba);
    let grand = l.max(h);
    if u64::from(l) * u64::from(h) <= PETITE_IMAGE
        && u64::from(grand) * 2 <= u64::from(dimension_max)
    {
        return Some(Preparee {
            bgra: doubler(&blanc, l, h),
            largeur: l * 2,
            hauteur: h * 2,
            facteur: 2.0,
        });
    }
    if grand > dimension_max {
        let facteur = dimension_max as f32 / grand as f32;
        let nl = reduit(l, grand, dimension_max);
        let nh = reduit(h, grand, dimension_max);
        return Some(Preparee {
            bgra: reduire(&blanc, l, h, nl, nh),
            largeur: nl,
            hauteur: nh,
            facteur,
        });
    }
    Some(Preparee {
        bgra: blanc,
        largeur: l,
        hauteur: h,
        facteur: 1.0,
    })
}

/// Le côté `cote` réduit dans le rapport `max / grand` (au moins 1, au plus `max`).
fn reduit(cote: u32, grand: u32, max: u32) -> u32 {
    let n = (u64::from(cote) * u64::from(max) + u64::from(grand) / 2) / u64::from(grand);
    (n as u32).clamp(1, max)
}

/// RGBA → BGRA opaque, chaque pixel composé sur blanc selon son alpha.
fn sur_blanc(rgba: &[u8]) -> Vec<u8> {
    let mut bgra = Vec::with_capacity(rgba.len());
    for p in rgba.chunks_exact(4) {
        let a = u32::from(p[3]);
        let c = |v: u8| ((u32::from(v) * a + 255 * (255 - a) + 127) / 255) as u8;
        bgra.extend_from_slice(&[c(p[2]), c(p[1]), c(p[0]), 255]);
    }
    bgra
}

/// Agrandit ×2 au plus proche voisin.
fn doubler(bgra: &[u8], l: u32, h: u32) -> Vec<u8> {
    let (l, h) = (l as usize, h as usize);
    let mut sortie = Vec::with_capacity(l * h * 16);
    for y in 0..h {
        let ligne = &bgra[y * l * 4..(y + 1) * l * 4];
        let debut = sortie.len();
        for p in ligne.chunks_exact(4) {
            sortie.extend_from_slice(p);
            sortie.extend_from_slice(p);
        }
        sortie.extend_from_within(debut..);
    }
    sortie
}

/// Réduit `l × h` en `nl × nh` (au plus aussi grand) : chaque pixel est la moyenne des
/// pixels d'origine qu'il couvre.
fn reduire(bgra: &[u8], l: u32, h: u32, nl: u32, nh: u32) -> Vec<u8> {
    let borne = |i: u32, n: u32, total: u32| -> usize {
        (u64::from(i) * u64::from(total) / u64::from(n)) as usize
    };
    let mut sortie = Vec::with_capacity(nl as usize * nh as usize * 4);
    for ny in 0..nh {
        let (y0, y1) = (
            borne(ny, nh, h),
            borne(ny + 1, nh, h).max(borne(ny, nh, h) + 1),
        );
        for nx in 0..nl {
            let (x0, x1) = (
                borne(nx, nl, l),
                borne(nx + 1, nl, l).max(borne(nx, nl, l) + 1),
            );
            let mut somme = [0u64; 3];
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = (y * l as usize + x) * 4;
                    for (s, v) in somme.iter_mut().zip(&bgra[i..i + 3]) {
                        *s += u64::from(*v);
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u64;
            for s in somme {
                sortie.push(((s + n / 2) / n) as u8);
            }
            sortie.push(255);
        }
    }
    sortie
}

/// La boîte d'un mot, en pixels de l'image d'origine, et sa ligne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mot {
    pub x: u32,
    pub y: u32,
    pub largeur: u32,
    pub hauteur: u32,
    pub ligne: u32,
}

/// Le résultat d'une lecture. Les textes restent privés (et hors de son `Debug`).
#[derive(Clone, PartialEq, Eq)]
pub struct Lu {
    pub langue: String,
    pub mots: Vec<Mot>,
    /// Le texte de chaque mot (même rang que `mots`).
    textes: Vec<String>,
    /// Le séparateur de la ligne de chaque mot (même rang que `mots`).
    separateurs: Vec<&'static str>,
}

/// La langue et les nombres de mots et de lignes : jamais le texte.
impl fmt::Debug for Lu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lignes = self.mots.last().map_or(0, |m| m.ligne as usize + 1);
        f.debug_struct("Lu")
            .field("langue", &self.langue)
            .field("mots", &self.mots.len())
            .field("lignes", &lignes)
            .finish_non_exhaustive()
    }
}

/// Ramène les boîtes du moteur à l'image d'origine (`facteur`, bornées à `l × h`), dans
/// l'ordre de lecture du moteur. Les lignes sans mot sont sautées ; `ligne` compte les
/// lignes gardées à partir de 0.
pub fn depuis_reconnu(r: Reconnu, facteur: f32, l: u32, h: u32) -> Lu {
    let facteur = if facteur.is_finite() && facteur > 0.0 {
        facteur
    } else {
        1.0
    };
    // En pixels d'origine, bornés à [0, max] (NaN : 0) : le coin haut gauche arrondi vers
    // le bas, le bord droit et le bas vers le haut, pour que la boîte couvre le mot.
    let bas = |v: f32, max: u32| -> u32 { ((v / facteur).floor().max(0.0) as u32).min(max) };
    let haut = |v: f32, max: u32| -> u32 { ((v / facteur).ceil().max(0.0) as u32).min(max) };
    let mut lu = Lu {
        langue: r.langue,
        mots: Vec::new(),
        textes: Vec::new(),
        separateurs: Vec::new(),
    };
    let mut ligne = 0u32;
    for brute in r.lignes {
        if brute.mots.is_empty() {
            continue;
        }
        let sep = separateur(&brute);
        for mot in brute.mots {
            let x = bas(mot.x, l);
            let y = bas(mot.y, h);
            let droite = haut(mot.x + mot.largeur.max(0.0), l).max(x);
            let dessous = haut(mot.y + mot.hauteur.max(0.0), h).max(y);
            lu.mots.push(Mot {
                x,
                y,
                largeur: droite - x,
                hauteur: dessous - y,
                ligne,
            });
            lu.textes.push(mot.texte);
            lu.separateurs.push(sep);
        }
        ligne += 1;
    }
    lu
}

/// Le séparateur des mots d'une ligne : `""` si le texte de la ligne est celui des mots
/// collés (chinois, japonais), ou si tous ses mots sont écrits en idéogrammes ou en kana
/// (le moteur peut y mettre des espaces que ces langues n'ont pas) ; sinon `" "`.
pub fn separateur(ligne: &LigneBrute) -> &'static str {
    if ligne.mots.len() < 2 {
        return " ";
    }
    let colles: String = ligne.mots.iter().map(|m| m.texte.as_str()).collect();
    let sans_espace = |m: &str| {
        m.chars().all(|c| {
            matches!(c as u32,
                0x3040..=0x30ff      // hiragana, katakana
                | 0x3000..=0x303f    // ponctuation CJK
                | 0x3400..=0x4dbf    // idéogrammes, extension A
                | 0x4e00..=0x9fff    // idéogrammes
                | 0xf900..=0xfaff    // idéogrammes de compatibilité
                | 0xff00..=0xffef    // pleine chasse
                | 0x20000..=0x2fa1f) // idéogrammes, extensions B et suivantes
        })
    };
    if ligne.texte == colles || ligne.mots.iter().all(|m| sans_espace(&m.texte)) {
        ""
    } else {
        " "
    }
}

/// Tout le texte lu, lignes séparées par `fin_de_ligne`.
pub fn assembler(lu: &Lu, fin_de_ligne: &str) -> String {
    if lu.mots.is_empty() {
        return String::new();
    }
    texte_de(lu, 0, lu.mots.len() - 1, fin_de_ligne)
}

/// Le texte des mots `debut` à `fin` (inclus, ordre de lecture) ; `None` si les bornes
/// sont inversées ou hors de la liste.
pub fn extrait(lu: &Lu, debut: usize, fin: usize, fin_de_ligne: &str) -> Option<String> {
    (debut <= fin && fin < lu.mots.len()).then(|| texte_de(lu, debut, fin, fin_de_ligne))
}

/// Les mots `debut..=fin` (bornes déjà vérifiées) : séparateur de la ligne entre deux mots
/// d'une même ligne, `fin_de_ligne` entre deux lignes.
fn texte_de(lu: &Lu, debut: usize, fin: usize, fin_de_ligne: &str) -> String {
    let mut texte = String::new();
    for i in debut..=fin {
        if i > debut {
            if lu.mots[i].ligne == lu.mots[i - 1].ligne {
                texte.push_str(lu.separateurs[i]);
            } else {
                texte.push_str(fin_de_ligne);
            }
        }
        texte.push_str(&lu.textes[i]);
    }
    texte
}

/// Ce qu'une lecture rend.
#[derive(Clone, PartialEq, Eq)]
pub enum Issue {
    Texte(Lu),
    Vide,
    AucuneLangue,
    /// Cause en code court.
    Echec(String),
}

/// Comme un `Debug` dérivé, mais `Texte` passe par celui de `Lu` (jamais le texte).
impl fmt::Debug for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Issue::Texte(lu) => f.debug_tuple("Texte").field(lu).finish(),
            Issue::Vide => f.write_str("Vide"),
            Issue::AucuneLangue => f.write_str("AucuneLangue"),
            Issue::Echec(code) => f.debug_tuple("Echec").field(code).finish(),
        }
    }
}

/// `preparer`, puis `systeme::texte::reconnaitre`, puis `depuis_reconnu`. Bloquant : dans
/// le fil `texte`, jamais sur le fil principal.
///
/// Pas de moteur sur ce système : `AucuneLangue`. Le moteur n'a pas pu donner sa dimension
/// maximale : `Echec`. Image vide, ou aucun mot lu : `Vide`.
pub fn lire(pixels: &Pixels) -> Issue {
    lire_avec(pixels, moteur::dimension(), moteur::reconnaitre)
}

/// `lire`, avec la dimension maximale et le moteur donnés (pour les tests).
fn lire_avec(
    pixels: &Pixels,
    dimension: Result<u32, ErreurMoteur>,
    reconnaitre: impl FnOnce(&[u8], u32, u32) -> Result<Reconnu, ErreurMoteur>,
) -> Issue {
    let dimension = match dimension {
        Ok(0) => return Issue::Echec("dimension 0x00000000".into()),
        Ok(d) => d,
        Err(ErreurMoteur::AucuneLangue) => return Issue::AucuneLangue,
        Err(ErreurMoteur::Moteur(code)) => return Issue::Echec(code),
    };
    let Some(preparee) = preparer(pixels, dimension) else {
        return Issue::Vide;
    };
    let reconnu = reconnaitre(&preparee.bgra, preparee.largeur, preparee.hauteur);
    drop(preparee.bgra);
    issue(reconnu, preparee.facteur, pixels.largeur, pixels.hauteur)
}

/// Ce que rend le moteur, en `Issue` (pur).
fn issue(reconnu: Result<Reconnu, ErreurMoteur>, facteur: f32, l: u32, h: u32) -> Issue {
    match reconnu {
        Ok(r) => {
            let lu = depuis_reconnu(r, facteur, l, h);
            if lu.textes.iter().all(|t| t.trim().is_empty()) {
                Issue::Vide
            } else {
                Issue::Texte(lu)
            }
        }
        Err(ErreurMoteur::AucuneLangue) => Issue::AucuneLangue,
        Err(ErreurMoteur::Moteur(code)) => Issue::Echec(code),
    }
}
