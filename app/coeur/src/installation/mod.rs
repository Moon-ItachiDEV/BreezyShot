//! Installation (version 0.3) : décisions pures de l'installateur et de la désinstallation.
//! Où écrire, quoi faire quand on relance le fichier téléchargé, quelles étapes, quelle
//! erreur, quelle réaction à chaque bouton, où voler. Les gestes sont dans
//! `systeme::installation` ; le câblage dans `application/installateur.rs`.
//! Ce module n'utilise que `systeme`.

pub mod emplacements;
pub mod envol;
pub mod parcours;
pub mod version;

pub use emplacements::{
    dossier_webview_a_nous, ecritures, emplacements, retraits, valeurs_desinstallation, Ecriture,
    Emplacements, Retrait,
};
pub use envol::{cible_envol, place_fenetre, position_envol};
pub use parcours::{
    besoin_octets, reagir, verifier_place, Action, ErreurInstallation, Etape, EtatInstallateur,
    Geste, ETAPES,
};
pub use version::{decider_plan, Plan, Trouvee, Version};

/// Identifiant de l'app (`tauri.conf.json`), d'où vient le nom du mutex de son instance.
pub const IDENTIFIANT: &str = "dev.breezyshot.desktop";

/// Mutex qui empêche deux installateurs à la fois.
pub const VERROU_INSTALLATEUR: &str = "BreezyShotInstallateur";

/// Nom du mutex de l'instance unique de l'app : celui que pose
/// `tauri-plugin-single-instance` 2.5.5 (sans son option `semver`), `<identifiant>-sim`.
/// Le même pour les versions 0.1, 0.2 et 0.3, installées ou non.
pub fn nom_verrou_app(identifiant: &str) -> String {
    format!("{identifiant}-sim")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verrou_de_l_app() {
        assert_eq!(nom_verrou_app(IDENTIFIANT), "dev.breezyshot.desktop-sim");
    }

    #[test]
    fn identifiant_de_la_configuration() {
        let conf = include_str!("../../tauri.conf.json");
        let v: serde_json::Value = serde_json::from_str(conf).unwrap();
        assert_eq!(v["identifier"], IDENTIFIANT);
        assert_eq!(v["mainBinaryName"], "breezyshot");
    }

    #[test]
    fn greffon_sans_semver() {
        // Avec l'option `semver`, le nom du mutex changerait : on le vérifie dans le verrou.
        let verrou = include_str!("../../Cargo.lock");
        let bloc = verrou
            .split("[[package]]")
            .find(|b| b.contains("name = \"tauri-plugin-single-instance\""))
            .unwrap();
        assert!(!bloc.contains("\"semver\""), "{bloc}");
        assert!(bloc.contains("version = \"2.5.5\""), "{bloc}");
    }
}
