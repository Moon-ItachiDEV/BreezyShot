// Page de l'éditeur d'annotation (contrat 0.4 « Éditeur », §3.4, §3.5, §4, §6, §7.2, et
// avenant « Éditeur épuré ») : entrée de la page. IPC avec le cœur, charge, image, les trois
// toiles et la vue, sorties et fermeture. La barre (barre.js) et les gestes (outils.js)
// reçoivent l'objet `ed`.
//
// Commandes (permises à la seule fenêtre `editeur`) : `editeur_pret` → ChargeEditeur ;
// `editeur_image` { id } → octets RGBA ; `editeur_affiche` { id } ; `editeur_vue` { id, x,
// y, largeur, hauteur, zoom } (pixels CSS) ; `editeur_etat` { id, version, vide } ;
// `editeur_exporter` (corps brut, en-têtes x-editeur-*) → numéro ; `editeur_fermer` { id,
// version, vide, force, preferences } → "fermee" | "demander" ; `editeur_garder` { id }.
// Événements : `editeur:charger`, `editeur:fermer_demande`, `editeur:resultat`,
// `editeur:glisser`, `interface:theme` (commun/theme.js). Copier le texte (0.5, texte.js) :
// `ChargeEditeur.texte`, export de but `texte`, `editeur_copier_texte`, `editeur:texte`,
// `editeur:texte_copie`.
//
// Toiles : `base` (ImageBitmap de la capture), `fond` (base + masques, recalculée quand un
// masque change), `vue` (la scène × devicePixelRatio). `vue` = { x, y, echelle, dpr } : place
// de l'image entière dans la toile (pixels physiques, entiers) et pixels physiques par
// pixel de l'image. Taille réelle : echelle 1, l'étiquette dit 100 %.

import { suivreTheme } from "../commun/theme.js";
import * as modele from "./document.js";
import * as dessin from "./dessin.js";
import { installerBarre } from "./barre.js";
import { installerOutils } from "./outils.js";
import { installerTexte } from "./texte.js";

const tauri = window.__TAURI__;
const $ = (s) => document.querySelector(s);
const racine = document.documentElement;
const style = () => getComputedStyle(racine);
// Un jeton sans unité (nombre) ou en ms / px : lu une fois quand il sert.
export const jetonNombre = (nom) => parseFloat(style().getPropertyValue(`--${nom}`)) || 0;
export const jeton = (nom) => style().getPropertyValue(`--${nom}`).trim();

const ZOOM_MIN = 0.1;
const ZOOM_MAX = 8;
const PAS_ZOOM = 1.25;
const ETAT_MS = 250; // editeur_etat : au plus une fois par 250 ms, et toujours le dernier

function appeler(commande, args, options) {
  if (!tauri?.core?.invoke) return Promise.reject(new Error("pas de cœur"));
  return Promise.resolve(tauri.core.invoke(commande, args, options));
}
const signaler = (commande) => (e) => console.info(`${commande} :`, e);

const toile = $("#vue");
const scene = $("#scene");

// L'état partagé par la page. `h.doc` est le document (il peut être remplacé par
// annuler et rétablir : jamais gardé ailleurs).
const ed = {
  modele,
  dessin,
  charge: null,
  h: null,
  base: null,
  fond: null,
  ctxFond: null,
  ctx: toile.getContext("2d"),
  toile,
  scene,
  vue: { x: 0, y: 0, echelle: 1, dpr: window.devicePixelRatio || 1 },
  cadrage: { mode: "ajuster" }, // ou { mode: "libre", echelle, x, y } (pixels de la toile)
  selection: [],
  prefs: null,
  enCours: null, // objet en cours de tracé (peint par peindreVue, option `ajout`), ou null
  surcouche: null, // fonction (ctx) de outils.js : poignées, recadrage, cadre, pipette
  affiche: false,
  exportee: null, // dernière version exportée avec succès
  exports: new Map(), // numéro → but
};

// ---------- préférences de l'outil (session, R9) ----------

