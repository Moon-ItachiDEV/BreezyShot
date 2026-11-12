// Document de l'éditeur d'annotation (contrat 0.4 « Éditeur », §3.2) : objets, ordre,
// recadrage et historique, en pixels de l'image. Module pur, sans DOM (la mesure du texte
// prend un `OffscreenCanvas` s'il existe, une estimation sinon), essayé sous Node et dans
// Chromium (`essais/pages/editeur-modele.cjs`). Formes et formules : celles du design.
//
// Document : { largeur, hauteur, objets: Objet[], recadrage: Rect|null, suivant: number }
// Rect : { x, y, largeur, hauteur } ; l'ordre de `objets` est l'ordre de dessin (les masques
// sont toujours peints sous les autres objets, dans leur ordre entre eux).
// Objet (champ commun `id` entier > 0, `type`) :
//   fleche     { x1, y1, x2, y2, cx?, cy?, double, couleur, epaisseur, opacite }
//   rectangle  { x1, y1, x2, y2, couleur, epaisseur, plein, arrondi, opacite }
//   ellipse    { x1, y1, x2, y2, couleur, epaisseur, plein, opacite }
//   ligne      { x1, y1, x2, y2, couleur, epaisseur, opacite }
//   crayon     { points: [[x, y], …], couleur, epaisseur, opacite }
//   surligneur { points: [[x, y], …], couleur, epaisseur }
//   texte      { x, y, texte, couleur, taille, gras, style, opacite }
//              style : "simple" | "contour" | "etiquette"
//   numero     { x, y, rayon, couleur, ordre, opacite }
//   masque     { x1, y1, x2, y2, sorte, intensite, graine }
//              sorte : "pixeliser"|"flouter"|"bloc" ; graine : entier tiré par `ajouter`
//   projecteur { x1, y1, x2, y2, forme }   forme : "rectangle" | "ovale" ; voile sur toute
//              l'image, percé de la réunion des projecteurs, peint après tous les objets
// epaisseur et intensite : 0 | 1 | 2. opacite : de 0 à 1 (1 : opaque ; absente : 1 ; une
// valeur au-dessus de 1 est lue en pour cent).
// Historique : { doc, passe, futur, version, marque }. Annuler et rétablir remplacent les
// objets du document par des copies : la page les cherche par `id`, jamais par référence
// gardée d'un pas à l'autre.

export const POLICE = '"Segoe UI Variable Text","Segoe UI",system-ui,-apple-system,sans-serif';
// Épaisseurs (px de l'image), réexportées par `dessin.js` : ici pour que `document.js` ne
// dépende pas de `dessin.js`.
export const TRAITS = [3, 5, 8]; // Fin, Moyen, Épais
export const TRAITS_SURLIGNEUR = [16, 24, 34];
const PAS_MAX = 200;
const OCTETS_MAX = 32 * 1024 * 1024; // JSON de l'historique (passé et futur) au plus
const DECALAGE_COPIE = 16;
const RECTANGULAIRES = { rectangle: 1, ellipse: 1, masque: 1, projecteur: 1 };

// Marges des ombres et des traits autour de la forme (px de l'image), d'après le dessin :
// une ombre de flou b décalée de d s'étend jusqu'à 1,5 b + d (écart type b / 2, trois écarts).
const ombre = (flou, dy) => 1.5 * flou + dy + 2;

const borne = (v, a, b) => Math.max(a, Math.min(b, v));
const copie = (o) => JSON.parse(JSON.stringify(o));
const trait = (o) => TRAITS[o.epaisseur] ?? TRAITS[1];
const traitSurligneur = (o) => TRAITS_SURLIGNEUR[o.epaisseur] ?? TRAITS_SURLIGNEUR[1];

export function nouveauDocument(largeur, hauteur) {
  return { largeur, hauteur, objets: [], recadrage: null, suivant: 1 };
}

// ---------- historique ----------

export function creerHistorique(doc) {
  return { doc, passe: [], futur: [], version: 0, marque: null };
}

