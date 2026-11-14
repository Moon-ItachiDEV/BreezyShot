//! S2 (Media Foundation, codeur H.264, fichier MP4) et S3 (piste son, micro). Windows
//! seulement.
//!
//!   mf --etape liste                     codeurs et décodeurs H.264 et AAC (MFTEnumEx)
//!   mf --etape tailles --dossier D        un .mp4 par taille : accepté ou refusé (HRESULT)
//!   mf --etape materiel --dossier D       MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS 1 puis 0
//!   mf --etape vitesse --genre simple|fixe|plein --taille 600x450 --duree 20 --sortie f.mp4
//!                                         30 i/s en temps réel : WriteSample, processeur
//!   mf --etape reglages --dossier D       mode « qualité », débit moyen, images clés à 2 s
//!   mf --etape justesse --sortie f.mp4 [--stride positif|defaut] [--png f.png]
//!                                         10 s écrites puis relues (IMFSourceReader)
//!   mf --etape finaliser --secondes 10 --sortie f.mp4 [--moov-debut]
//!   mf --etape son --sortie f.mp4         vidéo + piste AAC (440 Hz, 3 s de silence), relue
//!   mf --etape micro                      points de capture (IMMDeviceEnumerator)
//!   [--fond f.png]                        fond des images plus grandes que le témoin
//!
//! Entrée RGB32 en mémoire centrale (comme l'app : les images de la prise), sortie H.264 ;
//! matrice BT.709 déclarée sur la sortie. Une ligne « cle valeur » par mesure, « ok 1 » ou
//! « ok 0 » et « erreur <étape>: 0x…  (message) » à la fin. Chaque étape échouée dit
//! laquelle (AddStream, SetInputMediaType, BeginWriting, WriteSample, Finalize…).

#[cfg(not(windows))]
fn main() {
    eprintln!("mf : Windows seulement");
}

