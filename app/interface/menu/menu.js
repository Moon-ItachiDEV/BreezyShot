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
import { picto } from "../commun/pictos.js";

const tauri = window.__TAURI__;
const menu = document.getElementById("menu");

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
    { id: "capturer_defilante", libelle: "Capture défilante", raccourci: "Ctrl + Maj + 5" },
    { id: "capturer_texte", libelle: "Copier le texte", raccourci: "Ctrl + Maj + T" },
    { separateur: true },
    { id: "capturer_video", libelle: "Vidéo", raccourci: "Ctrl + Maj + 6" },
    { id: "capturer_gif", libelle: "GIF", raccourci: "Ctrl + Maj + 7" },
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
