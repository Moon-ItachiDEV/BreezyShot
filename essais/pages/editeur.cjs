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
  // « Texte » du haut (Copier le texte), à ne pas confondre avec l'outil Texte de la barre.
  const texteHaut = (p) =>
    p.locator('#barre-haut').getByRole('button', { name: 'Texte', exact: true });
  const outil = async (p, nom) => {
    await p.locator('#outils [data-outil]').and(bouton(p, nom)).click();
    await uneImage(p);
  };
  const ouvert = (p, sel) => p.$eval(sel, (m) => m.classList.contains('ouvert'));
  // L'outil en main (son bouton enfoncé) et l'outil déplié dans la barre.
  const choisi = (p) =>
    p.$$eval('#outils [data-outil][aria-pressed="true"]', (bs) =>
      bs.map((b) => b.getAttribute('aria-label')),
    );
  const deplie = (p) =>
    p.$$eval('#outils .pli[data-on]', (ps) => ps.map((x) => x.dataset.pli));
  // Le pli ouvert : les plis fermés sont inertes, mais Playwright y trouve encore les noms.
  const pli = (p) => p.locator('#outils .pli[data-on]');
  const reglage = async (p, nom) => {
    await pli(p).getByRole('button', { name: nom, exact: true }).click();
    await uneImage(p);
  };
  // Une variante dans le pli de l'outil (pris d'abord s'il ne l'est pas).
  const variante = async (p, outilNom, nom) => {
    if (JSON.stringify(await choisi(p)) !== JSON.stringify([outilNom])) await outil(p, outilNom);
    await reglage(p, nom);
  };
  // Ce que montre le pli ouvert : ses groupes (nom, boutons, enfoncés), sa glissière.
  const contenuPli = (p) =>
    p.evaluate(() => {
      const d = document.querySelector('#outils .pli[data-on] .in');
      if (!d) return null;
      const i = d.querySelector('input[type="range"]');
      return {
        groupes: [...d.querySelectorAll('[role="group"]')]
          .filter((g) => !g.hidden)
          .map((g) => [
            g.getAttribute('aria-label'),
            [...g.querySelectorAll('button')].map((b) => b.getAttribute('aria-label')),
          ]),
        enfonces: [...d.querySelectorAll('[aria-pressed="true"]')].map((b) =>
          b.getAttribute('aria-label'),
        ),
        opacite: i && [i.min, i.max, i.step, i.value, i.getAttribute('aria-label')],
      };
    });

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

  // ---------- 2. noms et rôles (avenant §4, barre C), absents, barre de la fenêtre ----------
  {
    const p = await page();
    const haut = p.locator('#barre-haut');
    let ok = true;
    const manque = [];
    for (const n of ['Fermer', 'Enregistrer', 'Copier']) {
      if ((await haut.getByRole('button', { name: n, exact: true }).count()) !== 1) manque.push(n);
    }
    const pal = p.getByRole('toolbar', { name: 'Annotation', exact: true });
    ok = (await pal.count()) === 1;
    for (const n of ['Annuler', 'Rétablir', 'Plus']) {
      if ((await pal.getByRole('button', { name: n, exact: true }).count()) !== 1) manque.push(n);
    }
    // Barre C : les neuf outils dans l'ordre, les quatre instruments dessinés puis un
    // séparateur ; la Flèche en main est dépliée, les autres plis sont fermés et inertes.
    const barre = await p.evaluate(() => {
      const tous = (s) => [...document.querySelectorAll(s)];
      return {
        outils: tous('#outils [data-outil]').map((b) => b.getAttribute('aria-label')),
        dessines: tous('#outils .tl.ib').map(
          (b) => b.dataset.outil + (b.querySelector('svg.inst') ? '' : ' sans dessin'),
        ),
        separe: !!document
          .querySelector('#outils .pli[data-pli="gomme"]')
          .nextElementSibling?.classList.contains('dv'),
        ouverts: tous('#outils .pli[data-on]').map((x) => x.dataset.pli),
        inertes: tous('#outils .pli > .in')
          .filter((x) => x.inert)
          .map((x) => x.parentElement.dataset.pli),
        anciens: [
          '#bouton-ajouter',
          '#menu-ajouter',
          '#reglages-outil',
          '#menu-couleur',
          '#bouton-couleur',
          '#couleurs',
          '[data-place="emplacement"]',
        ].filter((s) => document.querySelector(s)),
      };
    });
    verifier(
      ok &&
        !manque.length &&
        JSON.stringify(barre.outils) ===
          '["Flèche","Stylo","Surligneur","Gomme","Texte","Forme","Numéro","Flou","Projecteur"]' &&
        JSON.stringify(barre.dessines) === '["fleche","stylo","surligneur","gomme"]' &&
        barre.separe &&
        JSON.stringify(barre.ouverts) === '["fleche"]' &&
        JSON.stringify(barre.inertes) ===
          '["stylo","surligneur","gomme","texte","forme","numero","flou","projecteur"]' &&
        barre.anciens.length === 0,
      '2 barre « Annotation » : neuf outils dans l’ordre, quatre instruments dessinés, la Flèche seule dépliée, ni « Ajouter » ni bulle',
      JSON.stringify([manque, barre]),
    );
    const c = await contenuPli(p);
    verifier(
      c &&
        JSON.stringify(c.groupes) ===
          JSON.stringify([
            ['Taille', ['Fin', 'Moyen', 'Épais']],
            ['Couleur', ['Rouge', 'Jaune', 'Vert', 'Bleu', 'Noir', 'Blanc', 'Autre couleur']],
          ]) &&
        JSON.stringify(c.enfonces) === '["Moyen","Rouge"]' &&
        JSON.stringify(c.opacite) === '["20","100","5","100","Opacité"]',
      '2 Flèche dépliée : Taille, Couleur (six et « Autre couleur »), Opacité de 20 à 100 % par 5',
      JSON.stringify(c),
    );
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
      t.h === 52 && t.palH === 80 && t.bas === 14,
      '2 barre 52 px, barre d’annotation 80 px à 14 px du bas',
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
        o.courbe === undefined &&
        o.id === 1 &&
        JSON.stringify(await p.evaluate(() => __ed.selection)) === '[1]',
      '4 Flèche : flèche droite rouge (vrai modèle), restée choisie après le tracé',
      JSON.stringify(o),
    );
    // Courbée directement : la poignée du milieu, tirée, fait passer la courbe par la
    // pointe ; ramenée près du milieu de la corde, la flèche redevient droite.
    const pasAvant = await p.evaluate(() => __ed.h.passe.length);
    await glisser(p, [70, 30], [70, 70]);
    o = (await objets(p, 'fleche'))[0];
    const milieu = o && o.cx != null ? [(20 + 2 * o.cx + 120) / 4, (20 + 2 * o.cy + 40) / 4] : [];
    verifier(
      o &&
        o.cx != null &&
        Math.hypot(milieu[0] - 70, milieu[1] - 70) < 0.6 &&
        o.x1 === 20 &&
        o.y2 === 40 &&
        (await p.evaluate(() => __ed.h.passe.length)) === pasAvant + 1 &&
        JSON.stringify(await p.evaluate(() => __ed.selection)) === '[1]',
      '4 Flèche courbée à sa poignée du milieu : la courbe passe par la pointe, un pas, bouts gardés',
      JSON.stringify([o, milieu]),
    );
    await glisser(p, [70, 70], [71, 32]);
    o = (await objets(p, 'fleche'))[0];
    verifier(
      o && o.cx === undefined && o.cy === undefined && (await objets(p, 'fleche')).length === 1,
      '4 Flèche ramenée près du milieu de sa corde : droite de nouveau',
      JSON.stringify(o),
    );
    // La poignée du milieu d'une flèche droite à peine tirée : rien ne change, aucun pas.
    const pasDroite = await p.evaluate(() => __ed.h.passe.length);
    await glisser(p, [70, 30], [72, 31]);
    verifier(
      (await p.evaluate(() => __ed.h.passe.length)) === pasDroite &&
        (await objets(p, 'fleche'))[0].cx === undefined,
      '4 poignée du milieu à peine tirée : la flèche reste droite, aucun pas d’annulation',
    );
    await outil(p, 'Stylo');
    await glisser(p, [20, 120], [100, 150]);
    o = (await objets(p, 'crayon'))[0];
    verifier(o && o.points.length > 7, '4 Stylo : crayon lissé', JSON.stringify(o));
    await outil(p, 'Surligneur');
    await glisser(p, [20, 170], [100, 170]);
    o = (await objets(p, 'surligneur'))[0];
    verifier(o && o.couleur === '#FFD43B', '4 Surligneur : jaune', JSON.stringify(o));
    await outil(p, 'Forme');
    const forme = await contenuPli(p);
    verifier(
      JSON.stringify(await choisi(p)) === '["Forme"]' &&
        JSON.stringify(await deplie(p)) === '["forme"]' &&
        forme &&
        JSON.stringify(forme.groupes[0]) ===
          '["Variante",["Rectangle","Ellipse","Ligne"]]' &&
        forme.enfonces[0] === 'Rectangle',
      '4 Forme prise et dépliée : Rectangle, Ellipse, Ligne (Rectangle enfoncé)',
      JSON.stringify(forme),
    );
    await glisser(p, [150, 20], [230, 80]);
    o = (await objets(p, 'rectangle'))[0];
    verifier(
      o && o.plein === false && o.arrondi === 6 && o.x1 < o.x2,
      '4 Forme : rectangle en contour, rayon 6, normalisé',
      JSON.stringify(o),
    );
    await variante(p, 'Forme', 'Ellipse');
    await glisser(p, [250, 80], [170, 120]);
    o = (await objets(p, 'ellipse'))[0];
    verifier(o && o.x1 < o.x2 && o.y1 < o.y2, '4 Forme : ellipse normalisée', JSON.stringify(o));
    await outil(p, 'Numéro');
    await cliquer(p, 300, 30);
    await cliquer(p, 330, 30);
    const nums = await objets(p, 'numero');
    verifier(
      nums.length === 2 && nums[0].ordre === 1 && nums[1].ordre === 2 && nums[0].rayon === 16,
      '4 Numéro : deux numéros, ordres 1 et 2',
      JSON.stringify(nums),
    );
    await outil(p, 'Flou');
    await glisser(p, [260, 150], [380, 200]);
    o = (await objets(p, 'masque'))[0];
    verifier(
      o && o.sorte === 'pixeliser' && o.intensite === 1 && Number.isInteger(o.graine),
      '4 Flou : masque pixeliser (graine du modèle)',
      JSON.stringify(o),
    );
    await outil(p, 'Projecteur');
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
    await outil(p, 'Texte');
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
    await p.keyboard.press('b');
    await uneImage(p);
    verifier(
      JSON.stringify(await choisi(p)) === '["Flou"]' &&
        JSON.stringify(await deplie(p)) === '["flou"]' &&
        (await p.evaluate(() => __ed.prefs.variantes.flou)) === 'pixeliser',
      '4 touche B : Flou (Pixeliser), déplié',
    );
    await p.keyboard.press('a');
    await uneImage(p);
    verifier(JSON.stringify(await choisi(p)) === '["Flèche"]', '4 touche A : Flèche');
    await fin(p, '4');
  }

  // ---------- 5. Gomme au pixel : seulement là où elle passe, un geste = un pas ----------
  {
    const p = await page();
    // Un pixel de la vue, à un point de l'image.
    const pixel = (q, x, y) =>
      q.evaluate(
        ([a, b]) => {
          const v = __ed.vue;
          const c = new OffscreenCanvas(1, 1).getContext('2d', { willReadFrequently: true });
          c.drawImage(
            document.querySelector('#vue'),
            -Math.round(v.x + a * v.echelle),
            -Math.round(v.y + b * v.echelle),
          );
          return Array.from(c.getImageData(0, 0, 1, 1).data);
        },
        [x, y],
      );
    const egal = (a, b) => JSON.stringify(a) === JSON.stringify(b);
    const etat = (q) =>
      q.evaluate(() => ({
        types: __ed.h.doc.objets.map((o) => o.type),
        g: __ed.h.doc.objets.map((o) => (o.gommes ? o.gommes.length : 0)),
        pas: __ed.h.passe.length,
        apercu: __ed.gommes ? __ed.gommes.size : null,
      }));
    const nu = [await pixel(p, 100, 50), await pixel(p, 100, 150), await pixel(p, 75, 50)];
    await glisser(p, [50, 50], [150, 50]);
    await glisser(p, [50, 150], [150, 150]);
    await glisser(p, [50, 250], [150, 250]);
    const trace = [await pixel(p, 100, 50), await pixel(p, 75, 50)];
    const pas0 = (await etat(p)).pas;
    await p.keyboard.press('g');
    await uneImage(p);
    verifier(JSON.stringify(await choisi(p)) === '["Gomme"]', '5 touche G : Gomme');

    // Le rond suit le pointeur, à la taille de la gomme ; le pointeur du système est caché.
    const [gx, gy] = await point(p, 100, 20);
    const [hx, hy] = await point(p, 100, 180);
    await p.mouse.move(gx, gy);
    await uneImage(p);
    const rond = await p.evaluate(() => {
      const r = document.querySelector('#curseur-gomme');
      const b = r.getBoundingClientRect();
      return {
        cache: r.hidden,
        l: r.style.width,
        cx: b.left + b.width / 2,
        cy: b.top + b.height / 2,
        cur: document.querySelector('#scene').dataset.curseur,
        pointeur: getComputedStyle(document.querySelector('#vue')).cursor,
      };
    });
    verifier(
      !rond.cache &&
        rond.cur === 'gomme' &&
        rond.pointeur === 'none' &&
        rond.l === '20px' &&
        Math.abs(rond.cx - gx) < 1 &&
        Math.abs(rond.cy - gy) < 1,
      '5 Gomme : le rond de 20 px suit le pointeur',
      JSON.stringify([rond, gx, gy]),
    );

    // Pendant le geste : l'aperçu est au pixel, le document n'a pas bougé.
    await p.mouse.down();
    for (let i = 1; i <= 8; i++) await p.mouse.move(gx + ((hx - gx) * i) / 8, gy + ((hy - gy) * i) / 8);
    await uneImage(p);
    const pendant = await etat(p);
    const apercu = await pixel(p, 100, 50);
    const appui = await p.$eval('#curseur-gomme', (r) => r.classList.contains('appui'));
    await p.mouse.up();
    await uneImage(p);
    let r = await etat(p);
    verifier(
      pendant.apercu === 2 && pendant.pas === pas0 && egal(pendant.g, [0, 0, 0]) && appui &&
        egal(apercu, nu[0]),
      '5 Gomme, pendant le geste : aperçu au pixel, document intact',
      JSON.stringify([pendant, apercu, nu[0], appui]),
    );
    verifier(
      egal(r.types, ['fleche', 'fleche', 'fleche']) &&
        egal(r.g, [1, 1, 0]) &&
        r.pas === pas0 + 1 &&
        r.apercu === null,
      '5 Gomme : les deux flèches frottées restent, trouées, en un seul pas',
      JSON.stringify([pas0, r]),
    );
    const apres = [await pixel(p, 100, 50), await pixel(p, 100, 150), await pixel(p, 75, 50)];
    verifier(
      egal(apres[0], nu[0]) &&
        egal(apres[1], nu[1]) &&
        !egal(trace[0], nu[0]) &&
        egal(apres[2], trace[1]) &&
        !egal(trace[1], nu[2]),
      '5 Gomme : le fond revient là où elle passe (sans ombre), le trait reste à côté',
      JSON.stringify([nu, trace, apres]),
    );

    // Annuler et rétablir : le passage entier, d'un coup.
    await p.keyboard.press('Control+z');
    await uneImage(p);
    r = await etat(p);
    const defait = await pixel(p, 100, 50);
    await p.keyboard.press('Control+y');
    await uneImage(p);
    const refait = await etat(p);
    verifier(
      egal(r.g, [0, 0, 0]) && egal(defait, trace[0]) && egal(refait.g, [1, 1, 0]) &&
        refait.pas === pas0 + 1 && egal(await pixel(p, 100, 50), nu[0]),
      '5 Annuler rend les deux traits entiers, Rétablir les troue de nouveau',
      JSON.stringify([r, defait, refait]),
    );

    // Frotter dans le vide, ou dans un trou avec une gomme plus petite : rien ne change,
    // aucun pas.
    await glisser(p, [300, 20], [300, 280]);
    await outil(p, 'Gomme');
    await reglage(p, 'Petite');
    const petite = await p.evaluate(() => __ed.prefs.tailles.gomme);
    await glisser(p, [100, 44], [100, 56]);
    r = await etat(p);
    verifier(
      petite === 0 && egal(r.g, [1, 1, 0]) && r.pas === pas0 + 1,
      '5 Gomme dans le vide ou dans un trou : aucun pas',
      JSON.stringify([petite, r]),
    );

    // Échap pendant le geste : rien n'est gommé.
    const [ex, ey] = await point(p, 100, 230);
    await p.mouse.move(ex, ey);
    await p.mouse.down();
    await p.mouse.move(ex, ey + 20);
    await p.mouse.move(ex, ey + 40);
    await uneImage(p);
    const avantEchap = (await etat(p)).apercu;
    await p.keyboard.press('Escape');
    await uneImage(p);
    const echap = await etat(p);
    await p.mouse.up();
    await uneImage(p);
    r = await etat(p);
    verifier(
      avantEchap === 1 && echap.apercu === null && egal(r.g, [1, 1, 0]) && r.pas === pas0 + 1 &&
        JSON.stringify(await choisi(p)) === '["Gomme"]',
      '5 Échap pendant le geste : rien de gommé, aucun pas, la Gomme reste en main',
      JSON.stringify([avantEchap, echap, r]),
    );

    // Le trou ne se prend pas ; ce qui reste du trait se prend.
    await outil(p, 'Flèche');
    const rondCache = await p.$eval('#curseur-gomme', (e) => e.hidden);
    await cliquer(p, 100, 50);
    const dansTrou = await p.evaluate(() => [...__ed.selection]);
    await cliquer(p, 70, 50);
    const surReste = await p.evaluate(() => [...__ed.selection]);
    await p.keyboard.press('Escape');
    r = await etat(p);
    verifier(
      rondCache && egal(dansTrou, []) && egal(surReste, [1]) && r.types.length === 3,
      '5 un trait troué : le trou ne se prend pas, le reste se prend ; autre outil, plus de rond',
      JSON.stringify([rondCache, dansTrou, surReste, r]),
    );

    // Objets entiers (numéro) : la gomme qui passe dessus ne leur fait rien ; posée dessus,
    // elle les enlève en entier (clic ou frottement), cachés dès l'appui.
    await outil(p, 'Numéro');
    await cliquer(p, 300, 100);
    await cliquer(p, 340, 60);
    await outil(p, 'Gomme');
    const pas1 = (await etat(p)).pas;
    await glisser(p, [260, 100], [380, 100]);
    const frotte = await etat(p);
    await cliquer(p, 300, 100);
    const clique = await etat(p);
    verifier(
      egal(frotte.types, ['fleche', 'fleche', 'fleche', 'numero', 'numero']) &&
        frotte.pas === pas1 &&
        egal(frotte.g, [1, 1, 0, 0, 0]) &&
        egal(clique.types, ['fleche', 'fleche', 'fleche', 'numero']) && clique.pas === pas1 + 1,
      '5 Numéro : intact sous la gomme qui passe, enlevé par un clic de la gomme (un pas)',
      JSON.stringify([pas1, frotte, clique]),
    );
    const [nx, ny] = await point(p, 340, 60);
    const plein = await pixel(p, 340, 60);
    await p.mouse.move(nx, ny);
    await p.mouse.down();
    await uneImage(p);
    const pris = await p.evaluate(() => [__ed.gommeEntier, __ed.h.doc.objets.length]);
    const cache = await pixel(p, 340, 60);
    await p.mouse.move(nx + 15, ny + 4);
    await p.mouse.move(nx + 30, ny + 8);
    await p.mouse.up();
    await uneImage(p);
    r = await etat(p);
    verifier(
      pris[0] != null && pris[1] === 4 && !egal(cache, plein) &&
        egal(r.types, ['fleche', 'fleche', 'fleche']) && r.pas === pas1 + 2 &&
        (await p.evaluate(() => __ed.gommeEntier)) === null,
      '5 Numéro frotté depuis lui-même : caché dès l’appui, enlevé au relâchement (un pas)',
      JSON.stringify([pris, cache, plein, r]),
    );
    // Un clic sur un trait n'enlève que le point touché.
    await cliquer(p, 60, 250);
    r = await etat(p);
    verifier(
      egal(r.g, [1, 1, 1]) && r.pas === pas1 + 3 &&
        (await p.evaluate(() => __ed.h.doc.objets[2].gommes[0].points.length)) === 1,
      '5 clic de la gomme sur un trait : un point effacé, le trait reste',
      JSON.stringify(r),
    );

    // Gomme dépliée : ses trois tailles, ni couleur ni opacité.
    await outil(p, 'Stylo');
    await glisser(p, [300, 200], [320, 200]);
    await outil(p, 'Gomme');
    await calme(p);
    const gomme = await contenuPli(p);
    await reglage(p, 'Grande');
    const grande = await p.evaluate(() => __ed.prefs.tailles.gomme);
    const pas2 = (await etat(p)).pas;
    await glisser(p, [280, 200], [340, 200]);
    r = await etat(p);
    verifier(
      gomme &&
        egal(gomme.groupes, [['Taille', ['Petite', 'Moyenne', 'Grande']]]) &&
        gomme.opacite === null &&
        grande === 2 &&
        (await p.$eval('#curseur-gomme', (e) => e.style.width)) === '40px',
      '5 Gomme dépliée : Petite, Moyenne, Grande, sans couleur ; Grande = rond de 40 px',
      JSON.stringify([gomme, grande]),
    );
    verifier(
      egal(r.types, ['fleche', 'fleche', 'fleche']) && r.pas === pas2 + 1,
      '5 un trait tout effacé disparaît (un pas)',
      JSON.stringify([pas2, r]),
    );
    await p.keyboard.press('Control+z');
    await uneImage(p);
    r = await etat(p);
    verifier(
      egal(r.types, ['fleche', 'fleche', 'fleche', 'crayon']) && egal(r.g, [1, 1, 1, 0]),
      '5 Annuler : le trait tout effacé revient entier',
      JSON.stringify(r),
    );
    // Le pointeur sorti de l'image : plus de rond.
    await p.mouse.move(5, 5);
    await uneImage(p);
    verifier(await p.$eval('#curseur-gomme', (e) => e.hidden), '5 pointeur sorti de la vue : rond caché');
    await fin(p, '5');
  }

  // ---------- 5 bis. Gomme : ce qui est dessus décide (relecture du 13/11) ----------
  {
    const p = await page();
    const egal = (a, b) => JSON.stringify(a) === JSON.stringify(b);
    const etat = (q) =>
      q.evaluate(() => ({
        types: __ed.h.doc.objets.map((o) => o.type),
        g: __ed.h.doc.objets.map((o) => (o.gommes ? o.gommes.length : 0)),
        pas: __ed.h.passe.length,
      }));
    // Un numéro posé à côté d'une flèche puis emmené sur sa queue (il est au-dessus d'elle) ;
    // appui de la gomme sur lui, en tremblant de 2 px : il part en entier, la flèche qu'il
    // couvrait n'est pas trouée.
    await outil(p, 'Flèche');
    await glisser(p, [100, 60], [300, 60]);
    await outil(p, 'Numéro');
    await cliquer(p, 110, 110);
    await glisser(p, [110, 110], [110, 60]);
    await outil(p, 'Gomme');
    let pas = (await etat(p)).pas;
    const [x, y] = await point(p, 110, 60);
    await p.mouse.move(x, y);
    await p.mouse.down();
    await p.mouse.move(x + 2, y);
    await p.mouse.up();
    await uneImage(p);
    let r = await etat(p);
    verifier(
      egal(r.types, ['fleche']) && egal(r.g, [0]) && r.pas === pas + 1,
      '5 bis gomme posée sur un numéro qui couvre une flèche : le numéro part, la flèche n’est pas trouée (un pas)',
      JSON.stringify([pas, r]),
    );
    // Frottée depuis un numéro posé sur la flèche : le numéro part, puis la flèche est gommée
    // là où la gomme passe ensuite.
    await outil(p, 'Numéro');
    await cliquer(p, 200, 110);
    await glisser(p, [200, 110], [200, 60]);
    await outil(p, 'Gomme');
    pas = (await etat(p)).pas;
    await glisser(p, [200, 60], [260, 60]);
    r = await etat(p);
    verifier(
      egal(r.types, ['fleche']) && egal(r.g, [1]) && r.pas === pas + 1,
      '5 bis frottée depuis un numéro posé sur une flèche : le numéro part, la flèche est gommée où la gomme passe (un pas)',
      JSON.stringify([pas, r]),
    );
    // Un numéro, puis une flèche tracée par-dessus : l'appui sur la flèche la gomme, le
    // numéro reste.
    await outil(p, 'Numéro');
    await cliquer(p, 200, 150);
    await outil(p, 'Flèche');
    await glisser(p, [120, 150], [300, 150]);
    await outil(p, 'Gomme');
    pas = (await etat(p)).pas;
    await cliquer(p, 200, 150);
    r = await etat(p);
    verifier(
      egal(r.types, ['fleche', 'numero', 'fleche']) && egal(r.g, [1, 0, 1]) && r.pas === pas + 1,
      '5 bis gomme posée sur une flèche tracée par-dessus un numéro : la flèche est gommée, le numéro reste',
      JSON.stringify([pas, r]),
    );
    // Une flèche tracée sur un flou : l'appui à 6 px d'elle (sur le flou) la gomme, le flou
    // reste.
    await outil(p, 'Flou');
    await glisser(p, [80, 200], [320, 280]);
    await outil(p, 'Flèche');
    await glisser(p, [40, 240], [300, 240]);
    await outil(p, 'Gomme');
    pas = (await etat(p)).pas;
    await cliquer(p, 200, 246);
    r = await etat(p);
    verifier(
      egal(r.types, ['fleche', 'numero', 'fleche', 'masque', 'fleche']) &&
        egal(r.g, [1, 0, 1, 0, 1]) &&
        r.pas === pas + 1,
      '5 bis gomme posée près d’une flèche tracée sur un flou : la flèche est gommée, le flou reste',
      JSON.stringify([pas, r]),
    );
    // Sur le flou, loin de toute encre : le flou part en entier.
    await cliquer(p, 300, 210);
    r = await etat(p);
    verifier(
      egal(r.types, ['fleche', 'numero', 'fleche', 'fleche']) && r.pas === pas + 2,
      '5 bis gomme posée sur le flou loin des encres : il part en entier (un pas)',
      JSON.stringify([pas, r]),
    );
    // Une flèche, un numéro emmené dessus, un trait de stylo par-dessus le numéro : l'appui
    // de la gomme sur la pile gomme le trait seul ; le numéro reste et la flèche qu'il cache
    // n'est pas trouée (relecture des corrections).
    await outil(p, 'Flèche');
    await glisser(p, [40, 100], [180, 100]);
    await outil(p, 'Numéro');
    await cliquer(p, 110, 128);
    await glisser(p, [110, 128], [110, 100]);
    await outil(p, 'Stylo');
    await glisser(p, [86, 100], [134, 100]);
    await outil(p, 'Gomme');
    pas = (await etat(p)).pas;
    await cliquer(p, 110, 100);
    r = await etat(p);
    verifier(
      egal(r.types.slice(-3), ['fleche', 'numero', 'crayon']) &&
        egal(r.g.slice(-3), [0, 0, 1]) &&
        r.pas === pas + 1,
      '5 bis gomme posée sur un trait qui couvre un numéro posé sur une flèche : le trait est gommé, le numéro reste, la flèche cachée n’est pas trouée (un pas)',
      JSON.stringify([pas, r]),
    );
    await fin(p, '5 bis');
  }

  // ---------- 6. sélection, déplacement, suppression ----------
  {
    const p = await page();
    await outil(p, 'Forme');
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
    // Emmené puis ramené à sa place dans le même geste : aucun pas, et le document garde sa
    // version (« Fermer » après un Copier ne redemande rien).
    const [bx, by] = await point(p, 140, 160);
    const version0 = await p.evaluate(() => __ed.h.version);
    await p.mouse.move(bx, by);
    await p.mouse.down();
    await p.mouse.move(bx + 30, by + 10);
    await uneImage(p);
    const pendant = await p.evaluate(() => __ed.h.version);
    await p.mouse.move(bx, by);
    await p.mouse.up();
    await uneImage(p);
    const version1 = await p.evaluate(() => __ed.h.version);
    verifier(
      (await p.evaluate(() => __ed.h.passe.length)) === pas0 + 1 &&
        Math.round((await objets(p))[0].x1) === 140 &&
        pendant !== version0 &&
        version1 === version0,
      '6 emmené puis ramené à sa place : aucun pas de plus, la version d’avant le geste',
      JSON.stringify([version0, pendant, version1]),
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

  // ---------- 9. l'outil se déplie dans la barre (barre C) ----------
  {
    const p = await page();
    // Prendre un outil le déplie ; recliqué, il reste déplié. Rien ne s'ouvre par-dessus.
    await outil(p, 'Stylo');
    const pris = await deplie(p);
    await outil(p, 'Stylo');
    const repris = await deplie(p);
    await outil(p, 'Flèche');
    verifier(
      JSON.stringify(pris) === '["stylo"]' &&
        JSON.stringify(repris) === '["stylo"]' &&
        JSON.stringify(await deplie(p)) === '["fleche"]' &&
        (await p.$$('.pop.ouvert')).length === 0,
      '9 prendre un outil le déplie, le recliquer le garde déplié, aucune bulle',
      JSON.stringify([pris, repris]),
    );
    await reglage(p, 'Épais');
    await pli(p)
      .locator('input[type="range"]')
      .evaluate((i) => {
        i.value = 55;
        i.dispatchEvent(new Event('input', { bubbles: true }));
        i.dispatchEvent(new Event('change', { bubbles: true }));
      });
    await glisser(p, [20, 20], [120, 40]);
    let o = (await objets(p, 'fleche'))[0];
    verifier(
      o && o.epaisseur === 2 && o.opacite === 0.55,
      '9 taille et opacité de la Flèche : la flèche tracée les prend',
      JSON.stringify(o),
    );
    // La flèche tracée reste choisie : le pli montre ses valeurs et les lui applique, en un
    // pas ; la même valeur, aucun.
    const c0 = await contenuPli(p);
    const pas0 = await p.evaluate(() => __ed.h.passe.length);
    await reglage(p, 'Fin');
    const pas1 = await p.evaluate(() => __ed.h.passe.length);
    await reglage(p, 'Fin');
    o = (await objets(p, 'fleche'))[0];
    verifier(
      c0 &&
        JSON.stringify(c0.enfonces) === '["Épais","Rouge"]' &&
        c0.opacite[3] === '55' &&
        o.epaisseur === 0 &&
        pas1 === pas0 + 1 &&
        (await p.evaluate(() => __ed.h.passe.length)) === pas1 &&
        (await p.evaluate(() => __ed.prefs.tailles.fleche)) === 0,
      '9 flèche choisie : le pli montre ses valeurs ; « Fin » la règle en un pas, recliqué aucun',
      JSON.stringify([c0, o, pas0, pas1]),
    );
    // La glissière tirée de 55 à 30 puis ramenée à 55 dans le même geste : aucun pas ; tirée
    // de 55 à 40 : un pas.
    const tirer = (valeurs) =>
      pli(p)
        .locator('input[type="range"]')
        .evaluate((i, v) => {
          for (const x of v) {
            i.value = x;
            i.dispatchEvent(new Event('input', { bubbles: true }));
          }
          i.dispatchEvent(new Event('change', { bubbles: true }));
        }, valeurs);
    const v0 = await p.evaluate(() => [__ed.h.passe.length, __ed.h.version]);
    await tirer([45, 30, 45, 55]);
    const v1 = await p.evaluate(() => [__ed.h.passe.length, __ed.h.version, __ed.h.doc.objets[0].opacite]);
    await tirer([50, 40]);
    const v2 = await p.evaluate(() => [__ed.h.passe.length, __ed.h.doc.objets[0].opacite]);
    verifier(
      v1[0] === v0[0] && v1[1] === v0[1] && v1[2] === 0.55 && v2[0] === v0[0] + 1 && v2[1] === 0.4,
      '9 glissière tirée puis ramenée à sa valeur dans le même geste : aucun pas, même version ; tirée ailleurs : un pas',
      JSON.stringify([v0, v1, v2]),
    );

    // Ce que chaque outil déplie (Couleurs : les six et « Autre couleur »).
    const resume = async (nom) => {
      await outil(p, nom);
      const c = await contenuPli(p);
      if (!c) return '';
      return c.groupes
        .flatMap(([g, bs]) => (g === 'Couleur' ? ['Couleurs'] : bs))
        .concat(c.opacite ? ['Opacité'] : [])
        .join(',');
    };
    const vu = {};
    for (const nom of ['Flèche', 'Stylo', 'Surligneur', 'Gomme', 'Texte']) vu[nom] = await resume(nom);
    const glyphes = await p.$$eval('#outils .pli[data-on] .sz .a', (s) =>
      s.map((x) => getComputedStyle(x).fontSize),
    );
    await reglage(p, 'Grand');
    const tt = await p.evaluate(() => __ed.prefs.tailles.texte);
    for (const nom of ['Forme', 'Numéro', 'Flou']) vu[nom] = await resume(nom);
    await reglage(p, 'Bloc noir');
    const bloc = await contenuPli(p);
    vu.Projecteur = await resume('Projecteur');
    const attendu = {
      Flèche: 'Fin,Moyen,Épais,Couleurs,Opacité',
      Stylo: 'Fin,Moyen,Épais,Couleurs,Opacité',
      Surligneur: 'Fin,Moyen,Épais,Couleurs',
      Gomme: 'Petite,Moyenne,Grande',
      Texte: 'Petit,Moyen,Grand,Couleurs,Opacité',
      Forme: 'Rectangle,Ellipse,Ligne,Fin,Moyen,Épais,Couleurs,Opacité',
      Numéro: 'Petit,Moyen,Grand,Couleurs,Opacité',
      Flou: 'Pixeliser,Flouter,Bloc noir,Léger,Moyen,Fort',
      Projecteur: 'Rectangle,Ovale',
    };
    verifier(
      JSON.stringify(vu) === JSON.stringify(attendu) &&
        JSON.stringify(glyphes) === '["12px","16px","21px"]' &&
        tt === 2 &&
        JSON.stringify(bloc.groupes) === '[["Variante",["Pixeliser","Flouter","Bloc noir"]]]',
      '9 ce que chaque outil déplie ; Texte en A de trois tailles ; Bloc noir sans intensité',
      JSON.stringify({ vu, glyphes, tt, bloc }),
    );

    // Un objet choisi déplie l'outil qui l'a fait, à ses valeurs ; l'outil en main ne change
    // pas. Ce qu'on y règle va à l'objet et à son outil, pas à l'outil en main.
    await outil(p, 'Forme');
    await reglage(p, 'Rectangle');
    await reglage(p, 'Vert');
    await reglage(p, 'Épais');
    await glisser(p, [100, 100], [200, 180]);
    await outil(p, 'Flèche');
    await cliquer(p, 100, 140);
    const rect = (await objets(p, 'rectangle'))[0];
    const c1 = await contenuPli(p);
    verifier(
      rect &&
        JSON.stringify(await p.evaluate(() => __ed.selection)) === JSON.stringify([rect.id]) &&
        JSON.stringify(await deplie(p)) === '["forme"]' &&
        JSON.stringify(await choisi(p)) === '["Flèche"]' &&
        JSON.stringify(c1.enfonces) === '["Rectangle","Épais","Vert"]',
      '9 rectangle choisi avec la Flèche : la Forme se déplie à ses valeurs, la Flèche reste en main',
      JSON.stringify([rect, c1]),
    );
    const pas2 = await p.evaluate(() => __ed.h.passe.length);
    await reglage(p, 'Ellipse');
    const conv = await p.evaluate(() => ({
      types: __ed.h.doc.objets.map((x) => x.type),
      o: __ed.h.doc.objets.find((x) => x.type === 'ellipse'),
      pas: __ed.h.passe.length,
      sel: __ed.selection,
      pref: __ed.prefs.variantes.forme,
    }));
    verifier(
      conv.o &&
        JSON.stringify(conv.types) === '["fleche","ellipse"]' &&
        conv.o.id === rect.id &&
        conv.o.x1 === rect.x1 &&
        conv.o.y2 === rect.y2 &&
        conv.o.couleur === rect.couleur &&
        conv.o.plein === false &&
        conv.o.arrondi === undefined &&
        conv.pas === pas2 + 1 &&
        JSON.stringify(conv.sel) === JSON.stringify([rect.id]) &&
        conv.pref === 'ellipse',
      '9 « Ellipse » : le rectangle choisi devient une ellipse à la même place (un pas), et la Forme la garde',
      JSON.stringify([conv, rect]),
    );
    await reglage(p, 'Bleu');
    const couleurs = await p.evaluate(() => ({
      o: __ed.h.doc.objets.find((x) => x.type === 'ellipse').couleur,
      forme: __ed.prefs.couleurs.forme,
      fleche: __ed.prefs.couleurs.fleche,
    }));
    verifier(
      JSON.stringify(couleurs) === '{"o":"#0090FF","forme":"#0090FF","fleche":"#E5484D"}',
      '9 « Bleu » : l’ellipse choisie et la Forme, pas la Flèche en main',
      JSON.stringify(couleurs),
    );
    await outil(p, 'Stylo');
    verifier(
      (await p.evaluate(() => __ed.selection.length)) === 0 &&
        JSON.stringify(await deplie(p)) === '["stylo"]',
      '9 prendre le Stylo : plus rien de choisi, le Stylo se déplie',
    );
    await fin(p, '9');
  }

  // ---------- 9 bis. texte rouvert d'un double clic, un autre outil en main ----------
  {
    const p = await page();
    await outil(p, 'Texte');
    await cliquer(p, 150, 150);
    await p.keyboard.type('Salut');
    await p.keyboard.press('Enter');
    await uneImage(p);
    await outil(p, 'Flèche');
    const avant = await p.evaluate(() => ({ ...__ed.prefs.couleurs }));
    const pas0 = await p.evaluate(() => __ed.h.passe.length);
    const [x, y] = await point(p, 162, 162);
    await p.mouse.dblclick(x, y);
    await uneImage(p);
    const ouvert = { deplie: await deplie(p), saisie: await p.evaluate(() => !!__ed.outils.objetEnSaisie()) };
    // Le pli du texte : ses tailles (Petit, Moyen, Grand), ses couleurs, son opacité.
    const pli = await contenuPli(p);
    await reglage(p, 'Bleu');
    await reglage(p, 'Grand');
    const focus = await p.evaluate(() => document.activeElement && document.activeElement.id);
    await p.keyboard.press('Enter');
    await uneImage(p);
    const r = await p.evaluate(() => ({
      o: __ed.h.doc.objets.map((o) => [o.type, o.couleur, o.taille]),
      prefs: { ...__ed.prefs.couleurs },
      pas: __ed.h.passe.length,
    }));
    verifier(
      JSON.stringify(ouvert) === '{"deplie":["texte"],"saisie":true}' &&
        pli &&
        JSON.stringify(pli.groupes[0]) === '["Taille",["Petit","Moyen","Grand"]]' &&
        focus === 'saisie' &&
        JSON.stringify(r.o) === '[["texte","#0090FF",40]]' &&
        r.prefs.texte === '#0090FF' &&
        r.prefs.fleche === avant.fleche &&
        r.pas === pas0 + 1,
      '9 bis texte rouvert d’un double clic, Flèche en main : le pli du texte se déplie ; Bleu et Grand vont au texte (un pas), la flèche garde sa couleur',
      JSON.stringify([ouvert, pli, focus, r, avant.fleche]),
    );
    verifier(
      JSON.stringify(await deplie(p)) === '["fleche"]',
      '9 bis texte validé : le pli de la Flèche (outil en main) revient',
    );
    await fin(p, '9 bis');
  }

  // ---------- 9 ter. clavier : variante prise au clavier, focus du pli qui se referme ----------
  {
    const p = await page();
    // Une ellipse choisie, touche L : on prend Ligne pour dessiner ; l'ellipse n'est pas
    // changée et n'est plus choisie ; le pli montre Ligne enfoncée.
    await variante(p, 'Forme', 'Ellipse');
    await glisser(p, [60, 60], [160, 140]);
    await cliquer(p, 60, 100);
    const avant = await p.evaluate(() => ({ s: __ed.selection.length, pas: __ed.h.passe.length }));
    await p.keyboard.press('l');
    await uneImage(p);
    const apres = await p.evaluate(() => ({
      types: __ed.h.doc.objets.map((o) => o.type),
      s: __ed.selection.length,
      v: __ed.prefs.variantes.forme,
      pas: __ed.h.passe.length,
    }));
    const pliL = await contenuPli(p);
    verifier(
      avant.s === 1 &&
        JSON.stringify(apres.types) === '["ellipse"]' &&
        apres.s === 0 &&
        apres.v === 'ligne' &&
        apres.pas === avant.pas &&
        pliL &&
        pliL.enfonces.includes('Ligne'),
      '9 ter ellipse choisie, touche L : Ligne prise pour dessiner, l’ellipse inchangée et plus choisie, le pli montre Ligne',
      JSON.stringify([avant, apres, pliL]),
    );
    // Flèche en main, l'ellipse choisie : le pli de la Forme ; focus sur « Moyen », Échap :
    // la sélection part, le pli se referme, le focus va au bouton Forme (pas à la page).
    await outil(p, 'Flèche');
    await cliquer(p, 60, 100);
    const ouvert = await deplie(p);
    await pli(p).getByRole('button', { name: 'Moyen', exact: true }).focus();
    await p.keyboard.press('Escape');
    await uneImage(p);
    const focus = await p.evaluate(() => {
      const a = document.activeElement;
      return { tag: a && a.tagName, outil: a && a.dataset && a.dataset.outil, s: __ed.selection.length };
    });
    verifier(
      JSON.stringify(ouvert) === '["forme"]' &&
        focus.s === 0 &&
        focus.outil === 'forme' &&
        JSON.stringify(await deplie(p)) === '["fleche"]',
      '9 ter focus sur un réglage du pli de l’objet, Échap : le pli se referme, le focus va au bouton de son outil',
      JSON.stringify([ouvert, focus]),
    );
    await fin(p, '9 ter');
  }

  // ---------- 9 quater. barre plus claire (piste A « Calme », design/barre-claire) ----------
  {
    const p = await page();
    // Aucun outil ne répète le dessin d'une de ses variantes (la 0.5.4 montrait cinq grilles
    // dans le pli du Flou) ; chaque pictogramme visé existe.
    const dessins = await p.evaluate(() => {
      const ref = (e) => (e && e.querySelector('use') ? e.querySelector('use').getAttribute('href') : '');
      const r = {};
      for (const b of document.querySelectorAll('#outils [data-outil]')) {
        const variantes = [...b.closest('.pli').querySelectorAll('[data-variante]')];
        r[b.dataset.outil] = [ref(b), variantes.map(ref)];
      }
      return r;
    });
    const repetes = Object.entries(dessins).filter(([, [o, vs]]) => o && vs.includes(o));
    const manquants = await p.evaluate(() =>
      [...new Set([...document.querySelectorAll('#palette use')].map((u) => u.getAttribute('href')))].filter(
        (h) => !document.querySelector(h),
      ),
    );
    verifier(
      repetes.length === 0 &&
        manquants.length === 0 &&
        JSON.stringify(dessins.flou) === '["#i-goutte",["#i-mosaique","#i-doux","#i-box"]]' &&
        JSON.stringify(dessins.projecteur) === '["#i-lampe",["#i-spot","#i-spotE"]]',
      '9 quater aucun outil ne répète une de ses variantes ; Flou : goutte, mosaïque, tache floue, bande ; Projecteur : lampe',
      JSON.stringify({ dessins, manquants }),
    );
    // Le choix d'un groupe : une pastille claire en relief (jeton editeur-choix-fond, ombre),
    // le signe à la couleur du texte ; plus de carré plein. Un trait fin sépare les variantes
    // de l'intensité, en trois points comme les épaisseurs ; le dernier groupe visible garde
    // une marge au bord du pli.
    await outil(p, 'Flou');
    await calme(p);
    const lire = () =>
      p.evaluate(() => {
        const sonde = document.createElement('i');
        sonde.style.setProperty('background', 'var(--editeur-choix-fond)');
        sonde.style.setProperty('color', 'var(--text)');
        sonde.style.setProperty('padding-right', 'var(--editeur-pli-marge)');
        document.body.appendChild(sonde);
        const s = getComputedStyle(sonde);
        const attendu = [s.backgroundColor, s.color, s.paddingRight];
        sonde.remove();
        const d = document.querySelector('#outils .pli[data-on]');
        const vs = d.querySelector('[data-variante]').closest('.grp');
        const g = d.querySelector('[data-cle="intensite"]');
        const sep = d.querySelector('.sep');
        return {
          attendu,
          choix: [...d.querySelectorAll('.sz[aria-pressed="true"]')].map((b) => {
            const c = getComputedStyle(b);
            return [b.getAttribute('aria-label'), c.backgroundColor, c.boxShadow !== 'none', c.color];
          }),
          trait: !!sep && !sep.hidden && sep.getBoundingClientRect().width > 0,
          signes: g.hidden ? [] : [...g.querySelectorAll('[data-taille] b')].map((b) => getComputedStyle(b).width),
          marges: [vs, g].map((e) => (e.hidden ? '' : getComputedStyle(e).paddingRight)),
        };
      });
    const pix = await lire();
    await reglage(p, 'Flouter');
    await calme(p);
    const flo = await lire();
    await reglage(p, 'Bloc noir');
    await calme(p);
    const blo = await lire();
    const [fond, texte, marge] = pix.attendu;
    const points = JSON.stringify(['4px', '7px', '11px']);
    verifier(
      JSON.stringify(pix.choix) ===
        JSON.stringify([
          ['Pixeliser', fond, true, texte],
          ['Moyen', fond, true, texte],
        ]) &&
        fond !== texte &&
        marge === '4px' &&
        pix.trait &&
        JSON.stringify(pix.signes) === points &&
        JSON.stringify(pix.marges) === JSON.stringify(['0px', marge]) &&
        flo.trait &&
        JSON.stringify(flo.signes) === points &&
        !blo.trait &&
        JSON.stringify(blo.signes) === '[]' &&
        JSON.stringify(blo.marges) === JSON.stringify([marge, '']),
      '9 quater choix en pastille claire ; trait entre variantes et intensité en trois points ; cachés pour Bloc noir ; marge du dernier groupe',
      JSON.stringify({ pix, flo, blo }),
    );
    // Un outil replié garde une petite nuance bordée de sa couleur sous son pictogramme, pas
    // un trait ; un outil sans couleur (Projecteur) n'en a pas.
    const nuances = await p.evaluate(() =>
      ['texte', 'projecteur'].map((t) => {
        const a = getComputedStyle(document.querySelector(`#outils [data-outil="${t}"]`), '::after');
        return [a.width, a.height, a.borderTopLeftRadius, a.backgroundColor, a.boxShadow !== 'none'];
      }),
    );
    verifier(
      JSON.stringify(nuances) ===
        JSON.stringify([
          ['8px', '8px', '2px', 'rgb(229, 72, 77)', true],
          ['8px', '8px', '2px', 'rgba(0, 0, 0, 0)', false],
        ]),
      '9 quater outil replié : une nuance bordée de sa couleur, rien sans couleur',
      JSON.stringify(nuances),
    );
    await fin(p, '9 quater');
    // Bloc noir : une bande toujours noire, dans les deux thèmes (le dessin cloné par <use>
    // prend la règle de son original).
    const noirs = [];
    for (const theme of ['light', 'dark']) {
      const q = await page({ theme });
      noirs.push(await q.evaluate(() => getComputedStyle(document.querySelector('#i-box .bloc-noir')).fill));
      await fin(q, `9 quater ${theme}`);
    }
    verifier(
      JSON.stringify(noirs) === '["rgb(20, 21, 24)","rgb(11, 12, 14)"]',
      '9 quater Bloc noir : une bande noire en clair comme en sombre',
      JSON.stringify(noirs),
    );
  }

  // ---------- 10. couleurs : le changement se voit sur l'instrument ----------
  {
    const p = await page();
    const outilVu = (q, t) =>
      q.evaluate((x) => {
        const b = document.querySelector(`#outils [data-outil="${x}"]`);
        const inst = b.querySelector('.inst');
        const encre = inst && inst.querySelector('.i-encre');
        return {
          ink: b.style.getPropertyValue('--ink'),
          ton: b.dataset.ton || '',
          encre: inst ? inst.style.getPropertyValue('--encre') : null,
          rempli: encre ? getComputedStyle(encre).fill : null,
          saute: inst ? inst.classList.contains('saute') : null,
          fonce: inst ? inst.classList.contains('encre-foncee') : null,
          pale: inst ? inst.classList.contains('encre-pale') : null,
          t: inst ? inst.getAttribute('data-t') : null,
        };
      }, t);
    const enfoncee = (q) =>
      q.$$eval('#outils .pli[data-on] .sw[aria-pressed="true"]', (bs) =>
        bs.map((b) => b.getAttribute('aria-label')),
      );
    await reglage(p, 'Vert');
    const saut = (await outilVu(p, 'fleche')).saute;
    await calme(p);
    const r = { c: await p.evaluate(() => __ed.prefs.couleurs.fleche), ...(await outilVu(p, 'fleche')) };
    verifier(
      r.c === '#30A46C' &&
        JSON.stringify(await enfoncee(p)) === '["Vert"]' &&
        r.ink === '#30A46C' &&
        r.encre === '#30A46C' &&
        r.rempli === 'rgb(48, 164, 108)' &&
        saut &&
        !r.saute,
      '10 Vert : couleur de la Flèche, nuance enfoncée, corps de l’instrument vert (il saute)',
      JSON.stringify([saut, r]),
    );
    const autres = {};
    for (const t of ['texte', 'numero', 'gomme', 'flou']) autres[t] = (await outilVu(p, t)).ink;
    verifier(
      JSON.stringify(autres) === '{"texte":"#E5484D","numero":"#E5484D","gomme":"","flou":""}',
      '10 les outils repliés gardent un trait de leur couleur (aucun sans couleur)',
      JSON.stringify(autres),
    );
    await glisser(p, [20, 20], [120, 40]);
    // La flèche tracée reste choisie : « Bleu » la recolore.
    await reglage(p, 'Bleu');
    const o = (await objets(p, 'fleche'))[0];
    verifier(o.couleur === '#0090FF', '10 flèche choisie : sa couleur change', JSON.stringify(o));
    await p.$eval('#autre-couleur', (i) => {
      i.value = '#8e4ec6';
      i.dispatchEvent(new Event('input', { bubbles: true }));
      i.dispatchEvent(new Event('change', { bubbles: true }));
    });
    await uneImage(p);
    const r2 = await p.evaluate(() => {
      const w = document.querySelector('#outils .pli[data-on] .sw.roue');
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
    const nuancesGomme = (await p.$$('#outils .pli[data-on] .sw')).length;
    await outil(p, 'Surligneur');
    const surl = await enfoncee(p);
    verifier(
      nuancesGomme === 0 && JSON.stringify(surl) === '["Jaune"]',
      '10 Gomme : aucune couleur ; Surligneur : Jaune',
      JSON.stringify([nuancesGomme, surl]),
    );
    // Encre très foncée ou très pâle : un liseré plus marqué la détache de la barre.
    await outil(p, 'Stylo');
    await reglage(p, 'Noir');
    await reglage(p, 'Épais');
    const noir = await outilVu(p, 'stylo');
    await reglage(p, 'Blanc');
    const blanc = await outilVu(p, 'stylo');
    verifier(
      noir.ton === 'f' &&
        noir.fonce &&
        !noir.pale &&
        noir.t === '2' &&
        blanc.ton === 'p' &&
        blanc.pale &&
        !blanc.fonce,
      '10 Stylo noir puis blanc : liseré marqué ; Épais : pointe large',
      JSON.stringify([noir, blanc]),
    );
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
    await outil(p, 'Texte');
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
      const bague = document.querySelector('[data-outil="fleche"] .inst .i-bague');
      return {
        barre: cs('#barre-haut').backgroundColor,
        scene: cs('#fenetre').backgroundColor,
        copier: cs('[data-act="copier"]').backgroundColor,
        corps: getComputedStyle(bague).fill,
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
            'rgb(245, 246, 248)',
            '#1A1A1A',
          ]
        : [
            'rgb(35, 35, 35)',
            'rgb(23, 23, 23)',
            'rgb(76, 194, 255)',
            'rgb(211, 215, 221)',
            '#FFFFFF',
          ];
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

  // ---------- 17. largeur d'ouverture, fenêtre étroite : seuls les outils défilent ----------
  {
    // À la largeur d'ouverture du cœur (jeton editeur-ouverture-largeur, LARGEUR_OUVERTURE),
    // chaque outil déplié tient dans la barre sans défiler.
    const largeur = Number.parseFloat(
      fs.readFileSync(path.join(RACINE, 'commun', 'jetons.css'), 'utf8').match(/--editeur-ouverture-largeur:\s*([\d.]+)px/)[1],
    );
    const p = await page({ largeur, hauteur: 600 });
    const tiennent = [];
    for (const n of ['Flèche', 'Stylo', 'Surligneur', 'Gomme', 'Texte', 'Forme', 'Numéro', 'Flou', 'Projecteur']) {
      await outil(p, n);
      await p
        .waitForFunction(() => document.getAnimations().every((a) => a.playState !== 'running'), null, {
          timeout: 3000,
        })
        .catch(() => {});
      const m = await p.evaluate(() => {
        const o = document.querySelector('#outils');
        const b = document.querySelector('#palette').getBoundingClientRect();
        return { defile: o.scrollWidth - o.clientWidth, g: b.left, d: b.right, suite: o.dataset.suite || '' };
      });
      tiennent.push([n, m.defile <= 0 && m.g >= 0 && m.d <= largeur && m.suite === '']);
    }
    verifier(
      largeur === 1096 && tiennent.every(([, ok]) => ok),
      `17 ${largeur} px (largeur d'ouverture) : chaque outil déplié tient dans la barre sans défiler`,
      JSON.stringify(tiennent),
    );
    await fin(p, '17 ouverture');
  }
  {
    const p = await page({ largeur: 640, hauteur: 440 });
    const mesure = () =>
      p.evaluate(() => {
        const pal = document.querySelector('#palette');
        const o = document.querySelector('#outils');
        const b = pal.getBoundingClientRect();
        const zo = o.getBoundingClientRect();
        // Annuler, Rétablir et Plus : en entier dans la barre, hors de la zone qui défile.
        const fixes = ['[data-act="annuler"]', '[data-act="retablir"]', '#bouton-plus'].map((s) => {
          const r = document.querySelector(s).getBoundingClientRect();
          return r.left >= b.left && r.right <= b.right && (r.right <= zo.left || r.left >= zo.right);
        });
        const cs = getComputedStyle(pal);
        return {
          h: b.height,
          g: b.left,
          d: b.right,
          l: innerWidth,
          defile: o.scrollWidth - o.clientWidth,
          x: o.scrollLeft,
          suite: o.dataset.suite || '',
          fixes,
          // Le fond et l'ombre de la barre ne sont jamais estompés.
          masque: cs.maskImage || cs.webkitMaskImage || 'none',
          ombre: cs.boxShadow !== 'none',
        };
      });
    const r = await mesure();
    verifier(
      r.h === 80 &&
        r.g >= 0 &&
        r.d <= r.l &&
        r.defile > 0 &&
        r.x === 0 &&
        r.suite === 'fin' &&
        r.fixes.every(Boolean) &&
        r.masque === 'none' &&
        r.ombre,
      '17 640 × 440 : la barre tient dans la fenêtre, seuls les outils défilent, estompés du côté caché ; Annuler, Rétablir, Plus en entier ; fond et ombre gardés',
      JSON.stringify(r),
    );
    // La molette, sur Annuler (hors de la zone qui défile), fait défiler les outils.
    const [cx, cy] = await p.$eval('[data-act="annuler"]', (e) => {
      const b = e.getBoundingClientRect();
      return [b.left + b.width / 2, b.top + b.height / 2];
    });
    await p.mouse.move(cx, cy);
    await p.mouse.wheel(0, 120);
    await p
      .waitForFunction(() => document.querySelector('#outils').scrollLeft > 0, null, {
        timeout: 2000,
      })
      .catch(() => {});
    await uneImage(p);
    const r1 = await mesure();
    verifier(
      r1.x > 0 && r1.suite === 'deux' && r1.fixes.every(Boolean) && r1.masque === 'none',
      '17 la molette fait défiler les outils, estompés des deux côtés ; Annuler, Rétablir, Plus restent',
      JSON.stringify(r1),
    );
    // L'outil pris se montre en entier, son pli compris ; plus large que la zone, son début.
    const vuDans = (t) =>
      p.evaluate((t) => {
        const o = document.querySelector('#outils').getBoundingClientRect();
        const x = document.querySelector(`#outils .pli[data-pli="${t}"]`).getBoundingClientRect();
        const b = document.querySelector(`#outils [data-outil="${t}"]`).getBoundingClientRect();
        return { zone: [o.left, o.right], pli: [x.left, x.right], bouton: [b.left, b.right] };
      }, t);
    // Fin des transitions, puis du défilement doux (absent de getAnimations) : la position
    // ne bouge plus pendant cinq images.
    const calme = async () => {
      await p
        .waitForFunction(() => document.getAnimations().every((a) => a.playState !== 'running'), null, {
          timeout: 3000,
        })
        .catch(() => {});
      await p.evaluate(
        () =>
          new Promise((ok) => {
            const o = document.querySelector('#outils');
            let x = o.scrollLeft;
            let n = 0;
            let tours = 0;
            const tour = () => {
              n = o.scrollLeft === x ? n + 1 : 0;
              x = o.scrollLeft;
              if (n >= 5 || ++tours > 180) ok();
              else requestAnimationFrame(tour);
            };
            requestAnimationFrame(tour);
          }),
      );
    };
    await p.mouse.move(cx, 20);
    // Flou (pli plus étroit que la zone) : en entier, au bout des outils.
    await outil(p, 'Flou');
    await calme();
    const vu = await vuDans('flou');
    await reglage(p, 'Bloc noir');
    verifier(
      vu.pli[0] >= vu.zone[0] &&
        vu.pli[1] <= vu.zone[1] &&
        (await p.evaluate(() => __ed.prefs.variantes.flou)) === 'bloc',
      '17 Flou pris : son pli se montre en entier parmi les outils ; Bloc noir choisi',
      JSON.stringify(vu),
    );
    // Numéro (pli plus large que la zone à 640 px) : son début, le bouton de l'outil compris.
    await outil(p, 'Numéro');
    await calme();
    const vn = await vuDans('numero');
    await reglage(p, 'Noir');
    verifier(
      vn.pli[1] - vn.pli[0] > vn.zone[1] - vn.zone[0] &&
        vn.bouton[0] >= vn.zone[0] &&
        vn.bouton[1] <= vn.zone[1] &&
        (await p.evaluate(() => __ed.prefs.couleurs.numero)) === '#1A1A1A',
      '17 Numéro pris (pli plus large que la zone) : son début se montre, bouton compris ; Noir choisi',
      JSON.stringify(vn),
    );
    await outil(p, 'Flèche');
    await calme();
    const vf = await vuDans('fleche');
    verifier(
      vf.bouton[0] >= vf.zone[0] && vf.bouton[1] <= vf.zone[1],
      '17 Flèche reprise : son bouton se montre (le pli commence par lui)',
      JSON.stringify(vf),
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
    await reglage(p, 'Vert');
    await reglage(p, 'Épais');
    await outil(p, 'Stylo');
    const r = await p.evaluate(() => ({
      presse: document
        .querySelector('#outils .pli[data-on] .sw[aria-pressed="true"]')
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
    await outil(p, 'Forme');
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
    await outil(p, 'Projecteur');
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
    await texteHaut(p).click();
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
        (await texteHaut(p).count()) === 1,
      '21 « Texte » (infobulle « Copier le texte ») entre Fermer et Enregistrer',
      JSON.stringify(haut),
    );
    // R7 : un Bloc noir posé ; l'image lue est l'image composée, la vue du document.
    await outil(p, 'Flou');
    await variante(p, 'Flou', 'Bloc noir');
    await glisser(p, [30, 70], [200, 110]);
    await vider(p);
    const t0 = await p.evaluate(() => performance.now());
    await texteHaut(p).click();
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
    await texteHaut(p).click();
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
      await texteHaut(p).click();
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
    for (const sel of ['#barre-haut [data-act="copier"]', '#outils .tl.ip', '#outils .sz']) {
      await p.keyboard.press('Shift');
      await p.$eval(sel, (b) => b.focus());
      anneaux.push(
        await p.$eval(sel, (b) => {
          const s = getComputedStyle(b);
          return [b.matches(':focus-visible'), s.outlineStyle, s.outlineWidth];
        }),
      );
    }
    // L'instrument dessiné : un anneau intérieur (son masque coupe ce qui dépasse).
    await p.keyboard.press('Shift');
    await p.$eval('#outils .tl.ib', (b) => b.focus());
    const anneauInstrument = await p.$eval('#outils .tl.ib', (b) => {
      const s = getComputedStyle(b);
      return [b.matches(':focus-visible'), s.outlineStyle, /inset/.test(s.boxShadow)];
    });
    verifier(
      JSON.stringify(anneaux) ===
        '[[true,"solid","2px"],[true,"solid","2px"],[true,"solid","2px"]]' &&
        JSON.stringify(anneauInstrument) === '[true,"none",true]',
      '21 anneau de focus sur « Copier » du haut, un outil, un réglage et un instrument',
      JSON.stringify([anneaux, anneauInstrument]),
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

  // ---------- 22. gestes revus : Échap, Maj, trait redressé, Ctrl + A, couleur, bornes ----------
  {
    const p = await page();
    const hist = (q) =>
      q.evaluate(() => ({ pas: __ed.h.passe.length, futur: __ed.h.futur.length }));
    await outil(p, 'Forme');
    await glisser(p, [100, 100], [200, 180]);
    await outil(p, 'Flèche');
    await glisser(p, [20, 250], [120, 250]);
    await p.keyboard.press('Control+z');
    await uneImage(p);
    const h0 = await hist(p);

    // Échap pendant un déplacement : l'objet revient, aucun pas, « Rétablir » gardé.
    const [ax, ay] = await point(p, 100, 140);
    await p.mouse.move(ax, ay);
    await p.mouse.down();
    await p.mouse.move(ax + 20, ay + 10);
    await p.mouse.move(ax + 40, ay + 20);
    await uneImage(p);
    const enRoute = await p.evaluate(() => Math.round(__ed.h.doc.objets[0].x1));
    await p.keyboard.press('Escape');
    await uneImage(p);
    const revenu = await p.evaluate(() => __ed.h.doc.objets[0].x1);
    await p.mouse.move(ax + 60, ay + 30);
    await p.mouse.up();
    await uneImage(p);
    const h1 = await hist(p);
    verifier(
      enRoute === 140 && revenu === 100 && h1.pas === h0.pas && h1.futur === 1 &&
        (await p.evaluate(() => __ed.h.doc.objets[0].x1)) === 100,
      '22 Échap pendant un déplacement : retour au départ, aucun pas, Rétablir gardé',
      JSON.stringify([enRoute, revenu, h0, h1]),
    );

    // Échap pendant un tracé : rien n'est créé.
    await outil(p, 'Stylo');
    const [sx, sy] = await point(p, 250, 40);
    await p.mouse.move(sx, sy);
    await p.mouse.down();
    await p.mouse.move(sx + 30, sy + 10);
    await p.mouse.move(sx + 60, sy + 5);
    await p.keyboard.press('Escape');
    await p.mouse.up();
    await uneImage(p);
    const h2 = await hist(p);
    verifier(
      (await objets(p, 'crayon')).length === 0 && h2.pas === h0.pas && h2.futur === 1,
      '22 Échap pendant un tracé : rien de créé, aucun pas',
      JSON.stringify(h2),
    );

    // Stylo avec Maj : un segment droit, par 15°.
    const droit = (o) => {
      const [a, b] = [o.points[0], o.points[o.points.length - 1]];
      const L = Math.hypot(b[0] - a[0], b[1] - a[1]);
      const ecart = Math.max(
        ...o.points.map(
          (q) => Math.abs((b[0] - a[0]) * (a[1] - q[1]) - (a[0] - q[0]) * (b[1] - a[1])) / L,
        ),
      );
      return { L, ecart, angle: (Math.atan2(b[1] - a[1], b[0] - a[0]) * 180) / Math.PI };
    };
    await p.mouse.move(sx, sy);
    await p.mouse.down();
    await p.keyboard.down('Shift');
    await p.mouse.move(sx + 40, sy + 14);
    await p.mouse.move(sx + 90, sy + 20);
    await p.mouse.up();
    await p.keyboard.up('Shift');
    await uneImage(p);
    let d = droit((await objets(p, 'crayon'))[0]);
    verifier(
      d.ecart < 0.01 && Math.abs(d.angle - 15) < 0.01 && d.L > 80,
      '22 Stylo avec Maj : segment droit, à 15°',
      JSON.stringify(d),
    );

    // Surligneur tenu immobile en fin de trait : presque droit, il se redresse (ici sur l'axe).
    await outil(p, 'Surligneur');
    const [ux, uy] = await point(p, 250, 120);
    await p.mouse.move(ux, uy);
    await p.mouse.down();
    await p.mouse.move(ux + 30, uy + 2);
    await p.mouse.move(ux + 60, uy - 1);
    await p.mouse.move(ux + 100, uy + 3);
    await p
      .waitForFunction(() => __ed.enCours && __ed.enCours.points.length === 2, null, {
        timeout: 2000,
      })
      .catch(() => {});
    await p.mouse.up();
    await uneImage(p);
    d = droit((await objets(p, 'surligneur'))[0]);
    verifier(
      d.ecart < 0.01 && Math.abs(d.angle) < 0.01 && d.L > 95,
      '22 Surligneur tenu immobile : trait redressé, calé à l’horizontale',
      JSON.stringify(d),
    );
    // Un trait qui tourne n'est pas redressé.
    await p.mouse.move(ux, uy + 60);
    await p.mouse.down();
    await p.mouse.move(ux + 40, uy + 60);
    await p.mouse.move(ux + 60, uy + 100);
    await p.mouse.move(ux + 100, uy + 100);
    await sansAppel(p, 'jamais', {});
    const courbe = await p.evaluate(() => __ed.enCours.points.length);
    await p.mouse.up();
    await uneImage(p);
    verifier(courbe > 2, '22 un trait qui tourne reste à main levée', String(courbe));

    // Maj sur la poignée d'une flèche : par 15°.
    await outil(p, 'Flèche');
    await glisser(p, [20, 220], [120, 220]);
    await cliquer(p, 70, 220);
    const [px, py] = await point(p, 120, 220);
    await p.mouse.move(px, py);
    await p.mouse.down();
    await p.keyboard.down('Shift');
    await p.mouse.move(px - 5, py + 20);
    await p.mouse.move(px - 10, py + 40);
    await p.mouse.up();
    await p.keyboard.up('Shift');
    await uneImage(p);
    const f = (await objets(p, 'fleche')).pop();
    const angle = (Math.atan2(f.y2 - f.y1, f.x2 - f.x1) * 180) / Math.PI;
    verifier(
      f.x1 === 20 && f.y1 === 220 && Math.abs(angle / 15 - Math.round(angle / 15)) < 1e-6 &&
        Math.abs(angle - 30) < 8,
      '22 Maj sur la poignée d’une flèche : bout calé par 15°',
      JSON.stringify([f, angle]),
    );

    // La couleur choisie va à l'objet choisi et à SON outil, pas à l'outil en main.
    await p.keyboard.press('Escape');
    await cliquer(p, 290, 120);
    const choisiAvant = await p.evaluate(() => __ed.selection.map((i) => __ed.objet(i).type));
    await reglage(p, 'Bleu');
    const c = await p.evaluate(() => ({
      o: __ed.h.doc.objets.find((o) => o.type === 'surligneur').couleur,
      surligneur: __ed.prefs.couleurs.surligneur,
      fleche: __ed.prefs.couleurs.fleche,
    }));
    verifier(
      JSON.stringify(choisiAvant) === '["surligneur"]' &&
        JSON.stringify(c) === '{"o":"#0090FF","surligneur":"#0090FF","fleche":"#E5484D"}',
      '22 couleur : pour l’objet choisi et son outil (le Surligneur), pas pour la Flèche en main',
      JSON.stringify([choisiAvant, c]),
    );

    // Ctrl + A : tout est choisi.
    await p.keyboard.press('Escape');
    await p.keyboard.press('Control+a');
    await uneImage(p);
    const tout = await p.evaluate(() => [__ed.selection.length, __ed.h.doc.objets.length]);
    verifier(tout[0] === tout[1] && tout[0] >= 5, '22 Ctrl + A : tout est choisi', String(tout));
    await p.keyboard.press('Escape');

    // Un objet tiré hors de l'image y garde 12 px : il reste attrapable.
    await cliquer(p, 100, 140);
    const [bx, by] = await point(p, 100, 140);
    await p.mouse.move(bx, by);
    await p.mouse.down();
    await p.mouse.move(bx + 300, by);
    await p.mouse.move(bx + 900, by);
    await p.mouse.up();
    await uneImage(p);
    const x1 = await p.evaluate(() => __ed.h.doc.objets[0].x1);
    verifier(
      x1 > 380 && x1 <= 388.5,
      '22 objet tiré hors de l’image : 12 px y restent',
      String(x1),
    );
    await fin(p, '22');
  }

  await nav.close();
  srv.close();
  console.log(`OK : ${reussis}`);
  console.log(`ECHECS : ${echecs}`);
  process.exit(echecs ? 1 : 0);
})();
