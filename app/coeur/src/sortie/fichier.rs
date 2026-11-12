//! Ecriture sur le disque, sans jamais ecraser un fichier existant.
//!
//! Methode : on ecrit tout dans un fichier temporaire du meme dossier, puis on
//! cree un lien dur du temporaire vers le nom final (`fs::hard_link` echoue si
//! le nom existe deja : il n'ecrase jamais), puis on supprime le temporaire.
//! Si le disque ne prend pas en charge les liens durs, repli : on reserve le
//! nom final (creation exclusive d'un fichier vide), puis on renomme le
//! temporaire dessus. Seule notre propre reservation est alors remplacee.
//!
//! Seule exception : `enregistrer_a` (Enregistrer sous... de l'editeur) remplace le fichier
//! choisi, puisque l'utilisateur a confirme le remplacement dans le dialogue du systeme.

use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::nom::chemin_libre;
use super::{png, EtapeFichier};
use crate::systeme::gel::Pixels;

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

/// Enregistre `pixels` en PNG exactement a `chemin` (Enregistrer sous... de l'editeur, 0.4) :
/// temporaire du meme dossier, puis remplacement (l'utilisateur l'a confirme dans le
/// dialogue du systeme). Alpha garde avec `transparence`, sinon force a 255.
///
/// Le dossier n'est jamais cree : il a ete choisi dans le dialogue, il existe ou l'ecriture
/// echoue. Les causes ne nomment ni le dossier ni le fichier (« Le dossier est
/// introuvable. ») : aucun chemin de l'utilisateur ne peut en sortir.
pub fn enregistrer_a(pixels: &Pixels, chemin: &Path, transparence: bool) -> EtapeFichier {
    let dossier = chemin.parent().map(Path::to_path_buf).unwrap_or_default();
    match ecrire_a(pixels, chemin, &dossier, transparence) {
        Ok(()) => EtapeFichier::Fait(chemin.to_path_buf()),
        Err(cause) => EtapeFichier::Echec { dossier, cause },
    }
}

