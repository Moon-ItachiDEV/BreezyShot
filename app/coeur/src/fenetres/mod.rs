//! Fenêtres de l'interface (§6) : le bandeau de messages, la sélection, la vignette et
//! son souffle, `reglages` (fenêtre des Réglages), `editeur` (éditeur d'annotation, 0.4) et
//! `theme` (thème des pages). Le bandeau, la vignette et le souffle naissent par
//! `creer_fenetre_outil`.
//!
//! Le module crée, place, montre et cache ; il ne décide d'aucun message. Ce qu'il reçoit
//! de la page remonte à l'Application par `Signal`. La fenêtre du bandeau montre une seule
//! chose à la fois (`Contenu`) : un message, ou le décompte du retardateur (`decompte`).

pub mod decompte;
pub mod editeur;
pub mod historique;
pub mod installateur;
pub mod menu;
mod place;
pub mod reglages;
pub mod selection;
pub mod theme;
pub mod vignette;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::systeme;
pub use decompte::Decompte;
pub use place::{
    borner_zone, borner_zone_dans, cacher_d_office, cliquable, ecran_sous, geste_cliquable,
    intervalle, placer, zone_ecran, Ecran, Rect, ZoneCss, DELAI_REUSSITE_MAX, DELAI_ZONE_MAX,
    TAILLE_LOGIQUE,
};

/// Étiquette de la fenêtre du bandeau (aussi dans `capabilities/`).
pub const ETIQUETTE_BANDEAU: &str = "bandeau";
/// Événement écouté par la page.
pub const EVENEMENT_AFFICHER: &str = "bandeau:afficher";
/// Événement écouté par la page : le message `{ id }` est caché par le cœur (`cacher()`,
/// filet) ; la page efface la pilule pendant que la fenêtre est cachée.
pub const EVENEMENT_MESSAGE_CACHE: &str = "bandeau:message_cache";

/// Arguments de WebView2, les mêmes pour toutes les fenêtres : WebView2 refuse de créer
/// une seconde fenêtre dont les arguments diffèrent de la première (même dossier de
/// données). Ce sont ceux que wry passe par défaut (wry webview2/mod.rs, `default_args`,
/// plus `--autoplay-policy` car `autoplay` vaut vrai par défaut, wry lib.rs), avec en plus
/// `CalculateNativeWinOcclusion` coupé : une page cachée ou couverte reste vivante et se
/// repeint, la sélection apparaît donc tout de suite.
pub const ARGUMENTS_NAVIGATEUR: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,CalculateNativeWinOcclusion --autoplay-policy=no-user-gesture-required";

/// Genre d'un message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Genre {
    Reussite,
    Info,
    Erreur,
}

impl Genre {
    pub fn nom(self) -> &'static str {
        match self {
            Genre::Reussite => "reussite",
            Genre::Info => "info",
            Genre::Erreur => "erreur",
        }
    }

    /// Info et erreur portent une croix et restent jusqu'au clic.
    pub fn a_croix(self) -> bool {
        self != Genre::Reussite
    }
}

/// Un bouton d'action d'un message (fourni par l'Application).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Action {
    pub id: String,
    pub libelle: String,
}

/// Un message du bandeau.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub genre: Genre,
    pub titre: String,
    pub detail: String,
    pub actions: Vec<Action>,
    /// Une erreur qui part seule après ce délai, croix gardée (0.5, « Aucun texte
    /// trouvé ») ; `None` : la règle du genre (Réussite seule, Info et Erreur à la croix).
    pub duree_ms: Option<u32>,
}

impl Message {
    /// Une erreur qui part seule après `duree_ms`, avec ses actions et sa croix.
    pub fn erreur_breve(titre: &str, actions: Vec<Action>, duree_ms: u32) -> Message {
        Message {
            genre: Genre::Erreur,
            titre: titre.into(),
            detail: String::new(),
            actions,
            duree_ms: Some(duree_ms),
        }
    }
}

/// Marge du filet au-delà de `duree_ms` : la page part d'elle-même à `duree_ms`.
pub const MARGE_DUREE: Duration = Duration::from_secs(1);

/// Filet d'un message : la règle du genre (`cacher_d_office`), et pour un message à durée,
/// caché d'office `MARGE_DUREE` après sa durée si la page ne l'a pas fait.
pub fn cacher_message_d_office(
    a_croix: bool,
    duree_ms: Option<u32>,
    depuis_affichage: Duration,
    zone_recue: bool,
) -> bool {
    let echue = duree_ms
        .is_some_and(|d| depuis_affichage >= Duration::from_millis(u64::from(d)) + MARGE_DUREE);
    cacher_d_office(a_croix, depuis_affichage, zone_recue) || echue
}

/// Ce que la page envoie (`bandeau:afficher`).
#[derive(Debug, Clone, Serialize)]
struct Charge<'a> {
    id: u64,
    genre: Genre,
    titre: &'a str,
    detail: &'a str,
    actions: &'a [Action],
    duree_ms: Option<u32>,
}

