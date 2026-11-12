const { chromium } = require('playwright');
const http = require('http'), fs = require('fs'), path = require('path');
const { pathToFileURL } = require('url');
// Essai du modèle et du dessin de l'éditeur (contrat 0.4 « Éditeur », §3.2 à §3.4, §12.2).
// Partie 1, Node seul : `document.js` (historique, géométrie, numéros, ordre, recadrage).
// Partie 2, Chromium (Playwright) : `dessin.js` sous la CSP de l'app (exactitude au pixel
// de `composer`, masques irréversibles, vue = export, temps de Pixeliser et de `composer`).
// Images faites d'un bruit à graine fixe : rien ne dépend du hasard. Sort avec 1 si une
// vérification échoue.
//   node essais/pages/editeur-modele.cjs
const RACINE = path.join(__dirname, '..', '..', 'app', 'interface');
const CONF = path.join(__dirname, '..', '..', 'app', 'coeur', 'tauri.conf.json');
const CSP = Object.entries(JSON.parse(fs.readFileSync(CONF, 'utf8')).app.security.csp)
  .map(([k, v]) => `${k} ${v}`).join('; ');
const PAGE_VIDE = '<!doctype html><meta charset="utf-8"><title>essai</title><body>';
const serveur = http.createServer((q, r) => {
  const chemin = decodeURIComponent(new URL(q.url, 'http://x').pathname);
  const entetes = { 'content-security-policy': CSP };
  if (chemin === '/essai.html') {
    r.writeHead(200, { ...entetes, 'content-type': 'text/html' });
    return r.end(PAGE_VIDE);
  }
  const p = path.join(RACINE, chemin);
  if (!p.startsWith(RACINE) || !fs.existsSync(p) || !p.endsWith('.js')) {
    r.writeHead(404);
    return r.end();
  }
  r.writeHead(200, { ...entetes, 'content-type': 'text/javascript' });
  fs.createReadStream(p).pipe(r);
});

let echecs = 0;
const verifier = (ok, quoi, info = '') => {
  console.log(ok ? 'OK   ' : 'ECHEC', quoi, info);
  if (!ok) echecs++;
};

// ---------- partie 1 : document.js sous Node ----------

