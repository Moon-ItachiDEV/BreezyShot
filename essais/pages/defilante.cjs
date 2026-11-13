const { chromium } = require('playwright');
const http = require('http'), fs = require('fs'), path = require('path');
// Essai de la page de contrôle de la capture défilante dans Chromium (Playwright), servie
// avec la CSP de l'app, le cœur simulé par un `__TAURI__` qui note les appels et rend des
// PNG connus pour `defilante_apercu` (contrat 0.6, §11, §12, §14 E). Sort avec 1 si une
// vérification échoue. Captures dans $SORTIE (par défaut essais-resultats/pages).
//   node essais/pages/defilante.cjs
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
  if (!q.url.includes('sanscsp=1')) entetes['content-security-policy'] = CSP;
  r.writeHead(200, entetes); fs.createReadStream(p).pipe(r);
});
const TERMINER = 'Terminer';
const TROP_VITE = 'Trop vite. Remontez un peu.';
const VIEWPORT = { width: 1000, height: 700 };
const ACCENT = { light: 'rgb(0, 103, 192)', dark: 'rgb(76, 194, 255)' };
const PILULE_FOND = { light: 'rgb(249, 249, 249)', dark: 'rgb(44, 44, 44)' };
const FONDS = { light: 'linear-gradient(135deg,#8FBDF7,#B3A4F5 50%,#DCE7F6)', dark: 'linear-gradient(135deg,#1D4E9E,#47308E 50%,#0B1830)' };

/** Une charge `defilante:montrer` en pixels réels pour l'échelle `k` (placement du §10). */
function charge(id, k, o = {}) {
  const r = Math.round;
  const zone = { x: r(160 * k), y: r(120 * k), largeur: r(480 * k), hauteur: r(360 * k) };
  return {
    id, echelle: k, zone,
    apercu: { x: zone.x + zone.largeur + r(12 * k), y: zone.y, largeur: r(84 * k), hauteur_max: r(500 * k) },
    pilule: { x: zone.x + r(zone.largeur / 2) - r(50 * k), y: zone.y + zone.hauteur + r(10 * k) },
    libelle: TERMINER, reduites: false, ...o,
  };
}

// Le cœur simulé (dans la page, avant ses scripts).
function simuler(o) {
  window.__appels = []; window.__ecoutes = {}; window.__refus = [];
  window.__hauteurs = {}; window.__retard = 0; window.__largeurApercu = 84;
  document.addEventListener('securitypolicyviolation', (e) => window.__refus.push(e.violatedDirective + ' ' + e.blockedURI));
  const COULEURS = ['#2060E0', '#20A040', '#E08020'];
  async function png(largeur, hauteur, numero) {
    const c = document.createElement('canvas');
    c.width = largeur; c.height = hauteur;
    const g = c.getContext('2d');
    g.fillStyle = COULEURS[numero % 3]; g.fillRect(0, 0, largeur, hauteur);
    g.fillStyle = '#FF00FF'; g.fillRect(0, hauteur - 12, largeur, 12); // le bas de l'image
    const b = await new Promise((ok) => c.toBlob(ok, 'image/png'));
    return b.arrayBuffer();
  }
  window.__TAURI__ = {
    core: {
      invoke: async (c, a) => {
        window.__appels.push([c, a]);
        if (c === 'defilante_pret') return { reduites: !!o.reduites };
        if (c === 'defilante_apercu') {
          await new Promise((ok) => setTimeout(ok, window.__retard));
          const h = window.__hauteurs[a.numero];
          if (h === undefined) throw 'defilante';
          if (window.__illisibles?.includes(a.numero)) return new Uint8Array([137, 80, 78, 71, 1, 2, 3]).buffer;
          if (window.__apercuPng) return Uint8Array.from(atob(window.__apercuPng), (x) => x.charCodeAt(0)).buffer;
          return png(window.__largeurApercu, h, a.numero);
        }
        return null;
      },
    },
    event: { listen: (n, f) => { window.__ecoutes[n] = f; return Promise.resolve(() => {}); } },
  };
  window.__emettre = (n, payload) => window.__ecoutes[n]({ payload });
}

