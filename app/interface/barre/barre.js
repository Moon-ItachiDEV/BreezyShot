// Barre de capture « Galet » (design/barre-capture, piste A, choisie le 16/11/2026). La page
// montre et rapporte, le cœur décide.
//
// Contrat avec le cœur (application/fenetre_barre.rs) :
// - `barre_pret` une fois, quand l'écoute de `barre:ouvrir` et du thème est en place ; rend
//   la charge de la première ouverture (ou null).
// - écoute `barre:ouvrir` { boutons: [id], verrouilles: [id], retardateur_s: n | null,
//   retardateurs: [n] }, et `barre:vider` (barre cachée : rien d'ancien à la prochaine
//   ouverture). `verrouilles` : les fonctions Pro sans licence, montrées éteintes.
// - `barre_taille` { largeur, hauteur } : la carte mesurée, étage du retardateur ouvert
//   compris, après chaque ouverture et à chaque étage ouvert ou fermé.
// - `barre_choisi` { id } : un bouton choisi. Pour un bouton verrouillé, le cœur ouvre
//   Réglages › Licence au lieu de lancer la fonction.
// - `barre_retardateur` { secondes } : un choix de l'étage du retardateur ; rend la valeur
//   en vigueur, que montre le cadran. La barre reste ouverte.
// - `barre_fermer` : Échap, ou un clic dans la marge transparente autour de la carte.
// Clavier : Tab, Maj + Tab, flèches gauche et droite, Début, Fin d'un bouton à l'autre (la
// première pression prend le premier ou le dernier) ; Flèche bas ouvre l'étage du
// retardateur ; Entrée, Espace ; Échap ferme l'étage, sinon la barre.
// Sans window.__TAURI__ (aperçu dans un navigateur) : une barre de démonstration.
//
// Aucune donnée ne passe par innerHTML : textes par textContent, pictogrammes tirés de
// constantes. Le nom d'un bouton vient dans une étiquette sous la barre, au survol et au
// clavier ; il est aussi l'étiquette lue par les lecteurs d'écran.

import { suivreTheme } from "../commun/theme.js";
import { picto } from "../commun/pictos.js";

const tauri = window.__TAURI__;
const SVG = "http://www.w3.org/2000/svg";
const barre = document.getElementById("barre");
const nomVu = document.getElementById("nom");

const NOMS = {
  capturer_zone: "Zone",
  capturer_plein_ecran: "Plein écran",
  capturer_fenetre: "Fenêtre",
  capturer_defilante: "Capture défilante",
  capturer_video: "Vidéo",
  capturer_gif: "GIF",
  reglages: "Réglages",
};
const CAPTURES = ["capturer_zone", "capturer_plein_ecran", "capturer_fenetre"];
const FILMS = ["capturer_defilante", "capturer_video", "capturer_gif"];
const NOM_RETARDATEUR = "Retardateur";
// Fermeture de l'étage après un choix : le temps de voir la pastille se poser.
const DELAI_FERMETURE_MS = 170;
// L'étage se replie en 260 ms (--duree-ressort) : la fenêtre ne rétrécit qu'après.
const DELAI_REPLI_MS = 300;

function appeler(commande, args) {
  if (!tauri?.core?.invoke) return Promise.resolve(null);
  return Promise.resolve(tauri.core.invoke(commande, args)).catch((e) => {
    console.warn(`barre : ${commande} refusé :`, e);
    throw e;
  });
}

let ouverture = 0; // numéro de la charge montrée : une réponse d'une charge passée est ignorée
let choisi = false;
function choisir(id) {
  if (choisi) return;
  choisi = true;
  // Refusé : le bouton peut être choisi de nouveau.
  appeler("barre_choisi", { id }).catch(() => {
    choisi = false;
  });
}

// --- Taille -------------------------------------------------------------------------

let etage = null; // l'étage du retardateur : l'élément, ou null sans retardateur
let ouvert = null; // l'étage ouvert : { bouton }
let minuterieRepli = null;
let minuterieChoix = null;

// La hauteur que doit avoir la fenêtre : la carte fermée, plus l'étage quand il est ouvert
// (sa hauteur finale, pas celle de l'animation : la fenêtre grandit avant qu'il se déplie).
function hauteurVoulue() {
  const ferme = barre.offsetHeight - (etage ? etage.offsetHeight : 0);
  return ouvert && etage ? ferme + etage.firstElementChild.scrollHeight : ferme;
}

