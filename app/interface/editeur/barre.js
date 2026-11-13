// Barre du haut et palette de l'éditeur (avenant « Éditeur épuré », piste B « Fenêtre ») :
// instruments et emplacement de l'outil venu de « Ajouter », bulle « Réglages de l'outil »,
// pastilles de couleur, menus « Ajouter », « Plus », « Fermer » et de l'objet, vignette de
// prise, message, clavier. Textes posés en texte, jamais en HTML ; les instruments sont ceux
// de la maquette (constantes, sans texte de l'utilisateur).

import { jeton, jetonNombre, DEFAUTS } from "./editeur.js";

const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
const NS = "http://www.w3.org/2000/svg";

// Outils (avenant §2). Variante : [id, nom, picto, touche].
export const OUTILS = {
  fleche: {
    nom: "Flèche",
    touche: "A",
    variantes: [
      ["droite", "Droite", "arrow"],
      ["courbe", "Courbe", "arrowc"],
    ],
  },
  stylo: { nom: "Stylo", touche: "P" },
  surligneur: { nom: "Surligneur", touche: "H" },
  gomme: { nom: "Gomme", touche: "G" },
  texte: { nom: "Texte", touche: "T" },
  forme: {
    nom: "Forme",
    variantes: [
      ["rectangle", "Rectangle", "rect", "R"],
      ["ellipse", "Ellipse", "ell", "E"],
      ["ligne", "Ligne", "line", "L"],
    ],
  },
  numero: { nom: "Numéro", touche: "N" },
  flou: {
    nom: "Flou",
    variantes: [
      ["pixeliser", "Pixeliser", "pixel", "B"],
      ["flouter", "Flouter", "smooth"],
      ["bloc", "Bloc noir", "box"],
    ],
  },
  projecteur: {
    nom: "Projecteur",
    variantes: [
      ["rectangle", "Rectangle", "spot"],
      ["ovale", "Ovale", "spotE"],
    ],
  },
};
export const BASE = ["fleche", "stylo", "surligneur", "gomme"];
export const AJOUTS = ["texte", "forme", "numero", "flou", "projecteur"];

// Sorte de l'outil (variante comprise) → type d'objet créé.
export const TYPE_DE_SORTE = {
  droite: "fleche",
  courbe: "fleche",
  rectangle: "rectangle",
  ellipse: "ellipse",
  ligne: "ligne",
  crayon: "crayon",
  surligneur: "surligneur",
  texte: "texte",
  numero: "numero",
  pixeliser: "masque",
  flouter: "masque",
  bloc: "masque",
  projecteur: "projecteur",
};

export function sorteOutil(prefs) {
  const o = prefs.outil;
  if (o === "stylo") return "crayon";
  if (o === "fleche" || o === "forme" || o === "flou") return prefs.variantes[o];
  return o;
}
export function sorteObjet(o) {
  if (!o) return null;
  if (o.type === "fleche") return o.cx != null ? "courbe" : "droite";
  if (o.type === "masque") return o.sorte;
  return o.type;
}

const TRAITS = ["droite", "courbe", "rectangle", "ellipse", "ligne", "crayon", "surligneur"];
const AVEC_COULEUR = new Set([...TRAITS, "texte", "numero"]);
const AVEC_OPACITE = new Set([...AVEC_COULEUR].filter((s) => s !== "surligneur"));
export const TAILLES_TEXTE = [20, 28, 40];
// Type d'objet → l'outil dont il porte les réglages.
const OUTIL_DE_TYPE = {
  fleche: "fleche",
  crayon: "stylo",
  surligneur: "surligneur",
  texte: "texte",
  rectangle: "forme",
  ellipse: "forme",
  ligne: "forme",
  numero: "numero",
  masque: "flou",
};

// Réglage → champ de l'objet, et à quelles sortes d'objet il s'applique.
const CHAMPS = {
  couleur: ["couleur", (s) => AVEC_COULEUR.has(s)],
  epaisseur: ["epaisseur", (s) => TRAITS.includes(s)],
  tailleTexte: ["taille", (s) => s === "texte"],
  numero: ["rayon", (s) => s === "numero"],
  intensite: ["intensite", (s) => s === "pixeliser" || s === "flouter"],
  opacite: ["opacite", (s) => AVEC_OPACITE.has(s)],
  gomme: [null, () => false], // la taille de la gomme : une préférence, aucun objet
};

