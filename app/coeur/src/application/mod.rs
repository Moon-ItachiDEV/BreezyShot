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
pub mod fenetre_reglages;
pub mod icone;
pub mod installateur;
pub mod journal;
pub mod page_reglages;
pub mod vignette;
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
use crate::fenetres::vignette::{self as carte, Vignette};
use crate::fenetres::{self, Bandeau, Genre, Message};
use crate::raccourcis::{self, Bilan, Decision};
use crate::reglages::{self, Constat, Plateforme, Reglages};
use crate::sortie::{self, Annonce};
use crate::systeme::gel::{self, Changement};
use crate::systeme::{self, Dossiers, Systeme, Theme};
use arguments::Commande;
use etats::{Automate, Declencheur, Etat, Mode};
use icone::EntreeMenu;
use vignette::{CaptureMontree, EtatSuppression};
use zone::{Arrivee, Geste, Probleme, Vue};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Identifiant de l'entrée « Quitter BreezyShot ».
pub const MENU_QUITTER: &str = "quitter";

/// Identifiant de l'entrée « Réglages ».
pub const MENU_REGLAGES: &str = "reglages";

/// Si le fil principal ne répond plus pendant ce délai alors que l'écran figé est montré,
/// l'app s'arrête d'elle-même : l'écran ne doit jamais rester couvert.
pub const DELAI_BLOCAGE: Duration = Duration::from_secs(5);

/// Attente maximale du compositeur après avoir caché le bandeau, avant la photo.
pub const ATTENTE_COMPOSITION: Duration = Duration::from_millis(100);

/// Une capture demandée depuis le menu de l'icône part après ce délai : le menu, qui
/// disparaît en fondu, ne doit pas être sur la photo. Un raccourci n'attend jamais.
pub const ATTENTE_MENU: Duration = Duration::from_millis(350);

/// Vrai depuis que la vignette existe (étape 9) : l'enregistrement automatique ne dépend
/// plus que du réglage (`zone::doit_enregistrer`) ; sinon, le bouton Enregistrer.
pub const VIGNETTE_CONSTRUITE: bool = true;

/// Les actions dont la capture existe : seules celles-ci reçoivent un raccourci (§5) et
/// une entrée au menu.
pub const ACTIONS_CONSTRUITES: [&str; 3] = ["zone", "plein_ecran", "fenetre"];

/// L'ordre des entrées de capture dans le menu de l'icône (celui des raccourcis 1, 2, 3).
/// Seuls les modes construits (`ACTIONS_CONSTRUITES`) y paraissent.
pub const ORDRE_MENU: [Mode; 3] = [Mode::Zone, Mode::PleinEcran, Mode::Fenetre];

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
    /// Signal de la vignette ou de son souffle.
    Vignette(carte::Signal),
    /// Copier ou Enregistrer de la vignette est fini (rendu par son fil).
    ActionFaite(Box<FinAction>),
    /// Un fichier d'une capture supprimée n'a pas pu être effacé (rendu par le fil de la
    /// suppression, qui a déjà écrit son bilan au journal).
    SuppressionFaite { message: Message },
    /// Le glisser de la carte `id`, reposté hors du gestionnaire de la page.
    LancerGlisser { id: u64, chemin: PathBuf },
    /// Capture demandée par la ligne de commande (`--capturer <mode>`, l'installateur).
    CaptureCli(String),
}

/// Ce que le fil d'une action de la vignette rend au fil principal.
#[derive(Debug, Clone, PartialEq)]
pub struct FinAction {
    pub id: u64,
    pub action: String,
    pub bilan: sortie::Bilan,
}

/// L'aperçu de la vignette : le PNG et sa taille.
pub type Apercu = (Arc<Vec<u8>>, (u32, u32));

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
    /// Les pixels, gardés pour la vignette (Copier, Enregistrer).
    pub pixels: Arc<gel::Pixels>,
    /// Le rectangle capturé, pixels réels du bureau.
    pub capture: systeme::Rect,
    /// La vignette était voulue au lancement de la sortie ; l'aperçu (`None` s'il n'a pas
    /// pu être fait, ou pas voulu).
    pub vignette: bool,
    pub apercu: Option<Apercu>,
}

/// Une capture de zone en cours, de `montrer_ecran_fige` jusqu'au découpage ou à
/// l'abandon. Elle tient la photo et la garde de l'écran.
struct CaptureEnCours {
    gel: capture::Gel,
    /// L'affichage de sélection ; `None` pendant `Selection::montrer`.
    selection: Option<u64>,
    /// Le mode de la sélection montrée.
    mode: selection::Mode,
    /// Mode fenêtre : la liste envoyée à la Sélection (rectangles et rayons), dont
    /// `Signal::Fenetre::indice` est un indice. Vide en mode zone.
    fenetres: Vec<selection::FenetreProposee>,
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
    /// Réglages en vigueur : lus au démarrage, changés par la fenêtre Réglages.
    reglages: Mutex<Option<Reglages>>,
    /// Ce que l'app a lu ou écrit dans le fichier en dernier : un fichier relu différent
    /// a été changé de l'extérieur (fenêtre Réglages).
    derniere_lecture: Mutex<Option<Reglages>>,
    /// Les raccourcis tenus (réservés au démarrage, changés à chaud par les Réglages).
    reservations: OnceLock<raccourcis::Reservations>,
    /// Refus à dire sous un raccourci dans la fenêtre Réglages, par action.
    refus_raccourcis: Mutex<std::collections::BTreeMap<String, String>>,
    /// « Réglages non enregistrés » quand la dernière écriture a échoué.
    alerte_reglages: Mutex<Option<String>>,
    /// Saisie de raccourci en cours (son numéro), et le dernier numéro donné.
    saisie: Mutex<Option<u64>>,
    numero_saisie: AtomicU64,
    /// La page des Réglages a déjà demandé son état une fois.
    page_reglages_prete: AtomicBool,
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
    /// La capture que montre la vignette (fil principal ; verrou jamais tenu pendant un
    /// appel à un module).
    montree: Mutex<Option<CaptureMontree>>,
    /// Vrai pendant la boucle d'un glisser.
    glisser: AtomicBool,
    /// Captures parties dont l'effacement est lancé alors qu'un Enregistrer tournait :
    /// son fichier, arrivé en retard, est effacé aussi.
    retenues: Mutex<Vec<u64>>,
    /// Fils de suppression en cours, attendus (bornés) à l'arrêt.
    suppressions: (Mutex<u32>, std::sync::Condvar),
    /// Capture demandée par `--capturer` avant la fin du démarrage : jouée à `pret`.
    capture_en_attente: Mutex<Option<String>>,
}

