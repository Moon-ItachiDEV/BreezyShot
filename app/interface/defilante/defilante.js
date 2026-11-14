// Page de contrôle de la capture défilante (contrat 0.6, §10 à §12). La page pose ce que
// le cœur lui dit et rapporte ; elle ne décide rien (ARCHITECTURE.md §2.5).
//
// Contrat avec le cœur (fenetres/defilante.rs, application/fenetre_defilante.rs) ; toutes
// les places en pixels réels entiers, relatives au coin haut gauche de la fenêtre, qui
// couvre l'écran de la zone. Échelle (pixels réels par pixel CSS) : devicePixelRatio,
// à défaut `echelle` de la charge.
// - `defilante_pret` {} une fois, quand les écoutes (et celle du thème, commun/theme.js)
//   sont en place ; rend { reduites }.
// - écoute `defilante:montrer` { id, echelle, zone: { x, y, largeur, hauteur },
//   apercu: { x, y, largeur, hauteur_max }, pilule: { x, y }, libelle, reduites } : le voile
//   sur tout l'écran sauf la zone, le trait blanc dehors, la pilule (haut gauche donné) avec
//   `libelle`. Puis `defilante_zone` { id, x, y, largeur, hauteur } : le rectangle de la
//   pilule en pixels CSS, sans l'animation d'entrée ; une fois par affichage, de nouveau
//   seulement si le rectangle ou devicePixelRatio change. Tant que devicePixelRatio n'égale
//   pas `echelle`, il attend (300 ms au plus). Charge invalide : console.warn, rien montré.
// - écoute `defilante:apercu` { id, numero, hauteur } : la page demande les octets PNG par
//   `defilante_apercu` { id, numero } (une demande à la fois ; seul le plus récent compte)
//   et montre l'image, collée en bas, dans une bande haute de min(hauteur, hauteur_max).
//   Avant le premier aperçu, la bande n'est pas montrée.
// - écoute `defilante:message` { id, texte } : `texte` (texte seul) en haut de l'écran, ou
//   rien si `texte` est null.
// - écoute `defilante:cacher` { id } : tout est effacé.
// - `defilante_terminer` { id } au clic sur la pilule, une fois par id.
// Un `id` qui n'est pas celui de l'affichage en cours est ignoré. Aucun texte ne passe par
// innerHTML. La page ne prend jamais le focus : le clavier reste à la page visée.

import { suivreTheme } from "../commun/theme.js";

const tauri = window.__TAURI__;
const racine = document.documentElement;
const scene = document.getElementById("scene");
const zone = document.getElementById("zone");
const apercu = document.getElementById("apercu");
const pilule = document.getElementById("pilule");
const bouton = document.getElementById("terminer");
const message = document.getElementById("message");
const reduitSysteme = matchMedia("(prefers-reduced-motion: reduce)");

// Réglage du cœur (`reduites` de defilante_pret ou de la charge) : `data-reduites` sur la
// racine, lu aussi par defilante.css.
let reduitesCoeur = false;
const reduites = () => "reduites" in racine.dataset || reduitSysteme.matches;

/** L'affichage en cours : { id, charge, k, zoneEnvoyee, termine, animation, apercu }. */
let courant = null;
/** Id de la dernière fin envoyée. */
let dernierTermine = null;

function appeler(commande, args) {
  if (!tauri?.core?.invoke) return Promise.reject(new Error("hors de l'app"));
  try {
    return Promise.resolve(tauri.core.invoke(commande, args));
  } catch (e) {
    return Promise.reject(e);
  }
}
function signaler(commande, args) {
  appeler(commande, args).catch((e) => console.warn(`${commande} :`, e));
}

const jeton = (nom) => getComputedStyle(racine).getPropertyValue(nom).trim();
/** Lit une durée de jetons.css (« 260ms », « 0.3s ») en millisecondes. */
function duree(nom) {
  const v = jeton(nom);
  const n = parseFloat(v);
  if (!Number.isFinite(n)) return 0;
  return v.endsWith("ms") || !v.endsWith("s") ? n : n * 1000;
}

const entier = (v) => Number.isSafeInteger(v);
const positif = (v) => entier(v) && v > 0;