/// Ce que le module signale à l'Application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    /// Clic sur une action ou sur la croix (`fermer`) du message `id`.
    ActionBandeau { id: u64, action: String },
    /// Le message `id` est réellement à l'écran (premier `bandeau_zone` valide).
    MessageMontre { id: u64 },
}

type Rappel = Box<dyn Fn(Signal) + Send + Sync + 'static>;

/// Ce que la fenêtre du bandeau montre : un message, ou un décompte.
#[derive(Debug, Clone)]
enum Contenu {
    Message(Message),
    Decompte(Decompte),
}

impl Contenu {
    /// Reçoit-il des clics ? Un message à croix, et le décompte (sa croix « Annuler »).
    fn a_croix(&self) -> bool {
        match self {
            Contenu::Message(m) => m.genre.a_croix(),
            Contenu::Decompte(_) => true,
        }
    }

    fn est_decompte(&self) -> bool {
        matches!(self, Contenu::Decompte(_))
    }
}

/// Ce qui a été retiré du bandeau (ou ce qui est en cours) : son `id`, et s'il s'agit
/// d'un décompte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Retire {
    id: u64,
    decompte: bool,
}

/// Quels contenus un retrait peut viser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Vise {
    Tout,
    Message,
    Decompte,
}

/// Décision de retrait : `en_cours` est le contenu en cours, `id` celui visé (`None` :
/// quel qu'il soit). Rend ce qui est retiré, et si l'état doit être vidé (aussi quand rien
/// n'était en cours et qu'aucun `id` n'est visé).
fn decider_retrait(
    en_cours: Option<Retire>,
    id: Option<u64>,
    vise: Vise,
) -> (Option<Retire>, bool) {
    let genre_ok = |c: &Retire| match vise {
        Vise::Tout => true,
        Vise::Message => !c.decompte,
        Vise::Decompte => c.decompte,
    };
    let retire = en_cours.filter(|c| id.is_none_or(|id| c.id == id) && genre_ok(c));
    (retire, retire.is_some() || id.is_none())
}

/// Un nouveau contenu remplace `ancien` : rend l'`id` du décompte remplacé, s'il en est un
/// (`bandeau decompte <id> remplace`).
fn decompte_remplace(ancien: Option<Retire>) -> Option<u64> {
    ancien.filter(|a| a.decompte).map(|a| a.id)
}

/// Après un retrait, la fenêtre peut-elle être cachée ? Oui si rien n'est en cours, ou si
/// le nouveau contenu n'est pas encore montré (`montrer` le montrera ensuite) ; non si un
/// contenu arrivé entre-temps est déjà à l'écran.
fn cacher_apres_retrait(en_cours_montre: Option<bool>) -> bool {
    en_cours_montre != Some(true)
}

/// Le contenu en cours, vu du cœur.
#[derive(Debug, Clone)]
struct EnCours {
    id: u64,
    contenu: Contenu,
    /// Moment de la demande (un décompte envoyé plus tard part avec le temps qui reste).
    demande_a: Instant,
    /// Numéro d'envoi à la page (un `bandeau_pret` répété renvoie le message).
    envoi: u64,
    /// Moment de l'affichage, place de la fenêtre et échelle de son écran.
    montre_a: Option<Instant>,
    place: Option<(Rect, f64)>,
    zone: Option<ZoneCss>,
    /// `MessageMontre` déjà signalé.
    signale: bool,
}

impl EnCours {
    /// Un contenu mis en attente : ni envoyé, ni montré.
    fn nouveau(id: u64, contenu: Contenu) -> EnCours {
        EnCours {
            id,
            contenu,
            demande_a: Instant::now(),
            envoi: 0,
            montre_a: None,
            place: None,
            zone: None,
            signale: false,
        }
    }

    /// (Re)part vers une page prête : nouvel envoi, et tout ce que la page avait dit du
    /// précédent est oublié (zone, signal, moment de l'affichage). Rend le numéro d'envoi.
    fn renvoyer(&mut self) -> u64 {
        self.envoi += 1;
        self.montre_a = None;
        self.zone = None;
        self.signale = false;
        self.envoi
    }

    /// L'`id` et le genre du contenu.
    fn resume(&self) -> Retire {
        Retire {
            id: self.id,
            decompte: self.contenu.est_decompte(),
        }
    }

    /// Temps écoulé depuis l'envoi à une page prête ; nul tant que le message attend
    /// (premier lancement de WebView2 lent) : le filet de sécurité ne court pas encore.
    fn depuis_affichage(&self, maintenant: Instant) -> Duration {
        self.montre_a
            .map(|t| maintenant.saturating_duration_since(t))
            .unwrap_or_default()
    }
}

#[derive(Default)]
struct Etat {
    dernier_id: u64,
    en_cours: Option<EnCours>,
    /// La page écoute `bandeau:afficher` (sinon le message attend).
    page_prete: bool,
    /// La fenêtre reçoit les clics (ou un geste en ce sens est parti).
    recoit_clics: bool,
}

