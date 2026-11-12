const { chromium } = require('playwright');
const http = require('http'), fs = require('fs'), path = require('path');
// Essai du décompte du retardateur dans la page du bandeau (Chromium, Playwright), contrat 0.4
// §4.3 et §11.2. Page servie sous la CSP de l'app, `__TAURI__` simulé, horloge de Playwright
// pour avancer le temps. Aucune attente fixe : l'événement simulé est traité pendant l'appel
// (écouteur synchrone), et le temps n'avance que par l'horloge. Sort avec 1 si une
// vérification échoue. Captures dans $SORTIE (par défaut essais-resultats/pages).
//   node essais/pages/decompte.cjs
const RACINE = path.join(__dirname, '..', '..', 'app', 'interface');
const SORTIE = process.env.SORTIE || path.join(__dirname, '..', '..', 'essais-resultats', 'pages');
fs.mkdirSync(SORTIE, { recursive: true });
const CSP = Object.entries(JSON.parse(fs.readFileSync(path.join(__dirname, '..', '..', 'app', 'coeur', 'tauri.conf.json'), 'utf8')).app.security.csp)
  .map(([k, v]) => `${k} ${v}`).join('; ');
const types = { '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript' };
const serveur = http.createServer((q, r) => {
  const p = path.join(RACINE, decodeURIComponent(new URL(q.url, 'http://x').pathname));
  if (!p.startsWith(RACINE) || !fs.existsSync(p)) { r.writeHead(404); return r.end(); }
  r.writeHead(200, { 'content-type': types[path.extname(p)] || 'text/plain', 'content-security-policy': CSP });
  fs.createReadStream(p).pipe(r);
});
const FONDS = { light: 'linear-gradient(135deg,#DCE7F6,#C6D5EE)', dark: 'linear-gradient(135deg,#0B1830,#141A36)' };
const ANNEAU = 2 * Math.PI * 48;
const D = (o = {}) => ({ id: 5, secondes: 3, restant_ms: 3000, diametre: 80, reduites: false, ...o });

