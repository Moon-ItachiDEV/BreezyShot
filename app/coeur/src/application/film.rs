//! Vidéo et GIF (contrat 0.7) : les décisions du cœur, en fonctions pures (§4.2, §5, §8,
//! §10). `geste_film.rs` fait les gestes (fils, fenêtre de contrôle, sortie) ;
//! `fenetre_film.rs` reçoit les trois commandes de la page de contrôle. Un enregistrement,
//! vidéo ou GIF, s'appelle un **film** dans le code et le journal (jamais à l'écran).
//!
//! Les phases du film (`Phase`) ne sont pas des états de l'automate : elles ne changent
//! rien à ce que les autres demandes peuvent faire (§4.2).
//!
//! Choix de l'utilisateur pris provisoirement (contrat 0.7, §18), chacun isolé ici :
//! - U4 (piste B) : un film va toujours dans le dossier d'enregistrement, déplacé
//!   (`TOUJOURS_DANS_LE_DOSSIER`, `demande_film`) ;
//! - B1 : le raccourci qui a lancé le film l'arrête (`ARRET_PAR_RACCOURCI`,
//!   `arret_par_raccourci`) ;
//! - B2 : le rond de lecture de la vignette ouvre le film (`vignette::LECTURE_PAR_LE_ROND`,
//!   `vignette::actions_film`) ;
//! - U1 (piste A) : `Ctrl + Maj + 6` et `Ctrl + Maj + 7` (`reglages`, défauts), « Vidéo »
//!   et « GIF » (`libelle_menu`, `page_reglages::libelle_action`).

use std::time::{Duration, Instant};

use super::etats::Mode;
use crate::droits::{self, Fonction};
use crate::fenetres::film::{BoutonMicro, BoutonPause, ChargeEtat};
use crate::sortie::{self, BilanFilm, Etape, EtapeFichier, SorteFilm};
use crate::systeme::gel::{Changement, EcranInfo};
use crate::systeme::{mp4, Rect};

/// L'interrupteur du film : vrai quand vidéo et GIF sont construits (chantiers A à J,
/// ouvert à l'assemblage). Faux, ni « Vidéo » ni « GIF » ne sont des actions construites
/// (ni menu, ni raccourci, ni ligne de Réglages, ni `--capturer`), même avec une licence,
/// et la recherche du codeur n'est jamais lancée : rien de montré qui ne marche pas.
pub const CONSTRUIT: bool = true;

/// Côté minimal d'une zone, en pixels logiques (F4, choix U6).
pub const COTE_MIN_LOGIQUE: f64 = 120.0;
/// Le décompte quand le réglage « Retardateur » dit « Aucun » (F7).
pub const DECOMPTE_DEFAUT_S: u32 = 3;
/// Images par seconde d'une vidéo et d'un GIF (F13).
pub const CADENCE_VIDEO: u32 = 30;
pub const CADENCE_GIF: u32 = 15;
/// La page de contrôle doit avoir posé sa pilule (`film_zone`) dans ce délai après le
/// relâchement (F5).
pub const DELAI_PAGE: Duration = Duration::from_secs(2);
/// La préparation doit être finie dans ce délai après la fin du décompte (F11).
pub const DELAI_PRET: Duration = Duration::from_secs(3);
/// Le fichier doit être fermé dans ce délai après la fin (F29).
pub const DELAI_FERMETURE: Duration = Duration::from_secs(10);
/// L'arrêt de l'app attend la fermeture du fichier au plus ce délai (F35).
pub const DELAI_ARRET: Duration = Duration::from_secs(3);
/// Sans image pendant ce délai, `BitBlt` est essayé (F33).
pub const SOURCE_PERDUE_MAX: Duration = Duration::from_secs(30);
/// `BitBlt` essayé après une source perdue : sans image pendant ce délai, fin `source`
/// (F33).
pub const SOURCE_REPLI_MAX: Duration = Duration::from_secs(2);
/// Attente, au départ d'un film, que la source du film précédent soit rendue : deux
/// duplications du même écran ne peuvent pas être ouvertes ensemble (la seconde passe en
/// BitBlt). Passé ce délai, le film part quand même (journal).
pub const SOURCE_RENDUE_MAX: Duration = Duration::from_secs(2);
/// Deux tours du fil `film` séparés de plus que ce délai : une veille (F34).
pub const SAUT_VEILLE: Duration = Duration::from_secs(5);
/// Durées maximales (choix U3).
pub const DUREE_MAX_VIDEO: Duration = Duration::from_secs(60 * 60);
pub const DUREE_MAX_GIF: Duration = Duration::from_secs(60);
pub const PAUSE_MAX: Duration = Duration::from_secs(30 * 60);
/// Place libre minimale du volume de `<local>` (F28).
pub const PLACE_MIN: u64 = 200 * 1024 * 1024;
/// La place libre est relue aussi souvent pendant un film (F28).
pub const RELECTURE_DISQUE: Duration = Duration::from_secs(5);
/// La liste des écrans est relue aussi souvent par le fil `film` (F10).
pub const SURVEILLANCE_ECRANS: Duration = Duration::from_millis(500);
/// Le fil `film` fait un tour au moins aussi souvent pendant l'attente et le décompte (F10).
pub const TOUR: Duration = Duration::from_millis(100);
/// La file entre le fil `film` et le fil d'écriture (F20) : au plus 4 images ou 64 Mo,
/// jamais moins de 2 images.
pub const FILE_IMAGES_MAX: usize = 4;
pub const FILE_IMAGES_MIN: usize = 2;
pub const FILE_OCTETS_MAX: u64 = 64 * 1024 * 1024;
/// Vitesse de la quantification du GIF (F19).
pub const VITESSE_GIF: i32 = 10;

/// Nom accessible de la carte d'un film, et annonce sans copie ni enregistrement (F25, F26).
pub const TITRE_CARTE: &str = "Enregistrement terminé";
/// Le bandeau quand le micro est refusé ou perdu (F18, choix U5), titre seul.
pub const TITRE_MICRO_INDISPONIBLE: &str = "Micro indisponible";
/// Le bandeau quand le fichier ne se ferme pas (F29), titre seul.
pub const TITRE_FICHIER_IMPOSSIBLE: &str = "Impossible d'enregistrer";

/// Textes de la pilule (§11).
pub const LIBELLE_PAUSE: &str = "Pause";
pub const LIBELLE_REPRENDRE: &str = "Reprendre";
pub const LIBELLE_ARRETER: &str = "Arrêter";
pub const LIBELLE_MICRO: &str = "Micro";
pub const LIBELLE_MICRO_OUVERT: &str = "Micro activé";
pub const GROUPE: &str = "Enregistrement";

/// Placement de la pilule (§10), pixels logiques : à 10 de la zone, à 6 au moins des bords
/// de la zone utile.
pub const ECART_PILULE: f64 = 10.0;
pub const MARGE_PILULE: f64 = 6.0;

/// Choix U4 (piste B, provisoire) : un film va toujours dans le dossier d'enregistrement
/// (Images › BreezyShot), quel que soit « Enregistrer automatiquement », et il y est
/// déplacé, pas copié. Faux : la règle de F22 à F24 (le film suit les deux réglages de
/// « Après la capture » et reste sinon dans `<local>/Films`).
pub const TOUJOURS_DANS_LE_DOSSIER: bool = true;

/// Choix B1 (provisoire) : pendant un film, le raccourci qui l'a lancé l'arrête
/// (`Ctrl + Maj + 6` pendant une vidéo). Faux : ce raccourci est ignoré comme les autres.
pub const ARRET_PAR_RACCOURCI: bool = true;

/// La place libre à garder sur le volume de `<local>` pendant un film (F28) : 200 Mo ou
/// dix secondes d'écriture au débit observé, la plus grande des deux.
pub fn marge_disque(octets_par_s: u64) -> u64 {
    PLACE_MIN.max(octets_par_s.saturating_mul(10))
}

/// La recherche du codeur H.264 (F32) n'est lancée que si le film est construit et que la
/// vidéo est ouverte par la licence (§6) : sans licence, rien de Media Foundation n'est
/// appelé ni chargé.
pub fn recherche_voulue(pro: bool) -> bool {
    recherche_selon(CONSTRUIT, pro)
}

/// Règle pure de `recherche_voulue`, `construit` donné.
pub fn recherche_selon(construit: bool, pro: bool) -> bool {
    construit && droits::ouverte(Fonction::Video, pro)
}