/// Le bandeau : une fenêtre préparée au démarrage et cachée.
///
/// Règle des verrous : `etat` n'est jamais tenu pendant un appel à Tauri ni un `emit`.
/// Les gestes sur la fenêtre se font sur le fil principal.
pub struct Bandeau {
    app: AppHandle,
    rappel: Rappel,
    etat: Mutex<Etat>,
    fil_principal: ThreadId,
    /// Faux après `fermer` ou la destruction de la fenêtre : toute surveillance s'arrête.
    vivante: Arc<AtomicBool>,
    /// Identifiant système de la fenêtre (Windows : le HWND), gardé dès la création : la
    /// surveillance le passe à `systeme` sans appeler la fenêtre Tauri.
    identifiant: Option<isize>,
}

impl Bandeau {
    /// Crée la fenêtre cachée (à appeler sur le fil principal). Rend le bandeau, à confier
    /// à Tauri (`app.manage`).
    pub fn creer<F>(app: &AppHandle, rappel: F) -> Result<Bandeau, String>
    where
        F: Fn(Signal) + Send + Sync + 'static,
    {
        let outil = creer_fenetre_outil(
            app,
            ETIQUETTE_BANDEAU,
            "vignette/bandeau.html",
            TAILLE_LOGIQUE,
            "bandeau exclu",
        )?;
        let (vivante, identifiant) = (outil.vivante, outil.identifiant);
        Ok(Bandeau {
            app: app.clone(),
            rappel: Box::new(rappel),
            etat: Mutex::new(Etat::default()),
            fil_principal: std::thread::current().id(),
            vivante,
            identifiant,
        })
    }

    fn fenetre(&self) -> Option<WebviewWindow> {
        self.app.get_webview_window(ETIQUETTE_BANDEAU)
    }

    /// Identifiant système de la fenêtre du bandeau (Windows : le HWND).
    pub fn identifiant(&self) -> Option<isize> {
        self.identifiant
    }

