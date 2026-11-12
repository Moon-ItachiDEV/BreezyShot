//! Journal de l'app : `<local>/Journal/breezyshot.log`, une ligne par événement.
//!
//! Les modules écrivent avec la façade `log` (`log::info!`…) sans connaître l'Application.
//! Rotation : 1 Mo par fichier, 5 fichiers au plus (`breezyshot.log`, `.1` … `.4`).
//! Jamais d'image ni de contenu du presse-papiers ; les chemins de fichiers sont permis.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use log::{Level, LevelFilter, Log, Metadata, Record};

pub const NOM_FICHIER: &str = "breezyshot.log";
/// Journal de l'installateur et de la désinstallation (contrat installateur §6).
pub const NOM_INSTALLATION: &str = "installation.log";
/// Journal de secours, dans le dossier temporaire du système, si le dossier local manque.
pub const NOM_SECOURS: &str = "breezyshot-secours.log";
pub const TAILLE_MAX: u64 = 1024 * 1024;
pub const NOMBRE_FICHIERS: usize = 5;
/// Après une rotation impossible (fichier tenu par un autre programme), on attend avant de
/// retenter, pour ne pas essayer à chaque ligne.
pub const ATTENTE_ROTATION: Duration = Duration::from_secs(30);

/// Nom de la crate : ses lignes jusqu'au niveau Info ; les autres crates, Warn seulement.
const CRATE: &str = "breezyshot";

struct Sortie {
    fichier: Option<File>,
    taille: u64,
    /// Pas de nouvelle tentative de rotation avant cet instant.
    prochaine_rotation: Option<Instant>,
}

struct Journal {
    dossier: PathBuf,
    nom: &'static str,
    sortie: Mutex<Sortie>,
}

static INSTALLE: AtomicBool = AtomicBool::new(false);

/// Vrai si un journal (normal ou de secours) est installé.
pub fn installe() -> bool {
    INSTALLE.load(Ordering::SeqCst)
}

/// Installe le journal dans `dossier` (créé au besoin). À n'appeler qu'une fois.
pub fn installer(dossier: &Path) -> Result<PathBuf, String> {
    installer_nomme(dossier, NOM_FICHIER)
}

/// Installe le journal de secours `breezyshot-secours.log` dans `dossier`.
pub fn installer_secours(dossier: &Path) -> Result<PathBuf, String> {
    installer_nomme(dossier, NOM_SECOURS)
}

/// Installe le journal de l'installateur `installation.log` dans `dossier`.
pub fn installer_installation(dossier: &Path) -> Result<PathBuf, String> {
    installer_nomme(dossier, NOM_INSTALLATION)
}

fn installer_nomme(dossier: &Path, nom: &'static str) -> Result<PathBuf, String> {
    fs::create_dir_all(dossier).map_err(|e| format!("creation de {}: {e}", dossier.display()))?;
    let (fichier, taille) = ouvrir(dossier, nom)?;
    let journal = Journal {
        dossier: dossier.to_path_buf(),
        nom,
        sortie: Mutex::new(Sortie {
            fichier: Some(fichier),
            taille,
            prochaine_rotation: None,
        }),
    };
    log::set_boxed_logger(Box::new(journal)).map_err(|e| format!("journal deja installe: {e}"))?;
    log::set_max_level(LevelFilter::Info);
    INSTALLE.store(true, Ordering::SeqCst);
    let precedent = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("plantage: {info}");
        precedent(info);
    }));
    Ok(dossier.join(nom))
}

/// Vide les tampons (appelé à l'arrêt).
pub fn vider() {
    log::logger().flush();
}

fn ouvrir(dossier: &Path, nom: &str) -> Result<(File, u64), String> {
    let chemin = dossier.join(nom);
    let fichier = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&chemin)
        .map_err(|e| format!("ouverture de {}: {e}", chemin.display()))?;
    let taille = fichier.metadata().map(|m| m.len()).unwrap_or(0);
    Ok((fichier, taille))
}

/// Chemin du fichier numéro `n` (0 = fichier courant).
pub fn chemin_numero(dossier: &Path, nom: &str, n: usize) -> PathBuf {
    if n == 0 {
        dossier.join(nom)
    } else {
        dossier.join(format!("{nom}.{n}"))
    }
}