/** Vérifie une charge `defilante:montrer` ; null si elle est rejetée. */
function valider(c) {
  const rejeter = (raison) => {
    console.warn(`defilante : charge rejetée (${raison})`);
    return null;
  };
  if (!c || typeof c !== "object") return rejeter("pas un objet");
  if (!entier(c.id) || c.id < 0) return rejeter("id absent");
  const z = c.zone;
  if (!z || !entier(z.x) || !entier(z.y) || !positif(z.largeur) || !positif(z.hauteur)) return rejeter("zone");
  const a = c.apercu;
  if (!a || !entier(a.x) || !entier(a.y) || !positif(a.largeur) || !entier(a.hauteur_max) || a.hauteur_max < 0)
    return rejeter("apercu");
  const p = c.pilule;
  if (!p || !entier(p.x) || !entier(p.y)) return rejeter("pilule");
  if (typeof c.libelle !== "string" || c.libelle.trim() === "") return rejeter("libelle");
  const echelle = Number.isFinite(c.echelle) && c.echelle > 0 ? c.echelle : 1;
  return {
    id: c.id,
    echelle,
    zone: { x: z.x, y: z.y, largeur: z.largeur, hauteur: z.hauteur },
    apercu: { x: a.x, y: a.y, largeur: a.largeur, hauteurMax: a.hauteur_max },
    pilule: { x: p.x, y: p.y },
    libelle: c.libelle,
    reduites: c.reduites === true,
  };
}

const echelleDe = (charge) => {
  const d = window.devicePixelRatio;
  return Number.isFinite(d) && d > 0 ? d : charge.echelle;
};

function poserRect(el, x, y, largeur, hauteur, k) {
  el.style.left = `${x / k}px`;
  el.style.top = `${y / k}px`;
  if (largeur !== null) el.style.width = `${largeur / k}px`;
  if (hauteur !== null) el.style.height = `${hauteur / k}px`;
}

/** Pose tout ce que dit la charge, à l'échelle `k`. */
function poser(lui) {
  const { zone: z, apercu: a, pilule: p } = lui.charge;
  const k = (lui.k = echelleDe(lui.charge));
  lui.dpr = window.devicePixelRatio;
  poserRect(zone, z.x, z.y, z.largeur, z.hauteur, k);
  poserRect(apercu, a.x, a.y, a.largeur, null, k);
  if (lui.apercu.montre) poserHauteurApercu(lui);
  poserRect(pilule, p.x, p.y, null, null, k);
}

function poserHauteurApercu(lui) {
  const h = Math.min(lui.apercu.montre.hauteur, lui.charge.apercu.hauteurMax);
  apercu.style.height = `${h / lui.k}px`;
}

/** Attente au plus de l'échelle de la charge avant d'envoyer la zone (la fenêtre posée
 *  sur un écran d'une autre échelle que celui où elle est née) : bien en deçà du filet de
 *  2 s du cœur (D4). */
const ATTENTE_ECHELLE_MS = 300;
const echelleAtteinte = (lui) => Math.abs((window.devicePixelRatio || 0) - lui.charge.echelle) <= 0.01;

/** Le rectangle de la pilule (pixels CSS), sans la translation de l'animation d'entrée.
 *  Tant que devicePixelRatio n'a pas rejoint l'échelle de la charge, rien ne part, sauf
 *  après ATTENTE_ECHELLE_MS. Repart à chaque changement du rectangle ou de l'échelle. */
function envoyerZone() {
  const lui = courant;
  if (!lui || scene.hidden) return;
  if (lui.attenteEchelle) {
    if (!echelleAtteinte(lui)) return;
    clearTimeout(lui.attenteEchelle);
    lui.attenteEchelle = 0;
  }
  const r = pilule.getBoundingClientRect();
  const t = getComputedStyle(pilule).transform;
  const m = new DOMMatrixReadOnly(t === "none" ? undefined : t);
  const z = { id: lui.id, x: r.x - m.m41, y: r.y - m.m42, largeur: r.width, hauteur: r.height };
  const cle = JSON.stringify([z, window.devicePixelRatio]);
  if (cle === lui.zoneEnvoyee) return;
  lui.zoneEnvoyee = cle;
  signaler("defilante_zone", z);
}
new ResizeObserver(() => envoyerZone()).observe(pilule);
function echelleChangee() {
  if (!courant) return;
  poser(courant);
  envoyerZone();
}
addEventListener("resize", echelleChangee);
/** Un changement d'échelle ne donne pas toujours `resize` (même taille CSS) : pendant un
 *  affichage, devicePixelRatio est relu toutes les VEILLE_ECHELLE_MS. */