    fn etat(&self) -> Option<std::sync::MutexGuard<'_, Etat>> {
        match self.etat.lock() {
            Ok(e) => Some(e),
            Err(_) => {
                log::error!("bandeau: etat inaccessible");
                None
            }
        }
    }

    /// Montre un message ; il remplace aussitôt le précédent. Rend son `id`. Peut être
    /// appelé de n'importe quel fil : les gestes partent sur le fil principal.
    /// Un message qui arrive pendant un décompte le remplace (le délai, lui, continue) :
    /// `bandeau decompte <id> remplace` (id de l'ancien décompte).
    pub fn afficher(&self, message: Message) -> u64 {
        self.poser_contenu(Contenu::Message(message))
    }

    /// Range un nouveau contenu (il remplace le précédent) et le montre si la page est
    /// prête. Rend son `id`, 0 si l'état est inaccessible. Un décompte remplacé, par un
    /// message ou par un autre décompte, écrit `bandeau decompte <id> remplace`.
    fn poser_contenu(&self, contenu: Contenu) -> u64 {
        let (id, prete, remplace) = {
            let Some(mut etat) = self.etat() else {
                return 0;
            };
            let remplace = decompte_remplace(etat.en_cours.as_ref().map(EnCours::resume));
            etat.dernier_id += 1;
            let id = etat.dernier_id;
            etat.en_cours = Some(EnCours::nouveau(id, contenu));
            (id, etat.page_prete, remplace)
        };
        if let Some(ancien) = remplace {
            log::info!("bandeau decompte {ancien} remplace");
        }
        if prete {
            self.demander_montrer(id);
        }
        id
    }

    /// `montrer(id)` sur le fil principal.
    fn demander_montrer(&self, id: u64) {
        self.sur_fil_principal("affichage", move |b| b.montrer(id));
    }

    /// Fait `geste` tout de suite sur le fil principal, sinon l'y poste.
    fn sur_fil_principal<F>(&self, quoi: &str, geste: F)
    where
        F: FnOnce(&Bandeau) + Send + 'static,
    {
        if std::thread::current().id() == self.fil_principal {
            geste(self);
            return;
        }
        let app = self.app.clone();
        let poste = self.app.run_on_main_thread(move || {
            if let Some(b) = app.try_state::<Bandeau>() {
                geste(&b);
            }
        });
        if let Err(e) = poste {
            log::warn!("bandeau: {quoi} non poste: {e}");
        }
    }

    /// Place, montre sans activer, et envoie le contenu `id` à la page (fil principal).
    /// Un message va en haut de l'écran de la souris ; un décompte est centré sur sa cible,
    /// dans la zone utile de l'écran qui la contient.
    fn montrer(&self, id: u64) {
        if !self.vivante.load(Ordering::SeqCst) {
            return;
        }
        let (contenu, envoi, demande_a) = {
            let Some(mut etat) = self.etat() else {
                return;
            };
            let Some(c) = etat.en_cours.as_mut().filter(|c| c.id == id) else {
                return;
            };
            let envoi = c.renvoyer();
            (c.contenu.clone(), envoi, c.demande_a)
        };
        let Some(fenetre) = self.fenetre() else {
            log::warn!("bandeau: fenetre absente");
            return;
        };
        let (nom, place) = match &contenu {
            Contenu::Message(m) => {
                let ecran = ecran_sous(souris(&self.app), &ecrans(&self.app));
                let place = ecran.map(|e| (placer(&e, TAILLE_LOGIQUE), e.echelle));
                (format!("bandeau affiche {id} {}", m.genre.nom()), place)
            }
            Contenu::Decompte(d) => {
                let cible = (f64::from(d.centre.0), f64::from(d.centre.1));
                let ecran = ecran_sous(Some(cible), &ecrans(&self.app));
                let place =
                    ecran.map(|e| (decompte::placer(d.centre, &e, TAILLE_LOGIQUE), e.echelle));
                (format!("bandeau decompte {id}"), place)
            }
        };
        match place {
            Some((p, echelle)) => {
                poser(&fenetre, p);
                log::info!(
                    "{nom} ecran {},{} {}x{} echelle {echelle}",
                    p.x,
                    p.y,
                    p.largeur,
                    p.hauteur
                );
            }
            None => log::info!("{nom} ecran inconnu"),
        }
        match systeme::montrer_sans_activer(&fenetre) {
            Ok(true) => {}
            Ok(false) => {
                if let Err(e) = fenetre.show() {
                    log::warn!("bandeau: affichage impossible: {e}");
                }
            }
            Err(e) => log::warn!("bandeau: affichage impossible: {e}"),
        }
        // Un contenu part toujours avec des clics traversants ; la surveillance (message à
        // croix, décompte) les rend sur la pilule ou sur la croix seulement.
        traverser(&fenetre, true);
        {
            let Some(mut etat) = self.etat() else {
                return;
            };
            etat.recoit_clics = false;
            let Some(c) = etat.en_cours.as_mut().filter(|c| c.id == id) else {
                return;
            };
            c.montre_a = Some(Instant::now());
            c.place = place;
        }
        let envoye = match &contenu {
            Contenu::Message(message) => {
                let charge = Charge {
                    id,
                    genre: message.genre,
                    titre: &message.titre,
                    detail: &message.detail,
                    actions: &message.actions,
                    duree_ms: message.duree_ms,
                };
                self.app
                    .emit_to(ETIQUETTE_BANDEAU, EVENEMENT_AFFICHER, charge)
            }
            Contenu::Decompte(d) => {
                let charge = decompte::charge(id, d, demande_a.elapsed(), place);
                self.app
                    .emit_to(ETIQUETTE_BANDEAU, decompte::EVENEMENT_DECOMPTE, charge)
            }
        };
        if let Err(e) = envoye {
            log::warn!("bandeau: contenu non envoye: {e}");
        }
        self.surveiller(id, envoi);
    }

    /// Fil de surveillance du message `id` (envoi `envoi`) : filet de sécurité pour tous
    /// les genres, et clics sur la pilule pour un message à croix. S'arrête dès que le
    /// message n'est plus en cours, qu'il a été renvoyé, ou que le bandeau est fermé.
    fn surveiller(&self, id: u64, envoi: u64) {
        let app = self.app.clone();
        let vivante = Arc::clone(&self.vivante);
        let identifiant = self.identifiant;
        let lancement = std::thread::Builder::new()
            .name("bandeau-souris".into())
            .spawn(move || {
                let mut attente = place::INTERVALLE_PROCHE;
                while vivante.load(Ordering::SeqCst) {
                    std::thread::sleep(attente);
                    let Some(b) = app.try_state::<Bandeau>() else {
                        break;
                    };
                    // Appels au système avant de prendre le verrou ; aucune méthode de la
                    // fenêtre Tauri ici (chacune attend le fil principal).
                    let pos = souris(&app);
                    let reelle = identifiant.and_then(place_reelle);
                    let Some(mut etat) = b.etat() else {
                        break;
                    };
                    let recoit = etat.recoit_clics;
                    let Some(c) = etat
                        .en_cours
                        .as_ref()
                        .filter(|c| c.id == id && c.envoi == envoi)
                    else {
                        break;
                    };
                    let a_croix = c.contenu.a_croix();
                    let depuis = c.depuis_affichage(Instant::now());
                    let duree = match &c.contenu {
                        Contenu::Message(m) => m.duree_ms,
                        Contenu::Decompte(_) => None,
                    };
                    if cacher_message_d_office(a_croix, duree, depuis, c.zone.is_some()) {
                        drop(etat);
                        let pour = app.clone();
                        let _ = app.run_on_main_thread(move || {
                            if let Some(b) = pour.try_state::<Bandeau>() {
                                b.cacher_d_office(id);
                            }
                        });
                        break;
                    }
                    if !a_croix {
                        attente = place::INTERVALLE_LOIN;
                        continue;
                    }
                    let (fen, echelle) = match reelle.or(c.place) {
                        Some((r, e)) => ((r.x, r.y), e),
                        None => continue,
                    };
                    let pilule = c.zone.as_ref().map(|z| zone_ecran(z, echelle, fen));
                    let voulu = cliquable(a_croix, c.zone.as_ref(), echelle, fen, pos);
                    attente = intervalle(pos, pilule.as_ref());
                    if voulu != recoit {
                        etat.recoit_clics = voulu;
                        drop(etat);
                        let pour = app.clone();
                        let _ = app.run_on_main_thread(move || {
                            if let Some(b) = pour.try_state::<Bandeau>() {
                                b.geste_clics(id, voulu);
                            }
                        });
                    }
                }
            });
        if let Err(e) = lancement {
            log::warn!("bandeau: surveillance impossible: {e}");
        }
    }

    /// Geste posté par la surveillance (fil principal) : l'état est relu sous le verrou ;
    /// « cliquable » ne s'applique qu'au message en cours, s'il porte une croix.
    fn geste_clics(&self, id: u64, voulu: bool) {
        let cliquable = {
            let Some(mut etat) = self.etat() else {
                return;
            };
            let en_cours = etat.en_cours.as_ref().map(|c| (c.id, c.contenu.a_croix()));
            let ok = geste_cliquable(en_cours, id, voulu);
            etat.recoit_clics = ok;
            ok
        };
        if let Some(f) = self.fenetre() {
            traverser(&f, !cliquable);
        }
    }

    /// Retire le contenu `id` (`None` : quel qu'il soit) s'il est en cours et s'il passe
    /// le filtre `garder`. Rend ce qui a été retiré.
    fn retirer_si(&self, id: Option<u64>, vise: Vise) -> Option<Retire> {
        let mut etat = self.etat()?;
        let (retire, vider) =
            decider_retrait(etat.en_cours.as_ref().map(EnCours::resume), id, vise);
        if vider {
            etat.en_cours = None;
            etat.recoit_clics = false;
        }
        retire
    }

    /// Retire le contenu `id` (`None` : quel qu'il soit) s'il est en cours.
    fn retirer(&self, id: Option<u64>) -> Option<Retire> {
        self.retirer_si(id, Vise::Tout)
    }

    /// Après un retrait (fil principal) : la fenêtre est cachée, sauf si un contenu arrivé
    /// entre-temps est déjà à l'écran. La page efface ce qui a été retiré pendant que la
    /// fenêtre est cachée (`bandeau:decompte_cache` ou `bandeau:message_cache`, par `id` :
    /// la page ignore un `id` qui n'est plus le sien). Un décompte écrit
    /// `bandeau decompte cache <id>`.
    fn cacher_retire(&self, retire: Retire) {
        // État inaccessible : la fenêtre est cachée (rien de sûr à l'écran).
        let montre = self
            .etat()
            .and_then(|e| e.en_cours.as_ref().map(|c| c.montre_a.is_some()));
        if let Some(f) = self.fenetre().filter(|_| cacher_apres_retrait(montre)) {
            cacher(&f);
        }
        let evenement = if retire.decompte {
            decompte::EVENEMENT_DECOMPTE_CACHE
        } else {
            EVENEMENT_MESSAGE_CACHE
        };
        let charge = serde_json::json!({ "id": retire.id });
        if let Err(e) = self.app.emit_to(ETIQUETTE_BANDEAU, evenement, charge) {
            log::warn!("bandeau: retrait non envoye: {e}");
        }
        if retire.decompte {
            log::info!("bandeau decompte cache {}", retire.id);
        }
    }

    /// Filet de sécurité : la page n'a pas fait sa part à temps.
    fn cacher_d_office(&self, id: u64) {
        if let Some(r) = self.retirer(Some(id)) {
            self.cacher_retire(r);
            log::warn!("bandeau cache d'office {id}");
        }
    }

    /// Cache tout de suite le message en cours, sans attendre la page (avant une capture :
    /// le bandeau ne doit ni figurer sur la photo ni rester au-dessus de l'écran figé).
    /// Rend vrai s'il y avait un message en cours, donc une fenêtre peut-être à l'écran.
    /// Fil principal. La page garde son dernier message jusqu'au suivant, qui le remplace ;
    /// un `bandeau_termine` de l'ancien est ignoré (id qui n'est plus en cours).
    /// Un décompte en cours est caché de même, sans animation (sa page reçoit
    /// `bandeau:decompte_cache`).
    pub fn cacher(&self) -> bool {
        let Some(r) = self.retirer(None) else {
            return false;
        };
        self.cacher_retire(r);
        true
    }

    /// Montre le décompte `d` dans la fenêtre du bandeau ; il remplace le message en cours
    /// (la page efface la pilule, sans `bandeau_termine`). Rend son `id` (même compteur que
    /// les messages ; 0 si l'état est inaccessible). N'importe quel fil. Le filet du bandeau
    /// s'applique : sans `bandeau_zone` en 3 s, `bandeau cache d'office <id>`.
    pub fn montrer_decompte(&self, d: Decompte) -> u64 {
        self.poser_contenu(Contenu::Decompte(d))
    }

    /// Cache tout de suite le décompte `id` s'il est en cours, sans animation ; rend vrai
    /// s'il l'était (`bandeau decompte cache <id>`). Un autre contenu n'est pas touché.
    /// N'importe quel fil : le geste sur la fenêtre part sur le fil principal.
    pub fn cacher_decompte(&self, id: u64) -> bool {
        let Some(r) = self.retirer_si(Some(id), Vise::Decompte) else {
            return false;
        };
        self.sur_fil_principal("fin du decompte", move |b| b.cacher_retire(r));
        true
    }

    /// À l'arrêt de l'app : la surveillance s'arrête, la fenêtre est cachée puis détruite.
    pub fn fermer(&self) {
        self.vivante.store(false, Ordering::SeqCst);
        self.retirer(None);
        if let Some(f) = self.fenetre() {
            cacher(&f);
            let _ = f.destroy();
        }
    }

    /// La page écoute : le message en attente part ; un `bandeau_pret` répété (page
    /// rechargée) renvoie le message en cours.
    fn page_prete(&self) {
        let en_cours = {
            let Some(mut etat) = self.etat() else {
                return;
            };
            etat.page_prete = true;
            etat.en_cours.as_ref().map(|c| c.id)
        };
        log::info!("bandeau pret");
        if let Some(id) = en_cours {
            self.demander_montrer(id);
        }
    }
}

