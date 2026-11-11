//! Ligne de commande (§3.4) : `breezyshot.exe` ou `breezyshot.exe --quitter`.

/// Ce que la ligne de commande demande.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Commande {
    /// Démarrer (ou, si une instance tourne déjà, ne rien faire de visible).
    Demarrer,
    /// Demander à l'instance en cours de se fermer proprement.
    Quitter,
}

/// Lecture des arguments, sans le nom du programme. Rend la commande et les arguments
/// inconnus (ignorés, à écrire au journal).
pub fn lire<I, S>(arguments: I) -> (Commande, Vec<String>)
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut commande = Commande::Demarrer;
    let mut inconnus = Vec::new();
    for a in arguments {
        match a.as_ref() {
            "--quitter" => commande = Commande::Quitter,
            autre => inconnus.push(autre.to_string()),
        }
    }
    (commande, inconnus)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sans_argument_on_demarre() {
        assert_eq!(lire(Vec::<String>::new()), (Commande::Demarrer, vec![]));
    }

    #[test]
    fn quitter() {
        assert_eq!(lire(["--quitter"]), (Commande::Quitter, vec![]));
    }

    #[test]
    fn inconnus_ignores_mais_rendus() {
        let (c, i) = lire(["--bizarre", "--quitter", "x"]);
        assert_eq!(c, Commande::Quitter);
        assert_eq!(i, ["--bizarre", "x"]);
    }
}
