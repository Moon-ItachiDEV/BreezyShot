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
//! au lieu de le copier quand c'est possible : un lien dur puis l'effacement du fichier
//! de travail (même volume, jamais rien d'écrasé), sinon un renommage sur une réservation,
//! sinon (autre volume) une copie puis l'effacement.
//!
//! Le presse-papiers reçoit `<local>/Glisser/<nom du modèle>.<ext>`, un lien dur vers le
//! fichier de travail (une copie si le volume refuse les liens durs), posé avant le
//! déplacement : il garde les octets quoi qu'il arrive au fichier de travail.

use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Local};

use super::fichier::{cause_disque, dossier_utilisable, existe, placer_avec, sync_tolere};
use super::glisser::{est_film, fichier_a_glisser};
use super::nom::{chemin_libre_ext, nom_fichier};
use super::presse_papiers;
use super::recents::{base_recent, cle_recent};
use super::{annonce, Annonce, Bilan, Demande, Etape, EtapeFichier};

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

/// Le dossier des fichiers donnés au presse-papiers et au glisser, à côté de « Recents »
/// (le même nom que `application::vignette::SOUS_DOSSIER_GLISSER`).
const SOUS_DOSSIER_GLISSER: &str = "Glisser";

/// Nombre maximal de suffixes « _n » essayés pour un fichier de travail.
const ESSAIS_MAX: u32 = 10_000;

/// Nombre de réservations essayées si d'autres écritures prennent le nom entre-temps.
const TENTATIVES: usize = 64;

/// Cause quand le fichier du film a disparu avant sa sortie.
const CAUSE_INTROUVABLE: &str = "Le fichier du film est introuvable.";

/// Le nom de la carte d'un film, dit aussi quand il n'y a ni copie ni enregistrement (F26 ;
/// le même texte que `application::film::TITRE_CARTE`).
const TITRE_TERMINE: &str = "Enregistrement terminé";

/// La sorte d'un nom de fichier de travail au format exact `AAAAMMJJ-HHMMSS-mmm[_n].mp4`
/// ou `.gif` (sur le modèle de `cle_recent`) ; `None` pour tout autre nom.
pub(super) fn cle_film(nom: &str) -> Option<SorteFilm> {
    [SorteFilm::Video, SorteFilm::Gif].into_iter().find(|s| {
        nom.strip_suffix(s.extension())
            .and_then(|t| t.strip_suffix('.'))
            .is_some_and(|tige| cle_recent(&format!("{tige}.png")).is_some())
    })
}

/// Un film de `<local>/Films`, vu par le nettoyage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilmGarde {
    pub nom: String,
    pub octets: u64,
    pub date: SystemTime,
}

/// F22, pur : les noms des films à retirer, du plus récent au plus ancien. `proteges` : le
/// film en cours, celui de la vignette, ceux dont la sortie tourne.
///
/// Toujours gardés, même au-delà des bornes : les protégés, le plus récent, et tout film
/// de moins d'une heure (une date dans le futur compte comme maintenant). Ils comptent
/// dans les bornes ; les autres sont ensuite gardés du plus récent au plus ancien tant
/// qu'ils tiennent dans dix films et 2 Go et qu'ils ont au plus sept jours.
pub fn films_a_retirer(
    films: &[FilmGarde],
    maintenant: SystemTime,
    proteges: &[String],
) -> Vec<String> {
    let mut ordre: Vec<&FilmGarde> = films.iter().collect();
    ordre.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| b.nom.cmp(&a.nom)));
    let age = |f: &FilmGarde| maintenant.duration_since(f.date).unwrap_or(Duration::ZERO);
    let garde_toujours: Vec<bool> = ordre
        .iter()
        .enumerate()
        .map(|(i, f)| i == 0 || age(f) < FILM_JEUNE || proteges.contains(&f.nom))
        .collect();
    let (mut nombre, mut octets) = (0usize, 0u64);
    for (f, &g) in ordre.iter().zip(&garde_toujours) {
        if g {
            nombre += 1;
            octets = octets.saturating_add(f.octets);
        }
    }
    let mut retirer = Vec::new();
    for (f, &g) in ordre.iter().zip(&garde_toujours) {
        if g {
            continue;
        }
        let tient = age(f) <= FILMS_AGE_MAX
            && nombre < FILMS_GARDES
            && octets.saturating_add(f.octets) <= FILMS_OCTETS_MAX;
        if tient {
            nombre += 1;
            octets += f.octets;
        } else {
            retirer.push(f.nom.clone());
        }
    }
    retirer
}