async function partieDocument() {
  // Les modules de la page sont des modules ES sans `package.json` qui le dise : Node les
  // charge comme tels sans avertissement.
  require('module').registerHooks({
    load: (url, contexte, suite) => (url.startsWith(pathToFileURL(RACINE).href)
      ? suite(url, { ...contexte, format: 'module' }) : suite(url, contexte)),
  });
  const D = await import(pathToFileURL(path.join(RACINE, 'editeur', 'document.js')).href);
  const exemples = () => [
    { type: 'fleche', x1: 10, y1: 10, x2: 110, y2: 60, double: false, couleur: '#E5484D',
      epaisseur: 1, opacite: 1 },
    { type: 'fleche', x1: 10, y1: 100, x2: 200, y2: 100, cx: 100, cy: 20, double: true,
      couleur: '#0090FF', epaisseur: 2, opacite: 1 },
    { type: 'rectangle', x1: 50, y1: 50, x2: 150, y2: 120, couleur: '#E5484D', epaisseur: 1,
      plein: false, arrondi: 10, opacite: 1 },
    { type: 'ellipse', x1: 200, y1: 50, x2: 300, y2: 150, couleur: '#30A46C', epaisseur: 0,
      plein: false, opacite: 0.5 },
    { type: 'ligne', x1: 0, y1: 200, x2: 100, y2: 300, couleur: '#1A1A1A', epaisseur: 2,
      opacite: 1 },
    { type: 'crayon', points: [[10, 300], [20, 310], [40, 305], [60, 330]], couleur: '#8E4EC6',
      epaisseur: 1, opacite: 1 },
    { type: 'surligneur', points: [[100, 400], [200, 400]], couleur: '#FFD43B', epaisseur: 1 },
    { type: 'texte', x: 300, y: 300, texte: 'Bonjour', couleur: '#E5484D', taille: 28,
      gras: true, style: 'contour', opacite: 1 },
    { type: 'numero', x: 400, y: 100, rayon: 16, couleur: '#E5484D', opacite: 1 },
    { type: 'masque', x1: 400, y1: 200, x2: 480, y2: 260, sorte: 'pixeliser', intensite: 1 },
  ];
  const json = (d) => JSON.stringify(d);

  // (1) créer, annuler, rétablir chaque type ; le document garde son identité.
  {
    const doc = D.nouveauDocument(640, 480), h = D.creerHistorique(doc);
    const etats = [json(doc)];
    for (const o of exemples()) {
      D.modifier(h, (d) => D.ajouter(d, o));
      etats.push(json(doc));
    }
    let ok = doc.objets.length === 10 && new Set(doc.objets.map((o) => o.id)).size === 10;
    for (let i = etats.length - 1; i > 0; i--) {
      ok = ok && json(h.doc) === etats[i] && D.annuler(h);
    }
    ok = ok && json(doc) === etats[0] && !D.peutAnnuler(h) && !D.annuler(h) && h.doc === doc;
    for (let i = 1; i < etats.length; i++) ok = ok && D.retablir(h) && json(doc) === etats[i];
    ok = ok && !D.peutRetablir(h) && !D.retablir(h);
    verifier(ok, '1 chaque type créé, annulé, rétabli (même objet document)');
  }
  // (2) futur vidé par une modification ; (5) version croît toujours.
  {
    const doc = D.nouveauDocument(100, 100), h = D.creerHistorique(doc), versions = [h.version];
    const note = () => versions.push(h.version);
    D.modifier(h, (d) => D.ajouter(d, exemples()[2])); note();
    D.modifier(h, (d) => D.ajouter(d, exemples()[3])); note();
    D.annuler(h); note();
    D.annuler(h); note();
    D.retablir(h); note();
    const avant = D.peutRetablir(h);
    D.modifier(h, (d) => { d.objets[0].couleur = '#FFFFFF'; }); note();
    verifier(avant && !D.peutRetablir(h) && h.futur.length === 0, '2 futur vidé à la modification');
    const croit = versions.every((v, i) => i === 0 || v === versions[i - 1] + 1);
    verifier(croit && versions[0] === 0, '5 version +1 à chaque modifier, annuler, rétablir',
      versions.join(','));
  }
  // (3) 30 pas d'une glissière marquée = un pas ; une autre marque ou aucune : un pas neuf.
  {
    const doc = D.nouveauDocument(100, 100), h = D.creerHistorique(doc);
    D.modifier(h, (d) => D.ajouter(d, exemples()[2]));
    for (let i = 1; i <= 30; i++) D.modifier(h, (d) => { d.objets[0].opacite = i / 30; }, 'op');
    const un = h.passe.length === 2;
    D.annuler(h);
    const retour = doc.objets[0].opacite === 1;
    D.modifier(h, (d) => { d.objets[0].arrondi = 2; }, 'r');
    D.modifier(h, (d) => { d.objets[0].arrondi = 3; }, 'r');
    D.modifier(h, (d) => { d.objets[0].arrondi = 4; });
    D.modifier(h, (d) => { d.objets[0].arrondi = 5; }, 'r');
    verifier(un && retour && h.passe.length === 4 && h.version === 36,
      '3 trente pas marqués = un pas ; marque coupée par un pas sans marque',
      `${h.passe.length} ${h.version}`);
  }
  // (4) 201 modifications : 200 annulables, la plus ancienne tombe.
  {
    const doc = D.nouveauDocument(100, 100), h = D.creerHistorique(doc);
    for (let i = 0; i < 201; i++) D.modifier(h, (d) => D.ajouter(d, exemples()[8]));
    let n = 0;
    while (D.annuler(h)) n++;
    verifier(n === 200 && doc.objets.length === 1, '4 201 modifications : 200 annulables', n);
  }
  // Une erreur dans `fn` laisse le document et l'historique comme avant.
  {
    const doc = D.nouveauDocument(100, 100), h = D.creerHistorique(doc);
    let leve = false;
    try {
      D.modifier(h, (d) => { D.ajouter(d, exemples()[0]); throw new Error('voulu'); });
    } catch { leve = true; }
    verifier(leve && doc.objets.length === 0 && !D.peutAnnuler(h) && h.version === 0,
      '4b erreur dans la modification : rien ne change');
  }
  // (6) numéros 1, 2, 3 ; le 2 supprimé → 1, 2 ; dupliquer → ordre suivant.
  {
    const doc = D.nouveauDocument(500, 500);
    const ids = [0, 1, 2].map((i) => D.ajouter(doc, { type: 'numero', x: 50 * i, y: 50,
      rayon: 16, couleur: '#E5484D', opacite: 1 }));
    D.ordonner(doc, [ids[0]], true); // l'ordre de dessin ne change pas le chiffre
    const n = () => ids.map((id) => doc.objets.find((o) => o.id === id))
      .map((o) => (o ? D.numeroAffiche(doc, o) : 0));
    const a = n().join();
    D.supprimer(doc, [ids[1]]);
    const b = n().join();
    const [copie] = D.dupliquer(doc, [ids[0]]);
    const c = doc.objets.find((o) => o.id === copie);
    const orig = doc.objets.find((o) => o.id === ids[0]);
    verifier(a === '1,2,3' && b === '1,0,2' && D.numeroAffiche(doc, c) === 3
      && c.ordre > Math.max(...doc.objets.filter((o) => o !== c).map((o) => o.ordre))
      && c.x === orig.x + 16 && c.y === orig.y + 16 && c.id > ids[2],
    '6 numéros 1,2,3 ; 2 supprimé → 1,2 ; copie : ordre suivant, décalée de 16', `${a} ${b}`);
  }
  // (7) ordonner ; dupliquer plusieurs ; les masques entre eux, sous les autres pour le toucher.
  {
    const doc = D.nouveauDocument(500, 500);
    const m1 = D.ajouter(doc, { type: 'masque', x1: 0, y1: 0, x2: 100, y2: 100,
      sorte: 'bloc', intensite: 1 });
    const r = D.ajouter(doc, { ...exemples()[2], x1: 0, y1: 0, x2: 100, y2: 100, plein: true });
    const m2 = D.ajouter(doc, { type: 'masque', x1: 50, y1: 50, x2: 150, y2: 150,
      sorte: 'flouter', intensite: 1 });
    const ordre = () => doc.objets.map((o) => o.id).join();
    D.ordonner(doc, [m1], true);
    const a = ordre();
    D.ordonner(doc, [m1], false);
    const b = ordre();
    // Le masque m2 est après le rectangle dans la liste, mais peint dessous : le rectangle
    // est au-dessus sous la pointe ; hors du rectangle, le masque du dessus (m2).
    const sous = D.objetSous(doc, { x: 60, y: 60 }, 2, null)?.id;
    const hors = D.objetSous(doc, [140, 140], 2, null)?.id;
    const filtre = D.objetSous(doc, { x: 60, y: 60 }, 2, 'masque')?.id;
    const copies = D.dupliquer(doc, [r, m2]);
    verifier(a === `${r},${m2},${m1}` && b === `${m1},${r},${m2}` && sous === r && hors === m2
      && filtre === m2 && copies.length === 2 && ordre().endsWith(copies.join()),
    '7 ordonner ; masques sous les autres objets, rangés entre eux', `${a} ${b} ${sous}`);
  }
  // (8) touche, boite, poignées de chaque type ; déplacer, redimensionner.
  {
    let ok = true, info = [];
    const noms = {
      fleche: 'p1,p2,pc', ligne: 'p1,p2', rectangle: 'nw,ne,sw,se', ellipse: 'nw,ne,sw,se',
      crayon: 'nw,ne,sw,se', surligneur: 'nw,ne,sw,se', texte: 'nw,ne,sw,se',
      numero: 'nw,ne,sw,se', masque: 'nw,ne,sw,se',
    };
    const sur = { // un point sur l'objet, un point hors de lui
      fleche: [[60, 35], [60, 80]], rectangle: [[50, 80], [100, 85]],
      ellipse: [[250, 50], [250, 100]], ligne: [[50, 250], [80, 250]],
      crayon: [[20, 310], [20, 340]], surligneur: [[150, 410], [150, 420]],
      texte: [[310, 310], [290, 290]], numero: [[400, 100], [400, 120]],
      masque: [[440, 230], [490, 230]],
    };
    for (const o of exemples()) {
      if (o.cx != null) continue;
      const [a, b] = sur[o.type], bt = D.boite(o), bf = D.boiteForme(o);
      const dedans = (r, p) => p[0] >= r.x && p[0] <= r.x + r.largeur
        && p[1] >= r.y && p[1] <= r.y + r.hauteur;
      const p = D.poignees(o);
      const cas = D.touche(o, a, 2, false) && !D.touche(o, b, 2, false)
        && bt.x <= bf.x && bt.y <= bf.y && bt.x + bt.largeur >= bf.x + bf.largeur
        && p.map((q) => q[2]).join() === noms[o.type] && dedans(bt, a)
        && p.every((q) => dedans(bt, q));
      if (!cas) info.push(o.type);
      ok = ok && cas;
    }
    const courbe = exemples()[1];
    const pc = D.poignees(courbe).find((q) => q[2] === 'pc');
    ok = ok && D.touche(courbe, [pc[0], pc[1]], 1, false) && Math.abs(pc[1] - 60) < 1e-9;
    // Rectangle plein : touché au milieu ; vide : pas au milieu.
    const r = exemples()[2];
    ok = ok && !D.touche(r, [100, 85], 2, false) && D.touche({ ...r, plein: true }, [100, 85], 2);
    verifier(ok, '8 touche, boîte (contient la forme et les poignées), poignées de chaque type',
      info.join());

    // Déplacer : chaque type suit (dx, dy) depuis son origine.
    let dep = true;
    for (const o of exemples()) {
      const orig = JSON.parse(JSON.stringify(o)), b0 = D.boite(orig);
      D.deplacer(o, orig, 7, -3);
      D.deplacer(o, orig, 10, -5); // depuis l'origine, pas cumulé
      const b1 = D.boite(o);
      dep = dep && Math.abs(b1.x - b0.x - 10) < 1e-9 && Math.abs(b1.y - b0.y + 5) < 1e-9
        && Math.abs(b1.largeur - b0.largeur) < 1e-9;
    }
    verifier(dep, '8 déplacer : chaque type de (dx, dy) depuis l\'origine');

    // Redimensionner : coins tenus, minimum, extrémités, point de contrôle, échelle.
    const rect = exemples()[2], o1 = JSON.parse(JSON.stringify(rect));
    D.redimensionner(rect, o1, 'nw', { x: 200, y: 140 }, 1); // passe par-dessus le coin se
    const a = rect.x1 === 150 && rect.y1 === 120 && rect.x2 === 200 && rect.y2 === 140;
    D.redimensionner(rect, o1, 'se', { x: 52, y: 51 }, 2); // mini 10 px d'écran = 5 px
    const b = rect.x1 === 50 && rect.y1 === 50 && rect.x2 === 55 && rect.y2 === 55;
    const fl = exemples()[0], o2 = JSON.parse(JSON.stringify(fl));
    D.redimensionner(fl, o2, 'p2', { x: 300, y: 10 }, 1);
    const c = fl.x2 === 300 && fl.y2 === 10 && fl.x1 === 10;
    D.redimensionner(fl, JSON.parse(JSON.stringify(fl)), 'pc', { x: 60, y: 0 }, 1);
    const pc2 = D.poignees(fl).find((q) => q[2] === 'pc');
    const d = fl.cx != null && Math.abs(pc2[0] - 60) < 1e-9 && Math.abs(pc2[1]) < 1e-9;
    const nu = exemples()[8], o3 = JSON.parse(JSON.stringify(nu));
    D.redimensionner(nu, o3, 'se', { x: 384 + 64, y: 84 + 64 }, 1);
    const e = nu.rayon === 32 && nu.x === 416 && nu.y === 116;
    const cr = exemples()[5], o4 = JSON.parse(JSON.stringify(cr)), bf4 = D.boiteForme(o4);
    D.redimensionner(cr, o4, 'se', { x: bf4.x + bf4.largeur * 2, y: bf4.y + bf4.hauteur * 2 }, 1);
    const f = cr.points.every((q, i) => Math.abs(q[0] - (bf4.x + (o4.points[i][0] - bf4.x) * 2))
      < 1e-9 && Math.abs(q[1] - (bf4.y + (o4.points[i][1] - bf4.y) * 2)) < 1e-9);
    const tx = exemples()[7], o5 = JSON.parse(JSON.stringify(tx));
    D.redimensionner(tx, o5, 'se', { x: 300 + 1000, y: 300 + 1000 }, 1);
    const g = tx.taille === 160 && tx.x === 300;
    verifier(a && b && c && d && e && f && g,
      '8 redimensionner : coin opposé tenu, minimum à l\'échelle, extrémité, contrôle, tailles',
      [a, b, c, d, e, f, g].join());
  }
  // (9) recadrage borné, entier, annulé ; cadre ; « Image entière ».
  {
    const doc = D.nouveauDocument(800, 600), h = D.creerHistorique(doc);
    D.modifier(h, (d) => D.recadrer(d, { x: 700.4, y: -20, largeur: 200, hauteur: 100.6 }));
    const a = json(D.cadre(doc)) === json({ x: 700, y: 0, largeur: 100, hauteur: 81 })
      && json(doc.recadrage) === json(D.cadre(doc));
    D.modifier(h, (d) => D.recadrer(d, { x: 300, y: 200, largeur: -100, hauteur: -50 }));
    const b = json(D.cadre(doc)) === json({ x: 200, y: 150, largeur: 100, hauteur: 50 });
    D.annuler(h);
    const c = json(D.cadre(doc)) === json({ x: 700, y: 0, largeur: 100, hauteur: 81 });
    D.annuler(h);
    const d = doc.recadrage === null && json(D.cadre(doc)) === json({ x: 0, y: 0, largeur: 800,
      hauteur: 600 });
    D.retablir(h);
    D.modifier(h, (x) => D.recadrer(x, null));
    const e = doc.recadrage === null;
    D.modifier(h, (x) => D.recadrer(x, { x: 900, y: 0, largeur: 50, hauteur: 50 }));
    D.modifier(h, (x) => D.recadrer(x, { x: 0, y: 0, largeur: 800, hauteur: 600 }));
    const f = doc.recadrage === null;
    // Un recadrage écrit à la main hors de l'image est borné par `cadre`.
    doc.recadrage = { x: -10, y: 590, largeur: 30, hauteur: 30 };
    const g = json(D.cadre(doc)) === json({ x: 0, y: 590, largeur: 20, hauteur: 10 });
    verifier(a && b && c && d && e && f && g, '9 recadrage borné, entier, annulé, Image entière',
      [a, b, c, d, e, f, g].join());
  }
  // (10) vide.
  {
    const doc = D.nouveauDocument(100, 100), h = D.creerHistorique(doc);
    const a = D.vide(doc);
    D.modifier(h, (d) => D.recadrer(d, { x: 10, y: 10, largeur: 20, hauteur: 20 }));
    const b = !D.vide(doc);
    D.annuler(h);
    D.modifier(h, (d) => D.ajouter(d, exemples()[0]));
    const c = !D.vide(doc);
    D.annuler(h);
    verifier(a && b && c && D.vide(doc), '10 vide : ni objet ni recadrage');
  }
  // Rectangles normalisés ; crayon lissé (Chaikin deux fois).
  {
    const o = { type: 'masque', x1: 10, y1: 30, x2: 5, y2: 2 };
    D.normaliser(o);
    const l = D.lisser([[0, 0], [10, 0], [10, 10]]);
    verifier(o.x1 === 5 && o.x2 === 10 && o.y1 === 2 && o.y2 === 30 && l.length === 12
      && l[0][0] === 0 && l[11][1] === 10 && l[1][0] === 0.625, '11 normaliser, lisser',
    JSON.stringify(l));
  }
}

