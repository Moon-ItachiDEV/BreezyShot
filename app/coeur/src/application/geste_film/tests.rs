//! Le fil `film` et le fil d'écriture, essayés sous Linux avec des faux : une source, un
//! bureau, un micro et un écrivain menés par les essais, le temps donné à la main.

use super::*;
use std::collections::VecDeque;

const T: Duration = Duration::from_millis(1);

fn ms(n: u64) -> Duration {
    T * u32::try_from(n).unwrap()
}

fn ecran() -> EcranInfo {
    let r = Rect {
        x: 0,
        y: 0,
        largeur: 1920,
        hauteur: 1080,
    };
    EcranInfo {
        rect: r,
        zone_utile: r,
        echelle: 1.0,
        principal: true,
    }
}

fn cadre(reduite: bool) -> Cadre {
    let (l, h) = if reduite { (128, 96) } else { (64, 48) };
    Cadre {
        zone: Rect {
            x: 100,
            y: 100,
            largeur: l,
            hauteur: h,
        },
        image: (64, 48),
        agrandie: false,
        reduite,
    }
}

/// Ce que la fausse source rend à chaque prise.
#[derive(Debug, Clone)]
enum Rendu {
    Nouvelle,
    Inchangee,
    Perdue,
    Erreur,
}

/// La source menée par l'essai.
#[derive(Default)]
struct Script {
    rendus: VecDeque<Rendu>,
    defaut: Option<Rendu>,
    prises: usize,
    replis: usize,
}

struct FausseSource {
    script: Arc<Mutex<Script>>,
    zone: Rect,
    teinte: u8,
}

impl Prise for FausseSource {
    fn prendre(&mut self, dest: &mut Pixels) -> Result<(vivant::Image, Duration), String> {
        let mut s = self.script.lock().unwrap();
        s.prises += 1;
        let rendu = s
            .rendus
            .pop_front()
            .or_else(|| s.defaut.clone())
            .unwrap_or(Rendu::Nouvelle);
        match rendu {
            Rendu::Nouvelle => {
                self.teinte = self.teinte.wrapping_add(1);
                dest.largeur = self.zone.largeur as u32;
                dest.hauteur = self.zone.hauteur as u32;
                dest.rgba.clear();
                dest.rgba.resize(
                    dest.largeur as usize * dest.hauteur as usize * 4,
                    self.teinte,
                );
                Ok((vivant::Image::Nouvelle, ms(2)))
            }
            Rendu::Inchangee => Ok((vivant::Image::Inchangee, ms(1))),
            Rendu::Perdue => Ok((vivant::Image::Perdue, Duration::ZERO)),
            Rendu::Erreur => Err("figee".into()),
        }
    }

    fn replier(&mut self) -> Result<(), String> {
        self.script.lock().unwrap().replis += 1;
        Ok(())
    }

    fn repli(&mut self) -> Option<(&'static str, String)> {
        None
    }

    fn moyen(&self) -> &'static str {
        "duplication"
    }
}

/// Le bureau mené par l'essai.
struct FauxBureau {
    ecrans: Vec<EcranInfo>,
    verrouillages: u64,
    veilles: u64,
    place: Option<u64>,
    taille: Option<u64>,
    pointeur: Option<Pointeur>,
    micros: usize,
}

impl Default for FauxBureau {
    fn default() -> FauxBureau {
        FauxBureau {
            ecrans: vec![ecran()],
            verrouillages: 7,
            veilles: 3,
            place: Some(50 * 1024 * 1024 * 1024),
            taille: Some(0),
            pointeur: None,
            micros: 0,
        }
    }
}

struct FauxMicro(Arc<AtomicBool>);

