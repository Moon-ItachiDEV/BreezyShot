// Page de contrôle d'un film, vidéo ou GIF (contrat 0.7, §10 à §12). La page pose ce que
// le cœur lui dit et rapporte ; elle ne décide rien, ne compte rien (ARCHITECTURE.md §2.5).
//
// Contrat avec le cœur (fenetres/film.rs, application/fenetre_film.rs) ; toutes les places
// en pixels réels entiers, relatives au coin haut gauche de la fenêtre, qui couvre l'écran
// de la zone. Échelle (pixels réels par pixel CSS) : devicePixelRatio, à défaut `echelle`
// de la charge.
// - `film_pret` {} une fois, quand les écoutes (et celle du thème, commun/theme.js) sont en
//   place ; rend { reduites }.
// - écoute `film:montrer` { id, echelle, zone: { x, y, largeur, hauteur }, pilule: { x, y },
//   etat, reduites } : le cadre blanc dehors, le voile léger sur le reste de l'écran, la
//   pilule (haut gauche donné) dans l'état `etat`. Puis `film_zone` { id, x, y, largeur,
//   hauteur } : le rectangle de la pilule en pixels CSS, sans l'animation d'entrée ; une
//   fois par affichage, de nouveau seulement si le rectangle ou devicePixelRatio change.
//   Tant que devicePixelRatio n'égale pas `echelle`, il attend (300 ms au plus). Charge
//   invalide : console.warn, rien montré.
// - écoute `film:etat` { id, phase: "decompte" | "marche" | "pause", duree, pause: {
//   libelle, actif }, arreter, micro: null | { libelle, ouvert }, groupe } : ce que la
//   pilule montre. `micro` null : ni « Micro » ni le trait qui le précède (GIF, PC sans
//   micro). Les textes viennent du cœur et sont les noms des boutons.
// - écoute `film:cacher` { id } : tout est effacé.
// - `film_action` { id, action: "pause" | "arreter" | "micro" } au clic sur un bouton ;
//   « Pause » inactif n'envoie rien ; « Arrêter » une seule fois par id.
// Un `id` qui n'est pas celui de l'affichage en cours est ignoré. Aucun texte ne passe par
// innerHTML. La page ne prend jamais le focus : le clavier reste à l'application filmée.

import { suivreTheme } from "../commun/theme.js";

const tauri = window.__TAURI__;
const racine = document.documentElement;
const scene = document.getElementById("scene");
const cadre = document.getElementById("cadre");
const pilule = document.getElementById("pilule");
const texteDuree = document.getElementById("duree");
const boutons = {
  pause: document.getElementById("pause"),
  arreter: document.getElementById("arreter"),
  micro: document.getElementById("micro"),
};
const trait = document.getElementById("trait");
const reduitSysteme = matchMedia("(prefers-reduced-motion: reduce)");
const SVG = "http://www.w3.org/2000/svg";
const PHASES = ["decompte", "marche", "pause"];

// Pictogrammes 20 × 20 du design (constantes seulement). Chaque forme : [balise,
// attributs, plein].
const PICTOS = {
  pause: [
    ["rect", { x: 5.5, y: 4.5, width: 3, height: 11, rx: 1 }, true],
    ["rect", { x: 11.5, y: 4.5, width: 3, height: 11, rx: 1 }, true],
  ],
  reprendre: [["path", { d: "M6.5 4.6v10.8a.8.8 0 0 0 1.2.7l8.6-5.4a.8.8 0 0 0 0-1.4L7.7 3.9a.8.8 0 0 0-1.2.7z" }, true]],
  arreter: [["rect", { x: 5.5, y: 5.5, width: 9, height: 9, rx: 1.5 }, true]],
  micro: [
    ["rect", { x: 7.5, y: 2.5, width: 5, height: 9.5, rx: 2.5 }],
    ["path", { d: "M4.8 9.5a5.2 5.2 0 0 0 10.4 0M10 14.8v2.7" }],
  ],
  micro_coupe: [
    ["rect", { x: 7.5, y: 2.5, width: 5, height: 9.5, rx: 2.5 }],
    ["path", { d: "M4.8 9.5a5.2 5.2 0 0 0 10.4 0M10 14.8v2.7M3.5 3.5l13 13" }],
  ],
};

