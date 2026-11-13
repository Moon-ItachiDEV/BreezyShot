//! Copier le texte, côté gestes (contrat 0.5, §5) : la lecture dans le fil `texte`, son
//! échéance, l'écran figé gardé le temps du balayage, le résultat. Les décisions sont dans
//! `texte.rs`.
//!
//! Points d'accroche de `mod.rs` : `lancer_sonde` (démarrage, fil `texte-langues`, qui
//! poste `Evenement::TexteDisponible`), `zone_choisie` (zone choisie en mode texte),
//! `texte_lu` et `echu` (`Evenement::TexteLu`, `Evenement::TexteEchu`), `lire_export`
//! (bouton « Texte » de l'éditeur, export de but `texte`), `raccourci_reserve`, `arreter`.
//!
//! Aucune lecture ni copie sur le fil principal. L'écran figé (sa `GardeEcran`) est rangé
//! à part, dans `ECRAN` : le fil `texte-ecran` le libère à `ECRAN_FIGE_MAX_LECTURE` sans
//! passer par la boucle, même si la lecture ne rend rien.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use super::editeur::{But, EnTete};
use super::etats::Declencheur;
use super::texte::{self, FinLecture, LectureEnCours};
use super::{zone, CaptureEnCours, Evenement, GardeEcran};
use crate::capture;
use crate::fenetres::editeur as fenetre_editeur;
use crate::fenetres::selection::Selection;
use crate::raccourcis::ChoixAction;
use crate::sortie::{self, Etape};
use crate::systeme::gel::Pixels;
use crate::systeme::{self, Rect};
use crate::texte::{self as lecture, Issue};

/// Dernier numéro de lecture donné (raccourci et menu).
static NUMERO: AtomicU64 = AtomicU64::new(0);

/// L'annonce « Aucun raccourci pour le texte » est faite une fois par lancement.
static ANNONCE_FAITE: AtomicBool = AtomicBool::new(false);

/// L'écran figé d'une lecture : sa garde (prise à la libération) et quand il a été caché.
struct EcranGarde {
    numero: u64,
    relache: Instant,
    garde: Option<GardeEcran>,
    cache_apres: Option<Duration>,
}

/// L'écran figé de la lecture en cours (de n'importe quel fil ; verrou jamais tenu pendant
/// qu'on cache).
static ECRAN: Mutex<Option<EcranGarde>> = Mutex::new(None);

fn ecran() -> std::sync::MutexGuard<'static, Option<EcranGarde>> {
    ECRAN.lock().unwrap_or_else(|e| e.into_inner())
}

/// Libère l'écran figé de la lecture `numero` s'il est encore montré (sûr de n'importe
/// quel fil). Rend le délai, depuis le relâcher, auquel il a été caché ; `None` si cet
/// écran n'est pas (ou plus) celui rangé.
fn liberer_ecran(numero: u64) -> Option<Duration> {
    let (garde, apres) = {
        let mut place = ecran();
        let e = place.as_mut().filter(|e| e.numero == numero)?;
        let apres = *e.cache_apres.get_or_insert_with(|| e.relache.elapsed());
        (e.garde.take(), apres)
    };
    // Hors du verrou : la garde cache l'écran figé et la sélection, libère Échap.
    drop(garde);
    Some(apres)
}

/// Libère l'écran figé de la lecture `numero` et l'oublie.
fn oublier_ecran(numero: u64) -> Option<Duration> {
    let apres = liberer_ecran(numero);
    let ancien = ecran().take_if(|e| e.numero == numero);
    drop(ancien);
    apres
}

fn dormir_jusqu_a(moment: Instant) {
    let reste = moment.saturating_duration_since(Instant::now());
    if !reste.is_zero() {
        std::thread::sleep(reste);
    }
}

/// Démarrage : cherche les langues de lecture dans un petit fil, puis poste
/// `Evenement::TexteDisponible` (R10 : sans langue, la fonction reste absente).
pub fn lancer_sonde(app: &AppHandle) {
    if !texte::CONSTRUIT {
        return;
    }
    let pour = app.clone();
    let lance = std::thread::Builder::new()
        .name("texte-langues".into())
        .spawn(move || {
            let d = systeme::texte::disponibilite();
            log::info!(
                "{}",
                texte::ligne_langues(&d, systeme::texte::essai_sans_texte())
            );
            super::traiter(&pour, Evenement::TexteDisponible(texte::disponible(&d)));
        });
    if let Err(e) = lance {
        log::warn!("langues de lecture non cherchees: {e}");
    }
}

