//! Ligne de commande (§3.4) et ce que l'exe est (installateur, app ou désinstallation).
//!
//! `breezyshot.exe`, `breezyshot.exe --discret`, `breezyshot.exe --quitter`,
//! `breezyshot.exe --capturer <mode>`, `breezyshot.exe --desinstaller`, `breezyshot.exe <fichier>.breezy`. Sans option, l'exe est l'app s'il s'appelle
//! `breezyshot.exe` ou `breezyshot` (casse ignorée), l'installateur sinon (le fichier
//! téléchargé).

pub use crate::systeme::OPTION_DISCRET;

/// Ce que la ligne de commande demande.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Commande {
    /// Lancement à la main (icône épinglée, menu Démarrer, double-clic) : démarrer et ouvrir
    /// la barre de capture à `pret` ; si une instance tourne déjà, elle ouvre la barre.
    Demarrer,
    /// `--discret` : démarrer sans rien montrer (valeur `Run` de l'ouverture de session,
    /// app lancée par l'installateur, essais) ; une instance qui tourne déjà ne fait rien.
    Discret,
    /// Demander à l'instance en cours de se fermer proprement.
    Quitter,
    /// Lancer une capture de ce mode (nom d'action : `zone`, `plein_ecran`, `fenetre`,
    /// `derniere_zone` ; une valeur inconnue ne fait qu'une ligne au journal). Vide si
    /// l'option n'a pas de valeur. Le retardateur des réglages vaut aussi ici.
    Capturer(String),
    /// Désinstaller BreezyShot (« Applications installées »).
    Desinstaller,
    /// Un fichier `.breezy` donné en argument (double-clic dans l'Explorateur, 0.7.5) : le
    /// rouvrir dans l'éditeur. Le chemin tel que Windows l'a donné.
    Ouvrir(String),
}

/// Vrai si `argument` est un chemin de fichier `.breezy` (pas une option), casse ignorée.
pub fn est_fichier_breezy(argument: &str) -> bool {
    let ext = format!(".{}", crate::sortie::EXTENSION_BREEZY);
    !argument.starts_with("--")
        && argument.len() > ext.len()
        && argument[argument.len() - ext.len()..].eq_ignore_ascii_case(&ext)
}

/// Ce que l'exe va être.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lancement {
    App(Commande),
    Installateur,
    Desinstallation,
}

/// Le nom de l'exe installé (Windows) : il fait de l'exe l'app sans option.
pub const NOM_DE_L_APP: &str = "breezyshot.exe";
/// Le même sans extension : le nom du programme sous macOS et Linux, où il n'y a pas
/// d'installateur (sans cela, le programme de développement se prendrait pour lui).
pub const NOM_DE_L_APP_SANS_EXTENSION: &str = "breezyshot";