/** Pose le pictogramme `nom` dans `bouton`, s'il n'y est pas déjà. */
function poserPicto(bouton, nom) {
  if (bouton.dataset.picto === nom) return;
  bouton.dataset.picto = nom;
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("viewBox", "0 0 20 20");
  svg.setAttribute("aria-hidden", "true");
  for (const [balise, attributs, plein] of PICTOS[nom]) {
    const f = document.createElementNS(SVG, balise);
    for (const [k, v] of Object.entries(attributs)) f.setAttribute(k, String(v));
    if (plein) f.setAttribute("class", "plein");
    svg.append(f);
  }
  bouton.replaceChildren(svg);
}

// Réglage du cœur (`reduites` de film_pret ou de la charge) : `data-reduites` sur la
// racine, lu aussi par film.css.
let reduitesCoeur = false;
const reduites = () => "reduites" in racine.dataset || reduitSysteme.matches;

/** L'affichage en cours : { id, charge, etat, k, dpr, zoneEnvoyee, arrete, animations,
 *  attenteEchelle, veille }. */
let courant = null;
/** Id du dernier « Arrêter » envoyé. */
let dernierArrete = null;

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
const px = (nom) => parseFloat(jeton(nom)) || 0;
/** Lit une durée de jetons.css (« 260ms », « 1.6s ») en millisecondes. */
function duree(nom) {
  const v = jeton(nom);
  const n = parseFloat(v);
  if (!Number.isFinite(n)) return 0;
  return v.endsWith("ms") || !v.endsWith("s") ? n : n * 1000;
}

const entier = (v) => Number.isSafeInteger(v);
const positif = (v) => entier(v) && v > 0;
const texte = (v) => typeof v === "string" && v.trim() !== "";

/** Vérifie un état `film:etat` ; null s'il est rejeté. */
function validerEtat(e, raison) {
  const rejeter = (r) => {
    console.warn(`film : ${raison} rejeté (${r})`);
    return null;
  };
  if (!e || typeof e !== "object") return rejeter("pas un objet");
  if (!entier(e.id) || e.id < 0) return rejeter("id absent");
  if (!PHASES.includes(e.phase)) return rejeter("phase");
  if (typeof e.duree !== "string" || e.duree.trim() === "") return rejeter("duree");
  const p = e.pause;
  if (!p || !texte(p.libelle) || typeof p.actif !== "boolean") return rejeter("pause");
  if (!texte(e.arreter)) return rejeter("arreter");
  const m = e.micro;
  if (m !== null && (!m || !texte(m.libelle) || typeof m.ouvert !== "boolean")) return rejeter("micro");
  if (!texte(e.groupe)) return rejeter("groupe");
  return {
    id: e.id,
    phase: e.phase,
    duree: e.duree,
    pause: { libelle: p.libelle, actif: p.actif },
    arreter: e.arreter,
    micro: m === null ? null : { libelle: m.libelle, ouvert: m.ouvert },
    groupe: e.groupe,
  };
}

/** Vérifie une charge `film:montrer` ; null si elle est rejetée. */
function valider(c) {
  const rejeter = (raison) => {
    console.warn(`film : charge rejetée (${raison})`);
    return null;
  };
  if (!c || typeof c !== "object") return rejeter("pas un objet");
  if (!entier(c.id) || c.id < 0) return rejeter("id absent");
  const z = c.zone;
  if (!z || !entier(z.x) || !entier(z.y) || !positif(z.largeur) || !positif(z.hauteur)) return rejeter("zone");
  const p = c.pilule;
  if (!p || !entier(p.x) || !entier(p.y)) return rejeter("pilule");
  const etat = validerEtat(c.etat, "état de la charge");
  if (!etat) return null;
  if (etat.id !== c.id) return rejeter("état d'un autre id");
  const echelle = Number.isFinite(c.echelle) && c.echelle > 0 ? c.echelle : 1;
  return {
    id: c.id,
    echelle,
    zone: { x: z.x, y: z.y, largeur: z.largeur, hauteur: z.hauteur },
    pilule: { x: p.x, y: p.y },
    etat,
    reduites: c.reduites === true,
  };
}

const echelleDe = (charge) => {
  const d = window.devicePixelRatio;
  return Number.isFinite(d) && d > 0 ? d : charge.echelle;
};

/** Pose le cadre et la pilule à l'échelle `k`. Le cadre entoure la zone dehors : sa boîte
 *  de bordure dépasse la zone du liseré intérieur et du cadre blanc (pixels logiques). */
