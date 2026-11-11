const { chromium } = require('playwright');
const http = require('http'), fs = require('fs'), path = require('path');
// Essai de la page de sélection dans Chromium (Playwright). Sort avec 1 si une vérification
// échoue. Captures dans $SORTIE (par défaut essais-resultats/pages).
//   node essais/pages/selection.cjs
const RACINE = path.join(__dirname, '..', '..', 'app', 'interface');
const SORTIE = process.env.SORTIE || path.join(__dirname, '..', '..', 'essais-resultats', 'pages');
fs.mkdirSync(SORTIE, { recursive: true });
// La CSP de l'app (tauri.conf.json), servie avec la page pour vérifier qu'elle la respecte.
const CSP = Object.entries(JSON.parse(fs.readFileSync(path.join(__dirname, '..', '..', 'app', 'coeur', 'tauri.conf.json'), 'utf8')).app.security.csp)
  .map(([k, v]) => `${k} ${v}`).join('; ');
const types = { '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript' };
const serveur = http.createServer((q, r) => {
  const p = path.join(RACINE, decodeURIComponent(new URL(q.url, 'http://x').pathname));
  if (!p.startsWith(RACINE) || !fs.existsSync(p)) { r.writeHead(404); return r.end(); }
  const entetes = { 'content-type': types[path.extname(p)] || 'text/plain' };
  if (q.url.includes('csp=1')) entetes['content-security-policy'] = CSP;
  r.writeHead(200, entetes); fs.createReadStream(p).pipe(r);
});
// Fond uni posé sous la page transparente (captures et lecture des pixels).
const FOND = [58, 123, 213];
const VOILE = FOND.map(c => Math.round(c * 0.55)); // --scrim : noir à 45 %
const L = 1280, H = 400;
(async () => {
  await new Promise(ok => serveur.listen(0, ok));
  const base = `http://127.0.0.1:${serveur.address().port}/selection/selection.html`;
  const nav = await chromium.launch();
  const erreurs = [];
  let echecs = 0;
  const verifier = (ok, quoi, info = '') => { console.log(ok ? 'OK   ' : 'ECHEC', quoi, info); if (!ok) echecs++; };

  async function page(o = {}) {
    const ctx = await nav.newContext({ viewport: { width: L, height: H }, deviceScaleFactor: o.echelle || 1, colorScheme: o.theme || 'light' });
    const p = await ctx.newPage();
    p.on('pageerror', e => erreurs.push(String(e)));
    p.on('console', m => { if (m.type() === 'error') erreurs.push(m.text()); });
    await p.addInitScript(() => {
      window.__appels = []; const ecouteurs = {}; window.__rejeter = new Set();
      window.__TAURI__ = { core: { invoke: (c, a) => { window.__appels.push([c, JSON.parse(JSON.stringify(a))]); return window.__rejeter.has(c) ? Promise.reject(new Error('refus voulu')) : Promise.resolve(); } },
        event: { listen: (n, f) => { ecouteurs[n] = f; return Promise.resolve(() => {}); } } };
      window.__emettre = (nom, payload) => ecouteurs[nom]({ payload });
      window.__pointeur = null;
      addEventListener('pointerdown', e => { window.__pointeur = e.pointerId; }, true);
    });
    if (o.csp) await p.addInitScript(() => { window.__refus = []; document.addEventListener('securitypolicyviolation', e => window.__refus.push(e.violatedDirective + ' ' + e.blockedURI)); });
    await p.goto(base + (o.csp ? '?csp=1' : ''));
    if (!o.csp) await p.addStyleTag({ content: `html{background:rgb(${FOND.join(',')}) !important}` });
    await p.waitForTimeout(50);
    p.echelle = o.echelle || 1;
    return p;
  }
  const appels = (p) => p.evaluate(() => __appels.filter(x => x[0] !== 'selection_chargee'));
  const vider = (p) => p.evaluate(() => { __appels.length = 0; });
  let prochain = 1;
  async function montrer(p, ecrans = [{ x: 0, y: 0, largeur: L, hauteur: H }], coteMin = 3) {
    const id = prochain++;
    await p.evaluate(([id, ecrans, c, e]) => __emettre('selection:montrer', { id, ecrans, cote_min: c, largeur: innerWidth, hauteur: innerHeight, echelle: e }), [id, ecrans, coteMin, p.echelle]);
    await p.waitForTimeout(80);
    await vider(p);
    return id;
  }
  async function glisser(p, a, b, o = {}) {
    await p.mouse.move(...a); await p.mouse.down();
    await p.mouse.move(...b, { steps: 6 });
    if (o.avant) await o.avant();
    await p.mouse.up(); await p.waitForTimeout(40);
  }
  const termine = (p) => appels(p).then(a => a.filter(x => x[0] === 'selection_terminee'));
  const etatZone = (p) => p.evaluate(() => {
    const z = document.getElementById('zone'), s = getComputedStyle(z);
    return { glisse: document.body.classList.contains('glisse'), affiche: s.display, x: z.style.left, y: z.style.top, l: z.style.width, h: z.style.height, trait: getComputedStyle(document.documentElement).getPropertyValue('--trait').trim() || '1px' };
  });
  // Lit une ligne de pixels (réels) d'une capture.
  async function ligne(p, y) {
    const b64 = (await p.screenshot()).toString('base64');
    return p.evaluate(async ([b64, y]) => {
      const bm = await createImageBitmap(await (await fetch('data:image/png;base64,' + b64)).blob());
      const c = new OffscreenCanvas(bm.width, bm.height), x = c.getContext('2d');
      x.drawImage(bm, 0, 0);
      const d = x.getImageData(0, y, bm.width, 1).data, px = [];
      for (let i = 0; i < d.length; i += 4) px.push([d[i], d[i + 1], d[i + 2]]);
      return px;
    }, [b64, y]);
  }
  const proche = (a, b, t = 3) => a.every((v, i) => Math.abs(v - b[i]) <= t);

  // 1. Au repos : chargee une fois, voile partout, aucun rectangle, aucun texte.
  let p = await page();
  verifier(JSON.stringify(await p.evaluate(() => __appels)) === JSON.stringify([['selection_chargee', {}]]), '1 selection_chargee envoyé une fois, écoutes en place');
  let id = await montrer(p);
  let z = await etatZone(p);
  const texte = await p.evaluate(() => document.body.innerText.trim() + '|' + document.body.textContent.trim());
  let px = await ligne(p, 200);
  verifier(!z.glisse && z.affiche === 'none' && texte === '|' && px.every(c => proche(c, VOILE)), '1 repos : voile partout, aucun rectangle, aucun texte', JSON.stringify({ z, texte, premier: px[0] }));
  await p.screenshot({ path: `${SORTIE}/selection-repos.png` });
  verifier(await p.evaluate(() => getComputedStyle(document.documentElement).cursor) === 'crosshair', '1 curseur en croix');

  // 2. Quatre sens : coins exacts.
  const SENS = [[[100, 100], [300, 250]], [[300, 250], [100, 100]], [[300, 100], [100, 250]], [[100, 250], [300, 100]]];
  for (const [a, b] of SENS) {
    id = await montrer(p);
    await glisser(p, a, b);
    const t = await termine(p);
    verifier(t.length === 1 && JSON.stringify(t[0][1]) === JSON.stringify({ id, depart: { x: a[0], y: a[1] }, arrivee: { x: b[0], y: b[1] } }), `2 glisser ${a} → ${b}`, JSON.stringify(t));
  }
  // Rectangle dessiné pendant le glisser : voile autour, clair dedans, trait blanc.
  id = await montrer(p);
  await p.mouse.move(400, 100); await p.mouse.down(); await p.mouse.move(700, 300, { steps: 4 }); await p.waitForTimeout(60);
  px = await ligne(p, 200);
  verifier(proche(px[399], VOILE) && proche(px[400], [255, 255, 255], 0) && proche(px[401], FOND) && proche(px[698], FOND) && proche(px[699], [255, 255, 255], 0) && proche(px[700], VOILE),
    '2 dessin : voile autour, zone claire, trait de 1 px à l\'intérieur', JSON.stringify([px[399], px[400], px[401], px[698], px[699], px[700]]));
  await p.screenshot({ path: `${SORTIE}/selection-glisser-100.png` });
  await p.mouse.up(); await p.context().close();

  // 3. Maj : carré ; Espace : déplacement ; bornage à l'écran du départ.
  p = await page();
  id = await montrer(p);
  await p.keyboard.down('Shift'); await glisser(p, [100, 100], [300, 180]); await p.keyboard.up('Shift');
  let t = await termine(p);
  verifier(t.length === 1 && JSON.stringify(t[0][1].arrivee) === JSON.stringify({ x: 180, y: 180 }), '3 Maj : carré (le plus petit côté)', JSON.stringify(t));
  id = await montrer(p);
  await p.keyboard.down('Shift'); await glisser(p, [300, 300], [100, 250]); await p.keyboard.up('Shift');
  t = await termine(p);
  verifier(t.length === 1 && JSON.stringify(t[0][1].arrivee) === JSON.stringify({ x: 250, y: 250 }), '3 Maj : carré vers le haut à gauche', JSON.stringify(t));
  id = await montrer(p);
  await p.keyboard.down('Shift'); await glisser(p, [1250, 100], [1279.5, 300]); await p.keyboard.up('Shift');
  t = await termine(p);
  verifier(t.length === 1 && t[0][1].arrivee.x - 1250 === t[0][1].arrivee.y - 100 && t[0][1].arrivee.x <= L, '3 Maj au bord : carré qui reste dans l\'écran', JSON.stringify(t));
  id = await montrer(p);
  await p.mouse.move(100, 100); await p.mouse.down(); await p.mouse.move(200, 200, { steps: 3 });
  await p.keyboard.down('Space'); await p.mouse.move(250, 230, { steps: 3 }); await p.keyboard.up('Space');
  await p.mouse.up(); await p.waitForTimeout(40);
  t = await termine(p);
  verifier(t.length === 1 && JSON.stringify(t[0][1]) === JSON.stringify({ id, depart: { x: 150, y: 130 }, arrivee: { x: 250, y: 230 } }), '3 Espace : la zone se déplace sans changer de taille', JSON.stringify(t));
  // Deux écrans côte à côte (le premier est l'écran de gauche, d'origine négative dans le bureau).
  const DEUX = [{ x: 0, y: 0, largeur: 640, hauteur: H }, { x: 640, y: 0, largeur: 640, hauteur: H }];
  id = await montrer(p, DEUX);
  await glisser(p, [100, 100], [900, 300]);
  t = await termine(p);
  verifier(t.length === 1 && JSON.stringify(t[0][1].arrivee) === JSON.stringify({ x: 640, y: 300 }), '3 bornage à l\'écran du départ (gauche → droite)', JSON.stringify(t));
  id = await montrer(p, DEUX);
  await glisser(p, [900, 100], [100, 300]);
  t = await termine(p);
  verifier(t.length === 1 && JSON.stringify(t[0][1].arrivee) === JSON.stringify({ x: 640, y: 300 }), '3 bornage à l\'écran du départ (droite → gauche)', JSON.stringify(t));
  id = await montrer(p, DEUX);
  await p.mouse.move(500, 100); await p.mouse.down(); await p.mouse.move(600, 200, { steps: 3 });
  await p.keyboard.down('Space'); await p.mouse.move(800, 200, { steps: 4 });
  await p.mouse.up(); await p.keyboard.up('Space'); await p.waitForTimeout(40);
  t = await termine(p);
  verifier(t.length === 1 && JSON.stringify(t[0][1]) === JSON.stringify({ id, depart: { x: 540, y: 100 }, arrivee: { x: 640, y: 200 } }), '3 Espace : déplacement borné à l\'écran', JSON.stringify(t));
  // Écrans de hauteurs différentes : écran de gauche plus court.
  const INEGAUX = [{ x: 0, y: 100, largeur: 500, hauteur: 300 }, { x: 500, y: 0, largeur: 780, hauteur: H }];
  id = await montrer(p, INEGAUX);
  await p.screenshot({ path: `${SORTIE}/selection-deux-ecrans-repos.png` });
  await glisser(p, [100, 200], [50, 20]);
  t = await termine(p);
  verifier(t.length === 1 && JSON.stringify(t[0][1].arrivee) === JSON.stringify({ x: 50, y: 100 }), '3 bornage au haut de l\'écran de gauche', JSON.stringify(t));
  id = await montrer(p, INEGAUX);
  await p.mouse.move(900, 200); await p.mouse.down(); await p.mouse.move(100, 390, { steps: 5 }); await p.waitForTimeout(60);
  await p.screenshot({ path: `${SORTIE}/selection-deux-ecrans-borne.png` });
  await p.mouse.up(); await p.waitForTimeout(40);
  t = await termine(p);
  verifier(t.length === 1 && JSON.stringify(t[0][1].arrivee) === JSON.stringify({ x: 500, y: 390 }), '3 bornage à l\'écran de droite', JSON.stringify(t));
  await p.context().close();

  // 4. Sous le seuil : rien d'envoyé, rectangle effacé, la sélection continue.
  p = await page();
  id = await montrer(p);
  await p.mouse.move(100, 100); await p.mouse.down(); await p.mouse.move(150, 150, { steps: 2 }); await p.waitForTimeout(40);
  const pendant = await etatZone(p);
  await p.mouse.move(102, 101); await p.mouse.up(); await p.waitForTimeout(40);
  z = await etatZone(p);
  await p.mouse.click(300, 300); await p.waitForTimeout(40);
  verifier(pendant.glisse && !z.glisse && (await appels(p)).length === 0, '4 glisser de 2 px et simple clic : rien envoyé, rectangle effacé', JSON.stringify([pendant.glisse, z]));
  await glisser(p, [100, 100], [103, 103]);
  t = await termine(p);
  verifier(t.length === 1, '4 la sélection continue : 3 px de côté font une zone', JSON.stringify(t));
  await p.context().close();

  // 5. Échap et clic droit : une seule annulation.
  p = await page();
  id = await montrer(p);
  await p.keyboard.press('Escape'); await p.keyboard.press('Escape'); await p.mouse.click(10, 10, { button: 'right' }); await glisser(p, [100, 100], [300, 300]);
  let a = await appels(p);
  verifier(JSON.stringify(a) === JSON.stringify([['selection_annulee', { id, cause: 'echap' }]]), '5 Échap : une seule annulation, plus rien ensuite', JSON.stringify(a));
  id = await montrer(p);
  await p.mouse.click(10, 10, { button: 'right' }); await p.mouse.click(10, 10, { button: 'right' }); await p.keyboard.press('Escape');
  a = await appels(p);
  verifier(JSON.stringify(a) === JSON.stringify([['selection_annulee', { id, cause: 'clic_droit' }]]), '5 clic droit : une seule annulation', JSON.stringify(a));
  id = await montrer(p);
  await p.mouse.move(100, 100); await p.mouse.down(); await p.mouse.move(200, 200, { steps: 2 });
  await p.mouse.down({ button: 'right' }); await p.mouse.up({ button: 'right' }); await p.mouse.up(); await p.waitForTimeout(40);
  a = await appels(p);
  verifier(JSON.stringify(a) === JSON.stringify([['selection_annulee', { id, cause: 'clic_droit' }]]), '5 clic droit pendant un glisser : annulation, pas de zone', JSON.stringify(a));
  const menu = await p.evaluate(() => { const e = new MouseEvent('contextmenu', { bubbles: true, cancelable: true }); document.body.dispatchEvent(e); return e.defaultPrevented; });
  verifier(menu, '5 menu contextuel neutralisé');

  // 6. selection_prete : deux images après montrer, avec le bon id, une fois.
  const pr = await p.evaluate(async () => {
    __appels.length = 0; __emettre('selection:montrer', { id: 77, ecrans: [], cote_min: 3, largeur: 1280, hauteur: 400, echelle: 1 });
    const tout_de_suite = __appels.length;
    await new Promise(r => requestAnimationFrame(() => r()));
    const une_image = __appels.length;
    await new Promise(r => setTimeout(r, 150));
    return { tout_de_suite, une_image, appels: __appels };
  });
  verifier(pr.tout_de_suite === 0 && pr.une_image === 0 && JSON.stringify(pr.appels) === JSON.stringify([['selection_prete', { id: 77, largeur: 1280, hauteur: 400, echelle: 1 }]]), '6 selection_prete { id, largeur, hauteur, echelle } deux images après selection:montrer', JSON.stringify(pr));
  // Demandes invalides : ignorées, pas de selection_prete.
  await vider(p);
  for (const m of [null, {}, { id: -1, ecrans: [], cote_min: 3 }, { id: 1.5, ecrans: [], cote_min: 3 }, { id: 2, ecrans: [{ x: 0, y: 0, largeur: NaN, hauteur: 1 }], cote_min: 3 }, { id: 3, ecrans: 'x', cote_min: 3 }, { id: 4, ecrans: [], cote_min: -1 }]) {
    await p.evaluate(m => __emettre('selection:montrer', m), m);
  }
  await p.waitForTimeout(100);
  verifier((await appels(p)).length === 0, '6 demandes invalides ignorées');
  await p.context().close();

  // 7. selection:cachee : retour au repos ; relâchement : rien n'est redessiné.
  p = await page();
  id = await montrer(p);
  await p.mouse.move(100, 100); await p.mouse.down(); await p.mouse.move(300, 300, { steps: 3 }); await p.waitForTimeout(40);
  const avantCachee = await etatZone(p);
  await p.evaluate(() => __emettre('selection:cachee', null)); await p.waitForTimeout(40);
  z = await etatZone(p);
  await p.mouse.move(350, 350); await p.mouse.up(); await p.keyboard.press('Escape'); await p.waitForTimeout(40);
  verifier(avantCachee.glisse && !z.glisse && (await appels(p)).length === 0, '7 selection:cachee : repos, plus rien envoyé', JSON.stringify([avantCachee.glisse, z.glisse]));
  id = await montrer(p);
  await p.mouse.move(100, 100); await p.mouse.down(); await p.mouse.move(200, 200, { steps: 3 }); await p.waitForTimeout(60);
  const dessine = await etatZone(p);
  // Déplacement puis relâchement dans la même tâche : le dernier point part, rien n'est redessiné.
  await p.evaluate(() => {
    const o = { bubbles: true, cancelable: true, pointerId: __pointeur, pointerType: 'mouse', isPrimary: true };
    document.documentElement.dispatchEvent(new PointerEvent('pointermove', { ...o, clientX: 320, clientY: 330, button: -1, buttons: 1 }));
    document.documentElement.dispatchEvent(new PointerEvent('pointerup', { ...o, clientX: 320, clientY: 330, button: 0, buttons: 0 }));
  });
  await p.waitForTimeout(120);
  const apres = await etatZone(p);
  t = await termine(p);
  verifier(JSON.stringify(dessine) === JSON.stringify(apres) && apres.glisse && t.length === 1 && JSON.stringify(t[0][1].arrivee) === JSON.stringify({ x: 320, y: 330 }),
    '7 relâchement : coins envoyés, rien redessiné', JSON.stringify({ dessine, apres, t }));
  await p.mouse.up();
  await p.evaluate(() => __emettre('selection:cachee', null)); await p.waitForTimeout(40);
  z = await etatZone(p);
  verifier(!z.glisse, '7 puis selection:cachee : repos');
  await p.context().close();

  // 8. Échelles : trait net, coins justes au pixel réel près.
  for (const e of [1, 1.25, 1.5]) {
    p = await page({ echelle: e });
    // Écrans réels (1600 × 500 à 125 % par exemple) convertis par le cœur en pixels CSS.
    id = await montrer(p, [{ x: 0, y: 0, largeur: L, hauteur: H }], 3 / e);
    const A = [100.3, 100.7], B = [301.1, 250.2];
    await p.mouse.move(...A); await p.mouse.down(); await p.mouse.move(...B, { steps: 4 }); await p.waitForTimeout(60);
    z = await etatZone(p);
    const trait = parseFloat(z.trait);
    const x0 = Math.round(parseFloat(z.x)), x1 = x0 + Math.round(parseFloat(z.l)), ym = Math.round(200 * e);
    px = await ligne(p, ym);
    const blanc = (i) => proche(px[i], [255, 255, 255], 0);
    const netOk = Math.abs(trait - Math.round(trait)) < 1e-6 && trait >= 1 && blanc(x0) && !blanc(x0 - 1) && proche(px[x0 - 1], VOILE)
      && blanc(x1 - 1) && proche(px[x1], VOILE) && proche(px[x0 + Math.round(trait)], FOND);
    verifier(netOk, `8 à ${e * 100} % : trait net de ${trait} pixel(s) réel(s)`, JSON.stringify({ trait, x0, x1, bord: [px[x0 - 1], px[x0], px[x0 + 1], px[x1 - 1], px[x1]] }));
    await p.screenshot({ path: `${SORTIE}/selection-glisser-${e * 100}.png` });
    await p.mouse.up(); await p.waitForTimeout(40);
    t = await termine(p);
    const attendu = [...A, ...B].map(v => Math.round(v * e));
    const obtenu = t.length === 1 ? [t[0][1].depart.x, t[0][1].depart.y, t[0][1].arrivee.x, t[0][1].arrivee.y].map(v => v * e) : [];
    verifier(obtenu.length === 4 && obtenu.every((v, i) => Math.abs(v - attendu[i]) <= 1 && Math.abs(v - Math.round(v)) < 1e-6), `8 à ${e * 100} % : coins × échelle = pixels attendus`, JSON.stringify({ attendu, obtenu }));
    await p.context().close();
  }

  // 9. Thème sombre du système : même voile (la page force le thème clair).
  p = await page({ theme: 'dark' });
  id = await montrer(p);
  px = await ligne(p, 200);
  verifier(px.every(c => proche(c, VOILE)), '9 thème sombre : même voile');
  await p.context().close();

  // 11. Contre-essais de la relecture.
  // A. Espace jusqu'au bord de l'écran de gauche : le départ envoyé est le bord 640 ; une
  // fois rendu en pixels inclus (règle de decider_zone), il reste sur l'écran de gauche.
  p = await page();
  const DEUX_A = [{ x: 0, y: 0, largeur: 640, hauteur: H }, { x: 640, y: 0, largeur: 640, hauteur: H }];
  id = await montrer(p, DEUX_A);
  await p.mouse.move(600, 100); await p.mouse.down(); await p.mouse.move(500, 200, { steps: 3 });
  await p.keyboard.down('Space'); await p.mouse.move(800, 200, { steps: 4 }); await p.mouse.up(); await p.keyboard.up('Space'); await p.waitForTimeout(40);
  t = await termine(p);
  const inclus = (u, v) => (u < v ? [u, v - 1] : u > v ? [u - 1, v] : [u, v]);
  let zA = null;
  if (t.length === 1) {
    const { depart: d, arrivee: f } = t[0][1];
    const [ax, bx] = inclus(d.x, f.x), [ay, by] = inclus(d.y, f.y);
    zA = { depart: [ax, ay], l: Math.abs(ax - bx) + 1, h: Math.abs(ay - by) + 1 };
  }
  verifier(zA && zA.depart[0] < 640 && zA.l === 100 && zA.h === 100, 'A Espace au bord : départ découpé sur l\'écran de gauche, 100 × 100', JSON.stringify({ t, zA }));
  await p.context().close();

  // B. Envoi sans réponse : rien pendant 1,5 s, puis la page redevient utilisable.
  p = await page();
  id = await montrer(p);
  await glisser(p, [100, 100], [200, 200]);
  await vider(p);
  await p.keyboard.press('Escape'); await p.mouse.click(300, 300, { button: 'right' }); await p.waitForTimeout(200);
  const pendantAttente = (await appels(p)).length;
  await p.waitForTimeout(1500);
  z = await etatZone(p);
  await p.keyboard.press('Escape'); await p.waitForTimeout(40);
  a = await appels(p);
  verifier(pendantAttente === 0 && !z.glisse && JSON.stringify(a) === JSON.stringify([['selection_annulee', { id, cause: 'echap' }]]), 'B sans selection:cachee : attente, puis utilisable après 1,5 s', JSON.stringify({ pendantAttente, z: z.glisse, a }));
  await p.waitForTimeout(1600); await vider(p);
  await glisser(p, [100, 100], [300, 300]);
  t = await termine(p);
  verifier(t.length === 1, 'B après le filet, un glisser marche', JSON.stringify(t));
  // Envoi refusé par le cœur : utilisable tout de suite.
  id = await montrer(p);
  await p.evaluate(() => __rejeter.add('selection_terminee'));
  await glisser(p, [100, 100], [200, 200]); await p.waitForTimeout(40);
  await p.evaluate(() => __rejeter.clear()); await vider(p);
  await p.keyboard.press('Escape'); await p.waitForTimeout(40);
  a = await appels(p);
  verifier(JSON.stringify(a) === JSON.stringify([['selection_annulee', { id, cause: 'echap' }]]), 'B invoke rejeté : utilisable aussitôt', JSON.stringify(a));
  // Même demande renvoyée (zone refusée par le cœur) : repos, utilisable, prête renvoyé.
  id = await montrer(p);
  await glisser(p, [100, 100], [200, 200]);
  await vider(p);
  await p.evaluate(([id, e]) => __emettre('selection:montrer', { id, ecrans: [], cote_min: 3, largeur: innerWidth, hauteur: innerHeight, echelle: e }), [id, p.echelle]);
  await p.waitForTimeout(80);
  z = await etatZone(p);
  await glisser(p, [100, 100], [250, 250]);
  a = await appels(p);
  verifier(!z.glisse && a.length === 2 && a[0][0] === 'selection_prete' && a[1][0] === 'selection_terminee' && a[1][1].id === id, 'B demande renvoyée : repos puis nouveau glisser', JSON.stringify(a));
  await p.context().close();

  // C. Touches du navigateur neutralisées.
  p = await page();
  id = await montrer(p);
  const touches = await p.evaluate(() => [['F5', false], ['r', true], ['p', true], ['f', true], ['F12', false], ['Tab', false], ['a', false], ['Shift', false], [' ', false]].map(([k, ctrl]) => {
    const ev = new KeyboardEvent('keydown', { key: k, code: k === ' ' ? 'Space' : k, ctrlKey: ctrl, bubbles: true, cancelable: true });
    document.body.dispatchEvent(ev); return ev.defaultPrevented; }));
  verifier(touches.every(Boolean), 'C toute touche neutralisée (F5, Ctrl+R, Ctrl+P, Ctrl+F, F12…)', JSON.stringify(touches));
  // D. Bouton tenu avant montrer : pas de glisser fantôme.
  await p.evaluate(() => __emettre('selection:cachee', null));
  await p.mouse.move(100, 100); await p.mouse.down();
  id = await montrer(p);
  await p.mouse.move(300, 300, { steps: 3 }); await p.mouse.up(); await p.waitForTimeout(40);
  verifier((await appels(p)).length === 0, 'D bouton tenu avant montrer : rien envoyé');
  // Page pas encore à la bonne taille : ni prête ni glisser ; prête dès qu'elle l'est.
  await vider(p);
  await p.evaluate(() => __emettre('selection:montrer', { id: 500, ecrans: [], cote_min: 3, largeur: 1000, hauteur: 400, echelle: 1 }));
  await p.waitForTimeout(150);
  await glisser(p, [100, 100], [200, 200]);
  const avantTaille = await appels(p);
  await p.setViewportSize({ width: 1000, height: 400 }); await p.waitForTimeout(150);
  await glisser(p, [100, 100], [200, 200]);
  a = await appels(p);
  verifier(avantTaille.length === 0 && a.length === 2 && JSON.stringify(a[0]) === JSON.stringify(['selection_prete', { id: 500, largeur: 1000, hauteur: 400, echelle: 1 }]) && a[1][0] === 'selection_terminee',
    'taille différente : ni prête ni glisser ; prête une fois la taille juste', JSON.stringify({ avantTaille, a }));
  await p.context().close();

  // 10 (relecture). Espace relâchée avant la souris : pas de saut sans bornage.
  p = await page();
  id = await montrer(p);
  await p.mouse.move(100, 100); await p.mouse.down(); await p.mouse.move(200, 200, { steps: 3 });
  await p.keyboard.down('Space'); await p.mouse.move(250, 230, { steps: 3 }); await p.waitForTimeout(40);
  const sousEspace = await etatZone(p);
  await p.keyboard.up('Space'); await p.mouse.move(251, 231); await p.mouse.move(250, 230); await p.waitForTimeout(40);
  const apresEspace = await etatZone(p);
  await p.mouse.up(); await p.waitForTimeout(40);
  t = await termine(p);
  verifier(JSON.stringify(sousEspace) === JSON.stringify(apresEspace) && t.length === 1 && JSON.stringify(t[0][1]) === JSON.stringify({ id, depart: { x: 150, y: 130 }, arrivee: { x: 250, y: 230 } }),
    'Espace relâchée avant la souris : aucun saut', JSON.stringify({ sousEspace, apresEspace, t }));
  await p.context().close();

  // 10. CSP de l'app : aucun refus, styles chargés, glisser complet.
  p = await page({ csp: true });
  id = await montrer(p);
  await glisser(p, [100, 100], [200, 200], { avant: async () => { await p.waitForTimeout(40); } });
  const refus = await p.evaluate(() => window.__refus);
  t = await termine(p);
  const style = await p.evaluate(() => [getComputedStyle(document.body).cursor, getComputedStyle(document.getElementById('plein')).backgroundColor, getComputedStyle(document.getElementById('zone')).boxShadow]);
  verifier(refus.length === 0 && t.length === 1 && style[0] === 'crosshair' && style[1] === 'rgba(0, 0, 0, 0.45)' && style[2].includes('inset') && style[2].startsWith('rgb(255, 255, 255)'), 'CSP de l\'app respectée (aucun refus, styles chargés)', JSON.stringify({ refus, style }));
  await p.context().close();

  console.log('ECHECS :', echecs);
  console.log('ERREURS PAGE :', JSON.stringify(erreurs));
  if (erreurs.length) echecs++;
  await nav.close(); serveur.close();
  process.exitCode = echecs === 0 ? 0 : 1;
})().catch((e) => { console.error('essai interrompu :', e); process.exit(1); });
