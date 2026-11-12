const { chromium } = require('playwright');
const http = require('http');
const fs = require('fs');
const path = require('path');
// Essai de la page de l'éditeur d'annotation dans Chromium (contrat 0.4 « Éditeur », §12.2, et
// avenant « Éditeur épuré », piste B). Sort avec 1 si une vérification échoue. Copies d'écran
// dans $SORTIE.
//   node essais/pages/editeur.cjs
// Le cœur est simulé (`__TAURI__` qui note les appels, rend des octets connus et garde le
// corps et les en-têtes des exports). Le modèle et le dessin sont les vrais (document.js et
// dessin.js) : chaque geste est prouvé par l'objet que le vrai modèle a reçu.
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

// Le cœur simulé : `__appels` (commande, arguments, en-têtes), `__exports` (corps gardés),
// `__emettre(nom, charge)`. L'image : octets connus, d'après une graine.
function simulerCoeur({ charge, graine, alpha, reponseFermer }) {
  window.__essaiEditeur = true;
  window.__appels = [];
  window.__exports = [];
  window.__ecouteurs = {};
  window.__fermer = reponseFermer;
  window.__charge = charge;
  document.addEventListener(
    'securitypolicyviolation',
    (e) => (window.__csp = (window.__csp || []).concat(e.violatedDirective)),
  );
  window.__image = (c, g) => {
    const n = c.largeur * c.hauteur * 4;
    const o = new Uint8Array(n);
    let s = g >>> 0;
    for (let i = 0; i < n; i++) {
      s = (s * 1664525 + 1013904223) >>> 0;
      o[i] = s >>> 24;
    }
    if (alpha) for (let i = 3; i < n; i += 4) o[i] = [0, 128, 255][(i >> 2) % 3];
    else for (let i = 3; i < n; i += 4) o[i] = 255;
    return o;
  };
  let numero = 0;
  window.__TAURI__ = {
    core: {
      invoke: (c, a, opt) => {
        if (c === 'editeur_exporter') {
          numero += 1;
          window.__appels.push([c, { octets: a.byteLength }, opt && opt.headers]);
          window.__exports.push({ numero, entetes: opt.headers, longueur: a.byteLength });
          window.__dernierCorps = a;
          return Promise.resolve(numero);
        }
        window.__appels.push([c, a]);
        if (c === 'editeur_pret') return Promise.resolve(window.__charge);
        if (c === 'editeur_image')
          return Promise.resolve(window.__image(window.__charge, g).buffer);
        if (c === 'editeur_fermer') return Promise.resolve(a.force ? 'fermee' : window.__fermer);
        return Promise.resolve(null);
      },
    },
    event: {
      listen: (n, f) => {
        window.__ecouteurs[n] = f;
        return Promise.resolve(() => {});
      },
    },
  };
  const g = graine;
  window.__emettre = (n, payload) => window.__ecouteurs[n] && window.__ecouteurs[n]({ payload });
}

const CHARGE = {
  id: 7,
  largeur: 400,
  hauteur: 300,
  transparence: false,
  reduites: false,
  preferences: null,
};

