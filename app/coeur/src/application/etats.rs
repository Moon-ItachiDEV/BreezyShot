//! Les huit états d'une capture et la table unique de leurs passages.
//!
//! La table suit le tableau « États » du document de workflow. Ajouter un état ou un
//! passage, c'est ajouter des lignes à `PASSAGES` ; `suivant` ne change pas.

use std::time::{Duration, Instant};

/// L'état de l'app, toujours un seul à la fois.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Etat {
    Repos,
    Gel,
    Selection,
    Decoupe,
    Souffle,
    Vignette,
    /// Retardateur en cours : rien n'est photographié avant la fin du délai (0.4).
    Decompte,
    /// Copier le texte (0.5) : la zone est lue ; toute demande de capture est ignorée.
    Lecture,
}

/// Modes de capture. Ce sont aussi les noms d'action des raccourcis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Zone,
    PleinEcran,
    Fenetre,
    /// Refaire la dernière zone (0.4) : la même zone du même écran, sans sélection.
    DerniereZone,
    /// Copier le texte (0.5) : une zone lue, jamais rangée comme une capture.
    Texte,
}

impl Mode {
    pub const TOUS: [Mode; 5] = [
        Mode::Zone,
        Mode::PleinEcran,
        Mode::Fenetre,
        Mode::DerniereZone,
        Mode::Texte,
    ];