/// Une fenêtre « outil » prête : cachée, sans focus, hors barre des tâches, exclue des
/// captures (sauf essai), clics traversants.
pub(crate) struct FenetreOutil {
    /// Identifiant système (Windows : le HWND), gardé pour les fils de surveillance.
    pub identifiant: Option<isize>,
    /// Faux une fois la fenêtre détruite.
    pub vivante: Arc<AtomicBool>,
}

/// Crée une fenêtre de l'interface qui ne prend jamais le focus (bandeau, vignette,
/// souffle), sur le fil principal. Journal : `<etiquette> fenetre <hwnd>` et
/// `<exclusion> des captures: oui|non|non (essai)` (`exclusion` : « bandeau exclu »…).
pub(crate) fn creer_fenetre_outil(
    app: &AppHandle,
    etiquette: &str,
    page: &str,
    taille: (f64, f64),
    exclusion: &str,
) -> Result<FenetreOutil, String> {
    let capturable = systeme::essai_capturable();
    let fenetre = WebviewWindowBuilder::new(app, etiquette, WebviewUrl::App(page.into()))
        .title("BreezyShot")
        .inner_size(taille.0, taille.1)
        .visible(false)
        .focused(false)
        .focusable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .content_protected(!capturable)
        .additional_browser_args(ARGUMENTS_NAVIGATEUR)
        .build()
        .map_err(|e| format!("fenetre {etiquette}: {e}"))?;
    if let Err(e) = systeme::preparer_fenetre_outil(&fenetre) {
        log::warn!("{etiquette}: style outil non pose: {e}");
    }
    let identifiant = systeme::identifiant_fenetre(&fenetre);
    if let Some(h) = identifiant {
        log::info!("{etiquette} fenetre {h}");
    }
    if capturable {
        log::info!("{exclusion} des captures: non (essai)");
    } else {
        let exclue = systeme::exclure_des_captures(&fenetre);
        log::info!(
            "{exclusion} des captures: {}",
            if exclue { "oui" } else { "non" }
        );
    }
    traverser(&fenetre, true);
    let vivante = Arc::new(AtomicBool::new(true));
    let pour_destruction = Arc::clone(&vivante);
    fenetre.on_window_event(move |e| {
        if matches!(e, tauri::WindowEvent::Destroyed) {
            pour_destruction.store(false, Ordering::SeqCst);
        }
    });
    Ok(FenetreOutil {
        identifiant,
        vivante,
    })
}