(async () => {
  const srv = serveur();
  await new Promise((ok) => srv.listen(0, ok));
  const URL_PAGE = `http://127.0.0.1:${srv.address().port}/editeur/editeur.html`;
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
      viewport: { width: o.largeur || 1200, height: o.hauteur || 800 },
      deviceScaleFactor: o.echelle || 1,
      colorScheme: o.theme || 'light',
      reducedMotion: o.reduit ? 'reduce' : 'no-preference',
    });
    if (o.horloge) await ctx.clock.install();
    const p = await ctx.newPage();
    p.setDefaultTimeout(5000);
    p.erreurs = [];
    p.on('pageerror', (e) => p.erreurs.push(String(e)));
    p.on('console', (m) => {
      if (m.type() === 'error' || m.type() === 'warning') p.erreurs.push(m.text());
    });
    await p.addInitScript(simulerCoeur, {
      charge: { ...CHARGE, ...(o.charge || {}) },
      graine: o.graine || 12345,
      alpha: !!o.alpha,
      reponseFermer: o.reponseFermer || 'demander',
    });
    await p.goto(URL_PAGE);
    if (o.horloge) return p;
    await p
      .waitForFunction(() => window.__appels.some((a) => a[0] === 'editeur_affiche'), null, {
        timeout: 5000,
      })
      .catch(() => {});
    return p;
  }
  // Fin d'une page : aucune erreur de console, aucune violation de CSP.
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
  const docu = (p) => p.evaluate(() => window.__ed.h.doc);
  const objets = async (p, type) => (await docu(p)).objets.filter((o) => !type || o.type === type);
  // Deux images d'animation : la page a peint ce que l'événement a changé.
  const uneImage = (p) =>
    p.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
  // Attend un appel au cœur dont les champs valent `egal` ; jamais une durée fixe. Pas
  // d'appel dans les 3 s : la vérification qui suit échoue.
  const attendreAppel = (p, nom, egal = {}) =>
    p
      .waitForFunction(
        ([n, e]) =>
          __appels.some(
            (a) => a[0] === n && Object.entries(e).every(([k, v]) => a[1] && a[1][k] === v),
          ),
        [nom, egal],
        { timeout: 3000 },
      )
      .catch(() => {});
  // Pixels de l'image → point CSS de la page.
  const point = async (p, x, y) => {
    const v = await p.evaluate(() => ({ ...window.__ed.vue }));
    const r = await p.$eval('#vue', (t) => {
      const b = t.getBoundingClientRect();
      return [b.left, b.top];
    });
    return [r[0] + (v.x + x * v.echelle) / v.dpr, r[1] + (v.y + y * v.echelle) / v.dpr];
  };
  const glisserCss = async (p, x1, y1, x2, y2) => {
    await p.mouse.move(x1, y1);
    await p.mouse.down();
    for (let i = 1; i <= 6; i++)
      await p.mouse.move(x1 + ((x2 - x1) * i) / 6, y1 + ((y2 - y1) * i) / 6);
    await p.mouse.up();
    await uneImage(p);
  };
  const glisser = async (p, a, b) => {
    const [x1, y1] = await point(p, a[0], a[1]);
    const [x2, y2] = await point(p, b[0], b[1]);
    await glisserCss(p, x1, y1, x2, y2);
  };
  const cliquer = async (p, x, y) => {
    const [a, b] = await point(p, x, y);
    await p.mouse.click(a, b);
    await uneImage(p);
  };
  const bouton = (p, nom) => p.getByRole('button', { name: nom, exact: true });
  const outil = async (p, nom) => {
    await bouton(p, nom).click();
    await uneImage(p);
  };
  const ajouter = async (p, nom) => {
    await bouton(p, 'Ajouter').click();
    await p.getByRole('menuitem', { name: nom, exact: true }).click();
    await uneImage(p);
  };
  const ouvert = (p, sel) => p.$eval(sel, (m) => m.classList.contains('ouvert'));
  const choisi = (p) =>
    p.$$eval('#outils [aria-pressed="true"]', (bs) => bs.map((b) => b.getAttribute('aria-label')));
  // Une variante par la bulle de l'outil (l'instrument recliqué).
  const variante = async (p, outilNom, nom) => {
    if (JSON.stringify(await choisi(p)) !== JSON.stringify([outilNom])) await outil(p, outilNom);
    if (!(await ouvert(p, '#reglages-outil'))) await outil(p, outilNom);
    await p.locator('#reglages-outil').getByRole('button', { name: nom, exact: true }).click();
    await uneImage(p);
  };

  // ---------- 1. chargement ----------
  {
    const p = await page();
    const a = await appels(p);
    const noms = a.map((x) => x[0]);
    const iImage = noms.indexOf('editeur_image');
    verifier(
      noms[0] === 'editeur_pret' &&
        noms.filter((n) => n === 'editeur_image').length === 1 &&
        JSON.stringify(a[iImage][1]) === '{"id":7}',
      '1 editeur_pret puis une seule image',
      JSON.stringify(noms),
    );
    const affiches = a.filter((x) => x[0] === 'editeur_affiche');
    const peint = await p.evaluate(() => {
      const v = __ed.vue;
      return __ed.ctx.getImageData(v.x + 100, v.y + 100, 1, 1).data[3];
    });
    verifier(
      affiches.length === 1 && JSON.stringify(affiches[0][1]) === '{"id":7}' && peint === 255,
      '1 editeur_affiche une fois, après le premier dessin',
      JSON.stringify([affiches, peint]),
    );
    await attendreAppel(p, 'editeur_vue');
    const vue = await appels(p, 'editeur_vue');
    verifier(vue.length === 1 && vue[0][1].id === 7, '1 editeur_vue à la fin du placement');
    await fin(p, '1');
  }

  // ---------- 2. noms et rôles (avenant §4), absents, barre de la fenêtre ----------
  {
    const p = await page();
    const nb = async (role, nom) => p.getByRole(role, { name: nom, exact: true }).count();
    const haut = p.locator('#barre-haut');
    let ok = true;
    const manque = [];
    for (const n of ['Fermer', 'Enregistrer', 'Copier']) {
      if ((await haut.getByRole('button', { name: n, exact: true }).count()) !== 1) manque.push(n);
    }
    const pal = p.getByRole('toolbar', { name: 'Annotation', exact: true });
    ok = (await pal.count()) === 1;
    for (const n of ['Annuler', 'Rétablir', 'Flèche', 'Stylo', 'Surligneur', 'Gomme']) {
      if ((await pal.getByRole('button', { name: n, exact: true }).count()) !== 1) manque.push(n);
    }
    for (const n of ['Ajouter', 'Plus']) {
      if ((await pal.getByRole('button', { name: n, exact: true }).count()) !== 1) manque.push(n);
    }
    const ordre = await p.$$eval('#couleurs .dot', (bs) =>
      bs.map((b) => b.getAttribute('aria-label')),
    );
    verifier(
      ok &&
        !manque.length &&
        JSON.stringify(ordre) === '["Rouge","Jaune","Vert","Autre couleur","Bleu","Noir","Blanc"]',
      '2 barre du haut, palette « Annotation », pastilles dans l’ordre (Autre couleur 4e)',
      JSON.stringify([manque, ordre]),
    );
    const pastilles = [];
    for (const n of ['Rouge', 'Jaune', 'Vert', 'Autre couleur', 'Bleu', 'Noir', 'Blanc'])
      pastilles.push(await pal.getByRole('button', { name: n, exact: true }).count());
    verifier(
      pastilles.every((x) => x === 1),
      '2 pastilles : boutons nommés',
      String(pastilles),
    );
    verifier(
      (await p.locator('#bouton-couleur').getAttribute('aria-label')) === 'Couleur' &&
        (await nb('button', 'Couleur')) === 0,
      '2 « Couleur » nommé, caché en fenêtre large',
    );
    await bouton(p, 'Ajouter').click();
    const menuA = p.getByRole('menu', { name: 'Ajouter', exact: true });
    const itemsA = await menuA.getByRole('menuitem').allTextContents();
    verifier(
      JSON.stringify(itemsA.map((t) => t.trim())) ===
        '["Texte","Forme","Numéro","Flou","Projecteur"]',
      '2 menu « Ajouter » : Texte, Forme, Numéro, Flou, Projecteur',
      JSON.stringify(itemsA),
    );
    await p.keyboard.press('Escape');
    await bouton(p, 'Plus').click();
    const itemsP = await p
      .getByRole('menu', { name: 'Plus', exact: true })
      .getByRole('menuitem')
      .allTextContents();
    verifier(
      JSON.stringify(itemsP.map((t) => t.trim())) ===
        '["Enregistrer sous","Glisser vers une appli"]',
      '2 menu « Plus » : Enregistrer sous, Glisser vers une appli',
      JSON.stringify(itemsP),
    );
    await p.keyboard.press('Escape');
    const r = await p.evaluate(() => {
      const tous = [...document.querySelectorAll('*')];
      const noms = tous.map((e) => (e.getAttribute('aria-label') || '') + '|' + (e.title || ''));
      const textes = document.body.innerText;
      const interdits = [
        'Copier le texte',
        'Fond',
        'Épingler',
        'Pipette',
        'Zoom',
        'Tout copier',
        'Sélection',
        'Recadrer',
        'Ajuster',
        'Taille réelle',
      ];
      const trouves = interdits.filter(
        (n) => noms.some((x) => x.split('|').includes(n)) || textes.includes(n),
      );
      const boutons = [...document.querySelectorAll('button, [role="button"], [role="menuitem"]')];
      return {
        trouves,
        ocr: !!document.querySelector('#ocr, .pin, .allc, [draggable="true"]'),
        drag:
          document.querySelector('#barre-haut').hasAttribute('data-tauri-drag-region') &&
          document.querySelector('#barre-haut .sp').hasAttribute('data-tauri-drag-region'),
        dragBouton: boutons.filter((b) => b.closest('[data-tauri-drag-region]') === b).length,
        titres: boutons
          .filter((b) => b.title && b.title !== b.getAttribute('aria-label'))
          .map((b) => b.title),
        kbd: document.querySelectorAll('kbd, [data-key], [aria-keyshortcuts]').length,
        aide: [...document.querySelectorAll('[aria-description], [aria-describedby]')].length,
      };
    });
    verifier(
      r.trouves.length === 0 && !r.ocr,
      '2 absents : Copier le texte, Fond, Épingler, Pipette, Zoom, Sélection, Recadrer…',
      JSON.stringify(r),
    );
    verifier(
      r.drag && r.dragBouton === 0,
      '2 data-tauri-drag-region sur la barre et son vide, sur aucun bouton',
      JSON.stringify(r),
    );
    verifier(
      r.titres.length === 0 && r.kbd === 0 && r.aide === 0,
      '2 infobulle = le nom seul, aucun raccourci affiché, aucun texte d’aide',
      JSON.stringify(r),
    );
    const t = await p.evaluate(() => {
      const b = document.querySelector('#barre-haut').getBoundingClientRect();
      const pal = document.querySelector('#palette').getBoundingClientRect();
      return { h: b.height, palH: pal.height, bas: innerHeight - pal.bottom };
    });
    verifier(
      t.h === 52 && t.palH === 72 && t.bas === 16,
      '2 barre 52 px, palette 72 px à 16 px du bas',
      JSON.stringify(t),
    );
    await fin(p, '2');
  }

  // ---------- 3. export sans rien faire : octets égaux (base → fond → export) ----------
  for (const [l, h, alpha] of [
    [1024, 768, false],
    [1023, 767, true],
  ]) {
    const p = await page({ charge: { largeur: l, hauteur: h }, alpha, graine: 99 });
    await vider(p);
    await p.keyboard.press('Control+c');
    await p.waitForFunction(() => window.__exports.length === 1);
    const r = await p.evaluate(() => {
      const e = window.__exports[0];
      const a = window.__dernierCorps;
      const o = window.__image(window.__charge, 99);
      let ecarts = 0;
      let alphaFaux = 0;
      for (let i = 0; i < o.length; i += 4) {
        if (a[i + 3] !== o[i + 3]) alphaFaux++;
        if (o[i + 3] === 0) continue;
        const tol = o[i + 3] < 255 ? 3 : 0;
        for (let k = 0; k < 3; k++) if (Math.abs(a[i + k] - o[i + k]) > tol) ecarts++;
      }
      return { e: e.entetes, longueur: a.byteLength, attendu: o.length, ecarts, alphaFaux };
    });
    verifier(
      r.longueur === r.attendu && r.ecarts === 0 && r.alphaFaux === 0,
      `3 ${l}×${h}${alpha ? ' alpha 0/128/255' : ''} : export égal à l’image`,
      JSON.stringify(r),
    );
    verifier(
      JSON.stringify(r.e) ===
        JSON.stringify({
          'x-editeur-id': '7',
          'x-editeur-version': '0',
          'x-editeur-but': 'copier',
          'x-editeur-largeur': String(l),
          'x-editeur-hauteur': String(h),
        }),
      `3 ${l}×${h} : en-têtes de l’export (Ctrl+C : « copier »)`,
      JSON.stringify(r.e),
    );
    await fin(p, `3 ${l}×${h}`);
  }

  // ---------- 4. chaque outil crée son objet (vrai modèle) ----------
  {
    const p = await page();
    verifier(JSON.stringify(await choisi(p)) === '["Flèche"]', '4 Flèche choisie au départ');
    await glisser(p, [20, 20], [120, 40]);
    let o = (await objets(p, 'fleche'))[0];
    verifier(
      o &&
        o.couleur === '#E5484D' &&
        o.epaisseur === 1 &&
        o.opacite === 1 &&
        o.cx == null &&
        o.id === 1,
      '4 Flèche : flèche droite rouge (ajouter du vrai modèle)',
      JSON.stringify(o),
    );
    await variante(p, 'Flèche', 'Courbe');
    await p.keyboard.press('Escape');
    await glisser(p, [20, 60], [120, 90]);
    o = (await objets(p, 'fleche'))[1];
    verifier(o && o.cx != null && o.courbe === undefined, '4 Flèche courbe', JSON.stringify(o));
    await outil(p, 'Stylo');
    await glisser(p, [20, 120], [100, 150]);
    o = (await objets(p, 'crayon'))[0];
    verifier(o && o.points.length > 7, '4 Stylo : crayon lissé', JSON.stringify(o));
    await outil(p, 'Surligneur');
    await glisser(p, [20, 170], [100, 170]);
    o = (await objets(p, 'surligneur'))[0];
    verifier(o && o.couleur === '#FFD43B', '4 Surligneur : jaune', JSON.stringify(o));
    const avant = await p.$eval('[data-place="emplacement"]', (b) => b.hidden);
    await ajouter(p, 'Forme');
    const empl = await p.$eval('[data-place="emplacement"]', (b) => [
      b.hidden,
      b.getAttribute('aria-label'),
      b.getAttribute('aria-pressed'),
    ]);
    verifier(
      avant === true && JSON.stringify(empl) === '[false,"Forme","true"]',
      '4 « Ajouter » → Forme : l’emplacement apparaît, nommé, choisi',
      JSON.stringify([avant, empl]),
    );
    await glisser(p, [150, 20], [230, 80]);
    o = (await objets(p, 'rectangle'))[0];
    verifier(
      o && o.plein === false && o.arrondi === 6 && o.x1 < o.x2,
      '4 Forme : rectangle en contour, rayon 6, normalisé',
      JSON.stringify(o),
    );
    await variante(p, 'Forme', 'Ellipse');
    await p.keyboard.press('Escape');
    await glisser(p, [250, 80], [170, 120]);
    o = (await objets(p, 'ellipse'))[0];
    verifier(o && o.x1 < o.x2 && o.y1 < o.y2, '4 Forme : ellipse normalisée', JSON.stringify(o));
    await ajouter(p, 'Numéro');
    await cliquer(p, 300, 30);
    await cliquer(p, 330, 30);
    const nums = await objets(p, 'numero');
    verifier(
      nums.length === 2 && nums[0].ordre === 1 && nums[1].ordre === 2 && nums[0].rayon === 16,
      '4 Numéro : deux numéros, ordres 1 et 2',
      JSON.stringify(nums),
    );
    await ajouter(p, 'Flou');
    await glisser(p, [260, 150], [380, 200]);
    o = (await objets(p, 'masque'))[0];
    verifier(
      o && o.sorte === 'pixeliser' && o.intensite === 1 && Number.isInteger(o.graine),
      '4 Flou : masque pixeliser (graine du modèle)',
      JSON.stringify(o),
    );
    await ajouter(p, 'Projecteur');
    await glisser(p, [150, 200], [154, 204]);
    verifier((await objets(p, 'projecteur')).length === 0, '4 Projecteur de 6 px ou moins : rien');
    await glisser(p, [230, 280], [150, 220]);
    o = (await objets(p, 'projecteur'))[0];
    verifier(
      o &&
        o.forme === 'rectangle' &&
        o.x1 < o.x2 &&
        o.y1 < o.y2 &&
        o.couleur === undefined &&
        o.opacite === undefined,
      '4 Projecteur : rectangle normalisé, sans couleur ni opacité',
      JSON.stringify(o),
    );
    await variante(p, 'Projecteur', 'Ovale');
    await p.keyboard.press('Escape');
    await glisser(p, [20, 200], [120, 280]);
    o = (await objets(p, 'projecteur'))[1];
    verifier(o && o.forme === 'ovale', '4 Projecteur ovale', JSON.stringify(o));
    // Le tracé en cours passe par options.ajout : le voile apparaît pendant le geste.
    const [x1, y1] = await point(p, 300, 230);
    await p.mouse.move(x1, y1);
    await p.mouse.down();
    await p.mouse.move(x1 + 30, y1 + 30);
    await uneImage(p);
    const enCours = await p.evaluate(() => __ed.enCours && __ed.enCours.type);
    await p.mouse.up();
    verifier(enCours === 'projecteur', '4 Projecteur : tracé en cours (ajout)');
    await ajouter(p, 'Texte');
    await cliquer(p, 340, 110);
    await p.keyboard.type('Bonjour');
    await p.keyboard.press('Enter');
    await uneImage(p);
    o = (await objets(p, 'texte'))[0];
    verifier(
      o && o.texte === 'Bonjour' && o.gras === true && o.style === 'contour' && o.taille === 28,
      '4 Texte : gras, contour, 28 px',
      JSON.stringify(o),
    );
    verifier(
      (await p.$eval('[data-place="emplacement"]', (b) => b.getAttribute('aria-label'))) ===
        'Texte' && (await p.$$('#outils .tl:not([hidden])')).length === 5,
      '4 l’emplacement montre le dernier outil de « Ajouter » (un seul)',
    );
    await p.keyboard.press('b');
    await uneImage(p);
    verifier(
      JSON.stringify(await choisi(p)) === '["Flou"]' &&
        (await p.evaluate(() => __ed.prefs.variantes.flou)) === 'pixeliser',
      '4 touche B : Flou (Pixeliser) dans l’emplacement',
    );
    await p.keyboard.press('a');
    await uneImage(p);
    verifier(JSON.stringify(await choisi(p)) === '["Flèche"]', '4 touche A : Flèche');
    await fin(p, '4');
  }

  // ---------- 5. Gomme : chaque objet touché supprimé, un geste = un pas ----------
  {
    const p = await page();
    await glisser(p, [50, 50], [150, 50]);
    await glisser(p, [50, 150], [150, 150]);
    await glisser(p, [50, 250], [150, 250]);
    const pas0 = await p.evaluate(() => __ed.h.passe.length);
    await outil(p, 'Gomme');
    const curseur = await p.evaluate(() => {
      const t = document.querySelector('#vue');
      t.dispatchEvent(new PointerEvent('pointermove', { bubbles: true, clientX: 1, clientY: 1 }));
      return null;
    });
    void curseur;
    const [gx, gy] = await point(p, 100, 20);
    const [hx, hy] = await point(p, 100, 180);
    await glisserCss(p, gx, gy, hx, hy);
    const r = await p.evaluate(() => ({
      n: __ed.h.doc.objets.length,
      pas: __ed.h.passe.length,
      cur: document.querySelector('#scene').dataset.curseur,
    }));
    verifier(
      r.n === 1 && r.pas === pas0 + 1,
      '5 Gomme : deux objets touchés supprimés en un seul pas',
      JSON.stringify([pas0, r]),
    );
    await bouton(p, 'Gomme').click();
    verifier(!(await ouvert(p, '#reglages-outil')), '5 Gomme recliquée : aucune bulle');
    await p.keyboard.press('Control+z');
    await uneImage(p);
    verifier((await objets(p)).length === 3, '5 Annuler : les deux objets reviennent');
    await fin(p, '5');
  }

  // ---------- 6. sélection, déplacement, suppression ----------
  {
    const p = await page();
    await ajouter(p, 'Forme');
    await glisser(p, [100, 100], [200, 180]);
    await outil(p, 'Flèche');
    await p.evaluate(() => {
      __ed.selection = [];
      __ed.dessiner();
    });
    const pas0 = await p.evaluate(() => __ed.h.passe.length);
    // Appui sur le bord du rectangle avec Flèche : sélection ; 2 px : rien ne bouge.
    const [ax, ay] = await point(p, 100, 140);
    await p.mouse.move(ax, ay);
    await p.mouse.down();
    await p.mouse.move(ax + 2, ay);
    await uneImage(p);
    const r0 = await p.evaluate(() => ({ s: __ed.selection, x: __ed.h.doc.objets[0].x1 }));
    await p.mouse.move(ax + 30, ay + 10);
    await p.mouse.move(ax + 40, ay + 20);
    await p.mouse.up();
    await uneImage(p);
    const r1 = await p.evaluate(() => ({
      s: __ed.selection,
      o: __ed.h.doc.objets[0],
      n: __ed.h.doc.objets.length,
      pas: __ed.h.passe.length,
    }));
    verifier(
      JSON.stringify(r0.s) === '[1]' && r0.x === 100,
      '6 appui sur un objet avec Flèche : choisi, immobile sous 3 px',
      JSON.stringify(r0),
    );
    verifier(
      r1.n === 1 &&
        Math.round(r1.o.x1) === 140 &&
        Math.round(r1.o.y1) === 120 &&
        r1.pas === pas0 + 1,
      '6 glissé : déplacé de 40 × 20, un pas, rien de créé',
      JSON.stringify(r1),
    );
    await p.keyboard.press('Delete');
    await uneImage(p);
    verifier((await objets(p)).length === 0, '6 Suppr : supprimé');
    await p.keyboard.press('Control+z');
    await uneImage(p);
    await outil(p, 'Stylo');
    await glisser(p, [145, 125], [200, 160]);
    const r2 = await p.evaluate(() => ({
      s: __ed.selection,
      t: __ed.h.doc.objets.map((o) => o.type),
    }));
    verifier(
      JSON.stringify(r2) === '{"s":[],"t":["rectangle","crayon"]}',
      '6 avec Stylo, l’appui sur un objet trace sans le prendre',
      JSON.stringify(r2),
    );
    await outil(p, 'Flèche');
    await cliquer(p, 140, 160);
    await p.keyboard.press('Backspace');
    await uneImage(p);
    verifier(
      JSON.stringify((await objets(p)).map((o) => o.type)) === '["crayon"]',
      '6 Retour arrière : supprimé',
    );
    await cliquer(p, 160, 150);
    await p.keyboard.press('Escape');
    await uneImage(p);
    verifier((await p.evaluate(() => __ed.selection.length)) === 0, '6 Échap : désélectionne');
    await fin(p, '6');
  }

  // ---------- 7. recadrage par les quatre équerres ----------
  {
    const p = await page({ reduit: true });
    const coin = async (cx, cy, dx, dy) => {
      const [x, y] = await point(p, cx, cy);
      await glisserCss(p, x, y, x + dx, y + dy);
    };
    const cadre = () => p.evaluate(() => __ed.h.doc.recadrage);
    const e0 = await p.evaluate(() => __ed.vue.echelle);
    // Pendant le geste : toute l'image à l'échelle du début, le voile.
    const [x0, y0] = await point(p, 0, 0);
    await p.mouse.move(x0, y0);
    await p.mouse.down();
    await p.mouse.move(x0 + 20, y0 + 20);
    await p.mouse.move(x0 + 40, y0 + 30);
    await uneImage(p);
    const pendant = await p.evaluate(() => ({
      e: __ed.vue.echelle,
      r: __ed.outils.recadrageEnCours(),
      d: __ed.h.doc.recadrage,
    }));
    await p.mouse.up();
    await uneImage(p);
    const pas1 = await p.evaluate(() => __ed.h.passe.length);
    verifier(
      pendant.e === e0 && pendant.r && pendant.d === null,
      '7 pendant le geste : même échelle, rien de changé',
      JSON.stringify(pendant),
    );
    let c = await cadre();
    verifier(
      c && c.x === 40 && c.y === 30 && c.largeur === 360 && c.hauteur === 270 && pas1 === 1,
      '7 coin haut gauche : recadrer, un pas',
      JSON.stringify(c),
    );
    c = await cadre();
    let [bx, by] = [c.x + c.largeur, c.y];
    await coin(bx, by, -40 * (await p.evaluate(() => __ed.vue.echelle)), 0);
    c = await cadre();
    verifier(c && c.x === 40 && c.largeur === 320, '7 coin haut droit', JSON.stringify(c));
    const s = () => p.evaluate(() => __ed.vue.echelle / __ed.vue.dpr);
    await coin(c.x, c.y + c.hauteur, 0, -20 * (await s()));
    c = await cadre();
    verifier(c && c.hauteur === 250 && c.y === 30, '7 coin bas gauche', JSON.stringify(c));
    await coin(c.x + c.largeur, c.y + c.hauteur, -1000, -1000);
    c = await cadre();
    verifier(
      c && c.largeur === 40 && c.hauteur === 40,
      '7 coin bas droit : 40 px au moins',
      JSON.stringify(c),
    );
    await coin(c.x + c.largeur, c.y + c.hauteur, 2000, 2000);
    await coin(c.x, c.y, -2000, -2000);
    c = await cadre();
    const pas = await p.evaluate(() => __ed.h.passe.length);
    verifier(
      c === null && pas === 6,
      '7 retour à l’image entière : null',
      JSON.stringify([c, pas]),
    );
    await p.keyboard.press('Control+z');
    await uneImage(p);
    c = await cadre();
    verifier(
      c && c.x === 40 && c.y === 30 && c.largeur === 360,
      '7 Annuler : un pas',
      JSON.stringify(c),
    );
    const curseur = await (async () => {
      const k = await cadre();
      const [x, y] = await point(p, k.x + k.largeur, k.y);
      await p.mouse.move(x + 2, y - 2);
      return p.evaluate(() => document.querySelector('#scene').dataset.curseur);
    })();
    verifier(curseur === 'nesw', '7 curseur sur une équerre', curseur);
    await fin(p, '7');
  }

  // ---------- 8. annuler, rétablir (logique gardée) ----------
  {
    const p = await page();
    await glisser(p, [20, 20], [120, 40]);
    await glisser(p, [20, 60], [120, 80]);
    await bouton(p, 'Annuler').click();
    await uneImage(p);
    const a = await p.evaluate(() => [__ed.h.doc.objets.length, __ed.h.version]);
    await p.keyboard.press('Control+y');
    await uneImage(p);
    const b = await p.evaluate(() => __ed.h.doc.objets.length);
    await p.keyboard.press('Control+Shift+z');
    const dis = await p.$eval('[data-act="retablir"]', (x) => x.disabled);
    verifier(
      a[0] === 1 && b === 2 && dis,
      '8 Annuler, Ctrl+Y, Rétablir grisé au bout',
      JSON.stringify([a, b, dis]),
    );
    await fin(p, '8');
  }

  // ---------- 9. bulle « Réglages de l'outil » ----------
  {
    const p = await page();
    verifier(!(await ouvert(p, '#reglages-outil')), '9 bulle fermée au départ');
    await outil(p, 'Flèche');
    const bulle = p.getByRole('dialog', { name: 'Réglages de l’outil'.replace('’', "'") });
    const r = await p.evaluate(() => {
      const d = document.querySelector('#reglages-outil');
      const i = d.querySelector('input[type="range"]');
      return {
        ouvert: d.classList.contains('ouvert'),
        noms: [...d.querySelectorAll('.opt')].map((b) => b.getAttribute('aria-label')),
        range: i && [i.min, i.max, i.step, i.value, i.getAttribute('aria-label')],
        lignes: d.querySelectorAll('.row').length,
      };
    });
    verifier(
      (await bulle.count()) === 1 &&
        r.ouvert &&
        JSON.stringify(r.noms) === '["Droite","Courbe","Fin","Moyen","Épais"]' &&
        JSON.stringify(r.range) === '["20","100","5","100","Opacité"]' &&
        r.lignes === 3,
      '9 Flèche recliquée : variantes, trois tailles, Opacité 20 à 100 pas 5',
      JSON.stringify(r),
    );
    await p.locator('#reglages-outil').getByRole('button', { name: 'Épais' }).click();
    await p.$eval('#reglages-outil input', (i) => {
      i.value = 55;
      i.dispatchEvent(new Event('input', { bubbles: true }));
      i.dispatchEvent(new Event('change', { bubbles: true }));
    });
    await outil(p, 'Flèche');
    verifier(!(await ouvert(p, '#reglages-outil')), '9 recliquer referme la bulle');
    await glisser(p, [20, 20], [120, 40]);
    const o = (await objets(p, 'fleche'))[0];
    verifier(
      o && o.epaisseur === 2 && o.opacite === 0.55,
      '9 taille et opacité appliquées',
      JSON.stringify(o),
    );
    // Objet choisi : la bulle le règle (un pas).
    await cliquer(p, 70, 30);
    await outil(p, 'Flèche');
    await p.locator('#reglages-outil').getByRole('button', { name: 'Fin' }).click();
    await uneImage(p);
    verifier((await objets(p, 'fleche'))[0].epaisseur === 0, '9 l’objet choisi prend la taille');
    await p.keyboard.press('Escape');
    const contenu = async (aj, nom) => {
      if (aj) await ajouter(p, aj);
      await outil(p, nom);
      if (!(await ouvert(p, '#reglages-outil'))) await outil(p, nom);
      return p.evaluate(() => {
        const d = document.querySelector('#reglages-outil');
        return [
          ...[...d.querySelectorAll('.opt')].map((b) => b.getAttribute('aria-label')),
          ...(d.querySelector('input') ? ['Opacité'] : []),
        ].join(',');
      });
    };
    const surl = await contenu(null, 'Surligneur');
    await p.keyboard.press('Escape');
    const texte = await contenu('Texte', 'Texte');
    const tailles = await p.$$eval('#reglages-outil .opt .a', (s) =>
      s.map((x) => getComputedStyle(x).fontSize),
    );
    await p.locator('#reglages-outil').getByRole('button', { name: 'Épais' }).click();
    const tt = await p.evaluate(() => __ed.prefs.tailleTexte);
    await p.keyboard.press('Escape');
    const flou = await contenu('Flou', 'Flou');
    await p.locator('#reglages-outil').getByRole('button', { name: 'Bloc noir' }).click();
    await uneImage(p);
    const bloc = await p.evaluate(() =>
      [...document.querySelectorAll('#reglages-outil .opt')]
        .map((b) => b.getAttribute('aria-label'))
        .join(','),
    );
    await p.keyboard.press('Escape');
    const spot = await contenu('Projecteur', 'Projecteur');
    await p.keyboard.press('Escape');
    const num = await contenu('Numéro', 'Numéro');
    await p.keyboard.press('Escape');
    verifier(
      surl === 'Fin,Moyen,Épais' &&
        texte === 'Fin,Moyen,Épais,Opacité' &&
        JSON.stringify(tailles) === '["12px","16px","21px"]' &&
        tt === 40 &&
        flou === 'Pixeliser,Flouter,Bloc noir,Léger,Moyen,Fort' &&
        bloc === 'Pixeliser,Flouter,Bloc noir' &&
        spot === 'Rectangle,Ovale' &&
        num === 'Fin,Moyen,Épais,Opacité',
      '9 lignes par outil (Surligneur, Texte, Bloc noir, Projecteur, Numéro)',
      JSON.stringify({ surl, texte, tailles, tt, flou, bloc, spot, num }),
    );
    await fin(p, '9');
  }

  // ---------- 10. couleurs ----------
  {
    const p = await page();
    await bouton(p, 'Vert').click();
    await uneImage(p);
    const r = await p.evaluate(() => ({
      c: __ed.prefs.couleur,
      presse: [...document.querySelectorAll('#couleurs .dot[aria-pressed="true"]')].map((b) =>
        b.getAttribute('aria-label'),
      ),
      inst: document.querySelector('[data-outil="fleche"]').style.getPropertyValue('--c'),
    }));
    verifier(
      r.c === '#30A46C' && JSON.stringify(r.presse) === '["Vert"]' && r.inst === '#30A46C',
      '10 Vert : couleur de l’outil, pastille et instrument',
      JSON.stringify(r),
    );
    await glisser(p, [20, 20], [120, 40]);
    await cliquer(p, 70, 30);
    await bouton(p, 'Bleu').click();
    await uneImage(p);
    const o = (await objets(p, 'fleche'))[0];
    verifier(o.couleur === '#0090FF', '10 objet choisi : sa couleur change', JSON.stringify(o));
    await p.$eval('#autre-couleur', (i) => {
      i.value = '#8e4ec6';
      i.dispatchEvent(new Event('input', { bubbles: true }));
      i.dispatchEvent(new Event('change', { bubbles: true }));
    });
    await uneImage(p);
    const r2 = await p.evaluate(() => {
      const w = document.querySelector('#couleurs .wheel');
      return {
        c: __ed.h.doc.objets[0].couleur,
        autre: __ed.prefs.autre,
        has: w.classList.contains('has'),
        presse: w.getAttribute('aria-pressed'),
      };
    });
    verifier(
      JSON.stringify(r2) === '{"c":"#8E4EC6","autre":"#8E4EC6","has":true,"presse":"true"}',
      '10 Autre couleur : couleur libre gardée, roue choisie',
      JSON.stringify(r2),
    );
    await p.keyboard.press('Escape');
    await outil(p, 'Gomme');
    const dis = await p.$$eval('#couleurs .dot', (bs) => bs.every((b) => b.disabled));
    await outil(p, 'Surligneur');
    const surl = await p.$eval('#couleurs .dot[aria-pressed="true"]', (b) =>
      b.getAttribute('aria-label'),
    );
    verifier(dis && surl === 'Jaune', '10 Gomme : pastilles grisées ; Surligneur : Jaune');
    await fin(p, '10');
  }

  // ---------- 11. menu « Fermer » et ses trois issues ----------
  {
    const p = await page();
    await glisser(p, [50, 50], [150, 150]);
    await vider(p);
    await bouton(p, 'Fermer').click();
    await attendreAppel(p, 'editeur_fermer');
    await uneImage(p);
    const f = (await appels(p, 'editeur_fermer')).pop();
    const prefs = JSON.parse(f[1].preferences);
    const etat = (await appels(p)).map((a) => a[0]);
    verifier(
      f[1].id === 7 &&
        f[1].force === false &&
        f[1].vide === false &&
        prefs.outil === 'fleche' &&
        etat.indexOf('editeur_etat') >= 0 &&
        etat.indexOf('editeur_etat') < etat.indexOf('editeur_fermer'),
      '11 Fermer : editeur_etat puis editeur_fermer (force faux, préférences)',
      JSON.stringify([f, etat]),
    );
    const m = await p.evaluate(() => {
      const menu = document.querySelector('#menu-fermer');
      const b = document.querySelector('[data-act="fermer"]').getBoundingClientRect();
      const r = menu.getBoundingClientRect();
      return {
        ouvert: menu.classList.contains('ouvert'),
        focus: document.activeElement.textContent.trim(),
        sous: r.top > b.bottom && Math.abs(r.left - b.left) < 1,
      };
    });
    const items = await p
      .getByRole('menu', { name: 'Fermer', exact: true })
      .getByRole('menuitem')
      .allTextContents();
    verifier(
      m.ouvert &&
        m.sous &&
        m.focus === 'Copier et fermer' &&
        JSON.stringify(items.map((t) => t.trim())) === '["Copier et fermer","Fermer sans copier"]',
      '11 « demander » : menu « Fermer » sous le bouton, focus sur Copier et fermer',
      JSON.stringify([m, items]),
    );
    await p.keyboard.press('Tab');
    const f1 = await p.evaluate(() => document.activeElement.textContent.trim());
    await p.keyboard.press('a');
    await p.keyboard.press('h');
    verifier(
      f1 === 'Fermer sans copier' && JSON.stringify(await choisi(p)) === '["Flèche"]',
      '11 Tab : Fermer sans copier ; aucune touche d’outil sous le menu',
      f1,
    );
    await p.keyboard.press('Escape');
    await uneImage(p);
    verifier(
      !(await ouvert(p, '#menu-fermer')) && (await appels(p, 'editeur_garder')).length === 1,
      '11 Échap : menu refermé = editeur_garder',
    );
    await p.evaluate(() => __emettre('editeur:fermer_demande', { id: 7, raison: 'cadre' }));
    await attendreAppel(p, 'editeur_fermer');
    await uneImage(p);
    verifier(
      (await appels(p, 'editeur_fermer')).length === 2 && (await ouvert(p, '#menu-fermer')),
      '11 fermer_demande : le menu « Fermer »',
    );
    await p.mouse.click(600, 400);
    await uneImage(p);
    verifier(
      !(await ouvert(p, '#menu-fermer')) &&
        (await appels(p, 'editeur_garder')).length === 2 &&
        (await objets(p)).length === 1,
      '11 clic ailleurs : refermé = editeur_garder, rien de tracé',
    );
    await bouton(p, 'Fermer').click();
    await attendreAppel(p, 'editeur_fermer');
    await uneImage(p);
    await p.getByRole('menuitem', { name: 'Fermer sans copier' }).click();
    await p.waitForFunction(() => __ed.fond === null, null, { timeout: 3000 }).catch(() => {});
    const g = (await appels(p, 'editeur_fermer')).pop();
    verifier(
      g[1].force === true && (await p.evaluate(() => __ed.fond === null)),
      '11 Fermer sans copier : force, toiles relâchées',
      JSON.stringify(g),
    );
    await fin(p, '11');
  }
  {
    const p = await page({ reponseFermer: 'fermee' });
    await p.keyboard.press('Escape');
    await attendreAppel(p, 'editeur_fermer');
    await uneImage(p);
    const f = (await appels(p, 'editeur_fermer')).pop();
    verifier(
      f && f[1].vide === true && f[1].force === false && !(await ouvert(p, '#menu-fermer')),
      '11 document vide, Échap : fermée sans menu',
    );
    await fin(p, '11 vide');
  }
  // Rapport de A : réponse à fermer_demande tout de suite (< 2 s), texte en saisie validé ;
  // « fermee » suivie d'un editeur:charger (capture en attente) : la page se recharge.
  {
    const p = await page({ reponseFermer: 'fermee' });
    await ajouter(p, 'Texte');
    await cliquer(p, 50, 50);
    await p.keyboard.type('Saisi');
    await vider(p);
    const t0 = Date.now();
    await p.evaluate(() => {
      __TAURI__.core.invoke = ((f) => (c, a, o) => {
        if (c === 'editeur_fermer') window.__texte = __ed.h.doc.objets.map((x) => x.texte);
        const r = f(c, a, o);
        if (c === 'editeur_fermer') {
          window.__charge = { ...window.__charge, id: 9 };
          __emettre('editeur:charger', window.__charge);
        }
        return r;
      })(__TAURI__.core.invoke);
      __emettre('editeur:fermer_demande', { id: 7, raison: 'remplacer' });
    });
    await p
      .waitForFunction(
        () => __appels.some((a) => a[0] === 'editeur_affiche' && a[1].id === 9),
        null,
        {
          timeout: 2000,
        },
      )
      .catch(() => {});
    const r = await p.evaluate(() => ({
      f: __appels.filter((a) => a[0] === 'editeur_fermer').map((a) => [a[1].id, a[1].vide]),
      e: __appels.filter((a) => a[0] === 'editeur_etat').map((a) => a[1].vide),
      t: window.__texte,
      img: __appels.filter((a) => a[0] === 'editeur_image').map((a) => a[1].id),
      fond: !!__ed.fond,
      n: __ed.h.doc.objets.length,
    }));
    verifier(
      Date.now() - t0 < 2000 &&
        JSON.stringify(r) ===
          '{"f":[[7,false]],"e":[false],"t":["Saisi"],"img":[9],"fond":true,"n":0}',
      '11 fermer_demande : texte validé, état, editeur_fermer aussitôt ; puis nouveau document',
      JSON.stringify(r),
    );
    await outil(p, 'Flèche');
    await vider(p);
    await glisser(p, [60, 60], [160, 160]);
    await p
      .waitForFunction(() => __appels.some((a) => a[0] === 'editeur_etat'), null, { timeout: 400 })
      .catch(() => {});
    const et = (await appels(p, 'editeur_etat')).pop();
    verifier(et && et[1].id === 9 && et[1].vide === false, '11 editeur_etat sous 250 ms');
    await fin(p, '11 remplacer');
  }

  // ---------- 12. « Copier » : copier puis fermer ; Ctrl+C sans fermer ----------
  {
    const p = await page();
    await glisser(p, [50, 50], [150, 150]);
    await vider(p);
    await bouton(p, 'Copier').click();
    await p.waitForFunction(() => window.__exports.length === 1);
    const but = await p.evaluate(() => window.__exports[0].entetes['x-editeur-but']);
    await p.evaluate(() =>
      __emettre('editeur:resultat', {
        id: 7,
        numero: 1,
        but: 'copier_fermer',
        etat: 'echec',
        message: 'Impossible de copier',
      }),
    );
    await uneImage(p);
    const m = await p.$eval('#message', (x) => [
      x.getAttribute('role'),
      x.textContent.trim(),
      x.classList.contains('on'),
    ]);
    verifier(
      but === 'copier_fermer' &&
        JSON.stringify(m) === '["status","Impossible de copier",true]' &&
        (await appels(p, 'editeur_fermer')).length === 0 &&
        !(await p.$eval('[data-act="copier"]', (b) => b.classList.contains('done'))),
      '12 Copier en échec : la fenêtre reste, « Impossible de copier »',
      JSON.stringify([but, m]),
    );
    await bouton(p, 'Copier').click();
    await p.waitForFunction(() => window.__exports.length === 2);
    await p.evaluate(() =>
      __emettre('editeur:resultat', { id: 7, numero: 2, but: 'copier_fermer', etat: 'fait' }),
    );
    await uneImage(p);
    const done = await p.$eval('[data-act="copier"]', (b) => b.classList.contains('done'));
    const fermeTot = (await appels(p, 'editeur_fermer')).length;
    await attendreAppel(p, 'editeur_fermer');
    const f = (await appels(p, 'editeur_fermer')).pop();
    verifier(
      done && fermeTot === 0 && f && f[1].force === false,
      '12 Copier réussi : coche (.done) puis editeur_fermer',
      JSON.stringify([done, fermeTot, f]),
    );
    await fin(p, '12');
  }
  {
    const p = await page();
    await glisser(p, [50, 50], [150, 150]);
    await p.keyboard.press('Control+c');
    await p.waitForFunction(() => window.__exports.length === 1);
    await p.evaluate(() =>
      __emettre('editeur:resultat', { id: 7, numero: 1, but: 'copier', etat: 'fait' }),
    );
    await uneImage(p);
    const r = await p.evaluate(() => [
      window.__exports[0].entetes['x-editeur-but'],
      document.querySelector('#message').textContent.trim(),
      __appels.some((a) => a[0] === 'editeur_fermer'),
    ]);
    verifier(
      JSON.stringify(r) === '["copier","Copié",false]',
      '12 Ctrl+C : copie sans fermer, « Copié »',
      JSON.stringify(r),
    );
    await bouton(p, 'Enregistrer').click();
    await p.waitForFunction(() => window.__exports.length === 2);
    await p.evaluate(() =>
      __emettre('editeur:resultat', { id: 7, numero: 2, but: 'enregistrer', etat: 'deja_fait' }),
    );
    await uneImage(p);
    await bouton(p, 'Plus').click();
    await p.getByRole('menuitem', { name: 'Enregistrer sous' }).click();
    await p.waitForFunction(() => window.__exports.length === 3);
    const r2 = await p.evaluate(() => [
      document.querySelector('#message').textContent.trim(),
      window.__exports.map((e) => e.entetes['x-editeur-but']).join(','),
    ]);
    verifier(
      JSON.stringify(r2) === '["Enregistré","copier,enregistrer,enregistrer_sous"]',
      '12 Enregistrer : « Enregistré », la fenêtre reste ; Enregistrer sous',
      JSON.stringify(r2),
    );
    await fin(p, '12 Ctrl+C');
  }

  // ---------- 13. « Glisser vers une appli » : vignette de prise, 4 px ----------
  {
    const p = await page();
    await bouton(p, 'Plus').click();
    await p.getByRole('menuitem', { name: 'Glisser vers une appli' }).click();
    await uneImage(p);
    const v = p.getByRole('button', { name: 'Glisser vers une appli', exact: true });
    const r = await p.evaluate(() => {
      const g = document.querySelector('#vignette-prise');
      const b = g.getBoundingClientRect();
      const pal = document.querySelector('#palette').getBoundingClientRect();
      return {
        vu: !g.hidden,
        focus: document.activeElement === g,
        dessus: b.bottom < pal.top,
        menu: document.querySelector('#menu-plus').classList.contains('ouvert'),
        draggable: document.querySelectorAll('[draggable="true"]').length,
      };
    });
    verifier(
      (await v.count()) === 1 && r.vu && r.focus && r.dessus && !r.menu && r.draggable === 0,
      '13 vignette de prise au-dessus de la palette, focus, aucun draggable',
      JSON.stringify(r),
    );
    const b = await v.boundingBox();
    const cx = b.x + b.width / 2;
    const cy = b.y + b.height / 2;
    await p.mouse.move(cx, cy);
    await p.mouse.down();
    await p.mouse.move(cx + 3, cy);
    await uneImage(p);
    const n3 = await p.evaluate(() => window.__exports.length);
    await p.mouse.move(cx + 4, cy);
    await p
      .waitForFunction(() => window.__exports.length === 1, null, { timeout: 3000 })
      .catch(() => {});
    await p.mouse.up();
    const r2 = await p.evaluate(() => [
      window.__exports.map((e) => e.entetes['x-editeur-but']).join(','),
      document.querySelector('#vignette-prise').hidden,
    ]);
    verifier(
      n3 === 0 && JSON.stringify(r2) === '["glisser",true]',
      '13 3 px : rien ; 4 px : export « glisser », la vignette disparaît',
      JSON.stringify([n3, r2]),
    );
    await p.evaluate(() =>
      __emettre('editeur:resultat', { id: 7, numero: 1, but: 'glisser', etat: 'fait' }),
    );
    await bouton(p, 'Plus').click();
    await p.getByRole('menuitem', { name: 'Glisser vers une appli' }).click();
    await p.keyboard.press('Escape');
    const h1 = await p.$eval('#vignette-prise', (g) => g.hidden);
    await bouton(p, 'Plus').click();
    await p.getByRole('menuitem', { name: 'Glisser vers une appli' }).click();
    await p.mouse.click(600, 300);
    const h2 = await p.$eval('#vignette-prise', (g) => g.hidden);
    verifier(h1 && h2, '13 Échap ou un clic ailleurs : la vignette part');
    await fin(p, '13');
  }

  // ---------- 14. devicePixelRatio : un pixel de l'image par pixel de l'écran ----------
  for (const echelle of [1, 1.25, 1.5, 2]) {
    const p = await page({ echelle, charge: { largeur: 301, hauteur: 203 }, graine: 5 });
    await p.keyboard.press('Control+1');
    await uneImage(p);
    const r = await p.evaluate(() => {
      const ed = window.__ed;
      const t = ed.toile;
      const v = ed.vue;
      const d = ed.ctx.getImageData(v.x, v.y, 301, 203).data;
      const o = window.__image(window.__charge, 5);
      // Le bord (coins arrondis de rayon 5, ombre des équerres) est hors de la preuve.
      const R = Math.ceil(8 * v.dpr);
      let ecarts = 0;
      for (let y = 0; y < 203; y++)
        for (let x = 0; x < 301; x++) {
          if (x < R || x >= 301 - R || y < R || y >= 203 - R) continue;
          const i = (y * 301 + x) * 4;
          for (let k = 0; k < 4; k++) if (d[i + k] !== o[i + k]) ecarts++;
        }
      const coin = d[3];
      return {
        l: t.width,
        h: t.height,
        cl: t.clientWidth,
        ch: t.clientHeight,
        dpr: devicePixelRatio,
        v,
        ecarts,
        coin,
        z: document.querySelector('#etiquette-zoom').textContent,
      };
    });
    verifier(
      r.l === Math.round(r.cl * r.dpr) &&
        r.h === Math.round(r.ch * r.dpr) &&
        r.v.echelle === 1 &&
        Number.isInteger(r.v.x) &&
        Number.isInteger(r.v.y) &&
        r.ecarts === 0 &&
        r.coin < 255 &&
        r.z === '100 %',
      `14 dpr ${echelle} : Taille réelle pixel pour pixel, coins arrondis`,
      JSON.stringify(r),
    );
    await attendreAppel(p, 'editeur_vue', { zoom: 1 });
    verifier(
      (await appels(p, 'editeur_vue')).some((a) => a[1].zoom === 1),
      `14 dpr ${echelle} : editeur_vue zoom 1`,
    );
    await p.keyboard.press('Control+0');
    const aj = await p.evaluate(() => __ed.vue.echelle);
    verifier(aj <= 1, `14 dpr ${echelle} : Ajuster ≤ 100 %`);
    await fin(p, `14 dpr ${echelle}`);
  }

  // ---------- 15. vue : réserves de la maquette, ombre, équerres ----------
  {
    const p = await page({ charge: { largeur: 2400, hauteur: 1600 } });
    const r = await p.evaluate(() => {
      const s = document.querySelector('#scene').getBoundingClientRect();
      const o = document.querySelector('#ombre-image');
      const b = o.getBoundingClientRect();
      const cs = getComputedStyle(o);
      const pal = document.querySelector('#palette').offsetHeight;
      return {
        haut: b.top - s.top,
        bas: s.bottom - b.bottom,
        cote: Math.min(b.left - s.left, s.right - b.right),
        pal,
        ombre: cs.boxShadow,
        rayon: cs.borderRadius,
      };
    });
    verifier(
      (Math.abs(r.haut - 14) <= 1 || Math.abs(r.cote - 36) <= 1) &&
        r.bas >= r.pal + 34 - 1 &&
        r.ombre === 'rgba(0, 0, 0, 0.18) 0px 4px 18px 0px' &&
        r.rayon === '5px',
      '15 image ajustée dans les réserves (14, palette + 34, 36), ombre, rayon 5',
      JSON.stringify(r),
    );
    await fin(p, '15');
  }

  // ---------- 16. clair et sombre ----------
  for (const theme of ['light', 'dark']) {
    const p = await page({ theme });
    const r = await p.evaluate(() => {
      const cs = (s) => getComputedStyle(document.querySelector(s));
      const pastille = document.querySelector('[data-outil="fleche"] .inst .b');
      return {
        barre: cs('#barre-haut').backgroundColor,
        scene: cs('#fenetre').backgroundColor,
        copier: cs('[data-act="copier"]').backgroundColor,
        corps: getComputedStyle(pastille).fill,
        equerre: getComputedStyle(document.documentElement)
          .getPropertyValue('--editeur-equerre')
          .trim(),
      };
    });
    const attendu =
      theme === 'light'
        ? [
            'rgb(238, 240, 243)',
            'rgb(230, 232, 236)',
            'rgb(0, 103, 192)',
            'rgb(194, 199, 207)',
            '#1A1A1A',
          ]
        : ['rgb(35, 35, 35)', 'rgb(23, 23, 23)', 'rgb(76, 194, 255)', 'url("#argent")', '#FFFFFF'];
    verifier(
      JSON.stringify([r.barre, r.scene, r.copier, r.corps, r.equerre]) === JSON.stringify(attendu),
      `16 ${theme} : mica, scène, Copier, instruments, équerres`,
      JSON.stringify(r),
    );
    // L'équerre haut gauche est peinte dans la couleur du thème.
    const px = await p.evaluate(() => {
      const v = __ed.vue;
      const d = __ed.ctx.getImageData(v.x - 4, v.y + 10, 1, 1).data;
      return [d[0], d[1], d[2], d[3]];
    });
    const clair = theme === 'light';
    verifier(
      px[3] > 200 && (clair ? px[0] < 60 : px[0] > 200),
      `16 ${theme} : équerre peinte`,
      JSON.stringify(px),
    );
    await p.screenshot({ path: path.join(SORTIE, `editeur-${theme}.png`) });
    await fin(p, `16 ${theme}`);
  }

  // ---------- 17. fenêtre étroite : pastilles repliées dans « Couleur » ----------
  {
    const p = await page({ largeur: 600, hauteur: 440 });
    const r = await p.evaluate(() => ({
      cols: getComputedStyle(document.querySelector('#couleurs')).display,
      cur: getComputedStyle(document.querySelector('#bouton-couleur')).display,
      pal: document.querySelector('#palette').getBoundingClientRect().height,
      large: document.querySelector('#palette').getBoundingClientRect().width <= innerWidth,
    }));
    verifier(
      r.cols === 'none' && r.cur !== 'none' && r.pal === 68 && r.large,
      '17 600 × 440 : pastilles repliées, « Couleur » montré, palette 68 px',
      JSON.stringify(r),
    );
    await bouton(p, 'Couleur').click();
    const dlg = p.getByRole('dialog', { name: 'Couleur', exact: true });
    const noms = await dlg
      .getByRole('button')
      .evaluateAll((bs) => bs.map((b) => b.getAttribute('aria-label')));
    await dlg.getByRole('button', { name: 'Noir' }).click();
    await uneImage(p);
    verifier(
      JSON.stringify(noms) === '["Rouge","Jaune","Vert","Autre couleur","Bleu","Noir","Blanc"]' &&
        (await p.evaluate(() => __ed.prefs.couleur)) === '#1A1A1A',
      '17 « Couleur » : les pastilles, Noir choisi',
      JSON.stringify(noms),
    );
    await p.screenshot({ path: path.join(SORTIE, 'editeur-etroit.png') });
    await fin(p, '17');
  }

  // ---------- 18. messages et rechargement (horloge de Playwright) ----------
  {
    const p = await page({ horloge: true });
    const ctx = p.context();
    await ctx.clock.pauseAt(Date.now() + 1000);
    await ctx.clock.runFor(200);
    await p.keyboard.press('Control+c');
    await ctx.clock.runFor(50);
    const avant = await p.$eval('#message', (m) => m.classList.contains('on'));
    await p.evaluate(() =>
      __emettre('editeur:resultat', { id: 7, numero: 1, but: 'copier', etat: 'fait' }),
    );
    await ctx.clock.runFor(1599);
    const pendant = await p.$eval('#message', (m) => m.classList.contains('on'));
    await ctx.clock.runFor(2);
    const apres = await p.$eval('#message', (m) => m.classList.contains('on'));
    verifier(!avant && pendant && !apres, '18 « Copié » au résultat, 1 600 ms');
    await p.keyboard.press('Control+s');
    await ctx.clock.runFor(50);
    await p.evaluate(() =>
      __emettre('editeur:resultat', {
        id: 7,
        numero: 2,
        but: 'enregistrer',
        etat: 'echec',
        message: 'Impossible d’enregistrer',
      }),
    );
    await ctx.clock.runFor(3999);
    const m1 = await p.$eval('#message', (m) => [m.textContent.trim(), m.classList.contains('on')]);
    await ctx.clock.runFor(2);
    const m2 = await p.$eval('#message', (m) => m.classList.contains('on'));
    verifier(
      JSON.stringify(m1) === '["Impossible d’enregistrer",true]' && !m2,
      '18 « Impossible d’enregistrer » 4 s',
      JSON.stringify([m1, m2]),
    );
    await p.keyboard.press('r');
    const [a, b] = await point(p, 50, 50);
    await p.mouse.move(a, b);
    await p.mouse.down();
    await p.mouse.move(a + 80, b + 80);
    await p.mouse.up();
    await ctx.clock.runFor(50);
    await p.evaluate(() => {
      window.__charge = { ...window.__charge, id: 8, largeur: 200, hauteur: 100 };
      __emettre('editeur:charger', window.__charge);
    });
    await ctx.clock.runFor(200);
    const r = await p.evaluate(() => ({
      n: __ed.h.doc.objets.length,
      v: __ed.h.version,
      l: __ed.h.doc.largeur,
      aff: __appels.filter((x) => x[0] === 'editeur_affiche').map((x) => x[1].id),
      outil: __ed.prefs.outil,
    }));
    verifier(
      JSON.stringify(r) === '{"n":0,"v":0,"l":200,"aff":[7,8],"outil":"forme"}',
      '18 editeur:charger : document vidé, affiche par document, outil gardé',
      JSON.stringify(r),
    );
    await fin(p, '18');
  }

  // Image de longueur fausse : la page la refuse (editeur_fermer avec force).
  {
    const p = await page();
    await p.evaluate(() => {
      window.__image = () => new Uint8Array(12);
      window.__charge = { ...window.__charge, id: 11 };
      __emettre('editeur:charger', window.__charge);
    });
    await attendreAppel(p, 'editeur_fermer', { id: 11 });
    const r = await p.evaluate(() => ({
      f: __appels.filter((a) => a[0] === 'editeur_fermer').map((a) => [a[1].id, a[1].force]),
      aff: __appels.filter((a) => a[0] === 'editeur_affiche').map((a) => a[1].id),
    }));
    verifier(
      JSON.stringify(r) === '{"f":[[11,true]],"aff":[7]}',
      '19 image refusée : editeur_fermer force, sans editeur_affiche',
      JSON.stringify(r),
    );
    await fin(p, '19');
  }

  await nav.close();
  srv.close();
  console.log(`OK : ${reussis}`);
  console.log(`ECHECS : ${echecs}`);
  process.exit(echecs ? 1 : 0);
})();
