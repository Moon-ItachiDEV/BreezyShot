//! Le texte de la licence, gardé nettoyé (blancs des deux bouts retirés, tel que la
//! vérification l'a accepté) dans `<dossier des réglages>/licence.txt` (avenant 0.5
//! « Licence », §3.1), écrit de façon atomique comme les réglages : fichier temporaire vidé
//! sur le disque, puis renommé. Ce module ne lit ni ne vérifie la licence : il range le texte
//! qu'on lui donne, tel quel.

use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

use crate::systeme;

const NOM_FICHIER: &str = "licence.txt";
const NOM_TEMPORAIRE: &str = "licence.txt.tmp";

/// Taille lue au plus : un fichier énorme n'est jamais lu en entier (la licence en fait
/// au plus 1 000 caractères ; ce qui dépasse sera refusé par la vérification).
const TAILLE_MAX: u64 = 64 * 1024;

/// Le texte rangé, `None` s'il n'y en a pas ou s'il est illisible. Illisible (UTF-8
/// invalide, dossier à la place du fichier, accès refusé) : `licence illisible` au journal,
/// le fichier reste.
pub fn lire_licence() -> Option<String> {
    let lu = systeme::dossiers().and_then(|d| lire_dans(&d.reglages));
    match lu {
        Ok(texte) => texte,
        Err(_) => {
            log::warn!("licence illisible");
            None
        }
    }
}

/// Range `texte` (remplace l'ancien).
pub fn ecrire_licence(texte: &str) -> Result<(), String> {
    ecrire_dans(&systeme::dossiers()?.reglages, texte)
}

/// Efface le texte rangé (absent : pas une erreur).
pub fn effacer_licence() -> Result<(), String> {
    effacer_dans(&systeme::dossiers()?.reglages)
}

/// `Ok(None)` : pas de fichier. `Err` : il y a quelque chose, mais illisible. L'erreur ne
/// contient jamais le texte lu.
fn lire_dans(dossier: &Path) -> Result<Option<String>, String> {
    let chemin = dossier.join(NOM_FICHIER);
    let fichier = match fs::File::open(&chemin) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{:?}", e.kind())),
    };
    let mut octets = Vec::new();
    fichier
        .take(TAILLE_MAX)
        .read_to_end(&mut octets)
        .map_err(|e| format!("{:?}", e.kind()))?;
    String::from_utf8(octets)
        .map(Some)
        .map_err(|_| format!("{:?}", io::ErrorKind::InvalidData))
}

fn ecrire_dans(dossier: &Path, texte: &str) -> Result<(), String> {
    fs::create_dir_all(dossier).map_err(|e| format!("{:?}", e.kind()))?;
    let cible = dossier.join(NOM_FICHIER);
    let temporaire = dossier.join(NOM_TEMPORAIRE);
    let _ = fs::remove_file(&temporaire);
    let resultat = ecrire_temporaire(&temporaire, texte.as_bytes())
        .map_err(|e| format!("{:?}", e.kind()))
        .and_then(|()| fs::rename(&temporaire, &cible).map_err(|e| format!("{:?}", e.kind())));
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
        Err(e) => Err(format!("{:?}", e.kind())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecrire_lire_effacer() {
        let d = tempfile::tempdir().unwrap();
        let dossier = d.path().join("reglages");
        assert_eq!(lire_dans(&dossier), Ok(None));
        assert_eq!(effacer_dans(&dossier), Ok(()));
        // Rangé tel que donné (le nettoyage est fait avant) ; le dossier est créé au besoin.
        ecrire_dans(&dossier, "  BS1.a.b\r\n").unwrap();
        assert_eq!(lire_dans(&dossier), Ok(Some("  BS1.a.b\r\n".into())));
        ecrire_dans(&dossier, "BS1.c.d").unwrap();
        assert_eq!(lire_dans(&dossier), Ok(Some("BS1.c.d".into())));
        assert!(!dossier.join(NOM_TEMPORAIRE).exists());
        assert_eq!(effacer_dans(&dossier), Ok(()));
        assert!(!dossier.join(NOM_FICHIER).exists());
        assert_eq!(lire_dans(&dossier), Ok(None));
    }

    #[test]
    fn ecrire_vide_remplace_l_ancien() {
        // Le repli du retrait quand l'effacement échoue : un fichier vide, posé atomiquement.
        let d = tempfile::tempdir().unwrap();
        ecrire_dans(d.path(), "BS1.x.y").unwrap();
        ecrire_dans(d.path(), "").unwrap();
        assert_eq!(lire_dans(d.path()), Ok(Some(String::new())));
        assert!(!d.path().join(NOM_TEMPORAIRE).exists());
    }

    #[test]
    fn un_vieux_temporaire_ne_gene_pas() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join(NOM_TEMPORAIRE), "reste").unwrap();
        ecrire_dans(d.path(), "BS1.x.y").unwrap();
        assert_eq!(lire_dans(d.path()), Ok(Some("BS1.x.y".into())));
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
        assert!(ecrire_dans(d.path(), "").is_err());
        assert_eq!(lire_dans(d.path()), Ok(Some("ancien".into())));
    }

    #[test]
    fn octets_non_utf8_illisibles_et_gardes() {
        let d = tempfile::tempdir().unwrap();
        let octets = [0xffu8, 0xfe, b'B', b'S', b'1'];
        fs::write(d.path().join(NOM_FICHIER), octets).unwrap();
        let erreur = lire_dans(d.path()).unwrap_err();
        // Un code court, jamais le chemin.
        assert_eq!(erreur, "InvalidData");
        // Le fichier n'est pas effacé, ni changé.
        assert_eq!(fs::read(d.path().join(NOM_FICHIER)).unwrap(), octets);
    }

    #[test]
    fn dossier_a_la_place_du_fichier_illisible_et_garde() {
        let d = tempfile::tempdir().unwrap();
        let chemin = d.path().join(NOM_FICHIER);
        fs::create_dir(&chemin).unwrap();
        fs::write(chemin.join("dedans"), "x").unwrap();
        assert!(lire_dans(d.path()).is_err());
        assert!(chemin.is_dir());
        assert!(chemin.join("dedans").is_file());
        assert!(effacer_dans(d.path()).is_err());
        assert!(chemin.join("dedans").is_file());
    }

    #[test]
    fn enorme_lu_tronque() {
        let d = tempfile::tempdir().unwrap();
        let enorme = "A".repeat(TAILLE_MAX as usize * 2);
        fs::write(d.path().join(NOM_FICHIER), &enorme).unwrap();
        assert_eq!(
            lire_dans(d.path()).map(|t| t.map(|t| t.len())),
            Ok(Some(TAILLE_MAX as usize))
        );
        // C'est pourquoi le texte est rangé nettoyé : 70 000 espaces puis une licence, rangés
        // bruts, perdraient la licence à la relecture.
        let brut = format!("{}BS1.x.y", " ".repeat(70_000));
        ecrire_dans(d.path(), &brut).unwrap();
        let relu = lire_dans(d.path()).unwrap().unwrap();
        assert_eq!(relu.len(), TAILLE_MAX as usize);
        assert!(!relu.contains("BS1"));
    }

    #[test]
    fn bom_garde_tel_quel() {
        // `sortie` ne retire pas un BOM : c'est la vérification qui le refuse (`forme`).
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join(NOM_FICHIER), "\u{feff}BS1.x.y").unwrap();
        assert_eq!(lire_dans(d.path()), Ok(Some("\u{feff}BS1.x.y".into())));
    }
}