// `fn(doc)` ; même `marque` (non nulle) que la modification précédente : un seul pas
// d'annulation (une glissière tirée, un geste : une marque neuve par geste). Si `fn` lève
// une erreur, le document revient à son état d'avant l'appel, l'historique et `version`
// sont inchangés, et l'erreur repart.
export function modifier(h, fn, marque = null) {
  const suite = marque !== null && marque !== undefined && h.marque === marque;
  const avant = JSON.stringify(h.doc);
  try {
    fn(h.doc);
  } catch (e) {
    remettre(h.doc, avant);
    throw e;
  }
  if (!suite) {
    h.passe.push(avant);
    if (h.passe.length > PAS_MAX) h.passe.shift();
    h.futur = [];
    borner(h);
  }
  h.marque = marque ?? null;
  h.version += 1;
}

// Octets de JSON gardés par l'historique.
export function octetsHistorique(h) {
  let n = 0;
  for (const t of h.passe) n += t.length;
  for (const t of h.futur) n += t.length;
  return n;
}

// Au-delà d'OCTETS_MAX, les pas les plus anciens tombent ; il en reste toujours un.
function borner(h) {
  let n = octetsHistorique(h);
  while (n > OCTETS_MAX && h.futur.length) n -= h.futur.shift().length;
  while (n > OCTETS_MAX && h.passe.length > 1) n -= h.passe.shift().length;
}

// Le document garde son identité : les champs sont remplacés en place.
function remettre(doc, instantane) {
  const d = JSON.parse(instantane);
  for (const k of Object.keys(doc)) if (!(k in d)) delete doc[k];
  Object.assign(doc, d);
}

// Rend vrai si un pas a été annulé. La page retire de sa sélection les `id` disparus.
export function annuler(h) {
  if (!h.passe.length) return false;
  h.futur.push(JSON.stringify(h.doc));
  borner(h);
  remettre(h.doc, h.passe.pop());
  h.marque = null;
  h.version += 1;
  return true;
}

export function retablir(h) {
  if (!h.futur.length) return false;
  h.passe.push(JSON.stringify(h.doc));
  borner(h);
  remettre(h.doc, h.futur.pop());
  h.marque = null;
  h.version += 1;
  return true;
}

export function peutAnnuler(h) {
  return h.passe.length > 0;
}

export function peutRetablir(h) {
  return h.futur.length > 0;
}

// ---------- ajouts (exports en plus du contrat) ----------

// Range `objet` au-dessus des autres avec un `id` neuf (et un `ordre` neuf pour un numéro
// qui n'en a pas, une `graine` neuve pour un masque qui n'en a pas) ; rend l'`id`. À appeler
// dans `modifier`.
export function ajouter(doc, objet) {
  objet.id = doc.suivant++;
  if (objet.type === "numero" && objet.ordre == null) objet.ordre = ordreSuivant(doc);
  if (objet.type === "masque" && objet.graine == null) objet.graine = graineNeuve();
  doc.objets.push(objet);
  return objet.id;
}

// Graine du grain de Pixeliser : tirée une fois, gardée dans l'objet (l'écran, annuler,
// rétablir et l'export donnent les mêmes pixels), jamais devinable d'une capture à l'autre.
function graineNeuve() {
  return globalThis.crypto.getRandomValues(new Uint32Array(1))[0];
}

function ordreSuivant(doc) {
  let m = 0;
  for (const o of doc.objets) if (o.type === "numero") m = Math.max(m, o.ordre);
  return m + 1;
}

// x1 <= x2, y1 <= y2 pour rectangle, ellipse et masque (fin d'un geste).
export function normaliser(objet) {
  if (!RECTANGULAIRES[objet.type]) return;
  const [a, b] = [objet.x1, objet.x2].sort((p, q) => p - q);
  const [c, d] = [objet.y1, objet.y2].sort((p, q) => p - q);
  Object.assign(objet, { x1: a, x2: b, y1: c, y2: d });
}

