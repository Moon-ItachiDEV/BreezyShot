// Page de la fenêtre « Historique » (contrat 0.5, §2.2, §6.3 et §10) : entrée de la page.
// IPC avec le cœur, liste, aperçus en Blob, actions, pilule, menu « Plus d'options »,
// confirmation, glisser. La grille (grille.js) tient les cases et le clavier.
//
// Commandes (permises à la seule fenêtre `historique`) : `historique_pret` →
// ChargeHistorique { reduites } ; `historique_apercu` { nom } → octets PNG ;
// `historique_action` { nom, action } (copier | enregistrer | annoter | supprimer) ;
// `historique_annuler` ; `historique_tout_effacer` ; `historique_glisser` { nom } ;
// `historique_affiche` { n } (première rangée peinte, une fois) ; `historique_fermer`.
// Événements : `historique:liste` { entrees }, `historique:apercu` { nom },
// `historique:resultat` { nom, action, etat, message }, `historique:attente` { n },
// `historique:glisser` { nom, etat }, `interface:theme` (commun/theme.js).
// La page ne donne jamais un chemin : seulement le `nom` reçu dans la liste.

import { suivreTheme } from "../commun/theme.js";
import { installerGrille } from "./grille.js";

const tauri = window.__TAURI__;
const $ = (s) => document.querySelector(s);
const racine = document.documentElement;
const jetonNombre = (nom) => parseFloat(getComputedStyle(racine).getPropertyValue(`--${nom}`)) || 0;

function appeler(commande, args) {
  if (!tauri?.core?.invoke) return Promise.reject(new Error("pas de cœur"));
  return Promise.resolve(tauri.core.invoke(commande, args));
}
const signaler = (commande) => (e) => console.info(`${commande} :`, e);

const fenetre = $("#fenetre");
const corps = $("#corps");
const plus = $("#plus");
const menu = $("#menu");
const toutEffacer = $("#tout-effacer");
const confirmation = $("#confirmation");
const confNon = $("#confirmation-non");
const confOui = $("#confirmation-oui");
const pilule = $("#pilule");
const piluleTexte = $("#pilule-texte");
const piluleIc = $("#pilule-ic");
const piluleAnnuler = $("#pilule-annuler");

const reduites = () =>
  "reduites" in racine.dataset || matchMedia("(prefers-reduced-motion: reduce)").matches;

corps.tabIndex = -1;

const grille = installerGrille({
  grille: $("#grille"),
  vide: $("#vide"),
  reduites,
  agir,
  glisser: (nom) => appeler("historique_glisser", { nom }).catch(signaler("historique_glisser")),
  apercu: (nom) => apercus.get(nom)?.url ?? null,
  retirees: oublierApercus,
  focusVide: () => corps.focus({ preventScroll: true }),
});

function agir(nom, action) {
  if (action === "supprimer") grille.demander(nom);
  appeler("historique_action", { nom, action }).catch(signaler("historique_action"));
}

// ---------- aperçus : octets PNG du cœur → Blob → <img> ----------

// nom → { url, etat: "attente" | "pret" | "manquant", fin: Promise, redemander }
const apercus = new Map();
const PARALLELES = 4;
let file = [];
let enCours = 0;

function demanderApercus(noms) {
  for (const nom of noms) {
    if (apercus.has(nom)) continue;
    let finir;
    const fin = new Promise((ok) => (finir = ok));
    apercus.set(nom, { url: null, etat: "attente", fin, finir });
    file.push(nom);
  }
  pomper();
}

function pomper() {
  while (enCours < PARALLELES && file.length) {
    const nom = file.shift();
    const a = apercus.get(nom);
    if (!a) continue;
    enCours++;
    lireApercu(nom, a).finally(() => {
      enCours--;
      pomper();
    });
  }
}

async function lireApercu(nom, a) {
  // Un `historique:apercu` reçu pendant la demande la fera repartir si elle est refusée.
  a.redemander = false;
  try {
    const octets = await appeler("historique_apercu", { nom });
    if (apercus.get(nom) !== a) return;
    a.url = URL.createObjectURL(new Blob([octets], { type: "image/png" }));
    a.etat = "pret";
    const img = grille.image(nom);
    if (img) {
      img.src = a.url;
      await img.decode().catch(() => {});
    }
  } catch {
    // Pas encore fait (capture de la 0.4, échec) : la case attend sur son fond ;
    // `historique:apercu` le redemandera.
    if (apercus.get(nom) === a) {
      a.etat = a.redemander ? "attente" : "manquant";
      if (a.redemander) file.unshift(nom);
    }
  } finally {
    a.finir();
  }
}

