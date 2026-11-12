//! Où l'installateur écrit, et ce que la désinstallation retire (ARCHITECTURE §12.2 et §12.5).
//!
//! Garantie : l'installateur n'écrit que ce que `ecritures` énumère, et la désinstallation
//! ne retire que ce que `retraits` énumère : des fichiers nommés un par un, des valeurs de
//! registre nommées une par une, et le dossier installé **seulement s'il est vide** (jamais
//! un dossier entier par son nom). Le seul dossier supprimé avec son contenu est le dossier
//! WebView2 de l'installateur, et seulement si `dossier_webview_a_nous` le reconnaît.

use std::path::{Path, PathBuf};

use super::parcours::ErreurInstallation;
use crate::systeme::installation::{DossiersConnus, Valeur};

/// Nom de l'exe installé (et seul nom qui fait de l'exe l'app, `arguments::lancement`).
pub const NOM_EXE: &str = "breezyshot.exe";
/// Copie en cours, renommée en `NOM_EXE` une fois vérifiée.
pub const NOM_NOUVEAU: &str = "breezyshot.exe.nouveau";
pub const NOM_DOSSIER: &str = "BreezyShot";
pub const NOM_RACCOURCI: &str = "BreezyShot.lnk";
/// Dossier WebView2 de l'installateur, dans le dossier temporaire.
pub const NOM_WEBVIEW: &str = "BreezyShot-installateur";
/// Ce que WebView2 met dans son dossier de données.
pub const CONTENU_WEBVIEW: &[&str] = &["EBWebView"];
/// Clé de désinstallation réelle, sous HKCU.
pub const CLE_DESINSTALLATION: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Uninstall\BreezyShot";
/// Racine des clés d'essai, sous HKCU.
pub const RACINE_CLES_ESSAI: &str = r"Software\BreezyShot-essais";

/// Les valeurs de la clé de désinstallation, dans l'ordre d'écriture (ARCHITECTURE §12.3).
pub const NOMS_VALEURS: [&str; 11] = [
    "DisplayName",
    "DisplayVersion",
    "Publisher",
    "DisplayIcon",
    "InstallLocation",
    "UninstallString",
    "QuietUninstallString",
    "NoModify",
    "NoRepair",
    "EstimatedSize",
    "InstallDate",
];

/// Option de la ligne de commande qui désinstalle.
pub const OPTION_DESINSTALLER: &str = "--desinstaller";

/// Les emplacements, réels ou d'essai.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Emplacements {
    pub dossier: PathBuf,
    pub exe: PathBuf,
    pub nouveau: PathBuf,
    pub raccourci: PathBuf,
    /// Sous HKCU.
    pub cle_desinstallation: String,
    /// Où va l'exe désinstallé (il ne peut pas s'effacer lui-même).
    pub temporaire: PathBuf,
    /// Repli de l'exe désinstallé quand `temporaire` est sur un autre volume : le parent du
    /// dossier des programmes (`%LOCALAPPDATA%`), sur le même volume que l'exe.
    pub secours: PathBuf,
    pub webview_installateur: PathBuf,
    /// Dossier du journal (`installation.log`).
    pub journal: PathBuf,
}

