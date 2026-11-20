// Sélection d'une zone (étape 5), d'une fenêtre (étape 8), du texte à lire (0.5) ou de ce
// qu'une capture défilante fera défiler (0.6). La page montre et rapporte, le
// cœur décide (ARCHITECTURE.md §2.5). Elle ne dessine que le voile et le rectangle, ou le
// contour de la fenêtre survolée : ni texte, ni viseur, ni taille, ni poignée ; seul le mode
// texte s'anime, une fois la zone choisie (lecture, puis lignes copiées surlignées).
//
// Échelle k (pixels réels par pixel CSS) : devicePixelRatio, l'échelle de rendu vraie, quand
// il explique la taille réelle de la fenêtre (envoyée par le cœur) à 1 px CSS près sur chaque
// axe ; sinon, taille réelle / taille CSS non arrondie (visualViewport, à défaut innerWidth).
// innerWidth est arrondi au pixel CSS entier : 1366 px à 125 % font 1092,8 px CSS, pas 1093.
//
// Contrat avec le cœur (fenetres/selection.rs) :
// - écoute `selection:montrer` { id, numero, largeur, hauteur, ecrans: [{ x, y, largeur,
//   hauteur }], cote_min, mode: "zone" | "fenetre" | "texte" | "defilante" | "film", fenetres: [{ x, y,
//   largeur, hauteur, rayon }], souris: { x, y } | null, suite? } : tout en pixels réels entiers,
//   relatifs au coin haut
//   gauche de la fenêtre (une fenêtre proposée peut en déborder ; `mode` absent vaut "zone",
//   `fenetres` absent vaut []) (`largeur`, `hauteur` : rectangle réel de la fenêtre ; `numero` : celui de la
//   charge, renvoyé avec chaque message). Retour au repos (voile plein). Dès que
//   innerWidth/innerHeight sont non nuls et identiques deux images de suite, la page retient
//   son échelle (kx, ky), règle son dessin, puis envoie `selection_prete` { id, numero,
//   largeur, hauteur (taille CSS sur laquelle l'échelle est retenue), echelle:
//   devicePixelRatio, kx, ky, attente }. Avant cela, aucun glisser n'est accepté. Une demande
//   invalide est ignorée (console.warn) : le cœur cache la fenêtre au bout de son délai. Une
//   même demande reçue de nouveau (zone refusée, fenêtre changée, page rechargée) remet la
//   page au repos et la fait mesurer de nouveau.
// - `resize` pendant un affichage : le glisser en cours est abandonné, la page redevient
//   « pas prête » jusqu'à la charge suivante ; elle attend la stabilité et renvoie
//   `selection_prete` avec `attente: true` : le cœur relit la fenêtre et renvoie toujours une
//   charge (nouvelle si la fenêtre a changé, sinon la même, même numéro).
// - écoute `selection:cachee` : retour au repos, plus rien n'est envoyé.
// - `selection_terminee` { id, numero, depart: { x, y }, arrivee: { x, y } } au relâchement :
//   les deux pixels finals, inclus (après carré, déplacement et bornage), en pixels réels entiers
//   relatifs à la fenêtre. Rien n'est redessiné ensuite : le cœur cache la fenêtre.
// - `selection_cliquee` { id, numero, x, y } (mode fenêtre) au relâchement d'un clic sur une
//   fenêtre entourée : le pixel réel sous la pointe, relatif à la fenêtre. Même attente que
//   `selection_terminee`.
// - `selection_annulee` { id, numero, cause: "echap" | "clic_droit" }, une seule fois par
//   affichage.
// - Mode texte (Copier le texte, 0.5) : une zone, exactement comme le mode zone ; seul le nom
//   change (« Copier le texte : sélectionnez une zone »). Après `selection_terminee`, le cœur
//   envoie `selection:balayer` { id, x, y, largeur, hauteur, reduites } (pixels réels entiers
//   relatifs à la fenêtre, comme `selection_terminee`) : la zone est posée là, se teinte
//   (`accent`, --texte-teinte) et respire (--texte-lecture) le temps de la lecture ;
//   `reduites` : la teinte seule, sans mouvement. La teinte reste jusqu'à la fin. Le filet
//   repart de ce moment. Si le texte est copié, le cœur envoie ensuite `selection:surligner`
//   { id, lignes: [{ x, y, largeur, hauteur }], reduites, trace, pas, tenue, sortie } (lignes
//   en pixels réels entiers relatifs à la fenêtre, durées en ms) : la ligne de rang r se
//   surligne de gauche à droite de r × pas à r × pas + trace (`reduites` : fondu, pas nul) ;
//   après tenue, toute la sélection s'efface en sortie. Le filet repart de là, augmenté du
//   surlignage. Rien n'est envoyé : le cœur cache la fenêtre à la fin de la lecture, ou du
//   surlignage.
// - Mode défilante (0.6) : le tracé du mode zone, plus le contour du mode fenêtre sur les
//   pages proposées (`fenetres`, vide au départ). Sous la pointe, la page entourée reste
//   claire (même règle que le mode fenêtre, souris de la charge comprise) ; un clic (même
//   seuil que le mode fenêtre) sur une page entourée : `selection_cliquee` ; un clic sur
//   rien : rien, on continue ; un glisser (seuil franchi) : le tracé, `selection_terminee`
//   comme une zone. Maj et Espace : comme le mode zone, pendant le tracé. Les pages
//   arrivent après (ou jamais) par une charge `suite: true` du même affichage et du même
//   numéro : la page prend la liste sans revenir au repos (geste en cours gardé) et entoure
//   aussitôt la page sous la pointe. Après l'envoi, la zone ou la page reste montrée (le
//   cœur garde la fenêtre jusqu'à la fenêtre de contrôle) : filet FILET_DEFILANTE_MS.
//   Nom : « Capture défilante : sélectionnez une zone ou une page ».
// - Mode film (0.7, vidéo et GIF) : exactement le mode zone ; seul le nom change
//   (« Enregistrer l'écran : sélectionnez une zone »). Après l'envoi, la zone reste montrée
//   (le cœur garde la fenêtre jusqu'à la fenêtre de contrôle du film, 2 s au plus,
//   `DELAI_PAGE` de application/film.rs) : filet FILET_FILM_MS.
// - `selection_chargee` {} une fois, quand les écoutes sont en place.
// Gestes : glisser du bouton gauche ; Maj tenue : carré ; Espace tenue : la zone se déplace ;
// Échap ou clic droit : annuler. Une zone reste sur l'écran où le glisser a commencé.
// Sous `cote_min` de côté, ce n'est pas une zone : le rectangle s'efface et on continue.
// Toute touche est neutralisée (F5, Ctrl+R, Ctrl+P, F12… ne font rien).
// Mode fenêtre : le pixel sous la pointe (à défaut, `souris` de la charge, tant qu'aucun
// mouvement n'est arrivé) → la première fenêtre de la liste qui le contient → coupée à
// l'écran de ce pixel → entourée si elle garde `cote_min` de côté. Un coin n'est arrondi
// que si c'est un vrai coin de la fenêtre. Clic gauche (appui puis relâchement presque au
// même endroit) sur une fenêtre entourée : `selection_cliquee`. Glisser : rien. Le seuil est
// en pixels logiques, comme le rectangle de glisser de Windows : c'est un glisser si le
// pointeur a bougé de max(cote_min, round(4 × k)) pixels réels ou plus sur un axe (4 à
// 100 %, 5 à 125 %, 8 à 200 %). Maj et Espace : rien. Échap, clic droit : annuler.
// Filet : si `selection:cachee` n'arrive pas dans FILET_MS après un envoi (ou si l'envoi
// est refusé), la page redevient utilisable.

