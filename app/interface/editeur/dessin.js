// Rendu à l'écran et composition de l'export de l'éditeur (contrat 0.4 « Éditeur », §3.3 et
// §3.4) : un seul code dessine la vue et l'image exportée (Canvas 2D). Les constantes sont
// des couleurs et des mesures de l'image, pas de l'habillage : ni jetons ni thème. Formes,
// ombres et formules : celles du design (`design/editeur/index.html`, piste A v2). Les
// poignées et le recadrage en cours sont dessinés par la page, par-dessus la vue.

import {
  boite, boiteForme, cadre, entreePointe, longueurPointe, mesureTexte, numeroAffiche,
  policeTexte, POLICE, TRAITS, TRAITS_SURLIGNEUR,
} from "./document.js";

// Épaisseurs : valeurs tenues par `document.js` (qui ne dépend pas de ce module).
export { TRAITS, TRAITS_SURLIGNEUR };

// Les six de la maquette « Éditeur épuré », dans son ordre.
export const COULEURS = [
  ["#E5484D", "Rouge"],
  ["#FFD43B", "Jaune"],
  ["#30A46C", "Vert"],
  ["#0090FF", "Bleu"],
  ["#1A1A1A", "Noir"],
  ["#FFFFFF", "Blanc"],
];
export const RAYONS_NUMERO = [12, 16, 21]; // P, M, G
export const BLOCS = [8, 13, 20]; // Pixeliser : côté du bloc
export const FLOUS = [6, 10, 16]; // Flouter : facteur de réduction
export const COULEURS_PERSO_MAX = 1; // « Autre couleur » : une seule couleur libre gardée

const NOIR_BLOC = "#111214";
const VOILE_PROJECTEUR = "rgba(8,10,16,.55)";
const RAYON_PROJECTEUR = 8; // px de l'image
const SURLIGNEUR_ALPHA = 0.42;
const ACCENT_DE_SECOURS = "#0067C0";

const trait = (o) => TRAITS[o.epaisseur] ?? TRAITS[1];
const alphaDe = (o) => {
  const a = o.opacite ?? 1;
  return Math.max(0, Math.min(1, a > 1 ? a / 100 : a));
};

// Toile de travail : `OffscreenCanvas` s'il existe, sinon une toile du document.
function toile(l, h) {
  if (typeof OffscreenCanvas !== "undefined") return new OffscreenCanvas(l, h);
  const t = document.createElement("canvas");
  t.width = l;
  t.height = h;
  return t;
}

function luminance(hex) {
  const n = parseInt(String(hex).slice(1), 16);
  return (0.299 * ((n >> 16) & 255) + 0.587 * ((n >> 8) & 255) + 0.114 * (n & 255)) / 255;
}
const surCouleur = (c) => (luminance(c) > 0.62 ? "#1A1A1A" : "#FFFFFF");

function rectArrondi(c, x, y, w, h, r) {
  r = Math.max(0, Math.min(r, Math.abs(w) / 2, Math.abs(h) / 2));
  c.beginPath();
  c.moveTo(x + r, y);
  c.arcTo(x + w, y, x + w, y + h, r);
  c.arcTo(x + w, y + h, x, y + h, r);
  c.arcTo(x, y + h, x, y, r);
  c.arcTo(x, y, x + w, y, r);
  c.closePath();
}

// Les ombres du canevas ne suivent pas la transformation : elles sont mises à l'échelle à
// la main, pour que la vue zoomée montre l'ombre de l'export. `sansOmbres` : l'encre seule,
// pour le calque d'une encre gommée (son ombre est celle de ce qui reste, `OMBRES`).
let sansOmbres = false;
function poserOmbre(c, couleur, flou, dy) {
  if (sansOmbres) {
    c.shadowColor = "transparent";
    return;
  }
  const m = c.getTransform(), s = Math.hypot(m.a, m.b) || 1;
  c.shadowColor = couleur;
  c.shadowBlur = flou * s;
  c.shadowOffsetX = 0;
  c.shadowOffsetY = dy * s;
}
const sansOmbre = (c) => { c.shadowColor = "transparent"; };

function pointe(c, tx, ty, ux, uy, hl) {
  const hw = hl * 0.52, nx = -uy, ny = ux, bx = tx - ux * hl, by = ty - uy * hl;
  const ix = tx - ux * hl * 0.72, iy = ty - uy * hl * 0.72;
  c.beginPath();
  c.moveTo(tx, ty);
  c.lineTo(bx + nx * hw, by + ny * hw);
  c.lineTo(ix, iy);
  c.lineTo(bx - nx * hw, by - ny * hw);
  c.closePath();
  c.fill();
  c.stroke();
}

