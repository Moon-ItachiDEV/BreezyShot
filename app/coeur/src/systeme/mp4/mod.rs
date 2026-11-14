//! Le fichier MP4 d'une vidéo (contrat 0.7, §13.1, F15, F16, F31, F32) : Media Foundation,
//! et rien d'autre. Une piste vidéo H.264 (profil par défaut du codeur, sans réordonnancement
//! des images ; 4:2:0, 8 bits, BT.709) écrite par un `IMFSinkWriter`, nourrie d'images NV12
//! converties ici depuis les pixels RGBA du cœur (BT.709, plage réduite) ; une piste AAC
//! (48 kHz, mono, 96 kbit/s) si le PC a un micro. MP4 ordinaire, sans index au début (R15, R17).
//!
//! **Garde (F31)** : les DLL de Media Foundation sont chargées à la demande (`build.rs`) ;
//! avant tout appel, `bibliotheques` les charge par `LoadLibraryExW` (système 32 seulement)
//! et garde le résultat. Faux : aucun autre appel de ce module ne part (`creer` et `lire`
//! rendent une erreur, `codeur_h264_present` rend faux), et l'app vit sans vidéo.
//!
//! Ce qui est pur est ici, testé partout : la conversion RGBA vers NV12, le débit visé, la
//! vérification des paramètres, les horodatages des deux pistes. `windows.rs` fait le
//! reste (l'écrivain, la recherche du codeur, le lecteur de l'exemple `lire_film`) ;
//! `autre.rs` dit « non disponible » ailleurs.
//!
//! Horodatages : une image donnée n'est écrite qu'à l'arrivée de la suivante, avec pour
//! durée l'écart réel entre les deux (la dernière, à la fermeture, avec la durée d'un pas).
//! Le fichier garde donc les temps réels même quand des pas sont sautés (F13), quelle que
//! soit la façon dont le MP4 range les durées. La première image couvre le film depuis 0.

use std::ffi::OsStr;
use std::marker::PhantomData;
use std::path::Path;
use std::time::Duration;

use super::gel::Pixels;

#[cfg(windows)]
#[path = "windows.rs"]
mod plateforme;

#[cfg(not(windows))]
#[path = "autre.rs"]
mod plateforme;

/// Variable d'essai : la garde répond « absent » sans rien charger (essai 15, cas J).
pub const VARIABLE_SANS_CODEUR: &str = "BREEZYSHOT_ESSAI_SANS_CODEUR";

/// Plus grande image donnée au codeur, pixels : la limite du niveau 5.2 de H.264 (contrat
/// 0.7, D26 : 3840 × 2160 accepté et 5120 × 2880 refusé sur GitHub, S2 point 2).
pub const LARGEUR_MAX: u32 = 4096;
pub const HAUTEUR_MAX: u32 = 2304;

/// Plus petit côté donné au codeur, pixels (D26 : 64 × 64 accepté, 16 × 16 refusé par
/// `SetInputMediaType`, `0xC00D36B4`).
pub const COTE_MIN: u32 = 64;

/// Débit moyen visé (F15) : `largeur × hauteur × 4` bits par seconde, dans ces bornes.
pub const DEBIT_MIN_BIT_S: u32 = 1_000_000;
pub const DEBIT_MAX_BIT_S: u32 = 16_000_000;

/// La piste son (F17) : PCM 48 kHz, mono, 16 bits en entrée ; AAC à 96 kbit/s.
pub const TAUX_SON: u32 = 48_000;
pub const DEBIT_SON_BIT_S: u32 = 96_000;

/// Images par seconde acceptées par `creer`.
const CADENCE_MAX: u32 = 120;

/// Unités de Media Foundation par seconde (100 ns).
const UNITES_PAR_S: i64 = 10_000_000;

/// La garde F31 : les trois DLL de Media Foundation (`mfplat`, `mfreadwrite`, `mf`) se
/// chargent. Résultat gardé. Aucun autre appel de ce module ne part si elle rend faux.
/// Faux d'emblée, sans rien charger, si `VARIABLE_SANS_CODEUR` est posée. Journal, une
/// fois, quand elle est fausse : `film: media foundation absente`.
pub fn bibliotheques() -> bool {
    plateforme::bibliotheques()
}

/// Un codeur H.264 existe (F32 : `MFStartup`, `MFTEnumEx` sans activer aucun codeur,
/// `MFShutdown`). Bloquant : fil `film-codeurs` seulement. Faux si la garde l'est.
pub fn codeur_h264_present() -> bool {
    plateforme::codeur_h264_present()
}

/// Vrai si le codeur accepte une image de `largeur` × `hauteur` pixels (fonction pure,
/// F4, D26) : côtés pairs, de `COTE_MIN` au moins, dans les bornes `LARGEUR_MAX` ×
/// `HAUTEUR_MAX`.
pub fn taille_acceptee(largeur: u32, hauteur: u32) -> bool {
    largeur >= COTE_MIN
        && hauteur >= COTE_MIN
        && largeur.is_multiple_of(2)
        && hauteur.is_multiple_of(2)
        && largeur <= LARGEUR_MAX
        && hauteur <= HAUTEUR_MAX
}