// Lissage du crayon à la fin du trait : Chaikin deux fois (design).
export function lisser(points) {
  return chaikin(chaikin(points)).map(arrondirPoint);
}

// Points au 1/100 px : l'historique garde des instantanés JSON, chaque chiffre compte.
const centieme = (v) => Math.round(v * 100) / 100;
const arrondirPoint = (a) => [centieme(a[0]), centieme(a[1])];

function chaikin(p) {
  if (p.length < 3) return p;
  const r = [p[0]];
  for (let i = 0; i < p.length - 1; i++) {
    const [a, b] = [p[i], p[i + 1]];
    r.push([a[0] * 0.75 + b[0] * 0.25, a[1] * 0.75 + b[1] * 0.25]);
    r.push([a[0] * 0.25 + b[0] * 0.75, a[1] * 0.25 + b[1] * 0.75]);
  }
  r.push(p[p.length - 1]);
  return r;
}

// Pose le recadrage (null : « Image entière ») ; un cadre vide ou égal à l'image entière le
// retire. La position puis la taille sont arrondies (un carré reste carré) ; un cadre qui
// dépasse l'image y est ramené en gardant sa taille quand elle tient, sinon coupé. La page
// passe le cadre entier tracé, pas un cadre déjà coupé. À appeler dans `modifier`.
export function recadrer(doc, rect) {
  doc.recadrage = null;
  if (!rect) return;
  const axe = (debut, taille, max) => {
    const t = Math.min(Math.round(Math.abs(taille)), max);
    const d = Math.round(taille < 0 ? debut + taille : debut);
    return [borne(d, 0, max - t), t];
  };
  const [x, l] = axe(rect.x, rect.largeur, doc.largeur);
  const [y, h] = axe(rect.y, rect.hauteur, doc.hauteur);
  if (l < 1 || h < 1) return;
  if (l === doc.largeur && h === doc.hauteur) return;
  doc.recadrage = { x, y, largeur: l, hauteur: h };
}

// Police et boîte d'un texte : { largeur, hauteur, px, py, lignes } (design `tbox`).
let mesureur = null;
export function policeTexte(o) {
  return (o.gras ? "700 " : "500 ") + o.taille + "px " + POLICE;
}
export function mesureTexte(o) {
  const lignes = String(o.texte || " ").split("\n");
  if (mesureur === null && typeof OffscreenCanvas !== "undefined") {
    mesureur = new OffscreenCanvas(1, 1).getContext("2d");
  }
  let l = 0;
  if (mesureur) {
    mesureur.font = policeTexte(o);
    for (const t of lignes) l = Math.max(l, mesureur.measureText(t || " ").width);
  } else {
    for (const t of lignes) l = Math.max(l, (t || " ").length * o.taille * 0.56);
  }
  const h = o.taille * 1.2 * lignes.length;
  if (o.style === "etiquette") {
    const px = o.taille * 0.42, py = o.taille * 0.16;
    return { largeur: l + 2 * px, hauteur: h + 2 * py, px, py, lignes };
  }
  return { largeur: l, hauteur: h, px: 0, py: 0, lignes };
}

// ---------- géométrie ----------

function rectDe(x, y, x2, y2) {
  return { x, y, largeur: x2 - x, hauteur: y2 - y };
}

function norme(o) {
  return rectDe(Math.min(o.x1, o.x2), Math.min(o.y1, o.y2),
    Math.max(o.x1, o.x2), Math.max(o.y1, o.y2));
}

// Milieu de la courbe (t = 1/2), où se tient la poignée `pc`.
function milieuCourbe(o) {
  return [0.25 * o.x1 + 0.5 * o.cx + 0.25 * o.x2, 0.25 * o.y1 + 0.5 * o.cy + 0.25 * o.y2];
}

