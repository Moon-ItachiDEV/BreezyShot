//! Texte : la partie commune de « Copier le texte » (contrat 0.5, §5.3), au-dessus du
//! moteur du système (`systeme::texte`). Pur, sauf `lire`. N'importe que `crate::systeme`.
//!
//! Le texte lu ne va jamais au journal ni à une page : seules les boîtes des mots sortent
//! de `Lu` ; le texte n'en sort que par `assembler` et `extrait`, vers le presse-papiers.
//!
//! Socle : signatures (chantier A pour les corps).

use std::time::Duration;

use crate::systeme::gel::Pixels;
use crate::systeme::texte::{LigneBrute, Reconnu};

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
/// tient dans `dimension_max`, réduite si un côté le dépasse). `None` si l'image est vide.
pub fn preparer(_pixels: &Pixels, _dimension_max: u32) -> Option<Preparee> {
    None
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

/// Le résultat d'une lecture. Les textes restent privés.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lu {
    pub langue: String,
    pub mots: Vec<Mot>,
    textes: Vec<String>,
    separateurs: Vec<&'static str>,
}

/// Ramène les boîtes du moteur à l'image d'origine (`facteur`, bornées à `l × h`), dans
/// l'ordre de lecture du moteur.
pub fn depuis_reconnu(r: Reconnu, _facteur: f32, _l: u32, _h: u32) -> Lu {
    Lu {
        langue: r.langue,
        mots: Vec::new(),
        textes: Vec::new(),
        separateurs: Vec::new(),
    }
}

/// Le séparateur des mots d'une ligne : `""` si le texte de la ligne est celui des mots
/// collés (chinois, japonais), sinon `" "`.
pub fn separateur(_ligne: &LigneBrute) -> &'static str {
    " "
}

/// Tout le texte lu, lignes séparées par `fin_de_ligne`.
pub fn assembler(lu: &Lu, _fin_de_ligne: &str) -> String {
    let _ = (&lu.textes, &lu.separateurs);
    String::new()
}

/// Le texte des mots `debut` à `fin` (inclus, ordre de lecture) ; `None` si les bornes
/// sont inversées ou hors de la liste.
pub fn extrait(_lu: &Lu, _debut: usize, _fin: usize, _fin_de_ligne: &str) -> Option<String> {
    None
}

/// Ce qu'une lecture rend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Issue {
    Texte(Lu),
    Vide,
    AucuneLangue,
    /// Cause en code court.
    Echec(String),
}

/// `preparer`, puis `systeme::texte::reconnaitre`, puis `depuis_reconnu`. Bloquant : dans
/// le fil `texte`, jamais sur le fil principal.
pub fn lire(_pixels: &Pixels) -> Issue {
    Issue::AucuneLangue
}