function poser(lui) {
  const { zone: z, pilule: p } = lui.charge;
  const k = (lui.k = echelleDe(lui.charge));
  lui.dpr = window.devicePixelRatio;
  const dehors = px("--film-cadre-epaisseur");
  const lisere = px("--film-cadre-lisere");
  const s = cadre.style;
  s.left = `${z.x / k - dehors - lisere}px`;
  s.top = `${z.y / k - dehors - lisere}px`;
  s.width = `${z.largeur / k + 2 * lisere}px`;
  s.height = `${z.hauteur / k + 2 * lisere}px`;
  pilule.style.left = `${p.x / k}px`;
  pilule.style.top = `${p.y / k}px`;
}

/** Pose l'état `e` sur la pilule (textes en texte seulement). */
function poserEtat(lui, e) {
  lui.etat = e;
  pilule.dataset.phase = e.phase;
  pilule.setAttribute("aria-label", e.groupe);
  texteDuree.textContent = e.duree;
  const pause = boutons.pause;
  pause.setAttribute("aria-label", e.pause.libelle);
  pause.disabled = !e.pause.actif;
  poserPicto(pause, e.phase === "pause" ? "reprendre" : "pause");
  boutons.arreter.setAttribute("aria-label", e.arreter);
  poserPicto(boutons.arreter, "arreter");
  const micro = boutons.micro;
  micro.hidden = trait.hidden = e.micro === null;
  if (e.micro) {
    micro.setAttribute("aria-label", e.micro.libelle);
    micro.setAttribute("aria-pressed", String(e.micro.ouvert));
    poserPicto(micro, e.micro.ouvert ? "micro" : "micro_coupe");
  }
}

/** Attente au plus de l'échelle de la charge avant d'envoyer la zone (fenêtre posée sur un
 *  écran d'une autre échelle que celui où elle est née) : bien en deçà du filet de 2 s du
 *  cœur (F5). */
const ATTENTE_ECHELLE_MS = 300;
const echelleAtteinte = (lui) => Math.abs((window.devicePixelRatio || 0) - lui.charge.echelle) <= 0.01;

/** Le rectangle de la pilule (pixels CSS), sans l'animation d'entrée (translation et
 *  échelle autour du centre retirées). */
function rectPilule() {
  const r = pilule.getBoundingClientRect();
  const t = getComputedStyle(pilule).transform;
  const m = new DOMMatrixReadOnly(t === "none" ? undefined : t);
  const sx = m.a || 1;
  const sy = m.d || 1;
  const largeur = r.width / sx;
  const hauteur = r.height / sy;
  return {
    x: r.x + r.width / 2 - m.e - largeur / 2,
    y: r.y + r.height / 2 - m.f - hauteur / 2,
    largeur,
    hauteur,
  };
}

/** Envoie la place de la pilule, si elle a changé. Tant que devicePixelRatio n'a pas
 *  rejoint l'échelle de la charge, rien ne part, sauf après ATTENTE_ECHELLE_MS. */
function envoyerZone() {
  const lui = courant;
  if (!lui || scene.hidden) return;
  if (lui.attenteEchelle) {
    if (!echelleAtteinte(lui)) return;
    clearTimeout(lui.attenteEchelle);
    lui.attenteEchelle = 0;
  }
  const z = { id: lui.id, ...rectPilule() };
  const cle = JSON.stringify([z, window.devicePixelRatio]);
  if (cle === lui.zoneEnvoyee) return;
  lui.zoneEnvoyee = cle;
  signaler("film_zone", z);
}
new ResizeObserver(() => envoyerZone()).observe(pilule);
function echelleChangee() {
  if (!courant) return;
  poser(courant);
  envoyerZone();
}
addEventListener("resize", echelleChangee);
/** Un changement d'échelle ne donne pas toujours `resize` : pendant un affichage,
 *  devicePixelRatio est relu toutes les VEILLE_ECHELLE_MS. */
const VEILLE_ECHELLE_MS = 100;

/** Efface tout, sans rien envoyer. */
function effacer() {
  const lui = courant;
  courant = null;
  for (const a of lui?.animations ?? []) a.cancel();
  if (lui?.attenteEchelle) clearTimeout(lui.attenteEchelle);
  if (lui?.veille) clearInterval(lui.veille);
  scene.hidden = true;
  texteDuree.textContent = "";
  delete pilule.dataset.phase;
  for (const b of Object.values(boutons)) b.removeAttribute("aria-label");
  delete racine.dataset.reduites;
  if (reduitesCoeur) racine.dataset.reduites = "";
}

