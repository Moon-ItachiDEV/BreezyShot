const { chromium } = require('playwright');
const http = require('http');
const fs = require('fs');
const path = require('path');
const zlib = require('zlib');
const { pathToFileURL } = require('url');
// Essai de la page de l'historique dans Chromium (contrat 0.5, §2.2, §10, §12.2). Sort avec 1
// si une vérification échoue. Copies d'écran dans $SORTIE.
//   node essais/pages/historique.cjs
// Le cœur est simulé (`__TAURI__` qui note les appels, rend des PNG connus pour
// `historique_apercu` et joue la liste comme le cœur du §6.3 : une suppression retire la
// case et annonce l'attente, Annuler la remet, « Tout effacer » vide la liste).
const RACINE = path.join(__dirname, '..', '..', 'app', 'interface');
const SORTIE = process.env.SORTIE || path.join(__dirname, '..', '..', 'essais-resultats', 'pages');
fs.mkdirSync(SORTIE, { recursive: true });
const CONF = path.join(__dirname, '..', '..', 'app', 'coeur', 'tauri.conf.json');
const CSP = Object.entries(JSON.parse(fs.readFileSync(CONF, 'utf8')).app.security.csp)
  .map(([k, v]) => `${k} ${v}`)
  .join('; ');
const types = { '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript' };

function serveur() {
  return http.createServer((q, r) => {
    const nom = decodeURIComponent(new URL(q.url, 'http://x').pathname);
    const p = path.join(RACINE, nom);
    if (!p.startsWith(RACINE) || !fs.existsSync(p)) {
      r.writeHead(404);
      return r.end();
    }
    r.writeHead(200, {
      'content-security-policy': CSP,
      'content-type': types[path.extname(p)] || 'text/plain',
    });
    fs.createReadStream(p).pipe(r);
  });
}

