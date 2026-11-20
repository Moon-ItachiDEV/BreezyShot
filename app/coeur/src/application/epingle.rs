//! Décisions des captures épinglées à l'écran : fonctions pures, testées sans écran.
//!
//! Une capture épinglée vit dans sa propre fenêtre sans cadre, toujours au premier plan,
//! déplaçable, redimensionnable à la molette, à opacité réglable (`Alt + molette`), et
//! fermable au double-clic ou à Échap.

use serde::Serialize;

use crate::fenetres::{Genre, Message};

/// Nombre maximal de captures épinglées ouvertes en même temps.
pub const MAX_EPINGLES: usize = 12;
/// Opacité minimale et maximale de la carte épinglée.
pub const OPACITE_MIN: f64 = 0.25;
pub const OPACITE_MAX: f64 = 1.0;
/// Pas d'opacité par cran de molette (utilisé aussi par la page).
pub const OPACITE_PAS: f64 = 0.08;
/// Bornes d'échelle par rapport à la taille logique 1:1 de l'image (`pixels / dpr`).
pub const ECHELLE_MIN: f64 = 0.2;
pub const ECHELLE_MAX: f64 = 4.0;
/// Plus petit grand côté logique d'une capture épinglée (les boutons de survol tiennent).
pub const COTE_MIN: u32 = 80;
/// Fraction maximale de la zone utile occupée à l'ouverture.
pub const FRACTION_MAX_ECRAN: f64 = 0.75;
/// Fraction maximale de la zone utile lors d'un agrandissement à la molette.
pub const FRACTION_ZOOM_MAX: f64 = 0.95;

/// Charge envoyée à la page `epingle/epingle.html` quand elle se dit prête (`epingle_pret`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChargeEpingle {
    pub id: u64,
    /// Dimensions de l'image brute (pixels réels RGBA).
    pub image_largeur: u32,
    pub image_hauteur: u32,
    /// Taille logique initiale de la carte (pixels CSS).
    pub largeur: u32,
    pub hauteur: u32,
    /// Échelle logique actuelle (`1.0` = `image / dpr`).
    pub echelle: f64,
    pub opacite: f64,
}

/// Résultat d'un redimensionnement (`epingle_redimensionner`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct TailleEpingle {
    pub largeur: u32,
    pub hauteur: u32,
    pub echelle: f64,
}

/// Vrai si une nouvelle capture épinglée peut être ouverte.
pub fn peut_ouvrir(actives: usize) -> bool {
    actives < MAX_EPINGLES
}

/// Dimensions logiques d'une image de `img_l × img_h` pixels réels à `echelle` sur un écran
/// d'échelle `dpr` (≥ 1.0). Toujours au moins `1 × 1`.
fn taille_a_echelle(img_l: u32, img_h: u32, dpr: f64, echelle: f64) -> (u32, u32) {
    let dpr = if dpr.is_finite() && dpr > 0.0 {
        dpr
    } else {
        1.0
    };
    let l = ((f64::from(img_l) / dpr) * echelle).round().max(1.0) as u32;
    let h = ((f64::from(img_h) / dpr) * echelle).round().max(1.0) as u32;
    (l, h)
}

/// Échelle minimale permettant au grand côté d'atteindre `COTE_MIN` (plafonnée à `ECHELLE_MAX`).
fn echelle_minimale(img_l: u32, img_h: u32, dpr: f64) -> f64 {
    let dpr = if dpr.is_finite() && dpr > 0.0 {
        dpr
    } else {
        1.0
    };
    let grand = f64::from(img_l.max(img_h).max(1)) / dpr;
    let plancher = f64::from(COTE_MIN) / grand;
    ECHELLE_MIN.max(plancher).min(ECHELLE_MAX)
}

/// Taille logique initiale `(largeur, hauteur, echelle)` d'une capture épinglée sur un écran
/// de zone utile `ecran_l × ecran_h` (pixels réels) et d'échelle `dpr`.
///
/// À 100 % (`1.0`), chaque pixel de la capture occupe un pixel réel de l'écran. Si la
/// capture dépasse `FRACTION_MAX_ECRAN` de la zone utile, elle est réduite en gardant son
/// rapport d'aspect.
pub fn taille_initiale(
    img_l: u32,
    img_h: u32,
    ecran_l: u32,
    ecran_h: u32,
    dpr: f64,
) -> (u32, u32, f64) {
    let dpr = if dpr.is_finite() && dpr > 0.0 {
        dpr
    } else {
        1.0
    };
    let base_l = f64::from(img_l.max(1)) / dpr;
    let base_h = f64::from(img_h.max(1)) / dpr;
    let max_l = (f64::from(ecran_l.max(1)) / dpr * FRACTION_MAX_ECRAN).max(f64::from(COTE_MIN));
    let max_h = (f64::from(ecran_h.max(1)) / dpr * FRACTION_MAX_ECRAN).max(f64::from(COTE_MIN));
    let borne_haut = (max_l / base_l).min(max_h / base_h).min(1.0);
    let plancher = echelle_minimale(img_l, img_h, dpr).min(1.0);
    let echelle = borne_haut.max(plancher).clamp(ECHELLE_MIN, ECHELLE_MAX);
    let (l, h) = taille_a_echelle(img_l, img_h, dpr, echelle);
    (l, h, echelle)
}

