//! Capture défilante (contrat 0.6) : les décisions du cœur, en fonctions pures (§4.2, §5,
//! §8, §10). `geste_defilante.rs` fait les gestes (fils, fenêtre de contrôle, sortie) ;
//! `fenetre_defilante.rs` reçoit les quatre commandes de la page de contrôle.
//!
//! Les sous-états du défilement (`Conduite`, `perdue`) ne sont pas des états de
//! l'automate : ils ne changent rien à ce que les autres demandes peuvent faire.
//!
//! Socle : constantes et signatures ; corps inertes (chantier D).

use std::time::{Duration, Instant};

pub use crate::defilement::RECOUVREMENT_MIN;
use crate::defilement::{Limite, Mesure};
use crate::systeme::defiler::Position;
use crate::systeme::gel::EcranInfo;
use crate::systeme::Rect;

/// Vrai quand la capture défilante est construite (chantier D) : avant, elle n'est jamais
/// une action construite (ni menu, ni raccourci, ni `--capturer`), même avec une licence.
pub const CONSTRUITE: bool = false;

/// Côté minimal d'une zone, en pixels logiques : une zone plus petite est agrandie autour
/// de son centre (D3) ; une page proposée plus petite n'est pas proposée (D2).
pub const COTE_MIN_LOGIQUE: f64 = 120.0;

/// Au plus tant de pages proposées à la Sélection (D2).
pub const PAGES_MAX: usize = 32;

/// Attente après la première image avant le premier pas automatique (valeur du design).
pub const DEPART_AUTO: Duration = Duration::from_millis(450);

/// Arrêt en bas de la page avant la fin seule (valeur du design, D15).
pub const FIN_EN_BAS: Duration = Duration::from_millis(350);

/// Intervalle des prises une fois la main reprise à la molette (D9).
pub const PRISE_MAIN: Duration = Duration::from_millis(50);

/// Intervalle des prises après un pas, jusqu'à deux images égales (D6).
pub const PRISE_POSEE: Duration = Duration::from_millis(33);

/// Attente maximale d'une page posée après un pas (D6, S5).
pub const ATTENTE_POSEE_MAX: Duration = Duration::from_millis(400);

/// Pas minimal, en pixels réels (D6).
pub const PAS_MIN: i32 = 40;

/// Échéance globale d'une défilante (D20).
pub const DUREE_MAX: Duration = Duration::from_secs(10 * 60);

/// En `Main`, sans aucune image nouvelle pendant ce délai, la capture finit (D20).
pub const MAIN_SANS_IMAGE_MAX: Duration = Duration::from_secs(2 * 60);

/// Sans page de contrôle prête dans ce délai après le relâchement, abandon (D4, R1).
pub const DELAI_PAGE: Duration = Duration::from_secs(2);

/// Texte de la pilule de la page de contrôle.
pub const LIBELLE_TERMINER: &str = "Terminer";

/// Message « trop vite » (D10), montré en haut de l'écran.
pub const TEXTE_TROP_VITE: &str = "Trop vite. Remontez un peu.";

/// Titre du bandeau quand la page de contrôle n'est pas prête à temps (R1, §18).
pub const TITRE_LANCEMENT_IMPOSSIBLE: &str = "Impossible de lancer la capture défilante";

/// La zone `zone` (pixels réels) portée à `round(COTE_MIN_LOGIQUE × échelle)` sur chaque
/// côté trop petit, autour de son centre, sans sortir de `ecran` (D3). Rend la zone et si
/// elle a été agrandie.
pub fn agrandir(zone: Rect, ecran: &EcranInfo) -> (Rect, bool) {
    let _ = ecran;
    (zone, false)
}

/// Où poser l'aperçu en direct, la pilule « Terminer » et le message, en pixels réels du
/// bureau (§10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Placement {
    /// L'aperçu : son rectangle, la hauteur étant la hauteur maximale.
    pub apercu: Rect,
    /// Le point haut gauche de la pilule.
    pub pilule: (i32, i32),
    /// Le point haut centre du message.
    pub message: (i32, i32),
}

