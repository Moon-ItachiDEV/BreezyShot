// Fenêtre flottante d'une capture épinglée à l'écran.
//
// Commandes (permises aux seules fenêtres `epingle-*`) : `epingle_pret` → ChargeEpingle ;
// `epingle_image` → octets RGBA bruts ; `epingle_redimensionner` { facteur } → TailleEpingle ;
// `epingle_opacite` { opacite } → opacité bornée ; `epingle_copier` ; `epingle_fermer`.
// Déplacement natif par `getCurrentWindow().startDragging()` après un seuil de 4 px (ne
// bloque jamais le double-clic de fermeture). Aucun innerHTML.

import { poserTheme, suivreTheme } from "../commun/theme.js";

const tauri = window.__TAURI__;
const $ = (s) => document.querySelector(s);

const OPACITE_MIN = 0.25;
const OPACITE_MAX = 1.0;
const OPACITE_PAS = 0.08;
const FACTEUR_ZOOM = 1.1;
const ECHELLE_MIN = 0.2;
const ECHELLE_MAX = 4.0;
const SEUIL_GLISSER = 4;
const DUREE_PILULE_MS = 1100;
const DUREE_COPIE_MS = 1200;

const carte = $("#carte");
const cadre = $("#cadre");
const toile = $("#toile");
const bCopier = $("#copier");
const bFermer = $("#fermer");
const pilule = $("#pilule");

const ctx = toile.getContext("2d");

let source = null; // HTMLCanvasElement tenant l'image RGBA d'origine
let etat = {
  id: 0,
  image_largeur: 0,
  image_hauteur: 0,
  largeur: 0,
  hauteur: 0,
  echelle: 1,
  opacite: 1,
  fermee: false,
};

let minuteriePilule = null;
let minuterieCopie = null;
let facteurAttente = 1;
let zoomEnCours = false;
let copieEnCours = false;
let appui = null;

function appeler(commande, args) {
  if (!tauri?.core?.invoke) return Promise.reject(new Error("pas de cœur"));
  return Promise.resolve(tauri.core.invoke(commande, args));
}

function montrerPilule(texte) {
  pilule.textContent = texte;
  pilule.hidden = false;
  clearTimeout(minuteriePilule);
  minuteriePilule = setTimeout(() => {
    pilule.hidden = true;
  }, DUREE_PILULE_MS);
}

function appliquerOpacite(valeur) {
  const bornee = Math.max(OPACITE_MIN, Math.min(OPACITE_MAX, Math.round(valeur * 100) / 100));
  etat.opacite = bornee;
  cadre.style.setProperty("--opacite", String(bornee));
  return bornee;
}

function dessiner() {
  if (!source || !ctx) return;
  const dpr = window.devicePixelRatio || 1;
  const r = cadre.getBoundingClientRect();
  const lCss = Math.max(1, Math.round(r.width || etat.largeur || source.width));
  const hCss = Math.max(1, Math.round(r.height || etat.hauteur || source.height));
  const l = Math.max(1, Math.round(lCss * dpr));
  const h = Math.max(1, Math.round(hCss * dpr));
  if (toile.width !== l || toile.height !== h) {
    toile.width = l;
    toile.height = h;
  }
  ctx.clearRect(0, 0, l, h);
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(source, 0, 0, l, h);
}

function chargerPixels(rgba, largeur, hauteur) {
  const octets =
    rgba instanceof Uint8ClampedArray
      ? rgba
      : new Uint8ClampedArray(rgba.buffer ?? rgba, rgba.byteOffset ?? 0, rgba.byteLength ?? rgba.length);
  const hors = document.createElement("canvas");
  hors.width = largeur;
  hors.height = hauteur;
  const c = hors.getContext("2d");
  if (c && octets.length === largeur * hauteur * 4) {
    c.putImageData(new ImageData(octets, largeur, hauteur), 0, 0);
  }
  source = hors;
  dessiner();
}

