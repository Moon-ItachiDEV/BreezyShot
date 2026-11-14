//! Ce que la vignette donne a une autre application (le fichier glisse) et ce qu'elle
//! efface (Supprimer). Seuls fichiers touches : ceux que l'app a ecrits pour la capture.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::fichier::existe;
use super::film::{cle_film, SorteFilm};
use super::nom::chemin_libre_ext;
use super::recents::{cle_recent, RECENTS_GARDES};

/// Nombre de fichiers gardes dans `<local>/Glisser` (borne comme « Recents »).
pub const GLISSER_GARDES: usize = RECENTS_GARDES;

/// Les fichiers d'une capture que Supprimer efface : son fichier de « Recents », le
/// fichier enregistre pour elle (chemin rendu par son bilan) et ses fichiers de glisser.
/// Pour un film (0.7, F25) : son fichier de travail dans `<local>/Films`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Suppression {
    pub recent: Option<PathBuf>,
    pub enregistre: Option<PathBuf>,
    pub liens: Vec<PathBuf>,
    /// Le fichier de travail d'un film (`None` pour une image).
    pub film: Option<PathBuf>,
}

/// Le fichier a donner au glisser : `<dossier>/<nom>.png` (« (2) » si besoin), lien dur
/// vers le fichier de « Recents » (aucune donnee copiee) ; si le lien est impossible, une
/// copie ; si la copie l'est aussi, le fichier de « Recents » lui-meme. Le dossier garde au
/// plus `GLISSER_GARDES` fichiers `.png`, jamais celui qu'on vient d'y mettre.
///
/// 0.7 (F23) : l'extension est celle du fichier donne (`.mp4` ou `.gif` pour un film, `.png`
/// sans extension) ; les liens de films ne sont pas comptes ici, ils suivent
/// `films_a_retirer` (`nettoyer_films`).
pub fn fichier_a_glisser(recent: &Path, dossier: &Path, nom: &str) -> Result<PathBuf, String> {
    if !recent.is_file() {
        return Err("le fichier de la capture est introuvable".into());
    }
    if fs::create_dir_all(dossier).is_err() {
        return Ok(recent.to_path_buf());
    }
    let extension = recent
        .extension()
        .and_then(|e| e.to_str())
        .filter(|e| !e.is_empty())
        .unwrap_or("png");
    let chemin = chemin_libre_ext(dossier, nom, extension, existe);
    let fait = fs::hard_link(recent, &chemin).or_else(|_| fs::copy(recent, &chemin).map(|_| ()));
    if fait.is_err() {
        return Ok(recent.to_path_buf());
    }
    nettoyer(dossier, &chemin);
    Ok(chemin)
}

fn nettoyer(dossier: &Path, neuf: &Path) {
    let Some(nom_neuf) = neuf.file_name().and_then(|n| n.to_str()) else {
        return;
    };
    let Ok(entrees) = fs::read_dir(dossier) else {
        return;
    };
    let liste: Vec<(String, SystemTime)> = entrees
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| {
            let date = e.metadata().and_then(|m| m.modified()).ok()?;
            Some((e.file_name().into_string().ok()?, date))
        })
        .collect();
    for nom in liens_a_supprimer(&liste, nom_neuf, GLISSER_GARDES) {
        let _ = fs::remove_file(dossier.join(nom));
    }
}

/// Les noms a supprimer pour ne garder que les `garder` fichiers `.png` les plus recents
/// (date de modification), jamais `neuf`. Les autres fichiers sont ignores.
pub fn liens_a_supprimer(
    entrees: &[(String, SystemTime)],
    neuf: &str,
    garder: usize,
) -> Vec<String> {
    let mut png: Vec<&(String, SystemTime)> = entrees
        .iter()
        .filter(|(n, _)| n != neuf && est_png(Path::new(n)))
        .collect();
    png.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.0.cmp(&a.0)));
    png.into_iter()
        .skip(garder.saturating_sub(1))
        .map(|(n, _)| n.clone())
        .collect()
}

fn a_l_extension(chemin: &Path, voulue: &str) -> bool {
    chemin
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(voulue))
}

