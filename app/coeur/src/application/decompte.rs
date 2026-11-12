//! Le retardateur (0.4) : décisions pures du décompte, testées sans écran.
//!
//! Le câblage (fil `decompte`, Échap global, bandeau) est dans `mod.rs` ; ici, seulement ce
//! qu'il faut décider : quel déclencheur, combien de temps, quelle fin est à jour, quelle
//! arrivée annule, où montrer le disque, quelles lignes de journal.

use std::time::Duration;

use super::etats::{Declencheur, Etat, Mode};
use crate::systeme::gel::{Changement, EcranInfo};
use crate::systeme::Rect;

/// Les retardateurs que la page propose (0 = aucun). Le fichier en permet d'autres
/// (`reglages::RETARDATEUR_MAX_S`).
pub const RETARDATEURS_PROPOSES: [u32; 4] = [0, 3, 5, 10];

/// Le déclencheur d'une demande de capture : directe sans retardateur, sinon le décompte.
pub fn declencheur(mode: Mode, retardateur_s: u32) -> Declencheur {
    if retardateur_s == 0 {
        Declencheur::DemandeCapture(mode)
    } else {
        Declencheur::DemandeDecompte(mode)
    }
}

/// La durée du décompte.
pub fn duree(retardateur_s: u32) -> Duration {
    Duration::from_secs(u64::from(retardateur_s))
}

/// Vrai si la fin reçue (`recu`) est celle du décompte attendu ; une fin d'un décompte
/// annulé ou remplacé est périmée.
pub fn fin_a_jour(attendu: Option<u64>, recu: u64) -> bool {
    attendu == Some(recu)
}

/// Pourquoi un décompte est annulé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    Echap,
    Croix,
    Session,
    Arret,
}

impl Cause {
    /// Nom dans le journal : `decompte <n> annule (echap)`.
    pub fn nom(self) -> &'static str {
        match self {
            Cause::Echap => "echap",
            Cause::Croix => "croix",
            Cause::Session => "session",
            Cause::Arret => "arret",
        }
    }
}

/// Ce qui arrive à l'Application et peut annuler un décompte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArriveeDecompte<'a> {
    /// Échap global.
    Echap,
    /// Clic dans le bandeau (`bandeau_action`).
    Action { id: u64, action: &'a str },
    /// Le bureau a changé.
    Bureau(Changement),
}

/// La cause d'annulation, ou `None` si l'arrivée ne concerne pas le décompte (hors état
/// Décompte : Échap va à la sélection, comme en 0.3). `affichage` : l'`id` du décompte
/// dans le bandeau.
pub fn annulation(etat: Option<Etat>, affichage: Option<u64>, a: ArriveeDecompte) -> Option<Cause> {
    if etat != Some(Etat::Decompte) {
        return None;
    }
    match a {
        ArriveeDecompte::Echap => Some(Cause::Echap),
        ArriveeDecompte::Action { id, action } => {
            (affichage == Some(id) && action == "annuler").then_some(Cause::Croix)
        }
        ArriveeDecompte::Bureau(Changement::SessionVerrouillee) => Some(Cause::Session),
        ArriveeDecompte::Bureau(Changement::Ecrans) => None,
    }
}

/// Où montrer le disque : un centre, le petit côté de la dernière zone (s'il y en a une),
/// et la zone utile de l'écran où il est posé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cible {
    pub centre: (i32, i32),
    pub cote: Option<i32>,
    pub ecran: Rect,
}

/// La cible du décompte : en mode dernière zone, le centre de la zone (arrondi vers le
/// bas) et son petit côté, si un écran la contient entière ; sinon le centre de la zone
/// utile de l'écran de la souris, à défaut de l'écran principal, à défaut du premier.
/// `None` sans aucun écran.
pub fn cible(
    mode: Mode,
    derniere: Option<Rect>,
    souris: Option<(i32, i32)>,
    ecrans: &[EcranInfo],
) -> Option<Cible> {
    if mode == Mode::DerniereZone {
        if let Some(zone) = derniere {
            if let Some(e) = ecrans.iter().find(|e| e.rect.englobe(zone)) {
                return Some(Cible {
                    centre: centre(zone),
                    cote: Some(zone.largeur.min(zone.hauteur)),
                    ecran: e.zone_utile,
                });
            }
        }
    }
    let ecran = souris
        .and_then(|p| ecrans.iter().find(|e| e.rect.contient(p)))
        .or_else(|| ecrans.iter().find(|e| e.principal))
        .or_else(|| ecrans.first())?;
    Some(Cible {
        centre: centre(ecran.zone_utile),
        cote: None,
        ecran: ecran.zone_utile,
    })
}

