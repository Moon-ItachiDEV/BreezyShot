//! L'Application : le chef d'orchestre. Seul module qui connaît les autres.
//!
//! Pour brancher un module (étapes suivantes) :
//! 1. le créer dans `demarrer`, à l'endroit marqué « Modules », en lui passant un rappel
//!    qui transforme son signal en `Evenement` : `move |s| traiter(&app, Evenement::X(s))` ;
//! 2. ajouter la variante `Evenement::X(module::Signal)` et son cas dans `traiter` ;
//! 3. libérer ce qu'il tient dans `arreter` (raccourcis, fenêtres) ;
//! 4. ajouter ses entrées de menu dans `entrees_menu`.
//!
//! Capture d'une zone (§3.6) : `lancer_capture` puis `traiter_zone` font les gestes,
//! `zone.rs` dit lesquels (fonctions pures). Plein écran (§3.7) : `lancer_capture` puis
//! `capturer_plein_ecran`, sans écran figé ni sélection. Tout ce qui montre ou cache se
//! fait sur le fil principal ; la sortie (disque, presse-papiers) part dans un fil à part.

pub mod arguments;
pub mod etat;
pub mod etats;
pub mod icone;
pub mod journal;
pub mod zone;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, RunEvent};

use crate::capture;
use crate::fenetres::selection::{self, Selection};
use crate::fenetres::{self, Bandeau, Genre, Message};
use crate::raccourcis::{self, Bilan, Decision};
use crate::reglages::{self, Constat, Plateforme, Reglages};
use crate::sortie;
use crate::systeme::gel::{self, Changement};
use crate::systeme::{self, Dossiers, Systeme, Theme};
use arguments::Commande;
use etats::{Automate, Declencheur, Etat, Mode};
use icone::EntreeMenu;
use zone::{Arrivee, Geste, Probleme, Vue};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Identifiant de l'entrée « Quitter BreezyShot ».
pub const MENU_QUITTER: &str = "quitter";

/// Si le fil principal ne répond plus pendant ce délai alors que l'écran figé est montré,
/// l'app s'arrête d'elle-même : l'écran ne doit jamais rester couvert.
pub const DELAI_BLOCAGE: Duration = Duration::from_secs(5);

/// Attente maximale du compositeur après avoir caché le bandeau, avant la photo.
pub const ATTENTE_COMPOSITION: Duration = Duration::from_millis(100);

/// Une capture demandée depuis le menu de l'icône part après ce délai : le menu, qui
/// disparaît en fondu, ne doit pas être sur la photo. Un raccourci n'attend jamais.
pub const ATTENTE_MENU: Duration = Duration::from_millis(350);

/// Vrai quand la vignette existe (étape 9). D'ici là, chaque capture est enregistrée,
/// sans que le réglage soit modifié ni écrit (`zone::doit_enregistrer`).
pub const VIGNETTE_CONSTRUITE: bool = false;

/// Les actions dont la capture existe : seules celles-ci reçoivent un raccourci (§5) et
/// une entrée au menu. `fenetre` s'ajoutera avec son étape.
pub const ACTIONS_CONSTRUITES: [&str; 2] = ["zone", "plein_ecran"];

/// L'ordre des entrées de capture dans le menu de l'icône. Seuls les modes construits
/// (`ACTIONS_CONSTRUITES`) y paraissent.
pub const ORDRE_MENU: [Mode; 3] = [Mode::Zone, Mode::Fenetre, Mode::PleinEcran];

/// Tout ce qui arrive à l'Application : signaux des modules, du système, du menu.
#[derive(Debug, Clone, PartialEq)]
pub enum Evenement {
    /// Une autre instance a été lancée, avec ses arguments (le premier est le programme).
    SecondeInstance(Vec<String>),
    /// Le thème de la barre des tâches a changé.
    ThemeBarre(Theme),
    /// Une entrée du menu de l'icône a été choisie (son identifiant).
    Menu(String),
    /// Signal du module des raccourcis (appui).
    Raccourci(raccourcis::Signal),
    /// Signal du module des fenêtres (clic dans le bandeau).
    Fenetres(fenetres::Signal),
    /// Signal de la sélection de zone.
    Selection(selection::Signal),
    /// Le bureau a changé pendant que l'écran figé était montré (déjà caché par le Gel).
    Bureau(Changement),
    /// La sortie d'une capture est finie (rendue par son fil).
    SortieFaite(Box<FinSortie>),
    /// L'échéance de la sortie numéro `n` est tombée (ou son fil s'est arrêté).
    SortieEchue(u64),
    /// Annonce du démarrage, postée par `demarrer` pour un tour de la boucle déjà lancée.
    AnnonceDemarrage { local: PathBuf, bilan: Bilan },
}

/// Ce que le fil de sortie rend au fil principal.
#[derive(Debug, Clone, PartialEq)]
pub struct FinSortie {
    /// Numéro de la sortie : un bilan qui n'est pas celui attendu est ignoré.
    pub numero: u64,
    pub bilan: sortie::Bilan,
    pub demande: sortie::Demande,
    pub duree_decoupe: Duration,
    pub duree_sortie: Duration,
    /// Réception du signal `Zone` (en plein écran : de la demande de capture).
    pub relache: Instant,
}

/// Une capture de zone en cours, de `montrer_ecran_fige` jusqu'au découpage ou à
/// l'abandon. Elle tient la photo et la garde de l'écran.
struct CaptureEnCours {
    gel: capture::Gel,
    /// L'affichage de sélection ; `None` pendant `Selection::montrer`.
    selection: Option<u64>,
    garde: GardeEcran,
}

/// Filet de sécurité : à sa destruction, l'écran figé et la sélection sont cachés et Échap
/// libéré ; `cacher_tout` le fait plus tôt. Détruite à chaque fin et chaque erreur, et par
/// `lancer_capture` tant qu'elle n'est pas rangée. Rangée dans le `Mutex` de l'état, une
/// panique ne la détruit pas : c'est alors la mort du processus (abort dans une procédure
/// `extern "system"`, panique relancée par tao ailleurs) qui libère l'écran.
struct GardeEcran {
    app: AppHandle,
    fait: bool,
}

impl GardeEcran {
    fn new(app: &AppHandle) -> GardeEcran {
        GardeEcran {
            app: app.clone(),
            fait: false,
        }
    }

    /// Cache l'écran figé et la sélection, libère Échap. Sûr de n'importe quel fil (les
    /// deux masquages le sont ; Échap est libéré sur le fil principal).
    fn cacher_tout(&mut self) {
        self.fait = true;
        gel::cacher_ecran_fige();
        if let Some(s) = self.app.try_state::<Selection>() {
            s.cacher();
        }
        liberer_echap(&self.app);
    }
}

impl Drop for GardeEcran {
    fn drop(&mut self) {
        if !self.fait {
            self.cacher_tout();
        }
    }
}

