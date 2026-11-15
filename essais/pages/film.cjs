const { chromium } = require('playwright');
const http = require('http'), fs = require('fs'), path = require('path');
// Essai de la page de contrôle d'un film (vidéo et GIF) dans Chromium (Playwright), servie
// avec la CSP de l'app, le cœur simulé par un `__TAURI__` qui note les appels (contrat 0.7,
// §10 à §12, §14 G). Sort avec 1 si une vérification échoue. Captures dans $SORTIE (par
// défaut essais-resultats/pages).
//   node essais/pages/film.cjs
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
// Tailles de la pilule données au placement du cœur, lues dans fenetres/film.rs.
const RS = fs.readFileSync(path.join(__dirname, '..', '..', 'app', 'coeur', 'src', 'fenetres', 'film.rs'), 'utf8');
const taille = (nom) => RS.match(new RegExp(`pub const ${nom}: \\(f64, f64\\) = \\(([0-9.]+), ([0-9.]+)\\);`)).slice(1).map(Number);
const [GIF_L, PILULE_H] = taille('TAILLE_PILULE_LOGIQUE');
const [VIDEO_L] = taille('TAILLE_PILULE_MICRO_LOGIQUE');
const VIEWPORT = { width: 1000, height: 700 };
const PILULE_FOND = { light: 'rgb(249, 249, 249)', dark: 'rgb(44, 44, 44)' };
const REC = { light: 'rgb(224, 57, 62)', dark: 'rgb(255, 90, 95)' };
const GRIS = { light: 'rgb(118, 118, 118)', dark: 'rgb(157, 157, 157)' }; // --text-3
const PAUSE_TEXTE = { light: 'rgb(93, 93, 93)', dark: 'rgb(197, 197, 197)' }; // --text-2
const ACCENT_TEXTE = { light: 'rgb(0, 95, 184)', dark: 'rgb(153, 235, 255)' };
const FONDS = { light: 'linear-gradient(135deg,#8FBDF7,#B3A4F5 50%,#DCE7F6)', dark: 'linear-gradient(135deg,#1D4E9E,#47308E 50%,#0B1830)' };

/** Un état `film:etat`. `micro` : undefined → « Micro » coupé ; null → aucun ; true → ouvert. */
function etat(id, phase, o = {}) {
  const micro = o.micro === null ? null : { libelle: o.micro ? 'Micro activé' : 'Micro', ouvert: !!o.micro };
  return {
    id, phase, duree: o.duree || '0:00',
    pause: { libelle: phase === 'pause' ? 'Reprendre' : 'Pause', actif: phase !== 'decompte' },
    arreter: 'Arrêter', micro, groupe: 'Enregistrement', ...(o.etat || {}),
  };
}
/** Une charge `film:montrer` en pixels réels pour l'échelle `k` (placement du §10 : sous la
 *  zone, à 10 px, bord droit aligné). */
function charge(id, k, o = {}) {
  const r = Math.round;
  const zone = { x: r(160 * k), y: r(120 * k), largeur: r(480 * k), hauteur: r(360 * k) };
  const l = o.micro === null ? GIF_L : VIDEO_L;
  return {
    id, echelle: k, zone,
    pilule: { x: zone.x + zone.largeur - r(l * k), y: zone.y + zone.hauteur + r(10 * k) },
    etat: etat(id, o.phase || 'decompte', o), reduites: false, ...(o.charge || {}),
  };
}

// Le cœur simulé (dans la page, avant ses scripts).
function simuler(o) {
  window.__appels = []; window.__ecoutes = {}; window.__refus = [];
  document.addEventListener('securitypolicyviolation', (e) => window.__refus.push(e.violatedDirective + ' ' + e.blockedURI));
  window.__TAURI__ = {
    core: {
      invoke: async (c, a) => {
        window.__appels.push([c, a]);
        if (c === 'film_pret') return { reduites: !!o.reduites };
        return null;
      },
    },
    event: { listen: (n, f) => { window.__ecoutes[n] = f; return Promise.resolve(() => {}); } },
  };
  window.__emettre = (n, payload) => window.__ecoutes[n]({ payload });
}

