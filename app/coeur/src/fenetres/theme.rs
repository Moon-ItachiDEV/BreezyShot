//! Thème des pages (étape 10) : clair, sombre, ou celui du système.
//!
//! Le module ne sait pas d'où vient le choix (les réglages) : l'Application le lui donne
//! par `fixer`. Il le garde, l'envoie à toutes les pages (`interface:theme { theme }`, que
//! `commun/theme.js` traduit en `data-theme` sur `<html>`), le renvoie à une page qui
//! vient d'être prête (`envoyer_a`), et règle le cadre des fenêtres Réglages et Éditeur.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

/// Événement écouté par les pages.
pub const EVENEMENT_THEME: &str = "interface:theme";

/// Le thème voulu : `None` = celui du système.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemePage {
    Clair,
    Sombre,
}

impl ThemePage {
    /// Valeur de `data-theme` dans les pages (celle des jetons).
    pub fn attribut(self) -> &'static str {
        match self {
            ThemePage::Clair => "light",
            ThemePage::Sombre => "dark",
        }
    }

    fn tauri(self) -> tauri::Theme {
        match self {
            ThemePage::Clair => tauri::Theme::Light,
            ThemePage::Sombre => tauri::Theme::Dark,
        }
    }
}

static THEME: Mutex<Option<ThemePage>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ChargeTheme {
    /// `"light"`, `"dark"` ou `null` (système).
    pub theme: Option<&'static str>,
}

/// La charge envoyée aux pages pour un thème. Fonction pure.
pub fn charge(theme: Option<ThemePage>) -> ChargeTheme {
    ChargeTheme {
        theme: theme.map(ThemePage::attribut),
    }
}

/// Le thème actuel.
pub fn actuel() -> Option<ThemePage> {
    THEME.lock().map(|t| *t).unwrap_or(None)
}

/// Le thème du cadre d'une fenêtre (`None` : celui du système).
pub fn pour_cadre(theme: Option<ThemePage>) -> Option<tauri::Theme> {
    theme.map(ThemePage::tauri)
}

/// Retient le thème sans rien envoyer (démarrage, avant que les pages existent).
pub fn retenir(theme: Option<ThemePage>) {
    if let Ok(mut t) = THEME.lock() {
        *t = theme;
    }
}

/// Retient le thème et l'envoie à toutes les pages ouvertes ; règle le cadre des fenêtres
/// Réglages et Éditeur si elles sont ouvertes.
pub fn fixer(app: &AppHandle, theme: Option<ThemePage>) {
    retenir(theme);
    if let Err(e) = app.emit(EVENEMENT_THEME, charge(theme)) {
        log::warn!("theme non envoye: {e}");
    }
    if let Some(f) = app.get_webview_window(super::reglages::ETIQUETTE_REGLAGES) {
        if let Err(e) = f.set_theme(pour_cadre(theme)) {
            log::warn!("theme du cadre non regle: {e}");
        }
    }
    super::editeur::regler_cadre(app, theme);
}

/// Envoie le thème actuel à une seule page (elle vient de dire qu'elle écoute).
pub fn envoyer_a(app: &AppHandle, etiquette: &str) {
    if let Err(e) = app.emit_to(etiquette, EVENEMENT_THEME, charge(actuel())) {
        log::warn!("theme non envoye a {etiquette}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charges() {
        assert_eq!(charge(None).theme, None);
        assert_eq!(charge(Some(ThemePage::Clair)).theme, Some("light"));
        assert_eq!(charge(Some(ThemePage::Sombre)).theme, Some("dark"));
        assert_eq!(
            serde_json::to_string(&charge(None)).unwrap(),
            "{\"theme\":null}"
        );
        assert_eq!(pour_cadre(None), None);
        assert_eq!(
            pour_cadre(Some(ThemePage::Sombre)),
            Some(tauri::Theme::Dark)
        );
    }
}
