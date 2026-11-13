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
    const ctx = await nav.newContext({ viewport: { width: o.l || L, height: o.h || H }, deviceScaleFactor: o.echelle || 1, colorScheme: o.theme || 'light' });
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
  // La page a traité la charge `id` (elle l'a dit prête) : un geste fait avant serait
  // ignoré, et une attente fixe dépend de la machine. Sans réponse en 5 s, l'essai continue
  // et le point qui suit le dira.
  const attendrePrete = (p, id) => p.waitForFunction(
    id => __appels.some(a => a[0] === 'selection_prete' && a[1] && a[1].id === id), id, { timeout: 5000 }).catch(() => {});
  // La charge du cœur : tout en pixels réels relatifs à la fenêtre. Les écrans sont donnés
  // ici en pixels CSS et convertis à l'échelle k (par défaut, celle du navigateur) ;
  // cote_min est en pixels réels.
  // `numero` (numéro de charge) vaut l'id, sauf mention.
  const charge = (id, ecrans, coteMin, k, numero = id) => p => p.evaluate(([id, ecrans, c, k, numero]) => __emettre('selection:montrer', {
    id, numero, largeur: Math.round(innerWidth * k), hauteur: Math.round(innerHeight * k),
    ecrans: ecrans.map(e => ({ x: Math.round(e.x * k), y: Math.round(e.y * k), largeur: Math.round(e.largeur * k), hauteur: Math.round(e.hauteur * k) })), cote_min: c,
  }), [id, ecrans, coteMin, k, numero]);
  async function montrer(p, ecrans = [{ x: 0, y: 0, largeur: L, hauteur: H }], coteMin = 3, k = p.echelle) {
    const id = prochain++;
    await charge(id, ecrans, coteMin, k)(p);
    await attendrePrete(p, id);
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
    verifier(t.length === 1 && JSON.stringify(t[0][1]) === JSON.stringify({ id, numero: id, depart: { x: a[0], y: a[1] }, arrivee: { x: b[0], y: b[1] } }), `2 glisser ${a} → ${b}`, JSON.stringify(t));
  }
  // Rectangle dessiné pendant le glisser : voile autour, clair dedans, trait blanc.
  id = await montrer(p);
  await p.mouse.move(400, 100); await p.mouse.down(); await p.mouse.move(700, 300, { steps: 4 }); await p.waitForTimeout(60);
  px = await ligne(p, 200);
  verifier(proche(px[399], VOILE) && proche(px[400], [255, 255, 255], 0) && proche(px[401], FOND) && proche(px[699], FOND) && proche(px[700], [255, 255, 255], 0) && proche(px[701], VOILE),
    '2 dessin : voile autour, zone claire, trait de 1 px à l\'intérieur, pixels 400 et 700 inclus', JSON.stringify([px[399], px[400], px[401], px[699], px[700], px[701]]));
  await p.screenshot({ path: `${SORTIE}/selection-glisser-100.png` });
  await p.mouse.up();
  // Essai Windows : appui en (412, 40), relâché en (611, 239) → 200 × 200, pixels inclus.
  id = await montrer(p);
  await p.mouse.move(412, 40); await p.mouse.down(); await p.mouse.move(611, 239, { steps: 4 }); await p.waitForTimeout(60);
  px = await ligne(p, 100);
  const blanc2 = (i) => proche(px[i], [255, 255, 255], 0);
  const dessin412 = proche(px[411], VOILE) && blanc2(412) && blanc2(611) && proche(px[612], VOILE);
  await p.mouse.up(); await p.waitForTimeout(40);
  let t412 = await termine(p);
  verifier(dessin412 && t412.length === 1 && JSON.stringify(t412[0][1]) === JSON.stringify({ id, numero: id, depart: { x: 412, y: 40 }, arrivee: { x: 611, y: 239 } }),
    '2 glisser de (412, 40) à (611, 239) : pixels 412 et 611 inclus, 200 × 200', JSON.stringify({ t412, bord: [px[411], px[412], px[611], px[612]] }));
  // Jusqu'au dernier pixel de la fenêtre (1279, 399) et retour : dernière colonne et dernière ligne comprises.
  for (const [a, b] of [[[1000, 300], [1279, 399]], [[1279, 399], [1000, 300]], [[1279.9, 399.9], [0, 0]]]) {
    id = await montrer(p);
    await glisser(p, a, b);
    t412 = await termine(p);
    const f = t412.length === 1 ? t412[0][1] : null;
    verifier(f && Math.max(f.depart.x, f.arrivee.x) === L - 1 && Math.max(f.depart.y, f.arrivee.y) === H - 1, `2 glisser ${a} → ${b} : dernier pixel (${L - 1}, ${H - 1}) compris`, JSON.stringify(t412));
  }
  await p.context().close();

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
  verifier(t.length === 1 && JSON.stringify(t[0][1]) === JSON.stringify({ id, numero: id, depart: { x: 150, y: 130 }, arrivee: { x: 250, y: 230 } }), '3 Espace : la zone se déplace sans changer de taille', JSON.stringify(t));
  // Deux écrans côte à côte (le premier est l'écran de gauche, d'origine négative dans le bureau).
  const DEUX = [{ x: 0, y: 0, largeur: 640, hauteur: H }, { x: 640, y: 0, largeur: 640, hauteur: H }];
  id = await montrer(p, DEUX);
  await glisser(p, [100, 100], [900, 300]);
  t = await termine(p);
  verifier(t.length === 1 && JSON.stringify(t[0][1].arrivee) === JSON.stringify({ x: 639, y: 300 }), '3 bornage à l\'écran du départ (gauche → droite) : dernier pixel 639', JSON.stringify(t));
  id = await montrer(p, DEUX);
  await glisser(p, [900, 100], [100, 300]);
  t = await termine(p);
  verifier(t.length === 1 && JSON.stringify(t[0][1].arrivee) === JSON.stringify({ x: 640, y: 300 }), '3 bornage à l\'écran du départ (droite → gauche)', JSON.stringify(t));
  id = await montrer(p, DEUX);
  await p.mouse.move(500, 100); await p.mouse.down(); await p.mouse.move(600, 200, { steps: 3 });
  await p.keyboard.down('Space'); await p.mouse.move(800, 200, { steps: 4 });
  await p.mouse.up(); await p.keyboard.up('Space'); await p.waitForTimeout(40);
  t = await termine(p);
  verifier(t.length === 1 && JSON.stringify(t[0][1]) === JSON.stringify({ id, numero: id, depart: { x: 539, y: 100 }, arrivee: { x: 639, y: 200 } }), '3 Espace : déplacement borné à l\'écran', JSON.stringify(t));
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
  verifier(t.length === 1, '4 la sélection continue : 4 px de côté font une zone', JSON.stringify(t));
  await p.context().close();

  // 5. Échap et clic droit : une seule annulation.
  p = await page();
  id = await montrer(p);
  await p.keyboard.press('Escape'); await p.keyboard.press('Escape'); await p.mouse.click(10, 10, { button: 'right' }); await glisser(p, [100, 100], [300, 300]);
  let a = await appels(p);
  verifier(JSON.stringify(a) === JSON.stringify([['selection_annulee', { id, numero: id, cause: 'echap' }]]), '5 Échap : une seule annulation, plus rien ensuite', JSON.stringify(a));
  id = await montrer(p);
  await p.mouse.click(10, 10, { button: 'right' }); await p.mouse.click(10, 10, { button: 'right' }); await p.keyboard.press('Escape');
  a = await appels(p);
  verifier(JSON.stringify(a) === JSON.stringify([['selection_annulee', { id, numero: id, cause: 'clic_droit' }]]), '5 clic droit : une seule annulation', JSON.stringify(a));
  id = await montrer(p);
  await p.mouse.move(100, 100); await p.mouse.down(); await p.mouse.move(200, 200, { steps: 2 });
  await p.mouse.down({ button: 'right' }); await p.mouse.up({ button: 'right' }); await p.mouse.up(); await p.waitForTimeout(40);
  a = await appels(p);
  verifier(JSON.stringify(a) === JSON.stringify([['selection_annulee', { id, numero: id, cause: 'clic_droit' }]]), '5 clic droit pendant un glisser : annulation, pas de zone', JSON.stringify(a));
  const menu = await p.evaluate(() => { const e = new MouseEvent('contextmenu', { bubbles: true, cancelable: true }); document.body.dispatchEvent(e); return e.defaultPrevented; });
  verifier(menu, '5 menu contextuel neutralisé');

  // 6. selection_prete : deux images après montrer, avec le bon id, une fois.
  const pr = await p.evaluate(async () => {
    __appels.length = 0; __emettre('selection:montrer', { id: 77, numero: 4, largeur: 1280, hauteur: 400, ecrans: [], cote_min: 3 });
    const tout_de_suite = __appels.length;
    await new Promise(r => requestAnimationFrame(() => r()));
    const une_image = __appels.length;
    await new Promise(r => setTimeout(r, 150));
    return { tout_de_suite, une_image, appels: __appels };
  });
  verifier(pr.tout_de_suite === 0 && pr.une_image === 0 && JSON.stringify(pr.appels) === JSON.stringify([['selection_prete', { id: 77, numero: 4, largeur: 1280, hauteur: 400, echelle: 1, kx: 1, ky: 1, attente: false }]]), '6 selection_prete { id, numero, largeur, hauteur, echelle, kx, ky } deux images après selection:montrer', JSON.stringify(pr));
  // Demandes invalides : ignorées, pas de selection_prete.
  await vider(p);
  const T = { numero: 1, largeur: 1280, hauteur: 400 };
  for (const m of [null, {}, { ...T, id: -1, ecrans: [], cote_min: 3 }, { ...T, id: 1.5, ecrans: [], cote_min: 3 }, { ...T, id: 2, ecrans: [{ x: 0, y: 0, largeur: NaN, hauteur: 1 }], cote_min: 3 },
    { ...T, id: 3, ecrans: 'x', cote_min: 3 }, { ...T, id: 4, ecrans: [], cote_min: -1 }, { id: 5, numero: 1, ecrans: [], cote_min: 3 }, { id: 6, numero: 1, largeur: 1280.5, hauteur: 400, ecrans: [], cote_min: 3 },
    { id: 7, numero: 1, largeur: 0, hauteur: 400, ecrans: [], cote_min: 3 }, { ...T, id: 8, ecrans: [{ x: 0.5, y: 0, largeur: 10, hauteur: 10 }], cote_min: 3 }, { ...T, id: 9, ecrans: [], cote_min: 2.5 }, { id: 10, largeur: 1280, hauteur: 400, ecrans: [], cote_min: 3 }, { ...T, id: 11, numero: -1, ecrans: [], cote_min: 3 }]) {
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

  // 8. Échelles mesurées k = 1, 1,25, 1,5, 2 et 1,1 : trait net, coins exacts en pixels réels.
  for (const e of [1, 1.25, 1.5, 2, 1.1]) {
    p = await page({ echelle: e });
    id = await montrer(p);
    const A = [100, 100], B = [300, 250];
    await p.mouse.move(...A); await p.mouse.down(); await p.mouse.move(...B, { steps: 4 }); await p.waitForTimeout(60);
    z = await etatZone(p);
    const trait = parseFloat(z.trait);
    const x0 = parseInt(z.x), x1 = x0 + parseInt(z.l), ym = Math.round(200 * e);
    px = await ligne(p, ym);
    const blanc = (i) => proche(px[i], [255, 255, 255], 0);
    const netOk = trait === Math.max(1, Math.round(e)) && blanc(x0) && proche(px[x0 - 1], VOILE)
      && blanc(x0 + trait - 1) && proche(px[x0 + trait], FOND) && blanc(x1 - 1) && proche(px[x1], VOILE);
    verifier(netOk, `8 k = ${e} : trait net de ${trait} pixel(s) réel(s)`, JSON.stringify({ trait, x0, x1, bord: [px[x0 - 1], px[x0], px[x0 + 1], px[x1 - 1], px[x1]] }));
    await p.screenshot({ path: `${SORTIE}/selection-glisser-${Math.round(e * 100)}.png` });
    await p.mouse.up(); await p.waitForTimeout(40);
    t = await termine(p);
    const attendu = [...A, ...B].map(v => Math.floor(v * e + 1e-3));
    const obtenu = t.length === 1 ? [t[0][1].depart.x, t[0][1].depart.y, t[0][1].arrivee.x, t[0][1].arrivee.y] : [];
    verifier(JSON.stringify(obtenu) === JSON.stringify(attendu), `8 k = ${e} : coins exacts en pixels réels`, JSON.stringify({ attendu, obtenu }));
    // Coordonnées fractionnaires : au pixel réel le plus proche, entiers.
    id = await montrer(p);
    const Af = [100.3, 100.7], Bf = [301.1, 250.2];
    await glisser(p, Af, Bf);
    t = await termine(p);
    const attenduF = [...Af, ...Bf].map(v => v * e);
    const obtenuF = t.length === 1 ? [t[0][1].depart.x, t[0][1].depart.y, t[0][1].arrivee.x, t[0][1].arrivee.y] : [];
    verifier(obtenuF.length === 4 && obtenuF.every((v, i) => Number.isInteger(v) && Math.abs(v - attenduF[i]) <= 1), `8 k = ${e} : coins fractionnaires : pixel sous le curseur`, JSON.stringify({ attenduF, obtenuF }));
    await p.context().close();
  }

  // 8 bis (contre-essai de la relecture, repris). À chaque échelle, la zone vue sur l'image
  // est exactement celle que le cœur découpe (decider_zone : les deux pixels envoyés,
  // inclus), sans pixel adouci (ni voile ni clair) ; écrans séparés au pixel réel 1003.
  const coupe = (t) => {
    const { depart: d, arrivee: f } = t;
    return { x: Math.min(d.x, f.x), y: Math.min(d.y, f.y), l: Math.abs(d.x - f.x) + 1, h: Math.abs(d.y - f.y) + 1, xs: [d.x, f.x] };
  };
  // Gestes au pixel près (k quelconque) : pointe sur le coin haut gauche d'un pixel réel p
  // (position p / k, ce que Windows donne) ou en son milieu ((p + 0,5) / k) : c'est p.
  // Jusqu'au dernier pixel de la fenêtre, et retour ; le résultat attendu est vérifié.
  const casPixels = (R, RH, e, fx) => [
    ['412-611', async p => { await p.mouse.move(412 / e, 40 / e); await p.mouse.down(); await p.mouse.move(611 / e, 239 / e, { steps: 4 }); },
      t => t.depart.x === 412 && t.depart.y === 40 && t.arrivee.x === 611 && t.arrivee.y === 239],
    ['412-611-milieux', async p => { await p.mouse.move(412.5 / e, 40.5 / e); await p.mouse.down(); await p.mouse.move(611.5 / e, 239.5 / e, { steps: 4 }); },
      t => t.depart.x === 412 && t.depart.y === 40 && t.arrivee.x === 611 && t.arrivee.y === 239],
    ['jusqu-au-dernier-pixel', async p => { await p.mouse.move(fx + 20.3, 30.7); await p.mouse.down(); await p.mouse.move((R - 1) / e, (RH - 1) / e, { steps: 4 }); },
      t => t.arrivee.x === R - 1 && t.arrivee.y === RH - 1],
    ['depuis-le-dernier-pixel', async p => { await p.mouse.move((R - 0.5) / e, (RH - 0.5) / e); await p.mouse.down(); await p.mouse.move(fx + 0.2, 0, { steps: 4 }); },
      t => t.depart.x === R - 1 && t.depart.y === RH - 1 && t.arrivee.y === 0],
  ];
  const vue = (p) => p.screenshot().then(png => p.evaluate(async ([b64, fond, voile]) => {
    const bm = await createImageBitmap(await (await fetch('data:image/png;base64,' + b64)).blob());
    const c = new OffscreenCanvas(bm.width, bm.height), x = c.getContext('2d'); x.drawImage(bm, 0, 0);
    const d = x.getImageData(0, 0, bm.width, bm.height).data;
    let x0 = 1e9, y0 = 1e9, x1 = -1, y1 = -1, autres = 0;
    for (let i = 0, n = 0; i < d.length; i += 4, n++) {
      const c = [d[i], d[i + 1], d[i + 2]];
      const v = c.every((u, k) => Math.abs(u - voile[k]) <= 3);
      const clair = c.every((u, k) => Math.abs(u - fond[k]) <= 3) || c.every(u => u === 255);
      if (!v && !clair) autres++;
      if (!v) { const px = n % bm.width, py = (n - px) / bm.width; x0 = Math.min(x0, px); y0 = Math.min(y0, py); x1 = Math.max(x1, px); y1 = Math.max(y1, py); }
    }
    return { x: x0, y: y0, l: x1 - x0 + 1, h: y1 - y0 + 1, autres };
  }, [png.toString('base64'), FOND, VOILE]));
  for (const e of [1, 1.25, 1.5, 2, 1.1]) {
    const R = Math.round(L * e), RH = Math.round(H * e);
    const ecr = [{ x: 0, y: 0, largeur: 1003, hauteur: RH }, { x: 1003, y: 0, largeur: R - 1003, hauteur: RH }];
    const cas = [
      ['simple', async p => { await p.mouse.move(100.3, 50.6); await p.mouse.down(); await p.mouse.move(301.7, 250.2, { steps: 4 }); }],
      ['maj', async p => { await p.keyboard.down('Shift'); await p.mouse.move(700.4, 300.3); await p.mouse.down(); await p.mouse.move(500.1, 41.9, { steps: 4 }); }],
      ['borne', async p => { await p.mouse.move(600.2, 100.6); await p.mouse.down(); await p.mouse.move(1200, 390, { steps: 4 }); }],
      ['espace', async p => { await p.mouse.move(700, 100); await p.mouse.down(); await p.mouse.move(600, 200, { steps: 3 }); await p.keyboard.down('Space'); await p.mouse.move(1000, 260, { steps: 5 }); }],
      ...casPixels(R, RH, e, 1003 / e),
    ];
    for (const [nom, geste, attendu] of cas) {
      p = await page({ echelle: e });
      id = prochain++;
      await p.evaluate(([id, R, RH, ecr]) => __emettre('selection:montrer', { id, numero: id, largeur: R, hauteur: RH, ecrans: ecr, cote_min: 3 }), [id, R, RH, ecr]);
      await attendrePrete(p, id);
      await p.waitForTimeout(100);
      await geste(p); await p.waitForTimeout(80);
      const vu = await vue(p);
      if (nom === 'borne') await p.screenshot({ path: `${SORTIE}/selection-borne-${Math.round(e * 100)}.png` });
      await p.mouse.up(); await p.keyboard.up('Shift'); await p.keyboard.up('Space'); await p.waitForTimeout(50);
      t = await termine(p);
      const c = t.length === 1 ? coupe(t[0][1]) : null;
      const ecranDepart = c && t[0][1].depart.x < 1003 ? 0 : 1;
      const surEcran = c && c.xs.every(x => (ecranDepart === 0 ? x < 1003 : x >= 1003));
      verifier(c && vu.x === c.x && vu.y === c.y && vu.l === c.l && vu.h === c.h && vu.autres === 0 && surEcran && (!attendu || attendu(t[0][1])),
        `8 bis k = ${e} ${nom} : zone vue = zone découpée, aucun pixel adouci, sur l'écran du départ`, JSON.stringify({ vu, c, t: t[0] && t[0][1] }));
      await p.context().close();
    }
  }

  // 8 ter. Tailles CSS non entières (innerWidth arrondi) : 1366 × 501 à 125 % (viewport
  // 1093 × 401), 2560 × 1440 à 150 % (viewport 1707 × 960, 1706,67 px CSS), 1,3333 et
  // 1,7499. La taille réelle est celle de la capture. Zone vue = zone découpée au pixel près,
  // zéro pixel intermédiaire.
  for (const [e, l, h] of [[1.25, 1093, 401], [1.25, 1092, 400], [1.5, 1707, 960], [1.5, 1093, 401], [1.3333, 1093, 401], [1.7499, 1093, 401]]) {
    const p0 = await page({ echelle: e, l, h });
    const dims = await p0.evaluate(async (b64) => { const bm = await createImageBitmap(await (await fetch('data:image/png;base64,' + b64)).blob()); return [bm.width, bm.height]; }, (await p0.screenshot()).toString('base64'));
    await p0.context().close();
    const [R, RH] = dims;
    const frontiere = Math.round(R * 0.47) | 1;
    const ecr = [{ x: 0, y: 0, largeur: frontiere, hauteur: RH }, { x: frontiere, y: 0, largeur: R - frontiere, hauteur: RH }];
    const fx = frontiere / e;
    const cas = [
      ['bas-droite', async p => { await p.mouse.move(100.3, 50.6); await p.mouse.down(); await p.mouse.move(301.7, 250.2, { steps: 4 }); }],
      ['haut-gauche', async p => { await p.mouse.move(301.7, 250.2); await p.mouse.down(); await p.mouse.move(100.3, 50.6, { steps: 4 }); }],
      ['maj', async p => { await p.keyboard.down('Shift'); await p.mouse.move(fx - 30.4, 300.3); await p.mouse.down(); await p.mouse.move(fx - 230.1, 41.9, { steps: 4 }); }],
      ['borne', async p => { await p.mouse.move(fx - 100.2, 100.6); await p.mouse.down(); await p.mouse.move(l - 3, h + 50, { steps: 4 }); }],
      ['espace', async p => { await p.mouse.move(fx - 100, 100); await p.mouse.down(); await p.mouse.move(fx - 200, 200, { steps: 3 }); await p.keyboard.down('Space'); await p.mouse.move(fx + 300, 260, { steps: 5 }); }],
      ...casPixels(R, RH, e, fx),
    ];
    for (const [nom, geste, attendu] of cas) {
      p = await page({ echelle: e, l, h });
      id = prochain++;
      await p.evaluate(([id, R, RH, ecr]) => __emettre('selection:montrer', { id, numero: id, largeur: R, hauteur: RH, ecrans: ecr, cote_min: 3 }), [id, R, RH, ecr]);
      // Prête (mesure faite) plutôt qu'une pause fixe : sous charge, 120 ms ne suffisaient pas.
      await attendrePrete(p, id);
      await geste(p); await p.waitForTimeout(80);
      const vu = await vue(p);
      if (nom === 'borne') await p.screenshot({ path: `${SORTIE}/selection-sale-${Math.round(e * 10000)}-${l}.png` });
      await p.mouse.up(); await p.keyboard.up('Shift'); await p.keyboard.up('Space'); await p.waitForTimeout(50);
      const tout = await appels(p);
      t = tout.filter(x => x[0] === 'selection_terminee');
      const pr = tout.find(x => x[0] === 'selection_prete');
      const c = t.length === 1 ? coupe(t[0][1]) : null;
      verifier(c && vu.x === c.x && vu.y === c.y && vu.l === c.l && vu.h === c.h && vu.autres === 0 && (!attendu || attendu(t[0][1])),
        `8 ter ${R} × ${RH} à ${e} (viewport ${l}) ${nom} : zone vue = zone découpée, zéro pixel intermédiaire`, JSON.stringify({ vu, c, prete: pr && pr[1], t: t[0] && t[0][1] }));
      await p.context().close();
    }
  }

  // 9. Thème sombre du système : même voile (la page force le thème clair).
  p = await page({ theme: 'dark' });
  id = await montrer(p);
  px = await ligne(p, 200);
  verifier(px.every(c => proche(c, VOILE)), '9 thème sombre : même voile');
  await p.context().close();

  // 11. Contre-essais de la relecture.
  // A. Espace jusqu'au bord de l'écran de gauche : le départ envoyé est le dernier pixel de
  // l'écran de gauche (639), jamais le premier de l'écran voisin.
  p = await page();
  const DEUX_A = [{ x: 0, y: 0, largeur: 640, hauteur: H }, { x: 640, y: 0, largeur: 640, hauteur: H }];
  id = await montrer(p, DEUX_A);
  await p.mouse.move(600, 100); await p.mouse.down(); await p.mouse.move(500, 200, { steps: 3 });
  await p.keyboard.down('Space'); await p.mouse.move(800, 200, { steps: 4 }); await p.mouse.up(); await p.keyboard.up('Space'); await p.waitForTimeout(40);
  t = await termine(p);
  let zA = null;
  if (t.length === 1) {
    const { depart: d, arrivee: f } = t[0][1];
    zA = { depart: [d.x, d.y], l: Math.abs(d.x - f.x) + 1, h: Math.abs(d.y - f.y) + 1 };
  }
  verifier(zA && zA.depart[0] === 639 && zA.l === 101 && zA.h === 101, 'A Espace au bord : départ sur le dernier pixel de l\'écran de gauche, 101 × 101', JSON.stringify({ t, zA }));
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
  verifier(pendantAttente === 0 && !z.glisse && JSON.stringify(a) === JSON.stringify([['selection_annulee', { id, numero: id, cause: 'echap' }]]), 'B sans selection:cachee : attente, puis utilisable après 1,5 s', JSON.stringify({ pendantAttente, z: z.glisse, a }));
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
  verifier(JSON.stringify(a) === JSON.stringify([['selection_annulee', { id, numero: id, cause: 'echap' }]]), 'B invoke rejeté : utilisable aussitôt', JSON.stringify(a));
  // Même demande renvoyée (zone refusée par le cœur) : repos, utilisable, prête renvoyé.
  id = await montrer(p);
  await glisser(p, [100, 100], [200, 200]);
  await vider(p);
  await charge(id, [], 3, p.echelle)(p);
  await p.waitForTimeout(80);
  z = await etatZone(p);
  await glisser(p, [100, 100], [250, 250]);
  a = await appels(p);
  verifier(!z.glisse && a.length === 2 && a[0][0] === 'selection_prete' && a[1][0] === 'selection_terminee' && a[1][1].id === id, 'B demande renvoyée : repos puis nouveau glisser', JSON.stringify(a));
  await p.context().close();

  // C. Touches du navigateur neutralisées.
  p = await page();
  id = await montrer(p);
  const touches = await p.evaluate(() => [['F5', {}], ['r', { ctrlKey: true }], ['p', { ctrlKey: true }], ['f', { ctrlKey: true }], ['F12', {}], ['Tab', {}], ['a', {}], ['Shift', {}], [' ', {}],
    ['ArrowLeft', { altKey: true }], ['F11', {}], ['ContextMenu', {}], ['I', { ctrlKey: true, shiftKey: true }], ['=', { ctrlKey: true }]].map(([k, o]) => {
    const ev = new KeyboardEvent('keydown', { key: k, code: k === ' ' ? 'Space' : k, bubbles: true, cancelable: true, ...o });
    document.body.dispatchEvent(ev); return ev.defaultPrevented; }));
  verifier(touches.every(Boolean), 'C toute touche neutralisée (F5, Ctrl+R, Ctrl+P, Ctrl+F, F12, Alt+←, F11, Ctrl+Maj+I, Ctrl+=…)', JSON.stringify(touches));
  // D. Bouton tenu avant montrer : pas de glisser fantôme.
  await p.evaluate(() => __emettre('selection:cachee', null));
  await p.mouse.move(100, 100); await p.mouse.down();
  id = await montrer(p);
  await p.mouse.move(300, 300, { steps: 3 }); await p.mouse.up(); await p.waitForTimeout(40);
  verifier((await appels(p)).length === 0, 'D bouton tenu avant montrer : rien envoyé');
  // Aucun glisser avant selection_prete : gestes dans la même tâche que montrer, ignorés.
  await vider(p);
  const avantPrete = await p.evaluate(() => {
    __emettre('selection:montrer', { id: 500, numero: 500, largeur: 1280, hauteur: 400, ecrans: [], cote_min: 3 });
    const o = { bubbles: true, cancelable: true, pointerId: 1, pointerType: 'mouse', isPrimary: true };
    document.documentElement.dispatchEvent(new PointerEvent('pointerdown', { ...o, clientX: 100, clientY: 100, button: 0, buttons: 1 }));
    document.documentElement.dispatchEvent(new PointerEvent('pointermove', { ...o, clientX: 300, clientY: 300, button: -1, buttons: 1 }));
    document.documentElement.dispatchEvent(new PointerEvent('pointerup', { ...o, clientX: 300, clientY: 300, button: 0, buttons: 0 }));
    return __appels.length;
  });
  await p.waitForTimeout(100);
  a = await appels(p);
  verifier(avantPrete === 0 && JSON.stringify(a) === JSON.stringify([['selection_prete', { id: 500, numero: 500, largeur: 1280, hauteur: 400, echelle: 1, kx: 1, ky: 1, attente: false }]]), 'D avant selection_prete : aucun glisser', JSON.stringify({ avantPrete, a }));
  await p.context().close();

  // Taille qui change pendant un glisser : glisser abandonné, page « pas prête » jusqu'à la
  // charge suivante (un glisser complet fait avant elle n'envoie rien), selection_prete
  // renvoyée avec l'ancien numéro ; le cœur renvoie la charge (fenêtre relue, numéro
  // suivant) ; prête de nouveau, coins justes ensuite, avec le nouveau numéro.
  for (const e of [1, 1.25]) {
    p = await page({ echelle: e });
    id = await montrer(p);
    await p.mouse.move(100, 100); await p.mouse.down(); await p.mouse.move(300, 300, { steps: 3 }); await p.waitForTimeout(40);
    const pendantG = await etatZone(p);
    await p.setViewportSize({ width: 1000, height: 380 }); await p.waitForTimeout(40);
    const apresTaille = await etatZone(p);
    await p.mouse.move(350, 320, { steps: 2 }); await p.mouse.up(); await p.waitForTimeout(150);
    const apresResize = await appels(p);
    // Course : glisser complet avant la charge suivante.
    await vider(p);
    await glisser(p, [100, 100], [400, 300]); await p.waitForTimeout(40);
    const avantCharge = await appels(p);
    // Le cœur relit la fenêtre (1000 × 380 CSS) et renvoie la charge, numéro suivant.
    const n2 = id + 1000;
    await charge(id, [{ x: 0, y: 0, largeur: 1000, hauteur: 380 }], 3, e, n2)(p); await p.waitForTimeout(100);
    const pretes = await appels(p);
    await vider(p);
    await glisser(p, [100, 100], [900, 370]);
    t = await termine(p);
    const attendu = { id, numero: n2, depart: { x: Math.floor(100 * e), y: Math.floor(100 * e) }, arrivee: { x: Math.floor(900 * e), y: Math.floor(370 * e) } };
    const pr = { id, numero: n2, largeur: 1000, hauteur: 380, echelle: e, kx: e, ky: e, attente: false };
    const r0 = apresResize.length === 1 && apresResize[0][0] === 'selection_prete' ? apresResize[0][1] : null;
    verifier(pendantG.glisse && !apresTaille.glisse && r0 && r0.numero === id && r0.largeur === 1000 && r0.attente === true && avantCharge.length === 0
      && JSON.stringify(pretes) === JSON.stringify([['selection_prete', pr]]) && t.length === 1 && JSON.stringify(t[0][1]) === JSON.stringify(attendu),
      `taille changée pendant un glisser (k = ${e}) : abandonné, pas prête avant la charge suivante, coins justes ensuite`, JSON.stringify({ apresTaille: apresTaille.glisse, apresResize, avantCharge, pretes, t }));
    await p.context().close();
  }

  // Facteur seul changé (125 % → 100 %), rectangle réel inchangé (1600 × 500) : la page,
  // en attente, reçoit la même charge (même numéro, comme le cœur la renvoie), redevient
  // prête à k = 1 et les coins sont justes.
  {
    p = await page({ echelle: 1.25 });
    id = await montrer(p);
    const cdp = await p.context().newCDPSession(p);
    await cdp.send('Emulation.setDeviceMetricsOverride', { width: 1600, height: 500, deviceScaleFactor: 1, mobile: false });
    await p.waitForTimeout(150);
    const enAttente = await appels(p);
    await glisser(p, [100, 100], [300, 250]);
    const avantCharge = (await appels(p)).filter(x => x[0] !== 'selection_prete');
    await vider(p);
    await p.evaluate(([id]) => __emettre('selection:montrer', { id, numero: id, largeur: 1600, hauteur: 500, ecrans: [{ x: 0, y: 0, largeur: 1600, hauteur: 500 }], cote_min: 3 }), [id]);
    await p.waitForTimeout(100);
    const pretes = await appels(p);
    await vider(p);
    await glisser(p, [100, 100], [300, 250]);
    t = await termine(p);
    const e0 = enAttente.length === 1 ? enAttente[0][1] : null;
    verifier(e0 && enAttente[0][0] === 'selection_prete' && e0.attente === true && e0.numero === id && avantCharge.length === 0
      && JSON.stringify(pretes) === JSON.stringify([['selection_prete', { id, numero: id, largeur: 1600, hauteur: 500, echelle: 1, kx: 1, ky: 1, attente: false }]])
      && t.length === 1 && JSON.stringify(t[0][1]) === JSON.stringify({ id, numero: id, depart: { x: 100, y: 100 }, arrivee: { x: 300, y: 250 } }),
      'facteur seul changé, fenêtre inchangée : même charge renvoyée, prête de nouveau, coins justes', JSON.stringify({ enAttente, avantCharge, pretes, t }));
    await p.context().close();
  }

  // devicePixelRatio différent de ce qu'annoncerait tao : rien de tao n'est envoyé à la page ;
  // la page retient devicePixelRatio s'il explique la taille réelle, sinon k = taille réelle /
  // taille CSS ; elle devient prête et les coins suivent k.
  for (const [dpr, k] of [[1.25, 1.25], [1.25, 1.5], [1.5, 1.25], [1, 1.1]]) {
    p = await page({ echelle: dpr });
    await vider(p);
    id = prochain++;
    await charge(id, [{ x: 0, y: 0, largeur: L, hauteur: H }], 3, k)(p);
    await p.waitForTimeout(100);
    await glisser(p, [100, 100], [300, 250]);
    a = await appels(p);
    const attendu = [100, 100, 300, 250].map(v => Math.floor(v * k + 1e-3));
    const fin = a.find(x => x[0] === 'selection_terminee');
    const obtenu = fin ? [fin[1].depart.x, fin[1].depart.y, fin[1].arrivee.x, fin[1].arrivee.y] : [];
    const pr = a.find(x => x[0] === 'selection_prete');
    const regle = Math.abs(Math.round(L * k) / dpr - L) <= 1 && Math.abs(Math.round(H * k) / dpr - H) <= 1;
    const ke = regle ? [dpr, dpr] : [Math.round(L * k) / L, Math.round(H * k) / H];
    verifier(pr && JSON.stringify(pr[1]) === JSON.stringify({ id, numero: id, largeur: L, hauteur: H, echelle: dpr, kx: ke[0], ky: ke[1], attente: false }) && JSON.stringify(obtenu) === JSON.stringify(attendu),
      `devicePixelRatio ${dpr}, fenêtre réelle ${Math.round(L * k)} × ${Math.round(H * k)} : prête, coins justes à k = ${k}`, JSON.stringify({ pr, attendu, obtenu }));
    await p.context().close();
  }

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
  verifier(JSON.stringify(sousEspace) === JSON.stringify(apresEspace) && t.length === 1 && JSON.stringify(t[0][1]) === JSON.stringify({ id, numero: id, depart: { x: 150, y: 130 }, arrivee: { x: 250, y: 230 } }),
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

  // 12. Mode fenêtre (étape 8). Charges en pixels réels ; trait --window-highlight.
  const BLEU = [76, 194, 255];
  let nf = 0; // points du mode fenêtre
  const vf = (ok, quoi, info) => { nf++; verifier(ok, quoi, info); };
  async function montrerFenetre(p, { fenetres, ecrans, souris = null, numero, R, RH }) {
    const id = prochain++;
    const k = p.echelle;
    R = R || Math.round(L * k); RH = RH || Math.round(H * k);
    await p.evaluate(m => __emettre('selection:montrer', m), { id, numero: numero || id, largeur: R, hauteur: RH, ecrans: ecrans || [{ x: 0, y: 0, largeur: R, hauteur: RH }], cote_min: 3, mode: 'fenetre', fenetres, souris });
    await attendrePrete(p, id);
    await p.waitForTimeout(100);
    await vider(p);
    return id;
  }
  const etatFenetre = (p) => p.evaluate(() => {
    const f = document.getElementById('fenetre'), s = getComputedStyle(f);
    return { entoure: document.body.classList.contains('entoure'), affiche: s.display, x: parseInt(f.style.left), y: parseInt(f.style.top), l: parseInt(f.style.width), h: parseInt(f.style.height), rayons: f.style.borderRadius, curseur: getComputedStyle(document.documentElement).cursor };
  });
  // Pixels d'une capture : couleurs des points demandés et bilan de toute l'image autour du
  // rectangle `r` (coins arrondis de rayon `ray` exclus du compte des pixels mêlés).
  async function analyser(p, pts, r, ray) {
    const b64 = (await p.screenshot()).toString('base64');
    return p.evaluate(async ([b64, pts, r, ray, fond, voile, bleu]) => {
      const bm = await createImageBitmap(await (await fetch('data:image/png;base64,' + b64)).blob());
      const c = new OffscreenCanvas(bm.width, bm.height), x = c.getContext('2d'); x.drawImage(bm, 0, 0);
      const d = x.getImageData(0, 0, bm.width, bm.height).data;
      const col = (px, py) => { const i = (py * bm.width + px) * 4; return [d[i], d[i + 1], d[i + 2]]; };
      const pres = (a, b) => a.every((u, k) => Math.abs(u - b[k]) <= 3);
      const couleurs = pts.map(([px, py]) => col(px, py));
      if (!r) return { couleurs };
      let x0 = 1e9, y0 = 1e9, x1 = -1, y1 = -1, autres = 0, sombresDedans = 0;
      const coin = (px, py) => (px < r.x + ray || px >= r.x + r.l - ray) && (py < r.y + ray || py >= r.y + r.h - ray);
      for (let py = 0; py < bm.height; py++) for (let px = 0; px < bm.width; px++) {
        const v = col(px, py);
        const estVoile = pres(v, voile);
        if (!estVoile) { x0 = Math.min(x0, px); y0 = Math.min(y0, py); x1 = Math.max(x1, px); y1 = Math.max(y1, py); }
        if (coin(px, py)) continue;
        if (!estVoile && !pres(v, fond) && !pres(v, bleu)) autres++;
        if (estVoile && px >= r.x && px < r.x + r.l && py >= r.y && py < r.y + r.h) sombresDedans++;
      }
      return { couleurs, vu: { x: x0, y: y0, l: x1 - x0 + 1, h: y1 - y0 + 1 }, autres, sombresDedans };
    }, [b64, pts, r, ray, FOND, VOILE, BLEU]);
  }
  // Le dernier pixel de la diagonale d'un coin entièrement hors de l'arc (voile) et le premier
  // entièrement dans l'arc intérieur du trait (clair), décalés depuis le coin.
  const horsArc = (r) => { let i = 0; while ((r - (i + 1) - 1) * Math.SQRT2 > r + 1e-6) i++; return i; };
  const dansArc = (r, t) => Math.floor(r - (r - t) / Math.SQRT2) + 1;

  // 12 a. Contour au pixel près à 100, 125, 150 et 200 % : voile autour, fenêtre claire, trait
  // de max(2, round(2 k)) pixels réels à l'intérieur, coins arrondis au rayon du cœur.
  for (const e of [1, 1.25, 1.5, 2]) {
    p = await page({ echelle: e });
    const W = { x: Math.round(200 * e), y: Math.round(80 * e), largeur: Math.round(500 * e), hauteur: Math.round(250 * e), rayon: Math.round(8 * e) };
    id = await montrerFenetre(p, { fenetres: [W] });
    await p.mouse.move(400.3, 200.6); await p.waitForTimeout(60);
    const ef = await etatFenetre(p);
    const t = Math.max(2, Math.round(2 * e)), r = W.rayon;
    const x0 = W.x, y0 = W.y, x1 = W.x + W.largeur, y1 = W.y + W.hauteur, ym = y0 + 100, xm = x0 + 200;
    const ho = horsArc(r), di = dansArc(r, t);
    const pts = [[x0 - 1, ym], [x0, ym], [x0 + t - 1, ym], [x0 + t, ym], [x1 - t - 1, ym], [x1 - t, ym], [x1 - 1, ym], [x1, ym],
      [xm, y0 - 1], [xm, y0], [xm, y0 + t - 1], [xm, y0 + t], [xm, y1 - t - 1], [xm, y1 - t], [xm, y1 - 1], [xm, y1],
      [x0, y0], [x1 - 1, y0], [x0, y1 - 1], [x1 - 1, y1 - 1],
      [x0 + ho, y0 + ho], [x1 - 1 - ho, y0 + ho], [x0 + ho, y1 - 1 - ho], [x1 - 1 - ho, y1 - 1 - ho],
      [x0 + di, y0 + di], [x1 - 1 - di, y0 + di], [x0 + di, y1 - 1 - di], [x1 - 1 - di, y1 - 1 - di]];
    const a = await analyser(p, pts, { x: x0, y: y0, l: W.largeur, h: W.hauteur }, r);
    const C = a.couleurs, V = (c) => proche(c, VOILE), F = (c) => proche(c, FOND), B = (c) => proche(c, BLEU);
    const bords = V(C[0]) && B(C[1]) && B(C[2]) && F(C[3]) && F(C[4]) && B(C[5]) && B(C[6]) && V(C[7])
      && V(C[8]) && B(C[9]) && B(C[10]) && F(C[11]) && F(C[12]) && B(C[13]) && B(C[14]) && V(C[15]);
    vf(ef.entoure && ef.x === x0 && ef.y === y0 && ef.l === W.largeur && ef.h === W.hauteur && bords,
      `12 a k = ${e} : contour posé au pixel près, trait de ${t} pixels réels à l'intérieur`, JSON.stringify({ ef, C: C.slice(0, 16) }));
    vf(a.vu.x === x0 && a.vu.y === y0 && a.vu.l === W.largeur && a.vu.h === W.hauteur && a.autres === 0 && a.sombresDedans === 0,
      `12 a k = ${e} : rien de clair hors du contour, rien de sombre dedans, aucun pixel adouci hors des arcs`, JSON.stringify(a.vu) + ' ' + a.autres + ' ' + a.sombresDedans);
    vf(C.slice(16, 24).every(V) && C.slice(24, 28).every(F),
      `12 a k = ${e} : coins arrondis (rayon ${r}) : pixel du coin et dernier pixel hors de l'arc (${ho}) assombris, premier pixel dans l'arc (${di}) clair`, JSON.stringify(C.slice(16)));
    await p.screenshot({ path: `${SORTIE}/selection-fenetre-${Math.round(e * 100)}.png` });
    vf(ef.curseur === 'default', `12 a k = ${e} : curseur flèche en mode fenêtre`, ef.curseur);
    await p.context().close();
  }

  // 12 b. Fenêtre à cheval sur deux écrans : coupée à l'écran de la souris ; seuls les vrais
  // coins sont arrondis (le côté coupé garde des coins droits).
  for (const e of [1, 1.5]) {
    p = await page({ echelle: e });
    const R = Math.round(L * e), RH = Math.round(H * e), Fr = Math.round(640 * e);
    const ecr = [{ x: 0, y: 0, largeur: Fr, hauteur: RH }, { x: Fr, y: 0, largeur: R - Fr, hauteur: RH }];
    const W = { x: Math.round(500 * e), y: Math.round(100 * e), largeur: Math.round(300 * e), hauteur: Math.round(200 * e), rayon: Math.round(8 * e) };
    const r = W.rayon, t = Math.max(2, Math.round(2 * e)), y0 = W.y, y1 = W.y + W.hauteur, x0 = W.x, x1 = W.x + W.largeur;
    id = await montrerFenetre(p, { fenetres: [W], ecrans: ecr });
    await p.mouse.move(550, 200); await p.waitForTimeout(60);
    let ef = await etatFenetre(p);
    let a = await analyser(p, [[x0, y0], [x0, y1 - 1], [Fr - 1, y0], [Fr - 1, y1 - 1], [Fr, y0 + 50]], { x: x0, y: y0, l: Fr - x0, h: W.hauteur }, r);
    let C = a.couleurs;
    vf(ef.entoure && ef.x === x0 && ef.l === Fr - x0 && ef.rayons === `${r}px 0px 0px ${r}px` && proche(C[0], VOILE) && proche(C[1], VOILE) && proche(C[2], BLEU) && proche(C[3], BLEU) && proche(C[4], VOILE)
      && a.vu.x === x0 && a.vu.l === Fr - x0 && a.autres === 0 && a.sombresDedans === 0,
      `12 b k = ${e} : souris à gauche : partie gauche entourée, coins droits au bord de l'écran`, JSON.stringify({ ef, C, a: a.vu }));
    await p.mouse.move(Fr / e + 40, 200); await p.waitForTimeout(60);
    ef = await etatFenetre(p);
    a = await analyser(p, [[Fr, y0], [Fr, y1 - 1], [x1 - 1, y0], [x1 - 1, y1 - 1], [Fr - 1, y0 + 50]], { x: Fr, y: y0, l: x1 - Fr, h: W.hauteur }, r);
    C = a.couleurs;
    vf(ef.entoure && ef.x === Fr && ef.l === x1 - Fr && ef.rayons === `0px ${r}px ${r}px 0px` && proche(C[0], BLEU) && proche(C[1], BLEU) && proche(C[2], VOILE) && proche(C[3], VOILE) && proche(C[4], VOILE)
      && a.vu.x === Fr && a.vu.l === x1 - Fr && a.autres === 0 && a.sombresDedans === 0,
      `12 b k = ${e} : souris à droite : le contour suit, coupé à l'écran de droite`, JSON.stringify({ ef, C, a: a.vu, t }));
    await p.screenshot({ path: `${SORTIE}/selection-fenetre-cheval-${Math.round(e * 100)}.png` });
    // Trop petite une fois coupée (2 pixels sur l'écran de droite) : rien.
    id = await montrerFenetre(p, { fenetres: [{ x: Fr - 400, y: 50, largeur: 402, hauteur: 300, rayon: 8 }], ecrans: ecr });
    await p.mouse.move((Fr + 1) / e, 100 / e); await p.mouse.move((Fr + 1.5) / e, 101 / e); await p.waitForTimeout(60);
    ef = await etatFenetre(p);
    await p.mouse.click((Fr + 1.5) / e, 101 / e); await p.waitForTimeout(40);
    vf(!ef.entoure && (await appels(p)).length === 0, `12 b k = ${e} : fenêtre trop petite sur l'écran de la souris : rien d'entouré, clic sans effet`, JSON.stringify(ef));
    await p.context().close();
  }

  // 12 c. Survol de deux fenêtres qui se recouvrent, bureau, position initiale de la souris.
  p = await page();
  const A = { x: 100, y: 100, largeur: 300, hauteur: 200, rayon: 8 }, Bf = { x: 250, y: 150, largeur: 400, hauteur: 200, rayon: 0 };
  id = await montrerFenetre(p, { fenetres: [A, Bf], souris: { x: 500, y: 300 } });
  let ef = await etatFenetre(p);
  vf(ef.entoure && ef.x === 250 && ef.y === 150 && ef.l === 400 && ef.h === 200 && ef.rayons === '0px', '12 c position souris de la charge : fenêtre entourée avant tout mouvement', JSON.stringify(ef));
  await p.mouse.move(300, 200); await p.waitForTimeout(60);
  ef = await etatFenetre(p);
  const surA = ef.entoure && ef.x === 100 && ef.y === 100 && ef.l === 300 && ef.rayons === '8px';
  await p.mouse.move(500, 320); await p.waitForTimeout(60);
  ef = await etatFenetre(p);
  const surB = ef.entoure && ef.x === 250 && ef.l === 400;
  vf(surA && surB, '12 c recouvrement : la fenêtre du dessus, puis celle de dessous, sans attendre', JSON.stringify(ef));
  await p.mouse.move(900, 50); await p.waitForTimeout(60);
  ef = await etatFenetre(p);
  px = await ligne(p, 200);
  vf(!ef.entoure && ef.affiche === 'none' && px.every(c => proche(c, VOILE)), '12 c bureau : rien n\'est entouré, voile partout', JSON.stringify(ef));
  await p.mouse.click(900, 50); await p.waitForTimeout(40);
  vf((await appels(p)).length === 0, '12 c clic sur le bureau : rien envoyé');
  // Souris de la charge absente (et aucun mouvement depuis la charge) : rien.
  id = await montrerFenetre(p, { fenetres: [A] });
  ef = await etatFenetre(p);
  vf(!ef.entoure, '12 c souris absente de la charge, aucun mouvement depuis : rien d\'entouré', JSON.stringify(ef));
  await p.context().close();
  p = await page();
  id = await montrerFenetre(p, { fenetres: [A], souris: { x: 10, y: 10 } });
  ef = await etatFenetre(p);
  vf(!ef.entoure, '12 c souris de la charge sur le bureau : rien d\'entouré', JSON.stringify(ef));
  await p.context().close();

  // 12 d. Clic : selection_cliquee une seule fois, au pixel sous la pointe ; glisser : rien.
  for (const e of [1, 1.25]) {
    p = await page({ echelle: e });
    const W = { x: Math.round(100 * e), y: Math.round(100 * e), largeur: Math.round(400 * e), hauteur: Math.round(200 * e), rayon: 8 };
    id = await montrerFenetre(p, { fenetres: [W] });
    await p.mouse.move(150, 150); await p.waitForTimeout(40);
    await p.mouse.down(); await p.mouse.move(250, 220, { steps: 5 }); await p.mouse.up(); await p.waitForTimeout(60);
    const zoneVue = await etatZone(p);
    vf((await appels(p)).length === 0 && !zoneVue.glisse, `12 d k = ${e} : glisser : rien envoyé, rien tracé`, JSON.stringify(zoneVue));
    await p.keyboard.down('Shift'); await p.keyboard.down('Space'); await p.waitForTimeout(40);
    const sousTouches = await etatFenetre(p);
    await p.keyboard.up('Space'); await p.keyboard.up('Shift');
    await p.mouse.click(301.3, 170.7); await p.waitForTimeout(40);
    const avant = await etatFenetre(p);
    await p.mouse.move(600, 380); await p.waitForTimeout(40);
    await p.mouse.click(301.3, 170.7); await p.keyboard.press('Escape'); await p.waitForTimeout(40);
    const apres = await etatFenetre(p);
    const c = await appels(p);
    const attendu = { id, numero: id, x: Math.floor(301.3 * e + 1e-3), y: Math.floor(170.7 * e + 1e-3) };
    vf(JSON.stringify(c) === JSON.stringify([['selection_cliquee', attendu]]) && JSON.stringify(avant) === JSON.stringify(apres) && avant.entoure && sousTouches.entoure,
      `12 d k = ${e} : clic → selection_cliquee une seule fois, bon pixel, plus rien ne change ; Maj et Espace sans effet`, JSON.stringify({ c, attendu, avant, apres }));
    await p.context().close();
  }

  // 12 d bis. Seuil entre clic et glisser : max(cote_min, round(4 k)) pixels réels sur un axe
  // (4 pixels logiques). Juste en dessous : clic ; au seuil : glisser, rien.
  for (const e of [1, 1.25, 2]) {
    const S = Math.max(3, Math.round(4 * e));
    for (const [d, axe, clic] of [[S - 1, 'x', true], [S, 'x', false], [S - 1, 'y', true], [S, 'y', false]]) {
      p = await page({ echelle: e });
      const W = { x: 0, y: 0, largeur: Math.round(L * e), hauteur: Math.round(H * e), rayon: 0 };
      id = await montrerFenetre(p, { fenetres: [W] });
      const P0 = [400, 200], P1 = axe === 'x' ? [400 + d, 200] : [400, 200 + d];
      await p.mouse.move((P0[0] + 0.5) / e, (P0[1] + 0.5) / e); await p.mouse.down();
      await p.mouse.move((P1[0] + 0.5) / e, (P1[1] + 0.5) / e, { steps: 3 }); await p.mouse.up(); await p.waitForTimeout(40);
      a = await appels(p);
      const ok = clic ? JSON.stringify(a) === JSON.stringify([['selection_cliquee', { id, numero: id, x: P1[0], y: P1[1] }]]) : a.length === 0;
      vf(ok, `12 d bis k = ${e} : déplacement de ${d} pixels réels en ${axe} (seuil ${S}) : ${clic ? 'clic' : 'glisser, rien'}`, JSON.stringify(a));
      await p.context().close();
    }
  }

  // 12 d ter. Appui perdu, pointercancel, charge reçue pendant l'appui : rien, puis le clic
  // suivant part.
  p = await page();
  id = await montrerFenetre(p, { fenetres: [{ x: 0, y: 0, largeur: L, hauteur: H, rayon: 0 }] });
  await p.evaluate(() => document.documentElement.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, cancelable: true, pointerId: 7, pointerType: 'mouse', isPrimary: true, clientX: 100, clientY: 100, button: 0, buttons: 1 })));
  await p.mouse.click(600, 300); await p.waitForTimeout(40);
  a = await appels(p);
  vf(JSON.stringify(a) === JSON.stringify([['selection_cliquee', { id, numero: id, x: 600, y: 300 }]]), '12 d ter appui sans relâchement puis clic ailleurs : le clic part', JSON.stringify(a));
  id = await montrerFenetre(p, { fenetres: [{ x: 0, y: 0, largeur: L, hauteur: H, rayon: 0 }] });
  await p.mouse.move(300, 200); await p.mouse.down();
  await p.evaluate(() => document.documentElement.dispatchEvent(new PointerEvent('pointercancel', { bubbles: true, pointerId: __pointeur, pointerType: 'mouse', isPrimary: true })));
  await p.mouse.up(); await p.waitForTimeout(40);
  const apresAnnule = await appels(p);
  await p.mouse.click(310, 210); await p.waitForTimeout(40);
  a = await appels(p);
  vf(apresAnnule.length === 0 && JSON.stringify(a) === JSON.stringify([['selection_cliquee', { id, numero: id, x: 310, y: 210 }]]), '12 d ter pointercancel pendant l\'appui : rien, puis le clic suivant part', JSON.stringify({ apresAnnule, a }));
  id = await montrerFenetre(p, { fenetres: [{ x: 0, y: 0, largeur: L, hauteur: H, rayon: 0 }] });
  await p.mouse.move(300, 200); await p.mouse.down();
  await p.evaluate(([id, L, H]) => __emettre('selection:montrer', { id, numero: id, largeur: L, hauteur: H, ecrans: [{ x: 0, y: 0, largeur: L, hauteur: H }], cote_min: 3, mode: 'fenetre', fenetres: [{ x: 0, y: 0, largeur: L, hauteur: H, rayon: 0 }], souris: null }), [id, L, H]);
  await p.waitForTimeout(100);
  await p.mouse.up(); await p.waitForTimeout(40);
  const apresCharge = (await appels(p)).filter(x => x[0] !== 'selection_prete');
  await vider(p);
  await p.mouse.click(320, 220); await p.waitForTimeout(40);
  a = await appels(p);
  vf(apresCharge.length === 0 && JSON.stringify(a) === JSON.stringify([['selection_cliquee', { id, numero: id, x: 320, y: 220 }]]), '12 d ter charge reçue pendant l\'appui : rien, puis le clic suivant part', JSON.stringify({ apresCharge, a }));
  await p.context().close();

  // 12 e. Clic droit, Échap : annulation ; envoi refusé : utilisable de nouveau.
  p = await page();
  const W1 = { x: 100, y: 100, largeur: 400, hauteur: 200, rayon: 8 };
  id = await montrerFenetre(p, { fenetres: [W1] });
  await p.mouse.click(200, 200, { button: 'right' }); await p.mouse.click(200, 200); await p.waitForTimeout(40);
  a = await appels(p);
  vf(JSON.stringify(a) === JSON.stringify([['selection_annulee', { id, numero: id, cause: 'clic_droit' }]]), '12 e clic droit : annulation, plus rien ensuite', JSON.stringify(a));
  id = await montrerFenetre(p, { fenetres: [W1] });
  await p.keyboard.press('Escape'); await p.keyboard.press('Escape'); await p.waitForTimeout(40);
  a = await appels(p);
  vf(JSON.stringify(a) === JSON.stringify([['selection_annulee', { id, numero: id, cause: 'echap' }]]), '12 e Échap : une seule annulation', JSON.stringify(a));
  id = await montrerFenetre(p, { fenetres: [W1] });
  await p.evaluate(() => __rejeter.add('selection_cliquee'));
  await p.mouse.click(200, 200); await p.waitForTimeout(40);
  await p.evaluate(() => __rejeter.clear()); await vider(p);
  await p.mouse.move(210, 210); await p.mouse.click(210, 210); await p.waitForTimeout(40);
  a = await appels(p);
  vf(JSON.stringify(a) === JSON.stringify([['selection_cliquee', { id, numero: id, x: 210, y: 210 }]]), '12 e clic refusé par le cœur : utilisable aussitôt', JSON.stringify(a));
  // Même charge renvoyée (clic sur rien pour le cœur) : repos, contour de nouveau, nouveau clic.
  id = await montrerFenetre(p, { fenetres: [W1] });
  await p.mouse.click(220, 220); await p.waitForTimeout(40); await vider(p);
  await p.evaluate(([id, L, H]) => __emettre('selection:montrer', { id, numero: id, largeur: L, hauteur: H, ecrans: [{ x: 0, y: 0, largeur: L, hauteur: H }], cote_min: 3, mode: 'fenetre', fenetres: [{ x: 100, y: 100, largeur: 400, hauteur: 200, rayon: 8 }], souris: { x: 230, y: 230 } }), [id, L, H]);
  await p.waitForTimeout(100);
  ef = await etatFenetre(p);
  await p.mouse.click(230, 230); await p.waitForTimeout(40);
  a = await appels(p);
  vf(ef.entoure && a.length === 2 && a[0][0] === 'selection_prete' && JSON.stringify(a[1]) === JSON.stringify(['selection_cliquee', { id, numero: id, x: 230, y: 230 }]), '12 e charge renvoyée : contour de nouveau, nouveau clic', JSON.stringify({ ef, a }));
  await p.context().close();

  // 12 f. Charge de mode zone après une charge de mode fenêtre : plus aucune trace du contour.
  p = await page();
  id = await montrerFenetre(p, { fenetres: [W1], souris: { x: 200, y: 200 } });
  const avantZone = await etatFenetre(p);
  id = await montrer(p);
  ef = await etatFenetre(p);
  px = await ligne(p, 200);
  await glisser(p, [600, 100], [800, 300]);
  t = await termine(p);
  vf(avantZone.entoure && !ef.entoure && ef.affiche === 'none' && ef.curseur === 'crosshair' && px.every(c => proche(c, VOILE)) && t.length === 1,
    '12 f charge de mode zone ensuite : plus de contour, croix, voile partout, glisser normal', JSON.stringify({ avantZone, ef }));
  // Clic en mode zone : toujours rien.
  id = await montrer(p);
  await p.mouse.click(200, 200); await p.waitForTimeout(40);
  vf((await appels(p)).length === 0, '12 f mode zone : un clic ne fait rien');
  await p.context().close();

  // 12 g. Demandes invalides en mode fenêtre : ignorées ; taille changée : plus de contour
  // jusqu'à la charge suivante.
  p = await page();
  await vider(p);
  const TF = { numero: 1, largeur: L, hauteur: H, ecrans: [], cote_min: 3 };
  for (const m of [{ ...TF, id: 1, mode: 'fenêtre', fenetres: [] }, { ...TF, id: 2, mode: 'fenetre', fenetres: [{ x: 1.5, y: 0, largeur: 10, hauteur: 10, rayon: 0 }] },
    { ...TF, id: 3, mode: 'fenetre', fenetres: [{ x: 0, y: 0, largeur: 10, hauteur: 10, rayon: -1 }] }, { ...TF, id: 4, mode: 'fenetre', fenetres: 'x' },
    { ...TF, id: 5, mode: 'fenetre', fenetres: [{ x: 0, y: 0, largeur: 10, hauteur: 10 }] }, { ...TF, id: 6, mode: 'fenetre', fenetres: [], souris: { x: 1.5, y: 2 } },
    { ...TF, id: 7, mode: 3, fenetres: [] }, { ...TF, id: 8, mode: 'fenetre', fenetres: [{ x: 0, y: 0, largeur: -1, hauteur: 10, rayon: 0 }] }]) {
    await p.evaluate(m => __emettre('selection:montrer', m), m);
  }
  await p.waitForTimeout(100);
  vf((await appels(p)).length === 0, '12 g demandes invalides (mode inconnu, fenêtre non entière, rayon négatif…) ignorées');
  id = await montrerFenetre(p, { fenetres: [W1], souris: { x: 200, y: 200 } });
  const avantTaille = await etatFenetre(p);
  await p.setViewportSize({ width: 1000, height: 380 }); await p.waitForTimeout(150);
  await p.mouse.move(300, 200); await p.waitForTimeout(40);
  await p.mouse.click(300, 200); await p.waitForTimeout(40);
  const apresTaille = await etatFenetre(p);
  a = await appels(p);
  vf(avantTaille.entoure && !apresTaille.entoure && a.length === 1 && a[0][0] === 'selection_prete' && a[0][1].attente === true,
    '12 g taille changée : contour retiré, clic sans effet, prête en attente', JSON.stringify({ apresTaille, a }));
  await p.context().close();
  console.log('POINTS DU MODE FENETRE :', nf);

  {
    // 13. Mode texte (0.5, §2.3) : une zone comme le mode zone, avec le nom « Copier le texte :
    // sélectionnez une zone » ; `selection:balayer` teinte la zone et la fait respirer 200 ms
    // (réduit : la teinte seule) ; `selection:surligner` (13/11, choix « Surligné ») tire
    // chaque ligne copiée de gauche à droite, l'une après l'autre, puis efface la sélection.
    async function montrerTexte(p, numero = null) {
      const i = prochain++;
      await p.evaluate(
        ([i, n, k]) => {
          const l = Math.round(innerWidth * k),
            h = Math.round(innerHeight * k);
          __emettre('selection:montrer', {
            id: i,
            numero: n ?? i,
            largeur: l,
            hauteur: h,
            ecrans: [{ x: 0, y: 0, largeur: l, hauteur: h }],
            cote_min: 3,
            mode: 'texte',
          });
        },
        [i, numero, p.echelle],
      );
      await attendrePrete(p, i);
      await p.waitForTimeout(80);
      await vider(p);
      return i;
    }
    const etatBalayage = (p) =>
      p.evaluate(() => {
        const z = document.getElementById('zone');
        return {
          balaye: document.body.classList.contains('balaye'),
          bande: z.querySelector('.bande') !== null,
          anims: z
            .getAnimations()
            .map((a) => [a.animationName, a.effect.getTiming().duration])
            .sort(),
          teinte: getComputedStyle(z).backgroundColor,
          zone: [z.style.left, z.style.top, z.style.width, z.style.height],
          glisse: document.body.classList.contains('glisse'),
        };
      });
    const etatSurligne = (p) =>
      p.evaluate(() => {
        const z = document.getElementById('zone'),
          scene = document.getElementById('scene'),
          lignes = [...z.querySelectorAll('.lus i')];
        return {
          surligne: document.body.classList.contains('surligne'),
          affiche: getComputedStyle(z.querySelector('.lus')).display,
          lignes: lignes.map((i) => [i.style.left, i.style.top, i.style.width, i.style.height]),
          anims: lignes.flatMap((i) =>
            i.getAnimations().map((a) => {
              const t = a.effect.getTiming();
              return [a.animationName, t.duration, t.delay];
            }),
          ),
          fond: lignes.length ? getComputedStyle(lignes[0]).backgroundColor : '',
          sortie: scene.getAnimations().map((a) => {
            const t = a.effect.getTiming();
            return [a.animationName, t.duration, t.delay];
          }),
          opacite: getComputedStyle(scene).opacity,
          glisse: document.body.classList.contains('glisse'),
        };
      });
    // Les lignes d'un texte copié dans la zone (100, 100, 201, 121) : pixels réels de la fenêtre.
    const LIGNES = [
      { x: 110, y: 110, largeur: 150, hauteur: 14 },
      { x: 110, y: 130, largeur: 90, hauteur: 14 },
      { x: 110, y: 150, largeur: 120, hauteur: 14 },
    ];
    const DEROULE = { trace: 260, pas: 40, tenue: 220, sortie: 140 };
    const surligner = (p, charge) =>
      p.evaluate((c) => {
        __emettre('selection:surligner', c);
        return performance.now();
      }, charge);
    p = await page();
    const nomDe = () =>
      p.evaluate(() => [
        document.body.getAttribute('role'),
        document.body.getAttribute('aria-label'),
      ]);
    id = await montrer(p);
    const nomZone = await nomDe();
    const pxZone = await ligne(p, 200);
    id = await montrerTexte(p);
    const nomTexte = await nomDe();
    const pxTexte = await ligne(p, 200);
    const txt = await p.evaluate(
      () => document.body.innerText.trim() + '|' + document.body.textContent.trim(),
    );
    verifier(
      nomZone[0] === null &&
        nomZone[1] === null &&
        nomTexte[0] === 'application' &&
        nomTexte[1] === 'Copier le texte : sélectionnez une zone',
      '13 mode texte : nom « Copier le texte : sélectionnez une zone » (aucun en mode zone)',
      JSON.stringify([nomZone, nomTexte]),
    );
    verifier(
      txt === '|' &&
        JSON.stringify(pxZone) === JSON.stringify(pxTexte) &&
        (await p.evaluate(() => getComputedStyle(document.documentElement).cursor)) === 'crosshair',
      '13 mode texte : même voile, croix, aucun texte affiché',
      txt,
    );
    await glisser(p, [100, 100], [300, 220]);
    t = await termine(p);
    verifier(
      t.length === 1 &&
        JSON.stringify(t[0][1]) ===
          JSON.stringify({
            id,
            numero: id,
            depart: { x: 100, y: 100 },
            arrivee: { x: 300, y: 220 },
          }),
      '13 mode texte : selection_terminee comme une zone',
      JSON.stringify(t),
    );
    await vider(p);
    // Surligner avant le balayage : rien (la zone lue n'est pas encore posée).
    await surligner(p, { id, lignes: LIGNES, reduites: false, ...DEROULE });
    const avantBalayage = await etatSurligne(p);
    const relache = Date.now();
    await p.waitForTimeout(600); // le cœur balaie après la découpe : le filet du relâcher court
    // Balayage d'un autre affichage : ignoré.
    await p.evaluate(
      (i) =>
        __emettre('selection:balayer', {
          id: i + 1000,
          x: 100,
          y: 100,
          largeur: 201,
          hauteur: 121,
          reduites: false,
        }),
      id,
    );
    const ignore = await etatBalayage(p);
    const t0 = await p.evaluate((i) => {
      __emettre('selection:balayer', {
        id: i,
        x: 100,
        y: 100,
        largeur: 201,
        hauteur: 121,
        reduites: false,
      });
      return performance.now();
    }, id);
    const pendant = await etatBalayage(p);
    await p.waitForTimeout(100);
    await p.screenshot({ path: `${SORTIE}/selection-texte-balayage.png` });
    await p.waitForTimeout(300);
    const apres = await etatBalayage(p);
    verifier(
      !ignore.balaye &&
        !avantBalayage.surligne &&
        avantBalayage.lignes.length === 0 &&
        pendant.balaye &&
        !pendant.bande &&
        JSON.stringify(pendant.anims) ===
          JSON.stringify([
            ['respire', 200],
            ['teinte', 150],
          ]) &&
        JSON.stringify(pendant.zone) === JSON.stringify(['100px', '100px', '201px', '121px']) &&
        pendant.glisse,
      '13 selection:balayer : la zone se teinte et respire 200 ms, sans bande (autre id, surligner avant : rien)',
      JSON.stringify({ ignore, avantBalayage, pendant }),
    );
    verifier(
      apres.balaye &&
        apres.anims.length === 0 &&
        apres.teinte !== 'rgba(0, 0, 0, 0)' &&
        apres.glisse &&
        (await appels(p)).length === 0,
      "13 lecture : la teinte reste après la respiration, rien n'est envoyé",
      JSON.stringify(apres),
    );
    // Surlignage : autre affichage, durée fausse ou aucune ligne valable : ignoré.
    await surligner(p, { id: id + 1000, lignes: LIGNES, reduites: false, ...DEROULE });
    await surligner(p, { id, lignes: LIGNES, reduites: false, ...DEROULE, trace: -1 });
    await surligner(p, { id, lignes: LIGNES, reduites: false, ...DEROULE, sortie: 99999 });
    await surligner(p, { id, lignes: LIGNES, reduites: false, ...DEROULE, pas: 1.5 });
    await surligner(p, { id, lignes: [{ x: 110, y: 110, largeur: 0, hauteur: 14 }], reduites: false, ...DEROULE });
    await surligner(p, { id, lignes: [], reduites: false, ...DEROULE });
    const refuse = await etatSurligne(p);
    const t2 = await surligner(p, {
      id,
      lignes: [...LIGNES, { x: 'x', y: 0, largeur: 5, hauteur: 5 }],
      reduites: false,
      ...DEROULE,
    });
    const tire = await etatSurligne(p);
    await p.waitForTimeout(300);
    await p.screenshot({ path: `${SORTIE}/selection-texte-surligne.png` });
    const tenue = await etatSurligne(p);
    // La sortie finit à 560 + 140 ms : la sélection est alors effacée, encore montrée.
    await p.waitForFunction(() => getComputedStyle(document.getElementById('scene')).opacity === '0', null, { timeout: 3000 }).catch(() => {});
    const efface = await p.evaluate((t2) => performance.now() - t2, t2);
    const finie = await etatSurligne(p);
    verifier(
      !refuse.surligne &&
        refuse.lignes.length === 0 &&
        tire.surligne &&
        tire.affiche === 'block' &&
        JSON.stringify(tire.lignes) ===
          JSON.stringify([
            ['8px', '9px', '154px', '16px'],
            ['8px', '29px', '94px', '16px'],
            ['8px', '49px', '124px', '16px'],
          ]) &&
        JSON.stringify(tire.anims) ===
          JSON.stringify([
            ['tire', 260, 0],
            ['tire', 260, 40],
            ['tire', 260, 80],
          ]) &&
        JSON.stringify(tire.sortie) === JSON.stringify([['part', 140, 560]]) &&
        tire.fond !== 'rgba(0, 0, 0, 0)',
      '13 selection:surligner : une bande par ligne (2 px de plus sur les côtés, 1 en haut et en bas), tirée en 260 ms, 40 ms après la précédente ; sortie de 140 ms après 560 ms (autre id, durée fausse, aucune ligne : rien)',
      JSON.stringify({ refuse, tire }),
    );
    verifier(
      tenue.surligne && tenue.opacite === '1' && efface >= 690 && efface < 1500 && finie.glisse && finie.lignes.length === 3 && (await appels(p)).length === 0,
      "13 surlignage : la sélection reste nette pendant la tenue, s'efface à 700 ms, rien n'est envoyé",
      JSON.stringify({ tenue: tenue.opacite, efface, finie }),
    );
    // Le filet repart du surlignage : la page ne se remet pas au repos à 1,5 s du balayage.
    await p.waitForTimeout(Math.max(0, 1700 - (await p.evaluate((t0) => performance.now() - t0, t0))));
    const tenu = await etatSurligne(p);
    verifier(
      tenu.glisse && tenu.surligne && tenu.opacite === '0',
      '13 filet reparti au surlignage : sélection encore là (effacée) 1,7 s après le balayage',
      JSON.stringify({ tenu, depuisRelache: Date.now() - relache }),
    );
    await p.evaluate(() => __emettre('selection:cachee', {}));
    const cachee = await etatSurligne(p);
    verifier(
      !cachee.surligne && cachee.lignes.length === 0 && cachee.opacite === '1' && cachee.sortie.length === 0 && !(await etatBalayage(p)).balaye,
      '13 selection:cachee : lignes retirées, sélection de nouveau opaque pour la suivante',
      JSON.stringify(cachee),
    );
    // Animations réduites : la teinte seule, puis les lignes ensemble en fondu.
    id = await montrerTexte(p);
    await glisser(p, [400, 100], [600, 300]);
    await p.evaluate((i) => {
      __emettre('selection:balayer', {
        id: i,
        x: 400,
        y: 100,
        largeur: 201,
        hauteur: 201,
        reduites: true,
      });
    }, id);
    const reduit = await etatBalayage(p);
    await surligner(p, {
      id,
      lignes: [
        { x: 410, y: 110, largeur: 100, hauteur: 20 },
        { x: 410, y: 140, largeur: 60, hauteur: 20 },
      ],
      reduites: true,
      trace: 120,
      pas: 0,
      tenue: 220,
      sortie: 120,
    });
    const reduitS = await etatSurligne(p);
    verifier(
      reduit.balaye &&
        !reduit.bande &&
        reduit.anims.length === 0 &&
        reduit.teinte !== 'rgba(0, 0, 0, 0)' &&
        JSON.stringify(reduitS.anims) ===
          JSON.stringify([
            ['parait', 120, 0],
            ['parait', 120, 0],
          ]) &&
        JSON.stringify(reduitS.sortie) === JSON.stringify([['part', 120, 340]]),
      '13 animations réduites : teinte immobile, lignes ensemble en fondu de 120 ms, sortie de 120 ms',
      JSON.stringify({ reduit, reduitS }),
    );
    // Échelle 1,5 : les bandes débordent de 3 et 2 pixels réels.
    await p.evaluate(() => __emettre('selection:cachee', {}));
    await p.context().close();
    p = await page({ echelle: 1.5 });
    id = await montrerTexte(p);
    await glisser(p, [100, 100], [300, 220]);
    await p.evaluate((i) => {
      __emettre('selection:balayer', { id: i, x: 150, y: 150, largeur: 301, hauteur: 181, reduites: false });
    }, id);
    await surligner(p, { id, lignes: [{ x: 160, y: 170, largeur: 200, hauteur: 21 }], reduites: false, ...DEROULE });
    const k15 = await etatSurligne(p);
    verifier(
      JSON.stringify(k15.lignes) === JSON.stringify([['7px', '18px', '206px', '25px']]) &&
        JSON.stringify(k15.anims) === JSON.stringify([['tire', 260, 0]]) &&
        JSON.stringify(k15.sortie) === JSON.stringify([['part', 140, 480]]),
      '13 surlignage à 150 % : débord de 3 et 2 pixels réels, une ligne sans pas',
      JSON.stringify(k15),
    );
    // Échap en mode texte : une annulation, comme une zone.
    await p.evaluate(() => __emettre('selection:cachee', {}));
    id = await montrerTexte(p);
    await p.keyboard.press('Escape');
    await p.waitForTimeout(40);
    a = await appels(p);
    verifier(
      JSON.stringify(a) ===
        JSON.stringify([['selection_annulee', { id, numero: id, cause: 'echap' }]]),
      '13 mode texte : Échap annule une fois',
      JSON.stringify(a),
    );
    // Retour au mode zone : plus de nom.
    id = await montrer(p);
    const nomRetour = await nomDe();
    verifier(
      nomRetour[0] === null && nomRetour[1] === null,
      '13 retour au mode zone : plus de nom',
      JSON.stringify(nomRetour),
    );
    await p.context().close();
    // Mode texte sous la CSP de l'app, en sombre : aucun refus.
    p = await page({ csp: true, theme: 'dark' });
    id = await montrerTexte(p);
    await glisser(p, [100, 100], [200, 200]);
    await p.evaluate(
      (i) =>
        __emettre('selection:balayer', {
          id: i,
          x: 100,
          y: 100,
          largeur: 101,
          hauteur: 101,
          reduites: false,
        }),
      id,
    );
    await surligner(p, {
      id,
      lignes: [{ x: 110, y: 110, largeur: 60, hauteur: 12 }],
      reduites: false,
      ...DEROULE,
    });
    await p.waitForTimeout(50);
    const refusT = await p.evaluate(() => window.__refus);
    const bt = await etatBalayage(p);
    const st = await etatSurligne(p);
    verifier(
      refusT.length === 0 && bt.balaye && st.surligne && st.lignes.length === 1 && st.anims.length === 1,
      "13 mode texte, sombre, CSP de l'app : balayage et surlignage sans refus",
      JSON.stringify({ refusT, bt, st }),
    );
    await p.context().close();
    // Le filet du balayage seul garde la marge du mode texte (2 s) : le gel peut durer 1,5 s
    // après le relâcher et le surlignage arriver juste après. Puis la page redevient utilisable.
    p = await page();
    const balayerIci = (i, x = 100, y = 100, l = 201, h = 121) =>
      p.evaluate(
        ([i, x, y, l, h]) => {
          __emettre('selection:balayer', { id: i, x, y, largeur: l, hauteur: h, reduites: false });
          return performance.now();
        },
        [i, x, y, l, h],
      );
    const jusqua = async (t, ms) =>
      p.waitForTimeout(Math.max(0, ms - (await p.evaluate((t) => performance.now() - t, t))));
    id = await montrerTexte(p);
    await glisser(p, [100, 100], [300, 220]);
    await vider(p);
    const tb = await balayerIci(id);
    await jusqua(tb, 1800);
    const a1800 = await etatBalayage(p);
    await jusqua(tb, 2300);
    const a2300 = await etatBalayage(p);
    await surligner(p, { id, lignes: LIGNES, reduites: false, ...DEROULE });
    const tard = await etatSurligne(p);
    const envoyesFilet = await appels(p);
    await glisser(p, [400, 100], [600, 250]);
    const reprise = await termine(p);
    verifier(
      a1800.glisse &&
        a1800.balaye &&
        !a2300.glisse &&
        !a2300.balaye &&
        !tard.surligne &&
        tard.lignes.length === 0 &&
        envoyesFilet.length === 0 &&
        reprise.length === 1,
      '13 filet du balayage : sélection encore là 1,8 s après, au repos à 2,3 s, rien envoyé ; surlignage tardif ignoré, la page reprend',
      JSON.stringify({ a1800, a2300, tard: tard.lignes.length, envoyesFilet, reprise }),
    );
    // Tout le surlignage tient en 2 s, ou rien : 400 lignes à 2 s d'écart, 4 × 2 s, 2 001 ms.
    await p.evaluate(() => __emettre('selection:cachee', {}));
    id = await montrerTexte(p);
    await glisser(p, [100, 100], [300, 220]);
    await vider(p);
    await balayerIci(id);
    const CINQ_CENTS = Array.from({ length: 500 }, (_, k) => ({ x: 110, y: 100 + (k % 120), largeur: 50, hauteur: 1 }));
    await surligner(p, { id, lignes: CINQ_CENTS.slice(0, 400), reduites: false, trace: 260, pas: 2000, tenue: 220, sortie: 140 });
    await surligner(p, { id, lignes: [LIGNES[0]], reduites: false, trace: 2000, pas: 2000, tenue: 2000, sortie: 2000 });
    await surligner(p, { id, lignes: [LIGNES[0]], reduites: false, trace: 1000, pas: 0, tenue: 861, sortie: 140 });
    const tropLong = await etatSurligne(p);
    await surligner(p, { id, lignes: [LIGNES[0]], reduites: false, trace: 1000, pas: 0, tenue: 860, sortie: 140 });
    const juste = await etatSurligne(p);
    verifier(
      !tropLong.surligne &&
        tropLong.lignes.length === 0 &&
        juste.surligne &&
        juste.lignes.length === 1 &&
        JSON.stringify(juste.sortie) === JSON.stringify([['part', 140, 1860]]),
      "13 surlignage borné : refusé au-delà de 2 s en tout (400 lignes à 2 s d'écart, 4 × 2 s, 2 001 ms), accepté à 2 000 ms",
      JSON.stringify({ tropLong, juste }),
    );
    // 500 lignes : 400 bandes au plus, et la sortie attend la dernière (260 + 399 × 1 + 220).
    await p.evaluate(() => __emettre('selection:cachee', {}));
    id = await montrerTexte(p);
    await glisser(p, [100, 100], [300, 220]);
    await vider(p);
    await balayerIci(id);
    await surligner(p, { id, lignes: CINQ_CENTS, reduites: false, trace: 260, pas: 1, tenue: 220, sortie: 140 });
    const bandes = () =>
      p.evaluate(() => {
        const l = [...document.querySelectorAll('#zone .lus i')];
        const t = (a) => [a.animationName, a.effect.getTiming().duration, a.effect.getTiming().delay];
        return {
          surligne: document.body.classList.contains('surligne'),
          n: l.length,
          derniere: l.length ? l[l.length - 1].getAnimations().map(t) : [],
          sortie: document.getElementById('scene').getAnimations().map(t),
        };
      });
    const b500 = await bandes();
    verifier(
      b500.surligne &&
        b500.n === 400 &&
        JSON.stringify(b500.derniere) === JSON.stringify([['tire', 260, 399]]) &&
        JSON.stringify(b500.sortie) === JSON.stringify([['part', 140, 879]]),
      '13 surlignage de 500 lignes : 400 bandes, la dernière tirée à 399 ms, sortie après 879 ms',
      JSON.stringify(b500),
    );
    // Pendant le surlignage, Échap, un clic gauche ou droit ne font rien : il continue.
    await p.keyboard.press('Escape');
    await p.mouse.click(700, 200);
    await p.mouse.click(700, 200, { button: 'right' });
    await p.waitForTimeout(40);
    const pendantS = await bandes();
    const envoyesS = await appels(p);
    verifier(
      envoyesS.length === 0 && pendantS.surligne && pendantS.n === 400 && pendantS.sortie.length === 1,
      '13 pendant le surlignage : Échap, clic gauche, clic droit, rien envoyé ; le surlignage continue',
      JSON.stringify({ envoyesS, pendantS }),
    );
    await p.context().close();
  }
  {
    // 14. Mode défilante (0.6, §11, §12, D2, D3, §18) : le tracé du mode zone, plus le contour
    // du mode fenêtre (--window-highlight, 2 px) sur les pages proposées, qui arrivent après
    // par une charge `suite` (même id, même numéro), ou jamais. Clic sur une page :
    // selection_cliquee ; clic sur rien : rien ; glisser : selection_terminee. Après l'envoi,
    // la zone ou la page reste montrée (le cœur garde la fenêtre).
    const ND = 'Capture défilante : sélectionnez une zone ou une page';
    const chargeD = (id, o = {}) => ({ id, numero: o.numero || id, largeur: o.R || L, hauteur: o.RH || H, ecrans: o.ecrans || [{ x: 0, y: 0, largeur: o.R || L, hauteur: o.RH || H }], cote_min: o.coteMin || 3, mode: 'defilante', fenetres: o.fenetres || [], souris: o.souris || null, ...(o.suite === undefined ? {} : { suite: o.suite }) });
    async function montrerDefilante(p, o = {}) {
      const id = prochain++;
      const k = p.echelle;
      await p.evaluate(m => __emettre('selection:montrer', m), chargeD(id, { R: Math.round(L * k), RH: Math.round(H * k), ...o }));
      await attendrePrete(p, id);
      await p.waitForTimeout(100);
      await vider(p);
      return id;
    }
    const proposer = (p, id, fenetres, o = {}) => p.evaluate(m => __emettre('selection:montrer', m), chargeD(id, { R: Math.round(L * p.echelle), RH: Math.round(H * p.echelle), fenetres, suite: true, ...o }));
    const nomD = (p) => p.evaluate(() => [document.body.getAttribute('role'), document.body.getAttribute('aria-label')]);
    const etats = (p) => p.evaluate(() => ({ entoure: document.body.classList.contains('entoure'), glisse: document.body.classList.contains('glisse') }));
    const PAGE = { x: 200, y: 80, largeur: 500, hauteur: 250, rayon: 0 };
    let a;

    // 14 a. Nom accessible, curseur en croix ; sans liste : voile partout, rien d'entouré.
    p = await page();
    id = await montrerDefilante(p);
    const nom = await nomD(p);
    const curseur = await p.evaluate(() => getComputedStyle(document.documentElement).cursor);
    await p.mouse.move(400, 200); await p.waitForTimeout(60);
    ef = await etatFenetre(p);
    px = await ligne(p, 200);
    verifier(nom[0] === 'application' && nom[1] === ND && curseur === 'crosshair', '14 a nom accessible « ' + ND + ' », curseur en croix', JSON.stringify({ nom, curseur }));
    verifier(!ef.entoure && px.every(c => proche(c, VOILE)) && (await appels(p)).length === 0, '14 a sans liste : rien d\'entouré, voile partout, rien envoyé', JSON.stringify(ef));
    const texteD = await p.evaluate(() => document.body.innerText.trim());
    verifier(texteD === '', '14 a aucun texte montré', texteD);

    // 14 b. Liste arrivée après le premier mouvement : la page sous la pointe s'entoure
    // aussitôt, sans autre mouvement, sans retour au repos (aucun selection_prete).
    await proposer(p, id, [PAGE]); await p.waitForTimeout(80);
    ef = await etatFenetre(p);
    a = await appels(p);
    const an = await analyser(p, [[199, 200], [200, 200], [201, 200], [202, 200], [400, 79], [400, 80], [400, 81], [400, 82]], { x: 200, y: 80, l: 500, h: 250 }, 0);
    const Cb = an.couleurs;
    verifier(ef.entoure && ef.x === 200 && ef.y === 80 && ef.l === 500 && ef.h === 250 && a.length === 0,
      '14 b liste arrivée après le premier mouvement : page entourée aussitôt, sans nouvelle mesure', JSON.stringify({ ef, a }));
    verifier(proche(Cb[0], VOILE) && proche(Cb[1], BLEU) && proche(Cb[2], BLEU) && proche(Cb[3], FOND) && proche(Cb[4], VOILE) && proche(Cb[5], BLEU) && proche(Cb[6], BLEU) && proche(Cb[7], FOND)
      && an.vu.x === 200 && an.vu.y === 80 && an.vu.l === 500 && an.vu.h === 250 && an.autres === 0 && an.sombresDedans === 0,
      '14 b contour du mode fenêtre : trait bleu de 2 px à l\'intérieur, page claire, voile autour', JSON.stringify({ Cb, vu: an.vu, autres: an.autres }));
    await p.screenshot({ path: `${SORTIE}/selection-defilante-page.png` });
    // Le contour suit la pointe ; hors de la page, rien.
    await p.mouse.move(900, 360); await p.waitForTimeout(60);
    ef = await etatFenetre(p);
    px = await ligne(p, 360);
    verifier(!ef.entoure && px.every(c => proche(c, VOILE)), '14 b hors de la page : rien d\'entouré, voile partout', JSON.stringify(ef));

    // 14 c. Clic sur rien : rien, on continue. Clic sur la page : selection_cliquee une fois,
    // bon pixel ; la page reste entourée après le filet ordinaire (le cœur garde la fenêtre).
    await p.mouse.click(900, 360); await p.waitForTimeout(60);
    verifier((await appels(p)).length === 0, '14 c clic hors de toute page : rien envoyé');
    await p.mouse.click(301.4, 170.6); await p.waitForTimeout(60);
    await p.mouse.click(310, 180); await p.keyboard.press('Escape'); await p.waitForTimeout(40);
    a = await appels(p);
    verifier(JSON.stringify(a) === JSON.stringify([['selection_cliquee', { id, numero: id, x: 301, y: 170 }]]), '14 c clic sur la page : selection_cliquee une seule fois, au pixel sous la pointe', JSON.stringify(a));
    await p.waitForTimeout(1800);
    ef = await etatFenetre(p);
    verifier(ef.entoure && ef.x === 200 && ef.l === 500 && (await appels(p)).length === 1, '14 c après le clic : la page reste entourée au-delà de 1,5 s, plus rien ne part', JSON.stringify(ef));
    await p.evaluate(() => __emettre('selection:cachee', {})); await p.waitForTimeout(40);
    ef = await etatFenetre(p);
    verifier(!ef.entoure, '14 c selection:cachee : plus de contour', JSON.stringify(ef));

    // 14 d. Tracé : sur une page comme ailleurs, glisser trace une zone (contour effacé
    // pendant le tracé, trait blanc), selection_terminee comme le mode zone ; la zone reste.
    id = await montrerDefilante(p, { fenetres: [PAGE] });
    await p.mouse.move(250, 120); await p.mouse.down(); await p.mouse.move(450, 260, { steps: 6 }); await p.waitForTimeout(60);
    const pendant = await etats(p);
    px = await ligne(p, 200);
    const traitZone = proche(px[249], VOILE) && proche(px[250], [255, 255, 255], 0) && proche(px[251], FOND) && proche(px[450], [255, 255, 255], 0) && proche(px[451], VOILE) && proche(px[200], VOILE);
    await p.mouse.up(); await p.waitForTimeout(60);
    a = await appels(p);
    verifier(pendant.glisse && !pendant.entoure && traitZone, '14 d pendant le tracé : la zone et son trait, plus de contour de page', JSON.stringify({ pendant, bord: [px[200], px[249], px[250], px[251], px[450], px[451]] }));
    verifier(JSON.stringify(a) === JSON.stringify([['selection_terminee', { id, numero: id, depart: { x: 250, y: 120 }, arrivee: { x: 450, y: 260 } }]]), '14 d relâché : selection_terminee, coins exacts', JSON.stringify(a));
    await p.waitForTimeout(1800);
    z = await etatZone(p);
    verifier(z.glisse && z.x === '250px' && z.l === '201px' && (await appels(p)).length === 1, '14 d après le tracé : la zone reste au-delà de 1,5 s', JSON.stringify(z));
    await p.evaluate(() => __emettre('selection:cachee', {}));
    // Maj : carré, comme le mode zone.
    id = await montrerDefilante(p, { fenetres: [PAGE] });
    await p.keyboard.down('Shift'); await glisser(p, [100, 100], [300, 180]); await p.keyboard.up('Shift');
    a = await appels(p);
    verifier(a.length === 1 && a[0][0] === 'selection_terminee' && JSON.stringify(a[0][1].arrivee) === JSON.stringify({ x: 180, y: 180 }), '14 d Maj : carré', JSON.stringify(a));
    await p.evaluate(() => __emettre('selection:cachee', {}));
    await p.context().close();

    // 14 e. Liste jamais arrivée : le clic ne vise rien, la sélection continue ; puis on trace.
    p = await page();
    id = await montrerDefilante(p);
    await p.mouse.click(400, 200); await p.waitForTimeout(60);
    const apresClic = await appels(p);
    ef = await etatFenetre(p);
    await glisser(p, [100, 100], [300, 250]);
    a = await appels(p);
    verifier(apresClic.length === 0 && !ef.entoure && JSON.stringify(a) === JSON.stringify([['selection_terminee', { id, numero: id, depart: { x: 100, y: 100 }, arrivee: { x: 300, y: 250 } }]]),
      '14 e liste jamais arrivée : clic sans effet, puis le tracé part', JSON.stringify({ apresClic, a }));
    await p.evaluate(() => __emettre('selection:cachee', {}));
    // Liste arrivée pendant un tracé : le geste continue, même numéro.
    id = await montrerDefilante(p);
    await p.mouse.move(100, 100); await p.mouse.down(); await p.mouse.move(200, 200, { steps: 3 });
    await proposer(p, id, [PAGE]); await p.waitForTimeout(40);
    await p.mouse.move(300, 250, { steps: 3 }); await p.waitForTimeout(40);
    const pendantListe = await etats(p);
    await p.mouse.up(); await p.waitForTimeout(40);
    a = await appels(p);
    verifier(pendantListe.glisse && !pendantListe.entoure && JSON.stringify(a) === JSON.stringify([['selection_terminee', { id, numero: id, depart: { x: 100, y: 100 }, arrivee: { x: 300, y: 250 } }]]),
      '14 e liste arrivée pendant le tracé : le tracé continue et part, même numéro', JSON.stringify({ pendantListe, a }));
    await p.evaluate(() => __emettre('selection:cachee', {}));
    // Liste arrivée pendant l'appui d'un clic : le clic vise la page arrivée.
    id = await montrerDefilante(p);
    await p.mouse.move(300, 200); await p.mouse.down();
    await proposer(p, id, [PAGE]); await p.waitForTimeout(60);
    const sousAppui = await etatFenetre(p);
    await p.mouse.up(); await p.waitForTimeout(40);
    a = await appels(p);
    verifier(sousAppui.entoure && JSON.stringify(a) === JSON.stringify([['selection_cliquee', { id, numero: id, x: 300, y: 200 }]]), '14 e liste arrivée pendant l\'appui : page entourée, le clic la choisit', JSON.stringify({ sousAppui, a }));
    await p.evaluate(() => __emettre('selection:cachee', {}));
    // Position de la souris dans la charge des pages, aucun mouvement depuis la charge.
    await p.mouse.move(5, 5);
    id = await montrerDefilante(p);
    await proposer(p, id, [PAGE], { souris: { x: 300, y: 300 } }); await p.waitForTimeout(60);
    ef = await etatFenetre(p);
    verifier(ef.entoure && ef.x === 200, '14 e souris de la charge des pages : page entourée avant tout mouvement', JSON.stringify(ef));
    await p.evaluate(() => __emettre('selection:cachee', {}));
    await p.context().close();

    // 14 f. Charges des pages qui ne sont pas une suite de l'affichage montré : une charge
    // ordinaire (retour au repos, nouvelle mesure) ; `suite` invalide : ignorée.
    p = await page();
    id = await montrerDefilante(p);
    await proposer(p, id, [PAGE], { numero: id + 1000 }); await attendrePrete(p, id); await p.waitForTimeout(60);
    a = await appels(p);
    verifier(a.length === 1 && a[0][0] === 'selection_prete' && a[0][1].numero === id + 1000, '14 f numéro différent : charge ordinaire, la page mesure de nouveau', JSON.stringify(a));
    await vider(p);
    await p.evaluate(m => __emettre('selection:montrer', m), chargeD(id, { fenetres: [], suite: 'oui' })); await p.waitForTimeout(80);
    await p.mouse.move(300, 200); await p.waitForTimeout(60);
    ef = await etatFenetre(p);
    verifier(ef.entoure && (await appels(p)).length === 0, '14 f suite invalide : demande ignorée, la liste reste', JSON.stringify(ef));
    // Charge d'un autre mode après une défilante : plus de contour ni de nom.
    id = await montrer(p);
    ef = await etatFenetre(p);
    const nomZ = await nomD(p);
    verifier(!ef.entoure && nomZ[0] === null && nomZ[1] === null, '14 f charge de mode zone ensuite : plus de contour, plus de nom', JSON.stringify({ ef, nomZ }));
    await p.context().close();

    // 14 g. Seuil entre clic et tracé : celui du mode fenêtre (max(cote_min, round(4 k))).
    for (const e of [1, 1.25]) {
      const S = Math.max(3, Math.round(4 * e));
      for (const [d, clic] of [[S - 1, true], [S, false]]) {
        p = await page({ echelle: e });
        id = await montrerDefilante(p, { fenetres: [{ x: 0, y: 0, largeur: Math.round(L * e), hauteur: Math.round(H * e), rayon: 0 }] });
        await p.mouse.move(400.5 / e, 200.5 / e); await p.mouse.down();
        await p.mouse.move((400.5 + d) / e, 200.5 / e, { steps: 3 }); await p.mouse.up(); await p.waitForTimeout(40);
        a = await appels(p);
        // Au seuil, c'est un tracé d'un pixel de haut : sous cote_min, rien (on continue).
        const ok = clic ? JSON.stringify(a) === JSON.stringify([['selection_cliquee', { id, numero: id, x: 400 + d, y: 200 }]]) : a.length === 0;
        verifier(ok, `14 g k = ${e} : déplacement de ${d} pixels réels (seuil ${S}) : ${clic ? 'clic' : 'tracé trop fin, rien'}`, JSON.stringify(a));
        if (!clic) {
          ef = await etatFenetre(p);
          verifier(ef.entoure, `14 g k = ${e} : tracé refusé : de nouveau le contour sous la pointe`, JSON.stringify(ef));
        }
        await p.context().close();
      }
    }

    // 14 h. Clic droit et Échap : une annulation ; envoi refusé : la page redevient utilisable.
    p = await page();
    id = await montrerDefilante(p, { fenetres: [PAGE] });
    await p.mouse.click(300, 200, { button: 'right' }); await p.mouse.click(300, 200); await p.waitForTimeout(40);
    a = await appels(p);
    verifier(JSON.stringify(a) === JSON.stringify([['selection_annulee', { id, numero: id, cause: 'clic_droit' }]]), '14 h clic droit : annulation, plus rien ensuite', JSON.stringify(a));
    id = await montrerDefilante(p, { fenetres: [PAGE] });
    await p.keyboard.press('Escape'); await p.waitForTimeout(40);
    a = await appels(p);
    verifier(JSON.stringify(a) === JSON.stringify([['selection_annulee', { id, numero: id, cause: 'echap' }]]), '14 h Échap : annulation', JSON.stringify(a));
    id = await montrerDefilante(p, { fenetres: [PAGE] });
    await p.evaluate(() => __rejeter.add('selection_cliquee'));
    await p.mouse.click(300, 200); await p.waitForTimeout(80);
    await p.evaluate(() => __rejeter.clear());
    await vider(p);
    await p.mouse.move(310, 210); await p.waitForTimeout(40);
    await p.mouse.click(310, 210); await p.waitForTimeout(40);
    a = await appels(p);
    verifier(JSON.stringify(a) === JSON.stringify([['selection_cliquee', { id, numero: id, x: 310, y: 210 }]]), '14 h envoi refusé : la page redevient utilisable, le clic suivant part', JSON.stringify(a));
    await p.context().close();

    // 14 i. Contour au pixel près à 150 % (trait de max(2, round(2 k)) pixels réels) et clic.
    p = await page({ echelle: 1.5 });
    const W15 = { x: 300, y: 120, largeur: 750, hauteur: 375, rayon: 0 };
    id = await montrerDefilante(p);
    await p.mouse.move(400.3, 200.6); await p.waitForTimeout(40);
    await proposer(p, id, [W15]); await p.waitForTimeout(80);
    const a15 = await analyser(p, [[299, 300], [300, 300], [302, 300], [303, 300]], { x: 300, y: 120, l: 750, h: 375 }, 0);
    const C15 = a15.couleurs;
    verifier(proche(C15[0], VOILE) && proche(C15[1], BLEU) && proche(C15[2], BLEU) && proche(C15[3], FOND) && a15.vu.x === 300 && a15.vu.l === 750 && a15.autres === 0 && a15.sombresDedans === 0,
      '14 i k = 1,5 : contour de 3 pixels réels au pixel près, page claire', JSON.stringify({ C15, vu: a15.vu }));
    await p.screenshot({ path: `${SORTIE}/selection-defilante-150.png` });
    await p.mouse.click(400.3, 200.6); await p.waitForTimeout(40);
    a = await appels(p);
    verifier(JSON.stringify(a) === JSON.stringify([['selection_cliquee', { id, numero: id, x: 600, y: 300 }]]), '14 i k = 1,5 : clic au pixel réel sous la pointe', JSON.stringify(a));
    await p.context().close();

    // 14 j. Sous la CSP de l'app, en sombre : aucun refus, contour et tracé.
    p = await page({ csp: true, theme: 'dark' });
    id = await montrerDefilante(p);
    await p.mouse.move(300, 200); await p.waitForTimeout(40);
    await proposer(p, id, [PAGE]); await p.waitForTimeout(80);
    ef = await etatFenetre(p);
    await glisser(p, [100, 100], [300, 250]);
    a = await appels(p);
    const refusD = await p.evaluate(() => window.__refus);
    verifier(refusD.length === 0 && ef.entoure && a.length === 1 && a[0][0] === 'selection_terminee', '14 j CSP de l\'app, sombre : aucun refus, contour puis tracé', JSON.stringify({ refusD, ef, a }));
    await p.context().close();
  }
  console.log('ECHECS :', echecs);
  console.log('ERREURS PAGE :', JSON.stringify(erreurs));
  if (erreurs.length) echecs++;
  await nav.close(); serveur.close();
  process.exitCode = echecs === 0 ? 0 : 1;
})().catch((e) => { console.error('essai interrompu :', e); process.exit(1); });
