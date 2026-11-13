//! La fenêtre des Réglages, côté Application (étape 10) : les commandes de la page et
//! leurs gestes. Les décisions sont dans `page_reglages.rs` (fonctions pures).
//!
//! Chaque changement : relire le fichier (une modification extérieure n'est jamais
//! écrasée), appliquer et valider, écrire par le module `reglages`, garder en mémoire
//! (la capture suivante lit la mémoire), puis les effets (thème, démarrage, menu).
//! Si le fichier ne peut pas être écrit, le changement vaut pour la session et la page
//! le dit en une ligne.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use super::page_reglages::{
    self as page, Changement, Contexte, Refus, Relecture, Saisie, VueReglages,
    ALERTE_NON_ENREGISTRES,
};
use super::{
    actions_en_vigueur, plateforme_reglages, plus_tard_nomme, pro,
    refaire_menu as refaire_menu_actuel, zone, Application, VERSION, VIGNETTE_CONSTRUITE,
};
use crate::fenetres::{reglages as fenetre, theme};
use crate::raccourcis::{self, Raison};
use crate::reglages::{self, Raccourcis, Reglages};
use crate::systeme::gel;
use crate::systeme::{self, EtatDemarrage};

/// La saisie d'un raccourci rend les raccourcis d'office après ce délai.
pub const DELAI_SAISIE: Duration = Duration::from_secs(15);

/// Vrai tant qu'un sélecteur de dossier est ouvert : un second « Parcourir… » est ignoré.
static DIALOGUE_OUVERT: AtomicBool = AtomicBool::new(false);

/// Rend `DIALOGUE_OUVERT` à la fin du dialogue, quoi qu'il arrive.
struct GardeDialogue;
impl Drop for GardeDialogue {
    fn drop(&mut self) {
        DIALOGUE_OUVERT.store(false, Ordering::SeqCst);
    }
}

/// La réponse de chaque commande : l'état complet, et le refus éventuel.
#[derive(Debug, Clone, Serialize)]
pub struct Reponse {
    pub vue: VueReglages,
    pub refus: Option<Refus>,
}

/// Ouvre la fenêtre, ou la ramène devant (menu de l'icône, fil principal).
pub fn ouvrir(app: &AppHandle) {
    let pour_fermeture = app.clone();
    match fenetre::ouvrir(app, move || fermee(&pour_fermeture)) {
        Ok(fenetre::Ouverture::Creee { identifiant }) => {
            match identifiant {
                Some(h) => log::info!("reglages fenetre {h}"),
                None => log::info!("reglages fenetre ouverte"),
            }
            let etat = systeme::demarrage_avec_systeme(None).unwrap_or(EtatDemarrage::Indisponible);
            log::info!("demarrage avec le systeme: {}", etat.nom());
        }
        Ok(fenetre::Ouverture::RameneeDevant) => log::info!("reglages ramenes devant"),
        Err(e) => log::warn!("reglages non ouverts: {e}"),
    }
}

/// La fenêtre a été détruite : la saisie éventuelle est annulée, les refus oubliés, et la
/// prochaine page écrira de nouveau `reglages page prete`.
fn fermee(app: &AppHandle) {
    log::info!("reglages fermes");
    etat(app).page_reglages_prete.store(false, Ordering::SeqCst);
    annuler_saisie(app, "raccourcis repris");
}

fn etat(app: &AppHandle) -> tauri::State<'_, Application> {
    app.state::<Application>()
}

/// Les réglages en mémoire (ceux que lit la capture).
fn memoire(app: &AppHandle) -> Reglages {
    etat(app)
        .reglages
        .lock()
        .ok()
        .and_then(|r| r.clone())
        .unwrap_or_else(|| Reglages::par_defaut(plateforme_reglages(systeme::systeme())))
}

fn garder(app: &AppHandle, r: Reglages) {
    if let Ok(mut m) = etat(app).reglages.lock() {
        *m = Some(r);
    }
}

fn dossier_reglages(app: &AppHandle) -> Option<std::path::PathBuf> {
    etat(app)
        .dossiers
        .lock()
        .ok()
        .and_then(|d| d.as_ref().map(|d| d.reglages.clone()))
}

fn images(app: &AppHandle) -> Option<std::path::PathBuf> {
    etat(app)
        .dossiers
        .lock()
        .ok()
        .and_then(|d| d.as_ref().and_then(|d| d.images.clone()))
}

