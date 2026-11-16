// Barre du haut et socle d'annotation de l'éditeur. Piste C « Étages », choisie le 16/11
// (design/annotation-v3) : en bas, le socle porte les outils (Annuler, Rétablir, puis la
// rangée des outils) et, juste au-dessus d'eux, les réglages de l'outil déplié (variantes,
// tailles, couleurs, opacité) ; en haut, Fermer à gauche, et à droite Texte, Glisser,
// Enregistrer (sa flèche ouvre « Enregistrer sous… ») et Copier. Les règles restent celles
// de la 0.7 : un objet choisi déplie l'outil qui l'a fait, à ses valeurs ; un réglage va à
// l'objet choisi et à la préférence de son outil ; une variante convertit l'objet choisi.
// Menus « Fermer », « Enregistrer » et de l'objet, bouton Glisser, message, clavier. Textes
// posés en texte, jamais en HTML.

import { jeton, jetonNombre, DEFAUTS } from "./editeur.js";

const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
const NS = "http://www.w3.org/2000/svg";

// Variante : [id, nom, picto, touche].
export const OUTILS = {
  fleche: { nom: "Flèche", touche: "A", picto: "fleche" },
  forme: {
    nom: "Forme",
    picto: "shape",
    variantes: [
      ["rectangle", "Rectangle", "rect", "R"],
      ["ellipse", "Ellipse", "ell", "E"],
      ["ligne", "Ligne", "line", "L"],
    ],
  },
  texte: { nom: "Texte", touche: "T", picto: "text" },
  numero: { nom: "Numéro", touche: "N", picto: "num" },
  stylo: { nom: "Stylo", touche: "P", picto: "stylo" },
  surligneur: { nom: "Surligneur", touche: "H", picto: "surligneur" },
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
  gomme: { nom: "Gomme", touche: "G", picto: "gomme" },
};
// L'ordre des outils : ce qui montre, ce qui écrit, ce qui cache, ce qui corrige.
const ORDRE = ["fleche", "forme", "texte", "numero", "stylo", "surligneur", "flou", "projecteur", "gomme"];
const GROUPES = [4, 6, 8]; // un filet avant ces rangs, dans une rangée d'outils

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
const CHAMPS = {
  couleur: ["couleur", (s) => AVEC_COULEUR.has(s)],
  epaisseur: ["epaisseur", (s) => TRAITS.includes(s)],
  tailleTexte: ["taille", (s) => s === "texte"],
  numero: ["rayon", (s) => s === "numero"],
  intensite: ["intensite", (s) => s === "pixeliser" || s === "flouter"],
  opacite: ["opacite", (s) => AVEC_OPACITE.has(s)],
  gomme: [null, () => false],
};
const NOMS_TAILLES = ["Fin", "Moyen", "Épais"];
const PETIT_GRAND = ["Petit", "Moyen", "Grand"];
// Ce que chaque outil règle (ses variantes : OUTILS) : le réglage de ses trois tailles et
// leurs noms, ses couleurs, son opacité.
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

function svg(classe, vb = "0 0 20 20") {
  const s = document.createElementNS(NS, "svg");
  s.setAttribute("class", classe);
  s.setAttribute("viewBox", vb);
  s.setAttribute("aria-hidden", "true");
  return s;
}
function picto(id, classe = "ico") {
  const s = svg(classe);
  const use = document.createElementNS(NS, "use");
  use.setAttribute("href", `#i-${id}`);
  s.appendChild(use);
  return s;
}
function el(tag, classe, texte) {
  const e = document.createElement(tag);
  if (classe) e.className = classe;
  if (texte != null) e.textContent = texte;
  return e;
}
// L'infobulle : le nom seul.
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
function forme(nom, attributs) {
  const f = document.createElementNS(NS, nom);
  for (const [k, v] of Object.entries(attributs)) f.setAttribute(k, String(v));
  return f;
}
// Le signe d'une taille : le trait lui-même, à son épaisseur ; un A pour le texte ; un rond
// pour le numéro et la gomme ; un point pour l'intensité du Flou.
function signeTaille(t, cle, i) {
  if (cle === "tailleTexte") return el("span", `sg-a sg-a${i}`, "A");
  const s = svg("sg", "0 0 26 20");
  if (cle === "epaisseur") {
    const l = t === "surligneur" ? [5, 8, 11][i] : [1.6, 3, 5][i];
    const ligne = forme("path", { d: "M5 14.5 21 5.5", "stroke-width": l });
    ligne.setAttribute("class", t === "surligneur" ? "sg-trait sg-large" : "sg-trait");
    s.appendChild(ligne);
  } else if (cle === "gomme") {
    const c = forme("circle", { cx: 13, cy: 10, r: [3, 5.2, 7.6][i] });
    c.setAttribute("class", "sg-cercle");
    s.appendChild(c);
  } else {
    const c = forme("circle", { cx: 13, cy: 10, r: cle === "numero" ? [4, 6, 8][i] : [2.4, 4, 5.8][i] });
    c.setAttribute("class", "sg-plein");
    s.appendChild(c);
  }
  return s;
}
const clamp = (v, a, b) => Math.max(a, Math.min(b, v));
const reduites = () =>
  "reduites" in document.documentElement.dataset ||
  matchMedia("(prefers-reduced-motion: reduce)").matches;