/// État partagé de l'Application, tenu par Tauri (`app.state::<Application>()`).
#[derive(Default)]
pub struct Application {
    automate: Mutex<Automate>,
    /// Vrai une fois le démarrage complet : l'arrêt écrit alors `arret propre`.
    demarree: AtomicBool,
    taille_icone: AtomicU32,
    /// Réglages lus au démarrage.
    reglages: Mutex<Option<Reglages>>,
    /// Vrai si des raccourcis ont été réservés (à libérer à l'arrêt).
    raccourcis_reserves: AtomicBool,
    /// Annonce affichée mais pas encore vue : `(id du message, dossier local, situation)`.
    /// La situation n'est retenue dans `etat.json` qu'une fois le message montré.
    annonce_en_attente: Mutex<Option<AnnonceEnAttente>>,
    /// Le fil principal (celui de la boucle d'événements), noté au démarrage.
    fil_principal: OnceLock<ThreadId>,
    /// Dossiers résolus au démarrage.
    dossiers: Mutex<Option<Dossiers>>,
    /// La capture en cours. Touchée sur le fil principal seulement, verrou jamais tenu
    /// pendant un appel à un module.
    capture: Mutex<Option<CaptureEnCours>>,
    /// Vrai pendant qu'Échap est réservé en global.
    echap_reserve: AtomicBool,
    /// Numéro de la dernière sortie lancée.
    derniere_sortie: AtomicU64,
    /// La sortie dont on attend le bilan ; `None` une fois le bilan reçu ou l'échéance
    /// tombée.
    sortie_attendue: Mutex<Option<u64>>,
    /// Nombre de captures démarrées (demandes acceptées par la table). Une demande du
    /// menu, différée de `ATTENTE_MENU`, est caduque si ce nombre a changé depuis le clic.
    demandes: AtomicU64,
}

/// Une annonce envoyée au bandeau, dont la situation attend d'être retenue.
#[derive(Debug, Clone)]
struct AnnonceEnAttente {
    id: u64,
    local: std::path::PathBuf,
    situation: Option<raccourcis::Situation>,
}

/// Point d'entrée de l'app (appelé par `main.rs`).
pub fn lancer() {
    let (commande, inconnus) = arguments::lire(std::env::args().skip(1));
    let construction = tauri::Builder::default()
        // L'instance unique en premier : une seconde instance confie ses arguments à la
        // première et sort avec le code 0 avant d'aller plus loin.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _dossier| {
            traiter(app, Evenement::SecondeInstance(argv));
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(Application::default())
        .invoke_handler(tauri::generate_handler![
            fenetres::bandeau_pret,
            fenetres::bandeau_zone,
            fenetres::bandeau_action,
            fenetres::bandeau_termine,
            fenetres::selection::selection_chargee,
            fenetres::selection::selection_prete,
            fenetres::selection::selection_terminee,
            fenetres::selection::selection_annulee
        ])
        .setup(move |app| {
            demarrer(app.handle(), commande, &inconnus);
            Ok(())
        })
        .build(tauri::generate_context!());
    match construction {
        Ok(app) => app.run(|app, evenement| match evenement {
            // L'app vit sans fenêtre : fermer la dernière fenêtre ne la quitte pas.
            // Seul `app.exit(code)` (code présent) quitte.
            RunEvent::ExitRequested {
                code: None, api, ..
            } => api.prevent_exit(),
            RunEvent::Exit => arreter(app),
            _ => {}
        }),
        Err(e) => {
            if !journal::installe() {
                installer_journal_secours();
            }
            log::error!("lancement impossible: {e}");
            eprintln!("BreezyShot : lancement impossible : {e}");
            journal::vider();
            std::process::exit(1);
        }
    }
}

/// Journal de secours dans le dossier temporaire du système.
fn installer_journal_secours() {
    match journal::installer_secours(&systeme::dossier_secours()) {
        Ok(chemin) => eprintln!("BreezyShot : journal de secours : {}", chemin.display()),
        Err(e) => eprintln!("BreezyShot : aucun journal : {e}"),
    }
}

/// Démarrage de la première instance.
fn demarrer(app: &AppHandle, commande: Commande, inconnus: &[String]) {
    // `setup` tourne sur le fil principal (celui de la boucle d'événements).
    let _ = app
        .state::<Application>()
        .fil_principal
        .set(std::thread::current().id());
    // Le journal ne dépend que du dossier local.
    if let Err(e) = systeme::dossier_journal().and_then(|d| journal::installer(&d)) {
        // Dossier local introuvable ou non inscriptible : journal de secours.
        installer_journal_secours();
        log::error!("journal indisponible: {e}");
        eprintln!("BreezyShot : journal indisponible : {e}");
    }
    log::info!("demarrage BreezyShot {VERSION}");
    let dossiers = systeme::dossiers();
    match &dossiers {
        Ok(dossiers) => {
            let images = match &dossiers.images {
                Some(i) => i.display().to_string(),
                None => "introuvable".into(),
            };
            log::info!(
                "dossiers reglages={} local={} images={images}",
                dossiers.reglages.display(),
                dossiers.local.display(),
            );
        }
        Err(e) => {
            log::error!("dossiers introuvables: {e}");
            eprintln!("BreezyShot : dossiers introuvables : {e}");
        }
    }
    for a in inconnus {
        log::warn!("argument inconnu ignore: {a}");
    }

    if commande == Commande::Quitter {
        // Personne à fermer : on sort tout de suite, sans rien installer.
        log::info!("--quitter: aucune instance en cours");
        app.exit(0);
        return;
    }

    match systeme::version_windows() {
        Some(build) => log::info!("systeme windows build {build}"),
        None => log::info!("systeme {}", std::env::consts::OS),
    }
    let echelle = match systeme::echelle_par_ecran_v2() {
        Some(true) => "oui",
        Some(false) => "non",
        None => "sans objet",
    };
    log::info!("echelle par ecran v2: {echelle}");
    let dossiers = dossiers.ok();
    if let Ok(mut d) = app.state::<Application>().dossiers.lock() {
        d.clone_from(&dossiers);
    }

    // Réglages.
    let plateforme = plateforme_reglages(systeme::systeme());
    let reglages = match &dossiers {
        Some(d) => {
            let (reglages, constats) = reglages::charger(&d.reglages, plateforme);
            for c in &constats {
                match c {
                    Constat::Cree { .. } | Constat::MisANiveau { .. } => log::info!("{c}"),
                    _ => log::warn!("{c}"),
                }
            }
            reglages
        }
        None => Reglages::par_defaut(plateforme),
    };
    let candidats = table_candidats(&reglages);
    if let Ok(mut r) = app.state::<Application>().reglages.lock() {
        *r = Some(reglages);
    }

    // Bandeau de messages.
    let pour_bandeau = app.clone();
    match Bandeau::creer(app, move |s| traiter(&pour_bandeau, Evenement::Fenetres(s))) {
        Ok(bandeau) => {
            app.manage(bandeau);
        }
        Err(e) => log::warn!("bandeau indisponible: {e}"),
    }

    // Écran figé et sélection (capture de zone).
    preparer_capture(app);

    // Raccourcis.
    let pour_raccourcis = app.clone();
    let bilan = raccourcis::reserver(app, &candidats, systeme::impr_ecran_reserve(), move |s| {
        traiter(&pour_raccourcis, Evenement::Raccourci(s))
    });
    for ligne in raccourcis::lignes_journal(&bilan) {
        log::info!("{ligne}");
    }
    app.state::<Application>()
        .raccourcis_reserves
        .store(true, Ordering::SeqCst);

    // Icône et menu.
    let theme = systeme::theme_barre();
    let echelle_principale = icone::echelle_ecran_principal(app);
    let taille = icone::taille_icone(echelle_principale);
    app.state::<Application>()
        .taille_icone
        .store(taille, Ordering::Relaxed);
    match icone::installer(
        app,
        theme,
        taille,
        &entrees_menu(|mode| bilan.libelle_menu(mode.nom())),
        |app, id| traiter(app, Evenement::Menu(id)),
    ) {
        Ok(()) => log::info!(
            "icone installee ({taille} px, echelle {echelle_principale}, barre {})",
            theme.nom()
        ),
        Err(e) => log::warn!("icone non installee: {e}"),
    }
    verifier_icone_visible(app);
    let pour_theme = app.clone();
    if let Err(e) = systeme::surveiller_theme_barre(move |t| {
        traiter(&pour_theme, Evenement::ThemeBarre(t));
    }) {
        log::warn!("{e}");
    }

    // Annonce du démarrage (une seule fois par situation). Postée pour la boucle lancée,
    // jamais envoyée depuis `setup` : la création de la sélection (WebView2) y fait
    // tourner une boucle imbriquée, et un message montré là n'a jamais reçu sa zone.
    if let Some(d) = &dossiers {
        poster_plus_tard(
            app,
            Evenement::AnnonceDemarrage {
                local: d.local.clone(),
                bilan: bilan.clone(),
            },
        );
    }

    app.state::<Application>()
        .demarree
        .store(true, Ordering::SeqCst);
    log::info!("pret");
}

