// Barre de capture (design/capture, `.launcher`, validé le 12/11/2026). La page montre et
// rapporte, le cœur décide.
//
// Contrat avec le cœur (application/fenetre_barre.rs) :
// - `barre_pret` une fois, quand l'écoute de `barre:ouvrir` et du thème est en place ; rend
//   la charge de la première ouverture (ou null).
// - écoute `barre:ouvrir` { boutons: [id], retardateur_s: n | null, retardateurs: [n] }, et
//   `barre:vider` (barre cachée : rien d'ancien à la prochaine ouverture).
// - `barre_taille` { largeur, hauteur } : la carte mesurée, menu volant ouvert compris,
//   après chaque ouverture et à chaque menu volant ouvert ou fermé.
// - `barre_choisi` { id } : un bouton (ou Vidéo, GIF de son menu) choisi.
// - `barre_retardateur` { secondes } : un choix du menu du retardateur ; rend la valeur en
//   vigueur, que montre le badge. La barre reste ouverte.
// - `barre_fermer` : Échap, ou un clic dans la marge transparente autour de la carte.
// Clavier : Tab, Maj + Tab, flèches gauche et droite, Début, Fin d'un bouton à l'autre (la
// première pression prend le premier ou le dernier) ; Flèche bas ouvre un menu volant ;
// Entrée, Espace ; Échap ferme le menu volant, sinon la barre.
// Sans window.__TAURI__ (aperçu dans un navigateur) : une barre de démonstration.
//
// Aucune donnée ne passe par innerHTML : textes par textContent, pictogrammes tirés de
// constantes. Les info-bulles sont le nom seul, sur les boutons sans libellé.

import { suivreTheme } from "../commun/theme.js";
import { picto } from "../commun/pictos.js";

const tauri = window.__TAURI__;
const barre = document.getElementById("barre");
const marque = barre.querySelector(".marque");
const voletFilm = document.getElementById("volet-film");
const voletRetardateur = document.getElementById("volet-retardateur");

// Les noms, ceux du design. Zone, Plein écran et Fenêtre ont leur libellé ; les autres, le
// pictogramme seul.
const NOMS = {
  capturer_zone: "Zone",
  capturer_plein_ecran: "Plein écran",
  capturer_fenetre: "Fenêtre",
  capturer_defilante: "Capture défilante",
  capturer_video: "Vidéo",
  capturer_gif: "GIF",
  reglages: "Réglages",
};
const A_LIBELLE = ["capturer_zone", "capturer_plein_ecran", "capturer_fenetre"];
const NOM_FILM = "Enregistrer l'écran";
const NOM_RETARDATEUR = "Retardateur";

function appeler(commande, args) {
  if (!tauri?.core?.invoke) return Promise.resolve(null);
  return Promise.resolve(tauri.core.invoke(commande, args)).catch((e) => {
    console.warn(`barre : ${commande} refusé :`, e);
    throw e;
  });
}

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

let ouvert = null; // menu volant ouvert : { volet, bouton }

function signalerTaille() {
  if (barre.hidden) return;
  const largeur = Math.ceil(barre.getBoundingClientRect().width);
  let hauteur = barre.offsetHeight;
  if (ouvert) hauteur = Math.max(hauteur, ouvert.volet.offsetTop + ouvert.volet.offsetHeight);
  appeler("barre_taille", { largeur, hauteur }).catch(() => {});
}

// --- Construction -------------------------------------------------------------------

function bouton(id, nom, { pictogramme = id, libelle = false } = {}) {
  const b = document.createElement("button");
  b.type = "button";
  b.className = "mode";
  b.dataset.id = id;
  b.append(picto(pictogramme));
  if (libelle) {
    const l = document.createElement("span");
    l.className = "libelle";
    l.textContent = nom;
    b.append(l);
  } else {
    b.classList.add("picto-seul");
    b.setAttribute("aria-label", nom);
    b.title = nom;
  }
  return b;
}