    /// Nom d'action (`zone`, `plein_ecran`, `fenetre`, `derniere_zone`, `texte`).
    pub fn nom(self) -> &'static str {
        match self {
            Mode::Zone => "zone",
            Mode::PleinEcran => "plein_ecran",
            Mode::Fenetre => "fenetre",
            Mode::DerniereZone => "derniere_zone",
            Mode::Texte => "texte",
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
    /// La vignette n'a pas pu se montrer (filet : sa page n'a pas posé la carte à temps).
    VignetteImpossible,
    /// Fin du délai d'annulation de Supprimer : la capture est effacée.
    CaptureSupprimee,
    /// Capture demandée avec le retardateur (0.4).
    DemandeDecompte(Mode),
    /// Fin du délai du retardateur.
    DecompteFini(Mode),
    /// Échap, croix du décompte ou session verrouillée pendant le décompte.
    DecompteAnnule,
    /// Copier le texte : la zone est choisie, la lecture part (0.5).
    TexteChoisi,
    /// La lecture a rendu un texte, ou rien (« Aucun texte trouvé »).
    TexteLu,
    /// Plus de langue, moteur en échec ou échéance de la lecture.
    LectureImpossible,
}

impl Declencheur {
    /// Libellé pour le journal : `DemandeCapture zone`.
    pub fn libelle(&self) -> String {
        match self {
            Declencheur::DemandeCapture(m) => format!("DemandeCapture {}", m.nom()),
            Declencheur::PhotoPrise(m) => format!("PhotoPrise {}", m.nom()),
            Declencheur::DemandeDecompte(m) => format!("DemandeDecompte {}", m.nom()),
            Declencheur::DecompteFini(m) => format!("DecompteFini {}", m.nom()),
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
    (E::Souffle, D::VignetteImpossible, E::Repos),
    // Vignette : un raccourci la ferme et relance une capture.
    (E::Vignette, D::VignetteFermee, E::Repos),
    (E::Vignette, D::DelaiEcoule, E::Repos),
    (E::Vignette, D::GlisserReussi, E::Repos),
    (E::Vignette, D::VignetteImpossible, E::Repos),
    (E::Vignette, D::CaptureSupprimee, E::Repos),
    (E::Vignette, D::DemandeCapture(Mode::Zone), E::Gel),
    (E::Vignette, D::DemandeCapture(Mode::PleinEcran), E::Gel),
    (E::Vignette, D::DemandeCapture(Mode::Fenetre), E::Gel),
    // 0.4 : refaire la dernière zone, sans sélection (Gel → Découpe, comme le plein écran).
    // Dernière zone inutilisable : la suite passe par `PhotoPrise(Zone)` (ligne existante).
    (E::Repos, D::DemandeCapture(Mode::DerniereZone), E::Gel),
    (E::Vignette, D::DemandeCapture(Mode::DerniereZone), E::Gel),
    (E::Gel, D::PhotoPrise(Mode::DerniereZone), E::Decoupe),
    // 0.4 : le retardateur, avant la photo dans tous les modes. La vignette est fermée à
    // l'entrée ; en Décompte, seuls la fin et l'annulation passent.
    (E::Repos, D::DemandeDecompte(Mode::Zone), E::Decompte),
    (E::Repos, D::DemandeDecompte(Mode::PleinEcran), E::Decompte),
    (E::Repos, D::DemandeDecompte(Mode::Fenetre), E::Decompte),
    (
        E::Repos,
        D::DemandeDecompte(Mode::DerniereZone),
        E::Decompte,
    ),
    (E::Vignette, D::DemandeDecompte(Mode::Zone), E::Decompte),
    (
        E::Vignette,
        D::DemandeDecompte(Mode::PleinEcran),
        E::Decompte,
    ),
    (E::Vignette, D::DemandeDecompte(Mode::Fenetre), E::Decompte),
    (
        E::Vignette,
        D::DemandeDecompte(Mode::DerniereZone),
        E::Decompte,
    ),
    (E::Decompte, D::DecompteFini(Mode::Zone), E::Gel),
    (E::Decompte, D::DecompteFini(Mode::PleinEcran), E::Gel),
    (E::Decompte, D::DecompteFini(Mode::Fenetre), E::Gel),
    (E::Decompte, D::DecompteFini(Mode::DerniereZone), E::Gel),
    (E::Decompte, D::DecompteAnnule, E::Repos),
    // 0.5 : Copier le texte, sans retardateur (aucune ligne `DemandeDecompte(Texte)`). Les
    // sorties de la Sélection (annulée, écrans changés, muette…) servent telles quelles.
    (E::Repos, D::DemandeCapture(Mode::Texte), E::Gel),
    (E::Vignette, D::DemandeCapture(Mode::Texte), E::Gel),
    (E::Gel, D::PhotoPrise(Mode::Texte), E::Selection),
    (E::Selection, D::TexteChoisi, E::Lecture),
    (E::Lecture, D::TexteLu, E::Repos),
    (E::Lecture, D::LectureImpossible, E::Repos),
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
    /// Depuis quand l'automate est dans `etat` (journal d'un appui ignoré, veille).
    depuis: Instant,
}

impl Default for Automate {
    fn default() -> Self {
        Automate::dans(Etat::Repos)
    }
}

impl Automate {
    /// Un automate posé dans `etat`, à l'instant.
    pub fn dans(etat: Etat) -> Automate {
        Automate {
            etat,
            depuis: Instant::now(),
        }
    }

    pub fn etat(&self) -> Etat {
        self.etat
    }

    /// Le temps passé dans l'état courant.
    pub fn depuis(&self) -> Duration {
        self.depuis.elapsed()
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
                self.depuis = Instant::now();
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

    const TOUS_LES_ETATS: [Etat; 8] = [
        E::Repos,
        E::Gel,
        E::Selection,
        E::Decoupe,
        E::Souffle,
        E::Vignette,
        E::Decompte,
        E::Lecture,
    ];

    /// Les sept états de la 0.4.
    const ETATS_0_4: [Etat; 7] = [
        E::Repos,
        E::Gel,
        E::Selection,
        E::Decoupe,
        E::Souffle,
        E::Vignette,
        E::Decompte,
    ];

    /// Les quatre modes de la 0.4.
    const MODES_0_4: [Mode; 4] = [
        Mode::Zone,
        Mode::PleinEcran,
        Mode::Fenetre,
        Mode::DerniereZone,
    ];

    /// Copie littérale de la table de la 0.4 (44 lignes) : elle ne doit jamais changer.
    const PASSAGES_0_4: &[(Etat, Declencheur, Etat)] = &[
        (E::Repos, D::DemandeCapture(Mode::Zone), E::Gel),
        (E::Repos, D::DemandeCapture(Mode::PleinEcran), E::Gel),
        (E::Repos, D::DemandeCapture(Mode::Fenetre), E::Gel),
        (E::Gel, D::PhotoPrise(Mode::Zone), E::Selection),
        (E::Gel, D::PhotoPrise(Mode::Fenetre), E::Selection),
        (E::Gel, D::PhotoPrise(Mode::PleinEcran), E::Decoupe),
        (E::Gel, D::GelImpossible, E::Repos),
        (E::Selection, D::ZoneChoisie, E::Decoupe),
        (E::Selection, D::SelectionAnnulee, E::Repos),
        (E::Selection, D::EcransChanges, E::Repos),
        (E::Selection, D::SessionVerrouillee, E::Repos),
        (E::Selection, D::BureauSecurise, E::Repos),
        (E::Selection, D::SelectionMuette, E::Repos),
        (E::Selection, D::SelectionImpossible, E::Repos),
        (E::Decoupe, D::DecoupeImpossible, E::Repos),
        (E::Decoupe, D::SortieSansBilan, E::Repos),
        (E::Decoupe, D::ImagePrete, E::Souffle),
        (E::Decoupe, D::ImagePreteSansVignette, E::Repos),
        (E::Souffle, D::SouffleTermine, E::Vignette),
        (E::Souffle, D::VignetteImpossible, E::Repos),
        (E::Vignette, D::VignetteFermee, E::Repos),
        (E::Vignette, D::DelaiEcoule, E::Repos),
        (E::Vignette, D::GlisserReussi, E::Repos),
        (E::Vignette, D::VignetteImpossible, E::Repos),
        (E::Vignette, D::CaptureSupprimee, E::Repos),
        (E::Vignette, D::DemandeCapture(Mode::Zone), E::Gel),
        (E::Vignette, D::DemandeCapture(Mode::PleinEcran), E::Gel),
        (E::Vignette, D::DemandeCapture(Mode::Fenetre), E::Gel),
        (E::Repos, D::DemandeCapture(Mode::DerniereZone), E::Gel),
        (E::Vignette, D::DemandeCapture(Mode::DerniereZone), E::Gel),
        (E::Gel, D::PhotoPrise(Mode::DerniereZone), E::Decoupe),
        (E::Repos, D::DemandeDecompte(Mode::Zone), E::Decompte),
        (E::Repos, D::DemandeDecompte(Mode::PleinEcran), E::Decompte),
        (E::Repos, D::DemandeDecompte(Mode::Fenetre), E::Decompte),
        (
            E::Repos,
            D::DemandeDecompte(Mode::DerniereZone),
            E::Decompte,
        ),
        (E::Vignette, D::DemandeDecompte(Mode::Zone), E::Decompte),
        (
            E::Vignette,
            D::DemandeDecompte(Mode::PleinEcran),
            E::Decompte,
        ),
        (E::Vignette, D::DemandeDecompte(Mode::Fenetre), E::Decompte),
        (
            E::Vignette,
            D::DemandeDecompte(Mode::DerniereZone),
            E::Decompte,
        ),
        (E::Decompte, D::DecompteFini(Mode::Zone), E::Gel),
        (E::Decompte, D::DecompteFini(Mode::PleinEcran), E::Gel),
        (E::Decompte, D::DecompteFini(Mode::Fenetre), E::Gel),
        (E::Decompte, D::DecompteFini(Mode::DerniereZone), E::Gel),
        (E::Decompte, D::DecompteAnnule, E::Repos),
    ];

    /// Les six lignes de la 0.5.
    const LIGNES_0_5: &[(Etat, Declencheur, Etat)] = &[
        (E::Repos, D::DemandeCapture(Mode::Texte), E::Gel),
        (E::Vignette, D::DemandeCapture(Mode::Texte), E::Gel),
        (E::Gel, D::PhotoPrise(Mode::Texte), E::Selection),
        (E::Selection, D::TexteChoisi, E::Lecture),
        (E::Lecture, D::TexteLu, E::Repos),
        (E::Lecture, D::LectureImpossible, E::Repos),
    ];

    /// Les six états de la 0.3.
    const ETATS_0_3: [Etat; 6] = [
        E::Repos,
        E::Gel,
        E::Selection,
        E::Decoupe,
        E::Souffle,
        E::Vignette,
    ];

    /// Les trois modes de la 0.3.
    const MODES_0_3: [Mode; 3] = [Mode::Zone, Mode::PleinEcran, Mode::Fenetre];

    /// Copie littérale de la table de la 0.3 (28 lignes) : elle ne doit jamais changer.
    const PASSAGES_0_3: &[(Etat, Declencheur, Etat)] = &[
        (E::Repos, D::DemandeCapture(Mode::Zone), E::Gel),
        (E::Repos, D::DemandeCapture(Mode::PleinEcran), E::Gel),
        (E::Repos, D::DemandeCapture(Mode::Fenetre), E::Gel),
        (E::Gel, D::PhotoPrise(Mode::Zone), E::Selection),
        (E::Gel, D::PhotoPrise(Mode::Fenetre), E::Selection),
        (E::Gel, D::PhotoPrise(Mode::PleinEcran), E::Decoupe),
        (E::Gel, D::GelImpossible, E::Repos),
        (E::Selection, D::ZoneChoisie, E::Decoupe),
        (E::Selection, D::SelectionAnnulee, E::Repos),
        (E::Selection, D::EcransChanges, E::Repos),
        (E::Selection, D::SessionVerrouillee, E::Repos),
        (E::Selection, D::BureauSecurise, E::Repos),
        (E::Selection, D::SelectionMuette, E::Repos),
        (E::Selection, D::SelectionImpossible, E::Repos),
        (E::Decoupe, D::DecoupeImpossible, E::Repos),
        (E::Decoupe, D::SortieSansBilan, E::Repos),
        (E::Decoupe, D::ImagePrete, E::Souffle),
        (E::Decoupe, D::ImagePreteSansVignette, E::Repos),
        (E::Souffle, D::SouffleTermine, E::Vignette),
        (E::Souffle, D::VignetteImpossible, E::Repos),
        (E::Vignette, D::VignetteFermee, E::Repos),
        (E::Vignette, D::DelaiEcoule, E::Repos),
        (E::Vignette, D::GlisserReussi, E::Repos),
        (E::Vignette, D::VignetteImpossible, E::Repos),
        (E::Vignette, D::CaptureSupprimee, E::Repos),
        (E::Vignette, D::DemandeCapture(Mode::Zone), E::Gel),
        (E::Vignette, D::DemandeCapture(Mode::PleinEcran), E::Gel),
        (E::Vignette, D::DemandeCapture(Mode::Fenetre), E::Gel),
    ];

    /// Les déclencheurs sans mode de la 0.3.
    const SANS_MODE_0_3: [Declencheur; 18] = [
        D::GelImpossible,
        D::ZoneChoisie,
        D::SelectionAnnulee,
        D::EcransChanges,
        D::SessionVerrouillee,
        D::BureauSecurise,
        D::SelectionMuette,
        D::SelectionImpossible,
        D::DecoupeImpossible,
        D::SortieSansBilan,
        D::ImagePrete,
        D::ImagePreteSansVignette,
        D::SouffleTermine,
        D::VignetteFermee,
        D::DelaiEcoule,
        D::GlisserReussi,
        D::VignetteImpossible,
        D::CaptureSupprimee,
    ];

    /// Tous les déclencheurs de la 0.3 (les trois modes de la 0.3).
    fn declencheurs_0_3() -> Vec<Declencheur> {
        let mut v: Vec<Declencheur> = SANS_MODE_0_3.to_vec();
        for m in MODES_0_3 {
            v.push(D::DemandeCapture(m));
            v.push(D::PhotoPrise(m));
        }
        v
    }

    /// Tous les déclencheurs de la 0.4, chaque mode de la 0.4 compris.
    fn tous_les_declencheurs() -> Vec<Declencheur> {
        let mut v: Vec<Declencheur> = SANS_MODE_0_3.to_vec();
        v.push(D::DecompteAnnule);
        for m in MODES_0_4 {
            v.extend([
                D::DemandeCapture(m),
                D::PhotoPrise(m),
                D::DemandeDecompte(m),
                D::DecompteFini(m),
            ]);
        }
        v
    }

    fn suivant_0_3(etat: Etat, d: Declencheur) -> Option<Etat> {
        PASSAGES_0_3
            .iter()
            .find(|(e, x, _)| *e == etat && *x == d)
            .map(|(_, _, a)| *a)
    }

    fn porte_derniere_zone(d: &Declencheur) -> bool {
        matches!(
            d,
            D::DemandeCapture(Mode::DerniereZone)
                | D::PhotoPrise(Mode::DerniereZone)
                | D::DemandeDecompte(Mode::DerniereZone)
                | D::DecompteFini(Mode::DerniereZone)
        )
    }

    #[test]
    fn table_0_3_incluse() {
        assert_eq!(PASSAGES_0_3.len(), 28);
        for ligne in PASSAGES_0_3 {
            assert!(
                PASSAGES.contains(ligne),
                "ligne de la 0.3 perdue : {ligne:?}"
            );
        }
    }

    #[test]
    fn lignes_de_la_0_4_sur_la_0_3() {
        for ligne @ (depart, d, arrivee) in PASSAGES_0_4 {
            if PASSAGES_0_3.contains(ligne) {
                continue;
            }
            assert!(
                *depart == E::Decompte || *arrivee == E::Decompte || porte_derniere_zone(d),
                "ligne nouvelle sans Décompte ni dernière zone : {ligne:?}"
            );
        }
    }

    #[test]
    fn six_etats_inchanges() {
        for etat in ETATS_0_3 {
            for d in declencheurs_0_3() {
                assert_eq!(suivant(etat, d), suivant_0_3(etat, d), "{etat:?} {d:?}");
            }
        }
    }

    #[test]
    fn table_0_4_incluse() {
        assert_eq!(PASSAGES_0_4.len(), 44);
        for ligne in PASSAGES_0_4 {
            assert!(
                PASSAGES.contains(ligne),
                "ligne de la 0.4 perdue : {ligne:?}"
            );
        }
    }

    /// La 0.5 n'ajoute que ses six lignes : aucune retirée, aucune changée.
    #[test]
    fn seulement_des_lignes_nouvelles() {
        assert_eq!(PASSAGES.len(), PASSAGES_0_4.len() + LIGNES_0_5.len());
        for ligne in PASSAGES {
            assert!(
                PASSAGES_0_4.contains(ligne) || LIGNES_0_5.contains(ligne),
                "ligne inattendue : {ligne:?}"
            );
        }
        for ligne in LIGNES_0_5 {
            assert!(
                PASSAGES.contains(ligne),
                "ligne de la 0.5 absente : {ligne:?}"
            );
        }
        // Les sept états de la 0.4 font ce qu'ils faisaient pour chaque déclencheur de la 0.4.
        for etat in ETATS_0_4 {
            for d in tous_les_declencheurs() {
                let avant = PASSAGES_0_4
                    .iter()
                    .find(|(e, x, _)| *e == etat && *x == d)
                    .map(|(_, _, a)| *a);
                assert_eq!(suivant(etat, d), avant, "{etat:?} {d:?}");
            }
        }
    }

    /// Pendant la lecture, toute demande de capture (et de décompte) est ignorée ; seules
    /// ses deux fins passent.
    #[test]
    fn lecture_ignore_les_captures() {
        for m in Mode::TOUS {
            for d in [
                D::DemandeCapture(m),
                D::DemandeDecompte(m),
                D::PhotoPrise(m),
                D::DecompteFini(m),
            ] {
                assert_eq!(suivant(E::Lecture, d), None, "{d:?}");
            }
        }
        for d in tous_les_declencheurs() {
            assert_eq!(suivant(E::Lecture, d), None, "{d:?}");
        }
        assert_eq!(suivant(E::Lecture, D::TexteLu), Some(E::Repos));
        assert_eq!(suivant(E::Lecture, D::LectureImpossible), Some(E::Repos));
        let mut a = Automate::default();
        for d in [
            D::DemandeCapture(Mode::Texte),
            D::PhotoPrise(Mode::Texte),
            D::TexteChoisi,
        ] {
            assert!(a.traiter(d).is_some(), "{d:?}");
        }
        assert_eq!(a.etat(), E::Lecture);
        assert_eq!(a.traiter(D::DemandeCapture(Mode::Zone)), None);
        assert_eq!(a.traiter(D::TexteLu), Some(E::Repos));
        // Les sorties de la Sélection valent aussi en mode texte.
        a.traiter(D::DemandeCapture(Mode::Texte));
        a.traiter(D::PhotoPrise(Mode::Texte));
        assert_eq!(a.traiter(D::SelectionAnnulee), Some(E::Repos));
        // Hors de la Sélection et de la Lecture, les déclencheurs du texte sont ignorés.
        for etat in ETATS_0_4 {
            for d in [D::TexteChoisi, D::TexteLu, D::LectureImpossible] {
                let attendu = (etat == E::Selection && d == D::TexteChoisi).then_some(E::Lecture);
                assert_eq!(suivant(etat, d), attendu, "{etat:?} {d:?}");
            }
        }
    }

    /// Copier le texte n'a jamais de retardateur.
    #[test]
    fn texte_sans_decompte() {
        for etat in TOUS_LES_ETATS {
            assert_eq!(suivant(etat, D::DemandeDecompte(Mode::Texte)), None);
            assert_eq!(suivant(etat, D::DecompteFini(Mode::Texte)), None);
        }
        assert_eq!(
            D::DemandeCapture(Mode::Texte).libelle(),
            "DemandeCapture texte"
        );
        assert_eq!(Mode::depuis_nom("texte"), Some(Mode::Texte));
        assert_eq!(D::TexteChoisi.libelle(), "TexteChoisi");
    }

    /// L'historique n'est pas un état (comme Réglages et l'éditeur) : aucun état, aucun
    /// déclencheur, aucun mode ne le porte.
    #[test]
    fn historique_hors_automate() {
        for etat in TOUS_LES_ETATS {
            assert!(!format!("{etat:?}").contains("Historique"));
        }
        for m in Mode::TOUS {
            assert_ne!(m.nom(), "historique");
        }
        assert_eq!(Mode::depuis_nom("historique"), None);
        for (_, d, _) in PASSAGES {
            assert!(!d.libelle().contains("istorique"), "{d:?}");
        }
    }

    #[test]
    fn decompte_n_accepte_que_ses_sorties() {
        for d in tous_les_declencheurs() {
            let attendu = match d {
                D::DecompteFini(_) => Some(E::Gel),
                D::DecompteAnnule => Some(E::Repos),
                _ => None,
            };
            assert_eq!(suivant(E::Decompte, d), attendu, "{d:?}");
        }
    }

    #[test]
    fn parcours_avec_retardateur() {
        for depart in [E::Repos, E::Vignette] {
            for m in MODES_0_4 {
                let mut a = Automate::dans(depart);
                assert_eq!(a.traiter(D::DemandeDecompte(m)), Some(E::Decompte));
                // Un second appui pendant le décompte est ignoré.
                assert_eq!(a.traiter(D::DemandeCapture(m)), None);
                assert_eq!(a.traiter(D::DemandeDecompte(m)), None);
                assert_eq!(a.traiter(D::DecompteFini(m)), Some(E::Gel));
                match m {
                    Mode::Zone | Mode::Fenetre => {
                        assert_eq!(a.traiter(D::PhotoPrise(m)), Some(E::Selection));
                        assert_eq!(a.traiter(D::ZoneChoisie), Some(E::Decoupe));
                    }
                    Mode::PleinEcran | Mode::DerniereZone => {
                        assert_eq!(a.traiter(D::PhotoPrise(m)), Some(E::Decoupe));
                    }
                    Mode::Texte => unreachable!("pas de retardateur pour le texte"),
                }
                assert_eq!(a.traiter(D::ImagePreteSansVignette), Some(E::Repos));
            }
        }
        // Annulé : retour au repos ; le souffle ignore le retardateur.
        let mut a = Automate::default();
        a.traiter(D::DemandeDecompte(Mode::Zone));
        assert_eq!(a.traiter(D::DecompteAnnule), Some(E::Repos));
        for m in Mode::TOUS {
            assert_eq!(suivant(E::Souffle, D::DemandeDecompte(m)), None);
        }
        // Dernière zone inutilisable : la photo sert à la sélection de zone.
        let mut a = Automate::default();
        a.traiter(D::DemandeCapture(Mode::DerniereZone));
        assert_eq!(a.traiter(D::PhotoPrise(Mode::Zone)), Some(E::Selection));
    }

    #[test]
    fn libelles_du_decompte() {
        assert_eq!(
            D::DemandeDecompte(Mode::Zone).libelle(),
            "DemandeDecompte zone"
        );
        assert_eq!(
            D::DecompteFini(Mode::PleinEcran).libelle(),
            "DecompteFini plein_ecran"
        );
        assert_eq!(D::DecompteAnnule.libelle(), "DecompteAnnule");
        assert_eq!(
            D::PhotoPrise(Mode::DerniereZone).libelle(),
            "PhotoPrise derniere_zone"
        );
        assert_eq!(Mode::depuis_nom("derniere_zone"), Some(Mode::DerniereZone));
    }

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

    /// Plein écran de bout en bout (étape 7) : Repos → Gel → Découpe → Repos, sans
    /// Sélection ; un appui (tout mode) pendant Gel ou Découpe est ignoré ; après, une
    /// capture repart.
    #[test]
    fn parcours_plein_ecran_et_appui_ignore() {
        let mut a = Automate::default();
        assert_eq!(a.traiter(D::DemandeCapture(Mode::PleinEcran)), Some(E::Gel));
        for m in Mode::TOUS {
            assert_eq!(a.traiter(D::DemandeCapture(m)), None);
        }
        assert_eq!(a.traiter(D::PhotoPrise(Mode::PleinEcran)), Some(E::Decoupe));
        for m in Mode::TOUS {
            assert_eq!(a.traiter(D::DemandeCapture(m)), None, "{m:?} en Découpe");
        }
        assert_eq!(a.etat(), E::Decoupe);
        assert_eq!(a.traiter(D::ImagePreteSansVignette), Some(E::Repos));
        assert_eq!(a.traiter(D::DemandeCapture(Mode::PleinEcran)), Some(E::Gel));
        // Erreurs : pas d'écran ou photo refusée, pixels illisibles, sortie muette.
        assert_eq!(a.traiter(D::GelImpossible), Some(E::Repos));
        a.traiter(D::DemandeCapture(Mode::PleinEcran));
        a.traiter(D::PhotoPrise(Mode::PleinEcran));
        assert_eq!(a.traiter(D::DecoupeImpossible), Some(E::Repos));
        a.traiter(D::DemandeCapture(Mode::PleinEcran));
        a.traiter(D::PhotoPrise(Mode::PleinEcran));
        assert_eq!(a.traiter(D::SortieSansBilan), Some(E::Repos));
        assert_eq!(
            D::PhotoPrise(Mode::PleinEcran).libelle(),
            "PhotoPrise plein_ecran"
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
        for d in [
            D::VignetteFermee,
            D::DelaiEcoule,
            D::GlisserReussi,
            D::VignetteImpossible,
            D::CaptureSupprimee,
        ] {
            assert_eq!(suivant(E::Vignette, d), Some(E::Repos));
        }
        // Filet pendant le souffle ; Supprimer n'existe qu'une fois la carte montrée.
        assert_eq!(suivant(E::Souffle, D::VignetteImpossible), Some(E::Repos));
        assert_eq!(suivant(E::Souffle, D::CaptureSupprimee), None);
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
