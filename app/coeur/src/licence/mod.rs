//! Licence : lit et vérifie une licence collée (avenant 0.5 « Licence », §2 et §3.1). Pur,
//! sans Tauri ni système ; ne lit aucun fichier (le rangement est dans `sortie`).
//!
//! Format : `BS1.<charge>.<signature>`, base64 pour URL sans remplissage ; la signature
//! Ed25519 des octets de la charge décodée est vérifiée avant de lire le JSON.
//!
//! Socle : signatures, chaque licence est refusée (chantier L pour les corps).

/// Clé d'essai des versions d'avant la 1.0. Le lot « Pro » la remplacera par la clé de
/// vente : les licences d'essai cesseront alors de marcher.
const CLE_ESSAI: [u8; 32] = [
    0xbc, 0xcc, 0xcf, 0x56, 0x5e, 0x24, 0xd2, 0x63, 0xc9, 0x0f, 0x8f, 0x39, 0x62, 0x33, 0xd0, 0xf1,
    0x5c, 0xe0, 0xd4, 0xc9, 0x44, 0x0f, 0xbc, 0x1d, 0x82, 0xa7, 0x62, 0xe5, 0xf8, 0x7f, 0x86, 0x11,
];

/// Les clés publiques acceptées.
const CLES: [[u8; 32]; 1] = [CLE_ESSAI];

/// Une licence acceptée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Licence {
    pub id: String,
    pub essai: bool,
}

/// Pourquoi une licence est refusée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refus {
    Vide,
    TropLongue,
    Forme,
    Charge,
    Edition,
    Signature,
}

impl Refus {
    /// Le code du journal et de la commande `licence_activer`.
    pub fn code(&self) -> &'static str {
        match self {
            Refus::Vide => "vide",
            Refus::TropLongue => "trop_longue",
            Refus::Forme => "forme",
            Refus::Charge => "charge",
            Refus::Edition => "edition",
            Refus::Signature => "signature",
        }
    }
}

/// Lit et vérifie une licence collée. Ne lit aucun fichier.
pub fn verifier(texte: &str) -> Result<Licence, Refus> {
    verifier_avec(texte, &CLES)
}

/// Pour les tests : même chose avec une autre liste de clés.
pub(crate) fn verifier_avec(texte: &str, _cles: &[[u8; 32]]) -> Result<Licence, Refus> {
    if texte.trim().is_empty() {
        Err(Refus::Vide)
    } else {
        Err(Refus::Signature)
    }
}
