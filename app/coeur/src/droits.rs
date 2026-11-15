//! Droits : les fonctions que la licence Pro débloque, et le seul point qui dit si l'une est
//! ouverte. Pur, sans système ni Tauri. Appelé par l'Application seulement, là où elle
//! assemble ses listes (actions construites, menu) : aucun `if pro` ailleurs.

/// Les fonctions que la licence Pro débloque. Une fonction gratuite n'est pas ici.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fonction {
    Historique,
    Texte,
    /// La capture défilante (0.6).
    Defilante,
    /// L'enregistrement d'une vidéo (0.7), indépendant du GIF.
    Video,
    /// L'enregistrement d'un GIF (0.7), indépendant de la vidéo.
    Gif,
}

/// Vrai si `f` est ouverte. `pro` : une licence valide est en place.
pub fn ouverte(f: Fonction, pro: bool) -> bool {
    match f {
        Fonction::Historique
        | Fonction::Texte
        | Fonction::Defilante
        | Fonction::Video
        | Fonction::Gif => pro,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Chaque variante est nommée : en ajouter une oblige à relire ce test.
    #[test]
    fn pro_ouvre_tout_et_rien_sans_licence() {
        for f in [
            Fonction::Historique,
            Fonction::Texte,
            Fonction::Defilante,
            Fonction::Video,
            Fonction::Gif,
        ] {
            match f {
                Fonction::Historique
                | Fonction::Texte
                | Fonction::Defilante
                | Fonction::Video
                | Fonction::Gif => {
                    assert!(ouverte(f, true), "{f:?}");
                    assert!(!ouverte(f, false), "{f:?}");
                }
            }
        }
    }
}
