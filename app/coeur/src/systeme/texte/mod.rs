//! Reconnaissance de texte du système (contrat 0.5, §5.2).
//!
//! Façade commune : `windows.rs` passe par `Windows.Media.Ocr` (toujours hors du fil
//! principal) ; `autre.rs` répond « non pris en charge » ailleurs (Linux, et macOS jusqu'à
//! `macos.rs`, par Vision). Les langues sont celles du système : aucun modèle embarqué.
//!
//! Le moteur est créé à chaque lecture puis relâché (R15 : 4 ms mesurés sur la machine
//! d'essai), pour que la mémoire au repos ne change pas. Une lecture qui ne rend pas est
//! abandonnée après `ATTENTE_MAX` ; au plus `LECTURES_MAX` lectures à la fois.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

#[cfg(windows)]
#[path = "windows.rs"]
mod plateforme;

#[cfg(not(windows))]
#[path = "autre.rs"]
mod plateforme;

/// Au-delà, le moteur qui n'a pas rendu sa lecture est laissé (opération annulée) et la
/// lecture rend `Moteur("echeance")` : le fil qui l'attendait finit. Même durée que
/// `texte::DELAI_LECTURE`, que `systeme` ne peut pas lire.
pub const ATTENTE_MAX: Duration = Duration::from_secs(10);

/// Lectures en cours à la fois, au plus : au-delà, `Moteur("deja_en_cours")`.
pub const LECTURES_MAX: usize = 2;

/// Les lectures en cours (toutes plateformes).
static EN_COURS: AtomicUsize = AtomicUsize::new(0);

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
    if essai_sans_texte() {
        return Disponibilite::Aucune;
    }
    plateforme::disponibilite()
}

/// Le plus grand côté d'image accepté par le moteur (`OcrEngine::MaxImageDimension`, lu
/// une fois). 0 : pas de moteur.
pub fn dimension_max() -> u32 {
    plateforme::dimension_max()
}

/// Lit le texte d'une image BGRA 8 bits (`largeur × hauteur × 4` octets). Bloquant : à
/// appeler dans un fil à part, jamais sur le fil principal (au plus `ATTENTE_MAX`).
pub fn reconnaitre(bgra: &[u8], largeur: u32, hauteur: u32) -> Result<Reconnu, ErreurMoteur> {
    if essai_sans_texte() {
        return Err(ErreurMoteur::AucuneLangue);
    }
    let taille = (largeur as usize)
        .checked_mul(hauteur as usize)
        .and_then(|n| n.checked_mul(4));
    if largeur == 0 || hauteur == 0 || taille != Some(bgra.len()) {
        return Err(ErreurMoteur::Moteur("taille".into()));
    }
    let Some(_place) = Place::prendre(&EN_COURS, LECTURES_MAX) else {
        return Err(ErreurMoteur::Moteur("deja_en_cours".into()));
    };
    plateforme::reconnaitre(bgra, largeur, hauteur)
}

/// Durée de création du moteur pour la dernière lecture faite dans ce fil (journal
/// `texte moteur cree en <ms> ms`). `None` : aucun moteur créé dans ce fil.
pub fn derniere_creation() -> Option<Duration> {
    plateforme::derniere_creation()
}

/// La fin de ligne du texte copié : `"\r\n"` sous Windows, `"\n"` ailleurs.
pub fn fin_de_ligne() -> &'static str {
    plateforme::FIN_DE_LIGNE
}

/// Vrai si `BREEZYSHOT_ESSAI_SANS_TEXTE=1` (essais seulement).
pub fn essai_sans_texte() -> bool {
    std::env::var_os(VARIABLE_SANS_TEXTE).is_some_and(|v| v == "1")
}

/// Une place parmi les lectures en cours, rendue à la destruction.
struct Place<'a>(&'a AtomicUsize);

impl<'a> Place<'a> {
    /// Prend une place si moins de `max` sont prises.
    fn prendre(compteur: &'a AtomicUsize, max: usize) -> Option<Place<'a>> {
        compteur
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < max).then_some(n + 1)
            })
            .ok()
            .map(|_| Place(compteur))
    }
}

impl Drop for Place<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_bornees_et_rendues() {
        let compteur = AtomicUsize::new(0);
        let a = Place::prendre(&compteur, 2).expect("première place");
        let b = Place::prendre(&compteur, 2).expect("seconde place");
        assert!(Place::prendre(&compteur, 2).is_none());
        drop(a);
        let c = Place::prendre(&compteur, 2).expect("place rendue");
        drop((b, c));
        assert_eq!(compteur.load(Ordering::Acquire), 0);
    }

    #[test]
    fn taille_fausse_refusee_sans_moteur() {
        let attendu = Err(ErreurMoteur::Moteur("taille".into()));
        assert_eq!(reconnaitre(&[0; 15], 2, 2), attendu);
        assert_eq!(reconnaitre(&[], 0, 3), attendu);
        assert_eq!(reconnaitre(&[], 3, 0), attendu);
    }

    #[test]
    fn fin_de_ligne_du_systeme() {
        let attendue = if cfg!(windows) { "\r\n" } else { "\n" };
        assert_eq!(fin_de_ligne(), attendue);
    }

    #[cfg(not(windows))]
    #[test]
    fn hors_windows_rien() {
        // Sans la variable d'essai : `NonPrisEnCharge` ; avec : `Aucune`.
        assert!(matches!(
            disponibilite(),
            Disponibilite::NonPrisEnCharge | Disponibilite::Aucune
        ));
        assert_eq!(dimension_max(), 0);
        assert_eq!(derniere_creation(), None);
        assert_eq!(
            reconnaitre(&[255; 16], 2, 2),
            Err(ErreurMoteur::AucuneLangue)
        );
    }
}