function dessinerFleche(c, o) {
  const W = trait(o), dx = o.x2 - o.x1, dy = o.y2 - o.y1, L = Math.hypot(dx, dy);
  if (L < 3) return;
  poserOmbre(c, ...OMBRES.fleche);
  c.fillStyle = c.strokeStyle = o.couleur;
  c.lineJoin = "round";
  c.lineCap = "round";
  c.lineWidth = Math.max(1, W * 0.25);
  const hl = longueurPointe(o), sc = hl / (W * 3.4 + 14), Ws = W * Math.max(0.5, sc);
  if (o.cx != null) {
    const ex = o.x2 - o.cx, ey = o.y2 - o.cy, el = Math.hypot(ex, ey) || 1;
    const ux = ex / el, uy = ey / el;
    const sx = o.x1 - o.cx, sy = o.y1 - o.cy, sl = Math.hypot(sx, sy) || 1;
    // Le corps est la courbe du modèle même, arrêtée où elle entre dans la pointe (de
    // Casteljau) : il passe par la poignée du milieu, là où la prise et la gomme le voient.
    const t = entreePointe(o, hl * 0.7), u = 1 - t;
    c.lineWidth = Ws * 1.05;
    c.beginPath();
    c.moveTo(o.x1, o.y1);
    c.quadraticCurveTo(o.x1 + t * (o.cx - o.x1), o.y1 + t * (o.cy - o.y1),
      u * u * o.x1 + 2 * u * t * o.cx + t * t * o.x2, u * u * o.y1 + 2 * u * t * o.cy + t * t * o.y2);
    c.stroke();
    c.lineWidth = Math.max(1, W * 0.25);
    pointe(c, o.x2, o.y2, ux, uy, hl);
    if (o.double) pointe(c, o.x1, o.y1, sx / sl, sy / sl, hl);
    return;
  }
  const ux = dx / L, uy = dy / L, nx = -uy, ny = ux;
  const t0 = o.double ? Ws * 0.55 : Ws * 0.2, t1 = Ws * 0.6;
  const bx = o.x2 - ux * hl * 0.72, by = o.y2 - uy * hl * 0.72;
  const sx = o.double ? o.x1 + ux * hl * 0.72 : o.x1, sy = o.double ? o.y1 + uy * hl * 0.72 : o.y1;
  c.beginPath();
  c.moveTo(sx + nx * t0, sy + ny * t0);
  c.lineTo(bx + nx * t1, by + ny * t1);
  c.lineTo(bx - nx * t1, by - ny * t1);
  c.lineTo(sx - nx * t0, sy - ny * t0);
  if (!o.double) c.arc(o.x1, o.y1, t0, Math.atan2(-ny, -nx), Math.atan2(ny, nx));
  c.closePath();
  c.fill();
  pointe(c, o.x2, o.y2, ux, uy, hl);
  if (o.double) pointe(c, o.x1, o.y1, -ux, -uy, hl);
}

function cheminLisse(c, pts) {
  c.beginPath();
  c.moveTo(pts[0][0], pts[0][1]);
  if (pts.length < 3) {
    for (let j = 1; j < pts.length; j++) c.lineTo(pts[j][0], pts[j][1]);
    return;
  }
  for (let i = 1; i < pts.length - 1; i++) {
    const mx = (pts[i][0] + pts[i + 1][0]) / 2, my = (pts[i][1] + pts[i + 1][1]) / 2;
    c.quadraticCurveTo(pts[i][0], pts[i][1], mx, my);
  }
  const l = pts[pts.length - 1];
  c.lineTo(l[0], l[1]);
}

function trace(c, pts, largeur) {
  if (!pts.length) return;
  c.lineWidth = largeur;
  c.lineCap = "round";
  c.lineJoin = "round";
  if (pts.length === 1) {
    c.beginPath();
    c.arc(pts[0][0], pts[0][1], largeur / 2, 0, 7);
    c.fill();
  } else {
    cheminLisse(c, pts);
    c.stroke();
  }
}

function dessinerTexte(c, o) {
  const b = mesureTexte(o), lh = o.taille * 1.2;
  c.font = policeTexte(o);
  c.textBaseline = "top";
  c.textAlign = "left";
  c.lineJoin = "round";
  const tx = o.x + b.px, ty = o.y + b.py + o.taille * 0.1;
  const lignes = String(o.texte || "").split("\n");
  const chaque = (f) => lignes.forEach((l, i) => f(l, tx, ty + i * lh));
  if (o.style === "etiquette") {
    poserOmbre(c, "rgba(0,0,0,.2)", 6, 1);
    rectArrondi(c, o.x, o.y, b.largeur, b.hauteur, o.taille * 0.32);
    c.fillStyle = o.couleur;
    c.fill();
    sansOmbre(c);
    c.fillStyle = surCouleur(o.couleur);
    chaque((l, x, y) => c.fillText(l, x, y));
  } else if (o.style === "contour") {
    c.lineWidth = o.taille * 0.2;
    c.strokeStyle = luminance(o.couleur) > 0.62 ? "rgba(20,20,20,.85)" : "rgba(255,255,255,.95)";
    chaque((l, x, y) => c.strokeText(l, x, y));
    c.fillStyle = o.couleur;
    chaque((l, x, y) => c.fillText(l, x, y));
  } else {
    poserOmbre(c, "rgba(0,0,0,.28)", o.taille * 0.18, 1);
    c.fillStyle = o.couleur;
    chaque((l, x, y) => c.fillText(l, x, y));
  }
}

function dessinerNumero(c, doc, o) {
  const R = o.rayon, n = numeroAffiche(doc, o);
  poserOmbre(c, "rgba(0,0,0,.28)", 6, 1.5);
  c.fillStyle = o.couleur;
  c.beginPath();
  c.arc(o.x, o.y, R, 0, 7);
  c.fill();
  sansOmbre(c);
  c.lineWidth = Math.max(1.5, R * 0.13);
  c.strokeStyle = "#fff";
  c.beginPath();
  c.arc(o.x, o.y, R - c.lineWidth / 2, 0, 7);
  c.stroke();
  c.fillStyle = surCouleur(o.couleur);
  c.font = "700 " + Math.round(R * (n > 9 ? 0.9 : 1.05)) + "px " + POLICE;
  c.textAlign = "center";
  c.textBaseline = "middle";
  c.fillText(String(n), o.x, o.y + R * 0.06);
}

