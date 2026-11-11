//! BreezyShot : la partie sans affichage de l'app.
//!
//! Seule l'Application connaît les autres modules (voir docs/ARCHITECTURE.md, §2).

pub mod application;
pub mod systeme;

pub use application::lancer;
