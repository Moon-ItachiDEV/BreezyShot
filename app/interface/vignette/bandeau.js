// Bandeau de messages. La page montre et rapporte, le cœur décide (ARCHITECTURE.md §2.5, §6).
//
// Contrat avec le cœur :
// - écoute `bandeau:afficher` { id, genre, titre, detail, actions: [{ id, libelle }],
//   duree_ms }
//   genre : "reussite" | "info" | "erreur". Un nouveau message remplace l'ancien tout de suite ;
//   l'ancien ne reçoit plus rien (ni bandeau_termine ni bandeau_action).
//   Message invalide (id absent, genre inconnu, titre vide ou non texte) : console.warn, rien
//   n'est affiché.
// - `bandeau_action` { id, action } au clic d'une action (son id) ou de la croix ("fermer"),
//   une seule fois par message.
// - `bandeau_zone` { id, x, y, largeur, hauteur } (pixels CSS, relatifs à la page) dès qu'un
//   message à croix est affiché et à chaque changement de taille.
// - `bandeau_termine` { id } quand le message a fini de disparaître.
// - `bandeau_pret` {} une fois, quand l'écoute de `bandeau:afficher` (et celle du thème,
//   `interface:theme`, voir commun/theme.js) est en place.
// - `bandeau:decompte` et `bandeau:decompte_cache` : le décompte du retardateur
//   (decompte.js). Il partage la fenêtre : une seule chose à la fois. Un message efface le
//   disque ; un décompte efface la pilule sans `bandeau_termine`.
// - `bandeau:message_cache` { id } : le cœur a caché le message `id` (avant une capture) ;
//   la pilule est effacée tout de suite, sans `bandeau_termine`. Un autre `id` : rien.
// Réussite : part seule après --duree-bandeau-reussite. Info et erreur : restent jusqu'au clic,
// sauf avec `duree_ms` (entier > 0, 0.5 : « Aucun texte trouvé ») : elles gardent leur croix
// et leurs actions, et partent seules après `duree_ms` (puis `bandeau_termine`).
// Sans window.__TAURI__ (aperçu dans un navigateur) : ?demo=reussite|info|erreur
// (et &tenir pour qu'une réussite ne parte pas), ou ?demo=decompte.
//
// Aucune donnée du message ne passe par innerHTML : textes par textContent, pictogrammes
// tirés de constantes.

import { suivreTheme } from "../commun/theme.js";
import { creerDecompte } from "./decompte.js";

const tauri = window.__TAURI__;
const el = document.getElementById("bandeau");
const pastille = el.querySelector(".pastille");
const titre = el.querySelector(".titre");
const detail = el.querySelector(".detail");
const boutons = el.querySelector(".boutons");
const reduit = matchMedia("(prefers-reduced-motion: reduce)");
const GENRES = ["reussite", "info", "erreur"];

// Pictogrammes : constantes seulement.
const PICTOS = {
  reussite: "M2.5 6.4 5 8.8 9.6 3.6",
  erreur: "M6 2.8v3.8M6 8.9v.1",
  info: "M6 5.4v3.8M6 3.1v.1",
  croix: "M3 3l6 6M9 3l-6 6",
};
function picto(nom) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 12 12");
  svg.setAttribute("aria-hidden", "true");
  const chemin = document.createElementNS("http://www.w3.org/2000/svg", "path");
  chemin.setAttribute("d", PICTOS[nom]);
  svg.append(chemin);
  return svg;
}

// Le message affiché : { id, genre, minuterie, animation, ferme, zoneEnvoyee, tenir }
let courant = null;

function appeler(commande, args) {
  if (!tauri?.core?.invoke) return;
  try {
    Promise.resolve(tauri.core.invoke(commande, args)).catch((e) => console.warn(`${commande} :`, e));
  } catch (e) {
    console.warn(`${commande} :`, e);
  }
}

const jeton = (nom) => getComputedStyle(document.documentElement).getPropertyValue(nom).trim();
/** Lit une durée de jetons.css (« 300ms », « 2.5s ») en millisecondes. */
function duree(nom) {
  const v = jeton(nom);
  const n = parseFloat(v);
  if (!Number.isFinite(n)) return 0;
  return v.endsWith("ms") || !v.endsWith("s") ? n : n * 1000;
}

