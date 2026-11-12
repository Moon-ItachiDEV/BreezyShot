// Vignette : la carte qui reste dans un coin après une capture (ARCHITECTURE.md §11).
// La page montre et rapporte, le cœur décide (durée, résultat des actions, place).
//
// Contrat avec le cœur (pixels réels pour le cœur, convertis ici par devicePixelRatio) :
// - `vignette:preparer` { id, coin, retrait: { haut, bas, gauche, droite }, actions: [{ id,
//   libelle, libelle_fait, place, picto }], duree_ms | null, glisser, reduites, apercu:
//   { largeur, hauteur } } : la page lit l'aperçu (`vignette_image`), pose la carte
//   invisible, puis répond `vignette_zone` { id, x, y, largeur, hauteur, boutons: [{ action,
//   x, y, largeur, hauteur }] } (pixels CSS, sans transformation). Charge invalide :
//   console.warn, rien.
// - `vignette:entrer` { id } : entrée ; au premier dessin, `vignette_entree` { id }.
// - `vignette:survol` { id, dedans } : boutons montrés ou cachés (classe « reveal »).
// - `vignette:minuterie` { id, restant_ms, total_ms | null, en_marche } : la barre.
// - `vignette:confirmer` { id, action, reussi } : « Copié », « Enregistré » 1,8 s.
// - `vignette:supprimee` { id, oui }, `vignette:glisser` { id, en_cours }.
// - `vignette:partir` { id, animee } : coup de vent (miroir à droite), puis
//   `vignette_terminee` { id }. `vignette:cachee` { id } : tout s'arrête, sans réponse.
// - `vignette_action` { id, action } au clic ; `vignette_glisser` { id } après un appui sur
//   l'image et 4 px de déplacement, une fois par appui.
// - `vignette_pret` {} une fois l'écoute en place (événements de la carte et thème,
//   `interface:theme`, voir commun/theme.js).
// Sans window.__TAURI__ (aperçu dans un navigateur) : ?demo=bas_gauche|haut_droite…
//
// Aucune donnée de la charge ne passe par innerHTML : textes par textContent, pictogrammes
// tirés de constantes.

import { suivreTheme } from "../commun/theme.js";

const tauri = window.__TAURI__;
const carte = document.getElementById("carte");
const apercu = document.getElementById("apercu");
const image = document.getElementById("image");
const principales = carte.querySelector(".principales");
const secondaires = carte.querySelector(".secondaires");
const annulation = carte.querySelector(".annulation");
const barre = document.getElementById("barre");
const remplissage = barre.querySelector("i");
const infobulle = document.getElementById("infobulle");

const COINS = ["bas_gauche", "bas_droite", "haut_gauche", "haut_droite"];
const PLACES = ["principale", "secondaire", "fermer", "annulation"];
const SEUIL_GLISSER = 4; // pixels CSS
const SVG = "http://www.w3.org/2000/svg";

// Pictogrammes : constantes seulement. Chaque forme : [balise, attributs, classe].
const PICTOS = {
  copier: [
    "0 0 16 16",
    [
      ["rect", { x: 5, y: 5, width: 8, height: 8, rx: 1.5 }, "ar"],
      ["rect", { x: 3, y: 3, width: 8, height: 8, rx: 1.5 }, "av"],
      ["path", { d: "M3.5 8.4 6.6 11.4 12.5 4.8" }, "ok"],
    ],
  ],
  enregistrer: [
    "0 0 16 16",
    [
      ["path", { d: "M2.5 10.5v1.5a1.5 1.5 0 0 0 1.5 1.5h8a1.5 1.5 0 0 0 1.5-1.5v-1.5" }, "ba"],
      ["path", { d: "M8 2.5v7M5.2 6.8 8 9.6l2.8-2.8" }, "fl"],
      ["path", { d: "M3.5 7.4 6.6 10.4 12.5 3.8" }, "ok"],
    ],
  ],
  supprimer: [
    "0 0 20 20",
    [
      [
        "path",
        {
          d: "M4 6h12M8 6V4.5h4V6M6 6l.7 9.5a1.5 1.5 0 0 0 1.5 1.5h3.6a1.5 1.5 0 0 0 1.5-1.5L14 6",
        },
      ],
    ],
  ],
  annuler: [
    "0 0 20 20",
    [
      ["path", { d: "M7.5 5 4 8.5 7.5 12" }],
      ["path", { d: "M4 8.5h8a4 4 0 0 1 0 8H9" }],
    ],
  ],
  fermer: ["0 0 10 10", [["path", { d: "M1.5 1.5l7 7M8.5 1.5l-7 7" }]]],
  annoter: [
    "0 0 20 20",
    [
      ["path", { d: "M4 16l1-3.5 8.5-8.5a1.8 1.8 0 0 1 2.5 2.5L7.5 15z" }],
      ["path", { d: "M12 5.5l2.5 2.5" }],
    ],
  ],
  epingler: [
    "0 0 20 20",
    [
      [
        "path",
        {
          d:
            "M11.5 3.5 16.5 8.5l-2 .8-3 3 .4 3.2-1.4 1.4-3.2-3.2L4 17l-.4-.4" +
            " 3.3-3.3-3.2-3.2 1.4-1.4 3.2.4 3-3z",
        },
      ],
    ],
  ],
};

