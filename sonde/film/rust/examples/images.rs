//! S1 : prendre les images d'une zone à la cadence d'une vidéo. Windows seulement.
//!
//!   images --methode bitblt|dxgi|wgc --zone x,y,l,h --fps 30 --duree 20
//!          [--temoin x,y] [--pointeur] [--ressources] [--png f.png] [--attente-ms 0]
//!          [--sans-rectangles] (dxgi) [--avec-cadre] [--sans-acces] [--pointeur-wgc 0|1] (wgc)
//!   images --methode dxgi-repos --zone x,y,l,h --duree 5      (rien ne bouge dans la zone)
//!   images --methode ouverture --zone x,y,l,h                 (à froid puis à chaud, point 4)
//!   images --methode perte --zone x,y,l,h --duree 12 [--temoin x,y]
//!                                     (mode 800 × 600 puis retour, point 6 ; S7 point 2)
//!   images --methode adaptateurs | ecrans | pointeur --n 300
//!   images --methode wgc-cadre [--acces] [--avec-cadre]       (cadre jaune, point 3)
//!   images --methode temoin --x 100 --y 100 --duree 600 --etat f   (témoin rapide)
//!   images --methode pc [--sortie f.txt] [--duree 10]         (point 7, sur un PC)
//!
//! Les moyens sont dans `sonde_film::prise` : `bitblt` (celui de l'app, `systeme::vivant`
//! recopié), `dxgi` (duplication sur la carte de l'écran de la zone, délai nul, rectangles
//! sales croisés avec la zone, reprise après une perte d'accès) et `wgc`
//! (Windows.Graphics.Capture de l'écran entier, périphérique sur la carte de l'écran, zone
//! découpée ; par défaut sans cadre, accès « sans cadre » demandé avant). Horloge : une prise
//! par créneau de 1/fps ; une prise en retard fait sauter les créneaux passés (comptés).
//! `--temoin x,y` : coin du témoin à l'écran ; la valeur `k` est lue sur chaque image.
//! `--pointeur` : à chaque image, la lecture du pointeur de `systeme::gel::lire_pointeur`
//! (recopiée), son temps compté à part.
//!
//! `pc` lance tout seul, sans rien installer, ce que le point 7 demande sur un PC sous
//! Windows 11 : son propre témoin (une fenêtre de 600 × 450 dont `k` avance à chaque image
//! composée, `DwmFlush`), les moyens 1, 2 et 3 sur la zone et sur l'écran entier, à 30 puis
//! à 15 images par seconde, l'ouverture à froid (cinq processus) et à chaud, le cadre de WGC
//! et son pointeur ; il écrit tout dans un fichier texte.
//!
//! Écrit sur la sortie une ligne « cle valeur » par mesure (lue par la sonde PowerShell),
//! « ok 1 » ou « ok 0 » et « erreur … » (avec le HRESULT) à la fin.

#[cfg(not(windows))]
fn main() {
    eprintln!("images : Windows seulement");
}

#[cfg(windows)]
fn main() {
    let a = sonde_film::arguments();
    sonde_film::pixels_reels();
    let _minuterie = sonde_film::Minuterie1ms::poser();
    match windows_seul::principal(&a) {
        Ok(()) => println!("ok 1"),
        Err(e) => {
            println!("ok 0");
            println!("erreur {}", e.replace('\n', " "));
        }
    }
}