/// Décale les fichiers : le plus ancien disparaît, le courant devient `.1`.
/// Le courant est d'abord renommé vers un nom d'attente : si ce renommage échoue (fichier
/// tenu par un autre programme), rien n'est décalé et la fonction rend `false`.
pub fn tourner(dossier: &Path, nom: &str) -> bool {
    let courant = chemin_numero(dossier, nom, 0);
    let attente = dossier.join(format!("{nom}.attente"));
    if fs::rename(&courant, &attente).is_err() {
        return false;
    }
    let dernier = NOMBRE_FICHIERS - 1;
    let _ = fs::remove_file(chemin_numero(dossier, nom, dernier));
    for n in (1..dernier).rev() {
        let de = chemin_numero(dossier, nom, n);
        if de.exists() {
            let _ = fs::rename(&de, chemin_numero(dossier, nom, n + 1));
        }
    }
    let _ = fs::rename(&attente, chemin_numero(dossier, nom, 1));
    true
}

/// Peut-on tenter une rotation maintenant ?
pub fn peut_tenter(maintenant: Instant, prochaine: Option<Instant>) -> bool {
    prochaine.is_none_or(|p| maintenant >= p)
}

/// Faut-il tourner avant d'écrire `ajout` octets dans un fichier de `taille` octets ?
pub fn doit_tourner(taille: u64, ajout: u64) -> bool {
    taille > 0 && taille + ajout > TAILLE_MAX
}

/// Nom du module qui écrit, d'après la cible `log` (`breezyshot::raccourcis::x` → `raccourcis`).
pub fn nom_module(cible: &str) -> &str {
    let mut morceaux = cible.split("::");
    let premier = morceaux.next().unwrap_or(cible);
    if premier == CRATE {
        morceaux.next().unwrap_or("application")
    } else {
        premier
    }
}

/// Une ligne du journal, sans retour à la ligne interne :
/// `2026-11-11T18:40:12.345+01:00 INFO  raccourcis  message`.
pub fn formater_ligne(horodatage: &str, niveau: Level, module: &str, message: &str) -> String {
    let message = message.replace("\r\n", " / ").replace(['\n', '\r'], " / ");
    format!(
        "{horodatage} {:<5} {module:<11} {message}\n",
        niveau.as_str()
    )
}

fn horodatage() -> String {
    chrono::Local::now()
        .format("%Y-%m-%dT%H:%M:%S%.3f%:z")
        .to_string()
}

impl Log for Journal {
    fn enabled(&self, metadata: &Metadata) -> bool {
        if metadata.target().split("::").next() == Some(CRATE) {
            metadata.level() <= Level::Info
        } else {
            metadata.level() <= Level::Warn
        }
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let ligne = formater_ligne(
            &horodatage(),
            record.level(),
            nom_module(record.target()),
            &record.args().to_string(),
        );
        let Ok(mut sortie) = self.sortie.lock() else {
            return;
        };
        let maintenant = Instant::now();
        if doit_tourner(sortie.taille, ligne.len() as u64)
            && peut_tenter(maintenant, sortie.prochaine_rotation)
        {
            sortie.fichier = None;
            let tourne = tourner(&self.dossier, self.nom);
            sortie.prochaine_rotation = (!tourne).then(|| maintenant + ATTENTE_ROTATION);
            match ouvrir(&self.dossier, self.nom) {
                Ok((f, t)) => {
                    sortie.fichier = Some(f);
                    sortie.taille = t;
                }
                Err(e) => eprintln!("journal: {e}"),
            }
        }
        if let Some(f) = sortie.fichier.as_mut() {
            if f.write_all(ligne.as_bytes()).is_ok() {
                sortie.taille += ligne.len() as u64;
            }
        }
    }