/// La demande de sortie d'un film (F23, choix U4), d'après celle des réglages du moment :
/// jamais « Récents » (F24) ; avec `TOUJOURS_DANS_LE_DOSSIER`, toujours enregistré. Rend
/// aussi `deplacer` (`sortie::sortir_film`) : le fichier de travail est déplacé dans le
/// dossier d'enregistrement au lieu d'y être copié.
pub fn demande_film(base: &sortie::Demande) -> (sortie::Demande, bool) {
    let demande = sortie::Demande {
        enregistrer: base.enregistrer || TOUJOURS_DANS_LE_DOSSIER,
        ranger_recents: false,
        transparence: false,
        ..base.clone()
    };
    (demande, TOUJOURS_DANS_LE_DOSSIER)
}

/// Choix B1 : le raccourci du mode `presse`, pendant un film de sorte `en_cours`, est-il
/// « Arrêter » ? Vrai seulement pour le raccourci du même mode.
pub fn arret_par_raccourci(en_cours: Sorte, presse: Mode) -> bool {
    ARRET_PAR_RACCOURCI && Sorte::depuis_mode(presse) == Some(en_cours)
}

/// La sorte d'un film.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sorte {
    Video,
    Gif,
}

impl Sorte {
    /// La sorte du mode `mode`, `None` hors des deux modes d'un film.
    pub fn depuis_mode(mode: Mode) -> Option<Sorte> {
        match mode {
            Mode::Video => Some(Sorte::Video),
            Mode::Gif => Some(Sorte::Gif),
            _ => None,
        }
    }

    /// Le mode de l'automate.
    pub fn mode(self) -> Mode {
        match self {
            Sorte::Video => Mode::Video,
            Sorte::Gif => Mode::Gif,
        }
    }

    /// Nom au journal : `video`, `gif`.
    pub fn nom(self) -> &'static str {
        self.mode().nom()
    }

    /// La même sorte, vue de la sortie.
    pub fn sortie(self) -> SorteFilm {
        match self {
            Sorte::Video => SorteFilm::Video,
            Sorte::Gif => SorteFilm::Gif,
        }
    }

    /// Images par seconde (F13).
    pub fn cadence(self) -> u32 {
        match self {
            Sorte::Video => CADENCE_VIDEO,
            Sorte::Gif => CADENCE_GIF,
        }
    }

    /// La durée de film au-delà de laquelle il finit (`limite`, choix U3).
    pub fn duree_max(self) -> Duration {
        match self {
            Sorte::Video => DUREE_MAX_VIDEO,
            Sorte::Gif => DUREE_MAX_GIF,
        }
    }
}

/// La zone d'un film cadrée (F4) : la zone filmée (pixels réels du bureau, celle que la
/// source prend : pour une vidéo, ses côtés pairs, doublés si l'image est réduite), la
/// taille de l'image écrite, et si la zone a été agrandie ou l'image réduite de moitié.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cadre {
    pub zone: Rect,
    pub image: (u32, u32),
    pub agrandie: bool,
    pub reduite: bool,
}

/// Ce que donne le cadrage d'une zone (F4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cadrage {
    Bon(Cadre),
    /// Rien de valide : `SelectionAnnulee`, sans message.
    Vide,
    /// Le codeur refuse la taille, même réduite : `SelectionImpossible`.
    Refuse,
}

/// Le nombre pair inférieur ou égal.
fn pair(n: u32) -> u32 {
    n & !1
}

/// F4 : la zone portée au côté minimal autour de son centre, sur son écran
/// (`defilante::agrandir`) ; la taille de l'image (côtés ramenés au pair inférieur pour
/// une vidéo, la zone entière pour un GIF) ; pour une vidéo que `acceptee` refuse, l'image
/// réduite de moitié une fois, chaque côté obtenu ramené au pair inférieur (4098 donne
/// 2049, donc 2048) ; refusée encore : `Refuse`.
pub fn cadrer(
    zone: Rect,
    ecran: &EcranInfo,
    sorte: Sorte,
    acceptee: impl Fn(u32, u32) -> bool,
) -> Cadrage {
    if zone.est_vide() || ecran.rect.est_vide() {
        return Cadrage::Vide;
    }
    let (z, agrandie) = super::defilante::agrandir(zone, ecran);
    if z.est_vide() {
        return Cadrage::Vide;
    }
    let (l, h) = (z.largeur as u32, z.hauteur as u32);
    let filmee = |largeur: u32, hauteur: u32| Rect {
        x: z.x,
        y: z.y,
        largeur: largeur as i32,
        hauteur: hauteur as i32,
    };
    match sorte {
        Sorte::Gif => {
            if l > u32::from(u16::MAX) || h > u32::from(u16::MAX) {
                return Cadrage::Refuse;
            }
            Cadrage::Bon(Cadre {
                zone: z,
                image: (l, h),
                agrandie,
                reduite: false,
            })
        }
        Sorte::Video => {
            let (pl, ph) = (pair(l), pair(h));
            if acceptee(pl, ph) {
                return Cadrage::Bon(Cadre {
                    zone: filmee(pl, ph),
                    image: (pl, ph),
                    agrandie,
                    reduite: false,
                });
            }
            let (ml, mh) = (pair(pl / 2), pair(ph / 2));
            if ml > 0 && mh > 0 && acceptee(ml, mh) {
                return Cadrage::Bon(Cadre {
                    zone: filmee(ml * 2, mh * 2),
                    image: (ml, mh),
                    agrandie,
                    reduite: true,
                });
            }
            Cadrage::Refuse
        }
    }
}

/// L'échelle d'un écran, 1 si elle est absurde.
fn echelle(ecran: &EcranInfo) -> f64 {
    if ecran.echelle.is_finite() && ecran.echelle > 0.0 {
        ecran.echelle
    } else {
        1.0
    }
}

/// La taille réelle de la pilule (pixels réels) d'après sa taille logique et l'écran.
pub fn taille_pilule(logique: (f64, f64), ecran: &EcranInfo) -> (f64, f64) {
    let s = echelle(ecran);
    (logique.0 * s, logique.1 * s)
}

/// La place de la pilule (§10, pixels réels du bureau, son coin haut gauche) : à 10 px
/// logiques sous la zone, son bord droit sur celui de la zone ; sans place dessous dans
/// la zone utile de l'écran, au-dessus ; sans place au-dessus, dedans, en bas à droite, à
/// 10 px des bords ; toujours à 6 px au moins des bords de la zone utile. `pilule` : sa
/// taille, pixels réels.
pub fn placer(zone: Rect, ecran: &EcranInfo, pilule: (f64, f64)) -> (i32, i32) {
    let s = echelle(ecran);
    let ecart = (ECART_PILULE * s).round() as i64;
    let marge = (MARGE_PILULE * s).round() as i64;
    let pl = if pilule.0.is_finite() {
        pilule.0.max(0.0).ceil() as i64
    } else {
        0
    };
    let ph = if pilule.1.is_finite() {
        pilule.1.max(0.0).ceil() as i64
    } else {
        0
    };
    let utile = if ecran.zone_utile.est_vide() {
        ecran.rect
    } else {
        ecran.zone_utile
    };
    let (haut_min, bas_max) = (utile.y as i64 + marge, utile.bas() - marge);
    let (gauche_min, droite_max) = (utile.x as i64 + marge, utile.droite() - marge);
    let borner = |v: i64, min: i64, max: i64| v.min(max).max(min);
    let x = borner(zone.droite() - pl, gauche_min, droite_max - pl);
    let dessous = zone.bas() + ecart;
    let dessus = zone.y as i64 - ecart - ph;
    let (x, y) = if dessous + ph <= bas_max {
        (x, dessous)
    } else if dessus >= haut_min {
        (x, dessus)
    } else {
        (
            borner(zone.droite() - ecart - pl, gauche_min, droite_max - pl),
            borner(zone.bas() - ecart - ph, haut_min, bas_max - ph),
        )
    };
    let i = |v: i64| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    (i(x), i(y))
}

/// La durée du décompte (F7) : le réglage « Retardateur » s'il n'est pas « Aucun », sinon
/// `DECOMPTE_DEFAUT_S`.
pub fn decompte_s(retardateur_s: u32) -> u32 {
    if retardateur_s == 0 {
        DECOMPTE_DEFAUT_S
    } else {
        retardateur_s
    }
}

