//! Traduction d'un `Raccourci` vers les types du système de raccourcis globaux.
//!
//! Seul fichier qui connaît la crate des raccourcis. Dans l'app, remplacer la ligne `use`
//! ci-dessous par :
//!     use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};
//! (le plugin 2.4.x réexporte exactement ces types depuis global-hotkey 0.8).
//!
//! Prise en charge vérifiée dans les sources de global-hotkey 0.8.0 :
//! - Windows (`key_to_vk`) : toutes les touches de la notation, ImprEcran = VK_SNAPSHOT.
//! - macOS (`key_to_scancode`) : F21 à F24 absentes (la réservation échoue) ; ImprEcran est
//!   traduite en 0x46, code qu'aucune touche d'un clavier Mac n'envoie (le raccourci
//!   sera accepté mais ne se déclenchera jamais).
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};

use super::notation::{Raccourci, Touche};

fn code(touche: Touche) -> Option<Code> {
    Some(match touche {
        Touche::Lettre(c) => match c {
            'A' => Code::KeyA,
            'B' => Code::KeyB,
            'C' => Code::KeyC,
            'D' => Code::KeyD,
            'E' => Code::KeyE,
            'F' => Code::KeyF,
            'G' => Code::KeyG,
            'H' => Code::KeyH,
            'I' => Code::KeyI,
            'J' => Code::KeyJ,
            'K' => Code::KeyK,
            'L' => Code::KeyL,
            'M' => Code::KeyM,
            'N' => Code::KeyN,
            'O' => Code::KeyO,
            'P' => Code::KeyP,
            'Q' => Code::KeyQ,
            'R' => Code::KeyR,
            'S' => Code::KeyS,
            'T' => Code::KeyT,
            'U' => Code::KeyU,
            'V' => Code::KeyV,
            'W' => Code::KeyW,
            'X' => Code::KeyX,
            'Y' => Code::KeyY,
            'Z' => Code::KeyZ,
            _ => return None,
        },
        Touche::Chiffre(n) => match n {
            0 => Code::Digit0,
            1 => Code::Digit1,
            2 => Code::Digit2,
            3 => Code::Digit3,
            4 => Code::Digit4,
            5 => Code::Digit5,
            6 => Code::Digit6,
            7 => Code::Digit7,
            8 => Code::Digit8,
            9 => Code::Digit9,
            _ => return None,
        },
        Touche::F(n) => match n {
            1 => Code::F1,
            2 => Code::F2,
            3 => Code::F3,
            4 => Code::F4,
            5 => Code::F5,
            6 => Code::F6,
            7 => Code::F7,
            8 => Code::F8,
            9 => Code::F9,
            10 => Code::F10,
            11 => Code::F11,
            12 => Code::F12,
            13 => Code::F13,
            14 => Code::F14,
            15 => Code::F15,
            16 => Code::F16,
            17 => Code::F17,
            18 => Code::F18,
            19 => Code::F19,
            20 => Code::F20,
            21 => Code::F21,
            22 => Code::F22,
            23 => Code::F23,
            24 => Code::F24,
            _ => return None,
        },
        Touche::ImprEcran => Code::PrintScreen,
        Touche::Espace => Code::Space,
        Touche::Entree => Code::Enter,
        Touche::Tab => Code::Tab,
        Touche::Echap => Code::Escape,
        Touche::Suppr => Code::Delete,
        Touche::Inser => Code::Insert,
        Touche::Debut => Code::Home,
        Touche::Fin => Code::End,
        Touche::PagePrec => Code::PageUp,
        Touche::PageSuiv => Code::PageDown,
        Touche::Haut => Code::ArrowUp,
        Touche::Bas => Code::ArrowDown,
        Touche::Gauche => Code::ArrowLeft,
        Touche::Droite => Code::ArrowRight,
    })
}

/// Le raccourci dans les types du système. `None` seulement pour un `Raccourci` construit
/// à la main hors de la notation (lettre accentuée, chiffre 12…) ; `analyser` n'en rend pas.
pub fn vers_systeme(raccourci: &Raccourci) -> Option<Shortcut> {
    let m = &raccourci.modificateurs;
    let mut mods = Modifiers::empty();
    for (present, bit) in [
        (m.ctrl, Modifiers::CONTROL),
        (m.maj, Modifiers::SHIFT),
        (m.alt, Modifiers::ALT),
        (m.win, Modifiers::SUPER),
    ] {
        if present {
            mods |= bit;
        }
    }
    let mods = if mods.is_empty() { None } else { Some(mods) };
    Some(Shortcut::new(mods, code(raccourci.touche)?))
}

#[cfg(test)]
mod tests {
    use super::super::notation::{analyser, Modificateurs};
    use super::*;

    fn conv(t: &str) -> Shortcut {
        vers_systeme(&analyser(t).unwrap()).unwrap()
    }

    #[test]
    fn exemples() {
        let s = conv("Ctrl+Maj+1");
        assert_eq!(s.mods, Modifiers::CONTROL | Modifiers::SHIFT);
        assert_eq!(s.key, Code::Digit1);
        let s = conv("ImprEcran");
        assert_eq!(s.mods, Modifiers::empty());
        assert_eq!(s.key, Code::PrintScreen);
        assert_eq!(conv("Cmd+Option+Z").mods, Modifiers::SUPER | Modifiers::ALT);
        assert_eq!(conv("Win+PageSuiv").key, Code::PageDown);
    }

    #[test]
    fn toute_la_notation_se_convertit() {
        let mut textes: Vec<String> = Vec::new();
        textes.extend(('A'..='Z').map(|c| format!("Ctrl+Maj+{c}")));
        textes.extend((0..=9).map(|n| format!("Ctrl+{n}")));
        textes.extend((1..=24).map(|n| format!("F{n}")));
        for t in [
            "ImprEcran",
            "Espace",
            "Entree",
            "Tab",
            "Echap",
            "Suppr",
            "Inser",
            "Debut",
            "Fin",
            "PagePrec",
            "PageSuiv",
            "Haut",
            "Bas",
            "Gauche",
            "Droite",
        ] {
            textes.push(format!("Ctrl+Win+{t}"));
        }
        let mut vus = std::collections::HashSet::new();
        for t in &textes {
            let s = conv(t);
            assert!(vus.insert(s.key), "deux touches donnent le même code : {t}");
        }
    }

    #[test]
    fn raccourci_hors_notation() {
        let r = Raccourci {
            modificateurs: Modificateurs::default(),
            touche: Touche::Lettre('é'),
        };
        assert!(vers_systeme(&r).is_none());
    }
}
