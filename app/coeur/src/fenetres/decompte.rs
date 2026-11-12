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

/// Sous le disque, la croix « Annuler » : écart plus côté, en pixels logiques (jetons
/// `decompte-ecart` et `decompte-croix` de la page).
pub const CROIX_SOUS_DISQUE: f64 = 10.0 + 26.0;
/// Place laissée à l'ombre du disque et de la croix (jeton `shadow-8`), pixels logiques.
pub const MARGE_OMBRE: f64 = 8.0;

/// Déplacement du disque depuis le centre de la page, en pixels logiques (x vers la droite,
/// y vers le bas).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Decalage {
    pub x: f64,
    pub y: f64,
}

/// Le disque reste sur sa cible quand la fenêtre a été ramenée dans la zone utile : écart
/// entre le centre de la cible et celui de la fenêtre `fenetre` (pixels réels), converti en
/// pixels logiques, borné pour que le disque, sa croix et leur ombre restent entièrement
/// dans la fenêtre. Nul au centre d'un écran.
pub fn decalage(centre: (i32, i32), fenetre: Rect, echelle: f64, diametre: f64) -> Decalage {
    let e = echelle_sure(echelle);
    let (l, h) = (
        f64::from(fenetre.largeur) / e,
        f64::from(fenetre.hauteur) / e,
    );
    let milieu_x = f64::from(fenetre.x) + f64::from(fenetre.largeur) / 2.0;
    let milieu_y = f64::from(fenetre.y) + f64::from(fenetre.hauteur) / 2.0;
    let voulu_x = (f64::from(centre.0) - milieu_x) / e;
    let voulu_y = (f64::from(centre.1) - milieu_y) / e;
    let r = diametre / 2.0;
    // Bornes ; une fenêtre trop petite (bornes croisées) garde le disque au milieu.
    let borner = |v: f64, min: f64, max: f64| {
        if !v.is_finite() || min > max {
            0.0
        } else {
            v.clamp(min, max)
        }
    };
    let x_max = l / 2.0 - r - MARGE_OMBRE;
    let y_min = r + MARGE_OMBRE - h / 2.0;
    let y_max = h / 2.0 - r - CROIX_SOUS_DISQUE - MARGE_OMBRE;
    Decalage {
        x: borner(voulu_x, -x_max, x_max),
        y: borner(voulu_y, y_min, y_max),
    }
}