    fn flush(&self) {
        if let Ok(mut sortie) = self.sortie.lock() {
            if let Some(f) = sortie.fichier.as_mut() {
                let _ = f.flush();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dossier_vide(nom: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("breezyshot-test-{nom}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn format_de_ligne() {
        let l = formater_ligne(
            "2026-11-11T18:40:12.345+01:00",
            Level::Info,
            "raccourcis",
            "message",
        );
        assert_eq!(
            l,
            "2026-11-11T18:40:12.345+01:00 INFO  raccourcis  message\n"
        );
        let l = formater_ligne("t", Level::Warn, "application", "a\nb");
        assert_eq!(l, "t WARN  application a / b\n");
    }

    #[test]
    fn horodatage_avec_decalage() {
        let h = horodatage();
        // 2026-11-11T18:40:12.345+01:00
        assert_eq!(h.len(), 29, "{h}");
        assert_eq!(&h[10..11], "T");
        assert_eq!(&h[19..20], ".");
    }

    #[test]
    fn noms_de_module() {
        assert_eq!(nom_module("breezyshot::raccourcis::choix"), "raccourcis");
        assert_eq!(nom_module("breezyshot::application"), "application");
        assert_eq!(nom_module("breezyshot"), "application");
        assert_eq!(nom_module("tauri::app"), "tauri");
    }

    #[test]
    fn seuil_de_rotation() {
        assert!(!doit_tourner(0, TAILLE_MAX * 2));
        assert!(!doit_tourner(TAILLE_MAX - 10, 10));
        assert!(doit_tourner(TAILLE_MAX - 10, 11));
    }

    #[test]
    fn rotation_garde_cinq_fichiers() {
        let d = dossier_vide("rotation");
        for tour in 0..7 {
            fs::write(chemin_numero(&d, NOM_FICHIER, 0), format!("tour {tour}")).unwrap();
            assert!(tourner(&d, NOM_FICHIER));
        }
        fs::write(chemin_numero(&d, NOM_FICHIER, 0), "courant").unwrap();
        let mut noms: Vec<String> = fs::read_dir(&d)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        noms.sort();
        assert_eq!(
            noms,
            [
                "breezyshot.log",
                "breezyshot.log.1",
                "breezyshot.log.2",
                "breezyshot.log.3",
                "breezyshot.log.4"
            ]
        );
        assert_eq!(
            fs::read_to_string(chemin_numero(&d, NOM_FICHIER, 1)).unwrap(),
            "tour 6"
        );
        assert_eq!(
            fs::read_to_string(chemin_numero(&d, NOM_FICHIER, 4)).unwrap(),
            "tour 3"
        );
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn rotation_impossible_ne_decale_rien() {
        let d = dossier_vide("rotation-impossible");
        fs::write(chemin_numero(&d, NOM_FICHIER, 0), "courant").unwrap();
        fs::write(chemin_numero(&d, NOM_FICHIER, 1), "ancien").unwrap();
        // Un dossier non vide au nom d'attente empêche le renommage du courant.
        let attente = d.join(format!("{NOM_FICHIER}.attente"));
        fs::create_dir_all(attente.join("bloque")).unwrap();
        assert!(!tourner(&d, NOM_FICHIER));
        assert_eq!(
            fs::read_to_string(chemin_numero(&d, NOM_FICHIER, 0)).unwrap(),
            "courant"
        );
        assert_eq!(
            fs::read_to_string(chemin_numero(&d, NOM_FICHIER, 1)).unwrap(),
            "ancien"
        );
        assert!(!chemin_numero(&d, NOM_FICHIER, 2).exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn nouvelle_tentative_au_plus_toutes_les_30_s() {
        let t = Instant::now();
        assert!(peut_tenter(t, None));
        assert!(!peut_tenter(t, Some(t + ATTENTE_ROTATION)));
        assert!(peut_tenter(
            t + ATTENTE_ROTATION,
            Some(t + ATTENTE_ROTATION)
        ));
    }

    #[test]
    fn ecriture_tourne_au_dela_d_un_mo() {
        let d = dossier_vide("ecriture");
        let (f, t) = ouvrir(&d, NOM_FICHIER).unwrap();
        let journal = Journal {
            dossier: d.clone(),
            nom: NOM_FICHIER,
            sortie: Mutex::new(Sortie {
                fichier: Some(f),
                taille: t,
                prochaine_rotation: None,
            }),
        };
        let long = "x".repeat(1000);
        for _ in 0..1200 {
            journal.log(
                &Record::builder()
                    .args(format_args!("{long}"))
                    .level(Level::Info)
                    .target("breezyshot::application")
                    .build(),
            );
        }
        assert!(chemin_numero(&d, NOM_FICHIER, 1).exists());
        assert!(
            fs::metadata(chemin_numero(&d, NOM_FICHIER, 1))
                .unwrap()
                .len()
                <= TAILLE_MAX
        );
        assert!(
            fs::metadata(chemin_numero(&d, NOM_FICHIER, 0))
                .unwrap()
                .len()
                <= TAILLE_MAX
        );
        // Les lignes des autres crates sous Warn sont écartées.
        assert!(!journal.enabled(
            &Metadata::builder()
                .level(Level::Info)
                .target("tauri::app")
                .build()
        ));
        let _ = fs::remove_dir_all(&d);
    }
}
