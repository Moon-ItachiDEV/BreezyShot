//! Les gestes des fichiers `.breezy` (0.7.5, Pro) : enregistrer depuis l'éditeur, rouvrir un
//! fichier, rendre ses annotations à la page. Les décisions sont dans `breezy.rs`, le
//! format dans `sortie/breezy.rs`. Un enregistrement a son propre fil (`editeur-breezy`), à
//! part du chemin des exports de pixels : la page lui envoie le JSON de ses annotations, le
//! cœur y joint les pixels d'origine qu'il tient déjà.
//!
//! Le journal ne montre jamais de chemin, de nom de fichier ni de contenu.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use tauri::{AppHandle, Manager};

use super::breezy::{self, Issue, Refus};
use super::editeur::Document;
use super::{
    afficher, avec_editeur, breezy_ouvert, demande_sortie, poster_plus_tard, recevoir_document,
    Application, Evenement, GardeDialogueEditeur, DIALOGUE_EDITEUR, MENU_LICENCE, VERSION,
};
use crate::fenetres;
use crate::sortie::{self, EtapeFichier};
use crate::systeme;

/// Un entier positif écrit en chiffres ASCII seuls (comme les en-têtes de l'export).
fn entier(texte: Option<&str>) -> Option<u64> {
    let t = texte?;
    if t.is_empty() || t.len() > 20 || !t.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    t.parse().ok()
}

/// `editeur_breezy_enregistrer` : en-têtes `x-editeur-id` et `-version`, corps brut = le
/// JSON des annotations. Rend le numéro, ou le code du refus (`breezy::Refus::code`). Sans
/// licence : Réglages › Licence s'ouvre, rien n'est écrit.
pub fn enregistrer(app: &AppHandle, requete: &tauri::ipc::Request<'_>) -> Result<u64, String> {
    let entete = |nom: &str| requete.headers().get(nom).and_then(|v| v.to_str().ok());
    let (Some(id), Some(version)) = (
        entier(entete("x-editeur-id")),
        entier(entete("x-editeur-version")),
    ) else {
        log::info!("{}", breezy::ligne_refus(0, Refus::Id));
        return Err(Refus::Id.code().into());
    };
    let tauri::ipc::InvokeBody::Raw(corps) = requete.body() else {
        log::info!("{}", breezy::ligne_refus(id, Refus::Taille));
        return Err(Refus::Taille.code().into());
    };
    let pro = breezy_ouvert(app);
    let maintenant = Instant::now();
    let decision = avec_editeur(app, None, |e| {
        breezy::accepter(e, id, corps.len(), pro, maintenant).map(|()| {
            e.breezy_en_cours = Some(maintenant);
            (
                Arc::clone(&e.document.pixels),
                e.document.transparence,
                e.document.breezy.clone(),
            )
        })
    });
    let (pixels, transparence, origine) = match decision {
        None => {
            log::info!("{}", breezy::ligne_refus(id, Refus::Id));
            return Err(Refus::Id.code().into());
        }
        Some(Err(Refus::Pro)) => {
            log::info!("{}", breezy::ligne_refus(id, Refus::Pro));
            poster_plus_tard(app, Evenement::Menu(MENU_LICENCE.into()));
            return Err(Refus::Pro.code().into());
        }
        Some(Err(r)) => {
            log::info!("{}", breezy::ligne_refus(id, r));
            return Err(r.code().into());
        }
        Some(Ok(d)) => d,
    };
    let numero = app
        .state::<Application>()
        .numero_export
        .fetch_add(1, Ordering::SeqCst)
        + 1;
    log::info!(
        "editeur breezy {id} {numero} version {version} {}x{}",
        pixels.largeur,
        pixels.hauteur
    );
    let annotations = corps.to_vec();
    let demande = demande_sortie(app, transparence);
    let nom = breezy::nom_propose(
        origine.as_deref(),
        &sortie::nom_fichier(&demande.modele_nom, demande.maintenant),
    );
    let depart = breezy::dossier_depart(origine.as_deref(), &demande.dossier_enregistrement, |d| {
        d.is_dir()
    });
    let proprietaire = fenetres::editeur::identifiant(app);
    let pour_fil = app.clone();
    let lance = std::thread::Builder::new()
        .name("editeur-breezy".into())
        .spawn(move || {
            let debut = Instant::now();
            // Une panique du fil rend le drapeau et la page : jamais un éditeur bloqué.
            let issue = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                travail(
                    (&pixels, transparence),
                    &annotations,
                    proprietaire,
                    &depart,
                    &nom,
                )
            }))
            .unwrap_or_else(|_| {
                log::warn!("editeur breezy {id}: panique du fil");
                (Issue::Echec, None)
            });
            drop(pixels);
            let pour = pour_fil.clone();
            if let Err(e) = pour_fil.run_on_main_thread(move || {
                fin(&pour, id, version, issue, debut.elapsed().as_millis())
            }) {
                log::warn!("editeur breezy {id}: bilan non rendu: {e}");
            }
        });
    if let Err(e) = lance {
        log::warn!("editeur breezy {id}: fil impossible ({e})");
        fin(app, id, version, (Issue::Echec, None), 0);
    }
    Ok(numero)
}

