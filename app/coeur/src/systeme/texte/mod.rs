//! Reconnaissance de texte du système (contrat 0.5, §5.2).
//!
//! Façade commune : `windows.rs` passe par `Windows.Media.Ocr` (toujours hors du fil
//! principal) ; `autre.rs` répond « non pris en charge » ailleurs (Linux, et macOS jusqu'à
//! `macos.rs`, par Vision). Les langues sont celles du système : aucun modèle embarqué.

#[cfg(windows)]
#[path = "windows.rs"]
mod plateforme;

#[cfg(not(windows))]
#[path = "autre.rs"]
mod plateforme;

/// Variable d'essai : fait comme si aucune langue de lecture n'existait (essais seulement).
pub const VARIABLE_SANS_TEXTE: &str = "BREEZYSHOT_ESSAI_SANS_TEXTE";

/// Ce que le système sait lire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Disponibilite {
    /// Étiquettes BCP-47 des langues de lecture (`fr-FR`, `en-US`).
    Langues(Vec<String>),
    /// Le moteur existe, mais aucune langue de lecture n'est installée.
    Aucune,
    /// Pas de moteur sur ce système.
    NonPrisEnCharge,
}

/// Un mot reconnu, en pixels de l'image donnée au moteur.
#[derive(Debug, Clone, PartialEq)]
pub struct MotBrut {
    pub texte: String,
    pub x: f32,
    pub y: f32,
    pub largeur: f32,
    pub hauteur: f32,
}

/// Une ligne reconnue : son texte tel que le moteur l'assemble, et ses mots.
#[derive(Debug, Clone, PartialEq)]
pub struct LigneBrute {
    pub texte: String,
    pub mots: Vec<MotBrut>,
}

/// Ce que le moteur a lu, dans son ordre de lecture.
#[derive(Debug, Clone, PartialEq)]
pub struct Reconnu {
    pub langue: String,
    pub lignes: Vec<LigneBrute>,
}

/// Pourquoi la lecture n'a pas pu se faire. La cause est un code court, jamais du texte lu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErreurMoteur {
    AucuneLangue,
    Moteur(String),
}

/// Les langues de lecture (`BREEZYSHOT_ESSAI_SANS_TEXTE=1` : `Aucune`).
pub fn disponibilite() -> Disponibilite {
    plateforme::disponibilite()
}

/// Le plus grand côté d'image accepté par le moteur (`OcrEngine::MaxImageDimension`).
pub fn dimension_max() -> u32 {
    plateforme::dimension_max()
}

/// Lit le texte d'une image BGRA 8 bits (`largeur × hauteur × 4` octets). Bloquant : à
/// appeler dans un fil à part, jamais sur le fil principal.
pub fn reconnaitre(bgra: &[u8], largeur: u32, hauteur: u32) -> Result<Reconnu, ErreurMoteur> {
    plateforme::reconnaitre(bgra, largeur, hauteur)
}

/// La fin de ligne du texte copié : `"\r\n"` sous Windows, `"\n"` ailleurs.
pub fn fin_de_ligne() -> &'static str {
    plateforme::FIN_DE_LIGNE
}

/// Vrai si `BREEZYSHOT_ESSAI_SANS_TEXTE=1` (essais seulement).
pub fn essai_sans_texte() -> bool {
    std::env::var_os(VARIABLE_SANS_TEXTE).is_some_and(|v| v == "1")
}
