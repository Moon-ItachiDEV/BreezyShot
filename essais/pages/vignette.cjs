const { chromium } = require('playwright');
const http = require('http'), fs = require('fs'), path = require('path');
// Essai de la page de la vignette dans Chromium (Playwright). Sort avec 1 si une
// vérification échoue. Captures dans $SORTIE (par défaut essais-resultats/pages).
//   node essais/pages/vignette.cjs
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
const FONDS = { light: 'linear-gradient(135deg,#DCE7F6,#C6D5EE)', dark: 'linear-gradient(135deg,#0B1830,#141A36)' };
const ACTIONS = [
  { id: 'copier', libelle: 'Copier', libelle_fait: 'Copié', place: 'principale', picto: 'copier' },
  { id: 'enregistrer', libelle: 'Enregistrer', libelle_fait: 'Enregistré', place: 'principale', picto: 'enregistrer' },
  { id: 'supprimer', libelle: 'Supprimer', libelle_fait: null, place: 'secondaire', picto: 'supprimer' },
  { id: 'fermer', libelle: 'Fermer', libelle_fait: null, place: 'fermer', picto: 'fermer' },
  { id: 'annuler', libelle: 'Annuler', libelle_fait: null, place: 'annulation', picto: 'annuler' },
];
const charge = (o = {}) => ({ id: 5, coin: 'bas_gauche', retrait: { haut: 0, bas: 0, gauche: 0, droite: 0 }, actions: ACTIONS,
  duree_ms: 6000, glisser: true, reduites: false, apercu: { largeur: 64, hauteur: 40 }, ...o });