const VEILLE_ECHELLE_MS = 100;

function viderApercu(lui) {
  apercu.hidden = true;
  apercu.replaceChildren();
  apercu.style.height = "";
  if (lui?.apercu.url) URL.revokeObjectURL(lui.apercu.url);
  if (lui) lui.apercu.url = null;
}

/** Efface tout, sans rien envoyer. */
function effacer() {
  const lui = courant;
  courant = null;
  if (lui?.animation) lui.animation.cancel();
  if (lui?.attenteEchelle) clearTimeout(lui.attenteEchelle);
  if (lui?.veille) clearInterval(lui.veille);
  viderApercu(lui);
  scene.hidden = true;
  message.hidden = true;
  message.textContent = "";
  bouton.textContent = "";
  delete racine.dataset.reduites;
  if (reduitesCoeur) racine.dataset.reduites = "";
}

export function montrer(brut) {
  const charge = valider(brut);
  if (!charge) return;
  // Un même id montré de nouveau (page rechargée, charge renvoyée) garde sa fin rendue.
  const dejaTermine = dernierTermine === charge.id;
  effacer();
  const lui = {
    id: charge.id,
    charge,
    k: 1,
    zoneEnvoyee: null,
    termine: dejaTermine,
    attenteEchelle: 0,
    dpr: 0,
    veille: 0,
    animation: null,
    // Aperçu : dernier numéro demandé, celui montré, demande en cours, suivant en attente.
    apercu: { demande: -1, montre: null, enCours: false, attente: null, url: null },
  };
  courant = lui;
  if (charge.reduites) racine.dataset.reduites = "";
  bouton.textContent = charge.libelle; // texte seulement
  poser(lui);
  scene.hidden = false;
  const r = reduites();
  const entree = r
    ? [{ opacity: 0 }, { opacity: 1 }]
    : [
        { opacity: 0, transform: `translateY(${jeton("--defilante-pilule-entree-decalage")})` },
        { opacity: 1, transform: "none" },
      ];
  lui.animation = pilule.animate(entree, {
    duration: duree(r ? "--duree-reduite" : "--duree-ressort"),
    easing: r ? "linear" : jeton("--courbe-ressort"),
  });
  // Tout de suite, sans attendre une image : la fenêtre est encore cachée et le cœur la
  // montre à la réception de la zone. Sauf si l'échelle n'est pas encore celle de l'écran
  // de la zone : on attend `resize` (borné).
  lui.veille = setInterval(() => {
    if (courant === lui && window.devicePixelRatio !== lui.dpr) echelleChangee();
  }, VEILLE_ECHELLE_MS);
  if (!echelleAtteinte(lui)) {
    lui.attenteEchelle = setTimeout(() => {
      if (courant !== lui) return;
      lui.attenteEchelle = 0;
      poser(lui);
      envoyerZone();
    }, ATTENTE_ECHELLE_MS);
  }
  envoyerZone();
}

/** Octets reçus de `defilante_apercu` → Blob PNG. */
function blobDe(octets) {
  if (octets instanceof ArrayBuffer || ArrayBuffer.isView(octets)) return new Blob([octets], { type: "image/png" });
  if (Array.isArray(octets)) return new Blob([Uint8Array.from(octets)], { type: "image/png" });
  throw new Error("octets illisibles");
}

