// Barre du haut et barre d'annotation de l'éditeur. Barre C et instruments B, choisis le
// 13/11 (design/annotation-pistes, design/annotation-instruments) : l'outil en main se déplie
// dans la barre avec ses variantes, ses tailles, ses couleurs et son opacité, et rien ne
// s'ouvre par-dessus l'image ; les autres outils gardent une petite nuance de leur couleur.
// Dans le pli, le choix est une pastille claire en relief et un trait fin sépare les
// variantes des réglages (barre plus claire, piste A « Calme », design/barre-claire). Un
// objet choisi déplie l'outil qui l'a fait, à ses valeurs : ce qu'on y règle s'applique à
// lui (et à la préférence de son outil). Les quatre instruments (flèche, stylo, surligneur,
// gomme) sont dessinés, le corps à la couleur de l'encre, et sautent quand leur couleur ou
// leur taille change. Menus « Plus », « Fermer » et de l'objet, vignette de prise, message,
// clavier. Textes posés en texte, jamais en HTML ; les instruments sont des constantes de la
// maquette, sans texte de l'utilisateur.

import { jeton, jetonNombre, DEFAUTS } from "./editeur.js";

const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
const NS = "http://www.w3.org/2000/svg";

// Outils (avenant §2, barre C). Variante : [id, nom, picto, touche]. La flèche n'a plus de
// variante « Courbe » : elle se courbe à sa poignée du milieu (outils.js).
export const OUTILS = {
  fleche: { nom: "Flèche", touche: "A" },
  stylo: { nom: "Stylo", touche: "P" },
  surligneur: { nom: "Surligneur", touche: "H" },
  gomme: { nom: "Gomme", touche: "G" },
  texte: { nom: "Texte", touche: "T", picto: "text" },
  forme: {
    nom: "Forme",
    picto: "shape",
    variantes: [
      ["rectangle", "Rectangle", "rect", "R"],
      ["ellipse", "Ellipse", "ell", "E"],
      ["ligne", "Ligne", "line", "L"],
    ],
  },
  numero: { nom: "Numéro", touche: "N", picto: "num" },
  // Barre plus claire, piste A « Calme » (design/barre-claire) : aucun outil ne répète le
  // dessin d'une de ses variantes. Le Flou est une goutte ; Pixeliser, une mosaïque pleine ;
  // Flouter, une tache floue ; Bloc noir, une bande toujours noire. Le Projecteur est une
  // lampe ; ses variantes dessinent la forme de sa lumière.
  flou: {
    nom: "Flou",
    picto: "goutte",
    variantes: [
      ["pixeliser", "Pixeliser", "mosaique", "B"],
      ["flouter", "Flouter", "doux"],
      ["bloc", "Bloc noir", "box"],
    ],
  },
  projecteur: {
    nom: "Projecteur",
    picto: "lampe",
    variantes: [
      ["rectangle", "Rectangle", "spot"],
      ["ovale", "Ovale", "spotE"],
    ],
  },
};
// Dans la barre : les quatre instruments dessinés, un séparateur, les cinq autres outils.
export const INSTRUMENTS = ["fleche", "stylo", "surligneur", "gomme"];
const AUTRES = ["texte", "forme", "numero", "flou", "projecteur"];

// Sorte de l'outil (variante comprise) → type d'objet créé.
export const TYPE_DE_SORTE = {
  fleche: "fleche",
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
  if (o === "forme" || o === "flou") return prefs.variantes[o];
  return o;
}
export function sorteObjet(o) {
  if (!o) return null;
  if (o.type === "masque") return o.sorte;
  return o.type;
}

const TRAITS = ["fleche", "rectangle", "ellipse", "ligne", "crayon", "surligneur"];
const AVEC_COULEUR = new Set([...TRAITS, "texte", "numero"]);
const AVEC_OPACITE = new Set([...AVEC_COULEUR].filter((s) => s !== "surligneur"));
export const TAILLES_TEXTE = [20, 28, 40];
// Type d'objet → l'outil qui le fait, dont il porte les réglages.
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
  projecteur: "projecteur",
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

