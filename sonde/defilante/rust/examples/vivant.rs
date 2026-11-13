//! S2 : prendre des images vivantes d'un rectangle de l'écran. Windows seulement.
//!
//!   vivant --methode bitblt|captureblt|dxgi|wgc-ecran|wgc-fenetre --zone x,y,l,h
//!          [--images 200] [--intervalle 16] [--hwnd H] [--png f.png]
//!          [--suite dossier --chaque 10] [--ressources]
//!   vivant --methode clignotement --zone x,y,l,h --duree-ms 3000 [--captureblt]
//!   vivant --methode bitblt-detail --zone x,y,l,h
//!
//! WGC : `--avec-cadre` laisse le cadre jaune demandé (IsBorderRequired non posé à faux),
//! `--avec-pointeur` laisse le pointeur dans l'image (témoins de comparaison de S2).
//! `--ressources` : mémoire privée et objets GDI avant, au quart, à la moitié, aux trois
//! quarts et après les images (une fuite se voit comme une pente, pas comme un palier).
//! `bitblt-detail` : d'où viennent les 32 ms de BitBlt au premier passage (quelle que soit la
//! taille) : tailles 100 × 100, 600 × 400 et la zone ; 20 prises sans pause, 20 avec 16 ms
//! de pause, 20 après DwmFlush ; temps de DwmFlush seul ; et avec timeBeginPeriod(1).
//!
//! Écrit sur la sortie une ligne « cle valeur » par mesure (lue par la sonde PowerShell).
//! Une prise DXGI ou WGC ne bloque pas : s'il n'y a pas d'image neuve, la précédente est
//! rendue (comme le ferait `systeme::vivant::Prise::prendre`) ; « neuves » les compte.

#[cfg(not(windows))]
fn main() {
    eprintln!("vivant : Windows seulement");
}

