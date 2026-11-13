//! L'Application : le chef d'orchestre. Seul module qui connaît les autres.
//!
//! Pour brancher un module (étapes suivantes) :
//! 1. le créer dans `demarrer`, à l'endroit marqué « Modules », en lui passant un rappel
//!    qui transforme son signal en `Evenement` : `move |s| traiter(&app, Evenement::X(s))` ;
//! 2. ajouter la variante `Evenement::X(module::Signal)` et son cas dans `traiter` ;
//! 3. libérer ce qu'il tient dans `arreter` (raccourcis, fenêtres) ;
//! 4. ajouter ses entrées de menu dans `entrees_menu`.
//!
//! 0.5 : l'historique (`geste_historique`, `fenetre_historique`, décisions dans
//! `historique.rs`) et Copier le texte (`geste_texte`, décisions dans `texte.rs`) ne
//! touchent ce fichier que par leurs points d'accroche. Une fonction Pro n'est construite
//! que si `droits::ouverte` le dit, ici seulement (`actions_construites`), d'après la
//! licence en vigueur (`licence.rs`, `Evenement::LicenceChangee`).
//!
//! Retardateur (0.4) : `demarrer_capture` entre en Décompte (`commencer_decompte`), le fil
//! `decompte` poste `DecompteFini`, puis la séquence du mode part (`apres_gel`) ;
//! `decompte.rs` décide (fonctions pures). Dernière zone : `capturer_derniere_zone`.
//!
//! Capture d'une zone (§3.6) : `lancer_capture` puis `traiter_zone` font les gestes,
//! `zone.rs` dit lesquels (fonctions pures). Plein écran (§3.7) : `lancer_capture` puis
//! `capturer_plein_ecran`, sans écran figé ni sélection. Tout ce qui montre ou cache se
//! fait sur le fil principal ; la sortie (disque, presse-papiers) part dans un fil à part.

pub mod arguments;
pub mod decompte;
pub mod editeur;
pub mod etat;
pub mod etats;
pub mod fenetre_editeur;
pub mod fenetre_historique;
pub mod fenetre_reglages;
pub mod geste_historique;
pub mod geste_texte;
pub mod historique;
pub mod icone;
pub mod installateur;
pub mod journal;
pub mod licence;
pub mod page_reglages;
pub mod texte;
pub mod vignette;
pub mod zone;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, RunEvent};

use crate::capture;
use crate::droits::{self, Fonction};
use crate::fenetres::selection::{self, Selection};
use crate::fenetres::vignette::{self as carte, Vignette};
use crate::fenetres::{self, Bandeau, Genre, Message};
use crate::raccourcis::{self, Bilan, Decision};
use crate::reglages::{self, Constat, Plateforme, Reglages};
use crate::sortie::{self, Annonce};
use crate::systeme::gel::{self, Changement};
use crate::systeme::{self, Dossiers, Systeme, Theme};
use arguments::Commande;
use decompte::{ArriveeDecompte, Cause};
use etats::{Automate, Declencheur, Etat, Mode};
use icone::EntreeMenu;
use vignette::{CaptureMontree, EtatSuppression};
use zone::{Arrivee, DerniereZone, Geste, Probleme, Refaire, Vue};

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

/// Vrai quand l'éditeur d'annotation est construit (chantier A, 0.4) : la vignette ne
/// montre Annoter qu'à partir de là (seulement ce qui marche).
pub const EDITEUR_CONSTRUIT: bool = true;

/// Les actions de capture de la 0.4, toujours construites.
pub const ACTIONS_DE_CAPTURE: [&str; 4] = ["zone", "plein_ecran", "fenetre", "derniere_zone"];

/// Nom d'action du raccourci de l'historique, et identifiant de son entrée de menu.
pub const ACTION_HISTORIQUE: &str = "historique";
pub const MENU_HISTORIQUE: &str = "historique";

/// Les actions construites : seules celles-ci reçoivent un raccourci (§5), une entrée au
/// menu, une ligne dans Réglages › Raccourcis (et, pour `texte`, le bouton de l'éditeur).
/// `historique` si l'historique est construit et ouvert (`droits`) ; `texte` si Copier le
/// texte est construit, ouvert, et si une langue de lecture existe (R10).
pub fn actions_construites(pro: bool, texte_disponible: bool) -> Vec<&'static str> {
    actions_selon(
        pro,
        historique::CONSTRUIT,
        texte::CONSTRUIT && texte_disponible,
    )
}

/// Règle pure de `actions_construites` : `avec_historique`, `avec_texte` disent ce qui est
/// construit (et la langue), `pro` qu'une licence valide est en place ; les droits sont lus
/// ici et nulle part ailleurs.
pub fn actions_selon(pro: bool, avec_historique: bool, avec_texte: bool) -> Vec<&'static str> {
    let mut actions = ACTIONS_DE_CAPTURE.to_vec();
    if avec_historique && droits::ouverte(Fonction::Historique, pro) {
        actions.push(ACTION_HISTORIQUE);
    }
    if avec_texte && droits::ouverte(Fonction::Texte, pro) {
        actions.push(Mode::Texte.nom());
    }
    actions
}

/// L'ordre des entrées de capture dans le menu de l'icône (celui des raccourcis 1 à 4,
/// puis Copier le texte). Seuls les modes construits y paraissent.
pub const ORDRE_MENU: [Mode; 5] = [
    Mode::Zone,
    Mode::PleinEcran,
    Mode::Fenetre,
    Mode::DerniereZone,
    Mode::Texte,
];

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
    /// Le délai du décompte numéro `n` est écoulé (posté par le fil `decompte`).
    DecompteFini(u64),
    /// La fenêtre de l'éditeur est détruite (ramené sur le fil principal).
    EditeurDetruit,
    /// Croix du cadre, Alt+F4, menu système : la fermeture a été empêchée.
    EditeurFermetureDemandee,
    /// Un export de l'éditeur est fini (rendu par le fil `editeur-sortie`).
    EditeurExportFait(Box<editeur::FinExportEditeur>),
    /// L'échéance de l'export numéro `n` de l'éditeur est tombée.
    EditeurExportEchu(u64),
    /// Le glisser du document `id`, reposté hors du gestionnaire de la page.
    EditeurGlisser { id: u64, chemin: PathBuf },
    /// Signal de l'historique (0.5), traité par `geste_historique::traiter`.
    Historique(geste_historique::Signal),
    /// Une lecture de texte est finie (rendue par le fil `texte`).
    TexteLu(Box<texte::FinLecture>),
    /// L'échéance de la lecture numéro `n` est tombée.
    TexteEchu(u64),
    /// Résultat de la recherche des langues de lecture, au démarrage.
    TexteDisponible(bool),
    /// La licence a été activée ou retirée (`licence.rs`) : listes refaites.
    LicenceChangee,
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

/// Le décompte du retardateur en cours (état Décompte), de la demande à sa fin ou à son
/// annulation.
struct DecompteEnCours {
    numero: u64,
    mode: Mode,
    debut: Instant,
    /// Jeté à l'annulation : le fil `decompte` se réveille et finit sans rien poster.
    arret: Sender<()>,
    /// L'`id` du décompte dans le bandeau ; `None` : sans affichage.
    affichage: Option<u64>,
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
    /// L'annonce du démarrage, différée jusqu'au raccourci du texte (`lacher_annonce`).
    annonce_differee: Mutex<Option<(PathBuf, Bilan)>>,
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
    /// Le décompte en cours (fil principal ; verrou jamais tenu pendant un appel à un
    /// module), et le dernier numéro donné.
    decompte: Mutex<Option<DecompteEnCours>>,
    numero_decompte: AtomicU64,
    /// La dernière zone capturée, en mémoire seulement (§5 du contrat de la 0.4).
    derniere_zone: Mutex<Option<DerniereZone>>,
    /// L'éditeur ouvert (fil principal ; verrou jamais tenu pendant un appel à Tauri ou à
    /// un module).
    editeur: Mutex<Option<editeur::EtatEditeur>>,
    /// Préférences d'outil de la page de l'éditeur, pour la session (R9) : jamais écrites.
    preferences_editeur: Mutex<Option<String>>,
    /// Derniers numéros donnés : export de l'éditeur, fenêtre de l'éditeur, demande de
    /// fermeture.
    numero_export: AtomicU64,
    numero_fenetre_editeur: AtomicU64,
    numero_fermeture: AtomicU64,
    /// Numéro de la fenêtre d'éditeur vivante (0 : aucune) : son `Destroyed` compte une
    /// fois.
    fenetre_editeur_vivante: AtomicU64,
    /// Capture à ouvrir dès que la fenêtre précédente est détruite.
    editeur_suivant: Mutex<Option<(editeur::Document, Instant)>>,
    /// Le filet de la page de l'éditeur : fermé à `editeur_pret`, il finit son fil.
    filet_page: Mutex<Option<Sender<()>>>,
    /// L'historique ouvert (fil principal ; `None` : fenêtre fermée).
    historique: Mutex<Option<historique::EtatHistorique>>,
    /// La lecture de texte en cours (état Lecture).
    lecture: Mutex<Option<texte::LectureEnCours>>,
    /// Une langue de lecture existe (`Evenement::TexteDisponible`) ; faux jusque-là.
    texte_disponible: AtomicBool,
    /// La licence en vigueur (`licence.rs`) ; `None` : pas de Pro.
    licence: Mutex<Option<crate::licence::Licence>>,
    /// Les actions construites que tiennent le menu et les raccourcis (`suivre_les_actions`).
    actions_suivies: Mutex<Vec<&'static str>>,
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
            carte::souffle_termine,
            fenetre_editeur::editeur_pret,
            fenetre_editeur::editeur_image,
            fenetre_editeur::editeur_affiche,
            fenetre_editeur::editeur_vue,
            fenetre_editeur::editeur_etat,
            fenetre_editeur::editeur_exporter,
            fenetre_editeur::editeur_fermer,
            fenetre_editeur::editeur_garder,
            fenetre_editeur::editeur_copier_texte,
            fenetre_historique::historique_pret,
            fenetre_historique::historique_apercu,
            fenetre_historique::historique_action,
            fenetre_historique::historique_annuler,
            fenetre_historique::historique_tout_effacer,
            fenetre_historique::historique_glisser,
            fenetre_historique::historique_affiche,
            fenetre_historique::historique_fermer,
            licence::licence_etat,
            licence::licence_activer,
            licence::licence_retirer
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
    // La licence d'abord : les actions Pro en dépendent. Les langues de lecture ne sont pas
    // encore connues : `texte` attend `Evenement::TexteDisponible`.
    licence::charger(app);
    let pro = pro(app);
    if let Ok(mut suivies) = app.state::<Application>().actions_suivies.lock() {
        *suivies = actions_construites(pro, false);
    }
    let candidats = table_candidats(&reglages, &actions_construites(pro, false));
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
        &entrees_menu(&actions_construites(pro, false), |a| bilan.libelle_menu(a)),
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
    // tourner une boucle imbriquée, et un message montré là n'a jamais reçu sa zone. Avec
    // la licence, elle attend le raccourci du texte (`TexteDisponible`), au plus
    // `ATTENTE_ANNONCE_TEXTE`.
    let differee = annonce_attend_texte(pro) && dossiers.is_some();
    if let (true, Some(d)) = (differee, &dossiers) {
        if let Ok(mut a) = app.state::<Application>().annonce_differee.lock() {
            *a = Some((d.local.clone(), bilan.clone()));
        }
        borner_annonce(app);
    }

    // Langues de lecture (Copier le texte), dans un petit fil : `pret` n'attend pas.
    geste_texte::lancer_sonde(app);

    if let (false, Some(d)) = (differee, &dossiers) {
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
        Some(m) if mode_construit(app, m) => lancer_capture(app, m),
        Some(_) => log::warn!("--capturer: mode non construit"),
        None => log::warn!("--capturer: mode inconnu"),
    }
}

