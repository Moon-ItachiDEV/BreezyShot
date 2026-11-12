const { chromium } = require('playwright');
const http = require('http'), fs = require('fs'), path = require('path');
// Essai de la page du souffle dans Chromium (Playwright) : flash et envol aux images clés,
// `souffle_termine` après la fin. Sort avec 1 si une vérification échoue.
//   node essais/pages/souffle.cjs
const RACINE = path.join(__dirname, '..', '..', 'app', 'interface');
const SORTIE = process.env.SORTIE || path.join(__dirname, '..', '..', 'essais-resultats', 'pages');
fs.mkdirSync(SORTIE, { recursive: true });
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
(async () => {
  await new Promise(ok => serveur.listen(0, ok));
  const base = `http://127.0.0.1:${serveur.address().port}/vignette/souffle.html?csp=1`;
  const nav = await chromium.launch();
  const erreurs = [];
  let echecs = 0;
  const verifier = (ok, quoi, info = '') => { console.log(ok ? 'OK   ' : 'ECHEC', quoi, info); if (!ok) echecs++; };
  async function page(o = {}) {
    const ctx = await nav.newContext({ viewport: { width: 600, height: 500 }, deviceScaleFactor: o.echelle || 1 });
    const p = await ctx.newPage();
    p.on('pageerror', e => erreurs.push(String(e)));
    p.on('console', m => { if (m.type() === 'error' || m.type() === 'warning') erreurs.push(m.text()); });
    await p.addInitScript((refus) => {
      window.__refus = []; document.addEventListener('securitypolicyviolation', e => window.__refus.push(e.violatedDirective));
      window.__appels = []; const ecoutes = {}; let refuses = 0;
      const toile = document.createElement('canvas'); toile.width = 80; toile.height = 50;
      const g = toile.getContext('2d'); g.fillStyle = '#0067C0'; g.fillRect(0, 0, 80, 50);
      const octets = Uint8Array.from(atob(toile.toDataURL('image/png').split(',')[1]), x => x.charCodeAt(0));
      window.__TAURI__ = {
        core: { invoke: (c, a) => {
          window.__appels.push([c, a]);
          // `refus` premiers « prêt » rejetés, comme un cœur pas encore en place.
          if (c === 'souffle_pret' && refuses < refus) { refuses++; return Promise.reject('vignette pas encore en place'); }
          return c === 'vignette_image' ? Promise.resolve(octets.buffer) : Promise.resolve();
        } },
        event: { listen: (n, f) => { ecoutes[n] = f; return Promise.resolve(() => {}); } },
      };
      window.__emettre = (n, payload) => ecoutes[n]({ payload });
    }, o.refus || 0);
    await p.goto(base);
    await p.waitForTimeout(50);
    return p;
  }
  const appels = (p, nom) => p.evaluate(n => window.__appels.filter(a => !n || a[0] === n), nom);

  // Pixels réels à 150 % : départ 600,300 300×150 réels → 400,200 200×100 CSS.
  const p = await page({ echelle: 1.5 });
  verifier(JSON.stringify(await appels(p)) === JSON.stringify([['souffle_pret', {}]]), 'souffle_pret une fois');
  {
    // Cœur pas encore en place (vu sous Windows) : deux refus, reçu au troisième essai.
    const avant = erreurs.length;
    const q = await page({ refus: 2 });
    await q.waitForTimeout(400);
    const n = (await appels(q, 'souffle_pret')).length;
    verifier(n === 3 && erreurs.length === avant, 'souffle_pret rejeté deux fois : retenté, reçu au troisième, sans bruit', `${n} appel(s)`);
    await q.context().close();
  }
  await p.evaluate(() => __emettre('souffle:jouer', { id: 3, depart: { x: 600, y: 300, largeur: 300, hauteur: 150 }, arrivee: { x: 18, y: 540, largeur: 390, hauteur: 240 } }));
  await p.waitForFunction(() => document.getAnimations().length >= 2, null, { timeout: 2000 }).catch(() => {});
  const a = await p.evaluate(() => {
    const [f, e] = [document.getElementById('flash'), document.getElementById('envol')];
    const an = (el) => el.getAnimations()[0];
    for (const x of document.getAnimations()) x.pause();
    const r = { flash: [an(f).effect.getTiming().duration, an(f).effect.getKeyframes().map(k => Number(k.opacity))],
      envol: [an(e).effect.getTiming().duration, an(e).effect.getTiming().easing], depart: [e.offsetLeft, e.offsetTop, e.offsetWidth, e.offsetHeight] };
    const k = an(e).effect.getKeyframes()[2]; r.milieu = [k.offset, k.transform];
    an(e).currentTime = 420; r.fin = getComputedStyle(e).transform; r.rayon = getComputedStyle(e).borderTopLeftRadius;
    an(e).currentTime = 0; an(f).currentTime = 0;
    for (const x of document.getAnimations()) x.play();
    return r;
  });
  verifier(JSON.stringify(a.flash) === JSON.stringify([180, [0.55, 0]]), 'flash 180 ms, 0,55 → 0', JSON.stringify(a.flash));
  verifier(JSON.stringify(a.envol) === JSON.stringify([420, 'cubic-bezier(0.5, 0, 0.2, 1)']), 'envol 420 ms courbe-envol', JSON.stringify(a.envol));
  verifier(JSON.stringify(a.depart) === JSON.stringify([400, 200, 200, 100]), 'départ sur la zone capturée (pixels réels → CSS)', JSON.stringify(a.depart));
  // Arrivée 12,360 260×160 CSS : translate(-388, 160), échelle 1,3 × 1,6 ; mi-chemin 40 px plus haut.
  const m = (t) => t.match(/matrix\(([^)]+)\)/)[1].split(',').map(Number);
  const [fa, , , fd, fx, fy] = m(a.fin);
  verifier(Math.abs(fa - 1.3) < 0.01 && Math.abs(fd - 1.6) < 0.01 && Math.abs(fx + 388) < 0.5 && Math.abs(fy - 160) < 0.5 && a.rayon === '8px',
    'fin sur l\'image de la carte, coins 8 px', a.fin + ' ' + a.rayon);
  // Mi-chemin : dx/2 = -194, dy/2 - 40 = 40 (l'image clé, avant la courbe).
  verifier(a.milieu[0] === 0.62 && /^translate\(-194px, 40px\)/.test(a.milieu[1]), 'à 62 % : mi-chemin, 40 px plus haut (arc)', JSON.stringify(a.milieu));
  await p.waitForTimeout(150);
  await p.screenshot({ path: `${SORTIE}/souffle-envol.png` });
  await p.waitForTimeout(500);
  verifier(JSON.stringify((await appels(p, 'souffle_termine')).map(x => x[1])) === JSON.stringify([{ id: 3 }]), 'souffle_termine une fois après la fin');
  verifier(await p.evaluate(() => !document.getElementById('envol').hidden), 'dernière image tenue jusqu\'à l\'effacement');
  await p.evaluate(() => __emettre('souffle:effacer', { id: 3 }));
  verifier(await p.evaluate(() => document.getElementById('envol').hidden && document.getElementById('flash').hidden), 'effacer : page vide');
  await p.evaluate(() => __emettre('souffle:jouer', { id: 4, depart: { x: 0, y: 0, largeur: 0, hauteur: 10 }, arrivee: {} }));
  await p.waitForTimeout(100);
  verifier((await appels(p, 'vignette_image')).length === 1, 'charge invalide refusée');
  erreurs.splice(erreurs.findIndex(e => e.includes('charge rejetée')), 1);
  verifier((await p.evaluate(() => window.__refus)).length === 0, 'CSP de l\'app respectée');
  await p.context().close();

  console.log('ECHECS :', echecs);
  console.log('ERREURS PAGE :', JSON.stringify(erreurs));
  if (erreurs.length) echecs++;
  await nav.close(); serveur.close();
  process.exitCode = echecs === 0 ? 0 : 1;
})().catch((e) => { console.error('essai interrompu :', e); process.exit(1); });
