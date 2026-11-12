//! Décisions de l'éditeur d'annotation (fonctions pures, contrat 0.4 « Éditeur », §4 à §8) :
//! quand Annoter est possible, ce que vaut un export reçu de la page, ce qu'il faut sortir,
//! s'il faut demander avant de fermer, que faire d'une nouvelle capture, et les lignes du
//! journal. Les gestes sont dans `application/mod.rs` et `fenetre_editeur.rs`, la fenêtre
//! dans `fenetres/editeur.rs`, le document et le dessin dans la page (`interface/editeur/`).
//!
//! SOCLE : types, constantes et signatures posés ; les corps marqués « Provisoire (A) »
//! sont remplacés par le chantier A, avec les tests du §12.1.

use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use serde::Serialize;

use crate::sortie::Bilan;
use crate::systeme::gel::Pixels;

/// Côté maximal d'une capture annotable, pixels (R11).
pub const COTE_MAX: u32 = 16_384;
/// Nombre maximal de pixels d'une capture annotable (un peu plus que 3 × 4K, R11).
pub const PIXELS_MAX: u64 = 25_000_000;
/// Délai donné à la page pour dire `editeur_pret` après la création de la fenêtre.
pub const DELAI_PAGE: Duration = Duration::from_secs(5);
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
}

/// Un export parti dans le fil `editeur-sortie`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportEnCours {
    pub numero: u64,
    pub but: But,
    pub version: u64,
    pub debut: Instant,
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

/// Lit les en-têtes `x-editeur-*` d'un export.
pub fn lire_en_tete(
    id: Option<&str>,
    version: Option<&str>,
    but: Option<&str>,
    largeur: Option<&str>,
    hauteur: Option<&str>,
) -> Result<EnTete, Refus> {
    // Provisoire (A) : tout export est refusé.
    let _ = (id, version, but, largeur, hauteur);
    Err(Refus::Id)
}

/// Accepte un export lu (`octets` : longueur du corps) ; un export de plus de
/// `DELAI_SORTIE` est oublié.
pub fn accepter_export(
    etat: &EtatEditeur,
    e: &EnTete,
    octets: usize,
    maintenant: Instant,
) -> Result<(), Refus> {
    // Provisoire (A) : tout export est refusé.
    let _ = (etat, e, octets, maintenant);
    Err(Refus::Id)
}

/// Ce que fait un export accepté du but `but` pour la version `version`.
pub fn sortie_pour(etat: &EtatEditeur, but: But, version: u64) -> Sortie {
    // Provisoire (A) : rien n'est sorti.
    let _ = (etat, but, version);
    Sortie::DejaFait
}

/// Que faire d'un Annoter selon l'éditeur ouvert.
pub fn arrivee(ouvert: Option<&EtatEditeur>) -> Arrivee {
    // Provisoire (A) : toujours ouvrir.
    let _ = ouvert;
    Arrivee::Ouvrir
}

/// `Directe` si le document est vide ou si `exportee == Some(version)`.
pub fn fermeture(version: u64, vide: bool, exportee: Option<u64>) -> Fermeture {
    // Provisoire (A) : toujours demander.
    let _ = (version, vide, exportee);
    Fermeture::Demander
}

/// Le titre montré par la page après un export en échec (§7.4), ou rien.
pub fn message_echec(but: But, bilan: &Bilan) -> Option<&'static str> {
    // Provisoire (A) : rien de montré.
    let _ = (but, bilan);
    None
}

/// `editeur export <id> <n> <but> version <v> <l>x<h>`.
pub fn ligne_export(id: u64, numero: u64, but: But, version: u64, l: u32, h: u32) -> String {
    // Provisoire (A) : ligne vide.
    let _ = (id, numero, but, version, l, h);
    String::new()
}

/// `editeur export <id> <n> copie … recent … fichier … en <ms> ms`.
pub fn ligne_bilan(id: u64, numero: u64, bilan: &Bilan, ms: u128) -> String {
    // Provisoire (A) : ligne vide.
    let _ = (id, numero, bilan, ms);
    String::new()
}

/// `editeur ferme <id> (<raison>)`.
pub fn ligne_fermeture(id: u64, raison: RaisonFermeture) -> String {
    // Provisoire (A) : ligne vide.
    let _ = (id, raison);
    String::new()
}
