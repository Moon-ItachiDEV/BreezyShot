// Document de l'éditeur d'annotation (contrat 0.4 « Éditeur », §3.2) : objets, ordre,
// recadrage et historique, en pixels de l'image. Module pur, sans DOM, essayé sous Node
// (`essais/pages/editeur-modele.cjs`).
//
// SOCLE : les noms et les signatures sont posés ; chaque corps « Provisoire (B) » est
// remplacé par le chantier B.
//
// Document : { largeur, hauteur, objets: Objet[], recadrage: Rect|null, suivant: number }
// Rect : { x, y, largeur, hauteur } ; l'ordre de `objets` est l'ordre de dessin.
// Objet : voir le §3.2 du contrat (fleche, rectangle, ellipse, ligne, crayon, surligneur,
// texte, numero, masque).
// Historique : { doc, passe, futur, version }.

export function nouveauDocument(largeur, hauteur) {
  // Provisoire (B) : document vide, sans contrôle.
  return { largeur, hauteur, objets: [], recadrage: null, suivant: 1 };
}

export function creerHistorique(doc) {
  // Provisoire (B) : historique vide.
  return { doc, passe: [], futur: [], version: 0 };
}

// `fn(doc)` ; même `marque` que la modification précédente : un seul pas d'annulation.
export function modifier(h, fn, marque = null) {
  // Provisoire (B) : appliqué sans historique.
  void marque;
  fn(h.doc);
  h.version += 1;
}

export function annuler(h) {
  // Provisoire (B) : rien.
  void h;
}

export function retablir(h) {
  // Provisoire (B) : rien.
  void h;
}

export function peutAnnuler(h) {
  // Provisoire (B) : jamais.
  void h;
  return false;
}

export function peutRetablir(h) {
  // Provisoire (B) : jamais.
  void h;
  return false;
}

// Boîte de l'objet, ombre et pointe comprises.
export function boite(objet) {
  // Provisoire (B) : boîte vide.
  void objet;
  return { x: 0, y: 0, largeur: 0, hauteur: 0 };
}

export function touche(objet, point, tolerance, strict) {
  // Provisoire (B) : jamais.
  void objet, point, tolerance, strict;
  return false;
}

// L'objet du dessus sous `point`, ou null.
export function objetSous(doc, point, tolerance, filtreType) {
  // Provisoire (B) : aucun.
  void doc, point, tolerance, filtreType;
  return null;
}

// [[x, y, nom], …] : p1 p2 pc | nw ne sw se.
export function poignees(objet) {
  // Provisoire (B) : aucune.
  void objet;
  return [];
}

export function deplacer(objet, origine, dx, dy) {
  // Provisoire (B) : rien.
  void objet, origine, dx, dy;
}

export function redimensionner(objet, origine, poignee, point, echelleVue) {
  // Provisoire (B) : rien.
  void objet, origine, poignee, point, echelleVue;
}

// Rend les ids des copies (décalage 16 px, nouvel ordre pour les numéros).
export function dupliquer(doc, ids) {
  // Provisoire (B) : aucune copie.
  void doc, ids;
  return [];
}

export function supprimer(doc, ids) {
  // Provisoire (B) : rien.
  void doc, ids;
}

// `devant` vrai : premier plan ; faux : arrière-plan (un masque reste parmi les masques).
export function ordonner(doc, ids, devant) {
  // Provisoire (B) : rien.
  void doc, ids, devant;
}

// Rang du numéro (1, 2, 3…) par `ordre` parmi les numéros.
export function numeroAffiche(doc, objet) {
  // Provisoire (B) : toujours 1.
  void doc, objet;
  return 1;
}

// Le recadrage, ou l'image entière.
export function cadre(doc) {
  // Provisoire (B) : l'image entière.
  return doc.recadrage ?? { x: 0, y: 0, largeur: doc.largeur, hauteur: doc.hauteur };
}

// Aucun objet et aucun recadrage.
export function vide(doc) {
  // Provisoire (B) : d'après les seuls champs.
  return doc.objets.length === 0 && doc.recadrage === null;
}
