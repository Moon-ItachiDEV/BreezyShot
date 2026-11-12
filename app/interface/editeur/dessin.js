// Rendu à l'écran et composition de l'export de l'éditeur (contrat 0.4 « Éditeur », §3.3 et
// §3.4) : un seul code dessine la vue et l'image exportée (Canvas 2D). Les constantes sont
// des couleurs et des mesures de l'image, pas de l'habillage : ni jetons ni thème.
//
// SOCLE : les noms et les signatures sont posés ; chaque corps « Provisoire (B) » est
// remplacé par le chantier B (poignées et recadrage en cours : chantier C, dans la page).

export const COULEURS = [
  ["#E5484D", "Rouge"],
  ["#F5A524", "Orange"],
  ["#FFD43B", "Jaune"],
  ["#30A46C", "Vert"],
  ["#0090FF", "Bleu"],
  ["#8E4EC6", "Violet"],
  ["#1A1A1A", "Noir"],
  ["#FFFFFF", "Blanc"],
];
export const TRAITS = [3, 5, 8]; // Fin, Moyen, Épais (px de l'image)
export const TRAITS_SURLIGNEUR = [16, 24, 34];
export const RAYONS_NUMERO = [12, 16, 21]; // P, M, G
export const BLOCS = [8, 13, 20]; // Pixeliser : côté du bloc
export const FLOUS = [6, 10, 16]; // Flouter : facteur de réduction
export const COULEURS_PERSO_MAX = 3; // « Couleur perso », les plus récentes d'abord

// Dessine un objet (hors masque) dans `ctx`, dont la transformation est déjà posée
// (échelle de la vue, ou 1 pour l'export). `doc` sert au chiffre des numéros.
export function dessinerObjet(ctx, doc, objet) {
  // Provisoire (B) : rien n'est dessiné.
  void ctx, doc, objet;
}

// Peint dans `ctxFond` (toile pleine taille, logicielle) la base puis les masques de `doc`.
export function peindreMasques(ctxFond, base, doc) {
  // Provisoire (B) : rien n'est peint.
  void ctxFond, base, doc;
}

// L'image exportée, à la taille du cadre : { largeur, hauteur, rgba } (Uint8ClampedArray).
export function composer(doc, fond) {
  // Provisoire (B) : image vide à la taille du cadre.
  void fond;
  const c = doc.recadrage ?? { largeur: doc.largeur, hauteur: doc.hauteur };
  const rgba = new Uint8ClampedArray(c.largeur * c.hauteur * 4);
  return { largeur: c.largeur, hauteur: c.hauteur, rgba };
}

// Peint la vue : `fond` mis à l'échelle, puis les objets en vectoriel, et la sélection.
// `vue` : { x, y, echelle, dpr } (position de l'image et échelle, pixels de la toile).
export function peindreVue(ctx, doc, fond, vue, selection) {
  // Provisoire (B) : rien n'est peint.
  void ctx, doc, fond, vue, selection;
}