// Couleurs de contenu : celles de dessin.js, par leur nom.
const couleurDessin = (nom) => dessin.COULEURS.find((c) => c[1] === nom)[0];
// `emplacement` : l'outil venu de « Ajouter » (vide : l'emplacement n'est pas montré) ;
// `autre` : la couleur libre gardée (« Autre couleur »). Couleur, taille (rang 0 à 2) et
// opacité : par outil, comme la maquette (S.col, S.w, S.a).
const rouge = couleurDessin("Rouge");
export const DEFAUTS = {
  outil: "fleche",
  emplacement: "",
  variantes: { fleche: "droite", forme: "rectangle", flou: "pixeliser", projecteur: "rectangle" },
  couleurs: {
    fleche: rouge,
    stylo: rouge,
    surligneur: couleurDessin("Jaune"),
    texte: rouge,
    forme: rouge,
    numero: rouge,
  },
  tailles: { fleche: 1, stylo: 1, surligneur: 1, texte: 1, forme: 1, numero: 1, flou: 1 },
  opacites: { fleche: 100, stylo: 100, texte: 100, forme: 100, numero: 100 },
  autre: "",
};
const PAR_OUTIL = ["variantes", "couleurs", "tailles", "opacites"];
const OUTILS_CONNUS = [
  "fleche",
  "stylo",
  "surligneur",
  "gomme",
  "texte",
  "forme",
  "numero",
  "flou",
  "projecteur",
];

function lirePreferences(texte) {
  const p = structuredClone(DEFAUTS);
  if (!texte) return p;
  try {
    const lu = JSON.parse(texte);
    for (const k of Object.keys(DEFAUTS)) {
      if (lu[k] === undefined || typeof lu[k] !== typeof DEFAUTS[k]) continue;
      p[k] = PAR_OUTIL.includes(k) ? { ...p[k], ...lu[k] } : lu[k];
    }
    if (!OUTILS_CONNUS.includes(p.outil)) p.outil = DEFAUTS.outil;
  } catch {
    // Préférences illisibles : les défauts.
  }
  return p;
}

// ---------- document et historique ----------

ed.doc = () => ed.h.doc;
ed.objet = (id) => ed.h.doc.objets.find((o) => o.id === id) || null;
ed.objetsChoisis = () => ed.selection.map(ed.objet).filter(Boolean);

// Une modification du document : un pas d'annulation (ou le même pas que la précédente
// avec la même `marque`).
ed.modifier = (fn, marque = null) => {
  modele.modifier(ed.h, fn, marque);
  apresChangement();
};
// Pendant un geste, rien ; pendant une saisie, Annuler l'abandonne (design). Après annuler
// ou rétablir, les objets sont neufs : seuls les `id` servent, la sélection est nettoyée et
// le fond repeint.
ed.annuler = () => {
  if (!ed.h || ed.outils?.gesteEnCours()) return;
  if (ed.outils?.annulerSaisie()) return;
  if (!modele.peutAnnuler(ed.h)) return;
  modele.annuler(ed.h);
  apresChangement();
};
ed.retablir = () => {
  if (!ed.h || ed.outils?.gesteEnCours() || ed.outils?.saisieOuverte()) return;
  if (!modele.peutRetablir(ed.h)) return;
  modele.retablir(ed.h);
  apresChangement();
};

let cleMasques = "";
let cleCadre = "";
function apresChangement() {
  const doc = ed.h.doc;
  ed.selection = ed.selection.filter((id) => doc.objets.some((o) => o.id === id));
  const c = modele.cadre(doc);
  const cle = `${c.x},${c.y},${c.largeur},${c.hauteur}`;
  if (cle !== cleCadre) {
    cleCadre = cle;
    ed.cadrage = { mode: "ajuster" };
    placer();
    envoyerVue();
    animerRecadrage();
  }
  repeindreFond();
  ed.barre?.synchroniser();
  planifierEtat();
  ed.dessiner();
}

