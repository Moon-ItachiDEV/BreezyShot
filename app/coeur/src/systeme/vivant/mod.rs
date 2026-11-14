//! Images vivantes (contrat 0.6, §13.1) : prendre en continu un rectangle de l'écran, sans
//! les fenêtres de l'app exclues des captures (la fenêtre de contrôle de la capture
//! défilante). Le moyen est caché ici : `BitBlt` de la zone, `SRCCOPY` sans `CAPTUREBLT`
//! (décision R2 après la sonde) ; rien d'autre ne suppose comment l'image est prise.
//!
//! Façade commune : `windows.rs` la réalise, `autre.rs` dit « non disponible » ailleurs
//! (Linux, et macOS : la capture défilante y est absente).
//!
//! Tout se compte en pixels réels du bureau virtuel, comme `gel`.

use std::time::{Duration, Instant};

use super::gel::{Pixels, Rect};

#[cfg(windows)]
#[path = "windows.rs"]
mod plateforme;

#[cfg(not(windows))]
#[path = "autre.rs"]
mod plateforme;

/// Vrai si les images vivantes existent sur ce système. Faux : la capture défilante est
/// absente, même avec une licence (`application::actions_selon`).
pub fn disponible() -> bool {
    plateforme::disponible()
}

/// Une prise d'images vivantes d'un rectangle de l'écran : le tampon du système est gardé
/// et réutilisé d'une image à l'autre ; tout est rendu à la destruction. À garder dans un
/// seul fil (le fil `defilante`).
#[derive(Debug)]
pub struct Prise {
    interne: plateforme::Prise,
}

/// Ouvre une prise sur `zone` (pixels réels du bureau), coupée au bureau virtuel : `zone()`
/// dit ce qui est vraiment pris. Erreur : zone vide ou hors du bureau, tampon refusé, ou
/// système sans images vivantes.
pub fn ouvrir(zone: Rect) -> Result<Prise, String> {
    plateforme::ouvrir(zone).map(|interne| Prise { interne })
}

impl Prise {
    /// Le rectangle pris, en pixels réels du bureau.
    pub fn zone(&self) -> Rect {
        plateforme::zone(&self.interne)
    }

    /// Une image RGBA (alpha 255) de la zone dans `dest`, réutilisé (sa taille est mise à
    /// celle de la zone) ; rend la durée de la prise. Refuse tant que l'écran figé est
    /// montré (`gel::ecran_fige_montre`, la garde de `gel::figer_bureau`) : l'image serait
    /// celle de l'écran figé. La garde porte sur la fenêtre, pas sur la dernière image
    /// composée : juste après avoir caché l'écran figé, attendre le compositeur (au moins un
    /// `DwmFlush`, D4) avant la première prise.
    pub fn prendre(&mut self, dest: &mut Pixels) -> Result<Duration, String> {
        plateforme::prendre(&mut self.interne, dest)
    }
}

// ---------------------------------------------------------------------------------------
// Vidéo et GIF (contrat 0.7, §13.1, F12, F33) : une source à cadence fixe, qui s'ajoute à
// `Prise` (inchangée : la capture défilante s'en sert). Le moyen est caché ici : la
// duplication de bureau de l'écran de la zone d'abord, `BitBlt` en repli (D1).
//
// Ce qui ne dépend pas de Windows est ici, pur et testé partout : le choix de la sortie
// (écran de la zone, écran tourné), la zone vue par une sortie, les rectangles sales et
// déplacés croisés avec la zone, la copie qui dit si un pixel a changé, la reprise après
// un accès perdu (F33) et les séries d'échecs du journal. `windows.rs` n'y ajoute que les
// appels au système.

/// Le moyen d'une source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moyen {
    /// La duplication de bureau DXGI de l'écran de la zone.
    Duplication,
    /// Le `BitBlt` de la zone (celui de `Prise`).
    BitBlt,
}

impl Moyen {
    /// Nom au journal : `duplication`, `bitblt`.
    pub fn nom(self) -> &'static str {
        match self {
            Moyen::Duplication => "duplication",
            Moyen::BitBlt => "bitblt",
        }
    }
}

/// Ce qu'une prise de la source a rendu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Image {
    /// Une image neuve est dans la destination.
    Nouvelle,
    /// Rien n'a changé dans la zone depuis la dernière prise : destination intacte.
    Inchangee,
    /// Pas d'image pour l'instant (accès perdu) : destination intacte ; la source se
    /// recrée d'elle-même, au plus toutes les 500 ms (F33).
    Perdue,
}

/// Une source d'images à cadence fixe sur une zone de l'écran, pour un film. Vit dans un
/// seul fil (le fil `film`) ; tout est rendu à la destruction.
#[derive(Debug)]
pub struct Source {
    interne: plateforme::Source,
}

