// Sélection d'une zone (étape 5). La page montre et rapporte, le cœur décide
// (ARCHITECTURE.md §2.5). Elle ne dessine que le voile et le rectangle : ni texte, ni viseur,
// ni taille, ni poignée, ni animation.
//
// Contrat avec le cœur (fenetres/selection.rs) :
// - écoute `selection:montrer` { id, ecrans: [{ x, y, largeur, hauteur }], cote_min }
//   (pixels CSS de la page) : retour au repos (voile plein), puis, deux images plus tard,
//   `selection_prete` { id }. Une demande invalide est ignorée (console.warn) : le cœur
//   cache la fenêtre au bout de son délai.
// - écoute `selection:cachee` : retour au repos, plus rien n'est envoyé.
// - `selection_terminee` { id, depart: { x, y }, arrivee: { x, y } } au relâchement : les deux
//   coins finals (après carré, déplacement et bornage), en pixels CSS. Rien n'est redessiné
//   ensuite : le cœur cache la fenêtre.
// - `selection_annulee` { id, cause: "echap" | "clic_droit" }, une seule fois par affichage.
// - `selection_chargee` {} une fois, quand les deux écoutes sont en place.
// Gestes : glisser du bouton gauche ; Maj tenue : carré ; Espace tenue : la zone se déplace ;
// Échap ou clic droit : annuler. Une zone reste sur l'écran où le glisser a commencé.
// Sous `cote_min` de côté, ce n'est pas une zone : le rectangle s'efface et on continue.

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

let id = null; // affichage en cours ; null : fenêtre cachée
let ecrans = []; // rectangles des écrans, pixels CSS
let coteMin = 0; // pixels CSS
let attente = false; // relâché ou annulé : on attend le cœur, plus rien ne bouge
let glisser = null; // { pointeur, ecran, depart, courant, souris }
let maj = false;
let espace = false;
let image = 0; // requestAnimationFrame en attente

function appeler(commande, args) {
  if (!tauri?.core?.invoke) return;
  try {
    Promise.resolve(tauri.core.invoke(commande, args)).catch((e) => console.warn(`${commande} :`, e));
  } catch (e) {
    console.warn(`${commande} :`, e);
  }
}

const echelle = () => window.devicePixelRatio || 1;
/** Arrondi au pixel réel : le trait reste net et les coins tombent sur des pixels entiers. */
const net = (v) => Math.round(v * echelle()) / echelle();
const fini = (v) => typeof v === "number" && Number.isFinite(v);

/** La scène en pixels réels, réduite à la taille de la fenêtre ; trait de 1 px CSS arrondi
 *  au pixel réel (au moins un). */
function regler_scene() {
  const e = echelle();
  const s = scene.style;
  s.width = `${Math.round(innerWidth * e)}px`;
  s.height = `${Math.round(innerHeight * e)}px`;
  s.transform = `scale(${1 / e})`;
  document.documentElement.style.setProperty("--trait", `${Math.max(1, Math.round(e))}px`);
}

/** Vérifie une demande `selection:montrer` ; null si elle doit être ignorée. */
function valider(m) {
  const rejeter = (raison) => {
    console.warn(`selection : demande rejetée (${raison})`);
    return null;
  };
  if (!m || typeof m !== "object") return rejeter("pas un objet");
  if (!Number.isSafeInteger(m.id) || m.id < 0) return rejeter("id invalide");
  if (!Array.isArray(m.ecrans)) return rejeter("ecrans absents");
  const liste = [];
  for (const e of m.ecrans) {
    if (!e || ![e.x, e.y, e.largeur, e.hauteur].every(fini) || e.largeur <= 0 || e.hauteur <= 0) {
      return rejeter("ecran invalide");
    }
    liste.push({ x: e.x, y: e.y, largeur: e.largeur, hauteur: e.hauteur });
  }
  if (!fini(m.cote_min) || m.cote_min < 0) return rejeter("cote_min invalide");
  return { id: m.id, ecrans: liste, coteMin: m.cote_min };
}

/** Voile plein, sans rectangle. */
function repos() {
  if (image) cancelAnimationFrame(image);
  image = 0;
  glisser = null;
  corps.classList.remove("glisse");
}

