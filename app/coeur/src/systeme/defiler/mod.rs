//! Faire défiler la page d'une autre app (contrat 0.6, §13.1, décisions R3 et R4 après la
//! sonde) : trouver les pages sous la souris (S4), envoyer des pas de défilement, lire la
//! position (S5), entendre la molette de l'utilisateur (S6).
//!
//! Moyens, dans cet ordre, choisis une fois par capture sur les deux premières commandes
//! (l'effet est toujours vérifié sur les images, par l'appelant) : `WM_MOUSEWHEEL` posté à
//! la fenêtre fille sous le centre de la zone (`Message`), UI Automation
//! `Scroll(SmallIncrement)` (`Automation`), molette `SendInput` marquée, seulement quand le
//! pointeur est sur la zone (`Molette`). Jamais de touches ; `SetScrollPercent` n'est pas
//! utilisé.
//!
//! Le dialogue avec le cœur : `descendre` dit ce qui est parti (`Envoi`), jamais si la page
//! a bougé. Le cœur le voit sur les images : il dit « elle a bougé de tant » par
//! `constater` (le nombre de crans du pas suivant s'ajuste, `avance_par_cran` le dit), ou
//! « rien n'a bougé, passe au suivant » par `abandonner`. Le choix sur les deux premières
//! commandes (R3) est fait par le cœur : `abandonner` passe toujours au suivant.
//!
//! Gardes communes à chaque pas (`garde`, pure) : rien ne part pendant que Ctrl, Maj, Alt,
//! Win ou un bouton de souris est enfoncé (Ctrl + molette changerait le zoom de la page,
//! Maj la ferait défiler de côté) ; le message posté exige que la fenêtre sous le centre de
//! la zone soit de la cible ; la molette envoyée exige en plus le pointeur sur la zone,
//! aucun menu, aucune capture de souris ni déplacement en cours dans le fil de la cible, et,
//! sans « faire défiler les fenêtres inactives », le focus dans la cible. Une cible dont le
//! processus, le fil ou la classe ont changé (HWND réutilisé) est laissée.
//!
//! Façade commune : `windows.rs` la réalise, `autre.rs` donne des réponses neutres
//! ailleurs. Les règles (garde de la molette envoyée, ordre des moyens, crans d'un pas,
//! pages proposées) sont pures, ici, testées sous Linux. Rien ne va au journal qui viendrait
//! d'une page (ni titre, ni adresse, ni chemin) : seulement des classes de fenêtre, des
//! nombres et des codes.

use std::time::{Duration, Instant};

use super::gel::{FenetreInfo, Rect};

mod pages;
pub use pages::PAGES_TARDIVES_MAX;

#[cfg(windows)]
#[path = "windows.rs"]
mod plateforme;

#[cfg(not(windows))]
#[path = "autre.rs"]
mod plateforme;

/// Marque de BreezyShot dans `dwExtraInfo` de chaque molette envoyée : l'écoute de la
/// molette reconnaît ainsi les crans de l'app parmi ceux de l'utilisateur (R4).
pub const MARQUE: usize = 0x4252_5A59;

/// Temps donné à la recherche des pages pour sa première liste (S4 : 300 ms). Les fenêtres
/// filles sont lues avant (quelques millisecondes) ; UI Automation n'a que ce qui reste. Ce
/// qu'il rend plus tard est encore proposé, jusqu'à `PAGES_TARDIVES_MAX` (`chercher_pages`).
pub const BUDGET_PAGES: Duration = Duration::from_millis(300);

/// Une page qui défile, trouvée dans une fenêtre visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageTrouvee {
    /// La partie qui défile, en pixels réels du bureau, entièrement visible (rien d'une
    /// fenêtre plus au premier plan ne la couvre).
    pub rect: Rect,
    /// La fenêtre (Windows : le HWND) qui reçoit le défilement : la fenêtre fille de la
    /// page, ou la fenêtre de premier niveau quand la page vient d'UI Automation.
    pub fenetre: isize,
}

/// Les instants d'avant la recherche, pour le journal : la demande (fil principal, avant
/// le lancement du fil `defilante-pages`) et le début du fil (avant la liste des fenêtres).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Depart {
    pub demandee: Instant,
    pub fil: Instant,
}

impl Depart {
    /// Les deux instants à maintenant (tests, appel direct).
    pub fn maintenant() -> Depart {
        let t = Instant::now();
        Depart {
            demandee: t,
            fil: t,
        }
    }
}

/// Prépare UI Automation pour la recherche des pages (bassin de fils MTA, chacun avec son
/// client, créés une fois et gardés tant qu'ils servent) sans attendre. À appeler au début
/// d'une visée défilante (jamais sans licence : contrat §6) ; `chercher_pages` le fait de
/// toute façon. Rien ailleurs que sous Windows.
pub fn preparer_pages() {
    plateforme::preparer_pages()
}

/// Cherche les pages qui défilent dans les `max` premières fenêtres de `fenetres` (du
/// premier plan vers le fond) (S4, §18 : fenêtres filles d'abord, UI Automation en
/// complément, une fenêtre par fil du bassin : une fenêtre lente ne retient pas les autres).
/// `rappel` reçoit la première liste dès que toutes les fenêtres ont répondu, au plus tard
/// à `BUDGET_PAGES` (même vide), puis chaque fois que des pages arrivées en retard la font
/// grandir, jusqu'à `PAGES_TARDIVES_MAX` : une liste reçue commence toujours par la
/// précédente (les pages tardives sont ajoutées à la fin, `pages.rs`). Dans une même
/// fenêtre, la plus petite d'abord (la plus précise sous la souris). Rend quand plus rien
/// ne viendra (au plus `PAGES_TARDIVES_MAX` après le début). À appeler hors du fil
/// principal (fil `defilante-pages`) ; la sélection ne l'attend jamais. Ailleurs que sous
/// Windows, une seule liste, vide.
pub fn chercher_pages(
    fenetres: &[FenetreInfo],
    max: usize,
    depart: Depart,
    mut rappel: impl FnMut(&[PageTrouvee]),
) {
    plateforme::chercher_pages(fenetres, max, depart, &mut rappel)
}

/// La première liste de `chercher_pages` (sans les pages tardives) : rend au plus tard à
/// `BUDGET_PAGES`.
pub fn pages(fenetres: &[FenetreInfo], max: usize) -> Vec<PageTrouvee> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut premiere = Some(tx);
    let fenetres = fenetres.to_vec();
    // La recherche continue après la première liste (les pages tardives) : dans un fil à
    // elle, qui finit seul.
    let lance = std::thread::Builder::new()
        .name("defilante-pages-premiere".into())
        .spawn(move || {
            chercher_pages(&fenetres, max, Depart::maintenant(), |l| {
                if let Some(tx) = premiere.take() {
                    let _ = tx.send(l.to_vec());
                }
            })
        });
    match lance {
        Ok(_) => rx.recv().unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Un moyen de faire défiler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moyen {
    /// UI Automation, `ScrollPattern.Scroll(SmallIncrement)`.
    Automation,
    /// Molette `SendInput` marquée (`MARQUE`), le pointeur sur la zone.
    Molette,
    /// `WM_MOUSEWHEEL` posté à la fenêtre fille sous le centre de la zone.
    Message,
}

impl Moyen {
    /// L'ordre d'essai (R3).
    pub const ORDRE: [Moyen; 3] = [Moyen::Message, Moyen::Automation, Moyen::Molette];

    /// Nom écrit au journal : `automation`, `molette`, `message`.
    pub fn nom(self) -> &'static str {
        match self {
            Moyen::Automation => "automation",
            Moyen::Molette => "molette",
            Moyen::Message => "message",
        }
    }

    /// Sa place dans `ORDRE`.
    fn rang(self) -> usize {
        match self {
            Moyen::Message => 0,
            Moyen::Automation => 1,
            Moyen::Molette => 2,
        }
    }

    /// Au plus tant d'unités (crans, ou pas d'UI Automation) pour une seule commande.
    #[cfg_attr(not(windows), allow(dead_code))]
    fn unites_max(self) -> i32 {
        match self {
            Moyen::Message | Moyen::Molette => 8,
            Moyen::Automation => 32,
        }
    }
}

