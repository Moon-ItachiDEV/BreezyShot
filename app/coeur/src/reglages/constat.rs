//! Ce que `charger` a constaté, et pourquoi `ecrire` a refusé.
//!
//! Les textes (`Display`) sont des lignes de journal : français simple, sans accents, comme
//! les autres lignes que lisent les essais (« arret propre », « seconde instance refusee »).

use std::fmt;
use std::io;
use std::path::PathBuf;

/// Pourquoi une valeur a été remplacée par sa valeur par défaut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Raison {
    /// Le champ manque dans un fichier à jour.
    Absente,
    /// Mauvais type, valeur inconnue ou hors limites.
    Invalide,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Constat {
    /// Le fichier n'existait pas : il a été créé avec les valeurs par défaut.
    Cree { chemin: PathBuf },
    /// Le fichier n'était pas un objet JSON : gardé sous `sauvegarde`, puis recréé.
    Abime { raison: String, sauvegarde: PathBuf },
    /// Le fichier était abîmé mais n'a pas pu être mis de côté : il est laissé tel quel et
    /// l'app tourne avec les valeurs par défaut (on ne détruit jamais le fichier).
    AbimeNonSauvegarde { raison: String, erreur: String },
    /// Une vieille sauvegarde de fichier abîmé a été supprimée (on en garde 3).
    AncienneSauvegardeSupprimee { chemin: PathBuf },
    /// Le fichier existe mais n'a pas pu être lu (droits, verrou) : valeurs par défaut, et
    /// le fichier n'est pas touché.
    LectureImpossible { erreur: String },
    /// Le fichier a été mis à niveau de la version `de` à la version `a`.
    MisANiveau { de: u32, a: u32 },
    /// Une valeur a été remplacée par sa valeur par défaut. `champ` est un chemin pointé,
    /// par exemple `vignette.duree_s` ou `raccourcis.zone`.
    ValeurRemplacee { champ: String, raison: Raison },
    /// Copie, enregistrement et vignette étaient tous éteints : la copie a été rallumée.
    CopieRallumee,
    /// Le fichier vient d'une version plus récente de l'app : lu au mieux, pas réécrit.
    VersionPlusRecente { version: u32, connue: u32 },
    /// Les réglages corrigés n'ont pas pu être écrits (ils servent quand même en mémoire).
    EcritureImpossible { erreur: String },
}

impl Constat {
    /// Code court, sans chemin, pour les lignes de journal de la fenêtre Réglages.
    pub fn code(&self) -> &'static str {
        match self {
            Constat::Cree { .. } => "cree",
            Constat::Abime { .. } => "abime",
            Constat::AbimeNonSauvegarde { .. } => "abime_non_sauvegarde",
            Constat::AncienneSauvegardeSupprimee { .. } => "ancienne_sauvegarde_supprimee",
            Constat::LectureImpossible { .. } => "lecture_impossible",
            Constat::MisANiveau { .. } => "mis_a_niveau",
            Constat::ValeurRemplacee { .. } => "valeur_remplacee",
            Constat::CopieRallumee => "copie_rallumee",
            Constat::VersionPlusRecente { .. } => "version_recente",
            Constat::EcritureImpossible { .. } => "ecriture_impossible",
        }
    }
}

