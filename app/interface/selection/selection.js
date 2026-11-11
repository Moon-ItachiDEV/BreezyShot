// Sélection d'une zone (étape 5). La page montre et rapporte, le cœur décide
// (ARCHITECTURE.md §2.5). Elle ne dessine que le voile et le rectangle : ni texte, ni viseur,
// ni taille, ni poignée, ni animation.
//
// Contrat avec le cœur (fenetres/selection.rs) :
// - écoute `selection:montrer` { id, ecrans: [{ x, y, largeur, hauteur }], cote_min,
//   largeur, hauteur, echelle } (pixels CSS de la page ; `largeur`, `hauteur`, `echelle` :
//   taille et échelle attendues de la page) : retour au repos (voile plein). Dès que
//   innerWidth/innerHeight (à 1 px près) et devicePixelRatio correspondent pendant deux
//   images de suite : `selection_prete` { id, largeur, hauteur, echelle } (valeurs lues).
//   Avant cela, aucun glisser n'est accepté. Une demande invalide est ignorée
//   (console.warn) : le cœur cache la fenêtre au bout de son délai. Une même demande
//   reçue de nouveau (zone refusée, page rechargée) remet la page au repos.
// - écoute `selection:cachee` : retour au repos, plus rien n'est envoyé.
// - `selection_terminee` { id, depart: { x, y }, arrivee: { x, y } } au relâchement : les deux
//   coins finals (après carré, déplacement et bornage), en pixels CSS. Rien n'est redessiné
//   ensuite : le cœur cache la fenêtre.
// - `selection_annulee` { id, cause: "echap" | "clic_droit" }, une seule fois par affichage.
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

let id = null; // affichage en cours ; null : fenêtre cachée
let ecrans = []; // rectangles des écrans, pixels CSS
let coteMin = 0; // pixels CSS
let attente = false; // relâché ou annulé : on attend le cœur, plus rien ne bouge
let glisser = null; // { pointeur, ecran, depart, courant, souris }
let maj = false;
let espace = false;
let image = 0; // requestAnimationFrame en attente
let prete = false; // la page a la taille et l'échelle attendues : on peut glisser
let envoi = 0; // numéro de la dernière demande reçue
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
  if (![m.largeur, m.hauteur, m.echelle].every((v) => fini(v) && v > 0)) return rejeter("taille invalide");
  return { id: m.id, ecrans: liste, coteMin: m.cote_min, largeur: m.largeur, hauteur: m.hauteur, echelle: m.echelle };
}

/** Voile plein, sans rectangle. */
function repos() {
  if (image) cancelAnimationFrame(image);
  image = 0;
  glisser = null;
  corps.classList.remove("glisse");
}

/** La page a-t-elle la taille et l'échelle que le cœur attend ? */
const conforme = (m) =>
  Math.abs(innerWidth - m.largeur) <= 1 &&
  Math.abs(innerHeight - m.hauteur) <= 1 &&
  Math.abs(echelle() - m.echelle) < 1e-3;

export function montrer(brut) {
  const m = valider(brut);
  if (!m) return;
  repos();
  clearTimeout(filet);
  id = m.id;
  ecrans = m.ecrans;
  coteMin = m.coteMin;
  attente = false;
  prete = false;
  espace = false;
  maj = false;
  regler_scene();
  const numero = ++envoi;
  // Deux images de suite à la bonne taille : la fenêtre transparente est peinte, et le
  // conteneur WebView2 a fini de se redimensionner. Sinon, le cœur cache au bout de 2 s.
  let suite = 0;
  const verifier = () => {
    if (numero !== envoi || id === null) return;
    suite = conforme(m) ? suite + 1 : 0;
    if (suite < 2) {
      requestAnimationFrame(verifier);
      return;
    }
    regler_scene();
    prete = true;
    appeler("selection_prete", { id: m.id, largeur: innerWidth, hauteur: innerHeight, echelle: echelle() });
  };
  requestAnimationFrame(verifier);
}

export function cachee() {
  repos();
  clearTimeout(filet);
  envoi++;
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
  const numero = envoi;
  const liberer = () => {
    if (numero !== envoi || !attente) return;
    clearTimeout(filet);
    attente = false;
    repos();
  };
  clearTimeout(filet);
  filet = setTimeout(liberer, FILET_MS);
  appeler(commande, args, liberer);
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

/** Pose un panneau du voile (carré de 1 pixel réel) par une seule transformation : ni
 *  mise en page ni changement de taille pendant le glisser. */
function poserPanneau(n, x, y, largeur, hauteur) {
  const e = echelle();
  const reel = (v) => Math.max(0, Math.round(v * e));
  n.style.transform = `translate(${reel(x)}px, ${reel(y)}px) scale(${reel(largeur)}, ${reel(hauteur)})`;
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
  poserPanneau(morceaux.haut, 0, 0, l, y0);
  poserPanneau(morceaux.bas, 0, y1, l, h - y1);
  poserPanneau(morceaux.gauche, 0, y0, x0, y1 - y0);
  poserPanneau(morceaux.droite, x1, y0, l - x1, y1 - y0);
  poser(morceaux.zone, x0, y0, x1 - x0, y1 - y0);
  corps.classList.add("glisse");
}

/** Au plus un dessin par image. */
function planifier() {
  if (!image && glisser && !attente) image = requestAnimationFrame(dessiner);
}

function annuler(cause) {
  if (id === null || attente) return;
  envoyer("selection_annulee", { id, cause });
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
    const e2 = echelle();
    const petit = (v) => Math.round(Math.abs(v) * e2) < Math.round(coteMin * e2);
    if (petit(b.x - a.x) || petit(b.y - a.y)) {
      repos(); // pas une zone : on continue
      return;
    }
    // Rien n'est redessiné : le cœur cache la fenêtre.
    envoyer("selection_terminee", { id, depart: { x: a.x, y: a.y }, arrivee: { x: b.x, y: b.y } });
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
  if (new URLSearchParams(location.search).has("demo")) {
    montrer({ id: 1, ecrans: [], cote_min: 3, largeur: innerWidth, hauteur: innerHeight, echelle: echelle() });
  }
  // Pour les essais dans un navigateur seulement.
  window.selectionApercu = { montrer, cachee };
}