/// Les fichiers de `dossier` retenus par `garder` (nom), avec leur taille et leur date.
fn lister(dossier: &Path, garder: impl Fn(&str) -> bool) -> Vec<FilmGarde> {
    if dossier.as_os_str().is_empty() {
        return Vec::new();
    }
    let Ok(entrees) = fs::read_dir(dossier) else {
        return Vec::new();
    };
    entrees
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| {
            let nom = e.file_name().into_string().ok()?;
            if !garder(&nom) {
                return None;
            }
            let m = e.metadata().ok()?;
            Some(FilmGarde {
                nom,
                octets: m.len(),
                date: m.modified().ok()?,
            })
        })
        .collect()
}

/// Nettoie `Films` et les liens de films de `Glisser` (au démarrage, au début d'un film) ;
/// rend le nombre de films retirés de `Films` et leurs octets.
///
/// Seuls les fichiers au format exact de `cle_film` sont touchés dans `Films`. Dans
/// `Glisser`, les `.mp4` et `.gif` suivent les mêmes bornes (`films_a_retirer`, sans
/// protégés) : un lien dur a la date du film, et le fichier de travail peut être parti
/// (déplacé, choix U4) alors que le presse-papiers tient encore le lien ; l'heure de
/// grâce et le plus récent le gardent. Les `.png` de `Glisser` ne sont pas touchés.
pub fn nettoyer_films(
    dossier_films: &Path,
    dossier_glisser: &Path,
    maintenant: SystemTime,
    proteges: &[String],
) -> (usize, u64) {
    let films = lister(dossier_films, |n| cle_film(n).is_some());
    let (mut nombre, mut octets) = (0usize, 0u64);
    for nom in films_a_retirer(&films, maintenant, proteges) {
        let taille = films.iter().find(|f| f.nom == nom).map_or(0, |f| f.octets);
        if fs::remove_file(dossier_films.join(&nom)).is_ok() {
            nombre += 1;
            octets += taille;
        }
    }
    let liens = lister(dossier_glisser, |n| est_film(Path::new(n)));
    for nom in films_a_retirer(&liens, maintenant, &[]) {
        let _ = fs::remove_file(dossier_glisser.join(nom));
    }
    (nombre, octets)
}

