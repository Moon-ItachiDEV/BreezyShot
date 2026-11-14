//! S6 : un fichier dans le presse-papiers par `arboard` 3.7 (`set().file_list`), comme le
//! fera `sortie` pour un film. Windows seulement.
//!
//!   presse --fichier F --etat E --signal S
//!
//! Ouvre le presse-papiers (`Clipboard::new`) et le GARDE ouvert jusqu'à la fin, comme
//! l'app (`sortie/presse_papiers.rs`, `OUVERT`), pose la liste [F], la relit par arboard,
//! écrit E (« pose 1 », « relu … », ou l'erreur), puis attend « quitter » dans S (120 s au
//! plus) : pendant ce temps la sonde lit le presse-papiers et colle dans l'Explorateur.

#[cfg(not(windows))]
fn main() {
    eprintln!("presse : Windows seulement");
}

#[cfg(windows)]
fn main() {
    use std::path::PathBuf;
    use std::time::{Duration, Instant};
    let a = sonde_film::arguments();
    let (Some(f), Some(etat), Some(signal)) = (a.get("fichier"), a.get("etat"), a.get("signal"))
    else {
        println!("ok 0");
        println!("erreur --fichier, --etat et --signal sont demandés");
        return;
    };
    let (f, etat, signal) = (PathBuf::from(f), PathBuf::from(etat), PathBuf::from(signal));
    let mut lignes = vec![format!("pid {}", std::process::id())];
    let t = Instant::now();
    let mut garde = None;
    match arboard::Clipboard::new() {
        Err(e) => lignes.push(format!("erreur Clipboard::new: {e}")),
        Ok(mut c) => {
            match c.set().file_list(&[&f]) {
                Ok(()) => lignes.push("pose 1".into()),
                Err(e) => lignes.push(format!("erreur file_list: {e}")),
            }
            lignes.push(format!("pose_ms {:.1}", t.elapsed().as_secs_f64() * 1000.0));
            match c.get().file_list() {
                Ok(v) => lignes.push(format!(
                    "relu_arboard {}",
                    v.iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join(";")
                )),
                Err(e) => lignes.push(format!("relu_arboard erreur {e}")),
            }
            garde = Some(c);
        }
    }
    sonde_film::ecrire_etat(&etat, &lignes);
    let limite = Instant::now() + Duration::from_secs(120);
    while Instant::now() < limite {
        if std::fs::read_to_string(&signal).is_ok_and(|t| t.trim() == "quitter") {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    drop(garde);
    println!("ok 1");
}