// Ce que chaque outil déplie (ses variantes : OUTILS) : le réglage de ses trois tailles et
// leurs noms, ses couleurs, son opacité.
const NOMS_TAILLES = ["Fin", "Moyen", "Épais"];
const PETIT_GRAND = ["Petit", "Moyen", "Grand"];
const PLIS = {
  fleche: { taille: "epaisseur", couleur: true, opacite: true },
  stylo: { taille: "epaisseur", couleur: true, opacite: true },
  surligneur: { taille: "epaisseur", couleur: true },
  gomme: { taille: "gomme", noms: ["Petite", "Moyenne", "Grande"] },
  texte: { taille: "tailleTexte", noms: PETIT_GRAND, couleur: true, opacite: true },
  forme: { taille: "epaisseur", couleur: true, opacite: true },
  numero: { taille: "numero", noms: PETIT_GRAND, couleur: true, opacite: true },
  flou: { taille: "intensite", noms: ["Léger", "Moyen", "Fort"] },
  projecteur: {},
};

// ---------- instruments B « Couleur » (design/annotation-instruments) ----------
// Corps à la couleur de l'encre (--encre, posée par la page), bague blanche, bout gris à
// facettes ; la gomme taillée en biseau, dans un manchon à deux rainures. viewBox 28 × 84,
// pointe en haut, le bas sort du cadre. Couleurs par classes (editeur.css, jetons
// editeur-inst-*), jamais par un attribut `style` (CSP).
const FUT = "M7 36.2a2.2 2.2 0 0 1 2.2-2.2h9.6a2.2 2.2 0 0 1 2.2 2.2V100H7Z";
const FUT_L = "M6.2 36.2a2.2 2.2 0 0 1 2.2-2.2h11.2a2.2 2.2 0 0 1 2.2 2.2V100H6.2Z";
const BAGUE =
  "M7.9 32.2h12.2a1.4 1.4 0 0 1 1.4 1.4V35a1.4 1.4 0 0 1-1.4 1.4H7.9A1.4 1.4 0 0 1 6.5 35" +
  "v-1.4a1.4 1.4 0 0 1 1.4-1.4Z";
const BAGUE_L =
  "M7.1 32.2h13.8a1.4 1.4 0 0 1 1.4 1.4V35a1.4 1.4 0 0 1-1.4 1.4H7.1A1.4 1.4 0 0 1 5.7 35" +
  "v-1.4a1.4 1.4 0 0 1 1.4-1.4Z";
const CONE = "M7.7 32.6C8.6 26.4 11.3 18.8 12.2 15.4H15.8C16.7 18.8 19.4 26.4 20.3 32.6Z";
const PLUME =
  "M12.15 15.8C12.65 12.6 13.25 8.8 13.7 6.2Q14 5 14.3 6.2C14.75 8.8 15.35 12.6 15.85 15.8Z";
