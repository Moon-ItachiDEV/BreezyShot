//! Droits : les fonctions que la licence Pro débloque, et le seul point qui dit si l'une est
//! ouverte. Pur, sans système ni Tauri. Appelé par l'Application seulement, là où elle
//! assemble ses listes (actions construites, menu) : aucun `if pro` ailleurs.

/// Les fonctions que la licence Pro débloque. Une fonction gratuite n'est pas ici.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fonction {
    Historique,
    Texte,
}

/// Vrai si `f` est ouverte. `pro` : une licence valide est en place.
pub fn ouverte(f: Fonction, pro: bool) -> bool {
    match f {
        Fonction::Historique | Fonction::Texte => pro,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Chaque variante est nommée : en ajouter une oblige à relire ce test.
    #[test]
    fn pro_ouvre_tout_et_rien_sans_licence() {
        for f in [Fonction::Historique, Fonction::Texte] {
            match f {
                Fonction::Historique | Fonction::Texte => {
                    assert!(ouverte(f, true), "{f:?}");
                    assert!(!ouverte(f, false), "{f:?}");
                }
            }
        }
    }
}
