// Essai de la lecture d'un fichier .breezy par la page de l'éditeur (0.7.5) : `lireDocument`
// de `app/interface/editeur/document.js`, sous Node seul. Un fichier vient du disque : faux,
// abîmé ou fait pour nuire, il ne doit jamais fournir autre chose qu'un document propre.
// Sort avec 1 si une vérification échoue.
//   node essais/pages/editeur-breezy.cjs
const path = require('path');
const { pathToFileURL } = require('url');
const RACINE = path.join(__dirname, '..', '..', 'app', 'interface');
let echecs = 0;
const verifier = (ok, quoi, info = '') => {
  console.log(ok ? 'OK   ' : 'ECHEC', quoi, info);
  if (!ok) echecs++;
};

(async () => {
  require('module').registerHooks({
    load: (url, contexte, suite) => (url.startsWith(pathToFileURL(RACINE).href)
      ? suite(url, { ...contexte, format: 'module' }) : suite(url, contexte)),
  });
  const D = await import(pathToFileURL(path.join(RACINE, 'editeur', 'document.js')).href);
  const lire = (o, l = 400, h = 300) => D.lireDocument(typeof o === 'string' ? o : JSON.stringify(o), l, h);
  const leve = (o) => { try { lire(o); return false; } catch { return true; } };

  // 1. Un document fait par l'éditeur revient identique.
  const h = D.creerHistorique(D.nouveauDocument(400, 300));
  D.modifier(h, (d) => {
    D.ajouter(d, { type: 'fleche', x1: 10, y1: 10, x2: 110, y2: 60, cx: 40, cy: 80, double: true,
      couleur: '#E5484D', epaisseur: 2, opacite: 0.5 });
    D.ajouter(d, { type: 'rectangle', x1: 5, y1: 5, x2: 50, y2: 40, couleur: '#FFFFFF',
      epaisseur: 1, plein: true, arrondi: 6, opacite: 1 });
    D.ajouter(d, { type: 'ellipse', x1: 5, y1: 5, x2: 50, y2: 40, couleur: '#1A1A1A',
      epaisseur: 0, plein: false, opacite: 1 });
    D.ajouter(d, { type: 'ligne', x1: 0, y1: 0, x2: 9, y2: 9, couleur: '#1A1A1A', epaisseur: 1, opacite: 1 });
    D.ajouter(d, { type: 'crayon', points: [[1, 1], [2.5, 3.5], [8, 9]], couleur: '#2D7FF9',
      epaisseur: 1, opacite: 1, gommes: [{ largeur: 12, points: [[2, 2], [3, 3]] }] });
    D.ajouter(d, { type: 'surligneur', points: [[1, 1], [9, 1]], couleur: '#FFD60A', epaisseur: 1 });
    D.ajouter(d, { type: 'texte', x: 30, y: 40, texte: 'Salut\nà toi', couleur: '#1A1A1A',
      taille: 28, gras: true, style: 'etiquette', opacite: 1 });
    D.ajouter(d, { type: 'numero', x: 60, y: 60, rayon: 16, couleur: '#E5484D', opacite: 1 });
    D.ajouter(d, { type: 'masque', x1: 0, y1: 0, x2: 40, y2: 40, sorte: 'flouter', intensite: 2 });
    D.ajouter(d, { type: 'projecteur', x1: 20, y1: 20, x2: 90, y2: 90, forme: 'ovale' });
    D.recadrer(d, { x: 10, y: 10, largeur: 200, hauteur: 100 });
  });
  const original = JSON.parse(JSON.stringify(h.doc));
  const { doc, ecartes } = lire(JSON.stringify(h.doc));
  verifier(JSON.stringify(doc) === JSON.stringify(original) || (() => {
    // L'ordre des champs peut différer : on compare la valeur, pas le texte.
    const a = JSON.parse(JSON.stringify(doc));
    return JSON.stringify(sort(a)) === JSON.stringify(sort(original));
  })(), 'un document de l\'éditeur revient identique', `${doc.objets.length} objets`);
  verifier(ecartes === 0, 'aucun objet écarté');
  function sort(v) {
    if (Array.isArray(v)) return v.map(sort);
    if (v && typeof v === 'object') return Object.fromEntries(Object.keys(v).sort().map((k) => [k, sort(v[k])]));
    return v;
  }

  // 2. Pas un document.
  for (const mauvais of ['', '{', '[]', '12', 'null', '"x"', 'true']) {
    verifier(leve(mauvais), `refusé : ${JSON.stringify(mauvais)}`);
  }
  verifier(leve('x'.repeat(D.BREEZY_OCTETS_MAX + 1)), 'refusé : trop gros');
  let nonTexte = false;
  try { D.lireDocument(42, 1, 1); } catch { nonTexte = true; }
  verifier(nonTexte, 'refusé : pas un texte');

  // 3. Objets inconnus ou faux écartés, le reste lu.
  const sale = lire({ objets: [
    { type: 'virus', x: 1 }, null, 12, 'a', [], { type: 'fleche' },
    { type: 'texte', texte: 5, taille: 'énorme', x: Infinity, y: NaN },
  ] });
  verifier(sale.ecartes === 5 && sale.doc.objets.length === 2, 'inconnus écartés, vides remplis', `écartés ${sale.ecartes}`);
  const t = sale.doc.objets[1];
  verifier(t.texte === '' && t.taille === 28 && t.x === 0 && t.y === 0, 'texte : valeurs de repli');

  // 4. Nombres : bornés, jamais NaN ni infinis ; coordonnées hors image gardées dans les bornes.
  const n = lire({ objets: [{ type: 'rectangle', x1: 1e30, y1: -1e30, x2: '5', y2: null,
    couleur: 'rouge', epaisseur: 99, plein: 'oui', arrondi: -4, opacite: 250 }] }).doc.objets[0];
  verifier(n.x1 === 65536 && n.y1 === -65536 && n.x2 === 0 && n.y2 === 0, 'coordonnées bornées', JSON.stringify([n.x1, n.y1, n.x2, n.y2]));
  verifier(n.couleur === '#1A1A1A' && n.epaisseur === 2 && n.plein === false && n.arrondi === 0, 'couleur, niveau, booléen, arrondi');
  verifier(n.opacite === 1, 'opacité > 1 lue en pour cent puis bornée', String(n.opacite));
  verifier(lire({ objets: [{ type: 'ligne', opacite: 50 }] }).doc.objets[0].opacite === 0.5, 'opacité 50 = 0,5');

  // 5. Couleurs : hexadécimales seulement (aucun texte ne passe dans un style).
  for (const c of ['url(javascript:x)', '#12', '#GGGGGG', 'red', '#12345678', 5, null]) {
    const o = lire({ objets: [{ type: 'ligne', couleur: c }] }).doc.objets[0];
    verifier(o.couleur === '#1A1A1A', `couleur refusée : ${JSON.stringify(c)}`);
  }

  // 6. Identifiants : manquants, en double, négatifs ou faux reçoivent un numéro neuf, `suivant` au-dessus.
  const ids = lire({ suivant: 3, objets: [
    { type: 'ligne', id: 5 }, { type: 'ligne', id: 5 }, { type: 'ligne', id: -1 },
    { type: 'ligne', id: 'x' }, { type: 'ligne' }, { type: 'ligne', id: 2.5 },
  ] }).doc;
  const liste = ids.objets.map((o) => o.id);
  verifier(new Set(liste).size === liste.length && liste.every((i) => Number.isInteger(i) && i > 0), 'identifiants uniques et positifs', liste.join(','));
  verifier(ids.suivant > Math.max(...liste), '`suivant` au-dessus de tous');
  verifier(liste[0] === 5, 'le premier identifiant valide est gardé');

  // 7. Plafonds.
  const beaucoup = lire({ objets: Array.from({ length: 5000 }, () => ({ type: 'ligne' })) });
  verifier(beaucoup.doc.objets.length === 2000 && beaucoup.ecartes === 3000, 'objets plafonnés à 2000');
  const gros = lire({ objets: [{ type: 'crayon', points: Array.from({ length: 50000 }, (_, i) => [i, i]) }] });
  verifier(gros.doc.objets[0].points.length === 20000, 'points d\'un trait plafonnés');
  const trace = Array.from({ length: 20000 }, (_, i) => [i % 300, i % 200]);
  const total = lire({ objets: Array.from({ length: 60 }, () => ({ type: 'crayon', points: trace })) });
  const sommePoints = total.doc.objets.reduce((s, o) => s + o.points.length, 0);
  verifier(sommePoints <= 400000, 'points au total plafonnés', String(sommePoints));
  verifier(lire({ objets: [{ type: 'texte', texte: 'a'.repeat(100000) }] }).doc.objets[0].texte.length === 5000, 'texte plafonné');
  const gommes = lire({ objets: [{ type: 'crayon', points: [[1, 1]], gommes: Array.from({ length: 5000 }, () => ({ largeur: 9, points: [[1, 1]] })) }] });
  verifier(gommes.doc.objets[0].gommes.length === 2000, 'gommes plafonnées');
  verifier(!('gommes' in lire({ objets: [{ type: 'texte', gommes: [{ largeur: 1, points: [[1, 1]] }] }] }).doc.objets[0]), 'une gomme sur un texte est ignorée');

  // 7 bis. Un trait sans point n'a pas de boîte : écarté.
  const vides = lire({ objets: [{ type: 'crayon', points: [] }, { type: 'surligneur' }, { type: 'crayon', points: 'x' }, { type: 'crayon', points: [[1, 1]] }] });
  verifier(vides.doc.objets.length === 1 && vides.ecartes === 3, 'traits sans point écartés', `${vides.doc.objets.length} gardé(s), ${vides.ecartes} écarté(s)`);

  // 8. Noms inventés : jamais copiés ; __proto__ sans effet.
  const proto = lire('{"objets":[{"type":"ligne","__proto__":{"x":1},"constructor":{"a":2},"evil":"x"}],"__proto__":{"polluee":true},"extra":1}');
  verifier(!('evil' in proto.doc.objets[0]) && !Object.hasOwn(proto.doc.objets[0], 'constructor') && !Object.hasOwn(proto.doc.objets[0], '__proto__') && ({}).polluee === undefined && proto.doc.polluee === undefined, 'champs inventés ignorés, rien ne pollue');
  verifier(Object.keys(proto.doc).sort().join() === 'hauteur,largeur,objets,recadrage,suivant', 'le document n\'a que ses champs', Object.keys(proto.doc).join());

  // 9. La taille de l'image fait foi ; recadrage borné ; faux recadrage ignoré.
  const taille = lire({ largeur: 9, hauteur: 9, objets: [] }, 400, 300).doc;
  verifier(taille.largeur === 400 && taille.hauteur === 300, 'la taille vient de l\'image');
  const rc = lire({ objets: [], recadrage: { x: -50, y: 20, largeur: 1e9, hauteur: 100 } }).doc.recadrage;
  verifier(rc && rc.x === 0 && rc.largeur <= 400, 'recadrage borné à l\'image', JSON.stringify(rc));
  verifier(lire({ objets: [], recadrage: 'x' }).doc.recadrage === null, 'recadrage faux ignoré');
  verifier(lire({ objets: [], recadrage: { x: 0, y: 0, largeur: 0, hauteur: 0 } }).doc.recadrage === null, 'recadrage vide retiré');
  verifier(lire({ objets: 'x' }).doc.objets.length === 0, 'objets pas une liste : document vide');

  // 10. Le document lu s'utilise : l'historique, l'annulation, l'ajout.
  const lu = lire(JSON.stringify(h.doc)).doc;
  const hl = D.creerHistorique(lu);
  D.modifier(hl, (d) => D.ajouter(d, { type: 'ligne', x1: 0, y1: 0, x2: 1, y2: 1, couleur: '#1A1A1A', epaisseur: 1, opacite: 1 }));
  const ajoute = hl.doc.objets[hl.doc.objets.length - 1];
  verifier(ajoute.id === h.doc.suivant, 'l\'ajout suivant prend le bon identifiant', String(ajoute.id));
  verifier(D.annuler(hl) && hl.doc.objets.length === original.objets.length, 'annuler après la lecture');
  verifier(hl.version > 0 && D.creerHistorique(lu).version === 0, 'la version de départ est 0');

  // 11. Mesures : un gros fichier plausible se lit vite.
  const grosJson = JSON.stringify({ objets: Array.from({ length: 2000 }, (_, i) => ({ type: 'crayon', points: Array.from({ length: 150 }, (_, k) => [k, i]) })) });
  const t0 = process.hrtime.bigint();
  lire(grosJson);
  const ms = Number(process.hrtime.bigint() - t0) / 1e6;
  verifier(ms < 1500, `2000 traits de 150 points lus en ${ms.toFixed(0)} ms`);

  console.log(echecs ? `${echecs} vérification(s) en échec` : 'tout est bon');
  process.exit(echecs ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
