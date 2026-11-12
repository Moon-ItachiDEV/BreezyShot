//! Capture : figer le bureau et découper une zone.
//!
//! Aucun code propre à un système ici : la photo est prise par `systeme::gel`, qui seul
//! sait comment (GDI sous Windows). Ce module assemble le résultat, chronomètre, et fait
//! la géométrie et la découpe en fonctions pures. `Pixels` est le seul format qui sort.
//! Il n'écrit rien sur le disque ni dans le journal : l'Application écrira les durées
//! (`chrono gel photo <ms> total <ms>`).

pub mod decoupe;
pub mod fenetre;
pub mod geometrie;

use std::fmt;
use std::time::{Duration, Instant};

use crate::systeme::gel::{self, EcranInfo, FenetreInfo, ImageFigee, Pixels};
use crate::systeme::Rect;

pub use decoupe::arrondir;
pub use fenetre::{coins_a_arrondir, fenetre_sous, rayon_coins, zone_fenetre};
pub use geometrie::{borner_a_un_ecran, ecran_sous, rect_entre, zone_valide, COTE_MIN};

/// Un bureau figé : la photo, et ce qu'il faut pour choisir une zone ou une fenêtre.
#[derive(Debug)]
pub struct Gel {
    pub image: ImageFigee,
    /// Le bureau virtuel photographié.
    pub bureau: Rect,
    /// Triés de gauche à droite puis de haut en bas.
    pub ecrans: Vec<EcranInfo>,
    /// La copie du bureau seule (le `BitBlt`).
    pub duree_photo: Duration,
    /// Photo (allocation de l'image comprise) et liste des écrans.
    pub duree_totale: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Erreur {
    /// Le Gel n'existe pas sur ce système.
    NonDisponible,
    /// La photo a échoué.
    Photo(String),
    /// Zone trop petite pour être une capture.
    ZoneVide,
    /// Zone sans rien de commun avec le bureau.
    ZoneHorsBureau,
    /// Les pixels n'ont pas pu être lus.
    Lecture(String),
}

impl fmt::Display for Erreur {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Erreur::NonDisponible => write!(f, "gel non disponible sur ce systeme"),
            Erreur::Photo(e) => write!(f, "photo du bureau impossible: {e}"),
            Erreur::ZoneVide => write!(f, "zone trop petite"),
            Erreur::ZoneHorsBureau => write!(f, "zone hors du bureau"),
            Erreur::Lecture(e) => write!(f, "lecture des pixels impossible: {e}"),
        }
    }
}

impl std::error::Error for Erreur {}

/// Fige le bureau : photo de tous les écrans, puis la liste des écrans (nécessaire au
/// bornage, coût négligeable). Rien de plus : la liste des fenêtres se demande à part
/// (`fenetres`), après avoir montré l'écran figé. Depuis n'importe quel fil
/// (l'affichage de l'écran figé, lui, se fait sur le fil principal, par
/// `systeme::gel::montrer_ecran_fige`).
pub fn figer() -> Result<Gel, Erreur> {
    if !gel::disponible() {
        return Err(Erreur::NonDisponible);
    }
    let debut = Instant::now();
    let image = gel::figer_bureau().map_err(Erreur::Photo)?;
    let ecrans = gel::ecrans();
    let duree_totale = debut.elapsed();
    Ok(Gel {
        bureau: image.rect(),
        duree_photo: image.duree_photo(),
        image,
        ecrans,
        duree_totale,
    })
}

/// Les fenêtres visibles des autres apps, du premier plan vers le fond (mode « fenêtre »).
/// À demander après avoir montré l'écran figé : leur liste coûte plus que la photo.
pub fn fenetres() -> Vec<FenetreInfo> {
    gel::fenetres_visibles()
}

/// Les pixels d'une zone du bureau figé (coordonnées du bureau virtuel).
pub fn decouper(gel: &Gel, zone: Rect) -> Result<Pixels, Erreur> {
    let coupee = decoupe::verifier_zone(zone, gel.bureau)?;
    gel.image.lire(coupee).map_err(Erreur::Lecture)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erreurs_lisibles() {
        assert_eq!(Erreur::ZoneVide.to_string(), "zone trop petite");
        assert!(Erreur::Photo("x".into()).to_string().ends_with(": x"));
    }

    /// Hors Windows, le Gel répond « non disponible », sans panique.
    #[test]
    fn figer_sans_gel_dit_non_disponible() {
        if !gel::disponible() {
            assert_eq!(figer().err(), Some(Erreur::NonDisponible));
        }
    }
}
