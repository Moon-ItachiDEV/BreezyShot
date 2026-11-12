//! Décisions de l'éditeur d'annotation (fonctions pures, contrat 0.4 « Éditeur », §4 à §8) :
//! quand Annoter est possible, ce que vaut un export reçu de la page, ce qu'il faut sortir,
//! s'il faut demander avant de fermer, que faire d'une nouvelle capture, et les lignes du
//! journal. Les gestes sont dans `application/mod.rs` et `fenetre_editeur.rs`, la fenêtre
//! dans `fenetres/editeur.rs`, le document et le dessin dans la page (`interface/editeur/`).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use serde::Serialize;

use super::vignette::IssueGlisser;
use super::zone::DELAI_SORTIE;
use crate::fenetres::{Ecran, Genre, Message, Rect};
use crate::sortie::{Bilan, Etape, EtapeFichier};
use crate::systeme::gel::Pixels;

/// Côté maximal d'une capture annotable, pixels (R11).
pub const COTE_MAX: u32 = 16_384;
/// Nombre maximal de pixels d'une capture annotable (un peu plus que 3 × 4K, R11).
pub const PIXELS_MAX: u64 = 25_000_000;
/// Délai donné à la page pour dire `editeur_pret` après la création de la fenêtre. Le
/// contrat dit 5 s ; WebView2 a mis plus de 15 s à charger une page sur la machine lente
/// des essais : 20 s, pour qu'un PC lent ne perde jamais l'éditeur (le filet ne sert qu'à
/// une page cassée).
pub const DELAI_PAGE: Duration = Duration::from_secs(20);
/// Délai donné à la page pour répondre à `editeur:fermer_demande`.
pub const DELAI_FERMETURE: Duration = Duration::from_secs(2);
/// Taille maximale des préférences d'outil de la page (au-delà : ignorées).
pub const PREFERENCES_MAX: usize = 4096;

/// La capture que l'éditeur annote : reprise de la vignette, mêmes pixels (même `Arc`).
#[derive(Debug, Clone)]
pub struct Document {
    pub id: u64,
    pub pixels: Arc<Pixels>,
    pub transparence: bool,
    /// L'heure de la capture (nom du fichier enregistré, R8).
    pub maintenant: DateTime<Local>,
}

/// L'éditeur ouvert, tenu par l'Application (`Application.editeur`, fil principal).
#[derive(Debug)]
pub struct EtatEditeur {
    pub document: Document,
    /// Capture arrivée pendant que l'éditeur avait des modifications (§4.3), une seule.
    pub en_attente: Option<Document>,
    /// Le clic Annoter (chronos d'ouverture).
    pub demande_a: Instant,
    pub page_prete: bool,
    /// Dernier état connu de la page (`editeur_etat`).
    pub version: u64,
    pub vide: bool,
    /// Dernières versions copiées ou exportées, enregistrées, rangées dans « Récents ».
    pub exportee: Option<u64>,
    pub enregistree: Option<u64>,
    pub rangee: Option<u64>,
    /// L'export en cours (un seul à la fois).
    pub export: Option<ExportEnCours>,
    /// Pourquoi la fenêtre se ferme, une fois décidé.
    pub raison: Option<RaisonFermeture>,
    /// Le fichier « Récents » de la version `rangee` (donné au glisser).
    pub recent: Option<PathBuf>,
    /// Numéro de la fenêtre (filet de la page : une fenêtre plus récente n'est pas visée).
    pub fenetre: u64,
    /// `editeur:fermer_demande` envoyé et pas encore répondu (son numéro, filet de 2 s).
    pub fermeture_demandee: Option<u64>,
    /// La page a dit avoir dessiné l'image du document courant (`editeur_affiche`).
    pub affichee: bool,
}

/// Un export parti dans le fil `editeur-sortie`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportEnCours {
    pub numero: u64,
    pub but: But,
    pub version: u64,
    /// Début de la sortie (après le dialogue d'Enregistrer sous… : l'échéance
    /// `DELAI_SORTIE` ne court qu'à partir de là).
    pub debut: Instant,
    /// Le dialogue d'Enregistrer sous… est ouvert : il bloque sans limite, aucun autre
    /// export n'est accepté pendant ce temps.
    pub dialogue: bool,
}

/// Ce que la page demande d'un export (en-tête `x-editeur-but`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum But {
    Copier,
    Enregistrer,
    EnregistrerSous,
    Glisser,
    CopierFermer,
}

impl But {
    pub const TOUS: [But; 5] = [
        But::Copier,
        But::Enregistrer,
        But::EnregistrerSous,
        But::Glisser,
        But::CopierFermer,
    ];

    /// Le nom de l'en-tête, du journal et de `editeur:resultat`.
    pub fn nom(self) -> &'static str {
        match self {
            But::Copier => "copier",
            But::Enregistrer => "enregistrer",
            But::EnregistrerSous => "enregistrer_sous",
            But::Glisser => "glisser",
            But::CopierFermer => "copier_fermer",
        }
    }

    /// Le but d'un nom exact (casse comprise) ; `None` sinon.
    pub fn depuis_nom(nom: &str) -> Option<But> {
        But::TOUS.into_iter().find(|b| b.nom() == nom)
    }
}

/// L'en-tête d'un export, lu et vérifié.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnTete {
    pub id: u64,
    pub version: u64,
    pub but: But,
    pub largeur: u32,
    pub hauteur: u32,
}

/// Refus d'un export (code rendu à la page et écrit au journal).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refus {
    Id,
    But,
    Taille,
    DejaEnCours,
}

impl Refus {
    pub fn code(self) -> &'static str {
        match self {
            Refus::Id => "id",
            Refus::But => "but",
            Refus::Taille => "taille",
            Refus::DejaEnCours => "deja_en_cours",
        }
    }
}

/// Ce que fait un export accepté.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sortie {
    Faire {
        copier: bool,
        enregistrer: bool,
        ranger_recents: bool,
    },
    /// Enregistrer de la même version : « Enregistré » sans écrire.
    DejaFait,
}

/// Annoter alors que l'éditeur existe déjà, ou non (§4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrivee {
    Ouvrir,
    Remplacer,
    Demander,
}

/// Fermer tout de suite, ou demander à la page (§4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fermeture {
    Directe,
    Demander,
}

/// Pourquoi l'éditeur s'est fermé (`editeur ferme <id> (<raison>)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaisonFermeture {
    Propre,
    SansGarder,
    CopierFermer,
    Arret,
    PageMuette,
    Impossible,
}

impl RaisonFermeture {
    /// Le nom du journal (`editeur ferme <id> (<nom>)`).
    pub fn nom(self) -> &'static str {
        match self {
            RaisonFermeture::Propre => "propre",
            RaisonFermeture::SansGarder => "sans garder",
            RaisonFermeture::CopierFermer => "copier et fermer",
            RaisonFermeture::Arret => "arret",
            RaisonFermeture::PageMuette => "page muette",
            RaisonFermeture::Impossible => "impossible",
        }
    }
}