/// Prépare l'écran figé, ses filets de sécurité et la sélection (fil principal). Un échec
/// est écrit au journal ; une capture répondra alors « Impossible de capturer ».
fn preparer_capture(app: &AppHandle) {
    match gel::preparer_ecran_fige() {
        Ok(()) => {
            log::info!("ecran fige pret");
            if let Some(h) = gel::poignee_ecran_fige() {
                log::info!("ecran fige fenetre {h}");
            }
            log::info!(
                "ecran fige: verrouillage de session suivi: {}",
                if gel::surveillance_session() {
                    "oui"
                } else {
                    "non"
                }
            );
        }
        Err(e) => log::warn!("ecran fige non prepare: {e}"),
    }
    // Fil principal bloqué avec l'écran figé montré : on arrête tout plutôt que de laisser
    // l'écran couvert (rappel appelé depuis le fil de garde).
    gel::garder_ecran_fige(DELAI_BLOCAGE, || {
        log::error!("ecran fige: fil principal bloque, arret force");
        journal::vider();
        std::process::exit(1);
    });
    // Le rappel tourne dans la procédure de la fenêtre figée : il ne fait que poster.
    let pour_bureau = app.clone();
    gel::quand_bureau_change(move |c| poster_plus_tard(&pour_bureau, Evenement::Bureau(c)));
    let pour_selection = app.clone();
    match Selection::creer(app, move |s| {
        traiter(&pour_selection, Evenement::Selection(s))
    }) {
        Ok(s) => {
            app.manage(s);
            log::info!("selection prete a servir");
        }
        Err(e) => log::warn!("selection indisponible: {e}"),
    }
}

/// Vrai si l'appel vient d'un autre fil que le fil principal (faux tant que celui-ci
/// n'est pas noté : `demarrer` le note en premier).
fn hors_fil_principal(app: &AppHandle) -> bool {
    app.state::<Application>()
        .fil_principal
        .get()
        .is_some_and(|id| *id != std::thread::current().id())
}

/// Poste `evenement` vers le fil principal, pour un tour suivant de la boucle, même
/// depuis le fil principal (`run_on_main_thread` y exécuterait tout de suite). Si aucun
/// fil ne peut être lancé, l'événement est traité tout de suite.
fn poster_plus_tard(app: &AppHandle, evenement: Evenement) {
    let pour_fil = app.clone();
    let copie = evenement.clone();
    let lance = std::thread::Builder::new()
        .name("poste".into())
        .spawn(move || {
            let pour_traiter = pour_fil.clone();
            if let Err(e) = pour_fil.run_on_main_thread(move || traiter(&pour_traiter, evenement)) {
                log::warn!("evenement non poste: {e}");
                gel::cacher_ecran_fige();
            }
        });
    if let Err(e) = lance {
        log::warn!("evenement non poste ({e}): traite tout de suite");
        traiter(app, copie);
    }
}

