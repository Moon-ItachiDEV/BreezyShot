// Gestes de chaque outil sur la scène (contrat 0.4 « Éditeur », §3.3) : tracés, sélection,
// déplacement et poignées, texte en place, numéros, masques, recadrage, pipette, vue à la
// main. Tout changement du document passe par `ed.modifier` (document.js) ; la page dessine
// par-dessus la vue les poignées, le cadre de sélection, le recadrage en cours et la pipette.

import { jeton, jetonNombre } from "./editeur.js";
import { sorteObjet, TYPE_DE_SORTE, SORTE_DE_MASQUE } from "./barre.js";

const $ = (s) => document.querySelector(s);
const clamp = (v, a, b) => Math.max(a, Math.min(b, v));
const RECTANGULAIRES = new Set(["rectangle", "ellipse", "masque"]);
const RAPPORTS = new Map([
  [1, [1, 1]],
  [4 / 3, [4, 3]],
  [16 / 9, [16, 9]],
]);
const MIN_TRAIT = 8; // flèche ou ligne : ≤ 8 px, rien n'est créé
const MIN_BOITE = 6; // autre : ≤ 6 px
const ANGLE = Math.PI / 12; // Maj : 15°

const copie = (o) => JSON.parse(JSON.stringify(o));
const luminance = (hex) => {
  const n = parseInt(hex.slice(1), 16);
  return (0.299 * ((n >> 16) & 255) + 0.587 * ((n >> 8) & 255) + 0.114 * (n & 255)) / 255;
};

