//! La minuterie de Windows, affinée le temps d'un film à plus de 30 images par seconde
//! (0.7.7) : sans cela, un réveil attend un tick de 15,6 ms, trop grossier pour un pas de
//! 16,7 ms. Façade commune : `windows.rs` (`timeBeginPeriod`), `autre.rs` (rien).

#[cfg(windows)]
#[path = "windows.rs"]
mod plateforme;

#[cfg(not(windows))]
#[path = "autre.rs"]
mod plateforme;

/// La minuterie fine, tant que cette valeur vit (une demande de 1 ms, rendue à la
/// destruction).
#[derive(Debug)]
pub struct Fine {
    _interne: plateforme::Fine,
}

impl Fine {
    pub fn demander() -> Fine {
        Fine {
            _interne: plateforme::Fine::demander(),
        }
    }
}