/// Ouvre une source sur `zone` (pixels réels du bureau). `ecran` : le rectangle de l'écran
/// qui contient `zone` ; le périphérique Direct3D est créé sur la carte graphique qui porte
/// cet écran (F12). La duplication d'abord ; en repli (refusée, écran tourné), `BitBlt`.
/// Erreur : zone vide ou hors du bureau, système sans images vivantes.
pub fn ouvrir_source(zone: Rect, ecran: Rect) -> Result<Source, String> {
    plateforme::ouvrir_source(zone, ecran).map(|interne| Source { interne })
}

impl Source {
    /// Le moyen en service.
    pub fn moyen(&self) -> Moyen {
        plateforme::moyen_source(&self.interne)
    }

    /// Le rectangle pris, en pixels réels du bureau.
    pub fn zone(&self) -> Rect {
        plateforme::zone_source(&self.interne)
    }

    /// Une image RGBA (alpha 255) de la zone dans `dest`, réutilisé ; rend aussi la durée
    /// de la prise. `dest` doit tenir l'image que cette source y a mise la dernière fois
    /// (la dernière image brute du fil `film`, F14). `Inchangee` : aucun rectangle sale ne
    /// touche la zone depuis la dernière prise, ou aucun pixel n'a changé (`BitBlt`), `dest`
    /// n'est pas touché ; une image où seul le pointeur a bougé est `Inchangee` (le
    /// pointeur n'est jamais dans l'image). `Perdue` : pas d'image pour l'instant (accès
    /// perdu, prise en échec, ou rien encore reçu depuis l'ouverture), `dest` n'est pas
    /// touché ; la duplication se refait d'elle-même à la prise suivante, puis au plus
    /// toutes les 500 ms (F33). Un échec autre qu'un accès perdu écrit une ligne
    /// `film: prise en echec (<code>)` par série (§8). Erreur : l'écran figé est montré
    /// (la garde de `Prise`, lue avant la prise et après la copie), rien d'autre.
    pub fn prendre(&mut self, dest: &mut Pixels) -> Result<(Image, Duration), String> {
        plateforme::prendre_source(&mut self.interne, dest)
    }

    /// Passe à `BitBlt` pour le reste du film (F33, après `SOURCE_PERDUE_MAX`) ; `repli`
    /// le dira (`bitblt`, `acces perdu`). Déjà en `BitBlt` : rien. Erreur : tampon refusé.
    pub fn replier(&mut self) -> Result<(), String> {
        plateforme::replier_source(&mut self.interne)
    }

    /// Un changement de moyen a eu lieu depuis le dernier appel : le moyen et la raison
    /// (journal `film source <moyen> (<raison>)`). Le repli pris à l'ouverture
    /// (duplication refusée, écran tourné, zone hors des sorties) est rendu au premier
    /// appel ; `None` au premier appel veut dire que le moyen est `moyen()`, sans raison.
    pub fn repli(&mut self) -> Option<(Moyen, String)> {
        plateforme::repli_source(&mut self.interne)
    }
}

/// La zone prise : `zone` coupée au bureau virtuel. Erreur si rien n'en reste.
#[cfg_attr(not(windows), allow(dead_code))]
fn couper_au_bureau(zone: Rect, bureau: Rect) -> Result<Rect, String> {
    if zone.est_vide() {
        return Err(format!("zone vide ({}x{})", zone.largeur, zone.hauteur));
    }
    zone.intersection(bureau)
        .ok_or_else(|| "zone hors du bureau".to_string())
}

/// Met `dest` à la taille `largeur` × `hauteur` (RGBA) en gardant son tampon : aucune
/// allocation quand la taille ne change pas (ni quand elle diminue). Rend le nombre
/// d'octets.
#[cfg_attr(not(windows), allow(dead_code))]
fn preparer_destination(dest: &mut Pixels, largeur: u32, hauteur: u32) -> Result<usize, String> {
    let octets = (largeur as usize)
        .checked_mul(hauteur as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or("zone trop grande")?;
    dest.largeur = largeur;
    dest.hauteur = hauteur;
    // `resize` ne réalloue que si la capacité manque.
    dest.rgba.resize(octets, 0);
    Ok(octets)
}

// ---------------------------------------------------------------------------------------
// Source d'un film : logique pure (contrat 0.7, F12, F13, F33).

/// Délai entre deux essais de refaire la duplication perdue (F33) : le premier essai part
/// à la prise qui suit la perte, les suivants au plus toutes les 500 ms.
#[cfg_attr(not(windows), allow(dead_code))]
const DELAI_REPRISE: Duration = Duration::from_millis(500);

/// Raison du repli pris par `Source::replier` (journal `film source bitblt (acces perdu)`).
#[cfg_attr(not(windows), allow(dead_code))]
const RAISON_ACCES_PERDU: &str = "acces perdu";

/// Une sortie (un écran) d'une carte graphique, telle que DXGI la décrit.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SortieVue {
    /// Son rectangle sur le bureau virtuel, en pixels réels.
    bureau: Rect,
    /// L'écran est tourné (90, 180 ou 270 degrés) : la duplication rendrait une image non
    /// tournée (F12 : repli).
    tournee: bool,
}