/// Le débit moyen visé d'une vidéo (F15), bits par seconde : `largeur × hauteur × 4`,
/// borné de 1 à 16 Mbit/s (600 × 450 : 1,08 Mbit/s ; 1920 × 1080 : 8,3 ; 2560 × 1440 :
/// 14,7). `application::film::debit` peut s'en remettre à elle.
pub fn debit_vise(largeur: u32, hauteur: u32) -> u32 {
    let brut = (u64::from(largeur) * u64::from(hauteur)).saturating_mul(4);
    brut.clamp(u64::from(DEBIT_MIN_BIT_S), u64::from(DEBIT_MAX_BIT_S)) as u32
}

/// Le codeur que l'écrivain a pris.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codeur {
    Materiel,
    Logiciel,
}

impl Codeur {
    /// Nom au journal : `materiel`, `logiciel`.
    pub fn nom(self) -> &'static str {
        match self {
            Codeur::Materiel => "materiel",
            Codeur::Logiciel => "logiciel",
        }
    }
}

/// Ce que l'écrivain doit produire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parametres {
    /// Côtés de l'image, pairs (`taille_acceptee`).
    pub largeur: u32,
    pub hauteur: u32,
    /// Images par seconde (30, `application::film::CADENCE_VIDEO`).
    pub cadence: u32,
    /// Débit moyen visé (F15, `debit_vise`).
    pub debit_bit_s: u32,
    /// Une piste son (le PC a un micro, F17).
    pub son: bool,
    /// Essayer le codeur matériel d'abord (F16).
    pub materiel: bool,
}

/// L'écrivain d'un fichier MP4 en cours. Créé, servi et fini dans **un seul fil**
/// (`film-video`) : il n'est pas `Send`. Détruit sans `finir` ni `abandonner` (erreur,
/// panique), il se ferme comme par `abandonner`.
#[derive(Debug)]
pub struct Ecrivain {
    interne: plateforme::Ecrivain,
    _un_seul_fil: PhantomData<*const ()>,
}

/// Crée le fichier `chemin` et son écrivain (F15, F16). Erreur : paramètres refusés
/// (`verifier`, dont les côtés impairs), garde fausse, codeur ou fichier refusés. Avec
/// `materiel`, un refus du codeur matériel (type, démarrage) fait recommencer le fichier
/// une fois avec le codeur logiciel, et l'écrit au journal :
/// `film codeur logiciel (materiel refuse: <code>)`.
pub fn creer(chemin: &Path, p: &Parametres) -> Result<Ecrivain, String> {
    verifier(p)?;
    plateforme::creer(chemin, p).map(|interne| Ecrivain {
        interne,
        _un_seul_fil: PhantomData,
    })
}

impl Ecrivain {
    /// Le codeur pris (il peut passer à `Logiciel` pendant les 30 premières images, F16).
    pub fn codeur(&self) -> Codeur {
        plateforme::codeur(&self.interne)
    }

    /// Convertit les pixels RGBA en NV12 (BT.709, plage réduite) directement dans le
    /// tampon donné au codeur, puis les lui donne à `instant` du film (F15). Les côtés
    /// doivent être ceux de `Parametres`, et `instant` doit avancer. Rend le temps passé.
    pub fn image(&mut self, pixels: &Pixels, instant: Duration) -> Result<Duration, String> {
        plateforme::image(&mut self.interne, pixels, instant)
    }

    /// La dernière image donnée, de nouveau, à `instant` (F13) : aucune conversion, le
    /// même tampon. Erreur si aucune image n'a encore été donnée.
    pub fn meme_image(&mut self, instant: Duration) -> Result<Duration, String> {
        plateforme::meme_image(&mut self.interne, instant)
    }

    /// Du son du micro (PCM 48 kHz, mono, 16 bits) à `instant` de la piste. Sans piste
    /// son (`Parametres::son` faux), rien n'est écrit.
    pub fn son(&mut self, pcm: &[i16], instant: Duration) -> Result<(), String> {
        plateforme::son(&mut self.interne, pcm, instant)
    }

    /// Ferme le fichier (la dernière image, puis `Finalize`) ; rend la durée prise. En
    /// erreur, le fichier est relâché quand même (il peut être effacé).
    pub fn finir(self) -> Result<Duration, String> {
        plateforme::finir(self.interne)
    }

    /// Ferme sans finir (le fichier est à effacer) : le récepteur du fichier est arrêté
    /// tout de suite, et le fichier relâché au retour.
    pub fn abandonner(self) {
        plateforme::abandonner(self.interne)
    }
}

/// Ce que `lire` a relu d'un fichier MP4 (lecteur de l'essai 15, exemple `lire_film`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lecture {
    /// Durée annoncée par le fichier (`MF_PD_DURATION`).
    pub duree: Duration,
    /// Côtés de l'image montrée (la zone d'affichage, pas le tampon décodé).
    pub largeur: u32,
    pub hauteur: u32,
    /// La géométrie de la dernière image décodée : le tampon (608 × 464 pour une image de
    /// 600 × 450, mesuré par la sonde, D28), la largeur de ligne lue en octets (négative de
    /// bas en haut ; 2 432 dans ce cas) et le coin de la zone d'affichage.
    pub tampon_largeur: u32,
    pub tampon_hauteur: u32,
    pub pas_ligne: i64,
    pub zone_x: u32,
    pub zone_y: u32,
    /// Images relues.
    pub images: u64,
    /// Instant de la dernière image relue.
    pub dernier_instant: Option<Duration>,
    /// La piste vidéo est en H.264.
    pub h264: bool,
    /// Le fichier a une piste son.
    pub son: bool,
    /// Changements de type annoncés par `ReadSample` pendant la lecture.
    pub changements_de_type: u32,
}