export function montrer(brut) {
  const m = valider(brut);
  if (!m) return;
  repos();
  id = m.id;
  ecrans = m.ecrans;
  coteMin = m.coteMin;
  attente = false;
  espace = false;
  maj = false;
  regler_scene();
  const lui = id;
  // Deux images : la fenêtre transparente est réellement peinte.
  requestAnimationFrame(() =>
    requestAnimationFrame(() => {
      if (id === lui) appeler("selection_prete", { id: lui });
    }),
  );
}

export function cachee() {
  repos();
  id = null;
  attente = false;
}

const dansLaFenetre = () => ({ x: 0, y: 0, largeur: innerWidth, hauteur: innerHeight });
const dedans = (p, e) => p.x >= e.x && p.x < e.x + e.largeur && p.y >= e.y && p.y < e.y + e.hauteur;
/** L'écran du point (bords droit et bas exclus) ; à défaut, la fenêtre. */
const ecranSous = (p) => ecrans.find((e) => dedans(p, e)) || dansLaFenetre();
const borne = (v, min, max) => Math.min(Math.max(v, min), max);
const borner = (p, e) => ({ x: borne(p.x, e.x, e.x + e.largeur), y: borne(p.y, e.y, e.y + e.hauteur) });
const point = (ev) => ({ x: net(ev.clientX), y: net(ev.clientY) });

/** Les deux coins du rectangle : courant borné à l'écran du départ, puis carré si Maj. */
function coins(g) {
  const a = g.depart;
  let b = borner(g.courant, g.ecran);
  if (maj) {
    const dx = b.x - a.x;
    const dy = b.y - a.y;
    const cote = Math.min(Math.abs(dx), Math.abs(dy));
    b = { x: net(a.x + Math.sign(dx) * cote), y: net(a.y + Math.sign(dy) * cote) };
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
    g.depart = { x: net(a.x + dx), y: net(a.y + dy) };
    g.courant = { x: net(b.x + dx), y: net(b.y + dy) };
  } else {
    g.courant = p;
  }
  g.souris = p;
}

/** Pose un morceau de la scène ; reçoit des pixels CSS, écrit des pixels réels entiers. */
function poser(n, x, y, largeur, hauteur) {
  const e = echelle();
  const reel = (v) => `${Math.max(0, Math.round(v * e))}px`;
  const s = n.style;
  s.left = reel(x);
  s.top = reel(y);
  s.width = reel(largeur);
  s.height = reel(hauteur);
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
  const l = innerWidth;
  const h = innerHeight;
  poser(morceaux.haut, 0, 0, l, y0);
  poser(morceaux.bas, 0, y1, l, h - y1);
  poser(morceaux.gauche, 0, y0, x0, y1 - y0);
  poser(morceaux.droite, x1, y0, l - x1, y1 - y0);
  poser(morceaux.zone, x0, y0, x1 - x0, y1 - y0);
  corps.classList.add("glisse");
}

/** Au plus un dessin par image. */
function planifier() {
  if (!image && glisser && !attente) image = requestAnimationFrame(dessiner);
}

function annuler(cause) {
  if (id === null || attente) return;
  attente = true;
  if (image) cancelAnimationFrame(image);
  image = 0;
  appeler("selection_annulee", { id, cause });
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
    if (e.button !== 0 || glisser) return;
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
    const e2 = echelle();
    const petit = (v) => Math.round(Math.abs(v) * e2) < Math.round(coteMin * e2);
    if (petit(b.x - a.x) || petit(b.y - a.y)) {
      repos(); // pas une zone : on continue
      return;
    }
    // Rien n'est redessiné : le cœur cache la fenêtre.
    attente = true;
    if (image) cancelAnimationFrame(image);
    image = 0;
    appeler("selection_terminee", { id, depart: { x: a.x, y: a.y }, arrivee: { x: b.x, y: b.y } });
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
    if (e.key === "Escape") {
      e.preventDefault();
      annuler("echap");
    } else if (e.code === "Space" || e.key === " ") {
      e.preventDefault();
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
addEventListener("resize", regler_scene);
regler_scene();
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
  if (new URLSearchParams(location.search).has("demo")) montrer({ id: 1, ecrans: [], cote_min: 3 });
  // Pour les essais dans un navigateur seulement.
  window.selectionApercu = { montrer, cachee };
}