/// Le raccourci de `texte` vient d'être réservé (ou non) à l'arrivée des langues de
/// lecture ou de la licence : sans raccourci, l'annonce « Aucun raccourci pour le texte »,
/// une fois par lancement.
pub fn raccourci_reserve(app: &AppHandle, choix: &ChoixAction) {
    let Some(message) = texte::annonce_sans_raccourci(choix) else {
        return;
    };
    if ANNONCE_FAITE.swap(true, Ordering::SeqCst) {
        return;
    }
    log::info!("annonce raccourci: action_sans_raccourci");
    super::afficher(app, message);
}

/// Zone choisie en mode texte (fil principal, état Sélection, capture encore rangée) :
/// Lecture, découpe (l'écran figé reste montré), Échap libéré, balayage, lecture dans le
/// fil `texte` et son échéance, écran figé gardé de `balayage` à `ECRAN_FIGE_MAX_LECTURE`.
pub fn zone_choisie(app: &AppHandle, zone: Rect) {
    let relache = Instant::now();
    let Some(capture) = super::ranger_capture(app, None) else {
        // Plus de capture : la sélection gardée pour le balayage est cachée quand même.
        if let Some(s) = app.try_state::<Selection>() {
            s.cacher();
        }
        return;
    };
    let CaptureEnCours { gel, mut garde, .. } = capture;
    if super::appliquer(app, Declencheur::TexteChoisi).is_none() {
        garde.cacher_tout();
        return;
    }
    log::info!("{}", zone::ligne_zone_texte(zone));
    super::liberer_echap(app);
    let pixels = capture::decouper(&gel, zone);
    drop(gel);
    let pixels = match pixels {
        Ok(p) => p,
        Err(e) => {
            log::warn!("decoupe impossible: {e}");
            garde.cacher_tout();
            let issue = Issue::Echec("decoupe".into());
            log::info!("{}", texte::ligne_issue(&issue, Duration::ZERO, 1.0));
            super::appliquer(app, Declencheur::LectureImpossible);
            super::afficher(app, texte::message_impossible());
            return;
        }
    };
    let numero = NUMERO.fetch_add(1, Ordering::SeqCst) + 1;
    let reduites = super::reduites_editeur(app);
    let minimum = texte::balayage(reduites);
    if let Some(s) = app.try_state::<Selection>() {
        s.balayer(zone, reduites);
    }
    let ancien = ecran().replace(EcranGarde {
        numero,
        relache,
        garde: Some(garde),
        cache_apres: None,
    });
    drop(ancien);
    *super::lecture_en_cours(app) = Some(LectureEnCours {
        numero,
        relache,
        minimum,
        fin: None,
    });
    garder_ecran(app, numero, relache, minimum);
    lancer_lecture(app, numero, None, pixels);
}

/// Le fil `texte-ecran` : au bout du balayage, le résultat déjà arrivé est montré (fil
/// principal) ; à la borne, l'écran figé est libéré d'ici, quoi qu'il arrive.
fn garder_ecran(app: &AppHandle, numero: u64, relache: Instant, minimum: Duration) {
    let pour = app.clone();
    let lance = std::thread::Builder::new()
        .name("texte-ecran".into())
        .spawn(move || {
            dormir_jusqu_a(relache + minimum);
            let p = pour.clone();
            if pour
                .run_on_main_thread(move || balayage_fini(&p, numero))
                .is_err()
            {
                liberer_ecran(numero);
                return;
            }
            dormir_jusqu_a(relache + texte::ECRAN_FIGE_MAX_LECTURE);
            liberer_ecran(numero);
        });
    if let Err(e) = lance {
        log::warn!("garde de l'ecran fige impossible ({e}): ecran libere");
        liberer_ecran(numero);
    }
}

