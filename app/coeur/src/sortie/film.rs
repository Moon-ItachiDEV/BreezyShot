//! La sortie d'un film (contrat 0.7, §13.3, F21 à F26, F36) : le fichier `.mp4` ou `.gif`
//! écrit pendant l'enregistrement dans `<local>/Films`, puis posé dans le presse-papiers
//! (une liste de fichiers, jamais une image) et enregistré dans le dossier
//! d'enregistrement, selon la demande. Seul ce module fabrique un chemin de film.
//!
//! `<local>/Films` est borné (`films_a_retirer`) : dix films, 2 Go, sept jours, sauf les
//! films protégés, le plus récent et ceux de moins d'une heure.
//!
//! Choix U4 (pris provisoirement : un film va toujours dans le dossier d'enregistrement) :
//! l'Application le dit par `deplacer` et une demande qui enregistre
//! (`application::film::demande_film`) ; ce module déplace alors le fichier de travail
//! au lieu de le copier quand c'est possible.
//!
//! Socle : signatures, corps inertes (chantier E). Rien n'y passe tant que le film n'est
//! pas construit.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Local};

use super::{annonce, Annonce, Bilan, Demande, Etape, EtapeFichier};

/// Réponse des corps du socle.
const NON_CONSTRUIT: &str = "sortie du film non construite";

/// La sorte d'un film, vue de la sortie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SorteFilm {
    Video,
    Gif,
}

impl SorteFilm {
    /// L'extension du fichier : `mp4`, `gif`.
    pub fn extension(self) -> &'static str {
        match self {
            SorteFilm::Video => "mp4",
            SorteFilm::Gif => "gif",
        }
    }
}

/// Le dossier de travail des films, sous le dossier local.
pub const SOUS_DOSSIER_FILMS: &str = "Films";
/// Au plus dix films gardés.
pub const FILMS_GARDES: usize = 10;
/// Au plus 2 Go en tout.
pub const FILMS_OCTETS_MAX: u64 = 2 * 1024 * 1024 * 1024;
/// Aucun film de plus de sept jours.
pub const FILMS_AGE_MAX: Duration = Duration::from_secs(7 * 24 * 3600);
/// Jamais retiré avant une heure (il peut être encore dans le presse-papiers).
pub const FILM_JEUNE: Duration = Duration::from_secs(3600);

/// Un film de `<local>/Films`, vu par le nettoyage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilmGarde {
    pub nom: String,
    pub octets: u64,
    pub date: SystemTime,
}

/// F22, pur : les noms des films à retirer. `proteges` : le film en cours, celui de la
/// vignette, ceux dont la sortie tourne. Socle : aucun.
pub fn films_a_retirer(
    films: &[FilmGarde],
    maintenant: SystemTime,
    proteges: &[String],
) -> Vec<String> {
    let _ = (films, maintenant, proteges);
    Vec::new()
}

/// Nettoie `Films` et les liens de films de `Glisser` (au démarrage, au début d'un film) ;
/// rend le nombre de films retirés et leurs octets. Socle : rien n'est retiré.
pub fn nettoyer_films(
    dossier_films: &Path,
    dossier_glisser: &Path,
    maintenant: SystemTime,
    proteges: &[String],
) -> (usize, u64) {
    let _ = (dossier_films, dossier_glisser, maintenant, proteges);
    (0, 0)
}

/// Un chemin neuf dans `<local>/Films` (dossier créé), nommé `AAAAMMJJ-HHMMSS-mmm.<ext>`
/// (la base UTC de « Récents »). Socle : refusé.
pub fn chemin_film(
    dossier_films: &Path,
    maintenant: DateTime<Local>,
    sorte: SorteFilm,
) -> Result<PathBuf, String> {
    let _ = (dossier_films, maintenant, sorte);
    Err(NON_CONSTRUIT.into())
}

/// Ce que la sortie d'un film a fait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BilanFilm {
    pub copie: Etape,
    pub enregistrement: EtapeFichier,
    /// Le lien posé dans `<local>/Glisser` pour le presse-papiers, s'il y en a un.
    pub lien: Option<PathBuf>,
}

/// Copie du fichier dans le presse-papiers et enregistrement, selon `demande` (F23).
/// `deplacer` (choix U4) : le fichier de travail est déplacé dans le dossier
/// d'enregistrement au lieu d'y être copié (copié puis effacé s'il est sur un autre
/// volume). Bloquant, peut durer : fil `film-sortie` (F36). Socle : rien n'est fait, et
/// le bilan le dit (échec de ce qui était demandé).
pub fn sortir_film(
    fichier: &Path,
    sorte: SorteFilm,
    demande: &Demande,
    deplacer: bool,
) -> BilanFilm {
    let _ = (fichier, sorte, deplacer);
    BilanFilm {
        copie: if demande.copier {
            Etape::Echec(NON_CONSTRUIT.into())
        } else {
            Etape::NonDemandee
        },
        enregistrement: if demande.enregistrer {
            EtapeFichier::Echec {
                dossier: demande.dossier_enregistrement.clone(),
                cause: NON_CONSTRUIT.into(),
            }
        } else {
            EtapeFichier::NonDemande
        },
        lien: None,
    }
}

/// Le bouton « Copier » de la vignette : le fichier dans le presse-papiers ; rend l'étape
/// et le lien posé. Socle : échec.
pub fn copier_film(
    fichier: &Path,
    sorte: SorteFilm,
    demande: &Demande,
) -> (Etape, Option<PathBuf>) {
    let _ = (fichier, sorte, demande);
    (Etape::Echec(NON_CONSTRUIT.into()), None)
}

/// Le bouton « Enregistrer » de la vignette : une copie dans le dossier d'enregistrement,
/// sous le nom du modèle. Socle : échec.
pub fn enregistrer_film(fichier: &Path, sorte: SorteFilm, demande: &Demande) -> EtapeFichier {
    let _ = (fichier, sorte);
    EtapeFichier::Echec {
        dossier: demande.dossier_enregistrement.clone(),
        cause: NON_CONSTRUIT.into(),
    }
}

/// L'annonce du bandeau quand le bilan arrive sans vignette (F26), accordée pour une
/// vidéo (« Copiée », « Enregistrée »). Socle : l'annonce d'une image.
pub fn annonce_film(bilan: &BilanFilm, demande: &Demande, sorte: SorteFilm) -> Annonce {
    let _ = sorte;
    let image = Bilan {
        copie: bilan.copie.clone(),
        recent: None,
        enregistrement: bilan.enregistrement.clone(),
    };
    annonce(&image, demande)
}

/// Efface le fichier de travail d'un film annulé ou sans image (seulement dans
/// `<local>/Films`, au format exact). Socle : rien n'est effacé.
pub fn effacer_film(fichier: &Path) {
    let _ = fichier;
}