/// Lecture des arguments, sans le nom du programme. Rend la commande et les arguments
/// inconnus (ignorés, à écrire au journal). `--desinstaller` l'emporte sur tout, puis
/// `--quitter`, puis `--capturer`, puis un fichier `.breezy` (le premier), puis `--discret`.
pub fn lire<I, S>(arguments: I) -> (Commande, Vec<String>)
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut quitter = false;
    let mut desinstaller = false;
    let mut capturer = None;
    let mut discret = false;
    let mut fichier: Option<String> = None;
    let mut inconnus = Vec::new();
    let mut suite = arguments
        .into_iter()
        .map(|a| a.as_ref().to_string())
        .peekable();
    while let Some(a) = suite.next() {
        match a.as_str() {
            "--quitter" => quitter = true,
            "--desinstaller" => desinstaller = true,
            o if o == OPTION_DISCRET => discret = true,
            "--capturer" => {
                let valeur = match suite.peek() {
                    Some(v) if !v.starts_with("--") => suite.next().unwrap_or_default(),
                    _ => String::new(),
                };
                capturer = Some(valeur);
            }
            _ if est_fichier_breezy(&a) => {
                if fichier.is_none() {
                    fichier = Some(a);
                }
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
    } else if let Some(chemin) = fichier {
        Commande::Ouvrir(chemin)
    } else if discret {
        Commande::Discret
    } else {
        Commande::Demarrer
    };
    (commande, inconnus)
}

/// Les arguments d'une seconde instance, avec le chemin des fichiers `.breezy` rendu absolu
/// depuis `dossier` (le dossier courant de l'instance qui les a reçus). Le premier argument
/// est le programme : il reste tel quel.
pub fn absolus(argv: Vec<String>, dossier: &std::path::Path) -> Vec<String> {
    argv.into_iter()
        .enumerate()
        .map(|(i, a)| {
            if i > 0 && est_fichier_breezy(&a) && std::path::Path::new(&a).is_relative() {
                dossier.join(&a).to_string_lossy().into_owned()
            } else {
                a
            }
        })
        .collect()
}

/// Vrai si le nom de fichier est celui de l'app, avec ou sans `.exe` (casse ignorée, rien
/// d'autre de toléré).
pub fn est_nom_de_l_app(nom: &str) -> bool {
    nom.eq_ignore_ascii_case(NOM_DE_L_APP) || nom.eq_ignore_ascii_case(NOM_DE_L_APP_SANS_EXTENSION)
}

/// Ce que l'exe est, d'après le nom de son fichier (`None` : inconnu, on reste l'app) et
/// la commande. Les options valent quel que soit le nom.
pub fn lancement(nom_exe: Option<&str>, commande: &Commande) -> Lancement {
    match commande {
        Commande::Desinstaller => Lancement::Desinstallation,
        Commande::Demarrer | Commande::Ouvrir(_) => match nom_exe {
            Some(nom) if !est_nom_de_l_app(nom) => Lancement::Installateur,
            _ => Lancement::App(commande.clone()),
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
    fn discret() {
        assert_eq!(OPTION_DISCRET, "--discret");
        assert_eq!(lire(["--discret"]), (Commande::Discret, vec![]));
        // Les autres options l'emportent : rien ne s'y perd.
        assert_eq!(
            lire(["--discret", "--capturer", "zone"]).0,
            Commande::Capturer("zone".into())
        );
        assert_eq!(lire(["--quitter", "--discret"]).0, Commande::Quitter);
        let (c, i) = lire(["--discret", "--bizarre"]);
        assert_eq!((c, i), (Commande::Discret, vec!["--bizarre".to_string()]));
        // L'exe installé, lancé par `Run` : l'app, sans barre.
        assert_eq!(
            lancement(Some("breezyshot.exe"), &Commande::Discret),
            Lancement::App(Commande::Discret)
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
        for oui in [
            "breezyshot.exe",
            "BreezyShot.exe",
            "BREEZYSHOT.EXE",
            "breezyshot",
            "BreezyShot",
        ] {
            assert!(est_nom_de_l_app(oui), "{oui}");
        }
        for non in [
            "BreezyShot-0.3.0-essai.exe",
            "BreezyShot-0.3.0-essai (1).exe",
            "breezyshot (1).exe",
            "breezyshot (1)",
            "breezyshot.",
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

    #[test]
    fn fichier_breezy() {
        assert_eq!(
            lire([r"C:\Docs\Capture 1.breezy"]),
            (Commande::Ouvrir(r"C:\Docs\Capture 1.breezy".into()), vec![])
        );
        // Casse ignorée ; le premier fichier seul compte ; les options inconnues restent.
        let (c, i) = lire(["--bizarre", "A.BREEZY", "b.breezy"]);
        assert_eq!(c, Commande::Ouvrir("A.BREEZY".into()));
        assert_eq!(i, ["--bizarre"]);
        // Les options l'emportent, un nom sans nom n'est pas un fichier, une option non plus.
        assert_eq!(lire(["--quitter", "a.breezy"]).0, Commande::Quitter);
        assert_eq!(
            lire(["--capturer", "zone", "a.breezy"]).0,
            Commande::Capturer("zone".into())
        );
        for non in [
            ".breezy",
            "a.breezy.png",
            "--x.breezy",
            "a.breezyx",
            "breezy",
        ] {
            assert!(!est_fichier_breezy(non), "{non}");
        }
        assert_eq!(
            lire(["a.breezy", "--discret"]).0,
            Commande::Ouvrir("a.breezy".into())
        );
    }

    #[test]
    fn un_fichier_breezy_donne_a_l_exe_telecharge_lance_l_installateur() {
        let o = Commande::Ouvrir("a.breezy".into());
        assert_eq!(
            lancement(Some("BreezyShot-0.7.5-essai.exe"), &o),
            Lancement::Installateur
        );
        assert_eq!(
            lancement(Some("breezyshot.exe"), &o),
            Lancement::App(o.clone())
        );
        assert_eq!(lancement(None, &o), Lancement::App(o));
    }

    #[test]
    fn chemins_breezy_relatifs_d_une_seconde_instance() {
        let dossier = Path::new("/home/moi");
        let argv = ["prog.exe", "a.breezy", "--x", "/abs/b.breezy", "c.png"]
            .map(String::from)
            .to_vec();
        let sortie = absolus(argv, dossier);
        assert_eq!(sortie[0], "prog.exe");
        assert_eq!(Path::new(&sortie[1]), Path::new("/home/moi/a.breezy"));
        assert_eq!(&sortie[2..], ["--x", "/abs/b.breezy", "c.png"]);
        // Un programme qui s'appellerait « x.breezy » n'est pas touché.
        assert_eq!(absolus(vec!["x.breezy".into()], dossier), ["x.breezy"]);
    }
}