#[cfg(windows)]
mod windows_seul {
    use sonde_film::prise::*;
    use sonde_film::{
        attendre_jusqua, ecrire_etat, enregistrer_png_bgra, ligne_stats, lire_png_bgra, nombre,
        ressources, temoin, temps_processeur, Rect,
    };
    use std::collections::{BTreeSet, HashMap};
    use std::path::PathBuf;
    use std::time::{Duration, Instant};
    use windows::core::{Interface, HSTRING, PCWSTR};
    use windows::Graphics::Capture::{
        Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
    };
    use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
    use windows::Graphics::DirectX::DirectXPixelFormat;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP};
    use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
    use windows::Win32::Graphics::Dwm::DwmFlush;
    use windows::Win32::Graphics::Dxgi::*;
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Registry::{
        RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ,
    };
    use windows::Win32::System::WinRT::Direct3D11::{
        CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
    };
    use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
    use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};
    use windows::Win32::UI::WindowsAndMessaging::*;

    pub fn principal(a: &HashMap<String, String>) -> Result<(), String> {
        let methode = a.get("methode").cloned().unwrap_or_default();
        println!("methode {methode}");
        match methode.as_str() {
            "adaptateurs" => return adaptateurs(),
            "pointeur" => return pointeur(nombre(a, "n", 300)),
            "wgc-cadre" => return wgc_cadre(a),
            "ecrans" => {
                for (i, e) in ecrans().iter().enumerate() {
                    println!("ecran_{i} {e}");
                }
                println!("mode {}", mode_courant());
                return Ok(());
            }
            "temoin" => return temoin_fenetre(a),
            "pc" => return pc(a),
            _ => {}
        }
        let zone = Rect::lire(a.get("zone").map(String::as_str).unwrap_or(""))?;
        println!("zone {},{},{},{}", zone.x, zone.y, zone.l, zone.h);
        match methode.as_str() {
            "dxgi-repos" => dxgi_repos(zone, a),
            "ouverture" => ouverture(zone),
            "perte" => perte(zone, a),
            m => capture(m, zone, a),
        }
    }

    /// Le moyen demandé, ouvert ; ses lignes d'ouverture.
    fn source(
        methode: &str,
        zone: Rect,
        a: &HashMap<String, String>,
    ) -> Result<(Box<dyn Source>, Vec<String>), String> {
        match methode {
            "bitblt" => Ok((Box::new(Gdi::ouvrir(zone)?), Vec::new())),
            "dxgi" => {
                let s = Dxgi::ouvrir(
                    zone,
                    nombre(a, "attente-ms", 0),
                    !a.contains_key("sans-rectangles"),
                )?;
                let o = s.ouverture;
                let notes = vec![
                    format!("adaptateur {}", s.nom),
                    format!(
                        "dxgi_ouverture_ms carte {:.1} ; D3D11CreateDevice {:.1} ; DuplicateOutput {:.1} ; première image {:.1}",
                        o.carte_ms, o.appareil_ms, o.duplication_ms, o.premiere_ms
                    ),
                ];
                Ok((Box::new(s), notes))
            }
            "wgc" => {
                let o = OptionsWgc {
                    sans_cadre: !a.contains_key("avec-cadre"),
                    acces: !a.contains_key("sans-acces"),
                    pointeur: a.get("pointeur-wgc").map(|v| v == "1"),
                };
                let mut s = Wgc::ouvrir(zone, o)?;
                let notes = std::mem::take(&mut s.notes);
                Ok((Box::new(s), notes))
            }
            autre => Err(format!("méthode inconnue « {autre} »")),
        }
    }

    /// Point 4 : ouverture et première image, deux fois dans le même processus (la première
    /// à froid : les DLL se chargent alors, à la demande).
    fn ouverture(zone: Rect) -> Result<(), String> {
        for tour in ["froid", "chaud"] {
            let t = Instant::now();
            match Dxgi::ouvrir(zone, 0, true) {
                Ok(s) => {
                    let o = s.ouverture;
                    println!(
                        "{tour}_dxgi carte {:.1} ; D3D11CreateDevice {:.1} ; DuplicateOutput {:.1} ; première image {:.1} ; total {:.1}",
                        o.carte_ms,
                        o.appareil_ms,
                        o.duplication_ms,
                        o.premiere_ms,
                        ms(t)
                    );
                    println!("{tour}_dxgi_total_ms {:.1}", ms(t));
                }
                Err(e) => println!("{tour}_dxgi erreur {e}"),
            }
            let t = Instant::now();
            match Gdi::ouvrir(zone) {
                Ok(mut g) => {
                    let ouvert = ms(t);
                    let mut b = Vec::new();
                    let r = g.prendre(&mut b, &mut Etapes::default());
                    println!(
                        "{tour}_bitblt ouvrir {ouvert:.1} ; première prise {:.1}{}",
                        ms(t) - ouvert,
                        r.err()
                            .map(|e| format!(" ; erreur {e}"))
                            .unwrap_or_default()
                    );
                    println!("{tour}_bitblt_total_ms {:.1}", ms(t));
                }
                Err(e) => println!("{tour}_bitblt erreur {e}"),
            }
        }
        Ok(())
    }

    /// Point 6 (et S7 point 2) : le mode passe à 800 × 600 à 2 s, revient à 6 s ; la
    /// duplication et BitBlt prennent la zone à 30 images par seconde pendant ce temps.
    fn perte(zone: Rect, a: &HashMap<String, String>) -> Result<(), String> {
        let duree = Duration::from_secs_f64(nombre(a, "duree", 12.0));
        let temoin_xy = a.get("temoin").and_then(|t| {
            let v: Vec<i32> = t.split(',').filter_map(|s| s.trim().parse().ok()).collect();
            (v.len() == 2).then(|| (v[0] - zone.x, v[1] - zone.y))
        });
        println!("mode_avant {}", mode_courant());
        for (i, e) in ecrans().iter().enumerate() {
            println!("ecrans_avant_{i} {e}");
        }
        let mut d = Dxgi::ouvrir(zone, 0, true)?;
        println!("adaptateur {}", d.nom);
        let mut g = Gdi::ouvrir(zone)?;
        let periode = Duration::from_secs_f64(1.0 / 30.0);
        let debut = Instant::now();
        let (mut change, mut remis) = (None::<f64>, None::<f64>);
        let mut i = 0u64;
        let (mut dxgi_neuves_pendant, mut dxgi_neuves_apres, mut premiere_apres) =
            (0u64, 0u64, None::<f64>);
        let (mut gdi_ok, mut gdi_err, mut gdi_k, mut gdi_err_texte) =
            (0u64, 0u64, 0u64, String::new());
        let mut perdu_a = None::<f64>;
        let (mut tb, mut td) = (Vec::new(), Vec::new());
        while debut.elapsed() < duree {
            attendre_jusqua(debut + periode.mul_f64(i as f64));
            i += 1;
            let t = ms(debut);
            if change.is_none() && t >= 2000.0 {
                let c = changer_mode(800, 600);
                println!("mode_change_code {c} (0 : fait ; -2 : mode refusé ; 1 : redémarrage)");
                println!("mode_pendant {}", mode_courant());
                for (j, e) in ecrans().iter().enumerate() {
                    println!("ecrans_pendant_{j} {e}");
                }
                change = Some(ms(debut));
            }
            if remis.is_none() && t >= 6000.0 {
                let c = remettre_mode();
                println!("mode_remis_code {c}");
                remis = Some(ms(debut));
            }
            let pertes_avant = d.pertes + d.autres_erreurs;
            let neuve = d.prendre(&mut td, &mut Etapes::default())?;
            if perdu_a.is_none() && d.pertes + d.autres_erreurs > pertes_avant {
                perdu_a = Some(ms(debut));
            }
            if neuve {
                match (change, remis) {
                    (Some(_), None) => dxgi_neuves_pendant += 1,
                    (Some(_), Some(_)) => {
                        dxgi_neuves_apres += 1;
                        premiere_apres.get_or_insert(ms(debut));
                    }
                    _ => {}
                }
            }
            let pendant = change.is_some() && remis.is_none_or(|r| t < r + 1000.0);
            match g.prendre(&mut tb, &mut Etapes::default()) {
                Ok(_) if pendant => {
                    gdi_ok += 1;
                    if temoin_xy
                        .is_some_and(|(x, y)| temoin::lire_k(&tb, zone.l as usize, x, y).is_some())
                    {
                        gdi_k += 1;
                    }
                }
                Ok(_) => {}
                Err(e) if pendant => {
                    gdi_err += 1;
                    gdi_err_texte = e;
                }
                Err(_) => {}
            }
        }
        if remis.is_none() {
            println!("mode_remis_code {} (en fin)", remettre_mode());
        }
        std::thread::sleep(Duration::from_millis(500));
        println!("mode_apres {}", mode_courant());
        for (j, e) in ecrans().iter().enumerate() {
            println!("ecrans_apres_{j} {e}");
        }
        let f = |x: Option<f64>| {
            x.map(|v| format!("{v:.0}"))
                .unwrap_or_else(|| "jamais".into())
        };
        println!("change_a_ms {}", f(change));
        println!("remis_a_ms {}", f(remis));
        println!("perte_vue_a_ms {}", f(perdu_a));
        println!("pertes_acces {}", d.pertes);
        println!("dxgi_autres_erreurs {}", d.autres_erreurs);
        println!(
            "reprises_ms {}",
            d.refaites_ms
                .iter()
                .map(|v| format!("{v:.0}"))
                .collect::<Vec<_>>()
                .join(";")
        );
        println!("dxgi_neuves_pendant {dxgi_neuves_pendant}");
        println!("dxgi_neuves_apres_retour {dxgi_neuves_apres}");
        println!("dxgi_premiere_neuve_apres_retour_ms {}", f(premiere_apres));
        for j in &d.journal {
            println!("dxgi_journal {j}");
        }
        println!(
            "bitblt_pendant ok {gdi_ok} ; k lisible {gdi_k} ; erreurs {gdi_err}{}",
            if gdi_err_texte.is_empty() {
                String::new()
            } else {
                format!(" ({gdi_err_texte})")
            }
        );
        Ok(())
    }

    /// Toutes les cartes et leurs écrans.
    fn adaptateurs() -> Result<(), String> {
        // SAFETY : énumération DXGI.
        unsafe {
            let f: IDXGIFactory1 =
                CreateDXGIFactory1().map_err(|e| format!("CreateDXGIFactory1: {}", hr(&e)))?;
            let mut i = 0;
            while let Ok(a) = f.EnumAdapters1(i) {
                println!("adaptateur_{i} {}", nom_adaptateur(&a));
                let mut j = 0;
                while let Ok(s) = a.EnumOutputs(j) {
                    if let Ok(d) = s.GetDesc() {
                        let r = d.DesktopCoordinates;
                        let n = d
                            .DeviceName
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(d.DeviceName.len());
                        println!(
                            "ecran_{i}_{j} {} {},{} → {},{} attaché {}",
                            String::from_utf16_lossy(&d.DeviceName[..n]),
                            r.left,
                            r.top,
                            r.right,
                            r.bottom,
                            d.AttachedToDesktop.as_bool() as u8
                        );
                    }
                    j += 1;
                }
                i += 1;
            }
            println!("adaptateurs {i}");
        }
        Ok(())
    }

    fn pointeur(n: usize) -> Result<(), String> {
        let mut infos = Vec::new();
        let mut formes = Vec::new();
        let mut montre = 0;
        for _ in 0..n {
            let (m, ti, tf) = lire_pointeur(true);
            if m {
                montre += 1;
            }
            infos.push(ti);
            if !tf.is_nan() {
                formes.push(tf + ti);
            }
        }
        println!("pointeur_montre {montre}/{n}");
        println!("{}", ligne_stats("pointeur_getcursorinfo_ms", &infos));
        println!("{}", ligne_stats("pointeur_forme_ms", &formes));
        Ok(())
    }

    /// Une prise d'images à la cadence demandée (moyens 1, 2 et 3).
    fn capture(methode: &str, zone: Rect, a: &HashMap<String, String>) -> Result<(), String> {
        let fps: f64 = nombre(a, "fps", 30.0);
        let duree = Duration::from_secs_f64(nombre(a, "duree", 20.0));
        let temoin_xy = a.get("temoin").and_then(|t| {
            let v: Vec<i32> = t.split(',').filter_map(|s| s.trim().parse().ok()).collect();
            (v.len() == 2).then(|| (v[0] - zone.x, v[1] - zone.y))
        });
        let avec_pointeur = a.contains_key("pointeur");
        let suivre = a.contains_key("ressources");
        let r0 = ressources();
        let ouverture = Instant::now();
        let (mut source, notes) = source(methode, zone, a)?;
        println!(
            "ouverture_ms {:.1}",
            ouverture.elapsed().as_secs_f64() * 1000.0
        );
        for n in &notes {
            println!("{n}");
        }
        let periode = Duration::from_secs_f64(1.0 / fps);
        let mut tampon = Vec::new();
        let (mut totaux, mut acq, mut cop, mut lec, mut totaux_neuves, mut ptr) =
            (vec![], vec![], vec![], vec![], vec![], vec![]);
        let mut neuves = 0u64;
        let mut prises = 0u64;
        let mut manques = 0u64;
        let mut vus: BTreeSet<u32> = BTreeSet::new();
        let (mut illisibles, mut dechirees, mut sauts, mut reculs) = (0u64, 0u64, 0u64, 0u64);
        let mut k_prec: Option<u32> = None;
        let mut pointeur_montre = 0u64;
        let mut quarts: Vec<(u64, u32)> = Vec::new();
        let r1 = ressources();
        let cpu0 = temps_processeur();
        let debut = Instant::now();
        let mut i: u64 = 0;
        while debut.elapsed() < duree {
            attendre_jusqua(debut + periode.mul_f64(i as f64));
            if suivre {
                let q = debut.elapsed().as_secs_f64() / duree.as_secs_f64();
                if quarts.len() < 3 && q >= (quarts.len() + 1) as f64 * 0.25 {
                    let r = ressources();
                    quarts.push((r.0, r.2));
                }
            }
            let mut e = Etapes::default();
            let t = Instant::now();
            let neuve = source.prendre(&mut tampon, &mut e)?;
            let d = t.elapsed().as_secs_f64() * 1000.0;
            if avec_pointeur {
                let (m, ti, tf) = lire_pointeur(false);
                if m {
                    pointeur_montre += 1;
                }
                ptr.push(ti + if tf.is_nan() { 0.0 } else { tf });
            }
            prises += 1;
            totaux.push(d);
            if let Some(v) = e.acquisition {
                acq.push(v);
            }
            if let Some(v) = e.copie {
                cop.push(v);
            }
            if let Some(v) = e.lecture {
                lec.push(v);
            }
            if neuve {
                neuves += 1;
                totaux_neuves.push(d);
            }
            if let Some((ox, oy)) = temoin_xy {
                match temoin::lire_k(&tampon, zone.l as usize, ox, oy) {
                    Some(k) => {
                        if temoin::ecart_bande(&tampon, zone.l as usize, ox, oy, k)
                            .is_some_and(|x| x > 24.0)
                        {
                            dechirees += 1;
                        }
                        if let Some(p) = k_prec {
                            if k > p + 1 {
                                sauts += k as u64 - p as u64 - 1;
                            } else if k < p {
                                reculs += 1;
                            }
                        }
                        k_prec = Some(k);
                        vus.insert(k);
                    }
                    None => illisibles += 1,
                }
            }
            i += 1;
            let maintenant = debut.elapsed().as_secs_f64();
            let suivant = (maintenant / periode.as_secs_f64()).ceil() as u64;
            if suivant > i {
                manques += suivant - i;
                i = suivant;
            }
        }
        let ecoule = debut.elapsed().as_secs_f64();
        let cpu = temps_processeur() - cpu0;
        let r2 = ressources();
        println!("fps_demande {fps}");
        println!("duree_s {ecoule:.2}");
        println!("prises {prises}");
        println!("prises_par_s {:.2}", prises as f64 / ecoule);
        println!("creneaux_manques {manques}");
        println!("neuves {neuves}");
        println!("neuves_par_s {:.2}", neuves as f64 / ecoule);
        println!("{}", ligne_stats("prise_ms", &totaux));
        println!("{}", ligne_stats("prise_neuve_ms", &totaux_neuves));
        println!("{}", ligne_stats("acquisition_ms", &acq));
        println!("{}", ligne_stats("copie_ms", &cop));
        println!("{}", ligne_stats("lecture_ms", &lec));
        println!("processeur_pourcent_coeur {:.1}", cpu / ecoule * 100.0);
        if temoin_xy.is_some() {
            println!("k_distincts {}", vus.len());
            println!("k_distincts_par_s {:.2}", vus.len() as f64 / ecoule);
            println!(
                "k_premier_dernier {};{}",
                vus.iter().next().copied().unwrap_or(0),
                k_prec.unwrap_or(0)
            );
            println!("k_illisibles {illisibles}");
            println!("k_dechirees {dechirees}");
            println!("k_sautes {sauts}");
            println!("k_reculs {reculs}");
        }
        let (pertes, refaites) = source.pertes();
        if methode == "dxgi" {
            println!("pertes_acces {pertes}");
            println!("{}", ligne_stats("duplication_refaite_ms", &refaites));
        }
        for b in source.bilan() {
            println!("{b}");
        }
        if avec_pointeur {
            println!("pointeur_montre {pointeur_montre}/{prises}");
            println!("{}", ligne_stats("pointeur_ms", &ptr));
        }
        if suivre {
            println!("prive_repos {}", r0.0);
            println!("prive_avant {}", r1.0);
            println!("prive_apres {}", r2.0);
            println!("prive_pic {}", r2.1);
            println!("gdi_avant {}", r1.2);
            println!("gdi_apres {}", r2.2);
            println!("user_avant {}", r1.3);
            println!("user_apres {}", r2.3);
            let q: Vec<String> = quarts.iter().map(|(m, _)| m.to_string()).collect();
            let g: Vec<String> = quarts.iter().map(|(_, o)| o.to_string()).collect();
            println!("prive_quarts {}", q.join(";"));
            println!("gdi_quarts {}", g.join(";"));
        }
        if let Some(png) = a.get("png") {
            enregistrer_png_bgra(&PathBuf::from(png), &tampon, zone.l as u32, zone.h as u32)?;
            println!("png {png}");
        }
        Ok(())
    }

    /// Ce que rend la duplication quand rien ne bouge : AcquireNextFrame (délai --attente-ms, 0 par défaut, 30 fois par seconde) ; « rien n'a changé dans la zone » lu sur les rectangles sales.
    fn dxgi_repos(zone: Rect, a: &HashMap<String, String>) -> Result<(), String> {
        let secondes: f64 = nombre(a, "duree", 5.0);
        let attente: u32 = nombre(a, "attente-ms", 0);
        let mut s = Dxgi::ouvrir(zone, attente, true)?;
        println!("adaptateur {}", s.nom);
        println!("attente_ms {attente}");
        let mut tampon = Vec::new();
        let debut = Instant::now();
        let (mut appels, mut neuves) = (0u64, 0u64);
        let mut attentes = Vec::new();
        while debut.elapsed().as_secs_f64() < secondes {
            let mut e = Etapes::default();
            if s.prendre(&mut tampon, &mut e)? {
                neuves += 1;
            }
            if let Some(v) = e.acquisition {
                attentes.push(v);
            }
            appels += 1;
            if attente == 0 {
                std::thread::sleep(Duration::from_millis(33));
            }
        }
        println!("repos_appels {appels}");
        println!("repos_delais_depasses {}", s.sans_image);
        println!("repos_images_neuves {neuves}");
        println!("repos_pointeur_seul {}", s.pointeur_seul);
        println!("repos_pertes_acces {}", s.pertes);
        println!("repos_ecran_neuves {}", s.ecran_neuves);
        println!("repos_zone_intacte {}", s.zone_intacte);
        println!("repos_sans_metadonnees {}", s.sans_metadonnees);
        println!("{}", ligne_stats("repos_acquisition_ms", &attentes));
        Ok(())
    }

    // ---------------------------------------------------------------------------------
    // WGC : le cadre jaune part-il si l'accès « sans cadre » est attendu avant la session ?

    /// L'écran entier par BitBlt (BGRA).
    fn ecran_gdi(b: Rect) -> Result<Vec<u8>, String> {
        let mut g = Gdi::ouvrir(b)?;
        let mut t = Vec::new();
        g.prendre(&mut t, &mut Etapes::default())?;
        Ok(t)
    }

    fn ecran_dxgi(b: Rect) -> Result<Vec<u8>, String> {
        let mut d = Dxgi::ouvrir(b, 100, false)?;
        let mut t = Vec::new();
        // Deux appels : la première image est rendue par `ouvrir`, la seconde est récente.
        for _ in 0..2 {
            d.prendre(&mut t, &mut Etapes::default())?;
        }
        Ok(t)
    }

    /// (pixels changés sur le bord de `bord` px, pixels jaunes) entre deux images d'écran.
    fn bord_et_jaunes(avant: &[u8], apres: &[u8], l: usize, h: usize, bord: usize) -> (u64, u64) {
        let mut change = 0u64;
        let mut jaunes = 0u64;
        for y in 0..h {
            for x in 0..l {
                let i = (y * l + x) * 4;
                if i + 3 >= apres.len() || i + 3 >= avant.len() {
                    continue;
                }
                let sur_bord = x < bord || y < bord || x + bord >= l || y + bord >= h;
                if sur_bord && avant[i..i + 3] != apres[i..i + 3] {
                    change += 1;
                }
                let (b, v, r) = (apres[i] as i32, apres[i + 1] as i32, apres[i + 2] as i32);
                if r > 200 && v > 150 && b < 90 && (r - v) < 80 {
                    jaunes += 1;
                }
            }
        }
        (change, jaunes)
    }

    fn wgc_cadre(a: &HashMap<String, String>) -> Result<(), String> {
        let avec_cadre = a.contains_key("avec-cadre");
        let acces = a.contains_key("acces");
        // SAFETY : métriques et moniteur principal, sans effet de bord.
        let (b, moniteur) = unsafe {
            let b = Rect {
                x: 0,
                y: 0,
                l: GetSystemMetrics(SM_CXSCREEN),
                h: GetSystemMetrics(SM_CYSCREEN),
            };
            (
                b,
                MonitorFromPoint(POINT { x: 1, y: 1 }, MONITOR_DEFAULTTOPRIMARY),
            )
        };
        println!("avec_cadre {}", avec_cadre as u8);
        println!("acces_attendu_avant {}", acces as u8);
        let gdi_avant = ecran_gdi(b)?;
        let dxgi_avant = ecran_dxgi(b).ok();
        // SAFETY : WinRT initialisé pour ce fil ; objets WinRT et COM comptés.
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
        }
        if acces {
            println!("wgc_acces_sans_cadre {}", acces_sans_cadre());
        }
        println!(
            "wgc_pris_en_charge {}",
            GraphicsCaptureSession::IsSupported()
                .map(|b| b as u8 as i32)
                .unwrap_or(-1)
        );
        let (dev, _ctx) = appareil(None, D3D_DRIVER_TYPE_HARDWARE).or_else(|e| {
            println!("wgc_appareil_materiel_refuse {e}");
            appareil(None, D3D_DRIVER_TYPE_WARP)
        })?;
        let dxgi: IDXGIDevice = dev.cast().map_err(|e| format!("IDXGIDevice: {}", hr(&e)))?;
        // SAFETY : appareil D3D11 vivant.
        let d3d: IDirect3DDevice = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi) }
            .and_then(|i| i.cast())
            .map_err(|e| format!("CreateDirect3D11DeviceFromDXGIDevice: {}", hr(&e)))?;
        let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
            .map_err(|e| format!("IGraphicsCaptureItemInterop: {}", hr(&e)))?;
        // SAFETY : moniteur valide.
        let item: GraphicsCaptureItem = unsafe { interop.CreateForMonitor(moniteur) }
            .map_err(|e| format!("CreateForMonitor: {}", hr(&e)))?;
        let taille = item.Size().map_err(|e| format!("Size: {}", hr(&e)))?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &d3d,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            taille,
        )
        .map_err(|e| format!("CreateFreeThreaded: {}", hr(&e)))?;
        let session = pool
            .CreateCaptureSession(&item)
            .map_err(|e| format!("CreateCaptureSession: {}", hr(&e)))?;
        if !avec_cadre {
            println!(
                "wgc_sans_cadre {}",
                match session.SetIsBorderRequired(false) {
                    Ok(()) => "1".to_string(),
                    Err(e) => format!("0 ({})", hr(&e)),
                }
            );
        }
        println!(
            "wgc_cadre_relu {}",
            session
                .IsBorderRequired()
                .map(|b| (b as u8).to_string())
                .unwrap_or_else(|e| format!("erreur {}", hr(&e)))
        );
        session
            .StartCapture()
            .map_err(|e| format!("StartCapture: {}", hr(&e)))?;
        let debut = Instant::now();
        let mut images = 0u32;
        while debut.elapsed() < Duration::from_millis(1500) {
            if let Ok(f) = pool.TryGetNextFrame() {
                images += 1;
                if images == 1 {
                    let _ = f
                        .Surface()
                        .and_then(|s| s.cast::<IDirect3DDxgiInterfaceAccess>())
                        // SAFETY : interface d'accès d'une surface D3D11 vivante.
                        .and_then(|x| unsafe { x.GetInterface::<ID3D11Texture2D>() });
                }
                let _ = f.Close();
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        println!("wgc_images_1500ms {images}");
        let gdi_pendant = ecran_gdi(b)?;
        let dxgi_pendant = ecran_dxgi(b).ok();
        let _ = session.Close();
        let _ = pool.Close();
        let (l, h) = (b.l as usize, b.h as usize);
        let (bc, ja) = bord_et_jaunes(&gdi_avant, &gdi_pendant, l, h, 6);
        let (_, j0) = bord_et_jaunes(&gdi_avant, &gdi_avant, l, h, 6);
        println!("gdi_bord_change {bc}");
        println!("gdi_jaunes_avant_pendant {j0};{ja}");
        match (dxgi_avant, dxgi_pendant) {
            (Some(x), Some(y)) => {
                let (bc, ja) = bord_et_jaunes(&x, &y, l, h, 6);
                let (_, j0) = bord_et_jaunes(&x, &x, l, h, 6);
                println!("dxgi_bord_change {bc}");
                println!("dxgi_jaunes_avant_pendant {j0};{ja}");
            }
            _ => println!("dxgi_bord_change indisponible"),
        }
        Ok(())
    }

    // ---------------------------------------------------------------------------------
    // Témoin rapide (pour `pc`) : une fenêtre de 600 × 450 dont k avance à chaque image
    // composée (DwmFlush), le même dessin que le témoin WPF de la sonde.

    unsafe extern "system" fn proc_temoin(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        // SAFETY : la procédure par défaut, pour tous les messages.
        unsafe { DefWindowProcW(h, m, w, l) }
    }

    fn temoin_fenetre(a: &HashMap<String, String>) -> Result<(), String> {
        let (x, y): (i32, i32) = (nombre(a, "x", 100), nombre(a, "y", 100));
        let duree = Duration::from_secs_f64(nombre(a, "duree", 600.0));
        let etat = PathBuf::from(a.get("etat").ok_or("--etat manquant")?);
        let (l, h) = (temoin::L, temoin::H);
        // SAFETY : fenêtre et objets GDI créés et rendus ici ; la section fait l × h × 4
        // octets ; messages pompés sur ce fil.
        unsafe {
            let inst =
                GetModuleHandleW(None).map_err(|e| format!("GetModuleHandleW: {}", hr(&e)))?;
            let classe = windows::core::w!("SondeFilmTemoinRapide");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(proc_temoin),
                hInstance: inst.into(),
                lpszClassName: classe,
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                ..Default::default()
            };
            RegisterClassW(&wc);
            let fen = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                classe,
                windows::core::w!("Temoin film BreezyShot rapide"),
                WS_POPUP | WS_VISIBLE,
                x,
                y,
                l,
                h,
                None,
                None,
                Some(inst.into()),
                None,
            )
            .map_err(|e| format!("CreateWindowExW: {}", hr(&e)))?;
            let dc = CreateCompatibleDC(None);
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: l,
                    biHeight: -h,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let section = CreateDIBSection(Some(dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0)
                .map_err(|e| format!("CreateDIBSection: {}", hr(&e)))?;
            let ancien = SelectObject(dc, section.into());
            let buf = std::slice::from_raw_parts_mut(bits as *mut u8, (l * h * 4) as usize);
            let genre = temoin::Genre::Simple;
            temoin::peindre(buf, l as usize, h as usize, 0, 0, 0, genre);
            let debut = Instant::now();
            let mut k = 0u32;
            let mut seconde = Instant::now();
            let mut pas_seconde = 0u32;
            let mut msg = MSG::default();
            while debut.elapsed() < duree {
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                k = (k + 1) & 0xFFFF;
                let _ = GdiFlush();
                temoin::peindre_lignes(
                    buf,
                    l as usize,
                    h as usize,
                    (0, 0),
                    k,
                    genre,
                    0,
                    temoin::HAUT_CHANGEANT,
                );
                let _ = GdiFlush();
                let ecran = GetDC(Some(fen));
                let _ = BitBlt(ecran, 0, 0, l, h, Some(dc), 0, 0, SRCCOPY);
                ReleaseDC(Some(fen), ecran);
                let _ = DwmFlush();
                pas_seconde += 1;
                if seconde.elapsed() >= Duration::from_secs(1) {
                    let par_s = pas_seconde as f64 / seconde.elapsed().as_secs_f64();
                    ecrire_etat(
                        &etat,
                        &[
                            "pret 1".to_string(),
                            format!("zone {x},{y},{l},{h}"),
                            format!("k_par_s {par_s:.1}"),
                            format!("k {k}"),
                        ],
                    );
                    seconde = Instant::now();
                    pas_seconde = 0;
                }
            }
            SelectObject(dc, ancien);
            let _ = DeleteObject(section.into());
            let _ = DeleteDC(dc);
            let _ = DestroyWindow(fen);
        }
        Ok(())
    }

    // ---------------------------------------------------------------------------------
    // Point 7 : le même exemple sur un PC, d'une ligne, sans rien installer.

    fn registre(nom: &str) -> String {
        let cle = HSTRING::from(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
        let n = HSTRING::from(nom);
        let mut tampon = [0u16; 256];
        let mut taille = (tampon.len() * 2) as u32;
        // SAFETY : tampon et taille cohérents ; chaînes terminées par un zéro.
        let r = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(cle.as_ptr()),
                PCWSTR(n.as_ptr()),
                RRF_RT_REG_SZ | RRF_RT_REG_DWORD,
                None,
                Some(tampon.as_mut_ptr() as *mut core::ffi::c_void),
                Some(&mut taille),
            )
        };
        if r.is_err() {
            return "?".into();
        }
        if nom == "UBR" {
            return u32::from_le_bytes([
                tampon[0] as u8,
                (tampon[0] >> 8) as u8,
                tampon[1] as u8,
                (tampon[1] >> 8) as u8,
            ])
            .to_string();
        }
        let fin = tampon.iter().position(|&c| c == 0).unwrap_or(tampon.len());
        String::from_utf16_lossy(&tampon[..fin])
    }

    /// Lance cet exécutable avec `args` (délai `delai_s`) ; rend ses lignes, préfixées.
    fn enfant(args: &[&str], delai_s: u64, prefixe: &str, sortie: &mut Vec<String>) {
        let exe = match std::env::current_exe() {
            Ok(e) => e,
            Err(e) => {
                sortie.push(format!("{prefixe} erreur current_exe: {e}"));
                return;
            }
        };
        println!("... {prefixe} : {}", args.join(" "));
        let mut c = match std::process::Command::new(exe)
            .args(args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                sortie.push(format!("{prefixe} erreur lancement: {e}"));
                return;
            }
        };
        // La sortie est lue sur un autre fil pendant l'attente : un tuyau plein bloquerait
        // l'enfant.
        let lecteur = c.stdout.take().map(|mut o| {
            std::thread::spawn(move || {
                use std::io::Read;
                let mut texte = String::new();
                let _ = o.read_to_string(&mut texte);
                texte
            })
        });
        let limite = Instant::now() + Duration::from_secs(delai_s);
        loop {
            match c.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if Instant::now() < limite => {
                    std::thread::sleep(Duration::from_millis(50))
                }
                _ => {
                    let _ = c.kill();
                    sortie.push(format!("{prefixe} erreur pas fini en {delai_s} s"));
                    break;
                }
            }
        }
        let texte = lecteur.and_then(|l| l.join().ok()).unwrap_or_default();
        for l in texte.lines() {
            if !l.trim().is_empty() {
                sortie.push(format!("{prefixe} {l}"));
            }
        }
    }

    fn pc(a: &HashMap<String, String>) -> Result<(), String> {
        let fichier = PathBuf::from(
            a.get("sortie")
                .cloned()
                .unwrap_or_else(|| "sonde-film-pc-resultats.txt".into()),
        );
        let d: f64 = nombre(a, "duree", 10.0);
        let ds = format!("{d}");
        let mut s: Vec<String> = vec![
            "# Sonde du film (BreezyShot 0.7), S1 sur un PC : résultats bruts".into(),
            format!(
                "windows {} ; édition {} ; version {} ; build {}.{}",
                registre("ProductName"),
                registre("EditionID"),
                registre("DisplayVersion"),
                registre("CurrentBuild"),
                registre("UBR")
            ),
        ];
        // SAFETY : métriques de l'écran principal.
        let (el, eh) = unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };
        s.push(format!("ecran_principal {el}x{eh}"));
        enfant(&["--methode", "adaptateurs"], 30, "[cartes]", &mut s);
        enfant(&["--methode", "ecrans"], 30, "[ecrans]", &mut s);
        // Le témoin, dans son propre processus.
        let etat =
            std::env::temp_dir().join(format!("sonde-film-pc-temoin-{}.txt", std::process::id()));
        let _ = std::fs::remove_file(&etat);
        let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
        let mut t = std::process::Command::new(exe)
            .args([
                "--methode",
                "temoin",
                "--x",
                "100",
                "--y",
                "100",
                "--duree",
                "900",
                "--etat",
            ])
            .arg(&etat)
            .stdout(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("témoin : {e}"))?;
        let limite = Instant::now() + Duration::from_secs(20);
        while !std::fs::read_to_string(&etat).is_ok_and(|x| x.contains("pret 1")) {
            if Instant::now() > limite {
                let _ = t.kill();
                return Err("le témoin n'a rien écrit en 20 s".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        std::thread::sleep(Duration::from_secs(2));
        let rythme = |s: &mut Vec<String>, quand: &str| {
            let x = std::fs::read_to_string(&etat).unwrap_or_default();
            let v = x
                .lines()
                .find(|l| l.starts_with("k_par_s"))
                .unwrap_or("k_par_s ?");
            s.push(format!("[temoin {quand}] {v}"));
        };
        rythme(&mut s, "début");
        let ecran = format!("0,0,{el},{eh}");
        for (nom, zone) in [("zone", "100,100,600,450"), ("ecran", ecran.as_str())] {
            for m in ["bitblt", "dxgi", "wgc"] {
                for fps in ["30", "15"] {
                    let p = format!("[{nom} {m} {fps}]");
                    enfant(
                        &[
                            "--methode",
                            m,
                            "--zone",
                            zone,
                            "--fps",
                            fps,
                            "--duree",
                            &ds,
                            "--temoin",
                            "100,100",
                        ],
                        (d as u64) + 60,
                        &p,
                        &mut s,
                    );
                }
            }
            rythme(&mut s, nom);
        }
        // Le pointeur dans l'image de WGC, IsCursorCaptureEnabled vrai puis faux.
        // SAFETY : déplace le pointeur au milieu de la zone.
        unsafe {
            let _ = SetCursorPos(400, 325);
        }
        let dossier = std::env::temp_dir();
        for (v, t) in [("1", "avec"), ("0", "sans")] {
            let f = dossier.join(format!("sonde-film-pc-wgc-pointeur-{t}.png"));
            let fs = f.to_string_lossy().to_string();
            enfant(
                &[
                    "--methode",
                    "wgc",
                    "--zone",
                    "300,250,200,150",
                    "--fps",
                    "10",
                    "--duree",
                    "1",
                    "--pointeur-wgc",
                    v,
                    "--png",
                    &fs,
                ],
                60,
                &format!("[wgc pointeur {v}]"),
                &mut s,
            );
        }
        let (fa, fs) = (
            dossier.join("sonde-film-pc-wgc-pointeur-avec.png"),
            dossier.join("sonde-film-pc-wgc-pointeur-sans.png"),
        );
        match (lire_png_bgra(&fa), lire_png_bgra(&fs)) {
            (Ok(x), Ok(y)) if x.0.len() == y.0.len() => {
                let n =
                    x.0.chunks(4)
                        .zip(y.0.chunks(4))
                        .filter(|(p, q)| p[..3] != q[..3])
                        .count();
                s.push(format!(
                    "[wgc pointeur] pixels différents entre IsCursorCaptureEnabled 1 et 0 : {n}"
                ));
            }
            _ => s.push("[wgc pointeur] images illisibles".into()),
        }
        // SAFETY : rend le pointeur au coin.
        unsafe {
            let _ = SetCursorPos(el - 5, 5);
        }
        let _ = t.kill();
        let _ = t.wait();
        let zone = "100,100,600,450";
        for i in 1..=5 {
            enfant(
                &["--methode", "ouverture", "--zone", zone],
                60,
                &format!("[ouverture {i}]"),
                &mut s,
            );
        }
        enfant(
            &["--methode", "wgc-cadre", "--avec-cadre"],
            60,
            "[wgc cadre témoin]",
            &mut s,
        );
        enfant(
            &["--methode", "wgc-cadre"],
            60,
            "[wgc sans cadre, sans accès]",
            &mut s,
        );
        enfant(
            &["--methode", "wgc-cadre", "--acces"],
            60,
            "[wgc sans cadre, accès demandé avant]",
            &mut s,
        );
        let _ = std::fs::remove_file(&etat);
        std::fs::write(&fichier, s.join("\r\n") + "\r\n")
            .map_err(|e| format!("{} : {e}", fichier.display()))?;
        for l in &s {
            println!("{l}");
        }
        println!("fichier {}", fichier.display());
        Ok(())
    }
}
