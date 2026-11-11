//! Petit fichier d'état de l'app : `<local>/etat.json`. Pour l'instant, la dernière
//! situation de raccourci annoncée (§5), pour ne pas répéter le même message.
//!
//! Tolérant : absent ou illisible = aucune situation. Écriture atomique (fichier
//! temporaire puis renommage). Ne panique jamais.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::raccourcis::Situation;

pub const NOM_FICHIER: &str = "etat.json";
const CLE_ANNONCE: &str = "annonce_raccourci";

pub fn chemin(local: &Path) -> PathBuf {
    local.join(NOM_FICHIER)
}

/// L'objet JSON du fichier, ou un objet vide s'il est absent ou illisible.
fn objet_depuis_texte(texte: &str) -> Map<String, Value> {
    match serde_json::from_str::<Value>(texte) {
        Ok(Value::Object(o)) => o,
        _ => Map::new(),
    }
}

/// La situation retenue dans le texte du fichier (règle pure).
pub fn situation_depuis_texte(texte: &str) -> Option<Situation> {
    let objet = objet_depuis_texte(texte);
    let s = objet.get(CLE_ANNONCE)?.as_object()?;
    let champ = |nom: &str| s.get(nom).and_then(Value::as_str).map(str::to_string);
    Some(Situation {
        action: champ("action")?,
        candidat: champ("candidat")?,
        raison: champ("raison")?,
    })
}

/// Le nouveau texte du fichier : la situation posée ou retirée, le reste gardé (règle pure).
pub fn texte_avec(ancien: Option<&str>, situation: Option<&Situation>) -> String {
    let mut objet = ancien.map(objet_depuis_texte).unwrap_or_default();
    match situation {
        Some(s) => {
            objet.insert(
                CLE_ANNONCE.into(),
                json!({ "action": s.action, "candidat": s.candidat, "raison": s.raison }),
            );
        }
        None => {
            objet.remove(CLE_ANNONCE);
        }
    }
    let mut texte =
        serde_json::to_string_pretty(&Value::Object(objet)).unwrap_or_else(|_| "{}".into());
    texte.push('\n');
    texte
}

/// Lit la dernière situation annoncée.
pub fn lire_situation(local: &Path) -> Option<Situation> {
    let texte = fs::read_to_string(chemin(local)).ok()?;
    situation_depuis_texte(&texte)
}

/// Écrit (ou efface) la situation. Rien n'est écrit si rien ne change.
pub fn ecrire_situation(local: &Path, situation: Option<&Situation>) -> Result<(), String> {
    let cible = chemin(local);
    let ancien = fs::read_to_string(&cible).ok();
    if ancien.is_none() && situation.is_none() {
        return Ok(());
    }
    let nouveau = texte_avec(ancien.as_deref(), situation);
    if ancien.as_deref() == Some(nouveau.as_str()) {
        return Ok(());
    }
    fs::create_dir_all(local).map_err(|e| format!("{}: {e}", local.display()))?;
    let temporaire = local.join(format!("{NOM_FICHIER}.tmp"));
    fs::write(&temporaire, nouveau).map_err(|e| format!("{}: {e}", temporaire.display()))?;
    fs::rename(&temporaire, &cible).map_err(|e| {
        let _ = fs::remove_file(&temporaire);
        format!("{}: {e}", cible.display())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s() -> Situation {
        Situation {
            action: "zone".into(),
            candidat: "ImprEcran".into(),
            raison: "reserve_par_le_systeme".into(),
        }
    }

    #[test]
    fn lecture_tolerante() {
        assert_eq!(situation_depuis_texte(""), None);
        assert_eq!(situation_depuis_texte("pas du json"), None);
        assert_eq!(situation_depuis_texte("[1]"), None);
        assert_eq!(situation_depuis_texte("{}"), None);
        assert_eq!(
            situation_depuis_texte(r#"{"annonce_raccourci": {"action": "zone"}}"#),
            None
        );
        assert_eq!(
            situation_depuis_texte(
                r#"{"annonce_raccourci": {"action": 1, "candidat": "a", "raison": "b"}}"#
            ),
            None
        );
    }

    #[test]
    fn aller_retour_et_autres_champs_gardes() {
        let t = texte_avec(Some(r#"{"autre": 3}"#), Some(&s()));
        assert_eq!(situation_depuis_texte(&t), Some(s()));
        assert!(t.contains("\"autre\": 3"));
        assert!(t.ends_with('\n'));
        let efface = texte_avec(Some(&t), None);
        assert_eq!(situation_depuis_texte(&efface), None);
        assert!(efface.contains("\"autre\": 3"));
        assert_eq!(texte_avec(Some("abime"), None), "{}\n");
    }

    #[test]
    fn sur_le_disque() {
        let d = tempfile::tempdir().unwrap();
        let local = d.path().join("local");
        assert_eq!(lire_situation(&local), None);
        // Rien à effacer : aucun fichier créé.
        ecrire_situation(&local, None).unwrap();
        assert!(!chemin(&local).exists());
        ecrire_situation(&local, Some(&s())).unwrap();
        assert_eq!(lire_situation(&local), Some(s()));
        fs::write(chemin(&local), "{abime").unwrap();
        assert_eq!(lire_situation(&local), None);
        ecrire_situation(&local, Some(&s())).unwrap();
        assert_eq!(lire_situation(&local), Some(s()));
        ecrire_situation(&local, None).unwrap();
        assert_eq!(lire_situation(&local), None);
        assert!(!local.join("etat.json.tmp").exists());
    }
}