function picto(nom, classe) {
  const [vue, formes] = PICTOS[nom] ?? PICTOS.fermer;
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("viewBox", vue);
  svg.setAttribute("aria-hidden", "true");
  if (classe) svg.setAttribute("class", classe);
  for (const [balise, attributs, c] of formes) {
    const f = document.createElementNS(SVG, balise);
    for (const [k, v] of Object.entries(attributs)) f.setAttribute(k, String(v));
    if (c) f.setAttribute("class", c);
    svg.append(f);
  }
  return svg;
}

const jeton = (nom) => getComputedStyle(document.documentElement).getPropertyValue(nom).trim();
/** Durée d'un jeton (« 300ms », « 1.8s ») en millisecondes. */
function duree(nom) {
  const v = jeton(nom);
  const n = parseFloat(v);
  if (!Number.isFinite(n)) return 0;
  return v.endsWith("ms") || !v.endsWith("s") ? n : n * 1000;
}
const px = (nom) => parseFloat(jeton(nom)) || 0;

// Aperçu dans un navigateur seulement : l'image servie sans le cœur.
let demoOctets = null;

function appeler(commande, args) {
  if (!tauri?.core?.invoke)
    return Promise.resolve(commande === "vignette_image" ? demoOctets : undefined);
  try {
    return Promise.resolve(tauri.core.invoke(commande, args)).catch((e) => {
      console.warn(`${commande} :`, e);
      return undefined;
    });
  } catch (e) {
    console.warn(`${commande} :`, e);
    return Promise.resolve(undefined);
  }
}

// Dit « prête » au cœur. Un refus (sous Windows, la page peut parler avant que le cœur ait
// fini de se mettre en place) est retenté sans bruit toutes les 100 ms, 10 s au plus.
const ESSAIS_PRET = 100;
function direPret(commande, reste = ESSAIS_PRET) {
  new Promise((ok) => ok(tauri.core.invoke(commande, {}))).catch((e) => {
    if (reste > 1) setTimeout(() => direPret(commande, reste - 1), 100);
    else console.warn(`${commande} : refusé ${ESSAIS_PRET} fois :`, e);
  });
}

