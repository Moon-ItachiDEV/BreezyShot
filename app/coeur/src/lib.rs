//! BreezyShot : la partie sans affichage de l'app.
//!
//! Seule l'Application connaît les autres modules (voir docs/ARCHITECTURE.md, §2).

pub mod application;
pub mod capture;
pub mod fenetres;
pub mod raccourcis;
pub mod reglages;
pub mod systeme;

pub use application::lancer;
