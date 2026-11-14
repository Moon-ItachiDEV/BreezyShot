//! Moyens de prise d'images de S1, repris pour le fil de prise de S2 (point 10). Windows
//! seulement.
//!
//! - `Gdi` : `BitBlt` comme `systeme::vivant` de l'app (DC de l'écran prêté par GetDC à
//!   chaque prise, SRCCOPY sans CAPTUREBLT, section DIB réutilisée, GdiFlush).
//! - `Dxgi` : la duplication de bureau, reprise de l'exemple `vivant` de la sonde 0.6
//!   (`sonde-defilante.patch`) : le périphérique Direct3D est créé sur la carte qui porte
//!   l'écran de la zone (on parcourt les cartes et leurs sorties) ; `AcquireNextFrame` à
//!   délai nul ; « rien n'a changé dans la zone » lu sur les rectangles sales et déplacés
//!   croisés avec la zone ; une perte d'accès est comptée et la duplication refaite (carte,
//!   périphérique et sortie relus) au plus toutes les 500 ms.
//! - `Wgc` : Windows.Graphics.Capture de l'écran entier (`CreateForMonitor`), périphérique
//!   imposé sur la carte de l'écran, zone découpée ensuite comme pour la duplication (repris
//!   du moyen `wgc-ecran` de la sonde 0.6).