const tauri = window.__TAURI__;
const corps = document.body;
const scene = document.getElementById("scene");
const morceaux = {
  haut: document.getElementById("haut"),
  bas: document.getElementById("bas"),
  gauche: document.getElementById("gauche"),
  droite: document.getElementById("droite"),
  zone: document.getElementById("zone"),
  fenetre: document.getElementById("fenetre"),
};

// Toute la géométrie est en pixels réels entiers, relatifs au coin de la fenêtre.
let id = null; // affichage en cours ; null : fenêtre cachée
let numero = 0; // numéro de la charge reçue
let redimensionnee = false; // taille changée : pas prête avant la charge suivante
let largeur = 0; // taille réelle de la fenêtre
let hauteur = 0;
let ecrans = [];
let coteMin = 0;
let kx = 1; // échelle mesurée : pixels réels par pixel CSS
let ky = 1;
let attente = false; // relâché ou annulé : on attend le cœur, plus rien ne bouge
let glisser = null; // { pointeur, ecran, depart, courant, souris }
let maj = false;
let espace = false;
let image = 0; // requestAnimationFrame en attente
let prete = false; // l'échelle est mesurée : on peut glisser
let envoi = 0; // numéro de la dernière demande reçue
let mesure = 0; // numéro de la dernière mesure lancée
let filet = 0; // minuterie du filet après un envoi
const FILET_MS = 1500;
// Mode texte : le gel reste jusqu'à 1,5 s après le relâcher (ECRAN_FIGE_MAX_LECTURE), et un
// surlignage commencé juste avant cette borne arrive un peu après : le filet du balayage
// garde cette marge de plus.
const FILET_TEXTE_MS = FILET_MS + 500;
// Mode défilante : le cœur garde la sélection jusqu'à la fenêtre de contrôle, 2 s au plus
// après la fin (D4, R1 : `DELAI_PAGE` de application/defilante.rs, qu'un test compare à
// cette ligne) ; le filet ne doit pas rendre la page avant.
const FILET_DEFILANTE_MS = FILET_MS + 2000;
// Mode film : de même, la sélection reste jusqu'à la fenêtre de contrôle du film, 2 s au
// plus après la fin (F5 du contrat 0.7, `DELAI_PAGE` de application/film.rs).
const FILET_FILM_MS = FILET_MS + 2000;
// Mode fenêtre.
let mode = "zone";
let fenetres = []; // { x, y, largeur, hauteur, rayon }, du premier plan vers le fond
let souris = null; // pixel de la souris donné par la charge
let survol = null; // { cx, cy } : dernière position du pointeur (pixels CSS) depuis la charge
let appui = null; // { pointeur, p } : bouton gauche appuyé

function appeler(commande, args, siEchec) {
  if (!tauri?.core?.invoke) return;
  const echec = (e) => {
    console.warn(`${commande} :`, e);
    if (siEchec) siEchec();
  };
  try {
    Promise.resolve(tauri.core.invoke(commande, args)).catch(echec);
  } catch (e) {
    echec(e);
  }
}

const entier = (v) => Number.isSafeInteger(v);

/** Avant la mesure : la scène couvre la page en pixels CSS (seul le voile plein se voit). */
function scene_css() {
  const s = scene.style;
  s.width = "100%";
  s.height = "100%";
  s.transform = "none";
}

/** Après la mesure : la scène compte en pixels réels (largeur × hauteur), réduite à la page
 *  par scale(1/kx, 1/ky) ; trait de max(1, round(k)) pixels réels. */
function scene_reelle() {
  const s = scene.style;
  s.width = `${largeur}px`;
  s.height = `${hauteur}px`;
  s.transform = `scale(${1 / kx}, ${1 / ky})`;
  document.documentElement.style.setProperty("--trait", `${Math.max(1, Math.round((kx + ky) / 2))}px`);
  const racine = document.documentElement.style;
  racine.setProperty("--trait-fenetre", `${Math.max(2, Math.round(kx + ky))}px`);
  racine.setProperty("--etendue", `${largeur + hauteur}px`);
}

