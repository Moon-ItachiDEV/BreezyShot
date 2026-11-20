const { chromium } = require('playwright');
const http = require('http');
const fs = require('fs');
const path = require('path');

// Essai de la page d'une capture épinglée dans Chromium (Playwright).
// Sort avec 1 si une vérification échoue.
//   node essais/pages/epingle.cjs
const RACINE = path.join(__dirname, '..', '..', 'app', 'interface');
const SORTIE = process.env.SORTIE || path.join(__dirname, '..', '..', 'essais-resultats', 'pages');
fs.mkdirSync(SORTIE, { recursive: true });
const CSP = Object.entries(
  JSON.parse(
    fs.readFileSync(path.join(__dirname, '..', '..', 'app', 'coeur', 'tauri.conf.json'), 'utf8'),
  ).app.security.csp,
)
  .map(([k, v]) => `${k} ${v}`)
  .join('; ');
const types = { '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript' };

const serveur = http.createServer((q, r) => {
  const p = path.join(RACINE, decodeURIComponent(new URL(q.url, 'http://x').pathname));
  if (!p.startsWith(RACINE) || !fs.existsSync(p)) {
    r.writeHead(404);
    return r.end();
  }
  const entetes = { 'content-type': types[path.extname(p)] || 'text/plain' };
  if (q.url.includes('csp=1')) entetes['content-security-policy'] = CSP;
  r.writeHead(200, entetes);
  fs.createReadStream(p).pipe(r);
});

