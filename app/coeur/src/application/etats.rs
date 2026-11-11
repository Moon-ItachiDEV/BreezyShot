//! Les six états d'une capture et la table unique de leurs passages.
//!
//! La table suit le tableau « États » du document de workflow. Ajouter un état ou un
//! passage, c'est ajouter des lignes à `PASSAGES` ; `suivant` ne change pas.

/// L'état de l'app, toujours un seul à la fois.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Etat {
    Repos,
    Gel,
    Selection,
    Decoupe,
    Souffle,
    Vignette,
}

/// Modes de capture. Ce sont aussi les noms d'action des raccourcis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Zone,
    PleinEcran,
    Fenetre,
}

impl Mode {
    pub const TOUS: [Mode; 3] = [Mode::Zone, Mode::PleinEcran, Mode::Fenetre];

    /// Nom d'action (`zone`, `plein_ecran`, `fenetre`).
    pub fn nom(self) -> &'static str {
        match self {
            Mode::Zone => "zone",
            Mode::PleinEcran => "plein_ecran",
            Mode::Fenetre => "fenetre",
        }
    }

    pub fn depuis_nom(nom: &str) -> Option<Mode> {
        Mode::TOUS.into_iter().find(|m| m.nom() == nom)
    }
}

/// Ce qui peut faire changer d'état.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Declencheur {
    /// Raccourci, menu de l'icône ou barre de capture.
    DemandeCapture(Mode),
    /// Photo des écrans prise, pour le mode demandé.
    PhotoPrise(Mode),
    /// Windows refuse la photo.
    GelImpossible,
    /// Souris relâchée ou clic sur une fenêtre.
    ZoneChoisie,
    /// Échap ou clic droit.
    SelectionAnnulee,
    /// Écran branché ou débranché pendant la sélection.
    EcransChanges,
    /// PC verrouillé pendant la sélection.
    SessionVerrouillee,
    /// Demande d'autorisation administrateur (bureau sécurisé).
    BureauSecurise,
    /// La sélection ne donne plus signe de vie depuis 2 s.
    SelectionMuette,
    /// Erreur pendant la sélection.
    SelectionImpossible,
    /// Les pixels de la zone n'ont pas pu être lus.
    DecoupeImpossible,
    /// La sortie n'a rendu aucun bilan dans son délai.
    SortieSansBilan,
    /// Image découpée, vignette active : on joue le souffle.
    ImagePrete,
    /// Image découpée, vignette désactivée : retour au repos après confirmation.
    ImagePreteSansVignette,
    /// Fin de l'animation du souffle.
    SouffleTermine,
    /// Bouton Fermer de la vignette.
    VignetteFermee,
    /// 6 s sans rien faire sur la vignette.
    DelaiEcoule,
    /// Glisser-déposer réussi depuis la vignette.
    GlisserReussi,
}

impl Declencheur {
    /// Libellé pour le journal : `DemandeCapture zone`.
    pub fn libelle(&self) -> String {
        match self {
            Declencheur::DemandeCapture(m) => format!("DemandeCapture {}", m.nom()),
            Declencheur::PhotoPrise(m) => format!("PhotoPrise {}", m.nom()),
            autre => format!("{autre:?}"),
        }
    }
}

use Declencheur as D;
use Etat as E;