export function installerOutils(ed) {
  const toile = ed.toile;
  const scene = ed.scene;
  const saisie = $("#saisie");
  let geste = null; // { mode: pan|cadre|deplacer|poignee, … }
  let espace = false;
  let pipette = false;
  let survolPipette = null;
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

  function filtre() {
    const s = sorte();
    return s === "selection" ? null : (TYPE_DE_SORTE[s] ?? null);
  }
  function poigneeSous(p) {
    if (ed.selection.length !== 1) return null;
    const o = ed.objet(ed.selection[0]);
    if (!o) return null;
    const r = jetonNombre("editeur-poignee-prise") / echelleCss();
    for (const [x, y, nom] of ed.modele.poignees(o)) {
      if (Math.hypot(p.x - x, p.y - y) < r) return nom;
    }
    return null;
  }

  // ---------- nouveaux objets ----------
  function nouvelObjet(s, p) {
    const pr = ed.prefs;
    const couleur = ed.barre.couleur(s);
    const opacite = pr.opacite / 100;
    const base = { x1: p.x, y1: p.y, x2: p.x, y2: p.y };
    switch (TYPE_DE_SORTE[s]) {
      case "fleche":
        return {
          type: "fleche",
          ...base,
          double: s === "double",
          courbe: s === "courbe",
          couleur,
          epaisseur: pr.epaisseur,
          opacite,
        };
      case "rectangle":
        return {
          type: "rectangle",
          ...base,
          couleur,
          epaisseur: pr.epaisseur,
          plein: pr.plein,
          arrondi: pr.arrondi,
          opacite,
        };
      case "ellipse":
        return {
          type: "ellipse",
          ...base,
          couleur,
          epaisseur: pr.epaisseur,
          plein: pr.plein,
          opacite,
        };
      case "ligne":
        return { type: "ligne", ...base, couleur, epaisseur: pr.epaisseur, opacite };
      case "crayon":
        return { type: "crayon", points: [[p.x, p.y]], couleur, epaisseur: pr.epaisseur, opacite };
      case "surligneur":
        return { type: "surligneur", points: [[p.x, p.y]], couleur, epaisseur: pr.epaisseur };
      case "masque":
        return { type: "masque", ...base, sorte: SORTE_DE_MASQUE[s], intensite: pr.intensite };
      default:
        return null;
    }
  }
  // Pose `o` dans le document (un pas ; `ajouter` donne l'`id`, et l'`ordre` d'un numéro),
  // et le choisit.
  function poser(o) {
    let id = 0;
    ed.modifier((doc) => {
      id = ed.modele.ajouter(doc, o);
    });
    ed.selection = [id];
    ed.barre.synchroniser();
    ed.dessiner();
    return id;
  }

  // ---------- pointeur ----------
  function appui(e) {
    if (e.button === 2) return;
    if (e.button !== 0 && e.button !== 1) return;
    e.preventDefault();
    scene.focus({ preventScroll: true });
    if (!ed.h || !ed.fond) return;
    if (edition) {
      validerSaisie();
      return;
    }
    if (ed.barre.menuOuvert() && !espace) ed.barre.fermerMenus();
    toile.setPointerCapture?.(e.pointerId);
    const p = versImage(e);
    const s = sorte();
    if (espace || e.button === 1) {
      geste = { mode: "pan", x: e.clientX, y: e.clientY };
      scene.dataset.curseur = "grabbing";
      return;
    }
    if (pipette) {
      const c = couleurSous(e);
      if (c) ed.barre.regler("couleur", c);
      arreterPipette();
      return;
    }
    if (s !== "recadrer") {
      const h = poigneeSous(p);
      if (h) {
        const o = ed.objet(ed.selection[0]);
        geste = {
          mode: "poignee",
          poignee: h,
          origine: copie(o),
          marque: `geste-${++numeroGeste}`,
        };
        ed.barre.tracer(true);
        return;
      }
      const o = ed.modele.objetSous(ed.h.doc, p, tolerance(), filtre());
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
        geste = {
          mode: "deplacer",
          p,
          origines: ed.objetsChoisis().map(copie),
          marque: `geste-${++numeroGeste}`,
          bouge: false,
        };
        ed.barre.synchroniser();
        ed.dessiner();
        return;
      }
    }
    if (!e.shiftKey) ed.selection = [];
    if (s === "selection") {
      geste = {
        mode: "cadre",
        x0: p.x,
        y0: p.y,
        x1: p.x,
        y1: p.y,
        ajout: e.shiftKey,
        base: ed.selection.slice(),
      };
      ed.barre.synchroniser();
      ed.dessiner();
      return;
    }
    ed.selection = [];
    if (s === "numeros") {
      poser({
        type: "numero",
        x: p.x,
        y: p.y,
        rayon: ed.dessin.RAYONS_NUMERO[ed.prefs.numero],
        couleur: ed.barre.couleur(s),
        opacite: ed.prefs.opacite / 100,
      });
      return;
    }
    if (s === "texte") {
      const pr = ed.prefs;
      ouvrirSaisie(
        {
          type: "texte",
          x: p.x,
          y: p.y - pr.tailleTexte * 0.6,
          texte: "",
          couleur: ed.barre.couleur(s),
          taille: pr.tailleTexte,
          gras: pr.gras,
          style: pr.styleTexte,
          opacite: pr.opacite / 100,
        },
        false,
      );
      return;
    }
    if (s === "recadrer") {
      geste = { mode: "recadrage", x1: p.x, y1: p.y, x2: p.x, y2: p.y };
    } else {
      ed.enCours = nouvelObjet(s, p);
      geste = { mode: "trace" };
    }
    ed.barre.tracer(true);
    ed.dessiner();
  }

  function mouvement(e) {
    if (!ed.h) return;
    const p = versImage(e);
    if (pipette) {
      const r = scene.getBoundingClientRect();
      const c = couleurSous(e);
      survolPipette = c ? { x: e.clientX - r.left, y: e.clientY - r.top, c } : null;
      ed.dessiner();
      return;
    }
    const g = geste;
    if (!g) {
      if (e.target === toile) curseur(p);
      return;
    }
    if (g.mode === "pan") {
      const dpr = ed.vue.dpr;
      ed.deplacerVue((e.clientX - g.x) * dpr, (e.clientY - g.y) * dpr);
      g.x = e.clientX;
      g.y = e.clientY;
      return;
    }
    if (g.mode === "cadre") {
      g.x1 = p.x;
      g.y1 = p.y;
      const m = {
        x: Math.min(g.x0, g.x1),
        y: Math.min(g.y0, g.y1),
        l: Math.abs(g.x1 - g.x0),
        h: Math.abs(g.y1 - g.y0),
      };
      const ids = ed.h.doc.objets
        .filter((o) => {
          const b = ed.modele.boite(o);
          return (
            b.x < m.x + m.l && b.x + b.largeur > m.x && b.y < m.y + m.h && b.y + b.hauteur > m.y
          );
        })
        .map((o) => o.id);
      ed.selection = g.ajout ? g.base.concat(ids.filter((i) => !g.base.includes(i))) : ids;
      ed.dessiner();
      return;
    }
    if (g.mode === "deplacer") {
      const dx = p.x - g.p.x;
      const dy = p.y - g.p.y;
      if (!g.bouge && Math.hypot(dx, dy) * echelleCss() < 1) return;
      if (!g.bouge) ed.barre.tracer(true);
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
        if (o) ed.modele.redimensionner(o, g.origine, g.poignee, p, echelleCss());
      }, g.marque);
      return;
    }
    if (g.mode === "recadrage") {
      contraindre(g, p, e.shiftKey, true);
      ed.dessiner();
      return;
    }
    const c = ed.enCours;
    if (!c) return;
    if (c.points) {
      const l = c.points[c.points.length - 1];
      if (Math.hypot(p.x - l[0], p.y - l[1]) > 2 / echelleCss()) c.points.push([p.x, p.y]);
    } else {
      contraindre(c, p, e.shiftKey, false);
      if (c.courbe) {
        const dx = c.x2 - c.x1;
        const dy = c.y2 - c.y1;
        c.cx = (c.x1 + c.x2) / 2 - dy * 0.22;
        c.cy = (c.y1 + c.y2) / 2 + dx * 0.22;
      }
    }
    ed.dessiner();
  }

  // Maj : 15° pour flèche et ligne, carré pour les boîtes ; Recadrer suit son rapport et
  // reste dans le cadre.
  function contraindre(c, p, maj, recadrage) {
    let { x, y } = p;
    const trait = c.type === "fleche" || c.type === "ligne";
    if (maj && trait) {
      const a = Math.round(Math.atan2(y - c.y1, x - c.x1) / ANGLE) * ANGLE;
      const l = Math.hypot(x - c.x1, y - c.y1);
      x = c.x1 + Math.cos(a) * l;
      y = c.y1 + Math.sin(a) * l;
    } else if (recadrage && ed.prefs.ratio) {
      // Le rapport tenu jusqu'au bord du cadre.
      const r = ed.prefs.ratio;
      const k = ed.modele.cadre(ed.h.doc);
      const sx = Math.sign(x - c.x1 || 1);
      const sy = Math.sign(y - c.y1 || 1);
      const lx = sx > 0 ? k.x + k.largeur - c.x1 : c.x1 - k.x;
      const ly = sy > 0 ? k.y + k.hauteur - c.y1 : c.y1 - k.y;
      const l = Math.max(0, Math.min(Math.abs(x - c.x1), lx, ly * r));
      x = c.x1 + sx * l;
      y = c.y1 + (sy * l) / r;
    } else if (maj && (recadrage || RECTANGULAIRES.has(c.type))) {
      const m = Math.max(Math.abs(x - c.x1), Math.abs(y - c.y1));
      x = c.x1 + Math.sign(x - c.x1 || 1) * m;
      y = c.y1 + Math.sign(y - c.y1 || 1) * m;
    }
    if (recadrage) {
      const k = ed.modele.cadre(ed.h.doc);
      x = clamp(x, k.x, k.x + k.largeur);
      y = clamp(y, k.y, k.y + k.hauteur);
    }
    c.x2 = x;
    c.y2 = y;
  }

  function relache() {
    const g = geste;
    geste = null;
    ed.barre.tracer(false);
    if (!g || !ed.h) return;
    if (g.mode === "pan") {
      scene.dataset.curseur = espace ? "grab" : "";
      ed.envoyerVue();
      return;
    }
    if (g.mode === "cadre") {
      ed.barre.synchroniser();
      ed.dessiner();
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
    if (g.mode === "recadrage") {
      const k = ed.modele.cadre(ed.h.doc);
      let x = Math.round(Math.max(Math.min(g.x1, g.x2), k.x));
      let y = Math.round(Math.max(Math.min(g.y1, g.y2), k.y));
      let l = Math.round(Math.min(Math.max(g.x1, g.x2), k.x + k.largeur)) - x;
      let h = Math.round(Math.min(Math.max(g.y1, g.y2), k.y + k.hauteur)) - y;
      // Rapport exact en pixels entiers (1:1, 4:3, 16:9), ancré au point de départ.
      const rapport = RAPPORTS.get(ed.prefs.ratio);
      if (rapport) {
        const n = Math.min(Math.floor(l / rapport[0]), Math.floor(h / rapport[1]));
        if (g.x2 < g.x1) x += l - n * rapport[0];
        if (g.y2 < g.y1) y += h - n * rapport[1];
        l = n * rapport[0];
        h = n * rapport[1];
      }
      if (l > MIN_BOITE && h > MIN_BOITE) {
        ed.modifier((doc) => ed.modele.recadrer(doc, { x, y, largeur: l, hauteur: h }));
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

  function curseur(p) {
    if (espace) {
      scene.dataset.curseur = "grab";
      return;
    }
    if (pipette) {
      scene.dataset.curseur = "pick";
      return;
    }
    const s = sorte();
    let c = "";
    if (s !== "recadrer") {
      const h = poigneeSous(p);
      if (h) c = h === "nw" || h === "se" ? "nwse" : h === "ne" || h === "sw" ? "nesw" : "pt";
      else {
        const o = ed.modele.objetSous(ed.h.doc, p, tolerance(), filtre());
        if (o) c = s === "texte" && o.type === "texte" ? "text" : "move";
        else if (s === "texte") c = "text";
        else if (s === "selection") c = "def";
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
    if (geste || ed.enCours || pipette || e.target === toile) mouvement(e);
  });
  window.addEventListener("pointerup", relache);
  window.addEventListener("pointercancel", relache);
  toile.addEventListener("dblclick", (e) => {
    if (!ed.h) return;
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

  // ---------- pipette ----------
  function couleurSous(e) {
    const r = toile.getBoundingClientRect();
    const x = Math.floor((e.clientX - r.left) * ed.vue.dpr);
    const y = Math.floor((e.clientY - r.top) * ed.vue.dpr);
    const p = versImage(e);
    const k = ed.modele.cadre(ed.h.doc);
    if (p.x < k.x || p.y < k.y || p.x >= k.x + k.largeur || p.y >= k.y + k.hauteur) return null;
    if (x < 0 || y < 0 || x >= toile.width || y >= toile.height) return null;
    const d = ed.ctx.getImageData(x, y, 1, 1).data;
    const hex = [d[0], d[1], d[2]].map((v) => v.toString(16).padStart(2, "0")).join("");
    return `#${hex}`.toUpperCase();
  }
  function basculerPipette() {
    if (pipette) arreterPipette();
    else {
      pipette = true;
      scene.dataset.curseur = "pick";
      ed.barre.fermerMenus();
    }
    $("#bouton-pipette")?.setAttribute("aria-pressed", pipette ? "true" : "false");
  }
  function arreterPipette() {
    if (!pipette) return;
    pipette = false;
    survolPipette = null;
    scene.dataset.curseur = "";
    $("#bouton-pipette")?.setAttribute("aria-pressed", "false");
    ed.dessiner();
  }

  // ---------- texte en place ----------
  // La zone de saisie a la place, la police et la boîte du texte dessiné (mesureTexte).
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
    const etiquette = o.style === "etiquette";
    saisie.style.padding = `${m.py * s}px ${m.px * s}px`;
    saisie.style.width = `${m.largeur * s + (etiquette ? 0 : 8)}px`;
    saisie.style.height = `${m.hauteur * s}px`;
    const clair = luminance(o.couleur) > 0.62;
    if (etiquette) {
      saisie.style.background = o.couleur;
      saisie.style.color = clair ? "#1A1A1A" : "#FFFFFF";
      saisie.style.borderRadius = `${t * 0.32}px`;
      saisie.style.textShadow = "none";
    } else {
      saisie.style.background = "transparent";
      saisie.style.color = o.couleur;
      saisie.style.borderRadius = "";
      saisie.style.textShadow =
        o.style === "contour"
          ? clair
            ? "0 0 2px #222, 0 0 2px #222"
            : "0 0 2px #fff, 0 0 2px #fff, 0 0 3px #fff"
          : "0 1px 3px rgba(0,0,0,.3)";
    }
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
        if (v) ed.selection = [o.id];
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
    ed.modifier((doc) => {
      for (const o of doc.objets) if (ids.includes(o.id)) ed.modele.deplacer(o, copie(o), dx, dy);
    }, marquePousser);
  }
  function imageEntiere() {
    if (!ed.h?.doc.recadrage) return;
    ed.modifier((doc) => ed.modele.recadrer(doc, null));
  }

  // ---------- surcouche de la vue ----------
  // Poignées, cadre de sélection, recadrage en cours, pipette : dessinés par la page, en
  // pixels de la toile (mesures d'écran × dpr), couleurs des jetons.
  ed.surcouche = (ctx) => {
    const dpr = ed.vue.dpr;
    const acc = jeton("accent");
    ctx.save();
    if (geste?.mode === "recadrage") {
      const g = geste;
      const k = ed.modele.cadre(ed.h.doc);
      const [ax, ay] = versToile(Math.min(g.x1, g.x2), Math.min(g.y1, g.y2));
      const [bx, by] = versToile(Math.max(g.x1, g.x2), Math.max(g.y1, g.y2));
      const [kx, ky] = versToile(k.x, k.y);
      const [lx, ly] = versToile(k.x + k.largeur, k.y + k.hauteur);
      ctx.fillStyle = jeton("editeur-recadrage-voile");
      ctx.beginPath();
      ctx.rect(kx, ky, lx - kx, ly - ky);
      ctx.rect(ax, ay, bx - ax, by - ay);
      ctx.fill("evenodd");
      ctx.strokeStyle = jeton("editeur-recadrage-tiers");
      ctx.lineWidth = dpr;
      for (let i = 1; i < 3; i++) {
        ctx.beginPath();
        ctx.moveTo(ax + ((bx - ax) * i) / 3, ay);
        ctx.lineTo(ax + ((bx - ax) * i) / 3, by);
        ctx.moveTo(ax, ay + ((by - ay) * i) / 3);
        ctx.lineTo(bx, ay + ((by - ay) * i) / 3);
        ctx.stroke();
      }
      ctx.strokeStyle = jeton("editeur-recadrage-bord");
      ctx.lineWidth = 1.5 * dpr;
      ctx.strokeRect(ax, ay, bx - ax, by - ay);
      const texte = `${Math.round(Math.abs(g.x2 - g.x1))} × ${Math.round(Math.abs(g.y2 - g.y1))}`;
      ctx.font = `600 ${11 * dpr}px ${ed.modele.POLICE}`;
      const lw = ctx.measureText(texte).width + 12 * dpr;
      const hh = 18 * dpr;
      let ey = by + 8 * dpr;
      if (ey + hh + 2 * dpr > ed.toile.height) ey = by - 26 * dpr;
      const ex = (ax + bx) / 2 - lw / 2;
      ctx.beginPath();
      ctx.roundRect(ex, ey, lw, hh, 5 * dpr);
      ctx.fillStyle = jeton("editeur-etiquette-fond");
      ctx.fill();
      ctx.fillStyle = jeton("editeur-etiquette-texte");
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillText(texte, ex + lw / 2, ey + hh / 2);
    }
    if (geste?.mode === "cadre") {
      const g = geste;
      const [ax, ay] = versToile(Math.min(g.x0, g.x1), Math.min(g.y0, g.y1));
      const [bx, by] = versToile(Math.max(g.x0, g.x1), Math.max(g.y0, g.y1));
      ctx.fillStyle = acc;
      ctx.globalAlpha = 0.1;
      ctx.fillRect(ax, ay, bx - ax, by - ay);
      ctx.globalAlpha = 0.9;
      ctx.strokeStyle = acc;
      ctx.lineWidth = dpr;
      ctx.strokeRect(Math.round(ax) + 0.5, Math.round(ay) + 0.5, bx - ax, by - ay);
      ctx.globalAlpha = 1;
    }
    if (!edition && sorte() !== "recadrer" && ed.selection.length === 1) {
      const o = ed.objet(ed.selection[0]);
      const rayon = jetonNombre("editeur-poignee-rayon") * dpr;
      for (const [x, y, nom] of o ? ed.modele.poignees(o) : []) {
        const [px, py] = versToile(x, y);
        const milieu = nom === "pc";
        ctx.beginPath();
        ctx.arc(px, py, milieu ? rayon - 0.5 * dpr : rayon, 0, 2 * Math.PI);
        ctx.shadowColor = jeton("editeur-ombre-poignee");
        ctx.shadowBlur = 3 * dpr;
        ctx.fillStyle = milieu ? acc : jeton("editeur-poignee");
        ctx.fill();
        ctx.shadowColor = "transparent";
        ctx.lineWidth = 1.5 * dpr;
        ctx.strokeStyle = milieu ? jeton("editeur-poignee") : acc;
        ctx.stroke();
      }
    }
    if (pipette && survolPipette) {
      const d = jetonNombre("editeur-pipette-decalage");
      const r = jetonNombre("editeur-pipette-rayon") * dpr;
      ctx.beginPath();
      ctx.arc((survolPipette.x + d) * dpr, (survolPipette.y - d) * dpr, r, 0, 2 * Math.PI);
      ctx.fillStyle = survolPipette.c;
      ctx.shadowColor = jeton("editeur-ombre-pipette");
      ctx.shadowBlur = 6 * dpr;
      ctx.fill();
      ctx.shadowColor = "transparent";
      ctx.lineWidth = 2.5 * dpr;
      ctx.strokeStyle = jeton("editeur-recadrage-bord");
      ctx.stroke();
    }
    ctx.restore();
  };

  return {
    validerSaisie,
    annulerSaisie,
    saisieOuverte: () => !!edition,
    // Un geste en cours (bouton enfoncé) : ni annuler ni rétablir.
    gesteEnCours: () => !!geste,
    idEnEdition: () => (edition?.existant ? edition.objet.id : null),
    placerSaisie,
    pipette: () => pipette,
    basculerPipette,
    arreterPipette,
    imageEntiere,
    dupliquer,
    supprimer,
    ordonner,
    pousser,
    espace: (oui) => {
      espace = oui;
      if (!geste) scene.dataset.curseur = oui ? "grab" : "";
    },
    remiseAZero: () => {
      geste = null;
      edition = null;
      saisie.hidden = true;
      ed.enCours = null;
      arreterPipette();
    },
    sorteObjet,
  };
}