// Points extrêmes d'une courbe quadratique (bouts et sommets par axe).
function extremesCourbe(o) {
  const pts = [[o.x1, o.y1], [o.x2, o.y2]];
  const t = [(o.x1 - o.cx) / (o.x1 - 2 * o.cx + o.x2), (o.y1 - o.cy) / (o.y1 - 2 * o.cy + o.y2)];
  for (const s of t) {
    if (s > 0 && s < 1) {
      const u = 1 - s;
      pts.push([u * u * o.x1 + 2 * u * s * o.cx + s * s * o.x2,
        u * u * o.y1 + 2 * u * s * o.cy + s * s * o.y2]);
    }
  }
  return pts;
}

function enveloppe(pts, marge) {
  let a = Infinity, b = Infinity, c = -Infinity, d = -Infinity;
  for (const [x, y] of pts) {
    a = Math.min(a, x); b = Math.min(b, y); c = Math.max(c, x); d = Math.max(d, y);
  }
  return rectDe(a - marge, b - marge, c + marge, d + marge);
}

// Longueur de la pointe d'une flèche (design : W·3,4 + 14, au plus 55 % de la longueur).
export function longueurPointe(o) {
  const W = trait(o), L = Math.hypot(o.x2 - o.x1, o.y2 - o.y1);
  return Math.min(L * 0.55, W * 3.4 + 14);
}

// Boîte de la forme elle-même (sans ombre) : poignées, contour de sélection, redimensionner.
export function boiteForme(o) {
  switch (o.type) {
    case "rectangle": case "ellipse": case "masque": case "projecteur":
      return norme(o);
    case "fleche": case "ligne": {
      const pts = o.cx != null ? [[o.x1, o.y1], [o.x2, o.y2], milieuCourbe(o)]
        : [[o.x1, o.y1], [o.x2, o.y2]];
      return enveloppe(pts, trait(o) * 2);
    }
    case "crayon": case "surligneur": {
      const w = o.type === "crayon" ? trait(o) : traitSurligneur(o);
      return enveloppe(o.points, w / 2);
    }
    case "texte": {
      const m = mesureTexte(o);
      return { x: o.x, y: o.y, largeur: m.largeur, hauteur: m.hauteur };
    }
    case "numero":
      return { x: o.x - o.rayon, y: o.y - o.rayon, largeur: 2 * o.rayon, hauteur: 2 * o.rayon };
    default:
      return { x: 0, y: 0, largeur: 0, hauteur: 0 };
  }
}

// Boîte de tout ce que l'objet peint, ombre et pointe comprises : hors d'elle, aucun pixel
// ne change. Elle contient aussi la boîte de la forme (et donc les poignées).
export function boite(o) {
  const a = boiteOmbre(o), b = boiteForme(o);
  const x = Math.min(a.x, b.x), y = Math.min(a.y, b.y);
  return rectDe(x, y, Math.max(a.x + a.largeur, b.x + b.largeur),
    Math.max(a.y + a.hauteur, b.y + b.hauteur));
}

function boiteOmbre(o) {
  switch (o.type) {
    case "rectangle": case "ellipse": {
      const r = norme(o), m = trait(o) / 2 + ombre(6, 1);
      return rectDe(r.x - m, r.y - m, r.x + r.largeur + m, r.y + r.hauteur + m);
    }
    case "masque": case "projecteur": {
      // Projecteur : la zone éclairée ; son voile, lui, couvre toute l'image.
      const r = norme(o);
      return rectDe(Math.floor(r.x) - 1, Math.floor(r.y) - 1,
        Math.ceil(r.x + r.largeur) + 1, Math.ceil(r.y + r.hauteur) + 1);
    }
    case "fleche": {
      // Pointe : demi-largeur 0,52·hl à sa base, trait du contour W/4. Droite : la base est
      // sur le segment ; courbe : à hl du bout, ses coins à 1,13·hl au plus.
      const hl = longueurPointe(o), W = trait(o);
      const m = Math.max(hl * (o.cx != null ? 1.13 : 0.6), W)
        + Math.max(1, W * 0.25) / 2 + ombre(6, 1.5);
      const pts = o.cx != null ? extremesCourbe(o) : [[o.x1, o.y1], [o.x2, o.y2]];
      return enveloppe(pts, m);
    }
    case "ligne":
      return enveloppe([[o.x1, o.y1], [o.x2, o.y2]], trait(o) / 2 + ombre(5, 1));
    case "crayon":
      return enveloppe(o.points, trait(o) / 2 + ombre(4, 1));
    case "surligneur":
      return enveloppe(o.points, traitSurligneur(o) / 2 + 2);
    case "texte": {
      const m = mesureTexte(o), t = o.taille;
      // Contour (0,1·t), ombre (0,18·t), et les jambages et accents qui sortent de la
      // boîte de mesure.
      const g = t * 0.45 + 4;
      return rectDe(o.x - g, o.y - g, o.x + m.largeur + g, o.y + m.hauteur + g);
    }
    case "numero": {
      const m = o.rayon + ombre(6, 1.5);
      return rectDe(o.x - m, o.y - m, o.x + m, o.y + m);
    }
    default:
      return { x: 0, y: 0, largeur: 0, hauteur: 0 };
  }
}

