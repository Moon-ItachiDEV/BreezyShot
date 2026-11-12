//! Versions et choix du plan quand on relance le fichier téléchargé (ARCHITECTURE §12.3).

use std::fmt;

/// Une version `majeure.mineure.correctif`, comparée nombre par nombre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub majeure: u32,
    pub mineure: u32,
    pub correctif: u32,
}

impl Version {
    /// Lit `0.3.0` (espaces autour permis). Toute autre forme (étiquette, nombre manquant,
    /// signe) : `None`.
    pub fn lire(texte: &str) -> Option<Version> {
        let mut parties = texte.trim().split('.');
        let mut nombre = || -> Option<u32> {
            let p = parties.next()?;
            if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            p.parse().ok()
        };
        let v = Version {
            majeure: nombre()?,
            mineure: nombre()?,
            correctif: nombre()?,
        };
        parties.next().is_none().then_some(v)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.majeure, self.mineure, self.correctif)
    }
}

/// Ce qu'on trouve sur la machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trouvee {
    /// Ni clé de désinstallation ni exe installé.
    Aucune,
    /// Une clé de désinstallation (`version` : sa `DisplayVersion` lue, `None` si
    /// illisible) ou un exe installé.
    Installee {
        cle: bool,
        version: Option<Version>,
        exe_present: bool,
    },
}

impl Trouvee {
    /// Ligne du journal : `aucune`, `0.2.0 exe present`, `illisible exe absent`…
    pub fn ligne(&self) -> String {
        match self {
            Trouvee::Aucune => "aucune".into(),
            Trouvee::Installee {
                cle,
                version,
                exe_present,
            } => {
                let v = match (cle, version) {
                    (false, _) => "sans cle".to_string(),
                    (true, Some(v)) => v.to_string(),
                    (true, None) => "illisible".to_string(),
                };
                let e = if *exe_present {
                    "exe present"
                } else {
                    "exe absent"
                };
                format!("{v} {e}")
            }
        }
    }
}

/// Ce que l'installateur va faire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    Installer,
    Reinstaller,
    MettreAJour {
        depuis: Option<Version>,
    },
    /// Une version plus récente est installée : rien n'est copié.
    PlusRecenteInstallee {
        version: Version,
    },
}

impl Plan {
    /// Ligne du journal (`plan: …`).
    pub fn ligne(&self) -> String {
        match self {
            Plan::Installer => "installer".into(),
            Plan::Reinstaller => "reinstaller".into(),
            Plan::MettreAJour { depuis: Some(v) } => format!("mettre a jour depuis {v}"),
            Plan::MettreAJour { depuis: None } => "mettre a jour depuis illisible".into(),
            Plan::PlusRecenteInstallee { version } => format!("plus recente installee {version}"),
        }
    }

    /// Vrai si le plan copie l'exe (tous sauf `PlusRecenteInstallee`).
    pub fn copie(&self) -> bool {
        !matches!(self, Plan::PlusRecenteInstallee { .. })
    }
}

/// Le plan, d'après la version qui tourne et ce qu'on a trouvé (ARCHITECTURE §12.3) :
/// installation incomplète (clé sans exe, exe sans clé) → `Installer` ; même version →
/// `Reinstaller` ; plus ancienne ou illisible → `MettreAJour` ; plus récente → rien.
pub fn decider_plan(courante: Version, trouvee: &Trouvee) -> Plan {
    match trouvee {
        Trouvee::Aucune => Plan::Installer,
        Trouvee::Installee {
            cle: true,
            version,
            exe_present: true,
        } => match version {
            Some(v) if *v == courante => Plan::Reinstaller,
            Some(v) if *v > courante => Plan::PlusRecenteInstallee { version: *v },
            autre => Plan::MettreAJour { depuis: *autre },
        },
        Trouvee::Installee { .. } => Plan::Installer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(t: &str) -> Version {
        Version::lire(t).unwrap()
    }

    fn installee(version: Option<&str>, exe: bool) -> Trouvee {
        Trouvee::Installee {
            cle: true,
            version: version.and_then(Version::lire),
            exe_present: exe,
        }
    }

    #[test]
    fn lecture() {
        assert_eq!(v("0.3.0").to_string(), "0.3.0");
        assert_eq!(v(" 1.20.300 ").to_string(), "1.20.300");
        for faux in [
            "",
            "0.3",
            "0.3.0.1",
            "0.3.0-essai",
            "v0.3.0",
            "0..1",
            "-1.0.0",
            "a.b.c",
        ] {
            assert_eq!(Version::lire(faux), None, "{faux}");
        }
    }

    #[test]
    fn comparaison_numerique() {
        assert!(v("0.10.0") > v("0.9.2"));
        assert!(v("1.0.0") > v("0.99.99"));
        assert!(v("0.3.1") > v("0.3.0"));
    }

    #[test]
    fn plans() {
        let c = v("0.3.0");
        assert_eq!(decider_plan(c, &Trouvee::Aucune), Plan::Installer);
        assert_eq!(
            decider_plan(c, &installee(Some("0.3.0"), true)),
            Plan::Reinstaller
        );
        assert_eq!(
            decider_plan(c, &installee(Some("0.2.0"), true)),
            Plan::MettreAJour {
                depuis: Some(v("0.2.0"))
            }
        );
        assert_eq!(
            decider_plan(c, &installee(Some("x"), true)),
            Plan::MettreAJour { depuis: None }
        );
        assert_eq!(
            decider_plan(c, &installee(Some("0.10.0"), true)),
            Plan::PlusRecenteInstallee {
                version: v("0.10.0")
            }
        );
        // Clé sans exe, exe sans clé : on réinstalle depuis le début.
        assert_eq!(
            decider_plan(c, &installee(Some("0.9.0"), false)),
            Plan::Installer
        );
        let sans_cle = Trouvee::Installee {
            cle: false,
            version: None,
            exe_present: true,
        };
        assert_eq!(decider_plan(c, &sans_cle), Plan::Installer);
    }

    #[test]
    fn lignes() {
        assert_eq!(
            Plan::MettreAJour { depuis: None }.ligne(),
            "mettre a jour depuis illisible"
        );
        assert_eq!(installee(Some("0.2.0"), true).ligne(), "0.2.0 exe present");
        assert_eq!(installee(None, false).ligne(), "illisible exe absent");
        assert!(!Plan::PlusRecenteInstallee {
            version: v("1.0.0")
        }
        .copie());
        assert!(Plan::Reinstaller.copie());
    }
}