/** Vérifie et nettoie une charge `vignette:preparer` ; null si elle est rejetée. */
function valider(c) {
  const rejeter = (raison) => {
    console.warn(`vignette : charge rejetée (${raison})`);
    return null;
  };
  if (!c || typeof c !== "object") return rejeter("pas un objet");
  if (typeof c.id !== "number" || !Number.isFinite(c.id)) return rejeter("id absent");
  if (!COINS.includes(c.coin)) return rejeter("coin inconnu");
  const entier = (v) => (typeof v === "number" && Number.isFinite(v) && v >= 0 ? v : 0);
  const r = c.retrait && typeof c.retrait === "object" ? c.retrait : {};
  const retrait = {
    haut: entier(r.haut),
    bas: entier(r.bas),
    gauche: entier(r.gauche),
    droite: entier(r.droite),
  };
  const actions = [];
  for (const a of Array.isArray(c.actions) ? c.actions : []) {
    const texte = (v) => typeof v === "string" && v.trim() !== "";
    if (
      !a ||
      !texte(a.id) ||
      !texte(a.libelle) ||
      !PLACES.includes(a.place) ||
      actions.some((b) => b.id === a.id)
    ) {
      console.warn("vignette : action rejetée");
      continue;
    }
    actions.push({
      id: a.id,
      libelle: a.libelle,
      libelleFait: texte(a.libelle_fait) ? a.libelle_fait : null,
      place: a.place,
      picto: Object.hasOwn(PICTOS, a.picto) ? a.picto : null,
    });
  }
  if (actions.length === 0) return rejeter("aucune action");
  const a = c.apercu && typeof c.apercu === "object" ? c.apercu : {};
  if (!(entier(a.largeur) > 0 && entier(a.hauteur) > 0)) return rejeter("aperçu absent");
  const dureeMs =
    typeof c.duree_ms === "number" && Number.isFinite(c.duree_ms) && c.duree_ms > 0
      ? c.duree_ms
      : null;
  return {
    id: c.id,
    coin: c.coin,
    retrait,
    actions,
    dureeMs,
    glisser: c.glisser === true,
    reduites: c.reduites === true,
  };
}

// La carte en cours : { id, coin, actions, reduites, glisser, url, zone, animation,
// barre, supprimee, partie, entree }
let courant = null;
// Le cœur décide (réglage « Animations » et système) ; la page suit sa valeur seule.
const reduites = () => courant?.reduites === true;
const aDroite = () => courant?.coin.endsWith("droite");
const enHaut = () => courant?.coin.startsWith("haut");

/** Remet la page à vide (aucun appel au cœur). */
function vider() {
  cacherInfobulle();
  if (courant) {
    courant.animation?.cancel();
    courant.barre?.cancel();
    for (const t of courant.minuteries) clearTimeout(t);
    if (courant.url) URL.revokeObjectURL(courant.url);
  }
  courant = null;
  carte.hidden = true;
  carte.className = "carte";
  carte.removeAttribute("style");
  image.removeAttribute("src");
  principales.replaceChildren();
  secondaires.replaceChildren();
  annulation.replaceChildren();
  carte.querySelector(".croix")?.remove();
  barre.hidden = true;
}

function creerBouton(action) {
  const b = document.createElement("button");
  b.type = "button";
  b.tabIndex = -1;
  b.dataset.action = action.id;
  b.setAttribute("aria-label", action.libelle);
  if (action.place === "principale") {
    b.className = "pilule";
    if (action.picto) b.append(picto(action.picto, "pi"));
    const libelles = document.createElement("span");
    libelles.className = "libelles";
    const avant = document.createElement("span");
    avant.className = "avant";
    avant.textContent = action.libelle;
    libelles.append(avant);
    if (action.libelleFait) {
      const apres = document.createElement("span");
      apres.className = "apres";
      apres.textContent = action.libelleFait;
      libelles.append(apres);
    }
    b.append(libelles);
  } else if (action.place === "annulation") {
    b.className = "annuler";
    if (action.picto) b.append(picto(action.picto));
    b.append(document.createTextNode(action.libelle));
  } else {
    b.className = action.place === "fermer" ? "rond croix" : "rond";
    b.dataset.infobulle = action.libelle;
    b.append(picto(action.picto ?? "fermer"));
  }
  return b;
}

/** Rectangle d'un élément dans la page, sans les transformations (animations). */
function rectPage(el) {
  let x = 0;
  let y = 0;
  let n = el;
  while (n && n !== carte) {
    x += n.offsetLeft;
    y += n.offsetTop;
    n = n.offsetParent;
  }
  return {
    x: carte.offsetLeft + carte.clientLeft + x,
    y: carte.offsetTop + carte.clientTop + y,
    largeur: el.offsetWidth,
    hauteur: el.offsetHeight,
  };
}

function poserCarte(c) {
  const dpr = window.devicePixelRatio || 1;
  const marge = px("--vignette-marge");
  const s = carte.style;
  if (c.coin.endsWith("gauche")) s.left = `${c.retrait.gauche / dpr + marge}px`;
  else s.right = `${c.retrait.droite / dpr + marge}px`;
  if (c.coin.startsWith("bas")) s.bottom = `${c.retrait.bas / dpr + marge}px`;
  else s.top = `${c.retrait.haut / dpr + marge}px`;
}