#[cfg(windows)]
fn main() {
    let a = sonde_defilante::arguments();
    sonde_defilante::pixels_reels();
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
    use sonde_defilante::{enregistrer_png_bgra, ressources, statistiques, Rect};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use windows::core::Interface;
    use windows::Graphics::Capture::{
        Direct3D11CaptureFramePool, GraphicsCaptureAccess, GraphicsCaptureAccessKind,
        GraphicsCaptureItem, GraphicsCaptureSession,
    };
    use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
    use windows::Graphics::DirectX::DirectXPixelFormat;
    use windows::Win32::Foundation::{HMODULE, HWND, POINT};
    use windows::Win32::Graphics::Direct3D::{
        D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_UNKNOWN, D3D_DRIVER_TYPE_WARP,
    };
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Dwm::DwmFlush;
    use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
    use windows::Win32::Graphics::Dxgi::*;
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::System::WinRT::Direct3D11::{
        CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
    };
    use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
    use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};
    use windows::Win32::UI::WindowsAndMessaging::{GetCursorInfo, CURSORINFO, CURSOR_SHOWING};

    trait Source {
        /// Une image BGRA de la zone dans `dest` ; vrai si elle est neuve.
        fn prendre(&mut self, dest: &mut Vec<u8>) -> Result<bool, String>;
    }

    // ---------------------------------------------------------------------------------
    // Moyen 1 : BitBlt du DC de l'écran dans une section DIB réutilisée.

    struct Gdi {
        ecran: HDC,
        dc: HDC,
        bitmap: HBITMAP,
        ancien: HGDIOBJ,
        bits: *mut u8,
        zone: Rect,
        rop: ROP_CODE,
    }

    impl Gdi {
        fn ouvrir(zone: Rect, captureblt: bool) -> Result<Gdi, String> {
            // SAFETY : objets GDI créés ici, rendus par `drop`.
            unsafe {
                let ecran = GetDC(None);
                if ecran.is_invalid() {
                    return Err("GetDC(NULL) a échoué".into());
                }
                let dc = CreateCompatibleDC(Some(ecran));
                let bmi = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: zone.l,
                        biHeight: -zone.h,
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
                let bitmap = CreateDIBSection(Some(dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0)
                    .map_err(|e| format!("CreateDIBSection: {e}"))?;
                let ancien = SelectObject(dc, bitmap.into());
                Ok(Gdi {
                    ecran,
                    dc,
                    bitmap,
                    ancien,
                    bits: bits as *mut u8,
                    zone,
                    rop: if captureblt {
                        SRCCOPY | CAPTUREBLT
                    } else {
                        SRCCOPY
                    },
                })
            }
        }
    }

    impl Source for Gdi {
        fn prendre(&mut self, dest: &mut Vec<u8>) -> Result<bool, String> {
            let z = self.zone;
            // SAFETY : DC valides ; la section fait l × h × 4 octets.
            unsafe {
                BitBlt(
                    self.dc,
                    0,
                    0,
                    z.l,
                    z.h,
                    Some(self.ecran),
                    z.x,
                    z.y,
                    self.rop,
                )
                .map_err(|e| format!("BitBlt: {e}"))?;
                let _ = GdiFlush();
                let n = (z.l * z.h * 4) as usize;
                dest.resize(n, 0);
                std::ptr::copy_nonoverlapping(self.bits, dest.as_mut_ptr(), n);
            }
            Ok(true)
        }
    }

    impl Drop for Gdi {
        fn drop(&mut self) {
            // SAFETY : objets créés par `ouvrir`.
            unsafe {
                SelectObject(self.dc, self.ancien);
                let _ = DeleteObject(self.bitmap.into());
                let _ = DeleteDC(self.dc);
                ReleaseDC(None, self.ecran);
            }
        }
    }

    // ---------------------------------------------------------------------------------
    // Copie d'une texture D3D11 (sous-rectangle) vers la mémoire.

    struct Copieur {
        ctx: ID3D11DeviceContext,
        staging: ID3D11Texture2D,
        l: u32,
        h: u32,
    }

    impl Copieur {
        fn nouveau(
            device: &ID3D11Device,
            ctx: ID3D11DeviceContext,
            l: u32,
            h: u32,
        ) -> Result<Copieur, String> {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: l,
                Height: h,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
            };
            let mut t = None;
            // SAFETY : description valide ; la texture est rendue par le compte de références.
            unsafe { device.CreateTexture2D(&desc, None, Some(&mut t)) }
                .map_err(|e| format!("CreateTexture2D: {e}"))?;
            Ok(Copieur {
                ctx,
                staging: t.ok_or("texture absente")?,
                l,
                h,
            })
        }

        fn copier(
            &self,
            source: &ID3D11Texture2D,
            x: u32,
            y: u32,
            dest: &mut Vec<u8>,
        ) -> Result<(), String> {
            let boite = D3D11_BOX {
                left: x,
                top: y,
                front: 0,
                right: x + self.l,
                bottom: y + self.h,
                back: 1,
            };
            // SAFETY : textures du même appareil ; la boîte est dans la source (vérifié par
            // l'appelant) ; la carte est lue ligne par ligne selon RowPitch puis rendue.
            unsafe {
                self.ctx
                    .CopySubresourceRegion(&self.staging, 0, 0, 0, 0, source, 0, Some(&boite));
                let mut m = D3D11_MAPPED_SUBRESOURCE::default();
                self.ctx
                    .Map(&self.staging, 0, D3D11_MAP_READ, 0, Some(&mut m))
                    .map_err(|e| format!("Map: {e}"))?;
                let ligne = (self.l * 4) as usize;
                dest.resize(ligne * self.h as usize, 0);
                for r in 0..self.h as usize {
                    std::ptr::copy_nonoverlapping(
                        (m.pData as *const u8).add(r * m.RowPitch as usize),
                        dest.as_mut_ptr().add(r * ligne),
                        ligne,
                    );
                }
                self.ctx.Unmap(&self.staging, 0);
            }
            Ok(())
        }
    }

    fn appareil(
        adaptateur: Option<&IDXGIAdapter>,
        genre: D3D_DRIVER_TYPE,
    ) -> Result<(ID3D11Device, ID3D11DeviceContext), String> {
        let mut d = None;
        let mut c = None;
        // SAFETY : création d'un appareil D3D11 ; les sorties sont des Option.
        unsafe {
            D3D11CreateDevice(
                adaptateur,
                genre,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d),
                None,
                Some(&mut c),
            )
        }
        .map_err(|e| format!("D3D11CreateDevice({}): {e}", genre.0))?;
        Ok((d.ok_or("appareil absent")?, c.ok_or("contexte absent")?))
    }

    fn nom_adaptateur(a: &IDXGIAdapter1) -> String {
        // SAFETY : lecture de la description.
        unsafe { a.GetDesc1() }
            .map(|d| {
                let n = d
                    .Description
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(d.Description.len());
                format!(
                    "{} (vendeur 0x{:04X}, drapeaux {})",
                    String::from_utf16_lossy(&d.Description[..n]),
                    d.VendorId,
                    d.Flags
                )
            })
            .unwrap_or_else(|e| format!("? ({e})"))
    }

    // ---------------------------------------------------------------------------------
    // Moyen 2 : duplication DXGI de l'écran qui contient la zone.

    struct Dxgi {
        copieur: Copieur,
        dupl: IDXGIOutputDuplication,
        x: u32,
        y: u32,
        derniere: Vec<u8>,
        pub pointeur_maj: u32,
        pub pointeur_invisible: u32,
        attente_ms: u32,
    }

    impl Dxgi {
        fn ouvrir(zone: Rect, attente_ms: u32) -> Result<(Dxgi, String), String> {
            // SAFETY : énumération DXGI ; objets COM comptés.
            unsafe {
                let fabrique: IDXGIFactory1 =
                    CreateDXGIFactory1().map_err(|e| format!("CreateDXGIFactory1: {e}"))?;
                let mut i = 0;
                loop {
                    let a = fabrique.EnumAdapters1(i).map_err(|e| {
                        format!("aucun adaptateur avec un écran contenant la zone ({e})")
                    })?;
                    let mut j = 0;
                    while let Ok(sortie) = a.EnumOutputs(j) {
                        let d = sortie.GetDesc().map_err(|e| format!("GetDesc: {e}"))?;
                        let r = d.DesktopCoordinates;
                        if zone.x >= r.left
                            && zone.y >= r.top
                            && zone.x + zone.l <= r.right
                            && zone.y + zone.h <= r.bottom
                        {
                            let nom = nom_adaptateur(&a);
                            let base: IDXGIAdapter =
                                a.cast().map_err(|e| format!("IDXGIAdapter: {e}"))?;
                            let (dev, ctx) = appareil(Some(&base), D3D_DRIVER_TYPE_UNKNOWN)?;
                            let s1: IDXGIOutput1 =
                                sortie.cast().map_err(|e| format!("IDXGIOutput1: {e}"))?;
                            let dupl = s1
                                .DuplicateOutput(&dev)
                                .map_err(|e| format!("DuplicateOutput: {e}"))?;
                            let copieur =
                                Copieur::nouveau(&dev, ctx, zone.l as u32, zone.h as u32)?;
                            let mut s = Dxgi {
                                copieur,
                                dupl,
                                x: (zone.x - r.left) as u32,
                                y: (zone.y - r.top) as u32,
                                derniere: Vec::new(),
                                pointeur_maj: 0,
                                pointeur_invisible: 0,
                                attente_ms,
                            };
                            // Première image : la duplication en rend une tout de suite.
                            let limite = Instant::now() + Duration::from_secs(2);
                            let mut tampon = Vec::new();
                            while s.derniere.is_empty() {
                                if Instant::now() > limite {
                                    return Err("aucune première image en 2 s".into());
                                }
                                s.attente_ms = 200;
                                let _ = s.prendre(&mut tampon)?;
                            }
                            s.attente_ms = attente_ms;
                            return Ok((s, nom));
                        }
                        j += 1;
                    }
                    i += 1;
                }
            }
        }
    }

    impl Source for Dxgi {
        fn prendre(&mut self, dest: &mut Vec<u8>) -> Result<bool, String> {
            let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut res: Option<IDXGIResource> = None;
            // SAFETY : duplication valide ; chaque image acquise est rendue par ReleaseFrame.
            unsafe {
                match self
                    .dupl
                    .AcquireNextFrame(self.attente_ms, &mut info, &mut res)
                {
                    Ok(()) => {}
                    Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => {
                        dest.clone_from(&self.derniere);
                        return Ok(false);
                    }
                    Err(e) => return Err(format!("AcquireNextFrame: {e}")),
                }
                if info.LastMouseUpdateTime != 0 {
                    self.pointeur_maj += 1;
                    if !info.PointerPosition.Visible.as_bool() {
                        self.pointeur_invisible += 1;
                    }
                }
                let neuve = info.LastPresentTime != 0 || self.derniere.is_empty();
                let r = match res {
                    Some(r) if neuve => r
                        .cast::<ID3D11Texture2D>()
                        .map_err(|e| format!("ID3D11Texture2D: {e}"))
                        .and_then(|t| self.copieur.copier(&t, self.x, self.y, dest)),
                    _ => {
                        dest.clone_from(&self.derniere);
                        Ok(())
                    }
                };
                let _ = self.dupl.ReleaseFrame();
                r?;
                if neuve {
                    self.derniere.clone_from(dest);
                }
                Ok(neuve)
            }
        }
    }

    // ---------------------------------------------------------------------------------
    // Moyen 3 : Windows.Graphics.Capture, écran ou fenêtre.

    struct Wgc {
        copieur: Copieur,
        pool: Direct3D11CaptureFramePool,
        session: GraphicsCaptureSession,
        x: u32,
        y: u32,
        derniere: Vec<u8>,
    }

    impl Wgc {
        fn ouvrir(
            zone: Rect,
            fenetre: Option<HWND>,
            options: (bool, bool),
            notes: &mut Vec<String>,
        ) -> Result<Wgc, String> {
            let (avec_cadre, avec_pointeur) = options;
            // SAFETY : initialisation WinRT du fil (MTA), puis objets COM et WinRT comptés.
            unsafe {
                let _ = RoInitialize(RO_INIT_MULTITHREADED);
                notes.push(format!(
                    "wgc_pris_en_charge {}",
                    GraphicsCaptureSession::IsSupported()
                        .map(|b| b as u8 as i32)
                        .unwrap_or(-1)
                ));
                let (dev, ctx) = appareil(None, D3D_DRIVER_TYPE_HARDWARE).or_else(|e| {
                    notes.push(format!("wgc_appareil_materiel_refuse {e}"));
                    appareil(None, D3D_DRIVER_TYPE_WARP)
                })?;
                let dxgi: IDXGIDevice = dev.cast().map_err(|e| format!("IDXGIDevice: {e}"))?;
                if let Ok(a) = dxgi.GetAdapter().and_then(|a| a.cast::<IDXGIAdapter1>()) {
                    notes.push(format!("adaptateur {}", nom_adaptateur(&a)));
                }
                let d3d: IDirect3DDevice = CreateDirect3D11DeviceFromDXGIDevice(&dxgi)
                    .and_then(|i| i.cast())
                    .map_err(|e| format!("CreateDirect3D11DeviceFromDXGIDevice: {e}"))?;
                let interop =
                    windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
                        .map_err(|e| format!("IGraphicsCaptureItemInterop: {e}"))?;
                let (item, x, y): (GraphicsCaptureItem, u32, u32) = match fenetre {
                    Some(h) => (
                        interop
                            .CreateForWindow(h)
                            .map_err(|e| format!("CreateForWindow: {e}"))?,
                        0,
                        0,
                    ),
                    None => {
                        let centre = POINT {
                            x: zone.x + zone.l / 2,
                            y: zone.y + zone.h / 2,
                        };
                        let m = MonitorFromPoint(centre, MONITOR_DEFAULTTONEAREST);
                        let mut mi = MONITORINFO {
                            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                            ..Default::default()
                        };
                        let _ = GetMonitorInfoW(m, &mut mi);
                        (
                            interop
                                .CreateForMonitor(m)
                                .map_err(|e| format!("CreateForMonitor: {e}"))?,
                            (zone.x - mi.rcMonitor.left).max(0) as u32,
                            (zone.y - mi.rcMonitor.top).max(0) as u32,
                        )
                    }
                };
                let taille = item.Size().map_err(|e| format!("Size: {e}"))?;
                notes.push(format!("wgc_taille {}x{}", taille.Width, taille.Height));
                let l = (zone.l as u32).min(taille.Width as u32 - x.min(taille.Width as u32));
                let h = (zone.h as u32).min(taille.Height as u32 - y.min(taille.Height as u32));
                if l == 0 || h == 0 {
                    return Err("zone hors de l'élément capturé".into());
                }
                let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
                    &d3d,
                    DirectXPixelFormat::B8G8R8A8UIntNormalized,
                    2,
                    taille,
                )
                .map_err(|e| format!("CreateFreeThreaded: {e}"))?;
                let session = pool
                    .CreateCaptureSession(&item)
                    .map_err(|e| format!("CreateCaptureSession: {e}"))?;
                if avec_pointeur {
                    notes.push("wgc_sans_pointeur non demandé".into());
                } else {
                    notes.push(match session.SetIsCursorCaptureEnabled(false) {
                        Ok(()) => "wgc_sans_pointeur 1".into(),
                        Err(e) => format!("wgc_sans_pointeur 0 ({e})"),
                    });
                }
                if avec_cadre {
                    notes.push("wgc_sans_cadre non demandé".into());
                } else {
                    notes.push(match session.SetIsBorderRequired(false) {
                        Ok(()) => "wgc_sans_cadre 1".into(),
                        Err(e) => format!("wgc_sans_cadre 0 ({})", e.code()),
                    });
                }
                notes.push(format!(
                    "wgc_cadre_relu {}",
                    session
                        .IsBorderRequired()
                        .map(|b| (b as u8).to_string())
                        .unwrap_or_else(|e| format!("erreur {}", e.code()))
                ));
                let depart = Instant::now();
                session
                    .StartCapture()
                    .map_err(|e| format!("StartCapture: {e}"))?;
                let copieur = Copieur::nouveau(&dev, ctx, l, h)?;
                let mut s = Wgc {
                    copieur,
                    pool,
                    session,
                    x,
                    y,
                    derniere: Vec::new(),
                };
                let limite = Instant::now() + Duration::from_secs(3);
                let mut tampon = Vec::new();
                while s.derniere.is_empty() {
                    if Instant::now() > limite {
                        return Err("aucune première image en 3 s".into());
                    }
                    let _ = s.prendre(&mut tampon)?;
                    std::thread::sleep(Duration::from_millis(1));
                }
                notes.push(format!(
                    "wgc_premiere_image_ms {:.1}",
                    depart.elapsed().as_secs_f64() * 1000.0
                ));
                notes.push(format!("wgc_zone {}x{}", l, h));
                Ok(s)
            }
        }
    }

    impl Source for Wgc {
        fn prendre(&mut self, dest: &mut Vec<u8>) -> Result<bool, String> {
            let Ok(image) = self.pool.TryGetNextFrame() else {
                dest.clone_from(&self.derniere);
                return Ok(false);
            };
            let r = image
                .Surface()
                .and_then(|s| s.cast::<IDirect3DDxgiInterfaceAccess>())
                // SAFETY : interface d'accès d'une surface D3D11 vivante.
                .and_then(|a| unsafe { a.GetInterface::<ID3D11Texture2D>() })
                .map_err(|e| format!("surface: {e}"))
                .and_then(|t| self.copieur.copier(&t, self.x, self.y, dest));
            let _ = image.Close();
            r?;
            self.derniere.clone_from(dest);
            Ok(true)
        }
    }

    impl Drop for Wgc {
        fn drop(&mut self) {
            let _ = self.session.Close();
            let _ = self.pool.Close();
        }
    }

    /// Demande l'accord « sans cadre » (5 s au plus).
    fn acces_sans_cadre() -> String {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            // SAFETY : initialisation WinRT de ce fil.
            unsafe {
                let _ = RoInitialize(RO_INIT_MULTITHREADED);
            }
            let r =
                GraphicsCaptureAccess::RequestAccessAsync(GraphicsCaptureAccessKind::Borderless)
                    .and_then(|op| op.join())
                    .map(|s| format!("{}", s.0))
                    .unwrap_or_else(|e| format!("erreur {}", e.code()));
            let _ = tx.send(r);
        });
        rx.recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|_| "délai de 5 s dépassé".into())
    }

    // ---------------------------------------------------------------------------------

    pub fn principal(a: &HashMap<String, String>) -> Result<(), String> {
        let zone = Rect::lire(a.get("zone").map(String::as_str).unwrap_or(""))?;
        let methode = a.get("methode").cloned().unwrap_or_default();
        println!("methode {methode}");
        println!("zone {},{},{},{}", zone.x, zone.y, zone.l, zone.h);
        if methode == "clignotement" {
            return clignotement(zone, a);
        }
        if methode == "bitblt-detail" {
            return bitblt_detail(zone);
        }
        let images: usize = a.get("images").and_then(|s| s.parse().ok()).unwrap_or(200);
        let intervalle = Duration::from_millis(
            a.get("intervalle")
                .and_then(|s| s.parse().ok())
                .unwrap_or(16),
        );
        let suite = a.get("suite").map(PathBuf::from);
        let chaque: usize = a
            .get("chaque")
            .and_then(|s| s.parse().ok())
            .unwrap_or(10)
            .max(1);
        let suivre = a.contains_key("ressources");
        let r0 = ressources();
        let mut notes = Vec::new();
        let ouverture = Instant::now();
        let mut source: Box<dyn Source> = match methode.as_str() {
            "bitblt" => Box::new(Gdi::ouvrir(zone, false)?),
            "captureblt" => Box::new(Gdi::ouvrir(zone, true)?),
            "dxgi" => {
                let (s, nom) = Dxgi::ouvrir(zone, 0)?;
                notes.push(format!("adaptateur {nom}"));
                Box::new(s)
            }
            "wgc-ecran" | "wgc-fenetre" => {
                let h = if methode == "wgc-fenetre" {
                    let v: isize = a
                        .get("hwnd")
                        .and_then(|s| s.parse().ok())
                        .ok_or("--hwnd manquant")?;
                    Some(HWND(v as *mut core::ffi::c_void))
                } else {
                    None
                };
                notes.push(format!("wgc_acces_sans_cadre {}", acces_sans_cadre()));
                let options = (
                    a.contains_key("avec-cadre"),
                    a.contains_key("avec-pointeur"),
                );
                Box::new(Wgc::ouvrir(zone, h, options, &mut notes)?)
            }
            autre => return Err(format!("méthode inconnue « {autre} »")),
        };
        println!(
            "ouverture_ms {:.1}",
            ouverture.elapsed().as_secs_f64() * 1000.0
        );
        for n in &notes {
            println!("{n}");
        }
        let mut tampon = Vec::new();
        let mut durees = Vec::with_capacity(images);
        let mut durees_neuves = Vec::new();
        let mut neuves = 0usize;
        let r1 = ressources();
        let mut quarts: Vec<(u64, u32)> = Vec::new();
        for i in 0..images {
            if suivre && i > 0 && images >= 4 && i % (images / 4) == 0 && quarts.len() < 3 {
                let r = ressources();
                quarts.push((r.0, r.2));
            }
            let t = Instant::now();
            let neuve = source.prendre(&mut tampon)?;
            let d = t.elapsed();
            durees.push(d);
            if neuve {
                neuves += 1;
                durees_neuves.push(d);
            }
            if let Some(dossier) = &suite {
                if i % chaque == 0 {
                    let l = (tampon.len() / 4 / zone.h as usize) as u32;
                    let _ = enregistrer_png_bgra(
                        &dossier.join(format!("{methode}-{i:04}.png")),
                        &tampon,
                        l,
                        zone.h as u32,
                    );
                }
            }
            if !intervalle.is_zero() {
                std::thread::sleep(intervalle);
            }
        }
        let r2 = ressources();
        let (m, p, x) = statistiques(&durees);
        println!("images {images}");
        println!("neuves {neuves}");
        println!("mediane_ms {m:.2}");
        println!("p95_ms {p:.2}");
        println!("max_ms {x:.2}");
        let (mn, pn, _) = statistiques(&durees_neuves);
        println!("mediane_neuves_ms {mn:.2}");
        println!("p95_neuves_ms {pn:.2}");
        if suivre {
            println!("prive_repos {}", r0.0);
            println!("prive_avant {}", r1.0);
            println!("prive_apres {}", r2.0);
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
            let l = (tampon.len() / 4 / zone.h.max(1) as usize) as u32;
            let h = ((tampon.len() / 4) as u32).checked_div(l).unwrap_or(0);
            enregistrer_png_bgra(&PathBuf::from(png), &tampon, l, h)?;
            println!("png {png} {l}x{h}");
        }
        Ok(())
    }

    /// S2 (b) en détail : temps d'un BitBlt selon la taille et ce qui le précède.
    fn bitblt_detail(zone: Rect) -> Result<(), String> {
        let tailles = [
            Rect {
                x: zone.x,
                y: zone.y,
                l: 100.min(zone.l),
                h: 100.min(zone.h),
            },
            Rect {
                x: zone.x,
                y: zone.y,
                l: 600.min(zone.l),
                h: 400.min(zone.h),
            },
            zone,
        ];
        let mut tampon = Vec::new();
        let mediane = |d: &[Duration]| statistiques(d).0;
        // SAFETY : DwmFlush n'a pas d'argument ; un échec est noté.
        let flush = || unsafe { DwmFlush() };
        let mut d_flush = Vec::new();
        for _ in 0..20 {
            let t = Instant::now();
            let r = flush();
            d_flush.push(t.elapsed());
            if let Err(e) = r {
                println!("dwmflush_erreur {}", e.code());
                break;
            }
        }
        println!("dwmflush_mediane_ms {:.2}", mediane(&d_flush));
        for (k, z) in tailles.iter().enumerate() {
            let nom = format!("{}x{}", z.l, z.h);
            let mut g = Gdi::ouvrir(*z, false)?;
            let t = Instant::now();
            g.prendre(&mut tampon)?;
            println!(
                "detail_{nom}_premiere_ms {:.2}",
                t.elapsed().as_secs_f64() * 1000.0
            );
            for (mode, pause, avec_flush) in [
                ("suite", 0u64, false),
                ("pause16", 16, false),
                ("apres_dwmflush", 0, true),
            ] {
                let mut d = Vec::new();
                for _ in 0..20 {
                    if pause > 0 {
                        std::thread::sleep(Duration::from_millis(pause));
                    }
                    if avec_flush {
                        let _ = flush();
                    }
                    let t = Instant::now();
                    g.prendre(&mut tampon)?;
                    d.push(t.elapsed());
                }
                let (m, p, x) = statistiques(&d);
                println!("detail_{nom}_{mode} {m:.2};{p:.2};{x:.2}");
            }
            if k == tailles.len() - 1 {
                // Avec la minuterie du système à 1 ms (une attente arrondie au pas de 15,6 ms
                // se verrait ici).
                // SAFETY : appariés ; sans effet hors du processus à part la minuterie.
                unsafe {
                    let _ = windows::Win32::Media::timeBeginPeriod(1);
                }
                let mut d = Vec::new();
                for _ in 0..20 {
                    let t = Instant::now();
                    g.prendre(&mut tampon)?;
                    d.push(t.elapsed());
                }
                // SAFETY : idem.
                unsafe {
                    let _ = windows::Win32::Media::timeEndPeriod(1);
                }
                let (m, p, x) = statistiques(&d);
                println!("detail_{nom}_minuterie1ms {m:.2};{p:.2};{x:.2}");
            }
        }
        Ok(())
    }

    /// S2 (f) : pendant `duree-ms`, un fil copie la zone en boucle (avec ou sans
    /// CAPTUREBLT) ; ce fil-ci lit `GetCursorInfo` toutes les 0,5 ms et compte les lectures
    /// où le pointeur est caché ; un troisième suit les mises à jour du pointeur vues par
    /// la duplication DXGI (une autre voie que GDI), s'il y en a une.
    fn clignotement(zone: Rect, a: &HashMap<String, String>) -> Result<(), String> {
        let duree = Duration::from_millis(
            a.get("duree-ms")
                .and_then(|s| s.parse().ok())
                .unwrap_or(3000),
        );
        let captureblt = a.contains_key("captureblt");
        println!("captureblt {}", captureblt as u8);
        let fin = Arc::new(AtomicBool::new(false));
        let copies = {
            let fin = Arc::clone(&fin);
            std::thread::spawn(move || -> Result<usize, String> {
                let mut g = Gdi::ouvrir(zone, captureblt)?;
                let mut t = Vec::new();
                let mut n = 0;
                while !fin.load(Ordering::SeqCst) {
                    g.prendre(&mut t)?;
                    n += 1;
                }
                Ok(n)
            })
        };
        let dxgi = {
            let fin = Arc::clone(&fin);
            std::thread::spawn(move || -> Result<(u32, u32), String> {
                let (mut d, _) = Dxgi::ouvrir(zone, 50)?;
                let mut t = Vec::new();
                while !fin.load(Ordering::SeqCst) {
                    d.prendre(&mut t)?;
                }
                Ok((d.pointeur_maj, d.pointeur_invisible))
            })
        };
        let debut = Instant::now();
        let mut lectures = 0u64;
        let mut cache = 0u64;
        while debut.elapsed() < duree {
            let mut ci = CURSORINFO {
                cbSize: std::mem::size_of::<CURSORINFO>() as u32,
                ..Default::default()
            };
            // SAFETY : `ci` est de la taille annoncée.
            if unsafe { GetCursorInfo(&mut ci) }.is_ok() {
                lectures += 1;
                if ci.flags.0 & CURSOR_SHOWING.0 == 0 {
                    cache += 1;
                }
            }
            std::thread::sleep(Duration::from_micros(500));
        }
        fin.store(true, Ordering::SeqCst);
        match copies.join() {
            Ok(Ok(n)) => println!("copies {n}"),
            Ok(Err(e)) => println!("copies_erreur {e}"),
            Err(_) => println!("copies_erreur panique"),
        }
        println!("lectures_pointeur {lectures}");
        println!("lectures_pointeur_cache {cache}");
        match dxgi.join() {
            Ok(Ok((maj, inv))) => {
                println!("dxgi_pointeur_maj {maj}");
                println!("dxgi_pointeur_invisible {inv}");
            }
            Ok(Err(e)) => println!("dxgi_erreur {e}"),
            Err(_) => println!("dxgi_erreur panique"),
        }
        Ok(())
    }
}
