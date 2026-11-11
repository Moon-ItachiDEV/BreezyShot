// Sélection d'une zone (étape 5). La page montre et rapporte, le cœur décide
// (ARCHITECTURE.md §2.5). Elle ne dessine que le voile et le rectangle : ni texte, ni viseur,
// ni taille, ni poignée, ni animation.
//
// Échelle k (pixels réels par pixel CSS) : devicePixelRatio, l'échelle de rendu vraie, quand
// il explique la taille réelle de la fenêtre (envoyée par le cœur) à 1 px CSS près sur chaque
// axe ; sinon, taille réelle / taille CSS non arrondie (visualViewport, à défaut innerWidth).
// innerWidth est arrondi au pixel CSS entier : 1366 px à 125 % font 1092,8 px CSS, pas 1093.
//
// Contrat avec le cœur (fenetres/selection.rs) :
// - écoute `selection:montrer` { id, numero, largeur, hauteur, ecrans: [{ x, y, largeur,
//   hauteur }], cote_min } : tout en pixels réels entiers, relatifs au coin haut gauche de la
//   fenêtre (`largeur`, `hauteur` : rectangle réel de la fenêtre ; `numero` : celui de la
//   charge, renvoyé avec chaque message). Retour au repos (voile plein). Dès que
//   innerWidth/innerHeight sont non nuls et identiques deux images de suite, la page retient
//   son échelle (kx, ky), règle son dessin, puis envoie `selection_prete` { id, numero,
//   largeur, hauteur (taille CSS sur laquelle l'échelle est retenue), echelle:
//   devicePixelRatio, kx, ky }. Avant cela, aucun glisser n'est accepté. Une demande
//   invalide est ignorée (console.warn) : le cœur cache la fenêtre au bout de son délai. Une
//   même demande reçue de nouveau (zone refusée, fenêtre changée, page rechargée) remet la
//   page au repos et la fait mesurer de nouveau.
// - `resize` pendant un affichage : le glisser en cours est abandonné, la page redevient
//   « pas prête » jusqu'à la charge suivante ; elle attend la stabilité et renvoie
//   `selection_prete` (le cœur relit alors la fenêtre et renvoie la charge).
// - écoute `selection:cachee` : retour au repos, plus rien n'est envoyé.
// - `selection_terminee` { id, numero, depart: { x, y }, arrivee: { x, y } } au relâchement :
//   les deux coins finals (bords, après carré, déplacement et bornage), en pixels réels entiers
//   relatifs à la fenêtre. Rien n'est redessiné ensuite : le cœur cache la fenêtre.
// - `selection_annulee` { id, numero, cause: "echap" | "clic_droit" }, une seule fois par
//   affichage.
// - `selection_chargee` {} une fois, quand les deux écoutes sont en place.
// Gestes : glisser du bouton gauche ; Maj tenue : carré ; Espace tenue : la zone se déplace ;
// Échap ou clic droit : annuler. Une zone reste sur l'écran où le glisser a commencé.
// Sous `cote_min` de côté, ce n'est pas une zone : le rectangle s'efface et on continue.
// Toute touche est neutralisée (F5, Ctrl+R, Ctrl+P, F12… ne font rien).
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
  return { id: m.id, numero: m.numero, largeur: m.largeur, hauteur: m.hauteur, ecrans: liste, coteMin: m.cote_min };
}

/** Voile plein, sans rectangle. */
function repos() {
  if (image) cancelAnimationFrame(image);
  image = 0;
  glisser = null;
  corps.classList.remove("glisse");
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
    appeler("selection_prete", { id, numero, largeur: css.l, hauteur: css.h, echelle: dpr, kx, ky });
  };
  requestAnimationFrame(verifier);
}

export function montrer(brut) {
  const m = valider(brut);
  if (!m) return;
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
  attente = false;
  espace = false;
  maj = false;
  mesurer();
}

export function cachee() {
  repos();
  clearTimeout(filet);
  envoi++;
  mesure++;
  id = null;
  attente = false;
  prete = false;
}

/** Envoie la fin au cœur et attend `selection:cachee`. Sans réponse dans FILET_MS, ou si
 *  l'envoi est refusé, la page redevient utilisable (Échap, clic droit, glisser). */
