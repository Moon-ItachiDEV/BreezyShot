//! Gestes système de l'installation (fichiers, registre de l'utilisateur, raccourci,
//! processus). Aucune décision ici : `installation/` dit quoi faire et où.
//!
//! Exception à la règle du Microsoft Store (ARCHITECTURE §2) : ce fichier est la seule
//! écriture du registre de l'installateur, et seulement la clé de désinstallation (HKCU),
//! pour le chemin d'installation et de désinstallation. Les Réglages écrivent, eux, la
//! valeur `Run` du démarrage avec Windows. Aucun autre état de l'app n'y est écrit.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[cfg(windows)]
#[path = "windows.rs"]
mod plateforme;

#[cfg(not(windows))]
#[path = "autre.rs"]
mod plateforme;

/// Variable d'essai : place libre annoncée, en octets (essais seulement).
pub const VARIABLE_LIBRE: &str = "BREEZYSHOT_ESSAI_LIBRE_OCTETS";
/// Variable d'essai : faire comme si WebView2 manquait (essais seulement).
pub const VARIABLE_SANS_WEBVIEW2: &str = "BREEZYSHOT_ESSAI_SANS_WEBVIEW2";
/// Variable d'essai : jouer l'envol même si Windows coupe les animations (essais seulement).
pub const VARIABLE_ANIMATIONS: &str = "BREEZYSHOT_ESSAI_ANIMATIONS";

/// Vrai si `BREEZYSHOT_ESSAI_ANIMATIONS=1` **et** un dossier d'essai est donné : sans effet
/// hors essai.
pub fn essai_animations_forcees() -> bool {
    let essai = std::env::var_os(crate::systeme::VARIABLE_ESSAI).is_some_and(|v| !v.is_empty());
    essai && std::env::var_os(VARIABLE_ANIMATIONS).is_some_and(|v| v == "1")
}

/// Taille des blocs de la copie.
pub const BLOC: usize = 1 << 20;

/// Les dossiers que le système connaît, d'où `installation::emplacements` tire les siens.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DossiersConnus {
    /// Programmes de l'utilisateur (Windows : `FOLDERID_UserProgramFiles`,
    /// `%LOCALAPPDATA%\Programs`).
    pub programmes: Option<PathBuf>,
    /// Programmes du menu Démarrer de l'utilisateur (`FOLDERID_Programs`).
    pub menu: Option<PathBuf>,
    /// Données locales (`%LOCALAPPDATA%`).
    pub local: Option<PathBuf>,
    /// Dossier temporaire (`%TEMP%`).
    pub temporaire: PathBuf,
}

/// Une valeur écrite dans la clé de désinstallation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Valeur {
    Texte(String),
    Nombre(u32),
}

/// Erreur d'un geste sur les fichiers, réduite à ce qui change la suite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErreurFichier {
    /// Le fichier est pris par un autre programme.
    Pris,
    /// Déplacement vers un autre volume (impossible par renommage).
    AutreVolume,
    /// Accès refusé, dossier impossible à créer, disque plein… Le diagnostic donne le type
    /// d'erreur et son code système, jamais de chemin.
    Refuse(String),
}

impl std::fmt::Display for ErreurFichier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErreurFichier::Pris => write!(f, "fichier pris"),
            ErreurFichier::AutreVolume => write!(f, "autre volume"),
            ErreurFichier::Refuse(e) => write!(f, "{e}"),
        }
    }
}

fn vers_erreur(e: io::Error) -> ErreurFichier {
    if plateforme::est_partage_refuse(&e) {
        ErreurFichier::Pris
    } else if e.kind() == io::ErrorKind::CrossesDevices {
        ErreurFichier::AutreVolume
    } else {
        ErreurFichier::Refuse(diagnostic(&e))
    }
}

/// Type d'erreur et code système (« permission denied, code 5 »), sans le chemin.
pub fn diagnostic(e: &io::Error) -> String {
    match e.raw_os_error() {
        Some(code) => format!("{}, code {code}", e.kind()),
        None => e.kind().to_string(),
    }
}

/// Les dossiers connus du système.
pub fn dossiers_connus() -> DossiersConnus {
    plateforme::dossiers_connus()
}

/// Le chemin du programme en cours.
pub fn exe_courant() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| diagnostic(&e))
}

