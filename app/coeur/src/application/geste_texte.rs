//! Copier le texte, côté gestes (contrat 0.5, §5) : la lecture dans le fil `texte`, son
//! échéance, l'écran figé gardé le temps du balayage, le surlignage des lignes copiées, le
//! résultat. Les décisions sont dans `texte.rs`.
//!
//! Points d'accroche de `mod.rs` : `lancer_sonde` (démarrage, fil `texte-langues`, qui
//! poste `Evenement::TexteDisponible`), `zone_choisie` (zone choisie en mode texte),
//! `texte_lu` et `echu` (`Evenement::TexteLu`, `Evenement::TexteEchu`), `lire_export`
//! (bouton « Texte » de l'éditeur, export de but `texte`), `bureau_change`,
//! `arreter`. Aucune annonce ici : le raccourci du texte entre dans le bilan du démarrage.
//!
//! Aucune lecture ni copie sur le fil principal. L'écran figé (sa `GardeEcran`) est rangé
//! à part, dans `ECRAN` : le fil `texte-ecran` le libère à `ECRAN_FIGE_MAX_LECTURE` sans
//! passer par la boucle, même si la lecture ne rend rien ; pendant un surlignage, c'est le
//! fil `texte-surligne` qui le libère, à la fin du surlignage.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use super::editeur::{self, EnTete};
use super::etats::Declencheur;
use super::texte::{self, Arrivee, Attendue, FinLecture, LectureEnCours};
use super::{zone, CaptureEnCours, Evenement, GardeEcran};
use crate::capture;
use crate::fenetres::editeur as fenetre_editeur;
use crate::fenetres::selection::Selection;
use crate::sortie::{self, Etape};
use crate::systeme::gel::Pixels;
use crate::systeme::{self, Rect};
use crate::texte::{self as lecture, Issue};

/// La sonde des langues de lecture est partie (une fois par lancement).
static SONDE_LANCEE: AtomicBool = AtomicBool::new(false);

/// Dernier numéro de lecture donné (raccourci et menu).
static NUMERO: AtomicU64 = AtomicU64::new(0);

/// La lecture du raccourci dont le résultat est attendu : le fil `texte` (qui copie
/// alors) ou l'échéance la prend, jamais les deux ; une lecture abandonnée ne copie jamais.
static ATTENDUE: Attendue = Attendue::new();

/// L'écran figé d'une lecture : sa garde (prise à la libération), quand il a été caché, la
/// zone lue (pixels réels du bureau) et si ses lignes sont en train d'être surlignées (la
/// borne ne libère alors plus l'écran).
struct EcranGarde<G = GardeEcran> {
    numero: u64,
    relache: Instant,
    garde: Option<G>,
    cache_apres: Option<Duration>,
    zone: Rect,
    surligne: bool,
}

/// Prend la garde de l'écran figé de la lecture `numero` (pure, testée sans écran) : la
/// garde si elle était encore là (`None` si elle a déjà été prise, à la borne par exemple),
/// et le délai depuis le relâcher auquel l'écran a été caché ; `None` si l'écran rangé
/// n'est pas celui de `numero`.
fn prendre_garde<G>(
    place: &mut Option<EcranGarde<G>>,
    numero: u64,
    maintenant: Instant,
) -> Option<(Option<G>, Duration)> {
    let e = place.as_mut().filter(|e| e.numero == numero)?;
    let apres = *e
        .cache_apres
        .get_or_insert_with(|| maintenant.saturating_duration_since(e.relache));
    Some((e.garde.take(), apres))
}

/// La borne `ECRAN_FIGE_MAX_LECTURE` de la lecture `numero` : comme `prendre_garde`, sauf
/// pendant le surlignage (rien n'est pris : sa fin libère l'écran).
fn prendre_garde_a_la_borne<G>(
    place: &mut Option<EcranGarde<G>>,
    numero: u64,
    maintenant: Instant,
) -> Option<(Option<G>, Duration)> {
    if surlignage_en_cours(place, numero) {
        return None;
    }
    prendre_garde(place, numero, maintenant)
}

/// Le surlignage de la lecture `numero` peut commencer si son écran figé est encore montré
/// (ni libéré à la borne, ni par un changement d'écrans) et pas déjà surligné : il est
/// alors noté (la borne ne libère plus l'écran) et la zone lue est rendue.
fn commencer_surlignage<G>(place: &mut Option<EcranGarde<G>>, numero: u64) -> Option<Rect> {
    let e = place
        .as_mut()
        .filter(|e| e.numero == numero && e.garde.is_some() && !e.surligne)?;
    e.surligne = true;
    Some(e.zone)
}