// L'encre de l'objet, dans la transformation et l'opacité déjà posées sur `c`.
function encre(c, doc, o) {
  switch (o.type) {
    case "fleche":
      dessinerFleche(c, o);
      break;
    case "rectangle": case "ellipse": {
      const n = boiteForme(o);
      poserOmbre(c, ...OMBRES[o.type]);
      c.strokeStyle = c.fillStyle = o.couleur;
      c.lineWidth = trait(o);
      c.lineJoin = "round";
      if (o.type === "rectangle") rectArrondi(c, n.x, n.y, n.largeur, n.hauteur, o.arrondi || 0);
      else {
        c.beginPath();
        c.ellipse(n.x + n.largeur / 2, n.y + n.hauteur / 2, Math.max(0.5, n.largeur / 2),
          Math.max(0.5, n.hauteur / 2), 0, 0, 7);
      }
      if (o.plein) c.fill();
      else c.stroke();
      break;
    }
    case "ligne":
      poserOmbre(c, ...OMBRES.ligne);
      c.strokeStyle = o.couleur;
      c.lineWidth = trait(o);
      c.lineCap = "round";
      c.beginPath();
      c.moveTo(o.x1, o.y1);
      c.lineTo(o.x2, o.y2);
      c.stroke();
      break;
    case "crayon":
      poserOmbre(c, ...OMBRES.crayon);
      c.strokeStyle = c.fillStyle = o.couleur;
      trace(c, o.points, trait(o));
      break;
    case "surligneur":
      c.strokeStyle = c.fillStyle = o.couleur;
      trace(c, o.points, TRAITS_SURLIGNEUR[o.epaisseur] ?? TRAITS_SURLIGNEUR[1]);
      break;
    case "texte":
      dessinerTexte(c, o);
      break;
    case "numero":
      dessinerNumero(c, doc, o);
      break;
  }
}

// Ombre des encres : couleur, flou et décalage (px de l'image). Le surligneur n'en a pas.
const OMBRES = {
  fleche: ["rgba(0,0,0,.22)", 6, 1.5],
  rectangle: ["rgba(0,0,0,.2)", 6, 1],
  ellipse: ["rgba(0,0,0,.2)", 6, 1],
  ligne: ["rgba(0,0,0,.2)", 5, 1],
  crayon: ["rgba(0,0,0,.18)", 4, 1],
};

// Dessine un objet (hors masque, dans le fond, et hors projecteur, peint par `peindreVoile`)
// dans `ctx`, dont la transformation est déjà posée (échelle de la vue, ou 1 pour l'export).
// `doc` sert au chiffre des numéros. `enCours` : passages de gomme du geste en cours, en plus
// de ceux de l'objet. `garder` (la vue seule) : une encre gommée garde son rendu d'une
// peinture à l'autre tant qu'elle ne change pas (`memoire`).
export function dessinerObjet(ctx, doc, objet, enCours = null, garder = false) {
  const c = ctx, o = objet;
  if (o.type === "masque" || o.type === "projecteur") return;
  const gommes = enCours?.length ? (o.gommes || []).concat(enCours) : o.gommes;
  c.save();
  c.globalAlpha = o.type === "surligneur" ? SURLIGNEUR_ALPHA : alphaDe(o);
  if (o.type === "surligneur") c.globalCompositeOperation = "multiply";
  if (gommes?.length) dessinerGomme(c, doc, o, gommes, garder && !enCours?.length);
  else encre(c, doc, o);
  c.restore();
}

// ---------- encres gommées ----------

// Une encre gommée se pose en trois temps : l'encre seule sur le calque de travail, les
// passages de gomme effacés ; ce calque posé avec l'ombre de ce qui reste sur un rendu
// transparent ; le rendu posé avec l'opacité et le mélange de l'objet. L'ombre est celle de
// ce qui reste : effacer un trait n'en laisse pas. Deux toiles de travail pour toutes les
// encres, agrandies au besoin, relâchées avec les toiles du document (`relacherCalque`).
const travail = { calque: null, rendu: null };
function prendre(nom, l, h) {
  let t = travail[nom];
  if (!t || t.width < l || t.height < h) {
    const n = toile(Math.max(l, t?.width || 0), Math.max(h, t?.height || 0));
    if (t) t.width = t.height = 0;
    travail[nom] = t = n;
  }
  const k = t.getContext("2d");
  k.setTransform(1, 0, 0, 1, 0, 0);
  k.clearRect(0, 0, l, h);
  return k;
}

// La vue garde le rendu des encres gommées qui n'ont pas changé (l'objet, la transformation,
// la toile) : flouter l'ombre d'un calque coûte cher (environ 5 ms pour 800 × 560 px sans
// carte graphique), poser un rendu gardé presque rien. Au plus MEMOIRE_PX pixels ; une place
// ne se prend qu'à un rendu qui n'a pas servi à la peinture en cours (sinon une vue pleine
// d'encres gommées chasserait à chaque peinture ce qu'elle va reposer).
const MEMOIRE_PX = 16e6;
const memoire = new Map(); // id → { cle, rendu, px, tour }
let memoirePx = 0;
let tour = 0; // peintures de la vue
// Cadre de l'image dans la toile, en px de la toile [x0, y0, x1, y1] (posé par la vue et
// l'export le temps de dessiner les objets) : le calque d'une encre gommée s'y borne.
let bornes = null;
function oublier(id) {
  const e = memoire.get(id);
  if (!e) return;
  memoire.delete(id);
  memoirePx -= e.px;
  e.rendu.width = e.rendu.height = 0;
}
function placer(px) {
  for (const [id, e] of memoire) {
    if (memoirePx + px <= MEMOIRE_PX) break;
    if (e.tour !== tour) oublier(id);
  }
  return memoirePx + px <= MEMOIRE_PX;
}
export function relacherCalque() {
  for (const nom of Object.keys(travail)) {
    if (travail[nom]) travail[nom].width = travail[nom].height = 0;
    travail[nom] = null;
  }
  for (const id of [...memoire.keys()]) oublier(id);
}