/// La sortie à dupliquer pour une zone.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChoixSortie {
    /// L'indice de la sortie dans la liste donnée.
    Sortie(usize),
    /// La sortie qui porte la zone est tournée.
    Tournee,
    /// Aucune sortie ne contient la zone entière.
    Aucune,
}

impl ChoixSortie {
    /// Raison du repli au journal (`film source bitblt (<raison>)`), sans accent.
    #[cfg_attr(not(windows), allow(dead_code))]
    fn raison(self) -> Option<&'static str> {
        match self {
            ChoixSortie::Sortie(_) => None,
            ChoixSortie::Tournee => Some("ecran tourne"),
            ChoixSortie::Aucune => Some("zone hors des sorties"),
        }
    }
}

/// Choisit la sortie qui porte `zone` (F12). D'abord celle dont le rectangle est `ecran`
/// (l'écran de la zone, donné par l'appelant) et qui contient la zone ; sinon la première
/// qui contient la zone entière. La duplication d'une sortie ne voit que cette sortie : une
/// zone à cheval sur deux écrans n'a pas de sortie (`Aucune`).
#[cfg_attr(not(windows), allow(dead_code))]
fn choisir_sortie(zone: Rect, ecran: Rect, sorties: &[SortieVue]) -> ChoixSortie {
    let porte = |s: &SortieVue| s.bureau.englobe(zone);
    let trouvee = sorties
        .iter()
        .position(|s| s.bureau == ecran && porte(s))
        .or_else(|| sorties.iter().position(porte));
    match trouvee {
        Some(i) if sorties[i].tournee => ChoixSortie::Tournee,
        Some(i) => ChoixSortie::Sortie(i),
        None => ChoixSortie::Aucune,
    }
}

/// La zone vue par une sortie : ses coordonnées depuis le coin haut gauche de la sortie.
/// `None` si la sortie ne la contient pas entière (le mode a changé, l'écran a bougé).
#[cfg_attr(not(windows), allow(dead_code))]
fn zone_dans_la_sortie(zone: Rect, sortie: Rect) -> Option<Rect> {
    if !sortie.englobe(zone) {
        return None;
    }
    Some(Rect {
        x: zone.x - sortie.x,
        y: zone.y - sortie.y,
        ..zone
    })
}

/// Vrai si un rectangle sale ou déplacé d'une image de la duplication touche la zone
/// (tout en coordonnées de la sortie). Un déplacement touche la zone par sa destination
/// ou par sa source (F13 : « rien n'a changé dans la zone » se lit sur la zone, pas sur
/// l'écran entier).
#[cfg_attr(not(windows), allow(dead_code))]
fn zone_touchee(zone: Rect, deplaces: &[(Rect, Rect)], sales: &[Rect]) -> bool {
    let touche = |r: &Rect| r.intersection(zone).is_some();
    deplaces
        .iter()
        .any(|(source, destination)| touche(source) || touche(destination))
        || sales.iter().any(touche)
}

/// Copie une image BGRA (lignes de `pas` octets, `largeur` × `hauteur` pixels) dans `dest`
/// en RGBA, alpha 255, et dit si un pixel a changé : `dest` doit tenir l'image précédente
/// de la même source (le contrat de `Source::prendre`). Une taille nouvelle est un
/// changement. Les pixels égaux sont réécrits à l'identique : `dest` ne change donc que si
/// la réponse est vraie. Erreur (sans rien écrire) si la source est trop courte.
#[cfg_attr(not(windows), allow(dead_code))]
fn copier_bgra(
    source: &[u8],
    pas: usize,
    largeur: u32,
    hauteur: u32,
    dest: &mut Pixels,
) -> Result<bool, String> {
    let ligne = (largeur as usize)
        .checked_mul(4)
        .ok_or("zone trop grande")?;
    if pas < ligne {
        return Err(format!("pas de ligne {pas} sous la largeur {ligne}"));
    }
    if hauteur > 0 {
        let requis = (hauteur as usize - 1)
            .checked_mul(pas)
            .and_then(|n| n.checked_add(ligne))
            .ok_or("zone trop grande")?;
        if source.len() < requis {
            return Err(format!("image source courte ({} < {requis})", source.len()));
        }
    }
    let meme_taille = dest.largeur == largeur
        && dest.hauteur == hauteur
        && dest.rgba.len() == ligne * hauteur as usize;
    preparer_destination(dest, largeur, hauteur)?;
    let mut change = !meme_taille;
    if ligne == 0 {
        return Ok(change);
    }
    for (y, sortie) in dest.rgba.chunks_exact_mut(ligne).enumerate() {
        let entree = &source[y * pas..y * pas + ligne];
        for (s, d) in entree.chunks_exact(4).zip(sortie.chunks_exact_mut(4)) {
            let px = [s[2], s[1], s[0], 255];
            if d != px {
                d.copy_from_slice(&px);
                change = true;
            }
        }
    }
    Ok(change)
}

