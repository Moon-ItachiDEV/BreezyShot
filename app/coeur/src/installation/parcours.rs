//! Étapes, erreurs et table des réactions de l'installateur (contrat §3.2, §3.5, §4.4).

use std::time::Duration;

/// Place gardée en plus de l'exe (premier lancement de WebView2, journal).
pub const MARGE_PLACE: u64 = 64 << 20;
/// Délai pour qu'une app en cours se ferme après `--quitter`.
pub const DELAI_ARRET: Duration = Duration::from_secs(10);
/// Délai pour que l'app lancée tienne son instance unique.
pub const DELAI_LANCEMENT: Duration = Duration::from_secs(10);
/// Remplacement retenté tant que l'exe est pris.
pub const ATTENTE_REMPLACEMENT: Duration = Duration::from_secs(5);
/// Au plus une progression envoyée à la page par cet intervalle.
pub const PAS_PROGRESSION: Duration = Duration::from_millis(33);

/// Les étapes, dans l'ordre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Etape {
    Place,
    Arret,
    Copie,
    Verification,
    Raccourci,
    Inscription,
    Lancement,
}

pub const ETAPES: [Etape; 7] = [
    Etape::Place,
    Etape::Arret,
    Etape::Copie,
    Etape::Verification,
    Etape::Raccourci,
    Etape::Inscription,
    Etape::Lancement,
];

impl Etape {
    pub fn nom(self) -> &'static str {
        match self {
            Etape::Place => "place",
            Etape::Arret => "arret",
            Etape::Copie => "copie",
            Etape::Verification => "verification",
            Etape::Raccourci => "raccourci",
            Etape::Inscription => "inscription",
            Etape::Lancement => "lancement",
        }
    }
}

/// Les erreurs d'installation ; chacune a un code (journal) et une cause d'une ligne (page).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErreurInstallation {
    /// `manque` en octets.
    Place {
        lettre: Option<String>,
        manque: u64,
    },
    Ouvert,
    Ecriture,
    Copie,
    Raccourci,
    Inscription,
    Lancement,
    Dossier,
}

impl ErreurInstallation {
    pub fn code(&self) -> &'static str {
        match self {
            ErreurInstallation::Place { .. } => "place",
            ErreurInstallation::Ouvert => "ouvert",
            ErreurInstallation::Ecriture => "ecriture",
            ErreurInstallation::Copie => "copie",
            ErreurInstallation::Raccourci => "raccourci",
            ErreurInstallation::Inscription => "inscription",
            ErreurInstallation::Lancement => "lancement",
            ErreurInstallation::Dossier => "dossier",
        }
    }

    /// Le texte montré par la page, tel quel.
    pub fn cause(&self) -> String {
        match self {
            ErreurInstallation::Place { lettre, manque } => {
                let disque = match lettre {
                    Some(l) => format!("le disque {l}"),
                    None => "le disque".into(),
                };
                format!(
                    "Il manque de la place sur {disque}. Libérez {} Mo, puis réessayez.",
                    mo_a_liberer(*manque)
                )
            }
            ErreurInstallation::Ouvert => {
                "BreezyShot est encore ouvert. Quittez-le, puis réessayez.".into()
            }
            ErreurInstallation::Ecriture => {
                "Impossible d'écrire dans le dossier de BreezyShot. Réessayez dans un instant."
                    .into()
            }
            ErreurInstallation::Copie => "La copie de BreezyShot est incomplète. Réessayez.".into(),
            ErreurInstallation::Raccourci => {
                "Impossible d'ajouter BreezyShot au menu Démarrer.".into()
            }
            ErreurInstallation::Inscription => {
                "Impossible d'inscrire BreezyShot dans les applications installées.".into()
            }
            ErreurInstallation::Lancement => "BreezyShot est installé mais ne démarre pas.".into(),
            ErreurInstallation::Dossier => {
                "Impossible de trouver le dossier des applications.".into()
            }
        }
    }
}

/// Place nécessaire pour installer un exe de cette taille.
pub fn besoin_octets(taille_exe: u64) -> u64 {
    taille_exe.saturating_add(MARGE_PLACE)
}

/// Mo (2²⁰ octets) à libérer, arrondis aux 10 Mo supérieurs (au moins 10).
pub fn mo_a_liberer(manque: u64) -> u64 {
    let mo = manque.div_ceil(1 << 20).max(1);
    mo.div_ceil(10) * 10
}

/// Assez de place ? Place inconnue : on essaie (la copie dira si elle échoue).
pub fn verifier_place(
    libre: Option<u64>,
    besoin: u64,
    lettre: Option<&str>,
) -> Result<(), ErreurInstallation> {
    match libre {
        Some(l) if l < besoin => Err(ErreurInstallation::Place {
            lettre: lettre.map(str::to_string),
            manque: besoin - l,
        }),
        _ => Ok(()),
    }
}

/// État de l'installateur, vu du cœur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatInstallateur {
    Accueil,
    Installation,
    Erreur,
    Pret,
    Envol,
    /// La fenêtre est cachée ou se ferme : plus rien ne compte.
    Fermeture,
}

impl EtatInstallateur {
    pub fn nom(self) -> &'static str {
        match self {
            EtatInstallateur::Accueil => "accueil",
            EtatInstallateur::Installation => "installation",
            EtatInstallateur::Erreur => "erreur",
            EtatInstallateur::Pret => "pret",
            EtatInstallateur::Envol => "envol",
            EtatInstallateur::Fermeture => "fermeture",
        }
    }
}

/// Ce que la page rapporte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Installer,
    Reessayer,
    Essayer,
    Terminer,
    Fermer,
    Reduire,
}

