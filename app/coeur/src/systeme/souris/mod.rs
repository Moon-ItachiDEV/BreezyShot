//! La souris, lue sans rien changer (0.7.6, « Clics visibles » du film) : la position du
//! pointeur et l'état des boutons physiques (gauche, droit, milieu).
//!
//! Façade commune : `windows.rs` lit `GetCursorPos` et `GetAsyncKeyState` (aucun crochet,
//! aucune écoute globale) ; `autre.rs` ne lit rien. Rien ici ne décide : `capture::clics`
//! interprète.

#[cfg(windows)]
#[path = "windows.rs"]
mod plateforme;

#[cfg(not(windows))]
#[path = "autre.rs"]
mod plateforme;

/// Ce que la souris dit à l'instant de la lecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lecture {
    /// La pointe du pointeur, en pixels réels du bureau virtuel.
    pub position: (i32, i32),
    /// Un bouton est enfoncé en ce moment.
    pub enfonce: bool,
    /// Un bouton a été enfoncé depuis la lecture précédente (le bit « depuis le dernier
    /// appel » de `GetAsyncKeyState` : rattrape un clic plus court que l'écart entre deux
    /// lectures).
    pub appuye: bool,
}

/// Lit la souris. `None` si le système ne la rend pas (hors Windows, ou pointeur illisible).
/// N'importe quel fil.
pub fn lire() -> Option<Lecture> {
    plateforme::lire()
}