export function installerBarre(ed) {
  const fenetre = $("#fenetre");
  const scene = $("#scene");
  const haut = $("#barre-haut");
  const chrome = $("#socle");
  const menuFermer = $("#menu-fermer");
  const menuObjet = $("#menu-objet");
  const menuEnregistrer = $("#menu-enregistrer");
  const autre = $("#autre-couleur");
  const pops = [menuFermer, menuObjet, menuEnregistrer];
  let ouvreur = null;

  const prefs = () => ed.prefs ?? DEFAUTS;
  const sorte = () => sorteOutil(prefs());
  const couleurDe = (t = prefs().outil) => prefs().couleurs[t] ?? null;
  const rangDe = (t = prefs().outil) => prefs().tailles[t] ?? 1;
  const opaciteDe = (t = prefs().outil) => prefs().opacites[t] ?? 100;
  function prefDe(cle, t) {
    if (cle === "couleur") return couleurDe(t);
    if (cle === "opacite") return opaciteDe(t);
    if (cle === "tailleTexte") return TAILLES_TEXTE[rangDe(t)];
    return rangDe(t);
  }
  // L'outil de la sélection (le texte en saisie, sinon le premier objet choisi), ou null.
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
  const rendreFocus = (e) => {
    if (ed.outils?.saisieOuverte()) ed.outils.reprendreSaisie();
    else if (e.detail > 0) scene.focus({ preventScroll: true });
  };
  const bFermer = () => $('[data-act="fermer"]', haut);
  const bCopier = () => $('[data-act="copier"]', haut);

  // ---------- les réglages d'un outil : variantes, tailles, couleurs, opacité ----------
  // Un trait fin entre deux groupes.
  function reglages(t) {
    const r = PLIS[t];
    const racine = el("div", "rg");
    const poser = (contenu) => {
      if (racine.childElementCount) racine.appendChild(el("i", "rg-sep"));
      racine.appendChild(contenu);
    };
    if (OUTILS[t].variantes) {
      const g = groupe("rg-grp rg-var", "Variante");
      for (const [id, nom, ic] of OUTILS[t].variantes) {
        const b = bouton("rg-b", nom);
        b.dataset.variante = id;
        b.appendChild(picto(ic));
        b.addEventListener("click", (e) => {
          choisirVariante(t, id);
          rendreFocus(e);
        });
        g.appendChild(b);
      }
      poser(g);
    }
    if (r.taille) {
      const { taille: cle, noms = NOMS_TAILLES } = r;
      const g = groupe("rg-grp rg-tailles", cle === "intensite" ? "Intensité" : "Taille");
      g.dataset.cle = cle;
      for (let i = 0; i < 3; i++) {
        const b = bouton("rg-b", noms[i]);
        b.dataset.taille = String(i);
        b.appendChild(signeTaille(t, cle, i));
        b.addEventListener("click", (e) => {
          regler(cle, cle === "tailleTexte" ? TAILLES_TEXTE[i] : i, null, t);
          rendreFocus(e);
        });
        g.appendChild(b);
      }
      poser(g);
    }
    if (r.couleur) {
      const g = groupe("rg-grp rg-couleurs", "Couleur");
      for (const [c, nom] of [...ed.dessin.COULEURS, ["", "Autre couleur"]]) {
        const b = bouton(c ? "rg-c" : "rg-c rg-roue", nom);
        b.dataset.couleur = c;
        if (c) b.style.setProperty("--c", c);
        if (c === "#FFFFFF") b.classList.add("rg-pale");
        if (c === "#1A1A1A") b.classList.add("rg-nuit");
        b.appendChild(el("i"));
        b.addEventListener("click", (e) => {
          if (!c) {
            choisirAutre(t, b);
            return;
          }
          regler("couleur", c, null, t);
          rendreFocus(e);
        });
        g.appendChild(b);
      }
      poser(g);
    }
    if (r.opacite) {
      const g = el("label", "rg-op");
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
        const h = ed.h;
        if (h?.marque === marque && JSON.stringify(h.doc) === h.passe[h.passe.length - 1]) {
          ed.abandonner();
        }
        marque = `opacite-${++marqueGlissiere}`;
        if (ed.outils?.saisieOuverte()) ed.outils.reprendreSaisie();
      });
      g.appendChild(i);
      poser(g);
    }
    return racine;
  }
  let marqueGlissiere = 0;

  // La zone des réglages montre ceux de l'outil déplié ; elle n'est refaite que s'il change.
  // Le focus qui était sur un réglage (pris au clavier) va au bouton de l'outil dont les
  // réglages partent, pas à la page.
  function majReglages(zone) {
    const t = deplie();
    if (zone.dataset.pour !== t) {
      if (zone.contains(document.activeElement)) {
        const b = zone.dataset.pour && $(`[data-outil="${zone.dataset.pour}"]`, chrome);
        (b ?? scene).focus({ preventScroll: true });
      }
      zone.dataset.pour = t;
      zone.replaceChildren(reglages(t));
    }
    synchroniserReglages(zone, t);
    return t;
  }
  function synchroniserReglages(zone, t) {
    const r = PLIS[t];
    if (OUTILS[t].variantes) {
      const v = varianteDe(t);
      for (const b of $$("[data-variante]", zone)) presser(b, b.dataset.variante === v);
    }
    if (r.taille) {
      const g = $("[data-cle]", zone);
      // Flou : l'intensité ne vaut que pour Pixeliser et Flouter.
      const cache = t === "flou" && varianteDe(t) === "bloc";
      const porte = g;
      if (cache !== porte.hidden) {
        if (cache && porte.contains(document.activeElement)) {
          $('[data-variante][aria-pressed="true"]', zone)?.focus({ preventScroll: true });
        }
        porte.hidden = cache;
        const sep = porte.previousElementSibling;
        if (sep?.classList.contains("rg-sep")) sep.hidden = cache;
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
      for (const b of $$("[data-couleur]", zone)) {
        const d = b.dataset.couleur;
        if (d) presser(b, d === cu);
        else {
          b.classList.toggle("a", !!libre);
          if (libre) b.style.setProperty("--c", libre);
          presser(b, !!libre && cu === libre && !connue);
        }
      }
    }
    if (r.opacite) {
      const i = $('input[type="range"]', zone);
      const v = String(valeur("opacite", t));
      if (i.value !== v) i.value = v;
      i.style.setProperty("--part", `${((Number(v) - 20) / 80) * 100}%`);
    }
  }

  // ---------- outils, variantes, réglages (règles de barre.js) ----------
  function choisirOutil(id, variante) {
    if (!OUTILS[id]) return;
    // Reprendre un outil referme « Copier le texte ».
    ed.texte?.fermer();
    ed.outils?.validerSaisie();
    const p = prefs();
    if (variante) p.variantes[id] = variante;
    p.outil = id;
    const choisi = outilChoisi();
    const dessiner = choisi !== id || !!variante || ["stylo", "surligneur", "gomme"].includes(id);
    if (choisi && dessiner) ed.selection = [];
    synchroniser();
    ed.outils?.outilChange();
    ed.dessiner();
  }
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
  let autrePour = null;
  function choisirAutre(t, b) {
    autrePour = t;
    // Le sélecteur de couleur de Windows s'ouvre près de la roue.
    if (b) {
      const f = fenetre.getBoundingClientRect();
      const r = b.getBoundingClientRect();
      autre.style.left = `${Math.round(r.left - f.left + r.width / 2)}px`;
      autre.style.top = `${Math.round(r.bottom - f.top)}px`;
      autre.style.bottom = "auto";
    }
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
  function regler(cle, val, marque = null, t = deplie()) {
    const p = prefs();
    const [champ, sApplique] = CHAMPS[cle];
    const choisis = ed.objetsChoisis().filter((o) => sApplique(sorteObjet(o)));
    const outils = new Set(choisis.map((o) => OUTIL_DE_TYPE[o.type]).filter(Boolean));
    const enSaisie = !!ed.outils?.objetEnSaisie() && sApplique("texte");
    if (enSaisie) outils.add("texte");
    if (!outils.size) outils.add(t);
    for (const x of outils) {
      if (cle === "couleur") {
        if (x in p.couleurs) p.couleurs[x] = val;
      } else if (cle === "opacite") {
        if (x in p.opacites) p.opacites[x] = val;
      } else p.tailles[x] = cle === "tailleTexte" ? TAILLES_TEXTE.indexOf(val) : val;
    }
    const v = cle === "numero" ? ed.dessin.RAYONS_NUMERO[val] : cle === "opacite" ? val / 100 : val;
    if (enSaisie) ed.outils.reglerSaisie(champ, v);
    const cibles = choisis.filter((o) => o[champ] !== v).map((o) => o.id);
    if (cibles.length) {
      ed.modifier((doc) => {
        for (const o of doc.objets) if (cibles.includes(o.id)) o[champ] = v;
      }, marque);
    }
    synchroniser();
  }
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
  // ---------- pièces : outils, actions, sorties ----------
  function boutonOutil(t, classe, avecNom) {
    const def = OUTILS[t];
    const b = bouton(classe, def.nom);
    b.dataset.outil = t;
    b.appendChild(picto(def.picto));
    if (avecNom) b.appendChild(el("span", null, def.nom));
    b.addEventListener("click", (e) => {
      fermerMenus();
      choisirOutil(t);
      rendreFocus(e);
    });
    return b;
  }
  function boutonAct(act, ic, nom, classe, avecNom) {
    const b = bouton(classe, nom);
    b.dataset.act = act;
    if (ic) b.appendChild(picto(ic));
    if (avecNom) b.appendChild(el("span", null, avecNom === true ? nom : avecNom));
    return b;
  }
  // « Copier » : copier puis fermer, comme la coche de la 0.7.1 ; le mot dit ce qu'il fait.
  function boutonCopier(classe) {
    const b = bouton(classe, "Copier");
    b.dataset.act = "copier";
    const coche = svg("coche", "0 0 20 20");
    coche.appendChild(forme("path", { d: "M4.5 10.5l3.6 3.6L15.5 6.5" }));
    b.append(coche, el("span", null, "Copier"));
    return b;
  }
  // « Glisser vers une appli » : appui puis 4 px → export « glisser » ; un simple clic secoue
  // le bouton et dit quoi faire ; le cœur fait le glisser du système.
  let appui = null;
  let parti = false;
  function boutonGlisser(classe, avecNom) {
    const b = bouton(classe, "Glisser vers une appli");
    b.dataset.glisser = "";
    b.appendChild(picto("drag"));
    if (avecNom) b.appendChild(el("span", null, "Glisser"));
    b.addEventListener("pointerdown", (e) => {
      if (e.button !== 0 || b.disabled) return;
      e.preventDefault();
      fermerMenus();
      ed.outils?.validerSaisie();
      appui = { x: e.clientX, y: e.clientY, id: e.pointerId };
      parti = false;
      b.setPointerCapture?.(e.pointerId);
    });
    b.addEventListener("pointermove", (e) => {
      if (!appui || e.pointerId !== appui.id) return;
      if (Math.hypot(e.clientX - appui.x, e.clientY - appui.y) < jetonNombre("editeur-glisser-seuil")) return;
      appui = null;
      parti = true;
      if (!ed.fond || !ed.h) return;
      exporter("glisser");
    });
    for (const n of ["pointerup", "pointercancel"]) b.addEventListener(n, () => (appui = null));
    b.addEventListener("click", () => {
      if (parti) {
        parti = false;
        return;
      }
      b.classList.remove("secoue");
      void b.offsetWidth;
      b.classList.add("secoue");
      message("Fais-le glisser vers une appli", true);
    });
    b.addEventListener("animationend", () => b.classList.remove("secoue"));
    return b;
  }
  function boutonTexte(classe, avecNom) {
    const b = bouton(classe, "Copier le texte");
    b.dataset.act = "texte";
    b.appendChild(picto("ocr"));
    if (avecNom) b.appendChild(el("span", null, avecNom));
    return b;
  }
  function annulerRetablir(classe) {
    const liste = [
      boutonAct("annuler", "undo", "Annuler", classe),
      boutonAct("retablir", "redo", "Rétablir", classe),
    ];
    // Ils ne prennent pas le focus : une saisie ouverte n'est pas validée par le clic.
    for (const b of liste) {
      b.addEventListener("pointerdown", (e) => e.preventDefault());
      b.addEventListener("mousedown", (e) => e.preventDefault());
    }
    return liste;
  }
  const boutonFermer = (classe) => {
    const b = boutonAct("fermer", "x", "Fermer", classe);
    b.setAttribute("aria-haspopup", "menu");
    b.setAttribute("aria-expanded", "false");
    return b;
  };
  // Les sorties, à droite de la barre du haut : Texte, Glisser,
  // Enregistrer (sa flèche ouvre « Enregistrer sous… »), Copier.
  function sorties() {
    const g = el("div", "sorties");
    const duo = el("div", "duo");
    const plus = bouton("s-b s-plus", "Enregistrer sous…");
    plus.dataset.menu = "enregistrer";
    plus.setAttribute("aria-haspopup", "menu");
    plus.setAttribute("aria-expanded", "false");
    plus.appendChild(picto("chevron", "ico petit"));
    plus.addEventListener("click", () => {
      if (ouvert(menuEnregistrer)) fermerMenus();
      else ouvrir(menuEnregistrer, plus);
    });
    duo.append(boutonAct("enregistrer", "save", "Enregistrer", "s-b", true), plus);
    g.append(boutonTexte("s-b", "Texte"), boutonGlisser("s-b", true), duo, boutonCopier("copier"));
    return g;
  }
  function rangeeOutils(conteneur, classe) {
    ORDRE.forEach((t, i) => {
      if (GROUPES.includes(i)) conteneur.appendChild(el("i", "filet"));
      conteneur.appendChild(boutonOutil(t, classe));
    });
  }

  // ---------- construction : la barre du haut et le socle ----------
  haut.append(boutonFermer("h-b"), el("div", "h-vide"), sorties());
  $(".h-vide", haut).setAttribute("data-tauri-drag-region", "");
  const rangee = el("div", "c-outils");
  rangee.append(...annulerRetablir("outil"), el("i", "filet"));
  rangeeOutils(rangee, "outil");
  const zoneReglages = el("div", "rg-zone");
  chrome.append(zoneReglages, rangee);

  // ---------- menus : un seul ouvert à la fois ----------
  const ouvert = (p) => p.classList.contains("ouvert");
  const menuOuvert = () => pops.some(ouvert);
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
  function ouvrir(p, b) {
    ed.texte?.fermer();
    fermerMenus();
    ouvreur = b;
    p.classList.add("ouvert");
    if (b?.hasAttribute("aria-expanded")) b.setAttribute("aria-expanded", "true");
    placerPop(p, b);
    const premier = $("button, input", p);
    if (premier && document.querySelector(":focus-visible")) premier.focus({ preventScroll: true });
  }
  // Sous le bouton s'il est dans le haut de la fenêtre, au-dessus sinon ; « Fermer » calé à
  // gauche, « Enregistrer sous… » à droite.
  function placerPop(p, b) {
    const er = fenetre.getBoundingClientRect();
    const r = b.getBoundingClientRect();
    const l = p.offsetWidth;
    const h = p.offsetHeight;
    const bord = jetonNombre("editeur-pop-ecart-haut");
    const ecart = jetonNombre("editeur-pop-ecart");
    const dessous =
      r.top - er.top + r.height / 2 < er.height / 2 && r.bottom - er.top + h + 2 * bord < er.height;
    let x = clamp(r.left - er.left + r.width / 2 - l / 2, bord, er.width - l - bord);
    if (b.dataset.act === "fermer") x = r.left - er.left;
    if (b.dataset.menu === "enregistrer") x = clamp(r.right - er.left - l, bord, er.width - l - bord);
    const y = dessous ? r.bottom - er.top + bord : r.top - er.top - h - ecart;
    p.style.setProperty("--oy", dessous ? "0%" : "100%");
    p.style.setProperty("--ox", `${r.left - er.left + r.width / 2 - x}px`);
    p.style.left = `${x}px`;
    p.style.top = `${y}px`;
  }
  function ouvrirMenuFermer() {
    if (ouvert(menuFermer)) return;
    ed.outils?.validerSaisie();
    ouvrir(menuFermer, bFermer());
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
  function ouvrirMenuObjet(e) {
    const r = fenetre.getBoundingClientRect();
    ed.texte?.fermer();
    fermerMenus();
    menuObjet.classList.add("ouvert");
    const l = menuObjet.offsetWidth;
    const h = menuObjet.offsetHeight;
    menuObjet.style.setProperty("--ox", "0px");
    menuObjet.style.setProperty("--oy", "0%");
    const bord = jetonNombre("editeur-pop-ecart-haut");
    menuObjet.style.left = `${clamp(e.clientX - r.left, bord, r.width - l - bord)}px`;
    menuObjet.style.top = `${clamp(e.clientY - r.top, bord, r.height - h - bord)}px`;
    ouvreur = null;
    if (e.button !== 2) $(".mi", menuObjet)?.focus({ preventScroll: true });
  }

  // ---------- message, Copier ----------
  let minuterieMessage = null;
  function message(texte, echec = /^Impossible/.test(texte)) {
    const m = $("#message");
    $("span", m).textContent = texte;
    m.classList.toggle("echec", echec);
    m.classList.add("on");
    clearTimeout(minuterieMessage);
    const duree = jetonNombre(echec ? "duree-editeur-message-echec" : "duree-editeur-message-affiche");
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
    const b = bCopier();
    b.classList.remove("fait");
    void b.offsetWidth;
    b.classList.add("fait");
    const d = jetonNombre(`duree-editeur-copier-fermer${reduites() ? "-reduite" : ""}`);
    setTimeout(() => b.classList.remove("fait"), d);
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
    enregistrerBreezy: () => {
      fermerMenus();
      if (ed.exportEnCours()) return;
      ed.enregistrerBreezy();
    },
  };
  function glisserEnCours(oui) {
    for (const b of $$("[data-glisser]", haut)) b.classList.toggle("glisse", oui);
    if (!oui) parti = false;
  }

  // ---------- actions ----------
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
    else if (a === "enregistrer_breezy") ed.actions.enregistrerBreezy();
    else if (a === "texte") {
      fermerMenus();
      ed.texte?.lancer();
    } else if (a === "fermer") {
      // Pendant « Copier le texte », Fermer ferme d'abord le calque (texte.js).
      ed.texte?.fermer();
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
      } else if (ed.selection.length) {
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
    const p = prefs();
    for (const b of $$("[data-outil]", chrome)) {
      const t = b.dataset.outil;
      presser(b, t === p.outil);
      const c = couleurDe(t);
      if (c) b.style.setProperty("--encre", c);
      b.toggleAttribute("data-encre", !!c);
    }
    majReglages(zoneReglages);
    const h = ed.h;
    $('[data-act="annuler"]', chrome).disabled =
      !h || (!ed.modele.peutAnnuler(h) && !ed.outils?.saisieOuverte());
    $('[data-act="retablir"]', chrome).disabled =
      !h || !ed.modele.peutRetablir(h) || !!ed.outils?.saisieOuverte();
    // « Copier le texte » : le bouton n'est montré que si la charge le dit ; pendant le calque,
    // Enregistrer, Copier et Glisser sont grisés et le socle s'efface (texte.js).
    const calque = !!ed.texte?.ouvert();
    fenetre.toggleAttribute("data-calque", calque);
    // Effacé, le socle sort aussi de l'arbre d'accessibilité et du parcours au clavier.
    chrome.inert = calque;
    const bTexte = $('[data-act="texte"]', haut);
    bTexte.hidden = !ed.texte?.disponible();
    bTexte.disabled = ed.exportEnCours();
    const grises =
      '[data-act="enregistrer"], [data-act="copier"], [data-glisser], [data-menu="enregistrer"]';
    for (const b of $$(grises, haut)) b.disabled = calque;
    $('[data-act="enregistrer_breezy"]', menuEnregistrer).disabled = calque;
  }
  // « Enregistrer en .breezy » est une fonction Pro : sans licence l'entrée reste visible,
  // éteinte, avec sa mention ; son clic ouvre Réglages › Licence (c'est le cœur qui décide).
  function poserBreezy(pro) {
    const b = $('[data-act="enregistrer_breezy"]', menuEnregistrer);
    if (!b) return;
    b.classList.toggle("pro", !pro);
    // La mention « Pro » est peinte par la feuille de style ; le nom accessible la dit aussi.
    b.setAttribute("aria-label", pro ? "Enregistrer en .breezy" : "Enregistrer en .breezy, Pro");
  }
  function remiseAZero() {
    fermerMenus(false);
    appui = null;
    parti = false;
    for (const b of $$("[data-glisser]", haut)) b.classList.remove("glisse", "secoue");
    bCopier().classList.remove("fait");
    synchroniser();
  }

  synchroniser();
  return {
    synchroniser,
    remiseAZero,
    poserBreezy,
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