// La largeur est celle de la mise en page (computed style), pas le rectangle à l'écran : la
// carte qui entre est à 0,97 d'échelle un court instant.
function signalerTaille() {
  if (barre.hidden) return Promise.resolve();
  const largeur = Math.ceil(parseFloat(getComputedStyle(barre).width));
  return appeler("barre_taille", { largeur, hauteur: hauteurVoulue() }).catch(() => {});
}

// --- Pictogrammes -------------------------------------------------------------------

// Les chiffres du cadran, en traits dans une case de 3,4 × 6,4 (le « 1 » : 2 de large),
// pour que le délai ne dépende pas de la police de Windows.
const CHIFFRES = {
  0: { l: 3.4, forme: ["rect", 0, 0, 3.4, 6.4, 1.7] },
  1: { l: 2, forme: ["path", "M0.1 1.5 1.9 0.2V6.4"] },
  3: {
    l: 3.4,
    forme: [
      "path",
      "M0.4 0.9C0.7 0.35 1.2 0 1.8 0C2.6 0 3.2 0.5 3.2 1.4C3.2 2.3 2.5 2.8 1.5 2.8C2.6 2.8 3.4 3.4 3.4 4.6C3.4 5.8 2.7 6.4 1.7 6.4C1 6.4 0.5 6.1 0.1 5.5",
    ],
  },
  5: {
    l: 3.4,
    forme: [
      "path",
      "M3.1 0.2H0.7L0.4 3C0.8 2.6 1.3 2.5 1.8 2.5C2.8 2.5 3.4 3.3 3.4 4.4C3.4 5.6 2.7 6.4 1.7 6.4C1 6.4 0.5 6.1 0.1 5.5",
    ],
  },
};
const ECHELLE_CHIFFRES = 1.1;
const ECART_CHIFFRES = 1.2;

function chiffre(c, x, y) {
  const g = document.createElementNS(SVG, "g");
  g.setAttribute("transform", `translate(${x} ${y}) scale(${ECHELLE_CHIFFRES})`);
  const [genre, ...v] = c.forme;
  let e;
  if (genre === "rect") {
    e = document.createElementNS(SVG, "rect");
    e.setAttribute("x", v[0]);
    e.setAttribute("y", v[1]);
    e.setAttribute("width", v[2]);
    e.setAttribute("height", v[3]);
    e.setAttribute("rx", v[4]);
  } else {
    e = document.createElementNS(SVG, "path");
    e.setAttribute("d", v[0]);
  }
  g.append(e);
  return g;
}

// Retardateur armé : le cadran sans aiguille, le délai écrit dedans ; le bouton garde sa
// taille, la barre ne change pas de largeur. Des traits pour 3, 5 et 10 (les délais du
// cœur) ; un autre nombre, du texte.
function pictoDelai(secondes) {
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("viewBox", "0 0 20 20");
  svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("class", "p");
  const cadran = document.createElementNS(SVG, "circle");
  cadran.setAttribute("cx", "10");
  cadran.setAttribute("cy", "11");
  cadran.setAttribute("r", "6.5");
  const crete = document.createElementNS(SVG, "path");
  crete.setAttribute("d", "M8 2.5h4M15 5.2l1.2-1.2");
  svg.append(cadran, crete);
  const glyphes = String(secondes)
    .split("")
    .map((d) => CHIFFRES[d]);
  if (glyphes.every(Boolean)) {
    const largeur =
      glyphes.reduce((s, c) => s + c.l * ECHELLE_CHIFFRES, 0) + ECART_CHIFFRES * (glyphes.length - 1);
    let x = 10 - largeur / 2;
    const y = 11 - (6.4 * ECHELLE_CHIFFRES) / 2;
    const g = document.createElementNS(SVG, "g");
    g.setAttribute("class", "chiffres");
    for (const c of glyphes) {
      g.append(chiffre(c, x, y));
      x += c.l * ECHELLE_CHIFFRES + ECART_CHIFFRES;
    }
    svg.append(g);
  } else {
    const t = document.createElementNS(SVG, "text");
    t.setAttribute("x", "10");
    t.setAttribute("y", "13.9");
    t.textContent = String(secondes);
    svg.append(t);
  }
  return svg;
}

// --- Construction -------------------------------------------------------------------