/// Les écrans connus de Tauri, l'écran principal en tête.
fn ecrans(app: &AppHandle) -> Vec<Ecran> {
    let principal = app.primary_monitor().ok().flatten();
    let mut tous: Vec<tauri::Monitor> = app.available_monitors().unwrap_or_default();
    if let Some(p) = &principal {
        if let Some(i) = tous.iter().position(|m| m.position() == p.position()) {
            let m = tous.remove(i);
            tous.insert(0, m);
        }
    }
    tous.iter()
        .map(|m| {
            let (pos, taille, utile) = (m.position(), m.size(), m.work_area());
            Ecran {
                rect: Rect {
                    x: pos.x,
                    y: pos.y,
                    largeur: taille.width,
                    hauteur: taille.height,
                },
                zone_utile: Rect {
                    x: utile.position.x,
                    y: utile.position.y,
                    largeur: utile.size.width,
                    hauteur: utile.size.height,
                },
                echelle: m.scale_factor(),
            }
        })
        .collect()
}

/// Pose la fenêtre (pixels réels). La position est redonnée après la taille : changer
/// d'écran peut faire réajuster la taille par le système (changement d'échelle).
fn poser(fenetre: &WebviewWindow, place: Rect) {
    let position = tauri::PhysicalPosition::new(place.x, place.y);
    let taille = tauri::PhysicalSize::new(place.largeur, place.hauteur);
    let r = fenetre
        .set_position(position)
        .and_then(|_| fenetre.set_size(taille))
        .and_then(|_| fenetre.set_position(position));
    if let Err(e) = r {
        log::warn!("{}: placement impossible: {e}", fenetre.label());
    }
}