/// La position lue de la page (UI Automation), quand elle existe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    /// `VerticalScrollPercent`, de 0 à 100.
    pub pour_cent: f64,
    /// `VerticalViewSize`, la part visible de la page, de 0 à 100.
    pub vue: f64,
    /// La hauteur de la vue qui défile, en pixels réels : le rectangle de l'élément UI
    /// Automation qui porte le `ScrollPattern`, lu avec la position (une barre horizontale
    /// éventuelle y est comprise).
    pub hauteur_vue: i32,
}

/// Le décalage de la page entre deux positions lues, en pixels réels, positif quand la page
/// est descendue : `(après − avant) / 100 × hauteur_vue × (100 / vue − 1)`, la page faisant
/// `hauteur_vue × 100 / vue`. `None` si une vue est inutilisable (`vue` hors de ]0, 100[,
/// hauteur nulle) ou si la hauteur de la vue a changé de plus de 2 px entre les deux (vue
/// redimensionnée).
pub fn decalage_px(avant: &Position, apres: &Position) -> Option<i32> {
    if avant.hauteur_vue <= 0 || (avant.hauteur_vue - apres.hauteur_vue).abs() > 2 {
        return None;
    }
    let d = pixels_entre(*avant, *apres, apres.hauteur_vue)?;
    (d.abs() < f64::from(i32::MAX)).then(|| d.round() as i32)
}

/// Ce qu'a fait une demande de pas. Ne dit jamais si la page a bougé : seules les images le
/// disent (R3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Envoi {
    /// Le pas est parti.
    Envoye,
    /// Rien n'est parti : une garde l'a retenu (touche tenue, autre fenêtre sous le centre
    /// ou le pointeur, pointeur sur « Terminer », menu ouvert…), UI Automation n'est pas
    /// encore prêt ou n'a pas répondu à temps, il n'y a plus aucun moyen (`Defileur::moyen`
    /// est `None`), ou le pas demandé est nul. À retenter au tour suivant.
    Saute,
}

/// Ce qui fait défiler une page, le temps d'une capture. Vit dans le fil `defilante` ; UI
/// Automation tourne dans un fil MTA à lui (`defilante-automation`), qui finit avec le
/// défileur. Attentes au plus : `descendre` 280 ms (moyen `Automation` : une position lue,
/// 80 ms, puis le pas, 200 ms ; les autres moyens n'attendent pas), `position` 80 ms ;
/// `preparer`, `abandonner`, `constater`, `moyen`, `envoyes`, `avance_par_cran`
/// n'attendent jamais UI Automation. « Terminer », Entrée et Échap ne sont donc jamais
/// retardés de plus de 280 ms par le défileur.
#[derive(Debug)]
pub struct Defileur {
    interne: plateforme::Defileur,
}

/// Prépare le défilement de `zone` (pixels réels du bureau) ; `fenetre` : celle de la page
/// cliquée (`PageTrouvee::fenetre`), `None` pour une zone tracée (la fenêtre sous le
/// centre de la zone). Rapide : la recherche UI Automation part dans son fil (Chromium
/// n'ouvre son arbre qu'à la première demande, 0,3 à 0,4 s) et n'est attendue que si le
/// moyen `Automation` doit servir.
pub fn preparer(zone: Rect, fenetre: Option<isize>) -> Result<Defileur, String> {
    plateforme::preparer(zone, fenetre).map(|interne| Defileur { interne })
}

impl Defileur {
    /// Le moyen en cours, `None` quand tous ont été abandonnés.
    pub fn moyen(&self) -> Option<Moyen> {
        plateforme::moyen(&self.interne)
    }

    /// Les commandes parties (`Envoi::Envoye`) avec le moyen en cours : le choix se fait
    /// sur les deux premières (R3).
    pub fn envoyes(&self) -> u32 {
        plateforme::envoyes(&self.interne)
    }

    /// « Rien n'a bougé, passe au suivant » (D8), ou « un cran est trop grand pour la
    /// zone » : laisse le moyen en cours pour le suivant de `Moyen::ORDRE` qui peut servir ;
    /// rend le nouveau, `None` s'il n'y en a plus. N'attend pas : si le suivant est
    /// `Automation` et que sa recherche n'est pas finie, il est rendu, et `descendre` dit
    /// `Saute` jusqu'à la fin de la recherche (1,5 s au plus après `preparer`).
    pub fn abandonner(&mut self) -> Option<Moyen> {
        plateforme::abandonner(&mut self.interne)
    }

    /// Ce que les images ont montré après la dernière commande partie, une fois par
    /// commande : la page a parcouru `avance` pixels réels dans le sens demandé (0 :
    /// immobile). Règle le nombre de crans des pas suivants (jamais plus que ce qui a été vu
    /// par cran) ; 0 après un cran partiel coupe les crans partiels de ce moyen (l'app les
    /// ignore). Facultatif : sans lui, chaque pas reste d'un cran (Automation : réglé sur la
    /// position lue).
    pub fn constater(&mut self, avance: u32) {
        plateforme::constater(&mut self.interne, avance)
    }

    /// Ce qu'un cran (`Message`, `Molette`) ou un `SmallIncrement` (`Automation`) du moyen
    /// en cours fait au plus, en pixels réels, d'après `constater` (et la position lue pour
    /// `Automation`) ; `None` tant que rien n'a été vu. `Molette` et `Automation` ne font
    /// jamais moins : si c'est plus que la hauteur utile moins `RECOUVREMENT_MIN`, chaque
    /// pas perd des lignes, l'appelant passe au suivant (`abandonner`). `Message` envoie un
    /// cran partiel (delta de moins de 120) pour un pas plus petit, tant que l'app le suit.
    pub fn avance_par_cran(&self) -> Option<u32> {
        plateforme::avance_par_cran(&self.interne)
    }

    /// Demande un pas de `pixels` pixels réels vers le bas (négatif : vers le haut). Le
    /// premier pas d'un moyen est d'une unité (un cran, un `SmallIncrement` ; pour `Message`,
    /// un demi-cran si la zone fait moins de 400 px de haut), les suivants sont réglés sur ce
    /// qu'une unité a fait. Si le moyen en cours ne peut plus rien
    /// envoyer (fenêtre fermée ou remplacée, message refusé, aucun `ScrollPattern`, UI
    /// Automation trois fois sans réponse), il est laissé ici même pour le suivant
    /// (`moyen()` le dit, l'appelant le compare à ce qu'il avait) ; sans plus aucun moyen,
    /// `Saute`. `Err` n'est rendu aujourd'hui dans aucun cas (gardé pour le contrat) ;
    /// ailleurs que sous Windows, aucun défileur n'existe.
    pub fn descendre(&mut self, pixels: i32) -> Result<Envoi, String> {
        plateforme::descendre(&mut self.interne, pixels)
    }

    /// La position lue (UI Automation, `ScrollPattern`), `None` si la page n'en donne pas
    /// (ou pas encore, ou pas dans les 80 ms).
    pub fn position(&self) -> Option<Position> {
        plateforme::position(&self.interne)
    }
}