fn reservations(app: &AppHandle) -> Option<&raccourcis::Reservations> {
    app.try_state::<Application>()
        .and_then(|e| e.inner().reservations.get())
}

/// Relit le fichier. S'il a changé depuis la dernière lecture ou écriture faite par l'app
/// (modification extérieure), il devient la base, et ses raccourcis changés sont repris.
/// Sinon, ou s'il n'a pas pu être lu (valeurs protégées), la base reste la mémoire (qui
/// peut porter des changements de la session non écrits). Décision : `page::relecture`.
/// Rend la base.
fn relire(app: &AppHandle) -> Reglages {
    let Some(dossier) = dossier_reglages(app) else {
        return memoire(app);
    };
    let plateforme = plateforme_reglages(systeme::systeme());
    let (relu, constats) = reglages::charger(&dossier, plateforme);
    for c in &constats {
        log::info!("reglages relus: {}", c.code());
    }
    let derniere = etat(app)
        .derniere_lecture
        .lock()
        .ok()
        .and_then(|d| d.clone());
    if page::relecture(&relu, derniere.as_ref()) == Relecture::Garder {
        return memoire(app);
    }
    log::info!("reglages relus: changes exterieurs");
    let avant = memoire(app);
    if let Ok(mut d) = etat(app).derniere_lecture.lock() {
        *d = Some(relu.clone());
    }
    for action in actions_en_vigueur(app) {
        let nouveaux = relu.raccourcis.candidats(action).unwrap_or_default();
        if avant.raccourcis.candidats(action).unwrap_or_default() != nouveaux {
            changer_raccourci(app, action, nouveaux);
        }
    }
    garder(app, relu.clone());
    appliquer_theme(app, &relu);
    relu
}

/// Écrit les réglages et les garde en mémoire, écrits ou non.
fn ecrire_et_garder(app: &AppHandle, r: Reglages) {
    let ecrit = match dossier_reglages(app) {
        Some(d) => reglages::ecrire(&d, &r).map_err(|e| e.code()),
        None => Err("dossier_introuvable"),
    };
    let alerte = match ecrit {
        Ok(()) => {
            log::info!("reglages ecrits");
            if let Ok(mut d) = etat(app).derniere_lecture.lock() {
                *d = Some(r.clone());
            }
            None
        }
        Err(code) => {
            log::warn!("reglages non ecrits: {code}");
            Some(ALERTE_NON_ENREGISTRES.to_string())
        }
    };
    if let Ok(mut a) = etat(app).alerte_reglages.lock() {
        *a = alerte;
    }
    garder(app, r);
}

fn appliquer_theme(app: &AppHandle, r: &Reglages) {
    let voulu = page::theme_page(r.theme);
    if theme::actuel() != voulu {
        theme::fixer(app, voulu);
        log::info!("theme interface: {}", r.theme.texte());
    }
}

/// La vue actuelle, avec le démarrage relu dans le système.
fn vue(app: &AppHandle) -> VueReglages {
    let r = memoire(app);
    let bilan = reservations(app).map(|x| x.bilan()).unwrap_or_default();
    let refus = etat(app)
        .refus_raccourcis
        .lock()
        .map(|r| r.clone())
        .unwrap_or_default();
    let defaut = Raccourcis::par_defaut(plateforme_reglages(systeme::systeme()));
    let images = images(app);
    let (_, dossier_affiche) =
        zone::dossier_enregistrement(r.dossier_enregistrement.as_deref(), images.as_deref());
    let alerte = etat(app)
        .alerte_reglages
        .lock()
        .ok()
        .and_then(|a| a.clone());
    let contexte = Contexte {
        vignette_construite: VIGNETTE_CONSTRUITE,
        capture_disponible: gel::disponible(),
        actions: &actions_en_vigueur(app),
        bilan: &bilan,
        defaut: &defaut,
        refus_raccourcis: &refus,
        dossier_affiche,
        demarrage: systeme::demarrage_avec_systeme(None).unwrap_or(EtatDemarrage::Indisponible),
        licence_active: pro(app),
        version: VERSION,
        alerte,
        maintenant: chrono::Local::now(),
        saisie_en_cours: etat(app)
            .saisie
            .lock()
            .map(|s| s.is_some())
            .unwrap_or(false),
    };
    page::vue(&r, &contexte)
}

