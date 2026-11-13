//! L'historique, côté gestes (contrat 0.5, §3 et §4) : ouvrir la fenêtre, tenir son état,
//! faire les actions des cases dans des fils. Les décisions sont dans `historique.rs`.
//!
//! Points d'accroche de `mod.rs` : `ouvrir` (raccourci, menu), `traiter`
//! (`Evenement::Historique`), `apres_sortie` (fil de sortie et fil d'export de l'éditeur,
//! une fois « Récents » écrit), `rafraichir` (après une suppression de la vignette),
//! `arreter` (arrêt de l'app).
//!
//! Règles : la fenêtre n'est jamais créée dans un rappel (toujours `plus_tard_nomme`) ;
//! aucun PNG n'est lu ni aucun dossier parcouru sur le fil principal ; le verrou de l'état
//! n'est jamais tenu pendant un appel à Tauri ou à un module ; les suppressions en attente
//! sont faites à la fermeture et à l'arrêt.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Manager};

use super::etats::Declencheur;
use super::historique::{
    self, ActionCase, EntreePage, EtatResultat, Geste, RaisonFermeture, SOUS_DOSSIER_APERCUS,
};
use super::{editeur, vignette, Application, Evenement};
use crate::capture;
use crate::fenetres::historique as fenetre;
use crate::sortie::{self, EntreeRecente};
use crate::systeme;
use crate::systeme::gel::Pixels;

/// Ce qui revient au fil principal pour l'historique (`Evenement::Historique`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    /// La fenêtre numéro `numero` est détruite.
    Detruite { numero: u64 },
}

/// Nom des fils qui postent au fil principal (`plus_tard_nomme`).
const FIL_POSTE: &str = "historique-poste";
/// Fenêtre encore en train d'être détruite : nouvel essai après ce délai, au plus
/// `ESSAIS_OUVERTURE` fois.
const ATTENTE_OUVERTURE: Duration = Duration::from_millis(100);
const ESSAIS_OUVERTURE: u32 = 20;

/// Dernier numéro de fenêtre donné.
static NUMERO: AtomicU64 = AtomicU64::new(0);
/// Une seule liste lue et envoyée à la fois : la dernière envoyée est la plus fraîche.
static LISTAGE: Mutex<()> = Mutex::new(());
/// Noms de « Récents » vus en dernier (sert à effacer l'aperçu d'une capture supprimée
/// par la vignette). `None` : rien vu depuis le démarrage.
static CONNUS: Mutex<Option<BTreeSet<String>>> = Mutex::new(None);
/// Le filet de la page : fermé à `historique_pret` (ou à la fermeture), son fil finit.
static FILET: Mutex<Option<Sender<()>>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize)]
struct ChargeListe {
    entrees: Vec<EntreePage>,
}

#[derive(Debug, Clone, Serialize)]
struct ChargeApercu {
    nom: String,
}

