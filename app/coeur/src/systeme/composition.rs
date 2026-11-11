//! Attendre que le compositeur du système ait remis l'écran à jour.
//!
//! Sert avant une photo du bureau : une fenêtre qu'on vient de cacher (le bandeau) reste
//! sur l'image composée jusqu'à la prochaine image du compositeur.

use std::time::Duration;

/// Attend la prochaine image du compositeur de Windows (`DwmFlush`, deux fois : la
/// fenêtre cachée juste avant n'est plus sur l'écran composé), au plus `limite`. Les
/// `DwmFlush` tournent dans un petit fil : l'appelant (le fil principal) n'attend jamais
/// plus que `limite`. Rend faux si l'échéance est tombée ou si le fil n'a pas pu partir.
/// Hors Windows : rien à attendre, vrai.
#[cfg(windows)]
pub fn attendre_composition(limite: Duration) -> bool {
    let (envoi, reception) = std::sync::mpsc::channel();
    let lance = std::thread::Builder::new()
        .name("composition".into())
        .spawn(move || {
            for _ in 0..2 {
                // SAFETY : sans paramètre ; échoue sans effet si la composition est arrêtée.
                if unsafe { ::windows::Win32::Graphics::Dwm::DwmFlush() }.is_err() {
                    break;
                }
            }
            let _ = envoi.send(());
        });
    lance.is_ok() && reception.recv_timeout(limite).is_ok()
}

/// Hors Windows : rien à attendre.
#[cfg(not(windows))]
pub fn attendre_composition(_limite: Duration) -> bool {
    true
}