use crate::Rect;
use std::time::{Duration, Instant};
use windows::core::Interface;
use windows::Graphics::Capture::{
    Direct3D11CaptureFramePool, GraphicsCaptureAccess, GraphicsCaptureAccessKind,
    GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::{HMODULE, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_UNKNOWN};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::*;

pub fn hr(e: &windows::core::Error) -> String {
    crate::hresult(e.code().0, &e.message())
}

/// Millisecondes écoulées depuis `t`.
pub fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// Temps des étapes d'une prise (ms) ; seules celles qui ont eu lieu sont remplies.
#[derive(Default, Clone, Copy)]
pub struct Etapes {
    pub acquisition: Option<f64>,
    pub copie: Option<f64>,
    pub lecture: Option<f64>,
}

pub trait Source {
    /// Une image BGRA de la zone dans `dest` ; vrai si elle est neuve.
    fn prendre(&mut self, dest: &mut Vec<u8>, e: &mut Etapes) -> Result<bool, String>;
    /// (pertes d'accès, temps de chaque reprise en ms depuis la perte).
    fn pertes(&self) -> (u32, Vec<f64>) {
        (0, Vec::new())
    }
    /// Lignes « cle valeur » propres au moyen, à la fin d'une prise.
    fn bilan(&self) -> Vec<String> {
        Vec::new()
    }
}

// ---------------------------------------------------------------------------------------
// Moyen 1 : BitBlt, comme `systeme::vivant` (0.6).

pub struct Gdi {
    dc: HDC,
    bitmap: HBITMAP,
    ancien: HGDIOBJ,
    bits: *const u8,
    zone: Rect,
}

impl Gdi {
    pub fn ouvrir(zone: Rect) -> Result<Gdi, String> {
        // SAFETY : objets GDI créés ici, rendus par `drop`.
        unsafe {
            let dc = CreateCompatibleDC(None);
            if dc.is_invalid() {
                return Err("CreateCompatibleDC a échoué".into());
            }
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
                .map_err(|e| format!("CreateDIBSection: {}", hr(&e)))?;
            let ancien = SelectObject(dc, bitmap.into());
            Ok(Gdi {
                dc,
                bitmap,
                ancien,
                bits: bits as *const u8,
                zone,
            })
        }
    }
}

impl Source for Gdi {
    fn prendre(&mut self, dest: &mut Vec<u8>, e: &mut Etapes) -> Result<bool, String> {
        let z = self.zone;
        // SAFETY : DC valides ; la section fait l × h × 4 octets.
        unsafe {
            let t = Instant::now();
            let ecran = GetDC(None);
            if ecran.is_invalid() {
                return Err("GetDC a échoué".into());
            }
            let r = BitBlt(self.dc, 0, 0, z.l, z.h, Some(ecran), z.x, z.y, SRCCOPY);
            ReleaseDC(None, ecran);
            r.map_err(|e| format!("BitBlt: {}", hr(&e)))?;
            let _ = GdiFlush();
            e.copie = Some(ms(t));
            let t = Instant::now();
            let n = (z.l * z.h * 4) as usize;
            dest.resize(n, 0);
            std::ptr::copy_nonoverlapping(self.bits, dest.as_mut_ptr(), n);
            e.lecture = Some(ms(t));
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
        }
    }
}

// ---------------------------------------------------------------------------------------
// Copie d'une texture D3D11 (sous-rectangle) vers la mémoire.

pub struct Copieur {
    ctx: ID3D11DeviceContext,
    staging: ID3D11Texture2D,
    l: u32,
    h: u32,
}

impl Copieur {
    pub fn nouveau(
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
            .map_err(|e| format!("CreateTexture2D: {}", hr(&e)))?;
        Ok(Copieur {
            ctx,
            staging: t.ok_or("texture absente")?,
            l,
            h,
        })
    }

    pub fn copier(
        &self,
        source: &ID3D11Texture2D,
        x: u32,
        y: u32,
        dest: &mut Vec<u8>,
        e: &mut Etapes,
    ) -> Result<(), String> {
        let boite = D3D11_BOX {
            left: x,
            top: y,
            front: 0,
            right: x + self.l,
            bottom: y + self.h,
            back: 1,
        };
        // SAFETY : textures du même appareil ; la boîte est dans la source ; la carte est
        // lue ligne par ligne selon RowPitch puis rendue.
        unsafe {
            let t = Instant::now();
            self.ctx
                .CopySubresourceRegion(&self.staging, 0, 0, 0, 0, source, 0, Some(&boite));
            e.copie = Some(ms(t));
            let t = Instant::now();
            let mut m = D3D11_MAPPED_SUBRESOURCE::default();
            self.ctx
                .Map(&self.staging, 0, D3D11_MAP_READ, 0, Some(&mut m))
                .map_err(|e| format!("Map: {}", hr(&e)))?;
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
            e.lecture = Some(ms(t));
        }
        Ok(())
    }
}

pub fn appareil(
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
    .map_err(|e| format!("D3D11CreateDevice(type {}): {}", genre.0, hr(&e)))?;
    Ok((d.ok_or("appareil absent")?, c.ok_or("contexte absent")?))
}

pub fn nom_adaptateur(a: &IDXGIAdapter1) -> String {
    // SAFETY : lecture de la description.
    unsafe { a.GetDesc1() }
        .map(|d| {
            let n = d
                .Description
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(d.Description.len());
            let logiciel = d.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0;
            format!(
                "{} (vendeur 0x{:04X}, appareil 0x{:04X}, mémoire dédiée {} Mo, {})",
                String::from_utf16_lossy(&d.Description[..n]),
                d.VendorId,
                d.DeviceId,
                d.DedicatedVideoMemory / (1024 * 1024),
                if logiciel {
                    "pilote logiciel"
                } else if d.VendorId == 0x1414 {
                    "pilote Microsoft (rendu de base ou affichage seul)"
                } else {
                    "pilote matériel"
                }
            )
        })
        .unwrap_or_else(|e| format!("? ({})", hr(&e)))
}

/// La carte qui porte l'écran de la zone, et cette sortie.
pub struct Carte {
    pub adaptateur: IDXGIAdapter1,
    pub sortie: IDXGIOutput1,
    pub bureau: RECT,
    pub nom: String,
}

/// Parcourt les cartes et leurs sorties (fabrique neuve : à jour après un changement de
/// mode) ; garde celle dont le rectangle de bureau contient la zone.
pub fn carte_de_zone(zone: Rect) -> Result<Carte, String> {
    // SAFETY : énumération DXGI ; objets COM comptés.
    unsafe {
        let fabrique: IDXGIFactory1 =
            CreateDXGIFactory1().map_err(|e| format!("CreateDXGIFactory1: {}", hr(&e)))?;
        let mut i = 0;
        loop {
            let a = fabrique.EnumAdapters1(i).map_err(|e| {
                format!(
                    "aucune carte n'a d'écran qui contienne la zone {},{},{},{} ({})",
                    zone.x,
                    zone.y,
                    zone.l,
                    zone.h,
                    hr(&e)
                )
            })?;
            let mut j = 0;
            while let Ok(sortie) = a.EnumOutputs(j) {
                let d = sortie
                    .GetDesc()
                    .map_err(|e| format!("GetDesc: {}", hr(&e)))?;
                let r = d.DesktopCoordinates;
                if zone.x >= r.left
                    && zone.y >= r.top
                    && zone.x + zone.l <= r.right
                    && zone.y + zone.h <= r.bottom
                {
                    let s1: IDXGIOutput1 = sortie
                        .cast()
                        .map_err(|e| format!("IDXGIOutput1: {}", hr(&e)))?;
                    return Ok(Carte {
                        nom: nom_adaptateur(&a),
                        adaptateur: a,
                        sortie: s1,
                        bureau: r,
                    });
                }
                j += 1;
            }
            i += 1;
        }
    }
}

// ---------------------------------------------------------------------------------------
// Moyen 2 : duplication DXGI de l'écran qui contient la zone.

/// Temps de l'ouverture (ms).
#[derive(Default, Clone, Copy)]
pub struct Ouverture {
    pub carte_ms: f64,
    pub appareil_ms: f64,
    pub duplication_ms: f64,
    pub premiere_ms: f64,
}

pub struct Dxgi {
    zone: Rect,
    copieur: Copieur,
    device: ID3D11Device,
    sortie: IDXGIOutput1,
    dupl: Option<IDXGIOutputDuplication>,
    x: u32,
    y: u32,
    derniere: Vec<u8>,
    attente_ms: u32,
    rectangles: bool,
    perdu_depuis: Option<Instant>,
    prochain_essai: Instant,
    pub pertes: u32,
    pub refaites_ms: Vec<f64>,
    pub autres_erreurs: u32,
    /// Délai dépassé sans image.
    pub sans_image: u32,
    /// Image rendue sans changement de l'écran (pointeur seul).
    pub pointeur_seul: u32,
    /// Images neuves de l'écran.
    pub ecran_neuves: u32,
    /// Images neuves de l'écran dont aucun rectangle sale ni déplacé ne touche la zone.
    pub zone_intacte: u32,
    /// Images sans métadonnées (rectangles non lus : copiées par prudence).
    pub sans_metadonnees: u32,
    /// Événements datés (ms depuis l'ouverture) : pertes, reprises, refus.
    pub journal: Vec<String>,
    pub ouverture: Ouverture,
    pub nom: String,
    depart: Instant,
}

impl Dxgi {
    /// `attente_ms` : délai de AcquireNextFrame (0 : sans attendre) ; `rectangles` : lire
    /// les rectangles sales et déplacés pour savoir si la zone a changé.
    pub fn ouvrir(zone: Rect, attente_ms: u32, rectangles: bool) -> Result<Dxgi, String> {
        let mut o = Ouverture::default();
        let t = Instant::now();
        let carte = carte_de_zone(zone)?;
        o.carte_ms = ms(t);
        let base: IDXGIAdapter = carte
            .adaptateur
            .cast()
            .map_err(|e| format!("IDXGIAdapter: {}", hr(&e)))?;
        let t = Instant::now();
        let (dev, ctx) = appareil(Some(&base), D3D_DRIVER_TYPE_UNKNOWN)?;
        o.appareil_ms = ms(t);
        let t = Instant::now();
        // SAFETY : sortie et appareil vivants, de la même carte.
        let dupl = unsafe { carte.sortie.DuplicateOutput(&dev) }
            .map_err(|e| format!("DuplicateOutput: {}", hr(&e)))?;
        o.duplication_ms = ms(t);
        let copieur = Copieur::nouveau(&dev, ctx, zone.l as u32, zone.h as u32)?;
        let mut s = Dxgi {
            zone,
            copieur,
            device: dev,
            sortie: carte.sortie,
            dupl: Some(dupl),
            x: (zone.x - carte.bureau.left) as u32,
            y: (zone.y - carte.bureau.top) as u32,
            derniere: Vec::new(),
            attente_ms: 200,
            rectangles,
            perdu_depuis: None,
            prochain_essai: Instant::now(),
            pertes: 0,
            refaites_ms: Vec::new(),
            autres_erreurs: 0,
            sans_image: 0,
            pointeur_seul: 0,
            ecran_neuves: 0,
            zone_intacte: 0,
            sans_metadonnees: 0,
            journal: Vec::new(),
            ouverture: o,
            nom: carte.nom,
            depart: Instant::now(),
        };
        // Première image : la duplication en rend une tout de suite (l'écran entier).
        let t = Instant::now();
        let limite = t + Duration::from_secs(2);
        let mut tampon = Vec::new();
        let mut e = Etapes::default();
        while s.derniere.is_empty() {
            if Instant::now() > limite {
                return Err("aucune première image en 2 s".into());
            }
            let _ = s.prendre(&mut tampon, &mut e)?;
        }
        s.ouverture.premiere_ms = ms(t);
        s.attente_ms = attente_ms;
        s.sans_image = 0;
        s.pointeur_seul = 0;
        s.ecran_neuves = 0;
        s.zone_intacte = 0;
        s.sans_metadonnees = 0;
        Ok(s)
    }

    fn noter(&mut self, quoi: String) {
        let t = ms(self.depart);
        self.journal.push(format!("{t:.0} ms : {quoi}"));
    }

    /// Carte, périphérique, sortie et duplication refaits.
    fn rouvrir(&mut self) -> Result<(), String> {
        let carte = carte_de_zone(self.zone)?;
        let base: IDXGIAdapter = carte
            .adaptateur
            .cast()
            .map_err(|e| format!("IDXGIAdapter: {}", hr(&e)))?;
        let (dev, ctx) = appareil(Some(&base), D3D_DRIVER_TYPE_UNKNOWN)?;
        // SAFETY : sortie et appareil vivants, de la même carte.
        let dupl = unsafe { carte.sortie.DuplicateOutput(&dev) }
            .map_err(|e| format!("DuplicateOutput: {}", hr(&e)))?;
        self.copieur = Copieur::nouveau(&dev, ctx, self.zone.l as u32, self.zone.h as u32)?;
        self.device = dev;
        self.sortie = carte.sortie;
        self.dupl = Some(dupl);
        self.x = (self.zone.x - carte.bureau.left) as u32;
        self.y = (self.zone.y - carte.bureau.top) as u32;
        Ok(())
    }

    /// Un rectangle sale ou déplacé de l'image touche-t-il la zone ? `None` : pas lu.
    fn zone_touchee(
        &self,
        dupl: &IDXGIOutputDuplication,
        info: &DXGI_OUTDUPL_FRAME_INFO,
    ) -> Option<bool> {
        let taille = info.TotalMetadataBufferSize as usize;
        if taille == 0 {
            return None;
        }
        let z = RECT {
            left: self.x as i32,
            top: self.y as i32,
            right: (self.x + self.copieur.l) as i32,
            bottom: (self.y + self.copieur.h) as i32,
        };
        let coupe =
            |r: &RECT| r.left < z.right && r.right > z.left && r.top < z.bottom && r.bottom > z.top;
        let n_dep = taille / std::mem::size_of::<DXGI_OUTDUPL_MOVE_RECT>() + 1;
        let mut dep = vec![DXGI_OUTDUPL_MOVE_RECT::default(); n_dep];
        let mut requis = 0u32;
        // SAFETY : tampons alignés, de la taille annoncée ; image acquise et pas rendue.
        unsafe {
            dupl.GetFrameMoveRects(
                (n_dep * std::mem::size_of::<DXGI_OUTDUPL_MOVE_RECT>()) as u32,
                dep.as_mut_ptr(),
                &mut requis,
            )
            .ok()?;
        }
        let n = requis as usize / std::mem::size_of::<DXGI_OUTDUPL_MOVE_RECT>();
        for m in &dep[..n.min(dep.len())] {
            let d = m.DestinationRect;
            let s = RECT {
                left: m.SourcePoint.x,
                top: m.SourcePoint.y,
                right: m.SourcePoint.x + (d.right - d.left),
                bottom: m.SourcePoint.y + (d.bottom - d.top),
            };
            if coupe(&d) || coupe(&s) {
                return Some(true);
            }
        }
        let n_sales = taille / std::mem::size_of::<RECT>() + 1;
        let mut sales = vec![RECT::default(); n_sales];
        // SAFETY : idem.
        unsafe {
            dupl.GetFrameDirtyRects(
                (n_sales * std::mem::size_of::<RECT>()) as u32,
                sales.as_mut_ptr(),
                &mut requis,
            )
            .ok()?;
        }
        let n = requis as usize / std::mem::size_of::<RECT>();
        Some(sales[..n.min(sales.len())].iter().any(coupe))
    }
}

impl Source for Dxgi {
    fn prendre(&mut self, dest: &mut Vec<u8>, e: &mut Etapes) -> Result<bool, String> {
        if self.dupl.is_none() {
            if Instant::now() < self.prochain_essai {
                dest.clone_from(&self.derniere);
                return Ok(false);
            }
            match self.rouvrir() {
                Ok(()) => {
                    let d = self.perdu_depuis.take().map(ms).unwrap_or(0.0);
                    self.refaites_ms.push(d);
                    self.noter(format!("duplication refaite, {d:.0} ms après la perte"));
                }
                Err(x) => {
                    self.noter(format!("duplication pas refaite : {x}"));
                    self.prochain_essai = Instant::now() + Duration::from_millis(500);
                    dest.clone_from(&self.derniere);
                    return Ok(false);
                }
            }
        }
        let Some(dupl) = self.dupl.clone() else {
            dest.clone_from(&self.derniere);
            return Ok(false);
        };
        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut res: Option<IDXGIResource> = None;
        let t = Instant::now();
        // SAFETY : duplication valide ; chaque image acquise est rendue par ReleaseFrame.
        unsafe {
            match dupl.AcquireNextFrame(self.attente_ms, &mut info, &mut res) {
                Ok(()) => {}
                Err(x) if x.code() == DXGI_ERROR_WAIT_TIMEOUT => {
                    e.acquisition = Some(ms(t));
                    self.sans_image += 1;
                    dest.clone_from(&self.derniere);
                    return Ok(false);
                }
                Err(x) => {
                    if x.code() == DXGI_ERROR_ACCESS_LOST {
                        self.pertes += 1;
                        self.noter("DXGI_ERROR_ACCESS_LOST".into());
                    } else {
                        self.autres_erreurs += 1;
                        self.noter(format!("AcquireNextFrame: {}", hr(&x)));
                    }
                    self.dupl = None;
                    self.perdu_depuis = Some(Instant::now());
                    self.prochain_essai = Instant::now();
                    dest.clone_from(&self.derniere);
                    return Ok(false);
                }
            }
            e.acquisition = Some(ms(t));
            let mut neuve = info.LastPresentTime != 0 || self.derniere.is_empty();
            if !neuve {
                self.pointeur_seul += 1;
            } else if !self.derniere.is_empty() {
                self.ecran_neuves += 1;
                if self.rectangles {
                    match self.zone_touchee(&dupl, &info) {
                        Some(false) => {
                            self.zone_intacte += 1;
                            neuve = false;
                        }
                        Some(true) => {}
                        None => self.sans_metadonnees += 1,
                    }
                }
            }
            let r = match res {
                Some(r) if neuve => r
                    .cast::<ID3D11Texture2D>()
                    .map_err(|e| format!("ID3D11Texture2D: {}", hr(&e)))
                    .and_then(|t| self.copieur.copier(&t, self.x, self.y, dest, e)),
                _ => {
                    dest.clone_from(&self.derniere);
                    Ok(())
                }
            };
            let _ = dupl.ReleaseFrame();
            r?;
            if neuve {
                self.derniere.clone_from(dest);
            }
            Ok(neuve)
        }
    }
    fn pertes(&self) -> (u32, Vec<f64>) {
        (self.pertes, self.refaites_ms.clone())
    }
    fn bilan(&self) -> Vec<String> {
        let mut v = vec![
            format!("dxgi_ecran_neuves {}", self.ecran_neuves),
            format!("dxgi_zone_intacte {}", self.zone_intacte),
            format!("dxgi_sans_metadonnees {}", self.sans_metadonnees),
            format!("dxgi_pointeur_seul {}", self.pointeur_seul),
            format!("dxgi_delai_depasse {}", self.sans_image),
            format!("dxgi_autres_erreurs {}", self.autres_erreurs),
        ];
        for j in &self.journal {
            v.push(format!("dxgi_journal {j}"));
        }
        v
    }
}

// ---------------------------------------------------------------------------------------
// Moyen 3 : Windows.Graphics.Capture de l'écran entier, zone découpée.

#[derive(Default, Clone, Copy)]
pub struct OptionsWgc {
    /// SetIsBorderRequired(false).
    pub sans_cadre: bool,
    /// RequestAccessAsync(Borderless) attendu avant la session.
    pub acces: bool,
    /// IsCursorCaptureEnabled posé à cette valeur (None : laissé).
    pub pointeur: Option<bool>,
}

pub struct Wgc {
    copieur: Copieur,
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    x: u32,
    y: u32,
    derniere: Vec<u8>,
    /// Lignes « cle valeur » de l'ouverture.
    pub notes: Vec<String>,
}

impl Wgc {
    pub fn ouvrir(zone: Rect, o: OptionsWgc) -> Result<Wgc, String> {
        let mut notes = Vec::new();
        if o.acces {
            notes.push(format!("wgc_acces_sans_cadre {}", acces_sans_cadre()));
        }
        // SAFETY : initialisation WinRT du fil (MTA), puis objets COM et WinRT comptés.
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
            notes.push(format!(
                "wgc_pris_en_charge {}",
                GraphicsCaptureSession::IsSupported()
                    .map(|b| b as u8 as i32)
                    .unwrap_or(-1)
            ));
            let carte = carte_de_zone(zone)?;
            notes.push(format!("adaptateur {}", carte.nom));
            let base: IDXGIAdapter = carte
                .adaptateur
                .cast()
                .map_err(|e| format!("IDXGIAdapter: {}", hr(&e)))?;
            let (dev, ctx) = appareil(Some(&base), D3D_DRIVER_TYPE_UNKNOWN)?;
            let dxgi: IDXGIDevice = dev.cast().map_err(|e| format!("IDXGIDevice: {}", hr(&e)))?;
            let d3d: IDirect3DDevice = CreateDirect3D11DeviceFromDXGIDevice(&dxgi)
                .and_then(|i| i.cast())
                .map_err(|e| format!("CreateDirect3D11DeviceFromDXGIDevice: {}", hr(&e)))?;
            let interop =
                windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
                    .map_err(|e| format!("IGraphicsCaptureItemInterop: {}", hr(&e)))?;
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
            let item: GraphicsCaptureItem = interop
                .CreateForMonitor(m)
                .map_err(|e| format!("CreateForMonitor: {}", hr(&e)))?;
            let x = (zone.x - mi.rcMonitor.left).max(0) as u32;
            let y = (zone.y - mi.rcMonitor.top).max(0) as u32;
            let taille = item.Size().map_err(|e| format!("Size: {}", hr(&e)))?;
            notes.push(format!("wgc_taille {}x{}", taille.Width, taille.Height));
            let l = (zone.l as u32).min((taille.Width as u32).saturating_sub(x));
            let h = (zone.h as u32).min((taille.Height as u32).saturating_sub(y));
            if l == 0 || h == 0 {
                return Err("zone hors de l'écran capturé".into());
            }
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
            if let Some(p) = o.pointeur {
                notes.push(match session.SetIsCursorCaptureEnabled(p) {
                    Ok(()) => format!("wgc_pointeur_pose {}", p as u8),
                    Err(e) => format!("wgc_pointeur_pose refusé ({})", hr(&e)),
                });
            }
            if o.sans_cadre {
                notes.push(match session.SetIsBorderRequired(false) {
                    Ok(()) => "wgc_sans_cadre 1".into(),
                    Err(e) => format!("wgc_sans_cadre 0 ({})", hr(&e)),
                });
            }
            notes.push(format!(
                "wgc_cadre_relu {}",
                session
                    .IsBorderRequired()
                    .map(|b| (b as u8).to_string())
                    .unwrap_or_else(|e| format!("erreur {}", hr(&e)))
            ));
            let depart = Instant::now();
            session
                .StartCapture()
                .map_err(|e| format!("StartCapture: {}", hr(&e)))?;
            let copieur = Copieur::nouveau(&dev, ctx, l, h)?;
            let mut s = Wgc {
                copieur,
                pool,
                session,
                x,
                y,
                derniere: Vec::new(),
                notes: Vec::new(),
            };
            let limite = Instant::now() + Duration::from_secs(3);
            let mut tampon = Vec::new();
            while s.derniere.is_empty() {
                if Instant::now() > limite {
                    return Err("aucune première image en 3 s".into());
                }
                let _ = s.prendre(&mut tampon, &mut Etapes::default())?;
                std::thread::sleep(Duration::from_millis(1));
            }
            notes.push(format!("wgc_premiere_image_ms {:.1}", ms(depart)));
            notes.push(format!("wgc_zone {l}x{h}"));
            s.notes = notes;
            Ok(s)
        }
    }
}