function bouton(id, nom, { pictogramme = id, verrouille = false } = {}) {
  const b = document.createElement("button");
  b.type = "button";
  b.className = "b";
  b.dataset.id = id;
  b.dataset.nom = nom;
  b.append(picto(pictogramme));
  b.querySelector("svg").classList.add("p");
  if (verrouille) {
    b.classList.add("pro");
    b.setAttribute("aria-label", `${nom}, Pro`);
    b.setAttribute("aria-description", "Ouvre la licence");
  } else {
    b.setAttribute("aria-label", nom);
  }
  return b;
}

function filet() {
  const s = document.createElement("span");
  s.className = "filet";
  s.setAttribute("role", "separator");
  s.setAttribute("aria-orientation", "vertical");
  return s;
}

function puce() {
  const p = document.createElement("span");
  p.className = "puce";
  p.setAttribute("aria-hidden", "true");
  p.textContent = "Pro";
  return p;
}

let boutonRetardateur = null;
let choixRetardateur = [];

function poserRetardateur(secondes) {
  if (!boutonRetardateur || typeof secondes !== "number") return;
  const arme = secondes > 0;
  boutonRetardateur.classList.toggle("arme", arme);
  boutonRetardateur.setAttribute("aria-label", arme ? `${NOM_RETARDATEUR}, ${secondes} s` : NOM_RETARDATEUR);
  const nouveau = arme ? pictoDelai(secondes) : picto("retardateur");
  nouveau.classList.add("p");
  boutonRetardateur.querySelector("svg").replaceWith(nouveau);
  for (const c of choixRetardateur) {
    c.setAttribute("aria-checked", Number(c.dataset.secondes) === secondes ? "true" : "false");
  }
}

function montrer(charge) {
  if (!charge || !Array.isArray(charge.boutons)) return;
  ouverture++;
  choisi = false;
  fermerEtage(false, false);
  clearTimeout(minuterieChoix);
  clearTimeout(minuterieRepli);
  viderNom();
  const ids = charge.boutons.filter((id) => typeof id === "string" && Object.hasOwn(NOMS, id));
  const verrouilles = Array.isArray(charge.verrouilles) ? charge.verrouilles : [];
  const rang = document.createElement("div");
  rang.className = "rang";
  const groupes = [[], [], []];

  for (const id of CAPTURES.filter((i) => ids.includes(i))) groupes[0].push(bouton(id, NOMS[id]));
  boutonRetardateur = null;
  choixRetardateur = [];
  etage = null;
  let pli = null;
  if (typeof charge.retardateur_s === "number") {
    const b = bouton("retardateur", NOM_RETARDATEUR);
    b.setAttribute("aria-haspopup", "true");
    b.setAttribute("aria-expanded", "false");
    b.setAttribute("aria-controls", "etage-retardateur");
    boutonRetardateur = b;
    groupes[0].push(b);
    etage = document.createElement("div");
    etage.className = "etage";
    etage.id = "etage-retardateur";
    pli = document.createElement("div");
    const ligne = document.createElement("div");
    ligne.className = "etage-rang";
    ligne.setAttribute("role", "radiogroup");
    ligne.setAttribute("aria-label", NOM_RETARDATEUR);
    const delais = Array.isArray(charge.retardateurs) ? charge.retardateurs : [];
    for (const s of delais.filter((n) => Number.isInteger(n) && n >= 0)) {
      const c = document.createElement("button");
      c.type = "button";
      c.className = "choix";
      c.setAttribute("role", "radio");
      c.tabIndex = -1;
      c.dataset.secondes = String(s);
      c.textContent = s === 0 ? "Aucun" : `${s} s`;
      c.addEventListener("click", () => changerRetardateur(s));
      ligne.append(c);
      choixRetardateur.push(c);
    }
    pli.append(ligne);
    etage.append(pli);
    if (!choixRetardateur.length) {
      // Aucun choix à proposer : pas de bouton qui ouvre du vide.
      groupes[0].pop();
      boutonRetardateur = null;
      etage = null;
    }
  }
  for (const id of FILMS.filter((i) => ids.includes(i))) {
    groupes[1].push(bouton(id, NOMS[id], { verrouille: verrouilles.includes(id) }));
  }
  if (ids.includes("reglages")) groupes[2].push(bouton("reglages", NOMS.reglages));

  let place = false;
  for (const g of groupes.filter((x) => x.length)) {
    if (place) rang.append(filet());
    rang.append(...g);
    // Une seule puce « Pro », après le dernier bouton éteint.
    if (g === groupes[1] && g.some((b) => b.classList.contains("pro"))) rang.append(puce());
    place = true;
  }
  barre.replaceChildren(rang);
  if (etage) barre.append(etage);
  for (const b of barre.querySelectorAll(".b")) b.addEventListener("click", () => cliquer(b));
  poserRetardateur(charge.retardateur_s);
  barre.hidden = false;
  // Entrée : la carte descend en ressort.
  barre.classList.add("entree");
  void barre.offsetWidth;
  barre.classList.remove("entree");
  if (document.activeElement && document.activeElement !== document.body) document.activeElement.blur();
  signalerTaille();
}

