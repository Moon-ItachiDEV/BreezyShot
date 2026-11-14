//! Vidéo et GIF, côté gestes (contrat 0.7, §4.2, §5, F1 à F37 côté cœur) : la visée en
//! mode film, la fenêtre de contrôle (par `plus_tard_nomme`, jamais dans un rappel), le
//! décompte du film, les fils `film`, `film-video`, `film-gif`, `film-son`, `film-sortie`
//! et `film-codeurs` (F37), la Finalisation, la sortie après la vignette (F36), l'arrêt
//! (F35). Les décisions sont dans `film.rs`.
//!
//! Points d'accroche de `mod.rs`, tous sur le fil principal :
//! - `capturer` : la séquence du mode une fois en Gel (`apres_gel`, modes `video`, `gif`) ;
//! - `selection_montree` : la Sélection est montrée en mode film (affichage `id`, Échap
//!   déjà réservé) ;
//! - `zone_choisie` : zone tracée en Sélection (F2), la capture encore rangée
//!   (`ranger_capture`) ; la Sélection reste montrée jusqu'à `Selection::cacher`
//!   (`garder_si_film`) ;
//! - `visee_abandonnee` : la visée d'un film a été abandonnée (Échap, annulée, écrans,
//!   muette…), écran figé et sélection déjà cachés ;
//! - `signal_selection`, `echap`, `bureau_change`, `action_bandeau` (croix du disque du
//!   décompte) : vrais s'ils ont été pris ici ; faux, le chemin commun suit ;
//! - `raccourci` : un raccourci de capture pressé ; vrai s'il a été pris ici (choix B1 :
//!   le raccourci du film en cours l'arrête, `film::arret_par_raccourci`) ;
//! - `fenetres_bloquees` : de la visée à la fin de la Finalisation (F30) ;
//! - `traiter` : ce que les fils postent (`Evenement::Film`) ;
//! - `lancer_recherche` : la recherche du codeur H.264, si `film::recherche_voulue` (F32) ;
//!   elle rend `Evenement::VideoDisponible` ;
//! - `lancer_action_vignette`, `lire`, `demander_glisser` : Copier, Enregistrer, le rond
//!   de lecture (choix B2) et le glisser de la carte d'un film ;
//! - `fermer_pour_arret` : arrêt de l'app (F35), attente de la fermeture du fichier sur un
//!   canal, jamais par un `Evenement`.
//!
//! La Finalisation finie, la suite d'`ImagePrete` est `super::film_ferme` (vignette d'un
//! film, souffle depuis la zone), puis la sortie (`sortie::sortir_film`, fil
//! `film-sortie`).
//!
//! Socle : corps inertes (chantier F) ; rien n'est appelé tant que `film::CONSTRUIT` est
//! faux (ni menu, ni raccourci, ni fil), sauf la recherche du codeur, qui ne part pas non
//! plus (`film::recherche_voulue`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tauri::AppHandle;

use super::etats::{Declencheur, Mode};
use super::{film, Evenement};
use crate::fenetres::selection;
use crate::systeme::gel::Changement;
use crate::systeme::{mp4, Rect};

/// Ce que les fils du film postent au fil principal (`Evenement::Film`).
#[derive(Debug, Clone, PartialEq)]
pub enum Signal {}

/// Le film en cours (de la visée à la fin de la Finalisation), tenu par l'Application
/// (`super::film_en_cours`). Fil principal ; verrou jamais tenu pendant un appel à Tauri
/// ou à un module.
#[derive(Debug, Default)]
pub struct EnCours {}

/// La recherche du codeur est partie (une fois par lancement, F32).
static RECHERCHE_LANCEE: AtomicBool = AtomicBool::new(false);

/// Vrai de la visée à la fin de la Finalisation (F30) : le menu n'offre que « Quitter
/// BreezyShot », les raccourcis qui ouvrent une fenêtre et Annoter de l'Historique sont
/// ignorés. Socle : faux.
pub fn fenetres_bloquees(app: &AppHandle) -> bool {
    super::film_en_cours(app).is_some()
}

/// La séquence du mode `mode` (`video` ou `gif`), en Gel : contrôles, photo sans pointeur,
/// écran figé, Sélection en mode film, Échap réservé (le chemin commun).
pub fn capturer(app: &AppHandle, mode: Mode, recu_a: Instant, bandeau_cache: bool) {
    super::capturer_avec_selection(app, recu_a, mode, bandeau_cache);
}