fn ecrire_a(
    pixels: &Pixels,
    chemin: &Path,
    dossier: &Path,
    transparence: bool,
) -> Result<(), String> {
    png::valider(pixels)?;
    if chemin.file_name().is_none() || !dossier_utilisable(dossier) || !dossier.is_dir() {
        return Err(cause_disque(ErrorKind::NotFound, ""));
    }
    let (largeur, hauteur) = (pixels.largeur, pixels.hauteur);
    let octets = if transparence {
        png::encoder_rgba(largeur, hauteur, &pixels.rgba)?
    } else {
        png::encoder_rgba(largeur, hauteur, &png::opaques(&pixels.rgba))?
    };
    nettoyer_temporaires(dossier);

    let (temporaire, mut f) =
        creer_temporaire(dossier).map_err(|e| cause_temporaire(e.kind(), ""))?;
    // Sur le disque avant le remplacement : un ancien fichier n'est jamais remplace par un
    // fichier a moitie ecrit, meme si la machine s'arrete juste apres.
    let ecrit = f.write_all(&octets).and_then(|()| f.sync_all());
    drop(f);
    if let Err(e) = ecrit {
        let _ = fs::remove_file(&temporaire);
        return Err(cause_disque(e.kind(), ""));
    }
    // `fs::rename` remplace un fichier existant (Windows : MOVEFILE_REPLACE_EXISTING) ;
    // il echoue sur un dossier. Fichier tenu : trois essais, puis echec.
    let place = avec_essais_si_tenu(|| fs::rename(&temporaire, chemin), &mut thread::sleep);
    if let Err(e) = place {
        let _ = fs::remove_file(&temporaire);
        return Err(cause_disque(e.kind(), ""));
    }
    Ok(())
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
pub(super) fn suite_lien(genre: ErrorKind, code: Option<i32>) -> SuiteLien {
    if genre == ErrorKind::AlreadyExists {
        SuiteLien::NomSuivant
    } else if est_tenu(genre, code) {
        SuiteLien::Reessayer
    } else {
        SuiteLien::Repli
    }
}

/// Vrai pour un refus d'acces ou un fichier tenu par un autre programme.
/// Le code 32 est l'erreur de partage de Windows ; sous Unix, 32 est EPIPE,
/// que ni `link` ni `rename` ne rendent : le meme test vaut partout.
pub(super) fn est_tenu(genre: ErrorKind, code: Option<i32>) -> bool {
    genre == ErrorKind::PermissionDenied || code == Some(ERREUR_PARTAGE_WINDOWS)
}

fn tenu(e: &io::Error) -> bool {
    est_tenu(e.kind(), e.raw_os_error())
}

/// Lance `operation`, et la relance jusqu'a 3 fois en tout, a 50 ms
/// d'intervalle (par `dormir`), tant que le fichier est tenu.
pub(super) fn avec_essais_si_tenu<T>(
    mut operation: impl FnMut() -> io::Result<T>,
    dormir: &mut impl FnMut(Duration),
) -> io::Result<T> {
    let mut essai = 1;
    loop {
        match operation() {
            Err(e) if tenu(&e) && essai < ESSAIS_SI_TENU => {
                essai += 1;
                dormir(ATTENTE_SI_TENU);
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
    placer_avec(
        dossier,
        choisir,
        |cible| fs::hard_link(temporaire, cible),
        || placer_par_reservation(dossier, temporaire, choisir),
        &mut thread::sleep,
    )
}

/// Le coeur de `placer`, avec ses operations en parametres pour les tests :
/// `lier` cree le lien dur vers une cible, `repli` est la methode de repli.
pub(super) fn placer_avec(
    dossier: &Path,
    choisir: &impl Fn(&Path) -> PathBuf,
    mut lier: impl FnMut(&Path) -> io::Result<()>,
    repli: impl FnOnce() -> io::Result<PathBuf>,
    dormir: &mut impl FnMut(Duration),
) -> io::Result<PathBuf> {
    for _ in 0..TENTATIVES {
        let cible = choisir(dossier);
        let Err(e) = avec_essais_si_tenu(|| lier(&cible), dormir) else {
            return Ok(cible);
        };
        match suite_lien(e.kind(), e.raw_os_error()) {
            SuiteLien::NomSuivant => continue,
            // Encore refuse apres 3 essais. Sous Linux et macOS, un disque FAT
            // ou exFAT repond aussi « refuse » pour un lien dur : on tente
            // donc une fois le repli, qui echouera de meme si l'acces est
            // vraiment refuse.
            SuiteLien::Reessayer | SuiteLien::Repli => return repli(),
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
                return match avec_essais_si_tenu(
                    || fs::rename(temporaire, &cible),
                    &mut thread::sleep,
                ) {
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
/// Un age inconnu (date dans le futur, illisible) ne fait rien supprimer, ni
/// un temporaire du processus courant (`pid`) : il peut etre en cours
/// d'ecriture, meme si l'horloge d'un disque reseau le fait paraitre vieux.
pub(super) fn temporaire_perime(nom: &str, age: Option<Duration>, pid: u32) -> bool {
    est_nom_temporaire(nom)
        && !est_temporaire_du_processus(nom, pid)
        && age.is_some_and(|a| a >= AGE_TEMPORAIRE_PERIME)
}

/// Vrai si le temporaire porte le numero de processus `pid`.
fn est_temporaire_du_processus(nom: &str, pid: u32) -> bool {
    nom.strip_prefix(".breezyshot-")
        .and_then(|r| r.split('-').next())
        .is_some_and(|p| p == pid.to_string())
}

/// Supprime nos propres temporaires perimes du dossier. Les erreurs sont
/// ignorees : ce menage n'est jamais une raison d'echouer.
fn nettoyer_temporaires(dossier: &Path) {
    let Ok(entrees) = fs::read_dir(dossier) else {
        return;
    };
    let maintenant = SystemTime::now();
    let pid = std::process::id();
    for entree in entrees.filter_map(|e| e.ok()) {
        // Le nom d'abord : rien d'autre n'est lu pour un fichier etranger.
        let Ok(nom) = entree.file_name().into_string() else {
            continue;
        };
        if !est_nom_temporaire(&nom) || est_temporaire_du_processus(&nom, pid) {
            continue;
        }
        let Ok(genre) = entree.file_type() else {
            continue;
        };
        if !genre.is_file() {
            continue;
        }
        let age = entree
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| maintenant.duration_since(t).ok());
        if temporaire_perime(&nom, age, pid) {
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