/// Un chemin neuf dans `<local>/Films` (dossier créé), nommé `AAAAMMJJ-HHMMSS-mmm.<ext>`
/// (la base UTC de « Récents ») ; `_2`, `_3`… si le nom est pris. Le fichier n'est pas
/// créé : c'est l'écrivain du film qui le crée.
pub fn chemin_film(
    dossier_films: &Path,
    maintenant: DateTime<Local>,
    sorte: SorteFilm,
) -> Result<PathBuf, String> {
    const AFFICHE: &str = "des films";
    if !dossier_utilisable(dossier_films) {
        return Err(cause_disque(ErrorKind::NotFound, AFFICHE));
    }
    fs::create_dir_all(dossier_films).map_err(|e| cause_disque(e.kind(), AFFICHE))?;
    let base = base_recent(maintenant);
    let ext = sorte.extension();
    let mut chemin = dossier_films.join(format!("{base}.{ext}"));
    let mut n = 2;
    while existe(&chemin) {
        if n > ESSAIS_MAX {
            return Err(cause_disque(ErrorKind::AlreadyExists, AFFICHE));
        }
        chemin = dossier_films.join(format!("{base}_{n}.{ext}"));
        n += 1;
    }
    Ok(chemin)
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
/// volume). Bloquant, peut durer : fil `film-sortie` (F36).
///
/// La copie passe d'abord (son lien est fait sur le fichier de travail), l'enregistrement
/// ensuite ; un échec de l'une n'empêche pas l'autre. « Récents » n'est jamais touché
/// (F24).
pub fn sortir_film(
    fichier: &Path,
    sorte: SorteFilm,
    demande: &Demande,
    deplacer: bool,
) -> BilanFilm {
    let (copie, lien) = if demande.copier {
        copier_film(fichier, sorte, demande)
    } else {
        (Etape::NonDemandee, None)
    };
    let enregistrement = if !demande.enregistrer {
        EtapeFichier::NonDemande
    } else if deplacer {
        etape_fichier(demande, deplacer_dans(fichier, sorte, demande))
    } else {
        enregistrer_film(fichier, sorte, demande)
    };
    BilanFilm {
        copie,
        enregistrement,
        lien,
    }
}

/// Le bouton « Copier » de la vignette : le fichier dans le presse-papiers ; rend l'étape
/// et le lien posé dans `<local>/Glisser` (à effacer avec le film). Sans `Glisser`
/// utilisable, c'est le fichier lui-même qui est posé.
pub fn copier_film(
    fichier: &Path,
    sorte: SorteFilm,
    demande: &Demande,
) -> (Etape, Option<PathBuf>) {
    let _ = sorte;
    if !fichier.is_file() {
        return (Etape::Echec(CAUSE_INTROUVABLE.into()), None);
    }
    let nom = nom_fichier(&demande.modele_nom, demande.maintenant);
    let donne = match dossier_glisser(demande) {
        Some(d) => fichier_a_glisser(fichier, &d, &nom).unwrap_or_else(|_| fichier.into()),
        None => fichier.to_path_buf(),
    };
    let lien = (donne != fichier).then(|| donne.clone());
    let etape = match presse_papiers::copier_fichier(&donne) {
        Ok(()) => Etape::Faite,
        Err(cause) => Etape::Echec(cause),
    };
    (etape, lien)
}

/// Le bouton « Enregistrer » de la vignette : une copie dans le dossier d'enregistrement,
/// sous le nom du modèle (« (2) » si besoin), jamais rien d'écrasé.
pub fn enregistrer_film(fichier: &Path, sorte: SorteFilm, demande: &Demande) -> EtapeFichier {
    let resultat = preparer_dossier(fichier, demande).and_then(|nom| {
        copier_dans(
            fichier,
            &demande.dossier_enregistrement,
            &demande.dossier_affiche,
            &nom,
            sorte.extension(),
        )
    });
    etape_fichier(demande, resultat)
}

/// L'annonce du bandeau quand le bilan arrive sans vignette (F26), accordée pour une
/// vidéo (« Copiée », « Enregistrée ») ; pour un GIF, les textes d'une image. Sans copie
/// ni enregistrement : « Enregistrement terminé ». Les erreurs gardent leurs textes.
pub fn annonce_film(bilan: &BilanFilm, demande: &Demande, sorte: SorteFilm) -> Annonce {
    let video = sorte == SorteFilm::Video;
    let titre = |video_: &str, gif_: &str| Annonce::Reussite {
        titre: if video { video_ } else { gif_ }.to_string(),
    };
    let dossier = &demande.dossier_affiche;
    match (&bilan.copie, &bilan.enregistrement) {
        (Etape::Faite, EtapeFichier::Fait(_)) => {
            titre("Copiée et enregistrée", "Copié et enregistré")
        }
        (Etape::Faite, EtapeFichier::NonDemande) => titre(
            "Copiée dans le presse-papiers",
            "Copié dans le presse-papiers",
        ),
        (Etape::NonDemandee, EtapeFichier::Fait(_)) => titre(
            &format!("Enregistrée dans {dossier}"),
            &format!("Enregistré dans {dossier}"),
        ),
        (Etape::NonDemandee, EtapeFichier::NonDemande) => titre(TITRE_TERMINE, TITRE_TERMINE),
        // Les erreurs : les textes et l'ordre de `annonce`.
        _ => {
            let image = Bilan {
                copie: bilan.copie.clone(),
                recent: None,
                enregistrement: bilan.enregistrement.clone(),
            };
            annonce(&image, demande)
        }
    }
}

/// Efface le fichier de travail d'un film annulé ou sans image (seulement dans
/// `<local>/Films`, au format exact). Déjà absent : rien.
pub fn effacer_film(fichier: &Path) {
    let dans_films = fichier
        .parent()
        .and_then(|d| d.file_name())
        .is_some_and(|n| n == SOUS_DOSSIER_FILMS);
    let au_format = fichier
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| cle_film(n).is_some());
    if dans_films && au_format && fichier.is_absolute() {
        let _ = fs::remove_file(fichier);
    }
}

