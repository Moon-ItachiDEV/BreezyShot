//! Le texte vers le presse-papiers (contrat 0.5, §3.1, R6) : une voie à part de `sortir`,
//! qui est pour les pixels. Mêmes essais et même échéance que la copie d'image (250 ms,
//! `presse_papiers::avec_echeance`).
//!
//! Le presse-papiers du texte est gardé ouvert pour toute la vie du processus, comme celui
//! de l'image (sous X11, c'est notre processus qui sert le texte aux autres applications).

use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use arboard::{Clipboard, Error};

use super::presse_papiers::{avec_echeance, vaut_reessai, ECHEANCE};
use super::Etape;

/// Ouvert une fois, gardé jusqu'à la fin du processus.
static OUVERT: Mutex<Option<Clipboard>> = Mutex::new(None);

/// Pose `texte` dans le presse-papiers (`arboard`, 250 ms d'essais). Le texte remplace ce
/// qui y était (l'image comprise). Bloquant au plus 250 ms : hors du fil principal.
pub fn copier_texte(texte: &str) -> Etape {
    let mut garde = OUVERT.lock().unwrap_or_else(|e| e.into_inner());
    let resultat = copier_par(Instant::now, thread::sleep, || poser(&mut garde, texte));
    if matches!(&resultat, Err(e) if !vaut_reessai(e)) {
        // On repartira d'un presse-papiers neuf la prochaine fois.
        *garde = None;
    }
    etape(resultat)
}

/// Les essais de `copier_texte`, avec l'échéance commune (pour les tests : horloge et
/// attente données).
fn copier_par(
    horloge: impl Fn() -> Instant,
    dormir: impl FnMut(Duration),
    essai: impl FnMut() -> Result<(), Error>,
) -> Result<(), Error> {
    avec_echeance(ECHEANCE, horloge, dormir, essai)
}

fn poser(garde: &mut Option<Clipboard>, texte: &str) -> Result<(), Error> {
    let presse = match garde {
        Some(presse) => presse,
        None => garde.insert(Clipboard::new()?),
    };
    presse.set_text(texte)
}

/// L'étape de la copie, avec sa cause en clair (jamais le message de la crate).
fn etape(resultat: Result<(), Error>) -> Etape {
    match resultat {
        Ok(()) => Etape::Faite,
        Err(Error::ClipboardOccupied) => {
            Etape::Echec("Le presse-papiers est occupé par une autre application.".into())
        }
        Err(Error::ClipboardNotSupported) => {
            Etape::Echec("Le presse-papiers n'est pas disponible.".into())
        }
        Err(_) => Etape::Echec("Le presse-papiers a refusé le texte.".into()),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::systeme::presse_papiers::VERROU_ESSAIS;

    #[test]
    fn occupe_abandonne_a_l_echeance_commune() {
        let debut = Instant::now();
        let temps = Cell::new(Duration::ZERO);
        let essais = Cell::new(0u32);
        let r = copier_par(
            || debut + temps.get(),
            |d| temps.set(temps.get() + d),
            || {
                essais.set(essais.get() + 1);
                // Chaque essai occupé coûte aussi son temps.
                temps.set(temps.get() + Duration::from_millis(5));
                Err(Error::ClipboardOccupied)
            },
        );
        assert!(matches!(r, Err(Error::ClipboardOccupied)));
        assert!(temps.get() <= ECHEANCE, "{:?}", temps.get());
        assert!(essais.get() > 1, "aucun nouvel essai");
        assert_eq!(
            etape(r),
            Etape::Echec("Le presse-papiers est occupé par une autre application.".into())
        );
    }

    #[test]
    fn occupe_puis_libre_reussit() {
        let essais = Cell::new(0u32);
        let r = copier_par(
            Instant::now,
            |_| {},
            || {
                essais.set(essais.get() + 1);
                if essais.get() < 3 {
                    Err(Error::ClipboardOccupied)
                } else {
                    Ok(())
                }
            },
        );
        assert!(r.is_ok());
        assert_eq!(essais.get(), 3);
        assert_eq!(etape(r), Etape::Faite);
    }

    #[test]
    fn refus_definitif_sans_nouvel_essai() {
        let essais = Cell::new(0u32);
        let r = copier_par(
            Instant::now,
            |_| {},
            || {
                essais.set(essais.get() + 1);
                Err(Error::ClipboardNotSupported)
            },
        );
        assert_eq!(essais.get(), 1);
        assert_eq!(
            etape(r),
            Etape::Echec("Le presse-papiers n'est pas disponible.".into())
        );
    }

    /// Le vrai presse-papiers, s'il existe ici (sinon : sauté, sauf
    /// `BREEZYSHOT_PRESSE_PAPIERS_OBLIGATOIRE=1`).
    #[test]
    fn presse_papiers_pose_et_relit_le_texte() {
        let _seul = VERROU_ESSAIS.lock().unwrap_or_else(|e| e.into_inner());
        let texte = "BreezyShot lit ce texte\r\nVersion 0.5 · é";
        let etape = copier_texte(texte);
        if etape != Etape::Faite {
            let obligatoire =
                std::env::var("BREEZYSHOT_PRESSE_PAPIERS_OBLIGATOIRE").as_deref() == Ok("1");
            assert!(!obligatoire, "presse-papiers obligatoire : {etape:?}");
            println!("pas de presse-papiers ici ({etape:?}), saute.");
            return;
        }
        // Relu par le même objet gardé (sous X11, c'est lui qui sert le texte).
        let debut = Instant::now();
        let relu = loop {
            let mut garde = OUVERT.lock().unwrap_or_else(|e| e.into_inner());
            match garde.as_mut().map(Clipboard::get_text) {
                Some(Ok(t)) => break t,
                _ if debut.elapsed() >= Duration::from_secs(1) => panic!("texte non relu en 1 s"),
                _ => {}
            }
            drop(garde);
            thread::sleep(Duration::from_millis(50));
        };
        assert_eq!(relu, texte);
    }
}