async function lireApercu(lui, numero, hauteur) {
  const a = lui.apercu;
  a.enCours = true;
  try {
    const octets = await appeler("defilante_apercu", { id: lui.id, numero });
    if (courant !== lui || (a.montre && a.montre.numero >= numero)) return;
    const url = URL.createObjectURL(blobDe(octets));
    const img = new Image();
    img.alt = "";
    img.src = url;
    const lisible = await img.decode().then(
      () => img.naturalWidth > 0,
      () => false,
    );
    // Illisible : l'aperçu montré reste tel quel (rien de montré qui ne marche pas).
    if (!lisible || courant !== lui || (a.montre && a.montre.numero >= numero)) {
      URL.revokeObjectURL(url);
      return;
    }
    // L'image décodée remplace l'ancienne d'un coup : aucun blanc entre les deux.
    apercu.replaceChildren(img);
    if (a.url) URL.revokeObjectURL(a.url);
    a.url = url;
    a.montre = { numero, hauteur };
    poserHauteurApercu(lui);
    apercu.hidden = false;
  } catch {
    // Refusé (aperçu périmé, capture finie) : le suivant le remplacera.
  } finally {
    a.enCours = false;
    const suivant = a.attente;
    a.attente = null;
    if (suivant && courant === lui) lireApercu(lui, suivant.numero, suivant.hauteur);
  }
}

export function apercuPret(c) {
  const lui = courant;
  if (!lui || !c || c.id !== lui.id) return;
  if (!entier(c.numero) || c.numero < 0 || !entier(c.hauteur) || c.hauteur < 0) {
    console.warn("defilante : aperçu rejeté");
    return;
  }
  const a = lui.apercu;
  if (c.numero <= a.demande) return;
  a.demande = c.numero;
  if (a.enCours) a.attente = { numero: c.numero, hauteur: c.hauteur };
  else lireApercu(lui, c.numero, c.hauteur);
}

export function poserMessage(c) {
  const lui = courant;
  if (!lui || !c || c.id !== lui.id) return;
  if (c.texte === null || c.texte === undefined) {
    message.hidden = true;
    message.textContent = "";
  } else if (typeof c.texte === "string" && c.texte.trim() !== "") {
    message.textContent = c.texte; // texte seulement
    message.hidden = false;
  } else {
    console.warn("defilante : message rejeté");
  }
}

export function cacher(c) {
  if (courant && c && c.id === courant.id) effacer();
}

function terminer() {
  const lui = courant;
  if (!lui || lui.termine) return;
  lui.termine = true; // une seule fin par affichage (et par id)
  dernierTermine = lui.id;
  signaler("defilante_terminer", { id: lui.id });
}

// Le bouton marche à la souris sans que la fenêtre ait le focus : on agit quand le bouton
// gauche est relâché sur la pilule où il a été appuyé. Aucun appui ne prend le focus.
let appuye = false;
const surPilule = (e) => e.target instanceof Element && pilule.contains(e.target);
addEventListener(
  "pointerdown",
  (e) => {
    appuye = e.button === 0 && surPilule(e);
    if (surPilule(e)) e.preventDefault();
  },
  true,
);
addEventListener("mousedown", (e) => e.preventDefault(), true);
addEventListener(
  "pointerup",
  (e) => {
    const avant = appuye;
    appuye = false;
    if (e.button === 0 && avant && surPilule(e)) terminer();
  },
  true,
);
addEventListener("pointercancel", () => (appuye = false), true);
addEventListener("blur", () => (appuye = false));
bouton.addEventListener("click", (e) => {
  // Clic sans pointeur (lecteur d'écran) : rien avant, on agit ici.
  if (e.detail === 0) terminer();
});
addEventListener("focusin", (e) => {
  if (e.target instanceof HTMLElement) e.target.blur();
});
addEventListener("contextmenu", (e) => e.preventDefault(), true);

if (tauri?.event?.listen) {
  const ecoute = tauri.event.listen;
  Promise.all([
    ecoute("defilante:montrer", (e) => montrer(e?.payload)),
    ecoute("defilante:apercu", (e) => apercuPret(e?.payload)),
    ecoute("defilante:message", (e) => poserMessage(e?.payload)),
    ecoute("defilante:cacher", (e) => cacher(e?.payload)),
    suivreTheme(),
  ])
    .then(() => appeler("defilante_pret", {}))
    .then(
      (charge) => {
        reduitesCoeur = charge?.reduites === true;
        if (reduitesCoeur) racine.dataset.reduites = "";
      },
      (e) => console.warn("defilante : pas prête :", e),
    );
}