/// Le surlignage de la lecture `numero` n'a pas pu être joué : la borne reprend la main.
fn renoncer_surlignage<G>(place: &mut Option<EcranGarde<G>>, numero: u64) {
    if let Some(e) = place.as_mut().filter(|e| e.numero == numero) {
        e.surligne = false;
    }
}

/// Vrai pendant le surlignage de la lecture `numero`.
fn surlignage_en_cours<G>(place: &Option<EcranGarde<G>>, numero: u64) -> bool {
    place
        .as_ref()
        .is_some_and(|e| e.numero == numero && e.surligne)
}

/// Écrans changés : la garde de l'écran de la lecture en cours, si elle est encore là ; la
/// lecture n'est pas touchée. `(lecture en cours, garde prise)`.
fn garde_pour_bureau<G>(
    lecture: &Option<LectureEnCours>,
    place: &mut Option<EcranGarde<G>>,
    maintenant: Instant,
) -> (bool, Option<G>) {
    let Some(numero) = texte::lecture_a_liberer(lecture) else {
        return (false, None);
    };
    let garde = prendre_garde(place, numero, maintenant).and_then(|(g, _)| g);
    (true, garde)
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
///
/// Course connue, laissée : un fil suspendu entre le `take` et le `drop` de la garde,
/// repris après le début d'une capture suivante, cacherait l'écran figé de celle-ci
/// (`gel::cacher_ecran_fige` ne connaît pas de numéro) ; la sélection, elle, ne cache que
/// si aucun affichage n'est en cours.
fn liberer_ecran(numero: u64) -> Option<Duration> {
    let (garde, apres) = prendre_garde(&mut ecran(), numero, Instant::now())?;
    // Hors du verrou : la garde cache l'écran figé et la sélection, libère Échap.
    drop(garde);
    Some(apres)
}

/// La borne de la lecture `numero` (fil `texte-ecran`) : son écran figé est libéré, sauf
/// pendant le surlignage.
fn liberer_ecran_a_la_borne(numero: u64) {
    let prise = prendre_garde_a_la_borne(&mut ecran(), numero, Instant::now());
    // Hors du verrou, comme `liberer_ecran`.
    drop(prise);
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
    // Seulement avec une licence, une fois par lancement (le résultat reste connu après un
    // retrait puis une nouvelle activation).
    let deja = SONDE_LANCEE.load(Ordering::SeqCst);
    if !texte::sonder(texte::CONSTRUIT, super::pro(app), deja)
        || SONDE_LANCEE.swap(true, Ordering::SeqCst)
    {
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

/// Zone choisie en mode texte (fil principal, état Sélection, capture encore rangée) :
/// Lecture, découpe (l'écran figé reste montré), Échap libéré, balayage, lecture dans le
/// fil `texte` et son échéance, écran figé gardé de `balayage` à `ECRAN_FIGE_MAX_LECTURE`
/// (ou jusqu'à la fin du surlignage commencé avant).
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
        zone,
        surligne: false,
    });
    drop(ancien);
    *super::lecture_en_cours(app) = Some(LectureEnCours {
        numero,
        relache,
        minimum,
        fin: None,
    });
    ATTENDUE.poser(numero);
    if !garder_ecran(app, numero, relache, minimum) {
        // Sans fil de garde : le résultat sera montré dès son arrivée.
        if let Some(x) = super::lecture_en_cours(app).as_mut() {
            x.minimum = Duration::ZERO;
        }
    }
    lancer_lecture(app, numero, None, pixels);
}