/** Vérifie une demande `selection:montrer` ; null si elle doit être ignorée. */
function valider(m) {
  const rejeter = (raison) => {
    console.warn(`selection : demande rejetée (${raison})`);
    return null;
  };
  if (!m || typeof m !== "object") return rejeter("pas un objet");
  if (!entier(m.id) || m.id < 0) return rejeter("id invalide");
  if (!entier(m.numero) || m.numero < 0) return rejeter("numero invalide");
  if (!entier(m.largeur) || !entier(m.hauteur) || m.largeur <= 0 || m.hauteur <= 0) {
    return rejeter("taille invalide");
  }
  if (!Array.isArray(m.ecrans)) return rejeter("ecrans absents");
  const liste = [];
  for (const e of m.ecrans) {
    if (!e || ![e.x, e.y, e.largeur, e.hauteur].every(entier) || e.largeur <= 0 || e.hauteur <= 0) {
      return rejeter("ecran invalide");
    }
    liste.push({ x: e.x, y: e.y, largeur: e.largeur, hauteur: e.hauteur });
  }
  if (!entier(m.cote_min) || m.cote_min < 0) return rejeter("cote_min invalide");
  const md = m.mode === undefined ? "zone" : m.mode;
  if (
    md !== "zone" &&
    md !== "fenetre" &&
    md !== "texte" &&
    md !== "defilante" &&
    md !== "film" &&
    md !== "pipette"
  ) {
    return rejeter("mode inconnu");
  }
  if (m.suite !== undefined && typeof m.suite !== "boolean") return rejeter("suite invalide");
  const brutes = m.fenetres === undefined ? [] : m.fenetres;
  if (!Array.isArray(brutes)) return rejeter("fenetres invalides");
  const fen = [];
  for (const f of brutes) {
    if (!f || ![f.x, f.y, f.largeur, f.hauteur, f.rayon].every(entier) || f.largeur < 0 || f.hauteur < 0 || f.rayon < 0) {
      return rejeter("fenetre invalide");
    }
    fen.push({ x: f.x, y: f.y, largeur: f.largeur, hauteur: f.hauteur, rayon: f.rayon });
  }
  let sr = null;
  if (m.souris !== undefined && m.souris !== null) {
    if (typeof m.souris !== "object" || !entier(m.souris.x) || !entier(m.souris.y)) return rejeter("souris invalide");
    sr = { x: m.souris.x, y: m.souris.y };
  }
  return { id: m.id, numero: m.numero, largeur: m.largeur, hauteur: m.hauteur, ecrans: liste, coteMin: m.cote_min, mode: md, fenetres: fen, souris: sr, suite: m.suite === true };
}

/** Voile plein, sans rectangle. */
function repos() {
  if (image) cancelAnimationFrame(image);
  image = 0;
  finBalayage();
  glisser = null;
  appui = null;
  corps.classList.remove("glisse");
  corps.classList.remove("entoure");
}

/** Attend que la page ait une taille stable (non nulle, identique deux images de suite),
 *  mesure l'échelle, règle le dessin et prévient le cœur. Sinon, le cœur cache au bout de
 *  son délai. */
function mesurer() {
  prete = false;
  // En attente du cœur, la zone dessinée reste telle quelle jusqu'à la fin.
  if (!attente) scene_css();
  const tour = ++mesure;
  let avant = null;
  const verifier = () => {
    if (tour !== mesure || id === null) return;
    const l = innerWidth;
    const h = innerHeight;
    const stable = l > 0 && h > 0 && avant !== null && avant.l === l && avant.h === h;
    avant = { l, h };
    if (!stable) {
      requestAnimationFrame(verifier);
      return;
    }
    const dpr = window.devicePixelRatio || 1;
    let css = { l, h };
    if (Math.abs(largeur / dpr - l) <= 1 && Math.abs(hauteur / dpr - h) <= 1) {
      kx = ky = dpr; // cas normal : l'échelle de rendu, exacte
    } else {
      const vv = window.visualViewport;
      if (vv && vv.width > 0 && vv.height > 0) css = { l: vv.width, h: vv.height };
      kx = largeur / css.l;
      ky = hauteur / css.h;
    }
    scene_reelle();
    prete = !redimensionnee;
    if (mode === "pipette" && prete) {
      const depart = survol ? pixel(survol.cx, survol.cy) : souris;
      if (depart) {
        const p = { x: borne(depart.x, 0, largeur - 1), y: borne(depart.y, 0, hauteur - 1) };
        mettreAJourPipette(p, survol ? survol.cx : p.x / kx, survol ? survol.cy : p.y / ky);
      }
    }
    planifier(); // mode fenêtre : entourer la fenêtre sous la souris
    appeler("selection_prete", { id, numero, largeur: css.l, hauteur: css.h, echelle: dpr, kx, ky, attente: redimensionnee });
  };
  requestAnimationFrame(verifier);
}

/** Mode défilante : la charge `suite` de l'affichage montré (même id, même numéro, même
 *  taille) n'apporte que les pages ; vrai si elle est prise ainsi. */
function completer(m) {
  if (!m.suite || mode !== "defilante" || m.mode !== "defilante" || id !== m.id || numero !== m.numero) return false;
  if (largeur !== m.largeur || hauteur !== m.hauteur) return false;
  fenetres = m.fenetres;
  if (!survol) souris = m.souris;
  planifier();
  return true;
}

export function montrer(brut) {
  const m = valider(brut);
  if (!m) return;
  if (completer(m)) return;
  repos();
  clearTimeout(filet);
  envoi++;
  id = m.id;
  numero = m.numero;
  redimensionnee = false;
  largeur = m.largeur;
  hauteur = m.hauteur;
  ecrans = m.ecrans;
  coteMin = m.coteMin;
  mode = m.mode;
  fenetres = m.fenetres;
  souris = m.souris;
  survol = null;
  document.documentElement.classList.toggle("mode-fenetre", mode === "fenetre");
  document.documentElement.classList.toggle("mode-pipette", mode === "pipette");
  if (mode === "pipette") preparerPipette();
  else retirerPipette();
  nommer();
  attente = false;
  espace = false;
  maj = false;
  mesurer();
}

export function cachee() {
  repos();
  retirerPipette();
  document.documentElement.classList.remove("mode-pipette");
  clearTimeout(filet);
  envoi++;
  mesure++;
  id = null;
  attente = false;
  prete = false;
}

/** Envoie la fin au cœur et attend `selection:cachee`. Sans réponse dans FILET_MS, ou si
 *  l'envoi est refusé, la page redevient utilisable (Échap, clic droit, glisser). */
function envoyer(commande, args, ms = FILET_MS) {
  attente = true;
  if (image) cancelAnimationFrame(image);
  image = 0;
  const tour = envoi;
  const liberer = () => {
    if (tour !== envoi || !attente) return;
    clearTimeout(filet);
    attente = false;
    repos();
    planifier(); // mode fenêtre : de nouveau entourer la fenêtre sous la souris
  };
  clearTimeout(filet);
  filet = setTimeout(liberer, ms);
  appeler(commande, args, liberer);
}