/// Ce que le fil d'export rend au fil principal (`Evenement::EditeurExportFait`).
#[derive(Debug, Clone, PartialEq)]
pub struct FinExportEditeur {
    pub id: u64,
    pub numero: u64,
    pub but: But,
    pub version: u64,
    /// `None` : rien n'a été tenté (Enregistrer sous… annulé ou dialogue impossible ; la
    /// ligne est déjà écrite par le fil).
    pub bilan: Option<Bilan>,
    pub duree: Duration,
    /// Enregistrer d'une version déjà enregistrée : rien n'est écrit (`Sortie::DejaFait`).
    pub deja_fait: bool,
}

/// La charge de `editeur_pret` et de `editeur:charger`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChargeEditeur {
    pub id: u64,
    pub largeur: u32,
    pub hauteur: u32,
    pub transparence: bool,
    pub reduites: bool,
    pub preferences: Option<String>,
}

/// Charge de `editeur:fermer_demande` (`raison` : "cadre" ou "remplacer").
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChargeFermerDemande {
    pub id: u64,
    pub raison: &'static str,
}

/// Charge de `editeur:resultat` (`etat` : "fait", "deja_fait", "echec", "annule" ;
/// `message` : un titre du §7.4 ou rien).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChargeResultat {
    pub id: u64,
    pub numero: u64,
    pub but: &'static str,
    pub etat: &'static str,
    pub message: Option<&'static str>,
}

/// Charge de `editeur:glisser` (`etat` : "debut" ou "fini").
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChargeGlisser {
    pub id: u64,
    pub etat: &'static str,
}

/// Une capture est annotable : côtés non nuls, côté ≤ `COTE_MAX`, pixels ≤ `PIXELS_MAX`.
pub fn annotable(largeur: u32, hauteur: u32) -> bool {
    largeur > 0
        && hauteur > 0
        && largeur <= COTE_MAX
        && hauteur <= COTE_MAX
        && u64::from(largeur) * u64::from(hauteur) <= PIXELS_MAX
}

/// Un entier positif écrit en chiffres ASCII seuls (ni signe, ni espace, ni vide).
fn entier(texte: Option<&str>) -> Option<u64> {
    let t = texte?;
    if t.is_empty() || t.len() > 20 || !t.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    t.parse().ok()
}

/// Lit les en-têtes `x-editeur-*` d'un export : `id` et `version` faux → `Id`, but inconnu
/// (casse comprise) → `But`, largeur ou hauteur fausses ou hors `u32` → `Taille`.
pub fn lire_en_tete(
    id: Option<&str>,
    version: Option<&str>,
    but: Option<&str>,
    largeur: Option<&str>,
    hauteur: Option<&str>,
) -> Result<EnTete, Refus> {
    let id = entier(id).ok_or(Refus::Id)?;
    let version = entier(version).ok_or(Refus::Id)?;
    let but = but.and_then(But::depuis_nom).ok_or(Refus::But)?;
    let cote = |t: Option<&str>| {
        entier(t)
            .and_then(|v| u32::try_from(v).ok())
            .ok_or(Refus::Taille)
    };
    Ok(EnTete {
        id,
        version,
        but,
        largeur: cote(largeur)?,
        hauteur: cote(hauteur)?,
    })
}

/// Accepte un export lu (`octets` : longueur du corps) : le document en cours, une taille
/// de cadre possible (annotable, dans l'image), exactement `l × h × 4` octets, et aucun
/// autre export en cours ; un export de plus de `DELAI_SORTIE` est oublié.
pub fn accepter_export(
    etat: &EtatEditeur,
    e: &EnTete,
    octets: usize,
    maintenant: Instant,
) -> Result<(), Refus> {
    if e.id != etat.document.id {
        return Err(Refus::Id);
    }
    let image = &etat.document.pixels;
    let attendu = u64::from(e.largeur) * u64::from(e.hauteur) * 4;
    if !annotable(e.largeur, e.hauteur)
        || e.largeur > image.largeur
        || e.hauteur > image.hauteur
        || u64::try_from(octets).ok() != Some(attendu)
    {
        return Err(Refus::Taille);
    }
    let en_cours = etat.export.is_some_and(|x| {
        x.dialogue || maintenant.saturating_duration_since(x.debut) < DELAI_SORTIE
    });
    if en_cours {
        return Err(Refus::DejaEnCours);
    }
    Ok(())
}

/// Ce que fait un export accepté du but `but` pour la version `version`. Copier : copie
/// seule, jamais `DejaFait`. Enregistrer : fichier au modèle, ou `DejaFait` si cette
/// version l'est déjà. Enregistrer sous… : `enregistrer` veut dire le fichier choisi dans
/// le dialogue (`sortie::enregistrer_a`), pas l'enregistrement de `sortie::sortir`.
/// Glisser : rien d'autre que « Récents ». « Récents » une fois par version (R7) ; le
/// glisser le demande aussi tant que le fichier de cette version n'est pas connu.
pub fn sortie_pour(etat: &EtatEditeur, but: But, version: u64) -> Sortie {
    let rangee = etat.rangee == Some(version);
    let ranger_recents = !rangee || (but == But::Glisser && etat.recent.is_none());
    match but {
        But::Copier | But::CopierFermer => Sortie::Faire {
            copier: true,
            enregistrer: false,
            ranger_recents,
        },
        But::Enregistrer if etat.enregistree == Some(version) => Sortie::DejaFait,
        But::Enregistrer | But::EnregistrerSous => Sortie::Faire {
            copier: false,
            enregistrer: true,
            ranger_recents,
        },
        But::Glisser => Sortie::Faire {
            copier: false,
            enregistrer: false,
            ranger_recents,
        },
    }
}

/// Que faire d'un Annoter selon l'éditeur ouvert : ouvrir s'il n'y en a pas, remplacer
/// son document s'il est propre, sinon demander (§4.3).
pub fn arrivee(ouvert: Option<&EtatEditeur>) -> Arrivee {
    match ouvert {
        None => Arrivee::Ouvrir,
        Some(e) if fermeture(e.version, e.vide, e.exportee) == Fermeture::Directe => {
            Arrivee::Remplacer
        }
        Some(_) => Arrivee::Demander,
    }
}

/// `Directe` si le document est vide ou si `exportee == Some(version)`.
pub fn fermeture(version: u64, vide: bool, exportee: Option<u64>) -> Fermeture {
    if vide || exportee == Some(version) {
        Fermeture::Directe
    } else {
        Fermeture::Demander
    }
}

/// Le titre montré par la page après un export en échec (§7.4), ou rien : la copie, le
/// fichier ; « Récents » seul en échec et le glisser ne montrent rien (journal).
pub fn message_echec(but: But, bilan: &Bilan) -> Option<&'static str> {
    match but {
        But::Copier | But::CopierFermer => {
            matches!(bilan.copie, Etape::Echec(_)).then_some(TITRE_COPIE_IMPOSSIBLE)
        }
        But::Enregistrer | But::EnregistrerSous => {
            matches!(bilan.enregistrement, EtapeFichier::Echec { .. })
                .then_some(TITRE_ENREGISTREMENT_IMPOSSIBLE)
        }
        But::Glisser => None,
    }
}