function apercuPret(nom) {
  const a = apercus.get(nom);
  if (!a || a.etat === "pret") return;
  if (a.etat === "attente") a.redemander = true;
  else if (a.etat === "manquant") {
    a.etat = "attente";
    file.unshift(nom);
    pomper();
  }
}

function oublierApercus(noms) {
  for (const nom of noms) {
    const a = apercus.get(nom);
    if (a?.url) URL.revokeObjectURL(a.url);
    // Une case partie n'est plus attendue par `historique_affiche`.
    a?.finir();
    apercus.delete(nom);
  }
  file = file.filter((n) => apercus.has(n));
}

// ---------- liste ----------

let affiche = false;

function recevoirListe(entrees) {
  if (!Array.isArray(entrees)) return;
  grille.afficher(entrees);
  demanderApercus(entrees.map((e) => e.nom));
  plus.disabled = entrees.length === 0;
  if (!entrees.length) {
    fermerMenus(false);
    // « Effacer » rend le focus à « Plus d'options », grisé à présent : il va au corps.
    const f = document.activeElement;
    if (!f || f === document.body || f === plus) corps.focus({ preventScroll: true });
  }
  if (!affiche) {
    affiche = true;
    annoncerAffiche(entrees);
  }
}

// La première rangée est peinte (ses aperçus prêts, ou manquants) : une fois, pour le
// journal (`historique affiche <n> en <ms> ms`, budget de 700 ms).
async function annoncerAffiche(entrees) {
  const n = Math.min(entrees.length, grille.colonnes());
  await Promise.all(entrees.slice(0, n).map((e) => apercus.get(e.nom)?.fin));
  await new Promise((ok) => requestAnimationFrame(() => requestAnimationFrame(ok)));
  appeler("historique_affiche", { n }).catch(signaler("historique_affiche"));
}

// ---------- pilule de la page (écart U2) ----------

const COCHE = '<svg viewBox="0 0 12 12"><path d="M2.6 6.2l2.2 2.2 4.6-4.8"/></svg>';
const ALERTE = '<svg viewBox="0 0 12 12"><path d="M6 3v3.6M6 8.8v.1"/></svg>';
let attente = 0;
let erreurMinuterie = null;

function montrerPilule(genre, texte, annuler) {
  pilule.classList.toggle("err", genre === "erreur");
  pilule.setAttribute("role", genre === "erreur" ? "alert" : "status");
  piluleIc.innerHTML = genre === "erreur" ? ALERTE : COCHE;
  piluleTexte.textContent = texte;
  piluleAnnuler.hidden = !annuler;
  pilule.classList.add("on");
}

function cacherPilule() {
  if (pilule.contains(document.activeElement)) grille.focusActive();
  pilule.classList.remove("on");
  piluleAnnuler.hidden = true;
}

function pilulePourAttente() {
  if (erreurMinuterie) return;
  if (attente > 0) {
    montrerPilule("info", attente > 1 ? `${attente} supprimées` : "Supprimée", true);
  } else cacherPilule();
}

function recevoirAttente(n) {
  attente = Math.max(0, Number(n) || 0);
  pilulePourAttente();
}

function erreur(message) {
  clearTimeout(erreurMinuterie);
  montrerPilule("erreur", message, false);
  erreurMinuterie = setTimeout(() => {
    erreurMinuterie = null;
    if (attente > 0) pilulePourAttente();
    else cacherPilule();
  }, jetonNombre("duree-historique-erreur"));
}

function annuler() {
  if (attente > 0) appeler("historique_annuler", {}).catch(signaler("historique_annuler"));
}
piluleAnnuler.addEventListener("click", annuler);

function recevoirResultat(r) {
  if (!r || typeof r.nom !== "string") return;
  if (r.etat === "fait" || r.etat === "deja_fait") {
    if (r.action === "copier" || r.action === "enregistrer") grille.fait(r.nom, r.action);
  } else if (r.etat === "echec" && r.message) erreur(r.message);
}

// ---------- menu « Plus d'options » et confirmation ----------

function fermerMenus(rendreFocus) {
  const ouvert = !menu.hidden || !confirmation.hidden;
  menu.hidden = true;
  confirmation.hidden = true;
  plus.setAttribute("aria-expanded", "false");
  if (ouvert && rendreFocus) {
    if (!plus.disabled) plus.focus();
    else grille.focusActive();
  }
}

