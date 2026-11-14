//! Veille de la boucle principale (13/11/2026, « les raccourcis parfois figent ») : un
//! petit fil pose une tâche vide sur le fil principal chaque seconde. Si elle n'a pas
//! tourné au bout de `SEUIL`, une ligne au journal dit que la boucle est bloquée (et dans
//! quel état était la capture) ; une seconde ligne dit quand elle reprend. Rien d'autre :
//! la veille ne change aucun état, elle sert à lire un blocage après coup.
//!
//! Ce que le journal prouve : un appui reçu écrit `raccourci presse: …` (message `WM_HOTKEY`,
//! qui passe par la fenêtre cachée du plugin, pas par la file de tao) ; s'il manque, l'appui
//! n'est jamais arrivé. Un appui reçu que l'automate refuse est dit à part
//! (`ligne_appui_ignore`). La veille, elle, ne parle que de la boucle de tao : elle peut se
//! dire bloquée pendant une boucle imbriquée où les raccourcis arrivent encore, ou après une
//! mise en veille du PC, et l'arrêt de l'app (jusqu'à 2 s d'attente des suppressions) peut
//! aussi écrire une ligne `bloquee`.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use super::etats::Etat;
use super::Application;

/// Une tâche vide est posée sur le fil principal à cette cadence.
pub const CADENCE: Duration = Duration::from_secs(1);
/// Au-delà, la boucle principale est dite bloquée.
pub const SEUIL: Duration = Duration::from_secs(2);

/// L'état de l'automate et le temps passé dedans, sans attendre son verrou : si le fil
/// principal le tient pendant un blocage, la veille ne se bloque pas avec lui.
fn etat_sans_attendre(app: &AppHandle) -> Option<(Etat, Duration)> {
    let etat = app.try_state::<Application>()?;
    let automate = etat.automate.try_lock().ok()?;
    Some((automate.etat(), automate.depuis()))
}

/// L'état et le temps passé dedans (fil principal), pour le journal d'un appui ignoré.
pub(super) fn etat_et_duree(app: &AppHandle) -> Option<(Etat, Duration)> {
    let etat = app.try_state::<Application>()?;
    let automate = etat.automate.lock().ok()?;
    Some((automate.etat(), automate.depuis()))
}

/// `boucle principale bloquee depuis 2000 ms (etat Lecture depuis 3500 ms)`.
pub fn ligne_bloquee(attente: Duration, etat: Option<(Etat, Duration)>) -> String {
    format!(
        "boucle principale bloquee depuis {} ms ({})",
        attente.as_millis(),
        texte_etat(etat)
    )
}

/// `boucle principale reprise apres 4200 ms`.
pub fn ligne_reprise(attente: Duration) -> String {
    format!("boucle principale reprise apres {} ms", attente.as_millis())
}

/// `appui zone ignore: etat Lecture depuis 1200 ms` (la ligne `capture deja en cours:
/// appui ignore` reste telle quelle, les essais la cherchent).
pub fn ligne_appui_ignore(action: &str, etat: Option<(Etat, Duration)>) -> String {
    format!("appui {action} ignore: {}", texte_etat(etat))
}

fn texte_etat(etat: Option<(Etat, Duration)>) -> String {
    match etat {
        Some((e, d)) => format!("etat {e:?} depuis {} ms", d.as_millis()),
        None => "etat inconnu".into(),
    }
}

/// Lance la veille (une fois, après `pret`). Le fil s'arrête quand la boucle refuse une
/// tâche, au plus tard avec le processus.
pub fn lancer(app: &AppHandle) {
    let app = app.clone();
    let lance = std::thread::Builder::new()
        .name("veille-boucle".into())
        .spawn(move || loop {
            std::thread::sleep(CADENCE);
            let (fait, attente) = mpsc::channel::<()>();
            let pose = Instant::now();
            if app
                .run_on_main_thread(move || {
                    let _ = fait.send(());
                })
                .is_err()
            {
                return;
            }
            match attente.recv_timeout(SEUIL) {
                Ok(()) => {}
                Err(RecvTimeoutError::Disconnected) => return,
                Err(RecvTimeoutError::Timeout) => {
                    log::warn!(
                        "{}",
                        ligne_bloquee(pose.elapsed(), etat_sans_attendre(&app))
                    );
                    // Une seule ligne par blocage : la suivante dit la reprise. Canal fermé
                    // (boucle arrêtée, tâche jetée) : la veille s'arrête.
                    if attente.recv().is_err() {
                        return;
                    }
                    log::info!("{}", ligne_reprise(pose.elapsed()));
                }
            }
        });
    if let Err(e) = lance {
        log::warn!("veille de la boucle non lancee: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lignes_du_journal() {
        let ms = Duration::from_millis;
        assert_eq!(
            ligne_bloquee(ms(2003), Some((Etat::Lecture, ms(3500)))),
            "boucle principale bloquee depuis 2003 ms (etat Lecture depuis 3500 ms)"
        );
        assert_eq!(
            ligne_bloquee(ms(2000), None),
            "boucle principale bloquee depuis 2000 ms (etat inconnu)"
        );
        assert_eq!(
            ligne_reprise(ms(4200)),
            "boucle principale reprise apres 4200 ms"
        );
        assert_eq!(
            ligne_appui_ignore("zone", Some((Etat::Souffle, ms(800)))),
            "appui zone ignore: etat Souffle depuis 800 ms"
        );
    }

    #[test]
    fn la_veille_attend_plus_que_sa_cadence() {
        assert!(SEUIL > CADENCE);
    }
}