/// Relit le fichier MP4 `chemin` par un `IMFSourceReader`, derrière la garde F31 : chaque
/// image, en RGBA de haut en bas (la zone d'affichage, avec la largeur de ligne réelle du
/// tampon, relue à chaque changement de type), est passée à `voir(numéro, instant,
/// image)`. Sert l'exemple `lire_film` et les tests ; jamais l'app.
pub fn lire(
    chemin: &Path,
    mut voir: impl FnMut(u64, Duration, &Pixels),
) -> Result<Lecture, String> {
    plateforme::lire(chemin, &mut voir)
}

// ---------------------------------------------------------------------------------------
// Pur : vérifications, garde, horodatages, conversion NV12.

/// Les paramètres que `creer` accepte : côtés (`taille_acceptee`, donc pairs), cadence de
/// 1 à 120, débit non nul.
pub fn verifier(p: &Parametres) -> Result<(), String> {
    if !taille_acceptee(p.largeur, p.hauteur) {
        return Err(format!(
            "taille refusee {}x{} (cotes pairs, de {COTE_MIN} a {LARGEUR_MAX}x{HAUTEUR_MAX})",
            p.largeur, p.hauteur
        ));
    }
    if p.cadence == 0 || p.cadence > CADENCE_MAX {
        return Err(format!("cadence refusee {}", p.cadence));
    }
    if p.debit_bit_s == 0 {
        return Err("debit nul".into());
    }
    Ok(())
}

/// La variable d'essai `VARIABLE_SANS_CODEUR` demande la garde fausse : posée et non vide.
#[cfg_attr(not(windows), allow(dead_code))]
fn sans_codeur_demande(valeur: Option<&OsStr>) -> bool {
    valeur.is_some_and(|v| !v.is_empty())
}

/// Une durée en unités de Media Foundation (100 ns), sans débordement.
#[cfg_attr(not(windows), allow(dead_code))]
fn unites(d: Duration) -> i64 {
    i64::try_from(d.as_nanos() / 100).unwrap_or(i64::MAX)
}

