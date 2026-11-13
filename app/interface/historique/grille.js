// La grille de l'historique (contrat 0.5, §2.2 et §10) : les cases, leur pilule d'actions,
// le clavier (une case active, tabindex mobile), le glisser au seuil et les mouvements
// (case qui part, case qui entre, les autres qui glissent : FLIP). La liste vient toujours
// entière du cœur (`historique:liste`) : la grille ne retire ni n'ajoute rien d'elle-même,
// elle montre la liste reçue et anime la différence. Les cases sont tenues par leur `nom`.

import { age, nomCase } from "./temps.js";

const $ = (s, r = document) => r.querySelector(s);
const style = () => getComputedStyle(document.documentElement);
const jetonNombre = (nom) => parseFloat(style().getPropertyValue(`--${nom}`)) || 0;
const jeton = (nom) => style().getPropertyValue(`--${nom}`).trim();

// Les boutons de la pilule, dans l'ordre (écart U1 : pas d'Épingler).
const BOUTONS = [
  ["copier", "Copier", "i-copy", true],
  ["enregistrer", "Enregistrer", "i-save", true],
  ["annoter", "Annoter", "i-pen", false],
  ["supprimer", "Supprimer", "i-trash", false],
];

/**
 * `o` : { grille, vide, agir(nom, action), glisser(nom), reduites() → bool,
 * apercu(nom) → url | null, retirees(noms), focusVide() }. Rend l'objet de la grille.
 */