function envoyerZone() {
  if (!courant || carte.hidden) return;
  const z = rectPage(carte);
  z.x = carte.offsetLeft;
  z.y = carte.offsetTop;
  z.largeur = carte.offsetWidth;
  z.hauteur = carte.offsetHeight;
  if (z.x < 0 || z.y < 0 || z.x + z.largeur > innerWidth || z.y + z.hauteur > innerHeight) {
    console.warn("vignette : la carte dépasse la fenêtre");
  }
  const boutons = [...carte.querySelectorAll("button[data-action]")]
    .filter((b) => b.offsetParent !== null)
    .map((b) => ({ action: b.dataset.action, ...rectPage(b) }));
  appeler("vignette_zone", { id: courant.id, ...z, boutons });
}

export async function preparer(brut) {
  const c = valider(brut);
  if (!c) return;
  vider();
  const lui = {
    ...c,
    url: null,
    animation: null,
    barre: null,
    minuteries: [],
    supprimee: false,
    partie: false,
  };
  courant = lui;
  carte.classList.toggle("reduites", reduites());
  carte.classList.toggle("glissable", lui.glisser);
  for (const a of lui.actions) {
    const b = creerBouton(a);
    if (a.place === "principale") principales.append(b);
    else if (a.place === "secondaire") secondaires.append(b);
    else if (a.place === "annulation") annulation.append(b);
    else carte.append(b);
  }
  const premiere = lui.actions.find((a) => a.place === "principale");
  principales
    .querySelector(`[data-action="${CSS.escape(premiere?.id ?? "")}"]`)
    ?.classList.add("accent");
  barre.hidden = lui.dureeMs === null;
  remplissage.style.transform = "none";
  poserCarte(lui);

  const octets = await appeler("vignette_image", { id: lui.id, quoi: "apercu" });
  if (lui !== courant) return;
  if (
    !octets ||
    !(octets instanceof ArrayBuffer || ArrayBuffer.isView(octets) || Array.isArray(octets))
  ) {
    console.warn("vignette : aperçu absent");
    return;
  }
  const donnees = Array.isArray(octets) ? new Uint8Array(octets) : octets;
  lui.url = URL.createObjectURL(new Blob([donnees], { type: "image/png" }));
  image.src = lui.url;
  try {
    await image.decode();
  } catch {
    if (lui === courant) console.warn("vignette : aperçu illisible");
    return;
  }
  if (lui !== courant) return;
  carte.hidden = false;
  envoyerZone();
}

function entrer(id) {
  const lui = courant;
  if (!lui || lui.id !== id || lui.entree) return;
  lui.entree = true;
  carte.classList.add("montree");
  const decalage = px("--vignette-entree-decalage") * (enHaut() ? -1 : 1);
  lui.animation = reduites()
    ? carte.animate([{ opacity: 0 }, { opacity: 1 }], {
        duration: duree("--duree-reduite"),
        easing: "linear",
      })
    : carte.animate(
        [
          { opacity: 0, transform: `translateY(${decalage}px) scale(.96)` },
          { opacity: 1, transform: "none" },
        ],
        { duration: duree("--duree-vignette"), easing: jeton("--courbe-ressort") },
      );
  requestAnimationFrame(() => {
    if (lui === courant) appeler("vignette_entree", { id });
  });
}

function minuterie(m) {
  const lui = courant;
  if (!lui || lui.id !== m.id) return;
  lui.barre?.cancel();
  lui.barre = null;
  const total = typeof m.total_ms === "number" && m.total_ms > 0 ? m.total_ms : null;
  barre.hidden = total === null;
  if (total === null) return;
  const restant = Math.max(0, Math.min(total, Number(m.restant_ms) || 0));
  const part = restant / total;
  remplissage.style.transform = `scaleX(${part})`;
  if (m.en_marche && restant > 0) {
    lui.barre = remplissage.animate(
      [{ transform: `scaleX(${part})` }, { transform: "scaleX(0)" }],
      {
        duration: restant,
        easing: "linear",
        fill: "forwards",
      },
    );
  }
}

