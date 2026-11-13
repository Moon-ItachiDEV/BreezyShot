// Menu de l'icône, piste A « Liste ». La page montre et rapporte, le cœur décide.
//
// Contrat avec le cœur (application/fenetre_menu.rs) :
// - `menu_pret` une fois, quand l'écoute de `menu:ouvrir` et du thème est en place ; rend
//   les lignes de la première ouverture (ou null).
// - écoute `menu:ouvrir` { lignes: [{ id, libelle, raccourci, separateur }] }, et
//   `menu:vider` (menu caché : la carte se vide, rien d'ancien à la prochaine ouverture).
// - `menu_choisi` { id } au clic (ou Entrée, Espace) sur une ligne.
// - `menu_fermer` sur Échap ou Tab.
// Flèches haut et bas (la première pression prend la première ou la dernière ligne),
// Début, Fin : d'une ligne à l'autre.
// Sans window.__TAURI__ (aperçu dans un navigateur) : un menu de démonstration.
//
// Aucune donnée ne passe par innerHTML : textes par textContent, pictogrammes tirés de
// constantes.

import { suivreTheme } from "../commun/theme.js";

const tauri = window.__TAURI__;
const menu = document.getElementById("menu");

// Pictogrammes 20 × 20 (design/menu-icone), d'après l'identifiant de la ligne.
const PICTOS = {
  capturer_zone: [
    "M3 7V4.5A1.5 1.5 0 0 1 4.5 3H7M13 3h2.5A1.5 1.5 0 0 1 17 4.5V7M17 13v2.5a1.5 1.5 0 0 1-1.5 1.5H13M7 17H4.5A1.5 1.5 0 0 1 3 15.5V13",
    "M10 7.5v5M7.5 10h5",
  ],
  capturer_plein_ecran: ["rect 2.5 3.5 15 10 1.5", "M7 16.5h6M10 13.5v3"],
  capturer_fenetre: ["rect 2.5 4 15 12 1.5", "M2.5 7.5h15"],
  capturer_defilante: [
    "rect 4 2.5 12 8.5 1.5",
    "M4 13.5v1.5a1.5 1.5 0 0 0 1.5 1.5h9a1.5 1.5 0 0 0 1.5-1.5v-1.5",
    "M10 5.5v6.5M7.8 9.8 10 12l2.2-2.2",
  ],
  capturer_derniere_zone: ["rect 2.5 3.5 11 9 1.5 pointille", "M17.5 8v5.5a2 2 0 0 1-2 2H9", "M11 13.5 9 15.5l2 2"],
  capturer_texte: [
    "M3 6.5v-2A1.5 1.5 0 0 1 4.5 3h2M13.5 3h2A1.5 1.5 0 0 1 17 4.5v2M17 13.5v2a1.5 1.5 0 0 1-1.5 1.5h-2M6.5 17h-2A1.5 1.5 0 0 1 3 15.5v-2",
    "M7 7h6M10 7v6.5",
  ],
  historique: ["M3.2 10a6.8 6.8 0 1 0 2-4.8", "M3.2 3v3.4h3.4", "M10 6.5V10l2.5 1.6"],
  // Icône « Curseurs » (choix du 13/11/2026) : jamais d'engrenage.
  reglages: ["M3 6.5h5.5M13.5 6.5H17M3 13.5h2.5M10.5 13.5H17", "circle 11 6.5 2.2", "circle 8 13.5 2.2"],
  quitter: ["M10 3v6.5", "M6.2 5.4a6 6 0 1 0 7.6 0"],
};

const SVG = "http://www.w3.org/2000/svg";
function picto(id) {
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("viewBox", "0 0 20 20");
  svg.setAttribute("aria-hidden", "true");
  for (const forme of PICTOS[id] || []) {
    const [genre, ...v] = forme.split(" ");
    let el;
    if (genre === "rect") {
      el = document.createElementNS(SVG, "rect");
      const [x, y, l, h, r, style] = v;
      el.setAttribute("x", x);
      el.setAttribute("y", y);
      el.setAttribute("width", l);
      el.setAttribute("height", h);
      el.setAttribute("rx", r);
      if (style === "pointille") el.setAttribute("stroke-dasharray", "2 1.8");
    } else if (genre === "circle") {
      el = document.createElementNS(SVG, "circle");
      el.setAttribute("cx", v[0]);
      el.setAttribute("cy", v[1]);
      el.setAttribute("r", v[2]);
    } else {
      el = document.createElementNS(SVG, "path");
      el.setAttribute("d", forme);
      // Le bas de la capture défilante : en pointillé.
      if (id === "capturer_defilante" && forme.startsWith("M4 13.5")) el.setAttribute("stroke-dasharray", "2 2");
    }
    svg.append(el);
  }
  return svg;
}

function appeler(commande, args) {
  if (!tauri?.core?.invoke) return Promise.resolve(null);
  return Promise.resolve(tauri.core.invoke(commande, args)).catch((e) => {
    console.warn(`menu : ${commande} refusé :`, e);
    throw e;
  });
}