function vider() {
  ouverture++;
  fermerEtage(false, false);
  clearTimeout(minuterieChoix);
  clearTimeout(minuterieRepli);
  viderNom();
  barre.hidden = true;
  barre.replaceChildren();
  boutonRetardateur = null;
  choixRetardateur = [];
  etage = null;
}

function cliquer(b) {
  if (b === boutonRetardateur) basculerEtage(b);
  else choisir(b.dataset.id);
}

function changerRetardateur(secondes) {
  const mienne = ouverture;
  appeler("barre_retardateur", { secondes })
    .then((v) => {
      if (mienne === ouverture) poserRetardateur(typeof v === "number" ? v : secondes);
    })
    .catch(() => {});
  clearTimeout(minuterieChoix);
  minuterieChoix = setTimeout(() => fermerEtage(true, true), DELAI_FERMETURE_MS);
}

// --- Étage du retardateur -----------------------------------------------------------

function elements() {
  return ouvert ? Array.from(etage.querySelectorAll(".choix")) : [];
}

function ouvrirEtage(b) {
  if (!etage) return;
  clearTimeout(minuterieRepli);
  cacherNom();
  b.setAttribute("aria-expanded", "true");
  ouvert = { bouton: b };
  const mon = ouvert;
  // La fenêtre grandit d'abord : l'étage ne se déplie qu'une fois la taille rendue, dans une
  // fenêtre déjà assez haute.
  signalerTaille().then(() =>
    requestAnimationFrame(() => {
      if (ouvert !== mon) return;
      etage.classList.add("ouvert");
      const liste = elements();
      (liste.find((c) => c.getAttribute("aria-checked") === "true") || liste[0])?.focus();
    }),
  );
}

// `rendreFocus` : le focus revient au bouton ; `avecTaille` : la fenêtre rétrécit, une fois
// l'étage replié.
function fermerEtage(rendreFocus, avecTaille) {
  if (!ouvert) return;
  const { bouton: b } = ouvert;
  ouvert = null;
  clearTimeout(minuterieChoix);
  etage?.classList.remove("ouvert");
  b.setAttribute("aria-expanded", "false");
  if (rendreFocus) b.focus();
  clearTimeout(minuterieRepli);
  if (avecTaille) {
    minuterieRepli = setTimeout(() => {
      if (!ouvert) signalerTaille();
    }, DELAI_REPLI_MS);
  }
}

function basculerEtage(b) {
  if (ouvert) fermerEtage(true, true);
  else ouvrirEtage(b);
}

// --- Nom au survol ------------------------------------------------------------------

// Le milieu d'un bouton, compté depuis le bord gauche de la boîte. offsetLeft, pas le
// rectangle à l'écran : le bouton enfoncé ou la barre qui entre sont à une autre échelle
// un court instant.
function centreDans(b) {
  let x = b.offsetWidth / 2;
  for (let e = b; e && e !== document.getElementById("boite"); e = e.offsetParent) x += e.offsetLeft;
  return x;
}

function cacherNom() {
  nomVu.classList.remove("vu", "glisse");
}

// Barre cachée : l'étiquette est vidée sans transition, pour qu'aucun nom ne reste (ni ne
// s'efface) à la prochaine ouverture.
function viderNom() {
  cacherNom();
  nomVu.style.transition = "none";
  nomVu.textContent = "";
}

function montrerNom(b) {
  if (ouvert || !b.dataset.nom) return;
  nomVu.style.transition = "";
  nomVu.textContent = b.dataset.nom;
  nomVu.classList.toggle("glisse", nomVu.classList.contains("vu"));
  nomVu.style.left = `${Math.round(centreDans(b))}px`;
  nomVu.classList.add("vu");
}