impl Source for Wgc {
    fn prendre(&mut self, dest: &mut Vec<u8>, e: &mut Etapes) -> Result<bool, String> {
        let t = Instant::now();
        let Ok(image) = self.pool.TryGetNextFrame() else {
            e.acquisition = Some(ms(t));
            dest.clone_from(&self.derniere);
            return Ok(false);
        };
        e.acquisition = Some(ms(t));
        let r = image
            .Surface()
            .and_then(|s| s.cast::<IDirect3DDxgiInterfaceAccess>())
            // SAFETY : interface d'accès d'une surface D3D11 vivante.
            .and_then(|a| unsafe { a.GetInterface::<ID3D11Texture2D>() })
            .map_err(|x| format!("surface: {}", hr(&x)))
            .and_then(|t| self.copieur.copier(&t, self.x, self.y, dest, e));
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

/// Demande l'accord « sans cadre » et l'attend (5 s au plus).
pub fn acces_sans_cadre() -> String {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        // SAFETY : initialisation WinRT de ce fil.
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
        }
        let r = GraphicsCaptureAccess::RequestAccessAsync(GraphicsCaptureAccessKind::Borderless)
            .and_then(|op| op.join())
            .map(|s| {
                let nom = match s.0 {
                    0 => "DeniedBySystem",
                    1 => "NotDeclaredByApp",
                    2 => "DeniedByUser",
                    3 => "UserPromptRequired",
                    4 => "Allowed",
                    _ => "?",
                };
                format!("{} ({nom})", s.0)
            })
            .unwrap_or_else(|e| format!("erreur {}", hr(&e)));
        let _ = tx.send(r);
    });
    rx.recv_timeout(Duration::from_secs(5))
        .unwrap_or_else(|_| "délai de 5 s dépassé".into())
}

