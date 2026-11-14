//! Le film (vidéo et GIF, contrat 0.7, §13.2) : ce qui se calcule sans fenêtre ni Windows.
//! L'horloge du film et sa grille d'images (`horloge`), le rectangle qui change d'une
//! image à l'autre (`difference`), l'écriture du GIF au fil de l'eau (`gif`, crate `gif`)
//! et le calage du son du micro sur l'horloge du film (`son`).
//!
//! Module pur : il n'utilise que `systeme` (ses types de pixels et de rectangles), jamais
//! un autre module de l'app (`tests/architecture.rs`). Le mot « film » ne paraît jamais à
//! l'écran : il évite « enregistrer », qui veut déjà dire « écrire le fichier dans le
//! dossier ».
//!
//! Son seul utilisateur est `application::geste_film` (les fils `film`, `film-video`,
//! `film-gif` et `film-son`).

pub mod difference;
pub mod gif;
pub mod horloge;
pub mod son;

pub use difference::{moitie, rectangle_change, sous_image, sous_image_dans};
pub use gif::{delai_cs, opaques_hors_du_transparent, Gif};
pub use horloge::{Horloge, Pas};
pub use son::Calage;