/** Vérifie et nettoie un message reçu ; null s'il doit être rejeté. */
function valider(m) {
  const rejeter = (raison) => {
    console.warn(`bandeau : message rejeté (${raison})`);
    return null;
  };
  if (!m || typeof m !== "object") return rejeter("pas un objet");
  const idValide = typeof m.id === "string" || (typeof m.id === "number" && Number.isFinite(m.id));
  if (!idValide) return rejeter("id absent");
  if (!GENRES.includes(m.genre)) return rejeter("genre inconnu");
  if (typeof m.titre !== "string" || m.titre.trim() === "") return rejeter("titre vide");
  const actions = [];
  // Seule l'erreur porte des actions ; l'info n'a que la croix.
  if (m.genre === "erreur" && Array.isArray(m.actions)) {
    for (const a of m.actions) {
      if (!a || typeof a.id !== "string" || a.id === "" || typeof a.libelle !== "string" || a.libelle.trim() === "") {
        console.warn("bandeau : action rejetée (id ou libellé non textuel)");
      } else if (a.id === "fermer" || actions.some((b) => b.id === a.id)) {
        console.warn(`bandeau : action « ${a.id} » ignorée (réservée ou en double)`);
      } else {
        actions.push({ id: a.id, libelle: a.libelle });
      }
    }
  }
  return {
    id: m.id,
    genre: m.genre,
    titre: m.titre,
    detail: m.genre !== "reussite" && typeof m.detail === "string" ? m.detail : "",
    actions,
    tenir: m.tenir === true,
    // Absent, null ou invalide : le message reste jusqu'au clic (règle de la 0.4).
    dureeMs:
      m.genre !== "reussite" && Number.isSafeInteger(m.duree_ms) && m.duree_ms > 0
        ? m.duree_ms
        : 0,
  };
}

function envoyerZone() {
  if (!courant || courant.ferme || courant.genre === "reussite" || el.hidden) return;
  // Rectangle de mise en page, sans la translation de l'animation d'entrée.
  const r = el.getBoundingClientRect();
  const t = new DOMMatrixReadOnly(getComputedStyle(el).transform === "none" ? undefined : getComputedStyle(el).transform);
  const zone = { id: courant.id, x: r.x - t.m41, y: r.y - t.m42, largeur: r.width, hauteur: r.height };
  const cle = JSON.stringify(zone);
  if (cle === courant.zoneEnvoyee) return;
  courant.zoneEnvoyee = cle;
  appeler("bandeau_zone", zone);
}
new ResizeObserver(envoyerZone).observe(el);
addEventListener("resize", envoyerZone);

/** Coupe tout ce qui appartient au message courant (minuterie, animation). */
function arreter() {
  if (!courant) return;
  clearTimeout(courant.minuterie);
  const a = courant.animation;
  courant.animation = null;
  if (a) {
    a.onfinish = null;
    a.cancel();
  }
}

function creerBouton(classe, action, texte) {
  const b = document.createElement("button");
  b.type = "button";
  b.className = classe;
  b.tabIndex = -1;
  b.dataset.action = action;
  b.textContent = texte; // texte seulement
  b.title = texte;
  return b;
}

function creerCroix() {
  const b = document.createElement("button");
  b.type = "button";
  b.className = "xb";
  b.tabIndex = -1;
  b.dataset.action = "fermer";
  b.setAttribute("aria-label", "Fermer le message");
  b.append(picto("croix"));
  return b;
}

/** Retire la pilule tout de suite, sans `bandeau_termine` (un décompte la remplace). */
function effacerMessage() {
  arreter();
  courant = null;
  el.hidden = true;
  el.classList.remove("sortie");
}

const decompte = creerDecompte({ appeler, effacerMessage });

export function afficher(brut) {
  const message = valider(brut);
  if (!message) return;
  decompte.effacer();
  arreter();
  courant = { ...message, minuterie: 0, animation: null, ferme: false, zoneEnvoyee: null };
  const lui = courant;

  el.className = `bandeau ${lui.genre}${lui.genre === "reussite" ? "" : " large"}`;
  pastille.replaceChildren(picto(lui.genre));
  titre.textContent = lui.titre;
  titre.title = lui.titre;
  detail.textContent = lui.detail;
  detail.title = lui.detail;
  boutons.replaceChildren();
  if (lui.genre !== "reussite") {
    for (const a of lui.actions) boutons.append(creerBouton("act", a.id, a.libelle));
    boutons.append(creerCroix());
  }
  el.hidden = false;

  const entree = reduit.matches
    ? [{ opacity: 0 }, { opacity: 1 }]
    : [
        { opacity: 0, transform: `translateY(${jeton("--bandeau-entree-decalage")})` },
        { opacity: 1, transform: "none" },
      ];
  lui.animation = el.animate(entree, {
    duration: duree(reduit.matches ? "--duree-reduite" : "--duree-bandeau-entree"),
    easing: reduit.matches ? "linear" : jeton("--courbe-ressort"),
  });
  envoyerZone();

  if (lui.genre === "reussite" && !lui.tenir) {
    lui.minuterie = setTimeout(() => cacher(lui), duree("--duree-bandeau-reussite"));
  } else if (lui.dureeMs && !lui.tenir) {
    lui.minuterie = setTimeout(() => cacher(lui), lui.dureeMs);
  }
}

