// Fenêtre des Réglages (étape 10). La page montre et rapporte ; le cœur décide de tout
// (ARCHITECTURE.md §2.5) : chaque geste part au cœur, et la page affiche la vue qu'il
// renvoie (un interrupteur refusé revient à sa place parce que la vue le dit).
//
// Contrat avec le cœur (commandes permises à la seule fenêtre `reglages`) :
// - `reglages_lire` {} → Reponse, au chargement et au retour du focus (le cœur relit le
//   fichier) ;
// - `reglages_changer` { changement: { cle, valeur } } → Reponse ;
// - `reglages_apercu_nom` { modele } → { apercu } (rien n'est écrit) ;
// - `reglages_parcourir` {} → Reponse (sélecteur de dossier du système) ;
// - `reglages_saisie` { action, active } → Reponse (le cœur rend ses raccourcis au système
//   le temps de la saisie, sinon il les recevrait avant la page) ;
// - `reglages_raccourci` { action, saisie: { ctrl, maj, alt, win, cle, code } } → Reponse ;
// - `reglages_retablir` { action } → Reponse.
// Reponse = { vue, refus: { ligne, texte } | null }. Événements : `reglages:etat` (une vue),
// `interface:theme` (commun/theme.js).
//
// Les éléments ne sont trouvés que par data-*, rôles et identifiants, jamais par une
// classe : l'habillage (reglages.css) peut changer sans toucher à ce fichier. Aucune donnée
// ne passe par innerHTML.

import { poserTheme, suivreTheme } from "../commun/theme.js";

const tauri = window.__TAURI__;
const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));

const ATTENTE_NOM_MS = 400;
const THEMES = { systeme: null, clair: "light", sombre: "dark" };
const MODIFICATEURS = ["Control", "Shift", "Alt", "Meta", "AltGraph", "OS"];

let vue = null;
let saisie = null; // action en cours de saisie
let attenteNom = null; // minuterie de l'écriture du nom

function appeler(commande, args = {}) {
  if (!tauri?.core?.invoke) return Promise.resolve(null);
  return Promise.resolve(tauri.core.invoke(commande, args)).catch((e) => {
    console.warn(`${commande} :`, e);
    return null;
  });
}

// Une réponse du cœur : la vue d'abord, puis le refus sur sa ligne.
function recevoir(reponse) {
  if (!reponse || !reponse.vue) return;
  afficher(reponse.vue);
  const r = reponse.refus;
  if (r && r.texte) {
    const ligne = $(`[data-reglage="${CSS.escape(String(r.ligne))}"]`);
    const p = ligne && $("[data-refus]", ligne);
    if (p) {
      p.textContent = r.texte;
      p.hidden = false;
    }
  }
}

function poserInterrupteur(nom, oui) {
  const b = $(`[role="switch"][data-controle="${nom}"]`);
  if (b) b.setAttribute("aria-checked", oui ? "true" : "false");
}

function poserChoix(nom, valeur) {
  for (const b of $$(`[data-controle="${nom}"] [role="radio"]`)) {
    const oui = b.dataset.valeur === String(valeur);
    b.setAttribute("aria-checked", oui ? "true" : "false");
    b.tabIndex = oui ? 0 : -1;
  }
  // Aucun coché (valeur venue du fichier, hors de la liste) : le premier reste atteignable.
  const radios = $$(`[data-controle="${nom}"] [role="radio"]`);
  if (radios.length && !radios.some((b) => b.tabIndex === 0)) radios[0].tabIndex = 0;
}

function texteDuree(s) {
  return s === 0 ? "Toujours" : `${s} s`;
}

function texteRetardateur(s) {
  return s === 0 ? "Aucun" : `${s} s`;
}

// Segments rendus depuis une liste de secondes de la vue (durées, retardateurs).
function poserSegments(nom, valeurs, texte) {
  const groupe = $(`[data-controle="${nom}"]`);
  const voulues = valeurs.map(String).join(",");
  if (groupe.dataset.valeurs === voulues) return;
  groupe.dataset.valeurs = voulues;
  groupe.replaceChildren(
    ...valeurs.map((d) => {
      const b = document.createElement("button");
      b.setAttribute("role", "radio");
      b.dataset.valeur = String(d);
      b.textContent = texte(d);
      return b;
    }),
  );
}