const COU = "M7.4 32.6L9.2 24.6H18.8L20.6 32.6Z";
const BISEAU = "M9.2 25V15.6Q9.2 14.7 10 14.2L17.5 9.8Q18.8 9 18.8 10.5V25Z";
const FACETTE = "M9.2 15.6L18.8 9.8V12.3L9.2 18.1Z";
const TIGE = "M13.1 18.6h1.8a.9.9 0 0 1 .9.9V33h-3.6V19.5a.9.9 0 0 1 .9-.9Z";
const TETE = "M14 4.8L21.2 18.3Q21.9 19.6 20.5 19.6H7.5Q6.1 19.6 6.8 18.3Z";
const GOMME = "M7.6 31V11.9Q7.6 10.2 9.11 10.72L18.89 14.08Q20.4 14.6 20.4 16.2V31Z";
const BISEAU_G = "M7.6 11.9Q7.6 10.2 9.11 10.72L18.89 14.08Q20.4 14.6 20.4 16.2V17.7L7.6 13.3Z";
const CONE_D = "M14 15.4H15.8C16.7 18.8 19.4 26.4 20.3 32.6H14Z";
const TETE_D = "M14 4.8L21.2 18.3Q21.9 19.6 20.5 19.6H14Z";
const COU_D = "M14 24.6H18.8L20.6 32.6H14Z";
const MANCHON = "M7.9 29.8h12.2a1.4 1.4 0 0 1 1.4 1.4V100h-15V31.2a1.4 1.4 0 0 1 1.4-1.4Z";
const piece = (classe, d) => `<path class="${classe}" d="${d}"/>`;
// Une pièce à la couleur de l'encre, son reflet, sa facette ; `taille` : elle suit la taille
// de l'outil (la pointe du stylo s'élargit, celle de la flèche grandit).
function encre(d, taille = false, facette = null) {
  const s =
    piece("i-encre", d) + piece("i-reflet", d) + (facette ? piece("i-facette-encre", facette) : "");
  return taille ? `<g class="tp">${s}</g>` : s;
}
const DESSINS = {
  fleche:
    encre(FUT) + piece("i-bague", BAGUE) + piece("i-prise", TIGE) + encre(TETE, true, TETE_D),
  stylo:
    encre(FUT) +
    piece("i-bague", BAGUE) +
    piece("i-prise", CONE) +
    piece("i-facette", CONE_D) +
    encre(PLUME, true),
  surligneur:
    encre(FUT_L) +
    piece("i-bague", BAGUE_L) +
    piece("i-prise", COU) +
    piece("i-facette", COU_D) +
    encre(BISEAU) +
    piece("i-lumiere", FACETTE),
  gomme:
    piece("i-corps", FUT) +
    piece("i-gomme", GOMME) +
    piece("i-gomme-reflet", GOMME) +
    piece("i-facette", BISEAU_G) +
    piece("i-entaille i-arete", "M7.6 13.3L20.4 17.7") +
    piece("i-prise", MANCHON) +
    piece("i-entaille", "M6.9 32.4H21.1M6.9 34.2H21.1"),
};
function instrument(t) {
  const texte =
    `<svg xmlns="${NS}" class="inst o-${t}" viewBox="0 0 28 84" aria-hidden="true"` +
    ` focusable="false">${DESSINS[t]}</svg>`;
  const svg = new DOMParser().parseFromString(texte, "image/svg+xml").documentElement;
  return document.importNode(svg, true);
}
// « f » : encre très foncée, « p » : encre très pâle (liseré plus marqué selon le thème :
// le noir sur la barre sombre, le blanc sur la barre claire).
const lineaire = (v) => {
  const c = v / 255;
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
};
function ton(c) {
  const m = /^#([0-9a-f]{6})$/i.exec(c || "");
  if (!m) return "";
  const n = parseInt(m[1], 16);
  const l =
    0.2126 * lineaire(n >> 16) + 0.7152 * lineaire((n >> 8) & 255) + 0.0722 * lineaire(n & 255);
  return l < 0.06 ? "f" : l > 0.8 ? "p" : "";
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
function bouton(classe, nom) {
  const b = el("button", classe);
  b.type = "button";
  nommer(b, nom);
  return b;
}
function groupe(classe, nom) {
  const g = el("div", classe);
  g.setAttribute("role", "group");
  g.setAttribute("aria-label", nom);
  return g;
}
// Le signe d'une taille : un point (trait, numéro, gomme, et l'intensité du Flou, en trois
// points comme les épaisseurs), un A (texte).
function signeTaille(t, cle, i) {
  if (cle === "tailleTexte") return el("span", `a a${i}`, "A");
  const lettre = { gomme: "g", numero: "n" }[cle] ?? (t === "surligneur" ? "s" : "t");
  return el("b", `${lettre}${i}`);
}
const clamp = (v, a, b) => Math.max(a, Math.min(b, v));
const reduites = () =>
  "reduites" in document.documentElement.dataset ||
  matchMedia("(prefers-reduced-motion: reduce)").matches;

export function installerBarre(ed) {
  const fenetre = $("#fenetre");
  const scene = $("#scene");
  const palette = $("#palette");
  const outilsEl = $("#outils");
  const plus = $("#menu-plus");
  const menuFermer = $("#menu-fermer");
  const menuObjet = $("#menu-objet");
  const bPlus = $("#bouton-plus");
  const bFermer = $('[data-act="fermer"]');
  const bCopier = $('[data-act="copier"]');
  const autre = $("#autre-couleur");
  const vignette = $("#vignette-prise");
  const pops = [plus, menuFermer, menuObjet];
  let ouvreur = null;

  const prefs = () => ed.prefs ?? DEFAUTS;
  const sorte = () => sorteOutil(prefs());
  // Préférences par outil (maquette : S.col, S.w, S.a).
  const couleurDe = (t = prefs().outil) => prefs().couleurs[t] ?? null;
  const rangDe = (t = prefs().outil) => prefs().tailles[t] ?? 1;
  const opaciteDe = (t = prefs().outil) => prefs().opacites[t] ?? 100;
  // Valeur de la préférence de l'outil `t` pour un réglage (unités de l'objet sauf
  // l'opacité, en pour cent).
  function prefDe(cle, t) {
    if (cle === "couleur") return couleurDe(t);
    if (cle === "opacite") return opaciteDe(t);
    if (cle === "tailleTexte") return TAILLES_TEXTE[rangDe(t)];
    return rangDe(t);
  }

  // L'outil de la sélection : le texte en saisie (ouvert d'un double clic, quel que soit
  // l'outil en main), sinon celui du premier objet choisi qui en a un (null : rien de
  // choisi). L'outil déplié : celui de la sélection, sinon l'outil en main.
  function outilChoisi() {
    if (ed.outils?.objetEnSaisie()) return "texte";
    for (const o of ed.objetsChoisis()) {
      const t = OUTIL_DE_TYPE[o.type];
      if (t) return t;
    }
    return null;
  }
  const deplie = () => outilChoisi() ?? prefs().outil;
  const deCetOutil = (t) => ed.objetsChoisis().filter((o) => OUTIL_DE_TYPE[o.type] === t);

  // Après un clic de souris (pas au clavier) dans la barre, le focus revient à la scène :
  // Espace tenu y déplace la vue. Un texte en saisie le reprend (le clic ne l'a pas validé).
  const rendreFocus = (e) => {
    if (ed.outils?.saisieOuverte()) ed.outils.reprendreSaisie();
    else if (e.detail > 0) scene.focus({ preventScroll: true });
  };

  // ---------- outils et leurs plis ----------
  // Chaque outil : son bouton (instrument dessiné ou pictogramme) et son pli (variantes,
  // tailles, couleurs, opacité), ouvert pour l'outil déplié seulement, inerte sinon.
  const plis = new Map(); // outil → { pli, bouton, dedans }
  let marqueGlissiere = 0;
  for (const t of [...INSTRUMENTS, "", ...AUTRES]) {
    if (!t) {
      outilsEl.appendChild(el("div", "dv"));
      continue;
    }
    const def = OUTILS[t];
    const r = PLIS[t];
    const pli = el("div", "pli");
    pli.dataset.pli = t;
    const b = bouton(INSTRUMENTS.includes(t) ? "tl ib" : "tl ip", def.nom);
    b.dataset.outil = t;
    if (INSTRUMENTS.includes(t)) {
      const inst = instrument(t);
      inst.addEventListener("animationend", () => inst.classList.remove("saute"));
      b.appendChild(inst);
    } else b.appendChild(picto(def.picto));
    b.addEventListener("click", (e) => {
      fermerMenus();
      choisirOutil(t);
      rendreFocus(e);
    });
    pli.appendChild(b);
    let dedans = null;
    if (def.variantes || r.taille) {
      dedans = el("div", "in");
      const d = el("div");
      if (def.variantes) d.appendChild(groupeVariantes(t));
      // Un trait fin sépare les variantes des réglages (piste A « Calme »).
      if (def.variantes && r.taille) d.appendChild(el("i", "sep"));
      if (r.taille) d.appendChild(groupeTailles(t));
      if (r.couleur) d.appendChild(groupeCouleurs(t));
      if (r.opacite) d.appendChild(glissiereOpacite(t));
      dedans.appendChild(d);
      dedans.inert = true;
      // Le pli ouvert se montre en entier dans une barre qui défile.
      dedans.addEventListener("transitionend", (e) => {
        if (e.target === dedans && pli.hasAttribute("data-on")) montrerPli(pli);
      });
      pli.appendChild(dedans);
    }
    outilsEl.appendChild(pli);
    plis.set(t, { pli, bouton: b, dedans });
  }

  function groupeVariantes(t) {
    const g = groupe("grp", "Variante");
    for (const [id, nom, ic] of OUTILS[t].variantes) {
      const b = bouton("sz", nom);
      b.dataset.variante = id;
      b.appendChild(picto(ic));
      b.addEventListener("click", (e) => {
        choisirVariante(t, id);
        rendreFocus(e);
      });
      g.appendChild(b);
    }
    return g;
  }
  function groupeTailles(t) {
    const { taille: cle, noms = NOMS_TAILLES } = PLIS[t];
    const g = groupe("grp", cle === "intensite" ? "Intensité" : "Taille");
    g.dataset.cle = cle;
    for (let i = 0; i < 3; i++) {
      const b = bouton("sz", noms[i]);
      b.dataset.taille = String(i);
      b.appendChild(signeTaille(t, cle, i));
      b.addEventListener("click", (e) => {
        regler(cle, cle === "tailleTexte" ? TAILLES_TEXTE[i] : i, null, t);
        rendreFocus(e);
      });
      g.appendChild(b);
    }
    return g;
  }
  function groupeCouleurs(t) {
    const g = groupe("grp cols", "Couleur");
    for (const [c, nom] of [...ed.dessin.COULEURS, ["", "Autre couleur"]]) {
      const b = bouton(c ? "sw" : "sw roue", nom);
      b.dataset.couleur = c;
      if (c) b.style.setProperty("--c", c);
      else b.appendChild(el("i"));
      b.addEventListener("click", (e) => {
        if (!c) {
          choisirAutre(t);
          return;
        }
        regler("couleur", c, null, t);
        rendreFocus(e);
      });
      g.appendChild(b);
    }
    return g;
  }
  // Opacité, de 20 à 100 % par pas de 5 : une glissière tirée = un pas d'annulation.
  function glissiereOpacite(t) {
    const g = el("label", "op");
    g.title = "Opacité";
    g.appendChild(picto("op"));
    const i = el("input");
    i.type = "range";
    i.min = "20";
    i.max = "100";
    i.step = "5";
    i.setAttribute("aria-label", "Opacité");
    let marque = `opacite-${++marqueGlissiere}`;
    i.addEventListener("input", () => regler("opacite", Number(i.value), marque, t));
    i.addEventListener("change", () => {
      // Tirée puis ramenée à sa valeur de départ dans le même geste : aucun pas.
      const h = ed.h;
      if (h?.marque === marque && JSON.stringify(h.doc) === h.passe[h.passe.length - 1]) {
        ed.abandonner();
      }
      marque = `opacite-${++marqueGlissiere}`;
      if (ed.outils?.saisieOuverte()) ed.outils.reprendreSaisie();
    });
    g.appendChild(i);
    return g;
  }

  // Barre plus large que la fenêtre : les outils défilent (molette comprise, n'importe où
  // sur la barre) et s'estompent du côté qui en cache ; Annuler, Rétablir et Plus restent.
  function suite() {
    const g = outilsEl.scrollLeft > 2;
    const d = outilsEl.scrollLeft + outilsEl.clientWidth < outilsEl.scrollWidth - 2;
    if (g || d) outilsEl.dataset.suite = g && d ? "deux" : g ? "debut" : "fin";
    else delete outilsEl.dataset.suite;
  }
  function montrerPli(pli) {
    // Du côté du bord de la zone, l'outil n'est pas sous le fondu.
    const marge = jetonNombre("editeur-dock-fondu");
    const g = pli.offsetLeft - marge;
    const d = pli.offsetLeft + pli.offsetWidth + marge;
    const behavior = reduites() ? "auto" : "smooth";
    // Plus large que la zone : son début (le bouton de l'outil) d'abord.
    if (g < outilsEl.scrollLeft || d - g > outilsEl.clientWidth) {
      outilsEl.scrollTo({ left: Math.max(0, g), behavior });
    } else if (d > outilsEl.scrollLeft + outilsEl.clientWidth) {
      outilsEl.scrollTo({ left: d - outilsEl.clientWidth, behavior });
    }
    suite();
  }
  outilsEl.addEventListener("scroll", suite, { passive: true });
  palette.addEventListener(
    "wheel",
    (e) => {
      if (e.ctrlKey || outilsEl.scrollWidth <= outilsEl.clientWidth) return;
      if (Math.abs(e.deltaY) <= Math.abs(e.deltaX)) return;
      e.preventDefault();
      outilsEl.scrollLeft += e.deltaY;
    },
    { passive: false },
  );
  const observateur = new ResizeObserver(suite);
  observateur.observe(palette);
  observateur.observe(outilsEl);

  function choisirOutil(id, variante) {
    if (!OUTILS[id]) return;
    ed.outils?.validerSaisie();
    const p = prefs();
    if (variante) p.variantes[id] = variante;
    p.outil = id;
    // Prendre un autre outil que celui de la sélection, ou une variante d'un outil au
    // clavier (« L » : Ligne) : on va dessiner avec lui ; l'objet choisi n'est pas changé (le
    // bouton de la variante, dans son pli, le change). Stylo, surligneur et gomme ne prennent
    // pas d'objet.
    const choisi = outilChoisi();
    const dessiner = choisi !== id || !!variante || ["stylo", "surligneur", "gomme"].includes(id);
    if (choisi && dessiner) {
      ed.selection = [];
    }
    synchroniser();
    ed.outils?.outilChange();
    ed.dessiner();
  }

  // Une variante : celle de l'outil. Le pli déplié pour la sélection : les objets choisis de
  // cet outil la prennent aussi (un pas d'annulation, aucun si rien ne change).
  function choisirVariante(t, id) {
    if (outilChoisi() !== t) {
      choisirOutil(t, id);
      return;
    }
    prefs().variantes[t] = id;
    const cibles = deCetOutil(t)
      .filter((o) => ed.modele.changerVariante(JSON.parse(JSON.stringify(o)), id))
      .map((o) => o.id);
    if (cibles.length) {
      ed.modifier((doc) => {
        for (const o of doc.objets) if (cibles.includes(o.id)) ed.modele.changerVariante(o, id);
      });
    }
    synchroniser();
    ed.dessiner();
  }

  // ---------- couleurs ----------
  let autrePour = null; // l'outil dont la roue « Autre couleur » a été ouverte
  function choisirAutre(t) {
    autrePour = t;
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
    regler("couleur", prefs().autre, `autre-${marqueAutre}`, autrePour ?? deplie());
  });
  autre.addEventListener("change", () => {
    marqueAutre += 1;
    if (ed.outils?.saisieOuverte()) ed.outils.reprendreSaisie();
  });

  // Un réglage change : les objets choisis à qui il s'applique (un pas d'annulation, aucun
  // si rien ne change ; `marque` : une glissière tirée = un pas), le texte en saisie, et la
  // préférence de LEUR outil : recolorer un texte choisi ne change pas la couleur de la
  // flèche en main. Sans objet à qui l'appliquer : la préférence de l'outil déplié `t`.
  // L'instrument dont la couleur ou la taille change saute.
  function regler(cle, valeur, marque = null, t = deplie()) {
    const p = prefs();
    const [champ, sApplique] = CHAMPS[cle];
    const choisis = ed.objetsChoisis().filter((o) => sApplique(sorteObjet(o)));
    const outils = new Set(choisis.map((o) => OUTIL_DE_TYPE[o.type]).filter(Boolean));
    const enSaisie = !!ed.outils?.objetEnSaisie() && sApplique("texte");
    if (enSaisie) outils.add("texte");
    if (!outils.size) outils.add(t);
    const avant = new Map([...outils].map((x) => [x, `${couleurDe(x)} ${rangDe(x)}`]));
    for (const x of outils) {
      if (cle === "couleur") {
        if (x in p.couleurs) p.couleurs[x] = valeur;
      } else if (cle === "opacite") {
        if (x in p.opacites) p.opacites[x] = valeur;
      } else p.tailles[x] = cle === "tailleTexte" ? TAILLES_TEXTE.indexOf(valeur) : valeur;
    }
    const v =
      cle === "numero"
        ? ed.dessin.RAYONS_NUMERO[valeur]
        : cle === "opacite"
          ? valeur / 100
          : valeur;
    if (enSaisie) ed.outils.reglerSaisie(champ, v);
    const cibles = choisis.filter((o) => o[champ] !== v).map((o) => o.id);
    if (cibles.length) {
      ed.modifier((doc) => {
        for (const o of doc.objets) if (cibles.includes(o.id)) o[champ] = v;
      }, marque);
    }
    synchroniser();
    for (const [x, a] of avant) if (`${couleurDe(x)} ${rangDe(x)}` !== a) sauter(x);
  }
  function sauter(t) {
    const s = plis.get(t)?.bouton.querySelector(".inst");
    if (!s || !ed.affiche) return;
    s.classList.remove("saute");
    void s.getBoundingClientRect();
    s.classList.add("saute");
  }

  // Ce que montre le pli de `t` : la valeur du texte en saisie ou d'un objet choisi de cet
  // outil, sinon sa préférence. Une taille hors des trois (objet agrandi par ses poignées) :
  // aucune.
  function valeur(cle, t) {
    const [champ, sApplique] = CHAMPS[cle];
    const saisie = t === "texte" ? ed.outils?.objetEnSaisie() : null;
    const o = saisie ?? deCetOutil(t).find((x) => sApplique(sorteObjet(x)));
    const brut = o && champ ? (o[champ] ?? (cle === "opacite" ? 1 : undefined)) : undefined;
    if (brut === undefined) return prefDe(cle, t);
    if (cle === "numero") return ed.dessin.RAYONS_NUMERO.indexOf(brut);
    if (cle === "opacite") return Math.round(brut > 1 ? brut : brut * 100);
    return brut;
  }
  function varianteDe(t) {
    const o = deCetOutil(t)[0];
    if (o) return t === "projecteur" ? o.forme : sorteObjet(o);
    return prefs().variantes[t];
  }
  const presser = (b, oui) => b.setAttribute("aria-pressed", oui ? "true" : "false");

  function synchroniserPli(t, dedans) {
    const r = PLIS[t];
    if (OUTILS[t].variantes) {
      const v = varianteDe(t);
      for (const b of $$("[data-variante]", dedans)) presser(b, b.dataset.variante === v);
    }
    if (r.taille) {
      const g = $("[data-cle]", dedans);
      // Flou : l'intensité ne vaut que pour Pixeliser et Flouter (son trait part avec elle).
      const cache = t === "flou" && varianteDe(t) === "bloc";
      if (cache !== g.hidden) {
        // L'intensité qui part gardait le focus (Rétablir, Annuler) : il va à la variante
        // prise, pas à la page.
        const focus = cache && g.contains(document.activeElement);
        g.hidden = cache;
        const sep = $(".sep", dedans);
        if (sep) sep.hidden = cache;
        if (focus) $('[data-variante][aria-pressed="true"]', dedans)?.focus({ preventScroll: true });
        // Le pli change de largeur : dans une barre qui défile, il se montre encore en entier.
        const pli = dedans.closest(".pli");
        if (pli.hasAttribute("data-on")) montrerPli(pli);
      }
      const v = valeur(r.taille, t);
      for (const b of $$("[data-taille]", g)) {
        const i = Number(b.dataset.taille);
        presser(b, v === (r.taille === "tailleTexte" ? TAILLES_TEXTE[i] : i));
      }
    }
    if (r.couleur) {
      const c = valeur("couleur", t);
      const cu = c ? c.toUpperCase() : null;
      const libre = prefs().autre;
      const connue = ed.dessin.COULEURS.some((x) => x[0] === cu);
      for (const b of $$("[data-couleur]", dedans)) {
        const d = b.dataset.couleur;
        if (d) presser(b, d === cu);
        else {
          b.classList.toggle("has", !!libre);
          if (libre) $("i", b).style.setProperty("--c", libre);
          presser(b, !!libre && cu === libre && !connue);
        }
      }
    }
    if (r.opacite) {
      const i = $('input[type="range"]', dedans);
      const v = String(valeur("opacite", t));
      if (i.value !== v) i.value = v;
    }
  }

  function synchroniserOutils() {
    const p = prefs();
    const ouvert = deplie();
    for (const [t, { pli, bouton: b, dedans }] of plis) {
      presser(b, t === p.outil);
      const c = couleurDe(t);
      const tn = ton(c);
      if (c) b.style.setProperty("--ink", c);
      else b.style.removeProperty("--ink");
      // La nuance sous le pictogramme n'a un liseré que pour un outil qui a une couleur.
      b.toggleAttribute("data-encre", !!c);
      if (tn) b.dataset.ton = tn;
      else delete b.dataset.ton;
      const inst = $(".inst", b);
      if (inst) {
        if (c) inst.style.setProperty("--encre", c);
        inst.classList.toggle("encre-foncee", tn === "f");
        inst.classList.toggle("encre-pale", tn === "p");
        inst.setAttribute("data-t", String(rangDe(t)));
      }
      const on = t === ouvert;
      pli.toggleAttribute("data-on", on);
      if (!dedans) continue;
      // Le pli qui se referme gardait le focus (un réglage pris au clavier) : il revient au
      // bouton de son outil, pas à la page.
      if (!on && dedans.contains(document.activeElement)) b.focus({ preventScroll: true });
      dedans.inert = !on;
      if (on) synchroniserPli(t, dedans);
    }
  }

  // ---------- menus : un seul ouvert à la fois ----------
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
  // Au-dessus de la barre d'annotation, centré sur le bouton ; sous un bouton de la barre du
  // haut.
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
  bPlus.addEventListener("click", () => {
    if (ouvert(plus)) fermerMenus();
    else ouvrir(plus, bPlus);
  });

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
    // La glissière d'opacité garde ses touches (flèches, début, fin, pages).
    const glissiere = t.tagName === "INPUT" && t.type === "range";
    if (glissiere && /^(Arrow|Home$|End$|Page)/.test(e.key)) return;
    const pop = t.closest?.(".pop");
    // Flèches haut et bas : d'une ligne à l'autre dans un menu.
    if (pop && (e.key === "ArrowDown" || e.key === "ArrowUp") && t.tagName !== "INPUT") {
      const it = $$(".mi", pop).filter((x) => x.offsetParent && !x.disabled);
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
      // Un geste en cours (bouton enfoncé) : Échap l'abandonne, rien d'autre. Un texte en
      // saisie (le focus passé dans la barre) : Échap l'abandonne, comme dans la saisie.
      if (ed.outils.abandonnerGeste()) return;
      if (ed.outils.annulerSaisie()) return;
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
    synchroniserActions();
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