impl fmt::Display for Constat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Constat::Cree { chemin } => {
                write!(f, "reglages crees avec les valeurs par defaut : {}", chemin.display())
            }
            Constat::Abime { raison, sauvegarde } => write!(
                f,
                "reglages illisibles ({raison}) : fichier garde sous {}, valeurs par defaut",
                sauvegarde.display()
            ),
            Constat::AbimeNonSauvegarde { raison, erreur } => write!(
                f,
                "reglages illisibles ({raison}) et impossibles a mettre de cote ({erreur}) : \
                 fichier laisse tel quel, valeurs par defaut"
            ),
            Constat::AncienneSauvegardeSupprimee { chemin } => write!(
                f,
                "reglages : ancienne sauvegarde supprimee : {}",
                chemin.display()
            ),
            Constat::LectureImpossible { erreur } => write!(
                f,
                "reglages impossibles a lire ({erreur}) : fichier laisse tel quel, valeurs par defaut"
            ),
            Constat::MisANiveau { de, a } => {
                write!(f, "reglages mis a niveau de la version {de} a la version {a}")
            }
            Constat::ValeurRemplacee { champ, raison } => {
                let pourquoi = match raison {
                    Raison::Absente => "absente",
                    Raison::Invalide => "invalide",
                };
                write!(
                    f,
                    "reglages : valeur {pourquoi} pour {champ}, remplacee par la valeur par defaut"
                )
            }
            Constat::CopieRallumee => write!(
                f,
                "reglages : copie, enregistrement et vignette etaient eteints, copie rallumee"
            ),
            Constat::VersionPlusRecente { version, connue } => write!(
                f,
                "reglages en version {version}, plus recente que la version {connue} connue : \
                 lus au mieux, fichier non reecrit"
            ),
            Constat::EcritureImpossible { erreur } => {
                write!(f, "reglages impossibles a ecrire : {erreur}")
            }
        }
    }
}

/// Pourquoi `ecrire` a refusé ou échoué. Le fichier existant n'est jamais abîmé.
#[derive(Debug)]
pub enum ErreurEcriture {
    /// Copie, enregistrement et vignette tous éteints.
    AucuneSortie,
    /// Une valeur hors de ce que le fichier accepte (durée hors limites, modèle de nom vide,
    /// chemin vide ou pas en UTF-8).
    ValeurInvalide { champ: &'static str },
    /// Les réglages viennent d'un fichier plus récent que l'app : on ne le réécrit pas.
    VersionPlusRecente { version: u32 },
    /// Le fichier n'a pas pu être lu ou mis de côté (`LectureImpossible`,
    /// `AbimeNonSauvegarde`), ou le fichier présent est illisible au moment d'écrire : on ne
    /// l'écrase pas.
    FichierProtege,
    /// Le texte dépasserait la taille que `charger` accepte : il serait déclaré abîmé.
    TropGros { taille: usize },
    /// Le fichier est en lecture seule : on respecte ce choix.
    LectureSeule { chemin: PathBuf },
    /// Erreur du disque (dossier impossible à créer, disque plein, droits…).
    Disque {
        etape: &'static str,
        erreur: io::Error,
    },
}

impl ErreurEcriture {
    /// Code court, sans chemin, pour les lignes de journal de la fenêtre Réglages.
    pub fn code(&self) -> &'static str {
        match self {
            ErreurEcriture::AucuneSortie => "aucune_sortie",
            ErreurEcriture::ValeurInvalide { .. } => "valeur_invalide",
            ErreurEcriture::VersionPlusRecente { .. } => "version_recente",
            ErreurEcriture::FichierProtege => "protege",
            ErreurEcriture::TropGros { .. } => "trop_gros",
            ErreurEcriture::LectureSeule { .. } => "lecture_seule",
            ErreurEcriture::Disque { .. } => "disque",
        }
    }
}

impl fmt::Display for ErreurEcriture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErreurEcriture::AucuneSortie => {
                write!(
                    f,
                    "copie, enregistrement et vignette ne peuvent pas etre tous eteints"
                )
            }
            ErreurEcriture::ValeurInvalide { champ } => write!(f, "valeur invalide pour {champ}"),
            ErreurEcriture::VersionPlusRecente { version } => write!(
                f,
                "le fichier est en version {version}, plus recente que l'app : non reecrit"
            ),
            ErreurEcriture::FichierProtege => write!(
                f,
                "le fichier existant n'a pas pu etre lu ou mis de cote : non ecrase"
            ),
            ErreurEcriture::TropGros { taille } => {
                write!(f, "reglages trop gros ({taille} octets) : non ecrits")
            }
            ErreurEcriture::LectureSeule { chemin } => {
                write!(f, "fichier en lecture seule : {}", chemin.display())
            }
            ErreurEcriture::Disque { etape, erreur } => write!(f, "{etape} : {erreur}"),
        }
    }
}

impl std::error::Error for ErreurEcriture {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ErreurEcriture::Disque { erreur, .. } => Some(erreur),
            _ => None,
        }
    }
}