function confirmer(m) {
  const lui = courant;
  if (!lui || lui.id !== m.id || m.reussi !== true) return;
  const b = carte.querySelector(`button.pilule[data-action="${CSS.escape(String(m.action))}"]`);
  if (!b) return;
  b.classList.remove("onde", "fait");
  void b.offsetWidth;
  b.classList.add("onde", "fait");
  clearTimeout(b._fin);
  b._fin = setTimeout(
    () => b.classList.remove("onde", "fait"),
    duree("--duree-vignette-confirmation"),
  );
  lui.minuteries.push(b._fin);
}

function supprimee(m) {
  const lui = courant;
  if (!lui || lui.id !== m.id) return;
  lui.supprimee = m.oui === true;
  cacherInfobulle();
  carte.classList.toggle("supprimee", lui.supprimee);
}

function partir(m) {
  const lui = courant;
  if (!lui || lui.id !== m.id || lui.partie) return;
  lui.partie = true;
  cacherInfobulle();
  carte.classList.remove("reveal");
  lui.animation?.cancel();
  const fin = () => {
    if (lui !== courant) return;
    appeler("vignette_terminee", { id: lui.id });
    vider();
  };
  if (m.animee === false || !carte.classList.contains("montree")) return fin();
  const s = aDroite() ? -1 : 1; // la carte part vers le bord le plus proche
  const recul = px("--vignette-vent-recul");
  const angle = parseFloat(jeton("--vignette-vent-angle")) || 0;
  const loin = jeton("--vignette-vent-decalage");
  const haut = px("--vignette-entree-decalage");
  const flou = jeton("--vignette-vent-flou");
  lui.animation = reduites()
    ? carte.animate([{ opacity: 1 }, { opacity: 0 }], {
        duration: duree("--duree-reduite"),
        fill: "forwards",
      })
    : carte.animate(
        [
          { opacity: 1, transform: "none", filter: "blur(0px)" },
          {
            opacity: 1,
            transform: `translateX(${s * recul}px) rotate(${s}deg)`,
            filter: "blur(0px)",
            offset: 0.22,
          },
          {
            opacity: 0,
            transform: `translate(calc(${loin} * ${-s}), -${haut}px) rotate(${-s * angle}deg)`,
            filter: `blur(${flou})`,
          },
        ],
        { duration: duree("--duree-vent"), easing: jeton("--courbe-vent"), fill: "forwards" },
      );
  lui.animation.onfinish = fin;
}

// Infobulles des boutons ronds (450 ms de survol).
let infobulleMinuterie = 0;
function cacherInfobulle() {
  clearTimeout(infobulleMinuterie);
  infobulle.classList.remove("on");
}
function montrerInfobulle(b) {
  infobulle.textContent = b.dataset.infobulle;
  const r = b.getBoundingClientRect();
  const l = infobulle.offsetWidth;
  const h = infobulle.offsetHeight;
  const gauche = Math.min(innerWidth - l - 4, r.left + r.width / 2 - l / 2);
  infobulle.style.left = `${Math.max(4, gauche)}px`;
  infobulle.style.top = `${Math.max(4, r.top - h - 6)}px`;
  infobulle.classList.add("on");
}
carte.addEventListener("pointerover", (e) => {
  const b = e.target instanceof Element ? e.target.closest("[data-infobulle]") : null;
  if (!b || courant?.supprimee) return cacherInfobulle();
  clearTimeout(infobulleMinuterie);
  infobulleMinuterie = setTimeout(() => montrerInfobulle(b), 450);
});
carte.addEventListener("pointerleave", cacherInfobulle);

// Clics : on agit au relâchement sur le bouton même où l'appui a commencé (la fenêtre n'a
// jamais le focus). Glisser : appui sur l'image, puis 4 px.
let appuye = null;
let depart = null;
const boutonSous = (e) =>
  e.target instanceof Element ? e.target.closest("#carte button[data-action]") : null;