/// Cache la fenêtre et rend les clics traversants. Sous Windows par `systeme` : tao croit
/// la fenêtre cachée et son `hide()` ne ferait rien.
fn cacher(fenetre: &WebviewWindow) {
    traverser(fenetre, true);
    let fait = match systeme::cacher_sans_tao(fenetre) {
        Ok(true) => Ok(()),
        Ok(false) => fenetre.hide().map_err(|e| e.to_string()),
        Err(e) => Err(e),
    };
    if let Err(e) = fait {
        log::warn!("{}: fenetre non cachee: {e}", fenetre.label());
    }
}

/// Souris en pixels réels : directement par `systeme`, sinon par Tauri. Le seul repli
/// de l'app (le bandeau, et l'Application pour le plein écran).
pub fn souris(app: &AppHandle) -> Option<(f64, f64)> {
    systeme::position_souris().or_else(|| app.cursor_position().ok().map(|p| (p.x, p.y)))
}

/// Place réelle de la fenêtre et échelle de son écran, relues auprès du système par son
/// identifiant (`None` hors Windows : la place notée par `montrer` sert alors).
fn place_reelle(identifiant: isize) -> Option<(Rect, f64)> {
    let (x, y, largeur, hauteur, e) = systeme::rectangle_fenetre(identifiant)?;
    Some((
        Rect {
            x,
            y,
            largeur,
            hauteur,
        },
        e,
    ))
}

/// Clics traversants (`true`) ou reçus (`false`).
fn traverser(fenetre: &WebviewWindow, oui: bool) {
    let fait = match systeme::traverser_clics(fenetre, oui) {
        Ok(true) => Ok(()),
        // Par Tauri, seulement sur une fenêtre visible : sous Linux, tao panique sur une
        // fenêtre jamais montrée (event_loop.rs, `CursorIgnoreEvents`). Une fenêtre cachée
        // ne reçoit de toute façon aucun clic.
        Ok(false) if !fenetre.is_visible().unwrap_or(false) => Ok(()),
        Ok(false) => fenetre
            .set_ignore_cursor_events(oui)
            .map_err(|e| e.to_string()),
        Err(e) => Err(e),
    };
    if let Err(e) = fait {
        log::warn!("{}: clics non regles: {e}", fenetre.label());
    }
}

// Commandes appelées par la page. Un `id` qui n'est pas celui du message en cours est ignoré.

/// La page écoute `bandeau:afficher` : le message en attente peut partir.
#[tauri::command]
pub fn bandeau_pret(bandeau: State<'_, Bandeau>) {
    theme::envoyer_a(&bandeau.app, ETIQUETTE_BANDEAU);
    bandeau.page_prete();
}

/// Rectangle de la pilule (pixels CSS, relatif à la page). Refusé s'il n'est pas fini,
/// borné à la fenêtre sinon.
#[tauri::command]
pub fn bandeau_zone(
    bandeau: State<'_, Bandeau>,
    id: u64,
    x: f64,
    y: f64,
    largeur: f64,
    hauteur: f64,
) {
    let Some(zone) = borner_zone(x, y, largeur, hauteur) else {
        log::warn!("bandeau zone {id} refusee (valeur non finie)");
        return;
    };
    let (place, premiere) = {
        let Some(mut etat) = bandeau.etat() else {
            return;
        };
        let Some(c) = etat.en_cours.as_mut().filter(|c| c.id == id) else {
            return;
        };
        c.zone = Some(zone);
        // Seul un message signale `MessageMontre` (un décompte n'est pas un message).
        let premiere = !c.signale && !c.contenu.est_decompte();
        c.signale = true;
        (c.place, premiere)
    };
    if let Some((p, echelle)) = place {
        let r = zone_ecran(&zone, echelle, (p.x, p.y));
        log::info!(
            "bandeau zone {id} {},{} {}x{}",
            r.x,
            r.y,
            r.largeur,
            r.hauteur
        );
    }
    if premiere {
        (bandeau.rappel)(Signal::MessageMontre { id });
    }
}

/// Clic sur une action ou sur la croix (`fermer`).
#[tauri::command]
pub fn bandeau_action(bandeau: State<'_, Bandeau>, id: u64, action: String) {
    let valide = bandeau
        .etat()
        .is_some_and(|e| e.en_cours.as_ref().is_some_and(|c| c.id == id));
    if valide {
        (bandeau.rappel)(Signal::ActionBandeau { id, action });
    }
}

