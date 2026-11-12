// Barre de l'éditeur (contrat 0.4 « Éditeur », §0, §3.3, §4.5, §4.6, §5, §7.2, §7.4) :
// outils et leurs variantes, fenêtre « Réglages de l'outil », zoom, actions, menus,
// dialogue de fermeture, infobulles et clavier. Textes posés en texte, jamais en HTML.
// D'après le design validé (design/editeur/index.html, piste A v2), sans Mise en avant, Fond
// ni Épingler (U1, U2).

import { jeton, jetonNombre, DEFAUTS } from "./editeur.js";

const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
const NS = "http://www.w3.org/2000/svg";

// Outils, dans l'ordre de la barre (§0). `touche` du groupe, ou de chaque variante.
export const GROUPES = [
  { id: "selection", nom: "Sélection", touche: "V", picto: "sel" },
  {
    id: "fleche",
    nom: "Flèche",
    touche: "A",
    variantes: [
      { id: "droite", nom: "Droite", picto: "arrow" },
      { id: "courbe", nom: "Courbe", picto: "arrowc" },
      { id: "double", nom: "Double", picto: "arrowd" },
    ],
  },
  {
    id: "formes",
    nom: "Formes",
    variantes: [
      { id: "rectangle", nom: "Rectangle", touche: "R", picto: "rect" },
      { id: "ellipse", nom: "Ellipse", touche: "E", picto: "ell" },
      { id: "ligne", nom: "Ligne", touche: "L", picto: "line" },
    ],
  },
  {
    id: "dessin",
    nom: "Dessin",
    variantes: [
      { id: "crayon", nom: "Crayon", touche: "P", picto: "pen" },
      { id: "surligneur", nom: "Surligneur", touche: "H", picto: "hl" },
    ],
  },
  { id: "texte", nom: "Texte", touche: "T", picto: "text" },
  { id: "numeros", nom: "Numéros", touche: "N", picto: "num" },
  {
    id: "masquer",
    nom: "Masquer",
    variantes: [
      { id: "pixeliser", nom: "Pixeliser", touche: "B", picto: "pixel" },
      { id: "flouter", nom: "Flouter", picto: "smooth" },
      { id: "bloc", nom: "Bloc noir", picto: "box" },
    ],
  },
  { id: "recadrer", nom: "Recadrer", touche: "C", picto: "crop", ratios: true },
];
export const RATIOS = [
  [0, "Libre"],
  [1, "1:1"],
  [4 / 3, "4:3"],
  [16 / 9, "16:9"],
];

// Sorte de l'outil (variante comprise) → type d'objet créé.
export const TYPE_DE_SORTE = {
  droite: "fleche",
  courbe: "fleche",
  double: "fleche",
  rectangle: "rectangle",
  ellipse: "ellipse",
  ligne: "ligne",
  crayon: "crayon",
  surligneur: "surligneur",
  texte: "texte",
  numeros: "numero",
  pixeliser: "masque",
  flouter: "masque",
  bloc: "masque",
};
export const SORTE_DE_MASQUE = { pixeliser: "pixeliser", flouter: "flouter", bloc: "bloc" };

const groupe = (id) => GROUPES.find((g) => g.id === id);
export function sorteOutil(prefs) {
  const g = groupe(prefs.outil);
  return g.variantes ? prefs.variantes[g.id] : g.id;
}
export function sorteObjet(o) {
  if (!o) return null;
  if (o.type === "fleche") return o.cx != null ? "courbe" : o.double ? "double" : "droite";
  if (o.type === "masque") return o.sorte;
  if (o.type === "numero") return "numeros";
  return o.type;
}

// Lignes de la fenêtre des réglages (§3.3) par sorte.
const AVEC_COULEUR = new Set([
  "droite",
  "courbe",
  "double",
  "rectangle",
  "ellipse",
  "ligne",
  "crayon",
  "surligneur",
  "texte",
  "numeros",
]);
const AVEC_EPAISSEUR = new Set([
  "droite",
  "courbe",
  "double",
  "rectangle",
  "ellipse",
  "ligne",
  "crayon",
  "surligneur",
]);
const AVEC_OPACITE = new Set([
  "droite",
  "courbe",
  "double",
  "rectangle",
  "ellipse",
  "ligne",
  "crayon",
  "texte",
  "numeros",
]);