/// Table unique des passages `(état, déclencheur) -> nouvel état`.
/// Un couple absent est un déclencheur ignoré dans cet état.
pub const PASSAGES: &[(Etat, Declencheur, Etat)] = &[
    // Repos : un raccourci lance la capture.
    (E::Repos, D::DemandeCapture(Mode::Zone), E::Gel),
    (E::Repos, D::DemandeCapture(Mode::PleinEcran), E::Gel),
    (E::Repos, D::DemandeCapture(Mode::Fenetre), E::Gel),
    // Gel : vers la sélection, ou directement la découpe en plein écran.
    (E::Gel, D::PhotoPrise(Mode::Zone), E::Selection),
    (E::Gel, D::PhotoPrise(Mode::Fenetre), E::Selection),
    (E::Gel, D::PhotoPrise(Mode::PleinEcran), E::Decoupe),
    (E::Gel, D::GelImpossible, E::Repos),
    // Sélection.
    (E::Selection, D::ZoneChoisie, E::Decoupe),
    (E::Selection, D::SelectionAnnulee, E::Repos),
    (E::Selection, D::EcransChanges, E::Repos),
    (E::Selection, D::SessionVerrouillee, E::Repos),
    (E::Selection, D::BureauSecurise, E::Repos),
    (E::Selection, D::SelectionMuette, E::Repos),
    (E::Selection, D::SelectionImpossible, E::Repos),
    // Découpe : une erreur de copie ou d'écriture n'arrête rien (pas de ligne ici) ; seule
    // une lecture des pixels impossible ramène au repos sans image.
    (E::Decoupe, D::DecoupeImpossible, E::Repos),
    (E::Decoupe, D::SortieSansBilan, E::Repos),
    (E::Decoupe, D::ImagePrete, E::Souffle),
    (E::Decoupe, D::ImagePreteSansVignette, E::Repos),
    // Souffle.
    (E::Souffle, D::SouffleTermine, E::Vignette),
    // Vignette : un raccourci la ferme et relance une capture.
    (E::Vignette, D::VignetteFermee, E::Repos),
    (E::Vignette, D::DelaiEcoule, E::Repos),
    (E::Vignette, D::GlisserReussi, E::Repos),
    (E::Vignette, D::DemandeCapture(Mode::Zone), E::Gel),
    (E::Vignette, D::DemandeCapture(Mode::PleinEcran), E::Gel),
    (E::Vignette, D::DemandeCapture(Mode::Fenetre), E::Gel),
];

/// Fonction pure : l'état suivant, ou `None` si le déclencheur est ignoré dans cet état.
pub fn suivant(etat: Etat, declencheur: Declencheur) -> Option<Etat> {
    PASSAGES
        .iter()
        .find(|(depart, d, _)| *depart == etat && *d == declencheur)
        .map(|(_, _, arrivee)| *arrivee)
}

/// L'état courant, tenu par l'Application. Chaque changement écrit une ligne de journal.
#[derive(Debug)]
pub struct Automate {
    etat: Etat,
}

impl Default for Automate {
    fn default() -> Self {
        Automate { etat: Etat::Repos }
    }
}

impl Automate {
    pub fn etat(&self) -> Etat {
        self.etat
    }