/// Une annonce envoyée au bandeau, dont la situation attend d'être retenue.
#[derive(Debug, Clone)]
struct AnnonceEnAttente {
    id: u64,
    local: std::path::PathBuf,
    situation: Option<raccourcis::Situation>,
}

/// Point d'entrée de l'exe (appelé par `main.rs`). Le même fichier est l'app, l'installateur
/// ou la désinstallation, selon son nom et la ligne de commande (§3.1). Le contexte Tauri
/// (qui embarque les pages) n'est engendré qu'une fois, puis donné à l'un des deux parcours.
pub fn lancer() {
    let (commande, inconnus) = arguments::lire(std::env::args().skip(1));
    let nom_exe = std::env::current_exe()
        .ok()
        .as_deref()
        .and_then(arguments::nom_de_fichier);
    // La désinstallation ne construit pas Tauri (pas de fenêtre) ; les deux autres parcours
    // reçoivent le contexte, engendré une seule fois ici (un seul `generate_context!`).
    if let arguments::Lancement::Desinstallation =
        arguments::lancement(nom_exe.as_deref(), &commande)
    {
        std::process::exit(desinstaller());
    }
    let contexte = tauri::generate_context!();
    match arguments::lancement(nom_exe.as_deref(), &commande) {
        arguments::Lancement::App(commande) => lancer_app(contexte, commande, inconnus),
        arguments::Lancement::Installateur => {
            std::process::exit(installateur::lancer_installateur(contexte))
        }
        arguments::Lancement::Desinstallation => std::process::exit(desinstaller()),
    }
}

/// Désinstalle, en donnant à l'installateur le retrait du démarrage avec Windows (la valeur
/// `Run` appartient aux Réglages : l'installateur ne la connaît pas).
fn desinstaller() -> i32 {
    installateur::desinstaller_avec(Box::new(|| {
        let avant = systeme::demarrage_avec_systeme(None)?;
        systeme::demarrage_avec_systeme(Some(false))?;
        log::info!("{}", ligne_demarrage_retire(avant));
        Ok(())
    }))
}

/// Ligne du journal de la désinstallation pour le démarrage avec Windows, d'après l'état lu
/// avant le retrait. Fonction pure.
pub fn ligne_demarrage_retire(avant: systeme::EtatDemarrage) -> &'static str {
    use systeme::EtatDemarrage as E;
    match avant {
        E::Actif | E::DesactiveParWindows => "desinstallation demarrage: retire",
        E::Inactif | E::Indisponible => "desinstallation demarrage: absent",
    }
}

/// Démarre l'app elle-même (la commande n'est jamais `Desinstaller` ici).
fn lancer_app(contexte: tauri::Context, commande: Commande, inconnus: Vec<String>) {
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
            fenetres::selection::selection_cliquee,
            fenetres::selection::selection_annulee,
            fenetre_reglages::reglages_lire,
            fenetre_reglages::reglages_changer,
            fenetre_reglages::reglages_apercu_nom,
            fenetre_reglages::reglages_parcourir,
            fenetre_reglages::reglages_saisie,
            fenetre_reglages::reglages_raccourci,
            fenetre_reglages::reglages_retablir,
            carte::vignette_pret,
            carte::vignette_image,
            carte::vignette_zone,
            carte::vignette_entree,
            carte::vignette_action,
            carte::vignette_glisser,
            carte::vignette_terminee,
            carte::souffle_pret,
            carte::souffle_termine
        ])
        .setup(move |app| {
            demarrer(app.handle(), commande, &inconnus);
            Ok(())
        })
        .build(contexte);
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
    // `--capturer <mode>` au démarrage (l'exe lancé directement) : la demande attend `pret`.
    if let Commande::Capturer(mode) = &commande {
        log::info!("--capturer recu: {}", mode_journal(mode));
        if let Ok(mut attente) = app.state::<Application>().capture_en_attente.lock() {
            *attente = Some(mode.clone());
        }
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
    fenetre_reglages::retenir_theme(&reglages);
    log::info!("theme interface: {}", reglages.theme.texte());
    if systeme::demarrage::cle_d_essai() {
        log::info!("demarrage: cle d'essai");
    }
    fenetre_reglages::mettre_a_jour_demarrage();
    if let Ok(mut d) = app.state::<Application>().derniere_lecture.lock() {
        *d = Some(reglages.clone());
    }
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

    // Vignette et souffle, cachés jusqu'à la première capture.
    let pour_vignette = app.clone();
    match Vignette::creer(app, move |s| {
        traiter(&pour_vignette, Evenement::Vignette(s))
    }) {
        Ok(v) => {
            app.manage(v);
        }
        Err(e) => log::warn!("vignette indisponible: {e}"),
    }

    // Écran figé et sélection (capture de zone).
    preparer_capture(app);

    // Raccourcis.
    let pour_raccourcis = app.clone();
    let reservations =
        raccourcis::reserver(app, &candidats, systeme::impr_ecran_reserve(), move |s| {
            traiter(&pour_raccourcis, Evenement::Raccourci(s))
        });
    let bilan = reservations.bilan();
    let _ = app.state::<Application>().reservations.set(reservations);
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

    // Capture demandée avant `pret` (`--capturer` au démarrage) : postée pour le tour
    // suivant, maintenant que tout est en place.
    let en_attente = app
        .state::<Application>()
        .capture_en_attente
        .lock()
        .ok()
        .and_then(|mut a| a.take());
    if let Some(mode) = en_attente {
        poster_plus_tard(app, Evenement::CaptureCli(mode));
    }
}

/// Nom de mode pour le journal, ou `<vide>` / `<inconnu: …>`.
fn mode_journal(mode: &str) -> &str {
    if mode.is_empty() {
        "<vide>"
    } else if Mode::depuis_nom(mode).is_some() {
        mode
    } else {
        // Jamais le texte brut d'une option au journal.
        "<inconnu>"
    }
}