/// Garde de la molette envoyée (D7, fonction pure) : vrai seulement si la fenêtre sous le
/// point visé a la même racine que la cible (`GetAncestor(GA_ROOT)`) et, quand « Faire
/// défiler les fenêtres inactives » est coupé (`survol_actif` faux), si la cible a le
/// premier plan.
pub fn peut_envoyer(
    racine_sous_point: Option<isize>,
    racine_cible: isize,
    premier_plan: Option<isize>,
    survol_actif: bool,
) -> bool {
    racine_cible != 0
        && racine_sous_point == Some(racine_cible)
        && (survol_actif || premier_plan == Some(racine_cible))
}

/// Ce que le système dit au moment d'un pas, lu par `windows.rs` (seulement ce dont le
/// moyen a besoin).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(not(windows), allow(dead_code))]
struct Constat {
    /// Ctrl, Maj, Alt, Win ou un bouton de souris est enfoncé.
    touches_tenues: bool,
    /// La racine de la fenêtre que la souris toucherait au centre de la zone.
    racine_sous_centre: Option<isize>,
    /// Le pointeur, `None` s'il n'a pas pu être lu.
    pointeur: Option<(i32, i32)>,
    /// La racine de la fenêtre sous le pointeur.
    racine_sous_pointeur: Option<isize>,
    /// La racine de la fenêtre au premier plan.
    premier_plan: Option<isize>,
    /// « Faire défiler les fenêtres inactives » est mis.
    survol_actif: bool,
    /// Le focus du fil de la cible est la cible, l'un de ses ancêtres ou l'une de ses
    /// descendantes.
    focus_dans_cible: bool,
    /// Capture de souris, menu, déplacement ou redimensionnement en cours dans le fil de la
    /// cible (ou illisible).
    fil_occupe: bool,
}

/// La garde d'un pas (pure) : `Envoye` si le pas peut partir par `moyen`, sinon `Saute`.
#[cfg_attr(not(windows), allow(dead_code))]
fn garde(moyen: Moyen, zone: Rect, racine_cible: isize, c: &Constat) -> Envoi {
    if c.touches_tenues {
        return Envoi::Saute;
    }
    let permis = match moyen {
        Moyen::Automation => true,
        Moyen::Message => racine_cible != 0 && c.racine_sous_centre == Some(racine_cible),
        Moyen::Molette => {
            c.pointeur.is_some_and(|p| zone.contient(p))
                && peut_envoyer(
                    c.racine_sous_pointeur,
                    racine_cible,
                    c.premier_plan,
                    c.survol_actif,
                )
                && !c.fil_occupe
                && (c.survol_actif || c.focus_dans_cible)
        }
    };
    if permis {
        Envoi::Envoye
    } else {
        Envoi::Saute
    }
}

/// « Faire défiler les fenêtres inactives » est-il mis ? (`SPI_GETMOUSEWHEELROUTING` :
/// 2 = sous le pointeur ; 0 = au focus, 1 = au focus pour les apps de bureau). Illisible :
/// coupé (le plus prudent).
#[cfg_attr(not(windows), allow(dead_code))]
fn survol_actif(routage: Option<u32>) -> bool {
    routage == Some(2)
}

/// Un cran de molette entendu, qui n'est pas de l'app (sans `MARQUE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Molette {
    /// Le pointeur, en pixels réels du bureau.
    pub point: (i32, i32),
    /// Crans : positif vers le bas de la page, négatif vers le haut. Un demi-cran (pavé
    /// tactile) compte pour un.
    pub crans: i32,
}

/// L'écoute de la molette (crochet `WH_MOUSE_LL` dans son fil `defilante-molette`, R4). La
/// détruire retire le crochet et arrête son fil.
#[derive(Debug)]
pub struct Ecoute {
    interne: plateforme::Ecoute,
}

/// Écoute la molette de l'utilisateur : `rappel` reçoit chaque cran sans la marque de
/// l'app. Le crochet ne fait que poster le cran (il ne bloque jamais : Windows retire sans
/// prévenir un crochet trop lent, `LowLevelHooksTimeout`), dans un fil à la priorité
/// « temps critique » ; `rappel` est appelé d'un fil à part (`defilante-molette-rappel`),
/// il ne doit que poster, sans attendre. Une fois l'`Ecoute` détruite, `rappel` n'est plus
/// jamais appelé ; sa destruction attend au plus 1 s, en tout, un appel en cours et l'arrêt
/// du crochet. Erreur ailleurs que sous Windows.
pub fn ecouter_molette(rappel: impl Fn(Molette) + Send + 'static) -> Result<Ecoute, String> {
    plateforme::ecouter_molette(Box::new(rappel)).map(|interne| Ecoute { interne })
}

impl Ecoute {
    /// Vrai depuis que le crochet est posé, jusqu'à ce que l'app le retire. Un retrait
    /// silencieux par Windows (`LowLevelHooksTimeout` dépassé) n'est pas vu.
    pub fn active(&self) -> bool {
        plateforme::active(&self.interne)
    }
}

/// Le rappel de l'écoute, tel que le reçoit la plateforme.
type RappelMolette = Box<dyn Fn(Molette) + Send + 'static>;

// ---------------------------------------------------------------------------------------
// Règles pures, utilisées par `windows.rs`.