/// Vrai si les raccourcis et le menu tiennent déjà la liste des actions en vigueur.
fn listes_a_jour(app: &AppHandle) -> bool {
    let suivies = etat(app)
        .actions_suivies
        .lock()
        .map(|a| a.clone())
        .unwrap_or_default();
    suivies == actions_en_vigueur(app)
}

/// Combien de tours on attend `Evenement::LicenceChangee` avant de renoncer.
const TOURS_LISTES: u8 = 3;

/// Après « Activer » ou « Retirer », la page redemande sa vue (`reglages_lire`) alors que
/// `Evenement::LicenceChangee`, posté au tour suivant, n'a peut-être pas encore pris ou
/// rendu les raccourcis : une ligne nouvelle dirait « Aucun raccourci ». Tant que les
/// listes ne suivent pas la licence, la vue est renvoyée au tour suivant (`reglages:etat`),
/// `TOURS_LISTES` fois au plus. Aucune fenêtre n'est créée ici.
fn renvoyer_quand_les_listes_suivent(app: &AppHandle, tours: u8) {
    if listes_a_jour(app) || tours == 0 {
        return;
    }
    plus_tard_nomme(app, "reglages-poste", Duration::ZERO, move |app| {
        if listes_a_jour(app) {
            fenetre::emettre_etat(app, vue(app));
        } else {
            renvoyer_quand_les_listes_suivent(app, tours - 1);
        }
    });
}

fn reponse(app: &AppHandle, refus: Option<Refus>) -> Reponse {
    Reponse {
        vue: vue(app),
        refus,
    }
}

fn refuser(app: &AppHandle, refus: Refus) -> Reponse {
    log::info!("reglage refuse: {} ({})", refus.ligne, refus.code);
    reponse(app, Some(refus))
}

fn refaire_menu(app: &AppHandle) {
    refaire_menu_actuel(app);
}

fn noter_refus(app: &AppHandle, action: &str, texte: Option<String>) {
    if let Ok(mut r) = etat(app).refus_raccourcis.lock() {
        match texte {
            Some(t) => r.insert(action.to_string(), t),
            None => r.remove(action),
        };
    }
}

/// Essaie de donner `candidats` à l'action ; l'ancien raccourci reste tenu en cas d'échec.
/// Rend vrai si un candidat est tenu.
fn changer_raccourci(app: &AppHandle, action: &str, candidats: &[String]) -> bool {
    let Some(r) = reservations(app) else {
        return false;
    };
    let choix = r.changer(app, action, candidats, systeme::impr_ecran_reserve());
    if candidats.is_empty() {
        // Liste vide (écrite à la main) : « pas de raccourci voulu », jamais un refus.
        log::info!("raccourci {action} change: aucun");
        noter_refus(app, action, None);
        refaire_menu(app);
        return true;
    }
    match choix.retenu {
        Some(x) => {
            log::info!("raccourci {action} change: {}", x.canonique());
            noter_refus(app, action, None);
            refaire_menu(app);
            true
        }
        None => {
            let raison = choix
                .ecartes
                .last()
                .map(|e| e.raison.clone())
                .unwrap_or(Raison::RefuseParLeSysteme);
            for e in &choix.ecartes {
                log::info!(
                    "raccourci {action} refuse: {} ({})",
                    e.candidat.trim(),
                    raccourcis::texte_raison(&e.raison)
                );
            }
            noter_refus(app, action, Some(page::texte_refus(&raison)));
            false
        }
    }
}

/// Annule une saisie (Échap, perte du focus, filet, fermeture) : les refus affichés sont
/// oubliés, puis les raccourcis repris (une perte à la reprise reste dite).
fn annuler_saisie(app: &AppHandle, ligne: &str) {
    if let Ok(mut r) = etat(app).refus_raccourcis.lock() {
        r.clear();
    }
    finir_saisie(app, ligne);
}

/// Finit une saisie en cours (raccourcis repris), avec la ligne de journal donnée.
fn finir_saisie(app: &AppHandle, ligne: &str) {
    if let Ok(mut s) = etat(app).saisie.lock() {
        *s = None;
    }
    // À l'arrêt, les raccourcis sont déjà libérés (ou vont l'être) : ne rien reprendre.
    if !etat(app).raccourcis_reserves.load(Ordering::SeqCst) {
        return;
    }
    let Some(r) = reservations(app) else {
        return;
    };
    let avant = r.bilan();
    if r.reprendre(app) {
        log::info!("{ligne}");
        let apres = r.bilan();
        for a in &apres.actions {
            if a.retenu.is_none() && avant.retenu(&a.action).is_some() {
                log::warn!("raccourci {} perdu a la reprise", a.action);
                noter_refus(
                    app,
                    &a.action,
                    Some(page::texte_refus(&Raison::RefuseParLeSysteme)),
                );
            }
        }
        if apres != avant {
            refaire_menu(app);
        }
    }
}