/// Le fil `texte-ecran` : au bout du balayage, le résultat déjà arrivé est montré (fil
/// principal) ; à la borne, l'écran figé est libéré d'ici, quoi qu'il arrive, sauf pendant
/// le surlignage (le fil `texte-surligne` le libère à sa fin).
fn garder_ecran(app: &AppHandle, numero: u64, relache: Instant, minimum: Duration) -> bool {
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
            liberer_ecran_a_la_borne(numero);
        });
    if let Err(e) = lance {
        log::warn!("garde de l'ecran fige impossible ({e}): ecran libere");
        liberer_ecran(numero);
        return false;
    }
    true
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
            // Raccourci : seule la lecture encore attendue rend son résultat (et copie) ;
            // l'échéance tombée ou la lecture oubliée, rien n'est copié ni montré.
            if document.is_none() && !ATTENDUE.prendre(numero) {
                log::info!("texte lu {numero}: resultat perime ignore");
                let _ = fini.send(());
                return;
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
            // Avant le résultat : l'échéance se termine sans rien dire.
            let _ = fini.send(());
            super::traiter(&pour, Evenement::TexteLu(Box::new(fin)));
        });
    if let Err(e) = lance {
        log::warn!("fil de lecture impossible: {e}");
        arret_de_fil(app, numero, document);
        return;
    }
    // L'échéance : finie dès que la lecture a rendu ; un fil arrêté sans rien dire (canal
    // fermé) la fait partir tout de suite ; arrivée après le résultat, elle est ignorée.
    let pour_echeance = app.clone();
    let echeance = std::thread::Builder::new()
        .name("texte-echeance".into())
        .spawn(move || {
            let p = pour_echeance.clone();
            match (attente.recv_timeout(lecture::DELAI_LECTURE), document) {
                (Ok(()), _) => {}
                (Err(mpsc::RecvTimeoutError::Timeout), None) => {
                    super::traiter(&pour_echeance, Evenement::TexteEchu(numero));
                }
                (Err(mpsc::RecvTimeoutError::Timeout), Some(id)) => {
                    let _ = pour_echeance.run_on_main_thread(move || {
                        echu_editeur(&p, id, numero, texte::CAUSE_ECHEANCE);
                    });
                }
                // Canal fermé sans réponse : le fil `texte` s'est arrêté (panique).
                (Err(mpsc::RecvTimeoutError::Disconnected), _) => {
                    let _ = pour_echeance.run_on_main_thread(move || {
                        arret_de_fil(&p, numero, document);
                    });
                }
            }
        });
    if let Err(e) = echeance {
        log::warn!("echeance de lecture non armee: {e}");
        arret_de_fil(app, numero, document);
    }
}

/// Un fil de la lecture `numero` n'a pas pu être lancé ou s'est arrêté (fil principal) :
/// la lecture finit tout de suite en échec `fil` (jamais d'état Lecture sans fin).
fn arret_de_fil(app: &AppHandle, numero: u64, document: Option<u64>) {
    match document {
        Some(id) => echu_editeur(app, id, numero, texte::CAUSE_FIL),
        None => {
            let fin = texte::echeance(&super::lecture_en_cours(app), numero, texte::CAUSE_FIL);
            if let Some(fin) = fin {
                finir(app, fin);
            }
        }
    }
}

/// Le résultat d'une lecture (fil principal). Raccourci : montré tout de suite si le
/// balayage est fini, sinon gardé jusqu'à sa fin (`balayage_fini`).
pub fn texte_lu(app: &AppHandle, fin: FinLecture) {
    if let Some(id) = fin.document {
        lu_editeur(app, id, fin);
        return;
    }
    let numero = fin.numero;
    let suite = texte::arrivee(&mut super::lecture_en_cours(app), fin, Instant::now());
    match suite {
        Arrivee::Finir(fin) => finir(app, fin),
        Arrivee::Gardee => {}
        Arrivee::Perimee => log::info!("texte lu {numero}: resultat perime ignore"),
    }
}

/// Le balayage de la lecture `numero` est fini (fil principal) : son résultat, s'il est
/// arrivé, est montré. Pendant le surlignage, le résultat gardé attend sa fin.
fn balayage_fini(app: &AppHandle, numero: u64) {
    let fin = fin_du_balayage(&ecran(), &mut super::lecture_en_cours(app), numero);
    if let Some(fin) = fin {
        finir(app, fin);
    }
}

/// La fin gardée à montrer au bout du balayage de la lecture `numero` (pure, testée sans
/// écran) : aucune pendant son surlignage, que ce réveil tardif couperait sinon.
fn fin_du_balayage<G>(
    place: &Option<EcranGarde<G>>,
    lecture: &mut Option<LectureEnCours>,
    numero: u64,
) -> Option<FinLecture> {
    if surlignage_en_cours(place, numero) {
        return None;
    }
    texte::balayage_fini(lecture, numero)
}