// Recadrer, Image entière, l'annulation d'un recadrage : la vue arrive de 0.97 et 0.6
// d'opacité, en ressort ; jamais avec les animations réduites.
function animerRecadrage() {
  if ("reduites" in racine.dataset) return;
  if (matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  const depart = {
    transform: `scale(${jetonNombre("editeur-recadrage-echelle")})`,
    opacity: jetonNombre("editeur-recadrage-opacite"),
  };
  const options = {
    duration: jetonNombre("duree-editeur-recadrage"),
    easing: jeton("courbe-ressort"),
  };
  for (const el of [toile, $("#ombre-image")]) {
    el.animate([depart, { transform: "none", opacity: 1 }], options);
  }
}

// Le fond (base + masques) n'est repeint que si un masque a changé.
function repeindreFond(force = false) {
  if (!ed.fond || !ed.base) return;
  const doc = ed.h.doc;
  const cle = JSON.stringify(doc.objets.filter((o) => o.type === "masque"));
  if (!force && cle === cleMasques) return;
  cleMasques = cle;
  ed.ctxFond.setTransform(1, 0, 0, 1, 0, 0);
  ed.ctxFond.clearRect(0, 0, ed.fond.width, ed.fond.height);
  dessin.peindreMasques(ed.ctxFond, ed.base, doc);
}
ed.repeindreFond = repeindreFond;

let minuterieEtat = null;
let dernierEtat = -Infinity;
function planifierEtat() {
  if (minuterieEtat) return;
  const attente = Math.max(0, dernierEtat + ETAT_MS - performance.now());
  minuterieEtat = setTimeout(() => {
    minuterieEtat = null;
    envoyerEtat();
  }, attente);
}
// L'état à jour, sans attendre : avant chaque réponse à `editeur:fermer_demande`, chaque
// « Copier et fermer » et chaque export (décision du responsable, rapport de A).
function envoyerEtat() {
  clearTimeout(minuterieEtat);
  minuterieEtat = null;
  dernierEtat = performance.now();
  if (!ed.charge || !ed.h) return Promise.resolve();
  return appeler("editeur_etat", {
    id: ed.charge.id,
    version: ed.h.version,
    vide: modele.vide(ed.h.doc),
  }).catch(signaler("editeur_etat"));
}

// ---------- toile de la vue ----------

let tailleToile = null; // [largeur, hauteur] en pixels physiques, ou null : d'après le CSS
new ResizeObserver((entrees) => {
  const e = entrees[0];
  const p = e.devicePixelContentBoxSize?.[0];
  tailleToile = p ? [p.inlineSize, p.blockSize] : null;
  ajusterToile();
}).observe(toile, { box: "device-pixel-content-box" });

// La taille en pixels de l'écran que donne le navigateur, si elle s'accorde avec la taille
// CSS × devicePixelRatio (une émulation de l'échelle peut la donner en pixels CSS).
function ajusterToile() {
  const dpr = window.devicePixelRatio || 1;
  const cl = toile.clientWidth * dpr;
  const ch = toile.clientHeight * dpr;
  const juste =
    tailleToile && Math.abs(tailleToile[0] - cl) <= 1 && Math.abs(tailleToile[1] - ch) <= 1;
  const l = juste ? tailleToile[0] : Math.round(cl);
  const h = juste ? tailleToile[1] : Math.round(ch);
  if (toile.width !== l || toile.height !== h) {
    toile.width = l;
    toile.height = h;
  }
  ed.vue.dpr = dpr;
  placer();
}
matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`).addEventListener?.("change", () =>
  ajusterToile(),
);

// Zone utile de la scène pour l'image ajustée (pixels CSS) : les réserves de la maquette
// (reserve(), piste b), la palette comprise.
function zoneUtile() {
  const l = scene.clientWidth;
  const h = scene.clientHeight;
  const etroite = $("#fenetre").clientWidth < jetonNombre("editeur-etroite");
  const cote = jetonNombre(etroite ? "editeur-reserve-cote-etroite" : "editeur-reserve-cote");
  const haut = jetonNombre("editeur-reserve-haut");
  const bas = $("#palette").offsetHeight + jetonNombre("editeur-reserve-bas");
  return { l, h, al: Math.max(1, l - 2 * cote), ah: Math.max(1, h - haut - bas), haut };
}

ed.echelleAjuster = () => {
  if (!ed.h) return 1;
  const c = modele.cadre(ed.h.doc);
  const z = zoneUtile();
  const dpr = ed.vue.dpr;
  return Math.min((z.al * dpr) / c.largeur, (z.ah * dpr) / c.hauteur, 1);
};

// Calcule `ed.vue` d'après le cadrage : position entière, échelle bornée.
function placer() {
  if (!ed.h) return;
  const dpr = ed.vue.dpr;
  const c = modele.cadre(ed.h.doc);
  let { x, y, echelle } = ed.cadrage;
  if (ed.cadrage.mode === "ajuster") {
    const z = zoneUtile();
    echelle = ed.echelleAjuster();
    const gauche = (z.l - (c.largeur * echelle) / dpr) / 2;
    const haut = z.haut + (z.ah - (c.hauteur * echelle) / dpr) / 2;
    x = gauche * dpr - c.x * echelle;
    y = haut * dpr - c.y * echelle;
  }
  ed.vue = { x: Math.round(x), y: Math.round(y), echelle, dpr };
  const ombre = $("#ombre-image");
  ombre.hidden = !ed.base;
  ombre.style.left = `${(ed.vue.x + c.x * echelle) / dpr}px`;
  ombre.style.top = `${(ed.vue.y + c.y * echelle) / dpr}px`;
  ombre.style.width = `${(c.largeur * echelle) / dpr}px`;
  ombre.style.height = `${(c.hauteur * echelle) / dpr}px`;
  ed.outils?.placerSaisie();
  ed.texte?.placer();
  ed.dessiner();
}
ed.placer = placer;

// Zoom au point (mx, my) en pixels CSS de la scène.
ed.zoomer = (echelle, mx, my) => {
  const e = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, echelle));
  const dpr = ed.vue.dpr;
  const ux = (mx * dpr - ed.vue.x) / ed.vue.echelle;
  const uy = (my * dpr - ed.vue.y) / ed.vue.echelle;
  ed.cadrage = { mode: "libre", echelle: e, x: mx * dpr - ux * e, y: my * dpr - uy * e };
  placer();
  montrerZoom();
  envoyerVue();
};
ed.zoomerCentre = (facteur) => {
  const z = zoneUtile();
  ed.zoomer(ed.vue.echelle * facteur, z.l / 2, z.haut + z.ah / 2);
};
ed.ajuster = () => {
  ed.cadrage = { mode: "ajuster" };
  placer();
  montrerZoom();
  envoyerVue();
};
ed.tailleReelle = () => {
  const c = modele.cadre(ed.h.doc);
  const z = zoneUtile();
  const dpr = ed.vue.dpr;
  const x = ((z.l - c.largeur / dpr) / 2) * dpr - c.x;
  const y = (z.haut + (z.ah - c.hauteur / dpr) / 2) * dpr - c.y;
  ed.cadrage = { mode: "libre", echelle: 1, x, y };
  placer();
  montrerZoom();
  envoyerVue();
};
ed.estTailleReelle = () => ed.cadrage.mode === "libre" && Math.abs(ed.vue.echelle - 1) < 1e-6;
ed.deplacerVue = (dx, dy) => {
  ed.cadrage = { mode: "libre", echelle: ed.vue.echelle, x: ed.vue.x + dx, y: ed.vue.y + dy };
  placer();
};
ed.zoomPourcent = () => Math.round(ed.vue.echelle * 100);

let minuterieZoom = null;
function montrerZoom() {
  const e = $("#etiquette-zoom");
  e.textContent = `${ed.zoomPourcent()} %`;
  e.classList.add("on");
  clearTimeout(minuterieZoom);
  minuterieZoom = setTimeout(() => e.classList.remove("on"), jetonNombre("duree-editeur-zoom"));
}

// editeur_vue : à la fin d'un changement de vue (pixels CSS de la page).
let minuterieVue = null;
function envoyerVue() {
  clearTimeout(minuterieVue);
  minuterieVue = setTimeout(() => {
    if (!ed.charge || !ed.h) return;
    const c = modele.cadre(ed.h.doc);
    const { x, y, echelle, dpr } = ed.vue;
    // `ed.vue` est dans la toile ; la toile commence sous la barre du haut.
    const t = toile.getBoundingClientRect();
    appeler("editeur_vue", {
      id: ed.charge.id,
      x: t.left + (x + c.x * echelle) / dpr,
      y: t.top + (y + c.y * echelle) / dpr,
      largeur: (c.largeur * echelle) / dpr,
      hauteur: (c.hauteur * echelle) / dpr,
      zoom: echelle,
    }).catch(signaler("editeur_vue"));
  }, 150);
}
ed.envoyerVue = envoyerVue;

// ---------- peinture de la vue (une fois par image) ----------

let demande = 0;
ed.dessiner = () => {
  if (demande) return;
  demande = requestAnimationFrame(peindre);
};
function optionsVue() {
  // L'objet en cours de tracé ou le texte en saisie (`ajout`), et le texte existant en saisie
  // (`cacher`) : options de B.
  const options = {};
  const ajout = ed.enCours || ed.outils?.texteEnSaisie();
  if (ajout) options.ajout = ajout;
  const enEdition = ed.outils?.idEnEdition();
  if (enEdition) options.cacher = enEdition;
  return options;
}
function peindre() {
  demande = 0;
  const ctx = ed.ctx;
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.clearRect(0, 0, toile.width, toile.height);
  if (!ed.h || !ed.fond) return;
  // Pendant un recadrage, toute l'image à l'échelle du début du geste, coins droits.
  const recadrage = ed.outils?.recadrageEnCours();
  const doc = recadrage ? { ...ed.h.doc, recadrage: null } : ed.h.doc;
  const options = optionsVue();
  options.rayon = recadrage ? 0 : jetonNombre("editeur-image-rayon") * ed.vue.dpr;
  dessin.peindreVue(ctx, doc, ed.fond, ed.vue, recadrage ? [] : ed.selection, options);
  $("#ombre-image").hidden = !!recadrage;
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ed.surcouche?.(ctx);
  if (!ed.affiche && ed.charge) {
    ed.affiche = true;
    appeler("editeur_affiche", { id: ed.charge.id }).catch(signaler("editeur_affiche"));
  }
}

// ---------- chargement ----------

function relacherToiles() {
  if (ed.base) ed.base.close?.();
  ed.base = null;
  if (ed.fond) {
    ed.fond.width = 0;
    ed.fond.height = 0;
  }
  ed.fond = null;
  ed.ctxFond = null;
}

async function charger(charge) {
  if (!charge || typeof charge.id !== "number") return;
  if (ed.prefs) charge = { ...charge, preferences: preferencesTexte() };
  ed.charge = charge;
  ed.prefs = lirePreferences(charge.preferences);
  if (charge.reduites) racine.dataset.reduites = "";
  else delete racine.dataset.reduites;
  relacherToiles();
  ed.outils?.remiseAZero();
  ed.h = modele.creerHistorique(modele.nouveauDocument(charge.largeur, charge.hauteur));
  // Une demande de fermeture reçue avant la charge : la réponse, une seule fois.
  if (demandeEnAttente != null) {
    const d = demandeEnAttente;
    demandeEnAttente = null;
    if (d === charge.id) fermer(false);
  }
  ed.selection = [];
  ed.cadrage = { mode: "ajuster" };
  ed.exportee = null;
  ed.exports.clear();
  exportEnCours = false;
  ed.texte?.remiseAZero();
  ed.texte?.poserBouton(charge.texte === true);
  ed.affiche = false;
  cleMasques = "";
  cleCadre = "";
  ed.barre?.remiseAZero();
  const id = charge.id;
  let octets;
  try {
    octets = await appeler("editeur_image", { id });
  } catch (e) {
    // Image impossible : comme une image refusée, le cœur ferme et montre son bandeau.
    console.info("editeur_image :", e);
    if (ed.charge?.id === id) fermer(true);
    return;
  }
  if (ed.charge?.id !== id) return;
  const tampon =
    octets instanceof ArrayBuffer
      ? new Uint8ClampedArray(octets)
      : ArrayBuffer.isView(octets)
        ? new Uint8ClampedArray(octets.buffer, octets.byteOffset, octets.byteLength)
        : Array.isArray(octets)
          ? Uint8ClampedArray.from(octets)
          : null;
  const attendu = charge.largeur * charge.hauteur * 4;
  if (!tampon || tampon.length !== attendu || attendu === 0) {
    // Image refusée : le cœur ferme (« impossible ») et montre son bandeau.
    fermer(true);
    return;
  }
  const image = new ImageData(tampon, charge.largeur, charge.hauteur);
  const base = await createImageBitmap(image, {
    premultiplyAlpha: "none",
    colorSpaceConversion: "none",
  });
  if (ed.charge?.id !== id) {
    base.close();
    return;
  }
  ed.base = base;
  ed.fond = document.createElement("canvas");
  ed.fond.width = charge.largeur;
  ed.fond.height = charge.hauteur;
  ed.ctxFond = ed.fond.getContext("2d", { willReadFrequently: true });
  repeindreFond(true);
  cleCadre = (() => {
    const c = modele.cadre(ed.h.doc);
    return `${c.x},${c.y},${c.largeur},${c.hauteur}`;
  })();
  ed.barre?.synchroniser();
  placer();
  envoyerVue();
}

// ---------- sorties ----------

ed.preferences = () => preferencesTexte();
function preferencesTexte() {
  return ed.prefs ? JSON.stringify(ed.prefs) : null;
}

// Export : l'image composée part en corps brut, les en-têtes disent quoi en faire.
// Un export à la fois : le drapeau tombe au résultat ou à l'échec de l'appel.
let exportEnCours = false;
let demandeEnAttente = null; // editeur:fermer_demande reçue avant la charge
ed.exportEnCours = () => exportEnCours;
// Un export à la fois : « Texte » est grisé pendant un export (contrat 0.5, §5.6).
function poserExport(v) {
  exportEnCours = v;
  ed.texte?.synchroniser();
}
ed.finExport = () => poserExport(false);
ed.exporter = async (but) => {
  if (!ed.charge || !ed.h || !ed.fond || exportEnCours) return null;
  poserExport(true);
  ed.outils?.validerSaisie();
  await envoyerEtat();
  if (!ed.charge || !ed.fond) {
    poserExport(false);
    return null;
  }
  const version = ed.h.version;
  const image = dessin.composer(ed.h.doc, ed.fond);
  const rgba = image.rgba;
  const corps = new Uint8Array(rgba.buffer, rgba.byteOffset, rgba.byteLength);
  const headers = {
    "x-editeur-id": String(ed.charge.id),
    "x-editeur-version": String(version),
    "x-editeur-but": but,
    "x-editeur-largeur": String(image.largeur),
    "x-editeur-hauteur": String(image.hauteur),
  };
  try {
    const numero = await appeler("editeur_exporter", corps, { headers });
    ed.exports.set(numero, { but, version });
    return numero;
  } catch (e) {
    poserExport(false);
    console.info("editeur_exporter :", e);
    return null;
  }
};

function resultat(r) {
  if (!r || !ed.charge || r.id !== ed.charge.id) return;
  poserExport(false);
  const e = ed.exports.get(r.numero);
  ed.exports.delete(r.numero);
  const reussi = r.etat === "fait" || r.etat === "deja_fait";
  if (reussi && e) ed.exportee = e.version;
  if (reussi) ed.barre?.fait(r.but);
  else if (r.etat === "echec" && r.message) ed.barre?.message(r.message);
  // « Copier » (copier puis fermer) : la coche, puis la fenêtre se ferme ; en échec elle reste.
  if (r.but === "copier_fermer") {
    // Le cœur ferme seul (ou envoie `editeur:charger`) : la page ne rappelle pas
    // editeur_fermer, elle montre la coche.
    if (reussi) ed.barre?.copierReussi();
    else ed.barre?.fermerMenuFermer();
  }
}

// ---------- fermeture (§4.5, avenant §3) ----------

// `force` : « Fermer sans copier ». « demander » : le menu « Fermer ».
async function fermer(force = false) {
  if (!ed.charge) return;
  ed.outils?.validerSaisie();
  await envoyerEtat();
  if (!ed.charge) return;
  const id = ed.charge.id;
  const version = ed.h?.version ?? 0;
  const vide = ed.h ? modele.vide(ed.h.doc) : true;
  let reponse = null;
  try {
    reponse = await appeler("editeur_fermer", {
      id,
      version,
      vide,
      force,
      preferences: preferencesTexte(),
    });
  } catch (e) {
    console.info("editeur_fermer :", e);
  }
  if (reponse === "demander" && ed.charge?.id === id) {
    ed.barre?.ouvrirMenuFermer();
    return;
  }
  // Toiles relâchées après « fermee » seulement. « fermee » peut être suivie d'un
  // `editeur:charger` (capture en attente) : seules les toiles de CE document le sont.
  if (ed.charge?.id !== id) return;
  ed.barre?.fermerMenuFermer();
  if (reponse === "fermee") relacherToiles();
}
ed.fermer = fermer;
ed.garder = () => {
  if (ed.charge) appeler("editeur_garder", { id: ed.charge.id }).catch(signaler("editeur_garder"));
};

// ---------- démarrage ----------

ed.barre = installerBarre(ed);
// Lu par l'essai de la page (essais/pages/editeur.cjs), qui pose le drapeau avant le
// chargement ; rien en production.
if (window.__essaiEditeur === true) Object.defineProperty(window, "__ed", { value: ed });
ed.outils = installerOutils(ed);
ed.texte = installerTexte(ed, {
  appeler,
  message: (texte, echec) => ed.barre.message(texte, echec),
  reduites: () =>
    "reduites" in racine.dataset || matchMedia("(prefers-reduced-motion: reduce)").matches,
});

async function demarrer() {
  await suivreTheme();
  const ecoute = tauri?.event?.listen;
  if (ecoute) {
    await Promise.all([
      ecoute("editeur:charger", (e) => charger(e?.payload)),
      ecoute("editeur:fermer_demande", (e) => {
        const id = e?.payload?.id;
        if (!ed.h) demandeEnAttente = id ?? null;
        else if (id === ed.charge?.id) fermer(false);
      }),
      ecoute("editeur:resultat", (e) => resultat(e?.payload)),
      ecoute("editeur:texte", (e) => ed.texte.recu(e?.payload)),
      ecoute("editeur:texte_copie", (e) => ed.texte.copie(e?.payload)),
      ecoute("editeur:glisser", (e) => {
        if (e?.payload?.id === ed.charge?.id) ed.barre.glisserEnCours(e.payload.etat === "debut");
      }),
    ]).catch(signaler("écoute"));
  }
  scene.focus({ preventScroll: true });
  let charge = null;
  try {
    charge = await appeler("editeur_pret", {});
  } catch (e) {
    console.info("editeur_pret :", e);
  }
  if (charge) await charger(charge);
  else if (demandeEnAttente != null) {
    // Rien de chargé, rien à perdre : une seule réponse.
    const id = demandeEnAttente;
    demandeEnAttente = null;
    appeler("editeur_fermer", {
      id,
      version: 0,
      vide: true,
      force: false,
      preferences: preferencesTexte(),
    }).catch(signaler("editeur_fermer"));
  }
}
demarrer();