function poserRaccourcis(liste) {
  const conteneur = $('[data-liste="raccourcis"]');
  const modele = $('template[data-modele="raccourci"]');
  for (const r of liste) {
    let ligne = $(`[data-reglage="raccourci"][data-action="${CSS.escape(r.action)}"]`, conteneur);
    if (!ligne) {
      ligne = modele.content.firstElementChild.cloneNode(true);
      ligne.dataset.action = r.action;
      const id = `l-raccourci-${r.action}`;
      $('[data-valeur="libelle"]', ligne).id = id;
      $('[data-controle="saisie"]', ligne).setAttribute("aria-labelledby", id);
      conteneur.append(ligne);
    }
    $('[data-valeur="libelle"]', ligne).textContent = r.libelle;
    $('[data-controle="retablir"]', ligne).hidden = !r.modifie;
    const refus = $("[data-refus]", ligne);
    refus.textContent = r.refus || "";
    refus.hidden = !r.refus;
    if (saisie !== r.action) montrerTouches(ligne, r.touches);
  }
}

function montrerTouches(ligne, touches) {
  const bouton = $('[data-controle="saisie"]', ligne);
  bouton.replaceChildren(
    ...touches.map((t) => {
      const k = document.createElement("kbd");
      k.textContent = t;
      return k;
    }),
  );
  bouton.setAttribute("aria-description", touches.join(" + "));
}

function afficher(v) {
  vue = v;
  poserTheme(THEMES[v.theme] ?? null);
  const montrees = new Set(v.lignes);
  for (const l of $$("[data-reglage]")) {
    if (l.dataset.reglage === "raccourci") continue;
    l.hidden = !montrees.has(l.dataset.reglage);
  }
  for (const p of $$("[data-refus]")) {
    if (p.closest('[data-reglage="raccourci"]')) continue;
    p.textContent = "";
    p.hidden = true;
  }
  // Une rubrique sans ligne montrée disparaît, onglet compris.
  for (const s of $$('section[data-rubrique]')) {
    const vide = s.dataset.rubrique !== "raccourcis" && !$$("[data-reglage]", s).some((l) => !l.hidden);
    $(`[role="tab"][data-rubrique="${s.dataset.rubrique}"]`).hidden = vide;
  }
  // L'onglet ouvert caché (Capture sans Gel) : le premier onglet visible s'ouvre.
  const ouvert = $('[role="tab"][aria-selected="true"]');
  if (!ouvert || ouvert.hidden) {
    const premier = $$('[role="tab"]').find((o) => !o.hidden);
    if (premier) montrerRubrique(premier.dataset.rubrique);
  }
  poserInterrupteur("pointeur", v.capture.pointeur);
  poserSegments("retardateur", v.retardateurs, texteRetardateur);
  poserChoix("retardateur", v.capture.retardateur_s);
  poserInterrupteur("copier", v.copier);
  poserInterrupteur("enregistrer", v.enregistrer);
  poserInterrupteur("vignette", v.vignette.afficher);
  poserInterrupteur("demarrage", v.demarrage === "oui");
  poserSegments("vignette_duree", v.durees, texteDuree);
  poserChoix("vignette_duree", v.vignette.duree_s);
  poserChoix("vignette_coin", v.vignette.coin);
  poserChoix("theme", v.theme);
  for (const nom of ["vignette_duree", "vignette_coin"]) {
    $(`[data-reglage="${nom}"]`).toggleAttribute("data-inactif", !v.vignette.afficher);
  }
  $('[data-valeur="dossier"]').textContent = v.dossier;
  const champ = $('[data-controle="modele_nom"]');
  if (document.activeElement !== champ && attenteNom === null) champ.value = v.modele_nom;
  $('[data-valeur="apercu_nom"]').textContent = v.apercu_nom;
  // « BreezyShot » et le numéro en deux morceaux (habillage) ; le texte reste
  // « BreezyShot <version> ».
  const nomApp = document.createElement("span");
  nomApp.textContent = "BreezyShot";
  const numero = document.createElement("span");
  numero.className = "numero";
  numero.textContent = v.version;
  $('[data-valeur="version"]').replaceChildren(nomApp, " ", numero);
  poserRaccourcis(v.raccourcis);
  const alerte = $("[data-alerte]");
  alerte.textContent = v.alerte || "";
  alerte.hidden = !v.alerte;
}