(async () => {
  await new Promise((ok) => serveur.listen(0, ok));
  const base = `http://127.0.0.1:${serveur.address().port}/epingle/epingle.html`;
  const nav = await chromium.launch();
  const erreurs = [];
  let echecs = 0;
  const verifier = (ok, quoi, info = '') => {
    console.log(ok ? 'OK   ' : 'ECHEC', quoi, info);
    if (!ok) echecs++;
  };

  async function page(o = {}) {
    const l = o.largeur || 320;
    const h = o.hauteur || 200;
    const ctx = await nav.newContext({
      viewport: { width: l + 32, height: h + 32 },
      deviceScaleFactor: o.echelle || 1,
      colorScheme: o.theme || 'light',
    });
    const p = await ctx.newPage();
    p.on('pageerror', (e) => erreurs.push(String(e)));
    p.on('console', (m) => {
      if (m.type() === 'error') erreurs.push(m.text());
    });
    if (o.csp) {
      await p.addInitScript(() => {
        window.__refus = [];
        document.addEventListener('securitypolicyviolation', (e) =>
          window.__refus.push(e.violatedDirective + ' ' + e.blockedURI),
        );
      });
    }
    await p.addInitScript(
      ({ l, h, copieEchec }) => {
        window.__appels = [];
        window.__drag = 0;
        const ecoutes = {};
        let echelle = 1;
        // Image RGBA connue : moitié gauche bleue, moitié droite dorée.
        const rgba = new Uint8Array(l * h * 4);
        for (let y = 0; y < h; y++) {
          for (let x = 0; x < l; x++) {
            const i = (y * l + x) * 4;
            if (x < l / 2) {
              rgba[i] = 24;
              rgba[i + 1] = 104;
              rgba[i + 2] = 196;
              rgba[i + 3] = 255;
            } else {
              rgba[i] = 232;
              rgba[i + 1] = 178;
              rgba[i + 2] = 58;
              rgba[i + 3] = 255;
            }
          }
        }
        window.__TAURI__ = {
          core: {
            invoke: (c, a) => {
              window.__appels.push([c, a]);
              if (c === 'epingle_pret') {
                return Promise.resolve({
                  id: 3,
                  image_largeur: l,
                  image_hauteur: h,
                  largeur: l,
                  hauteur: h,
                  echelle: 1,
                  opacite: 1,
                });
              }
              if (c === 'epingle_image') return Promise.resolve(rgba.buffer);
              if (c === 'epingle_redimensionner') {
                echelle = Math.max(0.2, Math.min(4.0, echelle * (a?.facteur || 1)));
                return Promise.resolve({
                  largeur: Math.round(l * echelle),
                  hauteur: Math.round(h * echelle),
                  echelle,
                });
              }
              if (c === 'epingle_opacite') {
                return Promise.resolve(Math.max(0.25, Math.min(1, a?.opacite ?? 1)));
              }
              if (c === 'epingle_copier') {
                return copieEchec ? Promise.reject('presse-papiers occupe') : Promise.resolve();
              }
              return Promise.resolve();
            },
          },
          event: {
            listen: (n, f) => {
              ecoutes[n] = f;
              return Promise.resolve(() => {});
            },
          },
          window: {
            getCurrentWindow: () => ({
              startDragging: () => {
                window.__drag++;
                return Promise.resolve();
              },
            }),
          },
        };
        window.__emettre = (n, payload) => ecoutes[n]?.({ payload });
      },
      { l, h, copieEchec: !!o.copieEchec },
    );
    await p.goto(base + (o.csp ? '?csp=1' : ''));
    await p.waitForFunction(() => window.__appels.some((a) => a[0] === 'epingle_image'));
    await p.waitForTimeout(50);
    return p;
  }

  const appels = (p, nom) => p.evaluate((n) => window.__appels.filter((a) => !n || a[0] === n), nom);

  // 1. Démarrage, rendu RGBA sur la toile, boutons et accessibilité.
  {
    const p = await page({ csp: true });
    const init = await p.evaluate(() => {
      const c = document.getElementById('toile');
      const g = c.getContext('2d');
      const gPx = [...g.getImageData(10, 10, 1, 1).data];
      const dPx = [...g.getImageData(c.width - 10, 10, 1, 1).data];
      const bs = [...document.querySelectorAll('.outils button')].map((b) => [
        b.id,
        b.getAttribute('aria-label'),
        b.title,
      ]);
      return {
        largeur: c.width,
        hauteur: c.height,
        gPx,
        dPx,
        bs,
        opacite: getComputedStyle(document.getElementById('cadre')).opacity,
        refus: window.__refus,
      };
    });
    verifier(
      init.largeur === 320 &&
        init.hauteur === 200 &&
        init.gPx[2] > 180 &&
        init.dPx[0] > 200 &&
        init.opacite === '1' &&
        init.refus.length === 0,
      '1 rendu RGBA 320×200 sur la toile, opacité 1, CSP respectée',
      JSON.stringify(init),
    );
    verifier(
      JSON.stringify(init.bs) ===
        '[["copier","Copier","Copier"],["fermer","Fermer","Fermer"]]',
      '1 boutons Copier et Fermer avec aria-label = title',
      JSON.stringify(init.bs),
    );
    await p.locator('#carte').hover();
    await p.waitForTimeout(250);
    await p.screenshot({ path: path.join(SORTIE, 'epingle-clair.png') });

    // 2. Zoom à la molette (sans Alt) : epingle_redimensionner + pilule %.
    await p.mouse.move(160, 100);
    await p.mouse.wheel(0, -120);
    await p.waitForTimeout(80);
    const z1 = await p.evaluate(() => ({
      appels: window.__appels.filter((a) => a[0] === 'epingle_redimensionner'),
      pilule: document.getElementById('pilule').textContent,
      visible: !document.getElementById('pilule').hidden,
    }));
    verifier(
      z1.appels.length === 1 &&
        z1.appels[0][1].facteur > 1.05 &&
        z1.visible &&
        z1.pilule === '110 %',
      '2 molette haut : agrandissement (110 %) et pilule montrée',
      JSON.stringify(z1),
    );

    // 3. Alt + molette : opacité réglable et bornée [0.25..1.0].
    await p.keyboard.down('Alt');
    for (let i = 0; i < 12; i++) {
      await p.mouse.wheel(0, 120);
    }
    await p.keyboard.up('Alt');
    await p.waitForTimeout(80);
    const opMin = await p.evaluate(() => ({
      etat: window.epingleApercu.etat().opacite,
      css: document.getElementById('cadre').style.getPropertyValue('--opacite'),
      pilule: document.getElementById('pilule').textContent,
      appels: window.__appels.filter((a) => a[0] === 'epingle_opacite').length,
    }));
    verifier(
      opMin.etat === 0.25 &&
        opMin.css === '0.25' &&
        opMin.pilule === 'Opacité 25 %' &&
        opMin.appels === 12,
      '3 Alt + molette bas : opacité bornée à 25 % et notifiée au cœur',
      JSON.stringify(opMin),
    );

    // Touche 0 : réinitialise opacité (100 %) et échelle (100 %).
    await p.keyboard.press('0');
    await p.waitForTimeout(80);
    const reset = await p.evaluate(() => ({
      opacite: window.epingleApercu.etat().opacite,
      echelle: window.epingleApercu.etat().echelle,
      pilule: document.getElementById('pilule').textContent,
    }));
    verifier(
      reset.opacite === 1 && Math.abs(reset.echelle - 1) < 0.01 && reset.pilule === '100 %',
      '3 touche 0 : réinitialise opacité et échelle à 100 %',
      JSON.stringify(reset),
    );

    // 4. Glisser au seuil de 4 px : startDragging() appelé sans fermer.
    await p.mouse.move(120, 100);
    await p.mouse.down();
    await p.mouse.move(122, 100);
    await p.mouse.up();
    const drag2 = await p.evaluate(() => window.__drag);
    await p.mouse.move(120, 100);
    await p.mouse.down();
    await p.mouse.move(132, 100, { steps: 3 });
    await p.mouse.up();
    const drag12 = await p.evaluate(() => window.__drag);
    verifier(
      drag2 === 0 && drag12 === 1 && (await appels(p, 'epingle_fermer')).length === 0,
      '4 déplacement : 2 px rien, 12 px startDragging() sans fermer',
      JSON.stringify([drag2, drag12]),
    );

    // 5. Copier (bouton et Ctrl+C) : epingle_copier + pilule « Copié ».
    await p.locator('#copier').click();
    await p.waitForTimeout(60);
    const cp = await p.evaluate(() => ({
      n: window.__appels.filter((a) => a[0] === 'epingle_copier').length,
      fait: document.getElementById('copier').classList.contains('fait'),
      pilule: document.getElementById('pilule').textContent,
    }));
    verifier(
      cp.n === 1 && cp.fait && cp.pilule === 'Copié',
      '5 clic Copier : epingle_copier, coche et pilule « Copié »',
      JSON.stringify(cp),
    );
    await p.keyboard.press('Control+c');
    await p.waitForTimeout(60);
    verifier(
      (await appels(p, 'epingle_copier')).length === 2,
      '5 Ctrl+C : appelle epingle_copier',
    );

    // 6. Thème sombre via interface:theme.
    await p.evaluate(() => __emettre('interface:theme', { theme: 'dark' }));
    await p.waitForTimeout(100);
    verifier(
      (await p.evaluate(() => document.documentElement.dataset.theme)) === 'dark',
      '6 interface:theme pose le thème sombre',
    );
    await p.screenshot({ path: path.join(SORTIE, 'epingle-sombre.png') });

    // 7. Double-clic sur la carte : epingle_fermer.
    await p.mouse.dblclick(140, 110);
    await p.waitForTimeout(50);
    verifier(
      (await appels(p, 'epingle_fermer')).length === 1,
      '7 double-clic sur la carte : appelle epingle_fermer',
    );
    await p.context().close();
  }

  // 8. Fermeture par Échap et par le bouton Fermer.
  {
    const p = await page();
    await p.keyboard.press('Escape');
    await p.waitForTimeout(50);
    verifier(
      (await appels(p, 'epingle_fermer')).length === 1,
      '8 touche Échap : appelle epingle_fermer',
    );
    await p.context().close();
  }
  {
    const p = await page();
    await p.locator('#carte').hover();
    await p.waitForTimeout(200);
    await p.locator('#fermer').click();
    await p.waitForTimeout(50);
    verifier(
      (await appels(p, 'epingle_fermer')).length === 1,
      '8 clic bouton Fermer au survol : appelle epingle_fermer',
    );
    await p.context().close();
  }

  console.log('ECHECS :', echecs);
  console.log('ERREURS PAGE :', JSON.stringify(erreurs));
  if (erreurs.length) echecs++;
  await nav.close();
  serveur.close();
  process.exitCode = echecs === 0 ? 0 : 1;
})().catch((e) => {
  console.error('essai interrompu :', e);
  process.exit(1);
});