/// Le choix du moyen pendant une capture (R3, D8) : l'ordre de `Moyen::ORDRE`, les moyens
/// qui ne peuvent pas servir sautés.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
struct Choix {
    /// Les moyens qui ne peuvent pas servir (par rang).
    exclus: [bool; 3],
    /// Rang du moyen en cours (3 : plus aucun).
    en_cours: usize,
    /// Commandes parties avec le moyen en cours.
    envoyes: u32,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl Choix {
    fn nouveau() -> Choix {
        Choix {
            exclus: [false; 3],
            en_cours: 0,
            envoyes: 0,
        }
    }

    fn moyen(&self) -> Option<Moyen> {
        Moyen::ORDRE.get(self.en_cours).copied()
    }

    fn envoyes(&self) -> u32 {
        self.envoyes
    }

    /// Passe au moyen suivant qui n'est pas exclu.
    fn avancer(&mut self) {
        self.en_cours = (self.en_cours + 1).min(Moyen::ORDRE.len());
        while self.en_cours < Moyen::ORDRE.len() && self.exclus[self.en_cours] {
            self.en_cours += 1;
        }
        self.envoyes = 0;
    }

    /// `m` ne peut pas servir ; s'il est en cours, le suivant le remplace. Rend le moyen en
    /// cours.
    fn exclure(&mut self, m: Moyen) -> Option<Moyen> {
        self.exclus[m.rang()] = true;
        if self.moyen() == Some(m) {
            self.avancer();
        }
        self.moyen()
    }

    /// Rien n'a bougé : le suivant.
    fn abandonner(&mut self) -> Option<Moyen> {
        if self.moyen().is_some() {
            self.avancer();
        }
        self.moyen()
    }

    fn envoye(&mut self) {
        self.envoyes = self.envoyes.saturating_add(1);
    }
}

/// Combien de pixels fait une unité d'un moyen (un cran, un `SmallIncrement`), d'après ce
/// qui a été vu. Inconnu : une unité par pas (le plus prudent). Une commande n'a jamais plus
/// de deux fois les unités de la précédente, plus une : une valeur fausse ne fait pas sauter
/// la page d'un coup.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(not(windows), allow(dead_code))]
struct Etalon {
    par_unite: Option<f64>,
    /// Unités de la dernière commande partie (sans signe).
    derniere: u32,
    /// Un cran partiel est resté sans effet : plus de crans partiels pour ce moyen.
    partiel_refuse: bool,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl Etalon {
    /// Les unités d'un pas de `pixels` (signe compris) : autant d'unités entières que le
    /// pas en contient, au moins une, au plus `max` et au plus `2 × dernière + 1` ; 0 pour
    /// un pas nul.
    fn unites(&self, pixels: i32, max: i32) -> i32 {
        if pixels == 0 {
            return 0;
        }
        let n = match self.par_unite {
            Some(p) if p >= 1.0 => (pixels.unsigned_abs() as f64 / p).floor() as i64,
            _ => 1,
        };
        let plafond = (self.derniere as i64 * 2 + 1).min(max.max(1) as i64);
        n.clamp(1, plafond) as i32 * pixels.signum()
    }

    /// Le delta d'un cran partiel (1 à 119, sans signe) pour un pas de `pixels` plus petit
    /// qu'une unité vue : `120 × |pixels| / unité`. Unité encore inconnue : un demi-cran au
    /// tout premier pas si la zone fait moins de `ZONE_DEMI_CRAN` px de haut (un cran entier
    /// d'Edge, 200 px, la dépasserait). `None` si le pas vaut au moins une unité, si un cran
    /// partiel est déjà resté sans effet, ou pour un pas nul.
    fn delta_partiel(&self, pixels: i32, hauteur_zone: i32) -> Option<i32> {
        if self.partiel_refuse || pixels == 0 {
            return None;
        }
        let Some(p) = self.par_unite else {
            return (self.derniere == 0 && hauteur_zone < ZONE_DEMI_CRAN).then_some(CRAN / 2);
        };
        let voulu = pixels.unsigned_abs() as f64;
        if voulu >= p {
            return None;
        }
        Some(((voulu * CRAN as f64 / p).round() as i32).clamp(1, CRAN - 1))
    }

    /// Une commande de `unites` unités entières est partie (un cran partiel compte pour une).
    fn envoye(&mut self, unites: i32) {
        self.derniere = unites.unsigned_abs();
    }

    /// `unites` (en unités, un cran partiel en fraction) ont fait `pixels` (même signe
    /// attendu). La plus grande valeur vue est gardée : un pas coupé par le bas de la page ne
    /// fait jamais envoyer plus de crans.
    fn constater(&mut self, unites: f64, pixels: f64) {
        if unites == 0.0 || !unites.is_finite() || !pixels.is_finite() {
            return;
        }
        let par = pixels / unites;
        if par >= 1.0 {
            self.par_unite = Some(self.par_unite.map_or(par, |p| p.max(par)));
        }
    }

    /// Un cran partiel est resté sans effet.
    fn refuser_partiel(&mut self) {
        self.partiel_refuse = true;
    }

    /// Ce qu'une unité fait au plus, en pixels.
    fn par_unite(&self) -> Option<u32> {
        self.par_unite.map(|p| p.round() as u32)
    }
}

/// Pixels parcourus entre deux positions lues, pour une vue de `hauteur_vue` pixels : la
/// page fait `hauteur_vue × 100 / vue`, la partie qui défile `hauteur_vue × (100 / vue −
/// 1)`. `None` sans vue utilisable.
#[cfg_attr(not(windows), allow(dead_code))]
fn pixels_entre(avant: Position, apres: Position, hauteur_vue: i32) -> Option<f64> {
    let vue = apres.vue;
    if !(vue > 0.0 && vue < 100.0) || hauteur_vue <= 0 {
        return None;
    }
    if avant.pour_cent < 0.0 || apres.pour_cent < 0.0 {
        return None;
    }
    let defilable = hauteur_vue as f64 * (100.0 / vue - 1.0);
    let d = (apres.pour_cent - avant.pour_cent) / 100.0 * defilable;
    d.is_finite().then_some(d)
}

/// Une position lue qui a un sens : la page défile (`VerticallyScrollable`) et le
/// pourcentage n'est pas `UIA_ScrollPatternNoScroll` (−1).
#[cfg_attr(not(windows), allow(dead_code))]
fn position_lue(defilable: bool, pour_cent: f64, vue: f64, hauteur_vue: i32) -> Option<Position> {
    (defilable && pour_cent.is_finite() && (0.0..=100.0).contains(&pour_cent) && vue.is_finite())
        .then_some(Position {
            pour_cent,
            vue,
            hauteur_vue,
        })
}

/// Au-dessous de cette hauteur de zone (pixels réels), le premier pas d'un message posté est
/// un demi-cran.
#[cfg_attr(not(windows), allow(dead_code))]
const ZONE_DEMI_CRAN: i32 = 400;

/// Valeur d'un cran (`WHEEL_DELTA`).
#[cfg_attr(not(windows), allow(dead_code))]
const CRAN: i32 = 120;

/// Crans d'un mouvement de molette `delta` (Windows : positif quand la molette est poussée,
/// la page monte). Rend des crans positifs vers le bas de la page ; un demi-cran compte
/// pour un ; 0 pour 0.
#[cfg_attr(not(windows), allow(dead_code))]
fn crans_de(delta: i16) -> i32 {
    if delta == 0 {
        return 0;
    }
    let n = (i32::from(delta).abs() / CRAN).max(1);
    if delta < 0 {
        n
    } else {
        -n
    }
}

/// Ce que le crochet garde d'un message de molette : rien s'il porte la marque de l'app ou
/// s'il est nul. `donnee` : `mouseData` (le mouvement dans le mot haut), `extra` :
/// `dwExtraInfo`.
#[cfg_attr(not(windows), allow(dead_code))]
fn cran_entendu(donnee: u32, extra: usize, point: (i32, i32)) -> Option<Molette> {
    if extra == MARQUE {
        return None;
    }
    let crans = crans_de((donnee >> 16) as u16 as i16);
    (crans != 0).then_some(Molette { point, crans })
}

/// `wParam` et `lParam` d'un `WM_MOUSEWHEEL` posté : `delta` dans le mot haut de `wParam`
/// (aucune touche), le point en coordonnées écran dans `lParam` (x en bas, y en haut, sur 16
/// bits signés).
#[cfg_attr(not(windows), allow(dead_code))]
fn parametres_molette(delta: i16, point: (i32, i32)) -> (usize, isize) {
    let wparam = (delta as u16 as usize) << 16;
    let bas = point.0 as i16 as u16 as u32;
    let haut = point.1 as i16 as u16 as u32;
    let lparam = ((haut << 16) | bas) as i32 as isize;
    (wparam, lparam)
}

/// La plus grande partie de `page` que `devant` ne couvre pas, d'un seul tenant (une bande
/// au-dessus, au-dessous, à gauche ou à droite de `devant`). `None` si rien ne reste.
#[cfg_attr(not(windows), allow(dead_code))]
fn retirer(page: Rect, devant: Rect) -> Option<Rect> {
    let Some(commun) = page.intersection(devant) else {
        return (!page.est_vide()).then_some(page);
    };
    let (g, h, d, b) = (page.x as i64, page.y as i64, page.droite(), page.bas());
    let (cg, ch, cd, cb) = (
        commun.x as i64,
        commun.y as i64,
        commun.droite(),
        commun.bas(),
    );
    let bandes = [
        (g, h, d, ch), // au-dessus
        (g, cb, d, b), // au-dessous
        (g, h, cg, b), // à gauche
        (cd, h, d, b), // à droite
    ];
    bandes
        .iter()
        .map(|&(g, h, d, b)| Rect::depuis_bords(g as i32, h as i32, d as i32, b as i32))
        .filter(|r| !r.est_vide())
        .max_by_key(|r| r.largeur as i64 * r.hauteur as i64)
}

/// Côté au-dessous duquel une page trouvée n'est pas gardée (pixels réels) ; le cœur
/// applique ensuite sa propre règle (D2, 120 px logiques).
#[cfg_attr(not(windows), allow(dead_code))]
const COTE_TROUVE_MIN: i32 = 16;

/// Deux pages d'une même fenêtre à 2 px près sont la même.
#[cfg_attr(not(windows), allow(dead_code))]
fn presque_egaux(a: Rect, b: Rect) -> bool {
    (a.x - b.x).abs() <= 2
        && (a.y - b.y).abs() <= 2
        && (a.droite() - b.droite()).abs() <= 2
        && (a.bas() - b.bas()).abs() <= 2
}

/// La liste des pages (pure) : `bords[i]` est le rectangle de la fenêtre `i` (premier plan
/// d'abord), `trouvees[i]` ses pages (`pages_de_fenetre`). Les fenêtres restent dans leur
/// ordre. C'est la liste qu'un `Suivi` propose quand toutes les fenêtres ont répondu avant
/// le budget (`pages.rs`).
#[cfg(test)]
fn assembler(bords: &[Rect], trouvees: &[Vec<PageTrouvee>]) -> Vec<PageTrouvee> {
    trouvees
        .iter()
        .enumerate()
        .take(bords.len())
        .flat_map(|(i, pages)| pages_de_fenetre(bords, i, pages))
        .collect()
}

/// Les pages gardées de la fenêtre `i` (pure) : chacune coupée à sa fenêtre (`bords[i]`),
/// puis à ce que les fenêtres devant elle (`bords[..i]`) laissent voir (`retirer`) ; trop
/// petite, en double ou couverte, elle part. La plus petite d'abord. Ne dépend que des
/// bords et de ces pages : deux fenêtres n'ont jamais de page au même endroit.
#[cfg_attr(not(windows), allow(dead_code))]
fn pages_de_fenetre(bords: &[Rect], i: usize, pages: &[PageTrouvee]) -> Vec<PageTrouvee> {
    let Some(&bord) = bords.get(i) else {
        return Vec::new();
    };
    let mut gardees: Vec<PageTrouvee> = Vec::new();
    for page in pages {
        let mut rect = page.rect.intersection(bord);
        for devant in &bords[..i] {
            rect = rect.and_then(|r| retirer(r, *devant));
        }
        let Some(rect) = rect else {
            continue;
        };
        if rect.largeur < COTE_TROUVE_MIN || rect.hauteur < COTE_TROUVE_MIN {
            continue;
        }
        if gardees.iter().any(|g| presque_egaux(g.rect, rect)) {
            continue;
        }
        gardees.push(PageTrouvee {
            rect,
            fenetre: page.fenetre,
        });
    }
    gardees.sort_by_key(|p| p.rect.largeur as i64 * p.rect.hauteur as i64);
    gardees
}

/// Une fenêtre fille qui est une page par sa seule classe : le rendu de Chromium (Edge,
/// Chrome, Electron, WebView2), la liste des fichiers de l'Explorateur (`DirectUIHWND` dans
/// `SHELLDLL_DefView`).
#[cfg_attr(not(windows), allow(dead_code))]
fn page_par_classe(classe: &str, classe_parent: &str) -> bool {
    classe == "Chrome_RenderWidgetHostHWND"
        || (classe == "DirectUIHWND" && classe_parent == "SHELLDLL_DefView")
}

/// Une barre de défilement verticale qui a de quoi défiler (`GetScrollInfo` : `nMin`,
/// `nMax`, `nPage`).
#[cfg_attr(not(windows), allow(dead_code))]
fn barre_utile(min: i32, max: i32, page: u32) -> bool {
    let etendue = max as i64 - min as i64 + 1;
    if page == 0 {
        max > min
    } else {
        etendue > page as i64
    }
}

/// Le rectangle d'un élément UI Automation qui défile, sans sa barre verticale quand elle
/// est l'une de `barres` : plus haute que large, collée au bord droit (à 2 px près), au
/// moins la moitié de la hauteur (UI Automation compte la barre dans l'élément, S4 : 17 px
/// de trop sur WPF).
#[cfg_attr(not(windows), allow(dead_code))]
fn sans_barre(element: Rect, barres: &[Rect]) -> Rect {
    let mut r = element;
    for b in barres {
        let verticale = b.hauteur > b.largeur && b.hauteur * 2 >= element.hauteur;
        let a_droite = (b.droite() - element.droite()).abs() <= 2;
        if verticale && a_droite && b.x > r.x && (b.x as i64) < r.droite() {
            r.largeur = b.x - r.x;
        }
    }
    r
}

/// L'indice du rectangle qui recouvre le plus `zone` (le plus petit à égalité) : l'élément
/// qui défile est la page, pas un bloc intérieur.
#[cfg_attr(not(windows), allow(dead_code))]
fn meilleur_recouvrement(rects: &[Rect], zone: Rect) -> Option<usize> {
    let aire = |r: Rect| r.largeur as i64 * r.hauteur as i64;
    rects
        .iter()
        .enumerate()
        .filter_map(|(i, r)| r.intersection(zone).map(|c| (i, aire(c), aire(*r))))
        .max_by(|a, b| a.1.cmp(&b.1).then(b.2.cmp(&a.2)))
        .map(|(i, _, _)| i)
}

/// La classe d'une fenêtre telle qu'elle va au journal : coupée au premier `[` (WPF y met
/// le nom de l'exécutable), « aucune » si vide.
#[cfg_attr(not(windows), allow(dead_code))]
fn classe_pour_journal(classe: &str) -> &str {
    let c = classe.split('[').next().unwrap_or("");
    if c.is_empty() {
        "aucune"
    } else {
        c
    }
}

/// L'indice du plus petit rectangle qui contient `point`.
#[cfg_attr(not(windows), allow(dead_code))]
fn plus_petit_sous(rects: &[Rect], point: (i32, i32)) -> Option<usize> {
    rects
        .iter()
        .enumerate()
        .filter(|(_, r)| r.contient(point))
        .min_by_key(|(_, r)| r.largeur as i64 * r.hauteur as i64)
        .map(|(i, _)| i)
}

/// Le centre d'un rectangle.
#[cfg_attr(not(windows), allow(dead_code))]
fn centre(r: Rect) -> (i32, i32) {
    (
        (r.x as i64 + r.largeur as i64 / 2) as i32,
        (r.y as i64 + r.hauteur as i64 / 2) as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i32, y: i32, largeur: i32, hauteur: i32) -> Rect {
        Rect {
            x,
            y,
            largeur,
            hauteur,
        }
    }

    // --- Garde de la molette envoyée (D7) ---

    #[test]
    fn peut_envoyer_meme_racine_survol_mis() {
        assert!(peut_envoyer(Some(7), 7, None, true));
        assert!(peut_envoyer(Some(7), 7, Some(9), true));
    }

    #[test]
    fn peut_envoyer_refuse_une_autre_racine() {
        // Le pointeur sur « Terminer » (fenêtre de contrôle) ou sur une autre app.
        assert!(!peut_envoyer(Some(8), 7, Some(7), true));
        assert!(!peut_envoyer(None, 7, Some(7), true));
    }

    #[test]
    fn peut_envoyer_survol_coupe_demande_le_premier_plan() {
        assert!(!peut_envoyer(Some(7), 7, None, false));
        assert!(!peut_envoyer(Some(7), 7, Some(9), false));
        assert!(peut_envoyer(Some(7), 7, Some(7), false));
    }

    #[test]
    fn peut_envoyer_jamais_sans_cible() {
        assert!(!peut_envoyer(Some(0), 0, Some(0), true));
    }

    fn constat_molette() -> Constat {
        Constat {
            pointeur: Some((150, 120)),
            racine_sous_pointeur: Some(7),
            racine_sous_centre: Some(7),
            premier_plan: Some(7),
            survol_actif: true,
            focus_dans_cible: true,
            ..Constat::default()
        }
    }

    #[test]
    fn garde_molette_saute_hors_de_la_zone() {
        let zone = r(100, 100, 200, 150);
        let mut c = constat_molette();
        assert_eq!(garde(Moyen::Molette, zone, 7, &c), Envoi::Envoye);
        c.pointeur = Some((50, 50));
        assert_eq!(garde(Moyen::Molette, zone, 7, &c), Envoi::Saute);
        // Bord droit et bord bas exclus.
        c.pointeur = Some((300, 120));
        assert_eq!(garde(Moyen::Molette, zone, 7, &c), Envoi::Saute);
        c.pointeur = None;
        assert_eq!(garde(Moyen::Molette, zone, 7, &c), Envoi::Saute);
    }

    #[test]
    fn garde_molette_sur_la_zone_et_la_cible() {
        let zone = r(100, 100, 200, 150);
        // Sur la zone mais une autre fenêtre dessus (la pilule).
        let mut c = constat_molette();
        c.racine_sous_pointeur = Some(3);
        assert_eq!(garde(Moyen::Molette, zone, 7, &c), Envoi::Saute);
        // Survol coupé, la cible n'a pas le premier plan.
        let mut c = constat_molette();
        c.survol_actif = false;
        c.premier_plan = Some(3);
        assert_eq!(garde(Moyen::Molette, zone, 7, &c), Envoi::Saute);
        // Survol coupé, premier plan mais le focus ailleurs.
        c.premier_plan = Some(7);
        c.focus_dans_cible = false;
        assert_eq!(garde(Moyen::Molette, zone, 7, &c), Envoi::Saute);
        c.focus_dans_cible = true;
        assert_eq!(garde(Moyen::Molette, zone, 7, &c), Envoi::Envoye);
        // Survol mis : le focus n'importe pas.
        let mut c = constat_molette();
        c.focus_dans_cible = false;
        assert_eq!(garde(Moyen::Molette, zone, 7, &c), Envoi::Envoye);
    }

    #[test]
    fn garde_molette_saute_si_le_fil_est_occupe() {
        // Menu ouvert, capture de souris, déplacement en cours.
        let mut c = constat_molette();
        c.fil_occupe = true;
        assert_eq!(
            garde(Moyen::Molette, r(100, 100, 200, 150), 7, &c),
            Envoi::Saute
        );
    }

    #[test]
    fn garde_rien_ne_part_touche_tenue() {
        let zone = r(100, 100, 200, 150);
        let mut c = constat_molette();
        c.touches_tenues = true;
        for m in Moyen::ORDRE {
            assert_eq!(garde(m, zone, 7, &c), Envoi::Saute, "{m:?}");
        }
        c.touches_tenues = false;
        for m in Moyen::ORDRE {
            assert_eq!(garde(m, zone, 7, &c), Envoi::Envoye, "{m:?}");
        }
    }

    #[test]
    fn garde_du_message_au_centre() {
        let zone = r(100, 100, 200, 150);
        // Le message ne regarde ni le pointeur ni le premier plan.
        let mut c = Constat {
            racine_sous_centre: Some(7),
            ..Constat::default()
        };
        assert_eq!(garde(Moyen::Message, zone, 7, &c), Envoi::Envoye);
        // Une fenêtre posée devant le centre de la zone.
        c.racine_sous_centre = Some(3);
        assert_eq!(garde(Moyen::Message, zone, 7, &c), Envoi::Saute);
        c.racine_sous_centre = None;
        assert_eq!(garde(Moyen::Message, zone, 7, &c), Envoi::Saute);
        c.racine_sous_centre = Some(0);
        assert_eq!(garde(Moyen::Message, zone, 0, &c), Envoi::Saute);
        // UI Automation ne passe pas par la souris.
        assert_eq!(
            garde(Moyen::Automation, zone, 7, &Constat::default()),
            Envoi::Envoye
        );
    }

    #[test]
    fn survol_seulement_sous_le_pointeur() {
        assert!(survol_actif(Some(2)));
        assert!(!survol_actif(Some(0)));
        assert!(!survol_actif(Some(1)));
        assert!(!survol_actif(None));
    }

    // --- Ordre des moyens et passage au suivant (R3, D8) ---

    #[test]
    fn ordre_des_moyens() {
        assert_eq!(
            Moyen::ORDRE,
            [Moyen::Message, Moyen::Automation, Moyen::Molette]
        );
        for (i, m) in Moyen::ORDRE.iter().enumerate() {
            assert_eq!(m.rang(), i);
        }
        assert_eq!(Moyen::Message.nom(), "message");
        assert_eq!(Moyen::Automation.nom(), "automation");
        assert_eq!(Moyen::Molette.nom(), "molette");
        assert_eq!(Choix::nouveau().moyen(), Some(Moyen::Message));
    }

    #[test]
    fn abandonner_passe_au_suivant_jusqu_a_aucun() {
        let mut c = Choix::nouveau();
        assert_eq!(c.abandonner(), Some(Moyen::Automation));
        assert_eq!(c.abandonner(), Some(Moyen::Molette));
        assert_eq!(c.abandonner(), None);
        assert_eq!(c.abandonner(), None);
        assert_eq!(c.moyen(), None);
    }

    #[test]
    fn un_moyen_exclu_est_saute() {
        let mut c = Choix::nouveau();
        assert_eq!(c.exclure(Moyen::Automation), Some(Moyen::Message));
        assert_eq!(c.abandonner(), Some(Moyen::Molette));

        let mut c = Choix::nouveau();
        // Aucune fenêtre fille : le message ne peut pas servir, Automation d'emblée.
        assert_eq!(c.exclure(Moyen::Message), Some(Moyen::Automation));
        assert_eq!(c.exclure(Moyen::Automation), Some(Moyen::Molette));
        assert_eq!(c.exclure(Moyen::Molette), None);
    }

    #[test]
    fn abandonner_passe_au_suivant_meme_apres_un_effet() {
        // Un cran trop grand pour la zone : l'appelant laisse un moyen qui a fait bouger.
        let mut c = Choix::nouveau();
        c.envoye();
        c.envoye();
        assert_eq!(c.envoyes(), 2);
        assert_eq!(c.abandonner(), Some(Moyen::Automation));
        assert_eq!(c.envoyes(), 0);
    }

    #[test]
    fn les_envois_se_comptent_par_moyen() {
        let mut c = Choix::nouveau();
        c.envoye();
        assert_eq!(c.envoyes(), 1);
        c.abandonner();
        assert_eq!(c.envoyes(), 0);
        c.envoye();
        assert_eq!(c.exclure(Moyen::Automation), Some(Moyen::Molette));
        assert_eq!(c.envoyes(), 0);
    }

    // --- Unités d'un pas ---

    #[test]
    fn premier_pas_d_une_unite() {
        let e = Etalon::default();
        assert_eq!(e.unites(225, 8), 1);
        assert_eq!(e.unites(-225, 8), -1);
        assert_eq!(e.unites(0, 8), 0);
    }

    #[test]
    fn pas_regles_sur_ce_qu_un_cran_a_fait() {
        let mut e = Etalon::default();
        e.envoye(1);
        e.constater(1.0, 200.0);
        assert_eq!(e.unites(225, 8), 1);
        assert_eq!(e.unites(450, 8), 2);
        assert_eq!(e.unites(100, 8), 1, "jamais moins d'une unité");
        assert_eq!(e.unites(-450, 8), -2);
        e.envoye(3);
        assert_eq!(e.unites(100_000, 8), 7, "deux fois la dernière, plus une");
        e.envoye(7);
        assert_eq!(e.unites(100_000, 8), 8, "borné");
    }

    #[test]
    fn les_unites_montent_par_paliers() {
        // WPF : 16 px par `SmallIncrement`, pas de 225 px (14 unités) atteint en 3 pas.
        let mut e = Etalon::default();
        let premiere = e.unites(225, 64);
        assert_eq!(premiere, 1);
        e.envoye(premiere);
        e.constater(premiere as f64, 16.0);
        let mut suite = Vec::new();
        for _ in 0..4 {
            let n = e.unites(225, 64);
            e.envoye(n);
            suite.push(n);
        }
        assert_eq!(suite, vec![3, 7, 14, 14]);
    }

    #[test]
    fn la_plus_grande_valeur_vue_est_gardee() {
        let mut e = Etalon::default();
        e.constater(2.0, 200.0);
        e.constater(1.0, 48.0); // coupé par le bas de la page
        assert_eq!(e.par_unite, Some(100.0));
        e.constater(-1.0, -120.0);
        assert_eq!(e.par_unite, Some(120.0));
        // Sens contraire, nul, infini : ignorés.
        e.constater(1.0, -300.0);
        e.constater(1.0, 0.0);
        e.constater(0.0, 500.0);
        e.constater(1.0, f64::NAN);
        assert_eq!(e.par_unite, Some(120.0));
        assert_eq!(e.par_unite(), Some(120));
        // Un cran partiel (delta 60) qui a fait 70 px : 140 px par cran.
        e.constater(0.5, 70.0);
        assert_eq!(e.par_unite(), Some(140));
    }

    #[test]
    fn cran_partiel_pour_un_pas_plus_petit_qu_un_cran() {
        let mut e = Etalon::default();
        assert_eq!(e.delta_partiel(100, 600), None, "cran inconnu, grande zone");
        e.constater(1.0, 600.0);
        assert_eq!(e.delta_partiel(300, 600), Some(60));
        assert_eq!(e.delta_partiel(-300, 600), Some(60), "sans signe");
        assert_eq!(e.delta_partiel(1, 600), Some(1), "au moins 1");
        assert_eq!(e.delta_partiel(599, 600), Some(119), "au plus 119");
        assert_eq!(e.delta_partiel(600, 600), None, "un cran entier");
        assert_eq!(e.delta_partiel(0, 600), None);
        e.refuser_partiel();
        assert_eq!(
            e.delta_partiel(300, 600),
            None,
            "l'app ignore les crans partiels"
        );
    }

    #[test]
    fn demi_cran_au_premier_pas_d_une_petite_zone() {
        let mut e = Etalon::default();
        assert_eq!(e.delta_partiel(100, 399), Some(CRAN / 2));
        assert_eq!(e.delta_partiel(-100, 240), Some(CRAN / 2));
        assert_eq!(e.delta_partiel(100, 400), None, "zone assez haute");
        assert_eq!(e.delta_partiel(0, 240), None);
        // Seulement le tout premier pas.
        e.envoye(1);
        assert_eq!(e.delta_partiel(100, 240), None);
        // Un demi-cran resté sans effet : plus de demi-cran.
        let mut e = Etalon::default();
        e.refuser_partiel();
        assert_eq!(e.delta_partiel(100, 240), None);
        // Un demi-cran qui a fait 100 px : 200 px par cran, le pas suivant en partiel.
        let mut e = Etalon::default();
        e.envoye(1);
        e.constater(0.5, 100.0);
        assert_eq!(e.par_unite(), Some(200));
        assert_eq!(e.delta_partiel(150, 240), Some(90));
    }

    fn pos(pour_cent: f64, vue: f64, hauteur_vue: i32) -> Position {
        Position {
            pour_cent,
            vue,
            hauteur_vue,
        }
    }

    #[test]
    fn decalage_en_pixels_avec_la_hauteur_de_la_vue() {
        // Vue de 450 px qui montre 10 % de la page : page de 4 500 px, 4 050 px qui
        // défilent ; 10 % de plus = 405 px.
        let a = pos(0.0, 10.0, 450);
        let b = pos(10.0, 10.0, 450);
        assert_eq!(decalage_px(&a, &b), Some(405));
        assert_eq!(decalage_px(&b, &a), Some(-405), "vers le haut : négatif");
        assert_eq!(decalage_px(&a, &a), Some(0));
        // Vue de 600 px qui montre 25 % : page de 2 400 px, 1 800 qui défilent ; 2,5 % = 45.
        assert_eq!(
            decalage_px(&pos(40.0, 25.0, 600), &pos(42.5, 25.0, 600)),
            Some(45)
        );
        // Arrondi au pixel : 0,1 % de 1 800 = 1,8 px.
        assert_eq!(
            decalage_px(&pos(0.0, 25.0, 600), &pos(0.1, 25.0, 600)),
            Some(2)
        );
        // La hauteur à 2 px près (arrondis du rectangle) : gardée.
        assert_eq!(
            decalage_px(&a, &pos(10.0, 10.0, 452)).map(|d| d / 10),
            Some(40)
        );
    }

    #[test]
    fn pas_de_decalage_sans_vue_utilisable() {
        let a = pos(0.0, 10.0, 450);
        // Toute la page visible, ou vue illisible.
        assert_eq!(decalage_px(&a, &pos(10.0, 100.0, 450)), None);
        assert_eq!(decalage_px(&a, &pos(10.0, 0.0, 450)), None);
        assert_eq!(decalage_px(&a, &pos(10.0, f64::NAN, 450)), None);
        // Hauteur nulle, ou vue redimensionnée entre les deux.
        assert_eq!(decalage_px(&pos(0.0, 10.0, 0), &pos(10.0, 10.0, 0)), None);
        assert_eq!(decalage_px(&a, &pos(10.0, 10.0, 300)), None);
        assert_eq!(decalage_px(&a, &pos(10.0, 10.0, 447)), None);
    }

    #[test]
    fn pixels_entre_deux_positions() {
        let a = pos(0.0, 10.0, 450);
        let b = pos(10.0, 10.0, 450);
        // Page de 4 500 px pour une vue de 450 : 4 050 px qui défilent, 10 % = 405 px.
        let d = pixels_entre(a, b, 450).unwrap();
        assert!((d - 405.0).abs() < 1e-9, "{d}");
        assert!(pixels_entre(b, a, 450).unwrap() < 0.0);
        let sans_vue = pos(10.0, 100.0, 450);
        assert_eq!(pixels_entre(a, sans_vue, 450), None);
        assert_eq!(pixels_entre(a, b, 0), None);
    }

    #[test]
    fn position_lue_seulement_si_la_page_defile() {
        assert_eq!(
            position_lue(true, 42.0, 10.0, 317),
            Some(pos(42.0, 10.0, 317))
        );
        assert_eq!(position_lue(true, -1.0, 100.0, 317), None);
        assert_eq!(position_lue(false, 0.0, 100.0, 317), None);
        assert_eq!(position_lue(true, f64::NAN, 10.0, 317), None);
    }

    // --- Molette entendue, molette postée ---

    #[test]
    fn crans_vers_le_bas_positifs() {
        assert_eq!(crans_de(-120), 1);
        assert_eq!(crans_de(120), -1);
        assert_eq!(crans_de(-360), 3);
        assert_eq!(crans_de(240), -2);
        assert_eq!(crans_de(-12), 1, "un demi-cran compte");
        assert_eq!(crans_de(30), -1);
        assert_eq!(crans_de(0), 0);
    }

    #[test]
    fn le_cran_marque_est_ignore() {
        let bas = ((-120i16) as u16 as u32) << 16;
        assert_eq!(cran_entendu(bas, MARQUE, (5, 6)), None);
        assert_eq!(
            cran_entendu(bas, 0, (5, 6)),
            Some(Molette {
                point: (5, 6),
                crans: 1
            })
        );
        assert_eq!(cran_entendu(bas, 1, (5, 6)).map(|m| m.crans), Some(1));
        assert_eq!(cran_entendu(0, 0, (5, 6)), None);
    }

    #[test]
    fn parametres_du_message_poste() {
        let (w, l) = parametres_molette(-120, (-5, 300));
        assert_eq!((w >> 16) as u16 as i16, -120);
        assert_eq!(w & 0xFFFF, 0, "aucune touche");
        assert_eq!(l as u32 as u16 as i16, -5);
        assert_eq!((l as u32 >> 16) as u16 as i16, 300);
        let (w, l) = parametres_molette(240, (1500, -20));
        assert_eq!((w >> 16) as u16 as i16, 240);
        assert_eq!(l as u32 as u16 as i16, 1500);
        assert_eq!((l as u32 >> 16) as u16 as i16, -20);
    }

    // --- Pages proposées (S4) ---

    #[test]
    fn retirer_ce_qui_est_devant() {
        let page = r(0, 0, 400, 300);
        assert_eq!(retirer(page, r(500, 0, 10, 10)), Some(page));
        assert_eq!(retirer(page, r(-10, -10, 500, 500)), None);
        // Une fenêtre sur le haut : reste le bas.
        assert_eq!(
            retirer(page, r(-10, -10, 500, 110)),
            Some(r(0, 100, 400, 200))
        );
        // Un coin : la plus grande bande.
        assert_eq!(
            retirer(page, r(300, 250, 200, 200)),
            Some(r(0, 0, 400, 250))
        );
        assert_eq!(retirer(page, r(350, 0, 100, 300)), Some(r(0, 0, 350, 300)));
    }

    #[test]
    fn assembler_garde_l_ordre_des_fenetres_et_le_plus_petit_d_abord() {
        let bords = [r(0, 0, 500, 500), r(600, 0, 500, 500)];
        let trouvees = vec![
            vec![
                PageTrouvee {
                    rect: r(0, 0, 400, 400),
                    fenetre: 1,
                },
                PageTrouvee {
                    rect: r(10, 10, 100, 100),
                    fenetre: 2,
                },
            ],
            vec![PageTrouvee {
                rect: r(600, 0, 300, 300),
                fenetre: 3,
            }],
        ];
        let liste = assembler(&bords, &trouvees);
        let fenetres: Vec<isize> = liste.iter().map(|p| p.fenetre).collect();
        assert_eq!(fenetres, vec![2, 1, 3]);
    }

    #[test]
    fn assembler_coupe_a_la_fenetre_et_a_ce_qui_est_devant() {
        let bords = [r(0, 0, 300, 600), r(100, 0, 600, 600)];
        let trouvees = vec![
            vec![],
            vec![
                // Déborde de sa fenêtre à droite ; couverte à gauche par la fenêtre 0.
                PageTrouvee {
                    rect: r(100, 50, 700, 500),
                    fenetre: 5,
                },
                // Entièrement sous la fenêtre 0 : partie.
                PageTrouvee {
                    rect: r(120, 50, 100, 100),
                    fenetre: 6,
                },
            ],
        ];
        let liste = assembler(&bords, &trouvees);
        assert_eq!(
            liste,
            vec![PageTrouvee {
                rect: r(300, 50, 400, 500),
                fenetre: 5
            }]
        );
    }

    #[test]
    fn assembler_enleve_les_doubles_et_les_trop_petites() {
        let bords = [r(0, 0, 800, 800)];
        let trouvees = vec![vec![
            PageTrouvee {
                rect: r(10, 10, 400, 400),
                fenetre: 1,
            },
            PageTrouvee {
                rect: r(11, 9, 401, 402),
                fenetre: 2,
            },
            PageTrouvee {
                rect: r(500, 10, 10, 400),
                fenetre: 3,
            },
        ]];
        let liste = assembler(&bords, &trouvees);
        assert_eq!(liste.len(), 1);
        assert_eq!(liste[0].fenetre, 1);
        assert!(assembler(&[], &trouvees).is_empty());
    }

    #[test]
    fn pages_par_classe() {
        assert!(page_par_classe(
            "Chrome_RenderWidgetHostHWND",
            "Chrome_WidgetWin_1"
        ));
        assert!(page_par_classe("DirectUIHWND", "SHELLDLL_DefView"));
        assert!(!page_par_classe("DirectUIHWND", "CabinetWClass"));
        assert!(!page_par_classe("SysTreeView32", "NamespaceTreeControl"));
    }

    #[test]
    fn barre_qui_a_de_quoi_defiler() {
        assert!(barre_utile(0, 5999, 450));
        assert!(!barre_utile(0, 449, 450));
        assert!(!barre_utile(0, 100, 101));
        assert!(barre_utile(0, 100, 0));
        assert!(!barre_utile(0, 0, 0));
    }

    #[test]
    fn la_barre_verticale_n_est_pas_dans_la_page() {
        // WPF (S4) : élément de 489 px de large, barre de 17 px collée à droite.
        let element = r(8, 31, 489, 317);
        let barre = r(480, 31, 17, 317);
        let horizontale = r(8, 331, 472, 17);
        assert_eq!(
            sans_barre(element, &[horizontale, barre]),
            r(8, 31, 472, 317)
        );
        // Une barre au milieu (contenu) ne compte pas.
        assert_eq!(sans_barre(element, &[r(200, 31, 17, 317)]), element);
        assert_eq!(sans_barre(element, &[]), element);
    }

    #[test]
    fn l_element_qui_recouvre_le_plus_la_zone() {
        let zone = r(100, 100, 400, 300);
        // La page, un bloc intérieur qui défile, une barre latérale à côté.
        let rects = [r(0, 0, 50, 900), r(120, 120, 100, 100), r(90, 90, 500, 900)];
        assert_eq!(meilleur_recouvrement(&rects, zone), Some(2));
        // À égalité, le plus petit.
        let rects = [r(0, 0, 900, 900), r(100, 100, 400, 300)];
        assert_eq!(meilleur_recouvrement(&rects, zone), Some(1));
        assert_eq!(meilleur_recouvrement(&[r(0, 0, 10, 10)], zone), None);
        assert_eq!(meilleur_recouvrement(&[], zone), None);
    }

    #[test]
    fn classe_coupee_pour_le_journal() {
        assert_eq!(
            classe_pour_journal("HwndWrapper[Appli.exe;;3f2a-11]"),
            "HwndWrapper"
        );
        assert_eq!(
            classe_pour_journal("Chrome_WidgetWin_1"),
            "Chrome_WidgetWin_1"
        );
        assert_eq!(classe_pour_journal(""), "aucune");
        assert_eq!(classe_pour_journal("[x]"), "aucune");
    }

    #[test]
    fn plus_petit_rectangle_sous_le_point() {
        let rects = [r(0, 0, 500, 500), r(10, 10, 100, 100), r(600, 0, 50, 50)];
        assert_eq!(plus_petit_sous(&rects, (50, 50)), Some(1));
        assert_eq!(plus_petit_sous(&rects, (300, 300)), Some(0));
        assert_eq!(plus_petit_sous(&rects, (550, 550)), None);
        assert_eq!(centre(r(10, 20, 100, 51)), (60, 45));
    }

    #[test]
    fn budget_des_pages() {
        assert_eq!(BUDGET_PAGES, Duration::from_millis(300));
        assert_eq!(MARQUE, 0x4252_5A59);
    }

    #[cfg(not(windows))]
    #[test]
    fn hors_windows_rien_ne_defile() {
        assert!(pages(&[], 8).is_empty());
        assert!(preparer(r(0, 0, 100, 100), None).is_err());
        assert!(ecouter_molette(|_| {}).is_err());
    }
}