impl Ouvert for FauxMicro {
    fn perdu(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Un fil `film` et ce qui l'entoure.
struct Banc {
    fil: Fil<FausseSource>,
    signaux: Receiver<Signal>,
    envois: Receiver<Envoi>,
    _retour: Sender<Pixels>,
    _moi: Receiver<Courrier>,
    script: Arc<Mutex<Script>>,
    bureau: Arc<Mutex<FauxBureau>>,
    filets: Arc<AtomicUsize>,
    t0: Instant,
}

fn delais(sorte: Sorte) -> Delais {
    Delais {
        page: film::DELAI_PAGE,
        pret: film::DELAI_PRET,
        fermeture: film::DELAI_FERMETURE,
        perdue_max: film::SOURCE_PERDUE_MAX,
        repli_max: film::SOURCE_REPLI_MAX,
        saut_veille: film::SAUT_VEILLE,
        tour: film::TOUR,
        ecrans: film::SURVEILLANCE_ECRANS,
        disque: film::RELECTURE_DISQUE,
        duree_max: sorte.duree_max(),
        pause_max: film::PAUSE_MAX,
    }
}

fn banc_avec(sorte: Sorte, reduite: bool, source: bool, d: Delais) -> Banc {
    let t0 = Instant::now();
    let script = Arc::new(Mutex::new(Script::default()));
    let bureau = Arc::new(Mutex::new(FauxBureau::default()));
    let (envoi_signal, signaux) = mpsc::channel();
    let (ecriture, envois) = mpsc::channel();
    let (retour, retours) = mpsc::channel();
    let (moi, boite) = mpsc::channel();
    let filets = Arc::new(AtomicUsize::new(0));
    let c = cadre(reduite);
    let b = Arc::clone(&bureau);
    let (b1, b2, b3, b4, b5, b6, b7) = (
        Arc::clone(&b),
        Arc::clone(&b),
        Arc::clone(&b),
        Arc::clone(&b),
        Arc::clone(&b),
        Arc::clone(&b),
        Arc::clone(&b),
    );
    let fausse = FausseSource {
        script: Arc::clone(&script),
        zone: c.zone,
        teinte: 0,
    };
    let compte = Arc::clone(&filets);
    let fil = Fil::nouveau(
        Lancement {
            numero: 5,
            sorte,
            relache: t0,
            cadre: c,
            ecrans: vec![ecran()],
            verrouillages: 7,
            veilles: 3,
            envoyees: Arc::new(AtomicU64::new(0)),
            moi,
        },
        if source {
            Ok(fausse)
        } else {
            Err("refusee".into())
        },
        Liens {
            sortie: Box::new(move |s| {
                let _ = envoi_signal.send(s);
            }),
            ecriture,
            en_file: Arc::new(AtomicUsize::new(0)),
            retours,
            calage: Arc::new(Mutex::new(Calage::default())),
            ouvert: Arc::new(AtomicBool::new(false)),
        },
        Bureau {
            ecrans: Box::new(move || b1.lock().unwrap().ecrans.clone()),
            verrouillages: Box::new(move || b2.lock().unwrap().verrouillages),
            veilles: Box::new(move || b3.lock().unwrap().veilles),
            place: Box::new(move || b4.lock().unwrap().place),
            taille: Box::new(move |_| b5.lock().unwrap().taille),
            pointeur: Box::new(move || b6.lock().unwrap().pointeur.clone()),
            ouvrir_micro: Box::new(move |_, _| b7.lock().unwrap().micros += 1),
            apercu: |_| None,
        },
        d,
        Some(Box::new(move || {
            compte.fetch_add(1, Ordering::SeqCst);
        })),
    );
    Banc {
        fil,
        signaux,
        envois,
        _retour: retour,
        _moi: boite,
        script,
        bureau,
        filets,
        t0,
    }
}

fn banc(sorte: Sorte) -> Banc {
    banc_avec(sorte, false, true, delais(sorte))
}

/// Ce qu'un envoi au fil d'écriture était, pour les comparer.
#[derive(Debug, Clone, PartialEq)]
enum Vu2 {
    Image(u32, u32, Duration),
    Meme(Duration),
    Finir(Duration),
    Abandonner,
}

impl Banc {
    fn a(&self, n: u64) -> Instant {
        self.t0 + ms(n)
    }

    fn tour(&mut self, n: u64) -> Option<Duration> {
        let a = self.a(n);
        self.fil.tour(a)
    }

    /// Des tours de `de` à `a` (ms), tous les `pas` ms ; la file vidée à chaque tour.
    fn tours(&mut self, de: u64, jusqu_a: u64, pas: u64) {
        let mut n = de;
        while n <= jusqu_a {
            self.tour(n);
            self.vider_file();
            n += pas;
        }
    }

    fn ordre(&mut self, o: Ordre, n: u64) {
        let a = self.a(n);
        self.fil.recevoir(Courrier::Ordre(o), a);
    }

    fn pret(&mut self, n: u64) {
        let a = self.a(n);
        self.fil.recevoir(
            Courrier::Pret {
                fichier: PathBuf::from("/l/Films/20261114-101500-000.mp4"),
                a,
            },
            a,
        );
    }

    /// En marche à `n` ms : prêt (`Vu::Pret`, lu ici), puis départ.
    fn en_marche(&mut self, n: u64) {
        self.pret(n);
        assert_eq!(self.vus(), vec![Vu::Pret]);
        self.ordre(Ordre::PageMontree, n);
        self.ordre(Ordre::Demarrer(self.a(n)), n);
    }

    fn vus(&self) -> Vec<Vu> {
        self.signaux
            .try_iter()
            .filter_map(|s| match s {
                Signal::Vu { numero: 5, vu } => Some(vu),
                _ => None,
            })
            .collect()
    }

    fn signaux(&self) -> Vec<Signal> {
        self.signaux.try_iter().collect()
    }

    /// Le fil d'écriture vide la file (sans rien écrire).
    fn vider_file(&mut self) -> Vec<Vu2> {
        let mut vus = Vec::new();
        while let Ok(e) = self.envois.try_recv() {
            vus.push(match e {
                Envoi::Image { pixels, instant } => {
                    self.fil.liens.en_file.fetch_sub(1, Ordering::SeqCst);
                    Vu2::Image(pixels.largeur, pixels.hauteur, instant)
                }
                Envoi::Meme { instant } => {
                    self.fil.liens.en_file.fetch_sub(1, Ordering::SeqCst);
                    Vu2::Meme(instant)
                }
                Envoi::Finir { duree } => Vu2::Finir(duree),
                Envoi::Abandonner => Vu2::Abandonner,
            });
        }
        vus
    }

    fn envoyees(&self) -> u64 {
        self.fil.l.envoyees.load(Ordering::SeqCst)
    }

    fn sans_vu(&self) -> bool {
        self.vus().is_empty()
    }
}

/// F13 : 30 images par seconde pour une vidéo, 15 pour un GIF, sur la grille ; la première
/// est une image, les suivantes inchangées sont « la même » (vidéo).
#[test]
fn cadence_d_une_seconde() {
    for (sorte, attendu) in [(Sorte::Video, 30), (Sorte::Gif, 15)] {
        let mut b = banc(sorte);
        b.tour(0);
        b.en_marche(0);
        b.script.lock().unwrap().defaut = Some(Rendu::Inchangee);
        let mut envois = Vec::new();
        let mut n = 0;
        while n < 1000 {
            b.tour(n);
            envois.extend(b.vider_file());
            n += 10;
        }
        assert_eq!(b.envoyees(), attendu, "{sorte:?}");
        assert_eq!(b.fil.sautees, 0);
        assert_eq!(envois.first(), Some(&Vu2::Image(64, 48, Duration::ZERO)));
        match sorte {
            // Rien de neuf : « la même », au temps de la grille.
            Sorte::Video => {
                assert_eq!(envois.len(), 30);
                assert_eq!(envois[1], Vu2::Meme(Duration::from_nanos(33_333_333)));
            }
            // GIF : rien n'est écrit, l'image précédente s'allonge (F19).
            Sorte::Gif => assert_eq!(envois.len(), 1),
        }
        assert_eq!(
            b.vus(),
            vec![],
            "aucune seconde entière n'est encore passée"
        );
        b.tour(1000);
        assert_eq!(b.vus(), vec![Vu::Seconde(Duration::from_secs(1))]);
    }
}

/// Une image nouvelle à chaque pas est écrite à chaque pas.
#[test]
fn chaque_image_nouvelle_est_ecrite() {
    let mut b = banc(Sorte::Video);
    b.tour(0);
    b.en_marche(0);
    let mut envois = Vec::new();
    for n in (0..500).step_by(10) {
        b.tour(n);
        envois.extend(b.vider_file());
    }
    assert_eq!(envois.len(), 15);
    assert!(envois.iter().all(|e| matches!(e, Vu2::Image(64, 48, _))));
}

/// Seconde passe de la sonde (§7) : aucune image n'est poussée avant `pret`, même en
/// marche ; les pas d'avant sont sautés, jamais rattrapés.
#[test]
fn aucune_image_avant_pret() {
    let mut b = banc(Sorte::Video);
    b.tour(0);
    // Pendant l'attente, la source est prise (amorce) sans rien pousser.
    assert!(b.script.lock().unwrap().prises > 0);
    assert!(b.fil.a_image);
    b.ordre(Ordre::Demarrer(b.a(0)), 0);
    for n in (0..300).step_by(10) {
        b.tour(n);
    }
    assert!(b.vider_file().is_empty());
    assert_eq!(b.envoyees(), 0);
    assert_eq!(b.fil.sautees, 9);
    b.pret(300);
    b.tour(300);
    assert_eq!(b.vider_file(), vec![Vu2::Image(64, 48, ms(300))]);
    assert_eq!(b.envoyees(), 1);
}

/// F20 : la file bornée pleine, le pas est sauté (le fil d'écriture ne suit plus).
#[test]
fn file_pleine_les_pas_sont_sautes() {
    let mut b = banc(Sorte::Video);
    b.tour(0);
    b.en_marche(0);
    assert_eq!(b.fil.file_max, film::FILE_IMAGES_MAX);
    for n in (0..1000).step_by(10) {
        b.tour(n);
    }
    assert_eq!(b.envoyees(), film::FILE_IMAGES_MAX as u64);
    assert_eq!(b.fil.sautees, 30 - film::FILE_IMAGES_MAX as u64);
    assert_eq!(b.fil.file_vue, film::FILE_IMAGES_MAX);
    // La file se vide : les images reprennent.
    b.vider_file();
    b.tour(1000);
    assert_eq!(b.envoyees(), film::FILE_IMAGES_MAX as u64 + 1);
}

/// F4 : réduite de moitié, l'image écrite a la moitié des côtés de la zone.
#[test]
fn image_reduite_de_moitie() {
    let mut b = banc_avec(Sorte::Video, true, true, delais(Sorte::Video));
    b.tour(0);
    b.en_marche(0);
    b.tour(0);
    assert_eq!(b.vider_file(), vec![Vu2::Image(64, 48, Duration::ZERO)]);
}

/// Le pointeur : posé sur l'image quand il touche la zone ; un pointeur qui bouge sur une
/// image inchangée donne une nouvelle image (vidéo).
#[test]
fn le_pointeur_change_l_image() {
    let mut b = banc(Sorte::Video);
    b.tour(0);
    b.en_marche(0);
    b.script.lock().unwrap().defaut = Some(Rendu::Inchangee);
    b.tour(0);
    b.tour(40);
    assert_eq!(b.vider_file().len(), 2);
    let pointeur = |x| Pointeur {
        origine: (x, 110),
        largeur: 2,
        hauteur: 2,
        sur_noir: vec![255; 16],
        sur_blanc: vec![255; 16],
    };
    b.bureau.lock().unwrap().pointeur = Some(pointeur(110));
    b.tour(70);
    b.bureau.lock().unwrap().pointeur = Some(pointeur(120));
    b.tour(100);
    // Hors de la zone : comme sans pointeur, une fois.
    b.bureau.lock().unwrap().pointeur = Some(pointeur(5));
    b.tour(140);
    b.tour(170);
    let e = b.vider_file();
    assert!(matches!(e[0], Vu2::Image(..)), "{e:?}");
    assert!(matches!(e[1], Vu2::Image(..)), "{e:?}");
    assert!(matches!(e[2], Vu2::Image(..)), "{e:?}");
    assert!(matches!(e[3], Vu2::Meme(_)), "{e:?}");
}

/// F33 : la source perdue 3 s puis revenue : le film continue (la dernière image
/// s'allonge), sans repli ni fin.
#[test]
fn source_perdue_puis_revenue() {
    let mut b = banc(Sorte::Video);
    b.tour(0);
    b.en_marche(0);
    b.tours(0, 900, 100);
    b.script.lock().unwrap().defaut = Some(Rendu::Perdue);
    b.tours(1000, 4000, 30);
    assert!(b.fil.perdue.is_some());
    let avant = b.envoyees();
    assert!(avant > 50, "la dernière image s'allonge : {avant}");
    b.script.lock().unwrap().defaut = Some(Rendu::Nouvelle);
    b.tours(4030, 4100, 30);
    assert!(b.fil.perdue.is_none());
    assert_eq!(b.script.lock().unwrap().replis, 0);
    assert!(
        b.vus().iter().all(|v| matches!(v, Vu::Seconde(_))),
        "aucune fin"
    );
}

/// F33 : perdue 30 s, la source se replie ; rien 2 s de plus : fin `source`.
#[test]
fn source_perdue_trente_secondes() {
    let mut b = banc(Sorte::Video);
    b.tour(0);
    b.en_marche(0);
    b.tours(0, 900, 100);
    b.script.lock().unwrap().defaut = Some(Rendu::Perdue);
    b.tours(1000, 30_900, 100);
    assert_eq!(b.script.lock().unwrap().replis, 0);
    b.tours(31_000, 31_000, 100);
    assert_eq!(b.script.lock().unwrap().replis, 1);
    b.vus();
    b.tours(31_100, 32_900, 100);
    assert!(b.vus().iter().all(|v| matches!(v, Vu::Seconde(_))));
    b.tours(33_000, 33_200, 100);
    let vus: Vec<Vu> = b
        .vus()
        .into_iter()
        .filter(|v| !matches!(v, Vu::Seconde(_)))
        .collect();
    assert_eq!(vus, vec![Vu::SourceFinie]);
    assert_eq!(b.script.lock().unwrap().replis, 1);
}

/// Une source perdue avant la première image ne compte pas (l'écran figé est encore là).
#[test]
fn source_perdue_avant_la_premiere_image() {
    let mut b = banc(Sorte::Video);
    b.script.lock().unwrap().defaut = Some(Rendu::Perdue);
    b.tour(0);
    b.en_marche(0);
    b.tours(0, 40_000, 100);
    assert!(b.fil.perdue.is_none());
    assert_eq!(b.envoyees(), 0);
    assert_eq!(b.script.lock().unwrap().replis, 0);
}

/// F10, F34 : la session verrouillée ou la mise en veille comptée, pendant l'attente comme
/// en marche : dit une fois, le temps du film arrêté là.
#[test]
fn session_et_veille() {
    for marche in [false, true] {
        for veille in [false, true] {
            let mut b = banc(Sorte::Video);
            b.tour(0);
            if marche {
                b.en_marche(0);
                b.tours(0, 500, 100);
                b.vus();
            }
            {
                let mut g = b.bureau.lock().unwrap();
                if veille {
                    g.veilles += 1;
                } else {
                    g.verrouillages += 1;
                }
            }
            b.tour(600);
            b.tour(700);
            let attendu = if veille { Vu::Veille } else { Vu::Session };
            assert_eq!(b.vus(), vec![attendu]);
            assert_eq!(b.fil.gel_a, Some(b.a(600)));
            // Plus d'image après la fin vue.
            let n = b.envoyees();
            b.tours(800, 1500, 100);
            assert_eq!(b.envoyees(), n);
        }
    }
}

/// F10 : les écrans changent ; une liste vide (lecture ratée) ne compte pas ; relus toutes
/// les 500 ms.
#[test]
fn ecrans_changes() {
    let mut b = banc(Sorte::Gif);
    b.tour(0);
    b.bureau.lock().unwrap().ecrans = vec![];
    b.tours(100, 2000, 100);
    assert!(b.sans_vu());
    let mut autre = ecran();
    autre.echelle = 1.5;
    b.bureau.lock().unwrap().ecrans = vec![autre];
    b.tour(2100);
    b.tour(2400);
    assert!(b.sans_vu(), "relus toutes les 500 ms");
    b.tour(2500);
    assert_eq!(b.vus(), vec![Vu::Ecrans]);
}

/// F34 : deux tours séparés de plus de 5 s (une veille non annoncée) : fin `veille`, le
/// temps arrêté au dernier tour.
#[test]
fn saut_d_horloge() {
    let mut b = banc(Sorte::Video);
    b.tour(0);
    b.en_marche(0);
    b.tours(0, 1000, 100);
    b.vus();
    b.tour(6100);
    assert_eq!(b.vus(), vec![Vu::Veille]);
    assert_eq!(b.fil.gel_a, Some(b.a(1000)));
    // 5 s pile : rien.
    let mut c = banc(Sorte::Video);
    c.tour(0);
    c.tour(5000);
    assert!(c.sans_vu());
}

/// F28 : la place libre sous la marge (relue toutes les 5 s, en marche seulement) ; une
/// écriture refusée.
#[test]
fn disque() {
    let mut b = banc(Sorte::Video);
    b.bureau.lock().unwrap().place = Some(1);
    b.tours(0, 1000, 100);
    assert!(b.sans_vu(), "pas en attente");
    b.en_marche(1000);
    b.tour(1000);
    assert_eq!(b.vus(), vec![Vu::Disque]);

    let mut b = banc(Sorte::Video);
    b.tour(0);
    b.en_marche(0);
    b.tour(0);
    b.bureau.lock().unwrap().place = Some(film::PLACE_MIN - 1);
    b.tours(100, 4900, 100);
    assert!(b.vus().iter().all(|v| matches!(v, Vu::Seconde(_))));
    b.tour(5000);
    assert_eq!(
        b.vus()
            .into_iter()
            .filter(|v| !matches!(v, Vu::Seconde(_)))
            .collect::<Vec<_>>(),
        vec![Vu::Disque]
    );

    // La marge suit le débit : 10 s du débit mesuré.
    let mut b = banc(Sorte::Video);
    b.tour(0);
    b.en_marche(0);
    b.bureau.lock().unwrap().place = Some(film::PLACE_MIN + 10);
    b.tours(0, 4900, 100);
    b.bureau.lock().unwrap().taille = Some(5 * 100 * 1024 * 1024);
    b.tour(5000);
    assert!(b.vus().contains(&Vu::Disque));

    // Une écriture refusée : fin `disque` en marche ; avant, film impossible.
    let mut b = banc(Sorte::Video);
    b.tour(0);
    b.en_marche(0);
    b.fil.recevoir(Courrier::Refus, b.a(10));
    assert_eq!(b.vus(), vec![Vu::Disque]);
    let mut b = banc(Sorte::Video);
    b.tour(0);
    b.fil.recevoir(Courrier::Refus, b.a(10));
    assert_eq!(b.vus(), vec![Vu::Echec("disque")]);
}

/// U3 : la durée maximale (une vidéo 60 min, un GIF 60 s), le temps des pauses exclu ; la
/// pause trop longue (30 min).
#[test]
fn limites() {
    let mut d = delais(Sorte::Gif);
    assert_eq!(d.duree_max, Duration::from_secs(60));
    d.duree_max = Duration::from_secs(2);
    let mut b = banc_avec(Sorte::Gif, false, true, d);
    b.tour(0);
    b.en_marche(0);
    b.tours(0, 1000, 100);
    b.ordre(Ordre::Suspendre(b.a(1000)), 1000);
    b.tours(1100, 4000, 100);
    b.ordre(Ordre::Reprendre(b.a(4000)), 4000);
    b.tours(4000, 4900, 100);
    let vus = b.vus();
    assert!(!vus.contains(&Vu::Limite), "{vus:?}");
    b.tour(5000);
    assert_eq!(b.vus(), vec![Vu::Limite]);

    let mut d = delais(Sorte::Video);
    assert_eq!(d.duree_max, Duration::from_secs(3600));
    d.pause_max = Duration::from_secs(3);
    let mut b = banc_avec(Sorte::Video, false, true, d);
    b.tour(0);
    b.en_marche(0);
    b.ordre(Ordre::Suspendre(b.a(500)), 500);
    b.tours(500, 3400, 100);
    assert!(b.sans_vu());
    b.tour(3500);
    assert_eq!(b.vus(), vec![Vu::Limite]);
    assert_eq!(b.fil.gel_a, Some(b.a(500)));
}

/// F7, F11 : le décompte fini, puis `pret` échu 3 s après sans fichier prêt.
#[test]
fn decompte_et_pret_echu() {
    let mut b = banc(Sorte::Video);
    b.tour(0);
    b.ordre(Ordre::Decompter(b.a(3000)), 0);
    assert_eq!(b.tour(2950), Some(ms(50)));
    assert!(b.sans_vu());
    b.tour(3000);
    assert_eq!(b.vus(), vec![Vu::DecompteFini]);
    b.tours(3100, 5900, 100);
    assert!(b.sans_vu());
    b.tour(6000);
    assert_eq!(b.vus(), vec![Vu::PretEchu]);
    // Prêt avant : jamais échu.
    let mut b = banc(Sorte::Video);
    b.tour(0);
    b.pret(10);
    assert_eq!(b.vus(), vec![Vu::Pret]);
    b.ordre(Ordre::Decompter(b.a(1000)), 10);
    b.tours(100, 9000, 100);
    assert_eq!(b.vus(), vec![Vu::DecompteFini]);
}

/// La source refusée au départ : film impossible (`source`), jamais `pret`.
#[test]
fn source_refusee() {
    let mut b = banc_avec(Sorte::Video, false, false, delais(Sorte::Video));
    assert_eq!(b.vus(), vec![Vu::Echec("source")]);
    b.pret(10);
    assert!(b.sans_vu());
    b.fil.recevoir(Courrier::Echec("codeur"), b.a(20));
    assert!(b.sans_vu(), "un seul échec dit");
}

/// F5 : sans pilule posée 2 s après le relâchement, le filet tombe (une fois) ; la page
/// montrée l'ôte.
#[test]
fn filet_de_la_page() {
    let mut b = banc(Sorte::Video);
    assert_eq!(b.tour(0), Some(film::TOUR));
    b.tours(100, 1900, 100);
    assert_eq!(b.filets.load(Ordering::SeqCst), 0);
    assert_eq!(b.tour(1950), Some(ms(50)));
    b.tour(2000);
    b.tours(2100, 3000, 100);
    assert_eq!(b.filets.load(Ordering::SeqCst), 1);

    let mut b = banc(Sorte::Video);
    b.tour(0);
    b.ordre(Ordre::PageMontree, 500);
    b.tours(500, 3000, 100);
    assert_eq!(b.filets.load(Ordering::SeqCst), 0);
}

/// F18, U5 : le micro ouvert sur ordre ; perdu, il est dit et coupé ; refusé, aussi.
#[test]
fn micro() {
    let mut b = banc(Sorte::Video);
    b.tour(0);
    b.en_marche(0);
    b.ordre(Ordre::Micro(true), 10);
    b.ordre(Ordre::Micro(true), 20);
    assert_eq!(b.bureau.lock().unwrap().micros, 1, "une seule ouverture");
    let perdu = Arc::new(AtomicBool::new(false));
    b.fil.recevoir(
        Courrier::Micro(Ok(Box::new(FauxMicro(Arc::clone(&perdu))))),
        b.a(30),
    );
    assert!(b.fil.liens.ouvert.load(Ordering::SeqCst));
    b.tour(100);
    assert!(b.sans_vu());
    perdu.store(true, Ordering::SeqCst);
    b.tour(200);
    assert_eq!(b.vus(), vec![Vu::MicroPerdu]);
    assert!(!b.fil.liens.ouvert.load(Ordering::SeqCst));
    // Refusé.
    b.ordre(Ordre::Micro(true), 300);
    b.fil.recevoir(
        Courrier::Micro(Err(micro::Erreur::Autre("refus".into()))),
        b.a(310),
    );
    assert_eq!(b.vus(), vec![Vu::MicroPerdu]);
    // Coupé avant qu'il s'ouvre : l'ouverture tardive est jetée.
    b.ordre(Ordre::Micro(true), 400);
    b.ordre(Ordre::Micro(false), 410);
    b.fil.recevoir(
        Courrier::Micro(Ok(Box::new(FauxMicro(Arc::new(AtomicBool::new(false)))))),
        b.a(420),
    );
    assert!(!b.fil.liens.ouvert.load(Ordering::SeqCst));
    assert!(b.fil.micro.is_none());
}

/// F27, F29 : Finir (le temps du film arrêté là, pauses exclues), puis le fichier fermé :
/// `Ferme` avec le fichier, la durée et la dernière image brute.
#[test]
fn finir_puis_fermer() {
    let mut b = banc(Sorte::Video);
    b.tour(0);
    b.en_marche(0);
    b.tours(0, 1000, 100);
    b.ordre(Ordre::Suspendre(b.a(1000)), 1000);
    b.ordre(Ordre::Reprendre(b.a(1500)), 1500);
    b.tours(1500, 2400, 100);
    b.ordre(Ordre::Finir(Fin::Bouton, b.a(2500)), 2500);
    assert!(b.fil.source.is_none(), "source rendue dès la fin");
    assert_eq!(b.vider_file().last(), Some(&Vu2::Finir(ms(2000))));
    assert!(b.tour(2600).is_some());
    assert!(b.vider_file().is_empty(), "plus d'image après Finir");
    b.fil.recevoir(
        Courrier::Ferme(Ok(Ecrit {
            octets: 1000,
            ecriture: Some(3.0),
        })),
        b.a(2700),
    );
    let signaux = b.signaux();
    let ferme = signaux
        .iter()
        .find_map(|s| match s {
            Signal::Ferme { numero: 5, issue } => Some(issue.clone()),
            _ => None,
        })
        .expect("ferme");
    let f = ferme.expect("fichier");
    assert_eq!(f.fichier, PathBuf::from("/l/Films/20261114-101500-000.mp4"));
    assert_eq!(f.duree, ms(2000));
    assert_eq!((f.derniere.largeur, f.derniere.hauteur), (64, 48));
    assert_eq!(b.tour(2800), None, "le fil a fini");
    // Un second Finir, un Ferme tardif : rien.
    b.ordre(Ordre::Finir(Fin::Bouton, b.a(2900)), 2900);
    b.fil.recevoir(Courrier::Ferme(Err("fichier")), b.a(2900));
    assert!(b.signaux().is_empty());
}

/// Finir hors marche (attente, décompte) : rien ; c'est le fil principal qui annule.
#[test]
fn finir_hors_marche() {
    let mut b = banc(Sorte::Video);
    b.tour(0);
    b.ordre(Ordre::Finir(Fin::Bouton, b.a(10)), 10);
    assert!(b.vider_file().is_empty());
    assert!(b.fil.source.is_some());
    b.ordre(Ordre::Annuler, 20);
    assert_eq!(b.vider_file(), vec![Vu2::Abandonner]);
    assert!(b.fil.source.is_none());
    assert_eq!(b.tour(30), None);
}

/// F29 : le fichier pas fermé 10 s après la fin : `Ferme` en échec (`delai`), une fois ;
/// un compte tardif du fil d'écriture est ignoré.
#[test]
fn fermeture_hors_delai() {
    let mut b = banc(Sorte::Gif);
    b.tour(0);
    b.en_marche(0);
    b.tours(0, 500, 100);
    b.ordre(Ordre::Finir(Fin::Session, b.a(500)), 500);
    b.signaux();
    assert_eq!(b.tour(9500), Some(film::TOUR));
    assert_eq!(b.tour(10_450), Some(ms(50)));
    assert!(b.signaux().is_empty());
    assert_eq!(b.tour(10_500), None);
    let s = b.signaux();
    assert_eq!(
        s,
        vec![Signal::Ferme {
            numero: 5,
            issue: Err("delai")
        }]
    );
    b.fil.recevoir(
        Courrier::Ferme(Ok(Ecrit {
            octets: 1,
            ecriture: None,
        })),
        b.a(11_000),
    );
    assert!(b.signaux().is_empty());
    // L'écriture échouée pendant la fermeture : `Ferme` en échec (`fichier`).
    let mut b = banc(Sorte::Gif);
    b.tour(0);
    b.en_marche(0);
    b.ordre(Ordre::Finir(Fin::Bouton, b.a(100)), 100);
    b.fil.recevoir(Courrier::Echec("fil"), b.a(200));
    assert!(b.signaux().contains(&Signal::Ferme {
        numero: 5,
        issue: Err("fichier")
    }));
}

/// Un bilan n'est attendu que du film en Finalisation, sous son numéro (F29, F35).
#[test]
fn bilan_tardif_ignore() {
    assert!(bilan_attendu(Some((3, true)), 3));
    assert!(!bilan_attendu(Some((3, false)), 3));
    assert!(!bilan_attendu(Some((4, true)), 3));
    assert!(!bilan_attendu(None, 3));
}

/// F37 : une panique est arrêtée et dite.
#[test]
fn panique() {
    assert!(sans_panique(|| {}));
    assert!(!sans_panique(|| panic!("essai")));
}

/// La fermeture lue sur un canal (F35) : faite, pas faite à temps, ou le fil d'écriture
/// parti.
#[test]
fn fermeture_sur_le_canal() {
    let (envoi, fermeture) = mpsc::channel();
    let fil = std::thread::spawn(move || {
        std::thread::sleep(ms(30));
        let _ = envoi.send(());
    });
    assert!(attendre_fermeture(&fermeture, Duration::from_secs(5)));
    fil.join().unwrap();
    let (_envoi, fermeture) = mpsc::channel::<()>();
    assert!(!attendre_fermeture(&fermeture, ms(20)));
    let (envoi, fermeture) = mpsc::channel::<()>();
    drop(envoi);
    assert!(attendre_fermeture(&fermeture, ms(20)));
}

/// Une seule source de film à la fois (Windows refuse une seconde duplication du même
/// écran) : le jeton attend la source précédente ; le fil `film` le rend avec la source,
/// à la fin du film, à l'annulation, ou à sa fin. Un seul essai le prend (il est global).
#[test]
fn une_seule_source_a_la_fois() {
    let j = Jeton::prendre(Duration::ZERO).expect("libre");
    assert!(Jeton::prendre(ms(20)).is_none());
    let rendu = std::thread::spawn(move || {
        std::thread::sleep(ms(50));
        drop(j);
    });
    let debut = Instant::now();
    let j = Jeton::prendre(Duration::from_secs(5)).expect("rendu");
    assert!(debut.elapsed() >= ms(40));
    rendu.join().unwrap();
    drop(j);

    // Rendu à la fin du film, avant la fermeture du fichier.
    let mut b = banc(Sorte::Video);
    b.fil.tenir(Jeton::prendre(Duration::ZERO));
    b.tour(0);
    b.en_marche(0);
    b.tours(0, 300, 100);
    assert!(Jeton::prendre(Duration::ZERO).is_none());
    b.ordre(Ordre::Finir(Fin::Bouton, b.a(300)), 300);
    let j = Jeton::prendre(Duration::ZERO).expect("rendu à la fin");
    drop(j);
    // Rendu à l'annulation.
    let mut b = banc(Sorte::Video);
    b.fil.tenir(Jeton::prendre(Duration::ZERO));
    b.tour(0);
    assert!(Jeton::prendre(Duration::ZERO).is_none());
    b.ordre(Ordre::Annuler, 10);
    drop(Jeton::prendre(Duration::ZERO).expect("rendu à l'annulation"));
    // Rendu quand le fil finit (panique, canal coupé).
    let mut b = banc(Sorte::Gif);
    b.fil.tenir(Jeton::prendre(Duration::ZERO));
    assert!(Jeton::prendre(Duration::ZERO).is_none());
    drop(b);
    drop(Jeton::prendre(Duration::ZERO).expect("rendu à la fin du fil"));
    // Sans source, le jeton n'est pas gardé.
    let mut b = banc_avec(Sorte::Video, false, false, delais(Sorte::Video));
    b.fil.tenir(Jeton::prendre(Duration::ZERO));
    drop(Jeton::prendre(Duration::ZERO).expect("sans source"));
    drop(b);
}

/// La boucle du fil : le courrier coupé (le fil principal parti) annule et finit.
#[test]
fn boucle_courrier_coupe() {
    let b = banc(Sorte::Video);
    let (envoi, reception) = mpsc::channel();
    drop(envoi);
    let envois = b.envois;
    boucler(b.fil, reception);
    assert!(matches!(envois.try_recv(), Ok(Envoi::Abandonner)));
}

// ---------------------------------------------------------------------------------------
// Le fil d'écriture.

#[derive(Default)]
struct Journal {
    images: usize,
    memes: usize,
    fini: Option<Duration>,
    abandonne: bool,
}

struct FauxEcrivain {
    journal: Arc<Mutex<Journal>>,
    chemin: PathBuf,
    refuser_apres: Option<usize>,
    paniquer: bool,
}

impl Ecrivain for FauxEcrivain {
    fn image(&mut self, _: &Pixels, _: Duration) -> Result<Duration, String> {
        assert!(!self.paniquer, "essai");
        let mut j = self.journal.lock().unwrap();
        j.images += 1;
        if self.refuser_apres.is_some_and(|n| j.images > n) {
            return Err("plein".into());
        }
        Ok(ms(4))
    }

    fn meme(&mut self, _: Duration) -> Result<Duration, String> {
        self.journal.lock().unwrap().memes += 1;
        Ok(ms(1))
    }

    fn finir(self: Box<Self>, duree: Duration) -> Result<(), String> {
        std::fs::write(&self.chemin, b"film").map_err(|e| e.to_string())?;
        self.journal.lock().unwrap().fini = Some(duree);
        Ok(())
    }

    fn abandonner(self: Box<Self>) {
        self.journal.lock().unwrap().abandonne = true;
    }
}

struct Atelier {
    _dossier: tempfile::TempDir,
    films: PathBuf,
    envoi: Sender<Envoi>,
    rapport: Receiver<Courrier>,
    retours: Receiver<Pixels>,
    fini: Receiver<()>,
    en_file: Arc<AtomicUsize>,
    journal: Arc<Mutex<Journal>>,
    fil: std::thread::JoinHandle<()>,
}

fn atelier(sorte: Sorte, refuser_apres: Option<usize>, paniquer: bool, reelle: bool) -> Atelier {
    let dossier = tempfile::tempdir().unwrap();
    let films = dossier.path().join("Films");
    let (envoi, envois) = mpsc::channel();
    let (rapport_envoi, rapport) = mpsc::channel();
    let (retour, retours) = mpsc::channel();
    let (fini_envoi, fini) = mpsc::channel();
    let en_file = Arc::new(AtomicUsize::new(0));
    let journal = Arc::new(Mutex::new(Journal::default()));
    let e = Ecriture {
        sorte,
        dossier_films: films.clone(),
        dossier_glisser: dossier.path().join("Glisser"),
        proteges: Vec::new(),
        maintenant: Local::now(),
        rapport: rapport_envoi,
        envois,
        retour,
        en_file: Arc::clone(&en_file),
        fini: fini_envoi,
    };
    let j = Arc::clone(&journal);
    let fabrique: Fabrique = if reelle {
        Box::new(|chemin: &Path| creer_gif(chemin, (8, 6)))
    } else {
        Box::new(move |chemin: &Path| {
            std::fs::write(chemin, b"").map_err(|_| "disque")?;
            Ok(Box::new(FauxEcrivain {
                journal: j,
                chemin: chemin.to_path_buf(),
                refuser_apres,
                paniquer,
            }) as Box<dyn Ecrivain>)
        })
    };
    let fil = std::thread::spawn(move || ecrire_sans_panique(e, fabrique));
    Atelier {
        _dossier: dossier,
        films,
        envoi,
        rapport,
        retours,
        fini,
        en_file,
        journal,
        fil,
    }
}

impl Atelier {
    fn courrier(&self) -> Courrier {
        self.rapport.recv_timeout(Duration::from_secs(10)).unwrap()
    }

    fn pousser(&self, envoi: Envoi) {
        self.en_file.fetch_add(1, Ordering::SeqCst);
        self.envoi.send(envoi).unwrap();
    }
}

fn pixels(l: u32, h: u32) -> Pixels {
    Pixels {
        largeur: l,
        hauteur: h,
        rgba: vec![128; l as usize * h as usize * 4],
    }
}

/// F9, F29 : prêt (le fichier de travail dans `<local>/Films`), les images, la fin : le
/// fichier fermé, la fermeture dite sur le canal avant le compte, les tampons rendus.
#[test]
fn ecriture_jusqu_a_la_fin() {
    let a = atelier(Sorte::Video, None, false, false);
    let Courrier::Pret { fichier, .. } = a.courrier() else {
        panic!("pret attendu");
    };
    assert_eq!(fichier.parent(), Some(a.films.as_path()));
    assert_eq!(fichier.extension().and_then(|e| e.to_str()), Some("mp4"));
    a.pousser(Envoi::Image {
        pixels: pixels(4, 4),
        instant: Duration::ZERO,
    });
    a.pousser(Envoi::Meme { instant: ms(33) });
    a.envoi.send(Envoi::Finir { duree: ms(66) }).unwrap();
    let Courrier::Ferme(Ok(ecrit)) = a.courrier() else {
        panic!("ferme attendu");
    };
    assert!(a.fini.try_recv().is_ok(), "fermeture dite avant le compte");
    assert_eq!(ecrit.octets, 4);
    assert_eq!(ecrit.ecriture, Some(1.0));
    assert_eq!(a.en_file.load(Ordering::SeqCst), 0);
    assert_eq!(a.retours.try_recv().map(|p| p.largeur), Ok(4));
    let j = a.journal.lock().unwrap();
    assert_eq!((j.images, j.memes, j.fini), (1, 1, Some(ms(66))));
    drop(j);
    assert!(fichier.is_file());
    a.fil.join().unwrap();
}

/// L'abandon (annulation, F8) : le fichier de travail effacé, la fermeture dite ; le
/// canal coupé aussi.
#[test]
fn ecriture_abandonnee() {
    for coupe in [false, true] {
        let a = atelier(Sorte::Gif, None, false, false);
        let Courrier::Pret { fichier, .. } = a.courrier() else {
            panic!("pret attendu");
        };
        assert!(fichier.is_file());
        assert_eq!(fichier.extension().and_then(|e| e.to_str()), Some("gif"));
        if coupe {
            drop(a.envoi);
        } else {
            a.envoi.send(Envoi::Abandonner).unwrap();
        }
        a.fini.recv_timeout(Duration::from_secs(10)).unwrap();
        a.fil.join().unwrap();
        assert!(!fichier.exists());
        assert!(a.journal.lock().unwrap().abandonne);
    }
}

/// F28 : une écriture refusée est dite une fois (`Refus`), plus rien n'est écrit ; la fin
/// ferme quand même ce qui l'est.
#[test]
fn ecriture_refusee() {
    let a = atelier(Sorte::Video, Some(1), false, false);
    assert!(matches!(a.courrier(), Courrier::Pret { .. }));
    for i in 0..4 {
        a.pousser(Envoi::Image {
            pixels: pixels(2, 2),
            instant: ms(i * 33),
        });
    }
    assert!(matches!(a.courrier(), Courrier::Refus));
    a.envoi.send(Envoi::Finir { duree: ms(130) }).unwrap();
    assert!(matches!(a.courrier(), Courrier::Ferme(Ok(_))));
    assert_eq!(a.journal.lock().unwrap().images, 2);
    assert_eq!(a.en_file.load(Ordering::SeqCst), 0);
    a.fil.join().unwrap();
}

/// F37 : le fil d'écriture qui panique rend `Echec("fil")` et dit la fermeture (l'arrêt
/// n'attend pas un fil parti).
#[test]
fn ecriture_qui_panique() {
    let a = atelier(Sorte::Video, None, true, false);
    assert!(matches!(a.courrier(), Courrier::Pret { .. }));
    a.pousser(Envoi::Image {
        pixels: pixels(2, 2),
        instant: Duration::ZERO,
    });
    assert!(matches!(a.courrier(), Courrier::Echec("fil")));
    a.fini.recv_timeout(Duration::from_secs(10)).unwrap();
    a.fil.join().unwrap();
}

/// F9 : le dossier des films inutilisable : film impossible (`disque`), fermeture dite.
#[test]
fn ecriture_sans_dossier() {
    let dossier = tempfile::tempdir().unwrap();
    let bloque = dossier.path().join("Films");
    std::fs::write(&bloque, b"un fichier, pas un dossier").unwrap();
    let (_envoi, envois) = mpsc::channel();
    let (rapport_envoi, rapport) = mpsc::channel();
    let (retour, _retours) = mpsc::channel();
    let (fini_envoi, fini) = mpsc::channel();
    let e = Ecriture {
        sorte: Sorte::Video,
        dossier_films: bloque,
        dossier_glisser: dossier.path().join("Glisser"),
        proteges: Vec::new(),
        maintenant: Local::now(),
        rapport: rapport_envoi,
        envois,
        retour,
        en_file: Arc::new(AtomicUsize::new(0)),
        fini: fini_envoi,
    };
    ecrire_sans_panique(e, Box::new(|_: &Path| Err("codeur")));
    assert!(matches!(rapport.try_recv(), Ok(Courrier::Echec("disque"))));
    assert!(fini.try_recv().is_ok());
}

/// Le codeur refusé à la création : film impossible (`codeur`), rien ne reste.
#[test]
fn ecriture_codeur_refuse() {
    let dossier = tempfile::tempdir().unwrap();
    let films = dossier.path().join("Films");
    let (_envoi, envois) = mpsc::channel();
    let (rapport_envoi, rapport) = mpsc::channel();
    let (retour, _retours) = mpsc::channel();
    let (fini_envoi, fini) = mpsc::channel();
    let e = Ecriture {
        sorte: Sorte::Video,
        dossier_films: films.clone(),
        dossier_glisser: dossier.path().join("Glisser"),
        proteges: Vec::new(),
        maintenant: Local::now(),
        rapport: rapport_envoi,
        envois,
        retour,
        en_file: Arc::new(AtomicUsize::new(0)),
        fini: fini_envoi,
    };
    ecrire_sans_panique(
        e,
        Box::new(|chemin: &Path| {
            std::fs::write(chemin, b"").unwrap();
            Err("codeur")
        }),
    );
    assert!(matches!(rapport.try_recv(), Ok(Courrier::Echec("codeur"))));
    assert!(fini.try_recv().is_ok());
    assert_eq!(std::fs::read_dir(&films).unwrap().count(), 0);
}

/// Un vrai GIF (F19) : écrit par le fil d'écriture, lisible.
#[test]
fn un_vrai_gif() {
    let a = atelier(Sorte::Gif, None, false, true);
    let Courrier::Pret { fichier, .. } = a.courrier() else {
        panic!("pret attendu");
    };
    for i in 0..3u64 {
        let mut p = pixels(8, 6);
        p.rgba.iter_mut().for_each(|o| *o = (i * 80) as u8);
        a.pousser(Envoi::Image {
            pixels: p,
            instant: ms(i * 66),
        });
    }
    a.envoi.send(Envoi::Finir { duree: ms(200) }).unwrap();
    let Courrier::Ferme(Ok(ecrit)) = a.courrier() else {
        panic!("ferme attendu");
    };
    a.fil.join().unwrap();
    let octets = std::fs::read(&fichier).unwrap();
    assert_eq!(ecrit.octets, octets.len() as u64);
    assert!(octets.starts_with(b"GIF89a"));
    assert_eq!(octets.last(), Some(&0x3B));
}

/// Une prise en erreur (l'écran figé encore montré) : rien de neuf, pas une perte ; la
/// dernière image s'allonge.
#[test]
fn prise_en_erreur() {
    let mut b = banc(Sorte::Video);
    b.tour(0);
    b.en_marche(0);
    b.tour(0);
    b.script.lock().unwrap().defaut = Some(Rendu::Erreur);
    b.tour(40);
    b.tour(70);
    assert_eq!(
        b.vider_file(),
        vec![
            Vu2::Image(64, 48, Duration::ZERO),
            Vu2::Meme(Duration::from_nanos(33_333_333)),
            Vu2::Meme(Duration::from_nanos(66_666_666)),
        ]
    );
    assert!(b.fil.perdue.is_none());
}