// Réglage → champ de l'objet, et à quels objets il s'applique.
const CHAMPS = {
  couleur: ["couleur", (s) => AVEC_COULEUR.has(s)],
  epaisseur: ["epaisseur", (s) => AVEC_EPAISSEUR.has(s)],
  plein: ["plein", (s) => s === "rectangle" || s === "ellipse"],
  arrondi: ["arrondi", (s) => s === "rectangle"],
  tailleTexte: ["taille", (s) => s === "texte"],
  gras: ["gras", (s) => s === "texte"],
  styleTexte: ["style", (s) => s === "texte"],
  numero: ["rayon", (s) => s === "numeros"],
  intensite: ["intensite", (s) => s === "pixeliser" || s === "flouter"],
  opacite: ["opacite", (s) => AVEC_OPACITE.has(s)],
};

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
const clamp = (v, a, b) => Math.max(a, Math.min(b, v));

export function installerBarre(ed) {
  const fenetre = $("#fenetre");
  const scene = $("#scene");
  const outilsEl = $("#outils");
  const ind = $("#indicateur");
  const variantes = $("#menu-variantes");
  const reglages = $("#reglages-outil");
  const bReglages = $("#bouton-reglages");
  const plus = $("#menu-plus");
  const bPlus = $("#bouton-plus");
  const menuObjet = $("#menu-objet");
  const voile = $("#voile");
  const pops = [variantes, reglages, plus, menuObjet];
  let ouvreur = null;
  let marqueGlissiere = 0;
  let glisserGrise = false;
  let reglant = false;

  const prefs = () => ed.prefs ?? DEFAUTS;
  const sorte = () => sorteOutil(prefs());
  // Sorte que règle la fenêtre : celle du premier objet choisi avec Sélection.
  const sorteReglee = () =>
    prefs().outil === "selection" ? sorteObjet(ed.objetsChoisis()[0]) : sorte();

  // ---------- outils ----------
  for (const g of GROUPES) {
    const b = el("button", "tl");
    b.type = "button";
    b.dataset.outil = g.id;
    b.setAttribute("aria-pressed", "false");
    b.setAttribute("aria-label", g.nom);
    if (g.variantes || g.ratios) {
      b.setAttribute("aria-haspopup", "menu");
      b.setAttribute("aria-expanded", "false");
    }
    b.appendChild(picto(g.picto || g.variantes[0].picto));
    if (g.variantes || g.ratios) {
      const c = el("span", "chev");
      c.setAttribute("aria-hidden", "true");
      b.appendChild(c);
    }
    b.addEventListener("click", () => {
      if (prefs().outil === g.id && (g.variantes || g.ratios)) basculerVariantes(b, g);
      else {
        choisirOutil(g.id);
        fermerMenus();
      }
    });
    b.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      if (!g.variantes && !g.ratios) return;
      choisirOutil(g.id);
      basculerVariantes(b, g, true);
    });
    outilsEl.appendChild(b);
  }

  function synchroniserOutils() {
    const p = prefs();
    for (const g of GROUPES) {
      const b = $(`[data-outil="${g.id}"]`, outilsEl);
      const v = g.variantes ? g.variantes.find((x) => x.id === p.variantes[g.id]) : null;
      b.setAttribute("aria-pressed", p.outil === g.id ? "true" : "false");
      $("use", b).setAttribute("href", `#i-${v ? v.picto : g.picto}`);
      const nom = v ? v.nom : g.nom;
      const touche = v
        ? v.touche || g.touche || g.variantes.find((x) => x.touche)?.touche
        : g.touche;
      b.dataset.tip = nom;
      if (touche) {
        b.dataset.key = touche;
        b.setAttribute("aria-keyshortcuts", touche);
      } else {
        delete b.dataset.key;
        b.removeAttribute("aria-keyshortcuts");
      }
    }
  }

  function placerIndicateur(instant) {
    const b = $(`[data-outil="${prefs().outil}"]`, outilsEl);
    if (!b) return;
    if (instant) ind.classList.add("instant");
    ind.style.transform = `translate(${b.offsetLeft}px, ${b.offsetTop}px)`;
    if (instant) {
      void ind.offsetWidth;
      ind.classList.remove("instant");
    }
  }
  new ResizeObserver(() => placerIndicateur(true)).observe(outilsEl);

  function choisirOutil(id, variante) {
    ed.outils?.validerSaisie();
    ed.outils?.arreterPipette();
    const p = prefs();
    if (variante) p.variantes[id] = variante;
    p.outil = id;
    if (id !== "selection") ed.selection = [];
    synchroniserOutils();
    placerIndicateur(false);
    synchroniserPastille();
    if (reglages.classList.contains("ouvert")) construireReglages();
    ed.dessiner();
  }

  // ---------- menus flottants : un seul ouvert à la fois ----------
  function fermerMenus(sauf) {
    for (const p of pops) {
      if (p === sauf || !p.classList.contains("ouvert")) continue;
      p.classList.remove("ouvert");
      if (p.contains(document.activeElement)) {
        if (ouvreur && ouvreur !== sauf) ouvreur.focus({ preventScroll: true });
        else scene.focus({ preventScroll: true });
      }
    }
    for (const b of $$('[aria-expanded="true"]', fenetre)) {
      if (!sauf || b !== ouvreur) b.setAttribute("aria-expanded", "false");
    }
  }
  const menuOuvert = () => pops.some((p) => p.classList.contains("ouvert"));

  function ouvrir(p, bouton) {
    fermerMenus(p);
    ouvreur = bouton;
    p.classList.add("ouvert");
    if (bouton) bouton.setAttribute("aria-expanded", "true");
    placerPop(p, bouton);
  }
  // Au-dessus de la barre (des deux lignes si elle se replie), centré sur le bouton, dans
  // la fenêtre.
  function placerPop(p, bouton) {
    if (!bouton) return;
    const pill = p.parentElement;
    const haut = Math.min(...$$("#barre > .pill").map((x) => x.offsetTop));
    const ecart = jetonNombre("editeur-barre-ecart");
    p.style.bottom = `calc(100% + ${pill.offsetTop - haut + ecart}px)`;
    const l = p.offsetWidth;
    const c = bouton.offsetLeft + bouton.offsetWidth / 2;
    const pl = pill.getBoundingClientRect().left;
    const gauche = clamp(c - l / 2, 8 - pl, window.innerWidth - 8 - l - pl);
    p.style.left = `${gauche}px`;
    p.style.setProperty("--ox", `${c - gauche}px`);
  }

  function basculerVariantes(bouton, g, force) {
    if (variantes.classList.contains("ouvert") && !force) {
      fermerMenus();
      return;
    }
    variantes.replaceChildren();
    variantes.setAttribute("aria-label", g.nom);
    const p = prefs();
    if (g.ratios) {
      for (const [r, nom] of RATIOS) {
        const m = el("button", "mi");
        m.type = "button";
        m.setAttribute("role", "menuitemradio");
        m.setAttribute("aria-checked", p.ratio === r ? "true" : "false");
        m.append(picto("crop"), nom);
        m.addEventListener("click", () => {
          p.ratio = r;
          fermerMenus();
        });
        variantes.appendChild(m);
      }
      variantes.appendChild(el("div", "msep"));
      const f = el("button", "mi");
      f.type = "button";
      f.setAttribute("role", "menuitem");
      f.dataset.act = "image_entiere";
      f.append(picto("full"), "Image entière");
      f.disabled = !ed.h || !ed.h.doc.recadrage;
      f.addEventListener("click", () => {
        ed.outils.imageEntiere();
        fermerMenus();
      });
      variantes.appendChild(f);
    } else {
      for (const v of g.variantes) {
        const m = el("button", "mi");
        m.type = "button";
        m.setAttribute("role", "menuitemradio");
        m.dataset.variante = v.id;
        m.setAttribute("aria-checked", p.variantes[g.id] === v.id ? "true" : "false");
        m.append(picto(v.picto), v.nom);
        m.addEventListener("click", () => {
          choisirOutil(g.id, v.id);
          fermerMenus();
          bouton.focus({ preventScroll: true });
        });
        variantes.appendChild(m);
      }
    }
    ouvrir(variantes, bouton);
    const coche = $('[aria-checked="true"]', variantes);
    if (coche && bouton.matches(":focus-visible")) coche.focus();
  }

  // ---------- pastille et fenêtre « Réglages de l'outil » ----------
  function couleurCourante(s) {
    const o = prefs().outil === "selection" ? ed.objetsChoisis()[0] : null;
    if (o && o.couleur) return o.couleur;
    return s === "surligneur" ? prefs().couleurSurligneur : prefs().couleur;
  }
  function synchroniserPastille() {
    const s = sorteReglee();
    bReglages.classList.toggle("neutre", !AVEC_COULEUR.has(s));
    $("i", bReglages).style.setProperty("--c", couleurCourante(s));
    bReglages.disabled = !s || s === "recadrer" || s === "bloc" || s === "selection";
    if (bReglages.disabled && reglages.classList.contains("ouvert")) fermerMenus();
  }

  // Un réglage change : la préférence, et les objets choisis à qui il s'applique (un pas
  // d'annulation ; `marque` : une glissière tirée = un pas).
  function regler(cle, valeur, marque = null) {
    const p = prefs();
    const s = sorteReglee();
    if (cle === "couleur" && s === "surligneur") p.couleurSurligneur = valeur;
    else p[cle] = valeur;
    const [champ, sApplique] = CHAMPS[cle];
    const enSelection = p.outil === "selection";
    const cibles = ed
      .objetsChoisis()
      .filter((o) => {
        const so = sorteObjet(o);
        return sApplique(so) && (enSelection || so === s || o.type === TYPE_DE_SORTE[s]);
      })
      .map((o) => o.id);
    if (cle === "couleur") ajouterPerso(valeur);
    if (cibles.length) {
      const v =
        cle === "numero"
          ? ed.dessin.RAYONS_NUMERO[valeur]
          : cle === "opacite"
            ? valeur / 100
            : valeur;
      // La fenêtre n'est pas reconstruite sous le pointeur (glissière tirée).
      reglant = true;
      try {
        ed.modifier((doc) => {
          for (const o of doc.objets) if (cibles.includes(o.id)) o[champ] = v;
        }, marque);
      } finally {
        reglant = false;
      }
    }
    synchroniserPastille();
  }
  function ajouterPerso(c) {
    const p = prefs();
    c = c.toUpperCase();
    if (ed.dessin.COULEURS.some((x) => x[0] === c) || p.perso.includes(c)) return;
    p.perso.unshift(c);
    p.perso = p.perso.slice(0, ed.dessin.COULEURS_PERSO_MAX);
  }

  // Valeur montrée : celle de l'objet choisi s'il en a une, sinon la préférence.
  function valeur(cle) {
    const p = prefs();
    const o = p.outil === "selection" ? ed.objetsChoisis()[0] : null;
    const [champ, sApplique] = CHAMPS[cle];
    if (o && sApplique(sorteObjet(o)) && o[champ] !== undefined) {
      if (cle === "numero") return Math.max(0, ed.dessin.RAYONS_NUMERO.indexOf(o[champ]));
      if (cle === "opacite") return Math.round(o[champ] * 100);
      return o[champ];
    }
    if (cle === "couleur") return couleurCourante(sorteReglee());
    return p[cle];
  }

  function segments(libelle, items, courant, choisir, sansLibelle) {
    const r = el("div", "row");
    if (!sansLibelle) r.appendChild(el("span", "lab", libelle));
    const s = el("div", "seg2");
    s.setAttribute("role", "radiogroup");
    s.setAttribute("aria-label", libelle);
    for (const [v, texte, nom, trait] of items) {
      const b = el("button", "", trait == null ? texte : null);
      b.type = "button";
      b.setAttribute("role", "radio");
      b.setAttribute("aria-label", nom || texte);
      if (trait != null) b.appendChild(el("b", `t${trait}`));
      b.setAttribute("aria-checked", v === courant ? "true" : "false");
      b.addEventListener("click", () => {
        choisir(v);
        for (const x of $$("button", s)) x.setAttribute("aria-checked", x === b ? "true" : "false");
      });
      s.appendChild(b);
    }
    r.appendChild(s);
    return r;
  }
  function glissiere(libelle, min, max, v, cle, unite, sansLibelle) {
    const r = el("div", "row");
    if (!sansLibelle) r.appendChild(el("span", "lab", libelle));
    const i = el("input");
    i.type = "range";
    i.min = min;
    i.max = max;
    i.value = v;
    i.dataset.reglage = cle;
    i.setAttribute("aria-label", libelle);
    const val = el("span", "val");
    const maj = () => {
      val.textContent = `${i.value}${unite}`;
      i.style.setProperty("--v", `${((i.value - min) / (max - min)) * 100}%`);
    };
    maj();
    let marque = `${cle}-${++marqueGlissiere}`;
    i.addEventListener("input", () => {
      maj();
      regler(cle, Number(i.value), marque);
    });
    i.addEventListener("change", () => {
      marque = `${cle}-${++marqueGlissiere}`;
    });
    r.append(i, val);
    return r;
  }

  function construireReglages() {
    reglages.replaceChildren();
    const s = sorteReglee();
    if (!s || bReglages.disabled) return;
    const D = ed.dessin;
    if (AVEC_COULEUR.has(s)) {
      const row = el("div", "row");
      row.setAttribute("role", "radiogroup");
      row.setAttribute("aria-label", "Couleur");
      const cc = String(valeur("couleur")).toUpperCase();
      const liste = D.COULEURS.concat(prefs().perso.map((c) => [c, "Couleur perso"]));
      for (const [c, nom] of liste) {
        const b = el("button", "cl-sw");
        b.type = "button";
        b.setAttribute("role", "radio");
        b.setAttribute("aria-label", nom);
        b.dataset.couleur = c;
        b.setAttribute("aria-checked", c.toUpperCase() === cc ? "true" : "false");
        const i = el("i");
        i.style.setProperty("--c", c);
        b.appendChild(i);
        b.addEventListener("click", () => {
          regler("couleur", c);
          construireReglages();
        });
        row.appendChild(b);
      }
      const autre = el("label", "cl-sw arc");
      const arc = D.COULEURS.slice(0, 6).map((c) => c[0]);
      autre.style.setProperty("--arc", `conic-gradient(${[...arc, arc[0]].join(", ")})`);
      autre.dataset.tip = "Autre couleur";
      autre.appendChild(el("i"));
      const champ = el("input");
      champ.type = "color";
      champ.value = (/^#[0-9A-F]{6}$/.test(cc) ? cc : D.COULEURS[0][0]).toLowerCase();
      champ.setAttribute("aria-label", "Autre couleur");
      champ.addEventListener("change", () => {
        regler("couleur", champ.value.toUpperCase());
        construireReglages();
      });
      autre.appendChild(champ);
      row.appendChild(autre);
      const pk = el("button", "cl-sw pk");
      pk.type = "button";
      pk.id = "bouton-pipette";
      pk.setAttribute("aria-label", "Pipette");
      pk.dataset.tip = "Pipette";
      pk.dataset.key = "I";
      pk.setAttribute("aria-keyshortcuts", "I");
      pk.setAttribute("aria-pressed", ed.outils?.pipette() ? "true" : "false");
      pk.appendChild(picto("pick"));
      pk.addEventListener("click", () => ed.outils.basculerPipette());
      row.appendChild(pk);
      reglages.appendChild(row);
    }
    if (AVEC_EPAISSEUR.has(s)) {
      reglages.appendChild(
        segments(
          "Épaisseur",
          [
            [0, "", "Fin", 0],
            [1, "", "Moyen", 1],
            [2, "", "Épais", 2],
          ],
          valeur("epaisseur"),
          (v) => regler("epaisseur", v),
        ),
      );
    }
    if (s === "rectangle" || s === "ellipse") {
      reglages.appendChild(
        segments(
          "Style",
          [
            [false, "Contour"],
            [true, "Plein"],
          ],
          valeur("plein"),
          (v) => regler("plein", v),
        ),
      );
    }
    if (s === "rectangle") {
      reglages.appendChild(glissiere("Arrondi", 0, 40, valeur("arrondi"), "arrondi", " px"));
    }
    if (s === "texte") {
      reglages.appendChild(
        glissiere("Taille", 12, 96, valeur("tailleTexte"), "tailleTexte", " px", true),
      );
      const r = segments(
        "Style",
        [
          ["simple", "Simple"],
          ["contour", "Contour"],
          ["etiquette", "Étiquette"],
        ],
        valeur("styleTexte"),
        (v) => regler("styleTexte", v),
        true,
      );
      const g = el("button", "gbtn", "G");
      g.type = "button";
      g.setAttribute("aria-label", "Gras");
      g.dataset.tip = "Gras";
      g.setAttribute("aria-pressed", valeur("gras") ? "true" : "false");
      g.addEventListener("click", () => {
        const oui = g.getAttribute("aria-pressed") !== "true";
        g.setAttribute("aria-pressed", oui ? "true" : "false");
        regler("gras", oui);
      });
      r.insertBefore(g, r.firstChild);
      reglages.appendChild(r);
    }
    if (s === "numeros") {
      reglages.appendChild(
        segments(
          "Taille",
          [
            [0, "P", "Petit"],
            [1, "M", "Moyen"],
            [2, "G", "Grand"],
          ],
          valeur("numero"),
          (v) => regler("numero", v),
        ),
      );
    }
    if (s === "pixeliser" || s === "flouter") {
      reglages.appendChild(
        segments(
          "Intensité",
          [
            [0, "Légère"],
            [1, "Moyenne"],
            [2, "Forte"],
          ],
          valeur("intensite"),
          (v) => regler("intensite", v),
        ),
      );
    }
    if (AVEC_OPACITE.has(s)) {
      reglages.appendChild(
        glissiere("Opacité", 10, 100, valeur("opacite"), "opacite", " %", s === "texte"),
      );
    }
    if (reglages.classList.contains("ouvert")) placerPop(reglages, bReglages);
  }
  bReglages.addEventListener("click", () => {
    if (reglages.classList.contains("ouvert")) {
      fermerMenus();
      return;
    }
    construireReglages();
    ouvrir(reglages, bReglages);
  });

  // ---------- zoom ----------
  $("#zoom-ajuster").addEventListener("click", () => ed.ajuster());
  $("#zoom-reel").addEventListener("click", () => {
    if (ed.estTailleReelle()) ed.ajuster();
    else ed.tailleReelle();
  });
  function synchroniserZoom() {
    $("#zoom-ajuster").setAttribute(
      "aria-pressed",
      ed.cadrage.mode === "ajuster" ? "true" : "false",
    );
    $("#zoom-reel").setAttribute("aria-pressed", ed.estTailleReelle() ? "true" : "false");
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
  // le clic (Annuler l'abandonne, §3.3).
  for (const b of $$('[data-act="annuler"], [data-act="retablir"]')) {
    b.addEventListener("pointerdown", (e) => e.preventDefault());
    b.addEventListener("mousedown", (e) => e.preventDefault());
  }
  // L'onde au clic ; « Copié », « Enregistré » au résultat (§7.2).
  function onde(bouton, e) {
    const r = bouton.getBoundingClientRect();
    bouton.style.setProperty("--rx", `${e?.clientX ? e.clientX - r.left : r.width / 2}px`);
    bouton.style.setProperty("--ry", `${e?.clientY ? e.clientY - r.top : r.height / 2}px`);
    bouton.classList.remove("onde");
    void bouton.offsetWidth;
    bouton.classList.add("onde");
  }
  function fait(but) {
    const act =
      but === "copier"
        ? "copier"
        : but === "enregistrer" || but === "enregistrer_sous"
          ? "enregistrer"
          : null;
    if (!act) return;
    const b = $(`[data-act="${act}"]`);
    b.classList.remove("fait");
    void b.offsetWidth;
    b.classList.add("fait");
    $(".l1", b).setAttribute("aria-hidden", "true");
    $(".l2", b).removeAttribute("aria-hidden");
    clearTimeout(b.minuterie);
    b.minuterie = setTimeout(() => {
      b.classList.remove("fait", "onde");
      $(".l2", b).setAttribute("aria-hidden", "true");
      $(".l1", b).removeAttribute("aria-hidden");
    }, jetonNombre("duree-editeur-fait"));
  }
  let minuterieMessage = null;
  function message(texte) {
    const m = $("#message");
    m.textContent = texte;
    m.classList.add("on");
    clearTimeout(minuterieMessage);
    minuterieMessage = setTimeout(() => {
      m.classList.remove("on");
      m.textContent = "";
    }, jetonNombre("duree-editeur-message"));
  }
  // L'onde au clic, et au clavier (au centre du bouton) ; un export à la fois (editeur.js).
  const exporter = (but, e) => {
    if (ed.exportEnCours()) return;
    const b = $(`[data-act="${but === "copier" ? "copier" : "enregistrer"}"]`);
    if (but !== "enregistrer_sous") onde(b, e);
    ed.exporter(but);
  };
  ed.actions = {
    copier: (e) => exporter("copier", e),
    enregistrer: (e) => exporter("enregistrer", e),
    enregistrerSous: () => {
      fermerMenus();
      exporter("enregistrer_sous");
    },
  };

  // Menu « Plus » : vignette de l'image (blob:) pour la ligne Glisser.
  let urlVignette = null;
  function vignette() {
    if (!ed.fond || !ed.h) return;
    const img = $("#glisser-image");
    const l = jetonNombre("editeur-glisser-largeur") * 2;
    const h = jetonNombre("editeur-glisser-hauteur") * 2;
    const c = document.createElement("canvas");
    c.width = l;
    c.height = h;
    const cadre = ed.modele.cadre(ed.h.doc);
    const e = Math.max(l / cadre.largeur, h / cadre.hauteur);
    const vue = {
      x: Math.round((l - cadre.largeur * e) / 2 - cadre.x * e),
      y: Math.round((h - cadre.hauteur * e) / 2 - cadre.y * e),
      echelle: e,
      dpr: 1,
    };
    ed.dessin.peindreVue(c.getContext("2d"), ed.h.doc, ed.fond, vue, []);
    c.toBlob((blob) => {
      c.width = 0;
      if (!blob) return;
      if (urlVignette) URL.revokeObjectURL(urlVignette);
      urlVignette = URL.createObjectURL(blob);
      img.src = urlVignette;
    });
  }
  bPlus.addEventListener("click", () => {
    if (plus.classList.contains("ouvert")) {
      fermerMenus();
      return;
    }
    vignette();
    ouvrir(plus, bPlus);
  });

  // Glisser : appui puis 4 px de déplacement → export « glisser » ; le cœur fait le
  // glisser du système (§7.3).
  const ligneGlisser = $("#glisser");
  let appui = null;
  ligneGlisser.addEventListener("pointerdown", (e) => {
    if (e.button !== 0 || glisserGrise) return;
    e.preventDefault();
    appui = { x: e.clientX, y: e.clientY, id: e.pointerId };
    ligneGlisser.setPointerCapture?.(e.pointerId);
  });
  ligneGlisser.addEventListener("pointermove", (e) => {
    if (!appui || e.pointerId !== appui.id) return;
    const seuil = jetonNombre("editeur-glisser-seuil");
    if (Math.hypot(e.clientX - appui.x, e.clientY - appui.y) < seuil) return;
    appui = null;
    fermerMenus();
    ed.exporter("glisser");
  });
  const finAppui = () => {
    appui = null;
  };
  ligneGlisser.addEventListener("pointerup", finAppui);
  ligneGlisser.addEventListener("pointercancel", finAppui);
  function glisserEnCours(oui) {
    glisserGrise = oui;
    ligneGlisser.setAttribute("aria-disabled", oui ? "true" : "false");
  }

  // Menu contextuel d'un objet (« Forme »).
  function ouvrirMenuObjet(e) {
    const r = fenetre.getBoundingClientRect();
    fermerMenus(menuObjet);
    menuObjet.classList.add("ouvert");
    const l = menuObjet.offsetWidth;
    const h = menuObjet.offsetHeight;
    menuObjet.style.left = `${clamp(e.clientX - r.left, 8, r.width - l - 8)}px`;
    menuObjet.style.top = `${clamp(e.clientY - r.top, 8, r.height - h - 8)}px`;
    ouvreur = null;
    if (e.button !== 2) $(".mi", menuObjet)?.focus({ preventScroll: true });
  }

  // ---------- dialogue de fermeture (§4.5) ----------
  let focusAvant = null;
  const dialogueOuvert = () => voile.classList.contains("ouvert");
  function ouvrirDialogue() {
    fermerMenus();
    focusAvant = document.activeElement;
    voile.classList.add("ouvert");
    $('[data-act="copier_fermer"]', voile).focus();
  }
  function fermerDialogue() {
    if (!dialogueOuvert()) return;
    voile.classList.remove("ouvert");
    (focusAvant?.isConnected ? focusAvant : scene).focus?.({ preventScroll: true });
  }
  voile.addEventListener("keydown", (e) => {
    if (e.key !== "Tab") return;
    const bs = $$("button", voile);
    const i = bs.indexOf(document.activeElement);
    e.preventDefault();
    bs[(i + (e.shiftKey ? -1 : 1) + bs.length) % bs.length].focus();
  });
  voile.addEventListener("focusout", (e) => {
    if (dialogueOuvert() && !voile.contains(e.relatedTarget)) {
      $('[data-act="copier_fermer"]', voile).focus();
    }
  });

  // Les clics sur [data-act].
  fenetre.addEventListener("click", (e) => {
    const b = e.target.closest("[data-act]");
    if (!b || b.disabled) return;
    const a = b.dataset.act;
    if (a === "annuler") ed.annuler();
    else if (a === "retablir") ed.retablir();
    else if (a === "copier") ed.actions.copier(e);
    else if (a === "enregistrer") ed.actions.enregistrer(e);
    else if (a === "enregistrer_sous") ed.actions.enregistrerSous();
    else if (a === "dupliquer") {
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
    } else if (a === "copier_fermer") ed.exporter("copier_fermer");
    else if (a === "ne_pas_garder") ed.fermer(true);
    else if (a === "continuer") {
      fermerDialogue();
      ed.garder();
    }
  });
  document.addEventListener("pointerdown", (e) => {
    if (!menuOuvert()) return;
    if (e.target.closest(".pop") || e.target.closest('[aria-expanded="true"]')) return;
    if (e.target === ed.toile) return;
    fermerMenus();
  });

  // ---------- infobulles (nom et touche seulement) ----------
  const bulle = $("#infobulle");
  let minuterieBulle = 0;
  let bullePour = null;
  let derniereBulle = -Infinity;
  function montrerBulle(cible) {
    const t = cible.dataset.tip;
    if (!t) return;
    bulle.replaceChildren(el("span", "", t));
    if (cible.dataset.key) bulle.appendChild(el("kbd", "", cible.dataset.key));
    bulle.style.left = "0px";
    bulle.style.top = "0px";
    const r = cible.getBoundingClientRect();
    const l = bulle.offsetWidth;
    const h = bulle.offsetHeight;
    const x = clamp(r.left + r.width / 2 - l / 2, 8, window.innerWidth - l - 8);
    const y = clamp(r.top - h - 8, 8, window.innerHeight - h - 8);
    bulle.style.left = `${x}px`;
    bulle.style.top = `${y}px`;
    bulle.classList.add("on");
    bullePour = cible;
  }
  function cacherBulle() {
    clearTimeout(minuterieBulle);
    if (bullePour) derniereBulle = performance.now();
    bullePour = null;
    bulle.classList.remove("on");
  }
  document.addEventListener("mouseover", (e) => {
    const t = e.target.closest?.("[data-tip]");
    if (t === bullePour) return;
    if (!t || t.closest(".pop.ouvert")) {
      cacherBulle();
      return;
    }
    clearTimeout(minuterieBulle);
    const suite = performance.now() - derniereBulle < jetonNombre("duree-editeur-infobulle-suite");
    const attente = suite || bullePour ? 0 : jetonNombre("duree-editeur-infobulle");
    minuterieBulle = setTimeout(() => {
      if (!menuOuvert()) montrerBulle(t);
    }, attente);
  });
  document.addEventListener("focusin", (e) => {
    const t = e.target.closest?.("[data-tip]");
    if (t && t.matches(":focus-visible")) montrerBulle(t);
  });
  document.addEventListener("focusout", cacherBulle);
  document.addEventListener("pointerdown", cacherBulle, true);

  // ---------- clavier (§5) et raccourcis du navigateur (§4.6) ----------
  const TOUCHES = {};
  for (const g of GROUPES) {
    if (g.touche) TOUCHES[g.touche.toLowerCase()] = [g.id, null];
    for (const v of g.variantes || []) if (v.touche) TOUCHES[v.touche.toLowerCase()] = [g.id, v.id];
  }
  const NAVIGATEUR_CTRL = new Set([
    "r",
    "p",
    "f",
    "u",
    "g",
    "o",
    "n",
    "j",
    "h",
    "l",
    "e",
    "k",
    "w",
    "t",
    "i",
    "+",
    "=",
    "-",
    "0",
    "1",
    "s",
    "a",
  ]);
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

  document.addEventListener("keydown", (e) => {
    const t = e.target;
    if (enSaisie(t)) return;
    if (dialogueOuvert()) {
      if (e.key === "Escape") {
        e.preventDefault();
        fermerDialogue();
        ed.garder();
      }
      return;
    }
    const ctl = e.ctrlKey || e.metaKey;
    const k = e.key.toLowerCase();
    // Espace : déplacer la vue si le focus n'est pas sur un bouton (qu'il activerait).
    if (e.key === " " && !ctl && !t.closest?.("button, [role='menuitem']")) {
      e.preventDefault();
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
      if (ed.outils.pipette()) ed.outils.arreterPipette();
      else if (menuOuvert()) {
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
    if (e.key.startsWith("Arrow") && ed.selection.length && !t.closest?.(".pop")) {
      e.preventDefault();
      const pas = e.shiftKey ? 10 : 1;
      const dx = e.key === "ArrowLeft" ? -pas : e.key === "ArrowRight" ? pas : 0;
      const dy = e.key === "ArrowUp" ? -pas : e.key === "ArrowDown" ? pas : 0;
      ed.outils.pousser(dx, dy);
      return;
    }
    if (t.closest?.(".pop")) return;
    if (k === "i") {
      ed.outils.basculerPipette();
      return;
    }
    const m = TOUCHES[k];
    if (m && !e.repeat) {
      choisirOutil(m[0], m[1]);
      fermerMenus();
    }
  });
  document.addEventListener("keyup", (e) => {
    if (e.key === " ") {
      ed.outils.espace(false);
      e.preventDefault();
    }
  });
  window.addEventListener("blur", () => ed.outils.espace(false));

  // ---------- synchronisation ----------
  function synchroniser() {
    synchroniserOutils();
    synchroniserPastille();
    synchroniserActions();
    if (reglages.classList.contains("ouvert") && !reglant) construireReglages();
  }
  function remiseAZero() {
    fermerMenus();
    fermerDialogue();
    synchroniser();
    synchroniserZoom();
    placerIndicateur(true);
  }
  // Pendant un tracé, la barre passe à 35 % d'opacité.
  const tracer = (oui) => fenetre.classList.toggle("trace", oui);

  synchroniserOutils();
  return {
    synchroniser,
    synchroniserZoom,
    remiseAZero,
    fait,
    message,
    ouvrirDialogue,
    fermerDialogue,
    glisserEnCours,
    menuOuvert,
    fermerMenus,
    ouvrirMenuObjet,
    tracer,
    choisirOutil,
    sorte,
    construireReglages,
    reglagesOuverts: () => reglages.classList.contains("ouvert"),
    couleur: (s) => (s === "surligneur" ? prefs().couleurSurligneur : prefs().couleur),
    regler,
    accent: () => jeton("accent"),
  };
}