/** Entrée du cadre et de la pilule (260 ms) ; aucune avec les animations réduites. */
function animerEntree(lui) {
  if (reduites()) return;
  const d = duree("--duree-ressort");
  const fin = getComputedStyle(cadre).boxShadow;
  const depart = `0 0 0 0 transparent, inset 0 0 0 0 transparent, 0 0 0 200vmax ${jeton("--scrim")}`;
  lui.animations.push(
    cadre.animate(
      [
        { boxShadow: depart, borderRadius: "0px" },
        { boxShadow: fin, borderRadius: jeton("--film-cadre-rayon") },
      ],
      { duration: d, easing: jeton("--courbe-douce") },
    ),
    pilule.animate(
      [
        {
          opacity: 0,
          transform: `translateY(${jeton("--film-entree-decalage")}) scale(${jeton("--film-entree-echelle")})`,
        },
        { opacity: 1, transform: "none" },
      ],
      { duration: d, easing: jeton("--courbe-ressort") },
    ),
  );
}

export function montrer(brut) {
  const charge = valider(brut);
  if (!charge) return;
  effacer();
  const lui = {
    id: charge.id,
    charge,
    etat: null,
    k: 1,
    dpr: 0,
    zoneEnvoyee: null,
    // Un même id montré de nouveau (page rechargée, charge renvoyée) garde son arrêt.
    arrete: dernierArrete === charge.id,
    animations: [],
    attenteEchelle: 0,
    veille: 0,
  };
  courant = lui;
  if (charge.reduites) racine.dataset.reduites = "";
  poserEtat(lui, charge.etat);
  poser(lui);
  scene.hidden = false;
  animerEntree(lui);
  // Tout de suite : la fenêtre est encore cachée et le cœur la montre à la réception de la
  // zone. Sauf si l'échelle n'est pas encore celle de l'écran de la zone (borné).
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

export function etat(brut) {
  const lui = courant;
  if (!lui || !brut || brut.id !== lui.id) return;
  const e = validerEtat(brut, "état");
  if (e) poserEtat(lui, e);
}

export function cacher(c) {
  if (courant && c && c.id === courant.id) effacer();
}

function agir(action) {
  const lui = courant;
  const b = boutons[action];
  if (!lui || !b || b.hidden || b.disabled) return;
  if (action === "arreter") {
    if (lui.arrete) return;
    lui.arrete = true; // une seule fin par affichage (et par id)
    dernierArrete = lui.id;
  }
  signaler("film_action", { id: lui.id, action });
}

// Les boutons marchent à la souris sans que la fenêtre ait le focus : on agit quand le
// bouton gauche est relâché sur le bouton où il a été appuyé. Aucun appui ne prend le focus.
let appuye = null;
const boutonSous = (e) => (e.target instanceof Element ? e.target.closest("#pilule button[data-action]") : null);
addEventListener(
  "pointerdown",
  (e) => {
    appuye = e.button === 0 ? boutonSous(e) : null;
    if (e.target instanceof Element && pilule.contains(e.target)) e.preventDefault();
  },
  true,
);
addEventListener("mousedown", (e) => e.preventDefault(), true);
addEventListener(
  "pointerup",
  (e) => {
    const avant = appuye;
    appuye = null;
    const b = boutonSous(e);
    if (e.button === 0 && b && b === avant) agir(b.dataset.action);
  },
  true,
);
addEventListener("pointercancel", () => (appuye = null), true);
addEventListener("blur", () => (appuye = null));
for (const b of Object.values(boutons)) {
  b.addEventListener("click", (e) => {
    // Clic sans pointeur (lecteur d'écran) : rien avant, on agit ici.
    if (e.detail === 0) agir(b.dataset.action);
  });
}
addEventListener("focusin", (e) => {
  if (e.target instanceof HTMLElement) e.target.blur();
});
addEventListener("keydown", (e) => e.preventDefault(), true);
addEventListener("contextmenu", (e) => e.preventDefault(), true);

if (tauri?.event?.listen) {
  const ecoute = tauri.event.listen;
  Promise.all([
    ecoute("film:montrer", (e) => montrer(e?.payload)),
    ecoute("film:etat", (e) => etat(e?.payload)),
    ecoute("film:cacher", (e) => cacher(e?.payload)),
    suivreTheme(),
  ])
    .then(() => appeler("film_pret", {}))
    .then(
      (charge) => {
        reduitesCoeur = charge?.reduites === true;
        if (reduitesCoeur) racine.dataset.reduites = "";
      },
      (e) => console.warn("film : pas prête :", e),
    );
}
