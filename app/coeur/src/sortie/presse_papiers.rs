//! Copie de l'image dans le presse-papiers : par la crate `arboard`, ou, pour
//! une image avec transparence, par `systeme::presse_papiers` (format PNG et
//! version sur fond blanc) quand le systeme le permet.
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

use crate::systeme::presse_papiers::{self as natif, ErreurPressePapiers};

/// Presse-papiers ouvert une fois, garde jusqu'a la fin du processus.
static OUVERT: Mutex<Option<Clipboard>> = Mutex::new(None);

/// Temps total accorde quand le presse-papiers est occupe.
pub(super) const ECHEANCE: Duration = Duration::from_millis(250);

/// Attente entre deux essais.
pub(super) const PAS: Duration = Duration::from_millis(25);

/// Ce que coute au pire un essai occupe dans `arboard` sous Windows, et dans
/// `systeme::presse_papiers::ImagePreparee::poser` : 6 ouvertures separees par
/// 5 attentes de 5 ms.
pub(super) const COUT_ESSAI_INTERNE: Duration = Duration::from_millis(25);

/// Vrai si l'erreur vaut un nouvel essai.
pub(super) fn vaut_reessai(erreur: &Error) -> bool {
    matches!(erreur, Error::ClipboardOccupied)
}

/// Une erreur qui peut valoir un nouvel essai.
pub(super) trait Reessayable {
    fn vaut_reessai(&self) -> bool;
}

impl Reessayable for Error {
    fn vaut_reessai(&self) -> bool {
        vaut_reessai(self)
    }
}

impl Reessayable for ErreurPressePapiers {
    fn vaut_reessai(&self) -> bool {
        *self == ErreurPressePapiers::Occupe
    }
}

const CAUSE_OCCUPE: &str = "Le presse-papiers est occupé par une autre application.";
const CAUSE_NON_DISPONIBLE: &str = "Le presse-papiers n'est pas disponible.";
const CAUSE_REFUS: &str = "Le presse-papiers a refusé l'image.";

/// Cause en clair d'une erreur du presse-papiers (jamais le message de la crate).
pub(super) fn cause(erreur: &Error) -> String {
    match erreur {
        Error::ClipboardOccupied => CAUSE_OCCUPE.to_string(),
        Error::ClipboardNotSupported => CAUSE_NON_DISPONIBLE.to_string(),
        Error::ConversionFailure => {
            "L'image n'a pas pu être préparée pour le presse-papiers.".to_string()
        }
        _ => CAUSE_REFUS.to_string(),
    }
}

/// Cause en clair d'une erreur du presse-papiers natif, avec les memes textes.
pub(super) fn cause_native(erreur: ErreurPressePapiers) -> String {
    match erreur {
        ErreurPressePapiers::Occupe => CAUSE_OCCUPE,
        ErreurPressePapiers::NonDisponible => CAUSE_NON_DISPONIBLE,
        ErreurPressePapiers::Refuse => CAUSE_REFUS,
    }
    .to_string()
}

/// Lance `essai` jusqu'a reussite, erreur definitive ou echeance.
///
/// Un nouvel essai n'est lance que s'il peut finir avant l'echeance, en
/// comptant l'attente et le cout interne d'un essai dans `arboard`. Le temps
/// vient de `horloge` et les attentes passent par `dormir`, pour les tests.
pub(super) fn avec_echeance<T, E: Reessayable>(
    echeance: Duration,
    horloge: impl Fn() -> Instant,
    mut dormir: impl FnMut(Duration),
    mut essai: impl FnMut() -> Result<T, E>,
) -> Result<T, E> {
    let debut = horloge();
    loop {
        match essai() {
            Err(e) if e.vaut_reessai() => {
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

/// Pose dans le presse-papiers, par `arboard`, une image deja validee (opaque,
/// ou avec son alpha quand le presse-papiers natif n'existe pas).
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

/// Pose une image deja validee avec sa transparence. `png` : le fichier deja
/// encode avec son alpha. Si le systeme a son presse-papiers natif, deux
/// versions y sont posees ensemble (le PNG, et l'image sur fond blanc pour les
/// apps qui ne lisent pas le PNG), avec les memes essais et la meme echeance
/// que `copier`. Sinon, ou sans PNG (encodage rate), `copier` avec l'alpha.
pub(super) fn copier_transparent(
    largeur: u32,
    hauteur: u32,
    rgba: &[u8],
    png: Option<&[u8]>,
) -> Result<(), String> {
    match png {
        Some(png) if natif::disponible() => {
            // Prepare une fois (fond blanc, blocs de memoire) : seuls l'ouverture
            // et le depot sont refaits a chaque essai.
            let blanc = super::png::sur_blanc(rgba);
            let mut image = natif::ImagePreparee::preparer(png, largeur, hauteur, &blanc)
                .map_err(cause_native)?;
            drop(blanc);
            avec_echeance(ECHEANCE, Instant::now, thread::sleep, || image.poser())
                .map_err(cause_native)
        }
        _ => copier(largeur, hauteur, rgba),
    }
}

/// Pose `chemin` dans le presse-papiers, en liste de fichiers (0.7, F23 : un film, prouvé
/// par S6), avec les essais et l'échéance d'une image. Par l'objet gardé, le seul du
/// processus : un second objet `arboard` ouvert à côté se disputerait le presse-papiers
/// (sous X11, le dernier détruit emporte ce qu'il sert).
pub(super) fn copier_fichier(chemin: &std::path::Path) -> Result<(), String> {
    let mut garde = OUVERT.lock().unwrap_or_else(|e| e.into_inner());
    let resultat = avec_echeance(ECHEANCE, Instant::now, thread::sleep, || {
        ouvrir(&mut garde)?.set().file_list(&[chemin])
    });
    match resultat {
        Ok(()) => Ok(()),
        Err(e) => {
            if !vaut_reessai(&e) {
                *garde = None;
            }
            Err(cause(&e))
        }
    }
}

fn poser(garde: &mut Option<Clipboard>, image: ImageData<'_>) -> Result<(), Error> {
    ouvrir(garde)?.set_image(image)
}

/// Le presse-papiers garde, ouvert s'il ne l'est pas encore.
fn ouvrir(garde: &mut Option<Clipboard>) -> Result<&mut Clipboard, Error> {
    match garde {
        Some(presse) => Ok(presse),
        None => Ok(garde.insert(Clipboard::new()?)),
    }
}

/// Pour les tests : ouvre l'objet garde, sans jamais en creer un a part.
/// Sous X11, detruire le dernier objet ferme la derniere connexion, et un
/// serveur comme Xvfb se reinitialise alors : une connexion ouverte juste
/// apres est coupee (« Connection reset by peer »).
#[cfg(test)]
pub(super) fn sonder() -> Result<(), Error> {
    let mut garde = OUVERT.lock().unwrap_or_else(|e| e.into_inner());
    ouvrir(&mut garde).map(|_| ())
}

/// Pour les tests : relit l'image par l'objet garde.
#[cfg(test)]
pub(super) fn relire() -> Result<ImageData<'static>, Error> {
    let mut garde = OUVERT.lock().unwrap_or_else(|e| e.into_inner());
    ouvrir(&mut garde)?.get_image()
}

/// Pour les tests : relit la liste de fichiers par l'objet garde.
#[cfg(test)]
pub(super) fn relire_fichiers() -> Result<Vec<std::path::PathBuf>, Error> {
    let mut garde = OUVERT.lock().unwrap_or_else(|e| e.into_inner());
    ouvrir(&mut garde)?.get().file_list()
}