/// Place libre sur le volume du premier dossier existant parmi `dossier` et ses parents.
/// Essais : `BREEZYSHOT_ESSAI_LIBRE_OCTETS` la remplace.
pub fn espace_libre(dossier: &Path) -> Option<u64> {
    if let Some(v) = std::env::var_os(VARIABLE_LIBRE) {
        return v.to_str().and_then(|t| t.trim().parse().ok());
    }
    let existant = dossier.ancestors().find(|d| d.is_dir())?;
    plateforme::espace_libre(existant)
}

/// Lettre du volume (« C: »), `None` hors Windows ou pour un chemin réseau.
pub fn lettre_volume(dossier: &Path) -> Option<String> {
    plateforme::lettre_volume(dossier)
}

/// Copie le **flux principal** de `source` dans `cible` (créée ou remplacée), par blocs,
/// en appelant `rappel(octets copiés)` après chaque bloc. Aucun flux nommé n'est copié
/// (pas de `Zone.Identifier`). Les dossiers parents sont créés.
pub fn copier(
    source: &Path,
    cible: &Path,
    rappel: &mut dyn FnMut(u64),
) -> Result<u64, ErreurFichier> {
    if let Some(parent) = cible.parent() {
        std::fs::create_dir_all(parent).map_err(vers_erreur)?;
    }
    let mut lecture = File::open(source).map_err(vers_erreur)?;
    let mut ecriture = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(cible)
        .map_err(vers_erreur)?;
    let mut tampon = vec![0u8; BLOC];
    let mut total = 0u64;
    loop {
        let n = lecture.read(&mut tampon).map_err(vers_erreur)?;
        if n == 0 {
            break;
        }
        ecriture.write_all(&tampon[..n]).map_err(vers_erreur)?;
        total += n as u64;
        rappel(total);
    }
    ecriture.sync_all().map_err(vers_erreur)?;
    Ok(total)
}

/// Vrai si les deux fichiers ont exactement les mêmes octets.
pub fn identiques(a: &Path, b: &Path) -> Result<bool, ErreurFichier> {
    let mut fa = File::open(a).map_err(vers_erreur)?;
    let mut fb = File::open(b).map_err(vers_erreur)?;
    if fa.metadata().map_err(vers_erreur)?.len() != fb.metadata().map_err(vers_erreur)?.len() {
        return Ok(false);
    }
    let mut ta = vec![0u8; BLOC];
    let mut tb = vec![0u8; BLOC];
    loop {
        let na = lire_plein(&mut fa, &mut ta).map_err(vers_erreur)?;
        let nb = lire_plein(&mut fb, &mut tb).map_err(vers_erreur)?;
        if na != nb || ta[..na] != tb[..nb] {
            return Ok(false);
        }
        if na == 0 {
            return Ok(true);
        }
    }
}

fn lire_plein(f: &mut File, tampon: &mut [u8]) -> io::Result<usize> {
    let mut lu = 0;
    while lu < tampon.len() {
        let n = f.read(&mut tampon[lu..])?;
        if n == 0 {
            break;
        }
        lu += n;
    }
    Ok(lu)
}

/// Remplace `cible` par `provisoire` (renommage), en retentant tant que la cible est prise,
/// `attente` au plus.
pub fn remplacer(provisoire: &Path, cible: &Path, attente: Duration) -> Result<(), ErreurFichier> {
    let debut = Instant::now();
    loop {
        match plateforme::remplacer(provisoire, cible).map_err(vers_erreur) {
            Err(ErreurFichier::Pris) if debut.elapsed() < attente => {
                std::thread::sleep(Duration::from_millis(100));
            }
            autre => return autre,
        }
    }
}

/// Déplace un fichier (permis sous Windows pour un exe en cours, sur le même volume).
pub fn deplacer(source: &Path, cible: &Path) -> Result<(), ErreurFichier> {
    std::fs::rename(source, cible).map_err(vers_erreur)
}

/// Supprime un fichier. `Ok(false)` s'il n'existait pas.
pub fn supprimer_fichier(chemin: &Path) -> Result<bool, ErreurFichier> {
    match std::fs::remove_file(chemin) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(vers_erreur(e)),
    }
}