// ---------- partie 2 : dessin.js dans Chromium ----------

// Fonctions posées dans la page (sans fermeture sur Node).
function outilsPage() {
  window.graine = (s) => () => {
    s |= 0; s = (s + 0x6D2B79F5) | 0;
    let t = Math.imul(s ^ (s >>> 15), 1 | s);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  // Image de bruit l × h ; `alpha(i)` : alpha du pixel i (255 par défaut).
  window.preparer = async (l, h, alpha) => {
    const hasard = window.graine(l * 7 + h);
    const src = new Uint8ClampedArray(l * h * 4);
    for (let i = 0; i < l * h; i++) {
      src[4 * i] = hasard() * 256; src[4 * i + 1] = hasard() * 256;
      src[4 * i + 2] = hasard() * 256; src[4 * i + 3] = alpha ? alpha(i) : 255;
    }
    const base = await createImageBitmap(new ImageData(src, l, h),
      { premultiplyAlpha: 'none', colorSpaceConversion: 'none' });
    const fond = document.createElement('canvas');
    fond.width = l; fond.height = h;
    const ctx = fond.getContext('2d', { willReadFrequently: true });
    return { l, h, src, base, fond, ctx };
  };
  window.exporter = (im, doc) => {
    window.M.peindreMasques(im.ctx, im.base, doc);
    return window.M.composer(doc, im.fond);
  };
  window.docAvec = (l, h, objets, recadrage = null) => {
    const doc = window.D.nouveauDocument(l, h);
    for (const o of objets) window.D.ajouter(doc, JSON.parse(JSON.stringify(o)));
    doc.recadrage = recadrage;
    return doc;
  };
  // Pixels de `sortie` (cadre en (kx, ky)) différents de la source, dans et hors d'une boîte.
  window.ecarts = (im, sortie, boites, kx = 0, ky = 0) => {
    let dehors = 0, dedans = 0;
    for (let y = 0; y < sortie.hauteur; y++) {
      for (let x = 0; x < sortie.largeur; x++) {
        const i = (y * sortie.largeur + x) * 4, j = ((y + ky) * im.l + x + kx) * 4;
        let diff = false;
        for (let c = 0; c < 4; c++) if (sortie.rgba[i + c] !== im.src[j + c]) diff = true;
        const X = x + kx, Y = y + ky;
        const dans = boites.some((b) => X + 1 > b.x && X < b.x + b.largeur
          && Y + 1 > b.y && Y < b.y + b.hauteur);
        if (diff && dans) dedans++;
        if (diff && !dans) dehors++;
      }
    }
    return { dehors, dedans };
  };
}

async function partieDessin(nav) {
  const ctx = await nav.newContext({ viewport: { width: 400, height: 300 } });
  const p = await ctx.newPage();
  const erreurs = [];
  p.on('pageerror', (e) => erreurs.push(String(e)));
  p.on('console', (m) => { if (m.type() === 'error') erreurs.push(m.text()); });
  await p.goto(`http://127.0.0.1:${serveur.address().port}/essai.html`);
  await p.evaluate(async () => {
    window.refus = [];
    document.addEventListener('securitypolicyviolation', (e) => window.refus.push(e.blockedURI));
    window.D = await import('/editeur/document.js');
    window.M = await import('/editeur/dessin.js');
  });
  await p.evaluate(outilsPage);

  // (a) Sans objet : export égal à l'image au bit près ; alpha 0, 128, 255.
  let r = await p.evaluate(async () => {
    const im = await preparer(1024, 768);
    const s = exporter(im, docAvec(1024, 768, []));
    let n = 0;
    for (let i = 0; i < s.rgba.length; i++) if (s.rgba[i] !== im.src[i]) n++;
    const im2 = await preparer(1023, 767, (i) => [0, 128, 255][i % 3]);
    const s2 = exporter(im2, docAvec(1023, 767, []));
    let alpha = 0, zero = 0, couleur = 0, exact = 0;
    for (let i = 0; i < 1023 * 767; i++) {
      const a = im2.src[4 * i + 3];
      if (s2.rgba[4 * i + 3] !== a) alpha++;
      if (a === 0 && s2.rgba[4 * i + 3] !== 0) zero++;
      for (let c = 0; c < 3; c++) {
        const d = Math.abs(s2.rgba[4 * i + c] - im2.src[4 * i + c]);
        if (a === 128 && d > 3) couleur++;
        if (a === 255 && d !== 0) exact++;
      }
    }
    return { n, l: s.largeur, h: s.hauteur, alpha, zero, couleur, exact };
  });
  verifier(r.n === 0 && r.l === 1024 && r.h === 768, '12 bruit 1024 × 768 sans objet : égal au bit '
    + 'près', JSON.stringify(r));
  verifier(r.alpha === 0 && r.zero === 0 && r.couleur === 0 && r.exact === 0,
    '12 alpha 0, 128, 255 : alpha exact, 0 reste 0, couleur ± 3 (255 : exacte)', JSON.stringify(r));

  // (b) Chaque objet : rien ne change hors de sa boîte ; quelque chose change dedans. Aussi
  // à cheval sur le bord de l'image et hors de l'image.
  r = await p.evaluate(async () => {
    const im = await preparer(640, 480);
    const objets = [
      { type: 'fleche', x1: 60, y1: 60, x2: 300, y2: 200, double: false, couleur: '#E5484D',
        epaisseur: 2, opacite: 1 },
      { type: 'fleche', x1: 60, y1: 300, x2: 400, y2: 300, cx: 230, cy: 120, double: true,
        couleur: '#0090FF', epaisseur: 2, opacite: 1 },
      { type: 'fleche', x1: 500, y1: 400, x2: 380, y2: 260, cx: 600, cy: 250, double: false,
        couleur: '#0090FF', epaisseur: 0, opacite: 1 },
      { type: 'fleche', x1: 100, y1: 400, x2: 300, y2: 400, double: true, couleur: '#30A46C',
        epaisseur: 0, opacite: 0.6 },
      { type: 'rectangle', x1: 100, y1: 100, x2: 300, y2: 200, couleur: '#E5484D',
        epaisseur: 1, plein: false, arrondi: 10, opacite: 1 },
      { type: 'rectangle', x1: 320, y1: 40, x2: 420, y2: 140, couleur: '#1A1A1A',
        epaisseur: 2, plein: true, arrondi: 0, opacite: 100 },
      { type: 'ellipse', x1: 200, y1: 50, x2: 330, y2: 150, couleur: '#30A46C', epaisseur: 2,
        plein: false, opacite: 1 },
      { type: 'ligne', x1: 20, y1: 450, x2: 600, y2: 30, couleur: '#8E4EC6', epaisseur: 2,
        opacite: 1 },
      { type: 'crayon', points: [[50, 50], [80, 120], [120, 90], [160, 160], [200, 100]],
        couleur: '#F5A524', epaisseur: 2, opacite: 1 },
      { type: 'crayon', points: [[333, 333]], couleur: '#F5A524', epaisseur: 2, opacite: 1 },
      { type: 'surligneur', points: [[100, 250], [400, 260]], couleur: '#FFD43B', epaisseur: 2 },
      { type: 'texte', x: 120, y: 200, texte: 'Ég Ça jpq\nDeux lignes', couleur: '#E5484D',
        taille: 40, gras: true, style: 'contour', opacite: 1 },
      { type: 'texte', x: 120, y: 200, texte: 'Simple ÉÀ gjpqy', couleur: '#FFFFFF',
        taille: 60, gras: false, style: 'simple', opacite: 1 },
      { type: 'texte', x: 300, y: 380, texte: 'Étiquette', couleur: '#FFD43B', taille: 28,
        gras: true, style: 'etiquette', opacite: 1 },
      { type: 'numero', x: 500, y: 300, rayon: 21, couleur: '#E5484D', ordre: 1, opacite: 1 },
      { type: 'masque', x1: 400.3, y1: 150.7, x2: 520.2, y2: 250.1, sorte: 'pixeliser',
        intensite: 2 },
      { type: 'masque', x1: 30, y1: 30, x2: 130, y2: 90, sorte: 'flouter', intensite: 0 },
      { type: 'masque', x1: 30.5, y1: 300.5, x2: 130.5, y2: 390.5, sorte: 'bloc', intensite: 1 },
    ];
    const res = [];
    const essayer = (o, nom) => {
      const s = exporter(im, docAvec(640, 480, [o]));
      const e = ecarts(im, s, [D.boite(o)]);
      res.push({ nom, ...e, ok: e.dehors === 0 && e.dedans > 0 });
    };
    for (const o of objets) essayer(o, o.type + (o.sorte || o.style || (o.cx != null ? '~' : '')));
    // À cheval sur le bord, puis tout entier hors de l'image.
    for (const o of objets) {
      const q = JSON.parse(JSON.stringify(o)), b = D.boite(o);
      D.deplacer(q, o, -b.x - b.largeur / 2, -b.y - b.hauteur / 2);
      essayer(q, 'bord ' + o.type);
      const q2 = JSON.parse(JSON.stringify(o));
      D.deplacer(q2, o, 700 - b.x, 20);
      const s = exporter(im, docAvec(640, 480, [q2]));
      const e = ecarts(im, s, []);
      res.push({ nom: 'hors ' + o.type, ...e, ok: e.dehors === 0 && s.largeur === 640 });
    }
    return res;
  });
  const rates = r.filter((x) => !x.ok);
  verifier(rates.length === 0, `13 ${r.length} cas : hors de la boîte (ombre comprise) rien ne `
    + 'change, dedans l\'objet est peint ; à cheval sur le bord ; hors de l\'image',
  JSON.stringify(rates));

  // (c) Rectangle (100, 100)-(300, 200) : bord haut à #E5484D ± 6 ; bloc noir #111214.
  r = await p.evaluate(async () => {
    const im = await preparer(640, 480);
    const s = exporter(im, docAvec(640, 480, [
      { type: 'rectangle', x1: 100, y1: 100, x2: 300, y2: 200, couleur: '#E5484D',
        epaisseur: 1, plein: false, arrondi: 10, opacite: 1 },
      { type: 'masque', x1: 400, y1: 300, x2: 500, y2: 400, sorte: 'bloc', intensite: 1 }]));
    const px = (x, y) => Array.from(s.rgba.slice((y * 640 + x) * 4, (y * 640 + x) * 4 + 4));
    const ecart = (a, b) => Math.max(...a.slice(0, 3).map((v, i) => Math.abs(v - b[i])));
    let bord = 0, noir = 0;
    for (let x = 115; x <= 285; x++) bord = Math.max(bord, ecart(px(x, 100), [229, 72, 77]));
    for (let y = 304; y < 396; y++) {
      for (let x = 404; x < 496; x++) noir = Math.max(noir, ecart(px(x, y), [17, 18, 20]));
    }
    return { bord, noir };
  });
  verifier(r.bord <= 6 && r.noir === 0, '14 bord haut du rectangle #E5484D ± 6, bloc noir #111214',
    JSON.stringify(r));

  // (d) Pixeliser et Flouter irréversibles : blocs unis alignés sur la grille, aucun pixel
  // d'origine dans la zone, variance du flou < 5 % de l'origine.
  r = await p.evaluate(async () => {
    const im = await preparer(640, 480);
    const res = {};
    for (const [i, bs] of M.BLOCS.entries()) {
      const zoneM = { type: 'masque', x1: 101.4, y1: 57.6, x2: 333.2, y2: 270.3,
        sorte: 'pixeliser', intensite: i };
      const s = exporter(im, docAvec(640, 480, [zoneM]));
      let unis = true, origine = 0, total = 0;
      for (let y = 58; y < 270; y++) {
        for (let x = 101; x < 333; x++) {
          const k = (y * 640 + x) * 4;
          // Premier pixel de son bloc, sur la grille de l'image, bornée à la zone.
          const x0 = Math.max(101, x - (x % bs)), y0 = Math.max(58, y - (y % bs));
          const k0 = (y0 * 640 + x0) * 4;
          for (let c = 0; c < 4; c++) if (s.rgba[k + c] !== s.rgba[k0 + c]) unis = false;
          let meme = true;
          for (let c = 0; c < 3; c++) if (s.rgba[k + c] !== im.src[k + c]) meme = false;
          if (meme) origine++;
          total++;
        }
      }
      // Deux blocs voisins diffèrent (la grille n'est pas d'un seul bloc).
      const a = (y, x) => s.rgba[(y * 640 + x) * 4];
      res['pixeliser' + bs] = { unis, origine, total,
        voisins: a(130, 130) !== a(130, 130 + bs) || a(130, 130) !== a(130 + bs, 130) };
    }
    const zoneF = { type: 'masque', x1: 100, y1: 60, x2: 340, y2: 260, sorte: 'flouter',
      intensite: 1 };
    const s = exporter(im, docAvec(640, 480, [zoneF]));
    const variance = (t) => {
      let somme = 0, carres = 0, n = 0;
      for (let y = 60; y < 260; y++) {
        for (let x = 100; x < 340; x++) {
          const v = t[(y * 640 + x) * 4 + 1];
          somme += v; carres += v * v; n++;
        }
      }
      return carres / n - (somme / n) ** 2;
    };
    // Pixel d'origine, tel quel ou sous le seul voile blanc à 18 % (240, 242, 246).
    let origine = 0, voile = 0;
    const gris = [240, 242, 246];
    for (let y = 60; y < 260; y++) {
      for (let x = 100; x < 340; x++) {
        const k = (y * 640 + x) * 4;
        if ([0, 1, 2].every((c) => s.rgba[k + c] === im.src[k + c])) origine++;
        if ([0, 1, 2].every((c) => Math.abs(s.rgba[k + c]
          - (im.src[k + c] * 0.82 + gris[c] * 0.18)) <= 1.5)) voile++;
      }
    }
    res.flouter = { rapport: variance(s.rgba) / variance(im.src), origine, voile };
    return res;
  });
  for (const bs of [8, 13, 20]) {
    const x = r['pixeliser' + bs];
    verifier(x.unis && x.voisins && x.origine === 0, `15 Pixeliser ${bs} px : blocs unis alignés `
      + 'sur la grille, aucun pixel d\'origine dans la zone', JSON.stringify(x));
  }
  verifier(r.flouter.rapport < 0.05 && r.flouter.origine < 48 && r.flouter.voile < 48,
    '16 Flouter Moyenne : variance < 5 % de l\'origine, pixels d\'origine (même sous le voile) '
    + '< 0,1 %',
    JSON.stringify(r.flouter));

  // (e) Masques sous les autres objets ; dans leur ordre entre eux ; cadre décalé.
  r = await p.evaluate(async () => {
    const im = await preparer(640, 480);
    const f = { type: 'fleche', x1: 50, y1: 50, x2: 400, y2: 300, double: false,
      couleur: '#E5484D', epaisseur: 2, opacite: 1 };
    const m = { type: 'masque', x1: 100, y1: 100, x2: 300, y2: 300, sorte: 'pixeliser',
      intensite: 1 };
    const b = { type: 'masque', x1: 150, y1: 150, x2: 250, y2: 250, sorte: 'bloc',
      intensite: 1 };
    const egal = (a, c) => a.rgba.every((v, i) => v === c.rgba[i]);
    const s1 = exporter(im, docAvec(640, 480, [f, m]));
    const s2 = exporter(im, docAvec(640, 480, [m, f]));
    const s3 = exporter(im, docAvec(640, 480, [m, b]));
    const s4 = exporter(im, docAvec(640, 480, [b, m]));
    const dessus = egal(s1, s2);
    const ordre = !egal(s3, s4) && s3.rgba[(200 * 640 + 200) * 4] === 17
      && s4.rgba[(200 * 640 + 200) * 4] !== 17;
    // Cadre décalé : l'export est le morceau de l'export entier, au bit près.
    const tout = exporter(im, docAvec(640, 480, [f, m, b]));
    const k = { x: 37, y: 21, largeur: 301, hauteur: 203 };
    const sk = exporter(im, docAvec(640, 480, [f, m, b], k));
    let diff = 0, ecartMax = 0;
    const ou = [1e9, 1e9, -1, -1];
    for (let y = 0; y < k.hauteur; y++) {
      for (let x = 0; x < k.largeur; x++) {
        for (let c = 0; c < 4; c++) {
          const d = Math.abs(sk.rgba[(y * k.largeur + x) * 4 + c]
            - tout.rgba[((y + k.y) * 640 + x + k.x) * 4 + c]);
          if (d) {
            diff++; ecartMax = Math.max(ecartMax, d);
            ou[0] = Math.min(ou[0], x); ou[1] = Math.min(ou[1], y);
            ou[2] = Math.max(ou[2], x); ou[3] = Math.max(ou[3], y);
          }
        }
      }
    }
    const horsBoites = ecarts(im, sk, [D.boite(f), D.boite(m), D.boite(b)], k.x, k.y);
    return { dessus, ordre, taille: [sk.largeur, sk.hauteur], diff, ecartMax, ou, horsBoites };
  });
  verifier(r.dessus, '17 un masque posé sur une flèche : la flèche reste au-dessus');
  verifier(r.ordre, '17 masques peints dans leur ordre entre eux');
  verifier(r.taille.join() === '301,203' && r.diff === 0 && r.horsBoites.dehors === 0,
    '18 cadre décalé (37, 21) : taille du cadre, morceau de l\'export entier au bit près',
    JSON.stringify(r));

  // (f) La vue à la taille réelle est l'export ; à l'échelle 2, l'objet reste dans 2 × sa
  // boîte ; contour de sélection hors de l'export.
  r = await p.evaluate(async () => {
    const im = await preparer(300, 200);
    const objets = [
      { type: 'rectangle', x1: 40, y1: 40, x2: 200, y2: 150, couleur: '#E5484D',
        epaisseur: 1, plein: false, arrondi: 10, opacite: 1 },
      { type: 'texte', x: 50, y: 60, texte: 'Vue', couleur: '#0090FF', taille: 28, gras: true,
        style: 'etiquette', opacite: 1 },
      { type: 'numero', x: 250, y: 50, rayon: 16, couleur: '#30A46C', opacite: 1 },
      { type: 'masque', x1: 220, y1: 100, x2: 290, y2: 190, sorte: 'pixeliser', intensite: 1 },
    ];
    const k = { x: 10, y: 20, largeur: 270, hauteur: 170 };
    const doc = docAvec(300, 200, objets, k);
    const s = exporter(im, doc);
    const vue = document.createElement('canvas');
    vue.width = 400; vue.height = 300;
    const c = vue.getContext('2d', { willReadFrequently: true });
    M.peindreVue(c, doc, im.fond, { x: 50, y: 40, echelle: 1, dpr: 1 }, []);
    const v = c.getImageData(60, 60, k.largeur, k.hauteur).data;
    let diff = 0, max = 0;
    for (let i = 0; i < v.length; i++) {
      const d = Math.abs(v[i] - s.rgba[i]);
      if (d) diff++;
      max = Math.max(max, d);
    }
    const autour = c.getImageData(0, 0, 400, 300).data;
    let dehors = 0;
    for (let y = 0; y < 300; y++) {
      for (let x = 0; x < 400; x++) {
        const dans = x >= 60 && x < 330 && y >= 60 && y < 230;
        if (!dans && autour[(y * 400 + x) * 4 + 3] !== 0) dehors++;
      }
    }
    // Échelle 2, sans recadrage : hors de 2 × la boîte du rectangle, la vue est le fond agrandi.
    const doc2 = docAvec(300, 200, [objets[0]]);
    const doc0 = docAvec(300, 200, []);
    const grand = document.createElement('canvas');
    grand.width = 600; grand.height = 400;
    const g = grand.getContext('2d', { willReadFrequently: true });
    M.peindreMasques(im.ctx, im.base, doc0);
    M.peindreVue(g, doc0, im.fond, { x: 0, y: 0, echelle: 2, dpr: 2 }, []);
    const sans = g.getImageData(0, 0, 600, 400).data;
    M.peindreVue(g, doc2, im.fond, { x: 0, y: 0, echelle: 2, dpr: 2 }, []);
    const avec = g.getImageData(0, 0, 600, 400).data;
    const b = D.boite(doc2.objets[0]);
    let hors2 = 0, dans2 = 0;
    for (let y = 0; y < 400; y++) {
      for (let x = 0; x < 600; x++) {
        const i = (y * 600 + x) * 4;
        const d = [0, 1, 2, 3].some((q) => sans[i + q] !== avec[i + q]);
        const dans = x + 1 > 2 * b.x && x < 2 * (b.x + b.largeur)
          && y + 1 > 2 * b.y && y < 2 * (b.y + b.hauteur);
        if (d && !dans) hors2++;
        if (d && dans) dans2++;
      }
    }
    // Sélection : contour dessiné dans la vue, jamais dans l'export.
    M.peindreVue(g, doc2, im.fond, { x: 0, y: 0, echelle: 2, dpr: 2 }, [doc2.objets[0].id]);
    const sel = g.getImageData(0, 0, 600, 400).data;
    let contour = 0;
    for (let i = 0; i < sel.length; i++) if (sel[i] !== avec[i]) contour++;
    // Objets à cheval sur le bord de l'image, sans recadrage : vue = export aussi.
    const doc3 = docAvec(300, 200, [
      { type: 'fleche', x1: -60, y1: -30, x2: 150, y2: 100, double: true, couleur: '#E5484D',
        epaisseur: 2, opacite: 1 },
      { type: 'ellipse', x1: 200, y1: 120, x2: 360, y2: 260, couleur: '#30A46C', epaisseur: 2,
        plein: false, opacite: 1 },
      { type: 'masque', x1: 250, y1: -20, x2: 320, y2: 60, sorte: 'flouter', intensite: 2 }]);
    M.peindreMasques(im.ctx, im.base, doc3);
    const s4 = M.composer(doc3, im.fond);
    M.peindreVue(c, doc3, im.fond, { x: 50, y: 40, echelle: 1, dpr: 1 }, []);
    const v4 = c.getImageData(50, 40, 300, 200).data;
    let bord = 0;
    for (let i = 0; i < v4.length; i++) if (v4[i] !== s4.rgba[i]) bord++;
    M.peindreMasques(im.ctx, im.base, doc0);
    const s2 = M.composer(doc2, im.fond), s3 = M.composer(doc2, im.fond);
    const memeExport = s2.rgba.every((x, i) => x === s3.rgba[i]);
    return { diff, max, dehors, hors2, dans2, contour, memeExport, bord };
  });
  verifier(r.diff === 0, '19 vue à la taille réelle (image décalée, recadrée) = export au bit près',
    JSON.stringify(r));
  verifier(r.bord === 0, '19 vue à la taille réelle = export, objets à cheval sur le bord',
    r.bord);
  verifier(r.dehors === 0, '19 la vue ne peint que le cadre', r.dehors);
  verifier(r.hors2 === 0 && r.dans2 > 0, '19 vue à l\'échelle 2 : ombre à l\'échelle, dans 2 × '
    + 'la boîte', JSON.stringify(r));
  verifier(r.contour > 0 && r.memeExport, '19 contour de sélection dans la vue seulement',
    r.contour);

  // (g) Temps : Pixeliser sur une grande image (toute l'image masquée) ; composer 5760 × 1080
  // avec 50 objets (borne du contrat : 1 000 ms).
  r = await p.evaluate(async () => {
    const res = {};
    for (const [l, h] of [[5000, 5000]]) {
      let im;
      try { im = await preparer(l, h); } catch (e) { res.erreur = String(e); break; }
      const doc = docAvec(l, h, [{ type: 'masque', x1: 0, y1: 0, x2: l, y2: h,
        sorte: 'pixeliser', intensite: 1 }]);
      const t0 = performance.now();
      M.peindreMasques(im.ctx, im.base, doc);
      res.pixeliser = Math.round(performance.now() - t0);
      res.taille = `${l}x${h}`;
      const px = im.ctx.getImageData(4987, 4987, 2, 2).data;
      res.unis = px.every((v, i) => v === px[i % 4]) && px[3] === 255;
      im.fond.width = 0;
      im.base.close();
    }
    const im = await preparer(5760, 1080);
    const objets = [];
    const sortes = ['fleche', 'rectangle', 'ellipse', 'ligne', 'crayon', 'texte', 'numero'];
    for (let i = 0; i < 50; i++) {
      const x = (i * 113) % 5600, y = (i * 71) % 1000, t = sortes[i % sortes.length];
      const base = { couleur: '#E5484D', epaisseur: 1, opacite: 1 };
      if (t === 'crayon') {
        const pts = [];
        for (let j = 0; j < 200; j++) pts.push([x + j, y + 30 * Math.sin(j / 9)]);
        objets.push({ ...base, type: t, points: pts });
      } else if (t === 'texte') {
        objets.push({ ...base, type: t, x, y, texte: 'Texte ' + i, taille: 28, gras: true,
          style: 'contour' });
      } else if (t === 'numero') {
        objets.push({ ...base, type: t, x, y, rayon: 16 });
      } else {
        objets.push({ ...base, type: t, x1: x, y1: y, x2: x + 150, y2: y + 80, plein: false,
          arrondi: 10, double: false });
      }
    }
    objets.push({ type: 'masque', x1: 100, y1: 100, x2: 900, y2: 700, sorte: 'flouter',
      intensite: 1 });
    const doc = docAvec(5760, 1080, objets);
    M.peindreMasques(im.ctx, im.base, doc);
    const t0 = performance.now();
    const s = M.composer(doc, im.fond);
    res.composer = Math.round(performance.now() - t0);
    res.export = `${s.largeur}x${s.hauteur}`;
    return res;
  });
  verifier(!r.erreur && r.unis && r.pixeliser <= 3000, `20 Pixeliser ${r.taille} (25 Mpx), toute `
    + `l'image : ${r.pixeliser} ms (borne 3 000 ms)`, JSON.stringify(r));
  verifier(r.export === '5760x1080' && r.composer <= 1000,
    `21 composer 5760 × 1080, 50 objets : ${r.composer} ms (borne 1 000 ms)`);

  const refus = await p.evaluate(() => window.refus);
  verifier(erreurs.length === 0 && refus.length === 0, '22 aucune erreur ni violation de CSP',
    JSON.stringify([erreurs, refus]));
  await ctx.close();
}

(async () => {
  await partieDocument();
  await new Promise((ok) => serveur.listen(0, ok));
  const nav = await chromium.launch();
  try {
    await partieDessin(nav);
  } finally {
    await nav.close();
    serveur.close();
  }
  console.log('ECHECS :', echecs);
  process.exitCode = echecs === 0 ? 0 : 1;
})().catch((e) => { console.error('essai interrompu :', e); process.exit(1); });