function changer(cle, valeur) {
  return appeler("reglages_changer", { changement: { cle, valeur } }).then(recevoir);
}

// Interrupteurs.
for (const b of $$('[role="switch"]')) {
  b.addEventListener("click", () => {
    if (!vue) return;
    const nom = b.dataset.controle;
    changer(nom === "vignette" ? "vignette_afficher" : nom, b.getAttribute("aria-checked") !== "true");
  });
}

// Groupes de choix : clic et flèches.
const CLES_CHOIX = {
  retardateur: "retardateur",
  vignette_duree: "vignette_duree",
  vignette_coin: "vignette_coin",
  theme: "theme",
};
const CHOIX_NOMBRES = ["retardateur", "vignette_duree"];
function valeurChoix(nom, b) {
  return CHOIX_NOMBRES.includes(nom) ? Number(b.dataset.valeur) : b.dataset.valeur;
}
for (const groupe of $$('[role="radiogroup"]')) {
  const nom = groupe.dataset.controle;
  groupe.addEventListener("click", (e) => {
    const b = e.target.closest('[role="radio"]');
    if (b && vue) changer(CLES_CHOIX[nom], valeurChoix(nom, b));
  });
  groupe.addEventListener("keydown", (e) => {
    const radios = $$('[role="radio"]', groupe);
    const i = radios.indexOf(document.activeElement);
    const pas = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[e.key];
    if (i < 0 || !pas || !vue) return;
    e.preventDefault();
    const b = radios[(i + pas + radios.length) % radios.length];
    changer(CLES_CHOIX[nom], valeurChoix(nom, b)).then(() => b.focus());
  });
}

// Dossier.
$('[data-controle="parcourir"]').addEventListener("click", () => appeler("reglages_parcourir").then(recevoir));

// Nom des fichiers : aperçu à chaque frappe, écriture après une pause et à la sortie du champ.
const champNom = $('[data-controle="modele_nom"]');
function ecrireNom() {
  if (attenteNom === null) return;
  clearTimeout(attenteNom);
  attenteNom = null;
  changer("modele_nom", champNom.value);
}
champNom.addEventListener("input", () => {
  appeler("reglages_apercu_nom", { modele: champNom.value }).then((r) => {
    if (r && typeof r.apercu === "string") $('[data-valeur="apercu_nom"]').textContent = r.apercu;
  });
  clearTimeout(attenteNom);
  attenteNom = setTimeout(ecrireNom, ATTENTE_NOM_MS);
});
champNom.addEventListener("blur", ecrireNom);
champNom.addEventListener("keydown", (e) => {
  if (e.key === "Enter") ecrireNom();
});