/// Centre d'un rectangle, arrondi vers le bas (coordonnées négatives comprises).
fn centre(r: Rect) -> (i32, i32) {
    let m = |debut: i32, cote: i32| (i64::from(debut) + i64::from(cote).div_euclid(2)) as i32;
    (m(r.x, r.largeur), m(r.y, r.hauteur))
}

/// `decompte <n> debut <s> s <mode>`.
pub fn ligne_debut(n: u64, s: u32, m: Mode) -> String {
    format!("decompte {n} debut {s} s {}", m.nom())
}

/// `decompte <n> fini en <ms> ms`.
pub fn ligne_fin(n: u64, duree: Duration) -> String {
    format!("decompte {n} fini en {} ms", duree.as_millis())
}

/// `decompte <n> annule (<cause>)`.
pub fn ligne_annule(n: u64, cause: Cause) -> String {
    format!("decompte {n} annule ({})", cause.nom())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ecran(x: i32, y: i32, l: i32, h: i32, principal: bool) -> EcranInfo {
        let rect = Rect {
            x,
            y,
            largeur: l,
            hauteur: h,
        };
        EcranInfo {
            rect,
            zone_utile: Rect {
                hauteur: h - 48,
                ..rect
            },
            echelle: 1.0,
            principal,
        }
    }

    #[test]
    fn declencheur_et_duree() {
        assert_eq!(
            declencheur(Mode::Zone, 0),
            Declencheur::DemandeCapture(Mode::Zone)
        );
        for s in [3, 5, 10] {
            assert_eq!(
                declencheur(Mode::DerniereZone, s),
                Declencheur::DemandeDecompte(Mode::DerniereZone)
            );
        }
        assert_eq!(duree(3), Duration::from_secs(3));
        assert!(fin_a_jour(Some(2), 2));
        assert!(!fin_a_jour(Some(2), 1));
        assert!(!fin_a_jour(None, 1));
    }

    #[test]
    fn annulations() {
        let d = Some(Etat::Decompte);
        let a = ArriveeDecompte::Action {
            id: 4,
            action: "annuler",
        };
        assert_eq!(
            annulation(d, Some(4), ArriveeDecompte::Echap),
            Some(Cause::Echap)
        );
        assert_eq!(
            annulation(Some(Etat::Selection), Some(4), ArriveeDecompte::Echap),
            None
        );
        assert_eq!(annulation(d, Some(4), a), Some(Cause::Croix));
        assert_eq!(annulation(d, Some(5), a), None);
        assert_eq!(annulation(d, None, a), None);
        let fermer = ArriveeDecompte::Action {
            id: 4,
            action: "fermer",
        };
        assert_eq!(annulation(d, Some(4), fermer), None);
        let ecrans = ArriveeDecompte::Bureau(Changement::Ecrans);
        assert_eq!(annulation(d, Some(4), ecrans), None);
        let session = ArriveeDecompte::Bureau(Changement::SessionVerrouillee);
        assert_eq!(annulation(d, Some(4), session), Some(Cause::Session));
    }

    #[test]
    fn cibles() {
        let ecrans = [
            ecran(0, 0, 1920, 1080, true),
            ecran(1920, 0, 1280, 1024, false),
        ];
        let c = cible(Mode::Zone, None, Some((2000, 10)), &ecrans).unwrap();
        assert_eq!(c.centre, (1920 + 640, 488));
        assert_eq!(c.cote, None);
        // Souris hors écran, ou inconnue : l'écran principal.
        for souris in [Some((-50, -50)), None] {
            let c = cible(Mode::PleinEcran, None, souris, &ecrans).unwrap();
            assert_eq!(c.centre, (960, 516));
        }
        // Dernière zone : son centre (arrondi vers le bas) et son petit côté.
        let zone = Rect {
            x: 2001,
            y: 11,
            largeur: 301,
            hauteur: 151,
        };
        let c = cible(Mode::DerniereZone, Some(zone), None, &ecrans).unwrap();
        assert_eq!((c.centre, c.cote), ((2151, 86), Some(151)));
        assert_eq!(c.ecran, ecrans[1].zone_utile);
        // Une zone à cheval sur deux écrans ne sert pas de cible.
        let cheval = Rect { x: 1900, ..zone };
        let c = cible(Mode::DerniereZone, Some(cheval), None, &ecrans).unwrap();
        assert_eq!(c.cote, None);
        assert_eq!(cible(Mode::Zone, None, None, &[]), None);
    }

    #[test]
    fn lignes_du_journal() {
        assert_eq!(
            ligne_debut(3, 5, Mode::PleinEcran),
            "decompte 3 debut 5 s plein_ecran"
        );
        assert_eq!(
            ligne_fin(3, Duration::from_millis(5012)),
            "decompte 3 fini en 5012 ms"
        );
        assert_eq!(ligne_annule(3, Cause::Croix), "decompte 3 annule (croix)");
        assert_eq!(RETARDATEURS_PROPOSES, [0, 3, 5, 10]);
    }
}