/// Emplacements d'après le dossier d'essai (`BREEZYSHOT_DOSSIER_ESSAI`) ou les dossiers
/// connus. En essai, tout est sous ce dossier, la clé sous `RACINE_CLES_ESSAI\<son nom>`.
/// Variable d'essai présente mais vide : refus (`Dossier`), jamais les emplacements réels.
pub fn emplacements(
    essai: Option<&Path>,
    connus: &DossiersConnus,
) -> Result<Emplacements, ErreurInstallation> {
    if let Some(e) = essai {
        if e.as_os_str().is_empty() {
            return Err(ErreurInstallation::Dossier);
        }
        let nom = e
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|n| !n.is_empty())
            .ok_or(ErreurInstallation::Dossier)?;
        let dossier = e.join("programmes").join(NOM_DOSSIER);
        let temporaire = e.join("temp");
        return Ok(Emplacements {
            exe: dossier.join(NOM_EXE),
            nouveau: dossier.join(NOM_NOUVEAU),
            dossier,
            raccourci: e.join("menu").join(NOM_RACCOURCI),
            cle_desinstallation: format!(r"{RACINE_CLES_ESSAI}\{nom}\Uninstall\BreezyShot"),
            webview_installateur: temporaire.join(NOM_WEBVIEW),
            temporaire,
            secours: e.to_path_buf(),
            journal: e.join("local").join("Journal"),
        });
    }
    let programmes = connus
        .programmes
        .as_ref()
        .ok_or(ErreurInstallation::Dossier)?;
    let menu = connus.menu.as_ref().ok_or(ErreurInstallation::Dossier)?;
    let local = connus.local.as_ref().ok_or(ErreurInstallation::Dossier)?;
    let dossier = programmes.join(NOM_DOSSIER);
    Ok(Emplacements {
        exe: dossier.join(NOM_EXE),
        nouveau: dossier.join(NOM_NOUVEAU),
        dossier,
        raccourci: menu.join(NOM_RACCOURCI),
        cle_desinstallation: CLE_DESINSTALLATION.into(),
        temporaire: connus.temporaire.clone(),
        secours: programmes.parent().unwrap_or(programmes).to_path_buf(),
        webview_installateur: connus.temporaire.join(NOM_WEBVIEW),
        journal: local.join(NOM_DOSSIER).join("Journal"),
    })
}

/// Ce que l'installateur écrit, et rien d'autre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ecriture {
    Fichier(PathBuf),
    Raccourci(PathBuf),
    /// Les valeurs `NOMS_VALEURS` de cette clé de HKCU.
    Cle(String),
    /// Données WebView2 de la fenêtre de l'installateur.
    DossierWebview(PathBuf),
    /// `installation.log` et sa rotation, dans ce dossier.
    Journal(PathBuf),
    /// L'exe désinstallé, déplacé dans ce dossier sous `nom_exe_deplace` (désinstallation).
    ExeDesinstalle(PathBuf),
}

pub fn ecritures(e: &Emplacements) -> Vec<Ecriture> {
    vec![
        Ecriture::Fichier(e.nouveau.clone()),
        Ecriture::Fichier(e.exe.clone()),
        Ecriture::Raccourci(e.raccourci.clone()),
        Ecriture::Cle(e.cle_desinstallation.clone()),
        Ecriture::DossierWebview(e.webview_installateur.clone()),
        Ecriture::Journal(e.journal.clone()),
        Ecriture::ExeDesinstalle(e.temporaire.clone()),
        Ecriture::ExeDesinstalle(e.secours.clone()),
    ]
}

/// Ce que la désinstallation retire, dans l'ordre : l'exe part d'abord ; le raccourci et la
/// clé ne sont retirés que s'il est parti (`exige_exe_parti`), pour ne jamais laisser un
/// exe installé sans moyen de le désinstaller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Retrait {
    /// Le raccourci, seulement s'il pointe vers `exe` (`raccourci_vers`).
    Raccourci {
        lien: PathBuf,
        exe: PathBuf,
    },
    /// Les valeurs nommées, puis la clé si elle est vide.
    Cle {
        cle: String,
        valeurs: &'static [&'static str],
    },
    Fichier(PathBuf),
    /// L'exe en cours, déplacé (un exe qui tourne ne s'efface pas) dans `vers`, ou dans
    /// `secours` si `vers` est sur un autre volume.
    ExeDeplace {
        de: PathBuf,
        vers: PathBuf,
        secours: PathBuf,
    },
    /// Le dossier installé, seulement s'il est vide.
    DossierSiVide(PathBuf),
    /// Le dossier WebView2 de l'installateur, seulement s'il est reconnu.
    DossierWebview(PathBuf),
}