barre.addEventListener("pointerover", (e) => {
  // Le nom ne tient que sur son bouton : un filet, la puce ou la marge l'effacent.
  const b = e.target.closest?.(".b");
  if (b && barre.contains(b)) montrerNom(b);
  else cacherNom();
});
barre.addEventListener("pointerleave", cacherNom);
barre.addEventListener("focusin", (e) => {
  const b = e.target.closest?.(".b");
  if (b?.matches(":focus-visible")) montrerNom(b);
  else cacherNom();
});
barre.addEventListener("focusout", (e) => {
  if (!e.relatedTarget || !barre.contains(e.relatedTarget)) cacherNom();
});

// --- Clavier et souris --------------------------------------------------------------

function boutonsBarre() {
  return Array.from(barre.querySelectorAll(".b"));
}

function deplacer(liste, pas) {
  if (!liste.length) return;
  const i = liste.indexOf(document.activeElement);
  const suivant = i < 0 ? (pas > 0 ? 0 : liste.length - 1) : (i + pas + liste.length) % liste.length;
  liste[suivant].focus();
}

document.addEventListener("keydown", (e) => {
  if (barre.hidden) return;
  if (ouvert) {
    const liste = elements();
    const pas = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[e.key];
    if (pas) {
      e.preventDefault();
      deplacer(liste, pas);
    } else if (e.key === "Home" || e.key === "End") {
      e.preventDefault();
      liste[e.key === "Home" ? 0 : liste.length - 1]?.focus();
    } else if (e.key === "Escape" || e.key === "Tab") {
      e.preventDefault();
      fermerEtage(true, true);
    }
    return;
  }
  const liste = boutonsBarre();
  const pas = { ArrowRight: 1, ArrowLeft: -1 }[e.key];
  if (pas) {
    e.preventDefault();
    deplacer(liste, pas);
  } else if (e.key === "Tab") {
    e.preventDefault();
    deplacer(liste, e.shiftKey ? -1 : 1);
  } else if (e.key === "Home" || e.key === "End") {
    e.preventDefault();
    liste[e.key === "Home" ? 0 : liste.length - 1]?.focus();
  } else if (e.key === "ArrowDown" && document.activeElement === boutonRetardateur) {
    e.preventDefault();
    ouvrirEtage(boutonRetardateur);
  } else if (e.key === "Escape") {
    e.preventDefault();
    appeler("barre_fermer").catch(() => {});
  }
});

document.addEventListener("pointerdown", (e) => {
  if (barre.hidden) return;
  // Un clic ailleurs que sur l'étage ou son bouton : l'étage se replie.
  const dansEtage = ouvert && (etage.contains(e.target) || ouvert.bouton.contains(e.target));
  if (ouvert && !dansEtage) fermerEtage(false, true);
  // La marge transparente autour de la carte : un clic ailleurs, la barre se ferme.
  if (!barre.contains(e.target)) appeler("barre_fermer").catch(() => {});
});
// Pas de menu du navigateur dans la barre.
document.addEventListener("contextmenu", (e) => e.preventDefault());

const DEMO = {
  boutons: [
    "capturer_zone",
    "capturer_plein_ecran",
    "capturer_fenetre",
    "capturer_defilante",
    "capturer_video",
    "capturer_gif",
    "reglages",
  ],
  verrouilles: [],
  retardateur_s: 3,
  retardateurs: [0, 3, 5, 10],
};

async function demarrer() {
  if (!tauri?.event?.listen) {
    montrer(DEMO);
    return;
  }
  await suivreTheme();
  await Promise.all([
    tauri.event.listen("barre:ouvrir", (e) => montrer(e?.payload)),
    tauri.event.listen("barre:vider", () => vider()),
  ]).catch((e) => console.warn("barre : écoute impossible :", e));
  direPret();
}

// Le cœur peut refuser les tout premiers appels (pas encore en place, vu sous Windows) :
// `barre_pret` est retenté toutes les 100 ms, 20 fois au plus.
const ESSAIS_PRET = 20;
function direPret(reste = ESSAIS_PRET) {
  new Promise((ok) => ok(tauri.core.invoke("barre_pret", {}))).then(montrer, (e) => {
    if (reste > 1) setTimeout(() => direPret(reste - 1), 100);
    else console.warn(`barre_pret : refusé ${ESSAIS_PRET} fois :`, e);
  });
}

demarrer();