const dedans = (p, e) => p.x >= e.x && p.x < e.x + e.largeur && p.y >= e.y && p.y < e.y + e.hauteur;
/** L'écran du point (bords droit et bas exclus) ; à défaut, la fenêtre. */
const ecranSous = (p) => ecrans.find((e) => dedans(p, e)) || { x: 0, y: 0, largeur, hauteur };
const borne = (v, min, max) => Math.min(Math.max(v, min), max);
/** Ramène un pixel sur les pixels de l'écran `e` (`[e.x, e.x + e.largeur − 1]`). */
const borner = (p, e) => ({ x: borne(p.x, e.x, e.x + e.largeur - 1), y: borne(p.y, e.y, e.y + e.hauteur - 1) });
// Une position du pointeur désigne un pixel, inclus à l'appui comme au relâchement : le
// pixel réel qui contient la pointe, floor(position × k). Windows donne au navigateur le
// coin haut gauche du pixel du curseur (position = pixel / k) : × k redonne ce pixel, à
// l'erreur d'arrondi près, d'où PRES (Math.round le donnerait aussi ici, mais prendrait le
// pixel suivant pour une position au milieu du pixel).
const PRES = 1e-3;
/** Le pixel réel sous la pointe du curseur, dans la fenêtre. */
const point = (ev) => ({
  x: borne(Math.floor(ev.clientX * kx + PRES), 0, largeur - 1),
  y: borne(Math.floor(ev.clientY * ky + PRES), 0, hauteur - 1),
});

/** Les deux coins du rectangle : courant borné à l'écran du départ, puis carré si Maj. */
function coins(g) {
  const a = g.depart;
  let b = borner(g.courant, g.ecran);
  if (maj) {
    const dx = b.x - a.x;
    const dy = b.y - a.y;
    const cote = Math.min(Math.abs(dx), Math.abs(dy));
    b = { x: a.x + Math.sign(dx) * cote, y: a.y + Math.sign(dy) * cote };
  }
  return [a, b];
}

/** La souris est en `p` : le coin suit, ou (Espace tenue) toute la zone se déplace. */
function avancer(g, p) {
  if (espace) {
    const [a, b] = coins(g);
    const e = g.ecran;
    const dx = borne(p.x - g.souris.x, e.x - Math.min(a.x, b.x), e.x + e.largeur - 1 - Math.max(a.x, b.x));
    const dy = borne(p.y - g.souris.y, e.y - Math.min(a.y, b.y), e.y + e.hauteur - 1 - Math.max(a.y, b.y));
    g.depart = { x: a.x + dx, y: a.y + dy };
    g.courant = { x: b.x + dx, y: b.y + dy };
  } else {
    g.courant = p;
  }
  g.souris = p;
}

/** Pose la zone (pixels réels de la scène). */
function poser(n, x, y, l, h) {
  const s = n.style;
  s.left = `${x}px`;
  s.top = `${y}px`;
  s.width = `${Math.max(0, l)}px`;
  s.height = `${Math.max(0, h)}px`;
}

/** Pose un panneau du voile (carré de 1 pixel réel) par une seule transformation : ni
 *  mise en page ni changement de taille pendant le glisser. */
function poserPanneau(n, x, y, l, h) {
  n.style.transform = `translate(${x}px, ${y}px) scale(${Math.max(0, l)}, ${Math.max(0, h)})`;
}

/** Mode fenêtre : ce qu'il faut entourer pour le pixel `p` (pixels réels de la fenêtre), ou
 *  null. La première fenêtre de la liste qui contient `p`, coupée à l'écran de `p` ; null si
 *  `p` n'est sur aucune fenêtre ou aucun écran, ou si elle garde moins de `cote_min` de côté
 *  (même règle que `decider_fenetre` du cœur). Rayon de chaque coin (haut gauche, haut droit,
 *  bas gauche, bas droit) : celui de la fenêtre si ce coin de la partie coupée est un vrai
 *  coin de la fenêtre, sinon 0 ; jamais plus de la moitié du petit côté. */
function cible(p) {
  const f = fenetres.find((w) => dedans(p, w));
  const e = f && ecrans.find((w) => dedans(p, w));
  if (!e) return null;
  const x0 = Math.max(f.x, e.x);
  const y0 = Math.max(f.y, e.y);
  const x1 = Math.min(f.x + f.largeur, e.x + e.largeur);
  const y1 = Math.min(f.y + f.hauteur, e.y + e.hauteur);
  const l = x1 - x0;
  const h = y1 - y0;
  if (l <= 0 || h <= 0 || l < coteMin || h < coteMin) return null;
  const r = Math.min(f.rayon, Math.floor(Math.min(l, h) / 2));
  const g = x0 === f.x, d = x1 === f.x + f.largeur, hh = y0 === f.y, b = y1 === f.y + f.hauteur;
  return { x: x0, y: y0, l, h, coins: [g && hh, d && hh, g && b, d && b].map((v) => (v ? r : 0)) };
}

/** Pose le contour (ou l'efface si `c` est null). */
function entourer(c) {
  if (!c) {
    corps.classList.remove("entoure");
    return;
  }
  poser(morceaux.fenetre, c.x, c.y, c.l, c.h);
  const [hg, hd, bg, bd] = c.coins;
  morceaux.fenetre.style.borderRadius = `${hg}px ${hd}px ${bd}px ${bg}px`;
  corps.classList.add("entoure");
}

/** Mode fenêtre : déplacement (pixels réels, sur un axe) à partir duquel l'appui est un
 *  glisser et non un clic : 4 pixels logiques, au moins `cote_min`. */
const seuilGlisser = () => Math.max(coteMin, Math.round(4 * Math.max(kx, ky)));
/** De `a` à `b` (pixels réels), est-ce un glisser (seuil atteint sur un axe) ? */
const estGlisser = (a, b) => Math.max(Math.abs(b.x - a.x), Math.abs(b.y - a.y)) >= seuilGlisser();

/** Le pixel réel d'une position CSS du pointeur (même règle que `point`). */
const pixel = (cx, cy) => ({
  x: borne(Math.floor(cx * kx + PRES), 0, largeur - 1),
  y: borne(Math.floor(cy * ky + PRES), 0, hauteur - 1),
});

/** Mode défilante : le glisser en cours est un tracé (seuil franchi), pas un clic. */
const trace = () => mode === "defilante" && glisser !== null && glisser.trace;

function dessiner() {
  image = 0;
  if (mode === "fenetre" || (mode === "defilante" && !trace())) {
    if (attente || !prete || id === null) return;
    // Défilante : tant que l'appui n'est pas un tracé, c'est le contour qui se voit.
    if (mode === "defilante") corps.classList.remove("glisse");
    const p = survol ? pixel(survol.cx, survol.cy) : souris;
    entourer(p ? cible(p) : null);
    return;
  }
  if (!glisser || attente) return;
  if (mode === "defilante") entourer(null);
  const [a, b] = coins(glisser);
  // Un seul pixel (pas encore bougé) : rien. Sinon `[min, max + 1[` : les deux pixels des
  // coins sont dans la zone.
  if (a.x === b.x && a.y === b.y) {
    corps.classList.remove("glisse");
    return;
  }
  poserZone(Math.min(a.x, b.x), Math.min(a.y, b.y), Math.max(a.x, b.x) + 1, Math.max(a.y, b.y) + 1);
}