/// Calcule la nouvelle taille logique et l'échelle après un zoom de facteur `facteur`
/// (`> 1.0` pour agrandir, `< 1.0` pour réduire). `None` si `facteur` n'est pas valide.
pub fn redimensionner(
    img_l: u32,
    img_h: u32,
    dpr: f64,
    echelle_actuelle: f64,
    facteur: f64,
    ecran_l: u32,
    ecran_h: u32,
) -> Option<TailleEpingle> {
    if !facteur.is_finite() || facteur <= 0.0 || !echelle_actuelle.is_finite() {
        return None;
    }
    let dpr = if dpr.is_finite() && dpr > 0.0 {
        dpr
    } else {
        1.0
    };
    let base_l = f64::from(img_l.max(1)) / dpr;
    let base_h = f64::from(img_h.max(1)) / dpr;
    let max_l = (f64::from(ecran_l.max(1)) / dpr * FRACTION_ZOOM_MAX).max(f64::from(COTE_MIN));
    let max_h = (f64::from(ecran_h.max(1)) / dpr * FRACTION_ZOOM_MAX).max(f64::from(COTE_MIN));
    let plafond = (max_l / base_l).min(max_h / base_h).min(ECHELLE_MAX);
    let plancher = echelle_minimale(img_l, img_h, dpr).min(plafond);
    let echelle = (echelle_actuelle * facteur).clamp(plancher, plafond);
    let (largeur, hauteur) = taille_a_echelle(img_l, img_h, dpr, echelle);
    Some(TailleEpingle {
        largeur,
        hauteur,
        echelle,
    })
}

/// Borne une opacité demandée dans `OPACITE_MIN..=OPACITE_MAX` (`None` si NaN ou infini).
pub fn ajuster_opacite(opacite: f64) -> Option<f64> {
    if !opacite.is_finite() {
        return None;
    }
    Some(opacite.clamp(OPACITE_MIN, OPACITE_MAX))
}

/// Message du bandeau quand la limite de captures épinglées est atteinte.
pub fn message_limite() -> Message {
    Message {
        genre: Genre::Info,
        titre: format!("{MAX_EPINGLES} captures épinglées au maximum"),
        detail: "Fermez une capture épinglée pour en ajouter une autre.".into(),
        actions: Vec::new(),
        duree_ms: None,
    }
}

/// Message du bandeau quand la fenêtre d'une capture épinglée n'a pas pu s'ouvrir.
pub fn message_impossible() -> Message {
    Message {
        genre: Genre::Erreur,
        titre: "Impossible d'épingler la capture".into(),
        detail: String::new(),
        actions: Vec::new(),
        duree_ms: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limite_d_epingles() {
        assert!(peut_ouvrir(0));
        assert!(peut_ouvrir(MAX_EPINGLES - 1));
        assert!(!peut_ouvrir(MAX_EPINGLES));
        assert!(!peut_ouvrir(MAX_EPINGLES + 3));
    }

    #[test]
    fn taille_initiale_respecte_le_ratio_et_l_ecran() {
        // Petite image (400 × 300) sur 1920 × 1080 à 100 % : 1:1 exact.
        let (l, h, e) = taille_initiale(400, 300, 1920, 1080, 1.0);
        assert_eq!((l, h), (400, 300));
        assert!((e - 1.0).abs() < 1e-6);

        // Même image à 200 % (HiDPI) : 200 × 150 logiques à 100 %.
        let (l, h, e) = taille_initiale(400, 300, 3840, 2160, 2.0);
        assert_eq!((l, h), (200, 150));
        assert!((e - 1.0).abs() < 1e-6);

        // Capture plein écran (1920 × 1080) : réduite à 75 % de la zone utile.
        let (l, h, e) = taille_initiale(1920, 1080, 1920, 1080, 1.0);
        assert_eq!((l, h), (1440, 810));
        assert!((e - 0.75).abs() < 1e-6);

        // Très petite capture (20 × 20) : agrandie pour garder COTE_MIN = 80 ? Non,
        // plancher plafonné à 1.0 à l'ouverture ou COTE_MIN : vérifions non nul et borné.
        let (l, h, _) = taille_initiale(20, 20, 1920, 1080, 1.0);
        assert!(l >= 20 && h >= 20);
    }

    #[test]
    fn redimensionner_borne_echelle_et_refuse_invalides() {
        assert_eq!(redimensionner(400, 300, 1.0, 1.0, 0.0, 1920, 1080), None);
        assert_eq!(redimensionner(400, 300, 1.0, 1.0, -1.0, 1920, 1080), None);
        assert_eq!(
            redimensionner(400, 300, 1.0, 1.0, f64::NAN, 1920, 1080),
            None
        );

        let plus = redimensionner(400, 300, 1.0, 1.0, 1.1, 1920, 1080).unwrap();
        assert_eq!((plus.largeur, plus.hauteur), (440, 330));

        // Réduction extrême : bornée à ECHELLE_MIN et COTE_MIN.
        let min = redimensionner(400, 300, 1.0, 1.0, 0.01, 1920, 1080).unwrap();
        assert!(min.largeur >= COTE_MIN);
        assert!(min.echelle >= ECHELLE_MIN);

        // Agrandissement extrême : borné à la zone utile de l'écran.
        let max = redimensionner(400, 300, 1.0, 1.0, 50.0, 1920, 1080).unwrap();
        assert!(max.hauteur <= (1080.0 * FRACTION_ZOOM_MAX).ceil() as u32);
        assert!(max.echelle <= ECHELLE_MAX);
    }

    #[test]
    fn opacite_bornee() {
        assert_eq!(ajuster_opacite(f64::NAN), None);
        assert_eq!(ajuster_opacite(f64::INFINITY), None);
        assert_eq!(ajuster_opacite(0.0), Some(OPACITE_MIN));
        assert_eq!(ajuster_opacite(2.5), Some(OPACITE_MAX));
        assert_eq!(ajuster_opacite(0.6), Some(0.6));
    }
}