/// L'historique ouvert, sous son verrou (repris s'il est empoisonné). Fil principal ;
/// verrou jamais tenu pendant un appel à Tauri ou à un module.
pub(super) fn historique_ouvert(
    app: &AppHandle,
) -> std::sync::MutexGuard<'_, Option<historique::EtatHistorique>> {
    app.state::<Application>()
        .inner()
        .historique
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// La lecture de texte en cours, sous son verrou (repris s'il est empoisonné).
pub(super) fn lecture_en_cours(
    app: &AppHandle,
) -> std::sync::MutexGuard<'_, Option<texte::LectureEnCours>> {
    app.state::<Application>()
        .inner()
        .lecture
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Vrai si une langue de lecture existe (`Evenement::TexteDisponible`).
fn texte_disponible(app: &AppHandle) -> bool {
    app.state::<Application>()
        .texte_disponible
        .load(Ordering::SeqCst)
}

/// Vrai si une licence valide est en place.
pub(super) fn pro(app: &AppHandle) -> bool {
    licence_en_vigueur(app).is_some()
}

/// La licence en vigueur, sous son verrou (repris s'il est empoisonné).
pub(super) fn licence_en_vigueur(
    app: &AppHandle,
) -> std::sync::MutexGuard<'_, Option<crate::licence::Licence>> {
    app.state::<Application>()
        .inner()
        .licence
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Les actions construites en ce moment (licence et langues de lecture comprises).
pub(super) fn actions_en_vigueur(app: &AppHandle) -> Vec<&'static str> {
    actions_construites(pro(app), texte_disponible(app))
}

/// Vrai si le mode `mode` est une action construite en ce moment.
fn mode_construit(app: &AppHandle, mode: Mode) -> bool {
    actions_en_vigueur(app).contains(&mode.nom())
}

/// Vrai si Copier le texte est construit en ce moment : le bouton « Texte » de l'éditeur
/// découle de la même liste que le menu et les raccourcis (avenant 0.5, §2.2).
pub(super) fn texte_construit(app: &AppHandle) -> bool {
    mode_construit(app, Mode::Texte)
}

/// Les entrées du menu en ce moment : actions construites, raccourcis tenus.
pub(super) fn menu_actuel(app: &AppHandle) -> Option<Vec<EntreeMenu>> {
    let bilan = app.state::<Application>().reservations.get()?.bilan();
    Some(entrees_menu(&actions_en_vigueur(app), |a| {
        bilan.libelle_menu(a)
    }))
}

/// Refait le menu de l'icône d'après `menu_actuel`.
pub(super) fn refaire_menu(app: &AppHandle) {
    if let Some(entrees) = menu_actuel(app) {
        if let Err(e) = icone::changer_menu(app, &entrees) {
            log::warn!("menu non refait: {e}");
        }
    }
}

/// Résultat de la recherche des langues de lecture : retenu ; si Copier le texte devient
/// une action construite, son raccourci est réservé (`raccourci texte: …`) et le menu
/// refait. Pas de surveillance ensuite : une langue installée compte au prochain démarrage.
fn texte_devenu_disponible(app: &AppHandle, disponible: bool) {
    app.state::<Application>()
        .texte_disponible
        .store(disponible, Ordering::SeqCst);
    let apparues = suivre_les_actions(app);
    lacher_annonce(app, &apparues);
}

/// Range la licence en vigueur (`licence.rs`, après vérification et rangement), puis poste
/// `Evenement::LicenceChangee`, qui refait les listes au tour suivant.
pub(super) fn poser_licence(app: &AppHandle, licence: Option<crate::licence::Licence>) {
    *licence_en_vigueur(app) = licence;
    poster_plus_tard(app, Evenement::LicenceChangee);
}

/// Les actions construites ont pu changer (langues, licence) depuis la liste que tiennent
/// le menu et les raccourcis : une action apparue reçoit son raccourci (`raccourci
/// <action>: …`), une action partie rend le sien, puis le menu est refait. Rien si la
/// liste est la même.
fn suivre_les_actions(app: &AppHandle) -> Vec<raccourcis::ChoixAction> {
    let mut apparues = Vec::new();
    let apres = actions_en_vigueur(app);
    let avant = std::mem::replace(
        &mut *app
            .state::<Application>()
            .actions_suivies
            .lock()
            .unwrap_or_else(|e| e.into_inner()),
        apres.clone(),
    );
    if apres == avant {
        return apparues;
    }
    if let Some(r) = app.state::<Application>().reservations.get() {
        for action in apres.iter().filter(|a| !avant.contains(a)) {
            let candidats = reglages_lus(app)
                .raccourcis
                .candidats(action)
                .unwrap_or_default()
                .to_vec();
            let choix = r.changer(app, action, &candidats, systeme::impr_ecran_reserve());
            match &choix.retenu {
                Some(x) => log::info!("raccourci {action}: {}", x.canonique()),
                None => log::info!("raccourci {action}: aucun"),
            }
            if *action == Mode::Texte.nom() {
                geste_texte::raccourci_reserve(app, &choix);
            }
            apparues.push(choix);
        }
        for action in avant.iter().filter(|a| !apres.contains(a)) {
            // Liste vide : le raccourci est rendu au système.
            r.changer(app, action, &[], systeme::impr_ecran_reserve());
            log::info!("raccourci {action} rendu");
        }
    }
    refaire_menu(app);
    // Les lignes de Réglages › Raccourcis suivent tout de suite (licence, langues).
    fenetre_reglages::envoyer_vue(app);
    apparues
}

/// Le bilan du démarrage attend-il le texte ? Oui quand Copier le texte serait construit
/// si une langue existe (licence en place) : son raccourci n'est connu qu'à
/// `Evenement::TexteDisponible`.
pub fn annonce_attend_texte(pro: bool) -> bool {
    actions_construites(pro, true).contains(&Mode::Texte.nom())
}

/// Le bilan du démarrage complété des actions réservées après lui (`texte`) : la ligne de
/// chaque action est remplacée, ou ajoutée. Une seule règle d'annonce ensuite
/// (`raccourcis::decider_annonce`).
pub fn bilan_avec(mut bilan: Bilan, apparues: &[raccourcis::ChoixAction]) -> Bilan {
    for choix in apparues {
        match bilan.actions.iter_mut().find(|a| a.action == choix.action) {
            Some(a) => *a = choix.clone(),
            None => bilan.actions.push(choix.clone()),
        }
    }
    bilan
}

/// L'annonce du démarrage différée (le bilan attend le texte) part, complétée de
/// `apparues` ; rien si elle est déjà partie.
fn lacher_annonce(app: &AppHandle, apparues: &[raccourcis::ChoixAction]) {
    let differee = app
        .state::<Application>()
        .annonce_differee
        .lock()
        .ok()
        .and_then(|mut a| a.take());
    if let Some((local, bilan)) = differee {
        poster_plus_tard(
            app,
            Evenement::AnnonceDemarrage {
                local,
                bilan: bilan_avec(bilan, apparues),
            },
        );
    }
}

/// Borne de l'attente du texte par l'annonce du démarrage : sans réponse des langues de
/// lecture, le bilan part sans lui.
pub const ATTENTE_ANNONCE_TEXTE: Duration = Duration::from_secs(3);

/// Arme la borne de l'annonce différée (fil `annonce-attente`).
fn borner_annonce(app: &AppHandle) {
    let pour = app.clone();
    let lance = std::thread::Builder::new()
        .name("annonce-attente".into())
        .spawn(move || {
            std::thread::sleep(ATTENTE_ANNONCE_TEXTE);
            let p = pour.clone();
            let _ = pour.run_on_main_thread(move || {
                let attend = p
                    .state::<Application>()
                    .annonce_differee
                    .lock()
                    .is_ok_and(|a| a.is_some());
                if attend {
                    log::info!("annonce demarrage: langues non recues");
                    lacher_annonce(&p, &[]);
                }
            });
        });
    if let Err(e) = lance {
        log::warn!("attente de l'annonce impossible ({e}): annonce sans le texte");
        lacher_annonce(app, &[]);
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
/// construites (`actions`).
pub fn table_candidats(reglages: &Reglages, actions: &[&str]) -> BTreeMap<String, Vec<String>> {
    actions
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
                duree_ms: None,
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
pub(super) fn afficher(app: &AppHandle, message: Message) -> Option<u64> {
    match app.try_state::<Bandeau>() {
        Some(bandeau) => Some(bandeau.afficher(message)),
        None => {
            log::warn!("message non affiche (pas de bandeau): {}", message.titre);
            None
        }
    }
}

/// Les modes de capture au menu, dans l'ordre du menu : ceux qui sont construits.
pub fn modes_au_menu(actions: &[&str]) -> Vec<Mode> {
    ORDRE_MENU
        .into_iter()
        .filter(|m| actions.contains(&m.nom()))
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
        Mode::DerniereZone => "Refaire la dernière zone",
        Mode::Texte => "Copier le texte",
    }
}

/// Texte de l'entrée d'un mode, avec le raccourci actif à droite (tabulation : aligné à
/// droite par les menus Windows) ; sans raccourci retenu, le libellé seul.
pub fn texte_capturer(mode: Mode, raccourci: Option<&str>) -> String {
    avec_raccourci(libelle_menu(mode), raccourci)
}

/// Un libellé de menu, avec le raccourci après une tabulation s'il y en a un.
fn avec_raccourci(libelle: &str, raccourci: Option<&str>) -> String {
    match raccourci {
        Some(r) => format!("{libelle}\t{r}"),
        None => libelle.into(),
    }
}

/// Les entrées du menu de l'icône, dans l'ordre : les modes construits parmi `actions`
/// (`raccourci` donne le raccourci actif d'une action), un séparateur, « Historique » et un
/// séparateur si l'historique est construit, « Réglages », un séparateur, « Quitter
/// BreezyShot ». Seulement ce qui marche.
pub fn entrees_menu(
    actions: &[&str],
    raccourci: impl Fn(&str) -> Option<String>,
) -> Vec<EntreeMenu> {
    let mut entrees: Vec<EntreeMenu> = modes_au_menu(actions)
        .into_iter()
        .map(|mode| EntreeMenu::Action {
            id: id_menu(mode),
            texte: texte_capturer(mode, raccourci(mode.nom()).as_deref()),
        })
        .collect();
    entrees.push(EntreeMenu::Separateur);
    if actions.contains(&ACTION_HISTORIQUE) {
        entrees.push(EntreeMenu::Action {
            id: MENU_HISTORIQUE.into(),
            texte: avec_raccourci("Historique", raccourci(ACTION_HISTORIQUE).as_deref()),
        });
        entrees.push(EntreeMenu::Separateur);
    }
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
            | Evenement::DecompteFini(_)
            | Evenement::EditeurDetruit
            | Evenement::EditeurFermetureDemandee
            | Evenement::EditeurExportFait(_)
            | Evenement::EditeurExportEchu(_)
            | Evenement::EditeurGlisser { .. }
            | Evenement::Historique(_)
            | Evenement::TexteLu(_)
            | Evenement::TexteEchu(_)
            | Evenement::TexteDisponible(_)
            | Evenement::LicenceChangee
            | Evenement::Fenetres(fenetres::Signal::ActionBandeau { .. })
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
            // Sans `ATTENTE_MENU` : rien n'est photographié.
            (MENU_HISTORIQUE, _) if actions_en_vigueur(app).contains(&ACTION_HISTORIQUE) => {
                log::info!("menu: historique");
                geste_historique::ouvrir(app);
            }
            (_, Some(mode)) if mode_construit(app, mode) => {
                log::info!("menu: capturer {}", mode.nom());
                lancer_capture_apres(app, mode, ATTENTE_MENU);
            }
            (autre, _) => log::warn!("menu: entree inconnue {autre}"),
        },
        Evenement::Raccourci(raccourcis::Signal::Echap) => {
            log::info!("echap global presse");
            if !annuler_si_decompte(app, ArriveeDecompte::Echap) {
                traiter_zone(app, Arrivee::Echap);
            }
        }
        Evenement::Selection(signal) => traiter_zone(app, Arrivee::Selection(&signal)),
        Evenement::Bureau(changement) => {
            log::info!("bureau change pendant la capture: {changement:?}");
            if geste_texte::abandonner_lecture(app) {
                return;
            }
            if !annuler_si_decompte(app, ArriveeDecompte::Bureau(changement)) {
                traiter_zone(app, Arrivee::Bureau(changement));
            }
        }
        Evenement::DecompteFini(numero) => finir_decompte(app, numero),
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
            let construite = actions_en_vigueur(app).contains(&action.as_str());
            match Mode::depuis_nom(&action) {
                Some(mode) if construite => lancer_capture(app, mode),
                None if construite && action == ACTION_HISTORIQUE => geste_historique::ouvrir(app),
                _ => log::warn!("raccourci: action inconnue {action}"),
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
            // La page ferme un message elle-même ; seule la croix du décompte agit.
            log::info!("bandeau action {id} {action}");
            let decompte = annuler_si_decompte(
                app,
                ArriveeDecompte::Action {
                    id,
                    action: &action,
                },
            );
            // « Réessayer » de « Aucun texte trouvé » : Copier le texte, tout de suite (le
            // bandeau est caché avant la photo).
            if !decompte && action == texte::ACTION_REESSAYER && texte_construit(app) {
                lancer_capture(app, Mode::Texte);
            }
        }
        Evenement::Vignette(signal) => traiter_vignette(app, signal),
        Evenement::ActionFaite(fin) => action_faite(app, *fin),
        Evenement::SuppressionFaite { message } => {
            afficher(app, message);
        }
        Evenement::LancerGlisser { id, chemin } => lancer_glisser(app, id, &chemin),
        Evenement::EditeurDetruit => editeur_detruit(app),
        Evenement::EditeurFermetureDemandee => fermeture_du_cadre(app),
        Evenement::EditeurExportFait(fin) => finir_export(app, *fin),
        Evenement::EditeurExportEchu(numero) => export_echu(app, numero),
        Evenement::EditeurGlisser { id, chemin } => lancer_glisser_editeur(app, id, &chemin),
        Evenement::Historique(signal) => geste_historique::traiter(app, signal),
        Evenement::TexteLu(fin) => geste_texte::texte_lu(app, *fin),
        Evenement::TexteEchu(numero) => geste_texte::echu(app, numero),
        Evenement::TexteDisponible(oui) => texte_devenu_disponible(app, oui),
        // Aucune annonce à l'activation ni au retrait de la licence.
        Evenement::LicenceChangee => {
            suivre_les_actions(app);
        }
    }
}