fn echelle_sure(echelle: f64) -> f64 {
    if echelle.is_finite() && echelle > 0.0 {
        echelle
    } else {
        1.0
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
    /// Place du disque dans la page (`decalage`).
    pub decalage: Decalage,
    pub reduites: bool,
}

/// La charge envoyée à la page : `ecoule` est le temps passé depuis la demande (page pas
/// encore prête, renvoi après un rechargement) ; `place` la fenêtre posée et l'échelle de
/// son écran (qui convertit le petit côté de la zone en pixels logiques). Sans place
/// (écran inconnu) : échelle 1, disque au milieu.
pub fn charge(
    id: u64,
    d: &Decompte,
    ecoule: Duration,
    place: Option<(Rect, f64)>,
) -> ChargeDecompte {
    let e = echelle_sure(place.map_or(1.0, |(_, e)| e));
    let restant = d.restant.saturating_sub(ecoule);
    let diametre = diametre_logique(d.cote.map(|c| f64::from(c) / e));
    ChargeDecompte {
        id,
        secondes: d.secondes,
        restant_ms: u64::try_from(restant.as_millis()).unwrap_or(u64::MAX),
        diametre,
        decalage: place.map_or(Decalage { x: 0.0, y: 0.0 }, |(f, _)| {
            decalage(d.centre, f, e, diametre)
        }),
        reduites: d.reduites,
    }
}

/// Place de la fenêtre du décompte : centrée sur `centre`, taille logique `taille`
/// convertie avec l'échelle de l'écran, bornée à sa zone utile.
pub fn placer(centre: (i32, i32), ecran: &Ecran, taille: (f64, f64)) -> Rect {
    let e = echelle_sure(ecran.echelle);
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
        let a = |e: f64| Some((rect(-280, -110, 560, 220), e));
        let d = exemple();
        // Fenêtre de 840 × 330 à 150 % centrée sur la cible (0, 0) : aucun décalage.
        let f = rect(-420, -165, 840, 330);
        let c = charge(9, &d, Duration::from_millis(250), Some((f, 1.5)));
        assert_eq!(
            c,
            ChargeDecompte {
                id: 9,
                secondes: 3,
                restant_ms: 2750,
                // 300 réels à 150 % : 200 logiques, la moitié vaut 100, bornée à 88.
                diametre: 88.0,
                decalage: Decalage { x: 0.0, y: 0.0 },
                reduites: true,
            }
        );
        // Petite zone à 100 % : 150 réels, diamètre 75.
        let petite = Decompte {
            cote: Some(150),
            ..exemple()
        };
        assert_eq!(charge(1, &petite, Duration::ZERO, a(1.0)).diametre, 75.0);
        // Même zone à 200 % : 75 logiques, diamètre au plus petit.
        assert_eq!(charge(1, &petite, Duration::ZERO, a(2.0)).diametre, 64.0);
        // Échelle absurde : 1 ; temps écoulé au-delà : 0, jamais négatif.
        assert_eq!(charge(1, &petite, Duration::ZERO, a(0.0)).diametre, 75.0);
        assert_eq!(charge(1, &d, Duration::from_secs(9), None).restant_ms, 0);
        // Centre d'écran (aucune zone) : 88.
        let centre = Decompte {
            cote: None,
            ..exemple()
        };
        assert_eq!(charge(1, &centre, Duration::ZERO, None).diametre, 88.0);
    }

    #[test]
    fn charge_serialisee_pour_la_page() {
        let c = charge(9, &exemple(), Duration::ZERO, None);
        let json = serde_json::to_value(&c).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "id": 9, "secondes": 3, "restant_ms": 3000, "diametre": 88.0,
                "decalage": { "x": 0.0, "y": 0.0 }, "reduites": true
            })
        );
    }

    fn ecran_gauche() -> Ecran {
        // Écran à gauche du principal, barre des tâches en bas (40 px), 100 %.
        Ecran {
            rect: rect(-1920, -200, 1920, 1080),
            zone_utile: rect(-1920, -200, 1920, 1040),
            echelle: 1.0,
        }
    }

    /// Décalage d'un décompte de diamètre 88 centré sur `centre`, fenêtre posée par `placer`.
    fn decale(centre: (i32, i32), ecran: &Ecran) -> Decalage {
        let f = placer(centre, ecran, (560.0, 220.0));
        decalage(centre, f, ecran.echelle, 88.0)
    }

    #[test]
    fn decalage_nul_au_centre_d_un_ecran() {
        assert_eq!(
            decale((-960, 320), &ecran_gauche()),
            Decalage { x: 0.0, y: 0.0 }
        );
    }

    #[test]
    fn decalage_aux_quatre_bords_et_dans_un_coin() {
        let e = ecran_gauche();
        // Bord gauche : cible à 20 px du bord, fenêtre collée au bord (centre à 280 px).
        assert_eq!(decale((-1900, 320), &e), Decalage { x: -228.0, y: 0.0 });
        // Assez loin du bord : le disque est exactement sur la cible.
        assert_eq!(decale((-1800, 320), &e), Decalage { x: -160.0, y: 0.0 });
        // Bord droit.
        assert_eq!(decale((-30, 320), &e), Decalage { x: 228.0, y: 0.0 });
        // Haut : le disque ne sort pas en haut (rayon 44 + ombre 8 depuis le milieu 110).
        assert_eq!(decale((-960, -190), &e), Decalage { x: 0.0, y: -58.0 });
        assert_eq!(decale((-960, -130), &e), Decalage { x: 0.0, y: -40.0 });
        // Bas (barre des tâches) : la croix et son ombre restent dans la fenêtre.
        assert_eq!(decale((-960, 835), &e), Decalage { x: 0.0, y: 22.0 });
        assert_eq!(decale((-960, 740), &e), Decalage { x: 0.0, y: 10.0 });
        // Coin en bas à gauche (près du bouton Démarrer).
        assert_eq!(decale((-1915, 835), &e), Decalage { x: -228.0, y: 22.0 });
    }

    #[test]
    fn decalage_a_150_pour_cent() {
        let e = Ecran {
            rect: rect(0, 0, 2880, 1800),
            zone_utile: rect(0, 72, 2880, 1728),
            echelle: 1.5,
        };
        assert_eq!(decale((1440, 900), &e), Decalage { x: 0.0, y: 0.0 });
        // Cible à 60 réels du bord gauche : fenêtre collée (centre à 420), 360 réels = 240
        // logiques, borné à 228.
        assert_eq!(decale((60, 900), &e), Decalage { x: -228.0, y: 0.0 });
        // Fenêtre collée en haut de la zone utile (centre à 237) : 150 réels au-dessus, soit
        // 100 logiques, bornés à 58 ; 75 réels au-dessus, soit 50 logiques, tels quels.
        assert_eq!(decale((1440, 87), &e), Decalage { x: 0.0, y: -58.0 });
        assert_eq!(decale((1440, 162), &e), Decalage { x: 0.0, y: -50.0 });
    }

    #[test]
    fn decalage_dans_une_fenetre_trop_petite_ou_a_echelle_absurde() {
        // Fenêtre de 100 × 80 : bornes croisées, le disque reste au milieu.
        let f = rect(0, 0, 100, 80);
        assert_eq!(decalage((5, 5), f, 1.0, 88.0), Decalage { x: 0.0, y: 0.0 });
        // Échelle absurde : 1.
        let f = rect(0, 0, 560, 220);
        assert_eq!(
            decalage((180, 110), f, f64::NAN, 88.0),
            Decalage { x: -100.0, y: 0.0 }
        );
    }
}