fn action_connue(app: &AppHandle, action: &str) -> bool {
    actions_en_vigueur(app).contains(&action)
}

// Commandes de la page (fenêtre `reglages` seulement, capabilities/reglages.json). Sans
// `async` : elles tournent sur le fil principal, sauf `reglages_parcourir`.

/// L'état complet ; relit le fichier (ouverture, retour du focus).
#[tauri::command]
pub fn reglages_lire(app: AppHandle) -> Reponse {
    if !etat(&app).page_reglages_prete.swap(true, Ordering::SeqCst) {
        log::info!("reglages page prete");
    }
    relire(&app);
    renvoyer_quand_les_listes_suivent(&app, TOURS_LISTES);
    reponse(&app, None)
}

/// Un changement de la page.
#[tauri::command]
pub fn reglages_changer(app: AppHandle, changement: Changement) -> Reponse {
    let base = relire(&app);
    let nouveau = match page::appliquer(&base, &changement) {
        Ok(n) => n,
        Err(refus) => return refuser(&app, refus),
    };
    if let Changement::Demarrage(oui) = changement {
        match systeme::demarrage_avec_systeme(Some(oui)) {
            Ok(e) => {
                log::info!(
                    "demarrage avec le systeme ecrit: {}",
                    if oui { "oui" } else { "non" }
                );
                if e == EtatDemarrage::DesactiveParWindows {
                    log::info!("demarrage avec le systeme: desactive par windows");
                }
            }
            Err(e) => {
                log::warn!("demarrage avec le systeme: echec ({e})");
                return refuser(
                    &app,
                    Refus::nouveau("demarrage", "Impossible de changer le démarrage", "echec"),
                );
            }
        }
    }
    log::info!("{}", page::ligne_changement(&changement));
    ecrire_et_garder(&app, nouveau.clone());
    appliquer_theme(&app, &nouveau);
    reponse(&app, None)
}

#[derive(Debug, Clone, Serialize)]
pub struct Apercu {
    pub apercu: String,
}

/// L'aperçu du nom des fichiers, sans rien écrire.
#[tauri::command]
pub fn reglages_apercu_nom(modele: String) -> Apercu {
    Apercu {
        apercu: page::apercu_nom(&modele, chrono::Local::now()),
    }
}

/// « Parcourir… » : le sélecteur de dossier du système, dans un fil à lui.
#[tauri::command]
pub async fn reglages_parcourir(app: AppHandle) -> Reponse {
    if DIALOGUE_OUVERT.swap(true, Ordering::SeqCst) {
        log::info!("dossier: dialogue deja ouvert");
        return reponse(&app, None);
    }
    let _garde = GardeDialogue;
    let proprietaire = fenetre::identifiant(&app);
    let r = memoire(&app);
    let images = images(&app);
    let (depart, _) =
        zone::dossier_enregistrement(r.dossier_enregistrement.as_deref(), images.as_deref());
    let (tx, rx) = std::sync::mpsc::channel();
    let lance = std::thread::Builder::new()
        .name("dossier".into())
        .spawn(move || {
            let _ = tx.send(systeme::choisir_dossier(proprietaire, Some(depart)));
        });
    let resultat = match lance {
        Ok(_) => tauri::async_runtime::spawn_blocking(move || rx.recv())
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or_else(|| Err("fil du dialogue arrete".into())),
        Err(e) => Err(e.to_string()),
    };
    let choisi = match resultat {
        Ok(Some(d)) => d,
        Ok(None) => {
            log::info!("dossier: dialogue annule");
            return reponse(&app, None);
        }
        Err(e) => {
            log::warn!("dossier: dialogue impossible ({e})");
            return refuser(&app, Refus::nouveau("dossier", "", "dialogue_impossible"));
        }
    };
    let Ok(dossier) = page::dossier_choisi(&choisi, images.as_deref()) else {
        return refuser(&app, Refus::nouveau("dossier", "", "invalide"));
    };
    let mut nouveau = relire(&app);
    nouveau.dossier_enregistrement = dossier;
    if let Err(e) = reglages::valider(&nouveau) {
        return refuser(&app, Refus::nouveau("dossier", "", e.code()));
    }
    log::info!("reglage change: dossier");
    ecrire_et_garder(&app, nouveau);
    let rep = reponse(&app, None);
    fenetre::emettre_etat(&app, rep.vue.clone());
    rep
}