// Instruments dessinés de la maquette (instSVG), en 28 × 90 ; les couleurs par classes.
const BOUT = {
  stylo:
    '<path class="t" d="M8 28 L13 9 h2 L20 28Z"/>' +
    '<path d="M12.4 12 L14 5.5 L15.6 12Z" fill="currentColor"/>',
  fleche:
    '<path class="t" d="M11 28V15h6v13z"/><path d="M6.5 16.5 14 4l7.5 12.5z" fill="currentColor"/>',
  surligneur:
    '<path d="M7 28V17l14-7v18z" fill="currentColor" opacity=".9"/>' +
    '<path class="ombre-bout" d="M7 17l14-7v3L7 20z"/>',
  gomme:
    '<path class="gomme" d="M7 28V15a7 7 0 0 1 14 0v13z"/>' +
    '<path d="M7 26h14" stroke-opacity=".12"/>',
  texte:
    '<path class="t" d="M8.5 28 13 9.5h2L19.5 28z"/><path d="M14 9.5V20" stroke-opacity=".3"/>' +
    '<path d="M13 9.5 14 5l1 4.5z" fill="currentColor"/>',
  forme:
    '<path class="t" d="M7.5 28V17l3-5h7l3 5v11z"/>' +
    '<rect x="10.5" y="5" width="7" height="7" rx="1.2" fill="currentColor"/>',
  numero:
    '<path class="t" d="M8 28v-9q0-6 6-6t6 6v9z"/>' +
    '<ellipse cx="14" cy="11" rx="6" ry="2.6" fill="currentColor"/>',
  flou:
    '<path class="t" d="M8 28c0-9 2.6-17 6-21 3.4 4 6 12 6 21z"/>' +
    '<path d="M14 9c-1.4 3-2.2 9-2.2 19M14 9c1.4 3 2.2 9 2.2 19"' +
    ' stroke-opacity=".35" fill="none"/>',
  projecteur:
    '<path class="t" d="M8.5 28v-7l-1.5-9h14l-1.5 9v7z"/>' +
    '<ellipse class="lumiere" cx="14" cy="12" rx="7" ry="2.4"/>' +
    '<ellipse class="coeur" cx="14" cy="12" rx="3.5" ry="1.1"/>',
};
const GRAVURE = {
  texte: '<path class="em" d="M11 52h6M14 52v9"/>',
  forme: '<rect class="em" x="10.5" y="51" width="7" height="7" rx="1"/>',
  numero: '<path class="em" d="M12.6 52.5 14.4 51v10"/>',
  flou:
    '<path class="em" d="M10.5 51h2v2h-2zM15.5 51h2v2h-2zM13 53.5h2v2h-2z' +
    'M10.5 56h2v2h-2zM15.5 56h2v2h-2z"/>',
  projecteur: '<circle class="em" cx="14" cy="55" r="3.2"/>',
};
const SANS_BAGUE = new Set(["gomme", "flou", "projecteur"]);
function instrument(t) {
  const bague = SANS_BAGUE.has(t)
    ? ""
    : '<rect x="7" y="38" width="14" height="5" fill="currentColor"/>';
  const texte =
    `<svg xmlns="${NS}" class="inst" viewBox="0 0 28 90" aria-hidden="true">` +
    '<rect class="b" x="7" y="27" width="14" height="66" rx="2.5"/>' +
    `${bague}${BOUT[t]}${GRAVURE[t] || ""}</svg>`;
  const svg = new DOMParser().parseFromString(texte, "image/svg+xml").documentElement;
  return document.importNode(svg, true);
}

function picto(id, classe = "ico") {
  const svg = document.createElementNS(NS, "svg");
  svg.setAttribute("class", classe);
  svg.setAttribute("aria-hidden", "true");
  const use = document.createElementNS(NS, "use");
  use.setAttribute("href", `#i-${id}`);
  svg.appendChild(use);
  return svg;
}
function el(tag, classe, texte) {
  const e = document.createElement(tag);
  if (classe) e.className = classe;
  if (texte != null) e.textContent = texte;
  return e;
}
function nommer(b, nom) {
  b.setAttribute("aria-label", nom);
  b.title = nom;
}
const clamp = (v, a, b) => Math.max(a, Math.min(b, v));