/// Titres du §7.4.
pub const TITRE_COPIE_IMPOSSIBLE: &str = "Impossible de copier";
pub const TITRE_ENREGISTREMENT_IMPOSSIBLE: &str = "Impossible d'enregistrer";
pub const TITRE_OUVERTURE_IMPOSSIBLE: &str = "Impossible d'ouvrir l'éditeur";

/// Le titre montré quand un export n'a rendu aucun bilan à temps.
pub fn message_sans_bilan(but: But) -> Option<&'static str> {
    match but {
        But::Copier | But::CopierFermer => Some(TITRE_COPIE_IMPOSSIBLE),
        But::Enregistrer | But::EnregistrerSous => Some(TITRE_ENREGISTREMENT_IMPOSSIBLE),
        But::Glisser => None,
    }
}

/// L'export a fait ce qu'on lui demandait : la copie, le fichier, ou (glisser) un fichier
/// « Récents » disponible.
pub fn reussie(but: But, bilan: &Bilan) -> bool {
    match but {
        But::Copier | But::CopierFermer => bilan.copie == Etape::Faite,
        But::Enregistrer | But::EnregistrerSous => {
            matches!(bilan.enregistrement, EtapeFichier::Fait(_))
        }
        But::Glisser => !matches!(bilan.recent, Some(Err(_))),
    }
}

/// La charge de `editeur:resultat` pour un export fini.
pub fn resultat(fin: &FinExportEditeur) -> ChargeResultat {
    let (etat, message) = match &fin.bilan {
        _ if fin.deja_fait => ("deja_fait", None),
        None => ("annule", None),
        Some(b) if reussie(fin.but, b) => ("fait", None),
        Some(b) => ("echec", message_echec(fin.but, b)),
    };
    ChargeResultat {
        id: fin.id,
        numero: fin.numero,
        but: fin.but.nom(),
        etat,
        message,
    }
}

/// La charge de `editeur:resultat` pour un export sans bilan à temps.
pub fn resultat_sans_bilan(id: u64, numero: u64, but: But) -> ChargeResultat {
    ChargeResultat {
        id,
        numero,
        but: but.nom(),
        etat: "echec",
        message: message_sans_bilan(but),
    }
}

/// Ce que devient le bilan d'un export arrivé au fil principal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuiteExport {
    /// L'export attendu : état noté, `editeur:resultat` émis, suite (fermer, glisser).
    Attendu,
    /// Le document est là mais l'export a été oublié (échéance tombée) : état noté, rien
    /// d'émis (jamais deux résultats pour un export).
    Tardif,
    /// L'éditeur est fermé (ou a changé de document) : journal seulement ; le fichier
    /// éventuel est déjà écrit, rien n'est émis vers une page qui n'existe plus.
    DocumentParti,
}

/// La suite d'un export fini selon l'éditeur ouvert.
pub fn suite_export(ouvert: Option<&EtatEditeur>, fin: &FinExportEditeur) -> SuiteExport {
    match ouvert {
        Some(e) if e.document.id == fin.id => {
            if e.export.is_some_and(|x| x.numero == fin.numero) {
                SuiteExport::Attendu
            } else {
                SuiteExport::Tardif
            }
        }
        _ => SuiteExport::DocumentParti,
    }
}

/// La charge de `editeur_pret` et de `editeur:charger`.
pub fn charge(etat: &EtatEditeur, reduites: bool, preferences: Option<String>) -> ChargeEditeur {
    ChargeEditeur {
        id: etat.document.id,
        largeur: etat.document.pixels.largeur,
        hauteur: etat.document.pixels.hauteur,
        transparence: etat.document.transparence,
        reduites,
        preferences,
    }
}

/// Les préférences d'outil à garder pour la session (R9) : les nouvelles si elles tiennent
/// dans `PREFERENCES_MAX` octets, sinon les anciennes.
pub fn garder_preferences(ancien: Option<String>, nouveau: Option<String>) -> Option<String> {
    match nouveau {
        Some(n) if n.len() <= PREFERENCES_MAX => Some(n),
        _ => ancien,
    }
}

/// Le bandeau d'une ouverture impossible (Erreur, titre seul).
pub fn message_impossible() -> Message {
    Message {
        genre: Genre::Erreur,
        titre: TITRE_OUVERTURE_IMPOSSIBLE.into(),
        detail: String::new(),
        actions: Vec::new(),
    }
}

/// L'image dans la page (pixels CSS, relatifs à la page) en pixels réels d'écran ; `None`
/// si une valeur n'est pas finie ou si un côté est négatif.
pub fn vue_ecran(page: Rect, echelle: f64, x: f64, y: f64, l: f64, h: f64) -> Option<Rect> {
    if ![x, y, l, h, echelle].iter().all(|v| v.is_finite()) || l < 0.0 || h < 0.0 {
        return None;
    }
    let e = if echelle > 0.0 { echelle } else { 1.0 };
    let borne = |v: f64| v.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32;
    Some(Rect {
        x: page.x.saturating_add(borne((x * e).round())),
        y: page.y.saturating_add(borne((y * e).round())),
        largeur: (l * e).round().min(f64::from(u32::MAX)) as u32,
        hauteur: (h * e).round().min(f64::from(u32::MAX)) as u32,
    })
}

impl EtatEditeur {
    /// L'éditeur qui s'ouvre sur `document` (fenêtre numéro `fenetre`).
    pub fn nouveau(document: Document, demande_a: Instant, fenetre: u64) -> EtatEditeur {
        EtatEditeur {
            document,
            en_attente: None,
            demande_a,
            page_prete: false,
            version: 0,
            vide: true,
            exportee: None,
            enregistree: None,
            rangee: None,
            export: None,
            raison: None,
            recent: None,
            fenetre,
            fermeture_demandee: None,
            affichee: false,
        }
    }

    /// Prend `document` à la place du document courant (même fenêtre, même page) et rend
    /// l'ancien. Tout ce qui tenait à l'ancien est oublié ; la page reste prête.
    pub fn remplacer(&mut self, document: Document, demande_a: Instant) -> Document {
        let page_prete = self.page_prete;
        let neuf = EtatEditeur::nouveau(document, demande_a, self.fenetre);
        let ancien = std::mem::replace(self, neuf);
        self.page_prete = page_prete;
        ancien.document
    }

    /// « Copier et fermer » ou « Ne pas garder » avec une capture en attente : elle est
    /// chargée au lieu de fermer. Rend l'`id` de l'ancien document, `None` sans attente.
    pub fn charger_en_attente(&mut self, demande_a: Instant) -> Option<u64> {
        let suivant = self.en_attente.take()?;
        Some(self.remplacer(suivant, demande_a).id)
    }

    /// Range une capture arrivée pendant des modifications ; rend celle qu'elle remplace
    /// (abandonnée).
    pub fn mettre_en_attente(&mut self, document: Document) -> Option<Document> {
        self.en_attente.replace(document)
    }

