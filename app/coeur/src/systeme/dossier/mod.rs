//! Le sélecteur de dossier du système (« Parcourir… » de la fenêtre Réglages, étape 10) et
//! le dialogue « Enregistrer sous » (éditeur, 0.4).
//!
//! `choisir_dossier` et `choisir_fichier_png` bloquent jusqu'à la réponse de l'utilisateur :
//! l'appelant les lance dans un fil à part, jamais sur le fil principal (un raccourci de
//! capture ne doit pas tourner dans la boucle du dialogue). Windows : `IFileOpenDialog` en
//! mode dossier, `IFileSaveDialog` pour le PNG, modaux à la fenêtre donnée. Hors Windows :
//! « non disponible » pour le dossier, `Ok(None)` (annulé) pour le PNG.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[cfg(windows)]
#[path = "windows.rs"]
mod plateforme;

#[cfg(not(windows))]
#[path = "autre.rs"]
mod plateforme;

/// Montre le sélecteur de dossier. `proprietaire` : la fenêtre qu'il bloque (HWND), ou
/// rien. `depart` : le dossier montré d'abord, s'il existe. `Ok(None)` : annulé.
pub fn choisir_dossier(
    proprietaire: Option<isize>,
    depart: Option<PathBuf>,
) -> Result<Option<PathBuf>, String> {
    plateforme::choisir_dossier(proprietaire, depart)
}

/// Montre le dialogue « Enregistrer sous » du système pour un PNG : modal à `proprietaire`,
/// ouvert sur `depart` s'il existe, nom proposé `nom`, filtre PNG, extension `.png`
/// imposée, remplacement confirmé par le système. `Ok(None)` : annulé.
pub fn choisir_fichier_png(
    proprietaire: Option<isize>,
    depart: Option<&Path>,
    nom: &str,
) -> Result<Option<PathBuf>, String> {
    plateforme::choisir_fichier_png(proprietaire, depart, nom)
}

/// Le dialogue « Enregistrer sous » pour un fichier `.breezy` (0.7.7) : comme
/// `choisir_fichier_png`, filtre « Fichier BreezyShot », extension `.breezy` imposee.
pub fn choisir_fichier_breezy(
    proprietaire: Option<isize>,
    depart: Option<&Path>,
    nom: &str,
) -> Result<Option<PathBuf>, String> {
    plateforme::choisir_fichier_breezy(proprietaire, depart, nom)
}

// ---------- Ce qui se decide sans ouvrir le dialogue (teste partout, servi a Windows) ----------

/// Le seul filtre du dialogue « Enregistrer sous » : son nom montre et son motif.
#[cfg_attr(not(windows), allow(dead_code))]
const FILTRE_PNG: (&str, &str) = ("Image PNG", "*.png");

/// Le filtre du dialogue pour un fichier `.breezy`.
#[cfg_attr(not(windows), allow(dead_code))]
const FILTRE_BREEZY: (&str, &str) = ("Fichier BreezyShot", "*.breezy");

/// L'extension d'un fichier `.breezy`, sans point.
#[cfg_attr(not(windows), allow(dead_code))]
const EXTENSION_BREEZY: &str = "breezy";

/// L'extension imposee, sans point (`SetDefaultExtension`).
#[cfg_attr(not(windows), allow(dead_code))]
const EXTENSION_PNG: &str = "png";

