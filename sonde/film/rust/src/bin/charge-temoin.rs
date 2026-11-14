//! Témoin de S2 point 8 : le code de l'exemple `charge` (`sonde_film::charge`), lié SANS
//! /DELAYLOAD (build.rs ne touche qu'aux exemples) et sans la DLL inventée. Ses DLL de
//! Media Foundation, de DXGI et de Direct3D sont chargées dès le lancement.

#[cfg(not(windows))]
fn main() {
    eprintln!("charge-temoin : Windows seulement");
}

#[cfg(windows)]
fn main() {
    sonde_film::charge::principal(&sonde_film::arguments(), None);
}