function distSegment(px, py, ax, ay, bx, by) {
  const dx = bx - ax, dy = by - ay, l = dx * dx + dy * dy;
  const t = l ? borne(((px - ax) * dx + (py - ay) * dy) / l, 0, 1) : 0;
  return Math.hypot(px - ax - t * dx, py - ay - t * dy);
}

function distPolyligne(p, pts) {
  if (pts.length === 1) return Math.hypot(p.x - pts[0][0], p.y - pts[0][1]);
  let m = Infinity;
  for (let j = 1; j < pts.length; j++) {
    m = Math.min(m, distSegment(p.x, p.y, pts[j - 1][0], pts[j - 1][1], pts[j][0], pts[j][1]));
  }
  return m;
}

function pointsCourbe(o) {
  const r = [];
  for (let i = 0; i <= 20; i++) {
    const t = i / 20, u = 1 - t;
    r.push([u * u * o.x1 + 2 * u * t * o.cx + t * t * o.x2,
      u * u * o.y1 + 2 * u * t * o.cy + t * t * o.y2]);
  }
  return r;
}

// `point` : { x, y } ou [x, y] ; `tolerance` en px de l'image. `strict` est reçu comme le
// design le reçoit : il n'y change que la Mise en avant, absente de la 0.4.
export function touche(objet, point, tolerance, strict) {
  void strict;
  const p = Array.isArray(point) ? { x: point[0], y: point[1] } : point;
  const tol = tolerance ?? 0;
  const o = objet;
  if (o.type === "fleche" || o.type === "ligne") {
    const pts = o.cx != null ? pointsCourbe(o) : [[o.x1, o.y1], [o.x2, o.y2]];
    return distPolyligne(p, pts) < Math.max(trait(o) * 1.6, tol);
  }
  if (o.type === "crayon") return distPolyligne(p, o.points) < Math.max(trait(o), tol);
  if (o.type === "surligneur") return distPolyligne(p, o.points) < traitSurligneur(o) / 2 + 2;
  const b = boiteForme(o);
  const dedans = p.x >= b.x - tol && p.x <= b.x + b.largeur + tol
    && p.y >= b.y - tol && p.y <= b.y + b.hauteur + tol;
  if (!dedans) return false;
  if (o.type === "rectangle") {
    if (o.plein) return true;
    const t = Math.max(trait(o), tol);
    return !(p.x > b.x + t && p.x < b.x + b.largeur - t
      && p.y > b.y + t && p.y < b.y + b.hauteur - t);
  }
  if (o.type === "projecteur" && o.forme === "ovale") {
    // Toute la zone éclairée se touche, comme une forme pleine, à la tolérance près.
    const rx = b.largeur / 2 + tol, ry = b.hauteur / 2 + tol;
    return Math.hypot((p.x - b.x - b.largeur / 2) / rx, (p.y - b.y - b.hauteur / 2) / ry) <= 1;
  }
  if (o.type === "ellipse") {
    const rx = b.largeur / 2, ry = b.hauteur / 2;
    const d = Math.hypot((p.x - b.x - rx) / rx, (p.y - b.y - ry) / ry);
    return o.plein ? d <= 1.05 : Math.abs(d - 1) * Math.min(rx, ry) < Math.max(trait(o), tol);
  }
  return true;
}