function appliquerCharge(charge) {
  if (!charge) return;
  etat = {
    ...etat,
    id: charge.id ?? etat.id,
    image_largeur: charge.image_largeur ?? etat.image_largeur,
    image_hauteur: charge.image_hauteur ?? etat.image_hauteur,
    largeur: charge.largeur ?? etat.largeur,
    hauteur: charge.hauteur ?? etat.hauteur,
    echelle: charge.echelle ?? 1,
    opacite: charge.opacite ?? 1,
    fermee: false,
  };
  appliquerOpacite(etat.opacite);
}

async function fermer() {
  if (etat.fermee) return;
  etat.fermee = true;
  if (!tauri?.core?.invoke) return;
  try {
    await appeler("epingle_fermer", {});
  } catch (e) {
    console.info("epingle_fermer :", e);
  }
}

async function copier() {
  if (copieEnCours || etat.fermee) return;
  copieEnCours = true;
  try {
    if (tauri?.core?.invoke) await appeler("epingle_copier", {});
    bCopier.classList.add("fait");
    clearTimeout(minuterieCopie);
    minuterieCopie = setTimeout(() => bCopier.classList.remove("fait"), DUREE_COPIE_MS);
    montrerPilule("Copié");
  } catch (e) {
    console.info("epingle_copier :", e);
    montrerPilule("Impossible de copier");
  } finally {
    copieEnCours = false;
  }
}

async function pomperZoom() {
  if (zoomEnCours || etat.fermee) return;
  zoomEnCours = true;
  try {
    while (Math.abs(facteurAttente - 1) > 1e-4 && !etat.fermee) {
      const facteur = facteurAttente;
      facteurAttente = 1;
      if (tauri?.core?.invoke) {
        const res = await appeler("epingle_redimensionner", { facteur });
        if (res && typeof res.echelle === "number") {
          etat.largeur = res.largeur;
          etat.hauteur = res.hauteur;
          etat.echelle = res.echelle;
        }
      } else {
        const nouvelle = Math.max(ECHELLE_MIN, Math.min(ECHELLE_MAX, etat.echelle * facteur));
        etat.echelle = nouvelle;
        etat.largeur = Math.max(80, Math.round(etat.image_largeur * nouvelle));
        etat.hauteur = Math.max(60, Math.round(etat.image_hauteur * nouvelle));
      }
      montrerPilule(`${Math.round(etat.echelle * 100)} %`);
      requestAnimationFrame(dessiner);
    }
  } catch (e) {
    console.info("epingle_redimensionner :", e);
  } finally {
    zoomEnCours = false;
    if (Math.abs(facteurAttente - 1) > 1e-4 && !etat.fermee) pomperZoom();
  }
}

function reinitialiser() {
  const op = appliquerOpacite(OPACITE_MAX);
  if (tauri?.core?.invoke) {
    appeler("epingle_opacite", { opacite: op }).catch((e) => console.info("epingle_opacite :", e));
  }
  if (etat.echelle > 0 && Math.abs(etat.echelle - 1) > 1e-3) {
    facteurAttente = 1 / etat.echelle;
    pomperZoom();
  } else {
    montrerPilule("100 %");
  }
}

// ---------- interactions : déplacement, double-clic, molette, boutons, clavier ----------

carte.addEventListener("pointerdown", (e) => {
  if (e.button !== 0 || e.target.closest(".outils")) return;
  appui = { x: e.clientX, y: e.clientY, id: e.pointerId };
});

window.addEventListener("pointermove", (e) => {
  if (!appui || e.pointerId !== appui.id) return;
  if ((e.buttons & 1) === 0) {
    appui = null;
    return;
  }
  if (Math.hypot(e.clientX - appui.x, e.clientY - appui.y) < SEUIL_GLISSER) return;
  appui = null;
  const f = tauri?.window?.getCurrentWindow?.();
  if (!f?.startDragging) return;
  carte.classList.add("glisse");
  Promise.resolve(f.startDragging())
    .catch((err) => console.info("startDragging :", err))
    .finally(() => carte.classList.remove("glisse"));
});