function cacher(message) {
  if (message !== courant || el.hidden) return;
  arreter();
  message.ferme = true;
  el.classList.add("sortie");
  const fin = el.animate([{ opacity: 1 }, { opacity: 0 }], {
    duration: duree(reduit.matches ? "--duree-reduite" : "--duree-sortie"),
    fill: "forwards",
  });
  message.animation = fin;
  fin.onfinish = () => {
    if (message !== courant) return; // remplacé pendant le fondu : il ne reçoit plus rien
    el.hidden = true;
    el.classList.remove("sortie");
    message.animation = null;
    fin.cancel();
    courant = null;
    appeler("bandeau_termine", { id: message.id });
  };
}

function agir(bouton) {
  const lui = courant;
  if (!lui || lui.ferme || !el.contains(bouton)) return;
  lui.ferme = true; // les clics suivants sont ignorés
  appeler("bandeau_action", { id: lui.id, action: bouton.dataset.action });
  cacher(lui);
}

// Les boutons marchent à la souris sans que la fenêtre ait le focus : on agit quand le
// bouton gauche est relâché sur le bouton même où il a été appuyé, sans attendre « click ».
// Un appui commencé ailleurs ne déclenche jamais un bouton ; tout relâchement ou annulation
// remet l'appui à zéro.
let appuye = null;
const boutonSous = (e) => (e.target instanceof Element ? e.target.closest("#bandeau button") : null);
addEventListener(
  "pointerdown",
  (e) => {
    appuye = e.button === 0 ? boutonSous(e) : null;
    if (el.contains(e.target)) e.preventDefault(); // ne prend pas le focus
  },
  true,
);
addEventListener(
  "pointerup",
  (e) => {
    const b = boutonSous(e);
    const avant = appuye;
    appuye = null;
    if (e.button === 0 && b && b === avant) agir(b);
  },
  true,
);
addEventListener("pointercancel", () => (appuye = null), true);
addEventListener("blur", () => (appuye = null));
el.addEventListener("click", (e) => {
  // Clic sans pointeur (lecteur d'écran) : rien avant, on agit ici.
  if (e.detail === 0) {
    const b = boutonSous(e);
    if (b) agir(b);
  }
});
// Pas de menu contextuel, nulle part.
addEventListener("contextmenu", (e) => e.preventDefault(), true);

// Branchement : Tauri si présent, sinon aperçu.
if (tauri?.event?.listen) {
  // Une fois l'écoute en place, le cœur peut envoyer le message qui attendait (démarrage).
  Promise.all([
    tauri.event.listen("bandeau:afficher", (evenement) => afficher(evenement?.payload)),
    tauri.event.listen("bandeau:decompte", (evenement) => decompte.montrer(evenement?.payload)),
    tauri.event.listen("bandeau:decompte_cache", (e) => decompte.cacher(e?.payload)),
    tauri.event.listen("bandeau:message_cache", (e) => {
      if (courant && e?.payload?.id === courant.id) effacerMessage();
    }),
    suivreTheme(),
  ]).then(
    () => appeler("bandeau_pret", {}),
    (e) => console.warn("bandeau : écoute impossible :", e),
  );
} else {
  const params = new URLSearchParams(location.search);
  const EXEMPLES = {
    reussite: { genre: "reussite", titre: "Copié dans le presse-papiers" },
    info: {
      genre: "info",
      titre: "Impr. écran est prise. Raccourci : Ctrl + Maj + 1",
    },
    erreur: {
      genre: "erreur",
      titre: "Impossible d'enregistrer",
      detail: "Le dossier Images › BreezyShot est introuvable.",
      actions: [{ id: "choisir_dossier", libelle: "Choisir un dossier" }],
    },
  };
  const exemple = EXEMPLES[params.get("demo")];
  if (exemple) afficher({ id: "demo", tenir: params.has("tenir"), ...exemple });
  if (params.get("demo") === "decompte") {
    decompte.montrer({ id: "demo", secondes: 3, restant_ms: 3000, diametre: 88 });
  }
  // Pour les essais dans un navigateur seulement.
  window.bandeauApercu = { afficher, decompte };
}
