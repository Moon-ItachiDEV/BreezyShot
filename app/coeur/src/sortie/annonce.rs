//! Ce qu'on dit a l'utilisateur apres une capture (fonction pure).

use super::{Bilan, Demande, Etape, EtapeFichier};

/// Ce qu'il faut dire a l'utilisateur. L'Application le traduit en message
/// du bandeau. Une seule ligne de titre ; seule une erreur porte sa cause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Annonce {
    Reussite { titre: String },
    Erreur { titre: String, cause: String },
}

fn reussite(titre: impl Into<String>) -> Annonce {
    Annonce::Reussite {
        titre: titre.into(),
    }
}

fn erreur(titre: &str, cause: impl Into<String>) -> Annonce {
    Annonce::Erreur {
        titre: titre.to_string(),
        cause: cause.into(),
    }
}

/// Choisit l'annonce a partir du bilan. L'echec de « Recents » seul ne
/// s'annonce pas (l'Application le met au journal).
pub fn annonce(bilan: &Bilan, demande: &Demande) -> Annonce {
    let dossier = &demande.dossier_affiche;
    match (&bilan.copie, &bilan.enregistrement) {
        // L'enregistrement en echec passe avant tout le reste.
        (_, EtapeFichier::Echec { cause, .. }) => erreur("Impossible d'enregistrer", cause),
        (Etape::Faite, EtapeFichier::Fait(_)) => reussite("Copié et enregistré"),
        (Etape::Faite, EtapeFichier::NonDemande) => reussite("Copié dans le presse-papiers"),
        (Etape::NonDemandee, EtapeFichier::Fait(_)) => {
            reussite(format!("Enregistré dans {dossier}"))
        }
        (Etape::Echec(cause), EtapeFichier::NonDemande) => erreur("Impossible de copier", cause),
        // Copie ratee mais fichier enregistre : la cause technique reste
        // dans le bilan, on dit seulement ou est la capture.
        (Etape::Echec(_), EtapeFichier::Fait(_)) => erreur(
            "Impossible de copier",
            format!("La capture est enregistrée dans {dossier}."),
        ),
        // Ni copie ni enregistrement demandes : seul « Recents » compte.
        (Etape::NonDemandee, EtapeFichier::NonDemande) => match &bilan.recent {
            Some(Err(cause)) => erreur("Impossible de garder la capture", cause),
            Some(Ok(_)) | None => reussite("Capture prise"),
        },
    }
}