/// L'échéance de la lecture `numero` est tombée (fil principal) : « Impossible de lire le
/// texte ». Un résultat déjà arrivé (gardé pour la fin du balayage) l'emporte.
pub fn echu(app: &AppHandle, numero: u64) {
    // Le fil `texte` a pris le résultat (il arrive) : l'échéance ne dit rien.
    if !ATTENDUE.prendre(numero) {
        return;
    }
    let fin = texte::echeance(&super::lecture_en_cours(app), numero, texte::CAUSE_ECHEANCE);
    if let Some(fin) = fin {
        finir(app, fin);
    }
}

/// Fin d'une lecture du raccourci (fil principal) : un texte copié est d'abord surligné sur
/// la zone, l'écran figé encore montré (`surligner`) ; sinon, ou à la fin du surlignage,
/// `conclure`.
fn finir(app: &AppHandle, fin: FinLecture) {
    if let Some(fin) = surligner(app, fin) {
        conclure(app, fin);
    }
}

/// Surligne les lignes du texte copié de `fin` si son écran figé est encore montré
/// (`selection:surligner`) : la fin est gardée dans la lecture, le fil `texte-surligne`
/// libère l'écran au bout du surlignage puis la montre (`surlignage_fini`). Rend la fin à
/// montrer tout de suite si rien n'est surligné (pas un texte copié, écran déjà libéré,
/// aucune ligne, sélection absente, fil impossible).
fn surligner(app: &AppHandle, fin: FinLecture) -> Option<FinLecture> {
    let numero = fin.numero;
    if !texte::surlignable(&super::lecture_en_cours(app), &fin) {
        return Some(fin);
    }
    let Issue::Texte(lu) = &fin.issue else {
        return Some(fin);
    };
    let Some(zone) = commencer_surlignage(&mut ecran(), numero) else {
        return Some(fin);
    };
    let lignes = texte::lignes_lues(lu, zone);
    let reduites = super::reduites_editeur(app);
    let deroule = texte::deroule(lignes.len(), reduites);
    let joue = app
        .try_state::<Selection>()
        .is_some_and(|s| s.surligner(&lignes, reduites, deroule));
    if !joue || !attendre_surlignage(app, numero, texte::duree_surlignage(lignes.len(), reduites)) {
        renoncer_surlignage(&mut ecran(), numero);
        return Some(fin);
    }
    match super::lecture_en_cours(app).as_mut() {
        Some(x) if x.numero == numero => {
            x.fin = Some(Box::new(fin));
            None
        }
        _ => Some(fin),
    }
}

/// Le fil `texte-surligne` : au bout de `duree`, l'écran figé et la sélection sont libérés
/// d'ici, quoi qu'il arrive, puis la fin gardée est montrée (fil principal). Faux si le fil
/// n'a pas pu partir.
fn attendre_surlignage(app: &AppHandle, numero: u64, duree: Duration) -> bool {
    let pour = app.clone();
    let lance = std::thread::Builder::new()
        .name("texte-surligne".into())
        .spawn(move || {
            std::thread::sleep(duree);
            liberer_ecran(numero);
            let p = pour.clone();
            let _ = pour.run_on_main_thread(move || surlignage_fini(&p, numero));
        });
    if let Err(e) = lance {
        log::warn!("surlignage sans fil ({e}): fin montree tout de suite");
        return false;
    }
    true
}

/// Le surlignage de la lecture `numero` est fini (fil principal) : sa fin gardée est
/// montrée.
fn surlignage_fini(app: &AppHandle, numero: u64) {
    renoncer_surlignage(&mut ecran(), numero);
    let fin = texte::balayage_fini(&mut super::lecture_en_cours(app), numero);
    if let Some(fin) = fin {
        conclure(app, fin);
    }
}

/// Montre la fin d'une lecture du raccourci (fil principal) : l'écran figé libéré s'il ne
/// l'est pas encore, journal, Repos, bandeau.
fn conclure(app: &AppHandle, fin: FinLecture) {
    let Some(x) = texte::terminer(&mut super::lecture_en_cours(app), fin.numero) else {
        return;
    };
    // Plus rien d'attendu : un résultat tardif ne copie pas.
    ATTENDUE.prendre(fin.numero);
    let gel_cache = oublier_ecran(fin.numero).unwrap_or_else(|| x.relache.elapsed());
    log::info!("{}", texte::ligne_issue(&fin.issue, fin.duree, fin.facteur));
    if let Some(ligne) = texte::ligne_copie(fin.copie.as_ref()) {
        log::info!("{ligne}");
    }
    super::appliquer(app, texte::declencheur(&fin.issue));
    super::afficher(app, texte::message_fin(&fin));
    log::info!("{}", texte::ligne_chrono(gel_cache, x.relache.elapsed()));
}