    /// Un export fini du document courant : versions copiée ou exportée, enregistrée,
    /// rangée (réussites seulement) ; l'export en cours est oublié s'il est celui-ci. Un
    /// glisser ne compte comme exporté qu'au dépôt (`depose`).
    pub fn noter_export(&mut self, fin: &FinExportEditeur) {
        if fin.id != self.document.id {
            return;
        }
        if self.export.is_some_and(|x| x.numero == fin.numero) {
            self.export = None;
        }
        if fin.deja_fait {
            self.exportee = Some(fin.version);
            return;
        }
        let Some(b) = &fin.bilan else {
            return;
        };
        if let Some(Ok(p)) = &b.recent {
            self.rangee = Some(fin.version);
            self.recent = Some(p.clone());
        }
        if fin.but != But::Glisser && reussie(fin.but, b) {
            self.exportee = Some(fin.version);
            if fin.but == But::Enregistrer {
                self.enregistree = Some(fin.version);
            }
        }
    }

    /// Le dialogue de l'export `numero` est fermé : sa sortie commence, l'échéance court.
    pub fn dialogue_ferme(&mut self, numero: u64, maintenant: Instant) {
        if let Some(x) = self.export.as_mut().filter(|x| x.numero == numero) {
            x.dialogue = false;
            x.debut = maintenant;
        }
    }

    /// Le fichier « Récents » de la version `version`, s'il est connu (glisser).
    pub fn recent_de(&self, version: u64) -> Option<&PathBuf> {
        self.recent
            .as_ref()
            .filter(|_| self.rangee == Some(version))
    }

    /// Un glisser déposé : la version rangée compte comme exportée.
    pub fn noter_depot(&mut self) {
        if let Some(v) = self.rangee {
            self.exportee = Some(v);
        }
    }
}

// Lignes du journal (jamais de chemin, de nom de fichier ni de contenu).

/// `editeur export <id> <n> <but> version <v> <l>x<h>`.
pub fn ligne_export(id: u64, numero: u64, but: But, version: u64, l: u32, h: u32) -> String {
    format!(
        "editeur export {id} {numero} {} version {version} {l}x{h}",
        but.nom()
    )
}

/// `editeur export <id> <n> refuse (<code>)`.
pub fn ligne_refus(id: u64, numero: u64, refus: Refus) -> String {
    format!("editeur export {id} {numero} refuse ({})", refus.code())
}

/// `editeur export <id> <n> copie … recent … fichier … en <ms> ms`.
pub fn ligne_bilan(id: u64, numero: u64, bilan: &Bilan, ms: u128) -> String {
    let copie = match bilan.copie {
        Etape::Faite => "faite",
        Etape::NonDemandee => "non demandee",
        Etape::Echec(_) => "echec",
    };
    let recent = match bilan.recent {
        Some(Ok(_)) => "ok",
        Some(Err(_)) => "echec",
        None => "non demande",
    };
    let fichier = match bilan.enregistrement {
        EtapeFichier::Fait(_) => "fait",
        EtapeFichier::NonDemande => "non demande",
        EtapeFichier::Echec { .. } => "echec",
    };
    format!(
        "editeur export {id} {numero} copie {copie} recent {recent} fichier {fichier} en {ms} ms"
    )
}

/// `editeur ferme <id> (<raison>)`.
pub fn ligne_fermeture(id: u64, raison: RaisonFermeture) -> String {
    format!("editeur ferme {id} ({})", raison.nom())
}

/// `editeur ouvert <id> <l>x<h> ecran <x>,<y> <l>x<h> echelle <e> fenetre <x>,<y> <l>x<h>`.
pub fn ligne_ouvert(id: u64, image: (u32, u32), ecran: &Ecran, fenetre: &Rect) -> String {
    let r = ecran.rect;
    format!(
        "editeur ouvert {id} {}x{} ecran {},{} {}x{} echelle {:.2} fenetre {},{} {}x{}",
        image.0,
        image.1,
        r.x,
        r.y,
        r.largeur,
        r.hauteur,
        ecran.echelle,
        fenetre.x,
        fenetre.y,
        fenetre.largeur,
        fenetre.hauteur
    )
}

/// `editeur vue <id> <x>,<y> <l>x<h> zoom <z>` (deux décimales).
pub fn ligne_vue(id: u64, vue: &Rect, zoom: f64) -> String {
    format!(
        "editeur vue {id} {},{} {}x{} zoom {zoom:.2}",
        vue.x, vue.y, vue.largeur, vue.hauteur
    )
}