fn est_png(chemin: &Path) -> bool {
    a_l_extension(chemin, "png")
}

/// Vrai pour un fichier que Supprimer peut effacer hors des dossiers de l'app : une image
/// `.png`, ou un film `.mp4` ou `.gif` (0.7).
fn est_capture(chemin: &Path) -> bool {
    est_png(chemin) || est_film(chemin)
}

/// Vrai pour un fichier de film : `.mp4` ou `.gif`.
pub(super) fn est_film(chemin: &Path) -> bool {
    [SorteFilm::Video, SorteFilm::Gif]
        .iter()
        .any(|s| a_l_extension(chemin, s.extension()))
}

/// Ce que Supprimer efface (fonction pure) : le fichier de « Recents » s'il est dans
/// `dossier_recents` et au format exact ; le fichier enregistre pour cette capture s'il
/// est un chemin absolu `.png` ; les fichiers de glisser s'ils sont dans `dossier_glisser`
/// et en `.png`. Rien d'autre, sans doublon.
///
/// Pour un film (0.7, F25) : son fichier de travail s'il est dans `dossier_films` et au
/// format exact `AAAAMMJJ-HHMMSS-mmm.mp4` ou `.gif` (`cle_film`) ; le fichier enregistre
/// s'il est un chemin absolu `.mp4` ou `.gif` ; les liens de `dossier_glisser` en `.mp4` ou
/// `.gif`.
pub fn a_effacer(
    s: &Suppression,
    dossier_recents: &Path,
    dossier_glisser: &Path,
    dossier_films: &Path,
) -> Vec<PathBuf> {
    let mut liste: Vec<PathBuf> = Vec::new();
    let mut ajouter = |p: &Path| {
        if !liste.iter().any(|q| q == p) {
            liste.push(p.to_path_buf());
        }
    };
    if let Some(r) = &s.recent {
        let au_format = r
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| cle_recent(n).is_some());
        if !dossier_recents.as_os_str().is_empty()
            && r.parent() == Some(dossier_recents)
            && au_format
        {
            ajouter(r);
        }
    }
    if let Some(f) = &s.film {
        let au_format = f
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| cle_film(n).is_some());
        if !dossier_films.as_os_str().is_empty() && f.parent() == Some(dossier_films) && au_format {
            ajouter(f);
        }
    }
    if let Some(e) = &s.enregistre {
        if e.is_absolute() && est_capture(e) {
            ajouter(e);
        }
    }
    for l in &s.liens {
        if !dossier_glisser.as_os_str().is_empty()
            && l.parent() == Some(dossier_glisser)
            && est_capture(l)
        {
            ajouter(l);
        }
    }
    liste
}

/// Efface les fichiers de `a_effacer`. Un fichier deja absent compte comme efface. Rend,
/// pour chacun, son chemin et le resultat.
pub fn supprimer(
    s: &Suppression,
    dossier_recents: &Path,
    dossier_glisser: &Path,
    dossier_films: &Path,
) -> Vec<(PathBuf, Result<(), String>)> {
    a_effacer(s, dossier_recents, dossier_glisser, dossier_films)
        .into_iter()
        .map(|p| {
            let r = match fs::remove_file(&p) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(cause(&e)),
            };
            (p, r)
        })
        .collect()
}

