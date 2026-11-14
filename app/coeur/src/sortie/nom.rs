//! Noms et chemins des fichiers enregistres (fonctions pures).

use std::path::{Component, Path, PathBuf};

use chrono::{DateTime, Local};

/// Longueur maximale d'un nom, en caracteres, sans l'extension.
const LONGUEUR_MAX: usize = 120;

/// Longueur maximale d'un nom, en octets UTF-8, sans l'extension. Linux et
/// macOS limitent un nom a 255 octets ; il reste la place de « (n).png ».
const OCTETS_MAX: usize = 200;

/// Nombre maximal de suffixes « (n) » essayes avant d'abandonner.
const ESSAIS_MAX: u32 = 10_000;

/// Caracteres interdits dans un nom de fichier Windows (en plus des
/// caracteres de controle).
const INTERDITS: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

/// Noms reserves par Windows, quelle que soit l'extension.
const RESERVES: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];

/// Fabrique le nom du fichier (sans extension) a partir du modele.
///
/// `{date}` devient `2026-11-11`, `{heure}` devient `19.40.12`.
pub fn nom_fichier(modele: &str, maintenant: DateTime<Local>) -> String {
    let date = maintenant.format("%Y-%m-%d").to_string();
    let heure = maintenant.format("%H.%M.%S").to_string();

    let rempli = modele.replace("{date}", &date).replace("{heure}", &heure);

    let propre: String = rempli
        .chars()
        .map(|c| {
            if c.is_control() || INTERDITS.contains(&c) {
                '-'
            } else {
                c
            }
        })
        .take(LONGUEUR_MAX)
        .collect();

    let propre = sans_fin_interdite(coupe_octets(&propre, OCTETS_MAX));

    if propre.trim().is_empty() || est_reserve(propre) {
        format!("BreezyShot {date} {heure}")
    } else {
        propre.to_string()
    }
}

/// Coupe a `max` octets au plus, sur une frontiere de caractere.
fn coupe_octets(texte: &str, max: usize) -> &str {
    if texte.len() <= max {
        return texte;
    }
    let mut fin = max;
    while !texte.is_char_boundary(fin) {
        fin -= 1;
    }
    texte.get(..fin).unwrap_or("")
}

/// Retire les espaces et les points de la fin (Windows les supprime).
fn sans_fin_interdite(nom: &str) -> &str {
    nom.trim_end_matches([' ', '.'])
}

/// Vrai si le nom est reserve sous Windows (`CON`, `COM1`, `LPT1`…), meme
/// suivi d'une extension.
fn est_reserve(nom: &str) -> bool {
    let tige = nom.split('.').next().unwrap_or(nom).trim_end_matches(' ');
    let tige = tige.to_ascii_uppercase();
    if RESERVES.contains(&tige.as_str()) {
        return true;
    }
    for prefixe in ["COM", "LPT"] {
        if let Some(reste) = tige.strip_prefix(prefixe) {
            let mut chiffres = reste.chars();
            if let (Some(c), None) = (chiffres.next(), chiffres.next()) {
                if c.is_ascii_digit() || matches!(c, '¹' | '²' | '³') {
                    return true;
                }
            }
        }
    }
    false
}

/// Premier chemin libre : `nom.png`, puis `nom (2).png`, `nom (3).png`…
pub fn chemin_libre(dossier: &Path, nom: &str, existe: impl Fn(&Path) -> bool) -> PathBuf {
    chemin_libre_ext(dossier, nom, "png", existe)
}

/// Premier chemin libre avec l'extension donnee (0.7 : `mp4`, `gif`) : `nom.<ext>`, puis
/// `nom (2).<ext>`, `nom (3).<ext>`…
pub fn chemin_libre_ext(
    dossier: &Path,
    nom: &str,
    extension: &str,
    existe: impl Fn(&Path) -> bool,
) -> PathBuf {
    let mut chemin = dossier.join(format!("{nom}.{extension}"));
    let mut n = 2;
    while existe(&chemin) && n <= ESSAIS_MAX {
        chemin = dossier.join(format!("{nom} ({n}).{extension}"));
        n += 1;
    }
    chemin
}

/// Nom court d'un dossier pour l'utilisateur : ses deux derniers elements,
/// separes par « › » (ex. « Images › BreezyShot »).
pub fn nom_affiche(dossier: &Path) -> String {
    let parties: Vec<String> = dossier
        .components()
        .filter_map(|c| match c {
            Component::Normal(p) => Some(p.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    match parties.as_slice() {
        [] => dossier.display().to_string(),
        [seul] => seul.clone(),
        [.., avant, dernier] => format!("{avant} › {dernier}"),
    }
}