/// Le débit moyen visé d'une vidéo (F15), bits par seconde (`mp4::debit_vise`).
pub fn debit(largeur: u32, hauteur: u32) -> u32 {
    mp4::debit_vise(largeur, hauteur)
}

/// La file entre le fil `film` et le fil d'écriture pour des images de `octets` (F20) :
/// au plus 4 images ou 64 Mo, jamais moins de 2 images.
pub fn file_max(octets: u64) -> usize {
    let par_octets = FILE_OCTETS_MAX.checked_div(octets).unwrap_or(u64::MAX);
    (par_octets.min(FILE_IMAGES_MAX as u64) as usize).max(FILE_IMAGES_MIN)
}

/// « 0:07 », « 12:05 », « 60:00 ».
pub fn texte_duree(d: Duration) -> String {
    let s = d.as_secs();
    format!("{}:{:02}", s / 60, s % 60)
}

/// La durée de la vignette : « 0:07 » pour une vidéo, « GIF · 0:07 » pour un GIF, jamais
/// moins de « 0:01 ».
pub fn texte_vignette(sorte: Sorte, d: Duration) -> String {
    let texte = texte_duree(d.max(Duration::from_secs(1)));
    match sorte {
        Sorte::Video => texte,
        Sorte::Gif => format!("GIF · {texte}"),
    }
}

/// Les phases d'un film (§4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// La zone est choisie ; la fenêtre de contrôle n'est pas encore montrée.
    Attente,
    /// Le disque compte ; rien n'est enregistré.
    Decompte,
    /// Les images sont écrites.
    Marche,
    /// Rien n'est écrit ; l'horloge du film est arrêtée.
    Pause,
}

impl Phase {
    /// Nom au journal et pour la page : `attente`, `decompte`, `marche`, `pause`.
    pub fn nom(self) -> &'static str {
        match self {
            Phase::Attente => "attente",
            Phase::Decompte => "decompte",
            Phase::Marche => "marche",
            Phase::Pause => "pause",
        }
    }

    /// Avant que les images ne soient écrites (les annulations de F10).
    pub fn avant(self) -> bool {
        matches!(self, Phase::Attente | Phase::Decompte)
    }
}

/// Le cours d'un film : sa sorte, sa phase, le micro, la préparation, la fin rendue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cours {
    sorte: Sorte,
    phase: Phase,
    /// La durée du décompte (F7).
    secondes: u32,
    /// `None` : pas de bouton « Micro » ; `Some(ouvert)`.
    micro: Option<bool>,
    /// La présence du micro est connue (toujours pour un GIF).
    micro_connu: bool,
    /// La source et le fil d'écriture sont prêts (F9).
    pret: bool,
    decompte_fini: bool,
    /// Une fin (gardée, annulée ou impossible) est rendue : rien d'autre ne compte.
    fini: bool,
    /// La durée du film, à la seconde (`Seconde`).
    duree: Duration,
    /// « Micro indisponible » à dire au départ (le bandeau effacerait le disque).
    micro_a_dire: bool,
}

impl Cours {
    /// Un film de `sorte` qui commence (phase `Attente`), avec un décompte de `secondes`.
    pub fn nouveau(sorte: Sorte, secondes: u32) -> Cours {
        Cours {
            sorte,
            phase: Phase::Attente,
            secondes,
            micro: None,
            micro_connu: sorte == Sorte::Gif,
            pret: false,
            decompte_fini: false,
            fini: false,
            duree: Duration::ZERO,
            micro_a_dire: false,
        }
    }

    /// La présence du micro est connue (F17, vidéo seulement) : le bouton « Micro » est
    /// montré, coupé, s'il y en a un. Une seconde réponse ne change rien.
    pub fn micro_present(&mut self, present: bool) {
        if !self.micro_connu {
            self.micro_connu = true;
            self.micro = (present && self.sorte == Sorte::Video).then_some(false);
        }
    }

    pub fn sorte(&self) -> Sorte {
        self.sorte
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn secondes(&self) -> u32 {
        self.secondes
    }

    pub fn micro(&self) -> Option<bool> {
        self.micro
    }

    /// La page peut être montrée : on sait s'il y a un bouton « Micro ».
    pub fn micro_connu(&self) -> bool {
        self.micro_connu
    }

    pub fn duree(&self) -> Duration {
        self.duree
    }

    pub fn fini(&self) -> bool {
        self.fini
    }

    fn demarrer(&mut self) -> Vec<Geste> {
        if self.phase != Phase::Decompte || !self.pret || !self.decompte_fini {
            return Vec::new();
        }
        self.phase = Phase::Marche;
        let mut gestes = vec![Geste::Demarrer, Geste::Etat];
        if std::mem::take(&mut self.micro_a_dire) {
            gestes.push(Geste::Dire(TITRE_MICRO_INDISPONIBLE));
        }
        gestes
    }

    fn finir(&mut self, fin: Fin) -> Vec<Geste> {
        self.fini = true;
        vec![Geste::Finir(fin)]
    }

    fn annuler(&mut self, cause: Annulation) -> Vec<Geste> {
        self.fini = true;
        vec![Geste::Annuler(cause)]
    }

    fn impossible(&mut self, cause: &'static str) -> Vec<Geste> {
        self.fini = true;
        vec![Geste::Impossible(cause)]
    }

    /// Annulée avant les images, gardée ensuite (écrans, session, veille, arrêt).
    fn annuler_ou_finir(&mut self, avant: Annulation, apres: Fin) -> Vec<Geste> {
        if self.phase.avant() {
            self.annuler(avant)
        } else {
            self.finir(apres)
        }
    }
}

/// Ce que le cœur voit arriver pendant un film.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vu {
    PageMontree,
    DecompteFini,
    Pret,
    Pause,
    Arreter,
    Micro,
    Echap,
    Croix,
    /// Phase `Attente` seulement (F10).
    Bureau(Changement),
    /// Relus par le fil `film`.
    Ecrans,
    Session,
    Veille,
    Arret,
    Seconde(Duration),
    Limite,
    Disque,
    SourcePerdue,
    SourceReprise,
    SourceFinie,
    MicroPerdu,
    PretEchu,
    /// Une préparation ratée ou un fil arrêté (F9, F37) : `film impossible: <cause>`.
    Echec(&'static str),
}

/// Comment un film finit en gardant ce qui est enregistré (F27, F28).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fin {
    Bouton,
    Limite,
    Disque,
    Ecrans,
    Session,
    Veille,
    Arret,
    Source,
}

impl Fin {
    /// Nom au journal (`film fin <nom>`).
    pub fn nom(self) -> &'static str {
        match self {
            Fin::Bouton => "bouton",
            Fin::Limite => "limite",
            Fin::Disque => "disque",
            Fin::Ecrans => "ecrans",
            Fin::Session => "session",
            Fin::Veille => "veille",
            Fin::Arret => "arret",
            Fin::Source => "source",
        }
    }
}

/// Comment un film est annulé pendant l'attente ou le décompte (F10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Annulation {
    Echap,
    Croix,
    Arreter,
    Ecrans,
    Session,
    Veille,
    Arret,
}

impl Annulation {
    /// Nom au journal (`film annule <phase> (<nom>)`).
    pub fn nom(self) -> &'static str {
        match self {
            Annulation::Echap => "echap",
            Annulation::Croix => "croix",
            Annulation::Arreter => "arreter",
            Annulation::Ecrans => "ecrans",
            Annulation::Session => "session",
            Annulation::Veille => "veille",
            Annulation::Arret => "arret",
        }
    }
}