/// La Sélection est montrée en mode film (affichage `id`). Socle : rien.
pub fn selection_montree(app: &AppHandle, id: u64) {
    let _ = (app, id);
}

/// Zone tracée (en Sélection, capture encore rangée). Socle : la capture est abandonnée
/// sans message (la garde cache aussi la Sélection gardée).
pub fn zone_choisie(app: &AppHandle, zone: Rect) {
    let _ = zone;
    super::abandonner(app, Declencheur::SelectionAnnulee, None);
}

/// La visée d'un film est abandonnée (écran figé et sélection déjà cachés).
pub fn visee_abandonnee(app: &AppHandle) {
    // Pris sous le verrou, détruit hors du verrou.
    let _ancien = super::film_en_cours(app).take();
}

/// Un signal de la Sélection : vrai s'il est pris ici.
pub fn signal_selection(app: &AppHandle, signal: &selection::Signal) -> bool {
    let _ = (app, signal);
    false
}

/// Échap global : vrai s'il est pris ici (décompte ou film en cours).
pub fn echap(app: &AppHandle) -> bool {
    let _ = app;
    false
}

/// Écrans changés ou session verrouillée pendant l'écran figé : vrais s'ils sont pris ici.
pub fn bureau_change(app: &AppHandle, changement: Changement) -> bool {
    let _ = (app, changement);
    false
}

/// Une action du bandeau (la croix du disque du décompte du film, F8) : vraie si elle est
/// prise ici.
pub fn action_bandeau(app: &AppHandle, id: u64, action: &str) -> bool {
    let _ = (app, id, action);
    false
}

/// Un raccourci de capture pressé (`mode`) : vrai s'il est pris ici. Choix B1 : pendant
/// un film, son propre raccourci l'arrête (`film::arret_par_raccourci`), les autres sont
/// laissés à la table, qui les ignore.
pub fn raccourci(app: &AppHandle, mode: Mode) -> bool {
    let _ = (app, mode, film::ARRET_PAR_RACCOURCI);
    false
}

/// Ce que les fils postent.
pub fn traiter(app: &AppHandle, signal: Signal) {
    // La Finalisation finie, la suite d'`ImagePrete` sera `super::film_ferme`.
    let _ = (app, super::film_ferme);
    match signal {}
}

/// La recherche du codeur H.264 (F32, fil `film-codeurs`), une fois par lancement, et
/// seulement si `film::recherche_voulue` : elle rend `Evenement::VideoDisponible`. Avant,
/// aucune DLL de Media Foundation n'est chargée (F31).
pub fn lancer_recherche(app: &AppHandle) {
    if !film::recherche_voulue(super::pro(app)) || RECHERCHE_LANCEE.swap(true, Ordering::SeqCst) {
        return;
    }
    let pour = app.clone();
    let lance = std::thread::Builder::new()
        .name("film-codeurs".into())
        .spawn(move || {
            let present = mp4::codeur_h264_present();
            super::traiter(&pour, Evenement::VideoDisponible(present));
        });
    if let Err(e) = lance {
        log::warn!("codeur video non cherche: {e}");
    }
}

/// Copier (`copier`) ou Enregistrer depuis la carte d'un film (F25, F36). Socle : rien.
pub fn lancer_action_vignette(app: &AppHandle, id: u64, copier: bool) {
    let _ = (app, id, copier);
}

/// Le rond de lecture de la carte `id` (choix B2) : le film s'ouvre dans le lecteur de
/// Windows (`systeme::ouvrir_fichier`, jamais sur le fil principal). Socle : rien.
pub fn lire(app: &AppHandle, id: u64) {
    let _ = (app, id);
}

/// Appui sur la carte d'un film puis déplacement : le fichier à donner est posé
/// (`sortie::fichier_a_glisser`), puis le glisser reposté. Socle : rien.
pub fn demander_glisser(app: &AppHandle, id: u64) {
    let _ = (app, id);
}

/// Arrêt de l'app (F35) : demande la fin au fil `film`, puis attend la fermeture du
/// fichier sur un canal, `delai` au plus. Vrai si rien n'était en cours ou si le fichier
/// est fermé ; faux sinon (`arreter` écrit alors `film: fichier non ferme a l'arret`).
pub fn fermer_pour_arret(app: &AppHandle, delai: Duration) -> bool {
    let _ = delai;
    // Pris sous le verrou, détruit hors du verrou.
    let _ancien = super::film_en_cours(app).take();
    true
}