(async () => {
  await new Promise(ok => serveur.listen(0, ok));
  const base = `http://127.0.0.1:${serveur.address().port}/vignette/bandeau.html`;
  const nav = await chromium.launch();
  let echecs = 0;
  const verifier = (ok, quoi, info = '') => { console.log(ok ? 'OK   ' : 'ECHEC', quoi, info); if (!ok) echecs++; };

  /** Page du bandeau, Tauri simulé, horloge de Playwright posée avant le chargement. */
  async function page(theme = 'light', opts = {}) {
    const ctx = await nav.newContext({ viewport: { width: 560, height: 220 }, deviceScaleFactor: 2, colorScheme: theme,
      reducedMotion: opts.reduit ? 'reduce' : 'no-preference' });
    const p = await ctx.newPage();
    p.avert = []; p.erreurs = [];
    p.on('pageerror', e => p.erreurs.push(String(e)));
    p.on('console', m => { if (m.type() === 'warning') p.avert.push(m.text()); if (m.type() === 'error') p.erreurs.push(m.text()); });
    await p.clock.install({ time: 0 });
    await p.addInitScript(() => {
      window.__appels = []; window.__refus = []; const ecouteurs = {};
      document.addEventListener('securitypolicyviolation', e => window.__refus.push(e.violatedDirective));
      window.__TAURI__ = { core: { invoke: (c, a) => { window.__appels.push([c, a]); return Promise.resolve(); } },
        event: { listen: (n, f) => { ecouteurs[n] = f; return Promise.resolve(() => {}); } } };
      // Écouteur appelé tout de suite : au retour de l'appel, la page a traité l'événement.
      window.__emettre = (nom, payload) => { if (!ecouteurs[nom]) throw new Error(`pas d'écoute ${nom}`); ecouteurs[nom]({ payload }); };
      window.__chiffres = [];
      new MutationObserver(() => {
        const t = document.querySelector('#decompte .chiffre')?.textContent;
        if (t && window.__chiffres[window.__chiffres.length - 1] !== t) window.__chiffres.push(t);
      }).observe(document, { subtree: true, childList: true, characterData: true });
    });
    await p.goto(base);
    // Signe que la page écoute : son unique bandeau_pret.
    for (let i = 0; i < 200 && !(await p.evaluate(() => __appels.some(a => a[0] === 'bandeau_pret'))); i++) {
      await new Promise(ok => setTimeout(ok, 10));
    }
    // Le temps de la page ne passe plus que par runFor.
    await p.clock.pauseAt(1000);
    return p;
  }
  const appels = (p, c) => p.evaluate(c => __appels.filter(a => a[0] === c), c);
  const etat = (p) => p.evaluate(() => {
    const r = document.getElementById('decompte'), prg = r.querySelector('.progression');
    const d = r.querySelector('.disque'); // rectangle de mise en page (sans l'échelle de l'entrée)
    return { visible: !r.hidden, chiffre: r.querySelector('.chiffre').textContent, decalage: parseFloat(prg.style.strokeDashoffset),
      disque: [d.offsetWidth, d.offsetHeight, d.offsetLeft + d.offsetWidth / 2, d.offsetTop + d.offsetHeight / 2],
      pilule: !document.getElementById('bandeau').hidden };
  });
  const emettre = (p, nom, charge) => p.evaluate(([n, c]) => __emettre(n, c), [nom, charge]);

  // 1. Chiffres, anneau, diamètre, zone de la croix.
  let p = await page();
  await emettre(p, 'bandeau:decompte', D());
  let e = await etat(p);
  verifier(e.visible && e.chiffre === '3' && Math.abs(e.decalage) < 0.5, '1 « 3 » au départ, anneau plein', JSON.stringify(e));
  verifier(e.disque[0] === 80 && e.disque[1] === 80 && e.disque[2] === 280 && e.disque[3] === 110,
    '1 diamètre reçu appliqué, disque centré dans la fenêtre', JSON.stringify(e.disque));
  const zones = await appels(p, 'bandeau_zone');
  // Rectangle visible de la croix, une fois l'entrée finie : c'est celui qu'a donné la page.
  await p.evaluate(() => document.getAnimations().forEach(a => a.finish()));
  const croix = await p.$eval('#decompte .annuler', b => { const r = b.getBoundingClientRect(); return { x: r.x, y: r.y, largeur: r.width, hauteur: r.height }; });
  const z = zones[0]?.[1];
  verifier(zones.length === 1 && z.id === 5 && ['x', 'y', 'largeur', 'hauteur'].every(k => Math.abs(z[k] - croix[k]) < 0.01)
    && z.x >= 0 && z.y >= 0 && z.x + z.largeur <= 560 && z.y + z.hauteur <= 220,
    '1 bandeau_zone une fois, rectangle de la croix, dans la fenêtre', JSON.stringify([zones, croix]));
  await p.clock.runFor(500);
  e = await etat(p);
  verifier(e.chiffre === '3' && Math.abs(e.decalage + ANNEAU / 6) < 2, '1 à 0,5 s : « 3 », anneau à un sixième', JSON.stringify(e));
  await p.clock.runFor(498); // 0,998 s
  const a998 = (await etat(p)).chiffre;
  await p.clock.runFor(20); // 1,018 s
  const a1018 = await etat(p);
  verifier(a998 === '3' && a1018.chiffre === '2' && a1018.decalage < e.decalage, '1 « 2 » à la première seconde, l\'anneau avance', `${a998} ${JSON.stringify(a1018)}`);
  await p.clock.runFor(1000);
  e = await etat(p);
  verifier(e.chiffre === '1', '1 « 1 » à la deuxième seconde', JSON.stringify(e));
  await p.clock.runFor(1500); // 3,5 s : fini, le cœur cachera la fenêtre
  e = await etat(p);
  const vus = await p.evaluate(() => __chiffres);
  verifier(e.chiffre === '1' && JSON.stringify(vus) === '["3","2","1"]' && Math.abs(e.decalage + ANNEAU) < 0.5,
    '1 jamais « 0 », anneau vide à la fin', JSON.stringify([vus, e.decalage]));
  verifier((await appels(p, 'bandeau_zone')).length === 1, '1 bandeau_zone envoyé une seule fois');
  // Photos à 3 et à 1 : animations finies d'office (pas d'attente).
  for (const theme of ['light', 'dark']) {
    const q = await page(theme);
    // Fond par le CSSOM (la CSP refuse une feuille en ligne).
    await q.evaluate(f => { document.documentElement.style.background = f; document.documentElement.style.minHeight = '100%'; }, FONDS[theme]);
    await emettre(q, 'bandeau:decompte', D({ diametre: 88 }));
    await q.evaluate(() => document.getAnimations().forEach(a => a.finish()));
    await q.screenshot({ path: `${SORTIE}/decompte-3-${theme === 'light' ? 'clair' : 'sombre'}.png` });
    if (theme === 'light') {
      await q.clock.runFor(2400);
      await q.evaluate(() => document.getAnimations().forEach(a => a.finish()));
      await q.screenshot({ path: `${SORTIE}/decompte-1-clair.png` });
    }
    await q.context().close();
  }
  console.log('captures decompte-3-clair, decompte-3-sombre, decompte-1-clair');

  // 2. Clic sur le disque : rien ; clic sur la croix : une action « annuler », une seule.
  await p.context().close(); p = await page();
  await emettre(p, 'bandeau:decompte', D({ id: 6 }));
  await p.evaluate(() => document.getAnimations().forEach(a => a.finish()));
  await p.mouse.click(280, 110);
  verifier((await appels(p, 'bandeau_action')).length === 0, '2 clic sur le disque : rien');
  await p.click('#decompte .annuler');
  await p.click('#decompte .annuler', { force: true });
  const act = await appels(p, 'bandeau_action');
  verifier(JSON.stringify(act) === JSON.stringify([['bandeau_action', { id: 6, action: 'annuler' }]]), '2 croix : bandeau_action annuler une fois', JSON.stringify(act));
  verifier(await p.evaluate(() => document.activeElement === document.body), '2 focus jamais pris');
  const nom = await p.getByRole('button', { name: 'Annuler' }).count();
  const role = await p.$eval('#decompte', r => [r.getAttribute('role'), r.getAttribute('aria-live')]);
  verifier(nom === 1 && role[0] === 'timer' && role[1] === 'off', '2 nom accessible « Annuler », rôle timer, aria-live off', JSON.stringify([nom, role]));

  // 3. Un message efface le disque ; un décompte efface une pilule sans bandeau_termine.
  await emettre(p, 'bandeau:decompte', D({ id: 7 }));
  await emettre(p, 'bandeau:afficher', { id: 8, genre: 'erreur', titre: 'Impossible d\'enregistrer', detail: '', actions: [] });
  e = await etat(p);
  verifier(!e.visible && e.pilule, '3 bandeau:afficher efface le disque et montre la pilule', JSON.stringify(e));
  await emettre(p, 'bandeau:afficher', { id: 9, genre: 'reussite', titre: 'Copié dans le presse-papiers' });
  await emettre(p, 'bandeau:decompte', D({ id: 10 }));
  e = await etat(p);
  await p.clock.runFor(6000); // la réussite serait partie seule après 2,5 s
  const termines = await appels(p, 'bandeau_termine');
  verifier(e.visible && !e.pilule && termines.length === 0, '3 bandeau:decompte efface la pilule sans bandeau_termine', JSON.stringify([e, termines]));
  // 4. bandeau:decompte_cache : un autre id ne change rien ; le sien vide la page.
  await emettre(p, 'bandeau:decompte', D({ id: 11 }));
  await emettre(p, 'bandeau:decompte_cache', { id: 10 });
  const encore = (await etat(p)).visible;
  await emettre(p, 'bandeau:decompte_cache', { id: 11 });
  e = await etat(p);
  verifier(encore && !e.visible && !e.pilule, '4 bandeau:decompte_cache vide la page (le sien seulement)');
  // 5. Charges invalides refusées, avec console.warn ; rien ne s'affiche.
  const avant = p.avert.length;
  for (const c of [null, 'x', D({ id: undefined }), D({ secondes: 0 }), D({ secondes: 2.5 }), D({ restant_ms: -1 }),
    D({ restant_ms: NaN }), D({ diametre: 0 }), D({ diametre: 400 }), D({ diametre: '80' }), D({ reduites: 'oui' })]) {
    await emettre(p, 'bandeau:decompte', c);
  }
  e = await etat(p);
  verifier(!e.visible && p.avert.length - avant === 11, '5 charges invalides refusées (11 avertissements)', `${p.avert.length - avant}`);
  // Envoi tardif (page prête après la demande) : 1,2 s restantes, « 2 ».
  await emettre(p, 'bandeau:decompte', D({ id: 12, restant_ms: 1200 }));
  e = await etat(p);
  verifier(e.chiffre === '2' && Math.abs(e.decalage + ANNEAU * 1.8 / 3) < 1, '5 envoi tardif : chiffre et anneau du temps qui reste', JSON.stringify(e));
  const refus = await p.evaluate(() => __refus);
  verifier(refus.length === 0 && p.erreurs.length === 0, '5 CSP de l\'app respectée, aucune erreur de page', JSON.stringify([refus, p.erreurs]));
  await p.context().close();

  // 6. Animations : présentes d'ordinaire, aucune en mode réduit (préférence ou réglage du cœur).
  p = await page();
  await emettre(p, 'bandeau:decompte', D());
  const normales = await p.evaluate(() => document.getAnimations().length);
  verifier(normales >= 2, '6 entrée et chiffre animés d\'ordinaire', String(normales));
  await p.context().close();
  for (const [quoi, opts, charge] of [['préférence', { reduit: true }, D()], ['réglage du cœur', {}, D({ reduites: true })]]) {
    p = await page('light', opts);
    await emettre(p, 'bandeau:decompte', charge);
    const n0 = await p.evaluate(() => document.getAnimations().length);
    await p.clock.runFor(1100);
    await p.hover('#decompte .annuler');
    const n1 = await p.evaluate(() => document.getAnimations().length);
    e = await etat(p);
    const tr = await p.$eval('#decompte .annuler', b => [getComputedStyle(b).transform, getComputedStyle(b).transitionDuration]);
    verifier(n0 === 0 && n1 === 0 && e.chiffre === '2' && Math.abs(e.decalage + ANNEAU / 3) < 0.5 && tr[0] === 'none',
      `6 animations réduites (${quoi}) : aucune animation, l'anneau avance par cran`, JSON.stringify([n0, n1, e, tr]));
    await p.context().close();
  }

  console.log('ECHECS :', echecs);
  await nav.close(); serveur.close();
  process.exitCode = echecs === 0 ? 0 : 1;
})().catch((e) => { console.error('essai interrompu :', e); process.exit(1); });