/// Texte en UTF-16 termine par un zero, pour un `PCWSTR` qui vit autant que le tableau.
#[cfg_attr(not(windows), allow(dead_code))]
fn large(texte: &str) -> Vec<u16> {
    texte.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Le nom propose dans le dialogue : le dernier element de `nom` (jamais un dossier),
/// sans blancs autour, avec `.png` s'il ne l'a pas deja (casse ignoree). `None` : rien a
/// proposer, le dialogue garde son champ vide.
#[cfg_attr(not(test), allow(dead_code))]
fn nom_propose(nom: &str) -> Option<String> {
    nom_propose_avec(nom, EXTENSION_PNG)
}

/// `nom_propose` pour l'extension `extension` (sans point).
#[cfg_attr(not(windows), allow(dead_code))]
fn nom_propose_avec(nom: &str, extension: &str) -> Option<String> {
    let dernier = nom.rsplit(['/', '\\']).next().unwrap_or("").trim();
    if dernier.is_empty() || dernier.chars().all(|c| c == '.') {
        return None;
    }
    if a_extension(Path::new(dernier), extension) {
        Some(dernier.to_string())
    } else {
        Some(format!("{dernier}.{extension}"))
    }
}

/// Vrai si le chemin finit par `.extension`, quelle que soit la casse.
#[cfg_attr(not(windows), allow(dead_code))]
fn a_extension(chemin: &Path, extension: &str) -> bool {
    chemin
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(extension))
}

/// Le chemin rendu par le dialogue, avec `.png` impose. Le systeme l'ajoute deja (extension
/// par defaut, types stricts) ; s'il manquait quand meme, il est ajoute ici, sauf si le nom
/// ainsi forme existe deja : son remplacement n'a pas ete confirme, on refuse.
#[cfg_attr(not(test), allow(dead_code))]
fn chemin_png(choisi: PathBuf, existe: impl Fn(&Path) -> bool) -> Result<PathBuf, String> {
    chemin_avec_extension(choisi, EXTENSION_PNG, existe)
}

/// `chemin_png` pour l'extension `extension` (sans point).
#[cfg_attr(not(windows), allow(dead_code))]
fn chemin_avec_extension(
    choisi: PathBuf,
    extension: &str,
    existe: impl Fn(&Path) -> bool,
) -> Result<PathBuf, String> {
    if a_extension(&choisi, extension) {
        return Ok(choisi);
    }
    let mut texte = OsString::from(choisi);
    texte.push(".");
    texte.push(extension);
    let chemin = PathBuf::from(texte);
    if existe(&chemin) {
        Err("extension: remplacement non confirme".into())
    } else {
        Ok(chemin)
    }
}