/// La lecture de `pixels` dans le fil `texte` (la copie aussi, pour le raccourci), puis
/// `Evenement::TexteLu` ; le fil `texte-echeance` tient `DELAI_LECTURE`. `document` :
/// l'éditeur (`Some(id)`) ou le raccourci.
fn lancer_lecture(app: &AppHandle, numero: u64, document: Option<u64>, pixels: Pixels) {
    let (fini, attente) = mpsc::channel::<()>();
    let pour = app.clone();
    let lance = std::thread::Builder::new()
        .name("texte".into())
        .spawn(move || {
            let debut = Instant::now();
            let facteur = texte::facteur_de(
                pixels.largeur,
                pixels.hauteur,
                systeme::texte::dimension_max(),
            );
            let issue = lecture::lire(&pixels);
            let duree = debut.elapsed();
            drop(pixels);
            if let Some(d) = systeme::texte::derniere_creation() {
                log::info!("{}", texte::ligne_moteur(d));
            }
            let copie = match (&issue, document) {
                (Issue::Texte(lu), None) => Some(sortie::copier_texte(&lecture::assembler(
                    lu,
                    systeme::texte::fin_de_ligne(),
                ))),
                _ => None,
            };
            let fin = FinLecture {
                numero,
                issue,
                copie,
                duree,
                document,
                facteur,
            };
            super::traiter(&pour, Evenement::TexteLu(Box::new(fin)));
            let _ = fini.send(());
        });
    if let Err(e) = lance {
        log::warn!("fil de lecture impossible: {e}");
        let fin = FinLecture {
            numero,
            issue: Issue::Echec("fil".into()),
            copie: None,
            duree: Duration::ZERO,
            document,
            facteur: 1.0,
        };
        texte_lu(app, fin);
        return;
    }
    // L'échéance : finie dès que la lecture a rendu ; un fil arrêté sans rien dire (canal
    // fermé) la fait partir tout de suite ; arrivée après le résultat, elle est ignorée.
    let pour_echeance = app.clone();
    let echeance = std::thread::Builder::new()
        .name("texte-echeance".into())
        .spawn(move || {
            if attente.recv_timeout(lecture::DELAI_LECTURE).is_ok() {
                return;
            }
            match document {
                None => super::traiter(&pour_echeance, Evenement::TexteEchu(numero)),
                Some(id) => {
                    let p = pour_echeance.clone();
                    let _ = pour_echeance.run_on_main_thread(move || echu_editeur(&p, id, numero));
                }
            }
        });
    if let Err(e) = echeance {
        log::warn!("echeance de lecture non armee: {e}");
    }
}

/// Le résultat d'une lecture (fil principal). Raccourci : montré tout de suite si le
/// balayage est fini, sinon gardé jusqu'à sa fin (`balayage_fini`).
pub fn texte_lu(app: &AppHandle, fin: FinLecture) {
    if let Some(id) = fin.document {
        lu_editeur(app, id, fin);
        return;
    }
    let mut fin = Some(fin);
    {
        let mut lecture = super::lecture_en_cours(app);
        let numero = fin.as_ref().map_or(0, |f| f.numero);
        let Some(x) = lecture.as_mut().filter(|x| x.numero == numero) else {
            log::info!("texte lu {numero}: resultat perime ignore");
            return;
        };
        if !texte::liberer_ecran(x.relache.elapsed(), x.minimum, true) {
            x.fin = fin.take().map(Box::new);
        }
    }
    if let Some(fin) = fin {
        finir(app, fin);
    }
}

/// Le balayage de la lecture `numero` est fini (fil principal) : son résultat, s'il est
/// arrivé, est montré.
fn balayage_fini(app: &AppHandle, numero: u64) {
    let fin = super::lecture_en_cours(app)
        .as_mut()
        .filter(|x| x.numero == numero)
        .and_then(|x| x.fin.take());
    if let Some(fin) = fin {
        finir(app, *fin);
    }
}

/// L'échéance de la lecture `numero` est tombée (fil principal) : « Impossible de lire le
/// texte ». Un résultat déjà arrivé (gardé pour la fin du balayage) l'emporte.
pub fn echu(app: &AppHandle, numero: u64) {
    let attendue = super::lecture_en_cours(app)
        .as_ref()
        .is_some_and(|x| x.numero == numero && x.fin.is_none());
    if !attendue {
        return;
    }
    finir(
        app,
        FinLecture {
            numero,
            issue: Issue::Echec(texte::CAUSE_ECHEANCE.into()),
            copie: None,
            duree: lecture::DELAI_LECTURE,
            document: None,
            facteur: 1.0,
        },
    );
}

/// Fin d'une lecture du raccourci (fil principal) : l'écran figé libéré s'il ne l'est pas
/// encore, journal, Repos, bandeau.
fn finir(app: &AppHandle, fin: FinLecture) {
    let Some(x) = super::lecture_en_cours(app).take_if(|x| x.numero == fin.numero) else {
        return;
    };
    let gel_cache = oublier_ecran(fin.numero).unwrap_or_else(|| x.relache.elapsed());
    log::info!("{}", texte::ligne_issue(&fin.issue, fin.duree, fin.facteur));
    if let Some(ligne) = texte::ligne_copie(fin.copie.as_ref()) {
        log::info!("{ligne}");
    }
    super::appliquer(app, texte::declencheur(&fin.issue));
    super::afficher(app, texte::message_fin(&fin));
    log::info!("{}", texte::ligne_chrono(gel_cache, x.relache.elapsed()));
}

