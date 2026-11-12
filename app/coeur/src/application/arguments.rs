//! Ligne de commande (§3.4) et ce que l'exe est (installateur, app ou désinstallation).
//!
//! `breezyshot.exe`, `breezyshot.exe --quitter`, `breezyshot.exe --capturer <mode>`,
//! `breezyshot.exe --desinstaller`. Sans option, l'exe est l'app s'il s'appelle
//! `breezyshot.exe` (casse ignorée), l'installateur sinon (le fichier téléchargé).

/// Ce que la ligne de commande demande.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Commande {
    /// Démarrer (ou, si une instance tourne déjà, ne rien faire de visible).
    Demarrer,
    /// Demander à l'instance en cours de se fermer proprement.
    Quitter,
    /// Lancer une capture de ce mode (nom d'action : `zone`, `plein_ecran`, `fenetre` ;
    /// une valeur inconnue ne fait qu'une ligne au journal). Vide si l'option n'a pas de
    /// valeur.
    Capturer(String),
    /// Désinstaller BreezyShot (« Applications installées »).
    Desinstaller,
}

/// Ce que l'exe va être.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lancement {
    App(Commande),
    Installateur,
    Desinstallation,
}

/// Le nom de l'exe installé : le seul qui fait de l'exe l'app sans option.
pub const NOM_DE_L_APP: &str = "breezyshot.exe";

/// Lecture des arguments, sans le nom du programme. Rend la commande et les arguments
/// inconnus (ignorés, à écrire au journal). `--desinstaller` l'emporte sur tout, puis
/// `--quitter`, puis `--capturer`.
pub fn lire<I, S>(arguments: I) -> (Commande, Vec<String>)
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut quitter = false;
    let mut desinstaller = false;
    let mut capturer = None;
    let mut inconnus = Vec::new();
    let mut suite = arguments
        .into_iter()
        .map(|a| a.as_ref().to_string())
        .peekable();
    while let Some(a) = suite.next() {
        match a.as_str() {
            "--quitter" => quitter = true,
            "--desinstaller" => desinstaller = true,
            "--capturer" => {
                let valeur = match suite.peek() {
                    Some(v) if !v.starts_with("--") => suite.next().unwrap_or_default(),
                    _ => String::new(),
                };
                capturer = Some(valeur);
            }
            _ => inconnus.push(a),
        }
    }
    let commande = if desinstaller {
        Commande::Desinstaller
    } else if quitter {
        Commande::Quitter
    } else if let Some(mode) = capturer {
        Commande::Capturer(mode)
    } else {
        Commande::Demarrer
    };
    (commande, inconnus)
}

/// Vrai si le nom de fichier est celui de l'app (casse ignorée, rien d'autre de toléré).
pub fn est_nom_de_l_app(nom: &str) -> bool {
    nom.eq_ignore_ascii_case(NOM_DE_L_APP)
}

/// Ce que l'exe est, d'après le nom de son fichier (`None` : inconnu, on reste l'app) et
/// la commande. Les options valent quel que soit le nom.
pub fn lancement(nom_exe: Option<&str>, commande: &Commande) -> Lancement {
    match commande {
        Commande::Desinstaller => Lancement::Desinstallation,
        Commande::Demarrer => match nom_exe {
            Some(nom) if !est_nom_de_l_app(nom) => Lancement::Installateur,
            _ => Lancement::App(Commande::Demarrer),
        },
        autre => Lancement::App(autre.clone()),
    }
}

/// Le nom de fichier d'un chemin d'exe, tel quel.
pub fn nom_de_fichier(chemin: &std::path::Path) -> Option<String> {
    chemin
        .file_name()
        .and_then(|n| n.to_str())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn sans_argument_on_demarre() {
        assert_eq!(lire(Vec::<String>::new()), (Commande::Demarrer, vec![]));
    }

    #[test]
    fn quitter() {
        assert_eq!(lire(["--quitter"]), (Commande::Quitter, vec![]));
    }

    #[test]
    fn inconnus_ignores_mais_rendus() {
        let (c, i) = lire(["--bizarre", "--quitter", "x"]);
        assert_eq!(c, Commande::Quitter);
        assert_eq!(i, ["--bizarre", "x"]);
    }

    #[test]
    fn capturer() {
        assert_eq!(
            lire(["--capturer", "zone"]).0,
            Commande::Capturer("zone".into())
        );
        assert_eq!(
            lire(["--capturer", "fenetre"]).0,
            Commande::Capturer("fenetre".into())
        );
        assert_eq!(lire(["--capturer"]).0, Commande::Capturer(String::new()));
        let (c, i) = lire(["--capturer", "--bizarre"]);
        assert_eq!(c, Commande::Capturer(String::new()));
        assert_eq!(i, ["--bizarre"]);
        assert_eq!(
            lire(["--capturer", "zone", "--quitter"]).0,
            Commande::Quitter
        );
    }

    #[test]
    fn desinstaller_l_emporte() {
        assert_eq!(
            lire(["--quitter", "--desinstaller"]).0,
            Commande::Desinstaller
        );
    }

    #[test]
    fn nom_de_l_app() {
        for oui in ["breezyshot.exe", "BreezyShot.exe", "BREEZYSHOT.EXE"] {
            assert!(est_nom_de_l_app(oui), "{oui}");
        }
        for non in [
            "BreezyShot-0.3.0-essai.exe",
            "BreezyShot-0.3.0-essai (1).exe",
            "breezyshot (1).exe",
            "breezyshot",
            " breezyshot.exe",
            "breezyshot.exe.nouveau",
            "breezyshot.éxe",
        ] {
            assert!(!est_nom_de_l_app(non), "{non}");
        }
    }

    #[test]
    fn ce_que_l_exe_est() {
        let d = Commande::Demarrer;
        let chemin = |c: &str| nom_de_fichier(Path::new(c));
        let telecharge = chemin("/home/Mes Téléchargements/BreezyShot-0.3.0-essai (1).exe");
        assert_eq!(
            telecharge.as_deref(),
            Some("BreezyShot-0.3.0-essai (1).exe")
        );
        assert_eq!(
            lancement(telecharge.as_deref(), &d),
            Lancement::Installateur
        );
        let installe = chemin("/home/Données é/Programs/BreezyShot/BreezyShot.EXE");
        assert_eq!(
            lancement(installe.as_deref(), &d),
            Lancement::App(d.clone())
        );
        assert_eq!(lancement(None, &d), Lancement::App(d.clone()));
        // Les options valent quel que soit le nom.
        let t = telecharge.as_deref();
        assert_eq!(
            lancement(t, &Commande::Quitter),
            Lancement::App(Commande::Quitter)
        );
        let c = Commande::Capturer("zone".into());
        assert_eq!(lancement(t, &c), Lancement::App(c.clone()));
        assert_eq!(
            lancement(installe.as_deref(), &Commande::Desinstaller),
            Lancement::Desinstallation
        );
        assert_eq!(
            lancement(t, &Commande::Desinstaller),
            Lancement::Desinstallation
        );
    }
}