/// Placement de l'aperçu, de la pilule et du message autour de `zone` sur `ecran` (§10,
/// pur, pixels réels).
pub fn placer(zone: Rect, ecran: &EcranInfo) -> Placement {
    let _ = (zone, ecran);
    Placement::default()
}

/// La taille maximale de l'image assemblée pour une zone de `largeur` pixels (D13) :
/// celle de l'éditeur (`editeur::COTE_MAX`, `editeur::PIXELS_MAX`).
pub fn limite(largeur: u32) -> Limite {
    let _ = largeur;
    Limite {
        hauteur: 0,
        pixels: 0,
    }
}

/// Qui fait défiler la page (§4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conduite {
    /// `DEPART_AUTO` après la première image.
    Attente,
    /// L'app défile.
    Auto,
    /// Un cran de l'utilisateur est arrivé : définitif pour cette capture.
    Main,
}

/// Comment une défilante finit (D14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fin {
    Bouton,
    Entree,
    EnBas,
    Limite,
}

impl Fin {
    /// Nom écrit au journal : `bouton`, `entree`, `en_bas`, `limite`.
    pub fn nom(self) -> &'static str {
        match self {
            Fin::Bouton => "bouton",
            Fin::Entree => "entree",
            Fin::EnBas => "en_bas",
            Fin::Limite => "limite",
        }
    }
}

/// La course d'une capture : conduite, « trop vite », pas, moyens essayés, fin rendue.
#[derive(Debug, Clone, PartialEq)]
pub struct Course {
    conduite: Conduite,
    perdue: bool,
    fin: Option<Fin>,
}

impl Course {
    /// Une course qui commence à `debut` (la première image).
    pub fn nouvelle(debut: Instant) -> Course {
        let _ = debut;
        Course {
            conduite: Conduite::Attente,
            perdue: false,
            fin: None,
        }
    }

    pub fn conduite(&self) -> Conduite {
        self.conduite
    }

    /// Vrai pendant que « Trop vite » est montré.
    pub fn perdue(&self) -> bool {
        self.perdue
    }

    /// La fin rendue, s'il y en a une (une seule par capture).
    pub fn fin(&self) -> Option<Fin> {
        self.fin
    }
}

/// Ce que le fil `defilante` voit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Vu {
    /// Une image mesurée, et la position lue s'il y en a une.
    Image(Mesure, Option<Position>),
    /// Un cran de l'utilisateur ; `dans_zone` : le pointeur était dans la zone.
    Molette { dans_zone: bool },
    /// « Terminer ».
    Terminer,
    /// Entrée globale.
    Entree,
    /// Le temps passe.
    Temps(Instant),
}

/// Ce que le fil `defilante` doit faire.
#[derive(Debug, Clone, PartialEq)]
pub enum Geste {
    /// Un pas de défilement, en pixels réels (négatif : remonter).
    Descendre(i32),
    /// Prendre une image dans ce délai.
    Prendre(Duration),
    /// Montrer ce message en haut de l'écran, ou le cacher (`None`).
    Message(Option<&'static str>),
    /// Envoyer l'aperçu en direct.
    Apercu,
    /// Abandonner le moyen de défilement pour le suivant (D8).
    Repli,
    /// Finir la capture.
    Finir(Fin),
}

/// La suite de la course après `vu` (pure, §4.2, D6 à D15).
pub fn suite(course: &mut Course, vu: Vu, maintenant: Instant) -> Vec<Geste> {
    let _ = (course, vu, maintenant);
    Vec::new()
}

/// `defilante fin <bouton|entree|en_bas|limite> images <n> gardees <k> perdues <p>` (§8).
pub fn ligne_fin(fin: Fin, n: u32, gardees: u32, perdues: u32) -> String {
    format!(
        "defilante fin {} images {n} gardees {gardees} perdues {perdues}",
        fin.nom()
    )
}
