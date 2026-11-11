const { chromium } = require('playwright');
const http = require('http'), fs = require('fs'), path = require('path');
// Essai de la page du bandeau dans Chromium (Playwright). Sort avec 1 si une vérification
// échoue. Captures dans $SORTIE (par défaut essais-resultats/pages).
//   node essais/pages/bandeau.cjs
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
const FONDS = { light: 'linear-gradient(135deg,#DCE7F6,#C6D5EE)', dark: 'linear-gradient(135deg,#0B1830,#141A36)' };
const LONG = { id: 'long', genre: 'erreur', titre: "Impossible d'enregistrer la capture dans le dossier choisi parce que le disque réseau ne répond plus depuis un moment",
  detail: 'Le dossier \\\\serveur-de-fichiers-du-bureau\\partages\\equipe-communication\\captures-ecran-2026\\novembre est introuvable ou protégé en écriture.',
  actions: [{ id: 'choisir', libelle: 'Choisir un dossier' }, { id: 'reessayer', libelle: 'Réessayer' }] };
const LONG_OK = { id: 'long-ok', genre: 'reussite', titre: 'Enregistré dans Images › BreezyShot › BreezyShot 2026-11-11 à 16.42.08 avec un nom vraiment très long.png' };
(async () => {
  await new Promise(ok => serveur.listen(0, ok));
  const base = `http://127.0.0.1:${serveur.address().port}/vignette/bandeau.html`;
  const nav = await chromium.launch();
  const erreurs = [];
  async function page(theme, opts = {}) {
    const ctx = await nav.newContext({ viewport: { width: opts.largeur || 560, height: opts.hauteur || 200 }, deviceScaleFactor: 2, colorScheme: theme, reducedMotion: opts.reduit ? 'reduce' : 'no-preference' });
    const p = await ctx.newPage();
    p.on('pageerror', e => erreurs.push(String(e)));
    p.on('console', m => { if (m.type() === 'error' || m.type() === 'warning') erreurs.push(m.text()); });
    if (opts.tauri) await p.addInitScript(() => {
      window.__appels = []; let ecouteur = null;
      window.__TAURI__ = { core: { invoke: (c, a) => { window.__appels.push([c, a]); return Promise.resolve(); } },
        event: { listen: (n, f) => { if (n === 'bandeau:afficher') ecouteur = f; return Promise.resolve(() => {}); } } };
      window.__emettre = (payload) => ecouteur({ payload });
    });
    return p;
  }
  async function capture(p, nom, theme, fond = true) {
    if (fond) await p.addStyleTag({ content: `html{min-height:100%;background:${FONDS[theme]} !important}` });
    await p.waitForTimeout(450);
    await p.screenshot({ path: `${SORTIE}/${nom}.png`, omitBackground: !fond });
    console.log('capture', nom);
  }
  for (const theme of ['light', 'dark']) {
    const t = theme === 'light' ? 'clair' : 'sombre';
    for (const d of ['reussite', 'info', 'erreur']) {
      const p = await page(theme); await p.goto(`${base}?demo=${d}&tenir`);
      await capture(p, `${d}${d === 'erreur' ? '-avec-action' : ''}-${t}`, theme); await p.context().close();
    }
    let p = await page(theme); await p.goto(base);
    await p.evaluate(() => bandeauApercu.afficher({ id: 1, genre: 'erreur', titre: 'Aucun raccourci disponible', detail: "Lance une capture depuis l'icône BreezyShot, près de l'horloge.", actions: [] }));
    await capture(p, `erreur-sans-action-${t}`, theme); await p.context().close();
  }
  // Textes longs, page étroite
  let p = await page('light', { largeur: 400, hauteur: 260 }); await p.goto(base);
  await p.evaluate(m => bandeauApercu.afficher(m), LONG); await capture(p, 'texte-long-erreur-clair', 'light');
  const r = await p.evaluate(() => { const b = document.getElementById('bandeau').getBoundingClientRect(); return [b.left, b.right, innerWidth, document.documentElement.scrollWidth]; });
  console.log('rect long', r);
  await p.evaluate(m => bandeauApercu.afficher(m), { ...LONG_OK, tenir: true }); await capture(p, 'texte-long-reussite-clair', 'light', false);
  await p.context().close();
  // Disparition : réussite réelle (2,5 s + 167 ms), puis page vide transparente
  p = await page('light'); await p.goto(`${base}?demo=reussite`); await p.waitForTimeout(3000);
  await p.screenshot({ path: `${SORTIE}/apres-disparition-transparent.png`, omitBackground: true });
  const png = await p.screenshot({ omitBackground: true });
  const visible = await p.evaluate(() => !document.getElementById('bandeau').hidden);
  console.log('apres disparition visible ?', visible, 'taille png', png.length);
  await p.context().close();
  // Contrat Tauri simulé
  p = await page('dark', { tauri: true }); await p.goto(base);
  await p.evaluate(() => __emettre({ id: 7, genre: 'erreur', titre: 'Erreur', detail: 'Cause', actions: [{ id: 'choisir', libelle: 'Choisir un dossier' }] }));
  await p.waitForTimeout(400);
  await p.evaluate(() => __emettre({ id: 8, genre: 'info', titre: 'Info', detail: 'remplace', actions: [{ id: 'x', libelle: 'ne doit pas apparaitre' }] }));
  await p.waitForTimeout(400);
  console.log('boutons info :', await p.$$eval('#bandeau button', bs => bs.map(b => b.className + ':' + b.dataset.action)));
  await p.waitForTimeout(6500);
  console.log('info encore visible après 7 s ?', await p.evaluate(() => !document.getElementById('bandeau').hidden));
  await p.click('.xb'); await p.waitForTimeout(400);
  await p.evaluate(() => __emettre({ id: 9, genre: 'erreur', titre: 'Erreur', detail: 'Cause', actions: [{ id: 'choisir', libelle: 'Choisir un dossier' }] }));
  await p.waitForTimeout(400); await p.click('.act'); await p.waitForTimeout(400);
  await p.evaluate(() => __emettre({ id: 10, genre: 'reussite', titre: 'Copié' })); await p.waitForTimeout(3000);
  console.log('appels', JSON.stringify(await p.evaluate(() => __appels)));
  console.log('focus actif :', await p.evaluate(() => document.activeElement.tagName));
  await p.context().close();
  // Animations réduites
  p = await page('light', { reduit: true }); await p.goto(`${base}?demo=reussite`); await p.waitForTimeout(60);
  console.log('reduit, transform pendant entree :', await p.evaluate(() => getComputedStyle(document.getElementById('bandeau')).transform));
  await p.context().close();
  // ---- Cas de la relecture : chaque cas échouait avant les corrections ----
  let echecs = 0;
  const verifier = (ok, quoi, info = '') => { console.log(ok ? 'OK   ' : 'ECHEC', quoi, info); if (!ok) echecs++; };
  // bandeau_pret part une fois à l'écoute en place : vérifié à part.
  const sansZone = (a) => a.filter(x => x[0] !== 'bandeau_zone' && x[0] !== 'bandeau_pret');
  const ERR = { id: 'E', genre: 'erreur', titre: 'Erreur', detail: 'cause', actions: [{ id: 'choisir', libelle: 'Choisir un dossier' }] };
  async function tauriPage(o = {}) { const q = await page(o.theme || 'light', { tauri: true, ...o }); q.avert = []; q.on('console', m => { if (m.type() === 'warning') q.avert.push(m.text()); }); await q.goto(base); return q; }
  const lesAppels = (q) => q.evaluate(() => __appels);
  const centre = (q, s) => q.$eval(s, b => { const r = b.getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; });

  // 1. injection
  p = await tauriPage();
  await p.evaluate(() => __emettre({ id: 'i1', genre: 'erreur', titre: '<img src=x onerror="window.__pwn=1">', detail: '<b>gras</b>',
    actions: [{ id: 'a"><img src=x onerror=alert(1)>', libelle: '<img src=x onerror="window.__pwn=3">' }, { id: 's1', libelle: '<svg onload="window.__pwn=4"></svg>' }] }));
  await p.waitForTimeout(300);
  const inj = await p.evaluate(() => ({ pwn: window.__pwn, balises: bandeau.querySelectorAll('img,script,image,.detail *,.act *').length, svgs: bandeau.querySelectorAll('svg').length, titre: bandeau.querySelector('.titre').textContent }));
  verifier(!inj.pwn && inj.balises === 0 && inj.svgs === 2 && inj.titre.startsWith('<img'), '1 injection : texte seulement', JSON.stringify(inj));
  await p.context().close();

  // 2. hauteur maximale connue, croix visible
  for (const w of [320, 560]) {
    p = await tauriPage({ largeur: w, hauteur: 260 });
    await p.evaluate(() => __emettre({ id: 4, genre: 'erreur', titre: 'x'.repeat(2000), detail: 'mot '.repeat(500), actions: [{ id: 'a', libelle: 'L'.repeat(300) }, { id: 'b', libelle: 'Réessayer' }] }));
    await p.waitForTimeout(400);
    const m = await p.evaluate(() => { const b = bandeau.getBoundingClientRect(), x = bandeau.querySelector('.xb').getBoundingClientRect(); return { h: b.height, l: b.left, r: b.right, xr: x.right, xl: x.left, sw: document.documentElement.scrollWidth, iw: innerWidth }; });
    verifier(m.h <= 108 && m.xl >= m.l && m.xr <= m.r && m.sw === m.iw && m.l >= 8 && m.r <= m.iw - 8, `2 texte de 2000 caractères à ${w} px : hauteur ≤ 108, croix dans la pilule`, JSON.stringify(m));
    if (w === 320) await capture(p, 'texte-tres-long-erreur-320-clair', 'light');
    await p.context().close();
  }

  // 3. double clic et clic pendant le fondu
  p = await tauriPage();
  await p.evaluate(m => __emettre(m), ERR); await p.waitForTimeout(400);
  await p.dblclick('.act'); await p.waitForTimeout(400);
  let a = sansZone(await lesAppels(p));
  verifier(JSON.stringify(a) === JSON.stringify([['bandeau_action', { id: 'E', action: 'choisir' }], ['bandeau_termine', { id: 'E' }]]), '3 double clic : une action, un termine', JSON.stringify(a));
  await p.evaluate(m => __emettre({ ...m, id: 'F' }), ERR); await p.waitForTimeout(400);
  await p.click('.act'); await p.waitForTimeout(50);
  const pe = await p.evaluate(() => getComputedStyle(bandeau).pointerEvents);
  await p.click('.xb', { force: true }); await p.waitForTimeout(400);
  a = sansZone(await lesAppels(p)).filter(x => x[1].id === 'F');
  verifier(a.length === 2 && a[0][1].action === 'choisir' && pe === 'none', '3 croix pendant le fondu ignorée, pointer-events none', JSON.stringify(a) + ' ' + pe);
  await p.context().close();

  // 4. messages invalides
  p = await tauriPage();
  const invalides = [null, {}, { genre: 'info', titre: 't' }, { id: 1, genre: 'alerte', titre: 't' }, { id: 1, genre: 5, titre: 't' }, { id: 1, genre: 'info', titre: '' },
    { id: 1, genre: 'info', titre: '   ' }, { id: 1, genre: 'info', titre: 42 }, { id: {}, genre: 'info', titre: 't' }];
  for (const m of invalides) await p.evaluate(m => __emettre(m), m);
  await p.evaluate(() => __emettre({ id: 3, genre: 'info', titre: { toString() { throw new Error('boum'); } } }));
  await p.waitForTimeout(100);
  const inval = await p.evaluate(() => [bandeau.hidden, __appels.filter(x => x[0] !== 'bandeau_pret').length]);
  verifier(inval[0] === true && inval[1] === 0 && p.avert.length >= invalides.length + 1, '4 messages invalides rejetés avec console.warn', JSON.stringify(inval) + ' avert=' + p.avert.length);
  await p.evaluate(() => __emettre({ id: 2, genre: 'erreur', titre: 't', actions: [null, {}, { id: {} , libelle: 'x' }, { id: 7, libelle: 'n' }, { id: 'fermer', libelle: 'Fermer pirate' }, { id: 'x', libelle: null }, { id: 'ok', libelle: 'Bien' }, { id: 'ok', libelle: 'Doublon' }] }));
  await p.waitForTimeout(100);
  const bs = await p.$$eval('#bandeau button', b => b.map(x => x.dataset.action + ':' + x.textContent));
  verifier(JSON.stringify(bs) === JSON.stringify(['ok:Bien', 'fermer:']), '4 actions mal formées rejetées', JSON.stringify(bs));
  await p.context().close();

  // 5. remplacement : rien de l'ancien après l'affichage du nouveau
  p = await tauriPage();
  await p.evaluate(() => __emettre({ id: 'A', genre: 'info', titre: 'A' })); await p.waitForTimeout(400);
  await p.click('.xb'); await p.waitForTimeout(60);
  await p.evaluate(() => { __appels.push(['--nouveau--']); __emettre({ id: 'B', genre: 'info', titre: 'B' }); }); await p.waitForTimeout(500);
  a = await lesAppels(p); let apres = a.slice(a.findIndex(x => x[0] === '--nouveau--') + 1);
  verifier(apres.every(x => x[1].id === 'B') && await p.evaluate(() => !bandeau.hidden && getComputedStyle(bandeau).opacity === '1' && getComputedStyle(bandeau).pointerEvents === 'auto'), '5 sortie interrompue : rien pour A ensuite, B visible et cliquable', JSON.stringify(apres));
  await p.evaluate(() => { __appels.length = 0; __emettre({ id: 'C', genre: 'reussite', titre: 'C' }); }); await p.waitForTimeout(2500 + 80);
  await p.evaluate(() => { __appels.push(['--nouveau--']); __emettre({ id: 'D', genre: 'info', titre: 'D' }); }); await p.waitForTimeout(400);
  a = await lesAppels(p); apres = a.slice(a.findIndex(x => x[0] === '--nouveau--') + 1);
  verifier(apres.every(x => x[1].id === 'D') && await p.evaluate(() => !bandeau.hidden), '5 réussite remplacée pendant son fondu : pas de termine pour C', JSON.stringify(a));
  await p.context().close();

  // 6 et 14. clic droit, appui annulé, appui commencé dehors
  p = await tauriPage();
  await p.evaluate(m => __emettre(m), ERR); await p.waitForTimeout(400);
  const bAct = await centre(p, '.act'), bX = await centre(p, '.xb');
  const cm = await p.evaluate(() => { const e = new MouseEvent('contextmenu', { bubbles: true, cancelable: true }); bandeau.dispatchEvent(e); const f = new MouseEvent('contextmenu', { bubbles: true, cancelable: true }); document.documentElement.dispatchEvent(f); return [e.defaultPrevented, f.defaultPrevented]; });
  await p.mouse.click(...bAct, { button: 'right' }); await p.waitForTimeout(50);
  verifier(cm[0] && cm[1] && sansZone(await lesAppels(p)).length === 0, '6 menu contextuel annulé partout, clic droit sans effet', JSON.stringify(cm));
  await p.mouse.move(...bAct); await p.mouse.down(); await p.mouse.move(...bX); await p.mouse.up(); await p.waitForTimeout(50);
  await p.mouse.move(...bX); await p.mouse.down(); await p.mouse.move(5, 190); await p.mouse.up(); await p.waitForTimeout(50);
  await p.mouse.move(5, 190); await p.mouse.down(); await p.mouse.move(...bX); await p.mouse.up(); await p.waitForTimeout(50);
  await p.evaluate(() => window.dispatchEvent(new PointerEvent('pointercancel')));
  verifier(sansZone(await lesAppels(p)).length === 0, '14 appuis glissés ou commencés dehors : aucun déclenchement');
  await p.mouse.click(...bX); await p.waitForTimeout(300);
  a = sansZone(await lesAppels(p));
  verifier(a.length === 2 && a[0][1].action === 'fermer', '14 un vrai clic marche ensuite', JSON.stringify(a));
  verifier(await p.evaluate(() => document.activeElement === document.body), 'focus jamais pris');
  await p.context().close();

  // 10. réussite sur une ligne avec « … »
  p = await tauriPage({ largeur: 320 });
  await p.evaluate(() => __emettre({ id: 1, genre: 'reussite', titre: 'Enregistré dans Images › BreezyShot › BreezyShot 2026-11-11 à 16.42.08 avec un nom très long.png' })); await p.waitForTimeout(400);
  const ok1 = await p.evaluate(() => { const t = bandeau.querySelector('.titre'); const cs = getComputedStyle(t); return { h: bandeau.getBoundingClientRect().height, ws: cs.whiteSpace, to: cs.textOverflow, r: bandeau.getBoundingClientRect().right, iw: innerWidth }; });
  verifier(ok1.h <= 34 && ok1.ws === 'nowrap' && ok1.to === 'ellipsis' && ok1.r <= ok1.iw - 8, '10 réussite longue : une ligne, « … »', JSON.stringify(ok1));
  await capture(p, 'texte-long-reussite-320-clair', 'light');
  await p.context().close();

  // 13 et 15. interligne du détail, jeton d'entrée, durées
  for (const r of [false, true]) {
    p = await tauriPage({ reduit: r });
    await p.evaluate(() => __emettre({ id: 1, genre: 'erreur', titre: 't', detail: 'd' }));
    const an = await p.evaluate(() => bandeau.getAnimations().map(x => [x.effect.getTiming().duration, x.effect.getTiming().easing]));
    const li = await p.evaluate(() => getComputedStyle(bandeau.querySelector('.detail')).lineHeight);
    verifier(li === '18px' && JSON.stringify(an) === JSON.stringify(r ? [[120, 'linear']] : [[300, 'cubic-bezier(0.34, 1.4, 0.64, 1)']]), `13/15 interligne 18 px, entrée ${r ? 'réduite' : 'normale'}`, li + ' ' + JSON.stringify(an));
    await p.context().close();
  }

  // zone exacte
  p = await tauriPage({ largeur: 561 });
  await p.evaluate(m => __emettre(m), ERR); await p.waitForTimeout(450);
  const z = await p.evaluate(() => { const b = bandeau.getBoundingClientRect(); const s = __appels.filter(x => x[0] === 'bandeau_zone').pop()[1]; return [b.x - s.x, b.y - s.y, b.width - s.largeur, b.height - s.hauteur]; });
  verifier(z.every(v => Math.abs(v) < 0.01), 'zone = rectangle réel de la pilule', JSON.stringify(z));
  await p.context().close();

  // Info et erreur sans détail : une seule ligne, pastille et croix centrées, pas de ligne vide.
  for (const [genre, theme] of [['info', 'light'], ['info', 'dark'], ['erreur', 'light']]) {
    p = await tauriPage({ theme });
    const titre = genre === 'info' ? 'Impr. écran est prise. Raccourci : Ctrl + Maj + 1' : 'Aucun raccourci disponible';
    await p.evaluate(([g, t]) => __emettre({ id: 1, genre: g, titre: t, detail: '', actions: [] }), [genre, titre]);
    await p.waitForTimeout(450);
    const m = await p.evaluate(() => {
      const c = (s) => { const r = bandeau.querySelector(s).getBoundingClientRect(); return r.y + r.height / 2; };
      const b = bandeau.getBoundingClientRect();
      return { h: b.height, milieu: b.y + b.height / 2, pastille: c('.pastille'), croix: c('.xb'), titre: c('.titre'),
        lignesTitre: Math.round(bandeau.querySelector('.titre').getBoundingClientRect().height / 18), detail: getComputedStyle(bandeau.querySelector('.detail')).display };
    });
    const centre = (v) => Math.abs(v - m.milieu) < 1;
    verifier(m.h <= 42 && m.lignesTitre === 1 && m.detail === 'none' && centre(m.pastille) && centre(m.croix) && centre(m.titre),
      `${genre} sans détail (${theme}) : une ligne, pastille et croix centrées`, JSON.stringify(m));
    if (genre === 'info') await capture(p, `info-une-ligne-${theme === 'light' ? 'clair' : 'sombre'}`, theme);
    await p.context().close();
  }

  // bandeau_pret : une seule fois, avant tout autre appel.
  p = await tauriPage();
  await p.waitForTimeout(100);
  a = await lesAppels(p);
  verifier(JSON.stringify(a) === JSON.stringify([['bandeau_pret', {}]]), 'bandeau_pret envoyé une fois, écoute en place', JSON.stringify(a));
  await p.context().close();

  // CSP de l'app : la page ne s'en voit rien refuser (ni script ni style en ligne).
  p = await page('light', { tauri: true });
  await p.addInitScript(() => { window.__refus = []; document.addEventListener('securitypolicyviolation', e => window.__refus.push(e.violatedDirective + ' ' + e.blockedURI)); });
  await p.goto(base + '?csp=1');
  await p.evaluate(() => __emettre({ id: 1, genre: 'erreur', titre: 'Erreur', detail: 'Cause', actions: [{ id: 'a', libelle: 'Action' }] }));
  await p.waitForTimeout(500);
  const refus = await p.evaluate(() => window.__refus);
  const styleOk = await p.evaluate(() => getComputedStyle(bandeau).borderRadius !== '0px' && !bandeau.hidden);
  verifier(refus.length === 0 && styleOk, 'CSP de l\'app respectée (aucun refus, styles chargés)', JSON.stringify(refus));
  await p.context().close();

  console.log('ECHECS :', echecs);
  console.log('ERREURS PAGE :', JSON.stringify(erreurs));
  await nav.close(); serveur.close();
  process.exitCode = echecs === 0 ? 0 : 1;
})().catch((e) => { console.error('essai interrompu :', e); process.exit(1); });
