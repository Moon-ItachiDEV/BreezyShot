//! La licence, côté Application (avenant 0.5 « Licence », §3.3) : lue au démarrage,
//! activée ou retirée par la fenêtre Réglages (trois commandes, permises à la seule fenêtre
//! `reglages`). Après un changement : `Evenement::LicenceChangee`, qui refait le menu et les
//! raccourcis. Le texte de la licence n'est jamais écrit au journal : seuls l'`id` (borné,
//! ASCII imprimable) et le code du refus y vont.

use serde::Serialize;
use tauri::AppHandle;

/// Ce que les commandes rendent à la page (`{"active":true}`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EtatLicence {
    pub active: bool,
}

/// Démarrage, avant la réservation des raccourcis : `sortie::lire_licence` puis
/// `licence::verifier`, rangée par `*super::licence_en_vigueur(app) = …` (sans événement :
/// les listes ne sont pas encore faites). Après le démarrage, `super::poser_licence`.
pub fn charger(app: &AppHandle) {
    // Un fichier illisible ou une licence refusée : pas de licence, le fichier reste.
    let lu = crate::sortie::lire_licence();
    let licence = match juger_au_demarrage(lu.as_deref()) {
        AuDemarrage::Absente => {
            log::info!("licence absente");
            None
        }
        AuDemarrage::Active(licence) => {
            log::info!("licence active {}", licence.id);
            Some(licence)
        }
        AuDemarrage::Refusee(refus) => {
            log::info!("licence refusee {}", refus.code());
            None
        }
    };
    *super::licence_en_vigueur(app) = licence;
}

/// Ce que vaut au démarrage le texte rangé.
#[derive(Debug, PartialEq, Eq)]
enum AuDemarrage {
    Absente,
    Active(crate::licence::Licence),
    Refusee(crate::licence::Refus),
}

/// Pas de fichier, ou un fichier vide ou fait de blancs (ce que laisse un retrait dont
/// l'effacement a échoué) : absente, pas refusée.
fn juger_au_demarrage(texte: Option<&str>) -> AuDemarrage {
    match texte {
        None => AuDemarrage::Absente,
        Some(t) if crate::licence::nettoyer(t).is_empty() => AuDemarrage::Absente,
        Some(t) => match crate::licence::verifier(t) {
            Ok(licence) => AuDemarrage::Active(licence),
            Err(refus) => AuDemarrage::Refusee(refus),
        },
    }
}

/// Vérifie et range `texte` ; l'erreur est le code du refus.
pub fn activer(app: &AppHandle, texte: &str) -> Result<EtatLicence, &'static str> {
    let (licence, a_ranger) = verifier_pour_ranger(texte).map_err(|refus| {
        log::info!("licence refusee {}", refus.code());
        refus.code()
    })?;
    // Si le disque refuse, la licence vaut pour cette session.
    if let Err(e) = crate::sortie::ecrire_licence(a_ranger) {
        log::warn!("licence non rangee: {e}");
    }
    log::info!("licence active {}", licence.id);
    super::poser_licence(app, Some(licence));
    Ok(etat(app))
}

/// La licence acceptée et le texte à ranger : le texte nettoyé, celui que `verifier` a
/// jugé (un texte collé brut pourrait dépasser ce que relit le démarrage).
fn verifier_pour_ranger(
    texte: &str,
) -> Result<(crate::licence::Licence, &str), crate::licence::Refus> {
    let licence = crate::licence::verifier(texte)?;
    Ok((licence, crate::licence::nettoyer(texte)))
}

/// Retire la licence : texte rangé effacé (ou, à défaut, remplacé par un fichier vide),
/// plus de Pro, listes refaites. Si ni l'un ni l'autre ne marche, la licence reviendrait au
/// démarrage suivant : elle reste donc en vigueur, et l'état rendu le dit.
pub fn retirer(app: &AppHandle) -> EtatLicence {
    let retire = retirer_le_texte(crate::sortie::effacer_licence, || {
        crate::sortie::ecrire_licence("")
    });
    match retire {
        Ok(()) => {
            log::info!("licence retiree");
            super::poser_licence(app, None);
        }
        Err(e) => log::warn!("licence non retiree: {e}"),
    }
    etat(app)
}

