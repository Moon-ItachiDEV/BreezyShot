const { chromium } = require('playwright');
const http = require('http'), fs = require('fs'), path = require('path');
// Essai de la page de l'installateur dans Chromium (Playwright). Sort avec 1 si une
// vérification échoue. Captures dans $SORTIE (par défaut essais-resultats/pages).
//   node essais/pages/installateur.cjs
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
const TOTAL = 15519744; // 14,8 Mo

(async () => {
  await new Promise(ok => serveur.listen(0, ok));
  const base = `http://127.0.0.1:${serveur.address().port}/installateur/installateur.html`;
  const nav = await chromium.launch();
  const erreurs = [];
  let echecs = 0;
  const verifier = (ok, quoi, info = '') => { console.log(ok ? 'OK   ' : 'ECHEC', quoi, info); if (!ok) echecs++; };

  async function page(opts = {}) {
    const ctx = await nav.newContext({
      viewport: { width: opts.largeur || 800, height: opts.hauteur || 560 },
      deviceScaleFactor: opts.echelle || 1, colorScheme: opts.theme || 'light',
      reducedMotion: opts.reduit ? 'reduce' : 'no-preference',
    });
    const p = await ctx.newPage();
    p.on('pageerror', e => erreurs.push(String(e)));
    p.on('console', m => { if (m.type() === 'error') erreurs.push(m.text()); });
    await p.addInitScript(() => {
      window.__appels = []; const ecouteurs = {};
      window.__TAURI__ = {
        core: { invoke: (c, a) => { window.__appels.push([c, a]); return Promise.resolve(); } },
        event: { listen: (n, f) => { ecouteurs[n] = f; return Promise.resolve(() => {}); } },
      };
      window.__emettre = (n, payload) => { if (ecouteurs[n]) ecouteurs[n]({ payload }); };
    });
    await p.goto(base);
    await p.waitForFunction(() => window.__appels.some(a => a[0] === 'installateur_pret'));
    return p;
  }
  const appels = (p) => p.evaluate(() => window.__appels);
  const actions = async (p) => (await appels(p)).filter(a => a[0] === 'installateur_action').map(a => a[1].action);
  const etat = (p) => p.$eval('#win', w => w.dataset.s);
  const emettre = (p, n, charge) => p.evaluate(([n, c]) => window.__emettre(n, c), [n, charge]);

  async function capture(p, nom, theme) {
    // Fond posé par le CSSOM (pas de <style> : la CSP de l'app interdit l'inline).
    await p.evaluate(f => { document.documentElement.style.minHeight = '100%'; document.documentElement.style.background = f; }, FONDS[theme || 'light']);
    await p.waitForTimeout(500);
    await p.screenshot({ path: `${SORTIE}/installateur-${nom}.png` });
    console.log('capture', nom);
  }

  // 1. pret une fois ; rien d'autre avant un clic.
  let p = await page();
  await emettre(p, 'installateur:etat', { etat: 'accueil', cause: '', total: TOTAL, direct: false });
  await p.waitForTimeout(200);
  let a = await appels(p);
  verifier(a.filter(x => x[0] === 'installateur_pret').length === 1 && (await actions(p)).length === 0,
    '1 installateur_pret une fois, aucune action avant un clic', JSON.stringify(a.map(x => x[0])));

  // 2. accueil : textes exacts, « Installer » a le focus, aucun texte du prototype.
  const t2 = await p.evaluate(() => ({
    titre: document.querySelector('.welcome h1').textContent,
    bouton: document.getElementById('install').textContent,
    focus: document.activeElement === document.getElementById('install'),
    prototype: /Simuler une erreur|Recommencer/.test(document.body.textContent),
  }));
  verifier(t2.titre === 'BreezyShot' && t2.bouton === 'Installer' && t2.focus && !t2.prototype,
    '2 accueil : textes, focus, pas de prototype', JSON.stringify(t2));

  // 3. double clic sur « Installer » : une seule action, data-s starting.
  await p.click('#install'); await p.click('#install', { force: true });
  await p.waitForTimeout(50);
  verifier((await actions(p)).filter(x => x === 'installer').length === 1, '3 double clic : une action installer', JSON.stringify(await actions(p)));
  await p.context().close();

  // 4. progression : jamais au-dessus du réel, jamais en recul, < 100 % sans « prêt ».
  p = await page();
  await emettre(p, 'installateur:etat', { etat: 'accueil', cause: '', total: TOTAL, direct: false });
  await p.click('#install');
  await emettre(p, 'installateur:etat', { etat: 'installation', cause: '', total: TOTAL, direct: false });
  await p.waitForFunction(() => document.getElementById('win').dataset.s === 'installing');
  let dernier = 0, jamaisAuDessus = true, jamaisRecul = true, sousCent = true;
  for (let i = 1; i <= 10; i++) {
    const reel = i / 10 * 0.5; // on n'alimente que jusqu'à 50 %
    await emettre(p, 'installateur:progression', { octets: Math.round(TOTAL * reel), total: TOTAL });
    await p.waitForTimeout(60);
    const m = await p.evaluate(() => ({ now: Number(document.getElementById('pbar').getAttribute('aria-valuenow')), pct: document.getElementById('pct').textContent }));
    if (m.now > 51) jamaisAuDessus = false;
    if (m.now < dernier) jamaisRecul = false;
    if (m.now >= 100) sousCent = false;
    dernier = m.now;
  }
  verifier(jamaisAuDessus && jamaisRecul && sousCent, '4 progression bornée au réel, croissante, < 100 % sans prêt',
    `dernier=${dernier}`);
  // La ligne « Mo » suit le total.
  const moTexte = await p.$eval('#mo', e => e.textContent);
  const pctTexte = await p.$eval('#pct', e => e.textContent);
  verifier(/^\d+,\d\u00a0Mo sur 14,8\u00a0Mo$/.test(moTexte) && /^\d+\u00a0%$/.test(pctTexte),
    '4 « … Mo sur 14,8 Mo » et « … % » avec espaces insécables', JSON.stringify([moTexte, pctTexte]));
  // « prêt » : la barre finit à 100 %, l'écran 3 arrive.
  await emettre(p, 'installateur:progression', { octets: TOTAL, total: TOTAL });
  await emettre(p, 'installateur:etat', { etat: 'pret', cause: '', total: TOTAL, direct: false });
  await p.waitForFunction(() => document.getElementById('win').dataset.s === 'done', null, { timeout: 3000 });
  verifier(await p.$eval('#pct', e => e.textContent).then(t => t === '100\u00a0%'), '4 barre à 100 % après prêt');
  await p.context().close();

  // 4 bis. « prêt » : la barre se referme en anneau (closing-ring) avant l'écran 3.
  p = await page();
  await emettre(p, 'installateur:etat', { etat: 'accueil', cause: '', total: TOTAL, direct: false });
  await p.click('#install');
  await p.waitForFunction(() => document.getElementById('win').dataset.s === 'installing');
  await emettre(p, 'installateur:progression', { octets: TOTAL, total: TOTAL });
  await emettre(p, 'installateur:etat', { etat: 'pret', cause: '', total: TOTAL, direct: false });
  await p.waitForFunction(() => document.getElementById('win').dataset.s === 'closing-ring', null, { timeout: 3000 });
  await p.waitForTimeout(450);
  const e4b = await p.evaluate(() => {
    const b = getComputedStyle(document.getElementById('pbar')), l = getComputedStyle(document.querySelector('.prow'));
    return { etat: document.getElementById('win').dataset.s, barre: Number(b.opacity), ligne: Number(l.opacity), tr: b.transform };
  });
  const echelle4b = (() => { const m = /^matrix\(([^,]+),/.exec(e4b.tr); return m ? Math.abs(Number(m[1])) : NaN; })();
  verifier(e4b.etat === 'closing-ring' && e4b.barre < 0.2 && e4b.ligne < 0.2 && echelle4b < 0.1,
    '4 bis anneau : ligne et barre s\'effacent, barre réduite à rien', JSON.stringify({ ...e4b, echelle4b }));
  await p.waitForFunction(() => document.getElementById('win').dataset.s === 'done', null, { timeout: 2000 });
  await p.context().close();

  // 4 ter. Erreur reçue dans les 50 ms après « Installer » : la page ne reste pas figée en
  // « installation » ; relue après 1 s, elle montre toujours l'erreur et « Réessayer ».
  p = await page();
  await emettre(p, 'installateur:etat', { etat: 'accueil', cause: '', total: TOTAL, direct: false });
  await p.click('#install');
  await p.waitForTimeout(20);
  await emettre(p, 'installateur:etat', { etat: 'erreur', cause: 'BreezyShot est encore ouvert. Quittez-le, puis réessayez.', total: TOTAL, direct: false });
  const lire4t = () => p.evaluate(() => {
    const r = document.getElementById('retry'), etapes = [...document.querySelectorAll('[data-step]')];
    return {
      etat: document.getElementById('win').dataset.s,
      ecran: etapes.findIndex(e => !e.hidden),
      cause: getComputedStyle(document.getElementById('cause')).opacity,
      reessayer: getComputedStyle(r).display !== 'none' && !r.disabled,
    };
  });
  const tot = await lire4t();
  await p.waitForTimeout(1000);
  const tard = await lire4t();
  verifier(tot.etat === 'error' && tard.etat === 'error' && tard.ecran === 1 && Number(tard.cause) > 0.9 && tard.reessayer,
    '4 ter erreur immédiate : toujours l\'erreur après 1 s', JSON.stringify({ tot, tard }));
  // Réessayer repart de zéro : la part copiée d'avant ne compte plus.
  await emettre(p, 'installateur:progression', { octets: Math.round(TOTAL * 0.6), total: TOTAL });
  await p.click('#retry');
  await p.waitForTimeout(400);
  const e4r = await p.evaluate(() => ({ etat: document.getElementById('win').dataset.s, now: Number(document.getElementById('pbar').getAttribute('aria-valuenow')) }));
  verifier(e4r.etat === 'installing' && e4r.now === 0, '4 ter Réessayer : barre repartie de 0', JSON.stringify(e4r));
  await p.context().close();

  // 4 quater. Durées du script : celles des jetons (jetons.css), une à une.
  p = await page();
  const css = fs.readFileSync(path.join(RACINE, 'commun', 'jetons.css'), 'utf8');
  const jeton = (n) => { const m = css.match(new RegExp('--' + n + ':\\s*([0-9.]+)ms;')); return m ? Number(m[1]) : NaN; };
  const noms = { reduite: 'duree-reduite', sortie: 'duree-sortie', versProgression: 'duree-installateur-vers-progression',
    versBarre: 'duree-installateur-vers-barre', anneau: 'duree-installateur-anneau', anneauReduit: 'duree-installateur-anneau-reduit',
    focus: 'duree-installateur-focus', arretMoulinet: 'duree-installateur-moulinet-arret', envol: 'duree-envol-fenetre' };
  const lues = await p.evaluate(() => window.__dureesInstallateur);
  const ecarts = Object.entries(noms).filter(([k, n]) => !(lues[k] === jeton(n) && jeton(n) > 0));
  const source = fs.readFileSync(path.join(RACINE, 'installateur', 'installateur.js'), 'utf8');
  const enDur = source.split('\n').filter(l => /(later|laterEn|setTimeout)\([^;]*\b\d{2,}\b/.test(l) || /duration:\s*\d/.test(l));
  verifier(ecarts.length === 0 && Object.keys(lues).length === Object.keys(noms).length && enDur.length === 0,
    '4 quater durées du script = jetons, aucune en dur', JSON.stringify({ ecarts, enDur }));
  await p.context().close();

  // 5. erreur : cause posée comme texte (injection inerte), danger, « Réessayer » seul.
  p = await page();
  await emettre(p, 'installateur:etat', { etat: 'accueil', cause: '', total: TOTAL, direct: false });
  await p.click('#install');
  await emettre(p, 'installateur:etat', { etat: 'installation', cause: '', total: TOTAL, direct: false });
  await emettre(p, 'installateur:etat', { etat: 'erreur', cause: '<img src=x onerror="window.__pwn=1"> plus de place', total: TOTAL, direct: false });
  await p.waitForFunction(() => document.getElementById('win').dataset.s === 'error');
  const e5 = await p.evaluate(() => ({
    pwn: window.__pwn, imgs: document.querySelectorAll('.cause img').length,
    texte: document.getElementById('cause').textContent.startsWith('<img'),
    couleur: getComputedStyle(document.getElementById('pct')).color,
    retrySeul: getComputedStyle(document.getElementById('retry')).display !== 'none'
      && getComputedStyle(document.getElementById('install')).display === 'none'
      && getComputedStyle(document.getElementById('done')).display === 'none',
  }));
  verifier(!e5.pwn && e5.imgs === 0 && e5.texte && e5.retrySeul, '5 erreur : cause en texte, « Réessayer » seul', JSON.stringify(e5));
  await p.click('#retry');
  verifier((await actions(p)).includes('reessayer'), '5 clic Réessayer → action reessayer');
  await p.context().close();

  // 6. prêt direct : écran 3 sans l'écran 2 ; « Terminer » a le focus.
  p = await page();
  let vuInstall = false;
  p.on('console', () => {});
  await emettre(p, 'installateur:etat', { etat: 'pret', cause: '', total: TOTAL, direct: true });
  await p.waitForFunction(() => document.getElementById('win').dataset.s === 'done');
  const e6 = await p.evaluate(() => ({
    titre: document.querySelector('.ready h2').textContent,
    boutons: [document.getElementById('try').textContent, document.getElementById('done').textContent],
    focus: document.activeElement === document.getElementById('done'),
    installVisible: getComputedStyle(document.querySelectorAll('[data-step]')[1]).display !== 'none',
  }));
  verifier(e6.titre === 'BreezyShot est prêt' && e6.boutons[0] === 'Essayer une capture' && e6.boutons[1] === 'Terminer'
    && e6.focus && !e6.installVisible, '6 prêt direct : écran 3, textes, focus', JSON.stringify(e6));
  void vuInstall;

  // 7. bouton hors état : un clic programmé sur un bouton caché n'envoie rien de plus.
  const avant = (await actions(p)).length;
  await p.evaluate(() => document.getElementById('install').click());
  verifier((await actions(p)).length === avant, '7 bouton caché : aucun envoi');

  // 8. « Essayer » et « Terminer ».
  await p.click('#try');
  verifier((await actions(p)).includes('essayer'), '8 Essayer → action essayer');
  await p.click('#done');
  verifier((await actions(p)).filter(x => x === 'terminer').length === 1, '8 Terminer → action terminer');
  await p.context().close();

  // 8 bis. Fermer et Réduire.
  p = await page();
  await emettre(p, 'installateur:etat', { etat: 'accueil', cause: '', total: TOTAL, direct: false });
  await p.click('#min'); await p.click('#close');
  verifier((await actions(p)).join(',') === 'reduire,fermer', '8 Réduire puis Fermer', JSON.stringify(await actions(p)));
  await emettre(p, 'installateur:fermer', {});
  await p.waitForTimeout(250);
  verifier(await p.$eval('#win', w => w.dataset.s === 'closed'), '8 installateur:fermer joue le fondu');
  await p.context().close();

  // 9. envol : échelle finale faible, opacité nulle.
  p = await page();
  await emettre(p, 'installateur:etat', { etat: 'pret', cause: '', total: TOTAL, direct: true });
  await p.waitForFunction(() => document.getElementById('win').dataset.s === 'done');
  await emettre(p, 'installateur:envol', { duree: 460 });
  await p.waitForTimeout(650);
  const e9 = await p.evaluate(() => { const s = getComputedStyle(document.getElementById('win')); return { op: Number(s.opacity), tr: s.transform }; });
  const echelle9 = (() => { const m = /^matrix\(([^,]+),/.exec(e9.tr); return m ? Math.abs(Number(m[1])) : NaN; })();
  verifier(e9.op < 0.05 && echelle9 < 0.1, '9 envol : opacité nulle et échelle réduite à la fin', JSON.stringify({ ...e9, echelle9 }));
  await p.context().close();

  // 10. « réduire les animations » : entrées en 120 ms.
  p = await page({ reduit: true });
  await emettre(p, 'installateur:etat', { etat: 'accueil', cause: '', total: TOTAL, direct: false });
  const dur = await p.evaluate(() => getComputedStyle(document.querySelector('.welcome')).animationDuration);
  verifier(dur === '0.12s', '10 animations réduites à 120 ms', dur);
  // Le reflet de la barre disparaît ; hors réduction, il court (animation « shine »).
  const eclat = await p.evaluate(() => getComputedStyle(document.getElementById('fill'), '::after').display);
  const p10 = await page();
  const court = await p10.evaluate(() => getComputedStyle(document.getElementById('fill'), '::after').animationName);
  verifier(eclat === 'none' && court === 'shine', '10 reflet : absent en réduit, présent sinon', JSON.stringify({ eclat, court }));
  await p10.context().close();
  await p.context().close();

  // 11. hauteur bornée : fenêtre de 600 px, la carte tient (aucun défilement).
  p = await page({ hauteur: 600 });
  await emettre(p, 'installateur:etat', { etat: 'accueil', cause: '', total: TOTAL, direct: false });
  const e11 = await p.evaluate(() => ({ sw: document.documentElement.scrollWidth, cw: innerWidth, sh: document.documentElement.scrollHeight, ch: innerHeight }));
  verifier(e11.sw <= e11.cw && e11.sh <= e11.ch, '11 carte sans défilement à 600 px', JSON.stringify(e11));
  await p.context().close();

  // 12. captures clair et sombre des quatre états.
  for (const theme of ['light', 'dark']) {
    const nom = theme === 'light' ? 'clair' : 'sombre';
    let q = await page({ theme });
    await emettre(q, 'installateur:etat', { etat: 'accueil', cause: '', total: TOTAL, direct: false });
    await capture(q, `accueil-${nom}`, theme); await q.context().close();
    q = await page({ theme });
    await emettre(q, 'installateur:etat', { etat: 'accueil', cause: '', total: TOTAL, direct: false });
    await q.click('#install');
    await emettre(q, 'installateur:etat', { etat: 'installation', cause: '', total: TOTAL, direct: false });
    await q.waitForFunction(() => document.getElementById('win').dataset.s === 'installing');
    await emettre(q, 'installateur:progression', { octets: Math.round(TOTAL * 0.52), total: TOTAL });
    await q.waitForTimeout(900);
    await capture(q, `installation-${nom}`, theme);
    await emettre(q, 'installateur:etat', { etat: 'erreur', cause: 'Il manque de la place sur le disque C:. Libérez 120 Mo, puis réessayez.', total: TOTAL, direct: false });
    await capture(q, `erreur-${nom}`, theme); await q.context().close();
    q = await page({ theme });
    await emettre(q, 'installateur:etat', { etat: 'pret', cause: '', total: TOTAL, direct: true });
    await capture(q, `pret-${nom}`, theme); await q.context().close();
  }

  // 13. échelle 1,5 : la page tient.
  p = await page({ echelle: 1.5 });
  await emettre(p, 'installateur:etat', { etat: 'accueil', cause: '', total: TOTAL, direct: false });
  verifier(await p.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1), '13 échelle 1,5 : pas de débordement');
  await p.context().close();

  console.log('ECHECS :', echecs);
  console.log('ERREURS PAGE :', JSON.stringify(erreurs));
  await nav.close(); serveur.close();
  process.exitCode = (echecs === 0 && erreurs.length === 0) ? 0 : 1;
})().catch((e) => { console.error('essai interrompu :', e); process.exit(1); });