let choisi = false;
function choisir(id) {
  if (choisi) return;
  choisi = true;
  // Refusé : la ligne peut être choisie de nouveau.
  appeler("menu_choisi", { id }).catch(() => {
    choisi = false;
  });
}

function lignes() {
  return Array.from(menu.querySelectorAll(".ligne"));
}

function montrer(charge) {
  if (!charge || !Array.isArray(charge.lignes)) return;
  choisi = false;
  const enfants = [];
  for (const l of charge.lignes) {
    if (l.separateur) {
      const s = document.createElement("div");
      s.className = "separateur";
      s.setAttribute("role", "separator");
      enfants.push(s);
      continue;
    }
    if (typeof l.id !== "string" || typeof l.libelle !== "string") continue;
    const b = document.createElement("button");
    b.type = "button";
    b.className = "ligne";
    b.setAttribute("role", "menuitem");
    b.tabIndex = -1;
    b.dataset.id = l.id;
    const libelle = document.createElement("span");
    libelle.className = "libelle";
    libelle.textContent = l.libelle;
    // Nom accessible : le libellé seul (le raccourci reste un texte à part, et
    // aria-keyshortcuts).
    libelle.id = `libelle-${enfants.length}`;
    b.setAttribute("aria-labelledby", libelle.id);
    b.append(picto(l.id), libelle);
    if (typeof l.raccourci === "string" && l.raccourci) {
      const k = document.createElement("span");
      k.className = "raccourci";
      k.textContent = l.raccourci;
      b.append(k);
      b.setAttribute("aria-keyshortcuts", l.raccourci.replace(/ \+ /g, "+").replace("Ctrl", "Control").replace("Maj", "Shift"));
    }
    b.addEventListener("click", () => choisir(l.id));
    enfants.push(b);
  }
  menu.replaceChildren(...enfants);
  // Entrée : la carte monte de 8 px en fondu (167 ms).
  menu.classList.add("entree");
  void menu.offsetWidth;
  menu.classList.remove("entree");
  if (document.activeElement && document.activeElement !== document.body) document.activeElement.blur();
}

function vider() {
  menu.replaceChildren();
}

document.addEventListener("keydown", (e) => {
  const tous = lignes();
  const i = tous.indexOf(document.activeElement);
  const pas = { ArrowDown: 1, ArrowUp: -1 }[e.key];
  if (pas) {
    e.preventDefault();
    if (!tous.length) return;
    const suivant = i < 0 ? (pas > 0 ? 0 : tous.length - 1) : (i + pas + tous.length) % tous.length;
    tous[suivant].focus();
  } else if (e.key === "Home" || e.key === "End") {
    e.preventDefault();
    tous[e.key === "Home" ? 0 : tous.length - 1]?.focus();
  } else if (e.key === "Escape" || e.key === "Tab") {
    e.preventDefault();
    appeler("menu_fermer").catch(() => {});
  } else if ((e.key === "Enter" || e.key === " ") && i >= 0) {
    e.preventDefault();
    choisir(tous[i].dataset.id);
  }
});
// Pas de menu du navigateur dans le menu.
document.addEventListener("contextmenu", (e) => e.preventDefault());

const DEMO = {
  lignes: [
    { id: "capturer_zone", libelle: "Capturer une zone", raccourci: "Ctrl + Maj + 1" },
    { id: "capturer_plein_ecran", libelle: "Capturer l'écran", raccourci: "Ctrl + Maj + 2" },
    { id: "capturer_fenetre", libelle: "Capturer une fenêtre", raccourci: "Ctrl + Maj + 3" },
    { separateur: true },
    { id: "capturer_derniere_zone", libelle: "Refaire la dernière zone", raccourci: "Ctrl + Maj + 4" },
    { id: "capturer_texte", libelle: "Copier le texte", raccourci: "Ctrl + Maj + T" },
    { separateur: true },
    { id: "historique", libelle: "Historique", raccourci: "Ctrl + Maj + H" },
    { id: "reglages", libelle: "Réglages" },
    { separateur: true },
    { id: "quitter", libelle: "Quitter BreezyShot" },
  ],
};

async function demarrer() {
  if (!tauri?.event?.listen) {
    montrer(DEMO);
    return;
  }
  await suivreTheme();
  await Promise.all([
    tauri.event.listen("menu:ouvrir", (e) => montrer(e?.payload)),
    tauri.event.listen("menu:vider", () => vider()),
  ]).catch((e) => console.warn("menu : écoute impossible :", e));
  direPret();
}

// Le cœur peut refuser les tout premiers appels (pas encore en place, vu sous Windows) :
// `menu_pret` est retenté toutes les 100 ms, 20 fois au plus.
const ESSAIS_PRET = 20;
function direPret(reste = ESSAIS_PRET) {
  new Promise((ok) => ok(tauri.core.invoke("menu_pret", {}))).then(montrer, (e) => {
    if (reste > 1) setTimeout(() => direPret(reste - 1), 100);
    else console.warn(`menu_pret : refusé ${ESSAIS_PRET} fois :`, e);
  });
}

demarrer();
