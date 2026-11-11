//! Règles d'architecture (docs/ARCHITECTURE.md, §2) vérifiées en lisant les sources.
//!
//! - Règle 1 : seule l'Application connaît les autres modules. `raccourcis`, `capture`,
//!   `sortie`, `reglages` et `fenetres` ne s'importent pas entre eux ni `application` ;
//!   tous peuvent utiliser `systeme`, qui ne connaît aucun module.
//! - Règle 3 : `cfg(windows)`, `cfg(target_os…)` et la crate `windows` restent dans
//!   `systeme/` (et `capture/` à partir de l'étape 4).

use std::fs;
use std::path::{Path, PathBuf};

const MODULES: &[&str] = &[
    "application",
    "raccourcis",
    "capture",
    "sortie",
    "reglages",
    "fenetres",
    "systeme",
];

/// Dossiers où le code propre à un système est permis.
const DOSSIERS_SYSTEME: &[&str] = &["systeme", "capture"];

/// Motifs interdits hors de `systeme/` et `capture/` (cherchés sans espaces).
const MOTIFS_SYSTEME: &[&str] = &[
    "cfg(windows",
    "cfg(not(windows",
    "cfg!(windows",
    "cfg!(not(windows",
    "any(windows",
    "all(windows",
    "cfg(unix",
    "cfg(not(unix",
    "cfg!(unix",
    "target_os",
    "target_family",
    "windows::",
    "windows_sys::",
    "windows_core::",
];

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

/// Texte sans les lignes de commentaire, sans espaces.
fn code_sans_commentaires(texte: &str) -> String {
    texte
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            !(t.starts_with("//") || t.starts_with("/*") || t.starts_with('*'))
        })
        .collect::<Vec<_>>()
        .join("\n")
        .split_whitespace()
        .collect()
}

/// Modules de l'app cités par `crate::nom` ou `crate::{nom, …}`.
fn modules_cites(code: &str) -> Vec<String> {
    let mut cites = Vec::new();
    let mut reste = code;
    while let Some(i) = reste.find("crate::") {
        reste = &reste[i + "crate::".len()..];
        if let Some(groupe) = reste.strip_prefix('{') {
            let fin = groupe.find('}').unwrap_or(groupe.len());
            for morceau in groupe[..fin].split(',') {
                let nom: String = morceau
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                cites.push(nom);
            }
        } else {
            let nom: String = reste
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            cites.push(nom);
        }
    }
    cites.retain(|n| MODULES.contains(&n.as_str()));
    cites
}

/// Imports interdits pour un fichier du module `module`.
fn imports_interdits(module: &str, code: &str) -> Vec<String> {
    if module == "application" {
        return Vec::new();
    }
    modules_cites(code)
        .into_iter()
        .filter(|cite| cite != module)
        .filter(|cite| module == "systeme" || cite != "systeme")
        .collect()
}

fn motifs_systeme(code: &str) -> Vec<&'static str> {
    MOTIFS_SYSTEME
        .iter()
        .copied()
        .filter(|m| code.contains(m))
        .collect()
}

fn verifier(src: &Path) -> Vec<String> {
    let mut fichiers = Vec::new();
    fichiers_rs(src, &mut fichiers);
    assert!(
        !fichiers.is_empty(),
        "aucune source lue dans {}",
        src.display()
    );
    let mut fautes = Vec::new();
    for f in fichiers {
        let texte = fs::read_to_string(&f).unwrap();
        let code = code_sans_commentaires(&texte);
        let module = module_du_fichier(src, &f);
        let nom = f.strip_prefix(src).unwrap().display().to_string();
        if let Some(m) = &module {
            for interdit in imports_interdits(m, &code) {
                fautes.push(format!("{nom} : le module {m} utilise crate::{interdit}"));
            }
        }
        let permis = module
            .as_deref()
            .is_some_and(|m| DOSSIERS_SYSTEME.contains(&m));
        if !permis {
            for motif in motifs_systeme(&code) {
                fautes.push(format!("{nom} : « {motif} » hors de systeme/ et capture/"));
            }
        }
    }
    fautes
}

#[test]
fn les_sources_respectent_l_architecture() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let fautes = verifier(&src);
    assert!(
        fautes.is_empty(),
        "règles d'architecture :\n{}",
        fautes.join("\n")
    );
}

#[test]
fn le_controle_repere_les_fautes() {
    let c = code_sans_commentaires(
        "use crate::{systeme, reglages::Reglages};\nuse crate::application;",
    );
    assert_eq!(
        imports_interdits("raccourcis", &c),
        ["reglages", "application"]
    );
    assert!(imports_interdits("application", &c).is_empty());
    assert_eq!(
        imports_interdits("systeme", &code_sans_commentaires("crate::capture::x()")),
        ["capture"]
    );
    assert!(imports_interdits("sortie", &code_sans_commentaires("use crate::systeme;")).is_empty());
    assert!(
        imports_interdits("sortie", &code_sans_commentaires("use crate::sortie::x;")).is_empty()
    );

    let c = code_sans_commentaires("#[cfg( windows )]\nfn a(){}\n// cfg(target_os = \"x\")");
    assert_eq!(motifs_systeme(&c), ["cfg(windows"]);
    let c = code_sans_commentaires("use ::windows::Win32::Foundation::HWND;");
    assert_eq!(motifs_systeme(&c), ["windows::"]);
    assert!(motifs_systeme(&code_sans_commentaires(
        "#![cfg_attr(x, windows_subsystem = \"windows\")]"
    ))
    .is_empty());
}