    /// Applique un déclencheur. Rend le nouvel état, ou `None` s'il est ignoré.
    pub fn traiter(&mut self, declencheur: Declencheur) -> Option<Etat> {
        match suivant(self.etat, declencheur) {
            Some(nouveau) => {
                log::info!(
                    "etat {:?} -> {:?} ({})",
                    self.etat,
                    nouveau,
                    declencheur.libelle()
                );
                self.etat = nouveau;
                Some(nouveau)
            }
            None => {
                log::debug!("etat {:?}: {} ignore", self.etat, declencheur.libelle());
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOUS_LES_ETATS: [Etat; 6] = [
        E::Repos,
        E::Gel,
        E::Selection,
        E::Decoupe,
        E::Souffle,
        E::Vignette,
    ];

    #[test]
    fn parcours_normal_d_une_zone() {
        let mut e = E::Repos;
        for d in [
            D::DemandeCapture(Mode::Zone),
            D::PhotoPrise(Mode::Zone),
            D::ZoneChoisie,
            D::ImagePrete,
            D::SouffleTermine,
            D::DelaiEcoule,
        ] {
            e = suivant(e, d).unwrap();
        }
        assert_eq!(e, E::Repos);
    }

    #[test]
    fn plein_ecran_saute_la_selection() {
        assert_eq!(
            suivant(E::Gel, D::PhotoPrise(Mode::PleinEcran)),
            Some(E::Decoupe)
        );
    }

    #[test]
    fn sans_capture_le_gel_redescend_au_repos() {
        let mut a = Automate::default();
        assert_eq!(a.traiter(D::DemandeCapture(Mode::Zone)), Some(E::Gel));
        assert_eq!(a.traiter(D::GelImpossible), Some(E::Repos));
        assert_eq!(a.etat(), E::Repos);
    }

    #[test]
    fn raccourci_ignore_hors_repos_et_vignette() {
        for etat in TOUS_LES_ETATS {
            for mode in Mode::TOUS {
                let r = suivant(etat, D::DemandeCapture(mode));
                match etat {
                    E::Repos | E::Vignette => assert_eq!(r, Some(E::Gel)),
                    _ => assert_eq!(r, None, "{etat:?} doit ignorer {mode:?}"),
                }
            }
        }
    }

    #[test]
    fn toute_annulation_de_selection_ramene_au_repos() {
        for d in [
            D::SelectionAnnulee,
            D::EcransChanges,
            D::SessionVerrouillee,
            D::BureauSecurise,
            D::SelectionMuette,
            D::SelectionImpossible,
        ] {
            assert_eq!(suivant(E::Selection, d), Some(E::Repos));
        }
    }

    /// Contrat d'assemblage, point 1 : un appui pendant Gel, Sélection ou Découpe ne
    /// relance rien (aucun second `figer`).
    #[test]
    fn appui_ignore_pendant_une_capture() {
        let mut a = Automate::default();
        a.traiter(D::DemandeCapture(Mode::Zone));
        assert_eq!(a.traiter(D::DemandeCapture(Mode::Zone)), None);
        a.traiter(D::PhotoPrise(Mode::Zone));
        assert_eq!(a.etat(), E::Selection);
        assert_eq!(a.traiter(D::DemandeCapture(Mode::Zone)), None);
        a.traiter(D::ZoneChoisie);
        assert_eq!(a.etat(), E::Decoupe);
        assert_eq!(a.traiter(D::DemandeCapture(Mode::Zone)), None);
        assert_eq!(a.traiter(D::ImagePreteSansVignette), Some(E::Repos));
        assert_eq!(a.traiter(D::DemandeCapture(Mode::Zone)), Some(E::Gel));
    }

    #[test]
    fn decoupe_impossible_revient_au_repos() {
        assert_eq!(suivant(E::Decoupe, D::DecoupeImpossible), Some(E::Repos));
        assert_eq!(suivant(E::Decoupe, D::SortieSansBilan), Some(E::Repos));
    }

    #[test]
    fn vignette_desactivee_revient_au_repos() {
        assert_eq!(
            suivant(E::Decoupe, D::ImagePreteSansVignette),
            Some(E::Repos)
        );
    }

    #[test]
    fn sorties_de_la_vignette() {
        for d in [D::VignetteFermee, D::DelaiEcoule, D::GlisserReussi] {
            assert_eq!(suivant(E::Vignette, d), Some(E::Repos));
        }
    }

    #[test]
    fn table_sans_doublon_et_chaque_etat_a_une_sortie() {
        for (i, (e1, d1, _)) in PASSAGES.iter().enumerate() {
            for (e2, d2, _) in &PASSAGES[i + 1..] {
                assert!(!(e1 == e2 && d1 == d2), "doublon {e1:?} {d1:?}");
            }
        }
        for etat in TOUS_LES_ETATS {
            assert!(
                PASSAGES.iter().any(|(e, _, _)| *e == etat),
                "{etat:?} sans sortie"
            );
        }
    }

    #[test]
    fn libelles_et_noms_de_mode() {
        assert_eq!(
            D::DemandeCapture(Mode::Zone).libelle(),
            "DemandeCapture zone"
        );
        assert_eq!(D::GelImpossible.libelle(), "GelImpossible");
        for m in Mode::TOUS {
            assert_eq!(Mode::depuis_nom(m.nom()), Some(m));
        }
        assert_eq!(Mode::depuis_nom("autre"), None);
    }
}