// ---------------------------------------------------------------------------------------
// Pointeur : `systeme::gel::lire_pointeur` (GetCursorInfo, GetIconInfo, la forme peinte
// sur noir et sur blanc dans une section DIB).

/// Rend (montré, durée de GetCursorInfo seul en ms, durée de la forme en ms ou NaN).
/// `forcer_forme` : la forme de la flèche est lue même si le pointeur est caché.
pub fn lire_pointeur(forcer_forme: bool) -> (bool, f64, f64) {
    let t = Instant::now();
    let mut info = CURSORINFO {
        cbSize: std::mem::size_of::<CURSORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY : `info` est de la taille annoncée.
    let ok = unsafe { GetCursorInfo(&mut info) }.is_ok();
    let montre = ok && info.flags.0 & CURSOR_SHOWING.0 != 0 && !info.hCursor.is_invalid();
    let t_info = ms(t);
    if !montre && !forcer_forme {
        return (false, t_info, f64::NAN);
    }
    let t = Instant::now();
    // SAFETY : objets GDI créés et rendus ici ; l'icône est celle du système.
    unsafe {
        let icone = if montre {
            HICON(info.hCursor.0)
        } else {
            match LoadCursorW(None, IDC_ARROW) {
                Ok(c) => HICON(c.0),
                Err(_) => return (montre, t_info, f64::NAN),
            }
        };
        let mut ii = ICONINFO::default();
        if GetIconInfo(icone, &mut ii).is_err() {
            return (montre, t_info, f64::NAN);
        }
        let mut bm = BITMAP::default();
        let _ = GetObjectW(
            ii.hbmMask.into(),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bm as *mut _ as *mut core::ffi::c_void),
        );
        let l = bm.bmWidth.max(1);
        let h = if ii.hbmColor.is_invalid() {
            (bm.bmHeight / 2).max(1)
        } else {
            bm.bmHeight.max(1)
        };
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
        if let Ok(b) = CreateDIBSection(Some(dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0) {
            let ancien = SelectObject(dc, b.into());
            for fond in [0u8, 255u8] {
                if !bits.is_null() {
                    std::ptr::write_bytes(bits as *mut u8, fond, (l * h * 4) as usize);
                }
                let _ = DrawIconEx(dc, 0, 0, icone, l, h, 0, None, DI_NORMAL);
                let _ = GdiFlush();
            }
            SelectObject(dc, ancien);
            let _ = DeleteObject(b.into());
        }
        let _ = DeleteDC(dc);
        if !ii.hbmMask.is_invalid() {
            let _ = DeleteObject(ii.hbmMask.into());
        }
        if !ii.hbmColor.is_invalid() {
            let _ = DeleteObject(ii.hbmColor.into());
        }
    }
    (montre, t_info, ms(t))
}

// ---------------------------------------------------------------------------------------
// Écrans (`systeme::gel::ecrans` de l'app, recopiée) et mode d'affichage.

unsafe extern "system" fn rappel_ecran(
    moniteur: HMONITOR,
    _dc: HDC,
    _rect: *mut RECT,
    donnee: LPARAM,
) -> windows::core::BOOL {
    // SAFETY : `donnee` est l'adresse du `Vec` de `ecrans()`, vivant pendant l'énumération.
    let liste = unsafe { &mut *(donnee.0 as *mut Vec<String>) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY : `info` est un MONITORINFO dont la taille est renseignée.
    if unsafe { GetMonitorInfoW(moniteur, &mut info) }.as_bool() {
        let (mut dx, mut dy) = (0u32, 0u32);
        // SAFETY : deux u32 en sortie.
        let lu = unsafe { GetDpiForMonitor(moniteur, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) };
        let r = info.rcMonitor;
        let u = info.rcWork;
        liste.push(format!(
            "{},{},{},{} utile {},{},{},{} échelle {:.2} principal {}",
            r.left,
            r.top,
            r.right - r.left,
            r.bottom - r.top,
            u.left,
            u.top,
            u.right - u.left,
            u.bottom - u.top,
            if lu.is_ok() { dx as f64 / 96.0 } else { 0.0 },
            (info.dwFlags & MONITORINFOF_PRIMARY != 0) as u8
        ));
    }
    windows::core::BOOL(1)
}

/// Les écrans comme les voit `gel::ecrans` (EnumDisplayMonitors, GetMonitorInfoW,
/// GetDpiForMonitor) : « x,y,l,h utile x,y,l,h échelle e principal 0|1 ».
pub fn ecrans() -> Vec<String> {
    let mut liste: Vec<String> = Vec::new();
    // SAFETY : le rappel ne garde pas l'adresse de `liste` au-delà de l'appel.
    let _ = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(rappel_ecran),
            LPARAM(&mut liste as *mut Vec<String> as isize),
        )
    };
    liste
}

