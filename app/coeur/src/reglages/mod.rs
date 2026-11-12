//! Réglages de BreezyShot (étape 2) : le fichier `reglages.json`.
//!
//! Module pur : il reçoit le dossier où lire et écrire, ne connaît ni Tauri ni les autres
//! modules, et n'affiche rien. Ce qui dépend du système (les raccourcis par défaut) lui est
//! donné par une [`Plateforme`], que l'Application obtient de `systeme`.
//!
//! - [`charger`] ne panique jamais et ne renvoie jamais d'erreur : dans le pire cas, il rend les
//!   valeurs par défaut et un [`Constat`] qui dit pourquoi. L'Application écrit les constats
//!   au journal.
//! - [`ecrire`] écrit de façon atomique (fichier temporaire puis renommage) et refuse une
//!   combinaison sans aucune sortie active.
//!
//! Découpage : `modele` (types et valeurs par défaut), `lecture` (fonctions pures qui
//! transforment le JSON en réglages, mises à niveau comprises), `fichier` (disque),
//! `constat` (ce qui s'est passé, pour le journal).

mod constat;
mod fichier;
mod lecture;
mod modele;

#[cfg(test)]
mod tests;

pub use constat::{Constat, ErreurEcriture, Raison};
pub use fichier::{charger, ecrire, valider, MODELE_NOM_MAX, NOM_FICHIER};
pub use modele::{
    Animations, Capture, Coin, Plateforme, Raccourcis, Reglages, Selection, Theme, Vignette,
    DUREE_MAX_S, DUREE_MIN_S, DUREE_TOUJOURS, RETARDATEUR_MAX_S, VERSION_ACTUELLE,
};