fn cause(e: &std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::PermissionDenied => "Le fichier est protégé ou ouvert.".into(),
        _ => "Le fichier n'a pas pu être effacé.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn t(s: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(s)
    }

    #[test]
    fn dossier_de_glisser_borne() {
        let e: Vec<(String, SystemTime)> = vec![
            ("a.png".into(), t(1)),
            ("b.png".into(), t(3)),
            ("c.png".into(), t(2)),
            ("note.txt".into(), t(0)),
            ("neuf.png".into(), t(0)),
        ];
        // On garde 2 : le neuf et le plus récent des autres (b).
        assert_eq!(liens_a_supprimer(&e, "neuf.png", 2), ["c.png", "a.png"]);
        assert!(liens_a_supprimer(&e, "neuf.png", 10).is_empty());
        // Jamais le neuf, jamais un autre type de fichier.
        let tout = liens_a_supprimer(&e, "neuf.png", 0);
        assert!(!tout.contains(&"neuf.png".to_string()));
        assert!(!tout.contains(&"note.txt".to_string()));
    }

    #[test]
    fn seuls_les_fichiers_de_la_capture_s_effacent() {
        let rec = Path::new("/l/Recents");
        let gli = Path::new("/l/Glisser");
        let fil = Path::new("/l/Films");
        // Absolu sur tout système (sous Windows, « /i/… » sans lettre de lecteur ne l'est pas).
        let enregistre = std::env::temp_dir()
            .join("BreezyShot")
            .join("BreezyShot 2026-11-11 à 19.40.12.png");
        assert!(enregistre.is_absolute());
        let s = Suppression {
            recent: Some(rec.join("20261111-194012-123.png")),
            enregistre: Some(enregistre.clone()),
            liens: vec![
                gli.join("BreezyShot 2026-11-11 à 19.40.12.png"),
                PathBuf::from("/ailleurs/x.png"),
                gli.join("x.txt"),
            ],
            film: None,
        };
        assert_eq!(
            a_effacer(&s, rec, gli, fil),
            [
                rec.join("20261111-194012-123.png"),
                enregistre,
                gli.join("BreezyShot 2026-11-11 à 19.40.12.png"),
            ]
        );
        // Un « Récents » hors format ou hors dossier n'est pas effacé.
        let s = Suppression {
            recent: Some(rec.join("photo.png")),
            enregistre: Some(PathBuf::from("relatif.png")),
            liens: vec![],
            film: None,
        };
        assert!(a_effacer(&s, rec, gli, fil).is_empty());
        let s = Suppression {
            recent: Some(PathBuf::from("/autre/20261111-194012-123.png")),
            enregistre: Some(PathBuf::from("/i/doc.txt")),
            liens: vec![],
            film: None,
        };
        assert!(a_effacer(&s, rec, gli, fil).is_empty());
        // Le repli du glisser (le « Récents » lui-même) n'est compté qu'une fois.
        let r = rec.join("20261111-194012-123.png");
        let s = Suppression {
            recent: Some(r.clone()),
            enregistre: None,
            liens: vec![r.clone()],
            film: None,
        };
        assert_eq!(a_effacer(&s, rec, gli, fil), [r]);
        assert!(a_effacer(&Suppression::default(), rec, gli, fil).is_empty());
    }

    #[test]
    fn glisser_puis_supprimer_sur_le_disque() {
        let d = tempfile::tempdir().expect("dossier");
        let rec = d.path().join("Recents");
        let gli = d.path().join("Glisser");
        fs::create_dir_all(&rec).expect("recents");
        let recent = rec.join("20261111-194012-123.png");
        fs::write(&recent, b"png").expect("ecriture");
        let a = fichier_a_glisser(&recent, &gli, "BreezyShot 2026-11-11 à 19.40.12").expect("a");
        assert_eq!(a, gli.join("BreezyShot 2026-11-11 à 19.40.12.png"));
        assert_eq!(fs::read(&a).expect("lu"), b"png");
        let b = fichier_a_glisser(&recent, &gli, "BreezyShot 2026-11-11 à 19.40.12").expect("b");
        assert_eq!(b, gli.join("BreezyShot 2026-11-11 à 19.40.12 (2).png"));
        let autre = d.path().join("autre.png");
        fs::write(&autre, b"x").expect("ecriture");
        let s = Suppression {
            recent: Some(recent.clone()),
            enregistre: None,
            liens: vec![a.clone(), b.clone()],
            film: None,
        };
        let r = supprimer(&s, &rec, &gli, &d.path().join("Films"));
        assert_eq!(r.len(), 3);
        assert!(r.iter().all(|(_, r)| r.is_ok()));
        assert!(!recent.exists() && !a.exists() && !b.exists());
        assert!(autre.exists());
        // Déjà absent : compté comme effacé.
        assert!(supprimer(&s, &rec, &gli, &d.path().join("Films"))
            .iter()
            .all(|(_, r)| r.is_ok()));
        assert!(fichier_a_glisser(&recent, &gli, "x").is_err());
    }
}