/// Pose dynamiquement le mode l × h de l'écran principal (ChangeDisplaySettingsExW, sans
/// l'écrire au registre) ; rend le code DISP_CHANGE (0 : fait).
pub fn changer_mode(l: u32, h: u32) -> i32 {
    // SAFETY : DEVMODEW dont la taille est renseignée ; écran principal.
    unsafe {
        let mut d = DEVMODEW {
            dmSize: std::mem::size_of::<DEVMODEW>() as u16,
            ..Default::default()
        };
        if !EnumDisplaySettingsW(None, ENUM_CURRENT_SETTINGS, &mut d).as_bool() {
            return -100;
        }
        d.dmPelsWidth = l;
        d.dmPelsHeight = h;
        d.dmFields = DM_PELSWIDTH | DM_PELSHEIGHT;
        ChangeDisplaySettingsExW(None, Some(&d), None, CDS_TYPE(0), None).0
    }
}

/// Remet le mode du registre (celui d'avant `changer_mode`).
pub fn remettre_mode() -> i32 {
    // SAFETY : sans mode : retour au mode enregistré.
    unsafe { ChangeDisplaySettingsExW(None, None, None, CDS_TYPE(0), None).0 }
}

/// Mode courant « LxH ».
pub fn mode_courant() -> String {
    // SAFETY : DEVMODEW dont la taille est renseignée.
    unsafe {
        let mut d = DEVMODEW {
            dmSize: std::mem::size_of::<DEVMODEW>() as u16,
            ..Default::default()
        };
        if EnumDisplaySettingsW(None, ENUM_CURRENT_SETTINGS, &mut d).as_bool() {
            format!("{}x{}", d.dmPelsWidth, d.dmPelsHeight)
        } else {
            "illisible".into()
        }
    }
}