// L'objet du dessus sous `point`, ou null. Les annotations d'abord, puis les projecteurs
// (une zone éclairée laisse prendre ce qu'elle montre), puis les masques (sous tout).
// `filtreType` : null, un type (« fleche »…) ou une liste de types.
export function objetSous(doc, point, tolerance, filtreType) {
  const admis = filtreType == null ? null : [].concat(filtreType);
  const ok = (o) => !admis || admis.includes(o.type);
  const l = doc.objets;
  const rang = (o) => (o.type === "masque" ? 2 : o.type === "projecteur" ? 1 : 0);
  for (const r of [0, 1, 2]) {
    for (let i = l.length - 1; i >= 0; i--) {
      if (rang(l[i]) === r && ok(l[i]) && touche(l[i], point, tolerance)) return l[i];
    }
  }
  return null;
}

// [[x, y, nom], …] : p1 p2 pc (flèche), p1 p2 (ligne), nw ne sw se (les autres).
export function poignees(o) {
  if (o.type === "fleche") {
    const m = o.cx != null ? milieuCourbe(o) : [(o.x1 + o.x2) / 2, (o.y1 + o.y2) / 2];
    return [[o.x1, o.y1, "p1"], [o.x2, o.y2, "p2"], [m[0], m[1], "pc"]];
  }
  if (o.type === "ligne") return [[o.x1, o.y1, "p1"], [o.x2, o.y2, "p2"]];
  const b = boiteForme(o);
  return [[b.x, b.y, "nw"], [b.x + b.largeur, b.y, "ne"],
    [b.x, b.y + b.hauteur, "sw"], [b.x + b.largeur, b.y + b.hauteur, "se"]];
}

// `origine` : copie de l'objet au début du geste.
export function deplacer(objet, origine, dx, dy) {
  const q = origine;
  if (q.points) objet.points = q.points.map((a) => arrondirPoint([a[0] + dx, a[1] + dy]));
  if ("x1" in q) {
    objet.x1 = q.x1 + dx; objet.y1 = q.y1 + dy; objet.x2 = q.x2 + dx; objet.y2 = q.y2 + dy;
  }
  if (q.cx != null) { objet.cx = q.cx + dx; objet.cy = q.cy + dy; }
  if ("x" in q) { objet.x = q.x + dx; objet.y = q.y + dy; }
}

