//! Le texte de la licence, gardé tel que collé dans `<dossier des réglages>/licence.txt`
//! (avenant 0.5 « Licence », §3.1), écrit de façon atomique comme les réglages : fichier
//! temporaire vidé sur le disque, puis renommé. Ce module ne lit ni ne vérifie la licence.

use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

use crate::systeme;

const NOM_FICHIER: &str = "licence.txt";
const NOM_TEMPORAIRE: &str = "licence.txt.tmp";

/// Taille lue au plus : un fichier énorme n'est jamais lu en entier (la licence en fait
/// au plus 1 000 caractères ; ce qui dépasse sera refusé par la vérification).
const TAILLE_MAX: u64 = 64 * 1024;

/// Le texte rangé, `None` s'il n'y en a pas ou s'il est illisible.
pub fn lire_licence() -> Option<String> {
    lire_dans(&systeme::dossiers().ok()?.reglages)
}

/// Range `texte` (remplace l'ancien).
pub fn ecrire_licence(texte: &str) -> Result<(), String> {
    ecrire_dans(&systeme::dossiers()?.reglages, texte)
}

/// Efface le texte rangé (absent : pas une erreur).
pub fn effacer_licence() -> Result<(), String> {
    effacer_dans(&systeme::dossiers()?.reglages)
}

fn lire_dans(dossier: &Path) -> Option<String> {
    let mut octets = Vec::new();
    fs::File::open(dossier.join(NOM_FICHIER))
        .ok()?
        .take(TAILLE_MAX)
        .read_to_end(&mut octets)
        .ok()?;
    String::from_utf8(octets).ok()
}

fn ecrire_dans(dossier: &Path, texte: &str) -> Result<(), String> {
    fs::create_dir_all(dossier).map_err(|e| format!("{}: {e}", dossier.display()))?;
    let cible = dossier.join(NOM_FICHIER);
    let temporaire = dossier.join(NOM_TEMPORAIRE);
    let _ = fs::remove_file(&temporaire);
    let resultat = ecrire_temporaire(&temporaire, texte.as_bytes())
        .map_err(|e| format!("{}: {e}", temporaire.display()))
        .and_then(|()| {
            fs::rename(&temporaire, &cible).map_err(|e| format!("{}: {e}", cible.display()))
        });
    if resultat.is_err() {
        let _ = fs::remove_file(&temporaire);
    }
    resultat
}

fn ecrire_temporaire(chemin: &Path, octets: &[u8]) -> io::Result<()> {
    let mut f = fs::File::create(chemin)?;
    f.write_all(octets)?;
    f.sync_all()
}

fn effacer_dans(dossier: &Path) -> Result<(), String> {
    let cible = dossier.join(NOM_FICHIER);
    match fs::remove_file(&cible) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("{}: {e}", cible.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecrire_lire_effacer() {
        let d = tempfile::tempdir().unwrap();
        let dossier = d.path().join("reglages");
        assert_eq!(lire_dans(&dossier), None);
        assert_eq!(effacer_dans(&dossier), Ok(()));
        // Gardé tel que collé, blancs compris ; le dossier est créé au besoin.
        ecrire_dans(&dossier, "  BS1.a.b\r\n").unwrap();
        assert_eq!(lire_dans(&dossier).as_deref(), Some("  BS1.a.b\r\n"));
        ecrire_dans(&dossier, "BS1.c.d").unwrap();
        assert_eq!(lire_dans(&dossier).as_deref(), Some("BS1.c.d"));
        assert!(!dossier.join(NOM_TEMPORAIRE).exists());
        assert_eq!(effacer_dans(&dossier), Ok(()));
        assert!(!dossier.join(NOM_FICHIER).exists());
        assert_eq!(lire_dans(&dossier), None);
    }

    #[test]
    fn un_vieux_temporaire_ne_gene_pas() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join(NOM_TEMPORAIRE), "reste").unwrap();
        ecrire_dans(d.path(), "BS1.x.y").unwrap();
        assert_eq!(lire_dans(d.path()).as_deref(), Some("BS1.x.y"));
        assert!(!d.path().join(NOM_TEMPORAIRE).exists());
    }

    #[test]
    fn echec_laisse_l_ancien_intact() {
        let d = tempfile::tempdir().unwrap();
        ecrire_dans(d.path(), "ancien").unwrap();
        // Le temporaire ne peut pas être créé : c'est un dossier non vide.
        let bloque = d.path().join(NOM_TEMPORAIRE);
        fs::create_dir(&bloque).unwrap();
        fs::write(bloque.join("f"), "x").unwrap();
        assert!(ecrire_dans(d.path(), "nouveau").is_err());
        assert_eq!(lire_dans(d.path()).as_deref(), Some("ancien"));
    }

    #[test]
    fn illisible_ou_enorme() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join(NOM_FICHIER), [0xffu8, 0xfe, b'B']).unwrap();
        assert_eq!(lire_dans(d.path()), None);
        let enorme = "A".repeat(TAILLE_MAX as usize * 2);
        fs::write(d.path().join(NOM_FICHIER), &enorme).unwrap();
        assert_eq!(
            lire_dans(d.path()).map(|t| t.len()),
            Some(TAILLE_MAX as usize)
        );
        // Un dossier à la place du fichier : illisible, pas de panique.
        let autre = tempfile::tempdir().unwrap();
        fs::create_dir(autre.path().join(NOM_FICHIER)).unwrap();
        assert_eq!(lire_dans(autre.path()), None);
        assert!(effacer_dans(autre.path()).is_err());
    }
}