/// Écrit `icone visible` ou `icone invisible` selon que la zone de notification donne
/// le rectangle de l'icône (jusqu'à 3 s d'attente, sans bloquer le démarrage).
fn verifier_icone_visible(app: &AppHandle) {
    let app = app.clone();
    let lance = std::thread::Builder::new()
        .name("icone-visible".into())
        .spawn(move || {
            for _ in 0..15 {
                if icone::visible(&app) {
                    log::info!("icone visible");
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            log::warn!("icone invisible");
        });
    if let Err(e) = lance {
        log::warn!("icone invisible: verification impossible ({e})");
    }
}

/// Traduit le système en plateforme des réglages (les deux modules ne se connaissent pas).
pub fn plateforme_reglages(s: Systeme) -> Plateforme {
    match s {
        Systeme::Windows => Plateforme::Windows,
        Systeme::Mac => Plateforme::Mac,
        Systeme::Autre => Plateforme::Autre,
    }
}

/// La table `action → candidats` donnée au choix des raccourcis : seulement les actions
/// construites.
pub fn table_candidats(reglages: &Reglages) -> BTreeMap<String, Vec<String>> {
    ACTIONS_CONSTRUITES
        .iter()
        .map(|a| {
            let liste = reglages.raccourcis.candidats(a).unwrap_or_default();
            (a.to_string(), liste.to_vec())
        })
        .collect()
}

/// Ligne de journal d'une décision d'annonce (rien si tout est normal).
pub fn ligne_annonce(decision: &Decision) -> Option<String> {
    let situation = decision.a_retenir.as_ref()?;
    Some(match decision.annonce {
        Some(_) => format!("annonce raccourci: {}", situation.raison),
        None => "annonce raccourci: deja faite".into(),
    })
}

/// Lit la dernière situation, décide, montre l'annonce éventuelle, écrit ou efface.
fn annoncer_raccourcis(app: &AppHandle, local: &std::path::Path, bilan: &Bilan) {
    let derniere = etat::lire_situation(local);
    let decision = raccourcis::decider_annonce(derniere.as_ref(), bilan);
    if let Some(ligne) = ligne_annonce(&decision) {
        log::info!("{ligne}");
    }
    if let Some(annonce) = &decision.annonce {
        let id = afficher(
            app,
            Message {
                genre: if annonce.erreur {
                    Genre::Erreur
                } else {
                    Genre::Info
                },
                titre: annonce.titre.clone(),
                detail: annonce.detail.clone(),
                actions: Vec::new(),
            },
        );
        // Retenue au premier `bandeau_zone` de ce message (Signal::MessageMontre).
        if let (Some(id), Ok(mut attente)) =
            (id, app.state::<Application>().annonce_en_attente.lock())
        {
            *attente = Some(AnnonceEnAttente {
                id,
                local: local.to_path_buf(),
                situation: decision.a_retenir.clone(),
            });
            return;
        }
    }
    retenir_situation(local, decision.a_retenir.as_ref());
}

/// Écrit (ou efface) la situation dans `etat.json`.
fn retenir_situation(local: &std::path::Path, situation: Option<&raccourcis::Situation>) {
    if let Err(e) = etat::ecrire_situation(local, situation) {
        log::warn!("etat non ecrit: {e}");
    }
}

/// Montre un message dans le bandeau, s'il existe. Rend son `id`.
fn afficher(app: &AppHandle, message: Message) -> Option<u64> {
    match app.try_state::<Bandeau>() {
        Some(bandeau) => Some(bandeau.afficher(message)),
        None => {
            log::warn!("message non affiche (pas de bandeau): {}", message.titre);
            None
        }
    }
}

/// Les modes de capture au menu, dans l'ordre du menu : ceux qui sont construits.
pub fn modes_au_menu() -> Vec<Mode> {
    ORDRE_MENU
        .into_iter()
        .filter(|m| ACTIONS_CONSTRUITES.contains(&m.nom()))
        .collect()
}

/// Identifiant de l'entrée de menu d'un mode (reçu dans `Evenement::Menu`).
pub fn id_menu(mode: Mode) -> String {
    format!("capturer_{}", mode.nom())
}

/// Le mode d'une entrée de menu de capture, `None` pour une autre entrée.
pub fn mode_du_menu(id: &str) -> Option<Mode> {
    Mode::TOUS.into_iter().find(|m| id_menu(*m) == id)
}

/// Texte de l'entrée de menu d'un mode, sans raccourci.
pub fn libelle_menu(mode: Mode) -> &'static str {
    match mode {
        Mode::Zone => "Capturer une zone",
        Mode::PleinEcran => "Capturer l'écran",
        Mode::Fenetre => "Capturer une fenêtre",
    }
}

/// Texte de l'entrée d'un mode, avec le raccourci actif à droite (tabulation : aligné à
/// droite par les menus Windows) ; sans raccourci retenu, le libellé seul.
pub fn texte_capturer(mode: Mode, raccourci: Option<&str>) -> String {
    match raccourci {
        Some(r) => format!("{}\t{r}", libelle_menu(mode)),
        None => libelle_menu(mode).into(),
    }
}

/// Les entrées du menu de l'icône, dans l'ordre : les modes construits (`raccourci` donne
/// le raccourci actif d'un mode), un séparateur, « Quitter BreezyShot ». Seulement ce qui
/// marche.
pub fn entrees_menu(raccourci: impl Fn(Mode) -> Option<String>) -> Vec<EntreeMenu> {
    let mut entrees: Vec<EntreeMenu> = modes_au_menu()
        .into_iter()
        .map(|mode| EntreeMenu::Action {
            id: id_menu(mode),
            texte: texte_capturer(mode, raccourci(mode).as_deref()),
        })
        .collect();
    entrees.push(EntreeMenu::Separateur);
    entrees.push(EntreeMenu::Action {
        id: MENU_QUITTER.into(),
        texte: "Quitter BreezyShot".into(),
    });
    entrees
}

/// Le seul endroit où l'Application réagit à ce qui lui arrive.
pub fn traiter(app: &AppHandle, evenement: Evenement) {
    // Ce qui touche à la capture se décide sur le fil principal.
    let de_capture = matches!(
        evenement,
        Evenement::Selection(_)
            | Evenement::Bureau(_)
            | Evenement::SortieFaite(_)
            | Evenement::SortieEchue(_)
            | Evenement::Raccourci(raccourcis::Signal::Echap)
    );
    if de_capture && hors_fil_principal(app) {
        let pour = app.clone();
        if let Err(e) = app.run_on_main_thread(move || traiter(&pour, evenement)) {
            log::warn!("evenement non poste: {e}");
            gel::cacher_ecran_fige();
        }
        return;
    }
    match evenement {
        Evenement::SecondeInstance(argv) => {
            let (commande, _) = arguments::lire(argv.iter().skip(1));
            match commande {
                Commande::Quitter => {
                    log::info!("--quitter recu: fermeture demandee");
                    app.exit(0);
                }
                Commande::Demarrer => log::info!("seconde instance refusee"),
            }
        }
        Evenement::ThemeBarre(theme) => {
            let taille = app
                .state::<Application>()
                .taille_icone
                .load(Ordering::Relaxed);
            match icone::changer_image(app, theme, taille) {
                Ok(()) => log::info!("icone mise a jour: barre {}", theme.nom()),
                Err(e) => log::warn!("icone non mise a jour: {e}"),
            }
        }
        Evenement::Menu(id) => match (id.as_str(), mode_du_menu(&id)) {
            (MENU_QUITTER, _) => {
                log::info!("menu: quitter");
                app.exit(0);
            }
            (_, Some(mode)) => {
                log::info!("menu: capturer {}", mode.nom());
                lancer_capture_apres(app, mode, ATTENTE_MENU);
            }
            (autre, None) => log::warn!("menu: entree inconnue {autre}"),
        },
        Evenement::Raccourci(raccourcis::Signal::Echap) => {
            log::info!("echap global presse");
            traiter_zone(app, Arrivee::Echap);
        }
        Evenement::Selection(signal) => traiter_zone(app, Arrivee::Selection(&signal)),
        Evenement::Bureau(changement) => {
            log::info!("bureau change pendant la capture: {changement:?}");
            traiter_zone(app, Arrivee::Bureau(changement));
        }
        Evenement::SortieFaite(fin) => {
            if prendre_sortie_attendue(app, fin.numero) {
                finir_sortie(app, *fin);
            } else {
                log::warn!("sortie {}: bilan perime ignore", fin.numero);
            }
        }
        Evenement::AnnonceDemarrage { local, bilan } => annoncer_raccourcis(app, &local, &bilan),
        Evenement::SortieEchue(numero) => {
            if prendre_sortie_attendue(app, numero) {
                log::warn!("sortie sans bilan");
                appliquer(app, Declencheur::SortieSansBilan);
                afficher_probleme(app, Probleme::SortieSansBilan);
            }
        }
        Evenement::Raccourci(raccourcis::Signal::Presse(action)) => {
            log::info!("raccourci presse: {action}");
            match Mode::depuis_nom(&action) {
                Some(mode) => lancer_capture(app, mode),
                None => log::warn!("raccourci: action inconnue {action}"),
            }
        }
        Evenement::Fenetres(fenetres::Signal::MessageMontre { id }) => {
            let attente = match app.state::<Application>().annonce_en_attente.lock() {
                Ok(mut a) => a.take_if(|a| a.id == id),
                Err(_) => None,
            };
            if let Some(a) = attente {
                log::info!("annonce montree {id}: situation retenue");
                retenir_situation(&a.local, a.situation.as_ref());
            }
        }
        Evenement::Fenetres(fenetres::Signal::ActionBandeau { id, action }) => {
            // La page ferme le message elle-même ; aucune action n'existe encore.
            log::info!("bandeau action {id} {action}");
        }
    }
}

/// Applique un déclencheur à l'automate. Rend le nouvel état, `None` s'il est ignoré.
fn appliquer(app: &AppHandle, declencheur: Declencheur) -> Option<Etat> {
    match app.state::<Application>().automate.lock() {
        Ok(mut a) => a.traiter(declencheur),
        Err(_) => {
            log::error!("automate inaccessible");
            None
        }
    }
}

/// Montre le bandeau d'erreur d'une capture qui n'a pas pu se faire.
fn afficher_probleme(app: &AppHandle, probleme: Probleme) {
    log::info!("capture impossible: {}", zone::nom(probleme));
    afficher(app, zone::message_capture_impossible());
}

/// Le nombre de captures démarrées jusqu'ici.
fn captures_demarrees(app: &AppHandle) -> u64 {
    app.state::<Application>().demandes.load(Ordering::SeqCst)
}

/// Note une capture démarrée (demande acceptée par la table).
fn noter_demande(app: &AppHandle) {
    app.state::<Application>()
        .demandes
        .fetch_add(1, Ordering::SeqCst);
}

/// Lance la capture `mode` après `attente` (menu de l'icône) : le nombre de captures
/// démarrées est pris au clic, un petit fil dort, puis la demande est postée sur le fil
/// principal. Une capture démarrée entre-temps (raccourci, autre clic arrivé à échéance)
/// la rend caduque (`zone::demande_menu_a_jour`) ; un appui ignoré par la table, non. Si
/// l'état n'est plus Repos ni Vignette à l'échéance, la table ignore la demande.
fn lancer_capture_apres(app: &AppHandle, mode: Mode, attente: Duration) {
    let numero = captures_demarrees(app);
    let pour_fil = app.clone();
    let lance = std::thread::Builder::new()
        .name("menu-attente".into())
        .spawn(move || {
            std::thread::sleep(attente);
            let pour_traiter = pour_fil.clone();
            let poste = pour_fil.run_on_main_thread(move || {
                let actuelle = captures_demarrees(&pour_traiter);
                if zone::demande_menu_a_jour(numero, actuelle) {
                    demarrer_capture(&pour_traiter, mode);
                } else {
                    log::info!("menu: demande perimee");
                }
            });
            if let Err(e) = poste {
                log::warn!("capture non postee: {e}");
            }
        });
    if let Err(e) = lance {
        log::warn!("attente du menu impossible ({e}): capture tout de suite");
        demarrer_capture(app, mode);
    }
}

/// Demande une capture par raccourci. Fil principal (sinon postée), puis
/// `demarrer_capture`.
pub fn lancer_capture(app: &AppHandle, mode: Mode) {
    if hors_fil_principal(app) {
        let pour = app.clone();
        if let Err(e) = app.run_on_main_thread(move || lancer_capture(&pour, mode)) {
            log::warn!("capture non postee: {e}");
        }
        return;
    }
    demarrer_capture(app, mode);
}

/// Démarre une capture (fil principal). La table ignore la demande hors Repos et
/// Vignette (aucun second `figer` tant qu'une capture est en cours) ; acceptée, elle est
/// comptée (une demande du menu en attente devient caduque), puis chaque mode suit sa
/// séquence : zone (§3.6), plein écran (§3.7).
fn demarrer_capture(app: &AppHandle, mode: Mode) {
    let recu_a = Instant::now();
    if appliquer(app, Declencheur::DemandeCapture(mode)).is_none() {
        log::info!("capture deja en cours: appui ignore");
        return;
    }
    noter_demande(app);
    match mode {
        Mode::Zone => capturer_zone(app, recu_a),
        Mode::PleinEcran => capturer_plein_ecran(app, recu_a),
        Mode::Fenetre => {
            // Pas encore construite : aucun raccourci ni menu ne mène ici.
            log::warn!("capture {} pas encore construite", mode.nom());
            appliquer(app, Declencheur::GelImpossible);
        }
    }
}

/// Début commun à tous les modes, une fois en Gel : le bandeau est caché s'il montre un
/// message (il ne doit ni figurer sur la photo ni rester au-dessus de l'écran figé), le
/// compositeur attendu (borné), puis la photo. Erreur → `GelImpossible` (Repos) et bandeau
/// d'erreur ; rend `None`.
fn photographier(app: &AppHandle) -> Option<capture::Gel> {
    let bandeau_cache = app.try_state::<Bandeau>().is_some_and(|b| b.cacher());
    if bandeau_cache && !systeme::attendre_composition(ATTENTE_COMPOSITION) {
        log::warn!(
            "composition non attendue ({} ms): photo prise quand meme",
            ATTENTE_COMPOSITION.as_millis()
        );
    }
    match capture::figer() {
        Ok(g) => Some(g),
        Err(e) => {
            log::warn!("gel impossible: {e}");
            appliquer(app, Declencheur::GelImpossible);
            let probleme = match e {
                capture::Erreur::NonDisponible => Probleme::NonDisponible,
                _ => Probleme::Photo,
            };
            afficher_probleme(app, probleme);
            None
        }
    }
}

/// La souris en pixels réels du bureau, lue tout de suite : par `systeme`, sinon par
/// Tauri (le même repli que le bandeau, `fenetres`).
fn souris_bureau(app: &AppHandle) -> Option<(i32, i32)> {
    zone::point_souris(
        systeme::position_souris().or_else(|| app.cursor_position().ok().map(|p| (p.x, p.y))),
    )
}

/// Plein écran (§3.7), en Gel, sur le fil principal : souris lue avant tout, photo,
/// écran choisi (`capture::ecran_de_capture`), Découpe, puis la sortie. Rien n'est
/// montré : ni écran figé, ni sélection, ni Échap réservé, ni garde.
fn capturer_plein_ecran(app: &AppHandle, recu_a: Instant) {
    let souris = souris_bureau(app);
    let Some(gel_pris) = photographier(app) else {
        return;
    };
    let Some((ecran, origine)) = capture::ecran_de_capture(souris, &gel_pris.ecrans) else {
        log::warn!("plein ecran: aucun ecran");
        appliquer(app, Declencheur::GelImpossible);
        afficher_probleme(app, Probleme::Photo);
        return;
    };
    log::info!("{}", zone::ligne_plein_ecran(ecran.rect, origine));
    log::info!(
        "{}",
        zone::ligne_chrono_photo(gel_pris.duree_photo, gel_pris.duree_totale)
    );
    appliquer(app, Declencheur::PhotoPrise(Mode::PleinEcran));
    let debut = Instant::now();
    let pixels = capture::decouper(&gel_pris, ecran.rect);
    let duree_decoupe = debut.elapsed();
    drop(gel_pris);
    match pixels {
        Ok(pixels) => lancer_sortie(app, pixels, duree_decoupe, recu_a),
        Err(e) => {
            log::warn!("decoupe impossible: {e}");
            appliquer(app, Declencheur::DecoupeImpossible);
            afficher_probleme(app, Probleme::Lecture);
        }
    }
}

/// Zone (§3.6), en Gel : photo, écran figé, sélection, Échap réservé. Toute erreur après
/// `montrer_ecran_fige` cache tout (garde) et revient au Repos.
fn capturer_zone(app: &AppHandle, recu_a: Instant) {
    // Avant toute photo : aucune capture ne doit rester rangée (sa garde, détruite plus
    // tard, cacherait le nouvel écran figé), et la Sélection doit exister (sinon l'écran
    // figé ne serait jamais montré pour rien).
    if let Some(ancienne) = ranger_capture(app, None) {
        log::error!("capture precedente encore rangee: abandonnee avant la photo");
        drop(ancienne);
    }
    if app.try_state::<Selection>().is_none() {
        log::warn!("selection absente: pas de photo");
        appliquer(app, Declencheur::GelImpossible);
        afficher_probleme(app, Probleme::Selection);
        return;
    }
    // 2 et 3. Bandeau caché, compositeur attendu, la photo.
    let Some(gel_pris) = photographier(app) else {
        return;
    };
    // 4. L'écran figé. La garde vit dès maintenant : toute sortie le cache.
    let garde = GardeEcran::new(app);
    if let Err(e) = gel::montrer_ecran_fige(&gel_pris.image) {
        log::warn!("ecran fige non montre: {e}");
        drop(garde);
        appliquer(app, Declencheur::GelImpossible);
        afficher_probleme(app, Probleme::EcranFige);
        return;
    }
    log::info!(
        "{}",
        zone::ligne_chrono_gel(
            gel_pris.duree_photo,
            gel_pris.duree_totale,
            recu_a.elapsed()
        )
    );
    appliquer(app, Declencheur::PhotoPrise(Mode::Zone));
    // 5. La sélection, juste au-dessus de l'écran figé.
    let demande = selection::Demande {
        bureau: gel_pris.bureau,
        ecrans: gel_pris.ecrans.iter().map(|e| e.rect).collect(),
        dessous: gel::poignee_ecran_fige(),
        cote_min: capture::COTE_MIN,
    };
    // La capture est notée avant `montrer` : un signal de fin arrivé pendant `montrer`
    // la trouve et la termine. La place est vide : vidée avant la photo, sur ce même fil,
    // et rien entre-temps ne range de capture.
    let ancienne = ranger_capture(
        app,
        Some(CaptureEnCours {
            gel: gel_pris,
            selection: None,
            garde,
        }),
    );
    if ancienne.is_some() {
        log::error!("capture rangee pendant la photo: impossible, ignoree");
        // Jamais détruite ici : sa garde cacherait l'écran figé qu'on vient de montrer.
        std::mem::forget(ancienne);
    }
    let montree = match app.try_state::<Selection>() {
        Some(s) => s.montrer(demande),
        None => Err("selection absente".into()),
    };
    match montree {
        Ok(id) => {
            if !noter_selection(app, id) {
                // Terminée pendant `montrer` : rien à réserver.
                return;
            }
        }
        Err(e) => {
            log::warn!("selection non montree: {e}");
            abandonner(
                app,
                Declencheur::SelectionImpossible,
                Some(Probleme::Selection),
            );
            return;
        }
    }
    // 6. Échap global, le temps de la sélection.
    reserver_echap(app);
}

/// La capture en cours, sous son verrou. Un verrou empoisonné (panique pendant qu'il
/// était tenu) est repris : la garde qu'il contient doit rester joignable.
fn capture_en_cours(app: &AppHandle) -> std::sync::MutexGuard<'_, Option<CaptureEnCours>> {
    app.state::<Application>()
        .inner()
        .capture
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Range (ou retire avec `None`) la capture en cours. Rend l'ancienne, à détruire hors
/// du verrou.
fn ranger_capture(app: &AppHandle, nouvelle: Option<CaptureEnCours>) -> Option<CaptureEnCours> {
    std::mem::replace(&mut *capture_en_cours(app), nouvelle)
}

/// Note l'affichage de sélection de la capture en cours. Faux s'il n'y en a plus.
fn noter_selection(app: &AppHandle, id: u64) -> bool {
    match capture_en_cours(app).as_mut() {
        Some(capture) => {
            capture.selection = Some(id);
            true
        }
        None => false,
    }
}

/// Réserve Échap en global ; s'il ne peut pas l'être, la page gère Échap quand elle a le
/// focus.
fn reserver_echap(app: &AppHandle) {
    let pour_echap = app.clone();
    match raccourcis::reserver_echap(app, move |s| traiter(&pour_echap, Evenement::Raccourci(s))) {
        Ok(()) => {
            app.state::<Application>()
                .echap_reserve
                .store(true, Ordering::SeqCst);
            log::info!("echap global reserve");
        }
        Err(e) => log::warn!("echap global non reserve: {e}"),
    }
}

/// Libère Échap s'il est réservé, sur le fil principal.
fn liberer_echap(app: &AppHandle) {
    if !app
        .state::<Application>()
        .echap_reserve
        .swap(false, Ordering::SeqCst)
    {
        return;
    }
    let liberer = |app: &AppHandle| match raccourcis::liberer_echap(app) {
        Ok(()) => log::info!("echap global libere"),
        Err(e) => log::warn!("echap global non libere: {e}"),
    };
    if hors_fil_principal(app) {
        let pour = app.clone();
        if let Err(e) = app.run_on_main_thread(move || liberer(&pour)) {
            log::warn!("echap global non libere: {e}");
        }
    } else {
        liberer(app);
    }
}

/// Retire la capture en cours, cache tout (garde), revient au Repos, montre l'erreur.
/// Sans capture en cours (déjà terminée par un autre signal), ne fait rien.
fn abandonner(app: &AppHandle, declencheur: Declencheur, probleme: Option<Probleme>) {
    let Some(mut capture) = ranger_capture(app, None) else {
        return;
    };
    capture.garde.cacher_tout();
    drop(capture);
    appliquer(app, declencheur);
    if let Some(p) = probleme {
        afficher_probleme(app, p);
    }
}

/// Un signal pendant une capture de zone (fil principal) : décision pure, puis gestes.
fn traiter_zone(app: &AppHandle, arrivee: Arrivee<'_>) {
    let geste = {
        let c = capture_en_cours(app);
        zone::decider(
            c.as_ref().map(|c| Vue {
                selection: c.selection,
                ecrans: &c.gel.ecrans,
            }),
            arrivee,
        )
    };
    match geste {
        Geste::Rien => {}
        Geste::Abandonner {
            declencheur,
            probleme,
        } => abandonner(app, declencheur, probleme),
        Geste::Decouper(zone) => decouper(app, zone),
    }
}

/// Zone choisie : l'écran figé disparaît dans le même tour que la sélection, puis la
/// découpe (écran déjà libre, garde au repos) et la sortie dans son fil.
fn decouper(app: &AppHandle, zone: systeme::Rect) {
    let relache = Instant::now();
    let Some(mut capture) = ranger_capture(app, None) else {
        return;
    };
    capture.garde.cacher_tout();
    appliquer(app, Declencheur::ZoneChoisie);
    log::info!(
        "zone choisie {},{} {}x{}",
        zone.x,
        zone.y,
        zone.largeur,
        zone.hauteur
    );
    let debut = Instant::now();
    let pixels = capture::decouper(&capture.gel, zone);
    let duree_decoupe = debut.elapsed();
    drop(capture);
    match pixels {
        Ok(pixels) => lancer_sortie(app, pixels, duree_decoupe, relache),
        Err(e) => {
            log::warn!("decoupe impossible: {e}");
            appliquer(app, Declencheur::DecoupeImpossible);
            afficher_probleme(app, Probleme::Lecture);
        }
    }
}

/// Ce que la sortie doit faire, d'après les réglages lus au démarrage.
fn demande_sortie(app: &AppHandle) -> sortie::Demande {
    let etat = app.state::<Application>();
    let reglages = etat
        .reglages
        .lock()
        .ok()
        .and_then(|r| r.clone())
        .unwrap_or_else(|| Reglages::par_defaut(plateforme_reglages(systeme::systeme())));
    let dossiers = etat.dossiers.lock().ok().and_then(|d| d.clone());
    let images = dossiers.as_ref().and_then(|d| d.images.clone());
    let (dossier_enregistrement, dossier_affiche) = zone::dossier_enregistrement(
        reglages.dossier_enregistrement.as_deref(),
        images.as_deref(),
    );
    let dossier_recents = dossiers
        .map(|d| d.local.join(zone::SOUS_DOSSIER_RECENTS))
        .unwrap_or_else(PathBuf::new);
    sortie::Demande {
        copier: reglages.copier_apres_capture,
        // Jamais écrit dans les réglages : la valeur lue reste telle quelle.
        enregistrer: zone::doit_enregistrer(
            reglages.enregistrer_apres_capture,
            VIGNETTE_CONSTRUITE,
            !dossier_enregistrement.as_os_str().is_empty(),
        ),
        dossier_enregistrement,
        dossier_affiche,
        dossier_recents,
        modele_nom: reglages.modele_nom,
        maintenant: chrono::Local::now(),
    }
}

/// Vrai (et plus rien d'attendu ensuite) si `numero` est la sortie dont on attend le
/// bilan (`zone::bilan_attendu`).
fn prendre_sortie_attendue(app: &AppHandle, numero: u64) -> bool {
    let mut attendue = app
        .state::<Application>()
        .inner()
        .sortie_attendue
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if zone::bilan_attendu(*attendue, numero) {
        *attendue = None;
        true
    } else {
        false
    }
}

/// Sort une capture et mesure la durée (fil de sortie). Une panique de `sortir` donne un
/// bilan d'échec : le fil rend toujours un bilan.
fn sortir_et_mesurer(
    numero: u64,
    pixels: &gel::Pixels,
    demande: sortie::Demande,
    duree_decoupe: Duration,
    relache: Instant,
) -> Box<FinSortie> {
    let debut = Instant::now();
    let bilan = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        sortie::sortir(pixels, &demande)
    }))
    .unwrap_or_else(|_| {
        log::error!("sortie {numero}: fil interrompu (panique)");
        zone::bilan_d_echec(&demande, zone::CAUSE_SORTIE_INTERROMPUE)
    });
    Box::new(FinSortie {
        numero,
        bilan,
        demande,
        duree_decoupe,
        duree_sortie: debut.elapsed(),
        relache,
    })
}