/// Le chemin d'un texte UTF-16 du systeme, lu jusqu'au premier zero. Un texte vide ou qui
/// n'est pas de l'Unicode valide est refuse (la cause ne contient jamais le texte).
#[cfg_attr(not(windows), allow(dead_code))]
fn chemin_depuis_large(texte: &[u16]) -> Result<PathBuf, String> {
    let fin = texte.iter().position(|&c| c == 0).unwrap_or(texte.len());
    let texte = String::from_utf16(&texte[..fin]).map_err(|_| "chemin: non unicode")?;
    if texte.is_empty() {
        return Err("chemin: vide".into());
    }
    Ok(PathBuf::from(texte))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filtre_png_exact() {
        assert_eq!(
            large(FILTRE_PNG.0),
            [73, 109, 97, 103, 101, 32, 80, 78, 71, 0]
        );
        assert_eq!(large(FILTRE_PNG.1), [42, 46, 112, 110, 103, 0]);
        assert_eq!(large(EXTENSION_PNG), [112, 110, 103, 0]);
        assert_eq!(large(""), [0]);
        // Hors du plan de base : deux mots de 16 bits, un seul zero final.
        assert_eq!(large("é😀"), [0xE9, 0xD83D, 0xDE00, 0]);
    }

    #[test]
    fn nom_propose_avec_png() {
        let cas = [
            (
                "BreezyShot 2026-11-12 à 10.36.00",
                "BreezyShot 2026-11-12 à 10.36.00.png",
            ),
            ("capture.png", "capture.png"),
            ("capture.PNG", "capture.PNG"),
            ("  capture  ", "capture.png"),
            ("capture.jpg", "capture.jpg.png"),
            ("v1.2", "v1.2.png"),
            (".png", ".png.png"),
            ("C:\\Users\\x\\capture", "capture.png"),
            ("dossier/capture.png", "capture.png"),
        ];
        for (nom, attendu) in cas {
            assert_eq!(nom_propose(nom).as_deref(), Some(attendu), "{nom:?}");
        }
        for nom in ["", "   ", "dossier/", "C:\\x\\", ".", ".."] {
            assert_eq!(nom_propose(nom), None, "{nom:?}");
        }
    }

    #[test]
    fn chemin_png_impose_l_extension() {
        let jamais = |_: &Path| false;
        let toujours = |_: &Path| true;
        let d = Path::new("/d");
        assert_eq!(chemin_png(d.join("a.png"), toujours), Ok(d.join("a.png")));
        assert_eq!(chemin_png(d.join("a.Png"), toujours), Ok(d.join("a.Png")));
        assert_eq!(chemin_png(d.join("a"), jamais), Ok(d.join("a.png")));
        assert_eq!(chemin_png(d.join("a.jpg"), jamais), Ok(d.join("a.jpg.png")));
        assert_eq!(
            chemin_png(d.join("a. png"), jamais),
            Ok(d.join("a. png.png"))
        );
        // Le nom forme existe : son remplacement n'a pas ete confirme.
        let refus = chemin_png(d.join("a"), toujours).expect_err("refus");
        assert!(!refus.contains("/d"), "{refus}");
        assert_eq!(
            chemin_png(d.join("a"), |c| c == d.join("a")),
            Ok(d.join("a.png"))
        );
    }

    #[test]
    fn chemin_png_nom_fait_de_l_extension() {
        // Comportement actuel : « .png » n'a pas d'extension pour `Path` (fichier cache),
        // `.png` est donc ajoute, comme pour tout nom sans extension.
        let d = Path::new("/d");
        assert_eq!(
            chemin_png(d.join(".png"), |_| false),
            Ok(d.join(".png.png"))
        );
        assert!(chemin_png(d.join(".png"), |_| true).is_err());
    }

    #[test]
    fn chemin_depuis_large_s_arrete_au_zero() {
        let texte: Vec<u16> = "C:\\Images\\é.png"
            .encode_utf16()
            .chain([0, 65, 66])
            .collect();
        assert_eq!(
            chemin_depuis_large(&texte),
            Ok(PathBuf::from("C:\\Images\\é.png"))
        );
        let sans_zero: Vec<u16> = "a.png".encode_utf16().collect();
        assert_eq!(chemin_depuis_large(&sans_zero), Ok(PathBuf::from("a.png")));
        assert_eq!(chemin_depuis_large(&[0, 65]), Err("chemin: vide".into()));
        assert_eq!(chemin_depuis_large(&[]), Err("chemin: vide".into()));
        // Demi-paire isolee : refusee, sans le texte dans la cause.
        assert_eq!(
            chemin_depuis_large(&[67, 0xD800, 0]),
            Err("chemin: non unicode".into())
        );
    }

    #[test]
    fn fichier_breezy_nom_extension_et_filtre() {
        assert_eq!(
            large(FILTRE_BREEZY.0),
            "Fichier BreezyShot\0".encode_utf16().collect::<Vec<_>>()
        );
        assert_eq!(FILTRE_BREEZY.1, "*.breezy");
        assert_eq!(
            nom_propose_avec("Capture", EXTENSION_BREEZY).as_deref(),
            Some("Capture.breezy")
        );
        assert_eq!(
            nom_propose_avec(r"C:\x\Capture.BREEZY", EXTENSION_BREEZY).as_deref(),
            Some("Capture.BREEZY")
        );
        assert_eq!(nom_propose_avec("...", EXTENSION_BREEZY), None);
        let chemin = chemin_avec_extension("/d/a".into(), EXTENSION_BREEZY, |_| false);
        assert_eq!(chemin, Ok(PathBuf::from("/d/a.breezy")));
        assert!(chemin_avec_extension("/d/a".into(), EXTENSION_BREEZY, |_| true).is_err());
    }
}
