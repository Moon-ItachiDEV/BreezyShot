//! Le décompte du retardateur (0.4) dans la fenêtre du bandeau : ce qu'on montre et où.
//! Calculs purs, en pixels réels d'écran sauf le diamètre (pixels logiques de la page).
//!
//! Socle : les signatures du contrat ; le chantier « Affichage du décompte » les complète
//! (page `vignette/decompte.*`, `Bandeau::montrer_decompte`).

use std::time::Duration;

use super::place::{Ecran, Rect};

/// Diamètre du disque, en pixels logiques : au plus (et au centre d'un écran).
pub const DIAMETRE_MAX: f64 = 88.0;
/// Diamètre du disque, en pixels logiques : au moins.
pub const DIAMETRE_MIN: f64 = 64.0;

/// Un décompte à montrer.
#[derive(Debug, Clone, PartialEq)]
pub struct Decompte {
    /// Numéro du décompte (donné par l'Application).
    pub numero: u64,
    /// Durée totale du retardateur, en secondes.
    pub secondes: u32,
    /// Temps qui reste au moment de l'envoi.
    pub restant: Duration,
    /// Centre de la cible (pixels réels du bureau).
    pub centre: (i32, i32),
    /// Petit côté de la dernière zone (pixels réels) ; `None` : centre d'un écran.
    pub cote: Option<i32>,
    /// Animations réduites.
    pub reduites: bool,
}

/// Diamètre du disque : la moitié du petit côté de la zone, entre 64 et 88 pixels
/// logiques ; sans zone (ou valeur absurde), 88.
pub fn diametre_logique(cote_logique: Option<f64>) -> f64 {
    match cote_logique {
        Some(c) if c.is_finite() => (c * 0.5).clamp(DIAMETRE_MIN, DIAMETRE_MAX),
        _ => DIAMETRE_MAX,
    }
}

/// Place de la fenêtre du décompte : centrée sur `centre`, taille logique `taille`
/// convertie avec l'échelle de l'écran, bornée à sa zone utile.
pub fn placer(centre: (i32, i32), ecran: &Ecran, taille: (f64, f64)) -> Rect {
    let e = if ecran.echelle.is_finite() && ecran.echelle > 0.0 {
        ecran.echelle
    } else {
        1.0
    };
    let zone = ecran.zone_utile;
    let largeur = ((taille.0 * e).round().max(1.0) as u32).min(zone.largeur.max(1));
    let hauteur = ((taille.1 * e).round().max(1.0) as u32).min(zone.hauteur.max(1));
    let borner = |c: i32, debut: i32, cote_zone: u32, cote: u32| -> i32 {
        let voulu = i64::from(c) - i64::from(cote) / 2;
        let max = i64::from(debut) + i64::from(cote_zone) - i64::from(cote);
        voulu.clamp(i64::from(debut), max.max(i64::from(debut))) as i32
    };
    Rect {
        x: borner(centre.0, zone.x, zone.largeur, largeur),
        y: borner(centre.1, zone.y, zone.hauteur, hauteur),
        largeur,
        hauteur,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diametre_borne() {
        assert_eq!(diametre_logique(None), 88.0);
        assert_eq!(diametre_logique(Some(40.0)), 64.0);
        assert_eq!(diametre_logique(Some(150.0)), 75.0);
        assert_eq!(diametre_logique(Some(1000.0)), 88.0);
        assert_eq!(diametre_logique(Some(f64::NAN)), 88.0);
    }

    #[test]
    fn place_centree_puis_bornee() {
        let r = Rect {
            x: 0,
            y: 0,
            largeur: 1000,
            hauteur: 800,
        };
        let ecran = Ecran {
            rect: r,
            zone_utile: r,
            echelle: 1.0,
        };
        assert_eq!(
            placer((500, 400), &ecran, (560.0, 220.0)),
            Rect {
                x: 220,
                y: 290,
                largeur: 560,
                hauteur: 220
            }
        );
        let coin = placer((0, 799), &ecran, (560.0, 220.0));
        assert_eq!((coin.x, coin.y), (0, 580));
    }
}