/// Efface, sinon vide ; l'erreur est celle du dernier essai.
fn retirer_le_texte(
    effacer: impl FnOnce() -> Result<(), String>,
    vider: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    match effacer() {
        Ok(()) => Ok(()),
        Err(e) => {
            log::warn!("licence non effacee: {e}");
            vider()
        }
    }
}

/// L'état de la licence en vigueur.
pub fn etat(app: &AppHandle) -> EtatLicence {
    EtatLicence {
        active: super::pro(app),
    }
}

#[tauri::command]
pub fn licence_etat(app: AppHandle) -> EtatLicence {
    etat(&app)
}

#[tauri::command]
pub fn licence_activer(app: AppHandle, texte: String) -> Result<EtatLicence, String> {
    activer(&app, &texte).map_err(String::from)
}

#[tauri::command]
pub fn licence_retirer(app: AppHandle) -> EtatLicence {
    retirer(&app)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::licence::{Licence, Refus};

    /// La licence d'essai du dépôt, telle que rangée (fin de ligne comprise).
    const ESSAI: &str = include_str!("../../../../essais/licence-essai.txt");

    fn essai() -> Licence {
        Licence {
            id: "ESSAI-0001".into(),
            essai: true,
        }
    }

    #[test]
    fn vide_ou_blancs_au_demarrage_absente() {
        for texte in [None, Some(""), Some("   "), Some(" \r\n\t\n ")] {
            assert_eq!(juger_au_demarrage(texte), AuDemarrage::Absente, "{texte:?}");
        }
        assert_eq!(
            juger_au_demarrage(Some(ESSAI)),
            AuDemarrage::Active(essai())
        );
        assert_eq!(
            juger_au_demarrage(Some("BS1.a.b")),
            AuDemarrage::Refusee(Refus::Forme)
        );
    }

    #[test]
    fn bom_au_demarrage_refuse_forme() {
        let texte = format!("\u{feff}{ESSAI}");
        assert_eq!(
            juger_au_demarrage(Some(&texte)),
            AuDemarrage::Refusee(Refus::Forme)
        );
        // Un BOM seul n'est pas un blanc : refusé, pas absent.
        assert_eq!(
            juger_au_demarrage(Some("\u{feff}")),
            AuDemarrage::Refusee(Refus::Forme)
        );
    }

    #[test]
    fn collee_apres_70000_espaces_rangee_nettoyee_puis_relue() {
        let collee = format!("{}{ESSAI}", " ".repeat(70_000));
        let (licence, a_ranger) = verifier_pour_ranger(&collee).unwrap();
        assert_eq!(licence, essai());
        assert_eq!(a_ranger, ESSAI.trim_end_matches('\n'));
        // Rangé puis relu comme au démarrage, dans un vrai fichier.
        let d = tempfile::tempdir().unwrap();
        let chemin = d.path().join("licence.txt");
        std::fs::write(&chemin, a_ranger).unwrap();
        let relu = std::fs::read_to_string(&chemin).unwrap();
        assert!(relu.len() < 64 * 1024, "{}", relu.len());
        assert_eq!(
            juger_au_demarrage(Some(&relu)),
            AuDemarrage::Active(essai())
        );
        // Refusée : rien à ranger.
        assert_eq!(
            verifier_pour_ranger(&format!("{}BS1.a.b", " ".repeat(70_000))),
            Err(Refus::Forme)
        );
    }

    #[test]
    fn retrait_efface_sinon_vide_sinon_echoue() {
        let mut vide = false;
        assert_eq!(
            retirer_le_texte(|| Ok(()), || panic!("pas de repli")),
            Ok(())
        );
        assert_eq!(
            retirer_le_texte(
                || Err("tenu".into()),
                || {
                    vide = true;
                    Ok(())
                }
            ),
            Ok(())
        );
        assert!(vide);
        assert_eq!(
            retirer_le_texte(|| Err("tenu".into()), || Err("refuse".into())),
            Err("refuse".to_string())
        );
    }
}