// Efface de `k` les passages de gomme : traits ronds, en ligne brisée (celle que le geste a
// suivie), dans la transformation posée. Un seul tracé par largeur : des passages qui se
// recouvrent ne comptent qu'une fois, le bord d'un trou ne s'use pas quand on repasse dedans.
function passerGomme(k, gommes) {
  k.globalCompositeOperation = "destination-out";
  k.globalAlpha = 1;
  k.shadowColor = "transparent";
  k.strokeStyle = "#000";
  k.lineCap = k.lineJoin = "round";
  const parLargeur = new Map();
  for (const g of gommes) {
    if (!g.points.length) continue;
    const l = parLargeur.get(g.largeur);
    if (l) l.push(g.points);
    else parLargeur.set(g.largeur, [g.points]);
  }
  for (const [largeur, traces] of parLargeur) {
    k.lineWidth = largeur;
    k.beginPath();
    for (const p of traces) {
      k.moveTo(p[0][0], p[0][1]);
      // Un seul point : un trait presque nul, que les bouts ronds changent en disque.
      if (p.length === 1) k.lineTo(p[0][0] + 0.01, p[0][1]);
      for (let i = 1; i < p.length; i++) k.lineTo(p[i][0], p[i][1]);
    }
    k.stroke();
  }
}

// Une encre gommée, bornée à la boîte de l'objet en pixels de la toile et au cadre de
// l'image élargi de la portée de l'ombre : l'ombre d'une encre juste hors du cadre y entre,
// comme celle d'une encre non gommée. Borner au cadre, pas à la toile : la vue et l'export
// coupent l'encre au même endroit de l'image et la rendent au bit près pareil.
function dessinerGomme(c, doc, o, gommes, garder) {
  const m = c.getTransform(), b = boite(o), om = OMBRES[o.type];
  const s = Math.hypot(m.a, m.b) || 1;
  const marge = om ? Math.ceil((2 * om[1] + Math.abs(om[2])) * s) + 1 : 0;
  const [bx0, by0, bx1, by1] = bornes || [0, 0, c.canvas.width, c.canvas.height];
  const x0 = Math.max(bx0 - marge, Math.floor(m.e + b.x * m.a) - 1);
  const y0 = Math.max(by0 - marge, Math.floor(m.f + b.y * m.d) - 1);
  const x1 = Math.min(bx1 + marge, Math.ceil(m.e + (b.x + b.largeur) * m.a) + 1);
  const y1 = Math.min(by1 + marge, Math.ceil(m.f + (b.y + b.hauteur) * m.d) + 1);
  const l = x1 - x0, h = y1 - y0;
  if (l <= 0 || h <= 0) return;
  const cle = garder ? JSON.stringify([o, m.a, m.d, m.e, m.f, bx0, by0, bx1, by1]) : "";
  const vu = garder ? memoire.get(o.id) : null;
  let rendu;
  if (vu && vu.cle === cle) {
    vu.tour = tour;
    rendu = vu.rendu;
  } else {
    if (vu) oublier(o.id);
    const k = prendre("calque", l, h);
    k.save();
    k.setTransform(m.a, 0, 0, m.d, m.e - x0, m.f - y0);
    sansOmbres = true;
    try {
      encre(k, doc, o);
    } finally {
      sansOmbres = false;
    }
    passerGomme(k, gommes);
    k.restore();
    const garde = garder && placer(l * h);
    const r = garde ? toile(l, h).getContext("2d") : prendre("rendu", l, h);
    if (om) {
      r.shadowColor = om[0];
      r.shadowBlur = om[1] * s;
      r.shadowOffsetX = 0;
      r.shadowOffsetY = om[2] * s;
    }
    r.drawImage(travail.calque, 0, 0, l, h, 0, 0, l, h);
    r.shadowColor = "transparent";
    rendu = r.canvas;
    if (garde) {
      memoire.set(o.id, { cle, rendu, px: l * h, tour });
      memoirePx += l * h;
    }
  }
  c.setTransform(1, 0, 0, 1, 0, 0);
  sansOmbre(c);
  c.drawImage(rendu, 0, 0, l, h, x0, y0, l, h);
}