/// L'image composée de l'éditeur à lire (export `texte` accepté, fil principal ; l'export
/// en cours est noté, `exportee` n'est pas touchée).
pub fn lire_export(app: &AppHandle, numero: u64, entete: EnTete, pixels: Pixels) {
    lancer_lecture(app, numero, Some(entete.id), pixels);
}

/// Vrai (et l'export oublié) si `numero` est la lecture attendue par l'éditeur `id`.
fn prendre_lecture_editeur(app: &AppHandle, id: u64, numero: u64) -> bool {
    super::avec_editeur(app, Some(id), |e| {
        let attendue = e
            .export
            .is_some_and(|x| x.numero == numero && x.but == But::Texte);
        if attendue {
            e.export = None;
        }
        attendue
    })
    .unwrap_or(false)
}

/// Résultat d'une lecture de l'éditeur (fil principal) : la lecture gardée, les boîtes
/// envoyées à la page (jamais le texte).
fn lu_editeur(app: &AppHandle, id: u64, fin: FinLecture) {
    log::info!(
        "{}",
        texte::ligne_editeur(id, fin.numero, &fin.issue, fin.duree)
    );
    if !prendre_lecture_editeur(app, id, fin.numero) {
        log::info!("editeur texte {id} {}: resultat perime ignore", fin.numero);
        return;
    }
    let charge = texte::charge_editeur(id, fin.numero, &fin.issue);
    let garde = match fin.issue {
        Issue::Texte(lu) => Some((fin.numero, lu)),
        _ => None,
    };
    super::avec_editeur(app, Some(id), |e| e.texte = garde);
    fenetre_editeur::emettre(app, texte::EVENEMENT_EDITEUR_TEXTE, charge);
}

/// L'échéance de la lecture `numero` de l'éditeur `id` (fil principal).
fn echu_editeur(app: &AppHandle, id: u64, numero: u64) {
    if !prendre_lecture_editeur(app, id, numero) {
        return;
    }
    let issue = Issue::Echec(texte::CAUSE_ECHEANCE.into());
    log::info!(
        "{}",
        texte::ligne_editeur(id, numero, &issue, lecture::DELAI_LECTURE)
    );
    super::avec_editeur(app, Some(id), |e| e.texte = None);
    fenetre_editeur::emettre(
        app,
        texte::EVENEMENT_EDITEUR_TEXTE,
        texte::charge_editeur(id, numero, &issue),
    );
}

/// `editeur_copier_texte` décidé : copie dans le fil `texte-copie`, puis
/// `editeur:texte_copie`. `choix` : le texte et le nombre de mots, ou le code du refus.
pub(super) fn copier_extrait(
    app: &AppHandle,
    id: u64,
    numero: u64,
    choix: Result<(String, usize), &'static str>,
) {
    let (extrait, mots) = match choix {
        Ok(c) => c,
        Err(code) => {
            rendre_copie(app, id, numero, Err(code));
            return;
        }
    };
    let pour = app.clone();
    let lance = std::thread::Builder::new()
        .name("texte-copie".into())
        .spawn(move || {
            let etape = sortie::copier_texte(&extrait);
            drop(extrait);
            match etape {
                Etape::Faite => rendre_copie(&pour, id, numero, Ok(mots)),
                Etape::Echec(c) => rendre_copie(&pour, id, numero, Err(c.as_str())),
                Etape::NonDemandee => rendre_copie(&pour, id, numero, Err("non_demandee")),
            }
        });
    if let Err(e) = lance {
        log::warn!("fil de copie du texte impossible: {e}");
        rendre_copie(app, id, numero, Err("fil"));
    }
}

/// Journal et `editeur:texte_copie` d'une copie faite ou refusée.
fn rendre_copie(app: &AppHandle, id: u64, numero: u64, issue: Result<usize, &str>) {
    log::info!("{}", texte::ligne_editeur_copie(id, numero, issue));
    fenetre_editeur::emettre(
        app,
        texte::EVENEMENT_EDITEUR_TEXTE_COPIE,
        texte::ChargeTexteCopie {
            id,
            numero,
            etat: if issue.is_ok() { "fait" } else { "echec" },
        },
    );
}

/// Arrêt de l'app : la lecture en cours est oubliée, son écran figé libéré.
pub fn arreter(app: &AppHandle) {
    super::lecture_en_cours(app).take();
    let ancien = ecran().take();
    drop(ancien);
}
