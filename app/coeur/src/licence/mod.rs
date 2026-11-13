//! Licence : lit et vérifie une licence collée (avenant 0.5 « Licence », §2 et §3.1). Pur,
//! sans Tauri ni système ; ne lit aucun fichier (le rangement est dans `sortie`).
//!
//! Format : `BS1.<charge>.<signature>`, base64 pour URL sans remplissage ; la signature
//! Ed25519 des octets de la charge décodée est vérifiée avant de lire le JSON.
//!
//! Ordre des refus : vide, trop longue (bornée avant tout décodage), forme (préfixe,
//! parties, base64 strict, taille de la signature), signature, puis seulement la charge
//! (JSON, `id`) et l'édition.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ed25519_compact::{PublicKey, Signature};
use serde::Deserialize;

#[cfg(test)]
mod tests;

/// Clé d'essai des versions d'avant la 1.0. Le lot « Pro » la remplacera par la clé de
/// vente : les licences d'essai cesseront alors de marcher.
const CLE_ESSAI: [u8; 32] = [
    0xbc, 0xcc, 0xcf, 0x56, 0x5e, 0x24, 0xd2, 0x63, 0xc9, 0x0f, 0x8f, 0x39, 0x62, 0x33, 0xd0, 0xf1,
    0x5c, 0xe0, 0xd4, 0xc9, 0x44, 0x0f, 0xbc, 0x1d, 0x82, 0xa7, 0x62, 0xe5, 0xf8, 0x7f, 0x86, 0x11,
];

/// Préfixe du format.
const PREFIXE: &str = "BS1";

/// Longueur totale permise, en caractères, une fois les deux bouts nettoyés.
pub const LONGUEUR_MAX: usize = 1000;

/// Longueur permise de l'identifiant (il va au journal).
const ID_MAX: usize = 40;

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

/// Le texte que `verifier` juge : espaces, tabulations et fins de ligne retirés des deux
/// bouts. C'est aussi lui qui est rangé (avenant « Licence », gardé nettoyé).
pub(crate) fn nettoyer(texte: &str) -> &str {
    texte.trim_matches([' ', '\t', '\r', '\n'])
}

/// Pour les tests : même chose avec une autre liste de clés.
pub(crate) fn verifier_avec(texte: &str, cles: &[[u8; 32]]) -> Result<Licence, Refus> {
    let texte = nettoyer(texte);
    if texte.is_empty() {
        return Err(Refus::Vide);
    }
    if texte.chars().nth(LONGUEUR_MAX).is_some() {
        return Err(Refus::TropLongue);
    }
    let mut parties = texte.split('.');
    let (Some(PREFIXE), Some(charge), Some(signature), None) = (
        parties.next(),
        parties.next(),
        parties.next(),
        parties.next(),
    ) else {
        return Err(Refus::Forme);
    };
    let charge = URL_SAFE_NO_PAD.decode(charge).map_err(|_| Refus::Forme)?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| Refus::Forme)?;
    let signature = Signature::from_slice(&signature).map_err(|_| Refus::Forme)?;
    let signee = cles.iter().any(|cle| {
        PublicKey::from_slice(cle).is_ok_and(|cle| cle.verify(&charge, &signature).is_ok())
    });
    if !signee {
        return Err(Refus::Signature);
    }
    lire_charge(&charge)
}

/// La charge, une fois sa signature vérifiée. Un champ inconnu est ignoré.
#[derive(Deserialize)]
struct Charge {
    e: String,
    id: String,
    #[serde(default)]
    essai: bool,
}

fn lire_charge(octets: &[u8]) -> Result<Licence, Refus> {
    let charge: Charge = serde_json::from_slice(octets).map_err(|_| Refus::Charge)?;
    if !id_valide(&charge.id) {
        return Err(Refus::Charge);
    }
    if charge.e != "pro" {
        return Err(Refus::Edition);
    }
    Ok(Licence {
        id: charge.id,
        essai: charge.essai,
    })
}

/// De 1 à `ID_MAX` caractères ASCII imprimables (espace compris).
fn id_valide(id: &str) -> bool {
    (1..=ID_MAX).contains(&id.len()) && id.bytes().all(|o| (0x20..=0x7e).contains(&o))
}