// Ce que des passages de gomme changent à une encre : "rien" (aucun pixel : passage à côté
// ou sur du déjà gommé), "efface", ou "vide" (il ne reste rien de visible : l'objet est à
// retirer). Jugé sur l'encre seule, rendue à 512 px de côté au plus.
const COTE_EFFET = 512;
const RESTE_MIN = 10; // sur 255 : en dessous, une miette invisible
export function effetGomme(doc, objet, passages) {
  const b = boite(objet);
  const s = Math.min(1, COTE_EFFET / Math.max(b.largeur, b.hauteur, 1));
  const l = Math.max(1, Math.ceil(b.largeur * s)), h = Math.max(1, Math.ceil(b.hauteur * s));
  const rendu = (gommes) => {
    const t = toile(l, h), k = t.getContext("2d", { willReadFrequently: true });
    k.setTransform(s, 0, 0, s, -b.x * s, -b.y * s);
    sansOmbres = true;
    try {
      encre(k, doc, objet);
    } finally {
      sansOmbres = false;
    }
    passerGomme(k, gommes);
    const d = k.getImageData(0, 0, l, h).data;
    t.width = t.height = 0;
    return d;
  };
  const avant = rendu(objet.gommes || []);
  const apres = rendu((objet.gommes || []).concat(passages));
  let change = false, reste = 0;
  for (let i = 3; i < apres.length; i += 4) {
    if (apres[i] !== avant[i]) change = true;
    if (apres[i] > reste) reste = apres[i];
  }
  if (!change) return "rien";
  return reste < RESTE_MIN ? "vide" : "efface";
}

// ---------- masques ----------

// La zone d'un masque bornée à l'image, en entiers, arrondie vers l'extérieur : aucun pixel
// en partie couvert ne reste d'origine. null si la part dans l'image fait moins de 1 px.
function zone(o, l, h) {
  const n = boiteForme(o);
  const a = Math.max(0, n.x), b = Math.max(0, n.y);
  const a2 = Math.min(l, n.x + n.largeur), b2 = Math.min(h, n.y + n.hauteur);
  if (a2 - a < 1 || b2 - b < 1) return null;
  const x = Math.floor(a), y = Math.floor(b);
  return { x, y, w: Math.ceil(a2) - x, h: Math.ceil(b2) - y };
}

// Blocs sur un axe : coupes aux multiples de `bs` (grille de l'image) ; un reste de bord de
// moins d'un demi-bloc est fondu dans le bloc voisin (un bloc de 1 px s'inverserait), et une
// zone plus petite qu'un bloc en fait un seul. Rend [[début, fin, rang], …], en px de l'image.
function blocs(debut, taille, bs) {
  const fin = debut + taille, c = [debut];
  for (let m = (Math.floor(debut / bs) + 1) * bs; m < fin; m += bs) c.push(m);
  c.push(fin);
  const s = [];
  for (let i = 1; i < c.length; i++) s.push([c[i - 1], c[i]]);
  if (s.length > 1 && s[0][1] - s[0][0] < bs / 2) s.splice(0, 2, [s[0][0], s[1][1]]);
  const z = s.length - 1;
  if (z > 0 && s[z][1] - s[z][0] < bs / 2) s.splice(z - 1, 2, [s[z - 1][0], s[z][1]]);
  return s.map(([a, b]) => [a, b, Math.floor((a + b) / 2 / bs)]);
}

// Grain de Pixeliser : écart de luminosité de chaque bloc, de ±GRAINS[i] niveaux, tiré de la
// graine de l'objet et de la place du bloc. Sans lui, la moyenne d'un bloc se compare à celle
// de candidats rendus pareil (un texte se relit chiffre par chiffre) ; avec lui, l'écart entre
// deux candidats se perd dans le grain.
const GRAINS = [24, 18, 14];
function grain(graine, i, j) {
  let h = Math.imul((graine | 0) ^ 0x9E3779B9, 0x85EBCA6B);
  h = Math.imul(h ^ Math.imul(i, 0xC2B2AE35) ^ Math.imul(j, 0x27D4EB2F), 0x165667B1);
  h ^= h >>> 15; h = Math.imul(h, 0x2C1B3C6D); h ^= h >>> 12; h = Math.imul(h, 0x297A2D39);
  h ^= h >>> 15;
  return (h >>> 0) / 4294967296 * 2 - 1;
}

// Chaque bloc de `bs` px aligné sur la grille de l'image devient sa moyenne mêlée de gris
// (formule du design : k 0,82 ou 0,86 en damier ; gris 132, 140, 154), plus son grain,
// opaque. `c` : toile dont le pixel (0, 0) est le pixel (ox, oy) de l'image.
function pixeliser(c, n, intensite, graine, ox, oy) {
  const bs = BLOCS[intensite] ?? BLOCS[1], ampleur = GRAINS[intensite] ?? GRAINS[1];
  const img = c.getImageData(n.x - ox, n.y - oy, n.w, n.h), d = img.data, W = n.w;
  const lignes = blocs(n.y, n.h, bs), colonnes = blocs(n.x, n.w, bs);
  for (const [ya, yb, j] of lignes) {
    const y0 = ya - n.y, y1 = yb - n.y;
    for (const [xa, xb, i] of colonnes) {
      const x0 = xa - n.x, x1 = xb - n.x;
      let r = 0, g = 0, b = 0;
      for (let y = y0; y < y1; y++) {
        for (let p = (y * W + x0) * 4, f = (y * W + x1) * 4; p < f; p += 4) {
          r += d[p]; g += d[p + 1]; b += d[p + 2];
        }
      }
      const cnt = (x1 - x0) * (y1 - y0);
      const k = ((i + j) & 1) ? 0.86 : 0.82, e = ampleur * grain(graine, i, j);
      r = (r / cnt) * k + 132 * (1 - k) + e;
      g = (g / cnt) * k + 140 * (1 - k) + e;
      b = (b / cnt) * k + 154 * (1 - k) + e;
      for (let y = y0; y < y1; y++) {
        for (let p = (y * W + x0) * 4, f = (y * W + x1) * 4; p < f; p += 4) {
          d[p] = r; d[p + 1] = g; d[p + 2] = b; d[p + 3] = 255;
        }
      }
    }
  }
  c.putImageData(img, n.x - ox, n.y - oy);
}

