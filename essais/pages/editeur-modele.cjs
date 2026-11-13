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
    // Même marque que le pas précédent : le document revient à l'état d'avant l'appel.
    D.modifier(h, (d) => D.ajouter(d, exemples()[2]));
    D.modifier(h, (d) => { d.objets[0].opacite = 0.5; }, 'op');
    const v = h.version, avant = json(doc), pas = h.passe.length;
    leve = false;
    try {
      D.modifier(h, (d) => { d.objets[0].opacite = 0.2; throw new Error('voulu'); }, 'op');
    } catch { leve = true; }
    verifier(leve && json(doc) === avant && h.version === v && h.passe.length === pas,
      '4c erreur avec la même marque : document d\'avant l\'appel, version cohérente',
      `${doc.objets[0].opacite} ${v} ${h.version}`);
  }
  // Mémoire de l'historique : 40 traits de 1000 points, 200 pas ; points au 1/100 px.
  {
    let g = 1;
    const hasard = () => { g = (g * 16807) % 2147483647; return g / 2147483647; };
    const doc = D.nouveauDocument(1920, 1080), h = D.creerHistorique(doc);
    for (let i = 0; i < 40; i++) {
      const pts = [];
      let x = 100 + hasard() * 1000, y = 100 + hasard() * 600;
      for (let j = 0; j < 1000; j++) {
        x += hasard() * 6 - 3; y += hasard() * 6 - 3;
        pts.push([x, y]);
      }
      D.modifier(h, (d) => D.ajouter(d, { type: 'crayon', points: D.lisser(pts),
        couleur: '#E5484D', epaisseur: 1, opacite: 1 }));
    }
    const o = doc.objets[0], q = JSON.parse(json(o));
    D.deplacer(o, q, 1 / 3, 2 / 3);
    const centiemes = (t) => t.every(([x, y]) => Math.round(x * 100) / 100 === x
      && Math.round(y * 100) / 100 === y);
    const arrondis = centiemes(o.points);
    D.redimensionner(o, q, 'se', { x: 1700.123, y: 900.456 }, 1);
    const arrondis2 = centiemes(o.points);
    for (let i = 0; i < 200; i++) D.modifier(h, (d) => { d.objets[i % 40].opacite = 0.5; });
    const octets = D.octetsHistorique(h), document = json(doc).length;
    verifier(arrondis && arrondis2 && octets <= 32 * 1024 * 1024 && D.peutAnnuler(h)
      && h.passe.length >= 1, '4d historique borné à 32 Mo de JSON (au moins un pas), points '
      + 'au 1/100 px', `${(octets / 1e6).toFixed(1)} Mo, ${h.passe.length} pas, document `
      + `${(document / 1e6).toFixed(2)} Mo`);
    // Un document plus gros que la borne garde encore un pas annulable.
    const gros = D.nouveauDocument(10, 10), hg = D.creerHistorique(gros);
    const pts = Array.from({ length: 2e6 }, (_, i) => [i, i]);
    D.modifier(hg, (d) => D.ajouter(d, { type: 'crayon', points: pts, epaisseur: 1 }));
    D.modifier(hg, (d) => { d.objets[0].opacite = 0.5; });
    verifier(D.octetsHistorique(hg) > 32 * 1024 * 1024 && hg.passe.length === 1 && D.annuler(hg)
      && gros.objets[0].opacite === undefined, '4d document plus gros que la borne : un pas '
      + 'annulable reste');
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
  // Graine des masques : tirée par `ajouter`, gardée par annuler et rétablir, neuve à la copie.
  {
    const doc = D.nouveauDocument(100, 100), h = D.creerHistorique(doc);
    let id;
    D.modifier(h, (d) => { id = D.ajouter(d, { type: 'masque', x1: 0, y1: 0, x2: 50, y2: 50,
      sorte: 'pixeliser', intensite: 1 }); });
    const g = doc.objets[0].graine;
    D.modifier(h, (d) => { d.objets[0].intensite = 2; });
    D.annuler(h); D.retablir(h);
    const [copie] = D.dupliquer(doc, [id]);
    const g2 = doc.objets.find((o) => o.id === copie).graine;
    const fixe = { type: 'masque', x1: 0, y1: 0, x2: 5, y2: 5, sorte: 'bloc', graine: 3 };
    D.ajouter(doc, fixe);
    verifier(Number.isInteger(g) && g >= 0 && doc.objets[0].graine === g && Number.isInteger(g2)
      && g2 !== g && fixe.graine === 3, '6b graine des masques tirée, gardée, neuve à la copie');
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
    // Qui dépasse : ramené dans l'image, taille gardée.
    D.modifier(h, (d) => D.recadrer(d, { x: 700.4, y: -20, largeur: 200, hauteur: 100.6 }));
    const a = json(D.cadre(doc)) === json({ x: 600, y: 0, largeur: 200, hauteur: 101 })
      && json(doc.recadrage) === json(D.cadre(doc));
    D.modifier(h, (d) => D.recadrer(d, { x: 300, y: 200, largeur: -100, hauteur: -50 }));
    const b = json(D.cadre(doc)) === json({ x: 200, y: 150, largeur: 100, hauteur: 50 });
    D.annuler(h);
    const c = json(D.cadre(doc)) === json({ x: 600, y: 0, largeur: 200, hauteur: 101 });
    D.annuler(h);
    const d = doc.recadrage === null && json(D.cadre(doc)) === json({ x: 0, y: 0, largeur: 800,
      hauteur: 600 });
    D.retablir(h);
    D.modifier(h, (x) => D.recadrer(x, null));
    const e = doc.recadrage === null;
    D.modifier(h, (x) => D.recadrer(x, { x: 900, y: 0, largeur: 50, hauteur: 50 }));
    const f1 = json(doc.recadrage) === json({ x: 750, y: 0, largeur: 50, hauteur: 50 });
    D.modifier(h, (x) => D.recadrer(x, { x: 0, y: 0, largeur: 800, hauteur: 600 }));
    const f2 = doc.recadrage === null;
    // Plus grand que l'image : coupé, donc l'image entière.
    D.modifier(h, (x) => D.recadrer(x, { x: -10, y: -10, largeur: 1000, hauteur: 300 }));
    const f3 = json(doc.recadrage) === json({ x: 0, y: 0, largeur: 800, hauteur: 300 });
    // Un carré (rapport 1:1) reste carré : position puis taille arrondies.
    D.modifier(h, (x) => D.recadrer(x, { x: 10.4, y: 20.6, largeur: 100.6, hauteur: 100.6 }));
    const f4 = json(doc.recadrage) === json({ x: 10, y: 21, largeur: 101, hauteur: 101 });
    const f = f1 && f2 && f3 && f4;
    // Un recadrage écrit à la main hors de l'image est borné par `cadre`.
    doc.recadrage = { x: -10, y: 590, largeur: 30, hauteur: 30 };
    const g = json(D.cadre(doc)) === json({ x: 0, y: 590, largeur: 20, hauteur: 10 });
    verifier(a && b && c && d && e && f && g, '9 recadrage entier (carré gardé), ramené dans '
      + 'l\'image ou coupé, annulé, Image entière',
      [a, b, c, d, e, f, g].join());
  }
  // (P) Projecteur : modèle (créer, toucher, poignées, déplacer, redimensionner, copier,
  // annuler, rétablir, ordre de recherche).
  {
    const doc = D.nouveauDocument(640, 480), h = D.creerHistorique(doc);
    const proj = (forme) => ({ type: 'projecteur', forme, x1: 300, y1: 200, x2: 100, y2: 80 });
    let id, idO;
    D.modifier(h, (d) => { const o = proj('rectangle'); D.normaliser(o); id = D.ajouter(d, o); });
    D.modifier(h, (d) => { idO = D.ajouter(d, { ...proj('ovale'), x1: 400, y1: 300, x2: 600,
      y2: 400 }); });
    const r = doc.objets[0], ov = doc.objets[1];
    const a = r.x1 === 100 && r.y1 === 80 && r.x2 === 300 && r.y2 === 200
      && json(D.boiteForme(r)) === json({ x: 100, y: 80, largeur: 200, hauteur: 120 })
      && D.poignees(r).map((q) => q.slice(0, 2).join()).join(' ')
        === '100,80 300,80 100,200 300,200'
      && D.poignees(r).map((q) => q[2]).join() === 'nw,ne,sw,se';
    // Le bord seul se touche (bande de tolérance + 7 px) ; ovale : ni son centre ni les coins
    // de sa boîte.
    const b = !D.touche(r, [200, 140], 2) && D.touche(r, [103, 83], 0) && D.touche(r, [91, 140], 2)
      && !D.touche(r, [89, 140], 2) && D.touche(r, [200, 208], 2) && !D.touche(r, [200, 190], 2)
      && !D.touche(ov, [500, 350], 0) && D.touche(ov, [599, 350], 2) && D.touche(ov, [500, 395], 2)
      && !D.touche(ov, [405, 305], 2) && !D.touche(ov, [500, 412], 2);
    // Une flèche dans la zone éclairée se prend avant le projecteur ; ailleurs, le projecteur.
    D.modifier(h, (d) => D.ajouter(d, { type: 'fleche', x1: 150, y1: 140, x2: 250, y2: 140,
      double: false, couleur: '#E5484D', epaisseur: 1, opacite: 1 }));
    D.ordonner(doc, [id], true);
    // Dans la zone éclairée : un masque se prend en son centre ; l'intérieur d'un rectangle
    // non plein, loin de tout bord, ne rend rien ; le bord du projecteur rend le projecteur.
    let idM;
    D.modifier(h, (d) => { idM = D.ajouter(d, { type: 'masque', x1: 120, y1: 150, x2: 180,
      y2: 190, sorte: 'flouter', intensite: 1 }); });
    D.modifier(h, (d) => D.ajouter(d, { type: 'rectangle', x1: 200, y1: 100, x2: 290, y2: 190,
      couleur: '#E5484D', epaisseur: 1, plein: false, arrondi: 6, opacite: 1 }));
    D.ordonner(doc, [id], true);
    const c = D.objetSous(doc, [170, 140], 2)?.type === 'fleche'
      && D.objetSous(doc, [150, 170], 2)?.id === idM
      && D.objetSous(doc, [245, 160], 2) === null
      && D.objetSous(doc, [150, 82], 2)?.id === id;
    // Déplacer, redimensionner (coin nw tiré au-delà du coin se), dupliquer.
    const q = JSON.parse(json(r));
    D.modifier(h, () => D.deplacer(r, q, 10, 5), 'g1');
    const d1 = r.x1 === 110 && r.y2 === 205;
    const q2 = JSON.parse(json(r));
    D.modifier(h, () => D.redimensionner(r, q2, 'nw', { x: 400, y: 300 }, 1), 'g2');
    const d2 = r.x1 === 310 && r.y1 === 205 && r.x2 === 400 && r.y2 === 300;
    let copies;
    D.modifier(h, (d) => { copies = D.dupliquer(d, [idO]); });
    const cp = doc.objets.find((o) => o.id === copies[0]);
    const d3 = cp.type === 'projecteur' && cp.forme === 'ovale' && cp.x1 === 416 && cp.y2 === 416
      && D.objetSous(doc, [500, 350], 2) === null;
    // Annuler jusqu'au début puis tout rétablir.
    const fin = json(doc);
    while (D.annuler(h));
    const vide0 = doc.objets.length === 0;
    while (D.retablir(h));
    verifier(a && b && c && d1 && d2 && d3 && vide0 && json(doc) === fin,
      'P1 projecteur : normalisé, boîte, poignées aux coins, touché par son bord seul (ovale '
      + 'compris), ce qui est dans la zone se prend, déplacé, redimensionné, copié, annulé, '
      + 'rétabli',
      [a, b, c, d1, d2, d3, vide0].join());
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
  // (G) Gomme au pixel (fil « Rendre l'annotation plus naturelle ») : modèle.
  // (G1) Encres et objets entiers ; ce que la gomme atteint.
  {
    const t = Object.fromEntries(exemples().filter((o) => o.cx == null).map((o) => [o.type, o]));
    const encres = ['fleche', 'rectangle', 'ellipse', 'ligne', 'crayon', 'surligneur'];
    const types = exemples().map((o) => o.type).concat('projecteur');
    const a = types.every((n) => D.estEncre({ type: n }) === encres.includes(n));
    const P = (x, y) => ({ x, y });
    const trait = { type: 'crayon', points: [[100, 100], [300, 100]], couleur: '#E5484D',
      epaisseur: 1, opacite: 1 };
    // Rayon 10, trait de 5 px : atteint jusqu'à 10 + 2,5 (+ 1) px de la ligne.
    const b = D.gommeTouche(trait, P(200, 80), P(200, 120), 10)
      && D.gommeTouche(trait, P(200, 112), P(200, 112), 10)
      && !D.gommeTouche(trait, P(200, 115), P(200, 115), 10)
      && !D.gommeTouche(trait, P(320, 100), P(400, 100), 10)
      && D.gommeTouche(trait, P(50, 100), P(350, 100), 10);
    const rect = t.rectangle; // (50, 50)-(150, 120), bord seul
    const c = D.gommeTouche(rect, P(50, 80), P(50, 80), 5)
      && !D.gommeTouche(rect, P(100, 85), P(100, 85), 5)
      && D.gommeTouche({ ...rect, plein: true }, P(100, 85), P(100, 85), 5)
      && D.gommeTouche(rect, P(100, 85), P(200, 85), 5);
    const ell = t.ellipse; // (200, 50)-(300, 150)
    const d = D.gommeTouche(ell, P(250, 50), P(250, 50), 5)
      && !D.gommeTouche(ell, P(250, 100), P(250, 100), 5)
      && D.gommeTouche({ ...ell, plein: true }, P(250, 100), P(250, 100), 5);
    // Flèche (10, 10) → (110, 60) : le corps, la pointe (plus large que le corps), pas à côté.
    const f = t.fleche;
    const e = D.gommeTouche(f, P(60, 35), P(60, 35), 3)
      && D.gommeTouche(f, P(96, 60), P(96, 60), 3) && !D.gommeTouche(f, P(60, 60), P(60, 60), 3);
    const entiers = ['texte', 'numero', 'masque'].every((n) => {
      const o = t[n], bo = D.boite(o);
      return !D.gommeTouche(o, P(bo.x, bo.y), P(bo.x + bo.largeur, bo.y + bo.hauteur), 50);
    }) && !D.gommeTouche({ type: 'projecteur', x1: 0, y1: 0, x2: 50, y2: 50, forme: 'ovale' },
      P(0, 0), P(50, 50), 50);
    verifier(a && b && c && d && e && entiers,
      'G1 encres et objets entiers ; la gomme atteint le trait, le bord, le plein, la pointe, '
      + 'jamais un texte, un numéro, un flou ni un projecteur', [a, b, c, d, e, entiers].join());
  }
  // (G2) gommer : passages gardés au 1/100 px, sur une encre seulement ; annuler, rétablir.
  {
    const doc = D.nouveauDocument(640, 480), h = D.creerHistorique(doc);
    for (const o of exemples()) D.modifier(h, (d) => D.ajouter(d, o));
    const avant = json(doc);
    const crayon = doc.objets.find((o) => o.type === 'crayon');
    const texte = doc.objets.find((o) => o.type === 'texte');
    D.modifier(h, (d) => {
      D.gommer(d, crayon.id, [{ largeur: 20.004, points: [[20.001, 310.006], [40, 305]] },
        { largeur: 20, points: [] }, { largeur: 0, points: [[1, 1]] }]);
      D.gommer(d, texte.id, [{ largeur: 20, points: [[310, 310]] }]);
      D.gommer(d, 999, [{ largeur: 20, points: [[310, 310]] }]);
    });
    const o1 = doc.objets.find((o) => o.type === 'crayon');
    const a = json(o1.gommes) === '[{"largeur":20,"points":[[20,310.01],[40,305]]}]'
      && !('gommes' in doc.objets.find((o) => o.type === 'texte'));
    D.modifier(h, (d) => D.gommer(d, o1.id, [{ largeur: 8, points: [[60, 330]] }]));
    const b = doc.objets.find((o) => o.type === 'crayon').gommes.length === 2;
    D.annuler(h); D.annuler(h);
    const c = json(doc) === avant;
    D.retablir(h); D.retablir(h);
    const d1 = doc.objets.find((o) => o.type === 'crayon').gommes.length === 2;
    verifier(a && b && c && d1, 'G2 gommer : passages au 1/100 px, encres seules, passages '
      + 'vides écartés, annulé et rétabli', [a, b, c, d1].join());
  }
  // (G3) Une encre gommée ne se prend que là où il en reste ; ce qui est dessous se prend
  // dans le trou.
  {
    const doc = D.nouveauDocument(640, 480);
    const dessous = D.ajouter(doc, { type: 'rectangle', x1: 180, y1: 80, x2: 220, y2: 120,
      couleur: '#30A46C', epaisseur: 1, plein: true, arrondi: 0, opacite: 1 });
    const id = D.ajouter(doc, { type: 'surligneur', points: [[100, 100], [300, 100]],
      couleur: '#FFD43B', epaisseur: 1 });
    const o = doc.objets[1];
    const a = D.objetSous(doc, { x: 200, y: 100 }, 4, null).id === id;
    D.gommer(doc, id, [{ largeur: 30, points: [[200, 70], [200, 130]] }]);
    const b = !D.touche(o, { x: 200, y: 100 }, 4) && !D.touche(o, { x: 190, y: 95 }, 4)
      && D.touche(o, { x: 150, y: 100 }, 4) && D.touche(o, { x: 250, y: 105 }, 4)
      && !D.touche(o, { x: 150, y: 130 }, 4);
    const c = D.objetSous(doc, { x: 200, y: 100 }, 4, null).id === dessous;
    // Forme pleine : le trou ne se prend pas, le reste oui.
    const plein = doc.objets[0];
    D.gommer(doc, dessous, [{ largeur: 10, points: [[190, 90]] }]);
    const d = !D.touche(plein, { x: 190, y: 90 }, 0) && D.touche(plein, { x: 210, y: 110 }, 0);
    // Flèche coupée en son milieu : chaque bout se prend, pas la coupe.
    const f = { type: 'fleche', x1: 100, y1: 300, x2: 400, y2: 300, double: false,
      couleur: '#E5484D', epaisseur: 1, opacite: 1, id: 9 };
    f.gommes = [{ largeur: 40, points: [[250, 280], [250, 320]] }];
    const e = !D.touche(f, { x: 250, y: 300 }, 4) && D.touche(f, { x: 150, y: 300 }, 4)
      && D.touche(f, { x: 390, y: 300 }, 4);
    verifier(a && b && c && d && e, 'G3 encre gommée : prise là où il en reste, pas dans le '
      + 'trou (trait, forme pleine, flèche) ; l\'objet du dessous s\'y prend',
    [a, b, c, d, e].join());
  }
  // (G4) Les passages de gomme suivent l'objet : déplacer, dupliquer, redimensionner.
  {
    const g = () => [{ largeur: 10, points: [[100, 100], [120, 110]] }];
    const trait = { type: 'crayon', points: [[50, 100], [150, 100], [150, 140]],
      couleur: '#E5484D', epaisseur: 1, opacite: 1, gommes: g() };
    const m = JSON.parse(json(trait));
    D.deplacer(m, trait, 7, -3);
    const a = json(m.gommes) === '[{"largeur":10,"points":[[107,97],[127,107]]}]'
      && json(trait.gommes) === json(g());
    const doc = D.nouveauDocument(640, 480);
    const id = D.ajouter(doc, JSON.parse(json(trait)));
    const [copie] = D.dupliquer(doc, [id]);
    const b = json(doc.objets.find((o) => o.id === copie).gommes)
      === '[{"largeur":10,"points":[[116,116],[136,126]]}]';
    // Tracé agrandi × 2 par le coin bas droit : points et passages au même endroit du tracé,
    // largeur × 2.
    const bq = D.boiteForme(trait), r = JSON.parse(json(trait));
    D.redimensionner(r, trait, 'se', { x: bq.x + 2 * bq.largeur, y: bq.y + 2 * bq.hauteur }, 1);
    const c = json(r.points[1]) === json([bq.x + 2 * (150 - bq.x), bq.y + 2 * (100 - bq.y)])
      && json(r.gommes[0].points[0]) === json([bq.x + 2 * (100 - bq.x), bq.y + 2 * (100 - bq.y)])
      && r.gommes[0].largeur === 20;
    // Rectangle élargi × 2 : le passage garde sa place sur le bord.
    const rect = { type: 'rectangle', x1: 100, y1: 100, x2: 200, y2: 150, couleur: '#E5484D',
      epaisseur: 1, plein: false, arrondi: 0, opacite: 1,
      gommes: [{ largeur: 10, points: [[150, 100], [200, 125]] }] };
    const r2 = JSON.parse(json(rect));
    D.redimensionner(r2, rect, 'se', { x: 300, y: 150 }, 1);
    const d = json(r2.gommes[0].points) === '[[200,100],[300,125]]';
    // Flèche : le bout tiré d'un quart de tour, le passage tourne avec elle.
    const f = { type: 'fleche', x1: 100, y1: 100, x2: 200, y2: 100, double: false,
      couleur: '#E5484D', epaisseur: 1, opacite: 1,
      gommes: [{ largeur: 10, points: [[150, 100], [150, 90]] }] };
    const f2 = JSON.parse(json(f));
    D.redimensionner(f2, f, 'p2', { x: 100, y: 300 }, 1);
    const e = json(f2.gommes) === '[{"largeur":20,"points":[[100,200],[120,200]]}]';
    // La flèche courbée : les passages suivent l'encre (même place le long d'elle, même
    // écart), leur largeur gardée.
    const f3 = JSON.parse(json(f));
    D.redimensionner(f3, f, 'pc', { x: 150, y: 150 }, 1);
    const k = json(f3.gommes) === '[{"largeur":10,"points":[[150,150],[150,140]]}]'
      && f3.cy === 200;
    verifier(a && b && c && d && e && k, 'G4 les passages de gomme suivent l\'objet déplacé, '
      + 'dupliqué, redimensionné (tracé, rectangle, bout de flèche tourné, flèche courbée)',
    [a, b, c, d, e, k].join());
  }
  // (G5) Maj sur une poignée : 15° pour un bout, proportions gardées pour un coin.
  {
    const f = { type: 'fleche', x1: 100, y1: 100, x2: 200, y2: 100, double: false,
      couleur: '#E5484D', epaisseur: 1, opacite: 1 };
    const f1 = { ...f }, f2 = { ...f };
    D.redimensionner(f1, f, 'p2', { x: 200, y: 110 }, 1, true);
    D.redimensionner(f2, f, 'p2', { x: 200, y: 110 }, 1);
    const L = Math.hypot(100, 10);
    const a = Math.abs(f1.y2 - 100) < 1e-9 && Math.abs(f1.x2 - (100 + L)) < 1e-9
      && f2.y2 === 110;
    const f4 = { ...f };
    D.redimensionner(f4, f, 'p1', { x: 100, y: 0 }, 1, true); // 45° autour de p2
    const b = Math.abs((f4.x1 - 200) + (f4.y1 - 100)) < 1e-6 || Math.abs((f4.x1 - 200) - (f4.y1 - 100)) < 1e-6;
    const rect = { type: 'rectangle', x1: 100, y1: 100, x2: 300, y2: 200, couleur: '#E5484D',
      epaisseur: 1, plein: false, arrondi: 0, opacite: 1 };
    const r1 = { ...rect }, r2 = { ...rect };
    D.redimensionner(r1, rect, 'se', { x: 500, y: 210 }, 1, true);
    D.redimensionner(r2, rect, 'se', { x: 500, y: 210 }, 1);
    const c = r1.x2 === 500 && r1.y2 === 300 && r1.x1 === 100 && r1.y1 === 100
      && r2.x2 === 500 && r2.y2 === 210;
    const r3 = { ...rect };
    D.redimensionner(r3, rect, 'nw', { x: 200, y: 190 }, 1, true); // coin haut gauche, × 1/2
    const d = r3.x1 === 200 && r3.y1 === 150 && r3.x2 === 300 && r3.y2 === 200;
    const trait = { type: 'crayon', points: [[100, 100], [200, 150]], couleur: '#E5484D',
      epaisseur: 1, opacite: 1 };
    const bq = D.boiteForme(trait), t1 = JSON.parse(json(trait));
    D.redimensionner(t1, trait, 'se', { x: bq.x + 2 * bq.largeur, y: bq.y + bq.hauteur }, 1, true);
    // Les points du tracé : × 2 sur les deux axes, calés sur le coin haut gauche de la boîte.
    const e = json(t1.points) === json([[bq.x + 2 * (100 - bq.x), bq.y + 2 * (100 - bq.y)],
      [bq.x + 2 * (200 - bq.x), bq.y + 2 * (150 - bq.y)]]);
    verifier(a && b && c && d && e, 'G5 Maj sur une poignée : bout par 15°, rectangle et tracé '
      + 'aux proportions gardées ; sans Maj, libre', [a, b, c, d, e].join());
  }
  // (G6) abandonner : le pas du geste est défait, rien à rétablir de lui ; ce qui était à
  // rétablir avant lui le redevient.
  {
    const doc = D.nouveauDocument(100, 100), h = D.creerHistorique(doc);
    const vide0 = !D.abandonner(h) && h.version === 0;
    D.modifier(h, (d) => D.ajouter(d, exemples()[2]));
    D.modifier(h, (d) => D.ajouter(d, exemples()[3]));
    D.annuler(h);
    const avant = json(doc), v = h.version;
    for (let i = 1; i <= 5; i++) D.modifier(h, (d) => { d.objets[0].x1 = 50 + i; }, 'geste');
    const a = h.passe.length === 2 && !D.peutRetablir(h);
    const b = D.abandonner(h) && json(doc) === avant && h.passe.length === 1
      && h.futur.length === 1 && h.version === v + 6;
    D.retablir(h);
    const c = doc.objets.length === 2 && !D.abandonner(h) === false;
    // Après annuler ou rétablir, plus rien n'est écarté.
    const doc2 = D.nouveauDocument(100, 100), h2 = D.creerHistorique(doc2);
    D.modifier(h2, (d) => D.ajouter(d, exemples()[2]));
    D.annuler(h2);
    D.modifier(h2, (d) => D.ajouter(d, exemples()[3]));
    D.annuler(h2);
    D.retablir(h2);
    const d1 = D.abandonner(h2) && h2.futur.length === 0 && doc2.objets.length === 0;
    verifier(vide0 && a && b && c && d1, 'G6 abandonner : le pas du geste défait sans rester à '
      + 'rétablir, l\'ancien « à rétablir » rendu', [vide0, a, b, c, d1].join());
  }
  // (F) La flèche se courbe à sa poignée du milieu : la courbe passe par la pointe ; près du
  // milieu de la corde (6 px d'écran), elle redevient droite ; Maj : courbe symétrique. Les
  // bouts tirés gardent la courbe (similitude), les passages de gomme suivent.
  {
    const droite = { type: 'fleche', x1: 0, y1: 0, x2: 100, y2: 0, double: false,
      couleur: '#E5484D', epaisseur: 1, opacite: 1 };
    const pres = (u, v) => Math.abs(u - v) < 1e-9;
    const f1 = JSON.parse(json(droite));
    D.redimensionner(f1, droite, 'pc', { x: 50, y: 40 }, 1);
    const pc1 = D.poignees(f1).find((q) => q[2] === 'pc');
    const a = f1.cx === 50 && f1.cy === 80 && pres(pc1[0], 50) && pres(pc1[1], 40)
      && f1.x1 === 0 && f1.x2 === 100;
    // Ramenée à 4,5 px du milieu de la corde : droite (à l'échelle 1) ; à l'échelle 2, 6 px
    // d'écran font 3 px de l'image : encore courbe.
    const f2 = JSON.parse(json(f1));
    D.redimensionner(f2, f1, 'pc', { x: 52, y: 4 }, 1);
    const f2b = JSON.parse(json(f1));
    D.redimensionner(f2b, f1, 'pc', { x: 52, y: 4 }, 2);
    const b = !('cx' in f2) && !('cy' in f2) && f2b.cx === 54 && f2b.cy === 8;
    // Maj : la pointe ramenée sur la médiatrice de la corde.
    const f3 = JSON.parse(json(droite));
    D.redimensionner(f3, droite, 'pc', { x: 80, y: 40 }, 1, true);
    const c = pres(f3.cx, 50) && pres(f3.cy, 80);
    // Un bout tiré d'un quart de tour : la courbe tourne avec la flèche.
    const f4 = JSON.parse(json(f1));
    D.redimensionner(f4, f1, 'p2', { x: 0, y: 100 }, 1);
    const d = pres(f4.cx, -80) && pres(f4.cy, 50) && f4.x2 === 0 && f4.y2 === 100;
    // Les passages de gomme suivent la courbe : au milieu, le long d'elle ; au départ, le
    // départ ; leur largeur gardée.
    const g = { ...JSON.parse(json(droite)),
      gommes: [{ largeur: 10, points: [[50, 0], [50, -10], [0, 0]] }] };
    const f5 = JSON.parse(json(g));
    D.redimensionner(f5, g, 'pc', { x: 50, y: 40 }, 1);
    const e = json(f5.gommes) === '[{"largeur":10,"points":[[50,40],[50,30],[0,0]]}]';
    // Redevenue droite : les passages reviennent.
    const f6 = JSON.parse(json(f5));
    D.redimensionner(f6, f5, 'pc', { x: 50, y: 1 }, 1);
    const k = !('cx' in f6) && json(f6.gommes) === json(g.gommes);
    verifier(a && b && c && d && e && k, 'F flèche courbée à sa poignée du milieu : la courbe '
      + 'passe par la pointe, aimant droit de 6 px d\'écran, Maj symétrique, bouts et passages '
      + 'qui suivent', [a, b, c, d, e, k].join());
  }
  // (V) Une autre variante pour l'objet choisi : même place, mêmes réglages.
  {
    const rect = { type: 'rectangle', x1: 10, y1: 20, x2: 110, y2: 80, couleur: '#30A46C',
      epaisseur: 2, plein: true, arrondi: 10, opacite: 0.5, id: 3 };
    const r1 = JSON.parse(json(rect));
    const a = D.changerVariante(r1, 'ellipse') && r1.type === 'ellipse' && !('arrondi' in r1)
      && r1.plein === true && r1.couleur === '#30A46C' && r1.epaisseur === 2
      && r1.opacite === 0.5 && r1.id === 3 && r1.x1 === 10 && r1.y2 === 80;
    const b = D.changerVariante(r1, 'rectangle') && r1.type === 'rectangle' && r1.arrondi === 6
      && r1.plein === true;
    // Une ligne : la diagonale de la boîte, sans remplissage ni arrondi ; et retour.
    const l1 = JSON.parse(json(rect));
    const c = D.changerVariante(l1, 'ligne') && l1.type === 'ligne' && !('plein' in l1)
      && !('arrondi' in l1) && l1.x1 === 10 && l1.y1 === 20 && l1.x2 === 110 && l1.y2 === 80;
    const l2 = { type: 'ligne', x1: 110, y1: 80, x2: 10, y2: 20, couleur: '#E5484D',
      epaisseur: 1, opacite: 1 };
    const d = D.changerVariante(l2, 'ellipse') && l2.type === 'ellipse' && l2.plein === false
      && l2.x1 === 10 && l2.x2 === 110 && l2.y1 === 20 && l2.y2 === 80;
    const m = { type: 'masque', x1: 0, y1: 0, x2: 50, y2: 50, sorte: 'pixeliser', intensite: 2,
      graine: 7 };
    const e = D.changerVariante(m, 'bloc') && m.sorte === 'bloc' && m.intensite === 2
      && m.graine === 7 && D.changerVariante(m, 'flouter') && m.sorte === 'flouter';
    const s = { type: 'projecteur', x1: 0, y1: 0, x2: 50, y2: 50, forme: 'rectangle' };
    const f = D.changerVariante(s, 'ovale') && s.forme === 'ovale';
    // Rien ne change : même variante, variante d'un autre outil, objet sans variante.
    const tous = [JSON.parse(json(rect)), JSON.parse(json(m)), JSON.parse(json(s)),
      { type: 'fleche', x1: 0, y1: 0, x2: 10, y2: 0 }, { type: 'crayon', points: [[0, 0]] }];
    const avant = json(tous);
    const g = !D.changerVariante(tous[0], 'rectangle') && !D.changerVariante(tous[0], 'ovale')
      && !D.changerVariante(tous[0], 'bloc') && !D.changerVariante(tous[1], 'flouter')
      && !D.changerVariante(tous[1], 'ellipse') && !D.changerVariante(tous[2], 'ovale')
      && !D.changerVariante(tous[2], 'ellipse') && !D.changerVariante(tous[3], 'ligne')
      && !D.changerVariante(tous[4], 'rectangle') && json(tous) === avant;
    verifier(a && b && c && d && e && f && g, 'V changerVariante : forme, masque, projecteur à '
      + 'la même place et aux mêmes réglages ; rien pour une même variante ou un autre outil',
    [a, b, c, d, e, f, g].join());
  }
  // Rectangles normalisés ; crayon lissé (Chaikin deux fois).
  {
    const o = { type: 'masque', x1: 10, y1: 30, x2: 5, y2: 2 };
    D.normaliser(o);
    const l = D.lisser([[0, 0], [10, 0], [10, 10]]);
    verifier(o.x1 === 5 && o.x2 === 10 && o.y1 === 2 && o.y2 === 30 && l.length === 12
      && l[0][0] === 0 && l[11][1] === 10 && l[1][0] === 0.63, '11 normaliser, lisser',
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
  // Blocs de Pixeliser sur un axe (règle du contrat, réécrite ici) : coupes sur la grille de
  // l'image, reste de bord de moins d'un demi-bloc fondu dans son voisin.
  window.blocs = (debut, taille, bs) => {
    const fin = debut + taille, c = [debut];
    for (let m = (Math.floor(debut / bs) + 1) * bs; m < fin; m += bs) c.push(m);
    c.push(fin);
    let s = c.slice(1).map((v, i) => [c[i], v]);
    if (s.length > 1 && s[0][1] - s[0][0] < bs / 2) s = [[s[0][0], s[1][1]], ...s.slice(2)];
    const z = s.length - 1;
    if (z > 0 && s[z][1] - s[z][0] < bs / 2) s = [...s.slice(0, z - 1), [s[z - 1][0], s[z][1]]];
    return s;
  };
  // Début du bloc qui contient v.
  window.debutBloc = (segments, v) => segments.find(([a, b]) => v >= a && v < b)[0];
  window.exporter = (im, doc) => {
    window.M.peindreMasques(im.ctx, im.base, doc);
    return window.M.composer(doc, im.fond);
  };
  window.docAvec = (l, h, objets, recadrage = null) => {
    const doc = window.D.nouveauDocument(l, h);
    // Graine du grain de Pixeliser fixée ici (la page la tire au hasard) : rien n'y dépend
    // du hasard.
    for (const o of objets) {
      const q = JSON.parse(JSON.stringify(o));
      if (q.type === 'masque' && q.graine == null) q.graine = 7919;
      window.D.ajouter(doc, q);
    }
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
      // Zone arrondie vers l'extérieur : x de 101 à 334, y de 57 à 271.
      const zoneM = { type: 'masque', x1: 101.4, y1: 57.6, x2: 333.2, y2: 270.3,
        sorte: 'pixeliser', intensite: i };
      const s = exporter(im, docAvec(640, 480, [zoneM]));
      const bx = blocs(101, 233, bs), by = blocs(57, 214, bs);
      let unis = true, origine = 0, total = 0;
      for (let y = 57; y < 271; y++) {
        for (let x = 101; x < 334; x++) {
          const k = (y * 640 + x) * 4;
          // Premier pixel de son bloc, sur la grille de l'image, bornée à la zone.
          const x0 = debutBloc(bx, x), y0 = debutBloc(by, y);
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
      // Aucun bloc de moins d'un demi-bloc.
      const petits = [...bx, ...by].filter(([a, b]) => b - a < bs / 2).length;
      res['pixeliser' + bs] = { unis, origine, total, petits,
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
    verifier(x.unis && x.voisins && x.origine * 10000 <= x.total && x.petits === 0,
      `15 Pixeliser ${bs} px : blocs unis alignés sur la grille (restes de bord fondus), pixels `
      + 'd\'origine au niveau du hasard (au plus 1 sur 10 000)',
    JSON.stringify(x));
  }

  // (d bis) Bords et petites zones : bloc de coin de 1 px fondu, zone plus petite qu'un bloc
  // = un bloc, zones de 1 à 2 px masquées, bords fractionnaires couverts, Bloc noir plein ;
  // Flouter sur une image à alpha : rien d'origine sous le flou.
  const rb = await p.evaluate(async () => {
    const im = await preparer(200, 120);
    const px = (s, x, y) => Array.from(s.rgba.slice((y * 200 + x) * 4, (y * 200 + x) * 4 + 4));
    const egal = (a, b) => a.every((v, i) => v === b[i]);
    const orig = (x, y) => Array.from(im.src.slice((y * 200 + x) * 4, (y * 200 + x) * 4 + 4));
    const res = {};
    // Zone qui commence en (12, 12), blocs de 13 : le pixel (12, 12) est dans le bloc 12..26.
    let s = exporter(im, docAvec(200, 120, [{ type: 'masque', x1: 12, y1: 12, x2: 60, y2: 60,
      sorte: 'pixeliser', intensite: 1 }]));
    res.coin = egal(px(s, 12, 12), px(s, 25, 25)) && egal(px(s, 12, 12), px(s, 12, 25))
      && !egal(px(s, 12, 12), px(s, 26, 12));
    // Zone de 8 × 7 à cheval sur la ligne 52 de la grille : un seul bloc.
    s = exporter(im, docAvec(200, 120, [{ type: 'masque', x1: 50, y1: 50, x2: 58, y2: 57,
      sorte: 'pixeliser', intensite: 1 }]));
    let un = true;
    for (let y = 50; y < 57; y++) for (let x = 50; x < 58; x++) {
      if (!egal(px(s, x, y), px(s, 50, 50))) un = false;
    }
    res.petiteUnBloc = un;
    // Zones de 1 à 2 px, à cheval sur le bord, bords fractionnaires : aucun pixel d'origine.
    const restes = [];
    const compter = (o, x0, x1, y0, y1) => {
      const t = exporter(im, docAvec(200, 120, [o]));
      let n = 0;
      for (let y = y0; y < y1; y++) {
        for (let x = x0; x < x1; x++) if (egal(px(t, x, y), orig(x, y))) n++;
      }
      return n;
    };
    for (const sorte of ['pixeliser', 'flouter', 'bloc']) {
      for (const w of [1, 1.4, 1.9]) {
        const n = compter({ type: 'masque', x1: 50, y1: 50, x2: 50 + w, y2: 80, sorte,
          intensite: 1 }, 50, Math.ceil(50 + w), 50, 80);
        if (n) restes.push(`${sorte} ${w} px : ${n}`);
      }
      let n = compter({ type: 'masque', x1: -30, y1: 50, x2: 1, y2: 80, sorte, intensite: 1 },
        0, 1, 50, 80);
      if (n) restes.push(`${sorte} au bord : ${n}`);
      n = compter({ type: 'masque', x1: 40.45, y1: 40.45, x2: 90.55, y2: 90.55, sorte,
        intensite: 1 }, 40, 91, 40, 91);
      if (n) restes.push(`${sorte} bords fractionnaires : ${n}`);
    }
    res.restes = restes;
    // Bloc noir : coins compris, tout à #111214.
    s = exporter(im, docAvec(200, 120, [{ type: 'masque', x1: 100, y1: 20, x2: 150, y2: 60,
      sorte: 'bloc', intensite: 1 }]));
    let noir = true;
    for (let y = 20; y < 60; y++) for (let x = 100; x < 150; x++) {
      if (!egal(px(s, x, y), [17, 18, 20, 255])) noir = false;
    }
    res.noir = noir;
    // Flouter, image à alpha 0 sur sa moitié gauche, puis à alpha 128 : corrélation de la
    // sortie avec l'origine (canal vert), au niveau de celle d'une image opaque.
    const correlation = (a, b, l, x0, x1, y0, y1) => {
      let sa = 0, sb = 0, saa = 0, sbb = 0, sab = 0, n = 0;
      for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) {
        const i = (y * l + x) * 4 + 1;
        sa += a[i]; sb += b[i]; saa += a[i] * a[i]; sbb += b[i] * b[i]; sab += a[i] * b[i]; n++;
      }
      const cov = sab / n - (sa / n) * (sb / n);
      return cov / Math.sqrt((saa / n - (sa / n) ** 2) * (sbb / n - (sb / n) ** 2));
    };
    const moitie = await preparer(240, 120, (i) => (i % 240 < 120 ? 0 : 255));
    const moyen = await preparer(240, 120, () => 128);
    const opaque = await preparer(240, 120);
    res.alpha = [];
    for (const intensite of [0, 1, 2]) {
      const o = { type: 'masque', x1: 20, y1: 10, x2: 220, y2: 110, sorte: 'flouter', intensite };
      const r0 = correlation(exporter(opaque, docAvec(240, 120, [o])).rgba, opaque.src, 240,
        120, 132, 20, 100);
      const r1 = correlation(exporter(moitie, docAvec(240, 120, [o])).rgba, moitie.src, 240,
        120, 132, 20, 100);
      const r2 = correlation(exporter(moyen, docAvec(240, 120, [o])).rgba, moyen.src, 240,
        30, 210, 20, 100);
      res.alpha.push([r0, r1, r2].map((v) => Math.round(v * 100) / 100));
    }
    return res;
  });
  verifier(rb.coin, '15b Pixeliser : le bloc de coin de 1 px est fondu dans son voisin');
  verifier(rb.petiteUnBloc, '15b Pixeliser : une zone plus petite qu\'un bloc est un seul bloc');
  verifier(rb.restes.length === 0, '15c masques de 1 à 2 px, au bord, à bords fractionnaires : '
    + 'aucun pixel d\'origine', JSON.stringify(rb.restes));
  verifier(rb.noir, '15d Bloc noir plein, coins compris');
  verifier(rb.alpha.every(([r0, r1, r2]) => r1 < Math.max(0.4, r0 + 0.15) && Math.abs(r2) < 0.2),
    '16b Flouter sur alpha 0 et 128, trois intensités : corrélation avec l\'origine au niveau '
    + 'du bruit [opaque, moitié alpha 0, alpha 128]', JSON.stringify(rb.alpha));
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
    return { diff, max, dehors, hors2, dans2, bord };
  });
  verifier(r.diff === 0, '19 vue à la taille réelle (image décalée, recadrée) = export au bit près',
    JSON.stringify(r));
  verifier(r.bord === 0, '19 vue à la taille réelle = export, objets à cheval sur le bord',
    r.bord);
  verifier(r.dehors === 0, '19 la vue ne peint que le cadre', r.dehors);
  verifier(r.hors2 === 0 && r.dans2 > 0, '19 vue à l\'échelle 2 : ombre à l\'échelle, dans 2 × '
    + 'la boîte', JSON.stringify(r));

  // Contour de sélection : sur un fond gris uni, accent magenta (jeton --accent posé par
  // l'essai) : sa teinte est dans la vue avec sélection, ni sans sélection ni dans l'export.
  r = await p.evaluate(async () => {
    document.documentElement.style.setProperty('--accent', '#FF00FF');
    const gris = new Uint8ClampedArray(300 * 200 * 4).fill(128);
    const im = { l: 300, h: 200, src: gris, base: await createImageBitmap(
      new ImageData(gris, 300, 200), { premultiplyAlpha: 'none', colorSpaceConversion: 'none' }) };
    im.fond = document.createElement('canvas');
    im.fond.width = 300; im.fond.height = 200;
    im.ctx = im.fond.getContext('2d', { willReadFrequently: true });
    const doc = docAvec(300, 200, [{ type: 'rectangle', x1: 60, y1: 50, x2: 200, y2: 150,
      couleur: '#0090FF', epaisseur: 1, plein: false, arrondi: 10, opacite: 1 }]);
    const s = exporter(im, doc);
    const vue = document.createElement('canvas');
    vue.width = 600; vue.height = 400;
    const c = vue.getContext('2d', { willReadFrequently: true });
    const compter = (d) => {
      let n = 0;
      for (let i = 0; i < d.length; i += 4) {
        // Teinte de l'accent (rouge et bleu bien au-dessus du vert) : ni le gris, ni le
        // bleu du rectangle, ni son ombre.
        if (d[i] - d[i + 1] > 40 && d[i + 2] - d[i + 1] > 40) n++;
      }
      return n;
    };
    M.peindreVue(c, doc, im.fond, { x: 0, y: 0, echelle: 2, dpr: 2 }, [doc.objets[0].id]);
    const avec = compter(c.getImageData(0, 0, 600, 400).data);
    M.peindreVue(c, doc, im.fond, { x: 0, y: 0, echelle: 2, dpr: 2 }, []);
    const sans = compter(c.getImageData(0, 0, 600, 400).data);
    return { avec, sans, exportes: compter(s.rgba) };
  });
  verifier(r.avec > 200 && r.sans === 0 && r.exportes === 0,
    '19 contour de sélection (couleur de l\'accent) dans la vue sélectionnée seulement',
    JSON.stringify(r));

  // (h) Pixeliser ne se relit pas par comparaison de candidats. Un code de 8 chiffres (12 px)
  // est masqué ; chaque chiffre est cherché en masquant les 10 variantes de la même façon et
  // en gardant la plus proche. Témoin : candidats masqués avec la graine du secret (le grain
  // s'annule, comme sans grain) : l'attaque marche. Essai : graines différentes (la page en
  // tire une par objet) : l'attaque retombe près du hasard (0,8 chiffre sur 8 en moyenne).
  r = await p.evaluate(async () => {
    const L = 160, H = 40, X = 20, Y = 12;
    const rendre = async (texte, dx = 0) => {
      const c = new OffscreenCanvas(L, H), x = c.getContext('2d');
      x.fillStyle = '#FFFFFF'; x.fillRect(0, 0, L, H);
      x.fillStyle = '#1A1A1A'; x.font = '400 12px "Segoe UI", system-ui, sans-serif';
      x.textBaseline = 'top'; x.fillText(texte, X + dx, Y);
      const b = await createImageBitmap(c);
      const fond = document.createElement('canvas');
      fond.width = L; fond.height = H;
      return { base: b, fond, ctx: fond.getContext('2d', { willReadFrequently: true }) };
    };
    const zone = { type: 'masque', x1: X - 6, y1: Y - 6, x2: X + 110, y2: Y + 20,
      sorte: 'pixeliser' };
    const masquer = (im, intensite, graine) =>
      exporter(im, docAvec(L, H, [{ ...zone, intensite, graine }])).rgba;
    const dist = (a, b) => {
      let d = 0;
      for (let i = 0; i < a.length; i++) if ((i & 3) !== 3) d += (a[i] - b[i]) ** 2;
      return d;
    };
    const secret = '64861526';
    const res = [];
    for (const intensite of [0, 1, 2]) {
      const cible = masquer(await rendre(secret, 0.3), intensite, 1);
      const trouves = { temoin: 0, essai: 0 };
      for (let pos = 0; pos < 8; pos++) {
        for (const cas of ['temoin', 'essai']) {
          let meilleur = -1, dm = Infinity;
          for (let d = 0; d < 10; d++) {
            const t = secret.slice(0, pos) + d + secret.slice(pos + 1);
            const graine = cas === 'temoin' ? 1 : 1000 + pos * 10 + d;
            const dd = dist(cible, masquer(await rendre(t), intensite, graine));
            if (dd < dm) { dm = dd; meilleur = d; }
          }
          if (String(meilleur) === secret[pos]) trouves[cas]++;
        }
      }
      res.push(trouves);
    }
    return res;
  });
  verifier(r.reduce((n, x) => n + x.temoin, 0) >= 12 && r.every((x) => x.essai <= 3),
    '23 Pixeliser, attaque par candidats sur un texte de 12 px : au plus 3 chiffres sur 8 '
    + 'retrouvés (témoin à graine connue : 12 sur 24 au moins)', JSON.stringify(r));

  // (P) Projecteur : voile rgba(8, 10, 16, .55) sur toute l'image, percé de la réunion des
  // projecteurs, peint après tous les objets, à l'export comme à l'écran.
  r = await p.evaluate(async () => {
    const im = await preparer(480, 320);
    const L = 480, H = 320;
    const px = (s, x, y) => Array.from(s.rgba.slice((y * L + x) * 4, (y * L + x) * 4 + 4));
    const src = (x, y) => Array.from(im.src.slice((y * L + x) * 4, (y * L + x) * 4 + 4));
    const voile = (c) => [8, 10, 16].map((v, i) => c[i] * 0.45 + v * 0.55);
    const proche = (a, b, t = 2) => b.every((v, i) => Math.abs(a[i] - v) <= t);
    const egal = (a, b) => a.every((v, i) => v === b[i]);
    const A = { type: 'projecteur', forme: 'rectangle', x1: 100, y1: 80, x2: 300, y2: 200 };
    const B = { type: 'projecteur', forme: 'rectangle', x1: 250, y1: 150, x2: 400, y2: 260 };
    const O = { type: 'projecteur', forme: 'ovale', x1: 100, y1: 80, x2: 300, y2: 200 };
    const dans = (o, x, y, m) => x >= o.x1 + m && x < o.x2 - m && y >= o.y1 + m && y < o.y2 - m;
    // Classe chaque pixel loin des bords (2 px) : éclairé (= source), voilé une fois, autre.
    const classer = (s, liste, pred) => {
      let eclaire = 0, voile1 = 0, faux = 0;
      for (let y = 0; y < H; y++) for (let x = 0; x < L; x++) {
        const k = pred(x, y);
        if (k === null) continue;
        const v = px(s, x, y), o = src(x, y);
        if (k) { if (egal(v, o)) eclaire++; else faux++; }
        else if (proche(v, [...voile(o), 255])) voile1++; else faux++;
      }
      return { eclaire, voile1, faux };
    };
    // Prédicat : vrai (éclairé), faux (voilé), null (bord, ignoré).
    const rect = (liste) => (x, y) => {
      if (liste.some((o) => dans(o, x, y, 10))) return true;
      if (liste.every((o) => !dans(o, x, y, -2))) return false;
      return null;
    };
    const res = {};
    res.seul = classer(exporter(im, docAvec(L, H, [A])), [A], rect([A]));
    const sA = exporter(im, docAvec(L, H, [A]));
    // Coin arrondi (rayon 8) : (101, 81) est voilé, (110, 90) éclairé.
    res.coin = !egal(px(sA, 101, 81), src(101, 81)) && egal(px(sA, 110, 90), src(110, 90));
    res.deux = classer(exporter(im, docAvec(L, H, [A, B])), [A, B], rect([A, B]));
    // Ovale : centre éclairé, coins de sa boîte voilés.
    const sO = exporter(im, docAvec(L, H, [O]));
    res.ovale = egal(px(sO, 200, 140), src(200, 140)) && egal(px(sO, 150, 140), src(150, 140))
      && proche(px(sO, 104, 84), [...voile(src(104, 84)), 255])
      && proche(px(sO, 296, 196), [...voile(src(296, 196)), 255]);
    // Ordre : un rectangle rouge plein rangé APRÈS le projecteur reste sous le voile, comme
    // un masque.
    const rouge = { type: 'rectangle', x1: 50, y1: 50, x2: 350, y2: 250, couleur: '#E5484D',
      epaisseur: 1, plein: true, arrondi: 0, opacite: 1 };
    const m = { type: 'masque', x1: 20, y1: 260, x2: 120, y2: 310, sorte: 'bloc', intensite: 1 };
    const sR = exporter(im, docAvec(L, H, [A, rouge, m]));
    res.ordre = egal(px(sR, 200, 140), [229, 72, 77, 255])
      && proche(px(sR, 70, 60), [...voile([229, 72, 77]), 255])
      && proche(px(sR, 60, 280), [...voile([17, 18, 20]), 255]);
    // Recadrage : le voile reste celui de l'image entière (export recadré = son morceau).
    const k = { x: 37, y: 21, largeur: 301, hauteur: 203 };
    const tout = exporter(im, docAvec(L, H, [A, B, O, rouge]));
    const sk = exporter(im, docAvec(L, H, [A, B, O, rouge], k));
    let diffK = 0;
    for (let y = 0; y < k.hauteur; y++) for (let x = 0; x < k.largeur; x++) {
      for (let q = 0; q < 4; q++) {
        if (sk.rgba[(y * k.largeur + x) * 4 + q] !== tout.rgba[((y + k.y) * L + x + k.x) * 4 + q]) {
          diffK++;
        }
      }
    }
    res.cadre = diffK;
    // Écran = export, à la taille réelle, recadré, décalé ; le projecteur B passé en cours
    // de tracé (`ajout`) donne la même vue que B rangé dans le document.
    const vue = document.createElement('canvas');
    vue.width = 600; vue.height = 400;
    const c = vue.getContext('2d', { willReadFrequently: true });
    const docV = docAvec(L, H, [A, O, rouge], k);
    M.peindreMasques(im.ctx, im.base, docV);
    M.peindreVue(c, docV, im.fond, { x: 50, y: 40, echelle: 1, dpr: 1 }, [],
      { ajout: { ...B, id: 999 } });
    const v = c.getImageData(50 + k.x, 40 + k.y, k.largeur, k.hauteur).data;
    let diffV = 0;
    for (let i = 0; i < v.length; i++) if (v[i] !== sk.rgba[i]) diffV++;
    res.ecran = diffV;
    // Sans projecteur : un projecteur posé puis supprimé ne laisse rien, au bit près ; la
    // vue sans projecteur est l'export.
    const d0 = docAvec(L, H, [rouge, m]);
    const s0 = exporter(im, d0);
    const d1 = docAvec(L, H, [rouge, A, m]);
    D.supprimer(d1, d1.objets.filter((o) => o.type === 'projecteur').map((o) => o.id));
    const s1 = exporter(im, d1);
    let horsBoite = 0;
    const bR = D.boite(rouge), bM = D.boite(m);
    for (let y = 0; y < H; y++) for (let x = 0; x < L; x++) {
      const pris = [bR, bM].some((b) => x + 1 > b.x && x < b.x + b.largeur
        && y + 1 > b.y && y < b.y + b.hauteur);
      if (!pris && !egal(px(s0, x, y), src(x, y))) horsBoite++;
    }
    res.sans = s0.rgba.every((x, i) => x === s1.rgba[i]) && horsBoite === 0;
    return res;
  });
  const okClasse = (x) => x.faux === 0 && x.eclaire > 1000 && x.voile1 > 1000;
  verifier(okClasse(r.seul) && r.coin, 'P2 projecteur seul : éclairé au bit près, voilé une fois '
    + '(± 2) ailleurs, coins arrondis', JSON.stringify(r.seul));
  verifier(okClasse(r.deux), 'P3 deux projecteurs qui se chevauchent : la réunion n\'est voilée '
    + 'nulle part, aucun double voile', JSON.stringify(r.deux));
  verifier(r.ovale, 'P4 ovale : centre éclairé, coins de sa boîte voilés');
  verifier(r.ordre, 'P5 voile après tous les objets : rectangle rouge rangé après, intact dans '
    + 'la zone, voilé hors d\'elle ; masque voilé');
  verifier(r.cadre === 0 && r.ecran === 0, 'P6 recadré : voile de l\'image entière ; écran (avec '
    + 'le projecteur en cours de tracé) = export au bit près', JSON.stringify([r.cadre, r.ecran]));
  verifier(r.sans, 'P7 sans projecteur : rien ne change d\'un bit');

  // Image aux coins transparents (capture de fenêtre) : le voile n'assombrit que les pixels
  // qui existent, à l'export comme à l'écran.
  r = await p.evaluate(async () => {
    const L = 200, H = 150, R = 20;
    const coin = (i) => {
      const x = i % L, y = Math.floor(i / L);
      const dx = Math.max(R - x, x - (L - 1 - R), 0), dy = Math.max(R - y, y - (H - 1 - R), 0);
      return Math.hypot(dx, dy) > R ? 0 : 255;
    };
    const im = await preparer(L, H, coin);
    const doc = docAvec(L, H, [{ type: 'projecteur', forme: 'rectangle', x1: 60, y1: 40,
      x2: 140, y2: 110 }]);
    const s = exporter(im, doc);
    const vue = document.createElement('canvas');
    vue.width = L; vue.height = H;
    const c = vue.getContext('2d', { willReadFrequently: true });
    M.peindreVue(c, doc, im.fond, { x: 0, y: 0, echelle: 1, dpr: 1 }, []);
    const v = c.getImageData(0, 0, L, H).data;
    let transparents = 0, nonNuls = 0, voiles = 0, ecran = 0;
    for (let i = 0; i < L * H; i++) {
      if (coin(i) === 0) {
        transparents++;
        if (s.rgba[4 * i + 3] || s.rgba[4 * i] || v[4 * i + 3]) nonNuls++;
      } else if (i === 75 * L + 30 && s.rgba[4 * i + 3] === 255
        && s.rgba[4 * i] < im.src[4 * i] * 0.45 + 8 * 0.55 + 3) voiles++;
      for (let q = 0; q < 4; q++) if (s.rgba[4 * i + q] !== v[4 * i + q]) ecran++;
    }
    return { transparents, nonNuls, voiles, ecran };
  });
  verifier(r.transparents > 100 && r.nonNuls === 0 && r.voiles === 1 && r.ecran === 0,
    'P9 image aux coins transparents : ils restent transparents sous le voile (export et '
    + 'écran), l\'opaque est voilé, écran = export', JSON.stringify(r));

  // Coût du voile : vue 3840 × 2160 avec deux projecteurs, 20 peintures, chacune attendue
  // (lecture d'un pixel), comparée à la même vue sans projecteur. Ici, la vue sans
  // projecteur coûte déjà 9 à 12 ms (image de 8 Mpx et lecture) et le voile 5 à 11 ms de
  // plus. La borne porte sur le surcoût du voile : 40 ms, un garde-fou large contre un
  // voile devenu lent (la mesure varie trop d'une machine à l'autre pour une borne fine).
  r = await p.evaluate(async () => {
    const L = 3840, H = 2160;
    const im = await preparer(L, H);
    const objets = [
      { type: 'projecteur', forme: 'rectangle', x1: 400, y1: 300, x2: 1600, y2: 1200 },
      { type: 'projecteur', forme: 'ovale', x1: 1200, y1: 900, x2: 2600, y2: 1900 }];
    const vue = document.createElement('canvas');
    vue.width = L; vue.height = H;
    const c = vue.getContext('2d');
    const mesurer = (doc) => {
      M.peindreMasques(im.ctx, im.base, doc);
      M.peindreVue(c, doc, im.fond, { x: 0, y: 0, echelle: 1, dpr: 1 }, []);
      c.getImageData(0, 0, 1, 1);
      const t0 = performance.now();
      for (let i = 0; i < 20; i++) {
        M.peindreVue(c, doc, im.fond, { x: 0, y: 0, echelle: 1, dpr: 1 }, []);
        c.getImageData(0, 0, 1, 1); // attend la fin du dessin
      }
      return (performance.now() - t0) / 20;
    };
    const avec = mesurer(docAvec(L, H, objets)), sans = mesurer(docAvec(L, H, []));
    return { avec: Math.round(avec * 10) / 10, sans: Math.round(sans * 10) / 10 };
  });
  verifier(r.avec - r.sans <= 40, `P10 vue 3840 × 2160, deux projecteurs : ${r.avec} ms par `
    + `peinture, sans projecteur ${r.sans} ms (surcoût borné à 40 ms)`);

  // Option `rayon` : coins de l'image arrondis à l'écran seulement.
  r = await p.evaluate(async () => {
    const im = await preparer(200, 150);
    const doc = docAvec(200, 150, [{ type: 'projecteur', forme: 'rectangle', x1: 20, y1: 20,
      x2: 120, y2: 100 }]);
    M.peindreMasques(im.ctx, im.base, doc);
    const vue = document.createElement('canvas');
    vue.width = 300; vue.height = 250;
    const c = vue.getContext('2d', { willReadFrequently: true });
    const peindre = (o) => {
      M.peindreVue(c, doc, im.fond, { x: 30, y: 40, echelle: 1, dpr: 1 }, [], o);
      return c.getImageData(0, 0, 300, 250).data;
    };
    const sans = peindre({}), zero = peindre({ rayon: 0 }), arrondi = peindre({ rayon: 10 });
    let horsCoins = 0, coins = 0;
    for (let y = 0; y < 250; y++) for (let x = 0; x < 300; x++) {
      const i = (y * 300 + x) * 4;
      const d = [0, 1, 2, 3].some((q) => arrondi[i + q] !== zero[i + q]);
      const coin = (x < 40 || x >= 220) && (y < 50 || y >= 180);
      if (d && coin) coins++;
      if (d && !coin) horsCoins++;
    }
    const s = M.composer(doc, im.fond);
    const export0 = s.rgba[3] === 255; // coin haut gauche de l'export : opaque
    return { pareil: sans.every((v, i) => v === zero[i]), coins, horsCoins,
      coinVide: arrondi[(40 * 300 + 30) * 4 + 3] === 0, export0 };
  });
  verifier(r.pareil && r.coins > 0 && r.horsCoins === 0 && r.coinVide && r.export0,
    'P8 option rayon : coins de l\'image transparents à l\'écran, rien d\'autre ne change, '
    + 'absente = 0, export sans arrondi', JSON.stringify(r));

  // (G) Gomme au pixel : dessin (fil « Rendre l'annotation plus naturelle »).
  // (G7) Un rectangle plein coupé par une bande de 40 px : au milieu de la bande, l'image au
  // bit près (ni encre ni ombre) ; l'encre reste de part et d'autre ; rien ne change hors de
  // la boîte. Tout gommé (l'objet encore dans le document, comme pendant le geste) : aucun
  // pixel ne change, pas même l'ombre.
  r = await p.evaluate(async () => {
    const L = 300, H = 200, im = await preparer(L, H);
    const egal = (s, x, y) => {
      const i = (y * L + x) * 4;
      return [0, 1, 2, 3].every((q) => s.rgba[i + q] === im.src[i + q]);
    };
    const rect = { type: 'rectangle', x1: 40, y1: 40, x2: 260, y2: 160, couleur: '#E5484D',
      epaisseur: 1, plein: true, arrondi: 0, opacite: 1 };
    const s = exporter(im, docAvec(L, H, [{ ...rect,
      gommes: [{ largeur: 40, points: [[150, 10], [150, 190]] }] }]));
    let trou = 0, trouN = 0, encre = 0, encreN = 0;
    for (let y = 30; y < 170; y++) {
      for (let x = 144; x <= 156; x++) {
        trouN++;
        if (egal(s, x, y)) trou++;
      }
      if (y < 45 || y >= 155) continue;
      for (const x of [60, 100, 200, 240]) {
        encreN++;
        if (!egal(s, x, y)) encre++;
      }
    }
    const e = ecarts(im, s, [D.boite(rect)]);
    const tout = exporter(im, docAvec(L, H, [{ ...rect,
      gommes: [{ largeur: 400, points: [[150, 100]] }] }]));
    return { trou, trouN, encre, encreN, dehors: e.dehors, tout: ecarts(im, tout, []).dehors };
  });
  verifier(r.trou === r.trouN && r.encre === r.encreN && r.dehors === 0 && r.tout === 0,
    'G7 gommé : l\'image revient au bit près au milieu de la bande, l\'encre reste de part et '
    + 'd\'autre, rien hors de la boîte ; tout gommé : aucun pixel, pas même l\'ombre',
    JSON.stringify(r));

  // (G8) Vue à la taille réelle = export au bit près, encres gommées comprises : image
  // entière et recadrée, encres à cheval sur le bord, opacité, surligneur, passages d'un
  // point ; et les passages changent bien l'export.
  r = await p.evaluate(async () => {
    const im = await preparer(300, 200);
    const g = (l, ...traces) => traces.map((points) => ({ largeur: l, points }));
    const objets = [
      { type: 'fleche', x1: -40, y1: 30, x2: 160, y2: 120, double: true, couleur: '#E5484D',
        epaisseur: 2, opacite: 1, gommes: g(18, [[60, 60], [70, 110]]) },
      { type: 'ellipse', x1: 150, y1: 20, x2: 280, y2: 120, couleur: '#30A46C', epaisseur: 2,
        plein: false, opacite: 0.6, gommes: g(24, [[150, 70], [215, 20]]) },
      { type: 'ligne', x1: 20, y1: 190, x2: 290, y2: 140, couleur: '#8E4EC6', epaisseur: 2,
        opacite: 1, gommes: g(18, [[150, 140], [160, 200]]) },
      { type: 'crayon', points: [[30, 140], [70, 170], [110, 130], [150, 180]],
        couleur: '#F5A524', epaisseur: 2, opacite: 1, gommes: g(30, [[90, 150]]) },
      { type: 'surligneur', points: [[20, 100], [280, 104]], couleur: '#FFD43B', epaisseur: 2,
        gommes: g(40, [[100, 80], [100, 130]]).concat(g(10, [[200, 102]])) },
      { type: 'rectangle', x1: 200, y1: 150, x2: 330, y2: 230, couleur: '#0090FF',
        epaisseur: 1, plein: true, arrondi: 8, opacite: 1, gommes: g(20, [[260, 190], [320, 190]]) },
    ];
    const sansGomme = objets.map((o) => ({ ...o, gommes: undefined }));
    const vue = document.createElement('canvas');
    vue.width = 400; vue.height = 300;
    const c = vue.getContext('2d', { willReadFrequently: true });
    const res = {};
    for (const [nom, cadre] of [['entiere', null],
      ['recadree', { x: 10, y: 20, largeur: 270, hauteur: 170 }]]) {
      const doc = docAvec(300, 200, objets, cadre);
      const s = exporter(im, doc);
      M.peindreVue(c, doc, im.fond, { x: 50, y: 40, echelle: 1, dpr: 1 }, []);
      const kx = cadre ? cadre.x : 0, ky = cadre ? cadre.y : 0;
      const v = c.getImageData(50 + kx, 40 + ky, s.largeur, s.hauteur).data;
      let diff = 0;
      for (let i = 0; i < v.length; i++) if (v[i] !== s.rgba[i]) diff++;
      const s0 = exporter(im, docAvec(300, 200, sansGomme, cadre));
      let change = 0;
      for (let i = 0; i < s0.rgba.length; i += 4) {
        if ([0, 1, 2, 3].some((q) => s0.rgba[i + q] !== s.rgba[i + q])) change++;
      }
      res[nom] = { diff, change };
    }
    return res;
  });
  verifier(r.entiere.diff === 0 && r.recadree.diff === 0 && r.entiere.change > 1000
    && r.recadree.change > 1000, 'G8 encres gommées : vue à la taille réelle = export au bit '
    + 'près (image entière et recadrée, bord, opacité, surligneur), les passages changent '
    + 'l\'export', JSON.stringify(r));

  // (G9) Pendant le geste, l'aperçu (passages en option de la vue) est ce que le geste
  // laissera une fois fini (passages gardés dans l'objet), au bit près, aux échelles 1, 1,5
  // et 2 ; une encre déjà gommée cumule ses anciens passages et ceux du geste.
  r = await p.evaluate(async () => {
    const im = await preparer(300, 200);
    const base = [
      { type: 'crayon', points: [[20, 20], [120, 160], [220, 40], [280, 180]],
        couleur: '#E5484D', epaisseur: 2, opacite: 1 },
      { type: 'surligneur', points: [[20, 100], [280, 100]], couleur: '#FFD43B', epaisseur: 1,
        gommes: [{ largeur: 12, points: [[40, 100], [60, 100]] }] },
      { type: 'fleche', x1: 30, y1: 180, x2: 270, y2: 150, double: false, couleur: '#0090FF',
        epaisseur: 1, opacite: 0.8 }];
    const passages = [{ largeur: 20, points: [[60, 60], [90, 110], [140, 100]] },
      { largeur: 10, points: [[240, 100]] }, { largeur: 16, points: [[150, 165], [160, 160]] }];
    const apercu = docAvec(300, 200, base), garde = docAvec(300, 200, base);
    for (const o of garde.objets) D.gommer(garde, o.id, passages);
    const enCours = new Map(apercu.objets.map((o) => [o.id, passages]));
    M.peindreMasques(im.ctx, im.base, apercu);
    const vue = document.createElement('canvas');
    vue.width = 600; vue.height = 400;
    const c = vue.getContext('2d', { willReadFrequently: true });
    const lire = (doc, e, options) => {
      M.peindreVue(c, doc, im.fond, { x: 0, y: 0, echelle: e, dpr: e }, [], options);
      return c.getImageData(0, 0, 600, 400).data;
    };
    const compter = (a, b) => {
      let n = 0;
      for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) n++;
      return n;
    };
    const res = {};
    for (const e of [1, 1.5, 2]) {
      res['x' + e] = compter(lire(apercu, e, { gommes: enCours }), lire(garde, e));
    }
    res.change = compter(lire(apercu, 1), lire(apercu, 1, { gommes: enCours }));
    res.cumul = garde.objets[1].gommes.length === 4;
    return res;
  });
  verifier(r.x1 === 0 && r['x1.5'] === 0 && r.x2 === 0 && r.change > 1000 && r.cumul,
    'G9 aperçu pendant le geste = résultat gardé au bit près (échelles 1, 1,5, 2), anciens '
    + 'passages et passages du geste cumulés', JSON.stringify(r));

  // (G10) effetGomme : « rien » à côté de l'encre ou dans un trou déjà fait (même largeur ou
  // plus petite), « efface » en travers, « vide » quand il ne reste rien de visible, même
  // pour une encre de 4 000 px (jugée réduite) ; un passage d'un point compte.
  r = await p.evaluate(() => {
    const doc = D.nouveauDocument(4000, 300);
    const trait = { type: 'ligne', x1: 100, y1: 100, x2: 300, y2: 100, couleur: '#E5484D',
      epaisseur: 1, opacite: 1 };
    const troue = { ...trait, gommes: [{ largeur: 30, points: [[200, 80], [200, 120]] }] };
    const long = { type: 'crayon', points: [[50, 150], [3950, 150]], couleur: '#1A1A1A',
      epaisseur: 0, opacite: 1 };
    const E = (o, ...passages) => M.effetGomme(doc, o, passages);
    return {
      cote: E(trait, { largeur: 10, points: [[200, 140], [260, 140]] }),
      travers: E(trait, { largeur: 10, points: [[200, 80], [200, 120]] }),
      point: E(trait, { largeur: 10, points: [[150, 100]] }),
      dansTrou: E(troue, { largeur: 30, points: [[200, 80], [200, 120]] }),
      dansTrouPetite: E(troue, { largeur: 10, points: [[200, 90], [200, 110]] }),
      bordTrou: E(troue, { largeur: 10, points: [[215, 80], [215, 120]] }),
      tout: E(trait, { largeur: 60, points: [[80, 100], [320, 100]] }),
      enDeux: E(trait, { largeur: 40, points: [[90, 100], [200, 100]] },
        { largeur: 40, points: [[200, 100], [310, 100]] }),
      long: E(long, { largeur: 40, points: [[30, 150], [3970, 150]] }),
      longMoitie: E(long, { largeur: 40, points: [[30, 150], [2000, 150]] }),
    };
  });
  verifier(r.cote === 'rien' && r.travers === 'efface' && r.point === 'efface'
    && r.dansTrou === 'rien' && r.dansTrouPetite === 'rien' && r.bordTrou === 'efface'
    && r.tout === 'vide' && r.enDeux === 'vide' && r.long === 'vide'
    && r.longMoitie === 'efface', 'G10 effet d\'un geste : rien à côté ni dans un trou déjà '
    + 'fait, efface en travers (un point compte), vide quand il ne reste rien (encre de '
    + '4 000 px comprise)', JSON.stringify(r));

  // (G11) La vue garde le rendu des encres gommées qui n'ont pas changé : ce qu'elle repose
  // est, au bit près, ce qu'elle aurait redessiné ; un objet déplacé, gommé de nouveau,
  // recoloré, une vue décalée ou zoomée ne reposent jamais un rendu périmé.
  r = await p.evaluate(async () => {
    const im = await preparer(300, 200);
    const doc = docAvec(300, 200, [
      { type: 'crayon', points: [[20, 30], [120, 160], [220, 40], [280, 170]],
        couleur: '#E5484D', epaisseur: 2, opacite: 1,
        gommes: [{ largeur: 20, points: [[70, 95], [80, 100]] }] },
      { type: 'fleche', x1: 30, y1: 180, x2: 270, y2: 140, double: false, couleur: '#0090FF',
        epaisseur: 1, opacite: 0.8, gommes: [{ largeur: 16, points: [[150, 160]] }] }]);
    M.peindreMasques(im.ctx, im.base, doc);
    const vue = document.createElement('canvas');
    vue.width = 700; vue.height = 500;
    const c = vue.getContext('2d', { willReadFrequently: true });
    let v = { x: 20, y: 30, echelle: 1, dpr: 1 };
    const lire = () => {
      M.peindreVue(c, doc, im.fond, v, []);
      return c.getImageData(0, 0, 700, 500).data;
    };
    const frais = () => {
      M.relacherCalque();
      return lire();
    };
    const pareil = (a, b) => a.every((x, i) => x === b[i]);
    const res = {};
    const etape = (nom) => {
      const garde = lire(); // la première peinture garde, la seconde repose
      res[nom] = pareil(lire(), frais()) && pareil(garde, frais());
    };
    etape('repos');
    const [trait, fleche] = doc.objets;
    D.deplacer(trait, JSON.parse(JSON.stringify(trait)), 12, -7);
    etape('deplace');
    D.gommer(doc, fleche.id, [{ largeur: 30, points: [[100, 160], [110, 150]] }]);
    etape('regomme');
    fleche.couleur = '#30A46C';
    etape('recolore');
    v = { x: 27, y: 18, echelle: 1, dpr: 1 };
    etape('decale');
    v = { x: 0, y: 0, echelle: 2, dpr: 2 };
    etape('zoom');
    // ce que la gomme a changé se voit bien : la vue gardée n'est pas celle d'avant le geste
    v = { x: 20, y: 30, echelle: 1, dpr: 1 };
    const avant = lire();
    D.gommer(doc, trait.id, [{ largeur: 40, points: [[200, 50], [240, 80]] }]);
    res.change = !pareil(avant, lire());
    M.relacherCalque();
    return res;
  });
  verifier(Object.values(r).every(Boolean), 'G11 rendu gardé par la vue = rendu redessiné au '
    + 'bit près ; jamais périmé (objet déplacé, gommé de nouveau, recoloré, vue décalée, '
    + 'zoomée)', JSON.stringify(r));

  // (G12) Coût : vue 3840 × 2160, 50 traits de crayon dont 20 gommés (20 passages chacun),
  // 10 peintures attendues, comparées aux mêmes traits sans gomme. Rendus gardés : pas plus
  // lent que sans gomme (borne 40 ms de plus) ; tout redessiné à chaque peinture (vue qui
  // bouge, zoom) : borne large de 600 ms de plus, un garde-fou contre une gomme devenue lente
  // (sans carte graphique, flouter l'ombre de chaque calque coûte environ 5 ms). Les 50 traits
  // gommés dépassent ce que la vue garde : elle en garde une part, toujours la même.
  r = await p.evaluate(async () => {
    const L = 3840, H = 2160, im = await preparer(L, H);
    const hasard = graine(42);
    const traits = [], gommes = [];
    for (let i = 0; i < 50; i++) {
      const x = hasard() * (L - 900), y = hasard() * (H - 500), pts = [];
      for (let j = 0; j < 200; j++) pts.push([x + j * 4, y + 200 + 180 * Math.sin(j / 15)]);
      traits.push({ type: 'crayon', points: pts, couleur: '#E5484D', epaisseur: 2, opacite: 1 });
      const g = [];
      for (let k = 0; k < 20; k++) {
        const a = pts[k * 10], q = [];
        for (let m = 0; m < 10; m++) q.push([a[0] + m * 3, a[1] + (m % 2 ? 6 : -6)]);
        g.push({ largeur: 20, points: q });
      }
      gommes.push(g);
    }
    const vue = document.createElement('canvas');
    vue.width = L; vue.height = H;
    const c = vue.getContext('2d');
    const mesurer = (doc, froid) => {
      M.peindreMasques(im.ctx, im.base, doc);
      M.peindreVue(c, doc, im.fond, { x: 0, y: 0, echelle: 1, dpr: 1 }, []);
      c.getImageData(0, 0, 1, 1);
      const t0 = performance.now();
      for (let i = 0; i < 10; i++) {
        if (froid) M.relacherCalque();
        M.peindreVue(c, doc, im.fond, { x: 0, y: 0, echelle: 1, dpr: 1 }, []);
        c.getImageData(0, 0, 1, 1); // attend la fin du dessin
      }
      return Math.round((performance.now() - t0) / 10 * 10) / 10;
    };
    const vingt = docAvec(L, H, traits.map((o, i) => (i < 20 ? { ...o, gommes: gommes[i] } : o)));
    const res = {
      sans: mesurer(docAvec(L, H, traits)),
      gardes: mesurer(vingt),
      redessines: mesurer(vingt, true),
      cinquante: mesurer(docAvec(L, H, traits.map((o, i) => ({ ...o, gommes: gommes[i] })))),
    };
    M.relacherCalque();
    return res;
  });
  verifier(r.gardes - r.sans <= 40 && r.redessines - r.sans <= 600,
    `G12 vue 3840 × 2160, 20 traits gommés sur 50 : ${r.gardes} ms par peinture (rendus `
    + `gardés), ${r.redessines} ms (tout redessiné), sans gomme ${r.sans} ms ; 50 traits `
    + `gommés : ${r.cinquante} ms`, JSON.stringify(r));

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
