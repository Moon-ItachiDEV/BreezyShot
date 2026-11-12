//! Le décompte du retardateur (0.4) dans la fenêtre du bandeau : ce qu'on montre et où.
//! Calculs purs, en pixels réels d'écran sauf le diamètre (pixels logiques de la page).
//!
//! La page est `vignette/decompte.js` (chargée par `bandeau.html`) ; les gestes sont dans
//! `Bandeau::montrer_decompte` et `Bandeau::cacher_decompte`.

use std::time::Duration;

use serde::Serialize;

use super::place::{Ecran, Rect};

/// Événement écouté par la page : un décompte à montrer (`ChargeDecompte`).
pub const EVENEMENT_DECOMPTE: &str = "bandeau:decompte";
/// Événement écouté par la page : le décompte `{ id }` est caché (la page efface le disque).
pub const EVENEMENT_DECOMPTE_CACHE: &str = "bandeau:decompte_cache";

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

/// Ce que la page reçoit (`bandeau:decompte`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChargeDecompte {
    /// Identifiant du bandeau (même compteur que les messages).
    pub id: u64,
    pub secondes: u32,
    /// Temps qui reste au moment de l'envoi à la page, en millisecondes.
    pub restant_ms: u64,
    /// Diamètre du disque, pixels logiques.
    pub diametre: f64,
    pub reduites: bool,
}

/// La charge envoyée à la page : `ecoule` est le temps passé depuis la demande (page pas
/// encore prête, renvoi après un rechargement) ; `echelle` celle de l'écran de la fenêtre,
/// qui convertit le petit côté de la zone en pixels logiques.
pub fn charge(id: u64, d: &Decompte, ecoule: Duration, echelle: f64) -> ChargeDecompte {
    let e = if echelle.is_finite() && echelle > 0.0 {
        echelle
    } else {
        1.0
    };
    let restant = d.restant.saturating_sub(ecoule);
    ChargeDecompte {
        id,
        secondes: d.secondes,
        restant_ms: u64::try_from(restant.as_millis()).unwrap_or(u64::MAX),
        diametre: diametre_logique(d.cote.map(|c| f64::from(c) / e)),
        reduites: d.reduites,
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

    fn rect(x: i32, y: i32, largeur: u32, hauteur: u32) -> Rect {
        Rect {
            x,
            y,
            largeur,
            hauteur,
        }
    }

    #[test]
    fn place_aux_quatre_bords_de_la_zone_utile() {
        // Écran à gauche du principal, barre des tâches en bas (40 px).
        let ecran = Ecran {
            rect: rect(-1920, -200, 1920, 1080),
            zone_utile: rect(-1920, -200, 1920, 1040),
            echelle: 1.0,
        };
        let t = (560.0, 220.0);
        // Gauche, droite, haut, bas : collée au bord de la zone utile, jamais au-delà.
        assert_eq!(placer((-1900, 300), &ecran, t), rect(-1920, 190, 560, 220));
        assert_eq!(placer((-5, 300), &ecran, t), rect(-560, 190, 560, 220));
        assert_eq!(
            placer((-1000, -190), &ecran, t),
            rect(-1280, -200, 560, 220)
        );
        assert_eq!(placer((-1000, 879), &ecran, t), rect(-1280, 620, 560, 220));
        // Un centre hors de l'écran est ramené dedans.
        assert_eq!(placer((9000, 9000), &ecran, t), rect(-560, 620, 560, 220));
    }

    #[test]
    fn place_a_150_pour_cent() {
        let ecran = Ecran {
            rect: rect(0, 0, 2880, 1800),
            zone_utile: rect(0, 72, 2880, 1728),
            echelle: 1.5,
        };
        // 560 × 220 logiques : 840 × 330 réels, centrée sur la cible.
        assert_eq!(
            placer((1440, 900), &ecran, (560.0, 220.0)),
            rect(1020, 735, 840, 330)
        );
        // Barre des tâches en haut : jamais au-dessus de la zone utile.
        assert_eq!(
            placer((10, 0), &ecran, (560.0, 220.0)),
            rect(0, 72, 840, 330)
        );
    }

    #[test]
    fn place_bornee_a_une_zone_plus_petite_que_la_fenetre() {
        let ecran = Ecran {
            rect: rect(0, 0, 400, 200),
            zone_utile: rect(0, 0, 400, 200),
            echelle: f64::NAN,
        };
        assert_eq!(
            placer((200, 100), &ecran, (560.0, 220.0)),
            rect(0, 0, 400, 200)
        );
    }

    fn exemple() -> Decompte {
        Decompte {
            numero: 4,
            secondes: 3,
            restant: Duration::from_millis(3000),
            centre: (0, 0),
            cote: Some(300),
            reduites: true,
        }
    }

    #[test]
    fn charge_du_temps_restant_et_du_diametre() {
        let d = exemple();
        let c = charge(9, &d, Duration::from_millis(250), 1.5);
        assert_eq!(
            c,
            ChargeDecompte {
                id: 9,
                secondes: 3,
                restant_ms: 2750,
                // 300 réels à 150 % : 200 logiques, la moitié vaut 100, bornée à 88.
                diametre: 88.0,
                reduites: true,
            }
        );
        // Petite zone à 100 % : 150 réels, diamètre 75.
        let petite = Decompte {
            cote: Some(150),
            ..exemple()
        };
        assert_eq!(charge(1, &petite, Duration::ZERO, 1.0).diametre, 75.0);
        // Même zone à 200 % : 75 logiques, diamètre au plus petit.
        assert_eq!(charge(1, &petite, Duration::ZERO, 2.0).diametre, 64.0);
        // Échelle absurde : 1 ; temps écoulé au-delà : 0, jamais négatif.
        assert_eq!(charge(1, &petite, Duration::ZERO, 0.0).diametre, 75.0);
        assert_eq!(charge(1, &d, Duration::from_secs(9), 1.0).restant_ms, 0);
        // Centre d'écran (aucune zone) : 88.
        let centre = Decompte {
            cote: None,
            ..exemple()
        };
        assert_eq!(charge(1, &centre, Duration::ZERO, 1.0).diametre, 88.0);
    }

    #[test]
    fn charge_serialisee_pour_la_page() {
        let c = charge(9, &exemple(), Duration::ZERO, 1.0);
        let json = serde_json::to_value(&c).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "id": 9, "secondes": 3, "restant_ms": 3000, "diametre": 88.0, "reduites": true
            })
        );
    }
}
