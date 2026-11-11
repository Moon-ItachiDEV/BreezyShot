//! Boîte à outils des gestes propres à chaque système.
//!
//! Façade commune : le reste du code n'appelle que les fonctions de ce fichier.
//! `windows.rs` les réalise pour Windows, `autre.rs` donne des réponses neutres ailleurs
//! (Linux pour le développement ; `macos.rs` viendra ensuite).
//! Ce module ne connaît aucun autre module de l'app.
//!
//! Règle du Microsoft Store : on lit le registre, on n'y écrit jamais.

use std::path::{Path, PathBuf};

#[cfg(windows)]
#[path = "windows.rs"]
mod plateforme;

#[cfg(not(windows))]
#[path = "autre.rs"]
mod plateforme;

/// Nom du dossier de l'app dans les dossiers de l'utilisateur.
const NOM_DOSSIER: &str = "BreezyShot";

/// Variable d'environnement des essais : tout va dessous, rien ne touche aux vrais dossiers.
pub const VARIABLE_ESSAI: &str = "BREEZYSHOT_DOSSIER_ESSAI";

/// Les trois endroits où l'app range ses fichiers. Résolus ici et nulle part ailleurs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dossiers {
    /// Dossier qui contient `reglages.json` (Windows : `%APPDATA%\BreezyShot`).
    pub reglages: PathBuf,
    /// Données locales : `Journal`, `Recents` (Windows : `%LOCALAPPDATA%\BreezyShot`).
    pub local: PathBuf,
    /// Captures enregistrées (Windows : dossier Images connu, puis `BreezyShot`).
    /// Résolu mais non exigé avant l'étape 6 : `None` si Windows ne le donne pas.
    pub images: Option<PathBuf>,
}

impl Dossiers {
    /// Dossier du journal, sous le dossier local.
    pub fn journal(&self) -> PathBuf {
        self.local.join("Journal")
    }
}

/// Dossier du journal. Ne dépend que du dossier local, pour que le journal marche même
/// si les autres dossiers sont introuvables.
pub fn dossier_journal() -> Result<PathBuf, String> {
    let essai = std::env::var_os(VARIABLE_ESSAI).map(PathBuf::from);
    resoudre_local(essai.as_deref(), dirs::data_local_dir()).map(|l| l.join("Journal"))
}

/// Règle pure du dossier local (testée sans système).
pub fn resoudre_local(essai: Option<&Path>, local: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(racine) = essai.filter(|p| !p.as_os_str().is_empty()) {
        return Ok(racine.join("local"));
    }
    local
        .map(|l| l.join(NOM_DOSSIER))
        .ok_or_else(|| "dossier de données locales introuvable".to_string())
}

/// Résout les dossiers de l'app. Les dossiers ne sont pas créés ici.
pub fn dossiers() -> Result<Dossiers, String> {
    let essai = std::env::var_os(VARIABLE_ESSAI).map(PathBuf::from);
    resoudre_dossiers(
        essai.as_deref(),
        dirs::config_dir(),
        dirs::data_local_dir(),
        dirs::picture_dir(),
    )
}

/// Règle pure de résolution des dossiers (testée sans système).
pub fn resoudre_dossiers(
    essai: Option<&Path>,
    configuration: Option<PathBuf>,
    local: Option<PathBuf>,
    images: Option<PathBuf>,
) -> Result<Dossiers, String> {
    let local = resoudre_local(essai, local)?;
    if let Some(racine) = essai.filter(|p| !p.as_os_str().is_empty()) {
        return Ok(Dossiers {
            reglages: racine.join("reglages"),
            local,
            images: Some(racine.join("images")),
        });
    }
    let configuration =
        configuration.ok_or("dossier de configuration de l'utilisateur introuvable")?;
    Ok(Dossiers {
        reglages: configuration.join(NOM_DOSSIER),
        local,
        images: images.map(|i| i.join(NOM_DOSSIER)),
    })
}

/// Thème de la barre des tâches (pas celui des applis).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Clair,
    Sombre,
}

impl Theme {
    /// Lecture de `SystemUsesLightTheme` : 0 = sombre, autre valeur = clair.
    pub fn depuis_valeur(valeur: u32) -> Theme {
        if valeur == 0 {
            Theme::Sombre
        } else {
            Theme::Clair
        }
    }

    pub fn nom(self) -> &'static str {
        match self {
            Theme::Clair => "clair",
            Theme::Sombre => "sombre",
        }
    }
}

/// Thème actuel de la barre des tâches. Hors Windows, ou si la valeur manque : sombre
/// (la barre de Windows 11 est sombre par défaut).
pub fn theme_barre() -> Theme {
    plateforme::valeur_theme_barre()
        .map(Theme::depuis_valeur)
        .unwrap_or(Theme::Sombre)
}

