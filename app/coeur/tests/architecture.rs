//! Règles d'architecture (docs/ARCHITECTURE.md, §2) vérifiées en lisant les sources avec `syn`.
//!
//! - Règle 1 : seule l'Application connaît les autres modules. `raccourcis`, `capture`,
//!   `sortie`, `reglages` et `fenetres` ne s'importent pas entre eux ni `application` ;
//!   tous peuvent utiliser `systeme`, qui ne connaît aucun module. `texte` (0.5) n'utilise
//!   que `systeme` ; `droits` n'utilise rien, et `droits::ouverte` n'est appelé que depuis
//!   `application` (avenant 0.5, §2.2).
//!   `crate as …` et `super::super` sont interdits partout (ils contourneraient le contrôle).
//! - Règle 3 : hors de `systeme/`, aucun `cfg` (attribut, `cfg_attr` ou
//!   `cfg!`) qui parle du système, et aucune crate `windows`, sous quelque nom que ce soit.

use std::fs;
use std::path::{Path, PathBuf};

use proc_macro2::{TokenStream, TokenTree};
use syn::visit::{self, Visit};

const MODULES: &[&str] = &[
    "application",
    "raccourcis",
    "capture",
    "sortie",
    "reglages",
    "fenetres",
    "installation",
    "systeme",
    "texte",
    "droits",
];

/// Dossiers où le code propre à un système est permis.
const DOSSIERS_SYSTEME: &[&str] = &["systeme"];

/// Crates propres à Windows.
const CRATES_SYSTEME: &[&str] = &["windows", "windows_sys", "windows_core"];

/// Mots qui, dans un `cfg`, désignent un système (`test`, `debug_assertions`, `feature` restent permis).
const CFG_SYSTEME: &[&str] = &[
    "windows",
    "unix",
    "target_os",
    "target_family",
    "target_env",
    "target_arch",
];

struct Controle<'a> {
    module: Option<&'a str>,
    systeme_permis: bool,
    fautes: Vec<String>,
}

impl Controle<'_> {
    fn faute(&mut self, texte: String) {
        self.fautes.push(texte);
    }

    /// Le module courant a-t-il le droit d'utiliser `cible` ?
    fn verifier_module(&mut self, cible: &str) {
        let Some(module) = self.module else { return };
        if module == "application" || cible == module || !MODULES.contains(&cible) {
            return;
        }
        if cible == "systeme" && module != "systeme" {
            return;
        }
        self.faute(format!("le module {module} utilise crate::{cible}"));
    }

    fn verifier_chemin(&mut self, segments: &[String]) {
        // `droits::ouverte` : l'Application seulement (et `droits` lui-même).
        let ouverte = segments
            .windows(2)
            .any(|w| w[0] == "droits" && w[1] == "ouverte");
        if ouverte && !matches!(self.module, Some("application" | "droits")) {
            let module = self.module.unwrap_or("<racine>");
            self.faute(format!("droits::ouverte appele depuis {module}"));
        }
        if self.module == Some("droits") && segments.first().is_some_and(|p| p == "crate") {
            self.faute(format!("le module droits utilise {}", segments.join("::")));
        }
        match segments {
            [premier, second, ..] if premier == "crate" => {
                let second = second.clone();
                self.verifier_module(&second);
            }
            _ => {}
        }
        if segments.len() >= 2 && segments[0] == "super" && segments[1] == "super" {
            self.faute(format!("chemin super::super ({})", segments.join("::")));
        }
        if !self.systeme_permis {
            if let Some(premier) = segments.first() {
                if CRATES_SYSTEME.contains(&premier.as_str()) {
                    self.faute(format!("crate {premier} hors de systeme/"));
                }
            }
        }
    }

    fn verifier_cfg(&mut self, nom: &str, condition: &TokenStream) {
        if self.systeme_permis {
            return;
        }
        let mut mots = Vec::new();
        identifiants(condition, &mut mots);
        if let Some(m) = mots.iter().find(|m| CFG_SYSTEME.contains(&m.as_str())) {
            self.faute(format!("{nom}(… {m} …) hors de systeme/"));
        }
    }

    /// Dans le corps d'une macro, cherche `crate::x`, `super::super`, `windows::`.
    fn verifier_jetons(&mut self, jetons: &TokenStream) {
        let arbres: Vec<TokenTree> = jetons.clone().into_iter().collect();
        for (i, arbre) in arbres.iter().enumerate() {
            if let TokenTree::Group(g) = arbre {
                self.verifier_jetons(&g.stream());
                continue;
            }
            let TokenTree::Ident(id) = arbre else {
                continue;
            };
            let suivi_de_chemin = matches!(
                (arbres.get(i + 1), arbres.get(i + 2)),
                (Some(TokenTree::Punct(a)), Some(TokenTree::Punct(b))) if a.as_char() == ':' && b.as_char() == ':'
            );
            if !suivi_de_chemin {
                continue;
            }
            let apres = match arbres.get(i + 3) {
                Some(TokenTree::Ident(s)) => s.to_string(),
                _ => String::new(),
            };
            self.verifier_chemin(&[id.to_string(), apres]);
        }
    }
}

