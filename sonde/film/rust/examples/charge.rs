//! S2 points 8 et 12 (le code est dans `sonde_film::charge`). Lié avec /DELAYLOAD des DLL
//! de Media Foundation, de DXGI et de Direct3D (build.rs), et d'une DLL inventée,
//! `bzs-absente.dll`, dont une fonction est importée par `raw-dylib` (8c). Windows
//! seulement.

#[cfg(not(windows))]
fn main() {
    eprintln!("charge : Windows seulement");
}

#[cfg(windows)]
#[link(name = "bzs-absente", kind = "raw-dylib")]
extern "system" {
    fn BzsAbsente() -> u32;
}

#[cfg(windows)]
fn absente() -> u32 {
    // SAFETY : fonction sans argument ; sans la DLL, le chargeur à la demande lève son
    // exception (0xC06D007E) : c'est ce que 8c montre.
    unsafe { BzsAbsente() }
}

#[cfg(windows)]
fn main() {
    sonde_film::charge::principal(&sonde_film::arguments(), Some(absente));
}
