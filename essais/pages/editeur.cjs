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
function simulerCoeur({ charge, graine, alpha, reponseFermer, pretDiffere, imageEchec }) {
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
        if (c === 'editeur_pret') {
          if (!pretDiffere) return Promise.resolve(window.__charge);
          return new Promise((ok) => (window.__lacherPret = () => ok(window.__charge)));
        }
        if (c === 'editeur_image' && imageEchec) return Promise.reject(new Error('image'));
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
      pretDiffere: !!o.pretDiffere,
      imageEchec: !!o.imageEchec,
    });
    await p.goto(URL_PAGE);
    if (o.horloge || o.pretDiffere || o.imageEchec) return p;
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
  // Les animations et transitions finies (menus, bulles, boutons enfoncés) avant une mesure.
  const calme = (p) =>
    p
      .waitForFunction(
        () => document.getAnimations().every((a) => a.playState !== 'running'),
        null,
        { timeout: 3000 },
      )
      .catch(() => {});
  // Aucun appel `nom` d'id `id` dans la seconde : attente bornée, jamais une réussite gratuite
  // (la vérification qui suit lit les appels).
  const sansAppel = (p, nom, egal) =>
    p
      .waitForFunction(
        ([n, e]) =>
          __appels.some(
            (a) => a[0] === n && Object.entries(e).every(([k, v]) => a[1] && a[1][k] === v),
          ),
        [nom, egal],
        { timeout: 1000 },
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
    // Le rectangle envoyé est en pixels de la page : l'image est sous la barre du haut
    // (52 px) et sa réserve (14 px). Témoin : l'ombre de l'image, que la page place par un
    // autre chemin (une boîte CSS sous l'image).
    const place = await p.evaluate(() => {
      const o = document.querySelector('#ombre-image').getBoundingClientRect();
      return { x: o.left, y: o.top, largeur: o.width, hauteur: o.height };
    });
    const envoye = vue.length === 1 ? vue[0][1] : { x: NaN, y: NaN, largeur: NaN, hauteur: NaN };
    verifier(
      envoye.y >= 66 &&
        Math.abs(envoye.x - place.x) < 0.05 &&
        Math.abs(envoye.y - place.y) < 0.05 &&
        Math.abs(envoye.largeur - place.largeur) < 0.05 &&
        Math.abs(envoye.hauteur - place.hauteur) < 0.05,
      'editeur_vue : la place de l’image dans la page (celle de son ombre), pas dans la toile',
      JSON.stringify([envoye, place]),
    );
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
        o.id === 1 &&
        (await p.evaluate(() => __ed.selection.length)) === 0,
      '4 Flèche : flèche droite rouge (vrai modèle), non choisie après le tracé',
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
    const tt = await p.evaluate(() => __ed.prefs.tailles.texte);
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
        tt === 2 &&
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
      c: __ed.prefs.couleurs.fleche,
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
    await calme(p);
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
    await sansAppel(p, 'editeur_fermer', {});
    const f = await appels(p, 'editeur_fermer');
    verifier(
      done && f.length === 0,
      '12 Copier réussi : la coche, aucun editeur_fermer (le cœur ferme)',
      JSON.stringify([done, f]),
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

  // B1 : capture en attente (« remplacer ») : après « Copier et fermer » réussi, le cœur
  // charge la capture 8 ; la page ne ferme jamais la nouvelle capture.
  {
    const p = await page();
    await glisser(p, [50, 50], [150, 150]);
    await p.evaluate(() => __emettre('editeur:fermer_demande', { id: 7, raison: 'remplacer' }));
    await attendreAppel(p, 'editeur_fermer', { id: 7 });
    await uneImage(p);
    await p.getByRole('menuitem', { name: 'Copier et fermer' }).click();
    await p.waitForFunction(() => window.__exports.length === 1);
    await p.evaluate(() => {
      __emettre('editeur:resultat', { id: 7, numero: 1, but: 'copier_fermer', etat: 'fait' });
      window.__charge = { ...window.__charge, id: 8 };
      __emettre('editeur:charger', window.__charge);
    });
    await attendreAppel(p, 'editeur_affiche', { id: 8 });
    await sansAppel(p, 'editeur_fermer', { id: 8 });
    const r = await p.evaluate(() => ({
      f: __appels.filter((a) => a[0] === 'editeur_fermer').map((a) => a[1].id),
      aff: __appels.filter((a) => a[0] === 'editeur_affiche').map((a) => a[1].id),
    }));
    verifier(
      JSON.stringify(r) === '{"f":[7],"aff":[7,8]}',
      '12 remplacer, Copier et fermer réussi, charger 8 : la capture 8 affichée, jamais fermée',
      JSON.stringify(r),
    );
    await fin(p, '12 remplacer');
  }
  // B2 : un editeur:charger pendant un export lève le verrou.
  {
    const p = await page();
    await p.keyboard.press('Control+s');
    await p.waitForFunction(() => window.__exports.length === 1);
    await p.evaluate(() => {
      window.__charge = { ...window.__charge, id: 8 };
      __emettre('editeur:charger', window.__charge);
    });
    await attendreAppel(p, 'editeur_affiche', { id: 8 });
    await uneImage(p);
    await p.keyboard.press('Control+c');
    await p
      .waitForFunction(() => window.__exports.length === 2, null, { timeout: 3000 })
      .catch(() => {});
    const r = await p.evaluate(() =>
      window.__exports.map((e) => e.entetes['x-editeur-id'] + ':' + e.entetes['x-editeur-but']),
    );
    verifier(
      JSON.stringify(r) === '["7:enregistrer","8:copier"]',
      '12 charger pendant un export : le verrou tombe, Ctrl+C part pour la nouvelle capture',
      JSON.stringify(r),
    );
    await fin(p, '12 verrou');
  }
  // fermer_demande reçue avant la charge : une seule réponse, dès que la charge est là.
  {
    const p = await page({ pretDiffere: true, reponseFermer: 'fermee' });
    await p.waitForFunction(() => !!window.__lacherPret && !!__ecouteurs['editeur:fermer_demande']);
    await p.evaluate(() => {
      __emettre('editeur:fermer_demande', { id: 7, raison: 'bouton' });
      window.__lacherPret();
    });
    await attendreAppel(p, 'editeur_fermer', { id: 7 });
    await sansAppel(p, 'editeur_affiche', {});
    const f = (await appels(p, 'editeur_fermer')).map((a) => [a[1].id, a[1].vide, a[1].force]);
    verifier(
      JSON.stringify(f) === '[[7,true,false]]',
      '12 fermer_demande avant la charge : une réponse, dès la charge',
      JSON.stringify(f),
    );
    await fin(p, '12 demande tôt');
  }
  // editeur_image en échec : editeur_fermer avec force, sans editeur_affiche.
  {
    const p = await page({ imageEchec: true });
    await attendreAppel(p, 'editeur_fermer', { force: true });
    const r = await p.evaluate(() => ({
      f: __appels.filter((a) => a[0] === 'editeur_fermer').map((a) => [a[1].id, a[1].force]),
      aff: __appels.filter((a) => a[0] === 'editeur_affiche').length,
    }));
    verifier(
      JSON.stringify(r) === '{"f":[[7,true]],"aff":0}',
      '12 editeur_image en échec : editeur_fermer force, sans editeur_affiche',
      JSON.stringify(r),
    );
    p.erreurs = p.erreurs.filter((e) => !e.includes('editeur_image'));
    await fin(p, '12 image en échec');
  }

  // ---------- 13. « Glisser vers une appli » : vignette de prise, 4 px ----------
  {
    const p = await page();
    await bouton(p, 'Plus').click();
    await p.getByRole('menuitem', { name: 'Glisser vers une appli' }).click();
    await uneImage(p);
    await calme(p);
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
        (await p.evaluate(() => __ed.prefs.couleurs.fleche)) === '#1A1A1A',
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

  // ---------- 20. reprises de la relecture ----------
  {
    // Préférences par outil : chacun garde sa couleur, sa taille, son opacité.
    const p = await page();
    await bouton(p, 'Vert').click();
    await outil(p, 'Flèche');
    await p.locator('#reglages-outil').getByRole('button', { name: 'Épais' }).click();
    await p.keyboard.press('Escape');
    await outil(p, 'Stylo');
    const r = await p.evaluate(() => ({
      presse: document
        .querySelector('#couleurs .dot[aria-pressed="true"]')
        .getAttribute('aria-label'),
      p: [__ed.prefs.couleurs, __ed.prefs.tailles.fleche, __ed.prefs.tailles.stylo],
    }));
    await glisser(p, [20, 20], [120, 60]);
    const c = (await objets(p, 'crayon'))[0];
    verifier(
      r.presse === 'Rouge' &&
        r.p[0].fleche === '#30A46C' &&
        r.p[0].surligneur === '#FFD43B' &&
        r.p[1] === 2 &&
        r.p[2] === 1 &&
        c.couleur === '#E5484D' &&
        c.epaisseur === 1,
      '20 préférences par outil (Flèche verte et épaisse, Stylo rouge et moyen)',
      JSON.stringify([r, c]),
    );
    // Clic de souris sur un instrument : le focus revient à la scène, Espace déplace la vue.
    await outil(p, 'Surligneur');
    const foc = await p.evaluate(() => document.activeElement.id);
    const x0 = await p.evaluate(() => __ed.vue.x);
    await p.keyboard.down(' ');
    await p.mouse.move(600, 300);
    await p.mouse.down();
    await p.mouse.move(650, 320);
    await p.mouse.move(700, 340);
    await p.mouse.up();
    await p.keyboard.up(' ');
    await uneImage(p);
    const r2 = await p.evaluate(() => [__ed.vue.x, __ed.h.doc.objets.length]);
    verifier(
      foc === 'scene' && r2[0] === x0 + 100 && r2[1] === 1,
      '20 clic sur un instrument : focus à la scène ; Espace tenu déplace la vue',
      JSON.stringify([foc, x0, r2]),
    );
    await fin(p, '20 préférences');
  }
  {
    // La poignée d'un objet choisi passe avant l'équerre de recadrage.
    const p = await page({ reduit: true });
    await ajouter(p, 'Forme');
    await glisser(p, [100, 80], [1, 1]);
    await cliquer(p, 50, 1);
    const choisi1 = await p.evaluate(() => __ed.selection.slice());
    const [x, y] = await point(p, 1, 1);
    await glisserCss(p, x, y, x + 30, y + 30);
    const r = await p.evaluate(() => [__ed.h.doc.recadrage, Math.round(__ed.h.doc.objets[0]?.x1)]);
    verifier(
      JSON.stringify(choisi1) === '[1]' && r[0] === null && r[1] > 20,
      '20 poignée au coin de l’image : l’objet se redimensionne, l’image n’est pas recadrée',
      JSON.stringify([choisi1, r]),
    );
    await fin(p, '20 poignée');
  }
  {
    // Gomme : le bord d'un projecteur l'efface, le milieu non.
    const p = await page();
    await ajouter(p, 'Projecteur');
    await glisser(p, [100, 100], [300, 250]);
    await outil(p, 'Gomme');
    await cliquer(p, 200, 175);
    const n1 = (await objets(p, 'projecteur')).length;
    await cliquer(p, 100, 175);
    const n2 = (await objets(p, 'projecteur')).length;
    verifier(n1 === 1 && n2 === 0, '20 Gomme : le bord du projecteur l’efface, le milieu non');
    await fin(p, '20 gomme');
  }

  // ---------- 21. Copier le texte (contrat 0.5, §2.4, §5.6, §12.2) ----------
  // Le cœur simulé rend les boîtes de 7 mots sur 3 lignes, en pixels de l'image composée ;
  // jamais un texte. Les points sont pris à la place des mots, calculée depuis `__ed.vue`.
  const MOTS = [
    { x: 40, y: 40, largeur: 60, hauteur: 20, ligne: 0 },
    { x: 110, y: 40, largeur: 50, hauteur: 20, ligne: 0 },
    { x: 170, y: 40, largeur: 80, hauteur: 20, ligne: 0 },
    { x: 40, y: 80, largeur: 70, hauteur: 20, ligne: 1 },
    { x: 120, y: 80, largeur: 60, hauteur: 20, ligne: 1 },
    { x: 40, y: 120, largeur: 50, hauteur: 20, ligne: 2 },
    { x: 100, y: 120, largeur: 90, hauteur: 20, ligne: 2 },
  ];
  const centreMot = (i) => [MOTS[i].x + MOTS[i].largeur / 2, MOTS[i].y + MOTS[i].hauteur / 2];
  const calque = (p) =>
    p.evaluate(() => {
      const c = document.querySelector('.calque-texte');
      return c && { choix: c.classList.contains('choix'), nom: c.getAttribute('aria-label') };
    });
  // « Texte », l'export de but texte, puis `editeur:texte` ; attend le calque de choix (ou
  // sa fin). Rend le numéro de l'export.
  const lireTexte = async (p, o = {}) => {
    const avant = await p.evaluate(() => __exports.length);
    await bouton(p, 'Texte').click();
    await p.waitForFunction((n) => __exports.length > n, avant, { timeout: 3000 }).catch(() => {});
    const numero = await p.evaluate(() => __exports.at(-1).numero);
    await p.evaluate(
      ([numero, o, mots]) =>
        __emettre('editeur:texte', { id: 7, numero, etat: o.etat || 'lu', mots: o.mots || mots }),
      [numero, o, MOTS],
    );
    if (o.attendre !== false)
      await p
        .waitForFunction(() => document.querySelector('.calque-texte.choix'), null, {
          timeout: 3000,
        })
        .catch(() => {});
    await uneImage(p);
    return numero;
  };
  const copies = (p) =>
    p.evaluate(() => __appels.filter((a) => a[0] === 'editeur_copier_texte').map((a) => a[1]));
  // Boîtes affichées (pixels CSS de la page) et boîtes attendues depuis la vue.
  const boites = (p) =>
    p.$$eval('.calque-texte .mot', (bs) =>
      bs.map((b) => {
        const r = b.getBoundingClientRect();
        return [r.left, r.top, r.width, r.height];
      }),
    );
  const attendues = async (p) => {
    const r = [];
    for (const m of MOTS) {
      const [x0, y0] = await point(p, m.x, m.y);
      const [x1, y1] = await point(p, m.x + m.largeur, m.y + m.hauteur);
      r.push([x0, y0, x1 - x0, y1 - y0]);
    }
    return r;
  };
  const justes = (a, b) =>
    a.length === b.length && a.every((v, i) => v.every((x, j) => Math.abs(x - b[i][j]) < 0.02));
  {
    const p = await page({ charge: { texte: true } });
    const haut = await p.$$eval('#barre-haut > button', (bs) =>
      bs.map((b) => [b.getAttribute('aria-label') || b.textContent.trim(), b.title]),
    );
    verifier(
      JSON.stringify(haut) ===
        '[["Fermer","Fermer"],["Texte","Copier le texte"],["Enregistrer","Enregistrer"],["Copier","Copier"]]' &&
        (await p.getByRole('button', { name: 'Texte', exact: true }).count()) === 1,
      '21 « Texte » (infobulle « Copier le texte ») entre Fermer et Enregistrer',
      JSON.stringify(haut),
    );
    // R7 : un Bloc noir posé ; l'image lue est l'image composée, la vue du document.
    await ajouter(p, 'Flou');
    await variante(p, 'Flou', 'Bloc noir');
    await p.keyboard.press('Escape');
    await glisser(p, [30, 70], [200, 110]);
    await vider(p);
    const t0 = await p.evaluate(() => performance.now());
    await bouton(p, 'Texte').click();
    await p.waitForFunction(() => __exports.length > 0, null, { timeout: 3000 }).catch(() => {});
    const ex = await p.evaluate(() => {
      const e = __exports.at(-1);
      const img = __ed.dessin.composer(__ed.h.doc, __ed.fond);
      const corps = new Uint8Array(window.__dernierCorps);
      let egal = corps.length === img.rgba.length;
      for (let i = 0; egal && i < corps.length; i++) egal = corps[i] === img.rgba[i];
      // Le Bloc noir (de 30,70 à 200,110) : uni et sombre dans l'image lue.
      const px = (x, y) => [
        ...corps.slice((y * img.largeur + x) * 4, (y * img.largeur + x) * 4 + 4),
      ];
      const noir = [px(100, 90), px(150, 100), px(60, 75)].every(
        (v) => v.join() === px(100, 90).join() && v[0] < 30 && v[3] === 255,
      );
      return {
        masques: __ed.h.doc.objets.map((o) => o.type + ':' + (o.sorte || o.variante || '')),
        but: e.entetes['x-editeur-but'],
        egal,
        noir,
        grise: [...document.querySelectorAll('#barre-haut button')].find(
          (b) => b.textContent.trim() === 'Texte',
        ).disabled,
        calque: !!document.querySelector('.calque-texte[role="application"]'),
        numero: e.numero,
      };
    });
    verifier(
      ex.but === 'texte' && ex.egal && ex.noir && ex.grise && ex.calque,
      '21 clic : export de but texte = l’image composée (Bloc noir compris, R7), « Texte » grisé, balayage',
      JSON.stringify(ex),
    );
    const anim = await p.evaluate(() =>
      document
        .querySelector('.calque-texte .balayage')
        .getAnimations({ subtree: true })
        .map((a) => [a.animationName, a.effect.getTiming().duration])
        .sort(),
    );
    await p.evaluate(
      ([n, mots]) => __emettre('editeur:texte', { id: 7, numero: n, etat: 'lu', mots }),
      [ex.numero, MOTS.map((m) => ({ ...m, texte: 'SECRET' }))],
    );
    await p
      .waitForFunction(() => document.querySelector('.calque-texte.choix'), null, { timeout: 3000 })
      .catch(() => {});
    const dt = await p.evaluate((t0) => performance.now() - t0, t0);
    const c1 = await calque(p);
    const etat = await p.evaluate(() => ({
      n: document.querySelectorAll('.calque-texte .mot').length,
      copier: document.querySelector('.barre-texte [data-texte="copier"]').disabled,
      texte: document.querySelector('.calque-texte').innerText.replace(/\s+/g, ' ').trim(),
      secret: document.documentElement.outerHTML.includes('SECRET'),
      focus: document.activeElement === document.querySelector('.calque-texte'),
      grise: [...document.querySelectorAll('#barre-haut button')].find(
        (b) => b.textContent.trim() === 'Texte',
      ).disabled,
      exportee: __ed.exportee,
    }));
    verifier(
      JSON.stringify(anim) === '[["efface",400],["respire",400],["traverse",400]]' && dt >= 390,
      '21 balayage de 400 ms avant le choix',
      JSON.stringify([anim, dt]),
    );
    verifier(
      c1.choix &&
        c1.nom === 'Choisir le texte' &&
        etat.n === 7 &&
        etat.copier &&
        etat.focus &&
        !etat.grise &&
        etat.exportee === null,
      '21 editeur:texte : calque « Choisir le texte », 7 boîtes, « Copier » grisé, export fini sans « exporté »',
      JSON.stringify([c1, etat]),
    );
    verifier(
      etat.texte === 'Copier Tout copier' && !etat.secret,
      '21 la page ne contient jamais de texte reconnu',
      JSON.stringify(etat),
    );
    const tb = p.getByRole('toolbar', { name: 'Texte', exact: true });
    const noms = await tb
      .getByRole('button')
      .evaluateAll((bs) => bs.map((b) => b.getAttribute('aria-label') || b.textContent.trim()));
    verifier(
      (await p.getByRole('application', { name: 'Choisir le texte', exact: true }).count()) === 1 &&
        JSON.stringify(noms) === '["Copier","Tout copier","Fermer"]',
      '21 noms et rôles : calque « Choisir le texte », barre « Texte » : Copier, Tout copier, Fermer',
      JSON.stringify(noms),
    );
    verifier(
      justes(await boites(p), await attendues(p)),
      '21 boîtes à la place des mots (échelle 1)',
    );
    // Survol : 14 % d'accent ; zoom et déplacement ignorés pendant le choix.
    const [sx, sy] = await point(p, ...centreMot(1));
    await p.mouse.move(sx, sy);
    await uneImage(p);
    const survol = await p.$$eval('.calque-texte .mot', (bs) =>
      bs.map((b) => b.classList.contains('survol')),
    );
    const vue0 = await p.evaluate(() => JSON.stringify(__ed.vue));
    await p.mouse.wheel(0, -300);
    await p.keyboard.down('Control');
    await p.mouse.wheel(0, -300);
    await p.keyboard.press('Equal');
    await p.keyboard.press('Digit1');
    await p.keyboard.up('Control');
    await p.keyboard.press('Space');
    await p.keyboard.press('Shift+ArrowLeft');
    await uneImage(p);
    const vue1 = await p.evaluate(() => JSON.stringify(__ed.vue));
    verifier(
      JSON.stringify(survol) === '[false,true,false,false,false,false,false]' && vue0 === vue1,
      '21 survol d’un mot ; molette, Ctrl + molette, Ctrl + = et Ctrl + 1 sans effet sur la vue',
      JSON.stringify([survol, vue0, vue1]),
    );
    await p.screenshot({ path: path.join(SORTIE, 'editeur-texte-choix-clair.png') });
    // Tirer du mot 2 au mot 5, puis « Copier ».
    await glisser(p, centreMot(2), centreMot(5));
    const ch = await p.$$eval('.calque-texte .mot', (bs) =>
      bs.map((b) => b.classList.contains('choisi')),
    );
    await p.getByRole('button', { name: 'Copier', exact: true }).last().click();
    await uneImage(p);
    const c2 = await copies(p);
    verifier(
      JSON.stringify(ch) === '[false,false,true,true,true,true,false]' &&
        JSON.stringify(c2) === JSON.stringify([{ id: 7, numero: ex.numero, debut: 2, fin: 5 }]) &&
        !(await calque(p)),
      '21 tirer du mot 2 au mot 5 puis « Copier » : {debut: 2, fin: 5}, le calque se ferme',
      JSON.stringify([ch, c2]),
    );
    await p.evaluate(
      (n) => __emettre('editeur:texte_copie', { id: 7, numero: n, etat: 'fait' }),
      ex.numero,
    );
    await uneImage(p);
    const m1 = await p.$eval('#message', (m) => [
      m.textContent.trim(),
      m.classList.contains('on'),
      m.classList.contains('echec'),
    ]);
    verifier(
      JSON.stringify(m1) === '["Texte copié",true,false]',
      '21 editeur:texte_copie fait : « Texte copié »',
      JSON.stringify(m1),
    );
    // Clavier : Espace, Maj + Droite × 2, Entrée → {0, 2}.
    await vider(p);
    let n = await lireTexte(p);
    await p.keyboard.press('Space');
    await p.keyboard.press('Shift+ArrowRight');
    await p.keyboard.press('Shift+ArrowRight');
    const curseur = await p.$$eval('.calque-texte .mot', (bs) =>
      bs.map((b) => b.classList.contains('curseur')),
    );
    await p.keyboard.press('Enter');
    await uneImage(p);
    let c = await copies(p);
    verifier(
      JSON.stringify(c) === JSON.stringify([{ id: 7, numero: n, debut: 0, fin: 2 }]) &&
        curseur[2] &&
        curseur.filter(Boolean).length === 1,
      '21 Espace, Maj + Droite × 2, Entrée : {0, 2} (curseur sur le mot 2)',
      JSON.stringify([c, curseur]),
    );
    // Bas, Bas (ligne au plus près), Maj + Haut, Entrée.
    await vider(p);
    n = await lireTexte(p);
    for (const k of ['ArrowRight', 'ArrowRight', 'ArrowDown', 'Shift+ArrowDown'])
      await p.keyboard.press(k);
    await p.keyboard.press('Enter');
    c = await copies(p);
    verifier(
      JSON.stringify(c) === JSON.stringify([{ id: 7, numero: n, debut: 4, fin: 6 }]),
      '21 flèches : ligne du dessous au plus près, Maj étend : {4, 6}',
      JSON.stringify(c),
    );
    // Entrée sans choix : tout ; Ctrl + A puis Ctrl + C : tout ; « Tout copier ».
    await vider(p);
    n = await lireTexte(p);
    await p.keyboard.press('Enter');
    await vider(p);
    const n2 = await lireTexte(p);
    await p.keyboard.press('Control+a');
    await p.keyboard.press('Control+c');
    const n3 = await lireTexte(p);
    await p.getByRole('button', { name: 'Tout copier', exact: true }).click();
    c = await copies(p);
    const exportsCopier = await p.evaluate(
      () => __exports.filter((e) => e.entetes['x-editeur-but'] === 'copier').length,
    );
    verifier(
      JSON.stringify(c) ===
        JSON.stringify([
          { id: 7, numero: n2, debut: 0, fin: 6 },
          { id: 7, numero: n3, debut: 0, fin: 6 },
        ]) && exportsCopier === 0,
      '21 Ctrl + A puis Ctrl + C, « Tout copier » : tout (Ctrl + C ne copie pas l’image)',
      JSON.stringify(c),
    );
    // Échap : sort sans rien copier ni fermer la fenêtre ; « Fermer » aussi.
    await vider(p);
    await lireTexte(p);
    await p.keyboard.press('Escape');
    await uneImage(p);
    const e1 = [
      await calque(p),
      await appels(p, 'editeur_copier_texte'),
      await appels(p, 'editeur_fermer'),
    ];
    await lireTexte(p);
    await p.getByRole('button', { name: 'Fermer', exact: true }).last().click();
    const e2 = [await calque(p), await appels(p, 'editeur_copier_texte')];
    verifier(
      e1[0] === null &&
        e1[1].length === 0 &&
        e1[2].length === 0 &&
        e2[0] === null &&
        e2[1].length === 0,
      '21 Échap et « Fermer » : le calque se ferme, rien n’est copié, la fenêtre reste',
      JSON.stringify([e1, e2]),
    );
    // Copie en échec, lecture en échec, rien trouvé (4 s, sans « Réessayer »).
    await p.evaluate(() => __emettre('editeur:texte_copie', { id: 7, numero: 1, etat: 'echec' }));
    const m2 = await p.$eval('#message', (m) => m.textContent.trim());
    await lireTexte(p, { etat: 'echec', attendre: false });
    await p
      .waitForFunction(() => !document.querySelector('.calque-texte'), null, { timeout: 3000 })
      .catch(() => {});
    const m3 = await p.$eval('#message', (m) => m.textContent.trim());
    await lireTexte(p, { etat: 'vide', mots: [], attendre: false });
    await p
      .waitForFunction(() => !document.querySelector('.calque-texte'), null, { timeout: 3000 })
      .catch(() => {});
    const tv = await p.evaluate(() => performance.now());
    const m4 = await p.$eval('#message', (m) => [
      m.textContent.trim(),
      m.classList.contains('on'),
      m.classList.contains('echec'),
    ]);
    const reessayer = await p.getByText('Réessayer').count();
    await p.waitForTimeout(3600);
    const encore = await p.$eval('#message', (m) => m.classList.contains('on'));
    await p
      .waitForFunction(() => !document.querySelector('#message').classList.contains('on'), null, {
        timeout: 1500,
      })
      .catch(() => {});
    const tf = await p.evaluate((tv) => performance.now() - tv, tv);
    verifier(
      m2 === 'Impossible de copier' && m3 === 'Impossible de lire le texte',
      '21 copie en échec : « Impossible de copier » ; lecture en échec : « Impossible de lire le texte »',
      JSON.stringify([m2, m3]),
    );
    verifier(
      JSON.stringify(m4) === '["Aucun texte trouvé",true,true]' &&
        reessayer === 0 &&
        encore &&
        tf >= 3900 &&
        tf < 6000, // borne haute large : la pilule part seule, sans clic
      '21 vide : « Aucun texte trouvé » 4 s, sans « Réessayer »',
      JSON.stringify([m4, reessayer, encore, tf]),
    );
    // Un résultat d'un autre document ou d'un autre numéro : rien.
    await vider(p);
    await bouton(p, 'Texte').click();
    await p
      .waitForFunction(() => __appels.some((a) => a[0] === 'editeur_exporter'), null, {
        timeout: 3000,
      })
      .catch(() => {});
    const nn = await p.evaluate(() => __exports.at(-1).numero);
    await p.evaluate((n) => {
      __emettre('editeur:texte', { id: 8, numero: n, etat: 'lu', mots: [] });
      __emettre('editeur:texte', { id: 7, numero: n + 50, etat: 'vide', mots: [] });
    }, nn);
    await p.waitForTimeout(500);
    const autre = [await calque(p), await p.$eval('#message', (m) => m.classList.contains('on'))];
    await p.keyboard.press('Escape');
    verifier(
      autre[0] && !autre[0].choix && autre[1] === false,
      '21 editeur:texte d’un autre document ou d’un autre numéro : ignoré',
      JSON.stringify(autre),
    );
    await fin(p, '21 texte');
  }
  {
    // Sans `texte` dans la charge : aucun bouton, rien de créé.
    const p = await page({ charge: { texte: false } });
    const r = await p.evaluate(() => ({
      t: [...document.querySelectorAll('button')].filter(
        (b) => b.textContent.trim() === 'Texte' && b.closest('#barre-haut'),
      ).length,
      titre: [...document.querySelectorAll('[title="Copier le texte"]')].length,
    }));
    verifier(
      r.t === 0 && r.titre === 0,
      '21 texte: false : pas de bouton « Texte »',
      JSON.stringify(r),
    );
    // Document remplacé par une charge avec texte : le bouton paraît ; puis sans : il part.
    await p.evaluate(() =>
      __emettre('editeur:charger', { ...window.__charge, id: 7, texte: true }),
    );
    await p
      .waitForFunction(() => document.querySelector('#barre-haut [data-texte="lancer"]'), null, {
        timeout: 3000,
      })
      .catch(() => {});
    const avec = await p.locator('#barre-haut [data-texte="lancer"]').count();
    await p.evaluate(() =>
      __emettre('editeur:charger', { ...window.__charge, id: 7, texte: false }),
    );
    await uneImage(p);
    const sans = await p.locator('#barre-haut [data-texte="lancer"]').count();
    verifier(
      avec === 1 && sans === 0,
      '21 le bouton suit la charge (paraît, puis part)',
      JSON.stringify([avec, sans]),
    );
    await fin(p, '21 sans texte');
  }
  {
    // Le clavier appartient au calque, où que soit le focus ; barre du haut grisée.
    const p = await page({ charge: { texte: true } });
    const centre = (sel) =>
      p.$eval(sel, (b) => {
        const r = b.getBoundingClientRect();
        return [r.left + r.width / 2, r.top + r.height / 2];
      });
    await lireTexte(p);
    const grises = await p.evaluate(() =>
      ['enregistrer', 'copier', 'fermer'].map(
        (a) => document.querySelector(`#barre-haut [data-act="${a}"]`).disabled,
      ),
    );
    await vider(p);
    // Clic sur « Enregistrer » et « Copier » grisés : rien n'est exporté.
    await p.mouse.click(...(await centre('#barre-haut [data-act="enregistrer"]')));
    await p.mouse.click(...(await centre('#barre-haut [data-act="copier"]')));
    await uneImage(p);
    const sansExport = (await appels(p, 'editeur_exporter')).length;
    // Focus hors du calque (vide de la barre du haut) : Échap ne ferme que le calque.
    await p.mouse.click(...(await centre('#barre-haut .sp')));
    const hors = await p.evaluate(
      () => !document.querySelector('.calque-texte').contains(document.activeElement),
    );
    await p.keyboard.press('Escape');
    await uneImage(p);
    const e1 = [await calque(p), (await appels(p, 'editeur_fermer')).length];
    verifier(
      JSON.stringify(grises) === '[true,true,false]' &&
        sansExport === 0 &&
        hors &&
        e1[0] === null &&
        e1[1] === 0,
      '21 choix : « Enregistrer » et « Copier » du haut grisés (clic sans export) ; Échap, ' +
        'focus hors du calque, ne ferme que le calque',
      JSON.stringify([grises, sansExport, hors, e1]),
    );
    const apres = await p.evaluate(() =>
      ['enregistrer', 'copier'].map(
        (a) => document.querySelector(`#barre-haut [data-act="${a}"]`).disabled,
      ),
    );
    verifier(JSON.stringify(apres) === '[false,false]', '21 calque fermé : barre du haut rendue');
    // Ctrl + A puis focus hors du calque : Ctrl + C copie le texte, jamais l'image ; une
    // touche d'outil (P) est sans effet ; Tab reste dans le calque et sa barre.
    await vider(p);
    const n = await lireTexte(p);
    await p.keyboard.press('Control+a');
    await p.mouse.click(...(await centre('#barre-haut .sp')));
    const outil0 = await p.evaluate(() => __ed.prefs.outil);
    await p.keyboard.press('p');
    const outil1 = await p.evaluate(() => __ed.prefs.outil);
    const tabs = [];
    for (const k of ['Tab', 'Tab', 'Tab', 'Shift+Tab', 'Shift+Tab']) {
      await p.keyboard.press(k);
      tabs.push(
        await p.evaluate((n) => {
          const a = document.activeElement;
          return a.classList.contains('calque-texte')
            ? 'calque'
            : a.getAttribute('aria-label') || a.textContent.trim();
        }),
      );
    }
    const anneau = await p.evaluate(() => {
      const s = getComputedStyle(document.activeElement);
      return [document.activeElement.textContent.trim(), s.outlineStyle, s.outlineWidth];
    });
    await p.keyboard.press('Shift+Tab'); // le calque
    await p.keyboard.press('Shift+Tab'); // la croix, en boucle
    const anneauCroix = await p.evaluate(() => {
      const s = getComputedStyle(document.activeElement);
      return [document.activeElement.getAttribute('aria-label'), s.outlineStyle, s.outlineWidth];
    });
    await p.mouse.click(...(await centre('#barre-haut .sp')));
    await p.keyboard.press('Control+c');
    await uneImage(p);
    const c = await copies(p);
    const images = (await appels(p, 'editeur_exporter')).filter(
      (a) => a[2]['x-editeur-but'] !== 'texte',
    ).length;
    verifier(
      JSON.stringify(c) === JSON.stringify([{ id: 7, numero: n, debut: 0, fin: 6 }]) &&
        images === 0 &&
        outil0 === outil1,
      '21 focus hors du calque : Ctrl + C copie le texte choisi (pas l’image), P sans effet',
      JSON.stringify([c, images, outil0, outil1]),
    );
    verifier(
      JSON.stringify(tabs) === '["Copier","Tout copier","Fermer","Tout copier","Copier"]',
      '21 Tab et Maj + Tab restent dans le calque et sa barre',
      JSON.stringify(tabs),
    );
    verifier(
      JSON.stringify(anneau) === '["Copier","solid","2px"]' &&
        JSON.stringify(anneauCroix) === '["Fermer","solid","2px"]',
      '21 anneau de focus sur « Copier » et la croix',
      JSON.stringify([anneau, anneauCroix]),
    );
    // « Fermer » de la barre du haut : le calque se ferme, puis la fenêtre est demandée.
    await vider(p);
    await lireTexte(p);
    await p.mouse.click(...(await centre('#barre-haut [data-act="fermer"]')));
    await attendreAppel(p, 'editeur_fermer');
    verifier(
      (await calque(p)) === null && (await appels(p, 'editeur_fermer')).length === 1,
      '21 « Fermer » pendant le choix : le calque se ferme, puis la fenêtre suit son cours',
    );
    await fin(p, '21 clavier');
  }
  {
    // Résultat reçu avant le numéro de l'export : montré ; ou, après Échap, l'export finit.
    const p = await page({ charge: { texte: true } });
    await p.evaluate(() => {
      const orig = __TAURI__.core.invoke;
      __TAURI__.core.invoke = (c, a, o) => {
        const r = orig(c, a, o);
        if (c !== 'editeur_exporter') return r;
        return r.then((n) => new Promise((ok) => (window.__lacher = () => ok(n))));
      };
    });
    const avantNumero = async (escape) => {
      await p.evaluate(() => (window.__lacher = null));
      await bouton(p, 'Texte').click();
      await p.waitForFunction(() => window.__lacher, null, { timeout: 3000 }).catch(() => {});
      await p.evaluate((mots) => {
        const n = __exports.at(-1).numero;
        __emettre('editeur:texte', { id: 7, numero: n, etat: 'lu', mots });
      }, MOTS);
      if (escape) await p.keyboard.press('Escape');
      await p.evaluate(() => window.__lacher());
      await p.waitForTimeout(600);
      return p.evaluate(() => ({
        calque: document.querySelector('.calque-texte.choix') ? 'choix' : null,
        bloque: __ed.exportEnCours(),
        grise: document.querySelector('#barre-haut [data-texte="lancer"]').disabled,
      }));
    };
    const r1 = await avantNumero(false);
    await p.keyboard.press('Escape');
    const r2 = await avantNumero(true);
    verifier(
      JSON.stringify(r1) === '{"calque":"choix","bloque":false,"grise":false}' &&
        JSON.stringify(r2) === '{"calque":null,"bloque":false,"grise":false}',
      '21 editeur:texte avant le numéro : choix montré ; puis Échap : l’export ne reste pas bloqué',
      JSON.stringify([r1, r2]),
    );
    // Rejet de editeur_copier_texte : « Impossible de copier ».
    await p.evaluate(() => {
      const orig = __TAURI__.core.invoke;
      __TAURI__.core.invoke = (c, a, o) =>
        c === 'editeur_copier_texte' ? Promise.reject(new Error('refus')) : orig(c, a, o);
    });
    await avantNumero(false);
    await p.getByRole('button', { name: 'Tout copier', exact: true }).click();
    await p
      .waitForFunction(() => document.querySelector('#message').classList.contains('on'))
      .catch(() => {});
    const m = await p.$eval('#message', (m) => m.textContent.trim());
    verifier(
      m === 'Impossible de copier',
      '21 editeur_copier_texte rejeté : « Impossible de copier »',
      m,
    );
    await fin(p, '21 avant le numéro');
  }
  {
    // Recadrage : les boîtes sont en pixels de l'image composée (le cadre).
    const p = await page({ charge: { texte: true } });
    await p.evaluate(() =>
      __ed.modifier((d) => __ed.modele.recadrer(d, { x: 30, y: 20, largeur: 300, hauteur: 200 })),
    );
    await uneImage(p);
    await lireTexte(p);
    const cadre = await p.evaluate(() => __ed.modele.cadre(__ed.h.doc));
    const b = await boites(p);
    const att = [];
    for (const m of MOTS) {
      const [x0, y0] = await point(p, cadre.x + m.x, cadre.y + m.y);
      const [x1, y1] = await point(p, cadre.x + m.x + m.largeur, cadre.y + m.y + m.hauteur);
      att.push([x0, y0, x1 - x0, y1 - y0]);
    }
    verifier(
      cadre.x === 30 && cadre.y === 20 && justes(b, att),
      '21 recadrage : boîtes justes (décalées du cadre)',
      JSON.stringify([cadre, b[0], att[0]]),
    );
    await fin(p, '21 recadrage');
  }
  {
    // Menu « Fermer » ouvert par le cœur (`editeur:fermer_demande`, réponse « demander »)
    // pendant le calque : le calque se ferme d'abord, le clavier revient au menu.
    const p = await page({ charge: { texte: true } });
    const menu = () =>
      p.evaluate(() => ({
        ouvert: document.querySelector('#menu-fermer').classList.contains('ouvert'),
        focus: document.activeElement.closest('#menu-fermer')
          ? document.activeElement.textContent.trim()
          : document.activeElement.className,
      }));
    await lireTexte(p);
    await p.evaluate(() => __emettre('editeur:fermer_demande', { id: 7, raison: 'cadre' }));
    await p
      .waitForFunction(() => document.querySelector('#menu-fermer.ouvert'), null, { timeout: 3000 })
      .catch(() => {});
    const m0 = [await calque(p), await menu()];
    await p.keyboard.press('Tab');
    const m1 = await menu();
    await p.keyboard.press('Escape');
    await uneImage(p);
    const m2 = await menu();
    verifier(
      m0[0] === null &&
        JSON.stringify(m0[1]) === '{"ouvert":true,"focus":"Copier et fermer"}' &&
        JSON.stringify(m1) === '{"ouvert":true,"focus":"Fermer sans copier"}' &&
        m2.ouvert === false,
      '21 fermer_demande (cadre) pendant le calque : calque fermé, Tab reste dans le menu, ' +
        'Échap ferme le menu',
      JSON.stringify([m0, m1, m2]),
    );
    await vider(p);
    await lireTexte(p);
    await p.evaluate(() => __emettre('editeur:fermer_demande', { id: 7, raison: 'remplacer' }));
    await p
      .waitForFunction(() => document.querySelector('#menu-fermer.ouvert'), null, { timeout: 3000 })
      .catch(() => {});
    const r0 = await calque(p);
    await p.keyboard.press('Enter');
    await attendreAppel(p, 'editeur_exporter');
    const buts = (await appels(p, 'editeur_exporter')).map((a) => a[2]['x-editeur-but']);
    verifier(
      r0 === null &&
        JSON.stringify(buts) === '["texte","copier_fermer"]' &&
        (await copies(p)).length === 0,
      '21 fermer_demande (remplacer) pendant le calque : Entrée fait « Copier et fermer » ' +
        'de l’image, pas du texte',
      JSON.stringify([r0, buts]),
    );
    await fin(p, '21 menu Fermer');
  }
  {
    // Alt + F4 jamais annulé ; Espace active le bouton qui a le focus ; anneaux de focus.
    const p = await page({ charge: { texte: true } });
    await p.evaluate(() =>
      addEventListener('keydown', (e) => {
        if (e.key === 'F4') window.__f4 = e.defaultPrevented;
      }),
    );
    await lireTexte(p);
    await p.keyboard.press('Alt+F4');
    const f4 = await p.evaluate(() => window.__f4);
    verifier(
      f4 === false && (await calque(p)) !== null,
      '21 Alt + F4 pendant le calque : non annulé',
      JSON.stringify(f4),
    );
    await vider(p);
    await p.keyboard.press('Tab'); // Copier (grisé, sauté) → Tout copier
    const cible = await p.evaluate(() => document.activeElement.textContent.trim());
    await p.keyboard.press('Space');
    await uneImage(p);
    const c = await copies(p);
    await vider(p);
    await p.evaluate(() => document.querySelector('#barre-haut [data-act="enregistrer"]').focus());
    await p.keyboard.press('Space');
    await attendreAppel(p, 'editeur_exporter');
    const buts = (await appels(p, 'editeur_exporter')).map((a) => a[2]['x-editeur-but']);
    verifier(
      cible === 'Tout copier' &&
        c.length === 1 &&
        c[0].debut === 0 &&
        c[0].fin === 6 &&
        JSON.stringify(buts) === '["enregistrer"]',
      '21 Espace sur un bouton : « Tout copier » du calque, « Enregistrer » sans calque',
      JSON.stringify([cible, c, buts]),
    );
    await p.evaluate(
      (n) =>
        __emettre('editeur:resultat', {
          id: 7,
          numero: n,
          but: 'enregistrer',
          etat: 'fait',
          message: null,
        }),
      await p.evaluate(() => __exports.at(-1).numero),
    );
    const anneaux = [];
    for (const sel of ['#barre-haut [data-act="copier"]', '#outils .tl']) {
      await p.keyboard.press('Shift');
      await p.$eval(sel, (b) => b.focus());
      anneaux.push(
        await p.$eval(sel, (b) => {
          const s = getComputedStyle(b);
          return [b.matches(':focus-visible'), s.outlineStyle, s.outlineWidth];
        }),
      );
    }
    verifier(
      JSON.stringify(anneaux) === '[[true,"solid","2px"],[true,"solid","2px"]]',
      '21 anneau de focus sur « Copier » du haut et sur un outil',
      JSON.stringify(anneaux),
    );
    await fin(p, '21 Alt, Espace, anneaux');
  }
  // Boîtes justes aux échelles 1,25, 1,5 et 2, et après un changement de taille.
  for (const echelle of [1.25, 1.5, 2]) {
    const p = await page({ charge: { texte: true }, echelle });
    await lireTexte(p);
    const ok1 = justes(await boites(p), await attendues(p));
    await p.setViewportSize({ width: 900, height: 640 });
    await p.waitForFunction(() => innerWidth === 900, null, { timeout: 3000 }).catch(() => {});
    await uneImage(p);
    await uneImage(p);
    const b2 = await boites(p);
    const a2 = await attendues(p);
    verifier(
      ok1 && justes(b2, a2),
      `21 boîtes justes à ${echelle}, et replacées après changement de taille`,
      JSON.stringify([b2[0], a2[0]]),
    );
    // Tirer à cette échelle : mots 1 à 3.
    await vider(p);
    await glisser(p, centreMot(1), centreMot(3));
    await p.keyboard.press('Enter');
    const c = await copies(p);
    verifier(
      c.length === 1 && c[0].debut === 1 && c[0].fin === 3,
      `21 tirer à ${echelle} : {1, 3}`,
      JSON.stringify(c),
    );
    await fin(p, `21 échelle ${echelle}`);
  }
  {
    // Barre de l'éditeur à 640 px de large, en sombre, animations réduites (120 ms) ; barre
    // flottante dans la fenêtre.
    const p = await page({
      charge: { texte: true, reduites: true },
      largeur: 640,
      hauteur: 480,
      theme: 'dark',
    });
    const r = await p.evaluate(() => {
      const bs = [...document.querySelectorAll('#barre-haut > button')].map((b) =>
        b.getBoundingClientRect(),
      );
      const lib = [...document.querySelectorAll('#barre-haut span')].find(
        (s) => s.textContent === 'Texte',
      );
      const h = document.querySelector('#barre-haut').getBoundingClientRect();
      return {
        n: bs.length,
        rangee: bs.every((b) => b.top >= h.top && b.bottom <= h.bottom),
        ordre: bs.every((b, i) => i === 0 || bs[i - 1].right <= b.left),
        dedans: bs.every((b) => b.left >= 0 && b.right <= 640),
        lib:
          lib && lib.scrollWidth <= lib.clientWidth + 0.5 && lib.getBoundingClientRect().width > 20,
        deborde: document.documentElement.scrollWidth > innerWidth,
      };
    });
    verifier(
      r.n === 4 && r.rangee && r.ordre && r.dedans && r.lib && !r.deborde,
      '21 barre du haut à 640 px : quatre boutons sur une rangée, rien ne déborde',
      JSON.stringify(r),
    );
    const t0 = await p.evaluate(() => performance.now());
    await lireTexte(p);
    const dt = await p.evaluate((t0) => performance.now() - t0, t0);
    const b = await p.evaluate(() => {
      const r = document.querySelector('.barre-texte').getBoundingClientRect();
      const choisi = document.querySelector('.calque-texte .mot');
      choisi.classList.add('choisi');
      return {
        r: [r.left, r.top, r.right, r.bottom],
        bande: getComputedStyle(document.querySelector('.bande')).display,
        fond: getComputedStyle(choisi).backgroundColor,
      };
    });
    verifier(
      dt < 380 &&
        b.r[0] >= 0 &&
        b.r[2] <= 640 &&
        b.r[1] >= 52 &&
        b.r[3] <= 480 &&
        b.bande === 'none' &&
        b.fond !== 'rgba(0, 0, 0, 0)',
      '21 640 × 480 sombre, réduit : choix après 120 ms, sans bande ; barre dans la fenêtre ; mot choisi teinté',
      JSON.stringify([dt, b]),
    );
    await uneImage(p);
    await p.screenshot({ path: path.join(SORTIE, 'editeur-texte-640-sombre.png') });
    await fin(p, '21 640');
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
