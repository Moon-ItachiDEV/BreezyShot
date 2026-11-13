//! Droits : les fonctions que la licence Pro débloquera, et le seul point qui dit si l'une
//! est ouverte. Pur, sans système ni Tauri. Appelé par l'Application seulement, là où elle
//! assemble ses listes (actions construites, menu) : aucun `if pro` ailleurs.

/// Les fonctions que la licence Pro débloquera. Une fonction gratuite n'est pas ici.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fonction {
    Historique,
    Texte,
}

/// Vrai si `f` est ouverte à cet utilisateur. Aujourd'hui toujours vrai : la licence
/// n'existe pas encore (lot « Pro »). C'est le seul endroit à changer ce jour-là.
pub fn ouverte(_f: Fonction) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Chaque variante est nommée : en ajouter une oblige à relire ce test.
    #[test]
    fn tout_est_ouvert_sans_licence() {
        for f in [Fonction::Historique, Fonction::Texte] {
            match f {
                Fonction::Historique | Fonction::Texte => assert!(ouverte(f), "{f:?}"),
            }
        }
    }
}