/// Ce que le cœur doit faire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Geste {
    Decompter(u32),
    Demarrer,
    Suspendre,
    Reprendre,
    OuvrirMicro,
    CouperMicro,
    /// Renvoyer `film:etat` à la page.
    Etat,
    /// Un message du bandeau (titre seul).
    Dire(&'static str),
    Finir(Fin),
    Annuler(Annulation),
    /// `film impossible: <cause>`.
    Impossible(&'static str),
}

/// Ce que fait `vu` dans le cours du film (une seule fin par film : la première gagne).
pub fn suite(cours: &mut Cours, vu: Vu) -> Vec<Geste> {
    if cours.fini {
        return Vec::new();
    }
    let phase = cours.phase;
    match vu {
        Vu::PageMontree if phase == Phase::Attente => {
            cours.phase = Phase::Decompte;
            vec![Geste::Decompter(cours.secondes), Geste::Etat]
        }
        Vu::DecompteFini if phase == Phase::Decompte => {
            cours.decompte_fini = true;
            cours.demarrer()
        }
        Vu::Pret => {
            cours.pret = true;
            cours.demarrer()
        }
        Vu::PretEchu if phase == Phase::Decompte && !cours.pret => cours.impossible("preparation"),
        Vu::Pause => match phase {
            Phase::Marche => {
                cours.phase = Phase::Pause;
                vec![Geste::Suspendre, Geste::Etat]
            }
            Phase::Pause => {
                cours.phase = Phase::Marche;
                vec![Geste::Reprendre, Geste::Etat]
            }
            // Pendant le décompte, « Pause » est inactif (§11).
            _ => Vec::new(),
        },
        Vu::Arreter => match phase {
            Phase::Decompte => cours.annuler(Annulation::Arreter),
            Phase::Marche | Phase::Pause => cours.finir(Fin::Bouton),
            Phase::Attente => Vec::new(),
        },
        Vu::Micro => match cours.micro {
            Some(ouvert) if phase != Phase::Attente => {
                cours.micro = Some(!ouvert);
                let geste = if ouvert {
                    Geste::CouperMicro
                } else {
                    Geste::OuvrirMicro
                };
                vec![geste, Geste::Etat]
            }
            _ => Vec::new(),
        },
        Vu::MicroPerdu => {
            if cours.micro.is_none() {
                return Vec::new();
            }
            cours.micro = Some(false);
            if phase.avant() {
                cours.micro_a_dire = true;
                vec![Geste::Etat]
            } else {
                vec![Geste::Dire(TITRE_MICRO_INDISPONIBLE), Geste::Etat]
            }
        }
        Vu::Echap if phase.avant() => cours.annuler(Annulation::Echap),
        Vu::Croix if phase == Phase::Decompte => cours.annuler(Annulation::Croix),
        Vu::Bureau(changement) if phase == Phase::Attente => cours.annuler(match changement {
            Changement::Ecrans => Annulation::Ecrans,
            Changement::SessionVerrouillee => Annulation::Session,
        }),
        Vu::Ecrans => cours.annuler_ou_finir(Annulation::Ecrans, Fin::Ecrans),
        Vu::Session => cours.annuler_ou_finir(Annulation::Session, Fin::Session),
        Vu::Veille => cours.annuler_ou_finir(Annulation::Veille, Fin::Veille),
        Vu::Arret => cours.annuler_ou_finir(Annulation::Arret, Fin::Arret),
        Vu::Seconde(d) if phase == Phase::Marche => {
            cours.duree = d;
            vec![Geste::Etat]
        }
        Vu::Limite if !phase.avant() => cours.finir(Fin::Limite),
        Vu::Disque if phase.avant() => cours.impossible("disque"),
        Vu::Disque => cours.finir(Fin::Disque),
        Vu::SourceFinie if phase.avant() => cours.impossible("source"),
        Vu::SourceFinie => cours.finir(Fin::Source),
        Vu::Echec(cause) => cours.impossible(cause),
        _ => Vec::new(),
    }
}

/// La charge de `film:etat` (§11) : ce que montre la pilule du film `id`.
pub fn charge_etat(id: u64, cours: &Cours) -> ChargeEtat {
    let phase = match cours.phase {
        Phase::Attente | Phase::Decompte => "decompte",
        Phase::Marche => "marche",
        Phase::Pause => "pause",
    };
    ChargeEtat {
        id,
        phase,
        duree: texte_duree(cours.duree),
        pause: BoutonPause {
            libelle: if cours.phase == Phase::Pause {
                LIBELLE_REPRENDRE
            } else {
                LIBELLE_PAUSE
            }
            .into(),
            actif: !cours.phase.avant(),
        },
        arreter: LIBELLE_ARRETER.into(),
        micro: cours.micro.map(|ouvert| BoutonMicro {
            libelle: if ouvert {
                LIBELLE_MICRO_OUVERT
            } else {
                LIBELLE_MICRO
            }
            .into(),
            ouvert,
        }),
        groupe: GROUPE.into(),
    }
}

/// Médiane de durées en mémoire constante (cases de 0,1 ms jusqu'à 100 ms, la dernière
/// pour tout ce qui dépasse) : les lignes `chrono film` d'un film d'une heure.
#[derive(Debug, Clone)]
pub struct Mediane {
    cases: Vec<u64>,
    nombre: u64,
}

impl Default for Mediane {
    fn default() -> Self {
        Mediane {
            cases: vec![0; 1001],
            nombre: 0,
        }
    }
}

impl Mediane {
    pub fn ajouter(&mut self, d: Duration) {
        let case = (d.as_micros() / 100).min(1000) as usize;
        self.cases[case] += 1;
        self.nombre += 1;
    }

    /// La médiane en millisecondes (au dixième), `None` sans aucune mesure.
    pub fn valeur(&self) -> Option<f64> {
        if self.nombre == 0 {
            return None;
        }
        let rang = self.nombre.div_ceil(2);
        let mut vus = 0;
        for (i, n) in self.cases.iter().enumerate() {
            vus += n;
            if vus >= rang {
                return Some(i as f64 / 10.0);
            }
        }
        Some(100.0)
    }
}

/// Millisecondes entières d'une durée (journal).
fn ms(d: Duration) -> u128 {
    d.as_millis()
}

fn oui_non(v: bool) -> &'static str {
    if v {
        "oui"
    } else {
        "non"
    }
}

/// La ligne du journal du cadrage : `film <K> zone <x>,<y> <l>x<h> agrandie <oui|non>
/// image <l>x<h>` (`zone` : la zone tracée, pixels réels).
pub fn ligne_zone(sorte: Sorte, zone: Rect, cadre: &Cadre) -> String {
    format!(
        "film {} zone {},{} {}x{} agrandie {} image {}x{}",
        sorte.nom(),
        zone.x,
        zone.y,
        zone.largeur,
        zone.hauteur,
        oui_non(cadre.agrandie),
        cadre.image.0,
        cadre.image.1
    )
}

/// `film page prete en <ms> ms`.
pub fn ligne_page_prete(d: Duration) -> String {
    format!("film page prete en {} ms", ms(d))
}

/// `film decompte <s> s`.
pub fn ligne_decompte(secondes: u32) -> String {
    format!("film decompte {secondes} s")
}

/// `film annule <attente|decompte> (<cause>)`.
pub fn ligne_annule(phase: Phase, cause: Annulation) -> String {
    format!("film annule {} ({})", phase.nom(), cause.nom())
}

/// `film source <duplication|bitblt>[ (<raison>)]`.
pub fn ligne_source(moyen: &str, raison: Option<&str>) -> String {
    match raison {
        Some(r) if !r.is_empty() => format!("film source {moyen} ({r})"),
        _ => format!("film source {moyen}"),
    }
}

/// `film source reprise apres <ms> ms`.
pub fn ligne_reprise(perdue: Duration) -> String {
    format!("film source reprise apres {} ms", ms(perdue))
}

/// `film codeur <materiel|logiciel> <l>x<h> <kbit> kbit/s`.
pub fn ligne_codeur(codeur: &str, image: (u32, u32), debit_bit_s: u32) -> String {
    format!(
        "film codeur {codeur} {}x{} {} kbit/s",
        image.0,
        image.1,
        debit_bit_s / 1000
    )
}

/// `film son <oui|non>`.
pub fn ligne_son(son: bool) -> String {
    format!("film son {}", oui_non(son))
}

/// `film micro <ouvert|coupe|indisponible>`.
pub fn ligne_micro(etat: &str) -> String {
    format!("film micro {etat}")
}

/// L'instant de départ d'un film (F11) : `film debut (relache <ms> ms, pret <ms> ms,
/// decompte-premiere <ms> ms)` ; `relache` : du relâchement à la première image, `pret` :
/// du relâchement à la préparation finie, `decompte-premiere` : de la fin du décompte à la
/// première image.
pub fn ligne_debut(
    relache: Instant,
    pret: Instant,
    fin_decompte: Instant,
    premiere: Instant,
) -> String {
    format!(
        "film debut (relache {} ms, pret {} ms, decompte-premiere {} ms)",
        ms(premiere.saturating_duration_since(relache)),
        ms(pret.saturating_duration_since(relache)),
        ms(premiere.saturating_duration_since(fin_decompte))
    )
}