/// Début (les raccourcis sont rendus au système, pour que la page reçoive les touches) ou
/// fin d'une saisie de raccourci.
#[tauri::command]
pub fn reglages_saisie(app: AppHandle, action: String, active: bool) -> Reponse {
    if !action_connue(&app, &action) {
        return refuser(&app, Refus::nouveau("raccourci", "", "action_inconnue"));
    }
    if !active {
        annuler_saisie(&app, "raccourcis repris");
        return reponse(&app, None);
    }
    let Some(r) = reservations(&app) else {
        return reponse(&app, None);
    };
    r.suspendre(&app);
    log::info!("raccourcis suspendus (saisie {action})");
    let numero = etat(&app).numero_saisie.fetch_add(1, Ordering::SeqCst) + 1;
    if let Ok(mut s) = etat(&app).saisie.lock() {
        *s = Some(numero);
    }
    // Filet : la page ne rend jamais la main ? Les raccourcis reviennent d'office.
    let pour_filet = app.clone();
    let _ = std::thread::Builder::new()
        .name("saisie".into())
        .spawn(move || {
            std::thread::sleep(DELAI_SAISIE);
            let pour = pour_filet.clone();
            let _ = pour_filet.run_on_main_thread(move || {
                let en_cours = etat(&pour).saisie.lock().ok().and_then(|s| *s);
                if en_cours == Some(numero) {
                    annuler_saisie(&pour, "raccourcis repris d'office");
                    fenetre::emettre_etat(&pour, vue(&pour));
                }
            });
        });
    reponse(&app, None)
}

/// Une touche tapée pendant la saisie : le nouveau raccourci de l'action.
#[tauri::command]
pub fn reglages_raccourci(app: AppHandle, action: String, saisie: Saisie) -> Reponse {
    if !action_connue(&app, &action) {
        return refuser(&app, Refus::nouveau("raccourci", "", "action_inconnue"));
    }
    finir_saisie(&app, "raccourcis repris");
    let notation = match page::notation_saisie(&saisie) {
        Ok(n) => n,
        Err(refus) => {
            // `saisie.code` vient de la page : jamais écrit tel quel.
            log::info!(
                "raccourci {action} refuse: {} ({})",
                page::code_pour_journal(&saisie.code),
                refus.code
            );
            noter_refus(&app, &action, Some(refus.texte.clone()));
            return reponse(&app, None);
        }
    };
    let mut base = relire(&app);
    if !changer_raccourci(&app, &action, std::slice::from_ref(&notation)) {
        return reponse(&app, None);
    }
    page::fixer_candidats(&mut base.raccourcis, &action, vec![notation]);
    ecrire_et_garder(&app, base);
    reponse(&app, None)
}

/// « Rétablir » : la liste par défaut de l'action (avec son repli).
#[tauri::command]
pub fn reglages_retablir(app: AppHandle, action: String) -> Reponse {
    if !action_connue(&app, &action) {
        return refuser(&app, Refus::nouveau("raccourci", "", "action_inconnue"));
    }
    finir_saisie(&app, "raccourcis repris");
    let defaut = Raccourcis::par_defaut(plateforme_reglages(systeme::systeme()));
    let liste = defaut.candidats(&action).unwrap_or_default().to_vec();
    let mut base = relire(&app);
    if !changer_raccourci(&app, &action, &liste) {
        return reponse(&app, None);
    }
    page::fixer_candidats(&mut base.raccourcis, &action, liste);
    ecrire_et_garder(&app, base);
    reponse(&app, None)
}

/// Au démarrage de l'app : une valeur `Run` qui pointe vers une autre copie de l'exe est
/// réécrite vers celle-ci (journal sans chemin). Absente : rien.
pub fn mettre_a_jour_demarrage() {
    match systeme::mettre_a_jour_chemin() {
        Ok(true) => log::info!("demarrage avec le systeme: chemin mis a jour"),
        Ok(false) => {}
        Err(e) => log::warn!("demarrage avec le systeme: chemin non mis a jour ({e})"),
    }
}

/// Thème du démarrage (avant que les pages existent).
pub fn retenir_theme(r: &Reglages) {
    theme::retenir(page::theme_page(r.theme));
}