// La zone réduite de `amt` puis agrandie (deux passes), sous un voile blanc à 18 % (design
// `smoothBlur`). Écart au design : l'image réduite est agrandie depuis sa source entière ; le
// design la prenait de (-0,5, -0,5) à (tw + 0,5, th + 0,5), et le canevas, qui borne la source
// à l'image, laissait au bord de la zone une bande d'une demi-maille non floutée (5 px en
// Moyenne) où les pixels d'origine passaient sous le seul voile.
function flouter(c, n, amt, ox, oy) {
  const f = Math.max(2, amt), tw = Math.max(1, Math.round(n.w / f));
  const th = Math.max(1, Math.round(n.h / f));
  const a = toile(n.w, n.h), ac = a.getContext("2d");
  ac.drawImage(c.canvas, n.x - ox, n.y - oy, n.w, n.h, 0, 0, n.w, n.h);
  const t = toile(tw, th), tc = t.getContext("2d");
  tc.imageSmoothingQuality = "high";
  const m = toile(Math.max(1, Math.round(n.w / 2)), Math.max(1, Math.round(n.h / 2)));
  const mc = m.getContext("2d");
  mc.drawImage(a, 0, 0, m.width, m.height);
  tc.drawImage(m, 0, 0, tw, th);
  for (let pass = 0; pass < 2; pass++) {
    mc.clearRect(0, 0, m.width, m.height);
    mc.drawImage(t, 0, 0, m.width, m.height);
    tc.clearRect(0, 0, tw, th);
    tc.drawImage(m, 0, 0, tw, th);
  }
  c.save();
  c.setTransform(1, 0, 0, 1, -ox, -oy);
  c.beginPath();
  c.rect(n.x, n.y, n.w, n.h);
  c.clip();
  c.imageSmoothingEnabled = true;
  c.imageSmoothingQuality = "high";
  // La zone est effacée d'abord : sur une image à alpha, un flou en partie transparent
  // posé par-dessus laisserait voir les pixels d'origine.
  c.clearRect(n.x, n.y, n.w, n.h);
  c.drawImage(t, 0, 0, tw, th, n.x, n.y, n.w, n.h);
  c.fillStyle = "rgba(240,242,246,.18)";
  c.fillRect(n.x, n.y, n.w, n.h);
  c.restore();
  a.width = t.width = m.width = 0;
}

// Applique un masque à la toile `c`, dont le pixel (0, 0) est le pixel (ox, oy) de l'image
// et qui couvre `l` × `h` pixels de l'image entière.
// Bloc noir : la zone en rectangle plein (écart au design, rayon 3 : les pixels des coins
// arrondis restaient en partie d'origine).
function masquer(c, o, l, h, ox = 0, oy = 0) {
  const n = zone(o, l, h);
  if (!n) return;
  if (o.sorte === "bloc") {
    c.save();
    c.setTransform(1, 0, 0, 1, 0, 0);
    c.globalAlpha = 1;
    c.globalCompositeOperation = "source-over";
    c.fillStyle = NOIR_BLOC;
    c.fillRect(n.x - ox, n.y - oy, n.w, n.h);
    c.restore();
    return;
  }
  if (o.sorte === "pixeliser") pixeliser(c, n, o.intensite, o.graine ?? 0, ox, oy);
  else if (o.sorte === "flouter") flouter(c, n, FLOUS[o.intensite] ?? FLOUS[1], ox, oy);
}

// Peint dans `ctxFond` (toile pleine taille, logicielle) la base puis les masques de `doc`,
// dans leur ordre.
export function peindreMasques(ctxFond, base, doc) {
  const c = ctxFond;
  c.save();
  c.setTransform(1, 0, 0, 1, 0, 0);
  c.globalAlpha = 1;
  c.globalCompositeOperation = "copy";
  c.imageSmoothingEnabled = false;
  c.drawImage(base, 0, 0);
  c.restore();
  for (const o of doc.objets) if (o.type === "masque") masquer(c, o, doc.largeur, doc.hauteur);
}

// L'image exportée, à la taille du cadre : { largeur, hauteur, rgba } (Uint8ClampedArray).
// La toile a la taille de l'image entière, et le cadre en est lu : le lissage des bords d'un
// tracé par Chromium dépend de la taille de la toile (un trait coupé par son bord ne tombe
// pas aux mêmes pixels), et l'export recadré doit être au bit près le morceau de l'image
// entière. La mémoire est celle d'un export sans recadrage.
export function composer(doc, fond) {
  const k = cadre(doc);
  // Une toile du document comme la vue quand il y en a un : Chromium ne lisse pas le bord
  // d'une découpe pareil sur un `OffscreenCanvas` (bord des projecteurs).
  const t = typeof document !== "undefined" ? document.createElement("canvas")
    : toile(doc.largeur, doc.hauteur);
  t.width = doc.largeur;
  t.height = doc.hauteur;
  const c = t.getContext("2d", { willReadFrequently: true });
  c.globalCompositeOperation = "copy";
  c.imageSmoothingEnabled = false;
  c.drawImage(fond, 0, 0);
  c.globalCompositeOperation = "source-over";
  bornes = [k.x, k.y, k.x + k.largeur, k.y + k.hauteur];
  try {
    for (const o of doc.objets) dessinerObjet(c, doc, o);
  } finally {
    bornes = null;
  }
  // Même pile de découpes que la vue (le cadre, puis les trous) : Chromium lisse le bord
  // d'une découpe selon celles qui la précèdent, et l'export doit être l'écran au bit près.
  c.beginPath();
  c.rect(k.x, k.y, k.largeur, k.hauteur);
  c.clip();
  peindreVoile(c, projecteurs(doc.objets), doc.largeur, doc.hauteur);
  const rgba = c.getImageData(k.x, k.y, k.largeur, k.hauteur).data;
  t.width = 0;
  t.height = 0;
  return { largeur: k.largeur, hauteur: k.hauteur, rgba };
}