/// Les horodatages des deux pistes, en unités de 100 ns. Une image donnée attend la
/// suivante pour être écrite (sa durée est l'écart réel) ; la première couvre depuis 0.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
struct Horodatage {
    /// Durée d'un pas de la cadence (la dernière image).
    pas: i64,
    /// Début de l'image donnée et pas encore écrite (0 pour la première).
    attente: Option<i64>,
    /// Instant donné de la dernière image (il doit avancer).
    dernier_instant: Option<i64>,
    /// Début du dernier son écrit, et fin de la piste son.
    son_debut: Option<i64>,
    son_fin: i64,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl Horodatage {
    fn nouveau(cadence: u32) -> Horodatage {
        let c = i64::from(cadence.max(1));
        Horodatage {
            pas: (UNITES_PAR_S + c / 2) / c,
            attente: None,
            dernier_instant: None,
            son_debut: None,
            son_fin: 0,
        }
    }

    /// Erreur si `instant` n'avance pas sur la dernière image ; rien n'est changé.
    fn peut(&self, instant: Duration) -> Result<i64, String> {
        let t = unites(instant);
        match self.dernier_instant {
            Some(avant) if t <= avant => Err(format!(
                "image hors d'ordre ({} ms apres {} ms)",
                t / 10_000,
                avant / 10_000
            )),
            _ => Ok(t),
        }
    }

    /// Une image à `instant` : rend l'image en attente à écrire (début, durée) s'il y en
    /// a une, et met la nouvelle en attente.
    fn image(&mut self, instant: Duration) -> Result<Option<(i64, i64)>, String> {
        let t = self.peut(instant)?;
        let a_ecrire = self.attente.map(|debut| (debut, (t - debut).max(1)));
        self.attente = Some(if self.dernier_instant.is_none() { 0 } else { t });
        self.dernier_instant = Some(t);
        Ok(a_ecrire)
    }

    /// La fermeture : l'image en attente, avec la durée d'un pas après son instant.
    fn fin(&mut self) -> Option<(i64, i64)> {
        let debut = self.attente.take()?;
        let fin = self
            .dernier_instant
            .unwrap_or(debut)
            .saturating_add(self.pas);
        Some((debut, (fin - debut).max(1)))
    }

    /// Le fichier recommencé (repli sur le codeur logiciel, F16) : l'image en attente
    /// couvre de nouveau depuis 0 ; la piste son est à refaire en silence jusqu'à sa fin.
    fn repartir(&mut self) {
        if self.attente.is_some() {
            self.attente = Some(0);
        }
    }

    /// `echantillons` de son à `instant` (48 kHz) : (début, durée). Erreur si le son
    /// recule sur le précédent.
    fn son(&mut self, echantillons: usize, instant: Duration) -> Result<(i64, i64), String> {
        let t = unites(instant);
        if let Some(avant) = self.son_debut {
            if t <= avant {
                return Err(format!(
                    "son hors d'ordre ({} ms apres {} ms)",
                    t / 10_000,
                    avant / 10_000
                ));
            }
        }
        let duree = duree_son(echantillons);
        let fin = unites(instant.saturating_add(duree));
        self.son_debut = Some(t);
        self.son_fin = self.son_fin.max(fin);
        Ok((t, (fin - t).max(1)))
    }
}

/// La durée de `echantillons` de son à 48 kHz.
#[cfg_attr(not(windows), allow(dead_code))]
fn duree_son(echantillons: usize) -> Duration {
    let n = echantillons as u64;
    let taux = u64::from(TAUX_SON);
    Duration::from_secs(n / taux) + Duration::from_nanos((n % taux) * 1_000_000_000 / taux)
}

/// Octets d'une image NV12 de `largeur` × `hauteur` (côtés pairs) : le plan Y, puis le
/// plan UV entrelacé, à demi-hauteur, de même largeur de ligne.
#[cfg_attr(not(windows), allow(dead_code))]
fn taille_nv12(largeur: u32, hauteur: u32) -> Option<usize> {
    let plan = (largeur as usize).checked_mul(hauteur as usize)?;
    plan.checked_add(plan / 2)
}

// Coefficients × 65 536 : BT.709 (Kr 0,2126, Kb 0,0722), plage réduite (219/255 pour Y,
// 224/255 pour U et V). Ceux de la sonde (couleurs relues à ± 2).
const YR: i32 = 11966;
const YG: i32 = 40254;
const YB: i32 = 4064;
const UR: i32 = -6596;
const UG: i32 = -22189;
const UB: i32 = 28784;
const VR: i32 = 28784;
const VG: i32 = -26145;
const VB: i32 = -2639;

#[inline(always)]
fn luma(p: &[u8]) -> u8 {
    let (r, g, b) = (i32::from(p[0]), i32::from(p[1]), i32::from(p[2]));
    (((YR * r + YG * g + YB * b + 32768) >> 16) + 16).clamp(0, 255) as u8
}

/// U et V d'un carré de 2 × 2 pixels, depuis la somme de ses quatre couleurs.
#[inline(always)]
fn chroma(r: i32, g: i32, b: i32) -> (u8, u8) {
    // Somme de quatre : décalage de 18 au lieu de 16, arrondi au plus proche.
    let u = ((UR * r + UG * g + UB * b + (1 << 17)) >> 18) + 128;
    let v = ((VR * r + VG * g + VB * b + (1 << 17)) >> 18) + 128;
    (u.clamp(0, 255) as u8, v.clamp(0, 255) as u8)
}

/// RGBA (de haut en bas, `largeur × hauteur × 4` octets) vers NV12 dans `sortie` (au moins
/// `taille_nv12` octets ; le reste n'est pas touché) : BT.709, plage réduite (Y de 16 à
/// 235, U et V de 16 à 240), la chrominance prise sur la moyenne de chaque carré de 2 × 2.
/// L'alpha est ignoré. Côtés pairs et non nuls, sinon erreur (jamais de panique).
#[cfg_attr(not(windows), allow(dead_code))]
fn vers_nv12(rgba: &[u8], largeur: u32, hauteur: u32, sortie: &mut [u8]) -> Result<(), String> {
    if largeur == 0 || hauteur == 0 || !largeur.is_multiple_of(2) || !hauteur.is_multiple_of(2) {
        return Err(format!("nv12: cotes refuses {largeur}x{hauteur}"));
    }
    let l = largeur as usize;
    let h = hauteur as usize;
    let attendu = l
        .checked_mul(h)
        .and_then(|n| n.checked_mul(4))
        .ok_or("nv12: image trop grande")?;
    if rgba.len() != attendu {
        return Err(format!(
            "nv12: {} octets pour {largeur}x{hauteur}",
            rgba.len()
        ));
    }
    let total = taille_nv12(largeur, hauteur).ok_or("nv12: image trop grande")?;
    if sortie.len() < total {
        return Err(format!("nv12: tampon de {} octets", sortie.len()));
    }
    let (plan_y, plan_uv) = sortie[..total].split_at_mut(l * h);
    // Deux lignes à la fois : leurs Y, et la ligne UV de leurs carrés de 2 × 2.
    for ((deux, ys), uvs) in rgba
        .chunks_exact(8 * l)
        .zip(plan_y.chunks_exact_mut(2 * l))
        .zip(plan_uv.chunks_exact_mut(l))
    {
        let (haut, bas) = deux.split_at(4 * l);
        let (y_haut, y_bas) = ys.split_at_mut(l);
        for ((((ph, pb), yh), yb), uv) in haut
            .chunks_exact(8)
            .zip(bas.chunks_exact(8))
            .zip(y_haut.chunks_exact_mut(2))
            .zip(y_bas.chunks_exact_mut(2))
            .zip(uvs.chunks_exact_mut(2))
        {
            yh[0] = luma(&ph[0..3]);
            yh[1] = luma(&ph[4..7]);
            yb[0] = luma(&pb[0..3]);
            yb[1] = luma(&pb[4..7]);
            let r = i32::from(ph[0]) + i32::from(ph[4]) + i32::from(pb[0]) + i32::from(pb[4]);
            let g = i32::from(ph[1]) + i32::from(ph[5]) + i32::from(pb[1]) + i32::from(pb[5]);
            let b = i32::from(ph[2]) + i32::from(ph[6]) + i32::from(pb[2]) + i32::from(pb[6]);
            let (u, v) = chroma(r, g, b);
            uv[0] = u;
            uv[1] = v;
        }
    }
    Ok(())
}

/// La zone d'affichage d'une image décodée (`MF_MT_MINIMUM_DISPLAY_APERTURE`, sinon le
/// tampon entier), en pixels.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Zone {
    x: u32,
    y: u32,
    largeur: u32,
    hauteur: u32,
}