/// La sortie dans un fil à part (le fil principal n'attend jamais le disque ni le
/// presse-papiers), puis le bilan revient au fil principal (`SortieFaite`). Un second
/// petit fil tient l'échéance (`zone::DELAI_SORTIE`) : sans nouvelles du fil de sortie à
/// temps, il poste `SortieEchue`, et l'automate ne reste jamais en Découpe.
fn lancer_sortie(app: &AppHandle, pixels: gel::Pixels, duree_decoupe: Duration, relache: Instant) {
    let etat = app.state::<Application>();
    let numero = etat.derniere_sortie.fetch_add(1, Ordering::SeqCst) + 1;
    *etat
        .inner()
        .sortie_attendue
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(numero);
    let demande = demande_sortie(app);
    let pixels = Arc::new(pixels);
    let (fini, attente) = std::sync::mpsc::channel::<()>();
    let pour_fil = app.clone();
    let (pixels_fil, demande_fil) = (Arc::clone(&pixels), demande.clone());
    let lance = std::thread::Builder::new()
        .name("sortie".into())
        .spawn(move || {
            let fin = sortir_et_mesurer(numero, &pixels_fil, demande_fil, duree_decoupe, relache);
            let pour_traiter = pour_fil.clone();
            let rendu = pour_fil
                .run_on_main_thread(move || traiter(&pour_traiter, Evenement::SortieFaite(fin)));
            if let Err(e) = rendu {
                log::warn!("bilan de sortie non rendu: {e}");
            }
            let _ = fini.send(());
        });
    if let Err(e) = lance {
        // Sans fil, la capture n'est pas perdue : la sortie se fait ici (l'écran est déjà
        // libre, la garde ne surveille plus).
        log::warn!("fil de sortie impossible ({e}): sortie sur le fil principal");
        let fin = sortir_et_mesurer(numero, &pixels, demande, duree_decoupe, relache);
        traiter(app, Evenement::SortieFaite(fin));
        return;
    }
    // L'échéance : se termine dès que le fil de sortie a rendu son bilan. S'il s'arrête
    // sans rien dire (canal fermé), l'échéance part tout de suite ; arrivée après le
    // bilan, elle est ignorée.
    let pour_echeance = app.clone();
    let echeance = std::thread::Builder::new()
        .name("sortie-echeance".into())
        .spawn(move || {
            if attente.recv_timeout(zone::DELAI_SORTIE).is_ok() {
                return;
            }
            let pour_traiter = pour_echeance.clone();
            let _ = pour_echeance
                .run_on_main_thread(move || traiter(&pour_traiter, Evenement::SortieEchue(numero)));
        });
    if let Err(e) = echeance {
        log::warn!("echeance de sortie non armee: {e}");
    }
}