/// Lance la capture demandée par `--capturer`. Une valeur inconnue ne fait qu'une ligne au
/// journal (ARCHITECTURE §3.4). Sans `ATTENTE_MENU` : la demande est directe.
fn capturer_cli(app: &AppHandle, mode: &str) {
    match Mode::depuis_nom(mode) {
        Some(m) => lancer_capture(app, m),
        None => log::warn!("--capturer: mode inconnu"),
    }
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
/// le raccourci actif d'un mode), un séparateur, « Réglages », un séparateur, « Quitter
/// BreezyShot ». Seulement ce qui marche.
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
        id: MENU_REGLAGES.into(),
        texte: "Réglages".into(),
    });
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
            | Evenement::Vignette(_)
            | Evenement::ActionFaite(_)
            | Evenement::SuppressionFaite { .. }
            | Evenement::LancerGlisser { .. }
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
                Commande::Capturer(mode) => {
                    log::info!("--capturer recu: {}", mode_journal(&mode));
                    if app.state::<Application>().demarree.load(Ordering::SeqCst) {
                        capturer_cli(app, &mode);
                    } else if let Ok(mut a) = app.state::<Application>().capture_en_attente.lock() {
                        log::info!("--capturer: en attente du demarrage");
                        *a = Some(mode);
                    }
                }
                // Désinstaller via une seconde instance n'aurait pas de sens : ignoré.
                Commande::Desinstaller | Commande::Demarrer => {
                    log::info!("seconde instance refusee")
                }
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
            (MENU_REGLAGES, _) => {
                log::info!("menu: reglages");
                fenetre_reglages::ouvrir(app);
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
        Evenement::CaptureCli(mode) => capturer_cli(app, &mode),
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
        Evenement::Vignette(signal) => traiter_vignette(app, signal),
        Evenement::ActionFaite(fin) => action_faite(app, *fin),
        Evenement::SuppressionFaite { message } => {
            afficher(app, message);
        }
        Evenement::LancerGlisser { id, chemin } => lancer_glisser(app, id, &chemin),
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

/// L'état de l'automate, `None` s'il est inaccessible.
fn etat_automate(app: &AppHandle) -> Option<Etat> {
    app.state::<Application>()
        .automate
        .lock()
        .ok()
        .map(|a| a.etat())
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
    let glisser = app.state::<Application>().glisser.load(Ordering::SeqCst);
    if !vignette::capture_acceptee(glisser) {
        log::info!("capture ignoree: glisser en cours");
        return;
    }
    if appliquer(app, Declencheur::DemandeCapture(mode)).is_none() {
        log::info!("capture deja en cours: appui ignore");
        return;
    }
    noter_demande(app);
    match mode {
        Mode::Zone => capturer_avec_selection(app, recu_a, Mode::Zone),
        Mode::Fenetre => capturer_avec_selection(app, recu_a, Mode::Fenetre),
        Mode::PleinEcran => capturer_plein_ecran(app, recu_a),
    }
}

/// Début commun à tous les modes, une fois en Gel : la vignette (suppression en attente
/// validée) et le bandeau sont cachés sans animation (ni l'un ni l'autre ne doit figurer
/// sur la photo ni rester au-dessus de l'écran figé), le compositeur attendu (borné), puis
/// la photo. Erreur → `GelImpossible` (Repos) et bandeau d'erreur ; rend `None`.
fn photographier(app: &AppHandle) -> Option<capture::Gel> {
    let vignette_cachee = fermer_vignette(app, "capture");
    let bandeau_cache = app.try_state::<Bandeau>().is_some_and(|b| b.cacher());
    if (bandeau_cache || vignette_cachee) && !systeme::attendre_composition(ATTENTE_COMPOSITION) {
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

/// La souris en pixels réels du bureau, lue tout de suite (`fenetres::souris` : le seul
/// repli de l'app).
fn souris_bureau(app: &AppHandle) -> Option<(i32, i32)> {
    zone::point_souris(fenetres::souris(app))
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
        Ok(pixels) => lancer_sortie(app, pixels, ecran.rect, duree_decoupe, recu_a, false),
        Err(e) => {
            log::warn!("decoupe impossible: {e}");
            appliquer(app, Declencheur::DecoupeImpossible);
            afficher_probleme(app, Probleme::Lecture);
        }
    }
}

/// Zone et fenêtre (§3.6), en Gel : photo, écran figé, (en mode fenêtre, la liste des
/// fenêtres), sélection, Échap réservé. Même code pour les deux modes. Toute erreur après
/// `montrer_ecran_fige` cache tout (garde) et revient au Repos.
fn capturer_avec_selection(app: &AppHandle, recu_a: Instant, mode: Mode) {
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
    // Mode fenêtre : la liste des fenêtres, après l'écran figé (elle coûte plus que la
    // photo) ; jamais demandée en mode zone.
    let (mode_selection, fenetres) = if mode == Mode::Fenetre {
        let debut = Instant::now();
        let liste = capture::fenetres();
        log::info!(
            "{}",
            zone::ligne_chrono_fenetres(debut.elapsed(), liste.len())
        );
        (
            selection::Mode::Fenetre,
            zone::fenetres_proposees(&liste, &gel_pris.ecrans),
        )
    } else {
        (selection::Mode::Zone, Vec::new())
    };
    appliquer(app, Declencheur::PhotoPrise(mode));
    // 5. La sélection, juste au-dessus de l'écran figé.
    let demande = selection::Demande {
        bureau: gel_pris.bureau,
        ecrans: gel_pris.ecrans.iter().map(|e| e.rect).collect(),
        dessous: gel::poignee_ecran_fige(),
        cote_min: capture::COTE_MIN,
        mode: mode_selection,
        fenetres: fenetres.clone(),
    };
    // La capture est notée avant `montrer` : un signal de fin arrivé pendant `montrer`
    // la trouve et la termine. La place est vide : vidée avant la photo, sur ce même fil,
    // et rien entre-temps ne range de capture.
    let ancienne = ranger_capture(
        app,
        Some(CaptureEnCours {
            gel: gel_pris,
            selection: None,
            mode: mode_selection,
            fenetres,
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
                mode: c.mode,
                fenetres: &c.fenetres,
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
        Geste::Decouper(zone) => decouper(app, zone, None),
        Geste::DecouperFenetre { zone, rayon, coins } => decouper(app, zone, Some((rayon, coins))),
    }
}

/// Zone ou fenêtre choisie : l'écran figé disparaît dans le même tour que la sélection,
/// puis la découpe (écran déjà libre, garde au repos) et la sortie dans son fil. En mode
/// fenêtre (`fenetre` : rayon et coins), les coins sont arrondis et la sortie garde la
/// transparence si un coin l'est.
fn decouper(app: &AppHandle, zone: systeme::Rect, fenetre: Option<(i32, [bool; 4])>) {
    let relache = Instant::now();
    let Some(mut capture) = ranger_capture(app, None) else {
        return;
    };
    capture.garde.cacher_tout();
    appliquer(app, Declencheur::ZoneChoisie);
    match fenetre {
        Some((rayon, coins)) => log::info!("{}", zone::ligne_fenetre_choisie(zone, rayon, coins)),
        None => log::info!(
            "zone choisie {},{} {}x{}",
            zone.x,
            zone.y,
            zone.largeur,
            zone.hauteur
        ),
    }
    let debut = Instant::now();
    let pixels = capture::decouper(&capture.gel, zone).map(|mut p| {
        if let Some((rayon, coins)) = fenetre.filter(|(rayon, _)| *rayon > 0) {
            capture::arrondir(&mut p, rayon, coins);
        }
        p
    });
    let duree_decoupe = debut.elapsed();
    drop(capture);
    let transparence = fenetre.is_some_and(|(rayon, coins)| zone::avec_transparence(rayon, coins));
    match pixels {
        Ok(pixels) => lancer_sortie(app, pixels, zone, duree_decoupe, relache, transparence),
        Err(e) => {
            log::warn!("decoupe impossible: {e}");
            appliquer(app, Declencheur::DecoupeImpossible);
            afficher_probleme(app, Probleme::Lecture);
        }
    }
}

/// Ce que la sortie doit faire, d'après les réglages en vigueur ; `transparence` :
/// garder l'alpha (fenêtre aux coins arrondis).
fn demande_sortie(app: &AppHandle, transparence: bool) -> sortie::Demande {
    let reglages = reglages_lus(app);
    let dossiers = dossiers_lus(app);
    let images = dossiers.as_ref().and_then(|d| d.images.clone());
    let (dossier_enregistrement, dossier_affiche) = zone::dossier_enregistrement(
        reglages.dossier_enregistrement.as_deref(),
        images.as_deref(),
    );
    let dossier_recents = dossiers
        .map(|d| d.local.join(zone::SOUS_DOSSIER_RECENTS))
        .unwrap_or_default();
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
        ranger_recents: true,
        modele_nom: reglages.modele_nom,
        maintenant: chrono::Local::now(),
        transparence,
    }
}

/// Les réglages lus au démarrage (ceux par défaut s'ils manquent).
fn reglages_lus(app: &AppHandle) -> Reglages {
    app.state::<Application>()
        .reglages
        .lock()
        .ok()
        .and_then(|r| r.clone())
        .unwrap_or_else(|| Reglages::par_defaut(plateforme_reglages(systeme::systeme())))
}

/// Les dossiers résolus au démarrage.
fn dossiers_lus(app: &AppHandle) -> Option<Dossiers> {
    app.state::<Application>()
        .dossiers
        .lock()
        .ok()
        .and_then(|d| d.clone())
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
/// bilan d'échec : le fil rend toujours un bilan. Avec `vignette`, l'aperçu est fait en
/// même temps, dans un fil à part.
fn sortir_et_mesurer(
    numero: u64,
    pixels: &Arc<gel::Pixels>,
    demande: sortie::Demande,
    duree_decoupe: Duration,
    relache: Instant,
    (capture, vignette): (systeme::Rect, bool),
) -> Box<FinSortie> {
    let debut = Instant::now();
    let (bilan, duree_sortie, apercu) = std::thread::scope(|portee| {
        let fil = vignette
            .then(|| {
                std::thread::Builder::new()
                    .name("apercu".into())
                    .spawn_scoped(portee, || faire_apercu(pixels))
                    .ok()
            })
            .flatten();
        let bilan = sortir_ou_echec(numero, pixels, &demande);
        let duree = debut.elapsed();
        let apercu = match fil {
            Some(f) => f.join().ok().flatten(),
            None if vignette => faire_apercu(pixels),
            None => None,
        };
        (bilan, duree, apercu)
    });
    Box::new(FinSortie {
        numero,
        bilan,
        demande,
        duree_decoupe,
        duree_sortie,
        relache,
        pixels: Arc::clone(pixels),
        capture,
        vignette,
        apercu,
    })
}

/// `sortie::sortir`, et un bilan d'échec si elle panique.
fn sortir_ou_echec(numero: u64, pixels: &gel::Pixels, demande: &sortie::Demande) -> sortie::Bilan {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        sortie::sortir(pixels, demande)
    }))
    .unwrap_or_else(|_| {
        log::error!("sortie {numero}: fil interrompu (panique)");
        zone::bilan_d_echec(demande, zone::CAUSE_SORTIE_INTERROMPUE)
    })
}

/// L'aperçu de la vignette : pixels réduits, en PNG.
fn faire_apercu(pixels: &gel::Pixels) -> Option<Apercu> {
    let petit = capture::reduire(pixels, vignette::APERCU_COTE_MAX);
    let png = sortie::encoder_png(&petit).ok()?;
    Some((Arc::new(png), (petit.largeur, petit.hauteur)))
}

/// La sortie dans un fil à part (le fil principal n'attend jamais le disque ni le
/// presse-papiers), puis le bilan revient au fil principal (`SortieFaite`). Un second
/// petit fil tient l'échéance (`zone::DELAI_SORTIE`) : sans nouvelles du fil de sortie à
/// temps, il poste `SortieEchue`, et l'automate ne reste jamais en Découpe.
fn lancer_sortie(
    app: &AppHandle,
    pixels: gel::Pixels,
    capture: systeme::Rect,
    duree_decoupe: Duration,
    relache: Instant,
    transparence: bool,
) {
    let etat = app.state::<Application>();
    let numero = etat.derniere_sortie.fetch_add(1, Ordering::SeqCst) + 1;
    *etat
        .inner()
        .sortie_attendue
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(numero);
    let demande = demande_sortie(app, transparence);
    let voulue = vignette::doit_montrer(
        reglages_lus(app).vignette.afficher,
        app.try_state::<Vignette>().is_some(),
    );
    let quoi = (capture, voulue);
    let pixels = Arc::new(pixels);
    let (fini, attente) = std::sync::mpsc::channel::<()>();
    let pour_fil = app.clone();
    let (pixels_fil, demande_fil) = (Arc::clone(&pixels), demande.clone());
    let lance = std::thread::Builder::new()
        .name("sortie".into())
        .spawn(move || {
            let fin = sortir_et_mesurer(
                numero,
                &pixels_fil,
                demande_fil,
                duree_decoupe,
                relache,
                quoi,
            );
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
        let fin = sortir_et_mesurer(numero, &pixels, demande, duree_decoupe, relache, quoi);
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

/// Bilan de la sortie (fil principal) : journal, puis la vignette (souffle d'abord) ou,
/// sans elle, retour au Repos et annonce au bandeau.
fn finir_sortie(app: &AppHandle, fin: FinSortie) {
    log::info!("{}", zone::ligne_sortie(&fin.bilan));
    if let Some(Err(cause)) = &fin.bilan.recent {
        log::warn!("recents: echec ({cause})");
    }
    let annonce = sortie::annonce(&fin.bilan, &fin.demande);
    log::info!("{}", zone::ligne_annonce(&annonce));
    if fin.vignette && fin.apercu.is_none() {
        log::warn!("vignette: apercu impossible");
    }
    let chrono =
        zone::ligne_chrono_sortie(fin.duree_decoupe, fin.duree_sortie, fin.relache.elapsed());
    let montrer = fin.vignette && app.try_state::<Vignette>().is_some();
    let declencheur = vignette::issue(montrer, fin.apercu.is_some());
    appliquer(app, declencheur);
    match (declencheur, fin.apercu.clone()) {
        (Declencheur::ImagePrete, Some(apercu)) => montrer_vignette(app, fin, annonce, apercu),
        _ => {
            afficher(app, zone::message_annonce(&annonce));
        }
    }
    log::info!("{chrono}");
}

/// Prépare la vignette de la capture `fin` ; le souffle partira quand la carte sera posée.
fn montrer_vignette(app: &AppHandle, fin: FinSortie, annonce: Annonce, apercu: Apercu) {
    if let Some(m) = vignette::message_bandeau(&annonce, true) {
        afficher(app, m);
    }
    let reglages = reglages_lus(app);
    let reduites =
        vignette::animations_reduites(reglages.animations, systeme::animations_reduites());
    let recent = match &fin.bilan.recent {
        Some(Ok(p)) => Some(p.clone()),
        _ => None,
    };
    let enregistre = match &fin.bilan.enregistrement {
        sortie::EtapeFichier::Fait(p) => Some(p.clone()),
        _ => None,
    };
    let demande = carte::Demande {
        id: fin.numero,
        capture: vignette::rect_fenetres(fin.capture),
        coin: vignette::coin(reglages.vignette.coin),
        actions: vignette::actions(),
        duree: vignette::duree_affichage(reglages.vignette.duree_s),
        duree_annulation: vignette::DELAI_ANNULATION,
        glisser: recent.is_some(),
        reduites,
        apercu: apercu.0,
        apercu_taille: apercu.1,
    };
    let ancienne = montree(app).replace(CaptureMontree {
        id: fin.numero,
        pixels: fin.pixels,
        transparence: fin.demande.transparence,
        maintenant: fin.demande.maintenant,
        recent,
        enregistre,
        liens: Vec::new(),
        suppression: EtatSuppression::Aucune,
        copie: None,
        enregistrement: None,
        glisser: false,
        reduites,
        partie: false,
        annonce,
        relache: fin.relache,
        souffle_a: None,
    });
    if let Some(a) = ancienne {
        // Jamais attendu (la photo ferme la vignette) : la suppression en attente est faite.
        quitter(app, a, false);
    }
    if let Some(v) = app.try_state::<Vignette>() {
        v.preparer(demande);
    }
}

/// La capture montrée, sous son verrou (repris s'il est empoisonné).
fn montree(app: &AppHandle) -> std::sync::MutexGuard<'_, Option<CaptureMontree>> {
    app.state::<Application>()
        .inner()
        .montree
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// `f` sur la capture montrée si c'est la carte `id`.
fn avec_montree<R>(
    app: &AppHandle,
    id: u64,
    f: impl FnOnce(&mut CaptureMontree) -> R,
) -> Option<R> {
    montree(app).as_mut().filter(|c| c.id == id).map(f)
}

/// Retire la capture montrée si c'est la carte `id`.
fn retirer_montree(app: &AppHandle, id: u64) -> Option<CaptureMontree> {
    montree(app).take_if(|c| c.id == id)
}

/// Un signal de la vignette (fil principal).
fn traiter_vignette(app: &AppHandle, signal: carte::Signal) {
    let Some(v) = app.try_state::<Vignette>() else {
        return;
    };
    match signal {
        carte::Signal::Prete { id } => {
            // Une page qui redit prête plus tard ne rejoue pas le souffle.
            let etat = etat_automate(app);
            let Some(reduites) = avec_montree(app, id, |c| {
                vignette::prete_acceptee(etat, c.partie).then(|| {
                    c.souffle_a = Some(Instant::now());
                    c.reduites
                })
            })
            .flatten() else {
                log::info!("vignette prete {id} ignoree");
                return;
            };
            if reduites {
                log::info!("souffle saute {id}: animations reduites");
            } else if v.jouer_souffle(id) {
                return;
            } else {
                log::info!("souffle saute {id}: fenetre absente");
            }
            avec_montree(app, id, |c| c.souffle_a = None);
            appliquer(app, Declencheur::SouffleTermine);
            v.entrer(id);
        }
        carte::Signal::SouffleFini { id, .. } => {
            if avec_montree(app, id, |_| ()).is_some() {
                appliquer(app, Declencheur::SouffleTermine);
                v.entrer(id);
            }
        }
        carte::Signal::Entree { id } => {
            if let Some((relache, souffle)) = avec_montree(app, id, |c| (c.relache, c.souffle_a)) {
                let souffle = souffle.map(|s| s.saturating_duration_since(relache));
                log::info!("{}", vignette::ligne_chrono(souffle, relache.elapsed()));
            }
        }
        carte::Signal::Action { id, action } => action_vignette(app, &v, id, &action),
        carte::Signal::Glisser { id } => demander_glisser(app, id),
        carte::Signal::DelaiEcoule { id } => {
            let Some((declencheur, raison, suppression)) = avec_montree(app, id, |c| {
                c.partie = true;
                let (d, raison, supprimer) = vignette::au_delai(c.suppression);
                if supprimer {
                    c.suppression = EtatSuppression::Lancee;
                }
                (d, raison, supprimer.then(|| vignette::a_supprimer(c)))
            }) else {
                return;
            };
            appliquer(app, declencheur);
            log::info!("vignette fermee {id} {raison}");
            v.partir(id, true);
            if let Some(s) = suppression {
                lancer_suppression(app, id, s, false);
            }
        }
        // Carte partie : les pixels et l'aperçu sont rendus.
        carte::Signal::Terminee { id } => {
            if let Some(c) = retirer_montree(app, id) {
                quitter(app, c, false);
            }
        }
        carte::Signal::Impossible { id, cause } => {
            log::warn!("vignette impossible {id}: {cause}");
            let Some(c) = retirer_montree(app, id) else {
                return;
            };
            let annonce = c.annonce.clone();
            quitter(app, c, false);
            v.cacher();
            appliquer(app, Declencheur::VignetteImpossible);
            // L'annonce que la vignette devait remplacer passe au bandeau.
            if vignette::message_bandeau(&annonce, true).is_none() {
                afficher(app, zone::message_annonce(&annonce));
            }
        }
    }
}

/// Un clic sur un bouton de la carte `id` (décision pure, puis gestes).
fn action_vignette(app: &AppHandle, v: &Vignette, id: u64, action: &str) {
    let nom: String = action
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .take(32)
        .collect();
    let Some(geste) = avec_montree(app, id, |c| vignette::decider(c, action, Instant::now()))
    else {
        return;
    };
    if geste == vignette::Geste::Ignorer {
        log::info!("vignette action {id} {nom} ignoree");
        return;
    }
    log::info!("vignette action {id} {nom}");
    match geste {
        vignette::Geste::Copier => lancer_action(app, id, true),
        vignette::Geste::Enregistrer => lancer_action(app, id, false),
        vignette::Geste::DejaEnregistree => {
            log::info!("vignette enregistrement {id} deja fait");
            v.confirmer(id, vignette::ENREGISTRER, true);
        }
        vignette::Geste::Supprimer => {
            avec_montree(app, id, |c| c.suppression = EtatSuppression::EnAttente);
            log::info!("vignette suppression {id} en attente");
            v.marquer_supprimee(id, true);
        }
        vignette::Geste::Annuler => {
            avec_montree(app, id, |c| c.suppression = EtatSuppression::Aucune);
            log::info!("vignette suppression {id} annulee");
            v.marquer_supprimee(id, false);
        }
        vignette::Geste::Fermer => {
            avec_montree(app, id, |c| c.partie = true);
            appliquer(app, Declencheur::VignetteFermee);
            log::info!("vignette fermee {id} fermer");
            v.partir(id, true);
        }
        vignette::Geste::Ignorer => {}
    }
}

/// Copier (`copier`) ou Enregistrer depuis la vignette : `sortir` dans un fil, sur les
/// pixels gardés, puis `ActionFaite`.
fn lancer_action(app: &AppHandle, id: u64, copier: bool) {
    let base = demande_sortie(app, false);
    let Some((pixels, demande)) = avec_montree(app, id, |c| {
        let maintenant = Some(Instant::now());
        if copier {
            c.copie = maintenant;
        } else {
            c.enregistrement = maintenant;
        }
        (
            Arc::clone(&c.pixels),
            vignette::demande_action(&base, copier, c),
        )
    }) else {
        return;
    };
    let action = if copier {
        vignette::COPIER
    } else {
        vignette::ENREGISTRER
    };
    let pour_fil = app.clone();
    let (pixels_fil, demande_fil) = (Arc::clone(&pixels), demande.clone());
    let lance = std::thread::Builder::new()
        .name("vignette-action".into())
        .spawn(move || {
            let bilan = sortir_ou_echec(id, &pixels_fil, &demande_fil);
            let fin = Box::new(FinAction {
                id,
                action: action.into(),
                bilan,
            });
            let pour_traiter = pour_fil.clone();
            let rendu = pour_fil
                .run_on_main_thread(move || traiter(&pour_traiter, Evenement::ActionFaite(fin)));
            if let Err(e) = rendu {
                log::warn!("resultat de l'action non rendu: {e}");
            }
        });
    if let Err(e) = lance {
        log::warn!("fil de l'action impossible ({e}): action sur le fil principal");
        let bilan = sortir_ou_echec(id, &pixels, &demande);
        action_faite(
            app,
            FinAction {
                id,
                action: action.into(),
                bilan,
            },
        );
    }
}

/// Résultat de Copier ou Enregistrer : journal, page, et l'erreur au bandeau. Arrivé
/// après le départ de la carte : journal seulement. Le fichier d'un Enregistrer arrivé
/// après l'effacement de sa capture est effacé à son tour (ARCHITECTURE §11).
fn action_faite(app: &AppHandle, fin: FinAction) {
    let (reussi, message, fichier) = vignette::apres_action(&fin.action, &fin.bilan);
    log::info!(
        "{}",
        vignette::ligne_action_faite(fin.id, &fin.action, reussi)
    );
    let copie = fin.action == vignette::COPIER;
    let etat = avec_montree(app, fin.id, |c| {
        if copie {
            c.copie = None;
        } else {
            c.enregistrement = None;
            if fichier.is_some() && c.suppression != EtatSuppression::Lancee {
                c.enregistre = fichier.clone();
            }
        }
        (c.suppression, !c.partie)
    });
    if !copie {
        let retenue = etat.is_none() && {
            let etat_app = app.state::<Application>();
            let mut retenues = etat_app.retenues.lock().unwrap_or_else(|e| e.into_inner());
            let avant = retenues.len();
            retenues.retain(|i| *i != fin.id);
            retenues.len() != avant
        };
        let tardif =
            vignette::enregistrement_tardif(etat.map(|e| e.0), retenue, fichier.as_deref());
        if let Some(s) = tardif {
            lancer_suppression(app, fin.id, s, true);
            return;
        }
    }
    if etat.map(|e| e.1) != Some(true) {
        return;
    }
    if let Some(v) = app.try_state::<Vignette>() {
        v.confirmer(fin.id, &fin.action, reussi);
    }
    if let Some(m) = message {
        afficher(app, m);
    }
}

/// Combien de temps l'arrêt attend les fils de suppression encore en cours.
const ATTENTE_SUPPRESSIONS: Duration = Duration::from_secs(2);

/// Efface les fichiers de la capture `id` dans un fil, qui écrit le bilan au journal
/// (`tardive` : le fichier d'un Enregistrer en retard), puis `SuppressionFaite` en cas
/// d'échec. Le fil est compté : l'arrêt l'attend.
fn lancer_suppression(app: &AppHandle, id: u64, suppression: sortie::Suppression, tardive: bool) {
    let (recents, glisser) = dossiers_vignette(app);
    let pour_fil = app.clone();
    let copie = suppression.clone();
    compter_suppression(app, true);
    let lance = std::thread::Builder::new()
        .name("suppression".into())
        .spawn(move || {
            let resultats = sortie::supprimer(&suppression, &recents, &glisser);
            let (ligne, message) =
                vignette::bilan_suppression(id, &suppression, &resultats, tardive);
            log::info!("{ligne}");
            compter_suppression(&pour_fil, false);
            let Some(message) = message else {
                return;
            };
            let pour_traiter = pour_fil.clone();
            let rendu = pour_fil.run_on_main_thread(move || {
                traiter(&pour_traiter, Evenement::SuppressionFaite { message })
            });
            if let Err(e) = rendu {
                log::warn!("suppression {id}: message non rendu: {e}");
            }
        });
    if let Err(e) = lance {
        compter_suppression(app, false);
        log::warn!("fil de suppression impossible ({e}): suppression sur le fil principal");
        supprimer_tout_de_suite(app, id, &copie, tardive);
    }
}

/// Un fil de suppression commence (`plus`) ou finit.
fn compter_suppression(app: &AppHandle, plus: bool) {
    let etat = app.state::<Application>();
    let (compte, signal) = &etat.suppressions;
    let mut n = compte.lock().unwrap_or_else(|e| e.into_inner());
    *n = if plus { *n + 1 } else { n.saturating_sub(1) };
    signal.notify_all();
}

/// Arrêt : attend (au plus `ATTENTE_SUPPRESSIONS`) les fils de suppression en cours.
fn attendre_suppressions(app: &AppHandle) {
    let etat = app.state::<Application>();
    let (compte, signal) = &etat.suppressions;
    let n = compte.lock().unwrap_or_else(|e| e.into_inner());
    let (n, _) = signal
        .wait_timeout_while(n, ATTENTE_SUPPRESSIONS, |n| *n > 0)
        .unwrap_or_else(|e| e.into_inner());
    if *n > 0 {
        log::warn!("suppression non attendue ({} fil(s) encore en cours)", *n);
    }
}

/// Efface sur ce fil (arrêt) et écrit le bilan au journal.
fn supprimer_tout_de_suite(
    app: &AppHandle,
    id: u64,
    suppression: &sortie::Suppression,
    tardive: bool,
) {
    let (recents, glisser) = dossiers_vignette(app);
    let resultats = sortie::supprimer(suppression, &recents, &glisser);
    let (ligne, _) = vignette::bilan_suppression(id, suppression, &resultats, tardive);
    log::info!("{ligne}");
}

/// La capture `c` quitte l'Application : sa suppression en attente est faite (sur ce fil
/// à l'arrêt), et elle est retenue si un Enregistrer tourne encore.
fn quitter(app: &AppHandle, mut c: CaptureMontree, arret: bool) {
    match vignette::suppression_au_depart(c.suppression, arret) {
        vignette::SuppressionAuDepart::Aucune => {}
        vignette::SuppressionAuDepart::DansUnFil => {
            c.suppression = EtatSuppression::Lancee;
            lancer_suppression(app, c.id, vignette::a_supprimer(&c), false);
        }
        vignette::SuppressionAuDepart::SurCeFil => {
            c.suppression = EtatSuppression::Lancee;
            supprimer_tout_de_suite(app, c.id, &vignette::a_supprimer(&c), false);
        }
    }
    if vignette::a_retenir(&c) {
        let etat = app.state::<Application>();
        let mut retenues = etat.retenues.lock().unwrap_or_else(|e| e.into_inner());
        vignette::retenir(&mut retenues, c.id);
    }
}

/// `<local>/Recents` et `<local>/Glisser` ; vides sans dossier local (rien n'est alors
/// effacé ni posé).
fn dossiers_vignette(app: &AppHandle) -> (PathBuf, PathBuf) {
    match dossiers_lus(app) {
        Some(d) => (
            d.local.join(zone::SOUS_DOSSIER_RECENTS),
            d.local.join(vignette::SOUS_DOSSIER_GLISSER),
        ),
        None => (PathBuf::new(), PathBuf::new()),
    }
}

/// Retire la vignette sans animation (nouvelle capture, arrêt) : une suppression en
/// attente est faite tout de suite (sur ce fil à l'arrêt). Rend vrai si une carte était là.
/// La raison de fermeture de l'arrêt de l'app.
const RAISON_ARRET: &str = "arret";

fn fermer_vignette(app: &AppHandle, raison: &str) -> bool {
    let capture = montree(app).take();
    let cachee = app.try_state::<Vignette>().is_some_and(|v| v.cacher());
    if let Some(c) = capture {
        if !c.partie {
            log::info!("vignette fermee {} {raison}", c.id);
        }
        quitter(app, c, raison == RAISON_ARRET);
    }
    cachee
}

/// Appui sur l'image puis déplacement : le fichier à donner est posé, puis le glisser est
/// reposté (jamais lancé dans le gestionnaire de la page).
fn demander_glisser(app: &AppHandle, id: u64) {
    let Some(prete) = avec_montree(app, id, |c| {
        vignette::glisser_possible(c).map(|r| (r.to_path_buf(), c.maintenant))
    }) else {
        return;
    };
    let glisser = app.state::<Application>().glisser.load(Ordering::SeqCst);
    let Some((recent, maintenant)) = prete.filter(|_| !glisser) else {
        log::info!("vignette glisser {id} indisponible");
        return;
    };
    let chemin = match dossiers_lus(app) {
        Some(d) => {
            let nom = sortie::nom_fichier(&reglages_lus(app).modele_nom, maintenant);
            let dossier = d.local.join(vignette::SOUS_DOSSIER_GLISSER);
            sortie::fichier_a_glisser(&recent, &dossier, &nom)
        }
        None => Ok(recent),
    };
    match chemin {
        Ok(chemin) => {
            avec_montree(app, id, |c| c.liens.push(chemin.clone()));
            poster_plus_tard(app, Evenement::LancerGlisser { id, chemin });
        }
        Err(e) => log::info!("vignette glisser {id} refuse ({e})"),
    }
}

/// Les fenêtres de BreezyShot sur lesquelles un dépôt ne compte pas.
fn nos_fenetres(app: &AppHandle) -> Vec<isize> {
    let mut liste: Vec<isize> = app
        .try_state::<Vignette>()
        .map(|v| v.identifiants().into_iter().flatten().collect())
        .unwrap_or_default();
    liste.extend(app.try_state::<Bandeau>().and_then(|b| b.identifiant()));
    liste
}

/// Le glisser lui-même (fil principal, bloquant : boucle du système). Minuterie en pause
/// pendant toute la boucle ; un dépôt ailleurs que sur BreezyShot ferme la vignette.
fn lancer_glisser(app: &AppHandle, id: u64, chemin: &std::path::Path) {
    let Some(v) = app.try_state::<Vignette>() else {
        return;
    };
    let Some(possible) = avec_montree(app, id, |c| {
        let possible = vignette::glisser_possible(c).is_some();
        c.glisser = possible;
        possible
    }) else {
        return;
    };
    let pret = vignette::glisser_pret(
        possible,
        v.identifiants()[0],
        systeme::glisser::bouton_principal_enfonce(),
    );
    let fenetre = match pret {
        Ok(f) => f,
        Err(raison) => {
            avec_montree(app, id, |c| c.glisser = false);
            log::info!("vignette glisser {id} {raison}");
            return;
        }
    };
    let etat = app.state::<Application>();
    etat.glisser.store(true, Ordering::SeqCst);
    v.glisser_en_cours(id, true);
    log::info!("vignette glisser {id} debut");
    let resultat = systeme::glisser::glisser_fichier(fenetre, chemin);
    etat.glisser.store(false, Ordering::SeqCst);
    let sous = souris_bureau(app).and_then(systeme::glisser::fenetre_sous);
    let a_nous = sous.is_some_and(|h| nos_fenetres(app).contains(&h));
    let issue = vignette::depot(&resultat, a_nous);
    avec_montree(app, id, |c| c.glisser = false);
    v.glisser_en_cours(id, false);
    log::info!("{}", vignette::ligne_glisser(id, &issue));
    if let Some(d) = issue.declencheur() {
        avec_montree(app, id, |c| c.partie = true);
        appliquer(app, d);
        log::info!("vignette fermee {id} glisser");
        v.partir(id, true);
    }
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
    // Vignette : suppression en attente faite, fenêtres cachées puis détruites.
    fermer_vignette(app, RAISON_ARRET);
    attendre_suppressions(app);
    if let Some(v) = app.try_state::<Vignette>() {
        v.fermer();
    }
    // Réglages fermés d'abord : la fin d'une saisie ne doit pas reprendre des raccourcis
    // après leur libération. Puis libérer les raccourcis, fermer les autres fenêtres.
    fenetres::reglages::fermer(app);
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

    #[test]
    fn desinstallation_ligne_du_demarrage() {
        use systeme::EtatDemarrage as E;
        let retire = "desinstallation demarrage: retire";
        let absent = "desinstallation demarrage: absent";
        assert_eq!(ligne_demarrage_retire(E::Actif), retire);
        assert_eq!(ligne_demarrage_retire(E::DesactiveParWindows), retire);
        assert_eq!(ligne_demarrage_retire(E::Inactif), absent);
        assert_eq!(ligne_demarrage_retire(E::Indisponible), absent);
    }
    use crate::raccourcis::{choisir, decider_annonce, Situation};

    fn action(id: &str, texte: &str) -> EntreeMenu {
        EntreeMenu::Action {
            id: id.into(),
            texte: texte.into(),
        }
    }

    #[test]
    fn menu_avec_les_trois_raccourcis() {
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
                action("capturer_fenetre", "Capturer une fenêtre\tCtrl + Maj + 3"),
                EntreeMenu::Separateur,
                action(MENU_REGLAGES, "Réglages"),
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
                action("capturer_fenetre", "Capturer une fenêtre"),
                EntreeMenu::Separateur,
                action(MENU_REGLAGES, "Réglages"),
                EntreeMenu::Separateur,
                action(MENU_QUITTER, "Quitter BreezyShot"),
            ]
        );
        // Un seul raccourci retenu : les autres entrées restent, sans raccourci.
        let m = entrees_menu(|mode| (mode == Mode::Zone).then(|| "Impr. écran".to_string()));
        assert_eq!(
            m[0],
            action("capturer_zone", "Capturer une zone\tImpr. écran")
        );
        assert_eq!(m[1], action("capturer_plein_ecran", "Capturer l'écran"));
        assert_eq!(m[2], action("capturer_fenetre", "Capturer une fenêtre"));
    }

    #[test]
    fn identifiants_du_menu() {
        for mode in Mode::TOUS {
            assert_eq!(mode_du_menu(&id_menu(mode)), Some(mode));
        }
        assert_eq!(mode_du_menu(MENU_QUITTER), None);
        assert_eq!(mode_du_menu("capturer_autre"), None);
        assert_eq!(
            modes_au_menu(),
            [Mode::Zone, Mode::PleinEcran, Mode::Fenetre]
        );
        assert_eq!(texte_capturer(Mode::Fenetre, None), "Capturer une fenêtre");
    }

    #[test]
    fn actions_construites() {
        assert_eq!(ACTIONS_CONSTRUITES, ["zone", "plein_ecran", "fenetre"]);
        for a in ACTIONS_CONSTRUITES {
            assert!(Mode::depuis_nom(a).is_some(), "{a}");
        }
    }

    #[test]
    fn seules_les_actions_construites_sont_proposees() {
        let r = Reglages::par_defaut(Plateforme::Windows);
        let t = table_candidats(&r);
        assert_eq!(t.len(), 3);
        assert_eq!(t["zone"], ["ImprEcran", "Ctrl+Maj+1"]);
        assert_eq!(t["plein_ecran"], ["Ctrl+Maj+2"]);
        assert_eq!(t["fenetre"], ["Ctrl+Maj+3"]);
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