// Un PNG RGBA 400 × 250 connu : bandes de couleur d'après `graine`, une barre sombre en haut
// à gauche (on doit la voir : l'aperçu est calé en haut à gauche).
function png(graine) {
  const l = 400;
  const h = 250;
  const crc = (b) => {
    let c = ~0;
    for (const o of b) {
      c ^= o;
      for (let k = 0; k < 8; k++) c = (c >>> 1) ^ (0xedb88320 & -(c & 1));
    }
    return ~c >>> 0;
  };
  const bloc = (type, data) => {
    const t = Buffer.from(type);
    const n = Buffer.alloc(4);
    n.writeUInt32BE(data.length);
    const c = Buffer.alloc(4);
    c.writeUInt32BE(crc(Buffer.concat([t, data])));
    return Buffer.concat([n, t, data, c]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(l, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  const brut = Buffer.alloc((l * 4 + 1) * h);
  const teinte = [
    ((graine * 47) % 200) + 40,
    ((graine * 91) % 200) + 40,
    ((graine * 23) % 200) + 40,
  ];
  for (let y = 0; y < h; y++) {
    const o = y * (l * 4 + 1);
    for (let x = 0; x < l; x++) {
      const i = o + 1 + x * 4;
      const sombre = y < 28 && x < 160;
      const bande = ((x >> 5) + (y >> 5)) % 2 ? 1 : 0.8;
      brut[i] = sombre ? 30 : Math.round(teinte[0] * bande + 255 * (1 - bande) * 0.5);
      brut[i + 1] = sombre ? 30 : Math.round(teinte[1] * bande + 255 * (1 - bande) * 0.5);
      brut[i + 2] = sombre ? 34 : Math.round(teinte[2] * bande + 255 * (1 - bande) * 0.5);
      brut[i + 3] = 255;
    }
  }
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    bloc('IHDR', ihdr),
    bloc('IDAT', zlib.deflateSync(brut)),
    bloc('IEND', Buffer.alloc(0)),
  ]).toString('base64');
}
const PNGS = [1, 2, 3, 4, 5, 6, 7].map(png);

// Âges des 12 captures du design (minutes) et les noms attendus, écrits à la main (règle
// `ago` du design ; pas recalculés par la page).
const AGES = [2, 9, 26, 58, 140, 330, 1500, 1700, 2900, 4400, 7300, 11600];
const NOMS_ATTENDUS = [
  'il y a 2 min',
  'il y a 9 min',
  'il y a 26 min',
  'il y a 58 min',
  'il y a 2 h',
  'il y a 6 h',
  'hier',
  'hier',
  'il y a 2 j',
  'il y a 3 j',
  'il y a 5 j',
  'il y a 8 j',
].map((a) => `Capture, ${a}`);

// Le cœur simulé : `__appels`, `__emettre(nom, charge)`, `__entrees` (la liste « Récents »),
// `__manquants` (aperçus pas encore faits).
function simulerCoeur({ ages, pngs, charge, manquants, n, sansAnnoter, retenus }) {
  window.__appels = [];
  window.__ecouteurs = {};
  window.__lacher = {};
  document.addEventListener(
    'securitypolicyviolation',
    (e) => (window.__csp = (window.__csp || []).concat(e.violatedDirective)),
  );
  const maintenant = Date.now();
  const nomDe = (t, k) => {
    const d = new Date(t);
    const z = (v, l = 2) => String(v).padStart(l, '0');
    return (
      `${d.getUTCFullYear()}${z(d.getUTCMonth() + 1)}${z(d.getUTCDate())}-` +
      `${z(d.getUTCHours())}${z(d.getUTCMinutes())}${z(d.getUTCSeconds())}-` +
      `${z(d.getUTCMilliseconds(), 3)}${k ? `_${k}` : ''}.png`
    );
  };
  window.__nomDe = nomDe;
  const liste = n
    ? Array.from({ length: n }, (_, i) => maintenant - (i + 1) * 7 * 60000)
    : ages.map((a) => maintenant - a * 60000);
  window.__entrees = liste.map((t, i) => ({
    nom: nomDe(t, 0),
    horodatage_ms: t,
    largeur: 1920,
    hauteur: 1080,
    annotable: !(sansAnnoter && i === 1),
  }));
  window.__manquants = new Set((manquants || []).map((i) => window.__entrees[i].nom));
  window.__retenus = new Set((retenus || []).map((i) => window.__entrees[i].nom));
  window.__attente = [];
  const octets = pngs.map((b) => Uint8Array.from(atob(b), (c) => c.charCodeAt(0)));
  const emettre = (nom, payload) => window.__ecouteurs[nom] && window.__ecouteurs[nom]({ payload });
  window.__emettre = emettre;
  const plusTard = (f) => setTimeout(f, 0);
  const vues = () => window.__entrees.filter((e) => !window.__attente.some((a) => a.nom === e.nom));
  window.__envoyerListe = () => emettre('historique:liste', { entrees: vues() });
  window.__TAURI__ = {
    core: {
      invoke: (c, a) => {
        window.__appels.push([c, a]);
        if (c === 'historique_affiche') window.__afficheA = performance.now();
        if (c === 'historique_pret') {
          plusTard(() => window.__envoyerListe());
          return Promise.resolve(charge);
        }
        if (c === 'historique_apercu') {
          const i = window.__entrees.findIndex((e) => e.nom === a.nom);
          // Demande retenue (une fois) : l'essai la lâche par `__lacher[nom](refus)`.
          if (window.__retenus.has(a.nom)) {
            window.__retenus.delete(a.nom);
            return new Promise((ok, ko) => {
              window.__lacher[a.nom] = (refus) =>
                refus ? ko('apercu absent') : ok(octets[i % octets.length].slice().buffer);
            });
          }
          if (i < 0 || window.__manquants.has(a.nom)) return Promise.reject('apercu absent');
          return Promise.resolve(octets[i % octets.length].slice().buffer);
        }
        if (c === 'historique_action' && a.action === 'supprimer') {
          const e = window.__entrees.find((x) => x.nom === a.nom);
          if (e && !window.__attente.includes(e)) window.__attente.push(e);
          plusTard(() => {
            window.__envoyerListe();
            emettre('historique:attente', { n: window.__attente.length });
          });
        }
        if (c === 'historique_annuler') {
          window.__attente = [];
          plusTard(() => {
            window.__envoyerListe();
            emettre('historique:attente', { n: 0 });
          });
        }
        if (c === 'historique_tout_effacer') {
          window.__entrees = [];
          window.__attente = [];
          plusTard(() => {
            window.__envoyerListe();
            emettre('historique:attente', { n: 0 });
          });
        }
        return Promise.resolve(null);
      },
    },
    event: {
      listen: (nom, f) => {
        window.__ecouteurs[nom] = f;
        return Promise.resolve(() => {});
      },
    },
    window: {
      getCurrentWindow: () => ({
        minimize: () => {
          window.__appels.push(['fenetre.minimize', {}]);
          return Promise.resolve();
        },
      }),
    },
  };
}

(async () => {
  const srv = serveur();
  await new Promise((ok) => srv.listen(0, ok));
  const URL_PAGE = `http://127.0.0.1:${srv.address().port}/historique/historique.html`;
  const nav = await chromium.launch();
  let echecs = 0;
  let reussis = 0;
  const verifier = (ok, quoi, info = '') => {
    console.log(ok ? 'OK   ' : 'ECHEC', quoi, ok ? '' : info);
    if (ok) reussis++;
    else echecs++;
  };

  async function page(o = {}) {
    const ctx = await nav.newContext({
      viewport: { width: o.largeur || 760, height: o.hauteur || 560 },
      deviceScaleFactor: o.echelle || 1,
      colorScheme: o.theme || 'light',
      reducedMotion: o.reduit ? 'reduce' : 'no-preference',
    });
    const p = await ctx.newPage();
    p.setDefaultTimeout(5000);
    p.erreurs = [];
    p.on('pageerror', (e) => p.erreurs.push(String(e)));
    p.on('console', (m) => {
      if (m.type() === 'error' || m.type() === 'warning') p.erreurs.push(m.text());
    });
    await p.addInitScript(simulerCoeur, {
      ages: o.ages || AGES,
      pngs: PNGS,
      charge: { reduites: !!o.reduites },
      manquants: o.manquants || [],
      n: o.n || 0,
      sansAnnoter: !!o.sansAnnoter,
      retenus: o.retenus || [],
    });
    p.t0 = Date.now();
    await p.goto(URL_PAGE);
    if (o.retenus) {
      await p.waitForFunction(() => document.querySelectorAll('.tile').length > 0);
      return p;
    }
    await p.waitForFunction(() => __appels.some((a) => a[0] === 'historique_affiche'));
    return p;
  }
  const fin = async (p, nom) => {
    const csp = await p.evaluate(() => window.__csp || []);
    verifier(
      p.erreurs.length === 0 && csp.length === 0,
      `${nom} : aucune erreur, aucune violation de CSP`,
      JSON.stringify([p.erreurs, csp]),
    );
    await p.context().close();
  };
  const appels = (p, nom) => p.evaluate((n) => __appels.filter((a) => !n || a[0] === n), nom);
  const vider = (p) => p.evaluate(() => (__appels.length = 0));
  const noms = (p) =>
    p.evaluate(() => [...document.querySelectorAll('.tile')].map((t) => t.dataset.nom));
  const entrees = (p) => p.evaluate(() => __entrees.map((e) => e.nom));
  const focusNom = (p) => p.evaluate(() => document.activeElement?.closest('.tile')?.dataset.nom);
  const pause = (p, ms) => p.evaluate((d) => new Promise((ok) => setTimeout(ok, d)), ms);
  const cases = (p) => p.getByRole('list', { name: 'Captures récentes' }).getByRole('listitem');
  const colonnes = (p) =>
    p.evaluate(() => {
      const t = [...document.querySelectorAll('.tile')];
      return t.filter((x) => x.offsetTop === t[0].offsetTop).length;
    });
  // Une case montre sa pastille et sa pilule, au-dessus de l'aperçu (point du milieu).
  const visibles = (t) => {
    const dessus = (el) => {
      const r = el.getBoundingClientRect();
      const x = document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2);
      return getComputedStyle(el).opacity === '1' && !!x && (x === el || el.contains(x));
    };
    // La pastille ne prend pas le pointeur : elle doit être peinte au-dessus de l'aperçu.
    const ago = t.querySelector('.ago');
    const z = (el) => parseInt(getComputedStyle(el).zIndex, 10) || 0;
    const agoDessus = getComputedStyle(ago).opacity === '1' && z(ago) > z(t.querySelector('.pv'));
    return [t.contains(document.activeElement), agoDessus, dessus(t.querySelector('.qa .ib'))];
  };
  const animationsFinies = (p) =>
    p.waitForFunction(() => document.getAnimations().every((a) => a.playState !== 'running'));

  // (2) temps.js, sous Node : la règle du design.
  {
    const { age, nomCase } = await import(
      pathToFileURL(path.join(RACINE, 'historique', 'temps.js')).href
    );
    const M = 60000;
    const t = 1_800_000_000_000;
    const cas = [
      [0.4, "à l'instant"],
      [0.5, 'il y a 1 min'],
      [1, 'il y a 1 min'],
      [59, 'il y a 59 min'],
      [60, 'il y a 1 h'],
      [90, 'il y a 2 h'],
      [23 * 60, 'il y a 23 h'],
      [24 * 60, 'hier'],
      [36 * 60, 'il y a 2 j'],
      [47 * 60, 'il y a 2 j'],
      [3 * 24 * 60, 'il y a 3 j'],
    ];
    const faux = cas.filter(([m, a]) => age(t - m * M, t) !== a);
    verifier(!faux.length, 'temps.js : âges du design (0,4 à 3 j)', JSON.stringify(faux));
    verifier(
      nomCase(t - 3 * M, t) === 'Capture, il y a 3 min',
      'temps.js : nom « Capture, il y a 3 min »',
    );
  }

  // (1) (3) liste de 12, noms, survol, pilule d'actions.
  {
    const p = await page();
    await cases(p).first().waitFor();
    verifier(
      (await cases(p).count()) === 12,
      '12 cases (rôle listitem dans « Captures récentes »)',
    );
    verifier(
      JSON.stringify(await noms(p)) === JSON.stringify(await entrees(p)),
      'cases dans l’ordre de la liste (plus récente d’abord)',
    );
    const lus = await p.evaluate(() =>
      [...document.querySelectorAll('.tile .pv')].map((b) => b.getAttribute('aria-label')),
    );
    verifier(
      JSON.stringify(lus) === JSON.stringify(NOMS_ATTENDUS),
      'noms « Capture, <âge> »',
      JSON.stringify(lus),
    );
    for (const n of NOMS_ATTENDUS.slice(0, 2)) {
      verifier(
        (await p.getByRole('button', { name: n, exact: true }).count()) === 1,
        `bouton nommé « ${n} »`,
      );
    }
    verifier(
      await p.evaluate(() =>
        [...document.querySelectorAll('.tile img')].every(
          (i) => i.complete && i.naturalWidth === 400,
        ),
      ),
      'aperçus : 12 PNG connus affichés (Blob)',
    );
    verifier(
      await p.evaluate(() =>
        [...document.querySelectorAll('.tile img')].every((i) => i.src.startsWith('blob:')),
      ),
      'aperçus en blob:',
    );
    const deuxieme = p.locator('.tile').nth(1);
    const avant = await deuxieme.evaluate((t) => getComputedStyle(t.querySelector('.qa')).opacity);
    await deuxieme.hover();
    await pause(p, 400);
    const apres = await deuxieme.evaluate((t) => ({
      age: getComputedStyle(t.querySelector('.ago')).opacity,
      qa: getComputedStyle(t.querySelector('.qa')).opacity,
      texte: t.querySelector('.ago').textContent,
    }));
    verifier(
      avant === '0' && apres.qa === '1' && apres.age === '1',
      'survol : pastille et pilule paraissent',
      JSON.stringify([avant, apres]),
    );
    verifier(apres.texte === 'il y a 9 min', 'pastille : « il y a 9 min »', apres.texte);
    const boutons = await deuxieme
      .locator('.qa')
      .getByRole('button')
      .evaluateAll((b) => b.map((x) => [x.getAttribute('aria-label'), x.title]));
    verifier(
      JSON.stringify(boutons.map((b) => b[0])) ===
        '["Copier","Enregistrer","Annoter","Supprimer"]' && boutons.every((b) => b[0] === b[1]),
      'quatre boutons dans l’ordre, infobulle = nom',
      JSON.stringify(boutons),
    );
    verifier(
      (await p.getByRole('button', { name: 'Épingler' }).count()) === 0 &&
        (await p.locator('[href="#i-pin"], #i-pin').count()) === 0,
      'aucun Épingler (U1)',
    );
    for (const theme of ['light', 'dark']) {
      await p.emulateMedia({ colorScheme: theme });
      await pause(p, 200);
      await p.screenshot({ path: path.join(SORTIE, `historique-760-${theme}.png`) });
    }
    await p.emulateMedia({ colorScheme: 'light' });
    await p.evaluate(() => __emettre('interface:theme', { theme: 'dark' }));
    verifier(
      (await p.evaluate(() => document.documentElement.dataset.theme)) === 'dark',
      'interface:theme pose le thème sombre',
    );
    await p.evaluate(() => __emettre('interface:theme', { theme: null }));

    // (12) barre du haut.
    const barre = await p.evaluate(() => ({
      barre: document.querySelector('#barre-haut').hasAttribute('data-tauri-drag-region'),
      titre: document.querySelector('#barre-haut .t').hasAttribute('data-tauri-drag-region'),
      boutons: [...document.querySelectorAll('#barre-haut button')].map((b) => [
        b.getAttribute('aria-label'),
        b.hasAttribute('data-tauri-drag-region'),
      ]),
    }));
    verifier(
      barre.barre &&
        barre.titre &&
        JSON.stringify(barre.boutons) ===
          '[["Plus d\'options",false],["Réduire",false],["Fermer",false]]',
      'barre : data-tauri-drag-region sauf les boutons (Plus d’options, Réduire, Fermer)',
      JSON.stringify(barre),
    );
    await vider(p);
    await p.getByRole('button', { name: 'Réduire' }).click();
    await p.getByRole('button', { name: 'Fermer' }).click();
    verifier(
      JSON.stringify((await appels(p)).map((a) => a[0])) ===
        '["fenetre.minimize","historique_fermer"]',
      'Réduire → minimize ; Fermer → historique_fermer',
      JSON.stringify(await appels(p)),
    );
    await fin(p, 'liste de 12');
  }

  // (4) (5) (6) clavier, double-clic, résultats.
  {
    const p = await page();
    const n = await entrees(p);
    const cols = await colonnes(p);
    verifier(cols === 4, 'à 760 px : 4 colonnes', String(cols));
    await p.locator('.tile').first().click();
    verifier((await focusNom(p)) === n[0], 'clic : focus sur la case');
    const tabs = await p.evaluate(
      () => [...document.querySelectorAll('.tile .pv')].filter((b) => b.tabIndex === 0).length,
    );
    verifier(tabs === 1, 'tabindex mobile : une seule case dans l’ordre de tabulation');
    await p.keyboard.press('ArrowRight');
    verifier((await focusNom(p)) === n[1], 'Flèche droite');
    await p.mouse.move(700, 540);
    await pause(p, 400);
    const vu = await p.locator('.tile').nth(1).evaluate(visibles);
    verifier(
      vu.every(Boolean),
      'focus clavier : pastille et pilule paraissent, au-dessus',
      JSON.stringify(vu),
    );
    await p.keyboard.press('ArrowDown');
    verifier(
      (await focusNom(p)) === n[1 + cols],
      'Flèche bas : une rangée plus bas (colonnes réelles)',
    );
    await p.keyboard.press('ArrowLeft');
    await p.keyboard.press('ArrowUp');
    verifier((await focusNom(p)) === n[0], 'Flèches gauche et haut');
    await p.keyboard.press('End');
    verifier((await focusNom(p)) === n[11], 'Fin');
    await p.keyboard.press('Home');
    verifier((await focusNom(p)) === n[0], 'Début');
    await p.keyboard.press('ArrowRight');
    await vider(p);
    await p.keyboard.press('Enter');
    await p.keyboard.press(' ');
    verifier(
      JSON.stringify(await appels(p)) ===
        JSON.stringify([
          ['historique_action', { nom: n[1], action: 'annoter' }],
          ['historique_action', { nom: n[1], action: 'annoter' }],
        ]),
      'Entrée et Espace → annoter (ouverture dans l’éditeur)',
      JSON.stringify(await appels(p)),
    );
    await vider(p);
    await p.keyboard.press('Control+c');
    verifier(
      JSON.stringify(await appels(p)) ===
        JSON.stringify([['historique_action', { nom: n[1], action: 'copier' }]]),
      'Ctrl + C → copier',
    );
    verifier(
      await p
        .locator('.tile')
        .nth(1)
        .evaluate((t) => t.classList.contains('lisere')),
      'Ctrl + C : liseré vert',
    );
    await vider(p);
    await p.locator('.tile').nth(3).locator('.pv').dblclick();
    verifier(
      (await appels(p, 'historique_action')).some(
        (a) => a[1].nom === n[3] && a[1].action === 'annoter',
      ),
      'double-clic → annoter',
    );
    // Clic sur les boutons de la pilule.
    await vider(p);
    const t2 = p.locator('.tile').nth(2);
    await t2.hover();
    await t2.getByRole('button', { name: 'Copier' }).click();
    await t2.getByRole('button', { name: 'Enregistrer' }).click();
    await t2.getByRole('button', { name: 'Annoter' }).click();
    verifier(
      JSON.stringify((await appels(p)).map((a) => a[1].action)) ===
        '["copier","enregistrer","annoter"]' &&
        (await appels(p)).every((a) => a[0] === 'historique_action' && a[1].nom === n[2]),
      'boutons Copier, Enregistrer, Annoter → historique_action',
      JSON.stringify(await appels(p)),
    );
    // (6) résultat fait → coche 1 200 ms.
    verifier(
      !(await t2.locator('[data-action=copier]').evaluate((b) => b.classList.contains('fait'))),
      'pas de coche avant le résultat',
    );
    const d0 = await p.evaluate(() => {
      __emettre('historique:resultat', {
        nom: __entrees[2].nom,
        action: 'copier',
        etat: 'fait',
        message: null,
      });
      return performance.now();
    });
    verifier(
      await t2.locator('[data-action=copier]').evaluate((b) => b.classList.contains('fait')),
      'résultat fait : coche verte',
    );
    await p.waitForFunction(
      () =>
        !document
          .querySelectorAll('.tile')[2]
          .querySelector('[data-action=copier]')
          .classList.contains('fait'),
      null,
      { timeout: 3000 },
    );
    const d1 = await p.evaluate(() => performance.now());
    verifier(d1 - d0 >= 1150 && d1 - d0 < 1600, 'coche partie après 1 200 ms', String(d1 - d0));
    await p.evaluate(() =>
      __emettre('historique:resultat', {
        nom: __entrees[2].nom,
        action: 'enregistrer',
        etat: 'deja_fait',
        message: null,
      }),
    );
    verifier(
      await t2.locator('[data-action=enregistrer]').evaluate((b) => b.classList.contains('fait')),
      'déjà fait : coche aussi',
    );
    const e0 = await p.evaluate(() => {
      __emettre('historique:resultat', {
        nom: __entrees[2].nom,
        action: 'copier',
        etat: 'echec',
        message: 'Impossible de copier',
      });
      return performance.now();
    });
    const alerte = p.getByRole('alert');
    verifier(
      (await alerte.textContent()).includes('Impossible de copier') && (await alerte.isVisible()),
      'échec : pilule « Impossible de copier » (alert)',
    );
    await p.screenshot({ path: path.join(SORTIE, 'historique-erreur.png') });
    await p.waitForFunction(
      () => !document.querySelector('#pilule').classList.contains('on'),
      null,
      { timeout: 6000 },
    );
    const e1 = await p.evaluate(() => performance.now());
    verifier(e1 - e0 >= 3950 && e1 - e0 < 4600, 'pilule d’erreur partie à 4 s', String(e1 - e0));
    await fin(p, 'clavier et résultats');
  }

  // (7) suppression et annulation.
  {
    const p = await page();
    const n = await entrees(p);
    await p.locator('.tile').nth(1).click();
    await vider(p);
    await p.keyboard.press('Delete');
    await p.waitForFunction(() => document.querySelectorAll('.tile').length === 11);
    verifier(
      JSON.stringify(await appels(p, 'historique_action')) ===
        JSON.stringify([['historique_action', { nom: n[1], action: 'supprimer' }]]),
      'Suppr → supprimer',
    );
    verifier(!(await noms(p)).includes(n[1]), 'case partie');
    verifier((await focusNom(p)) === n[2], 'focus sur la case suivante');
    const statut = p.getByRole('status');
    await p.waitForFunction(() => document.querySelector('#pilule').classList.contains('on'));
    verifier(
      (await p.locator('#pilule-texte').textContent()) === 'Supprimée',
      'pilule « Supprimée »',
    );
    verifier(
      await statut.getByRole('button', { name: 'Annuler' }).isVisible(),
      'pilule : « Annuler »',
    );
    await animationsFinies(p);
    await pause(p, 400);
    const vu = await p.locator('.tile').nth(1).evaluate(visibles);
    verifier(
      vu.every(Boolean),
      'après Suppr : la case suivante montre sa pilule',
      JSON.stringify(vu),
    );
    await p.screenshot({ path: path.join(SORTIE, 'historique-supprimee.png') });
    await p.keyboard.press('Backspace');
    await p.waitForFunction(() => document.querySelectorAll('.tile').length === 10);
    await p.waitForFunction(
      () => document.querySelector('#pilule-texte').textContent === '2 supprimées',
    );
    verifier(true, 'Retour arrière → deuxième : « 2 supprimées »');
    await vider(p);
    await p.getByRole('button', { name: 'Annuler' }).click();
    await p.waitForFunction(() => document.querySelectorAll('.tile').length === 12);
    verifier(
      JSON.stringify((await appels(p, 'historique_annuler')).map((a) => a[0])) ===
        '["historique_annuler"]',
      '« Annuler » → historique_annuler',
    );
    verifier(
      JSON.stringify(await noms(p)) === JSON.stringify(n),
      'Annuler remet toute la série, dans l’ordre',
    );
    await p.waitForFunction(() => !document.querySelector('#pilule').classList.contains('on'));
    verifier(true, 'historique:attente { n: 0 } → pilule partie');
    verifier(
      [n[1], n[2]].includes(await focusNom(p)),
      'focus sur une case remise',
      await focusNom(p),
    );
    await p.keyboard.press('Delete');
    await p.waitForFunction(() => document.querySelectorAll('.tile').length === 11);
    await vider(p);
    await p.keyboard.press('Control+z');
    await p.waitForFunction(() => document.querySelectorAll('.tile').length === 12);
    verifier(
      JSON.stringify((await appels(p, 'historique_annuler')).map((a) => a[0])) ===
        '["historique_annuler"]',
      'Ctrl + Z → historique_annuler',
    );
    await vider(p);
    await p.keyboard.press('Control+z');
    verifier((await appels(p)).length === 0, 'Ctrl + Z sans suppression en attente : rien');
    // AZERTY : la touche Z envoie `code` KeyW ; c'est la lettre (`key`) qui compte.
    const ctrl = (key, code) =>
      p.evaluate(
        ([key, code]) =>
          document.activeElement.dispatchEvent(
            new KeyboardEvent('keydown', { key, code, ctrlKey: true, bubbles: true }),
          ),
        [key, code],
      );
    await p.keyboard.press('Delete');
    await p.waitForFunction(() => document.querySelectorAll('.tile').length === 11);
    await vider(p);
    await ctrl('w', 'KeyZ');
    await pause(p, 50);
    verifier(
      (await appels(p, 'historique_annuler')).length === 0,
      'Ctrl + W (code KeyZ, QWERTZ ou AZERTY) : rien',
    );
    await ctrl('z', 'KeyW');
    await p.waitForFunction(() => document.querySelectorAll('.tile').length === 12);
    verifier(
      (await appels(p, 'historique_annuler')).length === 1,
      'AZERTY : Ctrl + Z (key z, code KeyW) → historique_annuler',
    );
    await p.locator('.tile').nth(3).click();
    await vider(p);
    await ctrl('c', 'KeyC');
    await ctrl('v', 'KeyC');
    verifier(
      JSON.stringify((await appels(p, 'historique_action')).map((a) => a[1].action)) ===
        '["copier"]',
      'Ctrl + C selon la lettre (key c → copier ; key v, code KeyC → rien)',
    );
    // Bouton Supprimer de la pilule.
    const t = p.locator('.tile').nth(4);
    await t.hover();
    await t.getByRole('button', { name: 'Supprimer' }).click();
    await p.waitForFunction(() => document.querySelectorAll('.tile').length === 11);
    verifier(!(await noms(p)).includes(n[4]), 'bouton Supprimer : case partie');
    await fin(p, 'suppression');
  }

  // (8) nouvelle capture fenêtre ouverte : la case entre en tête, sans saut de focus.
  {
    const p = await page();
    const n = await entrees(p);
    await p.locator('.tile').nth(5).click();
    const anims = await p.evaluate(() => {
      const t = Date.now();
      __entrees.unshift({
        nom: __nomDe(t, 0),
        horodatage_ms: t,
        largeur: 800,
        hauteur: 600,
        annotable: true,
      });
      __envoyerListe();
      // Les animations du script seulement (FLIP et entrée), pas les transitions du CSS.
      return document
        .getAnimations()
        .filter((a) => !(a instanceof CSSTransition) && !(a instanceof CSSAnimation)).length;
    });
    verifier((await cases(p).count()) === 13, 'nouvelle liste : 13 cases');
    const premier = await p.locator('.tile .pv').first().getAttribute('aria-label');
    verifier(
      premier === "Capture, à l'instant",
      'la nouvelle case en tête, « à l’instant »',
      premier,
    );
    verifier((await focusNom(p)) === n[5], 'focus resté sur la même case');
    verifier(anims === 13, 'FLIP : la nouvelle entre, les 12 autres glissent', String(anims));
    await fin(p, 'nouvelle capture');
  }

  // (9) (10) Plus d'options, Tout effacer, vide.
  {
    const p = await page();
    const plus = p.getByRole('button', { name: "Plus d'options" });
    await plus.click();
    const menu = p.getByRole('menu');
    verifier(await menu.isVisible(), 'Plus d’options → menu');
    verifier((await plus.getAttribute('aria-expanded')) === 'true', 'aria-expanded vrai');
    const item = p.getByRole('menuitem', { name: 'Tout effacer' });
    verifier(
      await item.evaluate((e) => e === document.activeElement),
      'focus sur « Tout effacer »',
    );
    verifier((await menu.getByRole('menuitem').count()) === 1, 'menu d’une ligne');
    verifier(
      await item.evaluate(
        (e) =>
          getComputedStyle(e).color ===
          getComputedStyle(document.documentElement)
            .getPropertyValue('--danger')
            .trim()
            .replace(
              /^#(..)(..)(..)$/,
              (_, r, g, b) => `rgb(${parseInt(r, 16)}, ${parseInt(g, 16)}, ${parseInt(b, 16)})`,
            ),
      ),
      '« Tout effacer » en rouge (danger)',
    );
    await pause(p, 400);
    await p.screenshot({ path: path.join(SORTIE, 'historique-menu.png') });
    await p.keyboard.press('Escape');
    verifier(
      !(await menu.isVisible()) && (await plus.evaluate((e) => e === document.activeElement)),
      'Échap ferme le menu, focus à « Plus d’options »',
    );
    await plus.click();
    await item.click();
    const dlg = p.getByRole('alertdialog', { name: 'Tout effacer ?' });
    verifier(await dlg.isVisible(), 'confirmation « Tout effacer ? » (alertdialog)');
    verifier(
      await dlg
        .getByRole('button', { name: 'Annuler' })
        .evaluate((e) => e === document.activeElement),
      'focus sur « Annuler »',
    );
    verifier(await dlg.getByRole('button', { name: 'Effacer' }).isVisible(), 'bouton « Effacer »');
    await pause(p, 400);
    await p.screenshot({ path: path.join(SORTIE, 'historique-confirmation.png') });
    await p.keyboard.press('Escape');
    verifier(
      !(await dlg.isVisible()) && (await plus.evaluate((e) => e === document.activeElement)),
      'Échap ferme la confirmation',
    );
    await plus.click();
    await item.click();
    await dlg.getByRole('button', { name: 'Annuler' }).click();
    verifier(
      (await appels(p, 'historique_tout_effacer')).length === 0,
      '« Annuler » : rien effacé',
    );
    await plus.click();
    await item.click();
    await vider(p);
    await dlg.getByRole('button', { name: 'Effacer' }).click();
    await p.waitForFunction(() => document.querySelectorAll('.tile').length === 0);
    verifier(
      JSON.stringify((await appels(p)).map((a) => a[0])) === '["historique_tout_effacer"]',
      '« Effacer » → un appel historique_tout_effacer',
    );
    verifier(
      await p.evaluate(() => document.activeElement === document.querySelector('#corps')),
      'après « Effacer » : focus sur le corps de la fenêtre',
      await p.evaluate(() => document.activeElement?.id || document.activeElement?.tagName),
    );
    const vide = p.getByRole('img', { name: 'Aucune capture' });
    verifier(await vide.isVisible(), 'vide : picto « Aucune capture »');
    verifier(!(await p.locator('#grille').isVisible()), 'vide : grille cachée');
    verifier(await plus.isDisabled(), 'vide : « Plus d’options » grisé');
    verifier((await p.locator('#corps').innerText()).trim() === '', 'vide : aucun texte');
    await pause(p, 400);
    await p.screenshot({ path: path.join(SORTIE, 'historique-vide.png') });
    await fin(p, 'tout effacer');
  }

  // (10) ouvert vide.
  {
    const p = await page({ ages: [], theme: 'dark' });
    verifier(
      await p.getByRole('img', { name: 'Aucune capture' }).isVisible(),
      'ouvert vide : « Aucune capture »',
    );
    verifier(
      await p.getByRole('button', { name: "Plus d'options" }).isDisabled(),
      'ouvert vide : « Plus d’options » grisé',
    );
    const aff = await appels(p, 'historique_affiche');
    verifier(
      JSON.stringify(aff) === '[["historique_affiche",{"n":0}]]',
      'ouvert vide : historique_affiche { n: 0 }',
      JSON.stringify(aff),
    );
    await p.screenshot({ path: path.join(SORTIE, 'historique-vide-sombre.png') });
    await fin(p, 'ouvert vide');
  }

  // Aperçu manquant, puis annoncé ; case non annotable.
  {
    const p = await page({ manquants: [0, 3], sansAnnoter: true });
    const src = await p.evaluate(() =>
      [...document.querySelectorAll('.tile img')].map((i) => i.hasAttribute('src')),
    );
    verifier(
      !src[0] && !src[3] && src[1] && src[2],
      'aperçu manquant : la case attend sur son fond',
      JSON.stringify(src),
    );
    verifier(
      (await appels(p, 'historique_affiche')).length === 1,
      'aperçu manquant : historique_affiche quand même',
    );
    await p.evaluate(() => {
      __manquants.delete(__entrees[3].nom);
      __emettre('historique:apercu', { nom: __entrees[3].nom });
    });
    await p.waitForFunction(
      () =>
        document.querySelectorAll('.tile img')[3].complete &&
        document.querySelectorAll('.tile img')[3].naturalWidth === 400,
    );
    verifier(true, 'historique:apercu → aperçu redemandé et montré');
    verifier(
      !(await p.evaluate(() => document.querySelectorAll('.tile img')[0].hasAttribute('src'))),
      'l’autre attend toujours',
    );
    const t1 = p.locator('.tile').nth(1);
    await t1.hover();
    verifier(
      JSON.stringify(
        await t1
          .locator('.qa')
          .getByRole('button')
          .evaluateAll((b) => b.map((x) => x.getAttribute('aria-label'))),
      ) === '["Copier","Enregistrer","Supprimer"]',
      'case non annotable : pas d’Annoter',
    );
    await t1.locator('.pv').click();
    await vider(p);
    await p.keyboard.press('Enter');
    await t1.locator('.pv').dblclick();
    verifier(
      (await appels(p)).length === 0,
      'case non annotable : Entrée et double-clic ne font rien',
    );
    await fin(p, 'aperçu manquant');
  }

  // Aperçu annoncé pendant une demande en vol, refusée : la page le redemande.
  {
    const p = await page({ retenus: [0] });
    await p.waitForFunction(() => typeof __lacher[__entrees[0].nom] === 'function');
    await p.evaluate(() => {
      __emettre('historique:apercu', { nom: __entrees[0].nom });
      __lacher[__entrees[0].nom](true);
    });
    await p.waitForFunction(() => {
      const i = document.querySelectorAll('.tile img')[0];
      return i.complete && i.naturalWidth === 400;
    });
    const demandes = await p.evaluate(
      () =>
        __appels.filter((a) => a[0] === 'historique_apercu' && a[1].nom === __entrees[0].nom)
          .length,
    );
    verifier(demandes === 2, 'aperçu annoncé pendant une demande refusée : redemandé, montré');
    await fin(p, 'aperçu en vol');
  }

  // Une case de la première rangée qui part avant son aperçu : historique_affiche part quand même.
  {
    const p = await page({ retenus: [1] });
    await p.waitForFunction(() => typeof __lacher[__entrees[1].nom] === 'function');
    verifier((await appels(p, 'historique_affiche')).length === 0, 'affiche attend la rangée');
    await p.evaluate(() => {
      __entrees.splice(1, 1);
      __envoyerListe();
    });
    await p.waitForFunction(() => __appels.some((a) => a[0] === 'historique_affiche'));
    verifier(true, 'case partie avant son aperçu : historique_affiche envoyé');
    await fin(p, 'case partie');
  }

  // (11) glisser : 4 px → un appel, 3 px : rien.
  {
    const p = await page();
    const n = await entrees(p);
    const b = await p.locator('.tile').nth(2).boundingBox();
    const x = b.x + 40;
    const y = b.y + 30;
    await vider(p);
    await p.mouse.move(x, y);
    await p.mouse.down();
    await p.mouse.move(x + 3, y);
    await p.mouse.move(x, y + 3);
    verifier((await appels(p, 'historique_glisser')).length === 0, 'glisser : 3 px, rien');
    await p.mouse.move(x + 4, y);
    await p.mouse.move(x + 40, y + 20);
    await p.mouse.up();
    verifier(
      JSON.stringify(await appels(p, 'historique_glisser')) ===
        JSON.stringify([['historique_glisser', { nom: n[2] }]]),
      'glisser : 4 px → un seul appel historique_glisser',
      JSON.stringify(await appels(p)),
    );
    verifier((await appels(p, 'historique_action')).length === 0, 'glisser : aucune action');
    await p.evaluate((nom) => __emettre('historique:glisser', { nom, etat: 'debut' }), n[2]);
    verifier(
      await p
        .locator('.tile')
        .nth(2)
        .evaluate((t) => t.classList.contains('glissee')),
      'glisser en cours : case estompée',
    );
    await p.evaluate((nom) => __emettre('historique:glisser', { nom, etat: 'fini' }), n[2]);
    verifier(
      !(await p
        .locator('.tile')
        .nth(2)
        .evaluate((t) => t.classList.contains('glissee'))),
      'glisser fini : case rendue',
    );
    await fin(p, 'glisser');
  }

  // (13) animations réduites : système, puis `reduites` de la charge.
  for (const o of [{ reduit: true }, { reduites: true }]) {
    const p = await page(o);
    const nom = o.reduit ? 'animations réduites (système)' : 'animations réduites (charge)';
    await p.locator('.tile').nth(1).click();
    await p.keyboard.press('Delete');
    await p.waitForFunction(() => document.querySelectorAll('.tile').length === 11);
    const durees = await p.evaluate(() => {
      const ms = (v) =>
        Math.max(...v.split(',').map((s) => parseFloat(s) * (s.includes('ms') ? 1 : 1000)));
      let pire = 0;
      for (const el of document.querySelectorAll('*')) {
        const s = getComputedStyle(el);
        for (const p of [s.transitionDuration, s.animationDuration]) pire = Math.max(pire, ms(p));
        for (const ps of ['::before', '::after']) {
          const q = getComputedStyle(el, ps);
          pire = Math.max(pire, ms(q.animationDuration), ms(q.transitionDuration));
        }
      }
      // Les animations du script (FLIP, entrée) : aucune ; celles du CSS sont à 1 ms.
      const script = document
        .getAnimations()
        .filter((a) => !(a instanceof CSSTransition) && !(a instanceof CSSAnimation));
      return { pire, anims: script.length };
    });
    verifier(
      durees.pire <= 1 && durees.anims === 0,
      `${nom} : rien au-delà de 1 ms, aucun FLIP`,
      JSON.stringify(durees),
    );
    await fin(p, nom);
  }

  // (14) F5, Ctrl + P, Ctrl + molette sans effet de navigateur.
  {
    const p = await page();
    await p.locator('.tile').first().click();
    const empeches = await p.evaluate(() => {
      const k = (key, ctrlKey, code) =>
        !window.dispatchEvent(
          new KeyboardEvent('keydown', { key, code, ctrlKey, cancelable: true, bubbles: true }),
        );
      const w = !window.dispatchEvent(
        new WheelEvent('wheel', { deltaY: -100, ctrlKey: true, cancelable: true, bubbles: true }),
      );
      const m = !window.dispatchEvent(
        new MouseEvent('contextmenu', { cancelable: true, bubbles: true }),
      );
      return [
        k('F5', false, 'F5'),
        k('p', true, 'KeyP'),
        k('+', true, 'Equal'),
        k('0', true, 'Digit0'),
        w,
        m,
      ];
    });
    verifier(
      empeches.every(Boolean),
      'F5, Ctrl + P, Ctrl + +, Ctrl + 0, Ctrl + molette, menu contextuel : empêchés',
      JSON.stringify(empeches),
    );
    await fin(p, 'raccourcis du navigateur');
  }

  // (15) fenêtre étroite 480 × 360, clair et sombre : deux colonnes, rien ne déborde.
  for (const theme of ['light', 'dark']) {
    const p = await page({ largeur: 480, hauteur: 360, theme });
    verifier((await colonnes(p)) === 2, `480 × 360 ${theme} : deux colonnes`);
    const deb = await p.evaluate(() => {
      const g = document.querySelector('#grille');
      const b = document.querySelector('#barre-haut');
      return {
        page: document.documentElement.scrollWidth <= innerWidth,
        grille: g.scrollWidth <= g.clientWidth,
        barre: [...b.children].every((c) => c.getBoundingClientRect().right <= innerWidth + 0.5),
        ecart: getComputedStyle(g).columnGap,
        marge: getComputedStyle(g).paddingLeft,
      };
    });
    verifier(
      deb.page && deb.grille && deb.barre,
      `480 × 360 ${theme} : rien ne déborde`,
      JSON.stringify(deb),
    );
    verifier(
      deb.ecart === '8px' && deb.marge === '8px',
      `480 × 360 ${theme} : écart 8, marge 8`,
      JSON.stringify(deb),
    );
    const t = p.locator('.tile').nth(1);
    await t.hover();
    await pause(p, 400);
    await p.screenshot({ path: path.join(SORTIE, `historique-480-${theme}.png`) });
    await fin(p, `480 × 360 ${theme}`);
  }
  {
    const p = await page();
    const g = await p.evaluate(() => {
      const s = getComputedStyle(document.querySelector('#grille'));
      const t = getComputedStyle(document.querySelector('.tile'));
      const r = document.querySelector('.tile').getBoundingClientRect();
      return {
        ecart: s.columnGap,
        marge: s.paddingLeft,
        rayon: t.borderTopLeftRadius,
        ratio: r.width / r.height,
        l: r.width,
      };
    });
    verifier(
      g.ecart === '10px' &&
        g.marge === '12px' &&
        g.rayon === '6px' &&
        Math.abs(g.ratio - 1.6) < 0.02 &&
        g.l >= 150,
      '760 × 560 : écart 10, marge 12, rayon 6, cases 16:10 d’au moins 150 px',
      JSON.stringify(g),
    );
    await fin(p, 'mesures');
  }

  // (16) 50 cases : historique_affiche envoyé une fois, avec la première rangée.
  {
    const p = await page({ n: 50 });
    const aff = await appels(p, 'historique_affiche');
    const cols = await colonnes(p);
    verifier((await cases(p).count()) === 50, '50 cases');
    verifier(
      aff.length === 1 && aff[0][1].n === cols,
      `historique_affiche une fois, n = ${cols}`,
      JSON.stringify(aff),
    );
    const ms = await p.evaluate(() => window.__afficheA);
    console.log(
      `MESURE  50 cases : historique_affiche à ${Math.round(ms)} ms du départ de la page`,
    );
    await p.waitForFunction(() =>
      [...document.querySelectorAll('.tile img')].every(
        (i) => i.complete && i.naturalWidth === 400,
      ),
    );
    verifier(true, '50 aperçus montrés');
    await p.locator('.tile').nth(48).click();
    await p.keyboard.press('ArrowDown');
    verifier(
      (await focusNom(p)) === (await entrees(p))[49],
      '50 : flèche bas sur la dernière rangée → dernière case',
    );
    await fin(p, '50 cases');
  }

  await nav.close();
  srv.close();
  console.log(`\nREUSSIS : ${reussis}  ECHECS : ${echecs}`);
  process.exit(echecs ? 1 : 0);
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