// Menu « Insérer » : Date, Heure.
const boutonInserer = $('[data-controle="inserer"]');
const menuInserer = $('[data-menu="inserer"]');
function ouvrirMenu(oui) {
  menuInserer.hidden = !oui;
  boutonInserer.setAttribute("aria-expanded", oui ? "true" : "false");
  if (oui) $('[role="menuitem"]', menuInserer).focus();
}
boutonInserer.addEventListener("click", (e) => {
  e.stopPropagation();
  ouvrirMenu(menuInserer.hidden);
});
menuInserer.addEventListener("click", (e) => {
  const b = e.target.closest("[data-jeton]");
  if (!b) return;
  e.stopPropagation();
  const debut = champNom.selectionStart ?? champNom.value.length;
  const fin = champNom.selectionEnd ?? debut;
  const avant = champNom.value.slice(0, debut);
  const jeton = (avant && !/\s$/.test(avant) ? " " : "") + b.dataset.jeton;
  champNom.value = avant + jeton + champNom.value.slice(fin);
  ouvrirMenu(false);
  champNom.focus();
  const p = (avant + jeton).length;
  champNom.setSelectionRange(p, p);
  champNom.dispatchEvent(new Event("input"));
});
menuInserer.addEventListener("keydown", (e) => {
  const items = $$('[role="menuitem"]', menuInserer);
  const i = items.indexOf(document.activeElement);
  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    items[(i + (e.key === "ArrowDown" ? 1 : -1) + items.length) % items.length].focus();
  } else if (e.key === "Escape") {
    e.preventDefault();
    e.stopPropagation();
    ouvrirMenu(false);
    boutonInserer.focus();
  }
});
document.addEventListener("click", (e) => {
  if (!menuInserer.hidden && !menuInserer.contains(e.target)) ouvrirMenu(false);
});

// Raccourcis : saisie au clavier.
const conteneurRaccourcis = $('[data-liste="raccourcis"]');
function ligneDe(action) {
  return $(`[data-reglage="raccourci"][data-action="${CSS.escape(action)}"]`, conteneurRaccourcis);
}
function commencerSaisie(action) {
  if (saisie) finirSaisie(true);
  saisie = action;
  const ligne = ligneDe(action);
  ligne.dataset.saisie = "oui";
  const bouton = $('[data-controle="saisie"]', ligne);
  bouton.setAttribute("aria-pressed", "true");
  bouton.textContent = "Appuyez…";
  appeler("reglages_saisie", { action, active: true }).then(recevoir);
}
// Fin sans touche retenue (Échap, Tab, clic ailleurs) : `annuler` prévient le cœur.
function finirSaisie(annuler) {
  if (!saisie) return;
  const action = saisie;
  saisie = null;
  const ligne = ligneDe(action);
  if (ligne) {
    delete ligne.dataset.saisie;
    $('[data-controle="saisie"]', ligne).setAttribute("aria-pressed", "false");
  }
  if (annuler) appeler("reglages_saisie", { action, active: false }).then(recevoir);
  else if (vue) afficher(vue);
}
// Une vue poussée par le cœur (`reglages:etat`). Si elle dit que la saisie est finie
// (filet des 15 s), la page sort de « Appuyez… » sans rappeler le cœur.
function etatPousse(v) {
  if (saisie && v.saisie_en_cours === false) finirSaisie(false);
  afficher(v);
}
function envoyerTouche(e) {
  const action = saisie;
  finirSaisie(false);
  const touche = { ctrl: e.ctrlKey, maj: e.shiftKey, alt: e.altKey, win: e.metaKey, cle: e.key, code: e.code };
  appeler("reglages_raccourci", { action, saisie: touche }).then(recevoir);
}
conteneurRaccourcis.addEventListener("click", (e) => {
  const ligne = e.target.closest('[data-reglage="raccourci"]');
  if (!ligne) return;
  const action = ligne.dataset.action;
  if (e.target.closest('[data-controle="retablir"]')) {
    finirSaisie(true);
    appeler("reglages_retablir", { action }).then(recevoir);
  } else if (e.target.closest('[data-controle="saisie"]')) {
    if (saisie === action) finirSaisie(true);
    else commencerSaisie(action);
  }
});
conteneurRaccourcis.addEventListener("focusout", (e) => {
  if (saisie && e.target.closest?.('[data-controle="saisie"]')) finirSaisie(true);
});
window.addEventListener(
  "keydown",
  (e) => {
    if (!saisie) return;
    if (e.key === "Tab") {
      finirSaisie(true);
      return;
    }
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") {
      finirSaisie(true);
      return;
    }
    if (MODIFICATEURS.includes(e.key)) {
      const tenus = [];
      if (e.ctrlKey) tenus.push("Ctrl");
      if (e.shiftKey) tenus.push("Maj");
      if (e.altKey) tenus.push("Alt");
      if (e.metaKey) tenus.push("Win");
      const bouton = $('[data-controle="saisie"]', ligneDe(saisie));
      if (tenus.length) {
        bouton.replaceChildren(
          ...tenus.map((t) => {
            const k = document.createElement("kbd");
            k.textContent = t;
            return k;
          }),
        );
      }
      return;
    }
    // Windows n'envoie pas d'appui pour Impr. écran, seulement le relâchement.
    if (e.code === "PrintScreen") return;
    envoyerTouche(e);
  },
  true,
);
window.addEventListener(
  "keyup",
  (e) => {
    if (saisie && e.code === "PrintScreen") {
      e.preventDefault();
      envoyerTouche(e);
    }
  },
  true,
);
window.addEventListener("blur", () => finirSaisie(true));

