// Gestes sur la scène (contrat 0.4 « Éditeur », §3.3, et avenant « Éditeur épuré », §2) :
// tracés, gomme, projecteur, sélection et déplacement par appui sur un objet, poignées, texte
// en place, numéros, masques, recadrage par les équerres des coins, vue à la main. Tout
// changement du document passe par `ed.modifier` (document.js) ; la page dessine par-dessus
// la vue les équerres, le recadrage en cours et les poignées.
//
// Gomme : elle efface l'encre (stylo, surligneur, flèche, ligne, rectangle, ellipse) là où
// elle passe, et là seulement ; un geste = un pas d'annulation, et un geste qui n'efface
// aucun pixel n'en fait pas. Une encre dont il ne reste rien est retirée. Texte, numéro, flou
// et projecteur sont entiers : la gomme passe dessus sans les entamer, un clic de gomme sur
// l'un d'eux (sans encre effacée) l'enlève. Le rond qui suit le pointeur a la taille de la
// gomme, en pixels de l'écran : zoomer pour gommer fin.
// Échap pendant un geste l'abandonne (rien n'est créé, déplacé, recadré ni gommé). Maj :
// 15° pour flèche, ligne et bout tiré ; carré à la création, proportions gardées par un coin ;
// trait droit pour le stylo et le surligneur. Stylo et surligneur : le pointeur tenu immobile
// en fin de trait redresse un trait presque droit.

import { jeton, jetonNombre } from "./editeur.js";
import { sorteObjet, TYPE_DE_SORTE, TAILLES_TEXTE } from "./barre.js";

const $ = (s) => document.querySelector(s);
const clamp = (v, a, b) => Math.max(a, Math.min(b, v));
const RECTANGULAIRES = new Set(["rectangle", "ellipse", "masque", "projecteur"]);
const MIN_TRAIT = 8; // flèche ou ligne : ≤ 8 px, rien n'est créé
const MIN_BOITE = 6; // autre : ≤ 6 px
const ANGLE = Math.PI / 12; // Maj : 15°
const SANS_PRISE = new Set(["crayon", "surligneur", "gomme"]); // un appui ne prend pas d'objet
const ENTIERS = ["texte", "numero", "masque", "projecteur"]; // un clic de gomme les enlève
const PAS_GOMME = 1; // px d'écran entre deux points d'un passage de gomme
const DROIT_ECART = 0.08; // redresser : écart au plus, en part de la longueur du trait
const DROIT_AXE = (4 * Math.PI) / 180; // redresser : à moins de 4° d'un axe, le trait s'y cale

const copie = (o) => JSON.parse(JSON.stringify(o));