addEventListener(
  "pointerdown",
  (e) => {
    appuye = e.button === 0 ? boutonSous(e) : null;
    depart = null;
    if (
      e.button === 0 &&
      !appuye &&
      courant?.glisser &&
      !courant.supprimee &&
      !courant.partie &&
      apercu.contains(e.target)
    ) {
      depart = { x: e.clientX, y: e.clientY, envoye: false };
    }
    if (carte.contains(e.target)) e.preventDefault();
  },
  true,
);
addEventListener(
  "pointermove",
  (e) => {
    if (!depart || depart.envoye || !courant || (e.buttons & 1) === 0) return;
    if (Math.hypot(e.clientX - depart.x, e.clientY - depart.y) < SEUIL_GLISSER) return;
    depart.envoye = true;
    cacherInfobulle();
    appeler("vignette_glisser", { id: courant.id });
  },
  true,
);
addEventListener(
  "pointerup",
  (e) => {
    const b = boutonSous(e);
    const avant = appuye;
    appuye = null;
    depart = null;
    if (e.button !== 0 || !b || b !== avant || !courant || courant.partie) return;
    const r = b.getBoundingClientRect();
    b.style.setProperty("--rx", `${e.clientX - r.left}px`);
    b.style.setProperty("--ry", `${e.clientY - r.top}px`);
    appeler("vignette_action", { id: courant.id, action: b.dataset.action });
  },
  true,
);
addEventListener("pointercancel", () => ((appuye = null), (depart = null)), true);
addEventListener("dragstart", (e) => e.preventDefault(), true);
addEventListener("contextmenu", (e) => e.preventDefault(), true);

function pour(id, f) {
  return (evenement) => {
    const p = evenement?.payload;
    if (p && typeof p === "object" && (id === null || (courant && p.id === courant.id))) f(p);
  };
}

const ECOUTES = {
  "vignette:preparer": pour(null, preparer),
  "vignette:entrer": pour(true, (p) => entrer(p.id)),
  "vignette:survol": pour(true, (p) => {
    if (!p.dedans) cacherInfobulle();
    carte.classList.toggle("reveal", p.dedans === true && !courant.supprimee && !courant.partie);
  }),
  "vignette:minuterie": pour(true, minuterie),
  "vignette:confirmer": pour(true, confirmer),
  "vignette:supprimee": pour(true, supprimee),
  "vignette:glisser": pour(true, (p) => carte.classList.toggle("glisse", p.en_cours === true)),
  "vignette:partir": pour(true, partir),
  "vignette:cachee": pour(true, vider),
};

if (tauri?.event?.listen) {
  Promise.all([
    ...Object.entries(ECOUTES).map(([nom, f]) => tauri.event.listen(nom, f)),
    suivreTheme(),
  ]).then(
    () => direPret("vignette_pret"),
    (e) => console.warn("vignette : écoute impossible :", e),
  );
} else {
  // Aperçu dans un navigateur : une image dessinée, la carte montrée et ses boutons.
  window.vignetteApercu = { ECOUTES };
  const coin = new URLSearchParams(location.search).get("demo");
  if (COINS.includes(coin)) {
    const toile = document.createElement("canvas");
    toile.width = 640;
    toile.height = 400;
    const g = toile.getContext("2d");
    const d = g.createLinearGradient(0, 0, 640, 400);
    d.addColorStop(0, "#8FBDF7");
    d.addColorStop(1, "#B3A4F5");
    g.fillStyle = d;
    g.fillRect(0, 0, 640, 400);
    toile.toBlob(async (blob) => {
      const octets = await blob.arrayBuffer();
      const charge = {
        id: 1,
        coin,
        retrait: {},
        duree_ms: 6000,
        glisser: true,
        reduites: false,
        apercu: { largeur: 640, hauteur: 400 },
        actions: [
          {
            id: "copier",
            libelle: "Copier",
            libelle_fait: "Copié",
            place: "principale",
            picto: "copier",
          },
          {
            id: "enregistrer",
            libelle: "Enregistrer",
            libelle_fait: "Enregistré",
            place: "principale",
            picto: "enregistrer",
          },
          { id: "supprimer", libelle: "Supprimer", place: "secondaire", picto: "supprimer" },
          { id: "fermer", libelle: "Fermer", place: "fermer", picto: "fermer" },
          { id: "annuler", libelle: "Annuler", place: "annulation", picto: "annuler" },
        ],
      };
      demoOctets = octets;
      await preparer(charge);
      entrer(1);
      carte.classList.add("reveal");
    });
  }
}