/** La zone `[x0, x1[ × [y0, y1[` (pixels réels) et le voile autour. */
function poserZone(x0, y0, x1, y1) {
  poserPanneau(morceaux.haut, 0, 0, largeur, y0);
  poserPanneau(morceaux.bas, 0, y1, largeur, hauteur - y1);
  poserPanneau(morceaux.gauche, 0, y0, x0, y1 - y0);
  poserPanneau(morceaux.droite, x1, y0, largeur - x1, y1 - y0);
  poser(morceaux.zone, x0, y0, x1 - x0, y1 - y0);
  corps.classList.add("glisse");
}

/** Le nom de la page : ceux des modes texte, défilante, film et pipette (les autres modes
 *  n'en ont pas). */
const NOMS = {
  texte: "Copier le texte : sélectionnez une zone",
  defilante: "Capture défilante : sélectionnez une zone ou une page",
  film: "Enregistrer l'écran : sélectionnez une zone",
  pipette: "Pipette de couleur : choisissez un pixel",
};
function nommer() {
  if (NOMS[mode]) {
    corps.setAttribute("role", "application");
    corps.setAttribute("aria-label", NOMS[mode]);
  } else {
    corps.removeAttribute("role");
    corps.removeAttribute("aria-label");
  }
}

// Mode pipette : loupe circulaire 15 × 15 pixels réels et carte de valeur HEX / RGB / HSL.
// Créés dans le DOM seulement pendant ce mode : les autres modes gardent un body sans texte.
const PIPETTE_RAYON = 7;
const PIPETTE_COTE = PIPETTE_RAYON * 2 + 1;
const PIPETTE_PAS = 9; // 15 × 9 = 135 px dans le canvas
const PIPETTE_FORMATS = ["hex", "rgb", "hsl"];
let pipetteUi = null;
let pipetteFormat = "hex";
let pipetteCouleur = [0, 0, 0];
let pipettePoint = null;
let echantillonEnCours = false;
let echantillonSuivant = null;

function rgbVersHsl(r, g, b) {
  const rf = r / 255;
  const gf = g / 255;
  const bf = b / 255;
  const max = Math.max(rf, gf, bf);
  const min = Math.min(rf, gf, bf);
  const delta = max - min;
  const l = (max + min) / 2;
  let h = 0;
  let s = 0;
  if (delta !== 0) {
    s = delta / (1 - Math.abs(2 * l - 1));
    if (max === rf) h = ((gf - bf) / delta) % 6;
    else if (max === gf) h = (bf - rf) / delta + 2;
    else h = (rf - gf) / delta + 4;
    h *= 60;
    if (h < 0) h += 360;
  }
  return [((Math.round(h) % 360) + 360) % 360, Math.round(s * 100), Math.round(l * 100)];
}

function formaterCouleur(r, g, b, format) {
  if (format === "rgb") return `rgb(${r}, ${g}, ${b})`;
  if (format === "hsl") {
    const [h, s, l] = rgbVersHsl(r, g, b);
    return `hsl(${h}, ${s}%, ${l}%)`;
  }
  const hex = (n) => n.toString(16).toUpperCase().padStart(2, "0");
  return `#${hex(r)}${hex(g)}${hex(b)}`;
}

function preparerPipette() {
  pipetteFormat = "hex";
  pipetteCouleur = [0, 0, 0];
  pipettePoint = null;
  echantillonEnCours = false;
  echantillonSuivant = null;
  if (pipetteUi) {
    pipetteUi.bloc.hidden = true;
    actualiserTextePipette();
    return;
  }
  const bloc = document.createElement("div");
  bloc.id = "pipette";
  bloc.hidden = true;

  const loupe = document.createElement("div");
  loupe.className = "pipette-loupe";
  const canvas = document.createElement("canvas");
  canvas.className = "pipette-grille";
  canvas.width = PIPETTE_COTE * PIPETTE_PAS;
  canvas.height = PIPETTE_COTE * PIPETTE_PAS;
  loupe.append(canvas);

  const carte = document.createElement("div");
  carte.className = "pipette-carte";
  const pastille = document.createElement("span");
  pastille.className = "pipette-pastille";
  const infos = document.createElement("div");
  infos.className = "pipette-infos";
  const valeur = document.createElement("span");
  valeur.className = "pipette-valeur";
  const formats = document.createElement("div");
  formats.className = "pipette-formats";
  const badges = {};
  for (const f of PIPETTE_FORMATS) {
    const badge = document.createElement("span");
    badge.className = "pipette-format";
    badge.dataset.format = f;
    badge.textContent = f.toUpperCase();
    formats.append(badge);
    badges[f] = badge;
  }
  infos.append(valeur, formats);
  carte.append(pastille, infos);
  bloc.append(loupe, carte);
  corps.append(bloc);

  pipetteUi = {
    bloc,
    ctx: canvas.getContext("2d"),
    pastille,
    valeur,
    badges,
  };
  actualiserTextePipette();
}

function retirerPipette() {
  echantillonEnCours = false;
  echantillonSuivant = null;
  pipettePoint = null;
  if (pipetteUi) {
    pipetteUi.bloc.remove();
    pipetteUi = null;
  }
}

function actualiserTextePipette() {
  if (!pipetteUi) return;
  const [r, g, b] = pipetteCouleur;
  pipetteUi.pastille.style.background = `rgb(${r}, ${g}, ${b})`;
  pipetteUi.valeur.textContent = formaterCouleur(r, g, b, pipetteFormat);
  for (const f of PIPETTE_FORMATS) {
    pipetteUi.badges[f].classList.toggle("actif", f === pipetteFormat);
  }
}

function changerFormatPipette(pas) {
  const i = PIPETTE_FORMATS.indexOf(pipetteFormat);
  pipetteFormat = PIPETTE_FORMATS[(i + pas + PIPETTE_FORMATS.length) % PIPETTE_FORMATS.length];
  actualiserTextePipette();
}

function placerPipette(cx, cy) {
  if (!pipetteUi) return;
  pipetteUi.bloc.hidden = false;
  const rect = pipetteUi.bloc.getBoundingClientRect();
  const l = rect.width || 168;
  const h = rect.height || 192;
  const decalage = 24;
  let x = cx + decalage;
  let y = cy + decalage;
  if (x + l + 8 > innerWidth) x = cx - l - decalage;
  if (y + h + 8 > innerHeight) y = cy - h - decalage;
  x = borne(x, 8, Math.max(8, innerWidth - l - 8));
  y = borne(y, 8, Math.max(8, innerHeight - h - 8));
  pipetteUi.bloc.style.transform = `translate3d(${Math.round(x)}px, ${Math.round(y)}px, 0)`;
}

