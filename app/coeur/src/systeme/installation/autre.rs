//! Réponses neutres hors Windows : les fichiers sont réels, le reste est « sans objet ».
//! Le parcours entier se joue ainsi sous Linux pour le développement de la page.

use std::io;
use std::path::{Path, PathBuf};

use super::{DossiersConnus, Valeur};
use crate::systeme::Rect;

pub struct Poignee;

pub fn est_partage_refuse(_e: &io::Error) -> bool {
    false
}

pub fn dossiers_connus() -> DossiersConnus {
    DossiersConnus {
        programmes: dirs::data_local_dir().map(|d| d.join("Programs")),
        menu: None,
        local: dirs::data_local_dir(),
        temporaire: std::env::temp_dir(),
    }
}

pub fn espace_libre(_dossier: &Path) -> Option<u64> {
    None
}

pub fn lettre_volume(_dossier: &Path) -> Option<String> {
    None
}

pub fn remplacer(provisoire: &Path, cible: &Path) -> io::Result<()> {
    std::fs::rename(provisoire, cible)
}

pub fn creer_raccourci(_lien: &Path, _cible: &Path) -> Result<bool, String> {
    Ok(false)
}

pub fn cible_raccourci(_lien: &Path) -> Option<PathBuf> {
    None
}

pub fn ecrire_cle(_cle: &str, _valeurs: &[(&str, Valeur)]) -> Result<bool, String> {
    Ok(false)
}

pub fn lire_texte_cle(_cle: &str, _nom: &str) -> Option<String> {
    None
}

pub fn retirer_valeurs_et_cle(_cle: &str, _noms: &[&str]) -> Result<bool, String> {
    Ok(false)
}

pub fn verrou_existe(_nom: &str) -> bool {
    false
}

pub fn prendre_verrou(_nom: &str) -> Option<Poignee> {
    Some(Poignee)
}

pub fn permettre_premier_plan() {}

pub fn version_webview2() -> Option<String> {
    Some("sans objet".into())
}

pub fn zone_de_notification() -> Option<Rect> {
    None
}

pub fn zone_utile_sous_souris() -> Option<(Rect, f64)> {
    None
}

pub fn message_bloquant(titre: &str, texte: &str) {
    eprintln!("{titre} : {texte}");
}

pub fn deplacer_fenetre(_identifiant: isize, _x: i32, _y: i32) -> bool {
    false
}

pub fn rect_fenetre(_identifiant: isize) -> Option<Rect> {
    None
}

pub fn ramener_fenetre(_titre: &str) -> bool {
    false
}