function accent() {
  if (typeof document === "undefined" || typeof getComputedStyle === "undefined") {
    return ACCENT_DE_SECOURS;
  }
  return getComputedStyle(document.documentElement).getPropertyValue("--accent").trim()
    || ACCENT_DE_SECOURS;
}

// Contour de sélection du design (accent, tirets 4/3, 70 %), en px de la toile. Une flèche ou
// une ligne choisie seule se reconnaît à ses poignées ; choisie avec d'autres (`plusieurs`),
// elle n'en a pas : elle reçoit le cadre des autres objets.
function contourSelection(c, o, vue, plusieurs = false) {
  const e = vue.echelle, d = vue.dpr || 1;
  const X = (x) => vue.x + x * e, Y = (y) => vue.y + y * e;
  c.save();
  c.strokeStyle = accent();
  c.lineWidth = d;
  c.globalAlpha = 0.7;
  c.setLineDash([4 * d, 3 * d]);
  if (o.type === "numero") {
    c.beginPath();
    c.arc(X(o.x), Y(o.y), o.rayon * e + 4 * d, 0, 7);
    c.stroke();
  } else if ((o.type === "fleche" || o.type === "ligne") && !plusieurs) {
    if (o.cx != null) {
      c.globalAlpha = 0.35;
      c.beginPath();
      c.moveTo(X(o.x1), Y(o.y1));
      c.lineTo(X(o.cx), Y(o.cy));
      c.lineTo(X(o.x2), Y(o.y2));
      c.stroke();
    }
  } else {
    const b = boiteForme(o);
    const pad = (o.type === "rectangle" || o.type === "ellipse" ? (trait(o) / 2) * e : 0) + 4 * d;
    rectArrondi(c, X(b.x) - pad, Y(b.y) - pad, b.largeur * e + 2 * pad, b.hauteur * e + 2 * pad,
      4 * d);
    c.stroke();
  }
  c.restore();
}

// ---------- projecteurs ----------

function projecteurs(objets, ajout = null, cacher) {
  const l = objets.filter((o) => o.type === "projecteur" && o.id !== cacher);
  if (ajout && ajout.type === "projecteur") l.push(ajout);
  return l;
}

// Voile sur toute l'image (l × h px de l'image, dans la transformation posée sur `c`, sans
// rotation), percé de la RÉUNION des projecteurs, sans calque. Hors de la boîte des trous
// (arrondie au pixel de la toile), quatre bandes pleines ; dedans, une découpe pair-impair
// « boîte + trou » par projecteur (leur intersection est la boîte moins la réunion des trous :
// aucune zone commune revoilée), puis un seul `fillRect`. La découpe ne couvre que la boîte
// des trous, pas toute l'image. `source-atop` : le voile n'assombrit que les pixels qui
// existent (un pixel transparent de la capture le reste). Les coupes déjà posées sur `c`
// (cadre de la vue) s'appliquent aussi.
function peindreVoile(c, liste, l, h) {
  if (!liste.length) return;
  const m = c.getTransform();
  const X0 = m.e, Y0 = m.f, X1 = m.e + l * m.a, Y1 = m.f + h * m.d;
  let a = Infinity, b = Infinity, a2 = -Infinity, b2 = -Infinity;
  for (const o of liste) {
    const n = boiteForme(o);
    a = Math.min(a, n.x); b = Math.min(b, n.y);
    a2 = Math.max(a2, n.x + n.largeur); b2 = Math.max(b2, n.y + n.hauteur);
  }
  const bx0 = Math.min(X1, Math.max(X0, Math.floor(m.e + a * m.a) - 1));
  const by0 = Math.min(Y1, Math.max(Y0, Math.floor(m.f + b * m.d) - 1));
  const bx1 = Math.max(bx0, Math.min(X1, Math.ceil(m.e + a2 * m.a) + 1));
  const by1 = Math.max(by0, Math.min(Y1, Math.ceil(m.f + b2 * m.d) + 1));
  c.save();
  c.setTransform(1, 0, 0, 1, 0, 0);
  c.globalAlpha = 1;
  c.globalCompositeOperation = "source-atop";
  c.fillStyle = VOILE_PROJECTEUR;
  c.fillRect(X0, Y0, X1 - X0, by0 - Y0);
  c.fillRect(X0, by1, X1 - X0, Y1 - by1);
  c.fillRect(X0, by0, bx0 - X0, by1 - by0);
  c.fillRect(bx1, by0, X1 - bx1, by1 - by0);
  if (bx1 > bx0 && by1 > by0) {
    for (const o of liste) {
      const n = boiteForme(o);
      c.setTransform(1, 0, 0, 1, 0, 0);
      c.beginPath();
      c.rect(bx0, by0, bx1 - bx0, by1 - by0);
      c.setTransform(m);
      if (o.forme === "ovale") {
        c.moveTo(n.x + n.largeur, n.y + n.hauteur / 2);
        c.ellipse(n.x + n.largeur / 2, n.y + n.hauteur / 2, Math.max(0.5, n.largeur / 2),
          Math.max(0.5, n.hauteur / 2), 0, 0, 2 * Math.PI);
      } else rectArrondiSuite(c, n.x, n.y, n.largeur, n.hauteur, RAYON_PROJECTEUR);
      c.setTransform(1, 0, 0, 1, 0, 0);
      c.clip("evenodd");
    }
    c.fillRect(bx0, by0, bx1 - bx0, by1 - by0);
  }
  c.restore();
}

