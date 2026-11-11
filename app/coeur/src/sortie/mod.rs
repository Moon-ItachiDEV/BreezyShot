//! Sortie : tout ce qui quitte l'app apres une capture.
//!
//! Une seule porte d'entree, `sortir` : elle valide les pixels, encode le PNG
//! une fois, ecrit le fichier « Recents », copie dans le presse-papiers si
//! demande, puis enregistre si demande. Une etape qui echoue n'empeche pas les
//! suivantes. La copie tourne dans un fil a part pendant l'encodage et les
//! ecritures. `annonce` dit ensuite ce qu'il faut montrer a l'utilisateur.
//!
//! Ce module n'ecrit jamais au journal : il rend un `Bilan`.

mod annonce;
mod fichier;
mod nom;
mod png;
mod presse_papiers;
mod recents;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::thread;

use chrono::{DateTime, Local};

use crate::systeme::gel::Pixels;

pub use annonce::{annonce, Annonce};
pub use nom::{chemin_libre, nom_affiche, nom_fichier};
pub use png::encoder_png;
pub use recents::{recents_a_supprimer, RECENTS_GARDES};

/// Ce que l'Application demande de faire d'une capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Demande {
    /// Reglage copier_apres_capture.
    pub copier: bool,
    /// Reglage enregistrer_apres_capture.
    pub enregistrer: bool,
    /// Deja resolu par l'Application (reglage ou Images/BreezyShot).
    /// Doit etre un chemin absolu, sinon l'enregistrement echoue.
    pub dossier_enregistrement: PathBuf,
    /// Nom du dossier d'enregistrement pour l'utilisateur, fourni par
    /// l'Application (ex. « Images › BreezyShot »).
    pub dossier_affiche: String,
    /// <local>/Recents, chemin absolu.
    pub dossier_recents: PathBuf,
    /// Reglage modele_nom, ex. « BreezyShot {date} à {heure} ».
    pub modele_nom: String,
    pub maintenant: DateTime<Local>,
}

/// Resultat de la copie dans le presse-papiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Etape {
    Faite,
    NonDemandee,
    Echec(String),
}

/// Resultat de l'enregistrement en PNG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EtapeFichier {
    Fait(PathBuf),
    NonDemande,
    Echec { dossier: PathBuf, cause: String },
}

/// Ce qui s'est passe pour chaque sortie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bilan {
    pub copie: Etape,
    pub recent: Result<PathBuf, String>,
    pub enregistrement: EtapeFichier,
}

/// Fait sortir une capture : Recents, presse-papiers, fichier PNG.
///
/// Fonction bloquante (encodage, disque, presse-papiers, quelques centaines
/// de millisecondes au pire) : elle est faite pour etre appelee hors du fil
/// principal. Elle lance elle-meme la copie dans un fil a part et ne rend la
/// main que quand tout est fini.
pub fn sortir(pixels: &Pixels, demande: &Demande) -> Bilan {
    // 1. Valider les pixels.
    if let Err(cause) = png::valider(pixels) {
        return tout_en_echec(demande, &cause);
    }

    // Alpha a 255, calcule une seule fois ; aucune copie s'il l'est deja.
    let opaques = png::opaques(&pixels.rgba);
    let (largeur, hauteur) = (pixels.largeur, pixels.hauteur);

    thread::scope(|fils| {
        // 2. Copie dans un fil a part, si demandee.
        // Si le systeme refuse de creer un fil, la copie se fera dans
        // celui-ci, apres les ecritures.
        let copier = || presse_papiers::copier(largeur, hauteur, &opaques);
        let copie = demande
            .copier
            .then(|| thread::Builder::new().spawn_scoped(fils, copier));

        // 3. Pendant ce temps : encoder une fois, Recents, enregistrement.
        let encodage = png::encoder_opaques(largeur, hauteur, &opaques);
        let recent = match &encodage {
            Ok(octets) => recents::ecrire(&demande.dossier_recents, demande.maintenant, octets),
            Err(cause) => Err(cause.clone()),
        };
        let enregistrement = if !demande.enregistrer {
            EtapeFichier::NonDemande
        } else {
            let dossier = demande.dossier_enregistrement.clone();
            let resultat = encodage.as_ref().map_err(Clone::clone).and_then(|octets| {
                let nom = nom_fichier(&demande.modele_nom, demande.maintenant);
                fichier::enregistrer(&dossier, &demande.dossier_affiche, &nom, octets)
            });
            match resultat {
                Ok(chemin) => EtapeFichier::Fait(chemin),
                Err(cause) => EtapeFichier::Echec { dossier, cause },
            }
        };

        // 4. Attendre la copie.
        let copie = match copie {
            None => Etape::NonDemandee,
            Some(Ok(fil)) => match fil.join() {
                Ok(resultat) => etape_copie(resultat),
                Err(_) => Etape::Echec(CAUSE_COPIE_INTERROMPUE.to_string()),
            },
            Some(Err(_)) => etape_copie(copier()),
        };

        Bilan {
            copie,
            recent,
            enregistrement,
        }
    })
}

fn etape_copie(resultat: Result<(), String>) -> Etape {
    match resultat {
        Ok(()) => Etape::Faite,
        Err(cause) => Etape::Echec(cause),
    }
}

/// Cause si le fil de copie s'est arrete de facon inattendue.
const CAUSE_COPIE_INTERROMPUE: &str = "La copie a été interrompue.";

fn tout_en_echec(demande: &Demande, cause: &str) -> Bilan {
    Bilan {
        copie: if demande.copier {
            Etape::Echec(cause.to_string())
        } else {
            Etape::NonDemandee
        },
        recent: Err(cause.to_string()),
        enregistrement: if demande.enregistrer {
            EtapeFichier::Echec {
                dossier: demande.dossier_enregistrement.clone(),
                cause: cause.to_string(),
            }
        } else {
            EtapeFichier::NonDemande
        },
    }
}
