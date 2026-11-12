//! La fenêtre de l'installateur (ARCHITECTURE §12.3). Sans cadre système, transparente (la page
//! dessine la carte, son ombre et ses coins), elle prend le focus et paraît dans la barre
//! des tâches. Elle montre, elle rapporte ; le cœur (`application/installateur.rs`) décide.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::ThreadId;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use super::ARGUMENTS_NAVIGATEUR;
use crate::systeme;

pub const ETIQUETTE_INSTALLATEUR: &str = "installateur";
/// Titre de la fenêtre (barre des tâches), par lequel une seconde ouverture la retrouve.
pub const TITRE_INSTALLATEUR: &str = "Installer BreezyShot";
const EVENEMENT_ETAT: &str = "installateur:etat";
const EVENEMENT_PROGRESSION: &str = "installateur:progression";
const EVENEMENT_ENVOL: &str = "installateur:envol";
const EVENEMENT_FERMER: &str = "installateur:fermer";

/// Ce que la page rapporte au cœur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    /// La page a posé ses écoutes : un message en attente peut partir.
    Pret,
    /// Un bouton a été pressé (nom de l'action).
    Action(String),
    /// Alt+F4 ou la croix système : à traiter comme l'action « fermer ».
    FermetureDemandee,
}

/// Charge de `installateur:etat`.
#[derive(Debug, Clone, Serialize)]
pub struct EtatPage {
    /// `accueil`, `installation`, `erreur`, `pret`.
    pub etat: &'static str,
    /// Cause (état `erreur`), sinon vide.
    pub cause: String,
    /// Taille réelle de l'exe (octets), pour la ligne « Mo ».
    pub total: u64,
    /// `pret` atteint sans passer par la progression (plan « plus récente installée »).
    pub direct: bool,
}

#[derive(Debug, Clone, Serialize)]
struct Progression {
    octets: u64,
    total: u64,
}

#[derive(Debug, Clone, Serialize)]
struct Envol {
    duree: u64,
}

type Rappel = Box<dyn Fn(Signal) + Send + Sync + 'static>;

struct Interne {
    app: AppHandle,
    rappel: Rappel,
    fil_principal: ThreadId,
    vivante: AtomicBool,
    identifiant: Option<isize>,
}

/// La fenêtre de l'installateur.
pub struct Installateur {
    interne: Arc<Interne>,
}

impl Installateur {
    /// Crée la fenêtre cachée et charge la page (fil principal). `data_directory` est le
    /// dossier WebView2 propre à l'installateur, pour ne jamais partager celui de l'app.
    pub fn creer<F>(
        app: &AppHandle,
        webview: &std::path::Path,
        largeur: f64,
        hauteur: f64,
        rappel: F,
    ) -> Result<Installateur, String>
    where
        F: Fn(Signal) + Send + Sync + 'static,
    {
        let fenetre = WebviewWindowBuilder::new(
            app,
            ETIQUETTE_INSTALLATEUR,
            WebviewUrl::App("installateur/installateur.html".into()),
        )
        .title(TITRE_INSTALLATEUR)
        .inner_size(largeur, hauteur)
        .visible(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .maximizable(false)
        .center()
        .data_directory(webview.to_path_buf())
        .additional_browser_args(ARGUMENTS_NAVIGATEUR)
        .build()
        .map_err(|e| format!("fenetre de l'installateur: {e}"))?;
        let identifiant = systeme::identifiant_fenetre(&fenetre);
        if let Some(h) = identifiant {
            log::info!("installateur fenetre {h}");
        }
        let interne = Arc::new(Interne {
            app: app.clone(),
            rappel: Box::new(rappel),
            fil_principal: std::thread::current().id(),
            vivante: AtomicBool::new(true),
            identifiant,
        });
        let pour_evenements = Arc::clone(&interne);
        fenetre.on_window_event(move |e| match e {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                // La fenêtre n'est détruite que par `fermer` : Alt+F4 devient « fermer ».
                api.prevent_close();
                (pour_evenements.rappel)(Signal::FermetureDemandee);
            }
            tauri::WindowEvent::Destroyed => {
                pour_evenements.vivante.store(false, Ordering::SeqCst);
            }
            _ => {}
        });
        Ok(Installateur { interne })
    }

    fn fenetre(&self) -> Option<WebviewWindow> {
        self.interne.app.get_webview_window(ETIQUETTE_INSTALLATEUR)
    }

    /// L'identifiant système (HWND), pour déplacer la fenêtre pendant l'envol.
    pub fn identifiant(&self) -> Option<isize> {
        self.interne.identifiant
    }

    /// Pose la fenêtre et sa taille (pixels réels), avant de la montrer.
    pub fn poser(&self, x: i32, y: i32, largeur: i32, hauteur: i32) {
        if let Some(f) = self.fenetre() {
            if largeur > 0 && hauteur > 0 {
                let _ = f.set_size(tauri::PhysicalSize::new(largeur as u32, hauteur as u32));
            }
            let _ = f.set_position(tauri::PhysicalPosition::new(x, y));
        }
    }

    /// Montre la fenêtre et lui donne le focus (fil principal).
    pub fn montrer(&self) {
        if let Some(f) = self.fenetre() {
            let _ = f.show();
            let _ = f.set_focus();
        }
    }

    /// Envoie l'état à la page.
    pub fn envoyer(&self, etat: EtatPage) {
        self.emettre(EVENEMENT_ETAT, etat);
    }

    /// Envoie l'avancement de la copie.
    pub fn progression(&self, octets: u64, total: u64) {
        self.emettre(EVENEMENT_PROGRESSION, Progression { octets, total });
    }

    /// Demande à la page de réduire la carte pendant que le cœur déplace la fenêtre.
    pub fn envol(&self, duree: u64) {
        self.emettre(EVENEMENT_ENVOL, Envol { duree });
    }

    /// Demande à la page le fondu de fermeture.
    pub fn fermer_en_fondu(&self) {
        self.emettre(EVENEMENT_FERMER, ());
    }

    fn emettre<S: Serialize + Clone>(&self, evenement: &str, charge: S) {
        if let Err(e) = self
            .interne
            .app
            .emit_to(ETIQUETTE_INSTALLATEUR, evenement, charge)
        {
            log::warn!("installateur: {evenement} non emis: {e}");
        }
    }

    /// Cache la fenêtre tout de suite (fermeture pendant l'installation).
    pub fn cacher(&self) {
        if let Some(f) = self.fenetre() {
            let _ = f.hide();
        }
    }

    /// Réduit la fenêtre dans la barre des tâches.
    pub fn reduire(&self) {
        if let Some(f) = self.fenetre() {
            let _ = f.minimize();
        }
    }

    /// Détruit la fenêtre (avant de sortir).
    pub fn fermer(&self) {
        self.interne.vivante.store(false, Ordering::SeqCst);
        if let Some(f) = self.fenetre() {
            let _ = f.destroy();
        }
    }
}

/// La page a posé ses écoutes.
#[tauri::command]
pub fn installateur_pret(installateur: State<'_, Installateur>) {
    debug_assert_eq!(
        std::thread::current().id(),
        installateur.interne.fil_principal
    );
    (installateur.interne.rappel)(Signal::Pret);
}

/// Un bouton a été pressé.
#[tauri::command]
pub fn installateur_action(installateur: State<'_, Installateur>, action: String) {
    (installateur.interne.rappel)(Signal::Action(action));
}
