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
//!
//! Socle : constantes, types et signatures ; les corps sont inertes (chantier F), sauf
//! `recherche_voulue` et les choix ci-dessus.

use std::time::{Duration, Instant};

use super::etats::Mode;
use crate::droits::{self, Fonction};
use crate::sortie::{self, SorteFilm};
use crate::systeme::gel::{Changement, EcranInfo};
use crate::systeme::Rect;

/// L'interrupteur du film : vrai quand vidéo et GIF sont construits (chantiers A à J,
/// ouvert à l'assemblage). Faux, ni « Vidéo » ni « GIF » ne sont des actions construites
/// (ni menu, ni raccourci, ni ligne de Réglages, ni `--capturer`), même avec une licence,
/// et la recherche du codeur n'est jamais lancée : rien de montré qui ne marche pas.
pub const CONSTRUIT: bool = false;

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
/// Deux tours du fil `film` séparés de plus que ce délai : une veille (F34).
pub const SAUT_VEILLE: Duration = Duration::from_secs(5);
/// Durées maximales (choix U3).
pub const DUREE_MAX_VIDEO: Duration = Duration::from_secs(60 * 60);
pub const DUREE_MAX_GIF: Duration = Duration::from_secs(60);
pub const PAUSE_MAX: Duration = Duration::from_secs(30 * 60);
/// Place libre minimale du volume de `<local>` (F28).
pub const PLACE_MIN: u64 = 200 * 1024 * 1024;

/// Nom accessible de la carte d'un film, et annonce sans copie ni enregistrement (F25, F26).
pub const TITRE_CARTE: &str = "Enregistrement terminé";

/// Choix U4 (piste B, provisoire) : un film va toujours dans le dossier d'enregistrement
/// (Images › BreezyShot), quel que soit « Enregistrer automatiquement », et il y est
/// déplacé, pas copié. Faux : la règle de F22 à F24 (le film suit les deux réglages de
/// « Après la capture » et reste sinon dans `<local>/Films`).
pub const TOUJOURS_DANS_LE_DOSSIER: bool = true;

/// Choix B1 (provisoire) : pendant un film, le raccourci qui l'a lancé l'arrête
/// (`Ctrl + Maj + 6` pendant une vidéo). Faux : ce raccourci est ignoré comme les autres.
pub const ARRET_PAR_RACCOURCI: bool = true;

/// La place libre à garder sur le volume de `<local>` pendant un film (F28) : 200 Mo ou
/// dix secondes d'écriture au débit observé, la plus grande des deux. Socle : `PLACE_MIN`.
pub fn marge_disque(octets_par_s: u64) -> u64 {
    let _ = octets_par_s;
    PLACE_MIN
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
}

/// La zone d'un film cadrée (F4) : la zone filmée (pixels réels du bureau), la taille de
/// l'image écrite, et si la zone a été agrandie ou l'image réduite de moitié.
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

/// F4 : la zone portée au côté minimal autour de son centre, sur son écran ; la taille de
/// l'image (paire pour une vidéo) ; réduite de moitié une fois si `acceptee` la refuse.
/// Socle : `Vide`.
pub fn cadrer(
    zone: Rect,
    ecran: &EcranInfo,
    sorte: Sorte,
    acceptee: impl Fn(u32, u32) -> bool,
) -> Cadrage {
    let _ = (zone, ecran, sorte, acceptee(0, 0));
    Cadrage::Vide
}

/// La place de la pilule (§10, pixels réels du bureau) : sous la zone, son bord droit sur
/// celui de la zone ; sinon au-dessus ; sinon dedans, en bas à droite. `pilule` : sa
/// taille, pixels réels. Socle : le coin bas droit de la zone.
pub fn placer(zone: Rect, ecran: &EcranInfo, pilule: (f64, f64)) -> (i32, i32) {
    let _ = (ecran, pilule);
    (
        zone.x.saturating_add(zone.largeur),
        zone.y.saturating_add(zone.hauteur),
    )
}

/// La durée du décompte (F7) : le réglage « Retardateur » s'il n'est pas « Aucun », sinon
/// `DECOMPTE_DEFAUT_S`. Socle : `DECOMPTE_DEFAUT_S`.
pub fn decompte_s(retardateur_s: u32) -> u32 {
    let _ = retardateur_s;
    DECOMPTE_DEFAUT_S
}

/// Le débit moyen visé d'une vidéo (F15), bits par seconde. Socle : 0.
pub fn debit(largeur: u32, hauteur: u32) -> u32 {
    let _ = (largeur, hauteur);
    0
}

/// « 0:07 », « 12:05 », « 60:00 ». Socle : vide.
pub fn texte_duree(d: Duration) -> String {
    let _ = d;
    String::new()
}

/// La durée de la vignette : « 0:07 » pour une vidéo, « GIF · 0:07 » pour un GIF, jamais
/// moins de « 0:01 ». Socle : vide.
pub fn texte_vignette(sorte: Sorte, d: Duration) -> String {
    let _ = (sorte, d);
    String::new()
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

/// Le cours d'un film : sa sorte, sa phase, le micro, la préparation, la fin rendue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cours {
    sorte: Sorte,
    phase: Phase,
}

impl Cours {
    /// Un film de `sorte` qui commence (phase `Attente`).
    pub fn nouveau(sorte: Sorte) -> Cours {
        Cours {
            sorte,
            phase: Phase::Attente,
        }
    }

    pub fn sorte(&self) -> Sorte {
        self.sorte
    }

    pub fn phase(&self) -> Phase {
        self.phase
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
/// Socle : rien.
pub fn suite(cours: &mut Cours, vu: Vu) -> Vec<Geste> {
    let _ = (cours, vu);
    Vec::new()
}

/// La ligne du journal du cadrage : `film <K> zone <x>,<y> <l>x<h> agrandie <oui|non>
/// image <l>x<h>`. Socle : vide. (Les autres lignes du §8 suivent, chantier F.)
pub fn ligne_zone(sorte: Sorte, zone: Rect, cadre: &Cadre) -> String {
    let _ = (sorte, zone, cadre);
    String::new()
}

/// L'instant de départ d'un film, pour les lignes `film debut` (F11). Socle : rien.
pub fn ligne_debut(relache: Instant, pret: Option<Instant>, premiere: Instant) -> String {
    let _ = (relache, pret, premiere);
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        for m in Mode::TOUS {
            assert_eq!(Sorte::depuis_mode(m).is_some(), m.film(), "{m:?}");
        }
        assert_eq!(Cours::nouveau(Sorte::Gif).phase(), Phase::Attente);
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
}