(async () => {
  await new Promise((ok) => serveur.listen(0, ok));
  const base = `http://127.0.0.1:${serveur.address().port}/defilante/defilante.html`;
  const nav = await chromium.launch();
  const erreurs = [];
  let echecs = 0;
  const verifier = (ok, quoi, info = '') => { console.log(ok ? 'OK   ' : 'ECHEC', quoi, info); if (!ok) echecs++; };

  // Lecture de pixels d'une capture PNG, dans une page à part (sans CSP).
  const aide = await nav.newPage();
  const pixels = (png, points) => aide.evaluate(async ([b64, pts]) => {
    const bin = atob(b64); const u = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) u[i] = bin.charCodeAt(i);
    const bmp = await createImageBitmap(new Blob([u]));
    const c = new OffscreenCanvas(bmp.width, bmp.height); const g = c.getContext('2d');
    g.drawImage(bmp, 0, 0);
    return pts.map(([x, y]) => Array.from(g.getImageData(x, y, 1, 1).data));
  }, [png.toString('base64'), points]);

  async function ouvrir(o = {}) {
    const ctx = await nav.newContext({ viewport: VIEWPORT, deviceScaleFactor: o.k || 1, colorScheme: o.theme || 'light', reducedMotion: o.reduit ? 'reduce' : 'no-preference' });
    const p = await ctx.newPage();
    p.avert = [];
    p.on('pageerror', (e) => erreurs.push(String(e)));
    p.on('console', (m) => {
      if (m.type() === 'warning') p.avert.push(m.text());
      if (m.type() === 'error' || (m.type() === 'warning' && !o.avertissements)) erreurs.push(m.text());
    });
    await p.addInitScript(simuler, { reduites: !!o.reduites });
    await p.goto(base + (o.sanscsp ? '?sanscsp=1' : ''));
    await p.waitForFunction(() => window.__appels.some((a) => a[0] === 'defilante_pret'));
    await p.waitForTimeout(30);
    return p;
  }
  const appels = (p, nom) => p.evaluate((n) => window.__appels.filter((a) => !n || a[0] === n), nom);
  const fermer = async (p) => {
    const refus = await p.evaluate(() => window.__refus);
    if (refus.length) erreurs.push('CSP : ' + JSON.stringify(refus));
    await p.context().close();
  };
  const rect = (p, s) => p.$eval(s, (e) => { const r = e.getBoundingClientRect(); return { x: r.x, y: r.y, l: r.width, h: r.height }; });
  const proche = (a, b, e = 0.02) => Math.abs(a - b) <= e;
  const blanc = (px) => px[0] >= 235 && px[1] >= 235 && px[2] >= 235 && px[3] >= 235;
  const voile = (px) => px[0] <= 3 && px[1] <= 3 && px[2] <= 3 && Math.abs(px[3] - 115) <= 3;
  const vide = (px) => px[3] === 0;

  // 1. Prête une fois ; rien n'est montré avant `defilante:montrer`.
  {
    const p = await ouvrir();
    const a = await appels(p);
    const png = await p.screenshot({ omitBackground: true });
    const px = await pixels(png, [[10, 10], [500, 350], [990, 690]]);
    verifier(JSON.stringify(a) === JSON.stringify([['defilante_pret', {}]]) && await p.evaluate(() => scene.hidden && message.hidden) && px.every(vide),
      '1 defilante_pret une fois, écoutes en place ; page vide et transparente avant montrer', JSON.stringify(a));
    await fermer(p);
  }

  // 2. Géométrie, contour, voile, aperçu, pilule : trois échelles, deux thèmes.
  for (const k of [1, 1.25, 1.5]) {
    for (const theme of ['light', 'dark']) {
      const nom = `${k * 100} % ${theme === 'light' ? 'clair' : 'sombre'}`;
      const p = await ouvrir({ k, theme });
      const c = charge(7, k);
      await p.evaluate(([l]) => { window.__largeurApercu = l; }, [c.apercu.largeur]);
      await p.evaluate((c) => __emettre('defilante:montrer', c), c);
      await p.waitForTimeout(450); // entrée de la pilule finie
      const z = await rect(p, '#zone');
      verifier(proche(z.x, c.zone.x / k) && proche(z.y, c.zone.y / k) && proche(z.l, c.zone.largeur / k) && proche(z.h, c.zone.hauteur / k),
        `2 ${nom} : zone à sa place`, JSON.stringify(z));
      const z0 = c.zone, milieuY = z0.y + Math.round(z0.hauteur / 2), milieuX = z0.x + 20;
      const png = await p.screenshot({ omitBackground: true });
      const [dedansG, bordG, traitG, voileG, traitH, dedansH, traitB, dedansB, traitD, dedansD, coin, coin2, centre] = await pixels(png, [
        [z0.x, milieuY], [z0.x + 1, milieuY], [z0.x - 1, milieuY], [z0.x - 10, milieuY],
        [milieuX, z0.y - 1], [milieuX, z0.y], [milieuX, z0.y + z0.hauteur], [milieuX, z0.y + z0.hauteur - 1],
        [z0.x + z0.largeur, milieuY], [z0.x + z0.largeur - 1, milieuY],
        [2, 2], [Math.round(990 * k), Math.round(690 * k)], [z0.x + Math.round(z0.largeur / 2), milieuY]]);
      verifier([dedansG, bordG, dedansH, dedansB, dedansD, centre].every(vide),
        `2 ${nom} : la zone reste claire jusqu'à son bord (aucun trait dedans)`, JSON.stringify([dedansG, bordG, dedansH, dedansB, dedansD]));
      verifier([traitG, traitH, traitB, traitD].every(blanc), `2 ${nom} : trait blanc dehors, sur les quatre côtés`, JSON.stringify([traitG, traitH, traitB, traitD]));
      verifier([voileG, coin, coin2].every(voile), `2 ${nom} : voile --scrim sur le reste de l'écran`, JSON.stringify([voileG, coin, coin2]));
      // Pilule
      const pl = await rect(p, '#pilule');
      const zs = (await appels(p, 'defilante_zone'));
      const b = await p.evaluate(() => ({ texte: terminer.textContent, fond: getComputedStyle(terminer).backgroundColor, couleur: getComputedStyle(terminer).color, pilule: getComputedStyle(pilule).backgroundColor }));
      verifier(proche(pl.x, c.pilule.x / k) && proche(pl.y, c.pilule.y / k) && proche(pl.h, 42) && pl.l >= 100 - 0.02,
        `2 ${nom} : pilule à sa place, 42 px de haut, au moins 100 de large (TAILLE_PILULE_LOGIQUE)`, JSON.stringify(pl));
      verifier(zs.length === 1 && zs[0][1].id === 7 && proche(zs[0][1].x, pl.x, 0.01) && proche(zs[0][1].y, pl.y, 0.01) && proche(zs[0][1].largeur, pl.l, 0.01) && proche(zs[0][1].hauteur, pl.h, 0.01),
        `2 ${nom} : defilante_zone une fois, rectangle réel de la pilule`, JSON.stringify(zs));
      verifier(b.texte === TERMINER && b.fond === ACCENT[theme] && b.pilule === PILULE_FOND[theme],
        `2 ${nom} : « Terminer » sur l'accent, pilule acrylic-solid`, JSON.stringify(b));
      // Aperçu : caché avant la première image, puis à sa place, collé en bas.
      const avant = await p.evaluate(() => apercu.hidden);
      await p.evaluate(() => { window.__hauteurs = { 1: Math.round(200 * devicePixelRatio), 2: 5000 }; });
      await p.evaluate(() => __emettre('defilante:apercu', { id: 7, numero: 1, hauteur: window.__hauteurs[1] }));
      await p.waitForFunction(() => !apercu.hidden);
      let ap = await rect(p, '#apercu');
      const h1 = await p.evaluate(() => window.__hauteurs[1]);
      verifier(avant && proche(ap.x, c.apercu.x / k) && proche(ap.y, c.apercu.y / k) && proche(ap.l, c.apercu.largeur / k) && proche(ap.h, h1 / k),
        `2 ${nom} : aperçu caché avant la première image, puis à sa place`, JSON.stringify(ap));
      await p.evaluate(() => __emettre('defilante:apercu', { id: 7, numero: 2, hauteur: 5000 }));
      await p.waitForFunction(() => apercu.querySelector('img')?.naturalHeight === 5000);
      ap = await rect(p, '#apercu');
      const png2 = await p.screenshot({ omitBackground: true });
      const cx = c.apercu.x + Math.round(c.apercu.largeur / 2), bas = c.apercu.y + c.apercu.hauteur_max;
      const [pxBas, pxHaut] = await pixels(png2, [[cx, bas - Math.round(4 * k)], [cx, bas - Math.round(40 * k)]]);
      const appelsApercu = await appels(p, 'defilante_apercu');
      verifier(proche(ap.h, c.apercu.hauteur_max / k) && pxBas[0] > 240 && pxBas[1] < 15 && pxBas[2] > 240 && pxHaut[0] > 200 && pxHaut[1] > 100 && pxHaut[1] < 160 && pxHaut[2] < 60
        && JSON.stringify(appelsApercu.map((a) => a[1])) === JSON.stringify([{ id: 7, numero: 1 }, { id: 7, numero: 2 }]),
        `2 ${nom} : l'aperçu suit defilante:apercu, borné à hauteur_max, montre le bas de l'image`, JSON.stringify({ ap, pxBas, pxHaut }));
      await fermer(p);
    }
  }

  // 3. Aperçus rapprochés : une demande à la fois, seul le plus récent compte ; périmés,
  //    refusés et d'un autre id ignorés.
  {
    const p = await ouvrir();
    await p.evaluate((c) => { window.__hauteurs = { 3: 310, 4: 320, 5: 330, 6: 340 }; window.__retard = 150; __emettre('defilante:montrer', c); }, charge(9, 1));
    await p.evaluate(() => { for (const n of [3, 4, 5]) __emettre('defilante:apercu', { id: 9, numero: n, hauteur: window.__hauteurs[n] }); });
    await p.waitForTimeout(600);
    await p.evaluate(() => { __emettre('defilante:apercu', { id: 9, numero: 4, hauteur: 320 }); __emettre('defilante:apercu', { id: 8, numero: 6, hauteur: 340 }); __emettre('defilante:apercu', { id: 9, numero: 7, hauteur: 350 }); });
    await p.waitForTimeout(400);
    const n = await p.evaluate(() => ({ appels: __appels.filter((a) => a[0] === 'defilante_apercu').map((a) => a[1].numero), h: apercu.querySelector('img').naturalHeight, boite: apercu.getBoundingClientRect().height, images: apercu.querySelectorAll('img').length }));
    verifier(JSON.stringify(n.appels) === JSON.stringify([3, 5, 7]) && n.h === 330 && n.boite === 330 && n.images === 1,
      '3 aperçus : 3 puis 5 (4 sauté), 4 périmé et id 8 ignorés, 7 refusé sans effet', JSON.stringify(n));
    await fermer(p);
  }

  // 3 bis. Un PNG illisible ne montre rien : ni bande vide, ni changement de l'aperçu montré.
  {
    const p = await ouvrir();
    await p.evaluate((c) => { window.__hauteurs = { 1: 200, 2: 220, 3: 240 }; window.__illisibles = [1, 3]; __emettre('defilante:montrer', c); }, charge(10, 1));
    await p.evaluate(() => __emettre('defilante:apercu', { id: 10, numero: 1, hauteur: 200 }));
    await p.waitForTimeout(300);
    const avant = await p.evaluate(() => [apercu.hidden, apercu.querySelectorAll('img').length]);
    await p.evaluate(() => __emettre('defilante:apercu', { id: 10, numero: 2, hauteur: 220 }));
    await p.waitForFunction(() => !apercu.hidden);
    await p.evaluate(() => __emettre('defilante:apercu', { id: 10, numero: 3, hauteur: 240 }));
    await p.waitForTimeout(300);
    const apres = await p.evaluate(() => [apercu.hidden, apercu.querySelector('img').naturalHeight, apercu.getBoundingClientRect().height]);
    verifier(avant[0] && avant[1] === 0 && !apres[0] && apres[1] === 220 && apres[2] === 220,
      '3 bis PNG illisible : aucune bande vide, l\'aperçu montré reste', JSON.stringify({ avant, apres }));
    await fermer(p);
  }

  // 3 ter. Échelle de la fenêtre différente de celle de la charge (fenêtre née sur un autre
  //    écran) : la zone attend devicePixelRatio = echelle (borné), et repart à chaque
  //    changement d'échelle avec le rectangle juste.
  {
    const zones = (p) => appels(p, 'defilante_zone').then((a) => a.map((x) => x[1]));
    const echelle = (p, k) => p.context().newCDPSession(p).then((cdp) => cdp.send('Emulation.setDeviceMetricsOverride', { width: VIEWPORT.width, height: VIEWPORT.height, deviceScaleFactor: k, mobile: false }));
    // (a) l'échelle arrive après montrer : une seule zone, la juste.
    let p = await ouvrir({ k: 1 });
    let c = charge(21, 1.5);
    await p.evaluate((c) => __emettre('defilante:montrer', c), c);
    await p.waitForTimeout(100);
    const pendant = await zones(p);
    await echelle(p, 1.5);
    await p.waitForTimeout(500);
    let z = await zones(p);
    verifier(pendant.length === 0 && z.length === 1 && proche(z[0].x, c.pilule.x / 1.5, 0.01) && proche(z[0].y, c.pilule.y / 1.5, 0.01),
      '3 ter (a) échelle arrivée après montrer : rien avant, puis une zone juste', JSON.stringify({ pendant, z }));
    await fermer(p);
    // (b) l'échelle n'arrive jamais : la zone part quand même après la borne.
    p = await ouvrir({ k: 1 });
    await p.evaluate((c) => __emettre('defilante:montrer', c), charge(22, 1.5));
    await p.waitForTimeout(150);
    const tot = await zones(p);
    await p.waitForTimeout(450);
    z = await zones(p);
    verifier(tot.length === 0 && z.length === 1, '3 ter (b) échelle jamais atteinte : zone envoyée après 300 ms', JSON.stringify({ tot, z }));
    await fermer(p);
    // (c) l'échelle change après l'envoi : un second rectangle, juste.
    p = await ouvrir({ k: 1.5 });
    c = charge(23, 1.5);
    await p.evaluate((c) => __emettre('defilante:montrer', c), c);
    await p.waitForTimeout(400);
    await echelle(p, 1.25);
    await p.waitForTimeout(500);
    z = await zones(p);
    verifier(z.length === 2 && proche(z[0].x, c.pilule.x / 1.5, 0.01) && proche(z[1].x, c.pilule.x / 1.25, 0.01) && proche(z[1].y, c.pilule.y / 1.25, 0.01),
      '3 ter (c) échelle changée après l\'envoi : second defilante_zone, juste', JSON.stringify(z));
    await fermer(p);
  }

  // 4. « Terminer » : un clic → defilante_terminer une fois ; focus jamais pris.
  {
    const p = await ouvrir();
    await p.evaluate((c) => __emettre('defilante:montrer', c), charge(11, 1));
    await p.waitForTimeout(400);
    const pl = await rect(p, '#pilule'), cx = pl.x + pl.l / 2, cy = pl.y + pl.h / 2;
    // Appuis qui ne doivent rien faire : clic droit, appui dehors relâché dessus, appui
    // dessus relâché dehors, clic sur la zone et sur le voile.
    await p.mouse.click(cx, cy, { button: 'right' });
    await p.mouse.move(5, 5); await p.mouse.down(); await p.mouse.move(cx, cy); await p.mouse.up();
    await p.mouse.move(cx, cy); await p.mouse.down(); await p.mouse.move(5, 5); await p.mouse.up();
    await p.mouse.click(400, 300); await p.mouse.click(5, 5);
    const rien = await appels(p, 'defilante_terminer');
    await p.mouse.click(cx, cy); await p.waitForTimeout(50);
    await p.mouse.dblclick(cx, cy); await p.mouse.click(cx, cy); await p.waitForTimeout(50);
    const t = await appels(p, 'defilante_terminer');
    const focus = await p.evaluate(() => [document.activeElement === document.body, terminer.tabIndex]);
    const ctx = await p.evaluate(() => { const e = new MouseEvent('contextmenu', { bubbles: true, cancelable: true }); terminer.dispatchEvent(e); return e.defaultPrevented; });
    verifier(rien.length === 0, '4 clic droit, appuis glissés, clics hors de la pilule : rien', JSON.stringify(rien));
    verifier(JSON.stringify(t) === JSON.stringify([['defilante_terminer', { id: 11 }]]), '4 clic sur « Terminer » : defilante_terminer une fois (double clic compris)', JSON.stringify(t));
    verifier(focus[0] && focus[1] === -1 && ctx, '4 focus jamais pris (body actif, tabIndex -1), pas de menu contextuel', JSON.stringify(focus));
    // Le focus forcé est rendu tout de suite ; aucune touche n'est lue.
    await p.evaluate(() => terminer.focus());
    await p.keyboard.press('Enter'); await p.keyboard.press('Space'); await p.keyboard.press('Escape');
    const apres = await p.evaluate(() => [document.activeElement === document.body, __appels.filter((a) => a[0] === 'defilante_terminer').length]);
    verifier(apres[0] && apres[1] === 1, '4 focus forcé rendu, Entrée et Espace sans effet dans la page', JSON.stringify(apres));
    // Clic sans pointeur (lecteur d'écran) : une fois aussi.
    await p.evaluate((c) => __emettre('defilante:montrer', c), charge(12, 1));
    await p.evaluate(() => { terminer.click(); terminer.click(); });
    const lecteur = (await appels(p, 'defilante_terminer')).filter((a) => a[1].id === 12).length;
    verifier(lecteur === 1, '4 clic sans pointeur : une fois', String(lecteur));
    // Le même id montré de nouveau (charge renvoyée, page rechargée) : pas de seconde fin.
    await p.evaluate((c) => __emettre('defilante:montrer', c), charge(12, 1));
    await p.waitForTimeout(400);
    const pl2 = await rect(p, '#pilule');
    await p.mouse.click(pl2.x + pl2.l / 2, pl2.y + pl2.h / 2); await p.waitForTimeout(50);
    const meme = (await appels(p, 'defilante_terminer')).filter((a) => a[1].id === 12).length;
    verifier(meme === 1, '4 même id montré deux fois : defilante_terminer une seule fois', String(meme));
    await fermer(p);
  }

  // 5. Message « Trop vite » : montré en haut au centre, forme hud, puis caché.
  for (const theme of ['light', 'dark']) {
    const p = await ouvrir({ theme });
    await p.evaluate((c) => __emettre('defilante:montrer', c), charge(13, 1));
    const avant = await p.evaluate(() => message.hidden);
    await p.evaluate((t) => __emettre('defilante:message', { id: 13, texte: t }), TROP_VITE);
    const m = await p.evaluate(() => { const r = message.getBoundingClientRect(), s = getComputedStyle(message); return { texte: message.textContent, role: message.getAttribute('role'), haut: r.y, centre: r.x + r.width / 2, h: r.height, fond: s.backgroundColor, couleur: s.color, rayon: s.borderRadius, anims: message.getAnimations().length, visible: !message.hidden }; });
    verifier(avant && m.visible && m.texte === TROP_VITE && m.role === 'status' && proche(m.haut, 14) && proche(m.centre, 500, 0.5) && proche(m.h, 30)
      && m.fond === 'rgba(32, 32, 32, 0.7)' && m.couleur === 'rgb(255, 255, 255)' && m.anims === 0,
      `5 message montré (${theme}) : en haut à 14 px, centré, forme hud, sans animation`, JSON.stringify(m));
    await p.evaluate(() => __emettre('defilante:message', { id: 12, texte: 'autre id' }));
    const autre = await p.evaluate(() => message.textContent);
    await p.evaluate(() => __emettre('defilante:message', { id: 13, texte: null }));
    const apres = await p.evaluate(() => [message.hidden, message.textContent]);
    verifier(autre !== 'autre id' && apres[0] && apres[1] === '', `5 message caché par texte null (${theme}) ; un autre id ignoré`, JSON.stringify(apres));
    await fermer(p);
  }

  // 6. Textes posés en texte, jamais en HTML.
  {
    const p = await ouvrir();
    const piege = '<img src=x onerror="window.__pwn=1"><b>gras</b>';
    await p.evaluate(([c, t]) => { __emettre('defilante:montrer', c); __emettre('defilante:message', { id: 14, texte: t }); }, [charge(14, 1, { libelle: piege }), piege + '<script>window.__pwn=2</script>']);
    await p.waitForTimeout(200);
    const r = await p.evaluate(() => ({ pwn: window.__pwn, balises: document.body.querySelectorAll('img,b,script').length, bouton: terminer.textContent, message: message.textContent }));
    verifier(!r.pwn && r.balises === 0 && r.bouton.startsWith('<img') && r.message.endsWith('</script>'), '6 libellé et message posés en texte', JSON.stringify(r));
    const sources = fs.readFileSync(path.join(RACINE, 'defilante', 'defilante.js'), 'utf8').replace(/^\s*\/\/.*$/gm, '');
    verifier(!/innerHTML|outerHTML|insertAdjacentHTML|document\.write/.test(sources), '6 defilante.js n\'utilise jamais innerHTML');
    await fermer(p);
  }

  // 7. Cacher : tout part ; l'ancien id ne reçoit plus rien ; un nouvel affichage repart.
  {
    const p = await ouvrir();
    await p.evaluate((c) => { window.__hauteurs = { 1: 200 }; __emettre('defilante:montrer', c); __emettre('defilante:apercu', { id: 15, numero: 1, hauteur: 200 }); __emettre('defilante:message', { id: 15, texte: 'Trop vite. Remontez un peu.' }); }, charge(15, 1));
    await p.waitForFunction(() => !apercu.hidden);
    await p.evaluate(() => __emettre('defilante:cacher', { id: 14 }));
    const pasCache = await p.evaluate(() => !scene.hidden && !message.hidden);
    await p.evaluate(() => __emettre('defilante:cacher', { id: 15 }));
    const cache = await p.evaluate(() => ({ scene: scene.hidden, message: message.hidden, apercu: apercu.hidden, images: apercu.querySelectorAll('img').length }));
    const png = await p.screenshot({ omitBackground: true });
    const px = await pixels(png, [[10, 10], [400, 300], [820, 300]]);
    await p.evaluate(() => { window.__hauteurs[2] = 210; __emettre('defilante:apercu', { id: 15, numero: 2, hauteur: 210 }); __emettre('defilante:message', { id: 15, texte: 'x' }); });
    await p.waitForTimeout(100);
    const ignore = await p.evaluate(() => ({ message: message.hidden, scene: scene.hidden, apercus: __appels.filter((a) => a[0] === 'defilante_apercu').length }));
    verifier(pasCache && cache.scene && cache.message && cache.apercu && cache.images === 0 && px.every(vide) && ignore.message && ignore.scene && ignore.apercus === 1,
      '7 defilante:cacher efface tout ; un autre id ne cache rien ; l\'ancien id ignoré ensuite', JSON.stringify({ cache, ignore }));
    await p.evaluate((c) => __emettre('defilante:montrer', c), charge(16, 1));
    await p.waitForTimeout(400);
    const z = (await appels(p, 'defilante_zone')).map((a) => a[1].id);
    verifier(JSON.stringify(z) === JSON.stringify([15, 16]) && await p.evaluate(() => apercu.hidden && message.hidden),
      '7 nouvel affichage : defilante_zone une fois pour lui, aperçu et message repartis de rien', JSON.stringify(z));
    await fermer(p);
  }

  // 8. Animations : entrée de la pilule en ressort ; réduites (système, charge, pret) :
  //    fondu de 120 ms, aucun mouvement.
  for (const [quoi, o, c] of [['normales', {}, {}], ['réduites (système)', { reduit: true }, {}], ['réduites (charge)', {}, { reduites: true }], ['réduites (pret)', { reduites: true }, {}]]) {
    const p = await ouvrir(o);
    await p.evaluate((c) => __emettre('defilante:montrer', c), charge(17, 1, c));
    const a = await p.evaluate(() => pilule.getAnimations().map((x) => ({ d: x.effect.getTiming().duration, e: x.effect.getTiming().easing, mouvement: x.effect.getKeyframes().some((k) => k.transform && k.transform !== 'none') })));
    const t = await p.evaluate(() => getComputedStyle(terminer).transitionProperty);
    const attendu = quoi === 'normales' ? [{ d: 260, e: 'cubic-bezier(0.34, 1.4, 0.64, 1)', mouvement: true }] : [{ d: 120, e: 'linear', mouvement: false }];
    verifier(JSON.stringify(a) === JSON.stringify(attendu) && (quoi === 'normales' ? t.includes('transform') : t === 'background-color'),
      `8 animations ${quoi}`, JSON.stringify({ a, t }));
    await p.waitForTimeout(400);
    const z = await appels(p, 'defilante_zone');
    const pl = await rect(p, '#pilule');
    verifier(z.length === 1 && proche(z[0][1].y, pl.y, 0.01), `8 animations ${quoi} : zone envoyée une fois, sans le décalage d'entrée`, JSON.stringify(z));
    await fermer(p);
  }

  // 9. Thème donné par le cœur (interface:theme) : il l'emporte sur le système.
  {
    const p = await ouvrir({ theme: 'light' });
    await p.evaluate((c) => __emettre('defilante:montrer', c), charge(18, 1));
    await p.evaluate(() => __emettre('interface:theme', { theme: 'dark' }));
    await p.waitForTimeout(200); // transition de couleur du bouton
    const sombre = await p.evaluate(() => getComputedStyle(terminer).backgroundColor);
    await p.evaluate(() => __emettre('interface:theme', { theme: null }));
    await p.waitForTimeout(200);
    const systeme = await p.evaluate(() => getComputedStyle(terminer).backgroundColor);
    verifier(sombre === ACCENT.dark && systeme === ACCENT.light, '9 interface:theme sombre puis système', JSON.stringify([sombre, systeme]));
    await fermer(p);
  }

  // 10. Charges invalides : rejetées avec console.warn, rien montré, rien envoyé.
  {
    const p = await ouvrir({ avertissements: true });
    const bonne = charge(19, 1);
    const invalides = [null, 5, {}, { ...bonne, id: -1 }, { ...bonne, id: 'x' }, { ...bonne, zone: null }, { ...bonne, zone: { ...bonne.zone, largeur: 0 } },
      { ...bonne, zone: { ...bonne.zone, x: 1.5 } }, { ...bonne, apercu: { ...bonne.apercu, hauteur_max: -1 } }, { ...bonne, pilule: { x: 'a', y: 1 } },
      { ...bonne, libelle: '' }, { ...bonne, libelle: '  ' }, { ...bonne, libelle: 3 }];
    for (const c of invalides) await p.evaluate((c) => __emettre('defilante:montrer', c), c);
    await p.waitForTimeout(100);
    const r = await p.evaluate(() => [scene.hidden, __appels.filter((a) => a[0] !== 'defilante_pret').length]);
    verifier(r[0] && r[1] === 0 && p.avert.length === invalides.length, '10 charges invalides rejetées (console.warn), rien montré ni envoyé', JSON.stringify(r) + ' avert=' + p.avert.length);
    await fermer(p);
  }

  // Captures (clair et sombre, échelle 2, sans CSP pour poser un fond de bureau).
  for (const theme of ['light', 'dark']) {
    const ctx = await nav.newContext({ viewport: VIEWPORT, deviceScaleFactor: 2, colorScheme: theme });
    const p = await ctx.newPage();
    p.on('pageerror', (e) => erreurs.push(String(e)));
    await p.addInitScript(simuler, {});
    await p.goto(base + '?sanscsp=1');
    await p.waitForFunction(() => window.__appels.some((a) => a[0] === 'defilante_pret'));
    await p.addStyleTag({ content: `html{background:${FONDS[theme]} !important}` });
    const c = charge(20, 2);
    await p.evaluate(([c, t]) => { window.__largeurApercu = c.apercu.largeur; window.__hauteurs = { 1: 600 }; __emettre('defilante:montrer', c); __emettre('defilante:apercu', { id: 20, numero: 1, hauteur: 600 }); __emettre('defilante:message', { id: 20, texte: t }); }, [c, TROP_VITE]);
    await p.waitForTimeout(500);
    const nom = `defilante-${theme === 'light' ? 'clair' : 'sombre'}`;
    await p.screenshot({ path: path.join(SORTIE, `${nom}.png`) });
    console.log('capture', nom);
    await ctx.close();
  }

  verifier(erreurs.length === 0, 'aucune erreur de page, aucun avertissement inattendu, aucune violation de CSP', JSON.stringify(erreurs));
  console.log('ECHECS :', echecs);
  await nav.close(); serveur.close();
  process.exitCode = echecs === 0 ? 0 : 1;
})().catch((e) => { console.error('essai interrompu :', e); process.exit(1); });