function dessinerLoupe(octets) {
  if (!pipetteUi?.ctx) return;
  const ctx = pipetteUi.ctx;
  const attendu = PIPETTE_COTE * PIPETTE_COTE * 4;
  if (octets.length < attendu) return;
  ctx.clearRect(0, 0, PIPETTE_COTE * PIPETTE_PAS, PIPETTE_COTE * PIPETTE_PAS);
  for (let py = 0; py < PIPETTE_COTE; py++) {
    for (let px = 0; px < PIPETTE_COTE; px++) {
      const o = (py * PIPETTE_COTE + px) * 4;
      const a = octets[o + 3];
      ctx.fillStyle = a === 0 ? "#181818" : `rgb(${octets[o]}, ${octets[o + 1]}, ${octets[o + 2]})`;
      ctx.fillRect(px * PIPETTE_PAS, py * PIPETTE_PAS, PIPETTE_PAS, PIPETTE_PAS);
    }
  }
  ctx.strokeStyle = "rgba(128, 128, 128, 0.25)";
  ctx.lineWidth = 1;
  ctx.beginPath();
  for (let i = 1; i < PIPETTE_COTE; i++) {
    const pos = i * PIPETTE_PAS + 0.5;
    ctx.moveTo(pos, 0);
    ctx.lineTo(pos, PIPETTE_COTE * PIPETTE_PAS);
    ctx.moveTo(0, pos);
    ctx.lineTo(PIPETTE_COTE * PIPETTE_PAS, pos);
  }
  ctx.stroke();
  const c = PIPETTE_RAYON * PIPETTE_PAS;
  ctx.strokeStyle = "#000000";
  ctx.lineWidth = 2;
  ctx.strokeRect(c - 0.5, c - 0.5, PIPETTE_PAS + 1, PIPETTE_PAS + 1);
  ctx.strokeStyle = "#FFFFFF";
  ctx.lineWidth = 1.5;
  ctx.strokeRect(c + 0.5, c + 0.5, PIPETTE_PAS - 1, PIPETTE_PAS - 1);

  const centre = (PIPETTE_RAYON * PIPETTE_COTE + PIPETTE_RAYON) * 4;
  pipetteCouleur = [octets[centre], octets[centre + 1], octets[centre + 2]];
  actualiserTextePipette();
}

function versOctets(brut) {
  if (brut instanceof Uint8Array) return brut;
  if (brut instanceof ArrayBuffer) return new Uint8Array(brut);
  if (ArrayBuffer.isView(brut)) return new Uint8Array(brut.buffer, brut.byteOffset, brut.byteLength);
  if (Array.isArray(brut)) return Uint8Array.from(brut);
  return null;
}

function demanderEchantillon(p) {
  if (!tauri?.core?.invoke || mode !== "pipette" || id === null || attente || !prete) return;
  if (echantillonEnCours) {
    echantillonSuivant = p;
    return;
  }
  echantillonEnCours = true;
  const tour = envoi;
  Promise.resolve(
    tauri.core.invoke("selection_echantillon", {
      id,
      numero,
      x: p.x,
      y: p.y,
      rayon: PIPETTE_RAYON,
    }),
  )
    .then((brut) => {
      if (tour !== envoi || mode !== "pipette" || id === null) return;
      const octets = versOctets(brut);
      if (octets) dessinerLoupe(octets);
    })
    .catch(() => {})
    .finally(() => {
      echantillonEnCours = false;
      const suivant = echantillonSuivant;
      echantillonSuivant = null;
      if (suivant && tour === envoi && mode === "pipette" && id !== null && !attente) {
        demanderEchantillon(suivant);
      }
    });
}

function mettreAJourPipette(p, cx, cy) {
  pipettePoint = p;
  placerPipette(cx, cy);
  demanderEchantillon(p);
}

// Le mode texte : la zone lue (coin, pixels réels de la scène) et les lignes copiées,
// surlignées dans la zone (conteneur créé une fois).
let lue = null;
const lus = document.createElement("div");
lus.className = "lus";
morceaux.zone.append(lus);
// Au plus autant de lignes que le cœur en envoie (SURLIGNE_LIGNES_MAX) ; chaque durée et tout
// le surlignage bornés (le cœur ne dépasse jamais 780 ms).
const LIGNES_MAX = 400;
const DUREE_MAX = 2000;
/** Un jeton de dimension, en pixels CSS (0 s'il manque). */
const jetonPx = (nom) =>
  parseFloat(getComputedStyle(document.documentElement).getPropertyValue(`--${nom}`)) || 0;

function finBalayage() {
  lue = null;
  lus.replaceChildren();
  corps.classList.remove("balaye", "reduit", "surligne");
}

/** Le filet du mode texte : sans `selection:cachee` dans `ms`, la page redevient utilisable. */
function armerFilet(ms) {
  clearTimeout(filet);
  const tour = envoi;
  filet = setTimeout(() => {
    if (tour !== envoi || !attente) return;
    attente = false;
    repos();
  }, ms);
}

/** `selection:balayer` : la zone lue (pixels réels de la fenêtre) se teinte et respire le
 *  temps de la lecture. Seulement en mode texte, pour l'affichage en cours, une fois la zone
 *  envoyée. */
export function balayer(m) {
  const ok = m && m.id === id && id !== null && mode === "texte" && attente &&
    [m.x, m.y, m.largeur, m.hauteur].every(entier) && m.largeur > 0 && m.hauteur > 0;
  if (!ok) {
    console.warn("selection : balayage ignoré");
    return;
  }
  const x0 = borne(m.x, 0, largeur);
  const y0 = borne(m.y, 0, hauteur);
  poserZone(x0, y0, borne(m.x + m.largeur, x0, largeur), borne(m.y + m.hauteur, y0, hauteur));
  finBalayage();
  lue = { x: x0, y: y0 };
  // Le gel peut rester jusqu'à 1,5 s après le relâcher : le filet repart d'ici.
  armerFilet(FILET_TEXTE_MS);
  corps.classList.toggle("reduit", m.reduites === true);
  void morceaux.zone.offsetWidth; // l'animation repart de zéro
  corps.classList.add("balaye");
}

