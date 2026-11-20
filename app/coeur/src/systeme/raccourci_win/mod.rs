//! Interception optionnelle du raccourci `Win + Maj + S` sous Windows 11.
//!
//! Windows réserve `Win + Maj + S` à l'Outil Capture dans l'Explorateur : `RegisterHotKey`
//! échoue toujours. Un crochet clavier bas niveau (`WH_KEYBOARD_LL`) posé seulement quand
//! l'utilisateur choisit BreezyShot pour `Win + Maj + S` avale la touche `S` avant
//! l'Explorateur, sans toucher au registre ni redémarrer l'Explorateur. Quand le réglage
//! revient sur « Outil Capture de Windows », le crochet est aussitôt retiré.
//!
//! Limite du système (UIPI) : sans droits administrateur, Windows ne transmet pas les touches
//! tapées pendant qu'une fenêtre élevée est au premier plan.

#[cfg(windows)]
#[path = "windows.rs"]
mod plateforme;

#[cfg(not(windows))]
#[path = "autre.rs"]
mod plateforme;

/// Code virtuel de la touche `S` sous Windows (`VK_S`).
pub const VK_S: u32 = 0x53;

/// État des modificateurs au moment d'un appui sur `S`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModificateursWin {
    pub win: bool,
    pub maj: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl ModificateursWin {
    /// Vrai si `Win` et `Maj` sont tenus ensemble, sans `Ctrl` ni `Alt`.
    pub fn est_win_maj_seul(self) -> bool {
        self.win && self.maj && !self.ctrl && !self.alt
    }
}

/// Ce que fait le crochet pour un événement clavier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Laisser passer l'événement au système.
    Passer,
    /// Avaler l'événement sans déclencher (répétition ou relâchement d'un `S` déjà pris).
    Avaler,
    /// Avaler l'événement et déclencher l'action BreezyShot.
    Declencher,
}

/// Décision pure du crochet `Win + Maj + S` : évite les répétitions automatiques tant que
/// `S` reste enfoncée, avale le relâchement assorti, et se remet à zéro dès qu'un appui sur
/// `S` arrive sans `Win + Maj` (un relâchement perdu derrière un écran verrouillé ou UAC ne
/// bloque jamais la frappe normale).
#[derive(Debug, Default)]
pub struct FiltreWinMajS {
    s_avale: bool,
}

impl FiltreWinMajS {
    pub const fn nouveau() -> Self {
        Self { s_avale: false }
    }

    pub fn traiter(&mut self, appui: bool, touche: u32, mods: ModificateursWin) -> Decision {
        if touche != VK_S {
            return Decision::Passer;
        }
        if appui {
            if mods.est_win_maj_seul() {
                if self.s_avale {
                    Decision::Avaler
                } else {
                    self.s_avale = true;
                    Decision::Declencher
                }
            } else {
                self.s_avale = false;
                Decision::Passer
            }
        } else if self.s_avale {
            self.s_avale = false;
            Decision::Avaler
        } else {
            Decision::Passer
        }
    }
}

pub type Rappel = Box<dyn Fn() + Send + 'static>;

/// Garde de l'écoute `Win + Maj + S` : le crochet est retiré à la destruction.
#[derive(Debug)]
pub struct Ecoute(#[allow(dead_code)] plateforme::Ecoute);

/// Vrai sur les systèmes où `Win + Maj + S` peut être choisi (Windows).
pub fn disponible() -> bool {
    plateforme::disponible()
}

/// Pose le crochet `Win + Maj + S` et appelle `rappel` à chaque déclenchement.
/// Le crochet reste actif tant que l'`Ecoute` rendue vit.
pub fn ecouter<F>(rappel: F) -> Result<Ecoute, String>
where
    F: Fn() + Send + 'static,
{
    plateforme::ecouter(Box::new(rappel)).map(Ecoute)
}

#[cfg(test)]
mod tests {
    use super::*;

    const AUCUN: ModificateursWin = ModificateursWin {
        win: false,
        maj: false,
        ctrl: false,
        alt: false,
    };
    const WIN_MAJ: ModificateursWin = ModificateursWin {
        win: true,
        maj: true,
        ctrl: false,
        alt: false,
    };

    #[test]
    fn declenche_une_fois_puis_avale_repetitions_et_relachement() {
        let mut f = FiltreWinMajS::nouveau();
        assert_eq!(f.traiter(true, VK_S, WIN_MAJ), Decision::Declencher);
        assert_eq!(f.traiter(true, VK_S, WIN_MAJ), Decision::Avaler);
        assert_eq!(f.traiter(true, VK_S, WIN_MAJ), Decision::Avaler);
        assert_eq!(f.traiter(false, VK_S, WIN_MAJ), Decision::Avaler);
        // Un second appui après relâchement déclenche de nouveau.
        assert_eq!(f.traiter(true, VK_S, WIN_MAJ), Decision::Declencher);
        // Même si Win a déjà été relâchée avant S, le relâchement de S est avalé.
        assert_eq!(f.traiter(false, VK_S, AUCUN), Decision::Avaler);
        // Un relâchement ordinaire ensuite passe.
        assert_eq!(f.traiter(false, VK_S, AUCUN), Decision::Passer);
    }

    #[test]
    fn laisse_passer_les_autres_touches_et_combinaisons() {
        let mut f = FiltreWinMajS::nouveau();
        assert_eq!(f.traiter(true, 0x41, WIN_MAJ), Decision::Passer);
        assert_eq!(f.traiter(true, VK_S, AUCUN), Decision::Passer);
        assert_eq!(
            f.traiter(true, VK_S, ModificateursWin { maj: true, ..AUCUN }),
            Decision::Passer
        );
        assert_eq!(
            f.traiter(true, VK_S, ModificateursWin { win: true, ..AUCUN }),
            Decision::Passer
        );
        assert_eq!(
            f.traiter(
                true,
                VK_S,
                ModificateursWin {
                    ctrl: true,
                    ..WIN_MAJ
                }
            ),
            Decision::Passer
        );
        assert_eq!(
            f.traiter(
                true,
                VK_S,
                ModificateursWin {
                    alt: true,
                    ..WIN_MAJ
                }
            ),
            Decision::Passer
        );
    }

    #[test]
    fn relachement_perdu_ne_bloque_pas_la_frappe_normale() {
        let mut f = FiltreWinMajS::nouveau();
        assert_eq!(f.traiter(true, VK_S, WIN_MAJ), Decision::Declencher);
        // Relâchement manqué (écran verrouillé / UAC) : un `s` normal réinitialise l'état.
        assert_eq!(f.traiter(true, VK_S, AUCUN), Decision::Passer);
        assert_eq!(f.traiter(false, VK_S, AUCUN), Decision::Passer);
    }
}