export function installerGrille(o) {
  const { grille, vide } = o;
  const cases = new Map(); // nom → { el, entree }
  let ordre = []; // les noms montrés, dans l'ordre
  let active = null;
  let premiere = true;
  let generation = 0;
  // Noms dont l'utilisateur a demandé la suppression : leur départ est animé, leur retour
  // (Annuler) reprend le focus.
  const demandees = new Set();

  const animer = () => !o.reduites();

  function creerCase(entree) {
    const el = document.createElement("div");
    el.className = "tile";
    el.setAttribute("role", "listitem");
    el.dataset.nom = entree.nom;
    const pv = document.createElement("button");
    pv.type = "button";
    pv.className = "pv";
    pv.tabIndex = -1;
    const img = document.createElement("img");
    img.alt = "";
    img.draggable = false;
    const url = o.apercu?.(entree.nom);
    if (url) img.src = url;
    pv.append(img);
    const pastille = document.createElement("span");
    pastille.className = "ago";
    pastille.setAttribute("aria-hidden", "true");
    const qa = document.createElement("div");
    qa.className = "qa";
    for (const [action, libelle, picto, coche] of BOUTONS) {
      if (action === "annoter" && !entree.annotable) continue;
      const b = document.createElement("button");
      b.type = "button";
      b.className = "ib";
      b.tabIndex = -1;
      b.dataset.action = action;
      b.setAttribute("aria-label", libelle);
      b.title = libelle;
      b.innerHTML =
        `<svg class="ico" aria-hidden="true"><use href="#${picto}"/></svg>` +
        (coche ? `<svg class="ico ok" aria-hidden="true"><use href="#i-check"/></svg>` : "");
      b.addEventListener("click", (e) => {
        e.stopPropagation();
        o.agir(entree.nom, action);
      });
      qa.append(b);
    }
    el.append(pv, pastille, qa);
    const rafraichir = () => nommer(el, cases.get(entree.nom)?.entree || entree);
    el.addEventListener("mouseenter", rafraichir);
    pv.addEventListener("focus", () => {
      rafraichir();
      if (active !== entree.nom) rendreActive(entree.nom, false);
    });
    pv.addEventListener("dblclick", () => annoter(entree.nom));
    pv.addEventListener("pointerdown", (e) => appuyer(e, entree.nom));
    return el;
  }

  function nommer(el, entree) {
    const maintenant = Date.now();
    $(".pv", el).setAttribute("aria-label", nomCase(entree.horodatage_ms, maintenant));
    $(".ago", el).textContent = age(entree.horodatage_ms, maintenant);
  }

  function annoter(nom) {
    if (cases.get(nom)?.entree.annotable) o.agir(nom, "annoter");
  }

  // Une case active : elle seule (et sa pilule) est dans l'ordre de tabulation.
  function rendreActive(nom, focus) {
    active = nom;
    for (const [n, c] of cases) {
      const oui = n === nom;
      $(".pv", c.el).tabIndex = oui ? 0 : -1;
      for (const b of c.el.querySelectorAll(".qa .ib")) b.tabIndex = oui ? 0 : -1;
    }
    const c = cases.get(nom);
    if (focus && c) {
      $(".pv", c.el).focus({ preventScroll: true });
      c.el.scrollIntoView({ block: "nearest" });
    }
  }

  function colonnes() {
    const els = ordre.map((n) => cases.get(n)?.el).filter(Boolean);
    if (!els.length) return 0;
    let n = 1;
    while (n < els.length && els[n].offsetTop === els[0].offsetTop) n++;
    return n;
  }

  // ---------- liste ----------

  function afficher(entrees) {
    const ma = ++generation;
    const noms = new Set(entrees.map((e) => e.nom));
    const partantes = ordre.filter((n) => !noms.has(n) && demandees.has(n) && cases.has(n));
    if (animer() && partantes.length) {
      for (const n of partantes) cases.get(n).el.classList.add("partie");
      setTimeout(() => {
        if (ma === generation) appliquer(entrees);
      }, jetonNombre("duree-historique-sortie-case"));
    } else appliquer(entrees);
  }

  function appliquer(entrees) {
    const anime = animer() && !premiere;
    const avant = new Map();
    if (anime) for (const [n, c] of cases) avant.set(n, c.el.getBoundingClientRect());
    const focusDedans = grille.contains(document.activeElement);
    const indexActif = Math.max(0, ordre.indexOf(active));
    const noms = new Set(entrees.map((e) => e.nom));

    const retirees = [];
    for (const [n, c] of cases) {
      if (noms.has(n)) continue;
      c.el.remove();
      cases.delete(n);
      retirees.push(n);
    }
    let revenue = null;
    entrees.forEach((entree, i) => {
      let c = cases.get(entree.nom);
      if (!c) {
        c = { el: creerCase(entree), entree };
        cases.set(entree.nom, c);
        if (demandees.has(entree.nom) && revenue == null) revenue = entree.nom;
      }
      c.entree = entree;
      c.el.classList.remove("partie");
      nommer(c.el, entree);
      if (grille.children[i] !== c.el) grille.insertBefore(c.el, grille.children[i] || null);
    });
    for (const n of [...demandees]) if (noms.has(n)) demandees.delete(n);
    ordre = entrees.map((e) => e.nom);

    grille.hidden = !ordre.length;
    vide.hidden = ordre.length > 0;
    if (revenue != null) active = revenue;
    else if (!noms.has(active)) active = ordre[Math.min(indexActif, ordre.length - 1)] ?? null;
    const focusPerdu = focusDedans && !grille.contains(document.activeElement);
    rendreActive(active, (focusPerdu || (revenue != null && focusDedans)) && active != null);
    if (focusPerdu && active == null) o.focusVide?.();

    if (anime) {
      const ressort = jeton("courbe-ressort");
      for (const [n, c] of cases) {
        const a = avant.get(n);
        if (!a) {
          const echelle = jetonNombre("historique-entree-echelle");
          c.el.animate(
            [
              { opacity: 0, transform: `scale(${echelle})` },
              { opacity: 1, transform: "none" },
            ],
            { duration: jetonNombre("duree-historique-entree-case"), easing: ressort },
          );
          continue;
        }
        const b = c.el.getBoundingClientRect();
        const dx = a.left - b.left;
        const dy = a.top - b.top;
        if (dx || dy) {
          c.el.animate([{ transform: `translate(${dx}px, ${dy}px)` }, { transform: "none" }], {
            duration: jetonNombre("duree-historique-flip"),
            easing: ressort,
          });
        }
      }
    }
    premiere = false;
    if (retirees.length) o.retirees?.(retirees);
  }

  // ---------- clavier ----------

  grille.addEventListener("keydown", (e) => {
    const el = e.target.closest?.(".tile");
    if (!el) return;
    const nom = el.dataset.nom;
    const surCase = e.target.classList.contains("pv");
    const ctl = e.ctrlKey || e.metaKey;
    if (ctl && !e.shiftKey && !e.altKey && e.code === "KeyC") {
      e.preventDefault();
      liserer(nom);
      o.agir(nom, "copier");
      return;
    }
    if (e.key === "Delete" || e.key === "Backspace") {
      e.preventDefault();
      o.agir(nom, "supprimer");
      return;
    }
    if (!surCase || ctl || e.altKey) return;
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      annoter(nom);
      return;
    }
    const i = ordre.indexOf(nom);
    const cols = colonnes();
    const cible = {
      ArrowRight: i + 1,
      ArrowLeft: i - 1,
      ArrowDown: i + cols,
      ArrowUp: i - cols,
      Home: 0,
      End: ordre.length - 1,
    }[e.key];
    if (cible === undefined) return;
    e.preventDefault();
    const n = ordre[Math.max(0, Math.min(ordre.length - 1, cible))];
    rendreActive(n, true);
  });
  grille.addEventListener("click", (e) => {
    const el = e.target.closest(".tile");
    if (el && !e.target.closest(".qa")) rendreActive(el.dataset.nom, true);
  });

  // ---------- glisser (écart U3 : le Shell fait l'image, seuil de 4 px) ----------

  let appui = null;
  function appuyer(e, nom) {
    if (e.button !== 0) return;
    appui = { x: e.clientX, y: e.clientY, id: e.pointerId, nom };
  }
  window.addEventListener("pointermove", (e) => {
    if (!appui || e.pointerId !== appui.id) return;
    if (
      Math.hypot(e.clientX - appui.x, e.clientY - appui.y) < jetonNombre("historique-glisser-seuil")
    )
      return;
    const nom = appui.nom;
    appui = null;
    o.glisser(nom);
  });
  for (const n of ["pointerup", "pointercancel"]) {
    window.addEventListener(n, () => {
      appui = null;
    });
  }

  // ---------- retours ----------

  function relancer(el, classe) {
    el.classList.remove(classe);
    void el.offsetWidth;
    el.classList.add(classe);
  }
  function liserer(nom) {
    const c = cases.get(nom);
    if (c) relancer(c.el, "lisere");
  }
  function fait(nom, action) {
    const b = cases.get(nom)?.el.querySelector(`.qa [data-action="${action}"]`);
    if (!b) return;
    relancer(b, "fait");
    clearTimeout(b._coche);
    b._coche = setTimeout(() => b.classList.remove("fait"), jetonNombre("duree-historique-coche"));
  }

  // Rafraîchit les âges montrés (noms accessibles compris) : l'heure passe.
  setInterval(() => {
    for (const c of cases.values()) nommer(c.el, c.entree);
  }, 30000);

  return {
    afficher,
    demander: (nom) => demandees.add(nom),
    image: (nom) => cases.get(nom)?.el.querySelector(".pv img") || null,
    noms: () => [...ordre],
    colonnes,
    fait,
    glissee(nom, oui) {
      for (const [n, c] of cases) c.el.classList.toggle("glissee", oui && n === nom);
    },
    focusActive() {
      if (active != null) rendreActive(active, true);
    },
  };
}