/// Le message a fini de disparaître : on cache la fenêtre. Un décompte ne se termine pas
/// par la page (le cœur le cache) : un `bandeau_termine` de son `id` est ignoré.
#[tauri::command]
pub fn bandeau_termine(bandeau: State<'_, Bandeau>, id: u64) {
    if let Some(r) = bandeau.retirer_si(Some(id), Vise::Message) {
        bandeau.cacher_retire(r);
        log::info!("bandeau termine {id}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn info() -> Message {
        Message {
            genre: Genre::Info,
            titre: "t".into(),
            detail: String::new(),
            actions: Vec::new(),
            duree_ms: None,
        }
    }

    #[test]
    fn erreur_breve_part_seule_apres_sa_duree() {
        let m = Message::erreur_breve("Aucun texte trouvé", Vec::new(), 4000);
        assert_eq!((m.genre, m.duree_ms), (Genre::Erreur, Some(4000)));
        assert!(m.genre.a_croix());
        let zone = true;
        assert!(!cacher_message_d_office(
            true,
            Some(4000),
            Duration::from_secs(4),
            zone
        ));
        assert!(cacher_message_d_office(
            true,
            Some(4000),
            Duration::from_secs(5),
            zone
        ));
        // Sans durée : la règle du genre, inchangée.
        assert!(!cacher_message_d_office(
            true,
            None,
            Duration::from_secs(60),
            zone
        ));
        assert!(cacher_message_d_office(true, None, DELAI_ZONE_MAX, false));
        assert!(cacher_message_d_office(
            false,
            None,
            DELAI_REUSSITE_MAX,
            zone
        ));
    }

    fn decompte() -> Decompte {
        Decompte {
            numero: 1,
            secondes: 3,
            restant: Duration::from_secs(3),
            centre: (500, 400),
            cote: None,
            reduites: false,
        }
    }

    #[test]
    fn le_decompte_porte_une_croix_cliquable() {
        assert!(Contenu::Decompte(decompte()).a_croix());
        assert!(Contenu::Decompte(decompte()).est_decompte());
        assert!(!Contenu::Message(info()).est_decompte());
        let reussite = Message {
            genre: Genre::Reussite,
            ..info()
        };
        assert!(!Contenu::Message(reussite).a_croix());
        // Le filet du bandeau vaut pour le décompte : sans zone en 3 s, caché d'office.
        let c = EnCours::nouveau(2, Contenu::Decompte(decompte()));
        assert!(cacher_d_office(c.contenu.a_croix(), DELAI_ZONE_MAX, false));
        assert!(!cacher_d_office(
            c.contenu.a_croix(),
            Duration::from_secs(60),
            true
        ));
    }

    const fn r(id: u64, decompte: bool) -> Retire {
        Retire { id, decompte }
    }

    #[test]
    fn decision_de_retrait() {
        let (m, d) = (Some(r(3, false)), Some(r(4, true)));
        // Par id : le sien seulement, du bon genre.
        assert_eq!(decider_retrait(m, Some(3), Vise::Tout), (m, true));
        assert_eq!(decider_retrait(m, Some(2), Vise::Tout), (None, false));
        assert_eq!(decider_retrait(d, Some(4), Vise::Decompte), (d, true));
        assert_eq!(decider_retrait(m, Some(3), Vise::Decompte), (None, false));
        // bandeau_termine ne retire jamais un décompte.
        assert_eq!(decider_retrait(d, Some(4), Vise::Message), (None, false));
        assert_eq!(decider_retrait(m, Some(3), Vise::Message), (m, true));
        // Sans id (cacher, fermer) : tout, et l'état est vidé même s'il était vide.
        assert_eq!(decider_retrait(d, None, Vise::Tout), (d, true));
        assert_eq!(decider_retrait(None, None, Vise::Tout), (None, true));
        assert_eq!(decider_retrait(None, Some(1), Vise::Tout), (None, false));
    }

    #[test]
    fn decision_remplace() {
        assert_eq!(decompte_remplace(Some(r(4, true))), Some(4));
        assert_eq!(decompte_remplace(Some(r(3, false))), None);
        assert_eq!(decompte_remplace(None), None);
    }

    #[test]
    fn la_fenetre_n_est_pas_cachee_sous_un_contenu_deja_montre() {
        assert!(cacher_apres_retrait(None));
        assert!(cacher_apres_retrait(Some(false)));
        assert!(!cacher_apres_retrait(Some(true)));
    }

    #[test]
    fn le_delai_de_zone_ne_court_qu_apres_l_envoi() {
        let c = EnCours::nouveau(1, Contenu::Message(info()));
        // En attente d'une page prête : même longtemps après, rien n'est caché d'office.
        let tard = Instant::now() + Duration::from_secs(60);
        assert_eq!(c.depuis_affichage(tard), Duration::ZERO);
        assert!(!cacher_d_office(true, c.depuis_affichage(tard), false));
        // Envoyé à une page prête : le délai part de là.
        let mut c = c;
        c.renvoyer();
        let t0 = Instant::now();
        c.montre_a = Some(t0);
        assert!(!cacher_d_office(
            true,
            c.depuis_affichage(t0 + Duration::from_secs(2)),
            false
        ));
        assert!(cacher_d_office(
            true,
            c.depuis_affichage(t0 + DELAI_ZONE_MAX),
            false
        ));
    }

    #[test]
    fn un_renvoi_oublie_la_zone_et_le_signal() {
        let mut c = EnCours::nouveau(1, Contenu::Message(info()));
        assert_eq!(c.renvoyer(), 1);
        c.montre_a = Some(Instant::now());
        c.zone = borner_zone(10.0, 10.0, 100.0, 40.0);
        c.signale = true;
        assert_eq!(c.renvoyer(), 2);
        assert!(c.zone.is_none());
        assert!(!c.signale);
        assert!(c.montre_a.is_none());
        assert_eq!(c.depuis_affichage(Instant::now()), Duration::ZERO);
    }
}