/// `film fin <cause> duree <ms> ms images <n> sautees <k>`.
pub fn ligne_fin(fin: Fin, duree: Duration, images: u64, sautees: u64) -> String {
    format!(
        "film fin {} duree {} ms images {images} sautees {sautees}",
        fin.nom(),
        ms(duree)
    )
}

/// `film fichier <video|gif> <octets> octets en <ms> ms`.
pub fn ligne_fichier(sorte: Sorte, octets: u64, en: Duration) -> String {
    format!(
        "film fichier {} {octets} octets en {} ms",
        sorte.nom(),
        ms(en)
    )
}

/// `film arret: fichier ferme en <ms> ms`.
pub fn ligne_arret_ferme(en: Duration) -> String {
    format!("film arret: fichier ferme en {} ms", ms(en))
}

/// `chrono film cadence <i/s> prise-mediane <ms> ecriture-mediane <ms> file-max <n>
/// arret-fichier <ms>` (une médiane inconnue : `-`).
pub fn ligne_chrono(
    images: u64,
    duree: Duration,
    prise: Option<f64>,
    ecriture: Option<f64>,
    file: usize,
    arret_fichier: Duration,
) -> String {
    let cadence = if duree.is_zero() {
        0.0
    } else {
        images as f64 / duree.as_secs_f64()
    };
    let mediane = |m: Option<f64>| m.map_or("-".to_string(), |v| format!("{v:.1}"));
    format!(
        "chrono film cadence {cadence:.2} prise-mediane {} ecriture-mediane {} file-max {file} arret-fichier {}",
        mediane(prise),
        mediane(ecriture),
        ms(arret_fichier)
    )
}

/// `film impossible: <cause>`.
pub fn ligne_impossible(cause: &str) -> String {
    format!("film impossible: {cause}")
}

/// `sortie film copie <faite|non demandee|echec> fichier <fait|non demande|echec> en <ms>
/// ms` : aucun chemin.
pub fn ligne_sortie(bilan: &BilanFilm, en: Duration) -> String {
    let copie = match bilan.copie {
        Etape::Faite => "faite",
        Etape::NonDemandee => "non demandee",
        Etape::Echec(_) => "echec",
    };
    let fichier = match bilan.enregistrement {
        EtapeFichier::Fait(_) => "fait",
        EtapeFichier::NonDemande => "non demande",
        EtapeFichier::Echec { .. } => "echec",
    };
    format!(
        "sortie film copie {copie} fichier {fichier} en {} ms",
        ms(en)
    )
}

