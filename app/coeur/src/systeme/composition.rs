//! Attendre que le compositeur du système ait remis l'écran à jour.
//!
//! Sert avant une photo du bureau : une fenêtre qu'on vient de cacher (le bandeau) reste
//! sur l'image composée jusqu'à la prochaine image du compositeur.

/// Attend la prochaine image du compositeur de Windows (`DwmFlush`), deux fois : la
/// fenêtre cachée juste avant n'est plus sur l'écran composé. Une vingtaine de
/// millisecondes à 60 Hz. Fil quelconque. Hors Windows : ne fait rien.
#[cfg(windows)]
pub fn attendre_composition() {
    for _ in 0..2 {
        // SAFETY : sans paramètre ; échoue sans effet si la composition est arrêtée.
        if unsafe { ::windows::Win32::Graphics::Dwm::DwmFlush() }.is_err() {
            return;
        }
    }
}

/// Hors Windows : rien à attendre.
#[cfg(not(windows))]
pub fn attendre_composition() {}