/// Appelle `rappel` avec le nouveau thème chaque fois que celui de la barre change.
/// Lance un fil de surveillance ; hors Windows, ne fait rien.
pub fn surveiller_theme_barre<F>(rappel: F) -> Result<(), String>
where
    F: Fn(Theme) + Send + 'static,
{
    plateforme::surveiller_theme_barre(Box::new(rappel))
}

/// Conscience d'échelle réelle du processus : `Some(true)` si Per-Monitor V2 est actif,
/// `Some(false)` sinon, `None` hors Windows (sans objet).
pub fn echelle_par_ecran_v2() -> Option<bool> {
    plateforme::echelle_par_ecran_v2()
}

/// Numéro de version (build) de Windows, par exemple 26100. `None` hors Windows.
pub fn version_windows() -> Option<u32> {
    plateforme::version_windows()
}

/// Vrai si Windows réserve la touche Impr. écran à l'Outil Capture.
pub fn impr_ecran_reserve() -> bool {
    match plateforme::lecture_impr_ecran() {
        Some((valeur, build)) => decider_impr_ecran(valeur, build),
        None => false,
    }
}

/// Build de Windows 11 23H2, à partir duquel Impr. écran ouvre l'Outil Capture par défaut.
pub const BUILD_23H2: u32 = 22631;

/// Règle pure de `impr_ecran_reserve()` (§5 d'ARCHITECTURE.md).
/// `valeur` : `PrintScreenKeyForSnippingEnabled` si présente. `build` : version de Windows.
/// Valeur présente : réservée si différente de 0. Absente : réservée à partir de 22631.
/// Build inconnu et valeur absente : on suppose réservée (le repli est sans risque).
pub fn decider_impr_ecran(valeur: Option<u32>, build: Option<u32>) -> bool {
    match (valeur, build) {
        (Some(v), _) => v != 0,
        (None, Some(b)) => b >= BUILD_23H2,
        (None, None) => true,
    }
}

/// Réglage Windows « Effets d'animation » éteint. Hors Windows : non.
pub fn animations_reduites() -> bool {
    plateforme::animations_reduites().unwrap_or(false)
}

/// Réglage Windows « Effets de transparence » allumé. Hors Windows ou valeur absente : oui.
pub fn effets_de_transparence() -> bool {
    plateforme::valeur_transparence()
        .map(|v| v != 0)
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dossiers_essai_vont_dessous() {
        let d = resoudre_dossiers(Some(Path::new("/tmp/essai")), None, None, None).unwrap();
        assert_eq!(d.reglages, Path::new("/tmp/essai/reglages"));
        assert_eq!(d.local, Path::new("/tmp/essai/local"));
        assert_eq!(d.images.as_deref(), Some(Path::new("/tmp/essai/images")));
        assert_eq!(d.journal(), Path::new("/tmp/essai/local/Journal"));
    }

    #[test]
    fn dossiers_reels_portent_le_nom_de_l_app() {
        let d = resoudre_dossiers(
            Some(Path::new("")),
            Some(PathBuf::from("/c")),
            Some(PathBuf::from("/l")),
            Some(PathBuf::from("/i")),
        )
        .unwrap();
        assert_eq!(d.reglages, Path::new("/c/BreezyShot"));
        assert_eq!(d.local, Path::new("/l/BreezyShot"));
        assert_eq!(d.images.as_deref(), Some(Path::new("/i/BreezyShot")));
    }

    #[test]
    fn dossier_manquant_est_une_erreur() {
        assert!(resoudre_dossiers(None, None, Some("/l".into()), Some("/i".into())).is_err());
        assert!(resoudre_dossiers(None, Some("/c".into()), None, Some("/i".into())).is_err());
    }

    #[test]
    fn images_optionnel() {
        let d = resoudre_dossiers(None, Some("/c".into()), Some("/l".into()), None).unwrap();
        assert_eq!(d.images, None);
    }

    #[test]
    fn journal_ne_depend_que_du_local() {
        assert_eq!(
            resoudre_local(None, Some("/l".into())).unwrap(),
            Path::new("/l/BreezyShot")
        );
        assert_eq!(
            resoudre_local(Some(Path::new("/e")), None).unwrap(),
            Path::new("/e/local")
        );
        assert!(resoudre_local(None, None).is_err());
    }

    #[test]
    fn theme_depuis_registre() {
        assert_eq!(Theme::depuis_valeur(0), Theme::Sombre);
        assert_eq!(Theme::depuis_valeur(1), Theme::Clair);
    }

    #[test]
    fn regle_impr_ecran() {
        assert!(decider_impr_ecran(Some(1), Some(19045)));
        assert!(!decider_impr_ecran(Some(0), Some(26100)));
        assert!(decider_impr_ecran(None, Some(22631)));
        assert!(decider_impr_ecran(None, Some(26100)));
        assert!(!decider_impr_ecran(None, Some(22621)));
        assert!(decider_impr_ecran(None, None));
    }
}