#[cfg(windows)]
fn main() {
    let a = sonde_film::arguments();
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
    use sonde_film::temoin::{self, Genre};
    use sonde_film::{
        attendre_jusqua, enregistrer_png_bgra, ligne_stats, lire_png_bgra, nombre, ressources,
        statistiques_ms, taille, temps_processeur, Generateur,
    };
    use std::collections::{BTreeSet, HashMap};
    use std::io::{Read, Seek, SeekFrom};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};
    use windows::core::{Interface, GUID, HSTRING, PWSTR};
    use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
    use windows::Win32::Media::Audio::*;
    use windows::Win32::Media::MediaFoundation::*;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PROPVARIANT};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED,
        STGM_READ,
    };
    use windows::Win32::System::Variant::{VARIANT, VT_UI4};

    type R<T> = Result<T, String>;

    fn hr(e: &windows::core::Error) -> String {
        sonde_film::hresult(e.code().0, &e.message())
    }

    /// `etape` + HRESULT : « SetInputMediaType: 0xC00D36B4 (…) ».
    fn ctx<T>(etape: &str, r: windows::core::Result<T>) -> R<T> {
        r.map_err(|e| format!("{etape}: {}", hr(&e)))
    }

    const FPS: u32 = 30;
    /// 100 ns par image à 30 i/s.
    const PAS_100NS: i64 = 10_000_000 / FPS as i64;

    fn demarrer() -> R<()> {
        // SAFETY : COM (MTA) puis Media Foundation pour ce processus.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            ctx("MFStartup", MFStartup(MF_VERSION, MFSTARTUP_FULL))
        }
    }

    fn attributs(n: u32) -> R<IMFAttributes> {
        let mut a = None;
        // SAFETY : sortie Option.
        ctx("MFCreateAttributes", unsafe {
            MFCreateAttributes(&mut a, n)
        })?;
        a.ok_or_else(|| "MFCreateAttributes: rien rendu".into())
    }

    fn paire(a: u32, b: u32) -> u64 {
        ((a as u64) << 32) | b as u64
    }

    /// k du témoin pour l'image `i` à 30 i/s (le compteur avance toutes les 100 ms).
    fn k_de(i: u64) -> u32 {
        (i * 10 / FPS as u64) as u32
    }

    /// BGRA (de haut en bas) → NV12, matrice BT.709, plage réduite (16 à 235, 16 à 240) ;
    /// la chrominance est la moyenne de chaque carré de 2 × 2. Côtés pairs.
    fn vers_nv12(bgra: &[u8], l: usize, h: usize, sortie: &mut Vec<u8>) {
        sortie.resize(l * h + l * h.div_ceil(2), 0);
        let (y_plan, uv) = sortie.split_at_mut(l * h);
        // Coefficients × 65536 (BT.709 : Kr 0,2126, Kb 0,0722 ; 219/255 et 224/255).
        const YR: i32 = 11966;
        const YG: i32 = 40254;
        const YB: i32 = 4064;
        const UR: i32 = -6596;
        const UG: i32 = -22189;
        const UB: i32 = 28784;
        const VR: i32 = 28784;
        const VG: i32 = -26145;
        const VB: i32 = -2639;
        for y in 0..h {
            let ligne = &bgra[y * l * 4..(y + 1) * l * 4];
            let sortie_y = &mut y_plan[y * l..(y + 1) * l];
            for x in 0..l {
                let (b, g, r) = (
                    ligne[x * 4] as i32,
                    ligne[x * 4 + 1] as i32,
                    ligne[x * 4 + 2] as i32,
                );
                sortie_y[x] = (((YR * r + YG * g + YB * b + 32768) >> 16) + 16).clamp(0, 255) as u8;
            }
        }
        for y in (0..h).step_by(2) {
            let y2 = (y + 1).min(h - 1);
            for x in (0..l).step_by(2) {
                let x2 = (x + 1).min(l - 1);
                let (mut b, mut g, mut r) = (0i32, 0i32, 0i32);
                for (yy, xx) in [(y, x), (y, x2), (y2, x), (y2, x2)] {
                    let i = (yy * l + xx) * 4;
                    b += bgra[i] as i32;
                    g += bgra[i + 1] as i32;
                    r += bgra[i + 2] as i32;
                }
                let (b, g, r) = (b / 4, g / 4, r / 4);
                let u = ((UR * r + UG * g + UB * b + 32768) >> 16) + 128;
                let v = ((VR * r + VG * g + VB * b + 32768) >> 16) + 128;
                let i = (y / 2) * l + x;
                uv[i] = u.clamp(0, 255) as u8;
                uv[i + 1] = v.clamp(0, 255) as u8;
            }
        }
    }

    // ---------------------------------------------------------------------------------
    // L'écrivain (IMFSinkWriter)

    #[derive(Clone, Default)]
    struct Options {
        materiel: Option<bool>,
        moov_debut: bool,
        /// Some(vrai) : MF_MT_DEFAULT_STRIDE = +4l (de haut en bas) ; Some(faux) : -4l ;
        /// None : non posé (défaut de Media Foundation).
        stride_positif: Option<bool>,
        debit: u32,
        son: bool,
        /// Paramètres du codeur passés à SetInputMediaType (clés CODECAPI_*).
        parametres: Vec<(GUID, u32)>,
        /// Entrée NV12 (convertie par l'exemple, BT.709 plage réduite) au lieu de RGB32.
        nv12: bool,
        /// MP4 fragmenté (MFCreateFMPEG4MediaSink) au lieu du MP4 ordinaire.
        fragmente: bool,
    }

    struct Ecrivain {
        sw: IMFSinkWriter,
        video: u32,
        audio: Option<u32>,
    }

    fn type_video(l: u32, h: u32, sortie: bool, o: &Options) -> R<IMFMediaType> {
        // SAFETY : objet COM créé ici ; attributs posés par clés connues.
        unsafe {
            let t = ctx("MFCreateMediaType", MFCreateMediaType())?;
            ctx(
                "MF_MT_MAJOR_TYPE",
                t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video),
            )?;
            if sortie {
                ctx(
                    "MF_MT_SUBTYPE",
                    t.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264),
                )?;
                ctx(
                    "MF_MT_AVG_BITRATE",
                    t.SetUINT32(&MF_MT_AVG_BITRATE, o.debit),
                )?;
                ctx(
                    "MF_MT_YUV_MATRIX",
                    t.SetUINT32(&MF_MT_YUV_MATRIX, MFVideoTransferMatrix_BT709.0 as u32),
                )?;
                ctx(
                    "MF_MT_VIDEO_PRIMARIES",
                    t.SetUINT32(&MF_MT_VIDEO_PRIMARIES, MFVideoPrimaries_BT709.0 as u32),
                )?;
                ctx(
                    "MF_MT_TRANSFER_FUNCTION",
                    t.SetUINT32(&MF_MT_TRANSFER_FUNCTION, MFVideoTransFunc_709.0 as u32),
                )?;
                ctx(
                    "MF_MT_VIDEO_NOMINAL_RANGE",
                    t.SetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE, MFNominalRange_16_235.0 as u32),
                )?;
            } else if o.nv12 {
                ctx(
                    "MF_MT_SUBTYPE",
                    t.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12),
                )?;
                ctx(
                    "MF_MT_DEFAULT_STRIDE",
                    t.SetUINT32(&MF_MT_DEFAULT_STRIDE, l),
                )?;
                ctx(
                    "MF_MT_YUV_MATRIX",
                    t.SetUINT32(&MF_MT_YUV_MATRIX, MFVideoTransferMatrix_BT709.0 as u32),
                )?;
                ctx(
                    "MF_MT_VIDEO_NOMINAL_RANGE",
                    t.SetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE, MFNominalRange_16_235.0 as u32),
                )?;
            } else {
                ctx(
                    "MF_MT_SUBTYPE",
                    t.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32),
                )?;
                if let Some(p) = o.stride_positif {
                    let s = (l * 4) as i32;
                    ctx(
                        "MF_MT_DEFAULT_STRIDE",
                        t.SetUINT32(&MF_MT_DEFAULT_STRIDE, if p { s } else { -s } as u32),
                    )?;
                }
            }
            ctx(
                "MF_MT_INTERLACE_MODE",
                t.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32),
            )?;
            ctx(
                "MF_MT_FRAME_SIZE",
                t.SetUINT64(&MF_MT_FRAME_SIZE, paire(l, h)),
            )?;
            ctx(
                "MF_MT_FRAME_RATE",
                t.SetUINT64(&MF_MT_FRAME_RATE, paire(FPS, 1)),
            )?;
            ctx(
                "MF_MT_PIXEL_ASPECT_RATIO",
                t.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, paire(1, 1)),
            )?;
            Ok(t)
        }
    }

    fn type_audio(aac: bool) -> R<IMFMediaType> {
        // SAFETY : idem.
        unsafe {
            let t = ctx("MFCreateMediaType", MFCreateMediaType())?;
            ctx(
                "MF_MT_MAJOR_TYPE",
                t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio),
            )?;
            ctx(
                "MF_MT_SUBTYPE",
                t.SetGUID(
                    &MF_MT_SUBTYPE,
                    if aac {
                        &MFAudioFormat_AAC
                    } else {
                        &MFAudioFormat_PCM
                    },
                ),
            )?;
            ctx(
                "MF_MT_AUDIO_BITS_PER_SAMPLE",
                t.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16),
            )?;
            ctx(
                "MF_MT_AUDIO_SAMPLES_PER_SECOND",
                t.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48_000),
            )?;
            ctx(
                "MF_MT_AUDIO_NUM_CHANNELS",
                t.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 1),
            )?;
            if aac {
                ctx(
                    "MF_MT_AUDIO_AVG_BYTES_PER_SECOND",
                    t.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 16_000),
                )?;
            } else {
                ctx(
                    "MF_MT_AUDIO_BLOCK_ALIGNMENT",
                    t.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 2),
                )?;
                ctx(
                    "MF_MT_AUDIO_AVG_BYTES_PER_SECOND",
                    t.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 96_000),
                )?;
            }
            Ok(t)
        }
    }

    impl Ecrivain {
        fn ouvrir(chemin: &Path, l: u32, h: u32, o: &Options) -> R<Ecrivain> {
            let _ = std::fs::remove_file(chemin);
            let a = attributs(4)?;
            // SAFETY : objets COM créés ici, chemins et types valides.
            unsafe {
                if let Some(m) = o.materiel {
                    ctx(
                        "MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS",
                        a.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, m as u32),
                    )?;
                }
                if o.moov_debut {
                    ctx(
                        "MF_MPEG4SINK_MOOV_BEFORE_MDAT",
                        a.SetUINT32(&MF_MPEG4SINK_MOOV_BEFORE_MDAT, 1),
                    )?;
                }
                let url = HSTRING::from(chemin.as_os_str());
                let (sw, video) = if o.fragmente {
                    let flux = ctx(
                        "MFCreateFile",
                        MFCreateFile(
                            MF_ACCESSMODE_READWRITE,
                            MF_OPENMODE_DELETE_IF_EXIST,
                            MF_FILEFLAGS_NONE,
                            &url,
                        ),
                    )?;
                    let sink = ctx(
                        "MFCreateFMPEG4MediaSink",
                        MFCreateFMPEG4MediaSink(
                            &flux,
                            &type_video(l, h, true, o)?,
                            None::<&IMFMediaType>,
                        ),
                    )?;
                    let sw = ctx(
                        "MFCreateSinkWriterFromMediaSink",
                        MFCreateSinkWriterFromMediaSink(&sink, &a),
                    )?;
                    (sw, 0)
                } else if o.moov_debut {
                    // L'index au début fait relire le fichier par Finalize : ouvert en
                    // lecture et écriture (par l'URL seule, Finalize rendait 0x80070005).
                    let flux = ctx(
                        "MFCreateFile (lecture et écriture)",
                        MFCreateFile(
                            MF_ACCESSMODE_READWRITE,
                            MF_OPENMODE_DELETE_IF_EXIST,
                            MF_FILEFLAGS_NONE,
                            &url,
                        ),
                    )?;
                    let sw = ctx(
                        "MFCreateSinkWriterFromURL (flux)",
                        MFCreateSinkWriterFromURL(&url, &flux, &a),
                    )?;
                    let v = ctx("AddStream", sw.AddStream(&type_video(l, h, true, o)?))?;
                    (sw, v)
                } else {
                    let sw = ctx(
                        "MFCreateSinkWriterFromURL",
                        MFCreateSinkWriterFromURL(&url, None::<&IMFByteStream>, &a),
                    )?;
                    let v = ctx("AddStream", sw.AddStream(&type_video(l, h, true, o)?))?;
                    (sw, v)
                };
                let entree = type_video(l, h, false, o)?;
                let params = if o.parametres.is_empty() {
                    None
                } else {
                    let p = attributs(o.parametres.len() as u32)?;
                    for (g, v) in &o.parametres {
                        ctx("paramètre du codeur", p.SetUINT32(g, *v))?;
                    }
                    Some(p)
                };
                ctx(
                    "SetInputMediaType",
                    sw.SetInputMediaType(video, &entree, params.as_ref()),
                )?;
                let audio = if o.son {
                    let s = ctx("AddStream (AAC)", sw.AddStream(&type_audio(true)?))?;
                    ctx(
                        "SetInputMediaType (PCM)",
                        sw.SetInputMediaType(s, &type_audio(false)?, None),
                    )?;
                    Some(s)
                } else {
                    None
                };
                Ok(Ecrivain { sw, video, audio })
            }
        }

        fn commencer(&self) -> R<()> {
            // SAFETY : écrivain configuré.
            ctx("BeginWriting", unsafe { self.sw.BeginWriting() })
        }

        /// Prépare l'échantillon (tampon + copie) ; rend (échantillon, durée en ms).
        fn echantillon(octets: &[u8], t: i64, d: i64) -> R<(IMFSample, f64)> {
            let c = Instant::now();
            // SAFETY : tampon de la taille des octets, verrouillé le temps de la copie.
            unsafe {
                let n = octets.len() as u32;
                let b = ctx("MFCreateMemoryBuffer", MFCreateMemoryBuffer(n))?;
                let mut p: *mut u8 = std::ptr::null_mut();
                ctx("Lock", b.Lock(&mut p, None, None))?;
                std::ptr::copy_nonoverlapping(octets.as_ptr(), p, octets.len());
                ctx("Unlock", b.Unlock())?;
                ctx("SetCurrentLength", b.SetCurrentLength(n))?;
                let s = ctx("MFCreateSample", MFCreateSample())?;
                ctx("AddBuffer", s.AddBuffer(&b))?;
                ctx("SetSampleTime", s.SetSampleTime(t))?;
                ctx("SetSampleDuration", s.SetSampleDuration(d))?;
                Ok((s, c.elapsed().as_secs_f64() * 1000.0))
            }
        }

        /// Écrit l'image `i` (BGRA de haut en bas, ou NV12 si l'entrée l'est) ; rend
        /// (préparation, WriteSample) en ms.
        fn image(&self, bgra: &[u8], i: u64) -> R<(f64, f64)> {
            let (s, prep) = Self::echantillon(bgra, i as i64 * PAS_100NS, PAS_100NS)?;
            let c = Instant::now();
            // SAFETY : flux vidéo ouvert, échantillon complet.
            ctx("WriteSample", unsafe {
                self.sw.WriteSample(self.video, &s)
            })?;
            Ok((prep, c.elapsed().as_secs_f64() * 1000.0))
        }

        fn son(&self, pcm: &[i16], t: i64, d: i64) -> R<()> {
            let Some(flux) = self.audio else {
                return Ok(());
            };
            // SAFETY : vue en octets d'un tableau d'i16 vivant.
            let octets = unsafe {
                std::slice::from_raw_parts(pcm.as_ptr() as *const u8, std::mem::size_of_val(pcm))
            };
            let (s, _) = Self::echantillon(octets, t, d)?;
            // SAFETY : flux audio ouvert.
            ctx("WriteSample (son)", unsafe {
                self.sw.WriteSample(flux, &s)
            })
        }

        fn finir(&self) -> R<f64> {
            let c = Instant::now();
            // SAFETY : écrivain commencé.
            ctx("Finalize", unsafe { self.sw.Finalize() })?;
            Ok(c.elapsed().as_secs_f64() * 1000.0)
        }

        /// Les transformations de la chaîne du flux vidéo : catégorie et ce qu'on en sait.
        fn chaine(&self) -> Vec<String> {
            let mut v = Vec::new();
            let Ok(ex) = self.sw.cast::<IMFSinkWriterEx>() else {
                return vec!["IMFSinkWriterEx absent".into()];
            };
            for i in 0..8 {
                let mut cat = GUID::zeroed();
                let mut t: Option<IMFTransform> = None;
                // SAFETY : sorties valides.
                if unsafe { ex.GetTransformForStream(self.video, i, Some(&mut cat), &mut t) }
                    .is_err()
                {
                    break;
                }
                let nom_cat = if cat == MFT_CATEGORY_VIDEO_ENCODER {
                    "codeur"
                } else if cat == MFT_CATEGORY_VIDEO_PROCESSOR {
                    "processeur vidéo"
                } else {
                    "autre"
                };
                let mut d = format!("{i}:{nom_cat}");
                if let Some(t) = t {
                    // SAFETY : attributs de la transformation, lus sans les changer.
                    if let Ok(a) = unsafe { t.GetAttributes() } {
                        let asynchrone = unsafe { a.GetUINT32(&MF_TRANSFORM_ASYNC) }.unwrap_or(0);
                        let url = chaine_attr(&a, &MFT_ENUM_HARDWARE_URL_Attribute);
                        let vendeur = chaine_attr(&a, &MFT_ENUM_HARDWARE_VENDOR_ID_Attribute);
                        d.push_str(&format!(
                            " asynchrone={asynchrone} matériel={}",
                            if url.is_some() || vendeur.is_some() {
                                format!("oui ({})", url.or(vendeur).unwrap_or_default())
                            } else {
                                "non (pas d'URL ni de vendeur matériel)".into()
                            }
                        ));
                    }
                }
                v.push(d);
            }
            v
        }
    }

    fn chaine_attr(a: &IMFAttributes, cle: &GUID) -> Option<String> {
        let mut p = PWSTR::null();
        let mut n = 0u32;
        // SAFETY : chaîne allouée par Media Foundation, libérée par CoTaskMemFree.
        unsafe {
            a.GetAllocatedString(cle, &mut p, &mut n).ok()?;
            let s = p.to_string().unwrap_or_default();
            CoTaskMemFree(Some(p.0 as *const core::ffi::c_void));
            Some(s)
        }
    }

    // ---------------------------------------------------------------------------------
    // Étapes

    pub fn principal(a: &HashMap<String, String>) -> R<()> {
        let etape = a.get("etape").cloned().unwrap_or_default();
        println!("etape {etape}");
        if let (Some(p), Ok(mut x)) = (a.get("png-relues"), PNG_RELUES.lock()) {
            *x = Some((PathBuf::from(p), 0));
        }
        match etape.as_str() {
            "micro" => return micro(),
            "preparation" => return preparation(a),
            "chaine" => return chaine(a),
            _ => {}
        }
        let t = Instant::now();
        demarrer()?;
        println!("mfstartup_ms {:.1}", t.elapsed().as_secs_f64() * 1000.0);
        let fond = match a.get("fond") {
            Some(f) => match lire_png_bgra(Path::new(f)) {
                Ok(x) => Some(x),
                Err(e) => {
                    println!("fond_illisible {e}");
                    None
                }
            },
            None => None,
        };
        let dossier = a
            .get("dossier")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let sortie = a.get("sortie").map(PathBuf::from);
        match etape.as_str() {
            "liste" => liste(),
            "tailles" => tailles(&dossier, fond.as_ref()),
            "materiel" => materiel(&dossier),
            "vitesse" => vitesse(a, sortie.ok_or("--sortie manquant")?, fond.as_ref()),
            "reglages" => reglages(&dossier),
            "justesse" => justesse(a, sortie.ok_or("--sortie manquant")?),
            "finaliser" => finaliser(a, sortie.ok_or("--sortie manquant")?),
            "relire" => etape_relire(a),
            "son" => son(sortie.ok_or("--sortie manquant")?),
            autre => Err(format!("étape inconnue « {autre} »")),
        }
    }

    fn enumerer(
        nom: &str,
        categorie: GUID,
        entree: Option<GUID>,
        sortie: Option<GUID>,
        majeur: GUID,
    ) {
        let ti = entree.map(|s| MFT_REGISTER_TYPE_INFO {
            guidMajorType: majeur,
            guidSubtype: s,
        });
        let to = sortie.map(|s| MFT_REGISTER_TYPE_INFO {
            guidMajorType: majeur,
            guidSubtype: s,
        });
        let mut p: *mut Option<IMFActivate> = std::ptr::null_mut();
        let mut n = 0u32;
        // SAFETY : tableau alloué par MFTEnumEx, chaque élément rendu puis le tableau libéré.
        unsafe {
            if let Err(e) = MFTEnumEx(
                categorie,
                MFT_ENUM_FLAG_ALL,
                ti.as_ref().map(|t| t as *const _),
                to.as_ref().map(|t| t as *const _),
                &mut p,
                &mut n,
            ) {
                println!("{nom}_erreur MFTEnumEx: {}", hr(&e));
                return;
            }
            println!("{nom}_nombre {n}");
            let mut materiels = 0;
            if !p.is_null() {
                let t = std::slice::from_raw_parts_mut(p, n as usize);
                for (i, x) in t.iter_mut().enumerate() {
                    if let Some(act) = x.take() {
                        let nom_mft = chaine_attr(&act, &MFT_FRIENDLY_NAME_Attribute)
                            .unwrap_or_else(|| "(sans nom)".into());
                        let drapeaux = act.GetUINT32(&MF_TRANSFORM_FLAGS_Attribute).unwrap_or(0);
                        let materiel = drapeaux & MFT_ENUM_FLAG_HARDWARE.0 as u32 != 0
                            || chaine_attr(&act, &MFT_ENUM_HARDWARE_URL_Attribute).is_some();
                        if materiel {
                            materiels += 1;
                        }
                        println!(
                            "{nom}_{i} {nom_mft} ; {} ; drapeaux 0x{drapeaux:X}",
                            if materiel { "matériel" } else { "logiciel" }
                        );
                    }
                }
                CoTaskMemFree(Some(p as *const core::ffi::c_void));
            }
            println!("{nom}_materiels {materiels}");
        }
    }

    fn liste() -> R<()> {
        enumerer(
            "codeur_h264",
            MFT_CATEGORY_VIDEO_ENCODER,
            None,
            Some(MFVideoFormat_H264),
            MFMediaType_Video,
        );
        enumerer(
            "decodeur_h264",
            MFT_CATEGORY_VIDEO_DECODER,
            Some(MFVideoFormat_H264),
            None,
            MFMediaType_Video,
        );
        enumerer(
            "codeur_aac",
            MFT_CATEGORY_AUDIO_ENCODER,
            None,
            Some(MFAudioFormat_AAC),
            MFMediaType_Audio,
        );
        enumerer(
            "decodeur_aac",
            MFT_CATEGORY_AUDIO_DECODER,
            Some(MFAudioFormat_AAC),
            None,
            MFMediaType_Audio,
        );
        Ok(())
    }

    fn options_defaut() -> Options {
        Options {
            stride_positif: Some(true),
            debit: 4_000_000,
            ..Options::default()
        }
    }

    /// Trois images écrites puis le fichier fermé : accepté, ou la première étape refusée.
    fn essai_taille(
        chemin: &Path,
        l: u32,
        h: u32,
        o: &Options,
        fond: Option<&(Vec<u8>, u32, u32)>,
    ) -> R<(u64, f64)> {
        let e = Ecrivain::ouvrir(chemin, l, h, o)?;
        e.commencer()?;
        let mut g = Generateur::nouveau(l, h, Genre::Simple, fond);
        let mut ws = Vec::new();
        for i in 0..3u64 {
            let (_, w) = e.image(g.image(k_de(i * 3)), i)?;
            ws.push(w);
        }
        e.finir()?;
        let taille = std::fs::metadata(chemin).map(|m| m.len()).unwrap_or(0);
        Ok((taille, statistiques_ms(&ws).0))
    }

    fn tailles(dossier: &Path, fond: Option<&(Vec<u8>, u32, u32)>) -> R<()> {
        let liste = [
            (600, 450),
            (1024, 768),
            (2560, 1440),
            (3840, 2160),
            (5120, 2880),
            (601, 451),
            (120, 120),
            (64, 64),
            (16, 16),
        ];
        for (l, h) in liste {
            let f = dossier.join(format!("taille-{l}x{h}.mp4"));
            let o = options_defaut();
            match essai_taille(&f, l, h, &o, fond) {
                Ok((t, w)) => {
                    println!("taille_{l}x{h} acceptée ; {t} octets ; WriteSample médiane {w:.2} ms")
                }
                Err(e) => println!("taille_{l}x{h} refusée ; {e}"),
            }
            let _ = std::fs::remove_file(&f);
        }
        Ok(())
    }

    fn materiel(dossier: &Path) -> R<()> {
        for m in [true, false] {
            let f = dossier.join(format!("materiel-{}.mp4", m as u8));
            let o = Options {
                materiel: Some(m),
                ..options_defaut()
            };
            let r = (|| -> R<Vec<String>> {
                let e = Ecrivain::ouvrir(&f, 600, 450, &o)?;
                e.commencer()?;
                let mut g = Generateur::nouveau(600, 450, Genre::Simple, None);
                e.image(g.image(0), 0)?;
                let c = e.chaine();
                e.finir()?;
                Ok(c)
            })();
            match r {
                Ok(c) => println!("materiel_{} {}", m as u8, c.join(" | ")),
                Err(e) => println!("materiel_{} erreur {e}", m as u8),
            }
            let _ = std::fs::remove_file(&f);
        }
        Ok(())
    }

    fn vitesse(
        a: &HashMap<String, String>,
        chemin: PathBuf,
        fond: Option<&(Vec<u8>, u32, u32)>,
    ) -> R<()> {
        let genre = Genre::lire(a.get("genre").map(String::as_str).unwrap_or("simple"))?;
        let (l, h) = taille(a.get("taille").map(String::as_str).unwrap_or("600x450"))?;
        let secondes: f64 = nombre(a, "duree", 20.0);
        let e = Ecrivain::ouvrir(&chemin, l, h, &options_defaut())?;
        e.commencer()?;
        let mut g = Generateur::nouveau(l, h, genre, fond);
        let (mut preps, mut ecrits, mut gens) = (Vec::new(), Vec::new(), Vec::new());
        let mut retards = 0u64;
        let n = (secondes * FPS as f64) as u64;
        let periode = Duration::from_secs_f64(1.0 / FPS as f64);
        let cpu0 = temps_processeur();
        let debut = Instant::now();
        for i in 0..n {
            attendre_jusqua(debut + periode.mul_f64(i as f64));
            let c = Instant::now();
            let img = g.image(k_de(i)).to_vec();
            gens.push(c.elapsed().as_secs_f64() * 1000.0);
            let (p, w) = e.image(&img, i)?;
            preps.push(p);
            ecrits.push(w);
            if debut.elapsed() > periode.mul_f64((i + 1) as f64) {
                retards += 1;
            }
        }
        let ecoule = debut.elapsed().as_secs_f64();
        let fin = e.finir()?;
        let cpu = temps_processeur() - cpu0;
        let r = ressources();
        println!("images {n}");
        println!("duree_s {ecoule:.2}");
        println!("{}", ligne_stats("writesample_ms", &ecrits));
        println!("{}", ligne_stats("preparation_ms", &preps));
        println!("{}", ligne_stats("generation_ms", &gens));
        println!("images_en_retard {retards}");
        println!("finalize_ms {fin:.1}");
        println!(
            "processeur_pourcent_coeur {:.1}",
            cpu / (ecoule + fin / 1000.0) * 100.0
        );
        println!("prive_pic_mo {:.1}", r.1 as f64 / 1048576.0);
        println!(
            "taille_octets {}",
            std::fs::metadata(&chemin).map(|m| m.len()).unwrap_or(0)
        );
        Ok(())
    }

    fn variant_u4(v: u32) -> VARIANT {
        let mut x = VARIANT::default();
        // SAFETY : écriture du type et de la valeur d'un VARIANT vide.
        unsafe {
            (*x.Anonymous.Anonymous).vt = VT_UI4;
            (*x.Anonymous.Anonymous).Anonymous.ulVal = v;
        }
        x
    }

    fn lire_u4(v: &VARIANT) -> String {
        // SAFETY : lecture selon le type du VARIANT.
        unsafe {
            let a = &v.Anonymous.Anonymous;
            match a.vt.0 {
                19 => a.Anonymous.ulVal.to_string(),
                3 => a.Anonymous.lVal.to_string(),
                t => format!("(type {t})"),
            }
        }
    }

    /// Images clés d'un fichier : positions (en images) des échantillons « clean point ».
    fn images_cles(chemin: &Path) -> R<Vec<u64>> {
        // SAFETY : lecteur sans conversion ; échantillons compressés lus un par un.
        unsafe {
            let url = HSTRING::from(chemin.as_os_str());
            let r = ctx(
                "MFCreateSourceReaderFromURL",
                MFCreateSourceReaderFromURL(&url, None::<&IMFAttributes>),
            )?;
            let mut cles = Vec::new();
            let mut i = 0u64;
            loop {
                let mut flags = 0u32;
                let mut s: Option<IMFSample> = None;
                ctx(
                    "ReadSample",
                    r.ReadSample(
                        MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                        0,
                        None,
                        Some(&mut flags),
                        None,
                        Some(&mut s),
                    ),
                )?;
                if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                    break;
                }
                if let Some(s) = s {
                    if s.GetUINT32(&MFSampleExtension_CleanPoint).unwrap_or(0) == 1 {
                        cles.push(i);
                    }
                    i += 1;
                }
                if i > 100_000 {
                    break;
                }
            }
            Ok(cles)
        }
    }

    fn reglages(dossier: &Path) -> R<()> {
        // 1. Par les paramètres de SetInputMediaType (clés CODECAPI_*).
        for (nom, params) in [
            ("defaut_debit_moyen", vec![]),
            (
                "qualite_par_parametres",
                vec![
                    (
                        CODECAPI_AVEncCommonRateControlMode,
                        eAVEncCommonRateControlMode_Quality.0 as u32,
                    ),
                    (CODECAPI_AVEncCommonQuality, 70),
                    (CODECAPI_AVEncMPVGOPSize, 2 * FPS),
                ],
            ),
            (
                "gop_2s_par_parametres",
                vec![(CODECAPI_AVEncMPVGOPSize, 2 * FPS)],
            ),
        ] {
            let f = dossier.join(format!("reglages-{nom}.mp4"));
            let o = Options {
                parametres: params,
                ..options_defaut()
            };
            let r = (|| -> R<(u64, Vec<u64>)> {
                let e = Ecrivain::ouvrir(&f, 600, 450, &o)?;
                e.commencer()?;
                let mut g = Generateur::nouveau(600, 450, Genre::Plein, None);
                for i in 0..(6 * FPS as u64) {
                    e.image(g.image(k_de(i)), i)?;
                }
                e.finir()?;
                let t = std::fs::metadata(&f).map(|m| m.len()).unwrap_or(0);
                Ok((t, images_cles(&f)?))
            })();
            match r {
                Ok((t, cles)) => println!(
                    "reglages_{nom} accepté ; {t} octets pour 6 s (témoin plein) ; images clés aux images {:?}",
                    cles
                ),
                Err(e) => println!("reglages_{nom} refusé ; {e}"),
            }
            let _ = std::fs::remove_file(&f);
        }
        // 2. Par ICodecAPI sur le codeur, après SetInputMediaType et avant BeginWriting.
        let f = dossier.join("reglages-icodecapi.mp4");
        let e = Ecrivain::ouvrir(&f, 600, 450, &options_defaut())?;
        let mut p: *mut core::ffi::c_void = std::ptr::null_mut();
        // SAFETY : interface demandée par son IID ; reprise par from_raw si rendue.
        let api = unsafe {
            e.sw.GetServiceForStream(e.video, &GUID::zeroed(), &ICodecAPI::IID, &mut p)
                .map(|_| ICodecAPI::from_raw(p))
        };
        match api {
            Err(x) => println!("icodecapi absent ; GetServiceForStream: {}", hr(&x)),
            Ok(api) => {
                for (nom, cle, v) in [
                    (
                        "mode_debit",
                        CODECAPI_AVEncCommonRateControlMode,
                        eAVEncCommonRateControlMode_Quality.0 as u32,
                    ),
                    ("qualite", CODECAPI_AVEncCommonQuality, 70),
                    ("gop", CODECAPI_AVEncMPVGOPSize, 2 * FPS),
                ] {
                    // SAFETY : GUID et VARIANT valides le temps de l'appel.
                    unsafe {
                        let pris = api
                            .IsSupported(&cle)
                            .map(|_| "oui".to_string())
                            .unwrap_or_else(|e| format!("non ({})", hr(&e)));
                        let pose = api
                            .SetValue(&cle, &variant_u4(v))
                            .map(|_| "accepté".to_string())
                            .unwrap_or_else(|e| format!("refusé ({})", hr(&e)));
                        let relu = api
                            .GetValue(&cle)
                            .map(|x| lire_u4(&x))
                            .unwrap_or_else(|e| format!("illisible ({})", hr(&e)));
                        println!(
                            "icodecapi_{nom} pris en charge {pris} ; {v} {pose} ; relu {relu}"
                        );
                    }
                }
            }
        }
        e.commencer()?;
        let mut g = Generateur::nouveau(600, 450, Genre::Plein, None);
        for i in 0..(6 * FPS as u64) {
            e.image(g.image(k_de(i)), i)?;
        }
        e.finir()?;
        let t = std::fs::metadata(&f).map(|m| m.len()).unwrap_or(0);
        println!(
            "reglages_icodecapi {t} octets pour 6 s ; images clés aux images {:?}",
            images_cles(&f).unwrap_or_default()
        );
        let _ = std::fs::remove_file(&f);
        Ok(())
    }

    // ---------------------------------------------------------------------------------
    // Relecture vidéo (IMFSourceReader, RGB32 par le processeur vidéo)

    struct Relue {
        duree_100ns: u64,
        l: u32,
        h: u32,
        stride_lu: String,
        images: Vec<(i64, Vec<u8>)>,
    }

    struct Lecture {
        duree_100ns: u64,
        l: u32,
        h: u32,
        stride_lu: String,
        images: u64,
        dernier_ts: i64,
        erreur: Option<String>,
    }

    /// Préfixe des PNG des images relues 0, 1 et 150 (`--png-relues <préfixe>`), et le
    /// nombre de relectures déjà faites dans ce processus (un fichier par relecture).
    static PNG_RELUES: std::sync::Mutex<Option<(PathBuf, u32)>> = std::sync::Mutex::new(None);

    /// Géométrie du type courant de la sortie du lecteur : taille du tampon, pas d'une
    /// ligne (`MF_MT_DEFAULT_STRIDE`, négatif : de bas en haut) et zone d'affichage
    /// (`MF_MT_MINIMUM_DISPLAY_APERTURE`, sinon l'image entière).
    #[derive(Clone, Copy, PartialEq)]
    struct Geometrie {
        l: u32,
        h: u32,
        stride: Option<i32>,
        ox: u32,
        oy: u32,
        al: u32,
        ah: u32,
        ouverture_lue: bool,
    }

    /// # Safety
    /// `c` est un type de média vivant.
    unsafe fn geometrie(c: &IMFMediaType) -> R<Geometrie> {
        // SAFETY : lectures d'attributs d'un type vivant, tampon de la taille d'un
        // MFVideoArea.
        unsafe {
            let fs = ctx("MF_MT_FRAME_SIZE", c.GetUINT64(&MF_MT_FRAME_SIZE))?;
            let (l, h) = ((fs >> 32) as u32, (fs & 0xFFFF_FFFF) as u32);
            let stride = c.GetUINT32(&MF_MT_DEFAULT_STRIDE).ok().map(|s| s as i32);
            let mut g = Geometrie {
                l,
                h,
                stride,
                ox: 0,
                oy: 0,
                al: l,
                ah: h,
                ouverture_lue: false,
            };
            let mut zone = [0u8; std::mem::size_of::<MFVideoArea>()];
            let mut n = 0u32;
            if c.GetBlob(&MF_MT_MINIMUM_DISPLAY_APERTURE, &mut zone, Some(&mut n))
                .is_ok()
                && n as usize >= zone.len()
            {
                let v: MFVideoArea = std::ptr::read_unaligned(zone.as_ptr() as *const MFVideoArea);
                let (ox, oy) = (v.OffsetX.value.max(0) as u32, v.OffsetY.value.max(0) as u32);
                let (al, ah) = (v.Area.cx.max(0) as u32, v.Area.cy.max(0) as u32);
                if al > 0 && ah > 0 && ox + al <= l && oy + ah <= h {
                    g.ox = ox;
                    g.oy = oy;
                    g.al = al;
                    g.ah = ah;
                }
                g.ouverture_lue = true;
            }
            Ok(g)
        }
    }

    /// # Safety
    /// `r` est un lecteur vivant ; `flux` son flux vidéo.
    unsafe fn poser_rgb32(r: &IMFSourceReader, flux: u32) -> R<()> {
        // SAFETY : type créé ici, donné au lecteur.
        unsafe {
            let t = ctx("MFCreateMediaType", MFCreateMediaType())?;
            ctx(
                "MF_MT_MAJOR_TYPE",
                t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video),
            )?;
            ctx(
                "MF_MT_SUBTYPE",
                t.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32),
            )?;
            ctx(
                "SetCurrentMediaType (RGB32)",
                r.SetCurrentMediaType(flux, None, &t),
            )
        }
    }

    /// Relit le fichier (RGB32 par le processeur vidéo du lecteur) et passe chaque image
    /// gardée (`garder(i)`) à `voir(i, horodatage, image BGRA de haut en bas, largeur)` ;
    /// les autres images sont comptées sans être copiées. La géométrie (taille, pas d'une
    /// ligne, zone d'affichage) est relue au départ et à chaque changement de type signalé
    /// par `ReadSample` ; chacune est écrite (`relu_geometrie`). L'image rendue est la zone
    /// d'affichage, copiée ligne par ligne avec le pas réel du tampon. Sans
    /// `MF_PD_DURATION` (MP4 fragmenté coupé), la durée vaut 0 et la lecture continue.
    /// Une erreur après l'ouverture (fichier coupé) est rendue dans `erreur`, avec ce qui
    /// a été lu jusque-là.
    fn relire_avec(
        chemin: &Path,
        garder: impl Fn(u64) -> bool,
        mut voir: impl FnMut(u64, i64, &[u8], usize),
    ) -> R<Lecture> {
        let pngs = PNG_RELUES.lock().ok().and_then(|mut x| {
            x.as_mut().map(|(p, n)| {
                *n += 1;
                (p.clone(), *n)
            })
        });
        // SAFETY : objets COM créés ici ; tampons verrouillés le temps de la copie, chaque
        // ligne copiée vérifiée contre la longueur du tampon.
        unsafe {
            let at = attributs(1)?;
            ctx(
                "MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING",
                at.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1),
            )?;
            let url = HSTRING::from(chemin.as_os_str());
            let r = ctx(
                "MFCreateSourceReaderFromURL",
                MFCreateSourceReaderFromURL(&url, &at),
            )?;
            let flux = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
            let _ = r.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false);
            ctx("SetStreamSelection", r.SetStreamSelection(flux, true))?;
            poser_rgb32(&r, flux)?;
            let c = ctx("GetCurrentMediaType", r.GetCurrentMediaType(flux))?;
            let mut g = geometrie(&c)?;
            let ecrire_geo = |quand: &str, g: &Geometrie, drapeaux: u32| {
                println!(
                    "relu_geometrie {quand} : tampon {}x{} ; stride {} ; zone d'affichage {},{},{}x{}{} ; drapeaux 0x{drapeaux:X}",
                    g.l,
                    g.h,
                    g.stride
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "absent".into()),
                    g.ox,
                    g.oy,
                    g.al,
                    g.ah,
                    if g.ouverture_lue {
                        ""
                    } else {
                        " (MF_MT_MINIMUM_DISPLAY_APERTURE absent : image entière)"
                    }
                );
            };
            ecrire_geo("ouverture", &g, 0);
            let duree = match r
                .GetPresentationAttribute(MF_SOURCE_READER_MEDIASOURCE.0 as u32, &MF_PD_DURATION)
            {
                Ok(mut pv) => {
                    let d = pv.Anonymous.Anonymous.Anonymous.uhVal;
                    let _ = PropVariantClear(&mut pv);
                    d
                }
                Err(e) => {
                    println!("relu_duree_absente {}", hr(&e));
                    0
                }
            };
            let mut i = 0u64;
            let mut erreur = None;
            let mut dernier_ts = 0i64;
            let mut img = Vec::new();
            let mut changements = 0u32;
            loop {
                let mut flags = 0u32;
                let mut ts = 0i64;
                let mut s: Option<IMFSample> = None;
                if let Err(e) = ctx(
                    "ReadSample",
                    r.ReadSample(flux, 0, None, Some(&mut flags), Some(&mut ts), Some(&mut s)),
                ) {
                    erreur = Some(e);
                    break;
                }
                if flags & MF_SOURCE_READERF_NATIVEMEDIATYPECHANGED.0 as u32 != 0 {
                    // Le type d'origine a changé : la sortie RGB32 est redemandée.
                    if let Err(e) = poser_rgb32(&r, flux) {
                        erreur = Some(e);
                        break;
                    }
                }
                if flags
                    & (MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0
                        | MF_SOURCE_READERF_NATIVEMEDIATYPECHANGED.0) as u32
                    != 0
                {
                    match r
                        .GetCurrentMediaType(flux)
                        .map_err(|e| format!("GetCurrentMediaType: {}", hr(&e)))
                        .and_then(|c| geometrie(&c))
                    {
                        Ok(n) => {
                            changements += 1;
                            g = n;
                            ecrire_geo(&format!("avant l'image {i}"), &g, flags);
                        }
                        Err(e) => {
                            erreur = Some(e);
                            break;
                        }
                    }
                }
                if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                    break;
                }
                let Some(s) = s else { continue };
                dernier_ts = ts;
                let png = pngs.as_ref().filter(|_| i == 0 || i == 1 || i == 150);
                if garder(i) || png.is_some() {
                    let ligne = (g.al * 4) as usize;
                    let b = ctx("ConvertToContiguousBuffer", s.ConvertToContiguousBuffer())?;
                    img.resize(ligne * g.ah as usize, 0);
                    // (premier octet de la ligne 0 du tampon, pas d'une ligne, longueur).
                    let mut p: *mut u8 = std::ptr::null_mut();
                    let (debut, pas, long, deux_d) = if let Ok(b2) = b.cast::<IMF2DBuffer>() {
                        let mut pas = 0i32;
                        ctx("Lock2D", b2.Lock2D(&mut p, &mut pas))?;
                        (p, pas as isize, usize::MAX, Some(b2))
                    } else {
                        let mut n = 0u32;
                        ctx("Lock", b.Lock(&mut p, None, Some(&mut n)))?;
                        let pas = g.stride.unwrap_or((g.l * 4) as i32) as isize;
                        // De bas en haut : la ligne 0 est la dernière du tampon.
                        let debut = if pas < 0 {
                            p.offset((g.h as isize - 1) * -pas)
                        } else {
                            p
                        };
                        (debut, pas, n as usize, None)
                    };
                    let mut hors = false;
                    for y in 0..g.ah as usize {
                        let dy = (g.oy as isize + y as isize) * pas + g.ox as isize * 4;
                        let depuis = debut.offset(dy);
                        let fin = (depuis as usize).wrapping_add(ligne);
                        if long != usize::MAX && (depuis < p || fin > p as usize + long) {
                            hors = true;
                            break;
                        }
                        std::ptr::copy_nonoverlapping(
                            depuis,
                            img.as_mut_ptr().add(y * ligne),
                            ligne,
                        );
                    }
                    match deux_d {
                        Some(b2) => {
                            let _ = b2.Unlock2D();
                        }
                        None => {
                            let _ = b.Unlock();
                        }
                    }
                    if hors {
                        erreur = Some(format!(
                            "image {i} : tampon de {long} octets plus petit que la géométrie {}x{} pas {pas}",
                            g.l, g.h
                        ));
                        break;
                    }
                    if let Some((prefixe, n)) = png {
                        let f = PathBuf::from(format!("{}-{n}-{i}.png", prefixe.display()));
                        match enregistrer_png_bgra(&f, &img, g.al, g.ah) {
                            Ok(()) => println!("relu_png {}", f.display()),
                            Err(e) => println!("relu_png erreur {e}"),
                        }
                    }
                    if garder(i) {
                        voir(i, ts, &img, g.al as usize);
                    }
                }
                i += 1;
            }
            println!("relu_changements_de_type {changements}");
            Ok(Lecture {
                duree_100ns: duree,
                l: g.al,
                h: g.ah,
                stride_lu: g
                    .stride
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "absent".into()),
                images: i,
                dernier_ts,
                erreur,
            })
        }
    }

    /// `relire_avec` qui garde toutes les images en mémoire (10 s au plus).
    fn relire(chemin: &Path) -> R<Relue> {
        let mut images = Vec::new();
        let x = relire_avec(
            chemin,
            |i| i < 2000,
            |_, ts, img, _| images.push((ts, img.to_vec())),
        )?;
        if let Some(e) = x.erreur {
            return Err(e);
        }
        Ok(Relue {
            duree_100ns: x.duree_100ns,
            l: x.l,
            h: x.h,
            stride_lu: x.stride_lu,
            images,
        })
    }

    fn retourner(img: &[u8], l: usize, h: usize) -> Vec<u8> {
        let mut v = vec![0u8; img.len()];
        for y in 0..h {
            v[y * l * 4..(y + 1) * l * 4]
                .copy_from_slice(&img[(h - 1 - y) * l * 4..(h - y) * l * 4]);
        }
        v
    }

    fn justesse(a: &HashMap<String, String>, chemin: PathBuf) -> R<()> {
        let stride = match a.get("stride").map(String::as_str) {
            Some("defaut") => None,
            Some("negatif") => Some(false),
            _ => Some(true),
        };
        println!(
            "stride_entree {}",
            match stride {
                None => "non posé (défaut de Media Foundation)",
                Some(true) => "+4l (de haut en bas)",
                Some(false) => "-4l (de bas en haut)",
            }
        );
        let nv12 = a.get("entree").map(String::as_str) == Some("nv12");
        println!(
            "entree {}",
            if nv12 {
                "NV12 (convertie par l'exemple, BT.709, plage réduite)"
            } else {
                "RGB32"
            }
        );
        let (lw, hw) = taille(a.get("taille").map(String::as_str).unwrap_or("600x450"))?;
        println!("taille_ecrite {lw}x{hw}");
        let n = 10 * FPS as u64;
        let e = Ecrivain::ouvrir(
            &chemin,
            lw,
            hw,
            &Options {
                stride_positif: stride,
                nv12,
                ..options_defaut()
            },
        )?;
        e.commencer()?;
        let mut g = Generateur::nouveau(lw, hw, Genre::Simple, None);
        let (tx, ty) = (g.ox, g.oy);
        println!("temoin_dans_l_image {tx},{ty}");
        let (mut conv, mut ws) = (Vec::new(), Vec::new());
        let mut tampon = Vec::new();
        for i in 0..n {
            let img = g.image(k_de(i));
            let (_, w) = if nv12 {
                let c = Instant::now();
                vers_nv12(img, lw as usize, hw as usize, &mut tampon);
                conv.push(c.elapsed().as_secs_f64() * 1000.0);
                e.image(&tampon, i)?
            } else {
                e.image(img, i)?
            };
            ws.push(w);
        }
        e.finir()?;
        if nv12 {
            println!("{}", ligne_stats("conversion_nv12_ms", &conv));
        }
        println!("{}", ligne_stats("writesample_ms", &ws));
        println!("images_ecrites {n}");
        println!(
            "taille_octets {}",
            std::fs::metadata(&chemin).map(|m| m.len()).unwrap_or(0)
        );
        let r = relire(&chemin)?;
        println!("relu_taille {}x{}", r.l, r.h);
        println!("relu_stride {}", r.stride_lu);
        println!("relu_duree_s {:.3}", r.duree_100ns as f64 / 1e7);
        println!("relu_images {}", r.images.len());
        let (l, h) = (r.l as usize, r.h as usize);
        println!("relu_geometrie_egale {}", (r.l == lw && r.h == hw) as u8);
        // Comparaison au pixel près avec l'image écrite (même k), sur les images 0, 1, 150
        // et une sur 30 : écart moyen et maximal par canal, pixels à plus de 24 ; décalage
        // de -2 à +2 px qui rend l'écart le plus petit (image 150).
        let mut source = Generateur::nouveau(lw, hw, Genre::Simple, None);
        let ecart_a = |relue: &[u8], src: &[u8], dx: i32, dy: i32| -> (f64, u8, u64) {
            let (mut somme, mut max, mut gros, mut n) = (0u64, 0u8, 0u64, 0u64);
            for y in 2..h.saturating_sub(2) {
                for x in 2..l.saturating_sub(2) {
                    let (sx, sy) = (x as i32 + dx, y as i32 + dy);
                    if sx < 0 || sy < 0 || sx as usize >= lw as usize || sy as usize >= hw as usize
                    {
                        continue;
                    }
                    let i = (y * l + x) * 4;
                    let k = (sy as usize * lw as usize + sx as usize) * 4;
                    let mut pire = 0u8;
                    for c in 0..3 {
                        let d = relue[i + c].abs_diff(src[k + c]);
                        somme += d as u64;
                        pire = pire.max(d);
                    }
                    n += 3;
                    max = max.max(pire);
                    if pire > 24 {
                        gros += 1;
                    }
                }
            }
            (somme as f64 / n.max(1) as f64, max, gros)
        };
        if r.l == lw && r.h == hw {
            for (i, (_, img)) in r.images.iter().enumerate() {
                if !(i == 0 || i == 1 || i == 150 || i % 30 == 0) {
                    continue;
                }
                let src = source.image(k_de(i as u64)).to_vec();
                let (m, x, gros) = ecart_a(img, &src, 0, 0);
                println!(
                    "pixels_image_{i} écart moyen {m:.2}, max {x}, pixels à plus de 24 : {gros}"
                );
                if i == 150 {
                    let mut mieux = (f64::MAX, 0, 0);
                    for dy in -2..=2 {
                        for dx in -2..=2 {
                            let (m, _, _) = ecart_a(img, &src, dx, dy);
                            if m < mieux.0 {
                                mieux = (m, dx, dy);
                            }
                        }
                    }
                    println!(
                        "pixels_decalage_meilleur {},{} (écart moyen {:.2})",
                        mieux.1, mieux.2, mieux.0
                    );
                }
            }
        }
        let mut a_l_endroit = 0;
        let mut a_l_envers = 0;
        let mut illisibles = 0;
        let mut justes = 0;
        let mut reculs = 0;
        let mut prec: Option<u32> = None;
        let mut ecarts = Vec::new();
        let mut ecarts_bande = Vec::new();
        let mut premier_ts = None;
        for (i, (ts, img)) in r.images.iter().enumerate() {
            premier_ts.get_or_insert(*ts);
            let (img, k) = match temoin::lire_k(img, l, tx, ty) {
                Some(k) => {
                    a_l_endroit += 1;
                    (img.clone(), Some(k))
                }
                None => {
                    let v = retourner(img, l, h);
                    match temoin::lire_k(&v, l, tx, ty) {
                        Some(k) => {
                            a_l_envers += 1;
                            (v, Some(k))
                        }
                        None => (img.clone(), None),
                    }
                }
            };
            let Some(k) = k else {
                illisibles += 1;
                continue;
            };
            // L'image relue `i` doit montrer k_de(i) (à une image près : un décalage du
            // premier horodatage se verrait ici).
            let attendu = k_de(i as u64);
            if k == attendu {
                justes += 1;
            }
            if let Some(p) = prec {
                if k < p {
                    reculs += 1;
                }
            }
            prec = Some(k);
            if let Some(e) = temoin::ecarts_pastilles(&img, l, tx, ty) {
                ecarts.push(e.iter().cloned().fold(0.0, f64::max));
            }
            if let Some(e) = temoin::ecart_bande(&img, l, tx, ty, k) {
                ecarts_bande.push(e);
            }
            if i == 150 {
                if let Some(png) = a.get("png") {
                    let _ = enregistrer_png_bgra(Path::new(png), &img, r.l, r.h);
                }
            }
        }
        println!(
            "relu_premier_horodatage_ms {:.2}",
            premier_ts.unwrap_or(0) as f64 / 1e4
        );
        println!("images_a_l_endroit {a_l_endroit}");
        println!("images_a_l_envers {a_l_envers}");
        println!("images_k_illisible {illisibles}");
        println!("images_k_juste {justes}");
        println!("k_reculs {reculs}");
        println!(
            "k_dernier {}",
            prec.map(|k| k.to_string()).unwrap_or_else(|| "?".into())
        );
        let (m, p, x) = statistiques_ms(&ecarts);
        println!("pastilles_ecart_max_par_canal {m:.1};{p:.1};{x:.1}");
        let (m, p, x) = statistiques_ms(&ecarts_bande);
        println!("bande_ecart_max_par_canal {m:.1};{p:.1};{x:.1}");
        Ok(())
    }

    /// Ordre des boîtes de premier niveau d'un MP4.
    fn boites(chemin: &Path) -> String {
        let Ok(mut f) = std::fs::File::open(chemin) else {
            return "illisible".into();
        };
        let total = f.metadata().map(|m| m.len()).unwrap_or(0);
        let mut pos = 0u64;
        let mut v = Vec::new();
        while pos + 8 <= total && v.len() < 12 {
            let mut t = [0u8; 16];
            if f.seek(SeekFrom::Start(pos)).is_err() || f.read_exact(&mut t[..8]).is_err() {
                break;
            }
            let mut taille = u32::from_be_bytes([t[0], t[1], t[2], t[3]]) as u64;
            let genre = String::from_utf8_lossy(&t[4..8]).to_string();
            if taille == 1 {
                if f.read_exact(&mut t[8..16]).is_err() {
                    break;
                }
                taille = u64::from_be_bytes([t[8], t[9], t[10], t[11], t[12], t[13], t[14], t[15]]);
            } else if taille == 0 {
                taille = total - pos;
            }
            v.push(genre);
            if taille < 8 {
                break;
            }
            pos += taille;
        }
        v.join(",")
    }

    /// Octets libres du disque du dossier, et taille de tous les fichiers du dossier.
    fn disque(dossier: &Path) -> (u64, u64) {
        let mut libre = 0u64;
        let d = HSTRING::from(dossier.as_os_str());
        // SAFETY : chemin terminé par un zéro ; sortie u64.
        let _ = unsafe { GetDiskFreeSpaceExW(&d, Some(&mut libre), None, None) };
        let taille = std::fs::read_dir(dossier)
            .map(|r| {
                r.filter_map(|e| e.ok())
                    .filter_map(|e| e.metadata().ok())
                    .filter(|m| m.is_file())
                    .map(|m| m.len())
                    .sum()
            })
            .unwrap_or(0);
        (libre, taille)
    }

    /// Point 7 (et 13 avec `--fragmente`) : `secondes` d'images du témoin simple en
    /// 600 × 450, codées aussi vite que possible (ou en temps réel avec `--temps-reel`,
    /// pour la coupure du point 13 : `--progres F` reçoit « images n » toutes les 30), puis
    /// Finalize, la place disque suivie toutes les 20 ms pendant Finalize.
    fn finaliser(a: &HashMap<String, String>, chemin: PathBuf) -> R<()> {
        let secondes: u64 = nombre(a, "secondes", 10);
        let moov = a.contains_key("moov-debut");
        let fragmente = a.contains_key("fragmente");
        let temps_reel = a.contains_key("temps-reel");
        let progres = a.get("progres").map(PathBuf::from);
        println!("moov_avant_mdat_demande {}", moov as u8);
        println!("fragmente {}", fragmente as u8);
        let e = Ecrivain::ouvrir(
            &chemin,
            600,
            450,
            &Options {
                moov_debut: moov,
                fragmente,
                ..options_defaut()
            },
        )?;
        e.commencer()?;
        let mut g = Generateur::nouveau(600, 450, Genre::Simple, None);
        let n = secondes * FPS as u64;
        let c = Instant::now();
        let periode = Duration::from_secs_f64(1.0 / FPS as f64);
        let mut ws = Vec::new();
        for i in 0..n {
            if temps_reel {
                attendre_jusqua(c + periode.mul_f64(i as f64));
            }
            let (_, w) = e.image(g.image(k_de(i)), i)?;
            ws.push(w);
            if i % 30 == 29 {
                if let Some(p) = &progres {
                    sonde_film::ecrire_etat(p, &[format!("images {}", i + 1)]);
                }
            }
        }
        let ecriture = c.elapsed().as_secs_f64();
        let dossier = chemin
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(std::env::temp_dir);
        let (libre0, dossier0) = disque(&dossier);
        let taille0 = std::fs::metadata(&chemin).map(|m| m.len()).unwrap_or(0);
        let fini = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let f2 = fini.clone();
        let d2 = dossier.clone();
        let suivi = std::thread::spawn(move || {
            let (mut libre_min, mut dossier_max) = (u64::MAX, 0u64);
            while !f2.load(std::sync::atomic::Ordering::Relaxed) {
                let (l, t) = disque(&d2);
                libre_min = libre_min.min(l);
                dossier_max = dossier_max.max(t);
                std::thread::sleep(Duration::from_millis(20));
            }
            (libre_min, dossier_max)
        });
        let fin = e.finir();
        fini.store(true, std::sync::atomic::Ordering::Relaxed);
        let (libre_min, dossier_max) = suivi.join().unwrap_or((0, 0));
        let fin = fin?;
        let (libre1, dossier1) = disque(&dossier);
        println!("secondes {secondes}");
        println!("images {n}");
        println!("ecriture_s {ecriture:.2}");
        println!("{}", ligne_stats("writesample_ms", &ws));
        println!("finalize_ms {fin:.1}");
        let taille = std::fs::metadata(&chemin).map(|m| m.len()).unwrap_or(0);
        println!("taille_octets {taille}");
        println!("taille_avant_finalize {taille0}");
        println!(
            "disque_pendant_finalize dossier {dossier0} → pic {dossier_max} → {dossier1} octets ; libre {libre0} → creux {} → {libre1} octets",
            if libre_min == u64::MAX { 0 } else { libre_min }
        );
        println!(
            "disque_en_plus_pendant_finalize {}",
            dossier_max.saturating_sub(dossier0.max(dossier1))
        );
        let b = boites(&chemin);
        let moov_i = b.split(',').position(|x| x == "moov");
        let mdat_i = b.split(',').position(|x| x == "mdat");
        println!("boites {b}");
        println!(
            "moov_avant_mdat {}",
            match (moov_i, mdat_i) {
                (Some(m), Some(d)) => ((m < d) as u8).to_string(),
                _ => "?".into(),
            }
        );
        Ok(())
    }

    /// Relit un fichier (point 13 après la coupure, point 10) : durée, images, valeurs k
    /// lues sur le témoin en (`--temoin x,y`, 0,0 par défaut).
    fn etape_relire(a: &HashMap<String, String>) -> R<()> {
        let f = PathBuf::from(a.get("fichier").ok_or("--fichier manquant")?);
        let (tx, ty) = a
            .get("temoin")
            .and_then(|t| t.split_once(','))
            .and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?)))
            .unwrap_or((0i32, 0i32));
        println!(
            "taille_octets {}",
            std::fs::metadata(&f).map(|m| m.len()).unwrap_or(0)
        );
        println!("boites {}", boites(&f));
        let (mut vus, mut illisibles, mut reculs) = (BTreeSet::new(), 0u64, 0u64);
        let mut prec: Option<u32> = None;
        let r = relire_avec(
            &f,
            |_| true,
            |_, _, img, l| match temoin::lire_k(img, l, tx, ty) {
                Some(k) => {
                    if prec.is_some_and(|p| k < p) {
                        reculs += 1;
                    }
                    prec = Some(k);
                    vus.insert(k);
                }
                None => illisibles += 1,
            },
        );
        match r {
            Err(e) => {
                println!("lisible 0");
                println!("lecture_refusee {e}");
            }
            Ok(x) => {
                println!("lisible 1");
                println!("relu_taille {}x{}", x.l, x.h);
                println!("relu_duree_s {:.3}", x.duree_100ns as f64 / 1e7);
                println!("relu_images {}", x.images);
                println!("relu_dernier_horodatage_s {:.3}", x.dernier_ts as f64 / 1e7);
                println!("relu_k_distincts {}", vus.len());
                println!(
                    "relu_k_premier_dernier {};{}",
                    vus.iter().next().copied().unwrap_or(0),
                    prec.unwrap_or(0)
                );
                println!("relu_k_illisibles {illisibles}");
                println!("relu_k_reculs {reculs}");
                if let Some(e) = x.erreur {
                    println!("relu_arret {e}");
                }
            }
        }
        Ok(())
    }

    /// Point 10 : la chaîne complète. `--fils 2` : ce fil prend l'image (`--source bitblt`,
    /// `dxgi`, ou `synthetique` : une image de `--taille` faite par l'exemple, k = numéro du
    /// créneau), lit le pointeur, copie l'image et la pousse dans une file bornée à 4 (pleine :
    /// l'image est perdue, comptée) ; un second fil seul crée l'écrivain, convertit (NV12 avec
    /// `--entree nv12`), écrit et finit. `--fils 1` : ce fil fait tout. Le fichier est relu
    /// à la fin : valeurs k distinctes par seconde.
    fn chaine(a: &HashMap<String, String>) -> R<()> {
        use sonde_film::prise::{lire_pointeur, Dxgi, Etapes, Gdi, Source};
        use sonde_film::Rect;
        let nom = a.get("source").cloned().unwrap_or_else(|| "bitblt".into());
        let fils: u32 = nombre(a, "fils", 2);
        let duree: f64 = nombre(a, "duree", 20.0);
        let nv12 = a.get("entree").map(String::as_str) == Some("nv12");
        let chemin = PathBuf::from(a.get("sortie").ok_or("--sortie manquant")?);
        println!("source {nom}");
        println!("fils {fils}");
        println!("entree {}", if nv12 { "nv12" } else { "rgb32" });
        enum Prise {
            Moyen(Box<dyn Source>),
            Synthese(Generateur),
        }
        let (mut prise, l, h, tx, ty) = if nom == "synthetique" {
            let (l, h) = taille(a.get("taille").map(String::as_str).unwrap_or("2560x1440"))?;
            let g = Generateur::nouveau(l, h, Genre::Simple, None);
            let (ox, oy) = (g.ox, g.oy);
            (Prise::Synthese(g), l, h, ox, oy)
        } else {
            let zone = Rect::lire(a.get("zone").map(String::as_str).unwrap_or(""))?;
            let (tx, ty) = a
                .get("temoin")
                .and_then(|t| t.split_once(','))
                .and_then(|(x, y)| {
                    Some((x.trim().parse::<i32>().ok()?, y.trim().parse::<i32>().ok()?))
                })
                .map(|(x, y)| (x - zone.x, y - zone.y))
                .unwrap_or((0, 0));
            let s: Box<dyn Source> = match nom.as_str() {
                "bitblt" => Box::new(Gdi::ouvrir(zone)?),
                "dxgi" => {
                    let d = Dxgi::ouvrir(zone, 0, true)?;
                    println!("adaptateur {}", d.nom);
                    Box::new(d)
                }
                autre => return Err(format!("source inconnue « {autre} »")),
            };
            (Prise::Moyen(s), zone.l as u32, zone.h as u32, tx, ty)
        };
        println!("taille {l}x{h}");
        if nv12 && (l % 2 == 1 || h % 2 == 1) {
            return Err("NV12 : côtés pairs demandés".into());
        }
        let o = Options {
            nv12,
            ..options_defaut()
        };
        // Le travail du codeur : (conversion, préparation, WriteSample, tout) par image.
        #[derive(Default)]
        struct Codeur {
            conv: Vec<f64>,
            prep: Vec<f64>,
            ws: Vec<f64>,
            tout: Vec<f64>,
            fin: f64,
            ecrites: u64,
        }
        #[allow(clippy::too_many_arguments)]
        fn coder(
            e: &Ecrivain,
            c: &mut Codeur,
            nv12: bool,
            l: u32,
            h: u32,
            t: &mut Vec<u8>,
            img: &[u8],
            i: u64,
        ) -> R<()> {
            let d = Instant::now();
            let donnee: &[u8] = if nv12 {
                let x = Instant::now();
                vers_nv12(img, l as usize, h as usize, t);
                c.conv.push(x.elapsed().as_secs_f64() * 1000.0);
                t
            } else {
                img
            };
            let (p, w) = e.image(donnee, i)?;
            c.prep.push(p);
            c.ws.push(w);
            c.tout.push(d.elapsed().as_secs_f64() * 1000.0);
            c.ecrites += 1;
            Ok(())
        }
        let n = (duree * FPS as f64) as u64;
        let periode = Duration::from_secs_f64(1.0 / FPS as f64);
        let (mut t_prise, mut t_ptr, mut t_copie) = (Vec::new(), Vec::new(), Vec::new());
        let (mut prises, mut manques, mut pleine) = (0u64, 0u64, 0u64);
        let mut buf = Vec::new();
        let cpu0 = temps_processeur();
        let prendre = |prise: &mut Prise, buf: &mut Vec<u8>, i: u64| -> R<()> {
            match prise {
                Prise::Moyen(s) => {
                    s.prendre(buf, &mut Etapes::default())?;
                }
                Prise::Synthese(g) => {
                    let x = g.image(i as u32 & 0xFFFF);
                    buf.clear();
                    buf.extend_from_slice(x);
                }
            }
            Ok(())
        };
        let debut;
        let codeur: Codeur;
        if fils >= 2 {
            let (envoi, recu) = std::sync::mpsc::sync_channel::<(u64, Vec<u8>)>(4);
            let chemin2 = chemin.clone();
            let o2 = o.clone();
            let fil = std::thread::spawn(move || -> R<Codeur> {
                demarrer()?;
                let e = Ecrivain::ouvrir(&chemin2, l, h, &o2)?;
                e.commencer()?;
                let mut c = Codeur::default();
                let mut t = Vec::new();
                while let Ok((i, img)) = recu.recv() {
                    coder(&e, &mut c, nv12, l, h, &mut t, &img, i)?;
                }
                c.fin = e.finir()?;
                Ok(c)
            });
            debut = Instant::now();
            let mut i = 0u64;
            while i < n {
                attendre_jusqua(debut + periode.mul_f64(i as f64));
                let c = Instant::now();
                prendre(&mut prise, &mut buf, i)?;
                t_prise.push(c.elapsed().as_secs_f64() * 1000.0);
                let c = Instant::now();
                let _ = lire_pointeur(false);
                t_ptr.push(c.elapsed().as_secs_f64() * 1000.0);
                let c = Instant::now();
                let copie = buf.clone();
                t_copie.push(c.elapsed().as_secs_f64() * 1000.0);
                prises += 1;
                match envoi.try_send((i, copie)) {
                    Ok(()) => {}
                    Err(std::sync::mpsc::TrySendError::Full(_)) => pleine += 1,
                    Err(_) => break,
                }
                i += 1;
                let suivant = (debut.elapsed().as_secs_f64() / periode.as_secs_f64()).ceil() as u64;
                if suivant > i {
                    manques += suivant - i;
                    i = suivant;
                }
            }
            drop(envoi);
            codeur = fil
                .join()
                .map_err(|_| "le fil codeur a paniqué".to_string())??;
        } else {
            // Un seul fil : COM et Media Foundation sont posés ici (sans eux,
            // l'écrivain refuse de s'ouvrir : 0x800401F0).
            demarrer()?;
            let e = Ecrivain::ouvrir(&chemin, l, h, &o)?;
            e.commencer()?;
            let mut c = Codeur::default();
            let mut t = Vec::new();
            debut = Instant::now();
            let mut i = 0u64;
            while i < n {
                attendre_jusqua(debut + periode.mul_f64(i as f64));
                let x = Instant::now();
                prendre(&mut prise, &mut buf, i)?;
                t_prise.push(x.elapsed().as_secs_f64() * 1000.0);
                let x = Instant::now();
                let _ = lire_pointeur(false);
                t_ptr.push(x.elapsed().as_secs_f64() * 1000.0);
                prises += 1;
                coder(&e, &mut c, nv12, l, h, &mut t, &buf, i)?;
                i += 1;
                let suivant = (debut.elapsed().as_secs_f64() / periode.as_secs_f64()).ceil() as u64;
                if suivant > i {
                    manques += suivant - i;
                    i = suivant;
                }
            }
            c.fin = e.finir()?;
            codeur = c;
        }
        let ecoule = debut.elapsed().as_secs_f64();
        let cpu = temps_processeur() - cpu0;
        println!("duree_s {ecoule:.2}");
        println!("images_prises {prises}");
        println!("creneaux_manques {manques}");
        println!("file_pleine {pleine}");
        println!("images_ecrites {}", codeur.ecrites);
        println!("ecrites_par_s {:.2}", codeur.ecrites as f64 / duree);
        println!("{}", ligne_stats("prise_ms", &t_prise));
        println!("{}", ligne_stats("pointeur_ms", &t_ptr));
        println!("{}", ligne_stats("copie_ms", &t_copie));
        println!("{}", ligne_stats("conversion_ms", &codeur.conv));
        println!("{}", ligne_stats("preparation_ms", &codeur.prep));
        println!("{}", ligne_stats("writesample_ms", &codeur.ws));
        println!("{}", ligne_stats("codeur_par_image_ms", &codeur.tout));
        println!("finalize_ms {:.1}", codeur.fin);
        println!("processeur_pourcent_coeur {:.1}", cpu / ecoule * 100.0);
        println!(
            "taille_octets {}",
            std::fs::metadata(&chemin).map(|m| m.len()).unwrap_or(0)
        );
        let (mut vus, mut illisibles, mut reculs) = (BTreeSet::new(), 0u64, 0u64);
        let mut prec: Option<u32> = None;
        if fils >= 2 {
            demarrer()?;
        }
        match relire_avec(
            &chemin,
            |_| true,
            |_, _, img, ll| match temoin::lire_k(img, ll, tx, ty) {
                Some(k) => {
                    if prec.is_some_and(|p| k < p) {
                        reculs += 1;
                    }
                    prec = Some(k);
                    vus.insert(k);
                }
                None => illisibles += 1,
            },
        ) {
            Ok(x) => {
                let d = x.duree_100ns as f64 / 1e7;
                println!("relu_images {}", x.images);
                println!("relu_duree_s {d:.3}");
                println!("relu_k_distincts {}", vus.len());
                println!("relu_k_distincts_par_s {:.2}", vus.len() as f64 / duree);
                println!("relu_k_illisibles {illisibles}");
                println!("relu_k_reculs {reculs}");
                if let Some(e) = x.erreur {
                    println!("relu_arret {e}");
                }
            }
            Err(e) => println!("relu_refuse {e}"),
        }
        Ok(())
    }

    /// Point 11 : préparation de l'écrivain, deux fois dans le même processus.
    fn preparation(a: &HashMap<String, String>) -> R<()> {
        let materiel = a.get("materiel").map(|v| v == "1");
        println!(
            "materiel {}",
            materiel
                .map(|m| (m as u8).to_string())
                .unwrap_or_else(|| "non posé".into())
        );
        let dossier = a
            .get("dossier")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        for tour in ["froid", "chaud"] {
            let chemin = dossier.join(format!("preparation-{}-{tour}.mp4", std::process::id()));
            let o = Options {
                materiel,
                ..options_defaut()
            };
            let mut t = Vec::new();
            let r = (|| -> R<()> {
                let c = Instant::now();
                demarrer()?;
                t.push(("MFStartup", c.elapsed().as_secs_f64() * 1000.0));
                let at = attributs(2)?;
                // SAFETY : objets COM créés ici, types valides.
                unsafe {
                    if let Some(m) = materiel {
                        ctx(
                            "MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS",
                            at.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, m as u32),
                        )?;
                    }
                    let c = Instant::now();
                    let url = HSTRING::from(chemin.as_os_str());
                    let sw = ctx(
                        "MFCreateSinkWriterFromURL",
                        MFCreateSinkWriterFromURL(&url, None::<&IMFByteStream>, &at),
                    )?;
                    t.push((
                        "MFCreateSinkWriterFromURL",
                        c.elapsed().as_secs_f64() * 1000.0,
                    ));
                    let c = Instant::now();
                    let v = ctx("AddStream", sw.AddStream(&type_video(600, 450, true, &o)?))?;
                    ctx(
                        "SetInputMediaType",
                        sw.SetInputMediaType(v, &type_video(600, 450, false, &o)?, None),
                    )?;
                    t.push(("types", c.elapsed().as_secs_f64() * 1000.0));
                    let c = Instant::now();
                    ctx("BeginWriting", sw.BeginWriting())?;
                    t.push(("BeginWriting", c.elapsed().as_secs_f64() * 1000.0));
                    let mut g = Generateur::nouveau(600, 450, Genre::Simple, None);
                    let c = Instant::now();
                    let (smp, _) = Ecrivain::echantillon(g.image(0), 0, PAS_100NS)?;
                    ctx("WriteSample", sw.WriteSample(v, &smp))?;
                    t.push(("première image", c.elapsed().as_secs_f64() * 1000.0));
                    let c = Instant::now();
                    ctx("Finalize", sw.Finalize())?;
                    t.push(("Finalize", c.elapsed().as_secs_f64() * 1000.0));
                }
                Ok(())
            })();
            let total: f64 = t
                .iter()
                .filter(|(n, _)| *n != "Finalize")
                .map(|(_, v)| v)
                .sum();
            println!(
                "{tour} {}{}",
                t.iter()
                    .map(|(n, v)| format!("{n} {v:.1}"))
                    .collect::<Vec<_>>()
                    .join(" ; "),
                r.err()
                    .map(|e| format!(" ; erreur {e}"))
                    .unwrap_or_default()
            );
            println!("{tour}_total_ms {total:.1}");
            let _ = std::fs::remove_file(&chemin);
        }
        Ok(())
    }

    // ---------------------------------------------------------------------------------
    // S3 : piste son AAC et micro

    const TAUX: usize = 48_000;
    const SILENCE: (f64, f64) = (3.5, 6.5);

    fn sinus(debut: usize, n: usize) -> Vec<i16> {
        (debut..debut + n)
            .map(|i| {
                let t = i as f64 / TAUX as f64;
                if t >= SILENCE.0 && t < SILENCE.1 {
                    0
                } else {
                    (0.5 * 32767.0 * (2.0 * std::f64::consts::PI * 440.0 * t).sin()) as i16
                }
            })
            .collect()
    }

    fn son(chemin: PathBuf) -> R<()> {
        let e = Ecrivain::ouvrir(
            &chemin,
            600,
            450,
            &Options {
                son: true,
                ..options_defaut()
            },
        )?;
        e.commencer()?;
        let mut g = Generateur::nouveau(600, 450, Genre::Simple, None);
        let n = 10 * FPS as u64;
        let par_image = TAUX / FPS as usize;
        for i in 0..n {
            e.image(g.image(k_de(i)), i)?;
            let pcm = sinus(i as usize * par_image, par_image);
            e.son(&pcm, i as i64 * PAS_100NS, PAS_100NS)?;
        }
        e.finir()?;
        println!(
            "taille_octets {}",
            std::fs::metadata(&chemin).map(|m| m.len()).unwrap_or(0)
        );
        // Relecture : la piste son en PCM 16 bits, la vidéo pour sa durée.
        // SAFETY : objets COM créés ici ; tampons verrouillés le temps de la copie.
        unsafe {
            let url = HSTRING::from(chemin.as_os_str());
            let r = ctx(
                "MFCreateSourceReaderFromURL",
                MFCreateSourceReaderFromURL(&url, None::<&IMFAttributes>),
            )?;
            let fa = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
            let fv = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
            let piste = r.GetNativeMediaType(fa, 0);
            println!(
                "piste_son {}",
                if piste.is_ok() {
                    "présente"
                } else {
                    "absente"
                }
            );
            if piste.is_err() {
                return Ok(());
            }
            let t = type_audio(false)?;
            ctx(
                "SetCurrentMediaType (PCM)",
                r.SetCurrentMediaType(fa, None, &t),
            )?;
            let _ = r.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false);
            let _ = r.SetStreamSelection(fa, true);
            let mut pcm: Vec<i16> = Vec::new();
            let mut premier: Option<i64> = None;
            let mut fin_son = 0i64;
            loop {
                let mut flags = 0u32;
                let mut ts = 0i64;
                let mut s: Option<IMFSample> = None;
                ctx(
                    "ReadSample (son)",
                    r.ReadSample(fa, 0, None, Some(&mut flags), Some(&mut ts), Some(&mut s)),
                )?;
                if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                    break;
                }
                let Some(s) = s else { continue };
                premier.get_or_insert(ts);
                let d = s.GetSampleDuration().unwrap_or(0);
                fin_son = fin_son.max(ts + d);
                let b = ctx("ConvertToContiguousBuffer", s.ConvertToContiguousBuffer())?;
                let mut p: *mut u8 = std::ptr::null_mut();
                let mut n = 0u32;
                ctx("Lock", b.Lock(&mut p, None, Some(&mut n)))?;
                let v = std::slice::from_raw_parts(p as *const i16, n as usize / 2);
                pcm.extend_from_slice(v);
                let _ = b.Unlock();
            }
            // Durée de la vidéo : fin du dernier échantillon vidéo.
            let _ = r.SetStreamSelection(fa, false);
            let _ = r.SetStreamSelection(fv, true);
            let mut pv = PROPVARIANT::default();
            // Retour au début (position 0, VT_I8).
            (*pv.Anonymous.Anonymous).vt = windows::Win32::System::Variant::VT_I8;
            let _ = r.SetCurrentPosition(&GUID::zeroed(), &pv);
            let mut fin_video = 0i64;
            loop {
                let mut flags = 0u32;
                let mut ts = 0i64;
                let mut s: Option<IMFSample> = None;
                if r.ReadSample(fv, 0, None, Some(&mut flags), Some(&mut ts), Some(&mut s))
                    .is_err()
                    || flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0
                {
                    break;
                }
                if let Some(s) = s {
                    fin_video = fin_video.max(ts + s.GetSampleDuration().unwrap_or(0));
                }
            }
            let premier = premier.unwrap_or(0);
            println!("son_premier_horodatage_ms {:.2}", premier as f64 / 1e4);
            println!("son_duree_s {:.3}", fin_son as f64 / 1e7);
            println!(
                "son_echantillons {} ({:.3} s)",
                pcm.len(),
                pcm.len() as f64 / TAUX as f64
            );
            println!("video_duree_s {:.3}", fin_video as f64 / 1e7);
            println!(
                "ecart_son_video_ms {:.1}",
                (fin_son - fin_video) as f64 / 1e4
            );
            // Fréquence : passages par zéro montants sur 0,5 → 2,5 s.
            let decalage = (premier.max(0) as f64 / 1e7 * TAUX as f64) as usize;
            let morceau = |a: f64, b: f64| -> &[i16] {
                let i = ((a * TAUX as f64) as usize)
                    .saturating_sub(decalage)
                    .min(pcm.len());
                let j = ((b * TAUX as f64) as usize)
                    .saturating_sub(decalage)
                    .min(pcm.len());
                &pcm[i..j]
            };
            let m = morceau(0.5, 2.5);
            let montants = m.windows(2).filter(|w| w[0] < 0 && w[1] >= 0).count();
            println!(
                "frequence_hz {:.1}",
                montants as f64 / (m.len().max(1) as f64 / TAUX as f64)
            );
            // Silence : fenêtres de 10 ms, efficace < 1 % de la pleine échelle.
            let fen = TAUX / 100;
            let mut debut_s = None;
            let mut fin_s = None;
            for (w, bloc) in pcm.chunks(fen).enumerate() {
                let rms = (bloc.iter().map(|&x| (x as f64).powi(2)).sum::<f64>()
                    / bloc.len().max(1) as f64)
                    .sqrt();
                let t = (w * fen) as f64 / TAUX as f64 + premier as f64 / 1e7;
                let calme = rms < 327.0;
                if t > 1.0 && t < 9.0 {
                    if calme && debut_s.is_none() {
                        debut_s = Some(t);
                    }
                    if calme {
                        fin_s = Some(t + 0.01);
                    }
                }
            }
            println!(
                "silence_s {};{}",
                debut_s
                    .map(|x| format!("{x:.3}"))
                    .unwrap_or_else(|| "?".into()),
                fin_s
                    .map(|x| format!("{x:.3}"))
                    .unwrap_or_else(|| "?".into())
            );
        }
        Ok(())
    }

    fn micro() -> R<()> {
        // SAFETY : COM du fil, énumérateur des points audio, chaînes libérées.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let en: IMMDeviceEnumerator = ctx(
                "CoCreateInstance(MMDeviceEnumerator)",
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL),
            )?;
            for (nom, flux) in [("capture", eCapture), ("rendu", eRender)] {
                match en.EnumAudioEndpoints(flux, DEVICE_STATE_ACTIVE) {
                    Ok(c) => println!("{nom}_actifs {}", c.GetCount().unwrap_or(0)),
                    Err(e) => println!("{nom}_actifs erreur {}", hr(&e)),
                }
                match en.EnumAudioEndpoints(flux, DEVICE_STATE(DEVICE_STATEMASK_ALL)) {
                    Ok(c) => {
                        let n = c.GetCount().unwrap_or(0);
                        println!("{nom}_tous_etats {n}");
                        for i in 0..n.min(8) {
                            if let Ok(d) = c.Item(i) {
                                let etat = d.GetState().map(|s| s.0).unwrap_or(0);
                                let nom_d = d
                                    .OpenPropertyStore(STGM_READ)
                                    .and_then(|p| p.GetValue(&PKEY_Device_FriendlyName))
                                    .map(|mut v| {
                                        let s = v
                                            .Anonymous
                                            .Anonymous
                                            .Anonymous
                                            .pwszVal
                                            .to_string()
                                            .unwrap_or_default();
                                        let _ = PropVariantClear(&mut v);
                                        s
                                    })
                                    .unwrap_or_else(|e| format!("? ({})", hr(&e)));
                                println!("{nom}_{i} {nom_d} ; état {etat}");
                            }
                        }
                    }
                    Err(e) => println!("{nom}_tous_etats erreur {}", hr(&e)),
                }
                match en.GetDefaultAudioEndpoint(flux, eConsole) {
                    Ok(d) => println!(
                        "{nom}_defaut {}",
                        d.GetId()
                            .map(|p| {
                                let s = p.to_string().unwrap_or_default();
                                CoTaskMemFree(Some(p.0 as *const core::ffi::c_void));
                                s
                            })
                            .unwrap_or_default()
                    ),
                    Err(e) => println!("{nom}_defaut introuvable ; {}", hr(&e)),
                }
            }
        }
        Ok(())
    }
}