/// La reprise d'une duplication perdue (F33) : quand réessayer, et depuis quand l'image
/// manque. L'heure est donnée par l'appelant (les tests la choisissent).
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Reprise {
    /// Perdue depuis : pas d'image rendue depuis cet instant.
    depuis: Option<Instant>,
    /// Le prochain essai permis ; `None` : tout de suite.
    prochain: Option<Instant>,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl Reprise {
    /// L'accès est perdu à `a` : le premier essai est permis tout de suite. Une seconde
    /// perte avant la reprise garde l'instant de la première.
    fn perdre(&mut self, a: Instant) {
        self.depuis.get_or_insert(a);
        self.prochain = None;
    }

    /// Vrai tant qu'aucune image n'a été rendue depuis la perte.
    fn perdue(&self) -> bool {
        self.depuis.is_some()
    }

    /// Vrai si un essai de refaire la duplication est permis à `a`.
    fn essai_permis(&self, a: Instant) -> bool {
        self.prochain.is_none_or(|p| a >= p)
    }

    /// L'essai fait à `a` a échoué : le suivant attend `DELAI_REPRISE`.
    fn essai_rate(&mut self, a: Instant) {
        self.prochain = Some(a + DELAI_REPRISE);
    }

    /// Une image est rendue à `a` : fin de la perte ; rend sa durée s'il y en avait une.
    fn image_rendue(&mut self, a: Instant) -> Option<Duration> {
        self.prochain = None;
        self.depuis.take().map(|d| a.saturating_duration_since(d))
    }
}

/// Ce qu'une prise par la duplication doit faire, selon l'état de la duplication.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Etape {
    /// La duplication existe : acquérir une image (délai nul).
    Acquerir,
    /// La duplication est perdue et un essai est permis : la refaire, puis acquérir.
    Refaire,
    /// La duplication est perdue et l'essai attend : pas d'image.
    Attendre,
}

/// Le suivi d'une source par duplication, d'une prise à l'autre (F13, F33) : quand
/// refaire la duplication, quand copier l'image acquise, et ce que rend une prise sans
/// copie. Pur : `windows.rs` lui donne ce que DXGI a répondu et l'heure.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Suivi {
    /// La prochaine image acquise est copiée quels que soient ses rectangles : rien n'a
    /// été copié depuis l'ouverture ou depuis que la duplication a été refaite (ses
    /// premiers rectangles ne disent pas ce que tient la destination), ou la dernière image
    /// acquise a été écartée (ses rectangles sont perdus avec elle).
    forcer: bool,
    /// Une image a été rendue depuis l'ouverture : la destination en tient une.
    deja_rendue: bool,
    reprise: Reprise,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl Suivi {
    fn nouveau() -> Suivi {
        Suivi {
            forcer: true,
            deja_rendue: false,
            reprise: Reprise::default(),
        }
    }

    /// Ce que la prise doit faire à `a` ; `ouverte` : une duplication existe.
    fn etape(&self, ouverte: bool, a: Instant) -> Etape {
        if ouverte {
            Etape::Acquerir
        } else if self.reprise.essai_permis(a) {
            Etape::Refaire
        } else {
            Etape::Attendre
        }
    }

    /// L'accès est perdu à `a` (ou l'acquisition a échoué) : la duplication est à refaire.
    fn perdue(&mut self, a: Instant) {
        self.reprise.perdre(a);
        self.forcer = true;
    }

    /// L'essai de refaire la duplication, fait à `a`, a réussi ou non.
    fn refaite(&mut self, reussie: bool, a: Instant) {
        if reussie {
            self.forcer = true;
        } else {
            self.reprise.essai_rate(a);
        }
    }

    /// Une image a été acquise : vrai s'il faut la copier. `ecran_neuf` : le bureau a été
    /// recomposé (sinon seul le pointeur a bougé, et il n'est jamais dans l'image : F14) ;
    /// `touchee` : un rectangle sale ou déplacé touche la zone (`None` : non lu).
    fn a_copier(&self, ecran_neuf: bool, touchee: Option<bool>) -> bool {
        self.forcer || (ecran_neuf && touchee != Some(false))
    }

    /// L'image acquise a été écartée sans être rendue (garde de l'écran figé).
    fn ecartee(&mut self) {
        self.forcer = true;
    }

    /// Ce que rend une prise sans copie (délai dépassé, rien de neuf dans la zone, ou
    /// duplication perdue) : `Perdue` tant que la destination n'a rien de juste à garder
    /// (perte en cours, ou rien rendu depuis l'ouverture), `Inchangee` sinon.
    fn sans_copie(&self) -> Image {
        if self.reprise.perdue() || !self.deja_rendue {
            Image::Perdue
        } else {
            Image::Inchangee
        }
    }

    /// L'image acquise a été copiée à `a` ; `change` : un pixel au moins diffère de la
    /// destination. Rend ce que la prise rend, et la durée de la perte qui finit ici.
    fn copiee(&mut self, change: bool, a: Instant) -> (Image, Option<Duration>) {
        self.forcer = false;
        self.deja_rendue = true;
        let perte = self.reprise.image_rendue(a);
        let image = if change {
            Image::Nouvelle
        } else {
            Image::Inchangee
        };
        (image, perte)
    }
}