/// Le travail du fil : le dialogue (un seul à la fois avec « Enregistrer sous… » des PNG),
/// puis l'écriture. Rend l'issue et le fichier écrit.
fn travail(
    (pixels, transparence): (&systeme::gel::Pixels, bool),
    annotations: &[u8],
    proprietaire: Option<isize>,
    depart: &Path,
    nom: &str,
) -> (Issue, Option<PathBuf>) {
    let choisi = if DIALOGUE_EDITEUR.swap(true, Ordering::SeqCst) {
        log::info!("editeur breezy: dialogue deja ouvert");
        return (Issue::Annule, None);
    } else {
        let _garde = GardeDialogueEditeur;
        let depart = Some(depart).filter(|d| !d.as_os_str().is_empty());
        systeme::choisir_fichier_breezy(proprietaire, depart, nom)
    };
    let chemin = match choisi {
        Ok(Some(c)) => c,
        Ok(None) => {
            log::info!("editeur breezy: dialogue annule");
            return (Issue::Annule, None);
        }
        Err(e) => {
            log::warn!("editeur breezy: dialogue impossible ({e})");
            return (Issue::Echec, None);
        }
    };
    let cree = chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false);
    match sortie::enregistrer_breezy_a(pixels, transparence, annotations, &chemin, VERSION, &cree) {
        EtapeFichier::Fait(c) => (Issue::Fait, Some(c)),
        EtapeFichier::Echec { .. } => {
            log::warn!("editeur breezy: ecriture impossible");
            (Issue::Echec, None)
        }
        _ => (Issue::Echec, None),
    }
}

/// La fin d'un enregistrement (fil principal) : l'état du document, le journal, puis
/// `editeur:breezy`. Le document parti entre-temps : le journal seul.
fn fin(
    app: &AppHandle,
    id: u64,
    version: u64,
    (issue, chemin): (Issue, Option<PathBuf>),
    ms: u128,
) {
    let reste = avec_editeur(app, Some(id), |e| {
        e.breezy_en_cours = None;
        if issue == Issue::Fait {
            // La version enregistrée est propre : fermer ne demande plus rien. Le nom du
            // fichier sert à la proposition suivante.
            e.exportee = Some(version);
            e.document.breezy = chemin;
        }
    });
    log::info!("{}", breezy::ligne_fin(id, version, issue, ms));
    if reste.is_none() {
        log::info!("editeur breezy {id} apres fermeture");
        return;
    }
    fenetres::editeur::emettre(
        app,
        fenetres::editeur::EVENEMENT_BREEZY,
        breezy::charge(id, version, issue),
    );
}

/// `editeur_annotations` : le JSON du fichier rouvert, en octets. Refusé pour un autre
/// document ou une capture neuve.
pub fn annotations(app: &AppHandle, id: u64) -> Result<tauri::ipc::Response, String> {
    match avec_editeur(app, Some(id), |e| e.document.annotations.clone()).flatten() {
        Some(texte) => Ok(tauri::ipc::Response::new(texte.as_bytes().to_vec())),
        None => {
            log::info!("editeur annotations {id} ignoree");
            Err("id".into())
        }
    }
}

/// Rouvre un fichier `.breezy` dans l'éditeur (double-clic, ligne de commande, seconde
/// instance). Sans licence : Réglages › Licence s'ouvre, le fichier n'est pas lu. La
/// lecture se fait dans un fil ; le document suit le chemin d'une capture
/// (`recevoir_document` : ouvrir, remplacer, ou demander si l'éditeur a des modifications).
pub fn ouvrir(app: &AppHandle, chemin: String) {
    if !breezy_ouvert(app) {
        log::info!("breezy ouverture refusee (licence)");
        poster_plus_tard(app, Evenement::Menu(MENU_LICENCE.into()));
        return;
    }
    let demande_a = Instant::now();
    log::info!("breezy ouverture demandee");
    let pour_fil = app.clone();
    let lance = std::thread::Builder::new()
        .name("breezy-lecture".into())
        .spawn(move || {
            let lu = sortie::lire_breezy_a(Path::new(&chemin));
            let pour = pour_fil.clone();
            let poste = match lu {
                Ok(contenu) => {
                    let id = pour_fil
                        .state::<Application>()
                        .derniere_sortie
                        .fetch_add(1, Ordering::SeqCst)
                        + 1;
                    let document = Document {
                        id,
                        pixels: Arc::new(contenu.pixels),
                        transparence: contenu.transparence,
                        maintenant: chrono::Local::now(),
                        annotations: Some(Arc::new(contenu.annotations)),
                        breezy: Some(PathBuf::from(chemin)),
                    };
                    pour_fil.run_on_main_thread(move || {
                        log::info!("breezy ouvert {id}");
                        recevoir_document(&pour, document, demande_a)
                    })
                }
                Err(erreur) => {
                    log::warn!("{}", breezy::ligne_ouverture_refusee(erreur));
                    pour_fil.run_on_main_thread(move || {
                        afficher(&pour, breezy::message_ouverture(erreur));
                    })
                }
            };
            if let Err(e) = poste {
                log::warn!("breezy ouverture: poste impossible ({e})");
            }
        });
    if let Err(e) = lance {
        log::warn!("breezy ouverture: fil impossible ({e})");
    }
}
