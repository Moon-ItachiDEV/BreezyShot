//! BreezyShot : la partie sans affichage de l'app.
//!
//! Seule l'Application connaît les autres modules (voir docs/ARCHITECTURE.md, §2).

pub mod application;
pub mod capture;
pub mod defilement;
pub mod droits;
pub mod fenetres;
pub mod installation;
pub mod licence;
pub mod raccourcis;
pub mod reglages;
pub mod sortie;
pub mod systeme;
pub mod texte;

pub use application::lancer;