/// Copie `zone` d'une image BGRX décodée dans `sortie`, en RGBA opaque de haut en bas.
/// `donnees` est le tampon verrouillé ; la ligne 0 de l'image commence à `ligne0` octets
/// de son début, et chaque ligne à `pas` octets de la précédente (négatif : rangée de bas
/// en haut). Faux, sans lire hors du tampon, si une ligne en sort.
#[cfg_attr(not(windows), allow(dead_code))]
fn copier_zone(donnees: &[u8], ligne0: isize, pas: isize, zone: Zone, sortie: &mut Pixels) -> bool {
    let ligne = zone.largeur as usize * 4;
    // La ligne `y` de la zone dans le tampon, si elle y tient entière.
    let source = |y: u32| -> Option<&[u8]> {
        let debut = isize::try_from(u64::from(zone.y) + u64::from(y))
            .ok()?
            .checked_mul(pas)?
            .checked_add(ligne0)?
            .checked_add(isize::try_from(u64::from(zone.x) * 4).ok()?)?;
        let debut = usize::try_from(debut).ok()?;
        donnees.get(debut..debut.checked_add(ligne)?)
    };
    // Les débuts de ligne suivent `y` en ligne droite : la première et la dernière dans le
    // tampon, toutes y sont (vérifié avant d'allouer quoi que ce soit).
    if zone.hauteur > 0 && (source(0).is_none() || source(zone.hauteur - 1).is_none()) {
        return false;
    }
    let Some(total) = ligne.checked_mul(zone.hauteur as usize) else {
        return false;
    };
    sortie.largeur = zone.largeur;
    sortie.hauteur = zone.hauteur;
    sortie.rgba.resize(total, 0);
    for (y, dest) in (0..zone.hauteur).zip(sortie.rgba.chunks_exact_mut(ligne.max(1))) {
        let Some(source) = source(y) else {
            return false;
        };
        for (d, s) in dest.chunks_exact_mut(4).zip(source.chunks_exact(4)) {
            d[0] = s[2];
            d[1] = s[1];
            d[2] = s[0];
            d[3] = 255;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::time::Instant;

    // --- Copie d'une image relue ---

    /// Un tampon BGRX de `l` × `h` dont le pixel (x, y) vaut (b = x, g = y, r = 7, x = 0),
    /// rangé de haut en bas (`pas` positif) ou de bas en haut.
    fn tampon_bgrx(l: usize, h: usize, pas: usize, bas_en_haut: bool) -> Vec<u8> {
        let mut v = vec![0xEEu8; pas * h];
        for y in 0..h {
            let rang = if bas_en_haut { h - 1 - y } else { y };
            for x in 0..l {
                let i = rang * pas + x * 4;
                v[i..i + 4].copy_from_slice(&[x as u8, y as u8, 7, 0]);
            }
        }
        v
    }

    fn vide() -> Pixels {
        Pixels {
            largeur: 0,
            hauteur: 0,
            rgba: Vec::new(),
        }
    }

    /// 600 × 450 décodé en 608 × 464 (largeur de ligne réelle plus grande, zone d'affichage
    /// en haut à gauche) : la zone seule revient, chaque pixel à sa place, en RGBA opaque.
    #[test]
    fn copie_de_la_zone_d_affichage() {
        let (l, h, pas) = (40usize, 30usize, 40 * 4 + 32);
        let t = tampon_bgrx(l, h, pas, false);
        let zone = Zone {
            x: 3,
            y: 2,
            largeur: 34,
            hauteur: 26,
        };
        let mut s = vide();
        assert!(copier_zone(&t, 0, pas as isize, zone, &mut s));
        assert_eq!((s.largeur, s.hauteur), (34, 26));
        assert_eq!(s.rgba.len(), 34 * 26 * 4);
        for y in 0..26usize {
            for x in 0..34usize {
                let i = (y * 34 + x) * 4;
                assert_eq!(&s.rgba[i..i + 4], &[7, (y + 2) as u8, (x + 3) as u8, 255]);
            }
        }
    }

    /// Rangée de bas en haut (pas négatif, ligne 0 à la fin du tampon) : l'image revient à
    /// l'endroit.
    #[test]
    fn copie_de_bas_en_haut() {
        let (l, h) = (8usize, 6usize);
        let pas = l * 4;
        let t = tampon_bgrx(l, h, pas, true);
        let zone = Zone {
            x: 0,
            y: 0,
            largeur: l as u32,
            hauteur: h as u32,
        };
        let mut s = vide();
        assert!(copier_zone(
            &t,
            ((h - 1) * pas) as isize,
            -(pas as isize),
            zone,
            &mut s
        ));
        for y in 0..h {
            let i = y * l * 4;
            assert_eq!(&s.rgba[i..i + 4], &[7, y as u8, 0, 255]);
        }
    }

    /// Une géométrie plus grande que le tampon : faux, sans panique ni lecture dehors.
    #[test]
    fn copie_hors_du_tampon_refusee() {
        let t = tampon_bgrx(8, 6, 32, false);
        let mut s = vide();
        let zone = |x, y, largeur, hauteur| Zone {
            x,
            y,
            largeur,
            hauteur,
        };
        assert!(!copier_zone(&t, 0, 32, zone(0, 0, 8, 7), &mut s));
        assert!(!copier_zone(&t, 0, 32, zone(1, 0, 8, 6), &mut s));
        assert!(!copier_zone(&t, 4, 32, zone(0, 0, 8, 6), &mut s));
        assert!(!copier_zone(&t, 0, -32, zone(0, 0, 8, 6), &mut s));
        assert!(!copier_zone(&t, isize::MAX, 32, zone(0, 0, 8, 6), &mut s));
        assert!(!copier_zone(&t, 0, isize::MAX, zone(0, 0, 8, 6), &mut s));
        assert!(!copier_zone(
            &t,
            0,
            32,
            zone(u32::MAX, u32::MAX, 8, 6),
            &mut s
        ));
        assert!(!copier_zone(
            &t,
            0,
            32,
            zone(0, 0, u32::MAX, u32::MAX),
            &mut s
        ));
        assert!(copier_zone(&t, 0, 32, zone(0, 0, 8, 6), &mut s));
        assert!(copier_zone(&t, 0, 32, zone(0, 0, 0, 0), &mut s));
    }

    fn params(largeur: u32, hauteur: u32) -> Parametres {
        Parametres {
            largeur,
            hauteur,
            cadence: 30,
            debit_bit_s: debit_vise(largeur, hauteur),
            son: false,
            materiel: true,
        }
    }

    #[test]
    fn tailles_acceptees() {
        for (l, h) in [
            (600, 450),
            (1024, 768),
            (2560, 1440),
            (120, 120),
            (4096, 2304),
            (3840, 2160),
            (602, 452),
            (64, 64),
            (64, 2304),
        ] {
            assert!(taille_acceptee(l, h), "{l}x{h}");
        }
        for (l, h) in [
            (0, 450),
            (601, 450),
            (600, 451),
            (601, 451),
            (4098, 2304),
            (4096, 2306),
            (5120, 2880),
            (62, 64),
            (64, 62),
            (16, 16),
            (0, 0),
        ] {
            assert!(!taille_acceptee(l, h), "{l}x{h}");
        }
    }

    /// Hors Windows, et sous Windows sans Media Foundation : rien n'existe, sans panique.
    #[test]
    fn absent_sans_panique() {
        if !bibliotheques() {
            assert!(!codeur_h264_present());
            assert!(creer(Path::new("film-essai.mp4"), &params(600, 450)).is_err());
            assert!(lire(Path::new("film-essai.mp4"), |_, _, _| {}).is_err());
        }
        assert_eq!(Codeur::Logiciel.nom(), "logiciel");
        assert_eq!(Codeur::Materiel.nom(), "materiel");
    }

    /// F15 : `largeur × hauteur × 4`, borné de 1 à 16 Mbit/s.
    #[test]
    fn debit_vise_selon_la_taille() {
        assert_eq!(debit_vise(600, 450), 1_080_000);
        assert_eq!(debit_vise(1920, 1080), 8_294_400);
        assert_eq!(debit_vise(2560, 1440), 14_745_600);
        assert_eq!(debit_vise(1024, 768), 3_145_728);
        assert_eq!(debit_vise(120, 120), DEBIT_MIN_BIT_S);
        assert_eq!(debit_vise(0, 0), DEBIT_MIN_BIT_S);
        assert_eq!(debit_vise(4096, 2304), DEBIT_MAX_BIT_S);
        assert_eq!(debit_vise(u32::MAX, u32::MAX), DEBIT_MAX_BIT_S);
    }

    /// Côtés impairs, hors bornes, cadence ou débit nuls : refusés avant tout appel au
    /// système (donc sous Linux aussi), avec la taille dans le message.
    #[test]
    fn creer_refuse_les_parametres_faux() {
        for (l, h) in [
            (601, 451),
            (601, 450),
            (600, 451),
            (0, 450),
            (4098, 2304),
            (16, 16),
        ] {
            let e = creer(Path::new("film-essai.mp4"), &params(l, h)).unwrap_err();
            assert!(e.contains(&format!("{l}x{h}")), "{e}");
            assert!(verifier(&params(l, h)).is_err());
        }
        assert!(verifier(&params(602, 452)).is_ok());
        assert!(verifier(&Parametres {
            cadence: 0,
            ..params(600, 450)
        })
        .is_err());
        assert!(verifier(&Parametres {
            cadence: 121,
            ..params(600, 450)
        })
        .is_err());
        assert!(verifier(&Parametres {
            debit_bit_s: 0,
            ..params(600, 450)
        })
        .is_err());
    }

    #[test]
    fn variable_sans_codeur() {
        assert!(!sans_codeur_demande(None));
        assert!(!sans_codeur_demande(Some(OsStr::new(""))));
        assert!(sans_codeur_demande(Some(OsStr::new("1"))));
        let v = OsString::from("oui");
        assert!(sans_codeur_demande(Some(&v)));
    }

    #[test]
    fn unites_de_100_ns() {
        assert_eq!(unites(Duration::ZERO), 0);
        assert_eq!(unites(Duration::from_millis(1)), 10_000);
        assert_eq!(unites(Duration::from_secs(3)), 30_000_000);
        assert_eq!(unites(Duration::from_nanos(199)), 1);
        assert_eq!(unites(Duration::MAX), i64::MAX);
    }

    fn instant(n: u64) -> Duration {
        Duration::from_nanos(n * 1_000_000_000 / 30)
    }

    /// Chaque image est écrite à l'arrivée de la suivante, pour l'écart réel ; la
    /// dernière à la fin, pour un pas. La première couvre depuis 0.
    #[test]
    fn horodatage_des_images() {
        let mut h = Horodatage::nouveau(30);
        assert_eq!(h.pas, 333_333);
        assert_eq!(h.fin(), None);
        assert_eq!(h.image(instant(0)).unwrap(), None);
        assert_eq!(h.image(instant(1)).unwrap(), Some((0, 333_333)));
        assert_eq!(h.image(instant(2)).unwrap(), Some((333_333, 333_333)));
        // Deux pas sautés : l'image 2 dure trois pas.
        assert_eq!(h.image(instant(5)).unwrap(), Some((666_666, 1_000_000)));
        assert_eq!(h.fin(), Some((1_666_666, 333_333)));
        assert_eq!(h.fin(), None);
    }

    /// Le temps total écrit est l'instant de la dernière image plus un pas, même si la
    /// première image arrive après 0 et que des pas sont sautés.
    #[test]
    fn horodatage_couvre_tout_le_film() {
        let mut h = Horodatage::nouveau(30);
        let mut couvert = 0;
        let mut fin = 0;
        for n in [3u64, 4, 5, 9, 10, 30, 89] {
            if let Some((debut, duree)) = h.image(instant(n)).unwrap() {
                assert_eq!(debut, fin, "pistes continues");
                couvert += duree;
                fin = debut + duree;
            }
        }
        let (debut, duree) = h.fin().unwrap();
        assert_eq!(debut, fin);
        couvert += duree;
        assert_eq!(couvert, unites(instant(89)) + 333_333);
        assert!((couvert - 30_000_000).abs() <= 1);
    }

    #[test]
    fn horodatage_refuse_ce_qui_recule() {
        let mut h = Horodatage::nouveau(30);
        h.image(instant(10)).unwrap();
        let avant = h.clone();
        assert!(h.image(instant(10)).is_err());
        assert!(h.image(instant(9)).is_err());
        assert!(h.peut(instant(9)).is_err());
        assert_eq!(h, avant, "rien n'a changé");
        assert!(h.image(instant(11)).is_ok());
    }

    /// Repli (F16) : l'image en attente couvre de nouveau depuis 0.
    #[test]
    fn horodatage_repart_de_zero() {
        let mut h = Horodatage::nouveau(30);
        h.repartir();
        assert_eq!(h.attente, None);
        for n in 0..6 {
            h.image(instant(n)).unwrap();
        }
        h.repartir();
        assert_eq!(h.image(instant(6)).unwrap(), Some((0, unites(instant(6)))));
        assert_eq!(
            h.image(instant(7)).unwrap().map(|x| x.0),
            Some(unites(instant(6)))
        );
    }

    #[test]
    fn horodatage_du_son() {
        let mut h = Horodatage::nouveau(30);
        // 1 600 échantillons : 33,333 ms.
        assert_eq!(h.son(1600, Duration::ZERO).unwrap(), (0, 333_333));
        assert_eq!(h.son_fin, 333_333);
        let (t, d) = h.son(4800, Duration::from_millis(100)).unwrap();
        assert_eq!((t, d), (1_000_000, 1_000_000));
        assert_eq!(h.son_fin, 2_000_000);
        assert!(h.son(10, Duration::from_millis(100)).is_err());
        assert!(h.son(10, Duration::from_millis(50)).is_err());
        assert_eq!(h.son_fin, 2_000_000);
        assert_eq!(duree_son(48_000), Duration::from_secs(1));
        assert_eq!(duree_son(48_001), Duration::from_nanos(1_000_020_833));
        assert_eq!(duree_son(0), Duration::ZERO);
    }

    // --- NV12 ---

    fn image_unie(l: u32, h: u32, c: [u8; 3]) -> Vec<u8> {
        let mut v = Vec::with_capacity((l * h * 4) as usize);
        for _ in 0..l * h {
            v.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
        v
    }

    fn nv12(rgba: &[u8], l: u32, h: u32) -> Vec<u8> {
        let mut s = vec![0u8; taille_nv12(l, h).unwrap()];
        vers_nv12(rgba, l, h, &mut s).unwrap();
        s
    }

    /// Retour en RGB (BT.709, plage réduite), pour comparer.
    fn vers_rgb(y: u8, u: u8, v: u8) -> [i32; 3] {
        let y = 1.164_383_6 * (f64::from(y) - 16.0);
        let u = f64::from(u) - 128.0;
        let v = f64::from(v) - 128.0;
        [
            (y + 1.792_741 * v).round() as i32,
            (y - 0.213_249 * u - 0.532_909 * v).round() as i32,
            (y + 2.112_402 * u).round() as i32,
        ]
    }

    /// Les valeurs de référence de BT.709 en plage réduite.
    #[test]
    fn nv12_couleurs_de_reference() {
        for (c, attendu) in [
            ([255, 255, 255], [235, 128, 128]),
            ([0, 0, 0], [16, 128, 128]),
            ([255, 0, 0], [63, 102, 240]),
            ([0, 255, 0], [173, 42, 26]),
            ([0, 0, 255], [32, 240, 118]),
            ([128, 128, 128], [126, 128, 128]),
        ] {
            let s = nv12(&image_unie(4, 2, c), 4, 2);
            assert_eq!(s.len(), 12);
            for &y in &s[..8] {
                assert_eq!(y, attendu[0], "Y de {c:?}");
            }
            for uv in s[8..].chunks(2) {
                assert_eq!([uv[0], uv[1]], [attendu[1], attendu[2]], "UV de {c:?}");
            }
        }
    }

    /// Toute couleur revient à ± 2 par canal sur un carré uni de 2 × 2.
    #[test]
    fn nv12_aller_retour() {
        let mut pire = 0;
        for r in (0..=255).step_by(15) {
            for g in (0..=255).step_by(15) {
                for b in (0..=255).step_by(15) {
                    let c = [r as u8, g as u8, b as u8];
                    let s = nv12(&image_unie(2, 2, c), 2, 2);
                    let back = vers_rgb(s[0], s[4], s[5]);
                    for k in 0..3 {
                        pire = pire.max((back[k] - i32::from(c[k])).abs());
                    }
                }
            }
        }
        assert!(pire <= 2, "écart {pire}");
    }

    /// 602 × 452 (côtés qui ne sont pas des multiples de 16) : chaque point d'un motif
    /// connu est à sa place dans les deux plans ; le plan UV a la largeur de ligne de Y.
    #[test]
    fn nv12_motif_a_sa_place_en_602x452() {
        let (l, h) = (602u32, 452u32);
        let mut rgba = image_unie(l, h, [128, 128, 128]);
        // Carrés de 2 × 2 alignés sur la chrominance, dont les derniers pixels.
        let points = [
            (0u32, 0u32),
            (600, 0),
            (0, 450),
            (600, 450),
            (300, 226),
            (2, 448),
        ];
        for &(x, y) in &points {
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let i = (((y + dy) * l + x + dx) * 4) as usize;
                rgba[i..i + 3].copy_from_slice(&[255, 0, 0]);
            }
        }
        let s = nv12(&rgba, l, h);
        assert_eq!(s.len(), (l * h * 3 / 2) as usize);
        let (py, puv) = s.split_at((l * h) as usize);
        let mut rouges_y = 0;
        for (i, &y) in py.iter().enumerate() {
            let (x, yy) = (i as u32 % l, i as u32 / l);
            let dans = points
                .iter()
                .any(|&(px, pyy)| (px..px + 2).contains(&x) && (pyy..pyy + 2).contains(&yy));
            if dans {
                assert_eq!(y, 63, "Y en {x},{yy}");
                rouges_y += 1;
            } else {
                assert_eq!(y, 126, "Y en {x},{yy}");
            }
        }
        assert_eq!(rouges_y, points.len() * 4);
        for (i, uv) in puv.chunks(2).enumerate() {
            let (cx, cy) = ((i as u32 % (l / 2)) * 2, (i as u32 / (l / 2)) * 2);
            if points.contains(&(cx, cy)) {
                assert_eq!(uv, [102, 240], "UV en {cx},{cy}");
            } else {
                assert_eq!(uv, [128, 128], "UV en {cx},{cy}");
            }
        }
    }

    /// La chrominance est la moyenne du carré : quatre couleurs différentes donnent la
    /// chrominance de leur moyenne ; le luma reste propre à chaque pixel.
    #[test]
    fn nv12_moyenne_de_chaque_carre() {
        let rgba = [
            255, 0, 0, 255, 0, 255, 0, 255, //
            0, 0, 255, 255, 255, 255, 255, 0,
        ];
        let s = nv12(&rgba, 2, 2);
        assert_eq!(&s[..4], &[63, 173, 32, 235]);
        let moyenne = nv12(&image_unie(2, 2, [128, 128, 128]), 2, 2);
        // (255+0+0+255)/4 = 127,5 pour chaque canal : gris, à 1 près.
        assert!((i32::from(s[4]) - i32::from(moyenne[4])).abs() <= 1);
        assert!((i32::from(s[5]) - i32::from(moyenne[5])).abs() <= 1);
    }

    /// Côtés impairs, nuls ou longueurs fausses : une erreur, jamais une panique ; un
    /// tampon plus grand n'est touché que sur la taille NV12.
    #[test]
    fn nv12_refuse_sans_paniquer() {
        let mut s = vec![7u8; 64];
        for (l, h) in [(3u32, 2u32), (2, 3), (0, 2), (2, 0), (601, 451)] {
            let rgba = vec![0u8; (l * h * 4) as usize];
            assert!(vers_nv12(&rgba, l, h, &mut s).is_err(), "{l}x{h}");
        }
        assert!(vers_nv12(&[0u8; 15], 2, 2, &mut s).is_err());
        assert!(vers_nv12(&[0u8; 16], 2, 2, &mut [0u8; 5]).is_err());
        assert!(vers_nv12(&[0u8; 16], u32::MAX - 1, u32::MAX - 1, &mut s).is_err());
        assert_eq!(s, vec![7u8; 64], "rien d'écrit sur un refus");
        vers_nv12(&[0u8; 16], 2, 2, &mut s).unwrap();
        assert_eq!(&s[..6], &[16, 16, 16, 16, 128, 128]);
        assert!(s[6..].iter().all(|&x| x == 7));
        assert_eq!(taille_nv12(600, 450), Some(405_000));
    }

    /// Le temps de la conversion d'une grande image, noté (en version de test, sans
    /// optimisation : la mesure qui compte est celle de l'essai 15).
    #[test]
    fn nv12_temps_note() {
        let (l, h) = (2560u32, 1440u32);
        let rgba: Vec<u8> = (0..l * h * 4).map(|i| (i % 251) as u8).collect();
        let mut s = vec![0u8; taille_nv12(l, h).unwrap()];
        let debut = Instant::now();
        vers_nv12(&rgba, l, h, &mut s).unwrap();
        eprintln!("nv12 {l}x{h} en {:?}", debut.elapsed());
    }
}