(async () => {
  await new Promise(ok => serveur.listen(0, ok));
  const base = `http://127.0.0.1:${serveur.address().port}/vignette/vignette.html`;
  const nav = await chromium.launch();
  const erreurs = [];
  let echecs = 0;
  const verifier = (ok, quoi, info = '') => { console.log(ok ? 'OK   ' : 'ECHEC', quoi, info); if (!ok) echecs++; };
  // Page de 340 × 280 (TAILLE_LOGIQUE_VIGNETTE), cœur simulé : les appels sont notés,
  // `vignette_image` rend un vrai PNG.
  async function page(o = {}) {
    const ctx = await nav.newContext({ viewport: { width: 340, height: 280 }, deviceScaleFactor: o.echelle || 1,
      colorScheme: o.theme || 'light', reducedMotion: o.reduit ? 'reduce' : 'no-preference' });
    const p = await ctx.newPage();
    p.avert = [];
    p.on('pageerror', e => erreurs.push(String(e)));
    p.on('console', m => { if (m.type() === 'error') erreurs.push(m.text()); if (m.type() === 'warning') p.avert.push(m.text()); });
    if (o.csp) await p.addInitScript(() => { window.__refus = []; document.addEventListener('securitypolicyviolation', e => window.__refus.push(e.violatedDirective + ' ' + e.blockedURI)); });
    await p.addInitScript((refus) => {
      window.__appels = []; const ecoutes = {}; let refuses = 0;
      const toile = document.createElement('canvas'); toile.width = 64; toile.height = 40;
      const g = toile.getContext('2d'); g.fillStyle = '#E8B23A'; g.fillRect(0, 0, 64, 40); g.fillStyle = '#0067C0'; g.fillRect(0, 0, 32, 40);
      const octets = Uint8Array.from(atob(toile.toDataURL('image/png').split(',')[1]), x => x.charCodeAt(0));
      const png = Promise.resolve(octets.buffer); // sans fetch : la CSP de l'app l'interdit
      window.__TAURI__ = {
        core: { invoke: (c, a) => {
          window.__appels.push([c, a]);
          // `refus` premiers « prête » rejetés, comme un cœur pas encore en place.
          if (c === 'vignette_pret' && refuses < refus) { refuses++; return Promise.reject('vignette pas encore en place'); }
          return c === 'vignette_image' ? (window.__sansImage ? Promise.reject('vignette inconnue') : png) : Promise.resolve();
        } },
        event: { listen: (n, f) => { ecoutes[n] = f; return Promise.resolve(() => {}); } },
      };
      window.__emettre = (n, payload) => ecoutes[n]({ payload });
    }, o.refus || 0);
    await p.goto(base + (o.csp ? '?csp=1' : ''));
    await p.waitForTimeout(50);
    return p;
  }
  const appels = (p, nom) => p.evaluate(n => window.__appels.filter(a => !n || a[0] === n), nom);
  const emettre = (p, n, x) => p.evaluate(([n, x]) => __emettre(n, x), [n, x]);
  async function prete(p, c = charge()) {
    await emettre(p, 'vignette:preparer', c);
    await p.waitForFunction(() => window.__appels.some(a => a[0] === 'vignette_zone'), null, { timeout: 3000 }).catch(() => {});
  }
  const rect = (p, sel) => p.evaluate(s => { const r = document.querySelector(s).getBoundingClientRect(); return { x: r.x, y: r.y, l: r.width, h: r.height }; }, sel);
  const visibles = (p) => p.$$eval('#carte button', bs => bs.filter(b => b.offsetParent !== null && getComputedStyle(b).opacity > 0.5).map(b => b.dataset.action));

  // 1. Prête : vignette_pret une fois, puis la carte posée invisible et sa zone.
  let p = await page();
  verifier(JSON.stringify(await appels(p)) === JSON.stringify([['vignette_pret', {}]]), '1 vignette_pret une fois, écoute en place');
  {
    // Cœur pas encore en place : les deux premiers « prête » rejetés, le troisième reçu.
    const q = await page({ refus: 2 });
    await q.waitForTimeout(400);
    const n = (await appels(q, 'vignette_pret')).length;
    verifier(n === 3 && q.avert.length === 0, '1 vignette_pret rejeté deux fois : retenté, reçu au troisième, sans bruit', `${n} appel(s), ${q.avert.length} avertissement(s)`);
    await q.context().close();
  }
  await prete(p);
  let z = (await appels(p, 'vignette_zone'))[0]?.[1];
  // Bord de 1 px : l'image fait 258 × 16/10 dans la carte de 260.
  const marge = 12, largeur = 260, hauteur = 258 * 10 / 16 + 2;
  verifier(z && Math.abs(z.x - marge) < 1 && Math.abs(z.y + z.hauteur - (280 - marge)) < 1 && Math.abs(z.largeur - largeur) < 1 && Math.abs(z.hauteur - hauteur) < 1,
    '1 carte bas gauche à 12 px, 260 × 16/10 (jetons)', JSON.stringify(z));
  verifier(await p.evaluate(() => getComputedStyle(carte).visibility) === 'hidden', '1 carte invisible avant entrer');
  verifier(JSON.stringify(z.boutons.map(b => b.action)) === JSON.stringify(['copier', 'enregistrer', 'supprimer', 'fermer']),
    '1 boutons rendus depuis la liste (deux pilules, un rond, la croix ; Annuler caché)', JSON.stringify(z.boutons.map(b => b.action)));
  const pil = z.boutons.find(b => b.action === 'copier');
  verifier(Math.abs(pil.hauteur - 28) < 0.5 && Math.abs(z.boutons.find(b => b.action === 'fermer').largeur - 26) < 0.5, '1 pilule 28 px, rond 26 px', JSON.stringify(pil));
  verifier(await p.evaluate(() => !document.querySelector('[data-action=annoter],[data-action=epingler]')), '1 ni Annoter ni Épingler');

  // 2. Entrée : vignette_entree au premier dessin, boutons cachés avant le survol.
  await emettre(p, 'vignette:entrer', { id: 5 });
  await p.waitForTimeout(60);
  verifier((await appels(p, 'vignette_entree')).length === 1, '2 vignette_entree envoyé');
  const anim = await p.evaluate(() => carte.getAnimations().map(a => [a.effect.getTiming().duration, a.effect.getTiming().easing]));
  verifier(JSON.stringify(anim) === JSON.stringify([[300, 'cubic-bezier(0.34, 1.4, 0.64, 1)']]), '2 entrée 300 ms ressort', JSON.stringify(anim));
  await p.waitForTimeout(400);
  verifier((await visibles(p)).length === 0, '2 boutons invisibles avant le survol');
  await emettre(p, 'vignette:survol', { id: 5, dedans: true });
  await p.waitForTimeout(450);
  verifier(JSON.stringify(await visibles(p)) === JSON.stringify(['copier', 'enregistrer', 'supprimer', 'fermer']), '2 boutons visibles au survol', JSON.stringify(await visibles(p)));
  await p.addStyleTag({ content: `html{background:${FONDS.light} !important}` });
  await p.screenshot({ path: `${SORTIE}/vignette-survol-clair.png` });

  // 3. Clic : vignette_action ; confirmer réussi → « Copié » puis retour ; échoué → rien.
  const c = await rect(p, '[data-action=copier]');
  await p.mouse.click(c.x + c.l / 2, c.y + c.h / 2);
  verifier(JSON.stringify((await appels(p, 'vignette_action')).map(a => a[1])) === JSON.stringify([{ id: 5, action: 'copier' }]), '3 clic Copier → vignette_action');
  await emettre(p, 'vignette:confirmer', { id: 5, action: 'copier', reussi: true });
  await p.waitForTimeout(400);
  verifier(await p.evaluate(() => document.querySelector('[data-action=copier]').classList.contains('fait') && getComputedStyle(document.querySelector('[data-action=copier] .apres')).opacity === '1'), '3 « Copié » montré');
  await p.screenshot({ path: `${SORTIE}/vignette-copie-clair.png` });
  await p.waitForTimeout(1600);
  verifier(await p.evaluate(() => !document.querySelector('[data-action=copier]').classList.contains('fait')), '3 retour à « Copier » après 1,8 s');
  await emettre(p, 'vignette:confirmer', { id: 5, action: 'enregistrer', reussi: false });
  verifier(await p.evaluate(() => !document.querySelector('[data-action=enregistrer]').classList.contains('fait')), '3 échec : rien ne change');
  // Un id qui n'est pas le sien : ignoré.
  await emettre(p, 'vignette:confirmer', { id: 99, action: 'enregistrer', reussi: true });
  verifier(await p.evaluate(() => !document.querySelector('[data-action=enregistrer]').classList.contains('fait')), '3 autre id ignoré');

  // 4. Barre : en marche, en pause, absente pour « toujours ».
  await emettre(p, 'vignette:minuterie', { id: 5, restant_ms: 3000, total_ms: 6000, en_marche: true });
  await p.waitForTimeout(300);
  const b1 = await p.evaluate(() => new DOMMatrixReadOnly(getComputedStyle(document.querySelector('#barre i')).transform).a);
  await emettre(p, 'vignette:minuterie', { id: 5, restant_ms: 2400, total_ms: 6000, en_marche: false });
  await p.waitForTimeout(300);
  const b2 = await p.evaluate(() => new DOMMatrixReadOnly(getComputedStyle(document.querySelector('#barre i')).transform).a);
  verifier(b1 < 0.5 && b1 > 0.4 && Math.abs(b2 - 0.4) < 0.01, '4 barre qui avance puis en pause au temps restant', `${b1} ${b2}`);
  await emettre(p, 'vignette:minuterie', { id: 5, restant_ms: 0, total_ms: null, en_marche: true });
  verifier(await p.evaluate(() => document.getElementById('barre').hidden), '4 pas de barre pour « toujours »');

  // 5. Supprimée : seul Annuler, puis rétablie.
  await emettre(p, 'vignette:supprimee', { id: 5, oui: true });
  await p.waitForTimeout(450);
  verifier(JSON.stringify(await visibles(p)) === JSON.stringify(['annuler']), '5 supprimée : seul Annuler', JSON.stringify(await visibles(p)));
  await p.screenshot({ path: `${SORTIE}/vignette-supprimee-clair.png` });
  await emettre(p, 'vignette:supprimee', { id: 5, oui: false });
  await p.waitForTimeout(450);
  verifier(!(await visibles(p)).includes('annuler'), '5 rétablie : Annuler caché');

  // 6. Glisser : appui sur l'image + 5 px → une fois ; 3 px → rien ; pendant glisser : 85 %.
  await emettre(p, 'vignette:survol', { id: 5, dedans: false });
  const im = await rect(p, '#apercu');
  const cx = im.x + 30, cy = im.y + 30;
  await p.mouse.move(cx, cy); await p.mouse.down(); await p.mouse.move(cx + 3, cy); await p.mouse.up();
  verifier((await appels(p, 'vignette_glisser')).length === 0, '6 3 px : pas de glisser');
  await p.mouse.move(cx, cy); await p.mouse.down(); await p.mouse.move(cx + 5, cy); await p.mouse.move(cx + 30, cy); await p.mouse.up();
  verifier(JSON.stringify((await appels(p, 'vignette_glisser')).map(a => a[1])) === JSON.stringify([{ id: 5 }]), '6 5 px : vignette_glisser une fois');
  await emettre(p, 'vignette:glisser', { id: 5, en_cours: true });
  verifier(await p.evaluate(() => getComputedStyle(carte).opacity) === '0.85', '6 carte à 85 % pendant le glisser');
  await emettre(p, 'vignette:glisser', { id: 5, en_cours: false });
  verifier(await p.evaluate(() => document.getElementById('image').draggable === false), '6 image non glissable par la page');

  // 7. Partir : coup de vent, puis vignette_terminee.
  await emettre(p, 'vignette:partir', { id: 5, animee: true });
  await p.waitForTimeout(40);
  const vent = await p.evaluate(() => carte.getAnimations().map(a => { const k = a.effect.getKeyframes(); return [a.effect.getTiming().duration, k[2].transform]; }));
  verifier(vent[0]?.[0] === 280 && /calc\(-125%\)/.test(vent[0][1]) && /rotate\(-6deg\)/.test(vent[0][1]), '7 coup de vent à gauche 280 ms', JSON.stringify(vent));
  await p.waitForTimeout(400);
  verifier((await appels(p, 'vignette_terminee')).length === 1 && await p.evaluate(() => carte.hidden), '7 vignette_terminee puis carte cachée');
  await p.context().close();

  // 8. Coins et retrait, écran à 150 % ; miroir à droite.
  for (const [coin, attendu] of [['bas_droite', (z) => Math.abs(340 - z.x - z.largeur - 12 - 32) < 1 && Math.abs(280 - z.y - z.hauteur - 12) < 1],
    ['haut_gauche', (z) => Math.abs(z.x - 12) < 1 && Math.abs(z.y - 12 - 16) < 1], ['haut_droite', (z) => Math.abs(340 - z.x - z.largeur - 12) < 1 && Math.abs(z.y - 12) < 1]]) {
    p = await page({ echelle: 1.5 });
    const retrait = coin === 'bas_droite' ? { droite: 48 } : coin === 'haut_gauche' ? { haut: 24 } : {};
    await prete(p, charge({ coin, retrait }));
    z = (await appels(p, 'vignette_zone'))[0]?.[1];
    verifier(z && attendu(z), `8 ${coin} (retrait en pixels réels à 150 %)`, JSON.stringify(z && [z.x, z.y, z.largeur, z.hauteur]));
    if (coin === 'haut_droite') {
      await emettre(p, 'vignette:entrer', { id: 5 });
      await p.waitForTimeout(40);
      const e = await p.evaluate(() => carte.getAnimations()[0].effect.getKeyframes()[0].transform);
      verifier(/translateY\(-14px\)/.test(e), '8 entrée depuis le haut pour un coin du haut', e);
      await p.waitForTimeout(400);
      await emettre(p, 'vignette:partir', { id: 5, animee: true });
      await p.waitForTimeout(40);
      const v = await p.evaluate(() => carte.getAnimations()[0].effect.getKeyframes()[2].transform);
      verifier(/calc\(125%\)/.test(v) && /rotate\(6deg\)/.test(v), '8 coup de vent en miroir à droite', v);
    }
    await p.context().close();
  }

  // 9. Animations réduites (charge du cœur) : fondus de 120 ms.
  p = await page();
  await prete(p, charge({ reduites: true }));
  await emettre(p, 'vignette:entrer', { id: 5 });
  await p.waitForTimeout(30);
  let r = await p.evaluate(() => carte.getAnimations().map(a => [a.effect.getTiming().duration, a.effect.getKeyframes().map(k => k.transform ?? '-').join()]));
  verifier(r[0]?.[0] === 120 && !/translate/.test(r[0][1]), '9 réduites : entrée en fondu de 120 ms', JSON.stringify(r));
  await p.waitForTimeout(200);
  await emettre(p, 'vignette:partir', { id: 5, animee: true });
  await p.waitForTimeout(30);
  r = await p.evaluate(() => carte.getAnimations().map(a => a.effect.getTiming().duration));
  verifier(r[0] === 120, '9 réduites : départ en fondu de 120 ms', JSON.stringify(r));
  await p.context().close();
  // Le cœur décide seul : système « réduit » mais charge `reduites: false` → entrée complète.
  p = await page({ reduit: true });
  await prete(p, charge({ reduites: false }));
  await emettre(p, 'vignette:entrer', { id: 5 });
  await p.waitForTimeout(30);
  r = await p.evaluate(() => carte.getAnimations().map(a => a.effect.getKeyframes().map(k => k.transform ?? '-').join()));
  verifier(r.some(t => /translate/.test(t)), '9 la page suit le cœur, pas le système', JSON.stringify(r));
  await p.context().close();

  // 10. Charges invalides refusées ; textes posés en texte ; image absente : pas de zone.
  p = await page();
  const invalides = [null, 'x', { ...charge(), id: 'a' }, { ...charge(), coin: 'milieu' }, { ...charge(), actions: [] },
    { ...charge(), actions: [{ id: 'x', libelle: '', place: 'principale' }] }, { ...charge(), apercu: {} }];
  for (const c of invalides) await emettre(p, 'vignette:preparer', c);
  await p.waitForTimeout(200);
  verifier((await appels(p, 'vignette_image')).length === 0 && p.avert.length >= invalides.length - 2, '10 charges invalides rejetées avec console.warn', String(p.avert.length));
  await prete(p, charge({ actions: [{ id: 'copier', libelle: '<img src=x onerror="window.pwn=1">', libelle_fait: '<b>x</b>', place: 'principale', picto: '<svg>' }] }));
  const inj = await p.evaluate(() => ({ pwn: !!window.pwn, balises: carte.querySelectorAll('img:not(#image), b').length, texte: carte.querySelector('.avant').textContent }));
  verifier(!inj.pwn && inj.balises === 0 && inj.texte.startsWith('<img'), '10 textes posés en texte', JSON.stringify(inj));
  await p.context().close();
  p = await page();
  await p.evaluate(() => { window.__sansImage = true; });
  await prete(p);
  verifier((await appels(p, 'vignette_zone')).length === 0 && await p.evaluate(() => carte.hidden), '10 aperçu absent : aucune zone (le filet du cœur tombe)');
  await p.context().close();

  // 11. Cachée par le cœur (nouvelle capture) : rien ne répond ensuite.
  p = await page();
  await prete(p);
  await emettre(p, 'vignette:entrer', { id: 5 });
  await emettre(p, 'vignette:cachee', { id: 5 });
  await emettre(p, 'vignette:partir', { id: 5, animee: true });
  await p.waitForTimeout(400);
  verifier((await appels(p, 'vignette_terminee')).length === 0 && await p.evaluate(() => carte.hidden), '11 cachée : plus rien pour cette carte');
  await p.context().close();

  // 12. Thème sombre, et CSP de l'app respectée (blob: pour l'aperçu).
  p = await page({ theme: 'dark' });
  await prete(p);
  await emettre(p, 'vignette:entrer', { id: 5 });
  await emettre(p, 'vignette:survol', { id: 5, dedans: true });
  await p.waitForTimeout(500);
  await p.addStyleTag({ content: `html{background:${FONDS.dark} !important}` });
  await p.screenshot({ path: `${SORTIE}/vignette-survol-sombre.png` });
  await emettre(p, 'vignette:supprimee', { id: 5, oui: true });
  await p.waitForTimeout(450);
  await p.screenshot({ path: `${SORTIE}/vignette-supprimee-sombre.png` });
  await p.context().close();
  p = await page({ theme: 'dark', csp: true });
  await prete(p);
  await emettre(p, 'vignette:entrer', { id: 5 });
  await p.waitForTimeout(400);
  const refus = await p.evaluate(() => window.__refus);
  const fond = await p.evaluate(() => getComputedStyle(carte).backgroundColor);
  verifier(refus.length === 0 && fond === 'rgb(44, 44, 44)' && await p.evaluate(() => document.getElementById('image').naturalWidth === 64),
    '12 CSP respectée, aperçu chargé (blob:), fond uni sombre', JSON.stringify(refus) + ' ' + fond);
  await p.context().close();

  // 13. Annoter (0.4, quand le cœur le met dans la liste) : rond, avant Supprimer, nom
  // « Annoter », clic → vignette_action annoter.
  p = await page();
  const AVEC_ANNOTER = [...ACTIONS.slice(0, 2),
    { id: 'annoter', libelle: 'Annoter', libelle_fait: null, place: 'secondaire', picto: 'annoter' }, ...ACTIONS.slice(2)];
  await prete(p, charge({ actions: AVEC_ANNOTER }));
  await emettre(p, 'vignette:entrer', { id: 5 });
  await emettre(p, 'vignette:survol', { id: 5, dedans: true });
  await p.waitForTimeout(450);
  verifier(JSON.stringify(await visibles(p)) === JSON.stringify(['copier', 'enregistrer', 'annoter', 'supprimer', 'fermer']),
    '13 Annoter avant Supprimer', JSON.stringify(await visibles(p)));
  const ronds = await p.$$eval('.secondaires button', bs => bs.map(b => b.dataset.action));
  const nom = await p.evaluate(() => { const b = document.querySelector('[data-action=annoter]'); return b.getAttribute('aria-label') || b.title || b.textContent.trim(); });
  verifier(JSON.stringify(ronds) === JSON.stringify(['annoter', 'supprimer']) && nom === 'Annoter' && await p.evaluate(() => !!document.querySelector('[data-action=annoter] svg')),
    '13 premier rond, nom « Annoter », picto crayon', JSON.stringify(ronds) + ' ' + nom);
  const an = await rect(p, '[data-action=annoter]');
  await p.mouse.click(an.x + an.l / 2, an.y + an.h / 2);
  verifier(JSON.stringify((await appels(p, 'vignette_action')).map(a => a[1])) === JSON.stringify([{ id: 5, action: 'annoter' }]), '13 clic Annoter → vignette_action annoter');
  verifier(await p.evaluate(() => !document.querySelector('[data-action=epingler]')), '13 jamais Épingler');
  await p.context().close();

  // 14. La carte d'un film (0.7, F25, §12) : nom « Enregistrement terminé », rond de
  // lecture au centre (34 px ; choix B2 : bouton « Lire »), durée en bas à droite effacée au
  // survol, libellés accordés, jamais Annoter ; sans `film`, la carte d'une image.
  const actionsFilm = (gif, lire = true) => [
    { id: 'copier', libelle: 'Copier', libelle_fait: gif ? 'Copié' : 'Copiée', place: 'principale', picto: 'copier' },
    { id: 'enregistrer', libelle: 'Enregistrer', libelle_fait: gif ? 'Enregistré' : 'Enregistrée', place: 'principale', picto: 'enregistrer' },
    ...(lire ? [{ id: 'lire', libelle: 'Lire', libelle_fait: null, place: 'lecture', picto: 'lire' }] : []),
    ...ACTIONS.slice(2),
  ];
  const chargeFilm = (gif, o = {}) => charge({ actions: actionsFilm(gif, o.lire !== false), film: { duree: gif ? 'GIF · 0:07' : '0:07', nom: 'Enregistrement terminé' }, ...o.charge });
  const etatFilm = (p) => p.evaluate(() => {
    const f = carte.querySelector('.film'), rond = f?.querySelector('.lecture'), d = f?.querySelector('.duree');
    const r = (e) => { const b = e.getBoundingClientRect(); return [b.x, b.y, b.width, b.height]; };
    const a = document.getElementById('apercu').getBoundingClientRect();
    return {
      nom: carte.getAttribute('aria-label'), role: carte.getAttribute('role'),
      rond: rond && { balise: rond.tagName, nom: rond.getAttribute('aria-label'), cache: rond.getAttribute('aria-hidden'), r: r(rond), fond: getComputedStyle(rond).backgroundColor, couleur: getComputedStyle(rond).color, opacite: getComputedStyle(f).opacity },
      duree: d && { texte: d.textContent, r: r(d), opacite: getComputedStyle(d).opacity, fond: getComputedStyle(d).backgroundColor, police: [getComputedStyle(d).fontSize, getComputedStyle(d).fontWeight, getComputedStyle(d).lineHeight] },
      apercu: [a.x, a.y, a.width, a.height],
    };
  });
  {
    for (const theme of ['light', 'dark']) {
      p = await page({ theme });
      await prete(p, chargeFilm(false));
      z = (await appels(p, 'vignette_zone'))[0]?.[1];
      verifier(JSON.stringify(z?.boutons.map(b => b.action)) === JSON.stringify(['lire', 'copier', 'enregistrer', 'supprimer', 'fermer']),
        `14 ${theme} vidéo : boutons de la carte, le rond (« lire ») compris, ni Annoter`, JSON.stringify(z?.boutons.map(b => b.action)));
      const lire = z.boutons.find(b => b.action === 'lire');
      verifier(Math.abs(lire.largeur - 34) < 0.5 && Math.abs(lire.hauteur - 34) < 0.5 && Math.abs(lire.x + 17 - (z.x + z.largeur / 2)) < 1,
        `14 ${theme} rond de 34 px, centré`, JSON.stringify(lire));
      await emettre(p, 'vignette:entrer', { id: 5 });
      await p.waitForTimeout(400);
      let e = await etatFilm(p);
      const [ax, ay, al, ah] = e.apercu, [dx, dy, dl, dh] = e.duree.r, [rx, ry, rl, rh] = e.rond.r;
      verifier(e.nom === 'Enregistrement terminé' && e.role === 'group' && e.rond.balise === 'BUTTON' && e.rond.nom === 'Lire' && e.rond.cache === null
        && e.rond.fond === 'rgba(18, 18, 18, 0.48)' && e.rond.couleur === 'rgb(255, 255, 255)'
        && Math.abs(rx + rl / 2 - (ax + al / 2)) < 0.5 && Math.abs(ry + rh / 2 - (ay + ah / 2)) < 0.5,
      `14 ${theme} vidéo : carte « Enregistrement terminé », rond « Lire » au centre de l'aperçu (fond sombre, triangle blanc)`, JSON.stringify(e));
      verifier(e.duree.texte === '0:07' && e.duree.opacite === '1' && Math.abs(ax + al - dx - dl - 6) < 0.5 && Math.abs(ay + ah - dy - dh - 6) < 0.5
        && dh === 18 && e.duree.fond === 'rgba(18, 18, 18, 0.6)' && JSON.stringify(e.duree.police) === '["11px","600","16px"]',
      `14 ${theme} vidéo : « 0:07 » en bas à droite à 6 px, 11 px gras`, JSON.stringify(e.duree));
      await p.addStyleTag({ content: `html{background:${FONDS[theme]} !important}` });
      const nomTheme = theme === 'light' ? 'clair' : 'sombre';
      await p.screenshot({ path: `${SORTIE}/vignette-film-${nomTheme}.png` });
      await emettre(p, 'vignette:survol', { id: 5, dedans: true });
      await p.waitForTimeout(450);
      e = await etatFilm(p);
      verifier(e.duree.opacite === '0' && e.rond.opacite === '1' && JSON.stringify(await visibles(p)) === JSON.stringify(['lire', 'copier', 'enregistrer', 'supprimer', 'fermer']),
        `14 ${theme} survol : la durée s'efface, le rond reste, Copier, Enregistrer, Supprimer, Fermer`, JSON.stringify([e.duree, await visibles(p)]));
      await p.screenshot({ path: `${SORTIE}/vignette-film-survol-${nomTheme}.png` });
      // Clic sur le rond : vignette_action lire, pas de glisser.
      const [cx, cy] = [rx + rl / 2, ry + rh / 2];
      await p.mouse.move(cx, cy); await p.mouse.down(); await p.mouse.move(cx + 1, cy); await p.mouse.up();
      await p.mouse.move(cx, cy); await p.mouse.down(); await p.mouse.move(cx + 20, cy, { steps: 4 }); await p.mouse.up();
      verifier(JSON.stringify((await appels(p, 'vignette_action')).map(a => a[1])) === JSON.stringify([{ id: 5, action: 'lire' }]) && (await appels(p, 'vignette_glisser')).length === 0,
        `14 ${theme} clic sur le rond → vignette_action lire, une fois ; appui sur le rond : pas de glisser`);
      // Glisser depuis l'image, hors du rond (et depuis la durée) : la carte donne son fichier.
      await emettre(p, 'vignette:survol', { id: 5, dedans: false });
      await p.waitForTimeout(300);
      await p.mouse.move(ax + 20, ay + 20); await p.mouse.down(); await p.mouse.move(ax + 40, ay + 20, { steps: 4 }); await p.mouse.up();
      await p.mouse.move(dx + 5, dy + 5); await p.mouse.down(); await p.mouse.move(dx - 20, dy + 5, { steps: 4 }); await p.mouse.up();
      verifier((await appels(p, 'vignette_glisser')).length === 2, `14 ${theme} glisser depuis l'image et depuis la durée : vignette_glisser`);
      // « Copiée » (accordé).
      await emettre(p, 'vignette:survol', { id: 5, dedans: true });
      await emettre(p, 'vignette:confirmer', { id: 5, action: 'copier', reussi: true });
      await p.waitForTimeout(400);
      verifier(await p.evaluate(() => document.querySelector('[data-action=copier] .apres').textContent) === 'Copiée', `14 ${theme} « Copiée » pour une vidéo`);
      // Supprimée : le rond et la durée partent, seul Annuler.
      await emettre(p, 'vignette:supprimee', { id: 5, oui: true });
      await p.waitForTimeout(450);
      e = await etatFilm(p);
      const restent = (await visibles(p)).filter(x => x !== 'lire'); // le rond : calque à 0
      verifier(e.rond.opacite === '0' && JSON.stringify(restent) === JSON.stringify(['annuler']), `14 ${theme} supprimée : ni rond ni durée, seul Annuler`, JSON.stringify([e.rond, restent]));
      await p.mouse.click(cx, cy);
      verifier((await appels(p, 'vignette_action')).filter(a => a[1].action === 'lire').length === 1, `14 ${theme} supprimée : le rond ne répond plus`);
      await p.context().close();
    }
    // GIF : « GIF · 0:07 », « Enregistré ».
    p = await page();
    await prete(p, chargeFilm(true));
    await emettre(p, 'vignette:entrer', { id: 5 });
    await p.waitForTimeout(400);
    let e = await etatFilm(p);
    await p.addStyleTag({ content: `html{background:${FONDS.light} !important}` });
    await p.screenshot({ path: `${SORTIE}/vignette-gif-clair.png` });
    await emettre(p, 'vignette:confirmer', { id: 5, action: 'enregistrer', reussi: true });
    await p.waitForTimeout(400);
    const fait = await p.evaluate(() => document.querySelector('[data-action=enregistrer] .apres').textContent);
    verifier(e.duree.texte === 'GIF · 0:07' && fait === 'Enregistré', '14 GIF : « GIF · 0:07 », « Enregistré »', JSON.stringify([e.duree.texte, fait]));
    // Puis la carte d'une image : celle de la 0.6.1 (ni rond, ni durée, nom « Capture »).
    await emettre(p, 'vignette:cachee', { id: 5 });
    await prete(p, charge({ id: 6 }));
    e = await etatFilm(p);
    verifier(e.nom === 'Capture' && e.rond === undefined && e.duree === undefined && await p.evaluate(() => !document.querySelector('.film')),
      '14 carte d\'une image ensuite : nom « Capture », ni rond ni durée', JSON.stringify(e));
    await p.context().close();
    // Sans le choix B2 (aucune action « lecture ») : le rond n'est qu'un signe.
    p = await page();
    await prete(p, chargeFilm(false, { lire: false }));
    z = (await appels(p, 'vignette_zone'))[0]?.[1];
    e = await etatFilm(p);
    verifier(e.rond.balise === 'SPAN' && e.rond.cache === 'true' && !z.boutons.some(b => b.action === 'lire'),
      '14 sans « lecture » : le rond est un signe caché aux lecteurs d\'écran, pas un bouton', JSON.stringify(e.rond));
    await p.context().close();
    // Charges invalides : « lecture » sans film rejetée ; film mal formé : carte rejetée.
    p = await page();
    await prete(p, charge({ actions: actionsFilm(false) }));
    const sansFilm = await p.evaluate(() => [!!document.querySelector('[data-action=lire]'), !!document.querySelector('.film'), carte.getAttribute('aria-label')]);
    const avert = p.avert.length;
    for (const film of [{ duree: '', nom: 'x' }, { duree: '0:07' }, 'x', 3]) await emettre(p, 'vignette:preparer', charge({ id: 9, actions: actionsFilm(false), film }));
    await p.waitForTimeout(200);
    const images = (await appels(p, 'vignette_image')).map(a => a[1].id);
    verifier(JSON.stringify(sansFilm) === JSON.stringify([false, false, 'Capture']) && avert === 1 && p.avert.length === 5 && !images.includes(9),
      '14 « lecture » sans film : action rejetée (console.warn) ; film mal formé : carte rejetée', JSON.stringify({ sansFilm, avert, n: p.avert.length, images }));
    await p.context().close();
    // CSP de l'app, sombre : aucun refus avec la carte d'un film.
    p = await page({ theme: 'dark', csp: true });
    await prete(p, chargeFilm(true));
    await emettre(p, 'vignette:entrer', { id: 5 });
    await p.waitForTimeout(400);
    verifier((await p.evaluate(() => window.__refus)).length === 0, '14 CSP respectée avec la carte d\'un film');
    await p.context().close();
  }

  console.log('ECHECS :', echecs);
  console.log('ERREURS PAGE :', JSON.stringify(erreurs));
  if (erreurs.length) echecs++;
  await nav.close(); serveur.close();
  process.exitCode = echecs === 0 ? 0 : 1;
})().catch((e) => { console.error('essai interrompu :', e); process.exit(1); });