function separateur() {
  const s = document.createElement("span");
  s.className = "separateur";
  s.setAttribute("role", "separator");
  s.setAttribute("aria-orientation", "vertical");
  return s;
}

let boutonRetardateur = null;
let badge = null;

function poserRetardateur(secondes) {
  if (!boutonRetardateur || typeof secondes !== "number") return;
  const arme = secondes > 0;
  boutonRetardateur.classList.toggle("arme", arme);
  badge.textContent = arme ? String(secondes) : "";
  boutonRetardateur.setAttribute("aria-label", arme ? `${NOM_RETARDATEUR}, ${secondes} s` : NOM_RETARDATEUR);
  for (const c of voletRetardateur.querySelectorAll(".choix")) {
    c.setAttribute("aria-checked", Number(c.dataset.secondes) === secondes ? "true" : "false");
  }
}

function montrer(charge) {
  if (!charge || !Array.isArray(charge.boutons)) return;
  choisi = false;
  fermerVolet(false);
  const ids = charge.boutons.filter((id) => typeof id === "string" && Object.hasOwn(NOMS, id));
  const groupes = [[], [], []];

  for (const id of A_LIBELLE.filter((i) => ids.includes(i))) {
    const b = bouton(id, NOMS[id], { libelle: true });
    b.addEventListener("click", () => choisir(id));
    groupes[0].push(b);
  }
  if (ids.includes("capturer_defilante")) {
    const b = bouton("capturer_defilante", NOMS.capturer_defilante);
    b.addEventListener("click", () => choisir("capturer_defilante"));
    groupes[1].push(b);
  }
  const films = ["capturer_video", "capturer_gif"].filter((i) => ids.includes(i));
  voletFilm.replaceChildren();
  if (films.length) {
    const b = bouton("film", NOM_FILM, { pictogramme: "enregistrer_ecran" });
    b.setAttribute("aria-haspopup", "menu");
    b.setAttribute("aria-expanded", "false");
    b.setAttribute("aria-controls", voletFilm.id);
    b.addEventListener("click", () => basculerVolet(voletFilm, b));
    for (const id of films) {
      const e = document.createElement("button");
      e.type = "button";
      e.className = "element";
      e.setAttribute("role", "menuitem");
      e.tabIndex = -1;
      e.dataset.id = id;
      const l = document.createElement("span");
      l.textContent = NOMS[id];
      e.append(picto(id), l);
      e.addEventListener("click", () => choisir(id));
      voletFilm.append(e);
    }
    groupes[1].push(b);
  }
  boutonRetardateur = null;
  badge = null;
  voletRetardateur.replaceChildren();
  if (typeof charge.retardateur_s === "number") {
    const b = bouton("retardateur", NOM_RETARDATEUR, { pictogramme: "retardateur" });
    b.setAttribute("aria-haspopup", "true");
    b.setAttribute("aria-expanded", "false");
    b.setAttribute("aria-controls", voletRetardateur.id);
    badge = document.createElement("span");
    badge.className = "badge";
    badge.setAttribute("aria-hidden", "true");
    b.append(badge);
    b.addEventListener("click", () => basculerVolet(voletRetardateur, b));
    const choix = Array.isArray(charge.retardateurs) ? charge.retardateurs : [];
    for (const s of choix.filter((n) => Number.isInteger(n) && n >= 0)) {
      const c = document.createElement("button");
      c.type = "button";
      c.className = "choix";
      c.setAttribute("role", "radio");
      c.tabIndex = -1;
      c.dataset.secondes = String(s);
      c.textContent = s === 0 ? "Aucun" : `${s} s`;
      c.addEventListener("click", () => changerRetardateur(s));
      voletRetardateur.append(c);
    }
    boutonRetardateur = b;
    groupes[1].push(b);
  }
  if (ids.includes("reglages")) {
    const b = bouton("reglages", NOMS.reglages);
    b.addEventListener("click", () => choisir("reglages"));
    groupes[2].push(b);
  }

  const enfants = [marque];
  for (const g of groupes.filter((x) => x.length)) {
    if (enfants.length > 1) enfants.push(separateur());
    enfants.push(...g);
  }
  barre.replaceChildren(...enfants);
  poserRetardateur(charge.retardateur_s);
  barre.hidden = false;
  // Entrée : la carte descend de 4 px en fondu (167 ms).
  barre.classList.add("entree");
  void barre.offsetWidth;
  barre.classList.remove("entree");
  if (document.activeElement && document.activeElement !== document.body) document.activeElement.blur();
  signalerTaille();
}