for (const ev of ["pointerup", "pointercancel"]) {
  window.addEventListener(ev, () => {
    appui = null;
    carte.classList.remove("glisse");
  });
}

carte.addEventListener("dblclick", (e) => {
  if (e.target.closest(".outils")) return;
  e.preventDefault();
  fermer();
});

window.addEventListener(
  "wheel",
  (e) => {
    e.preventDefault();
    if (etat.fermee || e.deltaY === 0) return;
    if (e.altKey) {
      const delta = e.deltaY < 0 ? OPACITE_PAS : -OPACITE_PAS;
      const op = appliquerOpacite(etat.opacite + delta);
      montrerPilule(`Opacité ${Math.round(op * 100)} %`);
      if (tauri?.core?.invoke) {
        appeler("epingle_opacite", { opacite: op }).catch((err) => console.info("epingle_opacite :", err));
      }
      return;
    }
    const facteur = e.deltaY < 0 ? FACTEUR_ZOOM : 1 / FACTEUR_ZOOM;
    facteurAttente *= facteur;
    pomperZoom();
  },
  { passive: false },
);

bCopier.addEventListener("click", (e) => {
  e.stopPropagation();
  copier();
});

bFermer.addEventListener("click", (e) => {
  e.stopPropagation();
  fermer();
});

const NAVIGATEUR = new Set(["F1", "F3", "F5", "F6", "F7", "F10", "F11", "F12"]);
window.addEventListener(
  "keydown",
  (e) => {
    const ctl = e.ctrlKey || e.metaKey;
    const k = e.key.length === 1 ? e.key.toLowerCase() : e.key;
    if (NAVIGATEUR.has(e.key) || (ctl && ["r", "p", "f", "u", "s", "o", "g", "j", "+", "=", "-"].includes(k))) {
      e.preventDefault();
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      fermer();
    } else if (ctl && !e.shiftKey && !e.altKey && k === "c") {
      e.preventDefault();
      copier();
    } else if (e.key === "0" || e.code === "Digit0" || e.code === "Numpad0") {
      e.preventDefault();
      reinitialiser();
    }
  },
  true,
);

window.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("dragstart", (e) => e.preventDefault());
window.addEventListener("resize", () => requestAnimationFrame(dessiner));

// ---------- démarrage ----------

function creerImageDemo(l, h) {
  const c = document.createElement("canvas");
  c.width = l;
  c.height = h;
  const g = c.getContext("2d");
  if (g) {
    const d = g.createLinearGradient(0, 0, l, h);
    d.addColorStop(0, "#1d3557");
    d.addColorStop(0.5, "#457b9d");
    d.addColorStop(1, "#a8dadc");
    g.fillStyle = d;
    g.fillRect(0, 0, l, h);
    g.fillStyle = "rgba(255,255,255,0.18)";
    g.fillRect(28, 28, l - 56, h - 56);
    g.fillStyle = "#ffffff";
    g.font = "600 18px system-ui, sans-serif";
    g.fillText("Capture épinglée", 48, 72);
  }
  return c;
}

window.epingleApercu = {
  etat: () => ({ ...etat }),
  poserTheme,
  appliquerCharge,
  dessiner,
};

async function demarrer() {
  await suivreTheme();
  if (!tauri?.core?.invoke) {
    const l = 420;
    const h = 280;
    appliquerCharge({
      id: 1,
      image_largeur: l,
      image_hauteur: h,
      largeur: l,
      hauteur: h,
      echelle: 1,
      opacite: 1,
    });
    source = creerImageDemo(l, h);
    dessiner();
    return;
  }
  try {
    const charge = await appeler("epingle_pret", {});
    appliquerCharge(charge);
    const octets = await appeler("epingle_image", {});
    if (octets && charge?.image_largeur > 0 && charge?.image_hauteur > 0) {
      chargerPixels(octets, charge.image_largeur, charge.image_hauteur);
    }
  } catch (e) {
    console.warn("epingle : démarrage impossible :", e);
  }
}

demarrer();
