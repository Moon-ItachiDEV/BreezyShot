//! Ecriture sur le disque, sans jamais ecraser un fichier existant.
//!
//! Methode : on ecrit tout dans un fichier temporaire du meme dossier, puis on
//! cree un lien dur du temporaire vers le nom final (`fs::hard_link` echoue si
//! le nom existe deja : il n'ecrase jamais), puis on supprime le temporaire.
//! Si le disque ne prend pas en charge les liens durs, repli : on reserve le
//! nom final (creation exclusive d'un fichier vide), puis on renomme le
//! temporaire dessus. Seule notre propre reservation est alors remplacee.

use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::nom::chemin_libre;

/// Nombre de noms essayes si d'autres ecritures prennent le nom entre-temps.
const TENTATIVES: usize = 64;

/// Essais quand le fichier est tenu par un autre programme.
const ESSAIS_SI_TENU: usize = 3;

/// Attente entre deux essais quand le fichier est tenu.
const ATTENTE_SI_TENU: Duration = Duration::from_millis(50);

/// Age a partir duquel un de nos temporaires oublies est supprime.
pub(super) const AGE_TEMPORAIRE_PERIME: Duration = Duration::from_secs(3600);

/// Erreur systeme Windows 32 : fichier utilise par un autre processus.
const ERREUR_PARTAGE_WINDOWS: i32 = 32;

/// Compteur qui rend chaque nom de temporaire unique dans le processus.
static COMPTEUR: AtomicU64 = AtomicU64::new(0);

/// Un nom est occupe des qu'une entree existe, meme un lien casse.
pub(super) fn existe(chemin: &Path) -> bool {
    chemin.symlink_metadata().is_ok()
}

/// Enregistre `octets` dans `dossier` sous le premier nom libre derive de `nom`.
pub(super) fn enregistrer(
    dossier: &Path,
    affiche: &str,
    nom: &str,
    octets: &[u8],
) -> Result<PathBuf, String> {
    ecrire_sans_ecraser(dossier, affiche, octets, |d| chemin_libre(d, nom, existe))
}

/// Vrai si le dossier peut recevoir une ecriture : non vide et absolu.
/// Un chemin relatif ecrirait dans le dossier courant du processus.
pub(super) fn dossier_utilisable(dossier: &Path) -> bool {
    !dossier.as_os_str().is_empty() && dossier.is_absolute()
}

/// Cree le dossier si besoin et ecrit le fichier choisi par `choisir`.
/// `affiche` est le nom du dossier pour l'utilisateur, utilise dans la cause.
pub(super) fn ecrire_sans_ecraser(
    dossier: &Path,
    affiche: &str,
    octets: &[u8],
    choisir: impl Fn(&Path) -> PathBuf,
) -> Result<PathBuf, String> {
    if !dossier_utilisable(dossier) {
        return Err(cause_disque(ErrorKind::NotFound, affiche));
    }
    fs::create_dir_all(dossier).map_err(|e| cause_disque(e.kind(), affiche))?;
    nettoyer_temporaires(dossier);

    let (temporaire, mut f) =
        creer_temporaire(dossier).map_err(|e| cause_temporaire(e.kind(), affiche))?;
    let ecrit = f.write_all(octets);
    drop(f);
    if let Err(e) = ecrit {
        let _ = fs::remove_file(&temporaire);
        return Err(cause_disque(e.kind(), affiche));
    }

    let resultat = placer(dossier, &temporaire, &choisir);
    // Apres un lien dur, le temporaire est en trop ; apres un renommage, il
    // n'existe plus et cette suppression echoue sans consequence.
    let _ = fs::remove_file(&temporaire);
    resultat.map_err(|e| cause_disque(e.kind(), affiche))
}

/// Ce qu'on fait apres un echec du lien dur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SuiteLien {
    /// Le nom est pris : on en choisit un autre.
    NomSuivant,
    /// Le fichier est tenu ou refuse : on reessaie un peu.
    Reessayer,
    /// Le disque ne sait pas faire de lien dur : methode de repli.
    Repli,
}

/// Decision apres un echec de `fs::hard_link`.
pub(super) fn suite_lien(genre: ErrorKind, code: Option<i32>, sous_windows: bool) -> SuiteLien {
    if genre == ErrorKind::AlreadyExists {
        SuiteLien::NomSuivant
    } else if est_tenu(genre, code, sous_windows) {
        SuiteLien::Reessayer
    } else {
        SuiteLien::Repli
    }
}

/// Vrai pour un refus d'acces ou un fichier tenu par un autre programme.
pub(super) fn est_tenu(genre: ErrorKind, code: Option<i32>, sous_windows: bool) -> bool {
    genre == ErrorKind::PermissionDenied || (sous_windows && code == Some(ERREUR_PARTAGE_WINDOWS))
}

fn sous_windows() -> bool {
    std::env::consts::OS == "windows"
}

fn tenu(e: &io::Error) -> bool {
    est_tenu(e.kind(), e.raw_os_error(), sous_windows())
}

/// Lance `operation`, et la relance jusqu'a 3 fois en tout, a 50 ms
/// d'intervalle, tant que le fichier est tenu.
fn avec_essais_si_tenu(mut operation: impl FnMut() -> io::Result<()>) -> io::Result<()> {
    let mut essai = 1;
    loop {
        match operation() {
            Err(e) if tenu(&e) && essai < ESSAIS_SI_TENU => {
                essai += 1;
                thread::sleep(ATTENTE_SI_TENU);
            }
            autre => return autre,
        }
    }
}