// `point` : la pointe en px de l'image ; `echelleVue` : px d'écran CSS par px de l'image
// (les tailles mini du design, 12 et 10 px, sont des px d'écran).
export function redimensionner(objet, origine, poignee, point, echelleVue) {
  const o = objet, q = origine, s = echelleVue || 1;
  const p = Array.isArray(point) ? { x: point[0], y: point[1] } : point;
  if (poignee === "p1" || poignee === "p2") {
    const fx = poignee === "p1" ? q.x2 : q.x1, fy = poignee === "p1" ? q.y2 : q.y1;
    const mini = 12 / s;
    let { x, y } = p;
    if (Math.hypot(x - fx, y - fy) < mini) {
      const an = Math.atan2(y - fy, x - fx) || 0;
      x = fx + Math.cos(an) * mini; y = fy + Math.sin(an) * mini;
    }
    if (poignee === "p1") { o.x1 = x; o.y1 = y; } else { o.x2 = x; o.y2 = y; }
    return;
  }
  if (poignee === "pc") {
    o.cx = 2 * p.x - 0.5 * (q.x1 + q.x2); o.cy = 2 * p.y - 0.5 * (q.y1 + q.y2);
    return;
  }
  const b = boiteForme(q);
  const ax = poignee[1] === "w" ? b.x + b.largeur : b.x;
  const ay = poignee[0] === "n" ? b.y + b.hauteur : b.y;
  if (RECTANGULAIRES[q.type]) {
    const mini = 10 / s, dx = p.x - ax, dy = p.y - ay;
    o.x1 = ax; o.y1 = ay;
    o.x2 = ax + (Math.abs(dx) < mini ? (dx < 0 ? -mini : mini) : dx);
    o.y2 = ay + (Math.abs(dy) < mini ? (dy < 0 ? -mini : mini) : dy);
    normaliser(o);
    return;
  }
  let sx = Math.abs(p.x - ax) / Math.max(1, b.largeur);
  let sy = Math.abs(p.y - ay) / Math.max(1, b.hauteur);
  if (q.type === "texte" || q.type === "numero") {
    const k = borne(Math.max(sx, sy), 0.25, 8);
    if (q.type === "texte") {
      o.taille = borne(Math.round(q.taille * k), 10, 160);
      const m = mesureTexte(o);
      o.x = poignee[1] === "w" ? ax - m.largeur : ax;
      o.y = poignee[0] === "n" ? ay - m.hauteur : ay;
    } else {
      o.rayon = borne(q.rayon * k, 8, 400);
      o.x = poignee[1] === "w" ? ax - o.rayon : ax + o.rayon;
      o.y = poignee[0] === "n" ? ay - o.rayon : ay + o.rayon;
    }
    return;
  }
  if (q.points) {
    const nx = Math.min(ax, p.x), ny = Math.min(ay, p.y);
    sx = Math.max(sx, 0.15); sy = Math.max(sy, 0.15);
    o.points = q.points.map((a) => arrondirPoint([nx + (a[0] - b.x) * sx,
      ny + (a[1] - b.y) * sy]));
  }
}

// ---------- ordre, copies, numéros ----------

// Rend les ids des copies (décalage 16 px, au-dessus, nouvel ordre pour les numéros).
export function dupliquer(doc, ids) {
  const nouveaux = [];
  for (const o of doc.objets.filter((x) => ids.includes(x.id))) {
    const c = copie(o);
    deplacer(c, o, DECALAGE_COPIE, DECALAGE_COPIE);
    if (c.type === "numero") c.ordre = null;
    if (c.type === "masque") c.graine = null;
    nouveaux.push(ajouter(doc, c));
  }
  return nouveaux;
}

export function supprimer(doc, ids) {
  doc.objets = doc.objets.filter((o) => !ids.includes(o.id));
}

// `devant` vrai : premier plan ; faux : arrière-plan. Un masque, toujours peint sous les
// autres objets, passe ainsi devant ou derrière les autres masques.
export function ordonner(doc, ids, devant) {
  const a = doc.objets.filter((o) => ids.includes(o.id));
  const b = doc.objets.filter((o) => !ids.includes(o.id));
  doc.objets = devant ? b.concat(a) : a.concat(b);
}

// Rang du numéro (1, 2, 3…) par `ordre` parmi les numéros.
export function numeroAffiche(doc, objet) {
  let rang = 1;
  for (const o of doc.objets) {
    if (o.type === "numero" && o !== objet
      && (o.ordre < objet.ordre || (o.ordre === objet.ordre && o.id < objet.id))) rang++;
  }
  return rang;
}

// Le recadrage, borné à l'image et en entiers, ou l'image entière.
export function cadre(doc) {
  const r = doc.recadrage;
  if (!r) return { x: 0, y: 0, largeur: doc.largeur, hauteur: doc.hauteur };
  const x = borne(Math.round(r.x), 0, doc.largeur), y = borne(Math.round(r.y), 0, doc.hauteur);
  const x2 = borne(Math.round(r.x + r.largeur), x, doc.largeur);
  const y2 = borne(Math.round(r.y + r.hauteur), y, doc.hauteur);
  if (x2 - x < 1 || y2 - y < 1) return { x: 0, y: 0, largeur: doc.largeur, hauteur: doc.hauteur };
  return { x, y, largeur: x2 - x, hauteur: y2 - y };
}

// Aucun objet et aucun recadrage.
export function vide(doc) {
  return doc.objets.length === 0 && doc.recadrage === null;
}