/// Bilan de la sortie (fil principal) : journal, retour au Repos, annonce.
fn finir_sortie(app: &AppHandle, fin: FinSortie) {
    log::info!("{}", zone::ligne_sortie(&fin.bilan));
    if let Err(cause) = &fin.bilan.recent {
        log::warn!("recents: echec ({cause})");
    }
    let annonce = sortie::annonce(&fin.bilan, &fin.demande);
    log::info!("{}", zone::ligne_annonce(&annonce));
    appliquer(app, Declencheur::ImagePreteSansVignette);
    afficher(app, zone::message_annonce(&annonce));
    log::info!(
        "{}",
        zone::ligne_chrono_sortie(fin.duree_decoupe, fin.duree_sortie, fin.relache.elapsed())
    );
}

/// Arrêt : libère ce qui est tenu, retire l'icône, écrit `arret propre`.
fn arreter(app: &AppHandle) {
    if !app.state::<Application>().demarree.load(Ordering::SeqCst) {
        journal::vider();
        return;
    }
    // Capture en cours : la garde cache tout et libère Échap. Puis la sélection est
    // fermée avant la destruction de l'écran figé, et l'image du bureau rendue.
    let capture = ranger_capture(app, None);
    drop(capture);
    liberer_echap(app);
    if let Some(s) = app.try_state::<Selection>() {
        s.fermer();
    }
    gel::detruire_ecran_fige();
    gel::liberer_image();
    // Modules : libérer les raccourcis, fermer les fenêtres.
    if app
        .state::<Application>()
        .raccourcis_reserves
        .swap(false, Ordering::SeqCst)
    {
        match raccourcis::liberer(app) {
            Ok(()) => log::info!("raccourcis liberes"),
            Err(e) => log::warn!("raccourcis non liberes: {e}"),
        }
    }
    if let Some(bandeau) = app.try_state::<Bandeau>() {
        bandeau.fermer();
    }
    if icone::retirer(app) {
        log::info!("icone retiree");
    }
    log::info!("arret propre");
    journal::vider();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raccourcis::{choisir, decider_annonce, Situation};

    fn action(id: &str, texte: &str) -> EntreeMenu {
        EntreeMenu::Action {
            id: id.into(),
            texte: texte.into(),
        }
    }

    #[test]
    fn menu_avec_les_deux_raccourcis() {
        let m = entrees_menu(|mode| match mode {
            Mode::Zone => Some("Ctrl + Maj + 1".into()),
            Mode::PleinEcran => Some("Ctrl + Maj + 2".into()),
            Mode::Fenetre => Some("Ctrl + Maj + 3".into()),
        });
        assert_eq!(
            m,
            [
                action("capturer_zone", "Capturer une zone\tCtrl + Maj + 1"),
                action("capturer_plein_ecran", "Capturer l'écran\tCtrl + Maj + 2"),
                EntreeMenu::Separateur,
                action(MENU_QUITTER, "Quitter BreezyShot"),
            ]
        );
    }

    #[test]
    fn menu_sans_raccourci() {
        let m = entrees_menu(|_| None);
        assert_eq!(
            m,
            [
                action("capturer_zone", "Capturer une zone"),
                action("capturer_plein_ecran", "Capturer l'écran"),
                EntreeMenu::Separateur,
                action(MENU_QUITTER, "Quitter BreezyShot"),
            ]
        );
        // Un seul des deux pris : l'autre entrée reste, sans raccourci.
        let m = entrees_menu(|mode| (mode == Mode::Zone).then(|| "Impr. écran".to_string()));
        assert_eq!(
            m[0],
            action("capturer_zone", "Capturer une zone\tImpr. écran")
        );
        assert_eq!(m[1], action("capturer_plein_ecran", "Capturer l'écran"));
    }

    #[test]
    fn identifiants_du_menu() {
        for mode in Mode::TOUS {
            assert_eq!(mode_du_menu(&id_menu(mode)), Some(mode));
        }
        assert_eq!(mode_du_menu(MENU_QUITTER), None);
        assert_eq!(mode_du_menu("capturer_autre"), None);
        assert_eq!(modes_au_menu(), [Mode::Zone, Mode::PleinEcran]);
        assert_eq!(texte_capturer(Mode::Fenetre, None), "Capturer une fenêtre");
    }

    #[test]
    fn actions_construites() {
        assert_eq!(ACTIONS_CONSTRUITES, ["zone", "plein_ecran"]);
        for a in ACTIONS_CONSTRUITES {
            assert!(Mode::depuis_nom(a).is_some(), "{a}");
        }
    }

    #[test]
    fn seules_les_actions_construites_sont_proposees() {
        let r = Reglages::par_defaut(Plateforme::Windows);
        let t = table_candidats(&r);
        assert_eq!(t.len(), 2);
        assert_eq!(t["zone"], ["ImprEcran", "Ctrl+Maj+1"]);
        assert_eq!(t["plein_ecran"], ["Ctrl+Maj+2"]);
        assert!(!t.contains_key("fenetre"));
    }

    /// Ctrl + Maj + 4, T et H ne sont pris pour rien, sur aucune plateforme.
    #[test]
    fn raccourcis_reserves_a_plus_tard_jamais_pris() {
        for p in [Plateforme::Windows, Plateforme::Mac, Plateforme::Autre] {
            let t = table_candidats(&Reglages::par_defaut(p));
            let b = choisir(&t, false, |_| true);
            for a in &b.actions {
                let retenu = a.retenu.as_ref().map(|r| r.canonique());
                for interdit in ["Ctrl+Maj+4", "Ctrl+Maj+T", "Ctrl+Maj+H"] {
                    assert_ne!(retenu.as_deref(), Some(interdit), "{p:?} {}", a.action);
                }
            }
        }
    }

    /// Ctrl + Maj + 2 pris par une autre application : la zone garde son raccourci, le
    /// plein écran n'en a aucun et reste au menu sans raccourci affiché.
    #[test]
    fn plein_ecran_sans_raccourci() {
        let t = table_candidats(&Reglages::par_defaut(Plateforme::Windows));
        let b = choisir(&t, false, |r| r.canonique() != "Ctrl+Maj+2");
        assert_eq!(b.libelle_menu("zone").as_deref(), Some("Impr. écran"));
        assert_eq!(b.libelle_menu("plein_ecran"), None);
        let m = entrees_menu(|mode| b.libelle_menu(mode.nom()));
        assert_eq!(
            m[0],
            action("capturer_zone", "Capturer une zone\tImpr. écran")
        );
        assert_eq!(m[1], action("capturer_plein_ecran", "Capturer l'écran"));
        // L'annonce du démarrage (§5) : une info qui nomme l'action, pas une erreur.
        assert_eq!(
            b.annonce().map(|a| (a.erreur, a.titre)),
            Some((false, "Aucun raccourci pour l'écran".to_string()))
        );
        assert_eq!(
            ligne_annonce(&decider_annonce(None, &b)).as_deref(),
            Some("annonce raccourci: action_sans_raccourci")
        );
    }

    #[test]
    fn plateformes() {
        assert_eq!(plateforme_reglages(Systeme::Windows), Plateforme::Windows);
        assert_eq!(plateforme_reglages(Systeme::Mac), Plateforme::Mac);
        assert_eq!(plateforme_reglages(Systeme::Autre), Plateforme::Autre);
    }

    #[test]
    fn lignes_d_annonce() {
        let t = table_candidats(&Reglages::par_defaut(Plateforme::Windows));
        let normal = choisir(&t, false, |_| true);
        assert_eq!(ligne_annonce(&decider_annonce(None, &normal)), None);
        let reserve = choisir(&t, true, |_| true);
        let d = decider_annonce(None, &reserve);
        assert_eq!(
            ligne_annonce(&d).as_deref(),
            Some("annonce raccourci: reserve_par_le_systeme")
        );
        let deja: Option<&Situation> = d.a_retenir.as_ref();
        assert_eq!(
            ligne_annonce(&decider_annonce(deja, &reserve)).as_deref(),
            Some("annonce raccourci: deja faite")
        );
        let rien = choisir(&t, false, |_| false);
        assert_eq!(
            ligne_annonce(&decider_annonce(None, &rien)).as_deref(),
            Some("annonce raccourci: aucun_raccourci")
        );
    }
}