#[derive(Debug, Clone, Serialize)]
struct ChargeResultat {
    nom: String,
    action: &'static str,
    etat: &'static str,
    message: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
struct ChargeAttente {
    n: usize,
}

#[derive(Debug, Clone, Serialize)]
struct ChargeGlisser {
    nom: String,
    etat: &'static str,
}

/// `<local>/Recents`, `<local>/Apercus`, `<local>/Glisser` (vides sans dossier local).
struct Dossiers {
    recents: PathBuf,
    apercus: PathBuf,
    glisser: PathBuf,
}

fn dossiers(app: &AppHandle) -> Dossiers {
    let (recents, glisser) = super::dossiers_vignette(app);
    let apercus = apercus_de(&recents).unwrap_or_default();
    Dossiers {
        recents,
        apercus,
        glisser,
    }
}

/// `<local>/Apercus` d'après `<local>/Recents` (règle de `sortie`, chantier B).
fn apercus_de(recents: &Path) -> Option<PathBuf> {
    let local = recents.parent()?;
    (!local.as_os_str().is_empty()).then(|| local.join(SOUS_DOSSIER_APERCUS))
}

fn connus() -> std::sync::MutexGuard<'static, Option<BTreeSet<String>>> {
    CONNUS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Vrai si la fenêtre numéro `numero` est l'historique ouvert.
fn est_ouvert(app: &AppHandle, numero: u64) -> bool {
    super::historique_ouvert(app)
        .as_ref()
        .is_some_and(|e| e.numero == numero)
}

fn emettre_liste(app: &AppHandle, entrees: Vec<EntreePage>) {
    fenetre::emettre(app, fenetre::EVENEMENT_LISTE, ChargeListe { entrees });
}

fn emettre_attente(app: &AppHandle, n: usize) {
    fenetre::emettre(app, fenetre::EVENEMENT_ATTENTE, ChargeAttente { n });
}

fn emettre_resultat(
    app: &AppHandle,
    nom: String,
    action: ActionCase,
    etat: EtatResultat,
    message: Option<&'static str>,
) {
    let charge = ChargeResultat {
        nom,
        action: action.nom(),
        etat: etat.nom(),
        message,
    };
    fenetre::emettre(app, fenetre::EVENEMENT_RESULTAT, charge);
}

/// Lance `f` dans un fil nommé ; faux (et rien de fait) si le fil n'a pas pu être lancé.
fn dans_un_fil(nom: &str, f: impl FnOnce() + Send + 'static) -> bool {
    match std::thread::Builder::new().name(nom.into()).spawn(f) {
        Ok(_) => true,
        Err(e) => {
            log::warn!("historique: fil {nom} impossible ({e})");
            false
        }
    }
}

/// Comme `dans_un_fil`, mais sans fil le fait ici : seulement pour un effacement, qui ne
/// crée, ne montre ni ne détruit aucune fenêtre (il efface, relit et envoie).
fn dans_un_fil_ou_ici(nom: &str, f: impl FnOnce() + Send + 'static) {
    let f = Arc::new(Mutex::new(Some(f)));
    let pour_fil = Arc::clone(&f);
    let lance = std::thread::Builder::new().name(nom.into()).spawn(move || {
        if let Some(f) = pour_fil.lock().ok().and_then(|mut f| f.take()) {
            f();
        }
    });
    if let Err(e) = lance {
        log::warn!("historique: fil {nom} impossible ({e}): fait ici");
        if let Some(f) = f.lock().ok().and_then(|mut f| f.take()) {
            f();
        }
    }
}

// Ouverture et fermeture (§4.1).

/// Raccourci ou menu : ouvre la fenêtre, ou la ramène devant. Jamais créée ici : toujours
/// postée à un tour suivant de la boucle (`super::plus_tard_nomme`, fil
/// `historique-poste`).
pub fn ouvrir(app: &AppHandle) {
    let demande_a = Instant::now();
    super::plus_tard_nomme(app, FIL_POSTE, Duration::ZERO, move |a| {
        ouvrir_maintenant(a, demande_a, ESSAIS_OUVERTURE)
    });
}

/// Fil principal, à un tour de boucle à lui : ramène devant, attend la fin d'une
/// destruction, ou crée la fenêtre cachée.
fn ouvrir_maintenant(app: &AppHandle, demande_a: Instant, essais: u32) {
    let prete = super::historique_ouvert(app).as_ref().map(|e| e.page_prete);
    match historique::ouverture(prete, fenetre::ouverte(app), essais) {
        historique::Ouverture::Ramener => {
            if fenetre::ramener_devant(app) {
                log::info!("historique ramene devant");
            }
            return;
        }
        // La page se montrera d'elle-même à `historique_pret`.
        historique::Ouverture::Rien => return,
        historique::Ouverture::Abandonner => {
            log::warn!("historique impossible: fenetre (encore ouverte)");
            return;
        }
        // Encore entre `destroy()` et `Destroyed`.
        historique::Ouverture::Attendre => {
            super::plus_tard_nomme(app, FIL_POSTE, ATTENTE_OUVERTURE, move |a| {
                ouvrir_maintenant(a, demande_a, essais - 1)
            });
            return;
        }
        historique::Ouverture::Creer => {}
    }
    let numero = NUMERO.fetch_add(1, Ordering::SeqCst) + 1;
    // L'état est rangé avant la création, pour qu'une commande de la page le trouve.
    *super::historique_ouvert(app) = Some(historique::EtatHistorique::nouveau(numero, demande_a));
    let ecran = fenetre::ecran_souris(app).unwrap_or(super::ECRAN_DE_SECOURS);
    let place = fenetre::placer(&ecran);
    let pour_destruction = app.clone();
    // `Destroyed` est traité sur-le-champ (fil principal), pour cette fenêtre seulement.
    let detruite = move || {
        super::traiter(
            &pour_destruction,
            Evenement::Historique(Signal::Detruite { numero }),
        )
    };
    match fenetre::ouvrir(app, place, detruite) {
        Ok(identifiant) => {
            match identifiant {
                Some(h) => log::info!("historique fenetre {h}"),
                None => log::info!("historique fenetre ouverte"),
            }
            armer_filet(app, numero);
        }
        Err(e) => {
            super::historique_ouvert(app).take_if(|e| e.numero == numero);
            log::warn!("historique impossible: fenetre ({e})");
        }
    }
}

/// Filet : sans `historique_pret` de la fenêtre `numero` en `DELAI_PAGE`, elle est
/// détruite. Le fil attend sur un canal que `historique_pret` ferme : il finit aussitôt.
fn armer_filet(app: &AppHandle, numero: u64) {
    let (signal, attente) = mpsc::channel::<()>();
    *FILET.lock().unwrap_or_else(|e| e.into_inner()) = Some(signal);
    let pour_fil = app.clone();
    let lance = std::thread::Builder::new()
        .name("historique-filet".into())
        .spawn(move || {
            if attente.recv_timeout(historique::DELAI_PAGE) != Err(RecvTimeoutError::Timeout) {
                return;
            }
            let pour = pour_fil.clone();
            let _ = pour_fil.run_on_main_thread(move || {
                let muette =
                    super::historique_ouvert(&pour).take_if(|e| historique::page_muette(e, numero));
                if muette.is_some() {
                    log::warn!("historique impossible: page muette");
                    fenetre::fermer(&pour);
                }
            });
        });
    if let Err(e) = lance {
        log::warn!("historique: filet de la page non arme: {e}");
    }
}

/// La page a dit prête (ou la fenêtre se ferme) : le filet finit.
pub(super) fn desarmer_filet() {
    drop(FILET.lock().unwrap_or_else(|e| e.into_inner()).take());
}

/// Un signal de l'historique (fil principal).
pub fn traiter(app: &AppHandle, signal: Signal) {
    match signal {
        Signal::Detruite { numero } => {
            let etat = super::historique_ouvert(app).take_if(|e| e.numero == numero);
            if let Some(e) = etat {
                desarmer_filet();
                log::info!("{}", historique::ligne_fermeture(RaisonFermeture::Cadre));
                finir_suppressions(app, e, false);
            }
        }
    }
}

/// Ferme l'historique : état oublié, suppressions en attente faites (sur ce fil à
/// l'arrêt), fenêtre détruite (jamais cachée).
pub(super) fn fermer(app: &AppHandle, raison: RaisonFermeture) {
    desarmer_filet();
    let etat = super::historique_ouvert(app).take();
    if let Some(e) = etat {
        log::info!("{}", historique::ligne_fermeture(raison));
        finir_suppressions(app, e, raison == RaisonFermeture::Arret);
    }
    fenetre::fermer(app);
}

/// Arrêt : suppressions en attente faites, fenêtre détruite, état oublié.
pub fn arreter(app: &AppHandle) {
    fermer(app, RaisonFermeture::Arret);
}

// Liste et aperçus (§3.1).

/// Lit « Récents » (jamais sur le fil principal), garde la liste et l'envoie à la page si
/// la fenêtre est ouverte. Rend la liste lue.
fn relister(app: &AppHandle) -> Vec<EntreeRecente> {
    let _un_a_la_fois = LISTAGE.lock().unwrap_or_else(|e| e.into_inner());
    let d = dossiers(app);
    let debut = Instant::now();
    let liste = sortie::lister_recents(&d.recents);
    *connus() = Some(liste.iter().map(|e| e.nom.clone()).collect());
    let vues = super::historique_ouvert(app).as_mut().map(|e| {
        e.liste = liste.clone();
        e.vues()
    });
    if let Some(vues) = vues {
        let n = vues.len();
        emettre_liste(app, vues);
        log::info!(
            "historique liste {n} entrees en {} ms",
            debut.elapsed().as_millis()
        );
    }
    liste
}

/// La page est prête : la liste, puis les aperçus manquants (du plus récent au plus
/// ancien), chacun annoncé dès qu'il est prêt. Fil `historique`.
pub(super) fn lancer_liste(app: &AppHandle, numero: u64) {
    let app = app.clone();
    // Sans fil : rien (jamais de disque sur le fil principal).
    dans_un_fil("historique", move || {
        let liste = relister(&app);
        faire_apercus_manquants(&app, numero, &liste);
    });
}

fn faire_apercus_manquants(app: &AppHandle, numero: u64, liste: &[EntreeRecente]) {
    let d = dossiers(app);
    let debut = Instant::now();
    let mut faits = 0usize;
    for e in liste {
        if !est_ouvert(app, numero) {
            break;
        }
        let Some(chemin) = sortie::chemin_apercu(&d.apercus, &e.nom) else {
            continue;
        };
        if chemin.is_file() {
            continue;
        }
        let Some(recent) = sortie::chemin_recent(&d.recents, &e.nom) else {
            continue;
        };
        let Ok((pixels, _)) = sortie::lire_recent(&recent) else {
            continue;
        };
        let apercu = capture::apercu_historique(&pixels);
        drop(pixels);
        if sortie::ranger_apercu(&d.apercus, &e.nom, &apercu).is_ok() {
            faits += 1;
            let charge = ChargeApercu { nom: e.nom.clone() };
            fenetre::emettre(app, fenetre::EVENEMENT_APERCU, charge);
        }
    }
    if faits > 0 {
        log::info!(
            "historique apercus faits {faits} en {} ms",
            debut.elapsed().as_millis()
        );
    }
}

/// Après une sortie qui a rangé un fichier dans « Récents » (`bilan.recent`) : son aperçu,
/// puis la liste à jour si la fenêtre est ouverte. Appelé dans le fil de la sortie, jamais
/// sur le fil principal.
pub fn apres_sortie(app: &AppHandle, pixels: &Pixels, bilan: &sortie::Bilan) {
    let Some(Ok(chemin)) = &bilan.recent else {
        return;
    };
    let Some(nom) = historique::nom_du_chemin(chemin) else {
        return;
    };
    // Sans l'historique en vigueur (pas de licence) et fenêtre fermée : le chemin de la
    // 0.4, aucun aperçu ; les manquants se font à l'ouverture de l'historique.
    let ouverte = super::historique_ouvert(app).is_some();
    let avec_apercu = historique::apercu_a_la_sortie(&super::actions_en_vigueur(app), ouverte);
    if let Some(apercus) = chemin.parent().and_then(apercus_de).filter(|_| avec_apercu) {
        let apercu = capture::apercu_historique(pixels);
        if sortie::ranger_apercu(&apercus, &nom, &apercu).is_err() {
            log::warn!("historique apercu non fait");
        }
    }
    connus().get_or_insert_default().insert(nom);
    if super::historique_ouvert(app).is_some() {
        relister(app);
    }
}

/// « Récents » a changé (suppression de la vignette) : l'aperçu d'une capture partie part
/// aussi, et la liste est à jour si la fenêtre est ouverte. Jamais sur le fil principal.
pub fn rafraichir(app: &AppHandle) {
    let avant = connus().clone();
    let liste = relister(app);
    let Some(avant) = avant else {
        return;
    };
    let d = dossiers(app);
    for nom in historique::noms_partis(&avant, &liste) {
        // Seulement si la capture n'est vraiment plus là : `supprimer_recent` n'efface
        // alors que l'aperçu.
        let absente = sortie::chemin_recent(&d.recents, &nom).is_some_and(|c| !c.exists());
        if absente && sortie::supprimer_recent(&d.recents, &d.apercus, &nom).is_err() {
            log::warn!("historique apercu non efface");
        }
    }
}

// Actions des cases (§3.1, §4.2). Fil principal, sauf mention.

/// La case `nom` est-elle dans la liste gardée ? (Commandes : sinon refusée.)
pub(super) fn connue(app: &AppHandle, nom: &str) -> bool {
    super::historique_ouvert(app)
        .as_ref()
        .is_some_and(|e| e.connu(nom))
}

/// Une action de la page sur la case `nom` (déjà connue).
pub(super) fn action(app: &AppHandle, nom: String, action: ActionCase) {
    let maintenant = Instant::now();
    let decision = super::historique_ouvert(app).as_ref().map(|e| {
        let heure = e.liste.iter().find(|x| x.nom == nom).map(|x| x.horodatage);
        (
            historique::decider(e, &nom, action, maintenant),
            e.rang(&nom).unwrap_or_else(|| "-".into()),
            e.numero,
            heure,
        )
    });
    let Some((geste, court, numero, Some(heure))) = decision else {
        log::info!("historique action refusee");
        return;
    };
    let ligne = historique::ligne_action(&court, action);
    if geste == Geste::Ignore {
        log::info!("{ligne} ignoree");
        return;
    }
    log::info!("{ligne}");
    let heure = heure.with_timezone(&chrono::Local);
    match geste {
        Geste::Enregistrer { deja_fait: true } => {
            let etat = EtatResultat::DejaFait;
            log::info!("{}", historique::ligne_resultat(&court, action, etat));
            emettre_resultat(app, nom, action, etat, None);
        }
        Geste::Copier | Geste::Enregistrer { .. } | Geste::Annoter => {
            lancer_lecture(app, nom, court, action, numero, heure)
        }
        Geste::Supprimer => supprimer(app, nom, numero),
        Geste::Ignore => {}
    }
}

/// Copier, Enregistrer ou Annoter : les pixels sont relus dans un fil (jamais sur le fil
/// principal), puis `sortir` (Copier, Enregistrer) ou l'éditeur (Annoter, par le chemin de
/// la vignette : `recevoir_document` à un tour suivant de la boucle).
fn lancer_lecture(
    app: &AppHandle,
    nom: String,
    court: String,
    action: ActionCase,
    numero: u64,
    heure: chrono::DateTime<chrono::Local>,
) {
    let debut = Instant::now();
    let Some(chemin) = sortie::chemin_recent(&dossiers(app).recents, &nom) else {
        return;
    };
    // Toute action lancée est en cours jusqu'à son résultat (Annoter : jusqu'au document
    // posté ou à l'échec de la lecture) : Entrée tenue ne relit pas la capture.
    if let Some(e) = super::historique_ouvert(app).as_mut() {
        e.actions
            .insert(historique::cle_action(&nom, action), debut);
    }
    let base = super::demande_sortie(app, false);
    let pour_fil = app.clone();
    let (nom_fil, court_fil) = (nom.clone(), court.clone());
    let lance = dans_un_fil("historique-action", move || {
        let (app, nom, court) = (pour_fil, nom_fil, court_fil);
        let lu = sortie::lire_recent(&chemin);
        if action == ActionCase::Annoter {
            retirer_action(&app, numero, &nom, action);
            annoter_lu(&app, &court, lu, heure, debut);
            return;
        }
        let (etat, message) = match lu {
            Ok((pixels, transparence)) => {
                let demande = sortie::Demande {
                    copier: action == ActionCase::Copier,
                    enregistrer: action == ActionCase::Enregistrer,
                    ranger_recents: false,
                    maintenant: heure,
                    transparence,
                    ..base
                };
                let bilan = super::sortir_ou_echec(0, &pixels, &demande);
                drop(pixels);
                match historique::message_echec(action, &bilan) {
                    None => (EtatResultat::Fait, None),
                    Some(m) => (EtatResultat::Echec, Some(m)),
                }
            }
            Err(_) => (EtatResultat::Echec, historique::message_illisible(action)),
        };
        let pour = app.clone();
        let rendu = app.run_on_main_thread(move || {
            action_finie(&pour, nom, &court, action, numero, etat, message)
        });
        if let Err(e) = rendu {
            log::warn!("historique: resultat non rendu: {e}");
        }
    });
    if lance {
        return;
    }
    // Sans fil : rien n'est fait ici (ni lecture, ni sortie, ni fenêtre).
    retirer_action(app, numero, &nom, action);
    if action == ActionCase::Annoter {
        log::warn!("historique annoter {court} impossible");
        super::afficher(app, editeur::message_impossible());
        return;
    }
    let etat = EtatResultat::Echec;
    log::info!("{}", historique::ligne_resultat(&court, action, etat));
    let message = historique::message_illisible(action);
    emettre_resultat(app, nom, action, etat, message);
}

/// L'action `action` sur `nom` n'est plus en cours (n'importe quel fil).
fn retirer_action(app: &AppHandle, numero: u64, nom: &str, action: ActionCase) {
    if let Some(e) = super::historique_ouvert(app)
        .as_mut()
        .filter(|e| e.numero == numero)
    {
        e.actions.remove(&historique::cle_action(nom, action));
    }
}

/// Annoter : le document part à l'éditeur au tour suivant (fil de lecture).
fn annoter_lu(
    app: &AppHandle,
    court: &str,
    lu: Result<(Pixels, bool), String>,
    heure: chrono::DateTime<chrono::Local>,
    demande_a: Instant,
) {
    let pour = app.clone();
    let rendu = match lu {
        Ok((pixels, transparence)) => {
            let id = app
                .state::<Application>()
                .derniere_sortie
                .fetch_add(1, Ordering::SeqCst)
                + 1;
            let document = editeur::Document {
                id,
                pixels: Arc::new(pixels),
                transparence,
                maintenant: heure,
            };
            app.run_on_main_thread(move || super::recevoir_document(&pour, document, demande_a))
        }
        Err(_) => {
            log::warn!("historique annoter {court} impossible");
            app.run_on_main_thread(move || {
                super::afficher(&pour, editeur::message_impossible());
            })
        }
    };
    if let Err(e) = rendu {
        log::warn!("historique annoter {court} impossible: poste ({e})");
    }
}

/// Copier ou Enregistrer fini (fil principal) : journal, état, `historique:resultat`.
fn action_finie(
    app: &AppHandle,
    nom: String,
    court: &str,
    action: ActionCase,
    numero: u64,
    etat: EtatResultat,
    message: Option<&'static str>,
) {
    log::info!("{}", historique::ligne_resultat(court, action, etat));
    if let Some(e) = super::historique_ouvert(app)
        .as_mut()
        .filter(|e| e.numero == numero)
    {
        e.actions.remove(&historique::cle_action(&nom, action));
        if action == ActionCase::Enregistrer && etat == EtatResultat::Fait {
            e.enregistrees.insert(nom.clone());
        }
    }
    emettre_resultat(app, nom, action, etat, message);
}

/// La vignette se ferme sans animation si elle montre une capture de `noms` (toutes avec
/// `None`) ; ses pixels sont rendus, son fichier enregistré reste.
fn fermer_vignette_de(app: &AppHandle, noms: Option<&[String]>) {
    let concernee = super::montree(app)
        .as_ref()
        .is_some_and(|c| historique::vignette_a_fermer(c.recent.as_deref(), c.partie, noms));
    if concernee {
        super::appliquer(app, Declencheur::VignetteFermee);
        super::fermer_vignette(app, "historique");
    }
}

/// Supprimer : la case part de la liste, la suppression attend `DELAI_ANNULER`.
fn supprimer(app: &AppHandle, nom: String, numero: u64) {
    let fait = super::historique_ouvert(app).as_mut().map(|e| {
        let n = historique::ajouter_suppression(e, nom.clone(), Instant::now());
        (n, e.vues())
    });
    let Some((n, vues)) = fait else {
        return;
    };
    log::info!("historique suppression en attente {n}");
    emettre_liste(app, vues);
    emettre_attente(app, n);
    fermer_vignette_de(app, Some(std::slice::from_ref(&nom)));
    super::plus_tard_nomme(app, FIL_POSTE, historique::DELAI_ANNULER, move |a| {
        echeance(a, numero)
    });
}

/// Fin du délai d'Annuler (fil principal) : la série est effacée si sa dernière
/// suppression a 5 s.
fn echeance(app: &AppHandle, numero: u64) {
    let noms = super::historique_ouvert(app)
        .as_mut()
        .filter(|e| e.numero == numero && historique::suppressions_echues(e, Instant::now()))
        .map(historique::prendre_suppressions);
    let Some(noms) = noms else {
        return;
    };
    emettre_attente(app, 0);
    effacer(app, noms, false);
}

/// Annuler (Ctrl + Z, bouton de la pilule) : toute la série revient.
pub(super) fn annuler(app: &AppHandle) {
    let fait = super::historique_ouvert(app).as_mut().map(|e| {
        let n = historique::annuler_suppressions(e).len();
        (n, e.vues())
    });
    let Some((n, vues)) = fait else {
        log::info!("historique annuler refusee");
        return;
    };
    if n > 0 {
        log::info!("historique suppression annulee {n}");
    }
    emettre_liste(app, vues);
    emettre_attente(app, 0);
}

/// Les suppressions en attente d'un historique qui se ferme : faites tout de suite.
fn finir_suppressions(app: &AppHandle, mut etat: historique::EtatHistorique, ici: bool) {
    let noms = historique::prendre_suppressions(&mut etat);
    effacer(app, noms, ici);
}

/// Efface `noms` (PNG de « Récents » et aperçu) : dans un fil compté (l'arrêt l'attend),
/// ou sur ce fil (`ici`, arrêt). Puis la liste à jour, et l'échec à la page.
fn effacer(app: &AppHandle, noms: Vec<String>, ici: bool) {
    if noms.is_empty() {
        return;
    }
    let fait = move |app: &AppHandle| {
        let d = dossiers(app);
        let rates: Vec<String> = noms
            .iter()
            .filter(|n| sortie::supprimer_recent(&d.recents, &d.apercus, n).is_err())
            .cloned()
            .collect();
        log::info!(
            "historique suppression faite {} echecs {}",
            noms.len() - rates.len(),
            rates.len()
        );
        // Effacés ou restés, ils quittent l'ensemble en effacement (une case restée revient
        // par la liste relue). Fenêtre fermée (fermeture, arrêt) : rien à relire.
        let ouvert = super::historique_ouvert(app)
            .as_mut()
            .map(|e| e.effaces(&noms));
        if ouvert.is_some() {
            relister(app);
        }
        for nom in rates {
            let (action, etat) = (ActionCase::Supprimer, EtatResultat::Echec);
            emettre_resultat(app, nom, action, etat, Some(historique::MESSAGE_SUPPRIMER));
        }
    };
    if ici {
        fait(app);
        return;
    }
    super::compter_suppression(app, true);
    let pour = app.clone();
    dans_un_fil_ou_ici("historique-suppression", move || {
        fait(&pour);
        super::compter_suppression(&pour, false);
    });
}

/// « Tout effacer », confirmé par la page : « Récents », aperçus et `Glisser`, avec les
/// suppressions en attente. Rien à annuler ensuite.
pub(super) fn tout_effacer(app: &AppHandle) {
    let pris = super::historique_ouvert(app)
        .as_mut()
        .map(historique::prendre_tout);
    let Some(pris) = pris else {
        log::info!("historique tout_effacer refusee");
        return;
    };
    emettre_liste(app, Vec::new());
    emettre_attente(app, 0);
    fermer_vignette_de(app, None);
    super::compter_suppression(app, true);
    let pour = app.clone();
    dans_un_fil_ou_ici("historique-vidage", move || {
        let d = dossiers(&pour);
        let bilan = sortie::vider(&d.recents, &d.apercus, &d.glisser);
        log::info!(
            "historique tout efface {} echecs {}",
            bilan.effaces,
            bilan.echecs
        );
        if let Some(e) = super::historique_ouvert(&pour).as_mut() {
            e.effaces(&pris);
        }
        relister(&pour);
        super::compter_suppression(&pour, false);
    });
}

// Glisser (§2.2, U3).

/// Glisser de la case `nom` (déjà connue) : le lien est posé dans `Glisser`, puis le
/// glisser est reposté (jamais lancé dans le gestionnaire de la page).
pub(super) fn glisser(app: &AppHandle, nom: String) {
    let info = super::historique_ouvert(app).as_ref().map(|e| {
        let heure = e.liste.iter().find(|x| x.nom == nom).map(|x| x.horodatage);
        (e.rang(&nom), heure, e.glisser)
    });
    let Some((Some(court), Some(heure), en_cours)) = info else {
        log::info!("historique glisser refusee");
        return;
    };
    let glisser = app.state::<Application>().glisser.load(Ordering::SeqCst);
    let d = dossiers(app);
    let recent = sortie::chemin_recent(&d.recents, &nom);
    let Some(recent) = recent.filter(|_| !glisser && !en_cours) else {
        log::info!("historique glisser {court} indisponible");
        return;
    };
    let modele = super::reglages_lus(app).modele_nom;
    let fichier = sortie::nom_fichier(&modele, heure.with_timezone(&chrono::Local));
    match sortie::fichier_a_glisser(&recent, &d.glisser, &fichier) {
        Ok(chemin) => {
            super::plus_tard_nomme(app, FIL_POSTE, Duration::ZERO, move |a| {
                lancer_glisser(a, nom, court, &chemin)
            });
        }
        Err(e) => log::info!("historique glisser {court} refuse ({e})"),
    }
}

/// Le glisser lui-même (fil principal, bloquant : boucle du système), comme l'éditeur.
fn lancer_glisser(app: &AppHandle, nom: String, court: String, chemin: &Path) {
    let present = super::historique_ouvert(app)
        .as_ref()
        .is_some_and(|e| !e.glisser && e.vues().iter().any(|v| v.nom == nom));
    let etat_app = app.state::<Application>();
    let pret = vignette::glisser_pret(
        present && !etat_app.glisser.load(Ordering::SeqCst),
        fenetre::identifiant(app),
        systeme::glisser::bouton_principal_enfonce(),
    );
    let source = match pret {
        Ok(f) => f,
        Err(raison) => {
            log::info!("historique glisser {court} {raison}");
            return;
        }
    };
    let marquer = |valeur: bool| {
        if let Some(e) = super::historique_ouvert(app).as_mut() {
            e.glisser = valeur;
        }
    };
    let charge = |etat| ChargeGlisser {
        nom: nom.clone(),
        etat,
    };
    marquer(true);
    etat_app.glisser.store(true, Ordering::SeqCst);
    fenetre::emettre(app, fenetre::EVENEMENT_GLISSER, charge("debut"));
    log::info!("historique glisser {court} debut");
    let resultat = systeme::glisser::glisser_fichier(source, chemin);
    etat_app.glisser.store(false, Ordering::SeqCst);
    marquer(false);
    let sous = super::souris_bureau(app).and_then(systeme::glisser::fenetre_sous);
    let a_nous = sous.is_some_and(|h| super::nos_fenetres(app).contains(&h));
    let issue = match vignette::depot(&resultat, a_nous) {
        vignette::IssueGlisser::Depose => "depose".to_string(),
        vignette::IssueGlisser::Annule => "annule".to_string(),
        vignette::IssueGlisser::Refuse(c) => format!("refuse ({c})"),
    };
    log::info!("historique glisser {court} {issue}");
    fenetre::emettre(app, fenetre::EVENEMENT_GLISSER, charge("fini"));
}