/// `editeur glisser <id> depose|annule|refuse (<cause>)`.
pub fn ligne_glisser(id: u64, issue: &IssueGlisser) -> String {
    match issue {
        IssueGlisser::Depose => format!("editeur glisser {id} depose"),
        IssueGlisser::Annule => format!("editeur glisser {id} annule"),
        IssueGlisser::Refuse(c) => format!("editeur glisser {id} refuse ({c})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::etats::{Declencheur, Etat, Mode, PASSAGES};

    fn document(id: u64, l: u32, h: u32) -> Document {
        Document {
            id,
            pixels: Arc::new(Pixels {
                largeur: l,
                hauteur: h,
                rgba: vec![0; (l * h * 4) as usize],
            }),
            transparence: false,
            maintenant: Local::now(),
        }
    }

    fn etat() -> EtatEditeur {
        EtatEditeur::nouveau(document(7, 40, 30), Instant::now(), 1)
    }

    fn en_tete(l: u32, h: u32) -> EnTete {
        EnTete {
            id: 7,
            version: 3,
            but: But::Copier,
            largeur: l,
            hauteur: h,
        }
    }

    fn bilan(copie: Etape, recent: Option<Result<PathBuf, String>>, f: EtapeFichier) -> Bilan {
        Bilan {
            copie,
            recent,
            enregistrement: f,
        }
    }

    fn echec_fichier() -> EtapeFichier {
        EtapeFichier::Echec {
            dossier: PathBuf::from("/home/moi/Images"),
            cause: "disque plein".into(),
        }
    }

    fn fin(but: But, version: u64, b: Option<Bilan>) -> FinExportEditeur {
        FinExportEditeur {
            id: 7,
            numero: 2,
            but,
            version,
            bilan: b,
            duree: Duration::from_millis(5),
            deja_fait: false,
        }
    }

    #[test]
    fn annotable_aux_bornes() {
        assert!(annotable(COTE_MAX, 1));
        assert!(annotable(1, COTE_MAX));
        assert!(!annotable(COTE_MAX + 1, 1));
        assert!(!annotable(1, COTE_MAX + 1));
        // 25 000 000 pile (5000 × 5000), puis un pixel de trop (côtés sous 16 384).
        assert!(annotable(5000, 5000));
        // 25 000 001 n'a pas de côtés ≤ 16 384 : le plus proche au-dessus, 16 384 × 1 526.
        assert!(annotable(COTE_MAX, 1525));
        assert!(!annotable(COTE_MAX, 1526));
        assert!(annotable(10_000, 2500));
        assert!(!annotable(12_500, 2001));
        assert!(!annotable(0, 10) && !annotable(10, 0) && !annotable(0, 0));
    }

    #[test]
    fn en_tete_lu() {
        let ok = lire_en_tete(Some("7"), Some("0"), Some("copier"), Some("4"), Some("3"));
        assert_eq!(
            ok,
            Ok(EnTete {
                id: 7,
                version: 0,
                but: But::Copier,
                largeur: 4,
                hauteur: 3
            })
        );
        let lire = |id, v, b, l, h| lire_en_tete(id, v, b, l, h).err();
        let c = Some("copier");
        // Absent, non entier, négatif, signe, espace.
        assert_eq!(
            lire(None, Some("1"), c, Some("4"), Some("3")),
            Some(Refus::Id)
        );
        assert_eq!(
            lire(Some("x"), Some("1"), c, Some("4"), Some("3")),
            Some(Refus::Id)
        );
        assert_eq!(
            lire(Some("-7"), Some("1"), c, Some("4"), Some("3")),
            Some(Refus::Id)
        );
        assert_eq!(
            lire(Some("+7"), Some("1"), c, Some("4"), Some("3")),
            Some(Refus::Id)
        );
        assert_eq!(
            lire(Some(" 7"), Some("1"), c, Some("4"), Some("3")),
            Some(Refus::Id)
        );
        assert_eq!(
            lire(Some(""), Some("1"), c, Some("4"), Some("3")),
            Some(Refus::Id)
        );
        assert_eq!(
            lire(Some("7"), None, c, Some("4"), Some("3")),
            Some(Refus::Id)
        );
        assert_eq!(
            lire(Some("7"), Some("1.5"), c, Some("4"), Some("3")),
            Some(Refus::Id)
        );
        assert_eq!(
            lire(
                Some("99999999999999999999999"),
                Some("1"),
                c,
                Some("4"),
                Some("3")
            ),
            Some(Refus::Id)
        );
        // But inconnu, en majuscules, absent.
        for b in [
            None,
            Some("COPIER"),
            Some("Copier"),
            Some("epingler"),
            Some(""),
        ] {
            assert_eq!(
                lire(Some("7"), Some("1"), b, Some("4"), Some("3")),
                Some(Refus::But)
            );
        }
        // Côtés absents, négatifs, hors u32.
        assert_eq!(
            lire(Some("7"), Some("1"), c, None, Some("3")),
            Some(Refus::Taille)
        );
        assert_eq!(
            lire(Some("7"), Some("1"), c, Some("4"), Some("-3")),
            Some(Refus::Taille)
        );
        assert_eq!(
            lire(Some("7"), Some("1"), c, Some("4294967296"), Some("3")),
            Some(Refus::Taille)
        );
    }

    #[test]
    fn but_depuis_nom() {
        for b in But::TOUS {
            assert_eq!(But::depuis_nom(b.nom()), Some(b));
        }
        assert_eq!(
            But::depuis_nom("enregistrer_sous"),
            Some(But::EnregistrerSous)
        );
        assert_eq!(But::depuis_nom("copier_fermer"), Some(But::CopierFermer));
        assert_eq!(But::depuis_nom("Glisser"), None);
        assert_eq!(But::depuis_nom("glisser "), None);
        assert_eq!(Refus::DejaEnCours.code(), "deja_en_cours");
    }

    #[test]
    fn export_accepte_ou_refuse() {
        let t = Instant::now();
        let mut e = etat();
        let juste = 40 * 30 * 4;
        assert_eq!(accepter_export(&e, &en_tete(40, 30), juste, t), Ok(()));
        // Un cadre plus petit (recadrage) est possible.
        assert_eq!(accepter_export(&e, &en_tete(10, 5), 10 * 5 * 4, t), Ok(()));
        // Autre id.
        let autre = EnTete {
            id: 8,
            ..en_tete(40, 30)
        };
        assert_eq!(accepter_export(&e, &autre, juste, t), Err(Refus::Id));
        // Un octet de trop, un de moins.
        assert_eq!(
            accepter_export(&e, &en_tete(40, 30), juste + 1, t),
            Err(Refus::Taille)
        );
        assert_eq!(
            accepter_export(&e, &en_tete(40, 30), juste - 1, t),
            Err(Refus::Taille)
        );
        // Taille impossible : plus grande que l'image, nulle.
        assert_eq!(
            accepter_export(&e, &en_tete(41, 30), 41 * 30 * 4, t),
            Err(Refus::Taille)
        );
        assert_eq!(
            accepter_export(&e, &en_tete(40, 31), 40 * 31 * 4, t),
            Err(Refus::Taille)
        );
        assert_eq!(
            accepter_export(&e, &en_tete(0, 30), 0, t),
            Err(Refus::Taille)
        );
        // Export en cours ; oublié après DELAI_SORTIE.
        e.export = Some(ExportEnCours {
            numero: 1,
            but: But::Copier,
            version: 3,
            debut: t,
            dialogue: false,
        });
        assert_eq!(
            accepter_export(&e, &en_tete(40, 30), juste, t + Duration::from_secs(9)),
            Err(Refus::DejaEnCours)
        );
        assert_eq!(
            accepter_export(&e, &en_tete(40, 30), juste, t + DELAI_SORTIE),
            Ok(())
        );
        assert_eq!(
            accepter_export(&e, &en_tete(40, 30), juste, t + Duration::from_secs(11)),
            Ok(())
        );
    }

    #[test]
    fn sortie_selon_le_but_et_la_version() {
        let mut e = etat();
        let faire = |c, f, r| Sortie::Faire {
            copier: c,
            enregistrer: f,
            ranger_recents: r,
        };
        assert_eq!(sortie_pour(&e, But::Copier, 1), faire(true, false, true));
        assert_eq!(
            sortie_pour(&e, But::CopierFermer, 1),
            faire(true, false, true)
        );
        assert_eq!(
            sortie_pour(&e, But::Enregistrer, 1),
            faire(false, true, true)
        );
        assert_eq!(
            sortie_pour(&e, But::EnregistrerSous, 1),
            faire(false, true, true)
        );
        assert_eq!(sortie_pour(&e, But::Glisser, 1), faire(false, false, true));
        // Version 1 copiée, rangée et enregistrée.
        e.noter_export(&fin(
            But::Enregistrer,
            1,
            Some(bilan(
                Etape::NonDemandee,
                Some(Ok(PathBuf::from("/l/Recents/a.png"))),
                EtapeFichier::Fait(PathBuf::from("/i/a.png")),
            )),
        ));
        assert_eq!(e.rangee, Some(1));
        assert_eq!(e.enregistree, Some(1));
        assert_eq!(e.exportee, Some(1));
        // Même version : « Récents » une seule fois, Enregistrer déjà fait, Copier jamais.
        assert_eq!(sortie_pour(&e, But::Enregistrer, 1), Sortie::DejaFait);
        assert_eq!(sortie_pour(&e, But::Copier, 1), faire(true, false, false));
        assert_eq!(
            sortie_pour(&e, But::CopierFermer, 1),
            faire(true, false, false)
        );
        assert_eq!(
            sortie_pour(&e, But::EnregistrerSous, 1),
            faire(false, true, false)
        );
        assert_eq!(sortie_pour(&e, But::Glisser, 1), faire(false, false, false));
        assert_eq!(e.recent_de(1), Some(&PathBuf::from("/l/Recents/a.png")));
        assert_eq!(e.recent_de(2), None);
        // Version suivante : tout de nouveau.
        assert_eq!(
            sortie_pour(&e, But::Enregistrer, 2),
            faire(false, true, true)
        );
        assert_eq!(sortie_pour(&e, But::Copier, 2), faire(true, false, true));
    }

    #[test]
    fn export_note_seulement_en_cas_de_reussite() {
        let mut e = etat();
        e.export = Some(ExportEnCours {
            numero: 2,
            but: But::Copier,
            version: 4,
            debut: Instant::now(),
            dialogue: false,
        });
        // Copie en échec, « Récents » réussi : rangée, ni exportée ni oubliée deux fois.
        e.noter_export(&fin(
            But::Copier,
            4,
            Some(bilan(
                Etape::Echec("tenu".into()),
                Some(Ok(PathBuf::from("/l/Recents/b.png"))),
                EtapeFichier::NonDemande,
            )),
        ));
        assert_eq!(e.export, None);
        assert_eq!(e.rangee, Some(4));
        assert_eq!(e.exportee, None);
        // Glisser : rangé, pas exporté avant le dépôt.
        e.noter_export(&fin(
            But::Glisser,
            5,
            Some(bilan(
                Etape::NonDemandee,
                Some(Ok(PathBuf::from("/l/Recents/c.png"))),
                EtapeFichier::NonDemande,
            )),
        ));
        assert_eq!((e.rangee, e.exportee), (Some(5), None));
        e.noter_depot();
        assert_eq!(e.exportee, Some(5));
        // Enregistrer sous réussi : exporté, pas « enregistré » au modèle.
        e.noter_export(&fin(
            But::EnregistrerSous,
            6,
            Some(bilan(
                Etape::NonDemandee,
                None,
                EtapeFichier::Fait(PathBuf::from("/x.png")),
            )),
        ));
        assert_eq!((e.exportee, e.enregistree), (Some(6), None));
        // Un autre document : rien.
        let mut autre = fin(
            But::Copier,
            9,
            Some(bilan(Etape::Faite, None, EtapeFichier::NonDemande)),
        );
        autre.id = 8;
        e.noter_export(&autre);
        assert_eq!(e.exportee, Some(6));
    }

    /// Décision du responsable (1) : le dialogue d'Enregistrer sous… bloque sans limite et
    /// ne court pas sous l'échéance ; aucun autre export pendant qu'il est ouvert.
    #[test]
    fn dialogue_ouvert_hors_echeance() {
        let t = Instant::now();
        let mut e = etat();
        let juste = 40 * 30 * 4;
        e.export = Some(ExportEnCours {
            numero: 4,
            but: But::EnregistrerSous,
            version: 3,
            debut: t,
            dialogue: true,
        });
        let tard = t + Duration::from_secs(600);
        assert_eq!(
            accepter_export(&e, &en_tete(40, 30), juste, tard),
            Err(Refus::DejaEnCours)
        );
        // Un autre numéro ne ferme pas ce dialogue.
        e.dialogue_ferme(5, tard);
        assert!(e.export.is_some_and(|x| x.dialogue));
        // Dialogue fermé à `tard` : les 10 s comptent de là.
        e.dialogue_ferme(4, tard);
        assert_eq!(e.export.map(|x| (x.dialogue, x.debut)), Some((false, tard)));
        assert_eq!(
            accepter_export(&e, &en_tete(40, 30), juste, tard + Duration::from_secs(9)),
            Err(Refus::DejaEnCours)
        );
        assert_eq!(
            accepter_export(&e, &en_tete(40, 30), juste, tard + DELAI_SORTIE),
            Ok(())
        );
    }

    /// Décisions du responsable (1) et (3) : un seul résultat par export ; un bilan pour
    /// un éditeur fermé est accepté sans rien émettre.
    #[test]
    fn suite_d_un_export() {
        let t = Instant::now();
        let mut e = etat();
        let f = fin(
            But::EnregistrerSous,
            3,
            Some(bilan(
                Etape::NonDemandee,
                None,
                EtapeFichier::Fait(PathBuf::from("/x.png")),
            )),
        );
        e.export = Some(ExportEnCours {
            numero: 2,
            but: But::EnregistrerSous,
            version: 3,
            debut: t,
            dialogue: false,
        });
        assert_eq!(suite_export(Some(&e), &f), SuiteExport::Attendu);
        // Échéance tombée (export oublié) : tardif, rien d'émis, mais l'état est noté.
        e.export = None;
        assert_eq!(suite_export(Some(&e), &f), SuiteExport::Tardif);
        e.noter_export(&f);
        assert_eq!(e.exportee, Some(3));
        // Éditeur fermé pendant le dialogue, ou autre document.
        assert_eq!(suite_export(None, &f), SuiteExport::DocumentParti);
        let mut autre = etat();
        autre.document.id = 8;
        assert_eq!(suite_export(Some(&autre), &f), SuiteExport::DocumentParti);
    }

    #[test]
    fn fermeture_directe_ou_demandee() {
        assert_eq!(fermeture(0, true, None), Fermeture::Directe);
        assert_eq!(fermeture(5, true, None), Fermeture::Directe);
        assert_eq!(fermeture(0, false, None), Fermeture::Demander);
        assert_eq!(fermeture(0, false, Some(0)), Fermeture::Directe);
        assert_eq!(fermeture(3, false, Some(3)), Fermeture::Directe);
        // Modifiée après l'export.
        assert_eq!(fermeture(4, false, Some(3)), Fermeture::Demander);
    }

    #[test]
    fn arrivee_selon_l_editeur() {
        assert_eq!(arrivee(None), Arrivee::Ouvrir);
        let mut e = etat();
        assert_eq!(arrivee(Some(&e)), Arrivee::Remplacer);
        e.vide = false;
        e.version = 2;
        assert_eq!(arrivee(Some(&e)), Arrivee::Demander);
        e.exportee = Some(2);
        assert_eq!(arrivee(Some(&e)), Arrivee::Remplacer);
        e.version = 3;
        assert_eq!(arrivee(Some(&e)), Arrivee::Demander);
    }

    #[test]
    fn remplacer_et_attendre() {
        let mut e = etat();
        e.page_prete = true;
        e.version = 4;
        e.vide = false;
        e.exportee = Some(4);
        e.rangee = Some(4);
        e.recent = Some(PathBuf::from("/l/r.png"));
        e.affichee = true;
        let ancien = e.remplacer(document(9, 2, 2), Instant::now());
        assert_eq!(ancien.id, 7);
        assert_eq!(e.document.id, 9);
        assert!(e.page_prete && e.vide && !e.affichee);
        assert_eq!(
            (e.version, e.exportee, e.rangee, e.recent.clone()),
            (0, None, None, None)
        );
        assert_eq!(e.fenetre, 1);
        // Un seul en attente : le troisième remplace le deuxième.
        assert!(e.mettre_en_attente(document(10, 2, 2)).is_none());
        assert_eq!(
            e.mettre_en_attente(document(11, 2, 2)).map(|d| d.id),
            Some(10)
        );
        assert_eq!(e.charger_en_attente(Instant::now()), Some(9));
        assert_eq!(e.document.id, 11);
        assert!(e.en_attente.is_none());
        assert_eq!(e.charger_en_attente(Instant::now()), None);
        // Mêmes pixels : aucune copie.
        let d = document(12, 3, 3);
        let p = Arc::clone(&d.pixels);
        e.remplacer(d, Instant::now());
        assert!(Arc::ptr_eq(&p, &e.document.pixels));
    }

    #[test]
    fn messages_d_echec() {
        let ok = bilan(
            Etape::Faite,
            Some(Ok(PathBuf::from("/r.png"))),
            EtapeFichier::NonDemande,
        );
        let copie_ko = bilan(Etape::Echec("x".into()), None, EtapeFichier::NonDemande);
        let recent_ko = bilan(
            Etape::Faite,
            Some(Err("x".into())),
            EtapeFichier::NonDemande,
        );
        let fichier_ok = bilan(
            Etape::NonDemandee,
            None,
            EtapeFichier::Fait("/f.png".into()),
        );
        let fichier_ko = bilan(Etape::NonDemandee, None, echec_fichier());
        for b in [But::Copier, But::CopierFermer] {
            assert_eq!(message_echec(b, &ok), None);
            assert_eq!(message_echec(b, &copie_ko), Some("Impossible de copier"));
            assert_eq!(message_echec(b, &recent_ko), None);
        }
        for b in [But::Enregistrer, But::EnregistrerSous] {
            assert_eq!(message_echec(b, &fichier_ok), None);
            assert_eq!(
                message_echec(b, &fichier_ko),
                Some("Impossible d'enregistrer")
            );
            assert_eq!(
                message_echec(
                    b,
                    &bilan(
                        Etape::NonDemandee,
                        Some(Err("x".into())),
                        EtapeFichier::Fait("/f.png".into())
                    )
                ),
                None
            );
        }
        assert_eq!(message_echec(But::Glisser, &recent_ko), None);
        assert_eq!(message_echec(But::Glisser, &copie_ko), None);
        assert_eq!(
            message_sans_bilan(But::Copier),
            Some("Impossible de copier")
        );
        assert_eq!(
            message_sans_bilan(But::EnregistrerSous),
            Some("Impossible d'enregistrer")
        );
        assert_eq!(message_sans_bilan(But::Glisser), None);
    }

    #[test]
    fn resultats_pour_la_page() {
        let r = resultat(&fin(
            But::Copier,
            1,
            Some(bilan(Etape::Faite, None, EtapeFichier::NonDemande)),
        ));
        assert_eq!(
            (r.id, r.numero, r.but, r.etat, r.message),
            (7, 2, "copier", "fait", None)
        );
        let r = resultat(&fin(
            But::Enregistrer,
            1,
            Some(bilan(Etape::NonDemandee, None, echec_fichier())),
        ));
        assert_eq!(
            (r.etat, r.message),
            ("echec", Some("Impossible d'enregistrer"))
        );
        let r = resultat(&fin(But::EnregistrerSous, 1, None));
        assert_eq!(
            (r.but, r.etat, r.message),
            ("enregistrer_sous", "annule", None)
        );
        let mut d = fin(But::Enregistrer, 1, None);
        d.deja_fait = true;
        assert_eq!(resultat(&d).etat, "deja_fait");
        let r = resultat_sans_bilan(7, 3, But::CopierFermer);
        assert_eq!(
            (r.but, r.etat, r.message),
            ("copier_fermer", "echec", Some("Impossible de copier"))
        );
        assert_eq!(
            serde_json::to_string(&r).unwrap(),
            concat!(
                r#"{"id":7,"numero":3,"but":"copier_fermer","etat":"echec","#,
                r#""message":"Impossible de copier"}"#
            )
        );
        assert_eq!(
            serde_json::to_string(&ChargeFermerDemande {
                id: 7,
                raison: "cadre"
            })
            .unwrap(),
            r#"{"id":7,"raison":"cadre"}"#
        );
    }

    #[test]
    fn charge_et_preferences() {
        let e = etat();
        let c = charge(&e, true, Some("{}".into()));
        assert_eq!(
            (c.id, c.largeur, c.hauteur, c.transparence, c.reduites),
            (7, 40, 30, false, true)
        );
        assert_eq!(
            serde_json::to_string(&c).unwrap(),
            concat!(
                r#"{"id":7,"largeur":40,"hauteur":30,"transparence":false,"#,
                r#""reduites":true,"preferences":"{}"}"#
            )
        );
        let a = Some("a".to_string());
        assert_eq!(garder_preferences(None, Some("b".into())), Some("b".into()));
        assert_eq!(garder_preferences(a.clone(), None), a);
        assert_eq!(
            garder_preferences(a.clone(), Some("x".repeat(PREFERENCES_MAX))).map(|p| p.len()),
            Some(PREFERENCES_MAX)
        );
        assert_eq!(
            garder_preferences(a.clone(), Some("x".repeat(PREFERENCES_MAX + 1))),
            a
        );
    }

    #[test]
    fn vue_en_pixels_d_ecran() {
        let page = Rect {
            x: -1910,
            y: 40,
            largeur: 800,
            hauteur: 600,
        };
        assert_eq!(
            vue_ecran(page, 1.5, 10.0, 20.0, 100.0, 50.5),
            Some(Rect {
                x: -1895,
                y: 70,
                largeur: 150,
                hauteur: 76
            })
        );
        assert_eq!(vue_ecran(page, 1.0, f64::NAN, 0.0, 1.0, 1.0), None);
        assert_eq!(vue_ecran(page, 1.0, 0.0, 0.0, -1.0, 1.0), None);
        assert_eq!(
            vue_ecran(page, 0.0, 1.0, 1.0, 1.0, 1.0).map(|r| r.x),
            Some(-1909)
        );
    }

    #[test]
    fn lignes_du_journal() {
        assert_eq!(
            ligne_export(7, 2, But::EnregistrerSous, 5, 1024, 768),
            "editeur export 7 2 enregistrer_sous version 5 1024x768"
        );
        assert_eq!(
            ligne_refus(7, 3, Refus::Taille),
            "editeur export 7 3 refuse (taille)"
        );
        assert_eq!(
            ligne_refus(7, 3, Refus::DejaEnCours),
            "editeur export 7 3 refuse (deja_en_cours)"
        );
        let b = bilan(
            Etape::Faite,
            Some(Ok(PathBuf::from("/home/moi/Recents/x.png"))),
            EtapeFichier::NonDemande,
        );
        assert_eq!(
            ligne_bilan(7, 2, &b, 41),
            "editeur export 7 2 copie faite recent ok fichier non demande en 41 ms"
        );
        let b = bilan(Etape::NonDemandee, None, echec_fichier());
        let l = ligne_bilan(7, 2, &b, 3);
        assert_eq!(
            l,
            "editeur export 7 2 copie non demandee recent non demande fichier echec en 3 ms"
        );
        let b = bilan(
            Etape::Echec("tenu".into()),
            Some(Err("/home/moi disque plein".into())),
            EtapeFichier::Fait(PathBuf::from("/home/moi/Images/capture.png")),
        );
        let l = ligne_bilan(7, 2, &b, 3);
        assert_eq!(
            l,
            "editeur export 7 2 copie echec recent echec fichier fait en 3 ms"
        );
        assert!(!l.contains('/') && !l.contains('\n'));
        let attendu = [
            (RaisonFermeture::Propre, "editeur ferme 7 (propre)"),
            (RaisonFermeture::SansGarder, "editeur ferme 7 (sans garder)"),
            (
                RaisonFermeture::CopierFermer,
                "editeur ferme 7 (copier et fermer)",
            ),
            (RaisonFermeture::Arret, "editeur ferme 7 (arret)"),
            (RaisonFermeture::PageMuette, "editeur ferme 7 (page muette)"),
            (RaisonFermeture::Impossible, "editeur ferme 7 (impossible)"),
        ];
        for (r, l) in attendu {
            assert_eq!(ligne_fermeture(7, r), l);
        }
        let ecran = Ecran {
            rect: Rect {
                x: 0,
                y: 0,
                largeur: 1024,
                hauteur: 768,
            },
            zone_utile: Rect {
                x: 0,
                y: 0,
                largeur: 1024,
                hauteur: 728,
            },
            echelle: 1.0,
        };
        let f = Rect {
            x: 112,
            y: 64,
            largeur: 800,
            hauteur: 600,
        };
        assert_eq!(
            ligne_ouvert(7, (760, 516), &ecran, &f),
            "editeur ouvert 7 760x516 ecran 0,0 1024x768 echelle 1.00 fenetre 112,64 800x600"
        );
        assert_eq!(
            ligne_vue(7, &f, 1.0),
            "editeur vue 7 112,64 800x600 zoom 1.00"
        );
        assert_eq!(
            ligne_vue(7, &f, 0.456),
            "editeur vue 7 112,64 800x600 zoom 0.46"
        );
        assert_eq!(
            ligne_glisser(7, &IssueGlisser::Depose),
            "editeur glisser 7 depose"
        );
        assert_eq!(
            ligne_glisser(7, &IssueGlisser::Annule),
            "editeur glisser 7 annule"
        );
        assert_eq!(
            ligne_glisser(7, &IssueGlisser::Refuse("ole".into())),
            "editeur glisser 7 refuse (ole)"
        );
        let m = message_impossible();
        assert_eq!(
            (m.genre, m.titre.as_str(), m.detail.as_str()),
            (Genre::Erreur, "Impossible d'ouvrir l'éditeur", "")
        );
        assert!(m.actions.is_empty());
    }

    /// L'éditeur n'est ni un état ni un déclencheur (§4.4, R4) : la table est celle de la
    /// 0.4 « Confort », recopiée ici, et aucun déclencheur ne parle de l'éditeur.
    #[test]
    fn editeur_hors_automate() {
        use Declencheur as D;
        use Etat as E;
        let confort: &[(Etat, Declencheur, Etat)] = &[
            (E::Repos, D::DemandeCapture(Mode::Zone), E::Gel),
            (E::Repos, D::DemandeCapture(Mode::PleinEcran), E::Gel),
            (E::Repos, D::DemandeCapture(Mode::Fenetre), E::Gel),
            (E::Gel, D::PhotoPrise(Mode::Zone), E::Selection),
            (E::Gel, D::PhotoPrise(Mode::Fenetre), E::Selection),
            (E::Gel, D::PhotoPrise(Mode::PleinEcran), E::Decoupe),
            (E::Gel, D::GelImpossible, E::Repos),
            (E::Selection, D::ZoneChoisie, E::Decoupe),
            (E::Selection, D::SelectionAnnulee, E::Repos),
            (E::Selection, D::EcransChanges, E::Repos),
            (E::Selection, D::SessionVerrouillee, E::Repos),
            (E::Selection, D::BureauSecurise, E::Repos),
            (E::Selection, D::SelectionMuette, E::Repos),
            (E::Selection, D::SelectionImpossible, E::Repos),
            (E::Decoupe, D::DecoupeImpossible, E::Repos),
            (E::Decoupe, D::SortieSansBilan, E::Repos),
            (E::Decoupe, D::ImagePrete, E::Souffle),
            (E::Decoupe, D::ImagePreteSansVignette, E::Repos),
            (E::Souffle, D::SouffleTermine, E::Vignette),
            (E::Souffle, D::VignetteImpossible, E::Repos),
            (E::Vignette, D::VignetteFermee, E::Repos),
            (E::Vignette, D::DelaiEcoule, E::Repos),
            (E::Vignette, D::GlisserReussi, E::Repos),
            (E::Vignette, D::VignetteImpossible, E::Repos),
            (E::Vignette, D::CaptureSupprimee, E::Repos),
            (E::Vignette, D::DemandeCapture(Mode::Zone), E::Gel),
            (E::Vignette, D::DemandeCapture(Mode::PleinEcran), E::Gel),
            (E::Vignette, D::DemandeCapture(Mode::Fenetre), E::Gel),
            (E::Repos, D::DemandeCapture(Mode::DerniereZone), E::Gel),
            (E::Vignette, D::DemandeCapture(Mode::DerniereZone), E::Gel),
            (E::Gel, D::PhotoPrise(Mode::DerniereZone), E::Decoupe),
            (E::Repos, D::DemandeDecompte(Mode::Zone), E::Decompte),
            (E::Repos, D::DemandeDecompte(Mode::PleinEcran), E::Decompte),
            (E::Repos, D::DemandeDecompte(Mode::Fenetre), E::Decompte),
            (
                E::Repos,
                D::DemandeDecompte(Mode::DerniereZone),
                E::Decompte,
            ),
            (E::Vignette, D::DemandeDecompte(Mode::Zone), E::Decompte),
            (
                E::Vignette,
                D::DemandeDecompte(Mode::PleinEcran),
                E::Decompte,
            ),
            (E::Vignette, D::DemandeDecompte(Mode::Fenetre), E::Decompte),
            (
                E::Vignette,
                D::DemandeDecompte(Mode::DerniereZone),
                E::Decompte,
            ),
            (E::Decompte, D::DecompteFini(Mode::Zone), E::Gel),
            (E::Decompte, D::DecompteFini(Mode::PleinEcran), E::Gel),
            (E::Decompte, D::DecompteFini(Mode::Fenetre), E::Gel),
            (E::Decompte, D::DecompteFini(Mode::DerniereZone), E::Gel),
            (E::Decompte, D::DecompteAnnule, E::Repos),
        ];
        assert_eq!(PASSAGES, confort);
        for (depart, d, arrivee) in PASSAGES {
            for texte in [format!("{depart:?}"), d.libelle(), format!("{arrivee:?}")] {
                assert!(!texte.to_lowercase().contains("editeur"), "{texte}");
            }
        }
        // L'entrée dans l'éditeur passe par la ligne existante Vignette → Repos.
        assert!(PASSAGES.contains(&(E::Vignette, D::VignetteFermee, E::Repos)));
    }
}