fn placer(
    dossier: &Path,
    temporaire: &Path,
    choisir: &impl Fn(&Path) -> PathBuf,
) -> io::Result<PathBuf> {
    for _ in 0..TENTATIVES {
        let cible = choisir(dossier);
        let Err(e) = avec_essais_si_tenu(|| fs::hard_link(temporaire, &cible)) else {
            return Ok(cible);
        };
        match suite_lien(e.kind(), e.raw_os_error(), sous_windows()) {
            SuiteLien::NomSuivant => continue,
            // Encore refuse apres 3 essais. Sous Linux et macOS, un disque FAT
            // ou exFAT repond aussi « refuse » pour un lien dur : on tente
            // donc une fois le repli, qui echouera de meme si l'acces est
            // vraiment refuse.
            SuiteLien::Reessayer | SuiteLien::Repli => {
                return placer_par_reservation(dossier, temporaire, choisir)
            }
        }
    }
    Err(io::Error::from(ErrorKind::Other))
}

fn placer_par_reservation(
    dossier: &Path,
    temporaire: &Path,
    choisir: &impl Fn(&Path) -> PathBuf,
) -> io::Result<PathBuf> {
    for _ in 0..TENTATIVES {
        let cible = choisir(dossier);
        match OpenOptions::new().write(true).create_new(true).open(&cible) {
            Ok(reservation) => {
                drop(reservation);
                return match avec_essais_si_tenu(|| fs::rename(temporaire, &cible)) {
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

fn creer_temporaire(dossier: &Path) -> io::Result<(PathBuf, fs::File)> {
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

/// Vrai si le nom a exactement la forme de nos temporaires :
/// `.breezyshot-<chiffres>-<chiffres>-<chiffres>.tmp`.
pub(super) fn est_nom_temporaire(nom: &str) -> bool {
    let Some(milieu) = nom
        .strip_prefix(".breezyshot-")
        .and_then(|r| r.strip_suffix(".tmp"))
    else {
        return false;
    };
    let parties: Vec<&str> = milieu.split('-').collect();
    parties.len() == 3
        && parties
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Vrai si l'entree est un de nos temporaires oublie depuis plus d'une heure.
/// Un age inconnu (date dans le futur, illisible) ne fait rien supprimer.
pub(super) fn temporaire_perime(nom: &str, age: Option<Duration>) -> bool {
    est_nom_temporaire(nom) && age.is_some_and(|a| a >= AGE_TEMPORAIRE_PERIME)
}

/// Supprime nos propres temporaires perimes du dossier. Les erreurs sont
/// ignorees : ce menage n'est jamais une raison d'echouer.
fn nettoyer_temporaires(dossier: &Path) {
    let Ok(entrees) = fs::read_dir(dossier) else {
        return;
    };
    let maintenant = SystemTime::now();
    for entree in entrees.filter_map(|e| e.ok()) {
        let Ok(genre) = entree.file_type() else {
            continue;
        };
        if !genre.is_file() {
            continue;
        }
        let Ok(nom) = entree.file_name().into_string() else {
            continue;
        };
        let age = entree
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| maintenant.duration_since(t).ok());
        if temporaire_perime(&nom, age) {
            let _ = fs::remove_file(entree.path());
        }
    }
}

/// « Le dossier X », ou « Le dossier » si le nom est vide.
fn le_dossier(affiche: &str) -> String {
    if affiche.is_empty() {
        "Le dossier".to_string()
    } else {
        format!("Le dossier {affiche}")
    }
}

/// « le dossier X », ou « ce dossier » si le nom est vide.
fn dans_le_dossier(affiche: &str) -> String {
    if affiche.is_empty() {
        "ce dossier".to_string()
    } else {
        format!("le dossier {affiche}")
    }
}

/// Cause en clair d'une erreur de disque.
pub(super) fn cause_disque(genre: ErrorKind, affiche: &str) -> String {
    match genre {
        // Dossier absent, ou lui-meme ou un parent est un fichier.
        ErrorKind::NotFound | ErrorKind::NotADirectory | ErrorKind::AlreadyExists => {
            format!("{} est introuvable.", le_dossier(affiche))
        }
        ErrorKind::PermissionDenied => format!(
            "BreezyShot n'a pas le droit d'écrire dans {}.",
            dans_le_dossier(affiche)
        ),
        ErrorKind::StorageFull | ErrorKind::QuotaExceeded => "Le disque est plein.".to_string(),
        ErrorKind::ReadOnlyFilesystem => format!("{} est en lecture seule.", le_dossier(affiche)),
        _ => format!(
            "Le fichier n'a pas pu être écrit dans {}.",
            dans_le_dossier(affiche)
        ),
    }
}

/// Cause en clair quand le fichier temporaire ne peut pas etre cree.
pub(super) fn cause_temporaire(genre: ErrorKind, affiche: &str) -> String {
    match genre {
        ErrorKind::PermissionDenied
        | ErrorKind::StorageFull
        | ErrorKind::QuotaExceeded
        | ErrorKind::ReadOnlyFilesystem
        | ErrorKind::NotFound => cause_disque(genre, affiche),
        _ => format!(
            "Le fichier temporaire n'a pas pu être créé dans {}.",
            dans_le_dossier(affiche)
        ),
    }
}