/// Écrans changés ou session verrouillée pendant la Lecture (fil principal) : l'écran
/// figé et la sélection sont libérés tout de suite ; la lecture continue, finit, copie et
/// dit son message comme d'habitude. Faux sans lecture en cours.
pub fn bureau_change(app: &AppHandle) -> bool {
    let (en_lecture, garde) = {
        let lecture = super::lecture_en_cours(app);
        garde_pour_bureau(&lecture, &mut ecran(), Instant::now())
    };
    // Seulement si l'écran était encore montré (pas déjà libéré à la borne, ni par un
    // `Bureau` précédent). Hors des verrous : la garde cache l'écran figé et la sélection.
    if let Some(g) = garde {
        drop(g);
        log::info!("texte ecran libere: bureau change");
    }
    en_lecture
}

/// L'image composée de l'éditeur à lire (export `texte` accepté, fil principal ; l'export
/// en cours est noté, `exportee` n'est pas touchée).
pub fn lire_export(app: &AppHandle, numero: u64, entete: EnTete, pixels: Pixels) {
    lancer_lecture(app, numero, Some(entete.id), pixels);
}

/// Vrai (et l'export oublié) si `numero` est la lecture attendue par l'éditeur `id`.
fn prendre_lecture_editeur(app: &AppHandle, id: u64, numero: u64) -> bool {
    super::avec_editeur(app, Some(id), |e| editeur::prendre_lecture(e, numero)).unwrap_or(false)
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
fn echu_editeur(app: &AppHandle, id: u64, numero: u64, cause: &str) {
    if !prendre_lecture_editeur(app, id, numero) {
        return;
    }
    let issue = Issue::Echec(cause.into());
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
                Etape::Echec(c) => rendre_copie(&pour, id, numero, Err(texte::code_court(&c))),
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
    ATTENDUE.oublier();
    let ancien = ecran().take();
    drop(ancien);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rangee(numero: u64, relache: Instant) -> Option<EcranGarde<&'static str>> {
        Some(EcranGarde {
            numero,
            relache,
            garde: Some("garde"),
            cache_apres: None,
            zone: Rect {
                x: -100,
                y: 20,
                largeur: 300,
                hauteur: 80,
            },
            surligne: false,
        })
    }

    #[test]
    fn la_garde_n_est_prise_qu_une_fois() {
        let t = Instant::now();
        let ms = Duration::from_millis;
        let mut place = rangee(3, t);
        assert!(prendre_garde(&mut place, 2, t).is_none(), "autre lecture");
        assert_eq!(
            prendre_garde(&mut place, 3, t + ms(1500)),
            Some((Some("garde"), ms(1500)))
        );
        // Déjà prise (borne) : rien à cacher, le délai noté reste celui de la première fois.
        assert_eq!(
            prendre_garde(&mut place, 3, t + ms(2000)),
            Some((None, ms(1500)))
        );
        assert!(prendre_garde(&mut None::<EcranGarde<&str>>, 3, t).is_none());
    }

    #[test]
    fn surlignage_seulement_l_ecran_encore_montre_et_une_fois() {
        let t = Instant::now();
        let ms = Duration::from_millis;
        let zone = Rect {
            x: -100,
            y: 20,
            largeur: 300,
            hauteur: 80,
        };
        let mut place = rangee(3, t);
        assert_eq!(commencer_surlignage(&mut place, 2,), None, "autre lecture");
        assert!(!surlignage_en_cours(&place, 3));
        assert_eq!(commencer_surlignage(&mut place, 3), Some(zone));
        assert!(surlignage_en_cours(&place, 3));
        assert!(!surlignage_en_cours(&place, 2));
        // Déjà commencé : pas une seconde fois.
        assert_eq!(commencer_surlignage(&mut place, 3), None);
        // Pendant le surlignage, la borne ne prend rien ; la fin du surlignage, si.
        assert_eq!(prendre_garde_a_la_borne(&mut place, 3, t + ms(1500)), None);
        assert!(place.as_ref().is_some_and(|e| e.garde.is_some()));
        assert_eq!(
            prendre_garde(&mut place, 3, t + ms(1900)),
            Some((Some("garde"), ms(1900)))
        );
        // Écran déjà libéré (borne, écrans changés) : pas de surlignage.
        let mut libere = rangee(4, t);
        assert_eq!(
            prendre_garde_a_la_borne(&mut libere, 4, t + ms(1500)),
            Some((Some("garde"), ms(1500)))
        );
        assert_eq!(commencer_surlignage(&mut libere, 4), None);
        assert_eq!(commencer_surlignage(&mut None::<EcranGarde<&str>>, 4), None);
    }

    #[test]
    fn surlignage_abandonne_rend_la_main_a_la_borne() {
        let t = Instant::now();
        let ms = Duration::from_millis;
        let mut place = rangee(5, t);
        assert!(commencer_surlignage(&mut place, 5).is_some());
        // Une autre lecture ne touche rien.
        renoncer_surlignage(&mut place, 6);
        assert!(surlignage_en_cours(&place, 5));
        renoncer_surlignage(&mut place, 5);
        assert!(!surlignage_en_cours(&place, 5));
        assert_eq!(
            prendre_garde_a_la_borne(&mut place, 5, t + ms(1500)),
            Some((Some("garde"), ms(1500)))
        );
        renoncer_surlignage(&mut None::<EcranGarde<&str>>, 5);
    }

    #[test]
    fn fin_du_balayage_attend_le_surlignage() {
        let t = Instant::now();
        let fin = FinLecture {
            numero: 6,
            issue: crate::texte::Issue::Vide,
            copie: None,
            duree: Duration::ZERO,
            document: None,
            facteur: 1.0,
        };
        let gardee = || {
            Some(LectureEnCours {
                numero: 6,
                relache: t,
                minimum: texte::BALAYAGE,
                fin: Some(Box::new(fin.clone())),
            })
        };
        // Pendant le surlignage, un réveil tardif du balayage ne prend pas la fin gardée.
        let mut place = rangee(6, t);
        assert!(commencer_surlignage(&mut place, 6).is_some());
        let mut lecture = gardee();
        assert_eq!(fin_du_balayage(&place, &mut lecture, 6), None);
        assert_eq!(lecture.as_ref().and_then(|x| x.fin.as_deref()), Some(&fin));
        // Sans surlignage (ou celui d'une autre lecture), elle est rendue, une fois.
        let mut autre = rangee(7, t);
        assert!(commencer_surlignage(&mut autre, 7).is_some());
        assert_eq!(fin_du_balayage(&autre, &mut lecture, 6), Some(fin.clone()));
        assert_eq!(fin_du_balayage(&autre, &mut lecture, 6), None);
        let mut lecture = gardee();
        assert_eq!(
            fin_du_balayage(&rangee(6, t), &mut lecture, 6),
            Some(fin.clone())
        );
        let mut lecture = gardee();
        assert_eq!(
            fin_du_balayage(&None::<EcranGarde<&str>>, &mut lecture, 6),
            Some(fin)
        );
        assert_eq!(
            fin_du_balayage(&None::<EcranGarde<&str>>, &mut lecture, 5),
            None
        );
    }

    #[test]
    fn ecrans_changes_prennent_la_garde_une_fois_sans_toucher_la_lecture() {
        let t = Instant::now();
        let mut place = rangee(4, t);
        let fin = FinLecture {
            numero: 4,
            issue: crate::texte::Issue::Vide,
            copie: None,
            duree: Duration::ZERO,
            document: None,
            facteur: 1.0,
        };
        let lecture = Some(LectureEnCours {
            numero: 4,
            relache: t,
            minimum: texte::BALAYAGE,
            fin: Some(Box::new(fin.clone())),
        });
        assert_eq!(
            garde_pour_bureau(&lecture, &mut place, t),
            (true, Some("garde"))
        );
        // Un second `Bureau` : la lecture est toujours en cours, plus rien à libérer.
        assert_eq!(garde_pour_bureau(&lecture, &mut place, t), (true, None));
        // La lecture et son résultat gardé sont intacts.
        assert_eq!(lecture.as_ref().and_then(|x| x.fin.as_deref()), Some(&fin));
        // Hors Lecture : rien.
        let mut autre = rangee(5, t);
        assert_eq!(garde_pour_bureau(&None, &mut autre, t), (false, None));
        assert!(autre.is_some_and(|e| e.garde.is_some()));
    }
}