impl Action {
    pub const TOUTES: [Action; 6] = [
        Action::Installer,
        Action::Reessayer,
        Action::Essayer,
        Action::Terminer,
        Action::Fermer,
        Action::Reduire,
    ];

    pub fn nom(self) -> &'static str {
        match self {
            Action::Installer => "installer",
            Action::Reessayer => "reessayer",
            Action::Essayer => "essayer",
            Action::Terminer => "terminer",
            Action::Fermer => "fermer",
            Action::Reduire => "reduire",
        }
    }

    pub fn depuis_nom(nom: &str) -> Option<Action> {
        Action::TOUTES.into_iter().find(|a| a.nom() == nom)
    }
}

/// Le geste que fait le cœur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Geste {
    Installer,
    Capturer,
    Envol,
    FermerEnFondu,
    /// Fenêtre cachée tout de suite ; l'installation finit, l'app démarre, puis on sort.
    CacherPuisFinir,
    Reduire,
}

/// Table des réactions (contrat §4.4). `None` : action ignorée dans cet état.
pub fn reagir(etat: EtatInstallateur, action: Action) -> Option<(EtatInstallateur, Geste)> {
    use Action as A;
    use EtatInstallateur as E;
    match (etat, action) {
        (E::Envol | E::Fermeture, _) => None,
        (_, A::Reduire) => Some((etat, Geste::Reduire)),
        (E::Accueil, A::Installer) | (E::Erreur, A::Reessayer) => {
            Some((E::Installation, Geste::Installer))
        }
        (E::Accueil | E::Erreur, A::Fermer) => Some((E::Fermeture, Geste::FermerEnFondu)),
        (E::Installation, A::Fermer) => Some((E::Fermeture, Geste::CacherPuisFinir)),
        (E::Pret, A::Essayer) => Some((E::Pret, Geste::Capturer)),
        (E::Pret, A::Terminer | A::Fermer) => Some((E::Envol, Geste::Envol)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordre_des_etapes() {
        let noms: Vec<_> = ETAPES.iter().map(|e| e.nom()).collect();
        assert_eq!(
            noms,
            [
                "place",
                "arret",
                "copie",
                "verification",
                "raccourci",
                "inscription",
                "lancement"
            ]
        );
    }

    #[test]
    fn place() {
        let mo = 1u64 << 20;
        assert_eq!(besoin_octets(15 * mo), 79 * mo);
        assert_eq!(verifier_place(None, 100, None), Ok(()));
        assert_eq!(verifier_place(Some(100), 100, None), Ok(()));
        let e = verifier_place(Some(0), 115 * mo, Some("C:")).unwrap_err();
        assert_eq!(
            e.cause(),
            "Il manque de la place sur le disque C:. Libérez 120 Mo, puis réessayez."
        );
        assert_eq!(e.code(), "place");
        let sans_lettre = ErreurInstallation::Place {
            lettre: None,
            manque: 1,
        };
        assert_eq!(
            sans_lettre.cause(),
            "Il manque de la place sur le disque. Libérez 10 Mo, puis réessayez."
        );
        assert_eq!(mo_a_liberer(10 * mo), 10);
        assert_eq!(mo_a_liberer(10 * mo + 1), 20);
    }

    #[test]
    fn causes_d_une_ligne() {
        for e in [
            ErreurInstallation::Ouvert,
            ErreurInstallation::Ecriture,
            ErreurInstallation::Copie,
            ErreurInstallation::Raccourci,
            ErreurInstallation::Inscription,
            ErreurInstallation::Lancement,
            ErreurInstallation::Dossier,
        ] {
            let c = e.cause();
            assert!(
                !c.contains('\n') && c.ends_with('.') && c.chars().count() < 90,
                "{c}"
            );
        }
    }

    #[test]
    fn table() {
        use Action as A;
        use EtatInstallateur as E;
        assert_eq!(
            reagir(E::Accueil, A::Installer),
            Some((E::Installation, Geste::Installer))
        );
        assert_eq!(
            reagir(E::Erreur, A::Reessayer),
            Some((E::Installation, Geste::Installer))
        );
        assert_eq!(reagir(E::Accueil, A::Reessayer), None);
        assert_eq!(reagir(E::Installation, A::Installer), None);
        assert_eq!(
            reagir(E::Installation, A::Fermer),
            Some((E::Fermeture, Geste::CacherPuisFinir))
        );
        assert_eq!(
            reagir(E::Accueil, A::Fermer),
            Some((E::Fermeture, Geste::FermerEnFondu))
        );
        assert_eq!(
            reagir(E::Erreur, A::Fermer),
            Some((E::Fermeture, Geste::FermerEnFondu))
        );
        assert_eq!(
            reagir(E::Pret, A::Essayer),
            Some((E::Pret, Geste::Capturer))
        );
        assert_eq!(reagir(E::Pret, A::Terminer), Some((E::Envol, Geste::Envol)));
        assert_eq!(reagir(E::Pret, A::Fermer), Some((E::Envol, Geste::Envol)));
        assert_eq!(reagir(E::Accueil, A::Essayer), None);
        assert_eq!(reagir(E::Installation, A::Terminer), None);
        for a in Action::TOUTES {
            assert_eq!(reagir(E::Envol, a), None);
            assert_eq!(reagir(E::Fermeture, a), None);
            assert_eq!(Action::depuis_nom(a.nom()), Some(a));
        }
        assert_eq!(
            reagir(E::Installation, A::Reduire),
            Some((E::Installation, Geste::Reduire))
        );
        assert_eq!(Action::depuis_nom("<img>"), None);
    }
}