/// Retire un dossier **seulement s'il est vide** (jamais son contenu). `Ok(false)` s'il
/// n'existe pas ou n'est pas vide.
pub fn retirer_dossier_vide(chemin: &Path) -> Result<bool, ErreurFichier> {
    if !chemin.is_dir() {
        return Ok(false);
    }
    let vide = std::fs::read_dir(chemin)
        .map_err(vers_erreur)?
        .next()
        .is_none();
    if !vide {
        return Ok(false);
    }
    std::fs::remove_dir(chemin).map_err(vers_erreur)?;
    Ok(true)
}

/// Noms des entrées directes d'un dossier (vide s'il n'existe pas).
pub fn entrees(chemin: &Path) -> Vec<String> {
    std::fs::read_dir(chemin)
        .map(|l| {
            l.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// Supprime un dossier et tout son contenu. Réservé au dossier WebView2 de l'installateur,
/// après que `installation::dossier_webview_a_nous` l'a reconnu.
pub fn supprimer_dossier_webview(chemin: &Path) -> Result<bool, ErreurFichier> {
    match std::fs::remove_dir_all(chemin) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(vers_erreur(e)),
    }
}

/// Crée (ou remplace) le raccourci `lien` vers `cible`. `Ok(false)` : sans objet ici.
/// L'erreur donne l'appel et son HRESULT (ou son code système), jamais de chemin.
pub fn creer_raccourci(lien: &Path, cible: &Path) -> Result<bool, String> {
    plateforme::creer_raccourci(lien, cible)
}

/// Cible d'un raccourci, `None` s'il est absent ou illisible (ou hors Windows).
pub fn cible_raccourci(lien: &Path) -> Option<PathBuf> {
    plateforme::cible_raccourci(lien)
}

/// Écrit les valeurs dans la clé `cle` de HKCU (créée au besoin). `Ok(false)` : sans objet.
/// L'erreur donne le geste et le code Win32, jamais le nom de la clé.
pub fn ecrire_cle(cle: &str, valeurs: &[(&str, Valeur)]) -> Result<bool, String> {
    plateforme::ecrire_cle(cle, valeurs)
}

/// Lit une valeur texte de HKCU.
pub fn lire_texte(cle: &str, nom: &str) -> Option<String> {
    plateforme::lire_texte_cle(cle, nom)
}

/// Retire les valeurs `noms` de la clé, puis la clé si elle n'a plus ni valeur ni
/// sous-clé. Rend vrai si la clé a disparu.
pub fn retirer_valeurs_et_cle(cle: &str, noms: &[&str]) -> Result<bool, String> {
    plateforme::retirer_valeurs_et_cle(cle, noms)
}

/// Vrai si un mutex de ce nom existe dans la session.
pub fn verrou_existe(nom: &str) -> bool {
    plateforme::verrou_existe(nom)
}

/// Un mutex nommé tenu tant que la valeur vit.
pub struct Verrou(#[allow(dead_code)] plateforme::Poignee);

/// Prend le mutex `nom` ; `None` s'il est déjà tenu par un autre processus.
pub fn prendre_verrou(nom: &str) -> Option<Verrou> {
    plateforme::prendre_verrou(nom).map(Verrou)
}

/// Lance `exe` avec `arguments` (environnement hérité, sans passer par le shell).
/// Rend l'identifiant du processus.
pub fn lancer(exe: &Path, arguments: &[&str]) -> Result<u32, String> {
    let mut commande = std::process::Command::new(exe);
    commande.args(arguments);
    if let Some(dossier) = exe.parent() {
        commande.current_dir(dossier);
    }
    commande
        .spawn()
        .map(|enfant| enfant.id())
        .map_err(|e| diagnostic(&e))
}

/// Lance `exe` et attend sa fin, `delai` au plus. Rend le code de sortie, `None` au-delà.
/// Le processus n'est jamais tué.
pub fn lancer_et_attendre(exe: &Path, arguments: &[&str], delai: Duration) -> Option<i32> {
    let mut enfant = std::process::Command::new(exe)
        .args(arguments)
        .spawn()
        .ok()?;
    let debut = Instant::now();
    while debut.elapsed() < delai {
        match enfant.try_wait() {
            Ok(Some(statut)) => return Some(statut.code().unwrap_or(-1)),
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => return None,
        }
    }
    None
}

/// Permet à tout processus de prendre le premier plan jusqu'à la prochaine entrée
/// (Windows : `AllowSetForegroundWindow(ASFW_ANY)`). Sans effet ailleurs.
pub fn permettre_premier_plan() {
    plateforme::permettre_premier_plan()
}

/// Version de WebView2, `None` s'il manque. Hors Windows : « sans objet ».
/// Essais : `BREEZYSHOT_ESSAI_SANS_WEBVIEW2=1` le fait manquer.
pub fn version_webview2() -> Option<String> {
    if std::env::var_os(VARIABLE_SANS_WEBVIEW2).is_some_and(|v| v == "1") {
        return None;
    }
    plateforme::version_webview2()
}

/// Rectangle de la zone de notification (pixels réels), `None` s'il est inconnu.
pub fn zone_de_notification() -> Option<super::Rect> {
    plateforme::zone_de_notification()
}

/// Zone utile (sans la barre des tâches) de l'écran sous la souris, et son échelle.
pub fn zone_utile_sous_souris() -> Option<(super::Rect, f64)> {
    plateforme::zone_utile_sous_souris()
}

/// Message système bloquant, un bouton OK. Hors Windows : écrit sur la sortie d'erreur.
pub fn message_bloquant(titre: &str, texte: &str) {
    plateforme::message_bloquant(titre, texte)
}

/// Rectangle réel de la fenêtre à l'écran (pixels réels), `None` s'il est inconnu.
pub fn rect_fenetre(identifiant: isize) -> Option<super::Rect> {
    plateforme::rect_fenetre(identifiant)
}

/// Ramène au premier plan la fenêtre visible qui porte ce titre (seconde ouverture de
/// l'installateur). Rend vrai si une fenêtre visible a été trouvée.
pub fn ramener_fenetre(titre: &str) -> bool {
    plateforme::ramener_fenetre(titre)
}

/// Pose la fenêtre (son coin haut gauche, pixels réels) sans la redimensionner ni
/// l'activer. Rend vrai si c'est fait.
pub fn deplacer_fenetre(identifiant: isize, x: i32, y: i32) -> bool {
    plateforme::deplacer_fenetre(identifiant, x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copie_et_comparaison() {
        let d = tempfile::tempdir().unwrap();
        let source = d.path().join("source.bin");
        let octets: Vec<u8> = (0..(3 * BLOC + 17)).map(|i| (i % 251) as u8).collect();
        std::fs::write(&source, &octets).unwrap();
        let cible = d.path().join("sous dossier é").join("cible.bin");
        let mut vus = Vec::new();
        let n = copier(&source, &cible, &mut |o| vus.push(o)).unwrap();
        assert_eq!(n, octets.len() as u64);
        assert_eq!(vus.last(), Some(&n));
        assert!(vus.windows(2).all(|p| p[0] < p[1]));
        assert!(identiques(&source, &cible).unwrap());
        std::fs::write(&cible, b"autre").unwrap();
        assert!(!identiques(&source, &cible).unwrap());
    }

    #[test]
    fn remplacement() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("a.nouveau");
        let b = d.path().join("b");
        std::fs::write(&a, b"neuf").unwrap();
        std::fs::write(&b, b"vieux").unwrap();
        remplacer(&a, &b, Duration::from_millis(10)).unwrap();
        assert_eq!(std::fs::read(&b).unwrap(), b"neuf");
        assert!(!a.exists());
    }

    #[test]
    fn dossier_retire_seulement_vide() {
        let d = tempfile::tempdir().unwrap();
        let dossier = d.path().join("BreezyShot");
        std::fs::create_dir(&dossier).unwrap();
        std::fs::write(dossier.join("a moi.txt"), b"x").unwrap();
        assert!(!retirer_dossier_vide(&dossier).unwrap());
        assert!(dossier.join("a moi.txt").exists());
        std::fs::remove_file(dossier.join("a moi.txt")).unwrap();
        assert!(retirer_dossier_vide(&dossier).unwrap());
        assert!(!retirer_dossier_vide(&dossier).unwrap());
    }

    #[test]
    fn suppression_d_un_absent() {
        let d = tempfile::tempdir().unwrap();
        assert!(!supprimer_fichier(&d.path().join("rien")).unwrap());
    }
}
