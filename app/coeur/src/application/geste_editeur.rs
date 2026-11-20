//! Les gestes de l'éditeur d'annotation (contrat 0.4 « Éditeur », §4 à §7). Les décisions
//! sont dans `editeur.rs`, les commandes IPC dans `fenetre_editeur.rs` ; ici, seulement les
//! gestes : cycle de vie de la fenêtre, ouverture/remplacement, export dans le fil
//! `editeur-sortie`, dialogue « Enregistrer sous… », et glisser-déposer.
//!
//! Fil principal ; le verrou de l'éditeur n'est jamais tenu pendant un appel à Tauri ou à
//! un module.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use super::editeur;
use super::etats::Declencheur;
use super::{
    afficher, appliquer, breezy_ouvert, demande_sortie, dossiers_lus, geste_epingle,
    geste_historique, geste_texte, montree, nos_fenetres, plus_tard, plus_tard_nomme,
    poster_plus_tard, reglages_lus, sortir_ou_echec, souris_bureau, texte_construit, traiter,
    vignette, zone, Application, CaptureMontree, Evenement,
};
use crate::fenetres::vignette::Vignette;
use crate::fenetres::{self};
use crate::sortie;
use crate::systeme::{self, gel};

/// Vrai tant qu'un dialogue « Enregistrer sous » de l'éditeur est ouvert.
pub(super) static DIALOGUE_EDITEUR: AtomicBool = AtomicBool::new(false);

/// Rend `DIALOGUE_EDITEUR` à la fin du dialogue, quoi qu'il arrive.
pub(super) struct GardeDialogueEditeur;
impl Drop for GardeDialogueEditeur {
    fn drop(&mut self) {
        DIALOGUE_EDITEUR.store(false, Ordering::SeqCst);
    }
}

/// Écran supposé si le système n'en décrit aucun.
pub(super) const ECRAN_DE_SECOURS: fenetres::Ecran = fenetres::Ecran {
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
pub(super) fn reduites_editeur(app: &AppHandle) -> bool {
    vignette::animations_reduites(reglages_lus(app).animations, systeme::animations_reduites())
}

/// Les préférences d'outil de la session.
pub(super) fn preferences_editeur(app: &AppHandle) -> Option<String> {
    app.state::<Application>()
        .preferences_editeur
        .lock()
        .ok()
        .and_then(|p| p.clone())
}

/// Garde les préférences envoyées par la page (`editeur::garder_preferences`).
pub(super) fn garder_preferences_editeur(app: &AppHandle, nouveau: Option<String>) {
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
pub(super) fn annoter(app: &AppHandle, id: u64) {
    let demande_a = Instant::now();
    // Un film ne s'ouvre jamais dans l'éditeur (F24).
    let Some(c) = montree(app).take_if(|c| c.id == id && c.contenu.film().is_none()) else {
        return;
    };
    let CaptureMontree {
        contenu,
        transparence,
        maintenant,
        ..
    } = c;
    let pixels = Arc::clone(contenu.pixels());
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
        annotations: None,
        breezy: None,
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
pub(super) fn desarmer_filet_page(app: &AppHandle) {
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
    let breezy = breezy_ouvert(app);
    if let Some(c) = avec_editeur(app, None, |e| {
        editeur::charge(e, reduites, preferences, texte, breezy)
    }) {
        fenetres::editeur::emettre(app, fenetres::editeur::EVENEMENT_CHARGER, c);
    }
}

/// Ferme l'éditeur sans demander : état retiré, document rendu (journal), fenêtre
/// détruite (jamais cachée : la mémoire du moteur de rendu est rendue). Une capture en
/// attente s'ouvre ensuite dans une nouvelle fenêtre, sauf à l'arrêt.
pub(super) fn fermer_editeur(app: &AppHandle, raison: editeur::RaisonFermeture) {
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
pub(super) fn fermer_ou_suivant(app: &AppHandle, raison: editeur::RaisonFermeture) {
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
pub(super) fn editeur_detruit(app: &AppHandle) {
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
pub(super) fn fermeture_du_cadre(app: &AppHandle) {
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
pub(super) fn exporter_editeur(
    app: &AppHandle,
    requete: &tauri::ipc::Request<'_>,
) -> Result<u64, String> {
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
    // « Épingler » : l'image composée s'ouvre dans une fenêtre flottante, rien n'est exporté.
    if e.but == editeur::But::Epingler {
        avec_editeur(app, Some(e.id), |x| {
            if x.export.is_some_and(|ex| ex.numero == numero) {
                x.export = None;
            }
        });
        let ok = geste_epingle::ouvrir(app, Arc::new(pixels), transparence);
        let charge = editeur::ChargeResultat {
            id: e.id,
            numero,
            but: e.but.nom(),
            etat: if ok { "fait" } else { "echec" },
            message: None,
        };
        plus_tard_nomme(app, "editeur-epingle", Duration::ZERO, move |a| {
            fenetres::editeur::emettre(a, fenetres::editeur::EVENEMENT_RESULTAT, charge);
        });
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
pub(super) fn finir_export(app: &AppHandle, fin: editeur::FinExportEditeur) {
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
pub(super) fn export_echu(app: &AppHandle, numero: u64) {
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
pub(super) fn lancer_glisser_editeur(app: &AppHandle, id: u64, chemin: &Path) {
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