/// Tous les identifiants d'un flux de jetons, groupes compris.
fn identifiants(jetons: &TokenStream, sortie: &mut Vec<String>) {
    for arbre in jetons.clone() {
        match arbre {
            TokenTree::Ident(id) => sortie.push(id.to_string()),
            TokenTree::Group(g) => identifiants(&g.stream(), sortie),
            _ => {}
        }
    }
}

/// Aplati un arbre `use` en chemins complets ; signale `crate as …`.
fn chemins_use(arbre: &syn::UseTree, prefixe: &mut Vec<String>, sortie: &mut Vec<Vec<String>>) {
    match arbre {
        syn::UseTree::Path(p) => {
            prefixe.push(p.ident.to_string());
            chemins_use(&p.tree, prefixe, sortie);
            prefixe.pop();
        }
        syn::UseTree::Name(n) => {
            let mut c = prefixe.clone();
            c.push(n.ident.to_string());
            sortie.push(c);
        }
        syn::UseTree::Rename(r) => {
            let mut c = prefixe.clone();
            c.push(r.ident.to_string());
            if c.len() == 1 && c[0] == "crate" {
                // `use crate as c;` : marqué par un segment impossible.
                c.push("<crate as>".into());
            }
            sortie.push(c);
        }
        syn::UseTree::Glob(_) => {
            let mut c = prefixe.clone();
            c.push("*".into());
            sortie.push(c);
        }
        syn::UseTree::Group(g) => {
            for t in &g.items {
                chemins_use(t, prefixe, sortie);
            }
        }
    }
}

impl<'ast> Visit<'ast> for Controle<'_> {
    fn visit_item_use(&mut self, i: &'ast syn::ItemUse) {
        let mut chemins = Vec::new();
        chemins_use(&i.tree, &mut Vec::new(), &mut chemins);
        for c in chemins {
            if c.len() == 2 && c[0] == "crate" && c[1] == "<crate as>" {
                self.faute("use crate as … (alias de la crate)".into());
                continue;
            }
            self.verifier_chemin(&c);
        }
        visit::visit_item_use(self, i);
    }

    fn visit_path(&mut self, p: &'ast syn::Path) {
        let segments: Vec<String> = p.segments.iter().map(|s| s.ident.to_string()).collect();
        self.verifier_chemin(&segments);
        visit::visit_path(self, p);
    }

    fn visit_item_extern_crate(&mut self, i: &'ast syn::ItemExternCrate) {
        let nom = i.ident.to_string();
        if nom == "crate" || nom == "self" {
            self.faute("extern crate self as … (alias de la crate)".into());
        }
        if !self.systeme_permis && CRATES_SYSTEME.contains(&nom.as_str()) {
            self.faute(format!("extern crate {nom} hors de systeme/"));
        }
        visit::visit_item_extern_crate(self, i);
    }

    fn visit_attribute(&mut self, a: &'ast syn::Attribute) {
        if let syn::Meta::List(liste) = &a.meta {
            if liste.path.is_ident("cfg") {
                self.verifier_cfg("cfg", &liste.tokens);
            } else if liste.path.is_ident("cfg_attr") {
                // Seule la condition, avant la première virgule.
                let condition: TokenStream = liste
                    .tokens
                    .clone()
                    .into_iter()
                    .take_while(|t| !matches!(t, TokenTree::Punct(p) if p.as_char() == ','))
                    .collect();
                self.verifier_cfg("cfg_attr", &condition);
            }
        }
        visit::visit_attribute(self, a);
    }

    fn visit_macro(&mut self, m: &'ast syn::Macro) {
        if m.path.is_ident("cfg") {
            self.verifier_cfg("cfg!", &m.tokens);
        } else {
            self.verifier_jetons(&m.tokens);
        }
        visit::visit_macro(self, m);
    }
}