export function installerBarre(ed) {
  const fenetre = $("#fenetre");
  const scene = $("#scene");
  const palette = $("#palette");
  const outilsEl = $("#outils");
  const reglages = $("#reglages-outil");
  const menuCouleur = $("#menu-couleur");
  const ajouter = $("#menu-ajouter");
  const plus = $("#menu-plus");
  const menuFermer = $("#menu-fermer");
  const menuObjet = $("#menu-objet");
  const bAjouter = $("#bouton-ajouter");
  const bPlus = $("#bouton-plus");
  const bCouleur = $("#bouton-couleur");
  const bFermer = $('[data-act="fermer"]');
  const bCopier = $('[data-act="copier"]');
  const autre = $("#autre-couleur");
  const vignette = $("#vignette-prise");
  const pops = [reglages, menuCouleur, ajouter, plus, menuFermer, menuObjet];
  let ouvreur = null;
  let reglant = false;

  const prefs = () => ed.prefs ?? DEFAUTS;
  const sorte = () => sorteOutil(prefs());
  // Préférences par outil (maquette : S.col, S.w, S.a).
  const couleurDe = (t = prefs().outil) => prefs().couleurs[t] ?? null;
  const rangDe = (t = prefs().outil) => prefs().tailles[t] ?? 1;
  const opaciteDe = (t = prefs().outil) => prefs().opacites[t] ?? 100;
  // Valeur de la préférence de l'outil pour un réglage (unités de l'objet sauf l'opacité).
  function prefDe(cle) {
    if (cle === "couleur") return couleurDe();
    if (cle === "opacite") return opaciteDe();
    if (cle === "tailleTexte") return TAILLES_TEXTE[rangDe()];
    return rangDe();
  }

  // Après un clic de souris (pas au clavier) sur un instrument ou une pastille, le focus
  // revient à la scène : Espace tenu y déplace la vue.
  const rendreFocus = (e) => {
    if (e.detail > 0) scene.focus({ preventScroll: true });
  };

  // ---------- instruments ----------
  for (const t of [...BASE, ""]) {
    const b = el("button", "tl");
    b.type = "button";
    b.dataset.place = t || "emplacement";
    outilsEl.appendChild(b);
    b.addEventListener("click", (e) => {
      const id = b.dataset.outil;
      const encore = id === prefs().outil && !ouvert(reglages);
      fermerMenus();
      choisirOutil(id);
      rendreFocus(e);
      if (encore) ouvrirReglages(b);
    });
  }
  const emplacement = $('[data-place="emplacement"]', outilsEl);
  function poserInstrument(b, t) {
    if (b.dataset.outil === t) return false;
    b.dataset.outil = t;
    nommer(b, OUTILS[t].nom);
    b.replaceChildren(instrument(t));
    return true;
  }
  for (const t of BASE) poserInstrument($(`[data-place="${t}"]`, outilsEl), t);

  function synchroniserOutils() {
    const p = prefs();
    const place = p.emplacement;
    emplacement.hidden = !OUTILS[place];
    if (OUTILS[place] && poserInstrument(emplacement, place) && ed.affiche) {
      emplacement.classList.remove("in");
      void emplacement.offsetWidth;
      emplacement.classList.add("in");
      outilsEl.scrollLeft = outilsEl.scrollWidth;
    }
    for (const b of $$(".tl", outilsEl)) {
      const t = b.dataset.outil;
      b.setAttribute("aria-pressed", t === p.outil ? "true" : "false");
      const c = couleurDe(t);
      if (c) b.style.setProperty("--c", c);
      else b.style.removeProperty("--c");
    }
  }

  function choisirOutil(id, variante) {
    if (!OUTILS[id]) return;
    ed.outils?.validerSaisie();
    const p = prefs();
    if (variante) p.variantes[id] = variante;
    p.outil = id;
    if (AJOUTS.includes(id)) p.emplacement = id;
    if (id === "stylo" || id === "surligneur" || id === "gomme") ed.selection = [];
    synchroniser();
    ed.outils?.outilChange();
    ed.dessiner();
  }

  // ---------- couleurs ----------
  function pastilles(hote) {
    const liste = ed.dessin.COULEURS.map(([c, nom]) => [c, nom]);
    liste.splice(3, 0, ["", "Autre couleur"]);
    for (const [c, nom] of liste) {
      const b = el("button", c ? "dot" : "dot wheel");
      b.type = "button";
      b.dataset.couleur = c;
      nommer(b, nom);
      if (c) b.style.setProperty("--c", c);
      else b.appendChild(el("i"));
      b.addEventListener("click", (e) => {
        if (!c) {
          choisirAutre();
          return;
        }
        regler("couleur", c);
        if (!b.closest(".pop")) rendreFocus(e);
      });
      hote.appendChild(b);
    }
  }
  pastilles($("#couleurs"));
  pastilles($("#couleurs-menu"));
  function choisirAutre() {
    const c = prefs().autre || ed.dessin.COULEURS[0][0];
    autre.value = c.toLowerCase();
    try {
      if (autre.showPicker) autre.showPicker();
      else autre.click();
    } catch {
      autre.click();
    }
  }
  let marqueAutre = 0;
  autre.addEventListener("input", () => {
    prefs().autre = autre.value.toUpperCase();
    regler("couleur", prefs().autre, `autre-${marqueAutre}`);
  });
  autre.addEventListener("change", () => {
    marqueAutre += 1;
  });

  // Couleur montrée : celle de l'objet choisi s'il en a une, sinon celle de l'outil.
  function couleurCourante() {
    const o = ed.objetsChoisis().find((x) => x.couleur);
    return o ? o.couleur : couleurDe();
  }
  function synchroniserCouleurs() {
    const c = couleurCourante();
    const cu = c ? c.toUpperCase() : null;
    const libre = prefs().autre;
    const connue = ed.dessin.COULEURS.some((x) => x[0] === cu);
    for (const b of $$(".dot[data-couleur]")) {
      const d = b.dataset.couleur;
      b.disabled = !c;
      if (d) b.setAttribute("aria-pressed", d === cu ? "true" : "false");
      else {
        b.classList.toggle("has", !!libre);
        if (libre) $("i", b).style.setProperty("--c", libre);
        b.setAttribute("aria-pressed", libre && cu === libre && !connue ? "true" : "false");
      }
    }
    $("#pastille").style.setProperty("--c", c || "transparent");
    bCouleur.classList.toggle("none", !c);
    bCouleur.disabled = !c;
    if (!c && ouvert(menuCouleur)) fermerMenus();
  }

  // Un réglage change : les objets choisis à qui il s'applique (un pas d'annulation ;
  // `marque` : une glissière tirée = un pas), et la préférence de LEUR outil : recolorer un
  // texte choisi ne change pas la couleur de la flèche en main. Sans objet à qui l'appliquer :
  // la préférence de l'outil en main.
  function regler(cle, valeur, marque = null) {
    const p = prefs();
    const [champ, sApplique] = CHAMPS[cle];
    const choisis = ed.objetsChoisis().filter((o) => sApplique(sorteObjet(o)));
    const cibles = choisis.map((o) => o.id);
    const outils = new Set(choisis.map((o) => OUTIL_DE_TYPE[o.type]).filter(Boolean));
    if (!outils.size) outils.add(p.outil);
    for (const t of outils) {
      if (cle === "couleur") {
        if (t in p.couleurs) p.couleurs[t] = valeur;
      } else if (cle === "opacite") {
        if (t in p.opacites) p.opacites[t] = valeur;
      } else p.tailles[t] = cle === "tailleTexte" ? TAILLES_TEXTE.indexOf(valeur) : valeur;
    }
    if (cibles.length) {
      const v =
        cle === "numero"
          ? ed.dessin.RAYONS_NUMERO[valeur]
          : cle === "opacite"
            ? valeur / 100
            : valeur;
      reglant = true;
      try {
        ed.modifier((doc) => {
          for (const o of doc.objets) if (cibles.includes(o.id)) o[champ] = v;
        }, marque);
      } finally {
        reglant = false;
      }
    }
    synchroniserOutils();
    synchroniserCouleurs();
  }

  // ---------- menus et bulles : un seul ouvert à la fois ----------
  const ouvert = (p) => p.classList.contains("ouvert");
  const menuOuvert = () => pops.some(ouvert);
  // Refermer le menu « Fermer » sans choisir = l'ancien « Continuer » (editeur_garder).
  function fermerMenus(garder = true) {
    const fermerEtait = ouvert(menuFermer);
    let focusDedans = false;
    for (const p of pops) {
      if (!ouvert(p)) continue;
      if (p.contains(document.activeElement)) focusDedans = true;
      p.classList.remove("ouvert");
    }
    for (const b of $$('[aria-expanded="true"]', fenetre)) b.setAttribute("aria-expanded", "false");
    if (focusDedans) (ouvreur?.isConnected ? ouvreur : scene).focus({ preventScroll: true });
    if (fermerEtait && garder) ed.garder();
  }
  function ouvrir(p, bouton) {
    ed.texte?.fermer();
    fermerMenus();
    ouvreur = bouton;
    p.classList.add("ouvert");
    if (bouton?.hasAttribute("aria-expanded")) bouton.setAttribute("aria-expanded", "true");
    placerPop(p, bouton);
    const premier = $("button, input", p);
    if (premier && document.querySelector(":focus-visible")) premier.focus({ preventScroll: true });
  }
  // Au-dessus de la palette, centré sur le bouton ; sous un bouton de la barre du haut.
  function placerPop(p, bouton) {
    const er = fenetre.getBoundingClientRect();
    const r = bouton.getBoundingClientRect();
    const l = p.offsetWidth;
    const h = p.offsetHeight;
    const bord = jetonNombre("editeur-pop-ecart-haut");
    let x = clamp(r.left - er.left + r.width / 2 - l / 2, bord, er.width - l - bord);
    let y;
    if (palette.contains(bouton)) {
      y = palette.getBoundingClientRect().top - er.top - h - jetonNombre("editeur-pop-ecart");
      p.style.setProperty("--oy", "100%");
    } else {
      if (bouton === bFermer) x = r.left - er.left;
      y = r.bottom - er.top + bord;
      p.style.setProperty("--oy", "0%");
    }
    p.style.setProperty("--ox", `${r.left - er.left + r.width / 2 - x}px`);
    p.style.left = `${x}px`;
    p.style.top = `${y}px`;
  }
  function basculer(p, bouton, avant) {
    if (ouvert(p)) {
      fermerMenus();
      return;
    }
    avant?.();
    ouvrir(p, bouton);
  }
  bAjouter.addEventListener("click", () => basculer(ajouter, bAjouter));
  bPlus.addEventListener("click", () => basculer(plus, bPlus));
  bCouleur.addEventListener("click", () => basculer(menuCouleur, bCouleur));
  for (const m of $$(".mi[data-outil]", ajouter)) {
    m.addEventListener("click", () => {
      fermerMenus();
      choisirOutil(m.dataset.outil);
      if (document.querySelector(":focus-visible")) emplacement.focus({ preventScroll: true });
    });
  }

  // ---------- bulle « Réglages de l'outil » ----------
  function option(nom, contenu, choisie, choisir) {
    const b = el("button", "opt");
    b.type = "button";
    nommer(b, nom);
    b.setAttribute("aria-pressed", choisie ? "true" : "false");
    b.appendChild(contenu);
    b.addEventListener("click", () => {
      choisir();
      for (const x of $$(".opt", b.parentElement)) {
        x.setAttribute("aria-pressed", x === b ? "true" : "false");
      }
    });
    return b;
  }
  // Valeur montrée : celle de l'objet choisi s'il en a une, sinon la préférence.
  function valeur(cle) {
    const [champ, sApplique] = CHAMPS[cle];
    const o = ed.objetsChoisis().find((x) => sApplique(sorteObjet(x)) && x[champ] !== undefined);
    if (o && sorteObjet(o) === sorte()) {
      if (cle === "numero") return Math.max(0, ed.dessin.RAYONS_NUMERO.indexOf(o[champ]));
      if (cle === "opacite") return Math.round(o[champ] * 100);
      if (cle === "tailleTexte") return o[champ];
      return o[champ];
    }
    return prefDe(cle);
  }
  let marqueGlissiere = 0;
  function construireReglages() {
    reglages.replaceChildren();
    const t = prefs().outil;
    const s = sorte();
    const def = OUTILS[t];
    if (def.variantes) {
      const r = el("div", "row");
      for (const [id, nom, ic] of def.variantes) {
        r.appendChild(
          option(nom, picto(ic), prefs().variantes[t] === id, () => {
            choisirOutil(t, id);
            construireReglages();
          }),
        );
      }
      reglages.appendChild(r);
    }
    let cle = null;
    if (TRAITS.includes(s)) cle = "epaisseur";
    else if (s === "texte") cle = "tailleTexte";
    else if (s === "numero") cle = "numero";
    else if (s === "pixeliser" || s === "flouter") cle = "intensite";
    else if (s === "gomme") cle = "gomme";
    if (cle) {
      const r = el("div", "row");
      const noms =
        cle === "intensite"
          ? ["Léger", "Moyen", "Fort"]
          : cle === "gomme"
            ? ["Petite", "Moyenne", "Grande"]
            : ["Fin", "Moyen", "Épais"];
      const v = valeur(cle);
      for (let i = 0; i < 3; i++) {
        let contenu;
        if (cle === "tailleTexte") contenu = el("span", `a a${i}`, "A");
        else if (cle === "intensite") contenu = picto("pixel", `ico f${i}`);
        else if (cle === "gomme") contenu = el("b", `g${i}`);
        else contenu = el("b", `${s === "surligneur" ? "s" : cle === "numero" ? "n" : "t"}${i}`);
        const val = cle === "tailleTexte" ? TAILLES_TEXTE[i] : i;
        r.appendChild(option(noms[i], contenu, v === val, () => regler(cle, val)));
      }
      reglages.appendChild(r);
    }
    if (AVEC_OPACITE.has(s)) {
      const r = el("div", "row");
      const g = el("label", "rng");
      g.appendChild(picto("op"));
      const i = el("input");
      i.type = "range";
      i.min = 20;
      i.max = 100;
      i.step = 5;
      i.value = valeur("opacite");
      i.setAttribute("aria-label", "Opacité");
      let marque = `opacite-${++marqueGlissiere}`;
      i.addEventListener("input", () => regler("opacite", Number(i.value), marque));
      i.addEventListener("change", () => {
        marque = `opacite-${++marqueGlissiere}`;
      });
      g.appendChild(i);
      r.appendChild(g);
      reglages.appendChild(r);
    }
    return reglages.children.length > 0;
  }
  function ouvrirReglages(bouton) {
    if (!construireReglages()) return;
    ouvrir(reglages, bouton);
  }

  // ---------- menu « Fermer » (à la place du dialogue) ----------
  function ouvrirMenuFermer() {
    if (ouvert(menuFermer)) return;
    ed.outils?.validerSaisie();
    retirerVignette();
    ouvrir(menuFermer, bFermer);
    $('[data-act="copier_fermer"]', menuFermer).focus({ preventScroll: true });
  }
  function fermerMenuFermer() {
    if (!ouvert(menuFermer)) return;
    fermerMenus(false);
  }
  menuFermer.addEventListener("keydown", (e) => {
    if (e.key !== "Tab") return;
    const bs = $$(".mi", menuFermer);
    const i = bs.indexOf(document.activeElement);
    e.preventDefault();
    bs[(i + (e.shiftKey ? -1 : 1) + bs.length) % bs.length].focus();
  });

  // ---------- message, Copier ----------
  let minuterieMessage = null;
  function message(texte, echec = /^Impossible/.test(texte)) {
    const m = $("#message");
    $("span", m).textContent = texte;
    m.classList.toggle("echec", echec);
    m.classList.add("on");
    clearTimeout(minuterieMessage);
    const duree = jetonNombre(
      echec ? "duree-editeur-message-echec" : "duree-editeur-message-affiche",
    );
    minuterieMessage = setTimeout(() => m.classList.remove("on"), duree);
  }
  function fait(but) {
    if (but === "copier") message("Copié");
    else if (but === "enregistrer" || but === "enregistrer_sous") message("Enregistré");
  }
  // « Copier » réussi (copier puis fermer) : la coche ; le cœur ferme la fenêtre ou charge la
  // capture en attente (la page ne rappelle pas editeur_fermer).
  function copierReussi() {
    fermerMenus(false);
    bCopier.classList.remove("done");
    void bCopier.offsetWidth;
    bCopier.classList.add("done");
    const reduites =
      "reduites" in document.documentElement.dataset ||
      matchMedia("(prefers-reduced-motion: reduce)").matches;
    const d = jetonNombre(`duree-editeur-copier-fermer${reduites ? "-reduite" : ""}`);
    setTimeout(() => bCopier.classList.remove("done"), d);
  }
  const exporter = (but) => {
    if (ed.exportEnCours()) return;
    ed.exporter(but);
  };
  ed.actions = {
    copier: () => exporter("copier"),
    copierFermer: () => exporter("copier_fermer"),
    enregistrer: () => exporter("enregistrer"),
    enregistrerSous: () => {
      fermerMenus();
      exporter("enregistrer_sous");
    },
  };

  // ---------- vignette de prise (« Glisser vers une appli ») ----------
  let urlVignette = null;
  function montrerVignette() {
    if (!ed.fond || !ed.h) return;
    const cadre = ed.modele.cadre(ed.h.doc);
    const l = Math.round(
      (jetonNombre("editeur-vignette-largeur") - 2 * jetonNombre("editeur-pop-marge")) * 2,
    );
    const h = Math.max(1, Math.round((l * cadre.hauteur) / cadre.largeur));
    const c = document.createElement("canvas");
    c.width = l;
    c.height = h;
    const e = l / cadre.largeur;
    const vue = { x: Math.round(-cadre.x * e), y: Math.round(-cadre.y * e), echelle: e, dpr: 1 };
    ed.dessin.peindreVue(c.getContext("2d"), ed.h.doc, ed.fond, vue, []);
    c.toBlob((blob) => {
      c.width = 0;
      if (!blob) return;
      if (urlVignette) URL.revokeObjectURL(urlVignette);
      urlVignette = URL.createObjectURL(blob);
      $("#vignette-image").src = urlVignette;
    });
    vignette.hidden = false;
    vignette.tabIndex = 0;
    vignette.focus({ preventScroll: true });
  }
  function retirerVignette() {
    if (vignette.hidden) return;
    const focus = vignette.contains(document.activeElement);
    vignette.hidden = true;
    vignette.tabIndex = -1;
    appui = null;
    if (focus) scene.focus({ preventScroll: true });
  }
  // Appui puis 4 px de déplacement sur la vignette → export « glisser » ; le cœur fait le
  // glisser du système.
  let appui = null;
  vignette.addEventListener("pointerdown", (e) => {
    if (e.button !== 0) return;
    e.preventDefault();
    appui = { x: e.clientX, y: e.clientY, id: e.pointerId };
    vignette.setPointerCapture?.(e.pointerId);
  });
  vignette.addEventListener("pointermove", (e) => {
    if (!appui || e.pointerId !== appui.id) return;
    const seuil = jetonNombre("editeur-glisser-seuil");
    if (Math.hypot(e.clientX - appui.x, e.clientY - appui.y) < seuil) return;
    retirerVignette();
    ed.exporter("glisser");
  });
  for (const n of ["pointerup", "pointercancel"]) {
    vignette.addEventListener(n, () => {
      appui = null;
    });
  }
  function glisserEnCours(oui) {
    if (oui) retirerVignette();
  }

  // Menu contextuel d'un objet (« Forme »).
  function ouvrirMenuObjet(e) {
    const r = fenetre.getBoundingClientRect();
    ed.texte?.fermer();
    fermerMenus();
    menuObjet.classList.add("ouvert");
    const l = menuObjet.offsetWidth;
    const h = menuObjet.offsetHeight;
    const bord = jetonNombre("editeur-pop-ecart-haut");
    menuObjet.style.left = `${clamp(e.clientX - r.left, bord, r.width - l - bord)}px`;
    menuObjet.style.top = `${clamp(e.clientY - r.top, bord, r.height - h - bord)}px`;
    ouvreur = null;
    if (e.button !== 2) $(".mi", menuObjet)?.focus({ preventScroll: true });
  }

  // ---------- actions ----------
  function synchroniserActions() {
    const h = ed.h;
    $('[data-act="annuler"]').disabled =
      !h || (!ed.modele.peutAnnuler(h) && !ed.outils?.saisieOuverte());
    $('[data-act="retablir"]').disabled =
      !h || !ed.modele.peutRetablir(h) || !!ed.outils?.saisieOuverte();
  }
  // Annuler et Rétablir ne prennent pas le focus : une saisie ouverte n'est pas validée par
  // le clic (Annuler l'abandonne).
  for (const b of $$('[data-act="annuler"], [data-act="retablir"]')) {
    b.addEventListener("pointerdown", (e) => e.preventDefault());
    b.addEventListener("mousedown", (e) => e.preventDefault());
  }
  fenetre.addEventListener("click", (e) => {
    const b = e.target.closest("[data-act]");
    if (!b || b.disabled) return;
    const a = b.dataset.act;
    if (a === "annuler" || a === "retablir") {
      if (a === "annuler") ed.annuler();
      else ed.retablir();
      if (!ed.outils?.saisieOuverte()) rendreFocus(e);
    } else if (a === "copier") {
      fermerMenus();
      ed.actions.copierFermer();
    } else if (a === "enregistrer") {
      fermerMenus();
      ed.actions.enregistrer();
    } else if (a === "enregistrer_sous") ed.actions.enregistrerSous();
    else if (a === "glisser") {
      fermerMenus();
      montrerVignette();
    } else if (a === "fermer") {
      if (ouvert(menuFermer)) fermerMenus();
      else ed.fermer(false);
    } else if (a === "copier_fermer") {
      fermerMenus(false);
      ed.actions.copierFermer();
    } else if (a === "fermer_sans_copier") {
      fermerMenus(false);
      ed.fermer(true);
    } else if (a === "dupliquer") {
      fermerMenus();
      ed.outils.dupliquer();
    } else if (a === "devant") {
      fermerMenus();
      ed.outils.ordonner(true);
    } else if (a === "derriere") {
      fermerMenus();
      ed.outils.ordonner(false);
    } else if (a === "supprimer") {
      fermerMenus();
      ed.outils.supprimer();
    }
  });
  document.addEventListener(
    "pointerdown",
    (e) => {
      if (!vignette.hidden && !vignette.contains(e.target)) retirerVignette();
      if (!menuOuvert() || e.target === ed.toile) return;
      if (e.target.closest(".pop") || e.target.closest('[aria-expanded="true"]')) return;
      if (e.target.closest(".tl")) return;
      fermerMenus();
    },
    true,
  );

  // ---------- clavier et raccourcis du navigateur ----------
  const TOUCHES = {};
  for (const [id, o] of Object.entries(OUTILS)) {
    if (o.touche) TOUCHES[o.touche.toLowerCase()] = [id, null];
    for (const v of o.variantes || []) if (v[3]) TOUCHES[v[3].toLowerCase()] = [id, v[0]];
  }
  const NAVIGATEUR_CTRL = new Set("rpfugonjhlekwti+=-01sa".split(""));
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
  const enSaisie = (t) =>
    t &&
    ((t.tagName === "INPUT" && t.type !== "range" && t.type !== "color") ||
      t.tagName === "TEXTAREA" ||
      t.isContentEditable);

  // Toujours : les touches du navigateur ne font rien (même pendant une saisie).
  window.addEventListener(
    "keydown",
    (e) => {
      const ctl = e.ctrlKey || e.metaKey;
      const k = e.key.length === 1 ? e.key.toLowerCase() : e.key;
      if (
        NAVIGATEUR.has(e.key) ||
        (ctl && NAVIGATEUR_CTRL.has(k) && !enSaisie(e.target)) ||
        (ctl && ["r", "p", "f", "u", "s", "o", "g", "j", "+", "=", "-", "0"].includes(k)) ||
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

  let deplacement = false; // Espace tenue déplace la vue
  document.addEventListener("keydown", (e) => {
    const t = e.target;
    if (enSaisie(t)) return;
    const pop = t.closest?.(".pop");
    // Flèches haut et bas : d'une ligne à l'autre dans un menu ou une bulle.
    if (pop && (e.key === "ArrowDown" || e.key === "ArrowUp") && t.tagName !== "INPUT") {
      const it = $$(".mi, .opt, .dot", pop).filter((x) => x.offsetParent && !x.disabled);
      const i = it.indexOf(t);
      if (i >= 0) {
        e.preventDefault();
        it[(i + (e.key === "ArrowDown" ? 1 : it.length - 1)) % it.length].focus();
        return;
      }
    }
    if (ouvert(menuFermer)) {
      if (e.key === "Escape") {
        e.preventDefault();
        fermerMenus();
      }
      return;
    }
    const ctl = e.ctrlKey || e.metaKey;
    const k = e.key.toLowerCase();
    // Espace : déplacer la vue si le focus n'est pas sur un bouton (qu'il activerait).
    if (e.key === " " && !ctl && !t.closest?.("button, [role='menuitem'], [role='button']")) {
      e.preventDefault();
      deplacement = true;
      ed.outils.espace(true);
      return;
    }
    if (ctl) {
      if (e.altKey) return;
      if (k === "z" && !e.shiftKey) ed.annuler();
      else if (k === "y" || (k === "z" && e.shiftKey)) ed.retablir();
      else if (["c", "s", "d"].includes(k) && e.repeat) e.preventDefault();
      else if (k === "c" && !e.shiftKey) ed.actions.copier();
      else if (k === "s") (e.shiftKey ? ed.actions.enregistrerSous : ed.actions.enregistrer)();
      else if (k === "d") ed.outils.dupliquer();
      else if (k === "a" && !e.shiftKey) ed.outils.toutChoisir();
      else if (k === "0" || e.code === "Digit0" || e.code === "Numpad0") ed.ajuster();
      else if (k === "1" || e.code === "Digit1" || e.code === "Numpad1") ed.tailleReelle();
      else if (k === "=" || k === "+" || e.code === "NumpadAdd") ed.zoomerCentre(1.25);
      else if (k === "-" || e.code === "NumpadSubtract") ed.zoomerCentre(1 / 1.25);
      else return;
      e.preventDefault();
      return;
    }
    if (e.altKey) return;
    if (e.key === "Escape") {
      e.preventDefault();
      // Un geste en cours (bouton enfoncé) : Échap l'abandonne, rien d'autre.
      if (ed.outils.abandonnerGeste()) return;
      if (menuOuvert()) {
        const o = ouvreur;
        fermerMenus();
        o?.focus({ preventScroll: true });
      } else if (!vignette.hidden) retirerVignette();
      else if (ed.selection.length) {
        ed.selection = [];
        synchroniser();
        ed.dessiner();
      } else ed.fermer(false);
      return;
    }
    if (e.key === "Delete" || e.key === "Backspace") {
      if (ed.selection.length) {
        e.preventDefault();
        fermerMenus();
        ed.outils.supprimer();
      }
      return;
    }
    if (e.key.startsWith("Arrow") && ed.selection.length && !pop) {
      e.preventDefault();
      const pas = e.shiftKey ? 10 : 1;
      const dx = e.key === "ArrowLeft" ? -pas : e.key === "ArrowRight" ? pas : 0;
      const dy = e.key === "ArrowUp" ? -pas : e.key === "ArrowDown" ? pas : 0;
      ed.outils.pousser(dx, dy);
      return;
    }
    if (pop) return;
    const m = TOUCHES[k];
    if (m && !e.repeat) {
      fermerMenus();
      choisirOutil(m[0], m[1]);
    }
  });
  // Espace relâchée : annulée seulement si elle déplaçait la vue ou si la cible n'est pas un
  // bouton (sinon le bouton qui a le focus ne serait jamais activé, 0.5).
  document.addEventListener("keyup", (e) => {
    if (e.key === " ") {
      const actif = deplacement;
      deplacement = false;
      ed.outils.espace(false);
      const bouton = e.target.closest?.("button, [role='menuitem'], [role='button']");
      if (actif || !bouton) e.preventDefault();
    }
  });
  window.addEventListener("blur", () => {
    deplacement = false;
    ed.outils.espace(false);
  });

  // ---------- synchronisation ----------
  function synchroniser() {
    synchroniserOutils();
    synchroniserCouleurs();
    synchroniserActions();
    if (ouvert(reglages) && !reglant) {
      const b = $(`.tl[data-outil="${prefs().outil}"]`, outilsEl);
      if (!b || !construireReglages()) fermerMenus();
      else placerPop(reglages, b);
    }
  }
  function remiseAZero() {
    fermerMenus(false);
    retirerVignette();
    bCopier.classList.remove("done");
    synchroniser();
  }

  synchroniserOutils();
  return {
    synchroniser,
    remiseAZero,
    fait,
    message,
    copierReussi,
    ouvrirMenuFermer,
    fermerMenuFermer,
    glisserEnCours,
    menuOuvert,
    bulleOuverte: () => ouvert(reglages) || ouvert(menuCouleur),
    fermerMenus,
    ouvrirMenuObjet,
    choisirOutil,
    sorte,
    couleur: couleurDe,
    rang: rangDe,
    opacite: opaciteDe,
    regler,
    accent: () => jeton("accent"),
  };
}
