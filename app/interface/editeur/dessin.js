// Rendu à l'écran et composition de l'export de l'éditeur (contrat 0.4 « Éditeur », §3.3 et
// §3.4) : un seul code dessine la vue et l'image exportée (Canvas 2D). Les constantes sont
// des couleurs et des mesures de l'image, pas de l'habillage : ni jetons ni thème. Formes,
// ombres et formules : celles du design (`design/editeur/index.html`, piste A v2). Les
// poignées et le recadrage en cours sont dessinés par la page, par-dessus la vue.

import {
  boiteForme, cadre, longueurPointe, mesureTexte, numeroAffiche, policeTexte, POLICE,
  TRAITS, TRAITS_SURLIGNEUR,
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
export const COULEURS_PERSO_MAX = 3; // « Couleur perso », les plus récentes d'abord

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
// la main, pour que la vue zoomée montre l'ombre de l'export.
function poserOmbre(c, couleur, flou, dy) {
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
  poserOmbre(c, "rgba(0,0,0,.22)", 6, 1.5);
  c.fillStyle = c.strokeStyle = o.couleur;
  c.lineJoin = "round";
  c.lineCap = "round";
  c.lineWidth = Math.max(1, W * 0.25);
  const hl = longueurPointe(o), sc = hl / (W * 3.4 + 14), Ws = W * Math.max(0.5, sc);
  if (o.cx != null) {
    const ex = o.x2 - o.cx, ey = o.y2 - o.cy, el = Math.hypot(ex, ey) || 1;
    const ux = ex / el, uy = ey / el;
    const sx = o.x1 - o.cx, sy = o.y1 - o.cy, sl = Math.hypot(sx, sy) || 1;
    c.lineWidth = Ws * 1.05;
    c.beginPath();
    c.moveTo(o.x1, o.y1);
    c.quadraticCurveTo(o.cx, o.cy, o.x2 - ux * hl * 0.7, o.y2 - uy * hl * 0.7);
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

// Dessine un objet (hors masque, dans le fond, et hors projecteur, peint par `peindreVoile`)
// dans `ctx`, dont la
// transformation est déjà posée (échelle de la vue, ou 1 pour l'export). `doc` sert au
// chiffre des numéros.
export function dessinerObjet(ctx, doc, objet) {
  const c = ctx, o = objet;
  if (o.type === "masque" || o.type === "projecteur") return;
  c.save();
  c.globalAlpha = o.type === "surligneur" ? SURLIGNEUR_ALPHA : alphaDe(o);
  switch (o.type) {
    case "fleche":
      dessinerFleche(c, o);
      break;
    case "rectangle": case "ellipse": {
      const n = boiteForme(o);
      poserOmbre(c, "rgba(0,0,0,.2)", 6, 1);
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
      poserOmbre(c, "rgba(0,0,0,.2)", 5, 1);
      c.strokeStyle = o.couleur;
      c.lineWidth = trait(o);
      c.lineCap = "round";
      c.beginPath();
      c.moveTo(o.x1, o.y1);
      c.lineTo(o.x2, o.y2);
      c.stroke();
      break;
    case "crayon":
      poserOmbre(c, "rgba(0,0,0,.18)", 4, 1);
      c.strokeStyle = c.fillStyle = o.couleur;
      trace(c, o.points, trait(o));
      break;
    case "surligneur":
      c.globalCompositeOperation = "multiply";
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
  c.restore();
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
  const t = toile(doc.largeur, doc.hauteur);
  const c = t.getContext("2d", { willReadFrequently: true });
  c.globalCompositeOperation = "copy";
  c.imageSmoothingEnabled = false;
  c.drawImage(fond, 0, 0);
  c.globalCompositeOperation = "source-over";
  for (const o of doc.objets) dessinerObjet(c, doc, o);
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

// Contour de sélection du design (accent, tirets 4/3, 70 %), en px de la toile.
function contourSelection(c, o, vue) {
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
  } else if (o.type === "fleche" || o.type === "ligne") {
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

// Voile sur toute l'image (l × h px de l'image, dans la transformation posée sur `c`), percé
// de la RÉUNION des projecteurs : peint sur un calque de la taille de la toile, d'où chaque
// zone est ôtée (`destination-out` : deux zones qui se chevauchent ne se voilent pas), puis
// posé d'un coup. Les coupes de `c` (cadre de la vue) s'appliquent à la pose.
function peindreVoile(c, liste, l, h) {
  if (!liste.length) return;
  const t = toile(c.canvas.width, c.canvas.height), v = t.getContext("2d");
  v.setTransform(c.getTransform());
  v.fillStyle = VOILE_PROJECTEUR;
  v.fillRect(0, 0, l, h);
  v.globalCompositeOperation = "destination-out";
  v.fillStyle = "#000";
  for (const o of liste) {
    const n = boiteForme(o);
    if (o.forme === "ovale") {
      v.beginPath();
      v.ellipse(n.x + n.largeur / 2, n.y + n.hauteur / 2, Math.max(0.5, n.largeur / 2),
        Math.max(0.5, n.hauteur / 2), 0, 0, 7);
    } else rectArrondi(v, n.x, n.y, n.largeur, n.hauteur, RAYON_PROJECTEUR);
    v.fill();
  }
  c.save();
  c.setTransform(1, 0, 0, 1, 0, 0);
  c.globalAlpha = 1;
  c.globalCompositeOperation = "source-over";
  c.drawImage(t, 0, 0);
  c.restore();
  t.width = t.height = 0;
}

// Peint la vue : `fond` mis à l'échelle, puis les objets en vectoriel, et la sélection.
// `vue` : { x, y, echelle, dpr } : coin haut gauche de l'IMAGE ENTIÈRE dans la toile (px
// physiques, entiers), px de la toile par px de l'image, `devicePixelRatio`. `selection` :
// liste d'`id`. `options` (en plus du contrat) : { cacher: id à ne pas dessiner (texte en
// saisie), ajout: objet en cours de tracé, dessiné au-dessus (un masque en cours y est
// appliqué sur sa seule zone, un projecteur s'ajoute à la réunion), rayon: rayon des coins
// de l'image affichée, en px de la toile (l'écran seul ; absent ou 0 : coins droits) }.
export function peindreVue(ctx, doc, fond, vue, selection, options = {}) {
  const c = ctx, e = vue.echelle, k = cadre(doc);
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
  for (const o of doc.objets) if (o.id !== options.cacher) dessinerObjet(c, doc, o);
  if (ajout && ajout.type !== "masque") dessinerObjet(c, doc, ajout);
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
  for (const o of doc.objets) if (choisis.has(o.id)) contourSelection(c, o, vue);
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
