//! « Lancer au démarrage » (étape 10) : une seule fonction, `demarrage_avec_systeme`.
//!
//! Windows : la valeur `BreezyShot` de la clé `Run` de l'utilisateur courant, et la
//! désactivation éventuelle par le gestionnaire des tâches (`StartupApproved\Run`), lue
//! seulement. C'est la seule écriture de l'app dans le registre (ARCHITECTURE §2) ; elle
//! reste derrière cette fonction pour pouvoir passer plus tard à l'entrée d'un paquet.
//! Pendant les essais (`BREEZYSHOT_DOSSIER_ESSAI`), une clé d'essai remplace la vraie : la
//! vraie clé n'est alors ni lue ni écrite. Hors Windows : `Indisponible`.

use std::path::Path;

#[cfg(windows)]
#[path = "windows.rs"]
mod plateforme;

#[cfg(not(windows))]
#[path = "autre.rs"]
mod plateforme;

/// Nom de la valeur écrite dans la clé `Run`.
pub const NOM_VALEUR: &str = "BreezyShot";
/// Clés de l'utilisateur courant (sous `HKEY_CURRENT_USER`).
pub const CLE_RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
pub const CLE_APPROUVE: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
/// Clés d'essai, à la place des vraies quand `BREEZYSHOT_DOSSIER_ESSAI` est défini.
pub const CLE_RUN_ESSAI: &str = r"Software\BreezyShot\Essais\Run";
pub const CLE_APPROUVE_ESSAI: &str = r"Software\BreezyShot\Essais\StartupApproved\Run";

/// Ce que dit le système.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatDemarrage {
    /// L'app démarre avec la session.
    Actif,
    /// Rien n'est inscrit.
    Inactif,
    /// Inscrite, mais désactivée par l'utilisateur dans le gestionnaire des tâches :
    /// elle ne démarre pas. BreezyShot ne touche pas à ce choix.
    DesactiveParWindows,
    /// Pas de démarrage avec le système ici (hors Windows).
    Indisponible,
}

impl EtatDemarrage {
    /// Nom pour le journal.
    pub fn nom(self) -> &'static str {
        match self {
            EtatDemarrage::Actif => "actif",
            EtatDemarrage::Inactif => "inactif",
            EtatDemarrage::DesactiveParWindows => "desactive par windows",
            EtatDemarrage::Indisponible => "indisponible",
        }
    }
}

/// Vrai si la clé d'essai remplace la vraie (essais seulement).
pub fn cle_d_essai() -> bool {
    std::env::var_os(super::VARIABLE_ESSAI).is_some()
}

/// Les deux clés à utiliser : (`Run`, `StartupApproved\Run`).
pub fn cles(essai: bool) -> (&'static str, &'static str) {
    if essai {
        (CLE_RUN_ESSAI, CLE_APPROUVE_ESSAI)
    } else {
        (CLE_RUN, CLE_APPROUVE)
    }
}

/// Le texte écrit dans `Run` : le chemin de l'exe entre guillemets (un chemin avec des
/// espaces reste un seul programme). Fonction pure.
pub fn commande(exe: &Path) -> String {
    format!("\"{}\"", exe.display())
}

/// Règle pure : `run` est la valeur de `Run` si elle existe, `approuve` celle de
/// `StartupApproved\Run`. Le gestionnaire des tâches y écrit 12 octets dont le premier est
/// pair quand l'entrée est permise (2, 6) et impair quand elle est désactivée (3, 7).
pub fn etat_depuis(run: Option<&str>, approuve: Option<&[u8]>) -> EtatDemarrage {
    match (run, approuve.and_then(|o| o.first())) {
        (None, _) => EtatDemarrage::Inactif,
        (Some(_), Some(premier)) if premier % 2 == 1 => EtatDemarrage::DesactiveParWindows,
        (Some(_), _) => EtatDemarrage::Actif,
    }
}

/// Lit (`voulu = None`) ou change puis relit (`Some(oui)`) le démarrage avec le système.
/// `Some(true)` écrit la valeur `Run` avec le chemin de l'exe en cours (`current_exe`),
/// `Some(false)` la supprime (sans erreur si elle n'existait pas : c'est ce qu'appelle
/// la désinstallation). L'état rendu est toujours relu dans le système.
pub fn demarrage_avec_systeme(voulu: Option<bool>) -> Result<EtatDemarrage, String> {
    let (run, approuve) = cles(cle_d_essai());
    if let Some(oui) = voulu {
        if oui {
            let exe = std::env::current_exe().map_err(|e| format!("exe introuvable: {e}"))?;
            plateforme::ecrire(run, NOM_VALEUR, &commande(&exe))?;
        } else {
            plateforme::supprimer(run, NOM_VALEUR)?;
        }
    }
    Ok(plateforme::lire(run, approuve, NOM_VALEUR))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn etat_selon_les_valeurs() {
        assert_eq!(etat_depuis(None, None), EtatDemarrage::Inactif);
        assert_eq!(etat_depuis(None, Some(&[3, 0])), EtatDemarrage::Inactif);
        assert_eq!(
            etat_depuis(Some("\"C:\\a.exe\""), None),
            EtatDemarrage::Actif
        );
        assert_eq!(
            etat_depuis(Some("x"), Some(&[2, 0, 0])),
            EtatDemarrage::Actif
        );
        assert_eq!(etat_depuis(Some("x"), Some(&[6])), EtatDemarrage::Actif);
        assert_eq!(etat_depuis(Some("x"), Some(&[])), EtatDemarrage::Actif);
        assert_eq!(
            etat_depuis(Some("x"), Some(&[3, 0, 0])),
            EtatDemarrage::DesactiveParWindows
        );
        assert_eq!(
            etat_depuis(Some("x"), Some(&[7])),
            EtatDemarrage::DesactiveParWindows
        );
    }

    #[test]
    fn commande_entre_guillemets() {
        let exe = Path::new("/opt/Breezy Shot/breezyshot");
        assert_eq!(commande(exe), "\"/opt/Breezy Shot/breezyshot\"");
    }

    #[test]
    fn cle_d_essai_jamais_la_vraie() {
        let (run, approuve) = cles(true);
        assert_ne!(run, CLE_RUN);
        assert_ne!(approuve, CLE_APPROUVE);
        assert!(run.starts_with(r"Software\BreezyShot\"));
        assert!(approuve.starts_with(r"Software\BreezyShot\"));
        assert_eq!(cles(false), (CLE_RUN, CLE_APPROUVE));
    }
}