plus.addEventListener("click", (e) => {
  e.stopPropagation();
  const ouvrir = menu.hidden && confirmation.hidden;
  fermerMenus(false);
  if (!ouvrir) return;
  menu.hidden = false;
  plus.setAttribute("aria-expanded", "true");
  toutEffacer.focus();
});
toutEffacer.addEventListener("click", (e) => {
  e.stopPropagation();
  menu.hidden = true;
  confirmation.hidden = false;
  confNon.focus();
});
confNon.addEventListener("click", () => fermerMenus(true));
confOui.addEventListener("click", () => {
  fermerMenus(true);
  appeler("historique_tout_effacer", {}).catch(signaler("historique_tout_effacer"));
});
menu.addEventListener("keydown", (e) => {
  if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "Home" || e.key === "End") {
    e.preventDefault();
    toutEffacer.focus();
  }
});
for (const el of [menu, confirmation]) {
  el.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      fermerMenus(true);
    } else if (e.key === "Tab" && el === confirmation) {
      // Le focus reste dans la confirmation.
      e.preventDefault();
      (document.activeElement === confNon ? confOui : confNon).focus();
    } else if (e.key === "Tab") {
      fermerMenus(false);
    }
  });
}
document.addEventListener("pointerdown", (e) => {
  if (!e.target.closest(".menu, #plus")) fermerMenus(false);
});

// ---------- barre du haut ----------

$("#reduire").addEventListener("click", () => {
  const f = tauri?.window?.getCurrentWindow?.();
  if (f) Promise.resolve(f.minimize()).catch(signaler("réduire"));
});
$("#fermer").addEventListener("click", () =>
  appeler("historique_fermer", {}).catch(signaler("historique_fermer")),
);

// ---------- clavier et raccourcis du navigateur ----------

const NAVIGATEUR = new Set([
  "F1",
  "F3",
  "F5",
  "F6",
  "F7",
  "F10",
  "F11",
  "F12",
  "BrowserBack",
  "BrowserForward",
  "BrowserRefresh",
  "BrowserSearch",
  "BrowserHome",
]);
const NAVIGATEUR_CTRL = new Set("rpfugonjhlekwtisa+=-0".split(""));

window.addEventListener(
  "keydown",
  (e) => {
    const ctl = e.ctrlKey || e.metaKey;
    const k = e.key.length === 1 ? e.key.toLowerCase() : e.key;
    if (
      NAVIGATEUR.has(e.key) ||
      (ctl && NAVIGATEUR_CTRL.has(k)) ||
      (ctl && ["Digit0", "Equal", "Minus", "NumpadAdd", "NumpadSubtract"].includes(e.code)) ||
      (e.altKey && (e.key === "ArrowLeft" || e.key === "ArrowRight" || e.key === "Home"))
    ) {
      e.preventDefault();
    }
  },
  true,
);
window.addEventListener(
  "wheel",
  (e) => {
    if (e.ctrlKey) e.preventDefault();
  },
  { passive: false },
);
window.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("dragstart", (e) => e.preventDefault());
window.addEventListener("drop", (e) => e.preventDefault());
window.addEventListener("dragover", (e) => e.preventDefault());
for (const n of ["mouseup", "pointerup", "auxclick"]) {
  window.addEventListener(n, (e) => {
    if (e.button === 3 || e.button === 4) e.preventDefault();
  });
}

// Ctrl + Z : annuler les suppressions en attente (pas pendant le menu ni la confirmation).
document.addEventListener("keydown", (e) => {
  const ctl = e.ctrlKey || e.metaKey;
  // `e.key` et non `e.code` : la lettre tapée, quelle que soit la disposition (AZERTY).
  if (ctl && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "z") {
    e.preventDefault();
    if (menu.hidden && confirmation.hidden) annuler();
  }
});

// ---------- démarrage ----------

async function demarrer() {
  await suivreTheme();
  const ecoute = tauri?.event?.listen;
  if (ecoute) {
    await Promise.all([
      ecoute("historique:liste", (e) => recevoirListe(e?.payload?.entrees)),
      ecoute("historique:apercu", (e) => apercuPret(e?.payload?.nom)),
      ecoute("historique:resultat", (e) => recevoirResultat(e?.payload)),
      ecoute("historique:attente", (e) => recevoirAttente(e?.payload?.n)),
      ecoute("historique:glisser", (e) =>
        grille.glissee(e?.payload?.nom, e?.payload?.etat === "debut"),
      ),
    ]).catch(signaler("écoute"));
  }
  let charge = null;
  try {
    charge = await appeler("historique_pret", {});
  } catch (e) {
    console.info("historique_pret :", e);
  }
  if (charge?.reduites) racine.dataset.reduites = "";
  if (!reduites()) fenetre.classList.add("ouverture");
}
demarrer();