/// `films nettoyes: <n> (<octets> octets)`.
pub fn ligne_nettoyes(nombre: usize, octets: u64) -> String {
    format!("films nettoyes: {nombre} ({octets} octets)")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ecran(x: i32, y: i32, l: i32, h: i32, echelle: f64) -> EcranInfo {
        let rect = Rect {
            x,
            y,
            largeur: l,
            hauteur: h,
        };
        EcranInfo {
            rect,
            zone_utile: rect,
            echelle,
            principal: x == 0 && y == 0,
        }
    }

    fn rect(x: i32, y: i32, l: i32, h: i32) -> Rect {
        Rect {
            x,
            y,
            largeur: l,
            hauteur: h,
        }
    }

    /// La recherche du codeur n'est lancée qu'avec le droit de la vidéo, et seulement une
    /// fois le film construit (§6, F32).
    #[test]
    fn recherche_du_codeur_avec_le_droit_seulement() {
        assert!(recherche_selon(true, true));
        assert!(!recherche_selon(true, false));
        assert!(!recherche_selon(false, true));
        assert!(!recherche_selon(false, false));
        assert_eq!(recherche_voulue(true), CONSTRUIT);
        assert!(!recherche_voulue(false));
    }

    #[test]
    fn sortes_et_modes() {
        for s in [Sorte::Video, Sorte::Gif] {
            assert_eq!(Sorte::depuis_mode(s.mode()), Some(s));
        }
        assert_eq!(Sorte::Video.nom(), "video");
        assert_eq!(Sorte::Gif.nom(), "gif");
        assert_eq!(Sorte::Video.sortie().extension(), "mp4");
        assert_eq!(Sorte::Gif.sortie().extension(), "gif");
        assert_eq!(Sorte::Video.cadence(), 30);
        assert_eq!(Sorte::Gif.cadence(), 15);
        assert_eq!(Sorte::Video.duree_max(), Duration::from_secs(3600));
        assert_eq!(Sorte::Gif.duree_max(), Duration::from_secs(60));
        for m in Mode::TOUS {
            assert_eq!(Sorte::depuis_mode(m).is_some(), m.film(), "{m:?}");
        }
        assert_eq!(Cours::nouveau(Sorte::Gif, 3).phase(), Phase::Attente);
    }

    /// Choix B1 : seul le raccourci du film en cours l'arrête.
    #[test]
    fn arret_par_le_raccourci_du_film() {
        assert_eq!(
            arret_par_raccourci(Sorte::Video, Mode::Video),
            ARRET_PAR_RACCOURCI
        );
        assert_eq!(
            arret_par_raccourci(Sorte::Gif, Mode::Gif),
            ARRET_PAR_RACCOURCI
        );
        assert!(!arret_par_raccourci(Sorte::Video, Mode::Gif));
        assert!(!arret_par_raccourci(Sorte::Gif, Mode::Video));
        for m in [Mode::Zone, Mode::PleinEcran, Mode::Texte, Mode::Defilante] {
            assert!(!arret_par_raccourci(Sorte::Video, m), "{m:?}");
        }
    }

    /// Choix U4 : un film n'entre jamais dans « Récents », et va toujours dans le dossier.
    #[test]
    fn demande_d_un_film() {
        let base = sortie::Demande {
            copier: true,
            enregistrer: false,
            dossier_enregistrement: std::path::PathBuf::from("/i/BreezyShot"),
            dossier_affiche: "Images › BreezyShot".into(),
            dossier_recents: std::path::PathBuf::from("/l/Recents"),
            ranger_recents: true,
            modele_nom: "BreezyShot {date} à {heure}".into(),
            maintenant: chrono::Local::now(),
            transparence: true,
        };
        let (d, deplacer) = demande_film(&base);
        assert!(!d.ranger_recents);
        assert!(!d.transparence);
        assert!(d.copier);
        assert_eq!(d.enregistrer, TOUJOURS_DANS_LE_DOSSIER);
        assert_eq!(deplacer, TOUJOURS_DANS_LE_DOSSIER);
        assert_eq!(d.dossier_enregistrement, base.dossier_enregistrement);
        // « Enregistrer automatiquement » actif : enregistré dans tous les cas.
        let (d, _) = demande_film(&sortie::Demande {
            enregistrer: true,
            ..base
        });
        assert!(d.enregistrer);
    }

    /// Le filet de la page de sélection ne rend pas la main avant le filet du cœur (F5).
    #[test]
    fn filet_de_la_selection_apres_la_borne_de_la_page() {
        let page = include_str!("../../../interface/selection/selection.js");
        let ligne = format!(
            "const FILET_FILM_MS = FILET_MS + {};",
            DELAI_PAGE.as_millis()
        );
        assert!(page.contains(&ligne), "{ligne}");
    }

    #[test]
    fn marge_du_disque() {
        assert_eq!(marge_disque(0), PLACE_MIN);
        assert_eq!(marge_disque(1_000_000), PLACE_MIN);
        // 30 Mo/s : dix secondes, 300 Mo, plus que 200 Mo.
        assert_eq!(marge_disque(30_000_000), 300_000_000);
        assert_eq!(marge_disque(u64::MAX), u64::MAX);
    }

    #[test]
    fn decompte_et_debit() {
        assert_eq!(decompte_s(0), 3);
        assert_eq!(decompte_s(5), 5);
        assert_eq!(decompte_s(10), 10);
        assert_eq!(debit(600, 450), 1_080_000);
        assert_eq!(debit(1920, 1080), 8_294_400);
        assert_eq!(debit(64, 64), 1_000_000);
        assert_eq!(debit(4096, 2304), 16_000_000);
    }

    #[test]
    fn file_bornee() {
        // 600 × 450 : 1 Mo par image, quatre images.
        assert_eq!(file_max(600 * 450 * 4), 4);
        // 4096 × 2304 : 37,7 Mo, deux images (jamais moins).
        assert_eq!(file_max(4096 * 2304 * 4), 2);
        // 2560 × 1440 : 14,7 Mo, quatre images (58,9 Mo).
        assert_eq!(file_max(2560 * 1440 * 4), 4);
        // 3000 × 2000 : 24 Mo, deux images (48 Mo).
        assert_eq!(file_max(3000 * 2000 * 4), 2);
        assert_eq!(file_max(0), 4);
    }

    #[test]
    fn textes_de_duree() {
        assert_eq!(texte_duree(Duration::ZERO), "0:00");
        assert_eq!(texte_duree(Duration::from_millis(7_900)), "0:07");
        assert_eq!(texte_duree(Duration::from_secs(12 * 60 + 5)), "12:05");
        assert_eq!(texte_duree(Duration::from_secs(3600)), "60:00");
        assert_eq!(texte_vignette(Sorte::Video, Duration::from_secs(7)), "0:07");
        assert_eq!(
            texte_vignette(Sorte::Gif, Duration::from_secs(7)),
            "GIF · 0:07"
        );
        // Jamais moins d'une seconde.
        assert_eq!(
            texte_vignette(Sorte::Video, Duration::from_millis(300)),
            "0:01"
        );
        assert_eq!(texte_vignette(Sorte::Gif, Duration::ZERO), "GIF · 0:01");
        assert_eq!(
            texte_vignette(Sorte::Gif, Duration::from_secs(60)),
            "GIF · 1:00"
        );
    }

    // ---------- Cadrage (F4) ----------

    #[test]
    fn cadrer_une_zone_ordinaire() {
        let e = ecran(0, 0, 1024, 768, 1.0);
        let z = rect(100, 50, 601, 451);
        let Cadrage::Bon(c) = cadrer(z, &e, Sorte::Video, mp4::taille_acceptee) else {
            panic!("refusee");
        };
        // Vidéo : côtés pairs, la dernière colonne et la dernière ligne ne sont pas filmées.
        assert_eq!(c.image, (600, 450));
        assert_eq!(c.zone, rect(100, 50, 600, 450));
        assert!(!c.agrandie && !c.reduite);
        // GIF : la zone entière.
        let Cadrage::Bon(c) = cadrer(z, &e, Sorte::Gif, mp4::taille_acceptee) else {
            panic!("refusee");
        };
        assert_eq!(c.image, (601, 451));
        assert_eq!(c.zone, z);
        assert_eq!(
            ligne_zone(Sorte::Gif, z, &c),
            "film gif zone 100,50 601x451 agrandie non image 601x451"
        );
    }

    #[test]
    fn cadrer_agrandit_les_petites_zones() {
        // 80 × 60 à 100 % : 120 × 120 autour de son centre.
        let e = ecran(0, 0, 1024, 768, 1.0);
        let z = rect(500, 400, 80, 60);
        let Cadrage::Bon(c) = cadrer(z, &e, Sorte::Video, mp4::taille_acceptee) else {
            panic!("refusee");
        };
        assert!(c.agrandie);
        assert_eq!(c.image, (120, 120));
        assert_eq!(c.zone, rect(480, 370, 120, 120));
        assert_eq!(
            ligne_zone(Sorte::Video, z, &c),
            "film video zone 500,400 80x60 agrandie oui image 120x120"
        );
        // À 125 % : 150 ; à 150 % : 180 ; à 200 % : 240 ; collée au bord, sans sortir.
        for (s, cote) in [(1.25, 150), (1.5, 180), (2.0, 240)] {
            let e = ecran(-1920, 0, 1920, 1080, s);
            let z = rect(-1920, 1070, 10, 10);
            let Cadrage::Bon(c) = cadrer(z, &e, Sorte::Gif, mp4::taille_acceptee) else {
                panic!("refusee");
            };
            assert_eq!(c.image, (cote, cote), "{s}");
            assert_eq!(c.zone.x, -1920);
            assert_eq!(c.zone.bas(), 1080);
        }
    }

    #[test]
    fn cadrer_reduit_puis_pair() {
        // 4098 × 2306 est refusée (plus de 4096) : réduite de moitié, 2049 × 1153 ramenés
        // à 2048 × 1152 ; la zone filmée en est le double.
        let e = ecran(0, 0, 5120, 2880, 2.0);
        let z = rect(10, 20, 4099, 2307);
        let Cadrage::Bon(c) = cadrer(z, &e, Sorte::Video, mp4::taille_acceptee) else {
            panic!("refusee");
        };
        assert!(c.reduite);
        assert_eq!(c.image, (2048, 1152));
        assert_eq!(c.zone, rect(10, 20, 4096, 2304));
        assert!(mp4::taille_acceptee(c.image.0, c.image.1));
        // Tout l'écran 5K : 2560 × 1440.
        let Cadrage::Bon(c) = cadrer(e.rect, &e, Sorte::Video, mp4::taille_acceptee) else {
            panic!("refusee");
        };
        assert_eq!((c.image, c.reduite), ((2560, 1440), true));
        // Un GIF n'est jamais réduit.
        let Cadrage::Bon(c) = cadrer(z, &e, Sorte::Gif, mp4::taille_acceptee) else {
            panic!("refusee");
        };
        assert_eq!((c.image, c.reduite), ((4099, 2307), false));
    }

    #[test]
    fn cadrer_refuse_et_vide() {
        let e = ecran(0, 0, 1024, 768, 1.0);
        let z = rect(0, 0, 600, 450);
        // Un codeur qui refuse tout : refusée, même réduite.
        assert_eq!(cadrer(z, &e, Sorte::Video, |_, _| false), Cadrage::Refuse);
        // Une réduite essayée une seule fois.
        let essais = std::cell::Cell::new(0);
        let _ = cadrer(z, &e, Sorte::Video, |_, _| {
            essais.set(essais.get() + 1);
            false
        });
        assert_eq!(essais.get(), 2);
        // Le GIF ne demande rien au codeur vidéo.
        assert!(matches!(
            cadrer(z, &e, Sorte::Gif, |_, _| false),
            Cadrage::Bon(_)
        ));
        assert_eq!(
            cadrer(rect(0, 0, 0, 10), &e, Sorte::Video, mp4::taille_acceptee),
            Cadrage::Vide
        );
        // Hors de l'écran : agrandie dans l'écran, mais rien n'en reste au départ.
        assert_eq!(
            cadrer(rect(10, 10, -5, 4), &e, Sorte::Gif, mp4::taille_acceptee),
            Cadrage::Vide
        );
    }

    // ---------- Placement de la pilule (§10) ----------

    #[test]
    fn placer_sous_au_dessus_dedans() {
        for s in [1.0, 1.25, 1.5, 2.0] {
            let e = ecran(0, 0, 2560, 1440, s);
            let p = taille_pilule((143.0, 36.0), &e);
            let ecart = (10.0 * s).round() as i32;
            let marge = (6.0 * s).round() as i32;
            let (pl, ph) = (p.0.ceil() as i32, p.1.ceil() as i32);
            // Sous la zone, bord droit aligné.
            let z = rect(400, 300, 800, 400);
            assert_eq!(placer(z, &e, p), (1200 - pl, 700 + ecart), "{s}");
            // Zone collée au bas : au-dessus.
            let z = rect(400, 1440 - 400, 800, 400);
            assert_eq!(placer(z, &e, p), (1200 - pl, 1040 - ecart - ph), "{s}");
            // Tout l'écran : dedans, en bas à droite, à 10 des bords.
            let z = e.rect;
            assert_eq!(
                placer(z, &e, p),
                (2560 - ecart - pl, 1440 - ecart - ph),
                "{s}"
            );
            // Zone collée à gauche, plus étroite que la pilule : jamais à moins de 6 du bord.
            let z = rect(0, 300, 40, 400);
            assert_eq!(placer(z, &e, p).0, marge, "{s}");
        }
    }

    #[test]
    fn placer_sur_un_ecran_negatif_et_la_barre_a_gauche() {
        // Un écran à gauche du principal, à 150 %, barre des tâches à gauche (60 px).
        let mut e = ecran(-1920, -200, 1920, 1080, 1.5);
        e.zone_utile = rect(-1860, -200, 1860, 1080);
        let p = taille_pilule((180.0, 36.0), &e);
        assert_eq!(p, (270.0, 54.0));
        let z = rect(-1920, 0, 200, 300);
        let (x, y) = placer(z, &e, p);
        // À 9 px (6 logiques) de la barre, sous la zone (15 px).
        assert_eq!((x, y), (-1860 + 9, 300 + 15));
        // Une zone qui couvre l'écran : dedans, au-dessus de la barre du bas absente.
        let (x, y) = placer(e.rect, &e, p);
        assert_eq!((x, y), (-15 - 270, -200 + 1080 - 15 - 54));
    }

    // ---------- Suite du film (§4.2, F7 à F11, F27, F28, F34) ----------

    fn pret_a_compter(sorte: Sorte) -> Cours {
        let mut c = Cours::nouveau(sorte, 3);
        c.micro_present(false);
        assert_eq!(
            suite(&mut c, Vu::PageMontree),
            vec![Geste::Decompter(3), Geste::Etat]
        );
        c
    }

    fn en_marche(sorte: Sorte) -> Cours {
        let mut c = pret_a_compter(sorte);
        assert!(suite(&mut c, Vu::Pret).is_empty());
        assert_eq!(
            suite(&mut c, Vu::DecompteFini),
            vec![Geste::Demarrer, Geste::Etat]
        );
        assert_eq!(c.phase(), Phase::Marche);
        c
    }

    #[test]
    fn attente_puis_decompte_puis_marche() {
        let mut c = Cours::nouveau(Sorte::Video, 5);
        assert!(!c.micro_connu());
        // Rien avant la page montrée, sauf les annulations.
        assert!(suite(&mut c, Vu::DecompteFini).is_empty());
        assert!(suite(&mut c, Vu::Pause).is_empty());
        assert!(suite(&mut c, Vu::Arreter).is_empty());
        assert!(suite(&mut c, Vu::Croix).is_empty());
        c.micro_present(true);
        assert!(c.micro_connu());
        assert_eq!(c.micro(), Some(false));
        assert_eq!(
            suite(&mut c, Vu::PageMontree),
            vec![Geste::Decompter(5), Geste::Etat]
        );
        // Une page montrée deux fois ne recompte pas.
        assert!(suite(&mut c, Vu::PageMontree).is_empty());
        // Prête avant la fin du décompte : le départ attend zéro.
        assert!(suite(&mut c, Vu::Pret).is_empty());
        assert_eq!(c.phase(), Phase::Decompte);
        assert_eq!(
            suite(&mut c, Vu::DecompteFini),
            vec![Geste::Demarrer, Geste::Etat]
        );
        assert_eq!(c.phase(), Phase::Marche);
        assert_eq!(
            suite(&mut c, Vu::Seconde(Duration::from_secs(1))),
            vec![Geste::Etat]
        );
        assert_eq!(charge_etat(4, &c).duree, "0:01");
    }

    #[test]
    fn pret_en_retard() {
        let mut c = pret_a_compter(Sorte::Gif);
        // Zéro sans préparation : le disque reste sur « 1 », rien ne part.
        assert!(suite(&mut c, Vu::DecompteFini).is_empty());
        assert_eq!(c.phase(), Phase::Decompte);
        // Prête ensuite : départ.
        assert_eq!(suite(&mut c, Vu::Pret), vec![Geste::Demarrer, Geste::Etat]);
        // Jamais prête : impossible.
        let mut c = pret_a_compter(Sorte::Gif);
        assert!(suite(&mut c, Vu::DecompteFini).is_empty());
        assert_eq!(
            suite(&mut c, Vu::PretEchu),
            vec![Geste::Impossible("preparation")]
        );
        assert!(c.fini());
        assert!(suite(&mut c, Vu::Pret).is_empty());
        // Un `PretEchu` venu après le départ ne compte pas.
        let mut c = en_marche(Sorte::Gif);
        assert!(suite(&mut c, Vu::PretEchu).is_empty());
    }

    #[test]
    fn annulations_pendant_l_attente_et_le_decompte() {
        let attente = || {
            let mut c = Cours::nouveau(Sorte::Video, 3);
            c.micro_present(false);
            c
        };
        for (vu, cause) in [
            (Vu::Echap, Annulation::Echap),
            (Vu::Ecrans, Annulation::Ecrans),
            (Vu::Session, Annulation::Session),
            (Vu::Veille, Annulation::Veille),
            (Vu::Arret, Annulation::Arret),
            (Vu::Bureau(Changement::Ecrans), Annulation::Ecrans),
            (
                Vu::Bureau(Changement::SessionVerrouillee),
                Annulation::Session,
            ),
        ] {
            let mut c = attente();
            assert_eq!(suite(&mut c, vu), vec![Geste::Annuler(cause)], "{vu:?}");
            assert!(c.fini());
        }
        // En attente, ni la croix ni « Arrêter » n'existent encore.
        assert!(suite(&mut attente(), Vu::Croix).is_empty());
        for (vu, cause) in [
            (Vu::Echap, Annulation::Echap),
            (Vu::Croix, Annulation::Croix),
            (Vu::Arreter, Annulation::Arreter),
            (Vu::Ecrans, Annulation::Ecrans),
            (Vu::Session, Annulation::Session),
            (Vu::Veille, Annulation::Veille),
            (Vu::Arret, Annulation::Arret),
        ] {
            let mut c = pret_a_compter(Sorte::Video);
            assert_eq!(suite(&mut c, vu), vec![Geste::Annuler(cause)], "{vu:?}");
        }
        // Le bureau de l'écran figé ne compte qu'en attente.
        let mut c = pret_a_compter(Sorte::Video);
        assert!(suite(&mut c, Vu::Bureau(Changement::Ecrans)).is_empty());
        assert_eq!(
            ligne_annule(Phase::Decompte, Annulation::Croix),
            "film annule decompte (croix)"
        );
        assert_eq!(
            ligne_annule(Phase::Attente, Annulation::Arret),
            "film annule attente (arret)"
        );
    }

    #[test]
    fn pause_et_reprise() {
        // Pendant le décompte, « Pause » est inactif.
        let mut c = pret_a_compter(Sorte::Video);
        assert!(suite(&mut c, Vu::Pause).is_empty());
        assert!(!charge_etat(1, &c).pause.actif);
        let mut c = en_marche(Sorte::Video);
        assert!(charge_etat(1, &c).pause.actif);
        assert_eq!(
            suite(&mut c, Vu::Pause),
            vec![Geste::Suspendre, Geste::Etat]
        );
        assert_eq!(c.phase(), Phase::Pause);
        let e = charge_etat(1, &c);
        assert_eq!((e.phase, e.pause.libelle.as_str()), ("pause", "Reprendre"));
        // Les secondes ne bougent pas pendant la pause.
        assert!(suite(&mut c, Vu::Seconde(Duration::from_secs(9))).is_empty());
        assert_eq!(
            suite(&mut c, Vu::Pause),
            vec![Geste::Reprendre, Geste::Etat]
        );
        assert_eq!(c.phase(), Phase::Marche);
        // Arrêter pendant une pause : le film est gardé.
        suite(&mut c, Vu::Pause);
        assert_eq!(suite(&mut c, Vu::Arreter), vec![Geste::Finir(Fin::Bouton)]);
    }

    #[test]
    fn micro_ouvert_coupe_et_perdu() {
        // Un GIF n'a jamais de micro.
        let mut c = Cours::nouveau(Sorte::Gif, 3);
        assert!(c.micro_connu());
        c.micro_present(true);
        assert_eq!(c.micro(), None);
        assert!(charge_etat(1, &c).micro.is_none());
        // Une vidéo sur un PC sans micro non plus.
        let mut c = en_marche(Sorte::Video);
        assert!(suite(&mut c, Vu::Micro).is_empty());
        assert!(suite(&mut c, Vu::MicroPerdu).is_empty());
        // Avec un micro : coupé au départ, ouvert puis coupé.
        let mut c = Cours::nouveau(Sorte::Video, 3);
        c.micro_present(true);
        c.micro_present(false);
        assert_eq!(c.micro(), Some(false));
        suite(&mut c, Vu::PageMontree);
        assert_eq!(
            suite(&mut c, Vu::Micro),
            vec![Geste::OuvrirMicro, Geste::Etat]
        );
        let e = charge_etat(1, &c).micro.unwrap();
        assert_eq!((e.libelle.as_str(), e.ouvert), ("Micro activé", true));
        // Perdu pendant le décompte : rien au bandeau (il effacerait le disque) ; dit au
        // départ.
        assert_eq!(suite(&mut c, Vu::MicroPerdu), vec![Geste::Etat]);
        assert_eq!(c.micro(), Some(false));
        suite(&mut c, Vu::Pret);
        assert_eq!(
            suite(&mut c, Vu::DecompteFini),
            vec![
                Geste::Demarrer,
                Geste::Etat,
                Geste::Dire(TITRE_MICRO_INDISPONIBLE)
            ]
        );
        // Rouvert, puis perdu en marche : dit tout de suite, l'enregistrement continue.
        assert_eq!(
            suite(&mut c, Vu::Micro),
            vec![Geste::OuvrirMicro, Geste::Etat]
        );
        assert_eq!(
            suite(&mut c, Vu::MicroPerdu),
            vec![Geste::Dire(TITRE_MICRO_INDISPONIBLE), Geste::Etat]
        );
        assert_eq!(c.phase(), Phase::Marche);
        assert_eq!(
            suite(&mut c, Vu::Micro),
            vec![Geste::OuvrirMicro, Geste::Etat]
        );
        assert_eq!(
            suite(&mut c, Vu::Micro),
            vec![Geste::CouperMicro, Geste::Etat]
        );
        assert_eq!(charge_etat(1, &c).micro.unwrap().libelle, "Micro");
    }

    #[test]
    fn source_perdue_puis_reprise_puis_finie() {
        let mut c = en_marche(Sorte::Video);
        // La perte et la reprise ne changent rien au cours (le fil `film` les écrit).
        assert!(suite(&mut c, Vu::SourcePerdue).is_empty());
        assert!(suite(&mut c, Vu::SourceReprise).is_empty());
        assert_eq!(c.phase(), Phase::Marche);
        assert_eq!(
            suite(&mut c, Vu::SourceFinie),
            vec![Geste::Finir(Fin::Source)]
        );
        // Avant les images : impossible.
        let mut c = pret_a_compter(Sorte::Video);
        assert_eq!(
            suite(&mut c, Vu::SourceFinie),
            vec![Geste::Impossible("source")]
        );
        let mut c = pret_a_compter(Sorte::Video);
        assert_eq!(suite(&mut c, Vu::Disque), vec![Geste::Impossible("disque")]);
        let mut c = Cours::nouveau(Sorte::Gif, 3);
        assert_eq!(
            suite(&mut c, Vu::Echec("codeur")),
            vec![Geste::Impossible("codeur")]
        );
    }

    #[test]
    fn chaque_fin_et_une_seule() {
        for (vu, fin) in [
            (Vu::Arreter, Fin::Bouton),
            (Vu::Limite, Fin::Limite),
            (Vu::Disque, Fin::Disque),
            (Vu::Ecrans, Fin::Ecrans),
            (Vu::Session, Fin::Session),
            (Vu::Veille, Fin::Veille),
            (Vu::Arret, Fin::Arret),
            (Vu::SourceFinie, Fin::Source),
        ] {
            for pause in [false, true] {
                let mut c = en_marche(Sorte::Video);
                if pause {
                    suite(&mut c, Vu::Pause);
                }
                assert_eq!(suite(&mut c, vu), vec![Geste::Finir(fin)], "{vu:?}");
                assert!(c.fini());
                // La première fin gagne : rien d'autre ne compte.
                for autre in [
                    Vu::Arreter,
                    Vu::Limite,
                    Vu::Ecrans,
                    Vu::Session,
                    Vu::Veille,
                    Vu::Arret,
                    Vu::Echap,
                    Vu::Pause,
                    Vu::Micro,
                    Vu::Echec("fil"),
                    Vu::Seconde(Duration::from_secs(2)),
                ] {
                    assert!(suite(&mut c, autre).is_empty(), "{vu:?} puis {autre:?}");
                }
            }
        }
        // Échap est rendu au départ : en marche, il ne fait rien.
        let mut c = en_marche(Sorte::Gif);
        assert!(suite(&mut c, Vu::Echap).is_empty());
        assert!(suite(&mut c, Vu::Croix).is_empty());
        // La limite ne joue pas avant le départ.
        let mut c = pret_a_compter(Sorte::Gif);
        assert!(suite(&mut c, Vu::Limite).is_empty());
        assert_eq!(Fin::Veille.nom(), "veille");
    }

    #[test]
    fn etat_de_la_pilule() {
        let c = pret_a_compter(Sorte::Gif);
        let e = charge_etat(9, &c);
        assert_eq!(e.id, 9);
        assert_eq!(e.phase, "decompte");
        assert_eq!(e.duree, "0:00");
        assert_eq!(e.pause.libelle, "Pause");
        assert_eq!(e.arreter, "Arrêter");
        assert_eq!(e.groupe, "Enregistrement");
        assert!(e.micro.is_none());
        let c = en_marche(Sorte::Gif);
        assert_eq!(charge_etat(9, &c).phase, "marche");
    }

    #[test]
    fn mediane_en_memoire_constante() {
        let mut m = Mediane::default();
        assert_eq!(m.valeur(), None);
        for ms in [1, 2, 3, 50, 8] {
            m.ajouter(Duration::from_millis(ms));
        }
        assert_eq!(m.valeur(), Some(3.0));
        m.ajouter(Duration::from_millis(7_400));
        m.ajouter(Duration::from_micros(7_450));
        assert_eq!(m.valeur(), Some(7.4));
        let mut m = Mediane::default();
        m.ajouter(Duration::from_secs(1));
        assert_eq!(m.valeur(), Some(100.0));
    }

    #[test]
    fn lignes_du_journal() {
        let t = Instant::now();
        let ms = Duration::from_millis;
        assert_eq!(
            ligne_debut(t, t + ms(40), t + ms(3_200), t + ms(3_230)),
            "film debut (relache 3230 ms, pret 40 ms, decompte-premiere 30 ms)"
        );
        assert_eq!(ligne_decompte(3), "film decompte 3 s");
        assert_eq!(ligne_page_prete(ms(120)), "film page prete en 120 ms");
        assert_eq!(
            ligne_source("bitblt", Some("acces perdu")),
            "film source bitblt (acces perdu)"
        );
        assert_eq!(ligne_source("duplication", None), "film source duplication");
        assert_eq!(
            ligne_reprise(ms(3_050)),
            "film source reprise apres 3050 ms"
        );
        assert_eq!(
            ligne_codeur("logiciel", (600, 450), debit(600, 450)),
            "film codeur logiciel 600x450 1080 kbit/s"
        );
        assert_eq!(ligne_son(false), "film son non");
        assert_eq!(ligne_micro("ouvert"), "film micro ouvert");
        assert_eq!(
            ligne_fin(Fin::Bouton, ms(6_012), 180, 0),
            "film fin bouton duree 6012 ms images 180 sautees 0"
        );
        assert_eq!(
            ligne_fichier(Sorte::Video, 120_000, ms(25)),
            "film fichier video 120000 octets en 25 ms"
        );
        assert_eq!(
            ligne_arret_ferme(ms(31)),
            "film arret: fichier ferme en 31 ms"
        );
        assert_eq!(
            ligne_chrono(180, ms(6_000), Some(7.42), Some(1.6), 3, ms(30)),
            "chrono film cadence 30.00 prise-mediane 7.4 ecriture-mediane 1.6 file-max 3 arret-fichier 30"
        );
        assert_eq!(
            ligne_chrono(0, Duration::ZERO, None, None, 0, ms(0)),
            "chrono film cadence 0.00 prise-mediane - ecriture-mediane - file-max 0 arret-fichier 0"
        );
        assert_eq!(ligne_impossible("page"), "film impossible: page");
        let bilan = BilanFilm {
            copie: Etape::Faite,
            enregistrement: EtapeFichier::Fait("/i/a.mp4".into()),
            lien: None,
        };
        assert_eq!(
            ligne_sortie(&bilan, ms(812)),
            "sortie film copie faite fichier fait en 812 ms"
        );
        let bilan = BilanFilm {
            copie: Etape::NonDemandee,
            enregistrement: EtapeFichier::Echec {
                dossier: "/i".into(),
                cause: "plein".into(),
            },
            lien: None,
        };
        assert_eq!(
            ligne_sortie(&bilan, ms(3)),
            "sortie film copie non demandee fichier echec en 3 ms"
        );
        assert_eq!(ligne_nettoyes(2, 4096), "films nettoyes: 2 (4096 octets)");
    }
}