function envoyer(commande, args) {
  attente = true;
  if (image) cancelAnimationFrame(image);
  image = 0;
  const tour = envoi;
  const liberer = () => {
    if (tour !== envoi || !attente) return;
    clearTimeout(filet);
    attente = false;
    repos();
  };
  clearTimeout(filet);
  filet = setTimeout(liberer, FILET_MS);
  appeler(commande, args, liberer);
}

const dedans = (p, e) => p.x >= e.x && p.x < e.x + e.largeur && p.y >= e.y && p.y < e.y + e.hauteur;
/** L'écran du point (bords droit et bas exclus) ; à défaut, la fenêtre. */
const ecranSous = (p) => ecrans.find((e) => dedans(p, e)) || { x: 0, y: 0, largeur, hauteur };
const borne = (v, min, max) => Math.min(Math.max(v, min), max);
const borner = (p, e) => ({ x: borne(p.x, e.x, e.x + e.largeur), y: borne(p.y, e.y, e.y + e.hauteur) });
/** Le pixel réel sous la souris, dans la fenêtre. */
const point = (ev) => ({
  x: borne(Math.round(ev.clientX * kx), 0, largeur),
  y: borne(Math.round(ev.clientY * ky), 0, hauteur),
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
    const dx = borne(p.x - g.souris.x, e.x - Math.min(a.x, b.x), e.x + e.largeur - Math.max(a.x, b.x));
    const dy = borne(p.y - g.souris.y, e.y - Math.min(a.y, b.y), e.y + e.hauteur - Math.max(a.y, b.y));
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

function dessiner() {
  image = 0;
  if (!glisser || attente) return;
  const [a, b] = coins(glisser);
  const x0 = Math.min(a.x, b.x);
  const x1 = Math.max(a.x, b.x);
  const y0 = Math.min(a.y, b.y);
  const y1 = Math.max(a.y, b.y);
  if (x1 <= x0 || y1 <= y0) {
    corps.classList.remove("glisse");
    return;
  }
  poserPanneau(morceaux.haut, 0, 0, largeur, y0);
  poserPanneau(morceaux.bas, 0, y1, largeur, hauteur - y1);
  poserPanneau(morceaux.gauche, 0, y0, x0, y1 - y0);
  poserPanneau(morceaux.droite, x1, y0, largeur - x1, y1 - y0);
  poser(morceaux.zone, x0, y0, x1 - x0, y1 - y0);
  corps.classList.add("glisse");
}

/** Au plus un dessin par image. */
function planifier() {
  if (!image && glisser && !attente) image = requestAnimationFrame(dessiner);
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
    glisser = { pointeur: e.pointerId, ecran, depart, courant: depart, souris: p };
  },
  true,
);

addEventListener(
  "pointermove",
  (e) => {
    const g = glisser;
    if (!g || attente || e.pointerId !== g.pointeur) return;
    // Bouton droit appuyé pendant le glisser : un `pointermove` (pas de `pointerdown`).
    if (e.button === 2) {
      annuler("clic_droit");
      return;
    }
    maj = e.shiftKey;
    avancer(g, point(e));
    planifier();
  },
  true,
);

addEventListener(
  "pointerup",
  (e) => {
    const g = glisser;
    if (!g || attente || e.pointerId !== g.pointeur || e.button !== 0) return;
    maj = e.shiftKey;
    avancer(g, point(e));
    const [a, b] = coins(g);
    // Même règle que le cœur : un côté (écart des bords) sous cote_min, ce n'est pas une zone.
    if (Math.abs(b.x - a.x) < coteMin || Math.abs(b.y - a.y) < coteMin) {
      repos(); // on continue
      return;
    }
    // Rien n'est redessiné : le cœur cache la fenêtre.
    envoyer("selection_terminee", { id, numero, depart: { x: a.x, y: a.y }, arrivee: { x: b.x, y: b.y } });
  },
  true,
);

const perdu = (e) => {
  if (glisser && !attente && e.pointerId === glisser.pointeur) repos();
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
    } else if (e.key === "Shift") {
      maj = false;
      planifier();
    }
  },
  true,
);

addEventListener("blur", () => {
  espace = false;
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
  window.selectionApercu = { montrer, cachee };
}