/// Contrôle une source : `module` est le module de l'app auquel elle appartient
/// (`None` pour `lib.rs` et `main.rs`).
fn controler(module: Option<&str>, source: &str) -> Vec<String> {
    let fichier = match syn::parse_file(source) {
        Ok(f) => f,
        Err(e) => return vec![format!("source illisible par syn : {e}")],
    };
    let mut c = Controle {
        module,
        systeme_permis: module.is_some_and(|m| DOSSIERS_SYSTEME.contains(&m)),
        fautes: Vec::new(),
    };
    c.visit_file(&fichier);
    c.fautes
}

fn fichiers_rs(dossier: &Path, sortie: &mut Vec<PathBuf>) {
    let Ok(entrees) = fs::read_dir(dossier) else {
        return;
    };
    for e in entrees.flatten() {
        let p = e.path();
        if p.is_dir() {
            fichiers_rs(&p, sortie);
        } else if p.extension().is_some_and(|x| x == "rs") {
            sortie.push(p);
        }
    }
}

/// Module auquel appartient un fichier (`src/raccourcis/choix.rs` → `raccourcis`).
fn module_du_fichier(src: &Path, fichier: &Path) -> Option<String> {
    let relatif = fichier.strip_prefix(src).ok()?;
    let premier = relatif.components().next()?.as_os_str().to_string_lossy();
    let nom = premier.trim_end_matches(".rs").to_string();
    MODULES.contains(&nom.as_str()).then_some(nom)
}

#[test]
fn les_sources_respectent_l_architecture() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut fichiers = Vec::new();
    fichiers_rs(&src, &mut fichiers);
    assert!(
        !fichiers.is_empty(),
        "aucune source lue dans {}",
        src.display()
    );
    let mut fautes = Vec::new();
    for f in fichiers {
        let texte = fs::read_to_string(&f).unwrap();
        let module = module_du_fichier(&src, &f);
        let nom = f.strip_prefix(&src).unwrap().display().to_string();
        for faute in controler(module.as_deref(), &texte) {
            fautes.push(format!("{nom} : {faute}"));
        }
    }
    assert!(
        fautes.is_empty(),
        "règles d'architecture :\n{}",
        fautes.join("\n")
    );
}