// ---------- Fichiers ----------

/// `<local>/Glisser`, à côté de `<local>/Recents` ; `None` si « Recents » n'est pas un
/// chemin absolu.
fn dossier_glisser(demande: &Demande) -> Option<PathBuf> {
    let recents = &demande.dossier_recents;
    if !dossier_utilisable(recents) {
        return None;
    }
    recents.parent().map(|l| l.join(SOUS_DOSSIER_GLISSER))
}

fn etape_fichier(demande: &Demande, resultat: Result<PathBuf, String>) -> EtapeFichier {
    match resultat {
        Ok(chemin) => EtapeFichier::Fait(chemin),
        Err(cause) => EtapeFichier::Echec {
            dossier: demande.dossier_enregistrement.clone(),
            cause,
        },
    }
}

/// Vérifie le film et le dossier d'enregistrement (créé si besoin) ; rend le nom du
/// fichier (modèle, sans extension).
fn preparer_dossier(fichier: &Path, demande: &Demande) -> Result<String, String> {
    if !fichier.is_file() {
        return Err(CAUSE_INTROUVABLE.into());
    }
    let dossier = &demande.dossier_enregistrement;
    let affiche = &demande.dossier_affiche;
    if !dossier_utilisable(dossier) {
        return Err(cause_disque(ErrorKind::NotFound, affiche));
    }
    fs::create_dir_all(dossier).map_err(|e| cause_disque(e.kind(), affiche))?;
    Ok(nom_fichier(&demande.modele_nom, demande.maintenant))
}

/// Choix U4 : le film déplacé dans le dossier d'enregistrement. Lien dur puis effacement
/// du fichier de travail (même volume) ; sinon renommage sur une réservation ; sinon
/// (autre volume) copie puis effacement. Un fichier de travail qui ne s'efface pas reste
/// dans `Films`, où les bornes de F22 le retireront.
fn deplacer_dans(fichier: &Path, sorte: SorteFilm, demande: &Demande) -> Result<PathBuf, String> {
    let nom = preparer_dossier(fichier, demande)?;
    let dossier = &demande.dossier_enregistrement;
    let ext = sorte.extension();
    let choisir = |d: &Path| chemin_libre_ext(d, &nom, ext, existe);
    let lie = placer_avec(
        dossier,
        &choisir,
        |cible| fs::hard_link(fichier, cible),
        || Err(io::Error::from(ErrorKind::Unsupported)),
        &mut thread::sleep,
    );
    if let Ok(chemin) = lie {
        let _ = fs::remove_file(fichier);
        return Ok(chemin);
    }
    if let Ok(chemin) = reserver_puis_renommer(dossier, fichier, &choisir) {
        return Ok(chemin);
    }
    let chemin = copier_dans(fichier, dossier, &demande.dossier_affiche, &nom, ext)?;
    let _ = fs::remove_file(fichier);
    Ok(chemin)
}

