//! Faire défiler la page d'une autre app (contrat 0.6, §13.1, décisions R3 et R4 après la
//! sonde) : trouver les pages sous la souris (S4), envoyer des pas de défilement, lire la
//! position (S5), entendre la molette de l'utilisateur (S6).
//!
//! Moyens, dans cet ordre, choisis une fois par capture sur les deux premières commandes
//! (l'effet est toujours vérifié sur les images, par l'appelant) : `WM_MOUSEWHEEL` posté à
//! la fenêtre fille sous le centre de la zone (`Message`), UI Automation
//! `Scroll(SmallIncrement)` (`Automation`), molette `SendInput` marquée, seulement quand le
//! pointeur est sur la zone (`Molette`). Jamais de touches ; `SetScrollPercent` n'est pas
//! utilisé.
//!
//! Façade commune : `windows.rs` la réalise, `autre.rs` donne des réponses neutres
//! ailleurs. La règle de garde de la molette envoyée (`peut_envoyer`) est pure, ici, testée
//! sous Linux. Rien ne va au journal qui viendrait d'une page (ni titre, ni adresse).

use super::gel::{FenetreInfo, Rect};

#[cfg(windows)]
#[path = "windows.rs"]
mod plateforme;

#[cfg(not(windows))]
#[path = "autre.rs"]
mod plateforme;

/// Marque de BreezyShot dans `dwExtraInfo` de chaque molette envoyée : l'écoute de la
/// molette reconnaît ainsi les crans de l'app parmi ceux de l'utilisateur (R4).
pub const MARQUE: usize = 0x4252_5A59;

/// Une page qui défile, trouvée dans une fenêtre visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageTrouvee {
    /// La partie qui défile, en pixels réels du bureau.
    pub rect: Rect,
    /// La fenêtre (Windows : le HWND) qui reçoit le défilement.
    pub fenetre: isize,
}

/// Les pages qui défilent dans les `max` premières fenêtres de `fenetres` (du premier plan
/// vers le fond), dans le même ordre (S4 : fenêtres filles d'abord, UI Automation en
/// complément). À appeler hors du fil principal (fil `defilante-pages`). Vide ailleurs que
/// sous Windows.
pub fn pages(fenetres: &[FenetreInfo], max: usize) -> Vec<PageTrouvee> {
    plateforme::pages(fenetres, max)
}

/// Un moyen de faire défiler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moyen {
    /// UI Automation, `ScrollPattern.Scroll(SmallIncrement)`.
    Automation,
    /// Molette `SendInput` marquée (`MARQUE`), le pointeur sur la zone.
    Molette,
    /// `WM_MOUSEWHEEL` posté à la fenêtre fille sous le centre de la zone.
    Message,
}

impl Moyen {
    /// L'ordre d'essai (R3).
    pub const ORDRE: [Moyen; 3] = [Moyen::Message, Moyen::Automation, Moyen::Molette];

    /// Nom écrit au journal : `automation`, `molette`, `message`.
    pub fn nom(self) -> &'static str {
        match self {
            Moyen::Automation => "automation",
            Moyen::Molette => "molette",
            Moyen::Message => "message",
        }
    }
}

/// La position lue de la page (UI Automation), quand elle existe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    /// `VerticalScrollPercent`, de 0 à 100.
    pub pour_cent: f64,
    /// `VerticalViewSize`, la part visible de la page, de 0 à 100.
    pub vue: f64,
}

/// Ce qu'a fait une demande de pas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Envoi {
    /// Le pas est parti.
    Envoye,
    /// La garde de la molette envoyée l'a retenu (`peut_envoyer`) : à retenter au tour
    /// suivant.
    Saute,
}

/// Ce qui fait défiler une page, le temps d'une capture. Vit dans le fil `defilante`
/// (COM y est ouvert en MTA par le système) ; rendu à la destruction.
#[derive(Debug)]
pub struct Defileur {
    interne: plateforme::Defileur,
}

/// Prépare le défilement de `zone` (pixels réels du bureau) ; `fenetre` : celle de la page
/// cliquée (`PageTrouvee::fenetre`), `None` pour une zone tracée (la fenêtre sous le
/// centre de la zone).
pub fn preparer(zone: Rect, fenetre: Option<isize>) -> Result<Defileur, String> {
    plateforme::preparer(zone, fenetre).map(|interne| Defileur { interne })
}

impl Defileur {
    /// Le moyen en cours, `None` quand tous ont été abandonnés.
    pub fn moyen(&self) -> Option<Moyen> {
        plateforme::moyen(&self.interne)
    }

    /// Abandonne le moyen en cours pour le suivant de `Moyen::ORDRE` (D8) ; rend le
    /// nouveau, `None` s'il n'y en a plus.
    pub fn abandonner(&mut self) -> Option<Moyen> {
        plateforme::abandonner(&mut self.interne)
    }

    /// Demande un pas de `pixels` pixels réels vers le bas (négatif : vers le haut).
    pub fn descendre(&mut self, pixels: i32) -> Result<Envoi, String> {
        plateforme::descendre(&mut self.interne, pixels)
    }

    /// La position lue (UI Automation), `None` si la page n'en donne pas.
    pub fn position(&self) -> Option<Position> {
        plateforme::position(&self.interne)
    }
}

/// Garde de la molette envoyée (D7, fonction pure) : vrai seulement si la fenêtre sous le
/// point visé a la même racine que la cible (`GetAncestor(GA_ROOT)`) et, quand « Faire
/// défiler les fenêtres inactives » est coupé (`survol_actif` faux), si la cible a le
/// premier plan.
pub fn peut_envoyer(
    racine_sous_point: Option<isize>,
    racine_cible: isize,
    premier_plan: Option<isize>,
    survol_actif: bool,
) -> bool {
    // Socle : rien ne part (chantier B).
    let _ = (racine_sous_point, racine_cible, premier_plan, survol_actif);
    false
}

/// Un cran de molette entendu, qui n'est pas de l'app (sans `MARQUE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Molette {
    /// Le pointeur, en pixels réels du bureau.
    pub point: (i32, i32),
    /// Crans : positif vers le bas de la page, négatif vers le haut.
    pub crans: i32,
}

/// L'écoute de la molette (crochet `WH_MOUSE_LL` dans son fil `defilante-molette`, R4). La
/// détruire retire le crochet et arrête son fil.
#[derive(Debug)]
pub struct Ecoute {
    interne: plateforme::Ecoute,
}

/// Écoute la molette de l'utilisateur : `rappel` reçoit chaque cran sans la marque de
/// l'app, depuis le fil de l'écoute (il ne doit que poster). Erreur ailleurs que sous
/// Windows.
pub fn ecouter_molette(rappel: impl Fn(Molette) + Send + 'static) -> Result<Ecoute, String> {
    plateforme::ecouter_molette(Box::new(rappel)).map(|interne| Ecoute { interne })
}

impl Ecoute {
    /// Vrai tant que le crochet est posé.
    pub fn active(&self) -> bool {
        plateforme::active(&self.interne)
    }
}

/// Le rappel de l'écoute, tel que le reçoit la plateforme.
type RappelMolette = Box<dyn Fn(Molette) + Send + 'static>;