/// Les échecs de prise en série : une ligne de journal par série, pas par image (§8,
/// `film: prise en echec (<code>)`).
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Serie {
    en_echec: bool,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl Serie {
    /// Un échec : vrai s'il ouvre une série (c'est lui qu'on écrit).
    fn echec(&mut self) -> bool {
        !std::mem::replace(&mut self.en_echec, true)
    }

    /// Une prise réussie ferme la série.
    fn succes(&mut self) {
        self.en_echec = false;
    }
}

/// Un code d'erreur du système au journal : `0x887A0026`.
#[cfg_attr(not(windows), allow(dead_code))]
fn code_hex(code: i32) -> String {
    format!("0x{:08X}", code as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i32, y: i32, largeur: i32, hauteur: i32) -> Rect {
        Rect {
            x,
            y,
            largeur,
            hauteur,
        }
    }

    #[test]
    fn zone_coupee_au_bureau() {
        // Deux écrans : un à gauche du principal (x négatifs).
        let bureau = r(-1920, 0, 1920 + 2560, 1440);
        let dedans = r(100, 200, 600, 450);
        assert_eq!(couper_au_bureau(dedans, bureau), Ok(dedans));
        assert_eq!(
            couper_au_bureau(r(-2000, -10, 600, 450), bureau),
            Ok(r(-1920, 0, 520, 440))
        );
        assert_eq!(
            couper_au_bureau(r(2400, 1400, 600, 450), bureau),
            Ok(r(2400, 1400, 160, 40))
        );
        assert_eq!(couper_au_bureau(bureau, bureau), Ok(bureau));
        assert!(couper_au_bureau(r(2560, 0, 10, 10), bureau).is_err());
        assert!(couper_au_bureau(r(-1930, 0, 10, 10), bureau).is_err());
        assert!(couper_au_bureau(r(0, 0, 0, 10), bureau).is_err());
        assert!(couper_au_bureau(r(0, 0, 10, -1), bureau).is_err());
    }

    #[test]
    fn destination_reutilisee() {
        let mut dest = Pixels {
            largeur: 0,
            hauteur: 0,
            rgba: Vec::new(),
        };
        assert_eq!(preparer_destination(&mut dest, 600, 450), Ok(600 * 450 * 4));
        assert_eq!((dest.largeur, dest.hauteur), (600, 450));
        assert_eq!(dest.rgba.len(), 600 * 450 * 4);
        let adresse = dest.rgba.as_ptr();
        let capacite = dest.rgba.capacity();
        dest.rgba.fill(7);
        // Même taille : même tampon, contenu laissé (il sera écrasé par la prise).
        preparer_destination(&mut dest, 600, 450).unwrap();
        assert_eq!(dest.rgba.as_ptr(), adresse);
        assert_eq!(dest.rgba.capacity(), capacite);
        // Plus petite : même tampon.
        preparer_destination(&mut dest, 300, 200).unwrap();
        assert_eq!((dest.largeur, dest.hauteur), (300, 200));
        assert_eq!(dest.rgba.len(), 300 * 200 * 4);
        assert_eq!(dest.rgba.as_ptr(), adresse);
        // Une taille qui déborde : erreur, pas de panique.
        let mut autre = Pixels {
            largeur: 1,
            hauteur: 1,
            rgba: vec![0; 4],
        };
        assert!(preparer_destination(&mut autre, u32::MAX, u32::MAX).is_err());
    }

    // --- Source d'un film : logique pure ---

    fn sortie(x: i32, y: i32, largeur: i32, hauteur: i32) -> SortieVue {
        SortieVue {
            bureau: r(x, y, largeur, hauteur),
            tournee: false,
        }
    }

    #[test]
    fn sortie_de_l_ecran_de_la_zone() {
        // Deux écrans : un 1920 × 1080 à gauche du principal, le principal en 2560 × 1440.
        let gauche = sortie(-1920, 0, 1920, 1080);
        let principal = sortie(0, 0, 2560, 1440);
        let sorties = [principal, gauche];
        let zone = r(-1000, 100, 600, 450);
        assert_eq!(
            choisir_sortie(zone, gauche.bureau, &sorties),
            ChoixSortie::Sortie(1)
        );
        // L'écran donné ne compte que s'il contient la zone ; sinon celui qui la contient.
        assert_eq!(
            choisir_sortie(zone, principal.bureau, &sorties),
            ChoixSortie::Sortie(1)
        );
        assert_eq!(
            choisir_sortie(r(100, 100, 600, 450), r(0, 0, 1, 1), &sorties),
            ChoixSortie::Sortie(0)
        );
        // Tout l'écran : contenu, bord droit et bas exclus.
        assert_eq!(
            choisir_sortie(principal.bureau, principal.bureau, &sorties),
            ChoixSortie::Sortie(0)
        );
        // À cheval sur les deux : aucune sortie ne la voit entière.
        assert_eq!(
            choisir_sortie(r(-300, 100, 600, 450), principal.bureau, &sorties),
            ChoixSortie::Aucune
        );
        assert_eq!(
            choisir_sortie(r(2500, 0, 100, 100), principal.bureau, &sorties),
            ChoixSortie::Aucune
        );
        assert_eq!(
            choisir_sortie(zone, gauche.bureau, &[]),
            ChoixSortie::Aucune
        );
        // Le même écran vu par deux cartes (rendu logiciel et affichage) : l'ordre gagne.
        assert_eq!(
            choisir_sortie(zone, gauche.bureau, &[gauche, gauche]),
            ChoixSortie::Sortie(0)
        );
    }

    #[test]
    fn ecran_tourne_repli() {
        let tournee = SortieVue {
            tournee: true,
            ..sortie(0, 0, 1080, 1920)
        };
        let droit = sortie(1080, 0, 1920, 1080);
        let sorties = [tournee, droit];
        assert_eq!(
            choisir_sortie(r(10, 10, 600, 450), tournee.bureau, &sorties),
            ChoixSortie::Tournee
        );
        assert_eq!(
            choisir_sortie(r(1100, 10, 600, 450), droit.bureau, &sorties),
            ChoixSortie::Sortie(1)
        );
        assert_eq!(ChoixSortie::Tournee.raison(), Some("ecran tourne"));
        assert_eq!(ChoixSortie::Aucune.raison(), Some("zone hors des sorties"));
        assert_eq!(ChoixSortie::Sortie(3).raison(), None);
    }

    #[test]
    fn zone_vue_par_la_sortie() {
        let ecran = r(-1920, -200, 1920, 1080);
        assert_eq!(
            zone_dans_la_sortie(r(-1000, 100, 600, 450), ecran),
            Some(r(920, 300, 600, 450))
        );
        assert_eq!(zone_dans_la_sortie(ecran, ecran), Some(r(0, 0, 1920, 1080)));
        // Le mode est passé à 800 × 600 : la zone n'y tient plus.
        assert_eq!(
            zone_dans_la_sortie(r(700, 100, 200, 200), r(0, 0, 800, 600)),
            None
        );
        assert_eq!(
            zone_dans_la_sortie(r(10, 10, 0, 5), r(0, 0, 800, 600)),
            None
        );
    }

    #[test]
    fn rectangles_croises_avec_la_zone() {
        let zone = r(100, 100, 600, 450);
        // Rien : la zone est intacte.
        assert!(!zone_touchee(zone, &[], &[]));
        // Sale hors de la zone (l'horloge de la barre des tâches) : intacte.
        assert!(!zone_touchee(zone, &[], &[r(900, 1040, 100, 40)]));
        // Bord à bord, sans pixel commun : intacte (bord droit exclu).
        assert!(!zone_touchee(
            zone,
            &[],
            &[r(700, 100, 10, 10), r(0, 0, 100, 100)]
        ));
        // Un seul pixel dedans : touchée.
        assert!(zone_touchee(zone, &[], &[r(0, 0, 101, 101)]));
        assert!(zone_touchee(zone, &[], &[r(5, 5, 2, 2), r(699, 549, 1, 1)]));
        // Déplacement : par sa source ou par sa destination.
        let dehors = r(1000, 0, 50, 50);
        assert!(!zone_touchee(zone, &[(dehors, r(1000, 60, 50, 50))], &[]));
        assert!(zone_touchee(zone, &[(r(120, 120, 50, 50), dehors)], &[]));
        assert!(zone_touchee(zone, &[(dehors, r(120, 120, 50, 50))], &[]));
        // Rectangles vides ignorés.
        assert!(!zone_touchee(zone, &[], &[r(200, 200, 0, 10)]));
    }

    #[test]
    fn copie_qui_dit_ce_qui_change() {
        // 3 × 2 pixels, lignes de 16 octets (4 de bourrage) : BGRA vers RGBA, alpha 255.
        let mut source = vec![0u8; 16 * 2];
        for y in 0..2 {
            for x in 0..3 {
                let i = y * 16 + x * 4;
                source[i..i + 4].copy_from_slice(&[x as u8, y as u8, 200, 7]);
            }
            source[y * 16 + 12..y * 16 + 16].fill(99);
        }
        let mut dest = Pixels {
            largeur: 0,
            hauteur: 0,
            rgba: Vec::new(),
        };
        assert_eq!(copier_bgra(&source, 16, 3, 2, &mut dest), Ok(true));
        assert_eq!((dest.largeur, dest.hauteur), (3, 2));
        assert_eq!(dest.rgba.len(), 3 * 2 * 4);
        assert_eq!(&dest.rgba[0..4], &[200, 0, 0, 255]);
        assert_eq!(&dest.rgba[(3 + 2) * 4..(3 + 2) * 4 + 4], &[200, 1, 2, 255]);
        assert!(dest.rgba.chunks_exact(4).all(|p| p[3] == 255));
        // La même image : rien n'a changé, même tampon.
        let adresse = dest.rgba.as_ptr();
        let avant = dest.rgba.clone();
        assert_eq!(copier_bgra(&source, 16, 3, 2, &mut dest), Ok(false));
        assert_eq!(dest.rgba, avant);
        assert_eq!(dest.rgba.as_ptr(), adresse);
        // Seul l'alpha de la source change : ce n'est pas un changement.
        source[3] = 0;
        assert_eq!(copier_bgra(&source, 16, 3, 2, &mut dest), Ok(false));
        // Le bourrage change : rien non plus.
        source[12] = 1;
        assert_eq!(copier_bgra(&source, 16, 3, 2, &mut dest), Ok(false));
        // Un seul pixel, le dernier : changé.
        source[16 + 8] = 50;
        assert_eq!(copier_bgra(&source, 16, 3, 2, &mut dest), Ok(true));
        assert_eq!(&dest.rgba[20..24], &[200, 1, 50, 255]);
        // Une autre taille (tout l'écran de la source, sans bourrage) : changé.
        assert_eq!(copier_bgra(&source, 16, 4, 2, &mut dest), Ok(true));
        assert_eq!((dest.largeur, dest.hauteur), (4, 2));
        // Une destination de la bonne taille annoncée mais d'un autre contenu : changée.
        let mut autre = Pixels {
            largeur: 3,
            hauteur: 2,
            rgba: vec![0; 24],
        };
        assert_eq!(copier_bgra(&source, 16, 3, 2, &mut autre), Ok(true));
    }

    #[test]
    fn copie_bornee() {
        let mut dest = Pixels {
            largeur: 1,
            hauteur: 1,
            rgba: vec![1, 2, 3, 4],
        };
        // Source trop courte : la dernière ligne n'a pas sa largeur (le bourrage peut manquer
        // après elle, pas les pixels). Rien n'est écrit.
        let source = vec![0u8; 16 + 11];
        assert!(copier_bgra(&source, 16, 3, 2, &mut dest).is_err());
        assert_eq!(
            (dest.largeur, dest.hauteur, dest.rgba.as_slice()),
            (1, 1, &[1u8, 2, 3, 4][..])
        );
        assert_eq!(copier_bgra(&[0u8; 16 + 12], 16, 3, 2, &mut dest), Ok(true));
        // Pas de ligne plus petit que la largeur : erreur.
        assert!(copier_bgra(&[0u8; 64], 8, 3, 2, &mut dest).is_err());
        // Débordements : erreur, pas de panique.
        assert!(copier_bgra(&[], usize::MAX, u32::MAX, 2, &mut dest).is_err());
        assert!(copier_bgra(&[], usize::MAX, 4, u32::MAX, &mut dest).is_err());
        // Vide : rien à lire.
        assert_eq!(copier_bgra(&[], 0, 0, 0, &mut dest), Ok(true));
        assert_eq!(copier_bgra(&[], 0, 0, 0, &mut dest), Ok(false));
    }

    #[test]
    fn reprise_apres_acces_perdu() {
        let t0 = Instant::now();
        let ms = |n: u64| t0 + Duration::from_millis(n);
        let mut reprise = Reprise::default();
        assert!(!reprise.perdue());
        assert!(reprise.essai_permis(t0));
        assert_eq!(reprise.image_rendue(ms(1)), None);

        // Perte à 100 ms : un essai tout de suite.
        reprise.perdre(ms(100));
        assert!(reprise.perdue());
        assert!(reprise.essai_permis(ms(100)));
        // Raté à 133 ms : le suivant à 633 ms, pas avant.
        reprise.essai_rate(ms(133));
        assert!(!reprise.essai_permis(ms(166)));
        assert!(!reprise.essai_permis(ms(632)));
        assert!(reprise.essai_permis(ms(633)));
        reprise.essai_rate(ms(640));
        // Une seconde perte (la duplication refaite se perd encore) garde la première date
        // et permet un essai tout de suite.
        reprise.perdre(ms(700));
        assert!(reprise.essai_permis(ms(700)));
        assert!(reprise.perdue());
        // L'image revient à 750 ms : 650 ms sans image.
        assert_eq!(
            reprise.image_rendue(ms(750)),
            Some(Duration::from_millis(650))
        );
        assert!(!reprise.perdue());
        assert!(reprise.essai_permis(ms(751)));
        assert_eq!(reprise.image_rendue(ms(800)), None);
        // Une horloge qui recule ne panique pas.
        reprise.perdre(ms(900));
        assert_eq!(reprise.image_rendue(ms(800)), Some(Duration::ZERO));
    }

    /// Le suivi d'une duplication, pas à pas : ouverture, écran immobile, pointeur seul,
    /// accès perdu, essais toutes les 500 ms, reprise, image écartée par la garde.
    #[test]
    fn suivi_de_la_duplication() {
        let t0 = Instant::now();
        let ms = |n: u64| t0 + Duration::from_millis(n);
        let mut suivi = Suivi::nouveau();
        assert_eq!(suivi.etape(true, ms(0)), Etape::Acquerir);
        // Rien encore acquis : pas d'image (la destination est vide).
        assert_eq!(suivi.sans_copie(), Image::Perdue);
        // La première image est copiée même si ses rectangles ne touchent pas la zone.
        assert!(suivi.a_copier(false, Some(false)));
        assert_eq!(suivi.copiee(true, ms(33)), (Image::Nouvelle, None));
        // Écran immobile, pointeur seul, rectangles hors de la zone : rien à copier.
        assert!(!suivi.a_copier(false, None));
        assert!(!suivi.a_copier(true, Some(false)));
        assert_eq!(suivi.sans_copie(), Image::Inchangee);
        // Rectangles dans la zone, ou non lus : copier.
        assert!(suivi.a_copier(true, Some(true)));
        assert!(suivi.a_copier(true, None));
        // Copiée mais identique : inchangée.
        assert_eq!(suivi.copiee(false, ms(66)), (Image::Inchangee, None));

        // Accès perdu à 100 ms : pas d'image, un essai tout de suite.
        suivi.perdue(ms(100));
        assert_eq!(suivi.sans_copie(), Image::Perdue);
        assert_eq!(suivi.etape(false, ms(133)), Etape::Refaire);
        suivi.refaite(false, ms(133));
        assert_eq!(suivi.etape(false, ms(166)), Etape::Attendre);
        assert_eq!(suivi.etape(false, ms(632)), Etape::Attendre);
        assert_eq!(suivi.etape(false, ms(633)), Etape::Refaire);
        suivi.refaite(true, ms(633));
        assert_eq!(suivi.etape(true, ms(633)), Etape::Acquerir);
        // Refaite mais pas encore d'image : toujours perdue.
        assert_eq!(suivi.sans_copie(), Image::Perdue);
        // La première image de la duplication refaite est copiée entière ; 566 ms sans image.
        assert!(suivi.a_copier(false, Some(false)));
        assert_eq!(
            suivi.copiee(false, ms(666)),
            (Image::Inchangee, Some(Duration::from_millis(566)))
        );
        assert_eq!(suivi.sans_copie(), Image::Inchangee);
        assert!(!suivi.a_copier(true, Some(false)));

        // Une image écartée par la garde : la suivante est copiée entière, mais la
        // destination garde la précédente, qui reste juste.
        suivi.ecartee();
        assert!(suivi.a_copier(false, Some(false)));
        assert_eq!(suivi.sans_copie(), Image::Inchangee);
        assert_eq!(suivi.copiee(true, ms(700)), (Image::Nouvelle, None));
        assert!(!suivi.a_copier(false, None));
    }

    #[test]
    fn une_ligne_par_serie_d_echecs() {
        let mut serie = Serie::default();
        assert!(serie.echec());
        assert!(!serie.echec());
        assert!(!serie.echec());
        serie.succes();
        serie.succes();
        assert!(serie.echec());
        assert!(!serie.echec());
        assert_eq!(code_hex(0x887A0026_u32 as i32), "0x887A0026");
        assert_eq!(code_hex(0), "0x00000000");
    }

    /// Hors Windows : rien n'est disponible, et `ouvrir` le dit sans paniquer.
    #[cfg(not(windows))]
    #[test]
    fn absent_hors_de_windows() {
        assert!(!disponible());
        let e = ouvrir(r(0, 0, 100, 100)).unwrap_err();
        assert!(e.contains("non disponibles"), "{e}");
    }

    /// Sous Windows : disponible (R2).
    #[cfg(windows)]
    #[test]
    fn disponible_sous_windows() {
        assert!(disponible());
    }
}