const dureeValide = (v) => entier(v) && v >= 0 && v <= DUREE_MAX;
const ligneValide = (l) => l && [l.x, l.y, l.largeur, l.hauteur].every(entier) && l.largeur > 0 && l.hauteur > 0;

/** `selection:surligner` : les lignes du texte copié (pixels réels de la fenêtre) se
 *  surlignent dans la zone lue, de gauche à droite, l'une après l'autre ; puis toute la
 *  sélection s'efface. Seulement après le balayage de l'affichage en cours, et si tout le
 *  surlignage tient en DUREE_MAX. Chaque ligne déborde de ses mots comme une sélection de
 *  texte (jetons `texte-surligne-debord-x` et `-y`, pixels logiques) ; la zone coupe ce qui
 *  dépasse. */
export function surligner(m) {
  const lignes = Array.isArray(m?.lignes) ? m.lignes.filter(ligneValide).slice(0, LIGNES_MAX) : [];
  const valides = m && [m.trace, m.pas, m.tenue, m.sortie].every(dureeValide);
  const sortieA = valides ? m.trace + m.pas * (lignes.length - 1) + m.tenue : Infinity;
  const ok = valides && m.id === id && id !== null && mode === "texte" && attente &&
    lue !== null && lignes.length > 0 && sortieA + m.sortie <= DUREE_MAX;
  if (!ok) {
    console.warn("selection : surlignage ignoré");
    return;
  }
  // Le débord d'une ligne autour de ses mots (jetons), en pixels réels entiers.
  const dx = Math.round(jetonPx("texte-surligne-debord-x") * kx);
  const dy = Math.round(jetonPx("texte-surligne-debord-y") * ky);
  const morceau = document.createDocumentFragment();
  lignes.forEach((l, rang) => {
    const i = document.createElement("i");
    poser(i, l.x - dx - lue.x, l.y - dy - lue.y, l.largeur + 2 * dx, l.hauteur + 2 * dy);
    i.style.setProperty("--rang", String(rang));
    morceau.append(i);
  });
  corps.classList.remove("surligne");
  lus.replaceChildren(morceau);
  const s = corps.style;
  s.setProperty("--surligne-trace", `${m.trace}ms`);
  s.setProperty("--surligne-pas", `${m.pas}ms`);
  s.setProperty("--surligne-sortie-a", `${sortieA}ms`);
  s.setProperty("--surligne-sortie", `${m.sortie}ms`);
  corps.classList.toggle("reduit", m.reduites === true);
  armerFilet(sortieA + m.sortie + FILET_MS);
  void lus.offsetWidth; // les animations partent de zéro
  corps.classList.add("surligne");
}

/** Au plus un dessin par image. */
function planifier() {
  const voulu = mode === "fenetre" || mode === "defilante" ? prete && id !== null : glisser;
  if (!image && voulu && !attente) image = requestAnimationFrame(dessiner);
}

function annuler(cause) {
  if (id === null || attente) return;
  envoyer("selection_annulee", { id, numero, cause });
}

addEventListener(
  "pointerdown",
  (e) => {
    e.preventDefault();
    if (id === null || attente) return;
    if (e.button === 2) {
      annuler("clic_droit");
      return;
    }
    if (mode === "pipette") {
      if (e.button === 0 && prete) {
        const p = point(e);
        pipettePoint = p;
        envoyer("selection_pipette", { id, numero, x: p.x, y: p.y, format: pipetteFormat });
      }
      return;
    }
    if (mode === "fenetre") {
      // Chaque appui gauche remplace le précédent : un relâchement perdu ne laisse rien.
      if (e.button === 0) appui = prete ? { pointeur: e.pointerId, p: point(e) } : null;
      return;
    }
    if (e.button !== 0 || glisser || !prete) return;
    const p = point(e);
    const ecran = ecranSous(p);
    const depart = borner(p, ecran);
    try {
      document.documentElement.setPointerCapture(e.pointerId);
    } catch {
      // Sans capture, le glisser marche tant que la souris reste dans la fenêtre.
    }
    maj = e.shiftKey;
    // `appui`, `trace` : mode défilante (clic ou tracé, décidé au seuil du mode fenêtre).
    glisser = { pointeur: e.pointerId, ecran, depart, courant: depart, souris: p, appui: p, trace: false };
  },
  true,
);

addEventListener(
  "pointermove",
  (e) => {
    if (mode === "pipette") {
      if (id === null || attente) return;
      if (e.button === 2) {
        annuler("clic_droit");
        return;
      }
      survol = { cx: e.clientX, cy: e.clientY };
      if (!prete) return;
      mettreAJourPipette(point(e), e.clientX, e.clientY);
      return;
    }
    if (mode === "fenetre") {
      if (id === null || attente) return;
      if (e.button === 2) {
        annuler("clic_droit");
        return;
      }
      survol = { cx: e.clientX, cy: e.clientY };
      planifier();
      return;
    }
    if (mode === "defilante" && id !== null && !attente) {
      survol = { cx: e.clientX, cy: e.clientY };
      if (e.button === 2) {
        annuler("clic_droit");
        return;
      }
    }
    const g = glisser;
    if (!g || attente || e.pointerId !== g.pointeur) {
      if (mode === "defilante") planifier(); // le contour suit la pointe
      return;
    }
    // Bouton droit appuyé pendant le glisser : un `pointermove` (pas de `pointerdown`).
    if (e.button === 2) {
      annuler("clic_droit");
      return;
    }
    maj = e.shiftKey;
    const p = point(e);
    if (mode === "defilante" && !g.trace && estGlisser(g.appui, p)) g.trace = true;
    avancer(g, p);
    planifier();
  },
  true,
);