// Rubriques.
const onglets = $$('[role="tab"]');
function montrerRubrique(nom, focus) {
  if (saisie) finirSaisie(true);
  for (const o of onglets) {
    const oui = o.dataset.rubrique === nom;
    o.setAttribute("aria-selected", oui ? "true" : "false");
    o.tabIndex = oui ? 0 : -1;
    if (oui && focus) o.focus();
  }
  for (const s of $$('[role="tabpanel"]')) s.hidden = s.dataset.rubrique !== nom;
}
for (const o of onglets) o.addEventListener("click", () => montrerRubrique(o.dataset.rubrique));
$('[role="tablist"]').addEventListener("keydown", (e) => {
  const visibles = onglets.filter((o) => !o.hidden);
  const i = visibles.indexOf(document.activeElement);
  const pas = { ArrowDown: 1, ArrowRight: 1, ArrowUp: -1, ArrowLeft: -1 }[e.key];
  if (i < 0 || !pas) return;
  e.preventDefault();
  montrerRubrique(visibles[(i + pas + visibles.length) % visibles.length].dataset.rubrique, true);
});

// Pas de menu contextuel du navigateur.
addEventListener("contextmenu", (e) => e.preventDefault(), true);

// Branchement : Tauri si présent, sinon un aperçu fixe pour le navigateur.
if (tauri?.event?.listen) {
  Promise.all([tauri.event.listen("reglages:etat", (e) => e?.payload && etatPousse(e.payload)), suivreTheme()]).then(
    () => appeler("reglages_lire").then(recevoir),
    (e) => console.warn("reglages : écoute impossible :", e),
  );
  window.addEventListener("focus", () => {
    if (!saisie) appeler("reglages_lire").then(recevoir);
  });
} else {
  afficher({
    lignes: ["retardateur", "pointeur", "copier", "vignette", "vignette_duree",
      "vignette_coin", "dossier", "enregistrer", "modele_nom", "theme", "demarrage", "version"],
    capture: { retardateur_s: 0, pointeur: false },
    retardateurs: [0, 3, 5, 10],
    copier: true,
    enregistrer: false,
    vignette: { afficher: true, duree_s: 6, coin: "bas_gauche" },
    durees: [3, 6, 10, 0],
    dossier: "Images › BreezyShot",
    modele_nom: "BreezyShot {date} à {heure}",
    apercu_nom: "BreezyShot 2026-11-12 à 10.42.08.png",
    theme: "systeme",
    demarrage: "non",
    raccourcis: [
      { action: "zone", libelle: "Zone", touches: ["Ctrl", "Maj", "1"], modifie: false, refus: null },
      { action: "plein_ecran", libelle: "Plein écran", touches: ["Ctrl", "Maj", "2"], modifie: false, refus: null },
      { action: "fenetre", libelle: "Fenêtre", touches: ["Ctrl", "Maj", "3"], modifie: false, refus: null },
      {
        action: "derniere_zone",
        libelle: "Refaire la dernière zone",
        touches: ["Ctrl", "Maj", "4"],
        modifie: false,
        refus: null,
      },
    ],
    version: "0.0.0",
    alerte: null,
    saisie_en_cours: false,
  });
}