// Contour d'un rectangle arrondi ajouté au chemin en cours (sans `beginPath`).
function rectArrondiSuite(c, x, y, w, h, r) {
  r = Math.max(0, Math.min(r, Math.abs(w) / 2, Math.abs(h) / 2));
  c.moveTo(x + r, y);
  c.arcTo(x + w, y, x + w, y + h, r);
  c.arcTo(x + w, y + h, x, y + h, r);
  c.arcTo(x, y + h, x, y, r);
  c.arcTo(x, y, x + w, y, r);
  c.closePath();
}

// Peint la vue : `fond` mis à l'échelle, puis les objets en vectoriel, et la sélection.
// `vue` : { x, y, echelle, dpr } : coin haut gauche de l'IMAGE ENTIÈRE dans la toile (px
// physiques, entiers), px de la toile par px de l'image, `devicePixelRatio`. `selection` :
// liste d'`id`. `options` (en plus du contrat) : { cacher: id à ne pas dessiner (texte en
// saisie), ajout: objet en cours de tracé, dessiné au-dessus (un masque en cours y est
// appliqué sur sa seule zone, un projecteur s'ajoute à la réunion), rayon: rayon des coins
// de l'image affichée, en px de la toile (l'écran seul ; absent ou 0 : coins droits),
// gommes: Map id → passages de gomme du geste en cours (aperçu, pas encore dans le
// document), cadres: vrai quand la page ne montre pas de poignées (une flèche ou une ligne
// choisie reçoit alors un cadre) }.
export function peindreVue(ctx, doc, fond, vue, selection, options = {}) {
  const c = ctx, e = vue.echelle, k = cadre(doc);
  tour++;
  c.save();
  c.setTransform(1, 0, 0, 1, 0, 0);
  c.clearRect(0, 0, c.canvas.width, c.canvas.height);
  const x0 = Math.round(vue.x + k.x * e), y0 = Math.round(vue.y + k.y * e);
  const x1 = Math.round(vue.x + (k.x + k.largeur) * e);
  const y1 = Math.round(vue.y + (k.y + k.hauteur) * e);
  c.imageSmoothingEnabled = e < 1;
  c.imageSmoothingQuality = "high";
  const rayon = Math.max(0, options.rayon || 0);
  if (rayon) {
    rectArrondi(c, x0, y0, x1 - x0, y1 - y0, rayon);
    c.clip();
  }
  c.drawImage(fond, k.x, k.y, k.largeur, k.hauteur, x0, y0, x1 - x0, y1 - y0);
  if (!rayon) {
    c.beginPath();
    c.rect(x0, y0, x1 - x0, y1 - y0);
    c.clip();
  }
  const ajout = options.ajout || null;
  if (ajout && ajout.type === "masque") masqueEnCours(c, doc, fond, ajout, vue);
  c.setTransform(e, 0, 0, e, vue.x, vue.y);
  const gommes = options.gommes || null;
  bornes = [Math.max(0, x0), Math.max(0, y0), Math.min(c.canvas.width, x1),
    Math.min(c.canvas.height, y1)];
  try {
    for (const o of doc.objets) {
      if (o.id !== options.cacher) dessinerObjet(c, doc, o, gommes?.get(o.id), true);
    }
    if (ajout && ajout.type !== "masque") dessinerObjet(c, doc, ajout);
  } finally {
    bornes = null;
  }
  peindreVoile(c, projecteurs(doc.objets, ajout, options.cacher), doc.largeur, doc.hauteur);
  c.restore();
  const choisis = new Set(selection || []);
  if (!choisis.size) return;
  c.save();
  c.setTransform(1, 0, 0, 1, 0, 0);
  const d = vue.dpr || 1;
  c.beginPath();
  c.rect(x0 - 7 * d, y0 - 7 * d, x1 - x0 + 14 * d, y1 - y0 + 14 * d);
  c.clip();
  for (const o of doc.objets) {
    if (choisis.has(o.id)) contourSelection(c, o, vue, choisis.size > 1 || !!options.cadres);
  }
  c.restore();
}

// Aperçu d'un masque en cours de tracé : appliqué sur une copie de sa seule zone du fond.
function masqueEnCours(c, doc, fond, o, vue) {
  const n = zone(o, doc.largeur, doc.hauteur);
  if (!n) return;
  const t = toile(n.w, n.h), tc = t.getContext("2d", { willReadFrequently: true });
  tc.globalCompositeOperation = "copy";
  tc.drawImage(fond, n.x, n.y, n.w, n.h, 0, 0, n.w, n.h);
  tc.globalCompositeOperation = "source-over";
  masquer(tc, o, doc.largeur, doc.hauteur, n.x, n.y);
  const e = vue.echelle;
  c.drawImage(t, vue.x + n.x * e, vue.y + n.y * e, n.w * e, n.h * e);
  t.width = 0;
}
