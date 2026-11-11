//! Copie de l'image dans le presse-papiers, par la crate `arboard`.
//!
//! Le presse-papiers est garde ouvert pour toute la vie du processus : sous
//! Linux (X11), c'est notre processus qui sert l'image aux autres
//! applications, et elle disparait si le dernier objet `arboard` est detruit.
//! Sous Windows et macOS, garder l'objet ne coute rien.

use std::borrow::Cow;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use arboard::{Clipboard, Error, ImageData};

/// Presse-papiers ouvert une fois, garde jusqu'a la fin du processus.
static OUVERT: Mutex<Option<Clipboard>> = Mutex::new(None);

/// Temps total accorde quand le presse-papiers est occupe.
pub(super) const ECHEANCE: Duration = Duration::from_millis(250);

/// Attente entre deux essais.
pub(super) const PAS: Duration = Duration::from_millis(25);

/// Ce que coute au pire un essai occupe dans `arboard` sous Windows :
/// 6 ouvertures separees par 5 attentes de 5 ms.
pub(super) const COUT_ESSAI_INTERNE: Duration = Duration::from_millis(25);

/// Vrai si l'erreur vaut un nouvel essai.
pub(super) fn vaut_reessai(erreur: &Error) -> bool {
    matches!(erreur, Error::ClipboardOccupied)
}

/// Cause en clair d'une erreur du presse-papiers (jamais le message de la crate).
pub(super) fn cause(erreur: &Error) -> String {
    match erreur {
        Error::ClipboardOccupied => {
            "Le presse-papiers est occupé par une autre application.".to_string()
        }
        Error::ClipboardNotSupported => "Le presse-papiers n'est pas disponible.".to_string(),
        Error::ConversionFailure => {
            "L'image n'a pas pu être préparée pour le presse-papiers.".to_string()
        }
        _ => "Le presse-papiers a refusé l'image.".to_string(),
    }
}

/// Lance `essai` jusqu'a reussite, erreur definitive ou echeance.
///
/// Un nouvel essai n'est lance que s'il peut finir avant l'echeance, en
/// comptant l'attente et le cout interne d'un essai dans `arboard`. Le temps
/// vient de `horloge` et les attentes passent par `dormir`, pour les tests.
pub(super) fn avec_echeance<T>(
    echeance: Duration,
    horloge: impl Fn() -> Instant,
    mut dormir: impl FnMut(Duration),
    mut essai: impl FnMut() -> Result<T, Error>,
) -> Result<T, Error> {
    let debut = horloge();
    loop {
        match essai() {
            Err(e) if vaut_reessai(&e) => {
                let ecoule = horloge().saturating_duration_since(debut);
                if ecoule + PAS + COUT_ESSAI_INTERNE > echeance {
                    return Err(e);
                }
                dormir(PAS);
                // Une attente peut durer plus que prevu sur une machine
                // chargee : on verifie encore avant de relancer.
                let ecoule = horloge().saturating_duration_since(debut);
                if ecoule + COUT_ESSAI_INTERNE > echeance {
                    return Err(e);
                }
            }
            autre => return autre,
        }
    }
}

/// Pose dans le presse-papiers une image deja validee et deja opaque.
pub(super) fn copier(largeur: u32, hauteur: u32, rgba: &[u8]) -> Result<(), String> {
    let mut garde = OUVERT.lock().unwrap_or_else(|e| e.into_inner());
    let resultat = avec_echeance(ECHEANCE, Instant::now, thread::sleep, || {
        let image = ImageData {
            width: largeur as usize,
            height: hauteur as usize,
            bytes: Cow::Borrowed(rgba),
        };
        poser(&mut garde, image)
    });
    match resultat {
        Ok(()) => Ok(()),
        Err(e) => {
            if !vaut_reessai(&e) {
                // On repartira d'un presse-papiers neuf la prochaine fois.
                *garde = None;
            }
            Err(cause(&e))
        }
    }
}

fn poser(garde: &mut Option<Clipboard>, image: ImageData<'_>) -> Result<(), Error> {
    if garde.is_none() {
        *garde = Some(Clipboard::new()?);
    }
    match garde.as_mut() {
        Some(presse) => presse.set_image(image),
        None => Err(Error::ClipboardNotSupported),
    }
}