/// Nom de l'exe désinstallé, déplacé dans le dossier temporaire.
pub fn nom_exe_deplace(processus: u32) -> String {
    format!("BreezyShot-desinstalle-{processus}.exe")
}

impl Retrait {
    /// Vrai pour ce qui ne se retire qu'une fois l'exe parti.
    pub fn exige_exe_parti(&self) -> bool {
        matches!(self, Retrait::Raccourci { .. } | Retrait::Cle { .. })
    }
}

pub fn retraits(e: &Emplacements, processus: u32) -> Vec<Retrait> {
    let nom = nom_exe_deplace(processus);
    vec![
        Retrait::Fichier(e.nouveau.clone()),
        Retrait::ExeDeplace {
            de: e.exe.clone(),
            vers: e.temporaire.join(&nom),
            secours: e.secours.join(&nom),
        },
        Retrait::Raccourci {
            lien: e.raccourci.clone(),
            exe: e.exe.clone(),
        },
        Retrait::Cle {
            cle: e.cle_desinstallation.clone(),
            valeurs: &NOMS_VALEURS,
        },
        Retrait::DossierSiVide(e.dossier.clone()),
        Retrait::DossierWebview(e.webview_installateur.clone()),
    ]
}

/// Vrai si le dossier WebView2 ne contient que ce que WebView2 y met (ou rien) : on peut
/// alors le supprimer avec son contenu. Nettoyé au lancement de l'installateur et à la
/// désinstallation (pendant que l'installateur tourne, WebView2 le tient).
pub fn dossier_webview_a_nous(entrees: &[String]) -> bool {
    entrees
        .iter()
        .all(|n| CONTENU_WEBVIEW.contains(&n.as_str()))
}

/// Vrai si le raccourci (sa cible lue) pointe vers l'exe installé. Les chemins Windows ne
/// distinguent ni la casse ni `/` de `\`. Cible illisible : ce n'est pas le nôtre.
pub fn raccourci_vers(cible: Option<&Path>, exe: &Path) -> bool {
    let normal = |p: &Path| p.to_string_lossy().replace('/', "\\").to_lowercase();
    cible.is_some_and(|c| normal(c) == normal(exe))
}

/// Ligne de commande de désinstallation : `"<exe>" --desinstaller`.
pub fn commande_desinstallation(exe: &Path) -> String {
    format!("\"{}\" {OPTION_DESINSTALLER}", exe.display())
}

