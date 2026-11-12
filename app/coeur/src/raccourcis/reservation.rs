//! La partie mince qui touche au système : réserver les raccourcis choisis auprès du plugin
//! global-shortcut, traduire un appui en `Signal::Presse(action)`, tout libérer à l'arrêt.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Runtime};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut, ShortcutState};

use super::choix::{choisir, choisir_action, Bilan, ChoixAction, Ecarte, Raison};
use super::conversion::vers_systeme;
use super::notation::Raccourci;
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

/// Fonction de rappel d'un appui, partagée par tous les raccourcis réservés.
type Rappel = Arc<dyn Fn(Signal) + Send + Sync>;

/// Les raccourcis tenus par l'app : le bilan (qui sait quelle action tient quel raccourci)
/// et le rappel des appuis. Un appui relit le bilan au moment où il arrive : changer le
/// raccourci d'une action (`changer`) ne demande pas de refaire les autres.
pub struct Reservations {
    bilan: Arc<Mutex<Bilan>>,
    rappel: Rappel,
    /// Vrai pendant la saisie d'un raccourci dans la fenêtre Réglages : rien n'est tenu.
    suspendus: Mutex<bool>,
}

/// Réserve auprès du système un raccourci retenu, dont l'appui envoie l'action que le
/// bilan lui donne à ce moment-là. Vrai si le système l'a accepté.
fn tenir<R: Runtime>(
    app: &AppHandle<R>,
    raccourci: &Raccourci,
    bilan: &Arc<Mutex<Bilan>>,
    rappel: &Rappel,
) -> bool {
    let Some(systeme) = vers_systeme(raccourci) else {
        return false;
    };
    let pour_appui = *raccourci;
    let bilan = Arc::clone(bilan);
    let rappel = Arc::clone(rappel);
    let resultat = app
        .global_shortcut()
        .on_shortcut(systeme, move |_app, _s, evenement| {
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
}

/// Rend au système un raccourci tenu (sans effet s'il ne l'était pas).
fn lacher<R: Runtime>(app: &AppHandle<R>, raccourci: &Raccourci) {
    if let Some(systeme) = vers_systeme(raccourci) {
        let _ = app.global_shortcut().unregister(systeme);
    }
}

/// Réserve les raccourcis des actions données. `rappel` reçoit `Signal::Presse(action)` à
/// chaque appui. Ne panique pas : un raccourci refusé est un candidat écarté.
pub fn reserver<R, F>(
    app: &AppHandle<R>,
    candidats: &BTreeMap<String, Vec<String>>,
    impr_ecran_reserve: bool,
    rappel: F,
) -> Reservations
where
    R: Runtime,
    F: Fn(Signal) + Send + Sync + 'static,
{
    let rappel: Rappel = Arc::new(rappel);
    // Le bilan n'est complet qu'après le choix : l'appui le lit au moment où il arrive.
    let partage: Arc<Mutex<Bilan>> = Arc::new(Mutex::new(Bilan::default()));
    let bilan = choisir(candidats, impr_ecran_reserve, |raccourci| {
        tenir(app, raccourci, &partage, &rappel)
    });
    if let Ok(mut b) = partage.lock() {
        *b = bilan;
    }
    Reservations {
        bilan: partage,
        rappel,
        suspendus: Mutex::new(false),
    }
}

impl Reservations {
    /// Le bilan actuel (qui tient quoi).
    pub fn bilan(&self) -> Bilan {
        self.bilan.lock().map(|b| b.clone()).unwrap_or_default()
    }

    /// Vrai pendant une saisie (`suspendre` sans `reprendre`).
    pub fn suspendus(&self) -> bool {
        self.suspendus.lock().map(|s| *s).unwrap_or(false)
    }

    /// Change les candidats d'une action et essaie de les tenir, en gardant l'ancien
    /// raccourci tant qu'aucun nouveau n'est tenu. Rend le choix : `retenu` vaut `None` si
    /// aucun candidat n'a pu être pris, et alors rien n'a changé (l'ancien reste tenu).
    /// Reprend d'abord les raccourcis s'ils étaient suspendus.
    pub fn changer<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        action: &str,
        candidats: &[String],
        impr_ecran_reserve: bool,
    ) -> ChoixAction {
        self.reprendre(app);
        let avant = self.bilan();
        let actuel = avant.retenu(action).copied();
        let pris: Vec<(Raccourci, String)> = avant
            .actions
            .iter()
            .filter(|a| a.action != action)
            .filter_map(|a| a.retenu.map(|r| (r, a.action.clone())))
            .collect();
        let choix = choisir_action(action, candidats, &pris, actuel, impr_ecran_reserve, |r| {
            tenir(app, r, &self.bilan, &self.rappel)
        });
        let Some(nouveau) = choix.retenu else {
            if candidats.is_empty() {
                // Liste vide : « pas de raccourci voulu ». L'ancien est rendu au système.
                self.poser(&choix);
                if let Some(ancien) = actuel {
                    lacher(app, &ancien);
                }
            }
            return choix;
        };
        self.poser(&choix);
        if let Some(ancien) = actuel.filter(|a| *a != nouveau) {
            lacher(app, &ancien);
        }
        choix
    }

    /// Remplace, dans le bilan, la ligne de l'action de `choix`.
    fn poser(&self, choix: &ChoixAction) {
        if let Ok(mut b) = self.bilan.lock() {
            match b.actions.iter_mut().find(|a| a.action == choix.action) {
                Some(a) => *a = choix.clone(),
                None => b.actions.push(choix.clone()),
            }
        }
    }

    /// Rend au système tous les raccourcis tenus, le temps d'une saisie : sans cela,
    /// `Ctrl + Maj + 2` serait pris par le système avant la page et lancerait une capture.
    /// Le bilan est gardé pour `reprendre`.
    pub fn suspendre<R: Runtime>(&self, app: &AppHandle<R>) {
        let Ok(mut suspendus) = self.suspendus.lock() else {
            return;
        };
        if *suspendus {
            return;
        }
        *suspendus = true;
        drop(suspendus);
        for a in self.bilan().actions {
            if let Some(r) = a.retenu {
                lacher(app, &r);
            }
        }
    }

    /// Reprend les raccourcis rendus par `suspendre`. Un raccourci qu'une autre application
    /// a pris entre-temps est perdu : l'action n'a plus de raccourci, le bilan le dit
    /// (`RefuseParLeSysteme`). Sans effet si rien n'est suspendu. Rend vrai s'il y avait
    /// quelque chose à reprendre.
    pub fn reprendre<R: Runtime>(&self, app: &AppHandle<R>) -> bool {
        let Ok(mut suspendus) = self.suspendus.lock() else {
            return false;
        };
        if !*suspendus {
            return false;
        }
        *suspendus = false;
        drop(suspendus);
        let bilan = self.bilan();
        let mut perdus = Vec::new();
        for a in &bilan.actions {
            if let Some(r) = a.retenu {
                if !tenir(app, &r, &self.bilan, &self.rappel) {
                    perdus.push(a.action.clone());
                }
            }
        }
        if !perdus.is_empty() {
            if let Ok(mut b) = self.bilan.lock() {
                for a in b.actions.iter_mut().filter(|a| perdus.contains(&a.action)) {
                    if let Some(r) = a.retenu.take() {
                        a.ecartes.push(Ecarte {
                            candidat: r.canonique(),
                            raison: Raison::RefuseParLeSysteme,
                        });
                    }
                }
            }
        }
        true
    }
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
