//! Journal de l'app : `<local>/Journal/breezyshot.log`, une ligne par événement.
//!
//! Les modules écrivent avec la façade `log` (`log::info!`…) sans connaître l'Application.
//! Rotation : 1 Mo par fichier, 5 fichiers au plus (`breezyshot.log`, `.1` … `.4`).
//! Jamais d'image ni de contenu du presse-papiers ; les chemins de fichiers sont permis.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use log::{Level, LevelFilter, Log, Metadata, Record};

pub const NOM_FICHIER: &str = "breezyshot.log";
pub const TAILLE_MAX: u64 = 1024 * 1024;
pub const NOMBRE_FICHIERS: usize = 5;

/// Nom de la crate : ses lignes jusqu'au niveau Info ; les autres crates, Warn seulement.
const CRATE: &str = "breezyshot";

struct Sortie {
    fichier: Option<File>,
    taille: u64,
}

struct Journal {
    dossier: PathBuf,
    sortie: Mutex<Sortie>,
}

/// Installe le journal dans `dossier` (créé au besoin). À n'appeler qu'une fois.
pub fn installer(dossier: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(dossier).map_err(|e| format!("creation de {}: {e}", dossier.display()))?;
    let (fichier, taille) = ouvrir(dossier)?;
    let journal = Journal {
        dossier: dossier.to_path_buf(),
        sortie: Mutex::new(Sortie {
            fichier: Some(fichier),
            taille,
        }),
    };
    log::set_boxed_logger(Box::new(journal)).map_err(|e| format!("journal deja installe: {e}"))?;
    log::set_max_level(LevelFilter::Info);
    let precedent = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("plantage: {info}");
        precedent(info);
    }));
    Ok(dossier.join(NOM_FICHIER))
}

/// Vide les tampons (appelé à l'arrêt).
pub fn vider() {
    log::logger().flush();
}

fn ouvrir(dossier: &Path) -> Result<(File, u64), String> {
    let chemin = dossier.join(NOM_FICHIER);
    let fichier = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&chemin)
        .map_err(|e| format!("ouverture de {}: {e}", chemin.display()))?;
    let taille = fichier.metadata().map(|m| m.len()).unwrap_or(0);
    Ok((fichier, taille))
}

/// Chemin du fichier numéro `n` (0 = fichier courant).
pub fn chemin_numero(dossier: &Path, n: usize) -> PathBuf {
    if n == 0 {
        dossier.join(NOM_FICHIER)
    } else {
        dossier.join(format!("{NOM_FICHIER}.{n}"))
    }
}

/// Décale les fichiers : le plus ancien disparaît, le courant devient `.1`.
pub fn tourner(dossier: &Path) {
    let dernier = NOMBRE_FICHIERS - 1;
    let _ = fs::remove_file(chemin_numero(dossier, dernier));
    for n in (0..dernier).rev() {
        let de = chemin_numero(dossier, n);
        if de.exists() {
            let _ = fs::rename(&de, chemin_numero(dossier, n + 1));
        }
    }
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
        if doit_tourner(sortie.taille, ligne.len() as u64) {
            sortie.fichier = None;
            tourner(&self.dossier);
            match ouvrir(&self.dossier) {
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
            fs::write(chemin_numero(&d, 0), format!("tour {tour}")).unwrap();
            tourner(&d);
        }
        fs::write(chemin_numero(&d, 0), "courant").unwrap();
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
        assert_eq!(fs::read_to_string(chemin_numero(&d, 1)).unwrap(), "tour 6");
        assert_eq!(fs::read_to_string(chemin_numero(&d, 4)).unwrap(), "tour 3");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn ecriture_tourne_au_dela_d_un_mo() {
        let d = dossier_vide("ecriture");
        let (f, t) = ouvrir(&d).unwrap();
        let journal = Journal {
            dossier: d.clone(),
            sortie: Mutex::new(Sortie {
                fichier: Some(f),
                taille: t,
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
        assert!(chemin_numero(&d, 1).exists());
        assert!(fs::metadata(chemin_numero(&d, 1)).unwrap().len() <= TAILLE_MAX);
        assert!(fs::metadata(chemin_numero(&d, 0)).unwrap().len() <= TAILLE_MAX);
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