/// Réserve le premier nom libre (création exclusive d'un fichier vide), puis renomme
/// `source` dessus : seule notre réservation est remplacée. Échec du renommage (autre
/// volume) : la réservation est retirée.
fn reserver_puis_renommer(
    dossier: &Path,
    source: &Path,
    choisir: &impl Fn(&Path) -> PathBuf,
) -> io::Result<PathBuf> {
    for _ in 0..TENTATIVES {
        let cible = choisir(dossier);
        match OpenOptions::new().write(true).create_new(true).open(&cible) {
            Ok(reservation) => {
                drop(reservation);
                return match fs::rename(source, &cible) {
                    Ok(()) => Ok(cible),
                    Err(e) => {
                        let _ = fs::remove_file(&cible);
                        Err(e)
                    }
                };
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::from(ErrorKind::Other))
}

/// Compteur qui rend chaque nom de temporaire unique dans le processus.
static COMPTEUR: AtomicU64 = AtomicU64::new(0);

/// Un temporaire dans `dossier`, au nom des temporaires de l'app
/// (`.breezyshot-<pid>-<graine>-<n>.tmp`) : un oublié est retiré par le nettoyage des
/// enregistrements (`fichier::nettoyer_temporaires`).
fn creer_temporaire(dossier: &Path) -> io::Result<(PathBuf, File)> {
    let graine = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut erreur = io::Error::from(ErrorKind::AlreadyExists);
    for _ in 0..TENTATIVES {
        let numero = COMPTEUR.fetch_add(1, Ordering::Relaxed);
        let chemin = dossier.join(format!(
            ".breezyshot-{}-{graine}-{numero}.tmp",
            std::process::id()
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&chemin)
        {
            Ok(f) => return Ok((chemin, f)),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => erreur = e,
            Err(e) => return Err(e),
        }
    }
    Err(erreur)
}

/// Copie `source` dans `dossier` sous le premier nom libre `nom.<ext>`, au fil de l'eau
/// (jamais tout le film en mémoire) : un temporaire du dossier, écrit et vidé sur le
/// disque, puis posé sous son nom par lien dur ou réservation, comme une image
/// (`fichier::placer_avec`) ; rien n'est jamais écrasé, et aucun fichier à moitié écrit ne
/// porte le nom final.
fn copier_dans(
    source: &Path,
    dossier: &Path,
    affiche: &str,
    nom: &str,
    ext: &str,
) -> Result<PathBuf, String> {
    let mut entree = File::open(source).map_err(|_| CAUSE_INTROUVABLE.to_string())?;
    if !dossier_utilisable(dossier) {
        return Err(cause_disque(ErrorKind::NotFound, affiche));
    }
    fs::create_dir_all(dossier).map_err(|e| cause_disque(e.kind(), affiche))?;
    let (temporaire, mut sortie) =
        creer_temporaire(dossier).map_err(|e| cause_disque(e.kind(), affiche))?;
    let ecrit = io::copy(&mut entree, &mut sortie).and_then(|_| sync_tolere(sortie.sync_all()));
    drop(sortie);
    if let Err(e) = ecrit {
        let _ = fs::remove_file(&temporaire);
        return Err(cause_disque(e.kind(), affiche));
    }
    let choisir = |d: &Path| chemin_libre_ext(d, nom, ext, existe);
    let place = placer_avec(
        dossier,
        &choisir,
        |cible| fs::hard_link(&temporaire, cible),
        || reserver_puis_renommer(dossier, &temporaire, &choisir),
        &mut thread::sleep,
    );
    // Après un lien dur, le temporaire est en trop ; après un renommage, il n'existe plus.
    let _ = fs::remove_file(&temporaire);
    place.map_err(|e| cause_disque(e.kind(), affiche))
}