/// Applique un déclencheur à l'automate. Rend le nouvel état, `None` s'il est ignoré.
pub(super) fn appliquer(app: &AppHandle, declencheur: Declencheur) -> Option<Etat> {
    match app.state::<Application>().automate.lock() {
        Ok(mut a) => a.traiter(declencheur),
        Err(_) => {
            log::error!("automate inaccessible");
            None
        }
    }
}

/// L'état de l'automate, `None` s'il est inaccessible.
pub(super) fn etat_automate(app: &AppHandle) -> Option<Etat> {
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
/// Vignette (aucun second `figer` tant qu'une capture est en cours, ni pendant un
/// décompte) ; acceptée, elle est comptée (une demande du menu en attente devient
/// caduque). Avec le retardateur (réglages en vigueur, D12 : raccourci, menu et
/// `--capturer`), le décompte d'abord ; sinon la séquence du mode, tout de suite.
fn demarrer_capture(app: &AppHandle, mode: Mode) {
    let recu_a = Instant::now();
    let glisser = app.state::<Application>().glisser.load(Ordering::SeqCst);
    if !vignette::capture_acceptee(glisser) {
        log::info!("capture ignoree: glisser en cours");
        return;
    }
    let retardateur_s = reglages_lus(app).capture.retardateur_s;
    let declencheur = decompte::declencheur(mode, retardateur_s);
    if appliquer(app, declencheur).is_none() {
        log::info!("capture deja en cours: appui ignore");
        return;
    }
    noter_demande(app);
    match declencheur {
        Declencheur::DemandeDecompte(_) => commencer_decompte(app, mode, retardateur_s),
        _ => apres_gel(app, mode, recu_a, false),
    }
}

/// La séquence de chaque mode, une fois en Gel : zone et fenêtre (§3.6), plein écran
/// (§3.7), dernière zone (§5 de la 0.4). `bandeau_cache` : le décompte vient d'être
/// caché, le compositeur est attendu avant la photo.
fn apres_gel(app: &AppHandle, mode: Mode, recu_a: Instant, bandeau_cache: bool) {
    match mode {
        Mode::Zone => capturer_avec_selection(app, recu_a, Mode::Zone, bandeau_cache),
        Mode::Fenetre => capturer_avec_selection(app, recu_a, Mode::Fenetre, bandeau_cache),
        Mode::PleinEcran => capturer_plein_ecran(app, recu_a, bandeau_cache),
        Mode::DerniereZone => capturer_derniere_zone(app, recu_a, bandeau_cache),
        Mode::Texte => capturer_avec_selection(app, recu_a, Mode::Texte, bandeau_cache),
    }
}

/// Entrée en Décompte (fil principal, état déjà Décompte) : vignette fermée, disque montré
/// dans le bandeau sur sa cible, Échap réservé, fil `decompte` lancé. Sans fil possible,
/// la capture part tout de suite.
fn commencer_decompte(app: &AppHandle, mode: Mode, secondes: u32) {
    fermer_vignette(app, "capture");
    let etat = app.state::<Application>();
    let numero = etat.numero_decompte.fetch_add(1, Ordering::SeqCst) + 1;
    let duree = decompte::duree(secondes);
    log::info!("{}", decompte::ligne_debut(numero, secondes, mode));
    let derniere = etat
        .derniere_zone
        .lock()
        .ok()
        .and_then(|d| d.map(|d| d.zone));
    let cible = decompte::cible(mode, derniere, souris_bureau(app), &gel::ecrans());
    let reglages = reglages_lus(app);
    let reduites =
        vignette::animations_reduites(reglages.animations, systeme::animations_reduites());
    // Rangé avant tout appel à un module : une annulation arrivée pendant `montrer_decompte`
    // ou `reserver_echap` le trouve. Un verrou empoisonné est repris : sans décompte rangé,
    // l'état resterait Décompte.
    let (arret, attente) = mpsc::channel::<()>();
    let ancien = etat
        .decompte
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .replace(DecompteEnCours {
            numero,
            mode,
            debut: Instant::now(),
            arret,
            affichage: None,
        });
    if let Some(a) = ancien {
        // Impossible (la table refuse une demande en Décompte) : l'ancien fil s'arrête.
        log::error!("decompte {} encore range: remplace", a.numero);
    }
    let affichage = match (cible, app.try_state::<Bandeau>()) {
        (Some(c), Some(b)) => Some(b.montrer_decompte(fenetres::Decompte {
            numero,
            secondes,
            restant: duree,
            centre: c.centre,
            cote: c.cote,
            reduites,
        }))
        .filter(|id| *id != 0),
        _ => None,
    };
    if affichage.is_none() {
        log::info!("decompte {numero} sans affichage");
    }
    if !noter_affichage_decompte(app, numero, affichage) {
        // Annulé pendant `montrer_decompte` : le disque montré entre-temps est caché.
        cacher_affichage_decompte(app, affichage);
        return;
    }
    reserver_echap(app);
    if decompte_en_cours(app).0 != Some(numero) {
        // Annulé pendant la réservation : Échap est rendu.
        liberer_echap(app);
        return;
    }
    let pour_fil = app.clone();
    let lance = std::thread::Builder::new()
        .name("decompte".into())
        .spawn(move || {
            // Échéance : la fin part vers le fil principal (`traiter` la poste, l'événement
            // étant de capture). Message reçu ou canal fermé (annulation) : rien.
            if let Err(RecvTimeoutError::Timeout) = attente.recv_timeout(duree) {
                traiter(&pour_fil, Evenement::DecompteFini(numero));
            }
        });
    if let Err(e) = lance {
        log::warn!("decompte impossible ({e}): capture tout de suite");
        finir_decompte(app, numero);
    }
}

/// Note l'affichage du décompte `numero` ; faux s'il n'est plus en cours.
fn noter_affichage_decompte(app: &AppHandle, numero: u64, affichage: Option<u64>) -> bool {
    let etat = app.state::<Application>();
    let mut d = etat.decompte.lock().unwrap_or_else(|e| e.into_inner());
    match d.as_mut().filter(|d| d.numero == numero) {
        Some(d) => {
            d.affichage = affichage;
            true
        }
        None => false,
    }
}

/// Retire le décompte en cours s'il porte ce numéro (sinon le laisse).
fn prendre_decompte(app: &AppHandle, numero: Option<u64>) -> Option<DecompteEnCours> {
    let etat = app.state::<Application>();
    let mut d = etat.decompte.lock().unwrap_or_else(|e| e.into_inner());
    match numero {
        Some(n) => d.take_if(|d| d.numero == n),
        None => d.take(),
    }
}

/// Le numéro et l'affichage du décompte en cours.
fn decompte_en_cours(app: &AppHandle) -> (Option<u64>, Option<u64>) {
    let etat = app.state::<Application>();
    let d = etat.decompte.lock().unwrap_or_else(|e| e.into_inner());
    (
        d.as_ref().map(|d| d.numero),
        d.as_ref().and_then(|d| d.affichage),
    )
}

/// Fin du délai `numero` (fil principal) : une fin périmée (décompte annulé, ou d'un autre
/// numéro) est ignorée. Sinon : Échap rendu (la sélection le reprendra), disque caché,
/// Gel, puis la séquence du mode (les chronos partent d'ici ; plein écran : la souris
/// est lue maintenant, D14).
fn finir_decompte(app: &AppHandle, numero: u64) {
    let (attendu, _) = decompte_en_cours(app);
    if !decompte::fin_a_jour(attendu, numero) {
        log::info!("decompte {numero}: fin perimee ignoree");
        return;
    }
    let Some(d) = prendre_decompte(app, Some(numero)) else {
        log::info!("decompte {numero}: fin perimee ignoree");
        return;
    };
    liberer_echap(app);
    let cache = cacher_affichage_decompte(app, d.affichage);
    if appliquer(app, Declencheur::DecompteFini(d.mode)).is_none() {
        log::error!("decompte {numero}: fin refusee par la table");
        return;
    }
    log::info!("{}", decompte::ligne_fin(numero, d.debut.elapsed()));
    apres_gel(app, d.mode, Instant::now(), cache);
}

/// Cache le disque `affichage` dans le bandeau ; vrai s'il était montré.
fn cacher_affichage_decompte(app: &AppHandle, affichage: Option<u64>) -> bool {
    match (affichage, app.try_state::<Bandeau>()) {
        (Some(id), Some(b)) => b.cacher_decompte(id),
        _ => false,
    }
}

/// Annule le décompte si l'arrivée le concerne (`decompte::annulation`) ; rend vrai si
/// elle était pour lui (rien d'autre à faire).
fn annuler_si_decompte(app: &AppHandle, arrivee: ArriveeDecompte<'_>) -> bool {
    let (_, affichage) = decompte_en_cours(app);
    match decompte::annulation(etat_automate(app), affichage, arrivee) {
        Some(cause) => {
            annuler_decompte(app, cause);
            true
        }
        None => false,
    }
}

/// Annule le décompte en cours : fil arrêté, disque caché, Échap rendu, Repos (sauf à
/// l'arrêt de l'app). Aucun message. Sans décompte en cours, ne fait rien.
fn annuler_decompte(app: &AppHandle, cause: Cause) {
    let Some(d) = prendre_decompte(app, None) else {
        return;
    };
    let DecompteEnCours {
        numero,
        arret,
        affichage,
        ..
    } = d;
    drop(arret);
    cacher_affichage_decompte(app, affichage);
    liberer_echap(app);
    if cause != Cause::Arret {
        appliquer(app, Declencheur::DecompteAnnule);
    }
    log::info!("{}", decompte::ligne_annule(numero, cause));
}

/// Début commun à tous les modes, une fois en Gel : la vignette (suppression en attente
/// validée) et le bandeau (message ou décompte) sont cachés sans animation (ni l'un ni
/// l'autre ne doit figurer sur la photo ni rester au-dessus de l'écran figé), le
/// compositeur attendu (borné) si l'un d'eux était montré ou si `deja_cache` (le décompte
/// caché juste avant), puis la photo, avec le pointeur si le réglage le veut. Erreur →
/// `GelImpossible` (Repos) et bandeau d'erreur ; rend `None`.
pub(super) fn photographier(app: &AppHandle, deja_cache: bool, mode: Mode) -> Option<capture::Gel> {
    // Lu avant l'attente du compositeur : rien entre l'attente et la photo. Copier le
    // texte ne prend jamais le pointeur.
    let avec_pointeur = reglages_lus(app).capture.pointeur && mode != Mode::Texte;
    let vignette_cachee = fermer_vignette(app, "capture");
    let bandeau_cache = app.try_state::<Bandeau>().is_some_and(|b| b.cacher());
    if (bandeau_cache || vignette_cachee || deja_cache)
        && !systeme::attendre_composition(ATTENTE_COMPOSITION)
    {
        log::warn!(
            "composition non attendue ({} ms): photo prise quand meme",
            ATTENTE_COMPOSITION.as_millis()
        );
    }
    match capture::figer(avec_pointeur) {
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
pub(super) fn souris_bureau(app: &AppHandle) -> Option<(i32, i32)> {
    zone::point_souris(fenetres::souris(app))
}

/// Pose le pointeur pris avec la photo dans les pixels découpés de `zone` (avant
/// l'arrondi des coins d'une fenêtre). Rien si le pointeur n'a pas été pris.
fn poser_pointeur(gel_pris: &capture::Gel, pixels: &mut gel::Pixels, zone: systeme::Rect) {
    let Some(p) = gel_pris.pointeur.as_ref() else {
        return;
    };
    // Les pixels sont ceux de la zone coupée au bureau (`capture::decouper`).
    let lue = zone.intersection(gel_pris.bureau).unwrap_or(zone);
    let pose = capture::poser_pointeur(pixels, lue, p);
    log::info!("{}", capture::ligne_pose(pose));
}

/// Plein écran (§3.7), en Gel, sur le fil principal : souris lue avant tout, photo,
/// écran choisi (`capture::ecran_de_capture`), Découpe, puis la sortie. Rien n'est
/// montré : ni écran figé, ni sélection, ni Échap réservé, ni garde.
fn capturer_plein_ecran(app: &AppHandle, recu_a: Instant, bandeau_cache: bool) {
    let souris = souris_bureau(app);
    let Some(gel_pris) = photographier(app, bandeau_cache, Mode::PleinEcran) else {
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
    decouper_sans_selection(app, gel_pris, ecran.rect, recu_a);
}

/// Découpe sans sélection (plein écran, dernière zone), déjà en Découpe : pixels de `rect`,
/// pointeur posé, photo rendue, puis la sortie (sans transparence). Erreur → Repos et
/// bandeau d'erreur.
fn decouper_sans_selection(
    app: &AppHandle,
    gel_pris: capture::Gel,
    rect: systeme::Rect,
    recu_a: Instant,
) {
    let debut = Instant::now();
    let pixels = capture::decouper(&gel_pris, rect).map(|mut p| {
        poser_pointeur(&gel_pris, &mut p, rect);
        p
    });
    let duree_decoupe = debut.elapsed();
    drop(gel_pris);
    match pixels {
        Ok(pixels) => lancer_sortie(app, pixels, rect, duree_decoupe, recu_a, false),
        Err(e) => {
            log::warn!("decoupe impossible: {e}");
            appliquer(app, Declencheur::DecoupeImpossible);
            afficher_probleme(app, Probleme::Lecture);
        }
    }
}

/// Contrôles d'avant photo des captures qui peuvent montrer la sélection : aucune capture
/// ne doit rester rangée (sa garde, détruite plus tard, cacherait le nouvel écran figé), et
/// la Sélection doit exister (sinon l'écran figé ne serait jamais montré pour rien). Faux :
/// Repos et bandeau d'erreur, pas de photo.
fn pret_pour_selection(app: &AppHandle) -> bool {
    if let Some(ancienne) = ranger_capture(app, None) {
        log::error!("capture precedente encore rangee: abandonnee avant la photo");
        drop(ancienne);
    }
    if app.try_state::<Selection>().is_none() {
        log::warn!("selection absente: pas de photo");
        appliquer(app, Declencheur::GelImpossible);
        afficher_probleme(app, Probleme::Selection);
        return false;
    }
    true
}

/// Zone et fenêtre (§3.6), en Gel : contrôles, photo, puis `montrer_selection`. Même code
/// pour les deux modes.
fn capturer_avec_selection(app: &AppHandle, recu_a: Instant, mode: Mode, bandeau_cache: bool) {
    if !pret_pour_selection(app) {
        return;
    }
    // 2 et 3. Bandeau caché, compositeur attendu, la photo.
    let Some(gel_pris) = photographier(app, bandeau_cache, mode) else {
        return;
    };
    montrer_selection(app, recu_a, mode, gel_pris);
}

/// Refaire la dernière zone (§5 de la 0.4), en Gel : contrôles et photo comme la zone,
/// puis la zone retenue découpée tout de suite si son écran n'a pas changé (rien n'est
/// montré, comme le plein écran) ; sinon la sélection de zone sur cette même photo, sans
/// message.
fn capturer_derniere_zone(app: &AppHandle, recu_a: Instant, bandeau_cache: bool) {
    if !pret_pour_selection(app) {
        return;
    }
    let Some(gel_pris) = photographier(app, bandeau_cache, Mode::DerniereZone) else {
        return;
    };
    let derniere = app
        .state::<Application>()
        .derniere_zone
        .lock()
        .ok()
        .and_then(|d| *d);
    let refaire = zone::refaire(derniere.as_ref(), &gel_pris.ecrans);
    log::info!("{}", zone::ligne_refaire(&refaire));
    let Refaire::Zone(rect) = refaire else {
        montrer_selection(app, recu_a, Mode::Zone, gel_pris);
        return;
    };
    log::info!(
        "{}",
        zone::ligne_chrono_photo(gel_pris.duree_photo, gel_pris.duree_totale)
    );
    appliquer(app, Declencheur::PhotoPrise(Mode::DerniereZone));
    decouper_sans_selection(app, gel_pris, rect, recu_a);
}

/// La photo prise, en Gel (zone, fenêtre, repli de la dernière zone) : écran figé, (en
/// mode fenêtre, la liste des fenêtres), sélection, Échap réservé. Toute
/// erreur après `montrer_ecran_fige` cache tout (garde) et revient au Repos.
fn montrer_selection(app: &AppHandle, recu_a: Instant, mode: Mode, gel_pris: capture::Gel) {
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
    } else if mode == Mode::Texte {
        (selection::Mode::Texte, Vec::new())
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
pub(super) fn liberer_echap(app: &AppHandle) {
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
pub(super) fn abandonner(app: &AppHandle, declencheur: Declencheur, probleme: Option<Probleme>) {
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
    let (geste, texte) = {
        let c = capture_en_cours(app);
        let texte = c.as_ref().is_some_and(|c| c.mode == selection::Mode::Texte);
        let geste = zone::decider(
            c.as_ref().map(|c| Vue {
                selection: c.selection,
                ecrans: &c.gel.ecrans,
                mode: c.mode,
                fenetres: &c.fenetres,
            }),
            arrivee,
        );
        (geste, texte)
    };
    match geste {
        Geste::Rien => {}
        Geste::Abandonner {
            declencheur,
            probleme,
        } => abandonner(app, declencheur, probleme),
        // Copier le texte : la zone est lue, jamais découpée comme une capture.
        Geste::Decouper(zone) if texte => geste_texte::zone_choisie(app, zone),
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
        poser_pointeur(&capture.gel, &mut p, zone);
        if let Some((rayon, coins)) = fenetre.filter(|(rayon, _)| *rayon > 0) {
            capture::arrondir(&mut p, rayon, coins);
        }
        p
    });
    let duree_decoupe = debut.elapsed();
    // Seule une zone devient la dernière zone (D7) ; une fenêtre, non.
    let retenue =
        zone::nouvelle_derniere_zone(pixels.is_ok(), fenetre.is_none(), zone, &capture.gel.ecrans);
    drop(capture);
    if let Some(nouvelle) = retenue {
        // Oubliée sans ligne : « Refaire » écrira `derniere zone: aucune, selection`.
        if let Some(d) = &nouvelle {
            log::info!("{}", zone::ligne_retenue(d));
        }
        if let Ok(mut place) = app.state::<Application>().derniere_zone.lock() {
            *place = nouvelle;
        }
    }
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
pub(super) fn demande_sortie(app: &AppHandle, transparence: bool) -> sortie::Demande {
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
pub(super) fn reglages_lus(app: &AppHandle) -> Reglages {
    app.state::<Application>()
        .reglages
        .lock()
        .ok()
        .and_then(|r| r.clone())
        .unwrap_or_else(|| Reglages::par_defaut(plateforme_reglages(systeme::systeme())))
}

/// Les dossiers résolus au démarrage.
pub(super) fn dossiers_lus(app: &AppHandle) -> Option<Dossiers> {
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
pub(super) fn sortir_ou_echec(
    numero: u64,
    pixels: &gel::Pixels,
    demande: &sortie::Demande,
) -> sortie::Bilan {
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
            geste_historique::apres_sortie(&pour_fil, &fin.pixels, &fin.bilan);
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
        geste_historique::apres_sortie(app, &fin.pixels, &fin.bilan);
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
        actions: vignette::actions(
            EDITEUR_CONSTRUIT && editeur::annotable(fin.pixels.largeur, fin.pixels.hauteur),
        ),
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
pub(super) fn montree(app: &AppHandle) -> std::sync::MutexGuard<'_, Option<CaptureMontree>> {
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
pub(super) fn retirer_montree(app: &AppHandle, id: u64) -> Option<CaptureMontree> {
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
        vignette::Geste::Annoter => annoter(app, id),
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
            geste_historique::rafraichir(&pour_fil);
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
pub(super) fn dossiers_vignette(app: &AppHandle) -> (PathBuf, PathBuf) {
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

pub(super) fn fermer_vignette(app: &AppHandle, raison: &str) -> bool {
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
pub(super) fn nos_fenetres(app: &AppHandle) -> Vec<isize> {
    let mut liste: Vec<isize> = app
        .try_state::<Vignette>()
        .map(|v| v.identifiants().into_iter().flatten().collect())
        .unwrap_or_default();
    liste.extend(app.try_state::<Bandeau>().and_then(|b| b.identifiant()));
    liste.extend(fenetres::editeur::identifiant(app));
    liste.extend(fenetres::historique::identifiant(app));
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

// Éditeur d'annotation (contrat 0.4 « Éditeur », §4 à §7). Les décisions sont dans
// `editeur.rs` ; ici, seulement les gestes. Fil principal ; le verrou de l'éditeur n'est
// jamais tenu pendant un appel à Tauri ou à un module.

/// Vrai tant qu'un dialogue « Enregistrer sous » de l'éditeur est ouvert.
static DIALOGUE_EDITEUR: AtomicBool = AtomicBool::new(false);

/// Rend `DIALOGUE_EDITEUR` à la fin du dialogue, quoi qu'il arrive.
struct GardeDialogueEditeur;
impl Drop for GardeDialogueEditeur {
    fn drop(&mut self) {
        DIALOGUE_EDITEUR.store(false, Ordering::SeqCst);
    }
}

/// Écran supposé si le système n'en décrit aucun.
const ECRAN_DE_SECOURS: fenetres::Ecran = fenetres::Ecran {
    rect: fenetres::Rect {
        x: 0,
        y: 0,
        largeur: 1024,
        hauteur: 768,
    },
    zone_utile: fenetres::Rect {
        x: 0,
        y: 0,
        largeur: 1024,
        hauteur: 768,
    },
    echelle: 1.0,
};

/// `f` sur l'éditeur ouvert ; avec `id`, seulement si c'est son document.
pub(super) fn avec_editeur<R>(
    app: &AppHandle,
    id: Option<u64>,
    f: impl FnOnce(&mut editeur::EtatEditeur) -> R,
) -> Option<R> {
    let mut garde = app
        .state::<Application>()
        .inner()
        .editeur
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    garde
        .as_mut()
        .filter(|e| id.is_none_or(|i| e.document.id == i))
        .map(f)
}

/// Retire l'éditeur ouvert (son état) et le rend.
fn retirer_editeur(app: &AppHandle) -> Option<editeur::EtatEditeur> {
    app.state::<Application>()
        .inner()
        .editeur
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
}

/// Animations réduites pour la page de l'éditeur (réglage, ou Windows).
fn reduites_editeur(app: &AppHandle) -> bool {
    vignette::animations_reduites(reglages_lus(app).animations, systeme::animations_reduites())
}

/// Les préférences d'outil de la session.
fn preferences_editeur(app: &AppHandle) -> Option<String> {
    app.state::<Application>()
        .preferences_editeur
        .lock()
        .ok()
        .and_then(|p| p.clone())
}

/// Garde les préférences envoyées par la page (`editeur::garder_preferences`).
fn garder_preferences_editeur(app: &AppHandle, nouveau: Option<String>) {
    if let Ok(mut p) = app.state::<Application>().preferences_editeur.lock() {
        *p = editeur::garder_preferences(p.take(), nouveau);
    }
}

/// Un document rendu (fermeture, remplacement manqué) : ses pixels sont libérés ici.
fn rendre_document(id: u64, document: editeur::EtatEditeur) {
    if let Some(a) = &document.en_attente {
        log::info!("editeur: capture {} abandonnee", a.id);
    }
    drop(document);
    log::info!("editeur pixels rendus {id}");
}

/// Annoter de la vignette (§4.2) : la capture passe à l'éditeur (mêmes pixels, aucune
/// copie), la vignette se ferme sans animation et sans rien effacer.
fn annoter(app: &AppHandle, id: u64) {
    let demande_a = Instant::now();
    let Some(c) = retirer_montree(app, id) else {
        return;
    };
    let CaptureMontree {
        pixels,
        transparence,
        maintenant,
        ..
    } = c;
    appliquer(app, Declencheur::VignetteFermee);
    if let Some(v) = app.try_state::<Vignette>() {
        v.cacher();
    }
    log::info!("vignette fermee {id} annoter");
    let document = editeur::Document {
        id,
        pixels,
        transparence,
        maintenant,
    };
    // Annoter arrive dans une commande de la page de la vignette. Sous Windows, créer une
    // fenêtre Web dans le rappel d'une autre vue Web bloque le fil principal pour
    // toujours : le document est reçu à un tour suivant de la boucle.
    let pour_fil = app.clone();
    let lance = std::thread::Builder::new()
        .name("editeur-poste".into())
        .spawn(move || {
            let pour = pour_fil.clone();
            let poste =
                pour_fil.run_on_main_thread(move || recevoir_document(&pour, document, demande_a));
            if let Err(e) = poste {
                log::warn!("editeur impossible {id}: poste ({e})");
            }
        });
    if let Err(e) = lance {
        log::warn!("editeur impossible {id}: poste ({e})");
        afficher(app, editeur::message_impossible());
    }
}

/// Une capture pour l'éditeur : ouvrir, remplacer ou demander (§4.3).
pub(super) fn recevoir_document(app: &AppHandle, document: editeur::Document, demande_a: Instant) {
    let arrivee = {
        let garde = app
            .state::<Application>()
            .inner()
            .editeur
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        editeur::arrivee(garde.as_ref())
    };
    let id = document.id;
    match arrivee {
        editeur::Arrivee::Ouvrir => ouvrir_editeur(app, document, demande_a),
        editeur::Arrivee::Remplacer => {
            if let Some(ancien) = avec_editeur(app, None, |e| e.remplacer(document, demande_a)) {
                log::info!("editeur remplace {} par {id}", ancien.id);
            }
            ramener_editeur(app, id);
            charger_editeur(app);
        }
        editeur::Arrivee::Demander => {
            let Some((courant, abandonnee)) = avec_editeur(app, None, |e| {
                (e.document.id, e.mettre_en_attente(document).map(|d| d.id))
            }) else {
                return;
            };
            if let Some(a) = abandonnee {
                log::info!("editeur: capture {a} abandonnee");
            }
            ramener_editeur(app, courant);
            demander_fermeture(app, editeur::Demande::Remplacer);
        }
    }
}

/// Ramène la fenêtre devant (`editeur ramene devant <id>`).
fn ramener_editeur(app: &AppHandle, id: u64) {
    if fenetres::editeur::ramener_devant(app) {
        log::info!("editeur ramene devant {id}");
    }
}

/// Fait `f` sur le fil principal à un tour suivant de la boucle (jamais tout de suite).
pub(super) fn plus_tard(
    app: &AppHandle,
    attente: Duration,
    f: impl FnOnce(&AppHandle) + Send + 'static,
) {
    plus_tard_nomme(app, "editeur-poste", attente, f);
}

/// `plus_tard` par un fil nommé `nom` (`historique-poste`…). Rend faux si le fil n'a pas pu
/// être lancé (rien ne sera fait).
pub(super) fn plus_tard_nomme(
    app: &AppHandle,
    nom: &str,
    attente: Duration,
    f: impl FnOnce(&AppHandle) + Send + 'static,
) -> bool {
    let pour_fil = app.clone();
    let prefixe = nom.trim_end_matches("-poste").to_string();
    let pour_journal = prefixe.clone();
    let lance = std::thread::Builder::new().name(nom.into()).spawn(move || {
        if !attente.is_zero() {
            std::thread::sleep(attente);
        }
        let pour = pour_fil.clone();
        if let Err(e) = pour_fil.run_on_main_thread(move || f(&pour)) {
            log::warn!("{pour_journal}: geste non poste: {e}");
        }
    });
    match lance {
        Ok(_) => true,
        Err(e) => {
            log::warn!("{prefixe}: geste non poste ({e})");
            false
        }
    }
}

/// Range une capture à ouvrir dès que la fenêtre précédente est détruite (fermeture avec
/// une capture en attente, ou Annoter arrivé entre `destroy()` et `Destroyed`).
fn garder_suivant(app: &AppHandle, document: editeur::Document, demande_a: Instant) {
    let id = document.id;
    let ancien = app
        .state::<Application>()
        .editeur_suivant
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .replace((document, demande_a));
    if let Some((a, _)) = ancien {
        log::info!("editeur: capture {} abandonnee", a.id);
    }
    log::info!("editeur ouverture differee {id}");
}

/// La capture rangée par `garder_suivant` (`editeur::suivant`) : par le chemin normal
/// d'une capture qui arrive si la fenêtre précédente est partie ou si un éditeur neuf est
/// ouvert entre-temps ; sinon réessaie un peu plus tard (au plus `essais` fois).
fn ouvrir_suivant(app: &AppHandle, essais: u32) {
    let vivant = avec_editeur(app, None, |_| ()).is_some();
    let decision = editeur::suivant(vivant, fenetres::editeur::existe(app), essais);
    if decision == editeur::Suivant::Attendre {
        plus_tard(app, Duration::from_millis(100), move |a| {
            ouvrir_suivant(a, essais - 1)
        });
        return;
    }
    let Some((document, demande_a)) = app
        .state::<Application>()
        .editeur_suivant
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
    else {
        return;
    };
    if decision == editeur::Suivant::Abandonner {
        log::warn!(
            "editeur impossible {}: fenetre (encore ouverte)",
            document.id
        );
        drop(document);
        afficher(app, editeur::message_impossible());
        return;
    }
    recevoir_document(app, document, demande_a);
}

/// Crée la fenêtre (cachée) pour `document` ; l'état est rangé avant la création, pour
/// qu'une commande de la page le trouve. Une fenêtre encore en train d'être détruite :
/// la capture attend son `Destroyed`.
fn ouvrir_editeur(app: &AppHandle, document: editeur::Document, demande_a: Instant) {
    if fenetres::editeur::existe(app) {
        garder_suivant(app, document, demande_a);
        return;
    }
    let id = document.id;
    let image = (document.pixels.largeur, document.pixels.hauteur);
    let ecran = fenetres::editeur::ecran_souris(app).unwrap_or(ECRAN_DE_SECOURS);
    let place = fenetres::editeur::placer(image, &ecran);
    let etat_app = app.state::<Application>();
    let numero = etat_app
        .numero_fenetre_editeur
        .fetch_add(1, Ordering::SeqCst)
        + 1;
    *etat_app
        .inner()
        .editeur
        .lock()
        .unwrap_or_else(|e| e.into_inner()) =
        Some(editeur::EtatEditeur::nouveau(document, demande_a, numero));
    etat_app
        .fenetre_editeur_vivante
        .store(numero, Ordering::SeqCst);
    let (pour_destruction, pour_fermeture) = (app.clone(), app.clone());
    // `Destroyed` est traité sur-le-champ (fil principal), une seule fois, pour cette
    // fenêtre seulement.
    let detruite = move || {
        let vivante = &pour_destruction
            .state::<Application>()
            .inner()
            .fenetre_editeur_vivante;
        if vivante
            .compare_exchange(numero, 0, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            traiter(&pour_destruction, Evenement::EditeurDetruit);
        }
    };
    let fermeture_demandee =
        move || poster_plus_tard(&pour_fermeture, Evenement::EditeurFermetureDemandee);
    match fenetres::editeur::ouvrir(app, place, detruite, fermeture_demandee) {
        Ok(fenetres::editeur::Ouverture::Creee { identifiant }) => {
            match identifiant {
                Some(h) => log::info!("editeur fenetre {h}"),
                None => log::info!("editeur fenetre ouverte"),
            }
            log::info!("{}", editeur::ligne_ouvert(id, image, &ecran, &place));
            armer_filet_page(app, numero);
        }
        Ok(fenetres::editeur::Ouverture::Existante) => {
            ouverture_impossible(app, id, "fenetre (encore ouverte)")
        }
        Err(e) => ouverture_impossible(app, id, &format!("fenetre ({e})")),
    }
}

/// L'éditeur n'a pas pu s'ouvrir : document rendu, bandeau (Erreur, titre seul).
fn ouverture_impossible(app: &AppHandle, id: u64, cause: &str) {
    let etat = {
        let mut garde = app
            .state::<Application>()
            .inner()
            .editeur
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        garde.take_if(|e| e.document.id == id)
    };
    log::warn!("editeur impossible {id}: {cause}");
    if let Some(e) = etat {
        rendre_document(id, e);
    }
    afficher(app, editeur::message_impossible());
}

/// Filet : pas d'`editeur_pret` de la fenêtre `numero` en `DELAI_PAGE` → fenêtre détruite,
/// document rendu, bandeau. Le fil attend sur un canal que `editeur_pret` (ou la
/// fermeture) ferme : il finit aussitôt, sans dormir.
fn armer_filet_page(app: &AppHandle, numero: u64) {
    let (signal, attente) = mpsc::channel::<()>();
    *app.state::<Application>()
        .filet_page
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(signal);
    let pour_fil = app.clone();
    let lance = std::thread::Builder::new()
        .name("editeur-filet".into())
        .spawn(move || {
            if attente.recv_timeout(editeur::DELAI_PAGE) != Err(RecvTimeoutError::Timeout) {
                return;
            }
            let pour = pour_fil.clone();
            let _ = pour_fil.run_on_main_thread(move || {
                let muette = avec_editeur(&pour, None, |e| {
                    editeur::page_muette(e, numero).then_some(e.document.id)
                })
                .flatten();
                if let Some(id) = muette {
                    let etat = retirer_editeur(&pour);
                    log::warn!("editeur impossible {id}: page muette");
                    if let Some(e) = etat {
                        rendre_document(id, e);
                    }
                    fenetres::editeur::fermer(&pour);
                    afficher(&pour, editeur::message_impossible());
                }
            });
        });
    if let Err(e) = lance {
        log::warn!("editeur: filet de la page non arme: {e}");
    }
}

/// La page a dit prête (ou l'éditeur se ferme) : le filet de la page finit.
fn desarmer_filet_page(app: &AppHandle) {
    drop(
        app.state::<Application>()
            .filet_page
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take(),
    );
}

/// `editeur:charger` : la page vide tout et redemande l'image (remplacement).
fn charger_editeur(app: &AppHandle) {
    let reduites = reduites_editeur(app);
    let preferences = preferences_editeur(app);
    let texte = texte_construit(app);
    if let Some(c) = avec_editeur(app, None, |e| {
        editeur::charge(e, reduites, preferences, texte)
    }) {
        fenetres::editeur::emettre(app, fenetres::editeur::EVENEMENT_CHARGER, c);
    }
}

/// Ferme l'éditeur sans demander : état retiré, document rendu (journal), fenêtre
/// détruite (jamais cachée : la mémoire du moteur de rendu est rendue). Une capture en
/// attente s'ouvre ensuite dans une nouvelle fenêtre, sauf à l'arrêt.
fn fermer_editeur(app: &AppHandle, raison: editeur::RaisonFermeture) {
    desarmer_filet_page(app);
    if let Some(mut e) = retirer_editeur(app) {
        let id = e.document.id;
        log::info!("{}", editeur::ligne_fermeture(id, raison));
        if raison != editeur::RaisonFermeture::Arret {
            if let Some(suivant) = e.en_attente.take() {
                garder_suivant(app, suivant, Instant::now());
            }
        }
        rendre_document(id, e);
    }
    fenetres::editeur::fermer(app);
}

/// Fermeture décidée : la capture en attente est chargée à la place s'il y en a une
/// (« Copier et fermer », « Ne pas garder » d'un remplacement), sinon l'éditeur se ferme.
fn fermer_ou_suivant(app: &AppHandle, raison: editeur::RaisonFermeture) {
    let ancien = avec_editeur(app, None, |e| {
        e.charger_en_attente(Instant::now())
            .map(|a| (a, e.document.id))
    })
    .flatten();
    match ancien {
        Some((a, id)) => {
            log::info!("editeur remplace {a} par {id}");
            ramener_editeur(app, id);
            charger_editeur(app);
        }
        None => fermer_editeur(app, raison),
    }
}

/// La fenêtre est détruite : un état encore là (destruction que l'Application n'a pas
/// décidée) est retiré ; puis la capture rangée pour la suite s'ouvre, au tour suivant.
fn editeur_detruit(app: &AppHandle) {
    desarmer_filet_page(app);
    if let Some(e) = retirer_editeur(app) {
        let id = e.document.id;
        log::warn!(
            "{}",
            editeur::ligne_fermeture(id, editeur::RaisonFermeture::Impossible)
        );
        rendre_document(id, e);
    }
    plus_tard(app, Duration::ZERO, |a| ouvrir_suivant(a, 20));
}

/// Croix du cadre, Alt+F4, menu système (fermeture empêchée) : `editeur::au_cadre`.
fn fermeture_du_cadre(app: &AppHandle) {
    match avec_editeur(app, None, |e| editeur::au_cadre(e)) {
        Some(editeur::Cadre::Demander) => demander_fermeture(app, editeur::Demande::Cadre),
        Some(editeur::Cadre::FermerPropre) => fermer_editeur(app, editeur::RaisonFermeture::Propre),
        Some(editeur::Cadre::FermerMuette) => {
            fermer_editeur(app, editeur::RaisonFermeture::PageMuette)
        }
        None => fenetres::editeur::fermer(app),
    }
}

/// `editeur:fermer_demande { id, raison }`, et son filet (`DELAI_FERMETURE`) :
/// `editeur::au_filet` ne ferme d'office qu'un document connu comme propre.
fn demander_fermeture(app: &AppHandle, demande: editeur::Demande) {
    let numero = app
        .state::<Application>()
        .numero_fermeture
        .fetch_add(1, Ordering::SeqCst)
        + 1;
    let Some(id) = avec_editeur(app, None, |e| {
        e.fermeture_demandee = Some((numero, demande));
        e.document.id
    }) else {
        return;
    };
    let raison = demande.nom();
    log::info!("editeur fermeture demandee {id} ({raison})");
    fenetres::editeur::emettre(
        app,
        fenetres::editeur::EVENEMENT_FERMER_DEMANDE,
        editeur::ChargeFermerDemande { id, raison },
    );
    plus_tard(app, editeur::DELAI_FERMETURE, move |a| {
        let filet = avec_editeur(a, None, |e| (editeur::au_filet(e, numero), e.document.id));
        match filet {
            Some((editeur::Filet::Fermer, _)) => {
                fermer_editeur(a, editeur::RaisonFermeture::PageMuette)
            }
            Some((editeur::Filet::Garder, id)) => {
                log::warn!("editeur fermeture {id} sans reponse: gardee")
            }
            _ => {}
        }
    });
}

/// Un export de la page (`editeur_exporter`) : en-tête lu, export accepté, puis le corps
/// devient des `Pixels` et part dans le fil `editeur-sortie`. Rend le numéro de l'export
/// ou le code du refus.
fn exporter_editeur(app: &AppHandle, requete: &tauri::ipc::Request<'_>) -> Result<u64, String> {
    let numero = app
        .state::<Application>()
        .numero_export
        .fetch_add(1, Ordering::SeqCst)
        + 1;
    let entete = |nom: &str| requete.headers().get(nom).and_then(|v| v.to_str().ok());
    let refuser = |id: u64, refus: editeur::Refus| {
        log::info!("{}", editeur::ligne_refus(id, numero, refus));
        Err(refus.code().to_string())
    };
    let e = match editeur::lire_en_tete(
        entete("x-editeur-id"),
        entete("x-editeur-version"),
        entete("x-editeur-but"),
        entete("x-editeur-largeur"),
        entete("x-editeur-hauteur"),
    ) {
        Ok(e) => e,
        Err(r) => {
            let id = avec_editeur(app, None, |e| e.document.id).unwrap_or(0);
            return refuser(id, r);
        }
    };
    let tauri::ipc::InvokeBody::Raw(corps) = requete.body() else {
        return refuser(e.id, editeur::Refus::Taille);
    };
    let maintenant = Instant::now();
    let decision = avec_editeur(app, None, |etat| {
        // Un export trop vieux est rendu ici pour que son résultat d'échec parte : chaque
        // export accepté a exactement un `editeur:resultat`.
        let perime = editeur::export_perime(etat, maintenant);
        let id = etat.document.id;
        let suite = editeur::accepter_export(etat, &e, corps.len(), maintenant).map(|()| {
            let plan = editeur::plan_export(e.but, editeur::sortie_pour(etat, e.but, e.version));
            etat.export = Some(editeur::ExportEnCours {
                numero,
                but: e.but,
                version: e.version,
                debut: maintenant,
                dialogue: plan.is_some_and(|p| p.dialogue),
            });
            (plan, etat.document.transparence, etat.document.maintenant)
        });
        (perime.map(|x| (id, x)), suite)
    });
    let Some((perime, decision)) = decision else {
        return refuser(e.id, editeur::Refus::Id);
    };
    if let Some((id, x)) = perime {
        log::warn!("editeur export {id} {} sans bilan", x.numero);
        fenetres::editeur::emettre(
            app,
            fenetres::editeur::EVENEMENT_RESULTAT,
            editeur::resultat_sans_bilan(id, x.numero, x.but),
        );
    }
    let (plan, transparence, heure) = match decision {
        Ok(d) => d,
        Err(r) => return refuser(e.id, r),
    };
    log::info!(
        "{}",
        editeur::ligne_export(e.id, numero, e.but, e.version, e.largeur, e.hauteur)
    );
    let Some(plan) = plan else {
        // Déjà enregistrée : rien n'est écrit ; le résultat part au tour suivant, après la
        // réponse à la page.
        let fin = editeur::FinExportEditeur {
            id: e.id,
            numero,
            but: e.but,
            version: e.version,
            bilan: None,
            duree: Duration::ZERO,
            deja_fait: true,
        };
        poster_plus_tard(app, Evenement::EditeurExportFait(Box::new(fin)));
        return Ok(numero);
    };
    // Tauri 2 ne prête le corps que par référence (`Request::body`) : une seule copie,
    // exacte, après toutes les vérifications ; ensuite les pixels sont déplacés, jamais
    // recopiés (le fil les prend et les rend dès la sortie faite).
    let pixels = gel::Pixels {
        largeur: e.largeur,
        hauteur: e.hauteur,
        rgba: corps.to_vec(),
    };
    // « Texte » (0.5) : l'image composée est lue, rien n'est exporté (`exportee` reste).
    if e.but == editeur::But::Texte {
        geste_texte::lire_export(app, numero, e, pixels);
        return Ok(numero);
    }
    let base = demande_sortie(app, transparence);
    let demande = sortie::Demande {
        copier: plan.copier,
        enregistrer: plan.enregistrer,
        ranger_recents: plan.ranger_recents,
        maintenant: heure,
        transparence,
        ..base
    };
    let dialogue = plan.dialogue.then(|| {
        (
            fenetres::editeur::identifiant(app),
            demande.dossier_enregistrement.clone(),
            sortie::nom_fichier(&demande.modele_nom, heure),
        )
    });
    lancer_export(app, numero, e, pixels, demande, dialogue);
    Ok(numero)
}

/// Le dialogue d'Enregistrer sous… : propriétaire, dossier de départ, nom proposé.
type Dialogue = (Option<isize>, PathBuf, String);

/// L'export dans le fil `editeur-sortie`, puis `EditeurExportFait`. Un second fil tient
/// l'échéance (`DELAI_SORTIE`, comptée après le dialogue d'Enregistrer sous…) :
/// `EditeurExportEchu` s'il n'y a pas de bilan à temps.
fn lancer_export(
    app: &AppHandle,
    numero: u64,
    e: editeur::EnTete,
    pixels: gel::Pixels,
    demande: sortie::Demande,
    dialogue: Option<Dialogue>,
) {
    let (signal, attente) = mpsc::channel::<()>();
    let pour_fil = app.clone();
    let lance = std::thread::Builder::new()
        .name("editeur-sortie".into())
        .spawn(move || {
            let debut = Instant::now();
            let pour_dialogue = pour_fil.clone();
            // Le dialogue fermé : le fil principal le note sans jamais attendre ce fil.
            let dialogue_ferme = move || {
                let pour = pour_dialogue.clone();
                let _ = pour_dialogue.run_on_main_thread(move || {
                    avec_editeur(&pour, Some(e.id), |x| {
                        x.dialogue_ferme(numero, Instant::now())
                    });
                });
            };
            let bilan = exporter_dans_le_fil(
                &pour_fil,
                (e.id, numero),
                pixels,
                &demande,
                dialogue.map(|d| (d, dialogue_ferme)),
                &signal,
            );
            let fin = Box::new(editeur::FinExportEditeur {
                id: e.id,
                numero,
                but: e.but,
                version: e.version,
                bilan,
                duree: debut.elapsed(),
                deja_fait: false,
            });
            let pour = pour_fil.clone();
            if let Err(err) = pour_fil
                .run_on_main_thread(move || traiter(&pour, Evenement::EditeurExportFait(fin)))
            {
                log::warn!("editeur: bilan non rendu: {err}");
            }
            let _ = signal.send(());
        });
    if let Err(err) = lance {
        log::warn!("editeur: fil d'export impossible ({err})");
        if let Some(id) = avec_editeur(app, Some(e.id), |x| {
            x.export = None;
            x.document.id
        }) {
            log::warn!("editeur export {id} {numero} sans bilan");
            fenetres::editeur::emettre(
                app,
                fenetres::editeur::EVENEMENT_RESULTAT,
                editeur::resultat_sans_bilan(id, numero, e.but),
            );
        }
        return;
    }
    let pour_echeance = app.clone();
    let echeance = std::thread::Builder::new()
        .name("editeur-echeance".into())
        .spawn(move || {
            // Premier signal : la sortie commence (après le dialogue) ; fil arrêté : tout
            // de suite.
            let a_temps =
                attente.recv().is_ok() && attente.recv_timeout(zone::DELAI_SORTIE).is_ok();
            if !a_temps {
                let pour = pour_echeance.clone();
                let _ = pour_echeance.run_on_main_thread(move || {
                    traiter(&pour, Evenement::EditeurExportEchu(numero))
                });
            }
        });
    if let Err(err) = echeance {
        log::warn!("editeur: echeance d'export non armee: {err}");
    }
}

/// Le travail du fil `editeur-sortie` : le dialogue d'Enregistrer sous… s'il le faut,
/// puis `sortir` (copie, fichier au modèle, « Récents »), puis `enregistrer_a`. `signal`
/// part quand la sortie commence. `None` : rien n'a été tenté (dialogue annulé ou déjà
/// ouvert). Les pixels sont rendus avant le retour.
fn exporter_dans_le_fil(
    app: &AppHandle,
    (id, numero): (u64, u64),
    pixels: gel::Pixels,
    demande: &sortie::Demande,
    dialogue: Option<(Dialogue, impl FnOnce())>,
    signal: &Sender<()>,
) -> Option<sortie::Bilan> {
    let mut choisi = None;
    if let Some(((proprietaire, depart, nom), ferme)) = dialogue {
        let resultat = if DIALOGUE_EDITEUR.swap(true, Ordering::SeqCst) {
            log::info!("editeur enregistrer sous {id}: dialogue deja ouvert");
            None
        } else {
            let _garde = GardeDialogueEditeur;
            let depart = Some(depart.as_path()).filter(|d| !d.as_os_str().is_empty());
            Some(systeme::choisir_fichier_png(proprietaire, depart, &nom))
        };
        ferme();
        let _ = signal.send(());
        match resultat {
            None => return None,
            Some(Ok(None)) => {
                log::info!("editeur enregistrer sous {id}: dialogue annule");
                return None;
            }
            Some(Err(e)) => {
                log::warn!("editeur enregistrer sous {id}: dialogue impossible ({e})");
                return Some(sortie::Bilan {
                    copie: sortie::Etape::NonDemandee,
                    recent: None,
                    enregistrement: sortie::EtapeFichier::Echec {
                        dossier: demande.dossier_enregistrement.clone(),
                        cause: e,
                    },
                });
            }
            Some(Ok(Some(chemin))) => choisi = Some(chemin),
        }
    } else {
        let _ = signal.send(());
    }
    let mut bilan = if demande.copier || demande.enregistrer || demande.ranger_recents {
        sortir_ou_echec(numero, &pixels, demande)
    } else {
        sortie::Bilan {
            copie: sortie::Etape::NonDemandee,
            recent: None,
            enregistrement: sortie::EtapeFichier::NonDemande,
        }
    };
    if let Some(chemin) = choisi {
        bilan.enregistrement = sortie::enregistrer_a(&pixels, &chemin, demande.transparence);
    }
    // Une version rangée dans « Récents » : son aperçu, la liste de l'historique à jour.
    geste_historique::apres_sortie(app, &pixels, &bilan);
    drop(pixels);
    Some(bilan)
}

/// Un export fini (fil principal) : journal, état, `editeur:resultat`, puis la fermeture
/// de « Copier et fermer » ou le glisser.
fn finir_export(app: &AppHandle, fin: editeur::FinExportEditeur) {
    let (id, n) = (fin.id, fin.numero);
    if fin.deja_fait {
        log::info!("editeur export {id} {n} deja fait");
    } else if let Some(b) = &fin.bilan {
        log::info!("{}", editeur::ligne_bilan(id, n, b, fin.duree.as_millis()));
    }
    let suite = {
        let garde = app
            .state::<Application>()
            .inner()
            .editeur
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        editeur::suite_export(garde.as_ref(), &fin)
    };
    if suite == editeur::SuiteExport::DocumentParti {
        log::info!("editeur export {id} {n} apres fermeture");
        return;
    }
    let Some((recent, heure)) = avec_editeur(app, Some(id), |e| {
        e.noter_export(&fin);
        (e.recent_de(fin.version).cloned(), e.document.maintenant)
    }) else {
        return;
    };
    if suite == editeur::SuiteExport::Tardif {
        log::info!("editeur export {id} {n} tardif");
        return;
    }
    fenetres::editeur::emettre(
        app,
        fenetres::editeur::EVENEMENT_RESULTAT,
        editeur::resultat(&fin),
    );
    match editeur::apres_export(&fin) {
        editeur::Apres::Fermer => fermer_ou_suivant(app, editeur::RaisonFermeture::CopierFermer),
        editeur::Apres::Glisser => preparer_glisser_editeur(app, id, recent, heure),
        editeur::Apres::GlisserRefuse => log::info!("editeur glisser {id} refuse (recents)"),
        editeur::Apres::Rien => {}
    }
}

/// L'échéance d'un export est tombée sans bilan : l'export est oublié.
fn export_echu(app: &AppHandle, numero: u64) {
    let echu = avec_editeur(app, None, |e| {
        let x = e.export.filter(|x| x.numero == numero)?;
        e.export = None;
        Some((e.document.id, x.but))
    })
    .flatten();
    if let Some((id, but)) = echu {
        log::warn!("editeur export {id} {numero} sans bilan");
        fenetres::editeur::emettre(
            app,
            fenetres::editeur::EVENEMENT_RESULTAT,
            editeur::resultat_sans_bilan(id, numero, but),
        );
    }
}

/// Le fichier à glisser (lien vers « Récents », comme la vignette), puis le glisser est
/// reposté (jamais lancé dans le gestionnaire d'un événement).
fn preparer_glisser_editeur(
    app: &AppHandle,
    id: u64,
    recent: Option<PathBuf>,
    heure: chrono::DateTime<chrono::Local>,
) {
    let glisser = app.state::<Application>().glisser.load(Ordering::SeqCst);
    let Some(recent) = recent.filter(|_| !glisser) else {
        log::info!("editeur glisser {id} indisponible");
        return;
    };
    let chemin = match dossiers_lus(app) {
        Some(d) => {
            let nom = sortie::nom_fichier(&reglages_lus(app).modele_nom, heure);
            let dossier = d.local.join(vignette::SOUS_DOSSIER_GLISSER);
            sortie::fichier_a_glisser(&recent, &dossier, &nom)
        }
        None => Ok(recent),
    };
    match chemin {
        Ok(chemin) => poster_plus_tard(app, Evenement::EditeurGlisser { id, chemin }),
        Err(e) => log::info!("editeur glisser {id} refuse ({e})"),
    }
}

/// Le glisser depuis l'éditeur (fil principal, bloquant : boucle du système), même code
/// que la vignette. L'éditeur reste ouvert ; un dépôt compte comme un export.
fn lancer_glisser_editeur(app: &AppHandle, id: u64, chemin: &std::path::Path) {
    let present = avec_editeur(app, Some(id), |_| ()).is_some();
    let etat = app.state::<Application>();
    let pret = vignette::glisser_pret(
        present && !etat.glisser.load(Ordering::SeqCst),
        fenetres::editeur::identifiant(app),
        systeme::glisser::bouton_principal_enfonce(),
    );
    let fenetre = match pret {
        Ok(f) => f,
        Err(raison) => {
            log::info!("editeur glisser {id} {raison}");
            return;
        }
    };
    let charge = |etat| editeur::ChargeGlisser { id, etat };
    etat.glisser.store(true, Ordering::SeqCst);
    fenetres::editeur::emettre(app, fenetres::editeur::EVENEMENT_GLISSER, charge("debut"));
    log::info!("editeur glisser {id} debut");
    let resultat = systeme::glisser::glisser_fichier(fenetre, chemin);
    etat.glisser.store(false, Ordering::SeqCst);
    let sous = souris_bureau(app).and_then(systeme::glisser::fenetre_sous);
    let a_nous = sous.is_some_and(|h| nos_fenetres(app).contains(&h));
    let issue = vignette::depot(&resultat, a_nous);
    log::info!("{}", editeur::ligne_glisser(id, &issue));
    if issue == vignette::IssueGlisser::Depose {
        avec_editeur(app, Some(id), |e| e.noter_depot());
    }
    fenetres::editeur::emettre(app, fenetres::editeur::EVENEMENT_GLISSER, charge("fini"));
}

/// Arrêt : libère ce qui est tenu, retire l'icône, écrit `arret propre`.
fn arreter(app: &AppHandle) {
    if !app.state::<Application>().demarree.load(Ordering::SeqCst) {
        journal::vider();
        return;
    }
    // Décompte en cours : annulé (fil arrêté, disque caché, Échap rendu), aucune photo.
    annuler_decompte(app, Cause::Arret);
    // Lecture de texte en cours : oubliée (son résultat arrivera trop tard).
    geste_texte::arreter(app);
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
    // Éditeur fermé sans demander (R10), avant la libération des raccourcis : `destroy`
    // ne passe pas par `CloseRequested`, que l'éditeur empêche.
    fermer_editeur(app, editeur::RaisonFermeture::Arret);
    if let Some((d, _)) = app
        .state::<Application>()
        .editeur_suivant
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
    {
        log::info!("editeur: capture {} abandonnee", d.id);
    }
    // Historique : suppressions en attente faites, fenêtre détruite.
    geste_historique::arreter(app);
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
    fn menu_avec_les_quatre_raccourcis() {
        let m = entrees_menu(&ACTIONS_DE_CAPTURE, |a| match a {
            "zone" => Some("Ctrl + Maj + 1".into()),
            "plein_ecran" => Some("Ctrl + Maj + 2".into()),
            "fenetre" => Some("Ctrl + Maj + 3".into()),
            "derniere_zone" => Some("Ctrl + Maj + 4".into()),
            _ => None,
        });
        assert_eq!(
            m,
            [
                action("capturer_zone", "Capturer une zone\tCtrl + Maj + 1"),
                action("capturer_plein_ecran", "Capturer l'écran\tCtrl + Maj + 2"),
                action("capturer_fenetre", "Capturer une fenêtre\tCtrl + Maj + 3"),
                action(
                    "capturer_derniere_zone",
                    "Refaire la dernière zone\tCtrl + Maj + 4"
                ),
                EntreeMenu::Separateur,
                action(MENU_REGLAGES, "Réglages"),
                EntreeMenu::Separateur,
                action(MENU_QUITTER, "Quitter BreezyShot"),
            ]
        );
    }

    #[test]
    fn menu_sans_raccourci() {
        let m = entrees_menu(&ACTIONS_DE_CAPTURE, |_| None);
        assert_eq!(
            m,
            [
                action("capturer_zone", "Capturer une zone"),
                action("capturer_plein_ecran", "Capturer l'écran"),
                action("capturer_fenetre", "Capturer une fenêtre"),
                action("capturer_derniere_zone", "Refaire la dernière zone"),
                EntreeMenu::Separateur,
                action(MENU_REGLAGES, "Réglages"),
                EntreeMenu::Separateur,
                action(MENU_QUITTER, "Quitter BreezyShot"),
            ]
        );
        // Un seul raccourci retenu : les autres entrées restent, sans raccourci.
        let m = entrees_menu(&ACTIONS_DE_CAPTURE, |a| {
            (a == "zone").then(|| "Impr. écran".to_string())
        });
        assert_eq!(
            m[0],
            action("capturer_zone", "Capturer une zone\tImpr. écran")
        );
        assert_eq!(m[1], action("capturer_plein_ecran", "Capturer l'écran"));
        assert_eq!(m[2], action("capturer_fenetre", "Capturer une fenêtre"));
        assert_eq!(
            m[3],
            action("capturer_derniere_zone", "Refaire la dernière zone")
        );
    }

    #[test]
    fn identifiants_du_menu() {
        for mode in Mode::TOUS {
            assert_eq!(mode_du_menu(&id_menu(mode)), Some(mode));
        }
        assert_eq!(mode_du_menu(MENU_QUITTER), None);
        assert_eq!(mode_du_menu("capturer_autre"), None);
        assert_eq!(
            modes_au_menu(&ACTIONS_DE_CAPTURE),
            [
                Mode::Zone,
                Mode::PleinEcran,
                Mode::Fenetre,
                Mode::DerniereZone
            ]
        );
        assert_eq!(modes_au_menu(&actions_selon(true, true, true)), ORDRE_MENU);
        assert_eq!(id_menu(Mode::Texte), "capturer_texte");
        assert_eq!(id_menu(Mode::DerniereZone), "capturer_derniere_zone");
        assert_eq!(texte_capturer(Mode::Fenetre, None), "Capturer une fenêtre");
    }

    #[test]
    fn liste_des_actions_construites() {
        let base = ["zone", "plein_ecran", "fenetre", "derniere_zone"];
        assert_eq!(ACTIONS_DE_CAPTURE, base);
        assert_eq!(actions_selon(true, false, false), base);
        assert_eq!(
            actions_selon(true, true, false),
            [
                "zone",
                "plein_ecran",
                "fenetre",
                "derniere_zone",
                "historique"
            ]
        );
        assert_eq!(
            actions_selon(true, false, true),
            ["zone", "plein_ecran", "fenetre", "derniere_zone", "texte"]
        );
        assert_eq!(
            actions_selon(true, true, true),
            [
                "zone",
                "plein_ecran",
                "fenetre",
                "derniere_zone",
                "historique",
                "texte"
            ]
        );
        // Sans langue de lecture, jamais `texte` ; `historique` suit sa construction.
        assert!(!actions_construites(true, false).contains(&"texte"));
        assert_eq!(
            actions_construites(true, false).contains(&"historique"),
            historique::CONSTRUIT
        );
        assert_eq!(
            actions_construites(true, true).contains(&"texte"),
            texte::CONSTRUIT
        );
        // Sans licence : les quatre captures seulement, quoi qu'il soit construit.
        assert_eq!(actions_selon(false, true, true), base);
        assert_eq!(actions_construites(false, true), base);
        assert_eq!(
            entrees_menu(&actions_construites(false, true), |_| None),
            entrees_menu(&ACTIONS_DE_CAPTURE, |_| None)
        );
        for a in actions_selon(true, true, true) {
            assert!(
                Mode::depuis_nom(a).is_some() || a == ACTION_HISTORIQUE,
                "{a}"
            );
        }
        // `--capturer derniere_zone` est accepté sans code de plus.
        assert_eq!(mode_journal("derniere_zone"), "derniere_zone");
        assert_eq!(mode_journal("derniere"), "<inconnu>");
    }

    #[test]
    fn seules_les_actions_construites_sont_proposees() {
        let r = Reglages::par_defaut(Plateforme::Windows);
        let t = table_candidats(&r, &ACTIONS_DE_CAPTURE);
        assert_eq!(t.len(), 4);
        assert_eq!(t["zone"], ["ImprEcran", "Ctrl+Maj+1"]);
        assert_eq!(t["plein_ecran"], ["Ctrl+Maj+2"]);
        assert_eq!(t["fenetre"], ["Ctrl+Maj+3"]);
        assert_eq!(t["derniere_zone"], ["Ctrl+Maj+4"]);
    }

    /// Ctrl + Maj + 4 pris par une autre application : la dernière zone reste au menu sans
    /// raccourci, l'annonce nomme l'action.
    #[test]
    fn derniere_zone_sans_raccourci() {
        let t = table_candidats(
            &Reglages::par_defaut(Plateforme::Windows),
            &ACTIONS_DE_CAPTURE,
        );
        let b = choisir(&t, false, |r| r.canonique() != "Ctrl+Maj+4");
        assert_eq!(b.libelle_menu("derniere_zone"), None);
        let m = entrees_menu(&ACTIONS_DE_CAPTURE, |a| b.libelle_menu(a));
        assert_eq!(
            m[3],
            action("capturer_derniere_zone", "Refaire la dernière zone")
        );
        assert_eq!(
            b.annonce().map(|a| (a.erreur, a.titre)),
            Some((false, "Aucun raccourci pour la dernière zone".to_string()))
        );
        let tout = choisir(&t, false, |_| true);
        assert_eq!(
            tout.libelle_menu("derniere_zone").as_deref(),
            Some("Ctrl + Maj + 4")
        );
    }

    /// Ctrl + Maj + T et H ne sont pris pour rien, sur aucune plateforme.
    #[test]
    fn raccourcis_reserves_a_plus_tard_jamais_pris() {
        for p in [Plateforme::Windows, Plateforme::Mac, Plateforme::Autre] {
            let t = table_candidats(&Reglages::par_defaut(p), &ACTIONS_DE_CAPTURE);
            let b = choisir(&t, false, |_| true);
            for a in &b.actions {
                let retenu = a.retenu.as_ref().map(|r| r.canonique());
                for interdit in ["Ctrl+Maj+T", "Ctrl+Maj+H"] {
                    assert_ne!(retenu.as_deref(), Some(interdit), "{p:?} {}", a.action);
                }
            }
        }
    }

    /// Ctrl + Maj + 2 pris par une autre application : la zone garde son raccourci, le
    /// plein écran n'en a aucun et reste au menu sans raccourci affiché.
    #[test]
    fn plein_ecran_sans_raccourci() {
        let t = table_candidats(
            &Reglages::par_defaut(Plateforme::Windows),
            &ACTIONS_DE_CAPTURE,
        );
        let b = choisir(&t, false, |r| r.canonique() != "Ctrl+Maj+2");
        assert_eq!(b.libelle_menu("zone").as_deref(), Some("Impr. écran"));
        assert_eq!(b.libelle_menu("plein_ecran"), None);
        let m = entrees_menu(&ACTIONS_DE_CAPTURE, |a| b.libelle_menu(a));
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

    /// Menu de la 0.5 (§2.1, §7.3) : Copier le texte en cinquième, puis l'historique entre
    /// deux séparateurs ; sans langue, pas de Copier le texte.
    #[test]
    fn menu_avec_historique_et_texte() {
        let raccourci = |a: &str| match a {
            "historique" => Some("Ctrl + Maj + H".to_string()),
            "texte" => Some("Ctrl + Maj + T".to_string()),
            _ => None,
        };
        let m = entrees_menu(&actions_selon(true, true, true), raccourci);
        assert_eq!(
            m,
            [
                action("capturer_zone", "Capturer une zone"),
                action("capturer_plein_ecran", "Capturer l'écran"),
                action("capturer_fenetre", "Capturer une fenêtre"),
                action("capturer_derniere_zone", "Refaire la dernière zone"),
                action("capturer_texte", "Copier le texte\tCtrl + Maj + T"),
                EntreeMenu::Separateur,
                action(MENU_HISTORIQUE, "Historique\tCtrl + Maj + H"),
                EntreeMenu::Separateur,
                action(MENU_REGLAGES, "Réglages"),
                EntreeMenu::Separateur,
                action(MENU_QUITTER, "Quitter BreezyShot"),
            ]
        );
        let sans_texte = entrees_menu(&actions_selon(true, true, false), raccourci);
        assert_eq!(sans_texte.len(), m.len() - 1);
        assert!(!sans_texte.contains(&action("capturer_texte", "Copier le texte\tCtrl + Maj + T")));
        assert_eq!(
            sans_texte[5],
            action(MENU_HISTORIQUE, "Historique\tCtrl + Maj + H")
        );
        // Sans historique ni texte : le menu de la 0.4.
        assert_eq!(
            entrees_menu(&actions_selon(true, false, false), raccourci),
            entrees_menu(&ACTIONS_DE_CAPTURE, |_| None)
        );
        assert_eq!(mode_du_menu(MENU_HISTORIQUE), None);
    }

    #[test]
    fn plateformes() {
        assert_eq!(plateforme_reglages(Systeme::Windows), Plateforme::Windows);
        assert_eq!(plateforme_reglages(Systeme::Mac), Plateforme::Mac);
        assert_eq!(plateforme_reglages(Systeme::Autre), Plateforme::Autre);
    }

    #[test]
    fn lignes_d_annonce() {
        let t = table_candidats(
            &Reglages::par_defaut(Plateforme::Windows),
            &ACTIONS_DE_CAPTURE,
        );
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

    fn choix(action: &str, candidats: &[&str], retenu: Option<&str>) -> raccourcis::ChoixAction {
        let retenu = retenu.map(|r| raccourcis::analyser(r).expect("raccourci"));
        raccourcis::ChoixAction {
            action: action.into(),
            retenu,
            // Sans retenu, tous les candidats ont été écartés.
            ecartes: candidats
                .iter()
                .filter(|_| retenu.is_none())
                .map(|c| raccourcis::Ecarte {
                    candidat: (*c).into(),
                    raison: raccourcis::Raison::RefuseParLeSysteme,
                })
                .collect(),
            nb_candidats: candidats.len(),
        }
    }

    fn bilan_du_demarrage() -> Bilan {
        Bilan {
            actions: vec![
                choix("zone", &["Ctrl+Maj+1"], Some("Ctrl+Maj+1")),
                choix("historique", &["Ctrl+Maj+H"], Some("Ctrl+Maj+H")),
            ],
        }
    }

    #[test]
    fn l_annonce_attend_le_texte_seulement_avec_la_licence() {
        assert!(annonce_attend_texte(true));
        assert!(!annonce_attend_texte(false));
        // Avec la licence, l'historique est dans le bilan du démarrage.
        assert!(actions_construites(true, false).contains(&ACTION_HISTORIQUE));
        assert!(!actions_construites(false, false).contains(&ACTION_HISTORIQUE));
    }

    #[test]
    fn texte_sans_raccourci_annonce_par_la_regle_commune() {
        // Tous les candidats pris : « Aucun raccourci pour le texte », une fois.
        let pris = choix("texte", &["Ctrl+Maj+T"], None);
        let b = bilan_avec(bilan_du_demarrage(), &[pris]);
        let d = raccourcis::decider_annonce(None, &b);
        let titre = d.annonce.as_ref().map(|a| a.titre.clone());
        assert_eq!(titre.as_deref(), Some("Aucun raccourci pour le texte"));
        let situation = d.a_retenir.clone().expect("situation");
        assert_eq!(situation.action, "texte");
        // Même situation au démarrage suivant : rien.
        let encore = raccourcis::decider_annonce(Some(&situation), &b);
        assert_eq!(encore.annonce, None);
        assert_eq!(encore.a_retenir, Some(situation));
        // Liste vide (choix de l'utilisateur, mise à niveau) : rien.
        let vide = bilan_avec(bilan_du_demarrage(), &[choix("texte", &[], None)]);
        let d = raccourcis::decider_annonce(None, &vide);
        assert_eq!((d.annonce, d.a_retenir), (None, None));
        // Retenu : rien ; sans le texte (borne) : le bilan seul.
        let ok = choix("texte", &["Ctrl+Maj+T"], Some("Ctrl+Maj+T"));
        let b = bilan_avec(bilan_du_demarrage(), &[ok]);
        assert_eq!(raccourcis::decider_annonce(None, &b).annonce, None);
        assert_eq!(bilan_avec(bilan_du_demarrage(), &[]), bilan_du_demarrage());
    }

    #[test]
    fn historique_sans_raccourci_annonce_comme_les_autres() {
        let mut b = bilan_du_demarrage();
        b.actions[1] = choix("historique", &["Ctrl+Maj+H"], None);
        let d = raccourcis::decider_annonce(None, &b);
        let titre = d.annonce.map(|a| a.titre);
        assert_eq!(titre.as_deref(), Some("Aucun raccourci pour l'historique"));
    }

    #[test]
    fn bilan_avec_remplace_la_ligne_de_l_action() {
        let b = bilan_avec(
            bilan_du_demarrage(),
            &[choix("zone", &["Ctrl+Maj+9"], Some("Ctrl+Maj+9"))],
        );
        assert_eq!(b.actions.len(), 2);
        assert_eq!(b.actions[0].nb_candidats, 1);
        assert_eq!(
            b.retenu("zone").map(|r| r.canonique()),
            Some("Ctrl+Maj+9".to_string())
        );
    }
}