function vider() {
  fermerVolet(false);
  barre.hidden = true;
  barre.replaceChildren(marque);
  voletFilm.replaceChildren();
  voletRetardateur.replaceChildren();
  boutonRetardateur = null;
  badge = null;
}

function changerRetardateur(secondes) {
  appeler("barre_retardateur", { secondes })
    .then((v) => poserRetardateur(typeof v === "number" ? v : secondes))
    .catch(() => {});
  fermerVolet(true);
}

// --- Menus volants ------------------------------------------------------------------

function elements(volet) {
  return Array.from(volet.querySelectorAll("button"));
}

function ouvrirVolet(volet, b) {
  fermerVolet(false);
  if (!elements(volet).length) return;
  volet.hidden = false;
  const rb = barre.getBoundingClientRect();
  const rbouton = b.getBoundingClientRect();
  const largeur = volet.offsetWidth;
  const centre = rbouton.left - rb.left + rbouton.width / 2;
  const gauche = Math.max(0, Math.min(centre - largeur / 2, barre.offsetWidth - largeur));
  volet.style.left = `${Math.round(gauche)}px`;
  volet.classList.remove("ouvre");
  void volet.offsetWidth;
  volet.classList.add("ouvre");
  b.setAttribute("aria-expanded", "true");
  ouvert = { volet, bouton: b };
  signalerTaille();
  (volet.querySelector('[aria-checked="true"]') || elements(volet)[0]).focus();
}

function fermerVolet(rendreFocus) {
  if (!ouvert) return;
  const { volet, bouton: b } = ouvert;
  ouvert = null;
  volet.hidden = true;
  volet.classList.remove("ouvre");
  b.setAttribute("aria-expanded", "false");
  if (rendreFocus) b.focus();
  signalerTaille();
}

function basculerVolet(volet, b) {
  if (ouvert?.volet === volet) fermerVolet(true);
  else ouvrirVolet(volet, b);
}

// --- Clavier et souris --------------------------------------------------------------

function boutonsBarre() {
  return Array.from(barre.querySelectorAll(".mode"));
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
    const liste = elements(ouvert.volet);
    const pas = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[e.key];
    if (pas) {
      e.preventDefault();
      deplacer(liste, pas);
    } else if (e.key === "Home" || e.key === "End") {
      e.preventDefault();
      liste[e.key === "Home" ? 0 : liste.length - 1]?.focus();
    } else if (e.key === "Escape" || e.key === "Tab") {
      e.preventDefault();
      fermerVolet(true);
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
  } else if (e.key === "ArrowDown" && document.activeElement?.hasAttribute("aria-haspopup")) {
    e.preventDefault();
    document.activeElement.click();
  } else if (e.key === "Escape") {
    e.preventDefault();
    appeler("barre_fermer").catch(() => {});
  }
});

document.addEventListener("pointerdown", (e) => {
  if (barre.hidden) return;
  const dansVolet = ouvert && (ouvert.volet.contains(e.target) || ouvert.bouton.contains(e.target));
  if (ouvert && !dansVolet) fermerVolet(false);
  // La marge transparente autour de la carte : un clic ailleurs, la barre se ferme.
  if (!barre.contains(e.target) && !dansVolet) appeler("barre_fermer").catch(() => {});
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