#[test]
fn le_controle_repere_les_pieges() {
    // Les 13 formes trouvées par la relecture, chacune doit être repérée.
    let pieges: &[(&str, &str)] = &[
        (
            "reglages",
            "fn f() { super::super::application::lancer(); }",
        ),
        (
            "reglages",
            "use crate::{systeme::{Theme, Dossiers}, application};",
        ),
        (
            "reglages",
            "use crate as c;\nfn f(){ c::application::lancer(); }",
        ),
        ("reglages", "#[cfg(all(not(windows)))]\nfn f(){}"),
        (
            "reglages",
            "#[cfg(any(unix, target_env = \"msvc\"))]\nfn f(){}",
        ),
        (
            "reglages",
            "use windows as w;\nfn f(){ let _ = w::Win32::Foundation::HWND::default(); }",
        ),
        (
            "reglages",
            "fn f(x:&mut u32){\n    *x = crate::application::VERSION.len() as u32;\n}",
        ),
        (
            "reglages",
            "macro_rules! m { () => { crate::application::lancer() } }",
        ),
        ("reglages", "fn f(){ crate::application::lancer(); }"),
        ("raccourcis", "#[cfg(target_os = \"windows\")]\nfn f(){}"),
        ("raccourcis", "fn f(){ if cfg!(target_os=\"macos\") {} }"),
        (
            "raccourcis",
            "/* debut */ fn f(){ crate::application::lancer(); }",
        ),
        (
            "raccourcis",
            "#[cfg(feature = \"x\")] #[cfg_attr(windows, allow(dead_code))]\nfn f(){}",
        ),
    ];
    for (module, source) in pieges {
        assert!(
            !controler(Some(module), source).is_empty(),
            "piège non repéré dans {module} : {source}"
        );
    }

    // Autres fautes.
    let fautes: &[(Option<&str>, &str)] = &[
        (Some("raccourcis"), "use crate::reglages::Reglages;"),
        (Some("fenetres"), "fn f(){ crate::raccourcis::x(); }"),
        (Some("reglages"), "use crate::fenetres;"),
        (Some("systeme"), "use crate::reglages;"),
        (Some("systeme"), "use crate::application::lancer;"),
        (Some("application"), "#[cfg(windows)] fn f(){}"),
        (
            Some("application"),
            "use ::windows::Win32::Foundation::HWND;",
        ),
        (Some("application"), "extern crate windows as w;"),
        (Some("sortie"), "fn f(){ let _ = windows_sys::x; }"),
        (None, "#[cfg(target_arch = \"x86_64\")] fn f(){}"),
        (Some("capture"), "#[cfg(target_os = \"macos\")] fn f(){}"),
        (Some("capture"), "fn f(){ if cfg!(windows) {} }"),
        (Some("capture"), "use ::windows::Win32::Foundation::HWND;"),
        (Some("texte"), "use crate::sortie::copier_texte;"),
        (Some("texte"), "fn f(){ crate::application::lancer(); }"),
        (Some("systeme"), "use crate::texte::lire;"),
        (Some("droits"), "use crate::systeme;"),
        (Some("sortie"), "fn f(){ crate::droits::ouverte(x); }"),
        (Some("fenetres"), "use crate::droits::{ouverte, Fonction};"),
        (None, "fn f(){ breezyshot::droits::ouverte(x); }"),
    ];
    for (module, source) in fautes {
        assert!(
            !controler(*module, source).is_empty(),
            "faute non repérée dans {module:?} : {source}"
        );
    }

    // Ce qui est permis.
    let permis: &[(Option<&str>, &str)] = &[
        (Some("raccourcis"), "use crate::systeme as s;\nfn f(){ s::theme_barre(); }"),
        (Some("reglages"), "use crate::systeme::{self, Dossiers};"),
        (Some("reglages"), "use crate::reglages::x; use super::y;"),
        (Some("application"), "use crate::{reglages, raccourcis, fenetres, systeme};"),
        (Some("reglages"), "#[cfg(test)] mod tests { use super::*; }"),
        (Some("reglages"), "#[cfg(debug_assertions)] fn f(){}"),
        (Some("systeme"), "#[cfg(windows)] #[path = \"windows.rs\"] mod plateforme;"),
        (Some("systeme"), "use ::windows::Win32::Foundation::HWND;"),
        (Some("capture"), "#[cfg(test)] mod tests {}"),
        (Some("capture"), "use crate::systeme::gel::Pixels;"),
        (
            None,
            "#![cfg_attr(not(debug_assertions), windows_subsystem = \"windows\")]\nfn main(){ breezyshot::lancer(); }",
        ),
        (Some("reglages"), "// crate::application\nfn f(){ let _ = \"crate::application\"; }"),
        (Some("texte"), "use crate::systeme::texte::{self, Reconnu};"),
        (
            Some("application"),
            "use crate::droits::{self, Fonction};\nfn f(){ droits::ouverte(Fonction::Texte); }",
        ),
        (Some("droits"), "fn f(){ ouverte(Fonction::Texte); }"),
    ];
    for (module, source) in permis {
        let f = controler(*module, source);
        assert!(
            f.is_empty(),
            "faux positif dans {module:?} : {source} : {f:?}"
        );
    }
}