export function installerOutils(ed) {
  const toile = ed.toile;
  const scene = ed.scene;
  const saisie = $("#saisie");
  const rond = $("#curseur-gomme");
  let geste = null; // { mode: pan|equerre|gomme|deplacer|poignee|trace, … }
  let dernier = null; // dernier mouvement du pointeur (Maj enfoncée ou relâchée le rejoue)
  let survol = null; // { clientX, clientY } du pointeur sur la toile, ou null
  let espace = false;
  let edition = null; // { objet, existant }
  let numeroGeste = 0;

  const sorte = () => ed.barre.sorte();
  const echelleCss = () => ed.vue.echelle / ed.vue.dpr; // pixels CSS par pixel de l'image
  const tolerance = () => jetonNombre("editeur-tolerance") / echelleCss();

  // Pointeur → pixels de l'image ; image → pixels de la toile.
  function versImage(e) {
    const r = toile.getBoundingClientRect();
    const { x, y, echelle, dpr } = ed.vue;
    return {
      x: ((e.clientX - r.left) * dpr - x) / echelle,
      y: ((e.clientY - r.top) * dpr - y) / echelle,
    };
  }
  const versToile = (px, py) => [ed.vue.x + px * ed.vue.echelle, ed.vue.y + py * ed.vue.echelle];

  function poigneeSous(p) {
    if (ed.selection.length !== 1 || SANS_PRISE.has(sorte())) return null;
    const o = ed.objet(ed.selection[0]);
    if (!o) return null;
    const r = jetonNombre("editeur-poignee-prise") / echelleCss();
    for (const [x, y, nom] of ed.modele.poignees(o)) {
      if (Math.hypot(p.x - x, p.y - y) < r) return nom;
    }
    return null;
  }
  // L'équerre d'un coin de l'image sous le pointeur (zone de prise de 22 px), ou null.
  function equerreSous(e) {
    if (!ed.h || !ed.fond) return null;
    const r = toile.getBoundingClientRect();
    const dpr = ed.vue.dpr;
    const mx = (e.clientX - r.left) * dpr;
    const my = (e.clientY - r.top) * dpr;
    const k = ed.modele.cadre(ed.h.doc);
    const [ax, ay] = versToile(k.x, k.y);
    const [bx, by] = versToile(k.x + k.largeur, k.y + k.hauteur);
    const prise = jetonNombre("editeur-equerre-prise") * dpr;
    for (const [nom, x, y] of [
      ["nw", ax, ay],
      ["ne", bx, ay],
      ["sw", ax, by],
      ["se", bx, by],
    ]) {
      if (Math.hypot(mx - x, my - y) < prise) return nom;
    }
    return null;
  }

  // ---------- nouveaux objets ----------
  function nouvelObjet(s, p) {
    const pr = ed.prefs;
    const couleur = ed.barre.couleur();
    const opacite = ed.barre.opacite() / 100;
    const epaisseur = ed.barre.rang();
    const base = { x1: p.x, y1: p.y, x2: p.x, y2: p.y };
    switch (TYPE_DE_SORTE[s]) {
      case "fleche":
        return {
          type: "fleche",
          ...base,
          double: false,
          courbe: s === "courbe",
          couleur,
          epaisseur,
          opacite,
        };
      case "rectangle":
        return {
          type: "rectangle",
          ...base,
          couleur,
          epaisseur,
          plein: false,
          arrondi: 6,
          opacite,
        };
      case "ellipse":
        return {
          type: "ellipse",
          ...base,
          couleur,
          epaisseur,
          plein: false,
          opacite,
        };
      case "ligne":
        return { type: "ligne", ...base, couleur, epaisseur, opacite };
      case "crayon":
        return { type: "crayon", points: [[p.x, p.y]], couleur, epaisseur, opacite };
      case "surligneur":
        return { type: "surligneur", points: [[p.x, p.y]], couleur, epaisseur };
      case "masque":
        return { type: "masque", ...base, sorte: s, intensite: epaisseur };
      case "projecteur":
        return { type: "projecteur", forme: pr.variantes.projecteur, ...base };
      default:
        return null;
    }
  }
  // Pose `o` dans le document (un pas ; `ajouter` donne l'`id`, et l'`ordre` d'un numéro).
  // Il n'est pas choisi (maquette) : la sélection ne vient que d'un appui sur un objet.
  function poser(o) {
    let id = 0;
    ed.modifier((doc) => {
      id = ed.modele.ajouter(doc, o);
    });
    ed.selection = [];
    ed.barre.synchroniser();
    ed.dessiner();
    return id;
  }

  // ---------- pointeur ----------
  function appui(e) {
    if (e.button === 2) return;
    if (e.button !== 0 && e.button !== 1) return;
    e.preventDefault();
    // Une saisie ouverte : le clic ailleurs la valide seulement (lu avant le focus, qui la
    // validerait aussi).
    const enEdition = !!edition;
    if (enEdition) validerSaisie();
    scene.focus({ preventScroll: true });
    if (enEdition || !ed.h || !ed.fond) return;
    // Un menu ouvert : l'appui le referme seulement (maquette). Une bulle de réglage (taille,
    // couleur) se referme et le geste commence : le réglage choisi, on dessine.
    if (ed.barre.menuOuvert() && !espace) {
      const bulle = ed.barre.bulleOuverte();
      ed.barre.fermerMenus();
      if (!bulle) return;
    }
    toile.setPointerCapture?.(e.pointerId);
    const p = versImage(e);
    const s = sorte();
    if (espace || e.button === 1) {
      geste = { mode: "pan", x: e.clientX, y: e.clientY };
      scene.dataset.curseur = "grabbing";
      return;
    }
    // La poignée de l'objet choisi passe avant l'équerre de recadrage.
    const h = poigneeSous(p);
    const coin = h ? null : equerreSous(e);
    if (coin) {
      const k = ed.modele.cadre(ed.h.doc);
      geste = { mode: "equerre", coin, k0: { ...k }, cadre: { ...k } };
      ed.selection = [];
      ed.barre.synchroniser();
      ed.dessiner();
      return;
    }
    if (s === "gomme") {
      geste = {
        mode: "gomme",
        passages: new Map(), // id → [{ largeur, points }] : aperçu, posé au relâchement
        rayon: rayonGomme(),
        depart: p,
        p,
        ecran: [e.clientX, e.clientY],
        bouge: false,
      };
      ed.gommes = geste.passages;
      passer(p, p);
      placerRond(e);
      ed.dessiner();
      return;
    }
    if (!SANS_PRISE.has(s)) {
      if (h) {
        const o = ed.objet(ed.selection[0]);
        geste = {
          mode: "poignee",
          poignee: h,
          origine: copie(o),
          marque: `geste-${++numeroGeste}`,
          bouge: false,
        };
        return;
      }
      const o = ed.modele.objetSous(ed.h.doc, p, tolerance(), null);
      if (o) {
        if (s === "texte" && o.type === "texte" && !e.shiftKey) {
          ouvrirSaisie(o, true);
          return;
        }
        if (e.shiftKey) {
          const i = ed.selection.indexOf(o.id);
          if (i >= 0) ed.selection.splice(i, 1);
          else ed.selection.push(o.id);
        } else if (!ed.selection.includes(o.id)) ed.selection = [o.id];
        const origines = ed.objetsChoisis().map(copie);
        geste = {
          mode: "deplacer",
          p,
          ecran: [e.clientX, e.clientY],
          origines,
          limites: limites(origines),
          marque: `geste-${++numeroGeste}`,
          bouge: false,
        };
        ed.barre.synchroniser();
        ed.dessiner();
        return;
      }
    }
    ed.selection = [];
    ed.barre.synchroniser();
    if (!dansCadre(p)) {
      // Hors de l'image : rien n'est créé. Un geste commencé dans l'image peut finir dehors.
      ed.dessiner();
      return;
    }
    if (s === "numero") {
      poser({
        type: "numero",
        x: p.x,
        y: p.y,
        rayon: ed.dessin.RAYONS_NUMERO[ed.barre.rang()],
        couleur: ed.barre.couleur(),
        opacite: ed.barre.opacite() / 100,
      });
      return;
    }
    if (s === "texte") {
      const taille = TAILLES_TEXTE[ed.barre.rang()];
      ouvrirSaisie(
        {
          type: "texte",
          x: p.x,
          y: p.y - taille * 0.6,
          texte: "",
          couleur: ed.barre.couleur(),
          taille,
          gras: true,
          style: "contour",
          opacite: ed.barre.opacite() / 100,
        },
        false,
      );
      return;
    }
    ed.enCours = nouvelObjet(s, p);
    // `libre` : le trait à main levée du stylo ou du surligneur, gardé quand il est montré
    // droit (Maj, ou redressé).
    geste = { mode: "trace", libre: ed.enCours?.points ? [[p.x, p.y]] : null, droit: false };
    ed.dessiner();
  }

  function dansCadre(p) {
    const k = ed.modele.cadre(ed.h.doc);
    return p.x >= k.x && p.y >= k.y && p.x <= k.x + k.largeur && p.y <= k.y + k.hauteur;
  }

  // ---------- gomme ----------
  const rayonGomme = () => jetonNombre(`editeur-gomme-rayon-${ed.barre.rang("gomme")}`) / echelleCss();
  // La gomme passée de `a` à `b` : le passage est noté pour chaque encre atteinte. Un
  // passage se prolonge tant que la gomme reste sur l'encre ; revenue dessus, elle en ouvre
  // un autre (le chemin fait hors de l'encre n'est pas gardé).
  function passer(a, b) {
    const g = geste;
    for (const o of ed.h.doc.objets) {
      if (!ed.modele.gommeTouche(o, a, b, g.rayon)) continue;
      let l = g.passages.get(o.id);
      if (!l) g.passages.set(o.id, (l = []));
      const d = l[l.length - 1];
      const fin = d?.points[d.points.length - 1];
      if (fin && fin[0] === a.x && fin[1] === a.y) d.points.push([b.x, b.y]);
      else {
        const points = a.x === b.x && a.y === b.y ? [[a.x, a.y]] : [[a.x, a.y], [b.x, b.y]];
        l.push({ largeur: 2 * g.rayon, points });
      }
    }
  }
  // Fin du geste : un seul pas, pour ce qui a vraiment changé. Rien d'effacé et pas de
  // mouvement : le clic enlève l'objet entier sous la gomme, s'il y en a un.
  function finirGomme(g) {
    ed.gommes = null;
    const doc = ed.h.doc;
    const effets = [];
    for (const [id, passages] of g.passages) {
      const o = ed.objet(id);
      const effet = o ? ed.dessin.effetGomme(doc, o, passages) : "rien";
      if (effet !== "rien") effets.push([id, passages, effet]);
    }
    if (effets.length) {
      ed.modifier((d) => {
        const vides = effets.filter((x) => x[2] === "vide").map((x) => x[0]);
        for (const [id, passages, effet] of effets) {
          if (effet === "efface") ed.modele.gommer(d, id, passages);
        }
        if (vides.length) ed.modele.supprimer(d, vides);
      });
    } else if (!g.bouge) {
      const o = ed.modele.objetSous(doc, g.depart, tolerance(), ENTIERS);
      if (o) ed.modifier((d) => ed.modele.supprimer(d, [o.id]));
    }
    ed.dessiner();
  }
  // Le rond de la gomme, sous le pointeur, à la taille de la gomme (pixels de l'écran).
  function placerRond(e) {
    const r = scene.getBoundingClientRect();
    const d = 2 * jetonNombre(`editeur-gomme-rayon-${ed.barre.rang("gomme")}`);
    rond.style.width = rond.style.height = `${d}px`;
    rond.style.translate = `${e.clientX - r.left - d / 2}px ${e.clientY - r.top - d / 2}px`;
    rond.classList.toggle("appui", geste?.mode === "gomme");
    rond.hidden = false;
  }
  function cacherRond() {
    rond.hidden = true;
    rond.classList.remove("appui");
  }

  // ---------- déplacement borné ----------
  // Un objet déplacé reste à portée : au moins 12 px d'écran de sa boîte (ou toute sa boîte)
  // dans l'image. Rend les bornes de (dx, dy), ou null si les objets sont déjà dehors.
  function limites(origines) {
    if (!origines.length) return null;
    let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
    for (const o of origines) {
      const b = ed.modele.boiteForme(o);
      x0 = Math.min(x0, b.x); y0 = Math.min(y0, b.y);
      x1 = Math.max(x1, b.x + b.largeur); y1 = Math.max(y1, b.y + b.hauteur);
    }
    const k = ed.modele.cadre(ed.h.doc);
    const m = jetonNombre("editeur-deplacement-reste") / echelleCss();
    const mx = Math.min(m, x1 - x0, k.largeur), my = Math.min(m, y1 - y0, k.hauteur);
    const l = {
      dx: [k.x + mx - x1, k.x + k.largeur - mx - x0],
      dy: [k.y + my - y1, k.y + k.hauteur - my - y0],
    };
    return l.dx[0] > 0 || l.dx[1] < 0 || l.dy[0] > 0 || l.dy[1] < 0 ? null : l;
  }
  const borner = (l, dx, dy) =>
    l ? [clamp(dx, l.dx[0], l.dx[1]), clamp(dy, l.dy[0], l.dy[1])] : [dx, dy];

  function mouvement(e) {
    if (!ed.h) return;
    const p = versImage(e);
    const g = geste;
    dernier = g ? { clientX: e.clientX, clientY: e.clientY, target: e.target } : null;
    if (!g) {
      if (e.target === toile) curseur(e, p);
      return;
    }
    if (g.mode === "pan") {
      const dpr = ed.vue.dpr;
      ed.deplacerVue((e.clientX - g.x) * dpr, (e.clientY - g.y) * dpr);
      g.x = e.clientX;
      g.y = e.clientY;
      return;
    }
    if (g.mode === "equerre") {
      // Toute l'image reste à l'échelle du début du geste ; 40 px d'image au moins.
      const doc = ed.h.doc;
      const k = g.k0;
      const M = jetonNombre("editeur-recadrage-min");
      const px = clamp(p.x, 0, doc.largeur);
      const py = clamp(p.y, 0, doc.hauteur);
      let x0 = k.x;
      let y0 = k.y;
      let x1 = k.x + k.largeur;
      let y1 = k.y + k.hauteur;
      if (g.coin[1] === "w") x0 = Math.max(0, Math.min(px, x1 - M));
      else x1 = Math.min(doc.largeur, Math.max(px, x0 + M));
      if (g.coin[0] === "n") y0 = Math.max(0, Math.min(py, y1 - M));
      else y1 = Math.min(doc.hauteur, Math.max(py, y0 + M));
      x0 = Math.round(x0);
      y0 = Math.round(y0);
      g.cadre = { x: x0, y: y0, largeur: Math.round(x1) - x0, hauteur: Math.round(y1) - y0 };
      ed.dessiner();
      return;
    }
    if (g.mode === "gomme") {
      placerRond(e);
      const seuil = jetonNombre("editeur-deplacement-seuil");
      if (Math.hypot(e.clientX - g.ecran[0], e.clientY - g.ecran[1]) >= seuil) g.bouge = true;
      if (Math.hypot(p.x - g.p.x, p.y - g.p.y) < PAS_GOMME / echelleCss()) return;
      passer(g.p, p);
      g.p = p;
      ed.dessiner();
      return;
    }
    if (g.mode === "deplacer") {
      const seuil = jetonNombre("editeur-deplacement-seuil");
      if (!g.bouge && Math.hypot(e.clientX - g.ecran[0], e.clientY - g.ecran[1]) < seuil) return;
      const [dx, dy] = borner(g.limites, p.x - g.p.x, p.y - g.p.y);
      g.bouge = true;
      ed.modifier((doc) => {
        for (const q of g.origines) {
          const o = doc.objets.find((x) => x.id === q.id);
          if (o) ed.modele.deplacer(o, q, dx, dy);
        }
      }, g.marque);
      return;
    }
    if (g.mode === "poignee") {
      g.bouge = true;
      ed.modifier((doc) => {
        const o = doc.objets.find((x) => x.id === g.origine.id);
        if (o) ed.modele.redimensionner(o, g.origine, g.poignee, p, echelleCss(), !!e.shiftKey);
      }, g.marque);
      return;
    }
    const c = ed.enCours;
    if (!c) return;
    if (c.points) {
      const l = g.libre[g.libre.length - 1];
      const avance = Math.hypot(p.x - l[0], p.y - l[1]) > 2 / echelleCss();
      if (avance) g.libre.push([p.x, p.y]);
      // Maj tenue, ou trait redressé : un segment du départ à la pointe (Maj : par 15°).
      if (e.shiftKey || g.droit) {
        const d = g.libre[0];
        const bout = { x1: d[0], y1: d[1] };
        if (e.shiftKey) contraindre(Object.assign(bout, { type: "ligne" }), p, true);
        else Object.assign(bout, { x2: p.x, y2: p.y });
        c.points = [d, [bout.x2, bout.y2]];
      } else c.points = g.libre;
      g.maj = !!e.shiftKey;
      if (avance) attendreRedressement(g);
    } else {
      contraindre(c, p, e.shiftKey);
      if (c.courbe) {
        const dx = c.x2 - c.x1;
        const dy = c.y2 - c.y1;
        c.cx = (c.x1 + c.x2) / 2 - dy * 0.22;
        c.cy = (c.y1 + c.y2) / 2 + dx * 0.22;
      }
    }
    ed.dessiner();
  }

  // Stylo et surligneur : le pointeur resté immobile en fin de trait redresse un trait
  // presque droit (comme sur une tablette) ; ensuite la pointe mène le bout du segment.
  function attendreRedressement(g) {
    clearTimeout(g.minuterie);
    if (g.droit) return;
    g.minuterie = setTimeout(() => {
      const c = ed.enCours;
      if (geste !== g || !c?.points || g.droit || g.maj) return;
      const l = g.libre, a = l[0], b = l[l.length - 1];
      const L = Math.hypot(b[0] - a[0], b[1] - a[1]);
      if (L < jetonNombre("editeur-redresser-min") / echelleCss()) return;
      let ecart = 0;
      for (const q of l) {
        ecart = Math.max(ecart,
          Math.abs((b[0] - a[0]) * (a[1] - q[1]) - (a[0] - q[0]) * (b[1] - a[1])) / L);
      }
      if (ecart > L * DROIT_ECART) return;
      g.droit = true;
      // Presque horizontal ou vertical : calé sur l'axe.
      const an = Math.atan2(b[1] - a[1], b[0] - a[0]);
      const axe = Math.round(an / (Math.PI / 2)) * (Math.PI / 2);
      const bout = Math.abs(an - axe) < DROIT_AXE
        ? [a[0] + Math.cos(axe) * L, a[1] + Math.sin(axe) * L] : b;
      c.points = [a, bout];
      ed.dessiner();
    }, jetonNombre("duree-editeur-redresser"));
  }

  // Maj : 15° pour flèche et ligne, carré pour les boîtes.
  function contraindre(c, p, maj) {
    let { x, y } = p;
    const trait = c.type === "fleche" || c.type === "ligne";
    if (maj && trait) {
      const a = Math.round(Math.atan2(y - c.y1, x - c.x1) / ANGLE) * ANGLE;
      const l = Math.hypot(x - c.x1, y - c.y1);
      x = c.x1 + Math.cos(a) * l;
      y = c.y1 + Math.sin(a) * l;
    } else if (maj && RECTANGULAIRES.has(c.type)) {
      const m = Math.max(Math.abs(x - c.x1), Math.abs(y - c.y1));
      x = c.x1 + Math.sign(x - c.x1 || 1) * m;
      y = c.y1 + Math.sign(y - c.y1 || 1) * m;
    }
    c.x2 = x;
    c.y2 = y;
  }

  function relache() {
    const g = geste;
    geste = null;
    dernier = null;
    if (!g || !ed.h) return;
    clearTimeout(g.minuterie);
    if (g.mode === "pan") {
      scene.dataset.curseur = espace ? "grab" : "";
      ed.envoyerVue();
      return;
    }
    if (g.mode === "gomme") {
      rond.classList.remove("appui");
      finirGomme(g);
      return;
    }
    if (g.mode === "deplacer" || g.mode === "poignee") {
      if (g.bouge) {
        const ids = ed.selection.slice();
        ed.modifier((doc) => {
          for (const o of doc.objets) if (ids.includes(o.id)) ed.modele.normaliser(o);
        }, g.marque);
      }
      ed.dessiner();
      return;
    }
    if (g.mode === "equerre") {
      // Un pas d'annulation ; l'image entière (à 1 px près) retire le recadrage.
      const c = g.cadre;
      const k = g.k0;
      const doc = ed.h.doc;
      const entiere =
        c.x <= 1 && c.y <= 1 && c.largeur >= doc.largeur - 2 && c.hauteur >= doc.hauteur - 2;
      const pareil =
        c.x === k.x && c.y === k.y && c.largeur === k.largeur && c.hauteur === k.hauteur;
      if (!pareil && !(entiere && !doc.recadrage)) {
        ed.modifier((d) => ed.modele.recadrer(d, entiere ? null : c));
      }
      ed.dessiner();
      return;
    }
    const c = ed.enCours;
    ed.enCours = null;
    if (!c) return;
    if (c.points) {
      if (c.type === "crayon") c.points = ed.modele.lisser(c.points);
      poser(c);
      return;
    }
    const assez =
      c.type === "fleche" || c.type === "ligne"
        ? Math.hypot(c.x2 - c.x1, c.y2 - c.y1) > MIN_TRAIT
        : Math.abs(c.x2 - c.x1) > MIN_BOITE && Math.abs(c.y2 - c.y1) > MIN_BOITE;
    if (!assez) {
      ed.dessiner();
      return;
    }
    ed.modele.normaliser(c);
    delete c.courbe;
    poser(c);
  }

  // Échap pendant un geste : il est abandonné. Rien n'est créé ni gommé, le recadrage est
  // laissé, un déplacement ou une poignée tirée reviennent au départ (sans rien à rétablir).
  function abandonnerGeste() {
    const g = geste;
    if (!g) return false;
    geste = null;
    dernier = null;
    clearTimeout(g.minuterie);
    ed.enCours = null;
    ed.gommes = null;
    rond.classList.remove("appui");
    if (g.mode === "pan") {
      scene.dataset.curseur = espace ? "grab" : "";
      ed.envoyerVue();
    } else if ((g.mode === "deplacer" || g.mode === "poignee") && g.bouge) ed.abandonner();
    ed.dessiner();
    return true;
  }

  function curseur(e, p) {
    survol = { clientX: e.clientX, clientY: e.clientY };
    if (espace) {
      scene.dataset.curseur = "grab";
      cacherRond();
      return;
    }
    const coin = poigneeSous(p) ? null : equerreSous(e);
    if (coin) {
      scene.dataset.curseur = coin === "nw" || coin === "se" ? "nwse" : "nesw";
      cacherRond();
      return;
    }
    const s = sorte();
    let c = "";
    if (s === "gomme") {
      c = "gomme";
      placerRond(e);
    } else cacherRond();
    if (!SANS_PRISE.has(s)) {
      const h = poigneeSous(p);
      if (h) c = h === "nw" || h === "se" ? "nwse" : h === "ne" || h === "sw" ? "nesw" : "pt";
      else {
        const o = ed.modele.objetSous(ed.h.doc, p, tolerance(), null);
        if (o) c = s === "texte" && o.type === "texte" ? "text" : "move";
        else if (s === "texte") c = "text";
      }
    }
    scene.dataset.curseur = c;
  }

  toile.addEventListener("pointerdown", appui);
  scene.addEventListener("pointerdown", (e) => {
    if (e.target !== scene) return;
    if (espace) appui(e);
    else if (ed.selection.length && !edition && !ed.barre.menuOuvert()) {
      ed.selection = [];
      ed.barre.synchroniser();
      ed.dessiner();
    }
  });
  window.addEventListener("pointermove", (e) => {
    if (geste || ed.enCours || e.target === toile) mouvement(e);
  });
  window.addEventListener("pointerup", relache);
  window.addEventListener("pointercancel", relache);
  toile.addEventListener("pointerleave", () => {
    survol = null;
    if (geste?.mode !== "gomme") cacherRond();
  });
  // Maj enfoncée ou relâchée pendant un geste : il suit tout de suite, sans attendre le
  // prochain mouvement.
  for (const n of ["keydown", "keyup"]) {
    window.addEventListener(n, (e) => {
      if (e.key !== "Shift" || e.repeat || !geste || !dernier) return;
      mouvement({ ...dernier, shiftKey: n === "keydown" });
    });
  }
  // Capture perdue ou fenêtre quittée : le geste finit (gardé s'il a une taille).
  toile.addEventListener("lostpointercapture", () => {
    if (geste) relache();
  });
  window.addEventListener("blur", () => {
    if (geste) relache();
  });
  toile.addEventListener("dblclick", (e) => {
    // Stylo, surligneur, gomme : deux appuis sont deux gestes, pas une saisie.
    if (!ed.h || SANS_PRISE.has(sorte())) return;
    const o = ed.modele.objetSous(ed.h.doc, versImage(e), tolerance(), "texte");
    if (o) ouvrirSaisie(o, true);
  });
  toile.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    if (!ed.h) return;
    const o = ed.modele.objetSous(ed.h.doc, versImage(e), tolerance(), null);
    if (!o) {
      ed.barre.fermerMenus();
      return;
    }
    if (!ed.selection.includes(o.id)) ed.selection = [o.id];
    ed.barre.synchroniser();
    ed.dessiner();
    ed.barre.ouvrirMenuObjet(e);
  });
  // Ctrl + molette : zoom à la pointe (le zoom du navigateur est empêché par barre.js).
  scene.addEventListener(
    "wheel",
    (e) => {
      if (!(e.ctrlKey || e.metaKey) || !ed.h) return;
      e.preventDefault();
      const r = scene.getBoundingClientRect();
      ed.zoomer(
        ed.vue.echelle * Math.exp(-e.deltaY * 0.0018),
        e.clientX - r.left,
        e.clientY - r.top,
      );
    },
    { passive: false },
  );

  // ---------- texte en place ----------
  // La zone de saisie a la place, la police et la boîte du texte dessiné (mesureTexte). Son
  // texte est transparent : la vue dessine le texte en saisie (`ajout` de peindreVue), avec
  // les couleurs de dessin.js ; la zone ne montre que le curseur et la sélection.
  function placerSaisie() {
    if (!edition) return;
    const o = edition.objet;
    const s = echelleCss();
    const [tx, ty] = versToile(o.x, o.y);
    const dpr = ed.vue.dpr;
    const t = o.taille * s;
    const m = ed.modele.mesureTexte({ ...o, texte: saisie.value || "M" });
    saisie.style.left = `${tx / dpr}px`;
    saisie.style.top = `${ty / dpr}px`;
    saisie.style.font = ed.modele.policeTexte({ ...o, taille: t });
    saisie.style.padding = `${m.py * s}px ${m.px * s}px`;
    const marge = o.style === "etiquette" ? 0 : jetonNombre("editeur-saisie-marge");
    saisie.style.width = `${m.largeur * s + marge}px`;
    saisie.style.height = `${m.hauteur * s}px`;
    ed.dessiner();
  }
  // Le texte en saisie tel que la vue le dessine (null : rien à dessiner).
  function texteEnSaisie() {
    if (!edition || !saisie.value) return null;
    return { ...edition.objet, texte: saisie.value };
  }
  function ouvrirSaisie(o, existant) {
    ed.selection = [];
    edition = { objet: copie(o), existant };
    saisie.value = o.texte || "";
    saisie.hidden = false;
    placerSaisie();
    ed.barre.synchroniser();
    ed.dessiner();
    saisie.focus();
    saisie.select();
  }
  // Entrée : gardé ; texte vide : rien de gardé (un texte existant vidé est supprimé).
  function validerSaisie() {
    const e = edition;
    if (!e) return;
    edition = null;
    saisie.hidden = true;
    const v = saisie.value.trim();
    if (e.existant) {
      const o = ed.objet(e.objet.id);
      if (o && v !== o.texte) {
        ed.modifier((doc) => {
          if (v) doc.objets.find((x) => x.id === o.id).texte = v;
          else doc.objets = doc.objets.filter((x) => x.id !== o.id);
        });
      }
    } else if (v) {
      poser({ ...e.objet, texte: v });
    }
    ed.barre.synchroniser();
    ed.dessiner();
  }
  function annulerSaisie() {
    if (!edition) return false;
    edition = null;
    saisie.hidden = true;
    ed.barre.synchroniser();
    ed.dessiner();
    scene.focus({ preventScroll: true });
    return true;
  }
  saisie.addEventListener("input", placerSaisie);
  saisie.addEventListener("keydown", (e) => {
    e.stopPropagation();
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      validerSaisie();
      scene.focus({ preventScroll: true });
    } else if (e.key === "Escape") {
      e.preventDefault();
      annulerSaisie();
    }
  });
  saisie.addEventListener("blur", () => {
    if (edition) validerSaisie();
  });

  // ---------- opérations sur la sélection ----------
  function dupliquer() {
    if (!ed.selection.length) return;
    let ids = [];
    const choisis = ed.selection.slice();
    ed.modifier((doc) => {
      ids = ed.modele.dupliquer(doc, choisis);
    });
    if (ids?.length) ed.selection = ids;
    ed.barre.synchroniser();
  }
  function supprimer() {
    const ids = ed.selection.slice();
    if (!ids.length) return;
    ed.modifier((doc) => ed.modele.supprimer(doc, ids));
    ed.selection = [];
    ed.barre.synchroniser();
  }
  function ordonner(devant) {
    const ids = ed.selection.slice();
    if (!ids.length) return;
    ed.modifier((doc) => ed.modele.ordonner(doc, ids, devant));
  }
  // Flèches du clavier : un pas d'annulation pour une suite de poussées.
  let marquePousser = null;
  let dernierePoussee = 0;
  function pousser(dx, dy) {
    const ids = ed.selection.slice();
    if (!ids.length) return;
    const maintenant = performance.now();
    if (!marquePousser || maintenant - dernierePoussee > 1000) {
      marquePousser = `pousser-${++numeroGeste}`;
    }
    dernierePoussee = maintenant;
    const [bx, by] = borner(limites(ed.objetsChoisis()), dx, dy);
    if (!bx && !by) return;
    ed.modifier((doc) => {
      for (const o of doc.objets) if (ids.includes(o.id)) ed.modele.deplacer(o, copie(o), bx, by);
    }, marquePousser);
  }
  function toutChoisir() {
    if (!ed.h || geste || edition) return;
    ed.selection = ed.h.doc.objets.map((o) => o.id);
    ed.barre.synchroniser();
    ed.dessiner();
  }
  // L'outil a changé (clavier compris) : le curseur et le rond de la gomme suivent, sans
  // attendre un mouvement.
  function outilChange() {
    if (geste || !ed.h) return;
    if (!survol) cacherRond();
    else curseur(survol, versImage(survol));
  }

  // ---------- surcouche de la vue ----------
  // Équerres des coins, recadrage en cours, poignées : dessinés par la page, en pixels de la
  // toile (mesures d'écran × dpr), couleurs des jetons.
  function equerres(ctx, x, y, w, h) {
    const n = (nom) => jetonNombre(nom) * ed.vue.dpr;
    const L = Math.min(n("editeur-equerre-longueur"), w / 3, h / 3);
    const g = n("editeur-equerre-ecart");
    const [X0, Y0, X1, Y1] = [x - g, y - g, x + w + g, y + h + g];
    ctx.save();
    ctx.strokeStyle = jeton("editeur-equerre");
    ctx.lineWidth = n("editeur-equerre-trait");
    ctx.lineCap = "round";
    ctx.lineJoin = "round";
    ctx.shadowColor = jeton("editeur-equerre-ombre");
    ctx.shadowBlur = n("editeur-equerre-ombre-flou");
    ctx.beginPath();
    ctx.moveTo(X0, Y0 + L);
    ctx.lineTo(X0, Y0);
    ctx.lineTo(X0 + L, Y0);
    ctx.moveTo(X1 - L, Y0);
    ctx.lineTo(X1, Y0);
    ctx.lineTo(X1, Y0 + L);
    ctx.moveTo(X1, Y1 - L);
    ctx.lineTo(X1, Y1);
    ctx.lineTo(X1 - L, Y1);
    ctx.moveTo(X0 + L, Y1);
    ctx.lineTo(X0, Y1);
    ctx.lineTo(X0, Y1 - L);
    ctx.stroke();
    ctx.restore();
  }
  // Les poignées ne sont montrées que pour un seul objet choisi, avec un outil qui prend.
  const poignees = () => !edition && ed.selection.length === 1 && !SANS_PRISE.has(sorte());
  ed.surcouche = (ctx) => {
    const dpr = ed.vue.dpr;
    const n = (nom) => jetonNombre(nom) * dpr;
    const acc = jeton("accent");
    ctx.save();
    if (geste?.mode === "equerre") {
      const doc = ed.h.doc;
      const c = geste.cadre;
      const [ix, iy] = versToile(0, 0);
      const [jx, jy] = versToile(doc.largeur, doc.hauteur);
      const [ax, ay] = versToile(c.x, c.y);
      const [bx, by] = versToile(c.x + c.largeur, c.y + c.hauteur);
      ctx.fillStyle = jeton("editeur-scene");
      ctx.globalAlpha = jetonNombre("editeur-recadrage-voile");
      ctx.beginPath();
      ctx.rect(ix, iy, jx - ix, jy - iy);
      ctx.rect(ax, ay, bx - ax, by - ay);
      ctx.fill("evenodd");
      ctx.globalAlpha = 1;
      ctx.lineWidth = dpr;
      ctx.strokeStyle = jeton("editeur-recadrage-cadre");
      ctx.strokeRect(ax + dpr / 2, ay + dpr / 2, bx - ax - dpr, by - ay - dpr);
      ctx.strokeStyle = jeton("editeur-recadrage-tiers");
      ctx.beginPath();
      for (let i = 1; i < 3; i++) {
        ctx.moveTo(ax + ((bx - ax) * i) / 3, ay);
        ctx.lineTo(ax + ((bx - ax) * i) / 3, by);
        ctx.moveTo(ax, ay + ((by - ay) * i) / 3);
        ctx.lineTo(bx, ay + ((by - ay) * i) / 3);
      }
      ctx.stroke();
      equerres(ctx, ax, ay, bx - ax, by - ay);
      ctx.restore();
      return;
    }
    const k = ed.modele.cadre(ed.h.doc);
    const [ax, ay] = versToile(k.x, k.y);
    const [bx, by] = versToile(k.x + k.largeur, k.y + k.hauteur);
    equerres(ctx, Math.round(ax), Math.round(ay), Math.round(bx - ax), Math.round(by - ay));
    if (poignees()) {
      const o = ed.objet(ed.selection[0]);
      const rayon = n("editeur-poignee-rayon");
      for (const [x, y] of o ? ed.modele.poignees(o) : []) {
        const [px, py] = versToile(x, y);
        ctx.beginPath();
        ctx.arc(px, py, rayon, 0, 2 * Math.PI);
        ctx.shadowColor = jeton("editeur-poignee-ombre-couleur");
        ctx.shadowBlur = n("editeur-poignee-ombre");
        ctx.fillStyle = jeton("editeur-poignee");
        ctx.fill();
        ctx.shadowColor = "transparent";
        ctx.lineWidth = n("editeur-poignee-trait");
        ctx.strokeStyle = acc;
        ctx.stroke();
      }
    }
    ctx.restore();
  };

  return {
    validerSaisie,
    annulerSaisie,
    saisieOuverte: () => !!edition,
    // Un geste en cours (bouton enfoncé) : ni annuler ni rétablir.
    gesteEnCours: () => !!geste,
    recadrageEnCours: () => geste?.mode === "equerre",
    abandonnerGeste,
    poignees,
    toutChoisir,
    outilChange,
    idEnEdition: () => (edition?.existant ? edition.objet.id : null),
    texteEnSaisie,
    placerSaisie,
    dupliquer,
    supprimer,
    ordonner,
    pousser,
    espace: (oui) => {
      espace = oui;
      if (!geste) scene.dataset.curseur = oui ? "grab" : "";
    },
    remiseAZero: () => {
      clearTimeout(geste?.minuterie);
      geste = null;
      dernier = null;
      survol = null;
      ed.gommes = null;
      cacherRond();
      edition = null;
      saisie.hidden = true;
      ed.enCours = null;
    },
    sorteObjet,
  };
}