(async () => {
  await new Promise((ok) => serveur.listen(0, ok));
  const base = `http://127.0.0.1:${serveur.address().port}/film/film.html`;
  const nav = await chromium.launch();
  const erreurs = [];
  let echecs = 0;
  const verifier = (ok, quoi, info = '') => { console.log(ok ? 'OK   ' : 'ECHEC', quoi, info); if (!ok) echecs++; };
  // Chromium sans tête refuse parfois une capture sous forte charge : on réessaie.
  const cliche = async (p, o = {}) => {
    for (let n = 1; ; n++) {
      try { return await p.screenshot(o); } catch (e) { if (n >= 4 || !/Unable to capture screenshot/.test(String(e))) throw e; await p.waitForTimeout(200); }
    }
  };

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
    await p.waitForFunction(() => window.__appels.some((a) => a[0] === 'film_pret'));
    await p.waitForTimeout(30);
    return p;
  }
  const appels = (p, nom) => p.evaluate((n) => window.__appels.filter((a) => !n || a[0] === n), nom);
  const actions = (p) => appels(p, 'film_action').then((a) => a.map((x) => x[1]));
  const fermer = async (p) => {
    const refus = await p.evaluate(() => window.__refus);
    if (refus.length) erreurs.push('CSP : ' + JSON.stringify(refus));
    await p.context().close();
  };
  const rect = (p, s) => p.$eval(s, (e) => { const r = e.getBoundingClientRect(); return { x: r.x, y: r.y, l: r.width, h: r.height }; });
  const proche = (a, b, e = 0.02) => Math.abs(a - b) <= e;
  const vide = (px) => px[3] === 0;
  const blanc = (px) => px[0] >= 245 && px[1] >= 245 && px[2] >= 245 && px[3] >= 245;
  const noir = (px, alpha, e = 3) => px[0] <= 3 && px[1] <= 3 && px[2] <= 3 && Math.abs(px[3] - alpha) <= e;
  const montrer = (p, c) => p.evaluate((c) => __emettre('film:montrer', c), c);
  const poserEtat = (p, e) => p.evaluate((e) => __emettre('film:etat', e), e);
  const vue = (p) => p.evaluate(() => {
    const s = (e) => getComputedStyle(e);
    const b = (id) => { const e = document.getElementById(id); return { nom: e.getAttribute('aria-label'), cache: e.hidden || e.offsetParent === null, inactif: e.disabled, presse: e.getAttribute('aria-pressed'), picto: e.dataset.picto, couleur: s(e).color }; };
    const point = document.querySelector('.point');
    return {
      groupe: [pilule.getAttribute('role'), pilule.getAttribute('aria-label')],
      duree: duree.textContent, couleurDuree: s(duree).color,
      point: { fond: s(point).backgroundColor, anim: point.getAnimations().map((a) => [a.animationName, a.effect.getTiming().duration, a.effect.getTiming().iterations]) },
      pause: b('pause'), arreter: b('arreter'), micro: b('micro'), trait: trait.hidden || trait.offsetParent === null,
      fond: s(pilule).backgroundColor,
    };
  });

  // 1. Prête une fois ; rien n'est montré avant `film:montrer`.
  {
    const p = await ouvrir();
    const a = await appels(p);
    const png = await cliche(p, { omitBackground: true });
    const px = await pixels(png, [[10, 10], [500, 350], [990, 690]]);
    verifier(JSON.stringify(a) === JSON.stringify([['film_pret', {}]]) && await p.evaluate(() => scene.hidden) && px.every(vide),
      '1 film_pret une fois, écoutes en place ; page vide et transparente avant montrer', JSON.stringify(a));
    await fermer(p);
  }

  // 2. Cadre dehors, voile, pilule et film_zone : trois échelles, deux thèmes, vidéo et GIF.
  for (const k of [1, 1.25, 1.5]) {
    for (const theme of ['light', 'dark']) {
      for (const sorte of ['video', 'gif']) {
        const nom = `${k * 100} % ${theme === 'light' ? 'clair' : 'sombre'} ${sorte}`;
        const p = await ouvrir({ k, theme });
        const c = charge(7, k, sorte === 'gif' ? { micro: null } : {});
        await montrer(p, c);
        await p.waitForTimeout(450); // entrées finies
        const z0 = c.zone, milieuY = z0.y + Math.round(z0.hauteur / 2), milieuX = z0.x + 40;
        const png = await cliche(p, { omitBackground: true });
        const pts = await pixels(png, [
          [z0.x, milieuY], [z0.x + z0.largeur - 1, milieuY], [milieuX, z0.y], [milieuX, z0.y + z0.hauteur - 1], [z0.x + 200, milieuY],
          [z0.x - 1, milieuY], [z0.x + z0.largeur, milieuY], [milieuX, z0.y - 1], [milieuX, z0.y + z0.hauteur],
          [z0.x - 3, milieuY], [z0.x + z0.largeur + 2, milieuY], [milieuX, z0.y - 3], [milieuX, z0.y + z0.hauteur + 2],
          [z0.x - Math.round(4 * k) - 1, milieuY], [z0.x - Math.round(20 * k), milieuY], [2, 2], [Math.round(990 * k), Math.round(690 * k)],
        ]);
        const [dedans, lisere, blancs, dehors, voile] = [pts.slice(0, 5), pts.slice(5, 9), pts.slice(9, 13), pts[13], pts.slice(14)];
        verifier(dedans.every(vide), `2 ${nom} : la zone reste claire jusqu'à son bord (rien dedans)`, JSON.stringify(dedans));
        verifier(lisere.every((x) => noir(x, 46)), `2 ${nom} : liseré sombre de 18 % contre la zone, dehors`, JSON.stringify(lisere));
        verifier(blancs.every(blanc), `2 ${nom} : cadre blanc dehors, sur les quatre côtés`, JSON.stringify(blancs));
        verifier(noir(dehors, 46) && voile.every((x) => noir(x, 46)), `2 ${nom} : voile léger (18 %) sur le reste de l'écran`, JSON.stringify([dehors, voile]));
        const l = sorte === 'gif' ? GIF_L : VIDEO_L;
        const pl = await rect(p, '#pilule');
        const zs = await appels(p, 'film_zone');
        const v = await vue(p);
        verifier(proche(pl.x, c.pilule.x / k) && proche(pl.y, c.pilule.y / k) && proche(pl.h, PILULE_H) && proche(pl.l, l, 0.05),
          `2 ${nom} : pilule à sa place, ${l} × ${PILULE_H} (taille du cœur)`, JSON.stringify(pl));
        verifier(zs.length === 1 && zs[0][1].id === 7 && proche(zs[0][1].x, pl.x, 0.01) && proche(zs[0][1].y, pl.y, 0.01) && proche(zs[0][1].largeur, pl.l, 0.01) && proche(zs[0][1].hauteur, pl.h, 0.01),
          `2 ${nom} : film_zone une fois, rectangle réel de la pilule`, JSON.stringify(zs));
        verifier(v.fond === PILULE_FOND[theme] && v.point.fond === GRIS[theme] && v.point.anim.length === 0 && v.duree === '0:00',
          `2 ${nom} : pilule à fond plein, point gris fixe, « 0:00 » pendant le décompte`, JSON.stringify(v));
        verifier(sorte === 'gif' ? v.micro.cache && v.trait : !v.micro.cache && !v.trait && v.micro.nom === 'Micro' && v.micro.presse === 'false' && v.micro.picto === 'micro_coupe',
          `2 ${nom} : ${sorte === 'gif' ? '« Micro » et son trait absents' : '« Micro » coupé, après un trait'}`, JSON.stringify(v.micro));
        await fermer(p);
      }
    }
  }

  // 3. Chaque phase : décompte, marche, pause, reprise ; textes du cœur ; micro.
  for (const theme of ['light', 'dark']) {
    const p = await ouvrir({ theme });
    await montrer(p, charge(8, 1));
    await p.waitForTimeout(400); // entrée finie
    let v = await vue(p);
    verifier(JSON.stringify(v.groupe) === '["group","Enregistrement"]' && v.pause.nom === 'Pause' && v.pause.inactif && v.pause.picto === 'pause' && v.arreter.nom === 'Arrêter' && !v.arreter.inactif && v.arreter.picto === 'arreter',
      `3 ${theme} décompte : groupe « Enregistrement », « Pause » inactif, « Arrêter »`, JSON.stringify(v));
    await poserEtat(p, etat(8, 'marche', { duree: '0:07' }));
    v = await vue(p);
    verifier(!v.pause.inactif && v.pause.nom === 'Pause' && v.duree === '0:07' && v.point.fond === REC[theme]
      && JSON.stringify(v.point.anim) === JSON.stringify([['respire', 1600, Infinity]]),
    `3 ${theme} marche : point rouge qui respire (1,6 s), durée du cœur, « Pause » actif`, JSON.stringify(v));
    await poserEtat(p, etat(8, 'pause', { duree: '0:07' }));
    v = await vue(p);
    verifier(v.pause.nom === 'Reprendre' && v.pause.picto === 'reprendre' && !v.pause.inactif && v.point.fond === GRIS[theme] && v.point.anim.length === 0 && v.couleurDuree === PAUSE_TEXTE[theme],
      `3 ${theme} pause : « Reprendre » (triangle), point gris fixe, durée grise`, JSON.stringify(v));
    await poserEtat(p, etat(8, 'marche', { duree: '12:05', micro: true }));
    v = await vue(p);
    verifier(v.pause.nom === 'Pause' && v.pause.picto === 'pause' && v.duree === '12:05' && v.micro.nom === 'Micro activé' && v.micro.presse === 'true' && v.micro.picto === 'micro' && v.micro.couleur === ACCENT_TEXTE[theme],
      `3 ${theme} reprise, micro ouvert : « Micro activé », sans barre, couleur d'accent`, JSON.stringify(v));
    const pl = await rect(p, '#pilule');
    verifier(proche(pl.l, VIDEO_L, 0.05), `3 ${theme} « 12:05 » tient dans la largeur de la pilule`, JSON.stringify(pl));
    // Un état d'un autre id, ou invalide, ne change rien.
    await poserEtat(p, etat(9, 'pause', { duree: '9:99' }));
    v = await vue(p);
    verifier(v.duree === '12:05' && v.pause.nom === 'Pause', `3 ${theme} état d'un autre id ignoré`, JSON.stringify(v.duree));
    await fermer(p);
  }

  // 4. Clics : un clic → une film_action ; « Pause » inactif rien ; « Arrêter » une fois ;
  //    focus jamais pris ; touches sans effet.
  {
    const p = await ouvrir();
    await montrer(p, charge(11, 1));
    await p.waitForTimeout(400);
    const centre = async (s) => { const r = await rect(p, s); return [r.x + r.l / 2, r.y + r.h / 2]; };
    const [px, py] = await centre('#pause');
    const [ax, ay] = await centre('#arreter');
    const [mx, my] = await centre('#micro');
    await p.mouse.click(px, py); // inactif pendant le décompte
    await p.mouse.click(mx, my, { button: 'right' });
    await p.mouse.move(5, 5); await p.mouse.down(); await p.mouse.move(mx, my); await p.mouse.up();
    await p.mouse.move(mx, my); await p.mouse.down(); await p.mouse.move(5, 5); await p.mouse.up();
    await p.mouse.click(400, 300); await p.mouse.click(5, 5);
    const rien = await actions(p);
    verifier(rien.length === 0, '4 « Pause » inactif, clic droit, appuis glissés, clics hors de la pilule : rien', JSON.stringify(rien));
    await p.mouse.click(mx, my); // le micro peut s'ouvrir pendant le décompte
    await poserEtat(p, etat(11, 'marche', { duree: '0:01' }));
    await p.mouse.click(px, py);
    await poserEtat(p, etat(11, 'pause', { duree: '0:01' }));
    await p.mouse.click(px, py);
    await p.mouse.dblclick(ax, ay); await p.mouse.click(ax, ay);
    await p.waitForTimeout(50);
    const a = await actions(p);
    verifier(JSON.stringify(a) === JSON.stringify([{ id: 11, action: 'micro' }, { id: 11, action: 'pause' }, { id: 11, action: 'pause' }, { id: 11, action: 'arreter' }]),
      '4 Micro, Pause, Reprendre, Arrêter : une film_action chacun, Arrêter une seule fois', JSON.stringify(a));
    const focus = await p.evaluate(() => [document.activeElement === document.body, [...document.querySelectorAll('button')].every((b) => b.tabIndex === -1)]);
    const ctx = await p.evaluate(() => { const e = new MouseEvent('contextmenu', { bubbles: true, cancelable: true }); arreter.dispatchEvent(e); return e.defaultPrevented; });
    verifier(focus[0] && focus[1] && ctx, '4 focus jamais pris (body actif, tabIndex -1), pas de menu contextuel', JSON.stringify(focus));
    await p.evaluate(() => micro.focus());
    await p.keyboard.press('Enter'); await p.keyboard.press('Space'); await p.keyboard.press('Escape');
    const apres = await p.evaluate(() => [document.activeElement === document.body, __appels.filter((a) => a[0] === 'film_action').length]);
    verifier(apres[0] && apres[1] === 4, '4 focus forcé rendu, Entrée, Espace et Échap sans effet dans la page', JSON.stringify(apres));
    // Clic sans pointeur (lecteur d'écran).
    await montrer(p, charge(12, 1, { phase: 'marche' }));
    await p.evaluate(() => { pause.click(); micro.click(); arreter.click(); arreter.click(); });
    const lecteur = (await actions(p)).filter((x) => x.id === 12).map((x) => x.action);
    verifier(JSON.stringify(lecteur) === '["pause","micro","arreter"]', '4 clic sans pointeur : chaque bouton, Arrêter une fois', JSON.stringify(lecteur));
    // Le même id montré de nouveau : pas de second « Arrêter ».
    await montrer(p, charge(12, 1, { phase: 'marche' }));
    await p.waitForTimeout(400);
    await p.mouse.click(ax, ay); await p.waitForTimeout(50);
    const meme = (await actions(p)).filter((x) => x.id === 12 && x.action === 'arreter').length;
    verifier(meme === 1, '4 même id montré deux fois : « Arrêter » une seule fois', String(meme));
    // GIF : pas de bouton « Micro » à cliquer.
    await montrer(p, charge(13, 1, { micro: null, phase: 'marche' }));
    await p.evaluate(() => micro.click());
    verifier((await actions(p)).filter((x) => x.id === 13).length === 0, '4 GIF : « Micro » caché ne fait rien');
    await fermer(p);
  }

  // 5. Textes posés en texte, jamais en HTML.
  {
    const p = await ouvrir();
    const piege = '<img src=x onerror="window.__pwn=1"><b>gras</b>';
    await montrer(p, charge(14, 1, { duree: piege, etat: { arreter: piege, groupe: piege, pause: { libelle: piege, actif: true } }, micro: false }));
    await p.waitForTimeout(100);
    const r = await p.evaluate(() => ({ pwn: window.__pwn, balises: document.body.querySelectorAll('img,b,script').length, duree: duree.textContent, nom: arreter.getAttribute('aria-label') }));
    verifier(!r.pwn && r.balises === 0 && r.duree.startsWith('<img') && r.nom.startsWith('<img'), '5 durée et noms posés en texte', JSON.stringify(r));
    const sources = fs.readFileSync(path.join(RACINE, 'film', 'film.js'), 'utf8').replace(/^\s*\/\/.*$/gm, '');
    verifier(!/innerHTML|outerHTML|insertAdjacentHTML|document\.write/.test(sources), '5 film.js n\'utilise jamais innerHTML');
    await fermer(p);
  }

  // 6. Cacher : tout part ; l'ancien id ne reçoit plus rien ; un nouvel affichage repart.
  {
    const p = await ouvrir();
    await montrer(p, charge(15, 1, { phase: 'marche' }));
    await p.evaluate(() => __emettre('film:cacher', { id: 14 }));
    const pasCache = await p.evaluate(() => !scene.hidden);
    await p.evaluate(() => __emettre('film:cacher', { id: 15 }));
    const cache = await p.evaluate(() => scene.hidden);
    const png = await cliche(p, { omitBackground: true });
    const px = await pixels(png, [[10, 10], [400, 300], [820, 300]]);
    await poserEtat(p, etat(15, 'pause'));
    await p.evaluate(() => arreter.click());
    const ignore = await p.evaluate(() => [scene.hidden, __appels.filter((a) => a[0] === 'film_action').length]);
    verifier(pasCache && cache && px.every(vide) && ignore[0] && ignore[1] === 0,
      '6 film:cacher efface tout ; un autre id ne cache rien ; l\'ancien id ignoré ensuite', JSON.stringify(ignore));
    await montrer(p, charge(16, 1));
    await p.waitForTimeout(400);
    const z = (await appels(p, 'film_zone')).map((a) => a[1].id);
    verifier(JSON.stringify(z) === JSON.stringify([15, 16]), '6 nouvel affichage : film_zone une fois pour lui', JSON.stringify(z));
    await fermer(p);
  }

  // 7. Animations : entrée du cadre (260 ms, courbe douce) et de la pilule (260 ms, ressort) ;
  //    réduites (système, charge, pret) : aucune, ni entrée ni point qui respire.
  for (const [quoi, o, c] of [['normales', {}, {}], ['réduites (système)', { reduit: true }, {}], ['réduites (charge)', {}, { charge: { reduites: true } }], ['réduites (pret)', { reduites: true }, {}]]) {
    const p = await ouvrir(o);
    await montrer(p, charge(17, 1, c));
    const a = await p.evaluate(() => [cadre, pilule].map((e) => e.getAnimations().map((x) => ({ d: x.effect.getTiming().duration, e: x.effect.getTiming().easing }))));
    const attendu = quoi === 'normales' ? [[{ d: 260, e: 'cubic-bezier(0.2, 0.8, 0.2, 1)' }], [{ d: 260, e: 'cubic-bezier(0.34, 1.4, 0.64, 1)' }]] : [[], []];
    await p.waitForTimeout(400);
    const z = await appels(p, 'film_zone');
    const pl = await rect(p, '#pilule');
    await poserEtat(p, etat(17, 'marche', c.charge ? {} : {}));
    const point = await p.evaluate(() => document.querySelector('.point').getAnimations().length);
    verifier(JSON.stringify(a) === JSON.stringify(attendu) && point === (quoi === 'normales' ? 1 : 0),
      `7 animations ${quoi}`, JSON.stringify({ a, point }));
    verifier(z.length === 1 && proche(z[0][1].y, pl.y, 0.01) && proche(z[0][1].x, pl.x, 0.01) && proche(z[0][1].largeur, pl.l, 0.01),
      `7 animations ${quoi} : zone envoyée une fois, sans le décalage ni l'échelle d'entrée`, JSON.stringify(z));
    await fermer(p);
  }

  // 8. Thème donné par le cœur (interface:theme) : il l'emporte sur le système.
  {
    const p = await ouvrir({ theme: 'light' });
    await montrer(p, charge(18, 1, { phase: 'marche' }));
    await p.evaluate(() => __emettre('interface:theme', { theme: 'dark' }));
    await p.waitForTimeout(100);
    const sombre = await vue(p);
    await p.evaluate(() => __emettre('interface:theme', { theme: null }));
    await p.waitForTimeout(100);
    const systeme = await vue(p);
    verifier(sombre.fond === PILULE_FOND.dark && sombre.point.fond === REC.dark && systeme.fond === PILULE_FOND.light && systeme.point.fond === REC.light,
      '8 interface:theme sombre puis système', JSON.stringify([sombre.fond, systeme.fond]));
    await fermer(p);
  }

  // 9. Charges invalides : rejetées avec console.warn, rien montré, rien envoyé.
  {
    const p = await ouvrir({ avertissements: true });
    const bonne = charge(19, 1);
    const e = bonne.etat;
    const invalides = [null, 5, {}, { ...bonne, id: -1 }, { ...bonne, id: 'x' }, { ...bonne, zone: null }, { ...bonne, zone: { ...bonne.zone, largeur: 0 } },
      { ...bonne, zone: { ...bonne.zone, x: 1.5 } }, { ...bonne, pilule: { x: 'a', y: 1 } }, { ...bonne, etat: null },
      { ...bonne, etat: { ...e, id: 20 } }, { ...bonne, etat: { ...e, phase: 'finie' } }, { ...bonne, etat: { ...e, duree: '' } },
      { ...bonne, etat: { ...e, pause: { libelle: 'Pause' } } }, { ...bonne, etat: { ...e, arreter: ' ' } },
      { ...bonne, etat: { ...e, micro: { libelle: 'Micro' } } }, { ...bonne, etat: { ...e, micro: undefined } }, { ...bonne, etat: { ...e, groupe: 3 } }];
    for (const c of invalides) await montrer(p, c);
    await p.waitForTimeout(100);
    const r = await p.evaluate(() => [scene.hidden, __appels.filter((a) => a[0] !== 'film_pret').length]);
    verifier(r[0] && r[1] === 0 && p.avert.length === invalides.length, '9 charges invalides rejetées (console.warn), rien montré ni envoyé', JSON.stringify(r) + ' avert=' + p.avert.length);
    await montrer(p, bonne);
    await poserEtat(p, { ...e, phase: 'x' });
    const garde = await vue(p);
    verifier(garde.pause.inactif && p.avert.length === invalides.length + 1, '9 état invalide rejeté, l\'état montré reste', JSON.stringify(garde.pause));
    await fermer(p);
  }

  // 10. Échelle de la fenêtre différente de celle de la charge : la zone attend
  //     devicePixelRatio = echelle (borné), et repart à chaque changement d'échelle.
  {
    const zones = (p) => appels(p, 'film_zone').then((a) => a.map((x) => x[1]));
    const echelle = (p, k) => p.context().newCDPSession(p).then((cdp) => cdp.send('Emulation.setDeviceMetricsOverride', { width: VIEWPORT.width, height: VIEWPORT.height, deviceScaleFactor: k, mobile: false }));
    let p = await ouvrir({ k: 1 });
    let c = charge(21, 1.5);
    await montrer(p, c);
    await p.waitForTimeout(100);
    const pendant = await zones(p);
    await echelle(p, 1.5);
    await p.waitForTimeout(500);
    let z = await zones(p);
    verifier(pendant.length === 0 && z.length === 1 && proche(z[0].x, c.pilule.x / 1.5, 0.01) && proche(z[0].y, c.pilule.y / 1.5, 0.01),
      '10 (a) échelle arrivée après montrer : rien avant, puis une zone juste', JSON.stringify({ pendant, z }));
    await fermer(p);
    p = await ouvrir({ k: 1 });
    await montrer(p, charge(22, 1.5));
    await p.waitForTimeout(150);
    const tot = await zones(p);
    await p.waitForTimeout(450);
    z = await zones(p);
    verifier(tot.length === 0 && z.length === 1, '10 (b) échelle jamais atteinte : zone envoyée après 300 ms', JSON.stringify({ tot, z }));
    await fermer(p);
  }

  // Captures (clair et sombre, échelle 2, sans CSP pour poser un fond de bureau) : vidéo en
  // marche, micro ouvert ; GIF en pause ; décompte de la vidéo.
  for (const theme of ['light', 'dark']) {
    for (const [cas, o] of [['video', { phase: 'marche', duree: '0:07', micro: true }], ['gif-pause', { phase: 'pause', duree: '0:12', micro: null }], ['decompte', {}]]) {
      const ctx = await nav.newContext({ viewport: VIEWPORT, deviceScaleFactor: 2, colorScheme: theme });
      const p = await ctx.newPage();
      p.on('pageerror', (e) => erreurs.push(String(e)));
      await p.addInitScript(simuler, {});
      await p.goto(base + '?sanscsp=1');
      await p.waitForFunction(() => window.__appels.some((a) => a[0] === 'film_pret'));
      await p.addStyleTag({ content: `html{background:${FONDS[theme]} !important}` });
      await montrer(p, charge(20, 2, o));
      await p.waitForTimeout(500);
      const nom = `film-${cas}-${theme === 'light' ? 'clair' : 'sombre'}`;
      await cliche(p, { path: path.join(SORTIE, `${nom}.png`) });
      await cliche(p, { path: path.join(SORTIE, `${nom}-pilule.png`), clip: { x: 420, y: 440, width: 240, height: 100 } });
      console.log('capture', nom);
      await ctx.close();
    }
  }

  verifier(erreurs.length === 0, 'aucune erreur de page, aucun avertissement inattendu, aucune violation de CSP', JSON.stringify(erreurs));
  console.log('ECHECS :', echecs);
  await nav.close(); serveur.close();
  process.exitCode = echecs === 0 ? 0 : 1;
})().catch((e) => { console.error('essai interrompu :', e); process.exit(1); });
