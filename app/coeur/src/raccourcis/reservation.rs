//! La partie mince qui touche au système : réserver les raccourcis choisis auprès du plugin
//! global-shortcut, traduire un appui en `Signal::Presse(action)`, tout libérer à l'arrêt.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Runtime};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut, ShortcutState};

use super::choix::{choisir, Bilan, Raison};
use super::conversion::vers_systeme;
use super::Signal;

/// Texte d'une raison pour le journal (sans accents).
pub fn texte_raison(raison: &Raison) -> String {
    match raison {
        Raison::Invalide(e) => format!("invalide: {e}"),
        Raison::ReserveParLeSysteme => "reserve par le systeme".into(),
        Raison::DejaPrisParUneAutreAction(a) => format!("deja pris par {a}"),
        Raison::RefuseParLeSysteme => "refuse par le systeme".into(),
    }
}

/// Lignes de journal d'un bilan : une par candidat écarté, puis le retenu (ou `aucun`).
pub fn lignes_journal(bilan: &Bilan) -> Vec<String> {
    let mut lignes = Vec::new();
    for a in &bilan.actions {
        for e in &a.ecartes {
            lignes.push(format!(
                "raccourci {} ecarte: {} ({})",
                a.action,
                e.candidat.trim(),
                texte_raison(&e.raison)
            ));
        }
        let retenu = a
            .retenu
            .as_ref()
            .map(|r| r.canonique())
            .unwrap_or_else(|| "aucun".into());
        lignes.push(format!("raccourci {}: {retenu}", a.action));
    }
    lignes
}

/// Réserve les raccourcis des actions données. `rappel` reçoit `Signal::Presse(action)` à
/// chaque appui. Ne panique pas : un raccourci refusé est un candidat écarté.
pub fn reserver<R, F>(
    app: &AppHandle<R>,
    candidats: &BTreeMap<String, Vec<String>>,
    impr_ecran_reserve: bool,
    rappel: F,
) -> Bilan
where
    R: Runtime,
    F: Fn(Signal) + Send + Sync + 'static,
{
    let rappel = Arc::new(rappel);
    // Le bilan n'est complet qu'après le choix : l'appui le lit au moment où il arrive.
    let partage: Arc<Mutex<Bilan>> = Arc::new(Mutex::new(Bilan::default()));
    let gestion = app.global_shortcut();
    let bilan = choisir(candidats, impr_ecran_reserve, |raccourci| {
        let Some(systeme) = vers_systeme(raccourci) else {
            return false;
        };
        let pour_appui = *raccourci;
        let bilan = Arc::clone(&partage);
        let rappel = Arc::clone(&rappel);
        let resultat = gestion.on_shortcut(systeme, move |_app, _s, evenement| {
            if evenement.state != ShortcutState::Pressed {
                return;
            }
            let action = match bilan.lock() {
                Ok(b) => b.action_de(&pour_appui).map(str::to_string),
                Err(_) => None,
            };
            if let Some(action) = action {
                rappel(Signal::Presse(action));
            }
        });
        match resultat {
            Ok(()) => true,
            Err(e) => {
                log::info!(
                    "raccourci {} refuse par le systeme: {e}",
                    raccourci.canonique()
                );
                false
            }
        }
    });
    if let Ok(mut b) = partage.lock() {
        *b = bilan.clone();
    }
    bilan
}

/// Échap seul, dans les types du système.
fn echap() -> Shortcut {
    Shortcut::new(None, Code::Escape)
}

/// Réserve Échap en global, le temps d'une sélection : `rappel` reçoit `Signal::Echap` à
/// chaque appui. Rend l'erreur du système si une autre application le tient déjà.
/// Le plugin fait la réservation sur le fil principal et attend qu'elle soit faite.
pub fn reserver_echap<R, F>(app: &AppHandle<R>, rappel: F) -> Result<(), String>
where
    R: Runtime,
    F: Fn(Signal) + Send + Sync + 'static,
{
    app.global_shortcut()
        .on_shortcut(echap(), move |_app, _s, evenement| {
            if evenement.state == ShortcutState::Pressed {
                rappel(Signal::Echap);
            }
        })
        .map_err(|e| e.to_string())
}

/// Libère Échap (réservé par `reserver_echap`).
pub fn liberer_echap<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    app.global_shortcut()
        .unregister(echap())
        .map_err(|e| e.to_string())
}

/// Libère tous les raccourcis réservés.
pub fn liberer<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(lignes: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
        lignes
            .iter()
            .map(|(a, c)| (a.to_string(), c.iter().map(|s| s.to_string()).collect()))
            .collect()
    }

    #[test]
    fn journal_du_bilan() {
        let t = table(&[("zone", &["ImprEcran", "Ctrl+Maj+1"])]);
        let b = choisir(&t, true, |_| true);
        assert_eq!(
            lignes_journal(&b),
            [
                "raccourci zone ecarte: ImprEcran (reserve par le systeme)",
                "raccourci zone: Ctrl+Maj+1"
            ]
        );
        let b = choisir(&t, false, |_| false);
        assert_eq!(
            lignes_journal(&b),
            [
                "raccourci zone ecarte: ImprEcran (refuse par le systeme)",
                "raccourci zone ecarte: Ctrl+Maj+1 (refuse par le systeme)",
                "raccourci zone: aucun"
            ]
        );
        let b = choisir(&t, false, |_| true);
        assert_eq!(lignes_journal(&b), ["raccourci zone: ImprEcran"]);
        let vide = table(&[("zone", &[])]);
        assert_eq!(
            lignes_journal(&choisir(&vide, false, |_| true)),
            ["raccourci zone: aucun"]
        );
    }
}