addEventListener(
  "pointerup",
  (e) => {
    if (mode === "pipette") return;
    if (mode === "fenetre") {
      const a = appui;
      if (!a || attente || id === null || e.pointerId !== a.pointeur || e.button !== 0) return;
      appui = null;
      survol = { cx: e.clientX, cy: e.clientY };
      const p = point(e);
      // Un glisser n'est pas un clic : rien (seul le clic compte).
      const dx = Math.abs(p.x - a.p.x);
      const dy = Math.abs(p.y - a.p.y);
      if (Math.max(dx, dy) >= seuilGlisser()) {
        planifier();
        return;
      }
      const c = cible(p);
      if (!c) {
        planifier();
        return;
      }
      // Le contour montré est celui du pixel cliqué ; plus rien ne change ensuite.
      if (image) cancelAnimationFrame(image);
      image = 0;
      entourer(c);
      envoyer("selection_cliquee", { id, numero, x: p.x, y: p.y });
      return;
    }
    const g = glisser;
    if (!g || attente || e.pointerId !== g.pointeur || e.button !== 0) return;
    maj = e.shiftKey;
    const p = point(e);
    if (mode === "defilante") {
      survol = { cx: e.clientX, cy: e.clientY };
      if (!g.trace && !estGlisser(g.appui, p)) {
        // Un clic : la page entourée sous la pointe, ou rien (on continue).
        const c = cible(p);
        repos();
        if (!c) {
          planifier();
          return;
        }
        entourer(c); // le contour du pixel cliqué ; plus rien ne change ensuite
        envoyer("selection_cliquee", { id, numero, x: p.x, y: p.y }, FILET_DEFILANTE_MS);
        return;
      }
      g.trace = true;
    }
    avancer(g, p);
    const [a, b] = coins(g);
    // Même règle que le cœur : un côté (pixels inclus) sous cote_min, ce n'est pas une zone.
    if (Math.abs(b.x - a.x) + 1 < coteMin || Math.abs(b.y - a.y) + 1 < coteMin) {
      repos(); // on continue
      planifier(); // défilante : de nouveau le contour sous la pointe
      return;
    }
    // Rien n'est redessiné ensuite : le cœur cache la fenêtre (en mode défilante, il la garde
    // jusqu'à la fenêtre de contrôle, comme en mode film : on pose donc le dernier rectangle).
    if (mode === "defilante" || mode === "film") {
      poserZone(Math.min(a.x, b.x), Math.min(a.y, b.y), Math.max(a.x, b.x) + 1, Math.max(a.y, b.y) + 1);
    }
    const ms = mode === "defilante" ? FILET_DEFILANTE_MS : mode === "film" ? FILET_FILM_MS : FILET_MS;
    envoyer("selection_terminee", { id, numero, depart: { x: a.x, y: a.y }, arrivee: { x: b.x, y: b.y } }, ms);
  },
  true,
);

addEventListener(
  "wheel",
  (e) => {
    if (mode !== "pipette" || id === null || attente || !prete) return;
    e.preventDefault();
    const pas = e.deltaY < 0 ? -1 : 1;
    changerFormatPipette(pas);
  },
  { passive: false, capture: true },
);

const perdu = (e) => {
  if (appui && e.pointerId === appui.pointeur) appui = null;
  if (glisser && !attente && e.pointerId === glisser.pointeur) {
    repos();
    planifier(); // défilante : de nouveau le contour sous la pointe
  }
};
addEventListener("pointercancel", perdu, true);
addEventListener("lostpointercapture", perdu, true);

addEventListener(
  "keydown",
  (e) => {
    // Aucune touche n'a son effet de navigateur (recharger, imprimer, chercher, outils…).
    e.preventDefault();
    if (e.key === "Escape") {
      annuler("echap");
    } else if (mode === "pipette") {
      if (id === null || attente || !prete) return;
      if (e.key === "Tab") {
        changerFormatPipette(e.shiftKey ? -1 : 1);
      } else if (e.key === "c" || e.key === "C" || e.key === "f" || e.key === "F") {
        changerFormatPipette(1);
      } else if (e.key === "Enter" || e.code === "Space" || e.key === " ") {
        if (pipettePoint) {
          envoyer("selection_pipette", {
            id,
            numero,
            x: pipettePoint.x,
            y: pipettePoint.y,
            format: pipetteFormat,
          });
        }
      } else if (
        e.key === "ArrowLeft" ||
        e.key === "ArrowRight" ||
        e.key === "ArrowUp" ||
        e.key === "ArrowDown"
      ) {
        const base = pipettePoint || { x: Math.floor(largeur / 2), y: Math.floor(hauteur / 2) };
        const dx = e.key === "ArrowLeft" ? -1 : e.key === "ArrowRight" ? 1 : 0;
        const dy = e.key === "ArrowUp" ? -1 : e.key === "ArrowDown" ? 1 : 0;
        const suivant = {
          x: borne(base.x + dx, 0, largeur - 1),
          y: borne(base.y + dy, 0, hauteur - 1),
        };
        mettreAJourPipette(suivant, suivant.x / kx, suivant.y / ky);
      }
    } else if (mode === "fenetre") {
      // Maj et Espace ne font rien dans ce mode.
    } else if (e.code === "Space" || e.key === " ") {
      espace = true;
    } else if (e.key === "Shift") {
      maj = true;
      planifier();
    }
  },
  true,
);

addEventListener(
  "keyup",
  (e) => {
    if (e.code === "Space" || e.key === " ") {
      e.preventDefault();
      espace = false;
    } else if (mode === "fenetre" || mode === "pipette") {
      // Rien.
    } else if (e.key === "Shift") {
      maj = false;
      planifier();
    }
  },
  true,
);

addEventListener("blur", () => {
  espace = false;
  maj = false;
});
// La taille change : l'échelle n'est plus sûre. Le glisser en cours est abandonné, la page
// n'est plus prête jusqu'à la charge suivante, et elle mesure de nouveau (puis renvoie
// `selection_prete`, sur quoi le cœur relit la fenêtre).
addEventListener("resize", () => {
  if (id === null) return;
  redimensionnee = true;
  if (!attente) repos();
  mesurer();
});
scene_css();
// Pas de menu contextuel : le clic droit annule.
addEventListener("contextmenu", (e) => e.preventDefault(), true);

// Branchement : Tauri si présent, sinon aperçu dans un navigateur.
if (tauri?.event?.listen) {
  Promise.all([
    tauri.event.listen("selection:montrer", (evenement) => montrer(evenement?.payload)),
    tauri.event.listen("selection:cachee", () => cachee()),
    tauri.event.listen("selection:balayer", (evenement) => balayer(evenement?.payload)),
    tauri.event.listen("selection:surligner", (evenement) => surligner(evenement?.payload)),
  ]).then(
    () => appeler("selection_chargee", {}),
    (e) => console.warn("selection : écoute impossible :", e),
  );
} else {
  if (new URLSearchParams(location.search).has("demo")) {
    const d = window.devicePixelRatio || 1;
    montrer({
      id: 1,
      numero: 1,
      largeur: Math.round(innerWidth * d),
      hauteur: Math.round(innerHeight * d),
      ecrans: [],
      cote_min: 3,
    });
  }
  // Pour les essais dans un navigateur seulement.
  window.selectionApercu = { montrer, cachee, balayer, surligner };
}