/// Valeurs de la clé de désinstallation (ARCHITECTURE §12.3). `taille` en octets, `date` au
/// format `AAAAMMJJ`.
pub fn valeurs_desinstallation(
    e: &Emplacements,
    version: &str,
    taille: u64,
    date: &str,
) -> Vec<(&'static str, Valeur)> {
    let commande = commande_desinstallation(&e.exe);
    let kio = u32::try_from(taille.div_ceil(1024)).unwrap_or(u32::MAX);
    let valeurs = vec![
        Valeur::Texte("BreezyShot".into()),
        Valeur::Texte(version.into()),
        Valeur::Texte("BreezyShot".into()),
        Valeur::Texte(format!("\"{}\",0", e.exe.display())),
        Valeur::Texte(e.dossier.display().to_string()),
        Valeur::Texte(commande.clone()),
        Valeur::Texte(commande),
        Valeur::Nombre(1),
        Valeur::Nombre(1),
        Valeur::Nombre(kio),
        Valeur::Texte(date.into()),
    ];
    NOMS_VALEURS.into_iter().zip(valeurs).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connus() -> DossiersConnus {
        DossiersConnus {
            programmes: Some(PathBuf::from("/l/Programs")),
            menu: Some(PathBuf::from("/r/Menu/Programs")),
            local: Some(PathBuf::from("/l")),
            temporaire: PathBuf::from("/t"),
        }
    }

    #[test]
    fn emplacements_reels() {
        let e = emplacements(None, &connus()).unwrap();
        assert_eq!(e.dossier, Path::new("/l/Programs/BreezyShot"));
        assert_eq!(e.exe, Path::new("/l/Programs/BreezyShot/breezyshot.exe"));
        assert_eq!(e.raccourci, Path::new("/r/Menu/Programs/BreezyShot.lnk"));
        assert_eq!(e.cle_desinstallation, CLE_DESINSTALLATION);
        assert_eq!(
            e.webview_installateur,
            Path::new("/t/BreezyShot-installateur")
        );
        assert_eq!(e.journal, Path::new("/l/BreezyShot/Journal"));
        // Repli de l'exe désinstallé : même volume que l'exe, hors des données de l'app.
        assert_eq!(e.secours, Path::new("/l"));
        // Jamais dans le dossier des données de l'app.
        assert!(!e.dossier.starts_with("/l/BreezyShot"));
    }

    #[test]
    fn emplacements_d_essai() {
        let e = emplacements(
            Some(Path::new("/tmp/essai-09 é")),
            &DossiersConnus::default(),
        )
        .unwrap();
        assert_eq!(
            e.dossier,
            Path::new("/tmp/essai-09 é/programmes/BreezyShot")
        );
        assert_eq!(
            e.raccourci,
            Path::new("/tmp/essai-09 é/menu/BreezyShot.lnk")
        );
        assert_eq!(
            e.cle_desinstallation,
            r"Software\BreezyShot-essais\essai-09 é\Uninstall\BreezyShot"
        );
        assert_eq!(e.temporaire, Path::new("/tmp/essai-09 é/temp"));
        assert_eq!(e.journal, Path::new("/tmp/essai-09 é/local/Journal"));
        assert_eq!(e.secours, Path::new("/tmp/essai-09 é"));
        for ecr in ecritures(&e) {
            match ecr {
                Ecriture::Cle(c) => assert!(c.starts_with(RACINE_CLES_ESSAI)),
                Ecriture::Fichier(p)
                | Ecriture::Raccourci(p)
                | Ecriture::DossierWebview(p)
                | Ecriture::Journal(p)
                | Ecriture::ExeDesinstalle(p) => {
                    assert!(p.starts_with("/tmp/essai-09 é"), "{p:?}")
                }
            }
        }
    }

    #[test]
    fn dossiers_introuvables() {
        let mut c = connus();
        c.menu = None;
        assert_eq!(emplacements(None, &c), Err(ErreurInstallation::Dossier));
        assert_eq!(
            emplacements(Some(Path::new("/")), &connus()),
            Err(ErreurInstallation::Dossier)
        );
        // Variable d'essai présente mais vide : refus, même avec des dossiers réels connus.
        assert_eq!(
            emplacements(Some(Path::new("")), &connus()),
            Err(ErreurInstallation::Dossier)
        );
    }

    #[test]
    fn rien_hors_des_emplacements() {
        let e = emplacements(None, &connus()).unwrap();
        let ecrits = ecritures(&e);
        for ecr in &ecrits {
            match ecr {
                Ecriture::Fichier(p) => assert_eq!(p.parent(), Some(e.dossier.as_path())),
                Ecriture::Raccourci(p) => assert_eq!(p, &e.raccourci),
                Ecriture::Cle(c) => assert_eq!(c, CLE_DESINSTALLATION),
                Ecriture::DossierWebview(p) => assert_eq!(p, &e.webview_installateur),
                Ecriture::Journal(p) => assert_eq!(p, &e.journal),
                Ecriture::ExeDesinstalle(p) => {
                    assert!(p == &e.temporaire || p == &e.secours, "{p:?}")
                }
            }
        }
        // La désinstallation ne retire que ce qui a été écrit, et le dossier s'il est vide.
        let liste = retraits(&e, 42);
        // L'exe part d'abord ; raccourci et clé seulement après lui.
        let rang = |f: fn(&Retrait) -> bool| liste.iter().position(f).unwrap();
        let exe = rang(|r| matches!(r, Retrait::ExeDeplace { .. }));
        assert!(exe < rang(|r| matches!(r, Retrait::Raccourci { .. })));
        assert!(exe < rang(|r| matches!(r, Retrait::Cle { .. })));
        let exigeants: Vec<_> = liste.iter().filter(|r| r.exige_exe_parti()).collect();
        assert_eq!(exigeants.len(), 2);
        for r in liste {
            match r {
                Retrait::Raccourci { lien, exe } => {
                    assert!(ecrits.contains(&Ecriture::Raccourci(lien)));
                    assert_eq!(exe, e.exe);
                }
                Retrait::Cle { cle, valeurs } => {
                    assert!(ecrits.contains(&Ecriture::Cle(cle)));
                    assert_eq!(valeurs, NOMS_VALEURS);
                }
                Retrait::Fichier(p) => assert!(ecrits.contains(&Ecriture::Fichier(p))),
                Retrait::ExeDeplace { de, vers, secours } => {
                    assert!(ecrits.contains(&Ecriture::Fichier(de)));
                    assert_eq!(vers, Path::new("/t/BreezyShot-desinstalle-42.exe"));
                    assert_eq!(secours, Path::new("/l/BreezyShot-desinstalle-42.exe"));
                    for d in [vers.parent().unwrap(), secours.parent().unwrap()] {
                        assert!(ecrits.contains(&Ecriture::ExeDesinstalle(d.to_path_buf())));
                    }
                }
                Retrait::DossierSiVide(p) => assert_eq!(p, e.dossier),
                Retrait::DossierWebview(p) => {
                    assert!(ecrits.contains(&Ecriture::DossierWebview(p)))
                }
            }
        }
    }

    #[test]
    fn raccourci_a_nous() {
        let exe = Path::new(r"C:\Users\a\AppData\Local\Programs\BreezyShot\breezyshot.exe");
        let meme = Path::new(r"c:\users\A\AppData\Local\Programs\BreezyShot\BreezyShot.EXE");
        assert!(raccourci_vers(Some(meme), exe));
        assert!(raccourci_vers(Some(exe), exe));
        assert!(!raccourci_vers(None, exe));
        assert!(!raccourci_vers(
            Some(Path::new(r"C:\Autre\breezyshot.exe")),
            exe
        ));
    }

    #[test]
    fn dossier_webview() {
        assert!(dossier_webview_a_nous(&[]));
        assert!(dossier_webview_a_nous(&["EBWebView".into()]));
        assert!(!dossier_webview_a_nous(&[
            "EBWebView".into(),
            "mes notes.txt".into()
        ]));
    }

    #[test]
    fn valeurs_de_la_cle() {
        let e = emplacements(None, &connus()).unwrap();
        let v = valeurs_desinstallation(&e, "0.3.0", 15_519_744 + 1, "20261112");
        assert_eq!(v.len(), 11);
        let texte = |n: &str| match &v.iter().find(|(k, _)| *k == n).unwrap().1 {
            Valeur::Texte(t) => t.clone(),
            Valeur::Nombre(x) => x.to_string(),
        };
        assert_eq!(texte("DisplayName"), "BreezyShot");
        assert_eq!(texte("DisplayVersion"), "0.3.0");
        let exe = e.exe.display().to_string();
        assert_eq!(
            texte("UninstallString"),
            format!("\"{exe}\" --desinstaller")
        );
        assert_eq!(texte("QuietUninstallString"), texte("UninstallString"));
        assert_eq!(texte("DisplayIcon"), format!("\"{exe}\",0"));
        assert_eq!(texte("NoModify"), "1");
        assert_eq!(texte("NoRepair"), "1");
        assert_eq!(texte("EstimatedSize"), "15157");
        assert_eq!(texte("InstallDate"), "20261112");
    }
}
