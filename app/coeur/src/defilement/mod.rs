//! Assemblage de la capture défilante (contrat 0.6, §13.2, D10 à D13) : mesurer le
//! décalage entre deux images de la zone, reconnaître les bandes fixes (en-tête, pied),
//! ajouter les lignes nouvelles, rendre l'image finale. Module pur : il n'utilise que
//! `systeme` (le type `Pixels`), ne lit ni n'écrit rien, ne connaît ni Tauri ni le temps.
//!
//! Tolérance retenue après la sonde (§18) : ± 8 par canal, une ligne est égale si 90 % de
//! ses pixels le sont, un décalage est accepté si 97 % des lignes du recouvrement le sont ;
//! 1 à 4 lignes entières très différentes sont tolérées au bord du recouvrement (échelles
//! 125 et 150 %).
//!
//! Socle : corps inertes (rien n'est mesuré ni ajouté ; `finir` refuse).

use crate::systeme::gel::Pixels;

/// La comparaison tolérante de deux images (D12).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tolerance {
    /// Écart permis par canal (rouge, vert, bleu).
    pub canal: u8,
    /// Part des pixels d'une ligne qui doivent être égaux pour que la ligne le soit.
    pub part_ligne: f32,
    /// Part des lignes utiles du recouvrement qui doivent être égales pour retenir un
    /// décalage.
    pub part_lignes: f32,
}

/// La tolérance de la capture défilante (D12, §18).
pub const TOLERANCE: Tolerance = Tolerance {
    canal: 8,
    part_ligne: 0.90,
    part_lignes: 0.97,
};

/// Recouvrement minimal entre deux images pour mesurer un décalage, en pixels (D10) ; un
/// quart de la hauteur utile si c'est moins.
pub const RECOUVREMENT_MIN: u32 = 48;

/// Ce que vaut une image contre la dernière image gardée (D10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mesure {
    /// La page est descendue de `n` lignes : `n` lignes nouvelles en bas.
    Avance(u32),
    /// Rien n'a bougé.
    Immobile,
    /// La page est remontée de `n` lignes : rien n'est ajouté.
    Recul(u32),
    /// Aucun recouvrement suffisant : la page est allée trop loin (« Trop vite »).
    Perdue,
    /// Le recouvrement ne porte pas assez de lignes distinctes (page unie) : rien n'est
    /// ajouté, l'image suivante décidera.
    Ambigue,
}

/// Les bandes fixes de la zone (D11) : lignes du haut et du bas qui ne défilent pas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Fixes {
    pub haut: u32,
    pub bas: u32,
}

/// Mesure `apres` contre `avant` (même taille), hors des bandes `fixes` ; `attendu` : le
/// décalage attendu du pas, s'il est connu (départage une mesure ambiguë).
pub fn mesurer(
    avant: &Pixels,
    apres: &Pixels,
    fixes: Fixes,
    attendu: Option<i32>,
    t: &Tolerance,
) -> Mesure {
    let _ = (avant, apres, fixes, attendu, t);
    Mesure::Ambigue
}

/// Les bandes fixes de deux images séparées par une `avance` (D11) : au moins 8 lignes
/// égales à la même place, pas toutes unies.
pub fn fixes(avant: &Pixels, apres: &Pixels, avance: u32, t: &Tolerance) -> Fixes {
    let _ = (avant, apres, avance, t);
    Fixes::default()
}

/// Taille maximale de l'image assemblée (D13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limite {
    /// Hauteur maximale, en pixels.
    pub hauteur: u32,
    /// Nombre maximal de pixels (largeur × hauteur).
    pub pixels: u64,
}

/// Ce qu'a fait un ajout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ajout {
    /// `n` lignes ajoutées.
    Ajoutees(u32),
    /// Rien d'ajouté (immobile, recul, perdue, ambiguë).
    Rien,
    /// La limite est atteinte : les `n` lignes qui tenaient sont ajoutées, la capture finit.
    Limite(u32),
}

/// L'image en cours d'assemblage : bandes de 1 024 lignes, jamais une réallocation de
/// toute l'image ; une seule copie à `finir`.
#[derive(Debug)]
pub struct Assemblage {
    largeur: u32,
    hauteur: u32,
    fixes: Fixes,
    limite: Limite,
}

impl Assemblage {
    /// Commence avec la première image. Erreur : image vide ou mémoire refusée.
    pub fn nouveau(premiere: &Pixels, limite: Limite) -> Result<Assemblage, String> {
        Ok(Assemblage {
            largeur: premiere.largeur,
            hauteur: 0,
            fixes: Fixes::default(),
            limite,
        })
    }

    /// Ajoute les lignes nouvelles de `image` selon `mesure` (faite contre la dernière
    /// image gardée).
    pub fn ajouter(&mut self, image: &Pixels, mesure: &Mesure) -> Ajout {
        let _ = (image, mesure, self.limite);
        Ajout::Rien
    }

    /// Les bandes fixes reconnues.
    pub fn fixes(&self) -> Fixes {
        self.fixes
    }

    /// La hauteur assemblée jusqu'ici, en pixels.
    pub fn hauteur(&self) -> u32 {
        self.hauteur
    }

    /// L'aperçu en direct : le bas de l'image assemblée, réduit à `largeur` et au plus
    /// `hauteur_max` lignes.
    pub fn apercu(&self, largeur: u32, hauteur_max: u32) -> Pixels {
        let _ = (largeur, hauteur_max, self.largeur);
        Pixels {
            largeur: 0,
            hauteur: 0,
            rgba: Vec::new(),
        }
    }

    /// L'image finale, le pied de `derniere` ajouté (une seule copie).
    pub fn finir(self, derniere: &Pixels) -> Result<Pixels, String> {
        let _ = derniere;
        Err("assemblage non construit".into())
    }
}
