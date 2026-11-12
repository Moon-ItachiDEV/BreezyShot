const { chromium } = require('playwright');
const http = require('http');
const fs = require('fs');
const path = require('path');
// Essai de la page de l'éditeur d'annotation dans Chromium (contrat 0.4 « Éditeur », §12.2).
// Sort avec 1 si une vérification échoue. Copies d'écran dans $SORTIE.
//   node essais/pages/editeur.cjs
// Le cœur est simulé (`__TAURI__` qui note les appels, rend des octets connus et garde le
// corps et les en-têtes des exports). Deux parties :
// - « page » : document.js et dessin.js sont remplacés par un modèle simulé, simple et
//   connu, qui note chaque appel : on prouve la page (barre, gestes et leurs arguments,
//   clavier, zoom, toiles, sorties, fermeture), indépendamment du chantier B ;
// - « assemblage » : les vrais document.js et dessin.js ; les points qui ne se prouvent
//   qu'avec leurs corps (pixels de l'export) ne sont joués que si ces corps ne sont plus
//   provisoires, sinon ils sont dits EN ATTENTE (jamais comptés comme réussis).
const RACINE = path.join(__dirname, '..', '..', 'app', 'interface');
const SORTIE = process.env.SORTIE || path.join(__dirname, '..', '..', 'essais-resultats', 'pages');
fs.mkdirSync(SORTIE, { recursive: true });
const CONF = path.join(__dirname, '..', '..', 'app', 'coeur', 'tauri.conf.json');
const CSP = Object.entries(JSON.parse(fs.readFileSync(CONF, 'utf8')).app.security.csp)
  .map(([k, v]) => `${k} ${v}`)
  .join('; ');
const types = { '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript' };
const ED = path.join(RACINE, 'editeur');
const B_PROVISOIRE = ['document.js', 'dessin.js'].some((f) =>
  fs.readFileSync(path.join(ED, f), 'utf8').includes('Provisoire (B)'),
);

// ---------- modèle simulé (servi à la place de document.js et dessin.js) ----------
const DOUBLE_DOCUMENT = String.raw`
const note = (nom, ...a) => (window.__modele = window.__modele || []).push([nom, JSON.parse(
  JSON.stringify(a.map((x) => (x && x.doc && x.passe ? '#h' : x && x.objets ? '#doc' : x))))]);
export function nouveauDocument(largeur, hauteur) {
  return { largeur, hauteur, objets: [], recadrage: null, suivant: 1 };
}
export function creerHistorique(doc) {
  return { doc, passe: [], futur: [], version: 0, marque: null };
}
export function modifier(h, fn, marque = null) {
  note('modifier', marque);
  if (!(marque !== null && marque === h.marque)) {
    h.passe.push(JSON.stringify(h.doc));
    if (h.passe.length > 200) h.passe.shift();
  }
  h.futur = [];
  h.marque = marque;
  fn(h.doc);
  h.version += 1;
}
export function annuler(h) {
  note('annuler');
  if (!h.passe.length) return;
  h.futur.push(JSON.stringify(h.doc));
  h.doc = JSON.parse(h.passe.pop());
  h.marque = null;
  h.version += 1;
}
export function retablir(h) {
  note('retablir');
  if (!h.futur.length) return;
  h.passe.push(JSON.stringify(h.doc));
  h.doc = JSON.parse(h.futur.pop());
  h.marque = null;
  h.version += 1;
}
export const peutAnnuler = (h) => h.passe.length > 0;
export const peutRetablir = (h) => h.futur.length > 0;
export function boite(o) {
  if (o.points) {
    const xs = o.points.map((p) => p[0]), ys = o.points.map((p) => p[1]);
    return { x: Math.min(...xs), y: Math.min(...ys), largeur: Math.max(...xs) - Math.min(...xs),
      hauteur: Math.max(...ys) - Math.min(...ys) };
  }
  if (o.type === 'texte') return { x: o.x, y: o.y, largeur: o.texte.length * o.taille * 0.6,
    hauteur: o.taille * 1.2 };
  if (o.type === 'numero') return { x: o.x - o.rayon, y: o.y - o.rayon, largeur: 2 * o.rayon,
    hauteur: 2 * o.rayon };
  return { x: Math.min(o.x1, o.x2), y: Math.min(o.y1, o.y2), largeur: Math.abs(o.x2 - o.x1),
    hauteur: Math.abs(o.y2 - o.y1) };
}
export function touche(o, p, t) {
  const b = boite(o);
  return p.x >= b.x - t && p.x <= b.x + b.largeur + t && p.y >= b.y - t
    && p.y <= b.y + b.hauteur + t;
}
export function objetSous(doc, p, t, filtre) {
  note('objetSous', p, t, filtre);
  for (let i = doc.objets.length - 1; i >= 0; i--) {
    const o = doc.objets[i];
    if (filtre && o.type !== filtre) continue;
    if (touche(o, p, t)) return o;
  }
  return null;
}
export function poignees(o) {
  if (o.type === 'fleche') return [[o.x1, o.y1, 'p1'], [o.x2, o.y2, 'p2'],
    [(o.x1 + o.x2) / 2, (o.y1 + o.y2) / 2, 'pc']];
  if (o.type === 'ligne') return [[o.x1, o.y1, 'p1'], [o.x2, o.y2, 'p2']];
  const b = boite(o);
  return [[b.x, b.y, 'nw'], [b.x + b.largeur, b.y, 'ne'], [b.x, b.y + b.hauteur, 'sw'],
    [b.x + b.largeur, b.y + b.hauteur, 'se']];
}
export function deplacer(o, q, dx, dy) {
  note('deplacer', o.id, dx, dy);
  if (q.points) o.points = q.points.map((p) => [p[0] + dx, p[1] + dy]);
  if ('x1' in q) Object.assign(o, { x1: q.x1 + dx, y1: q.y1 + dy, x2: q.x2 + dx, y2: q.y2 + dy });
  if ('x' in q) Object.assign(o, { x: q.x + dx, y: q.y + dy });
}
export function redimensionner(o, q, nom, p, echelleVue) {
  note('redimensionner', o.id, nom, p, echelleVue);
  if (nom === 'se') Object.assign(o, { x2: p.x, y2: p.y });
  if (nom === 'nw') Object.assign(o, { x1: p.x, y1: p.y });
  if (nom === 'p2') Object.assign(o, { x2: p.x, y2: p.y });
}
export function dupliquer(doc, ids) {
  note('dupliquer', ids);
  const r = [];
  for (const id of ids) {
    const o = JSON.parse(JSON.stringify(doc.objets.find((x) => x.id === id)));
    o.id = doc.suivant++;
    deplacer(o, JSON.parse(JSON.stringify(o)), 16, 16);
    doc.objets.push(o);
    r.push(o.id);
  }
  return r;
}
export function supprimer(doc, ids) {
  note('supprimer', ids);
  doc.objets = doc.objets.filter((o) => !ids.includes(o.id));
}
export function ordonner(doc, ids, devant) {
  note('ordonner', ids, devant);
  const a = doc.objets.filter((o) => ids.includes(o.id));
  const b = doc.objets.filter((o) => !ids.includes(o.id));
  doc.objets = devant ? b.concat(a) : a.concat(b);
}
export const numeroAffiche = () => 1;
export function ajouter(doc, o) {
  note('ajouter', o.type);
  o.id = doc.suivant++;
  if (o.type === 'numero' && o.ordre == null) o.ordre = 1 + Math.max(0, ...doc.objets
    .filter((x) => x.type === 'numero').map((x) => x.ordre));
  doc.objets.push(o);
  return o.id;
}
export function normaliser(o) {
  note('normaliser', o.type);
  if (!['rectangle', 'ellipse', 'masque'].includes(o.type)) return;
  const [x1, x2] = [o.x1, o.x2].sort((a, b) => a - b);
  const [y1, y2] = [o.y1, o.y2].sort((a, b) => a - b);
  Object.assign(o, { x1, x2, y1, y2 });
}
export function lisser(p) {
  note('lisser', p.length);
  return p.flatMap((q, i) => (i ? [[(q[0] + p[i - 1][0]) / 2, (q[1] + p[i - 1][1]) / 2], q] : [q]));
}
export function recadrer(doc, r) {
  note('recadrer', r);
  doc.recadrage = r ? { x: Math.round(r.x), y: Math.round(r.y), largeur: Math.round(r.largeur),
    hauteur: Math.round(r.hauteur) } : null;
}
export const POLICE = 'sans-serif';
export const policeTexte = (o) => (o.gras ? '700 ' : '500 ') + o.taille + 'px ' + POLICE;
export const mesureTexte = (o) => ({ largeur: String(o.texte).length * o.taille * 0.6,
  hauteur: o.taille * 1.2, px: 0, py: 0, lignes: String(o.texte).split('\n') });
export const cadre = (doc) => doc.recadrage ?? { x: 0, y: 0, largeur: doc.largeur,
  hauteur: doc.hauteur };
export const vide = (doc) => doc.objets.length === 0 && doc.recadrage === null;
`;
const DOUBLE_DESSIN = String.raw`
const note = (nom, ...a) => (window.__dessin = window.__dessin || []).push([nom, a]);
export const COULEURS = [["#E5484D","Rouge"],["#F5A524","Orange"],["#FFD43B","Jaune"],
  ["#30A46C","Vert"],["#0090FF","Bleu"],["#8E4EC6","Violet"],["#1A1A1A","Noir"],
  ["#FFFFFF","Blanc"]];
export const TRAITS = [3, 5, 8];
export const TRAITS_SURLIGNEUR = [16, 24, 34];
export const RAYONS_NUMERO = [12, 16, 21];
export const BLOCS = [8, 13, 20];
export const FLOUS = [6, 10, 16];
export const COULEURS_PERSO_MAX = 3;
export function dessinerObjet(ctx, doc, o) {
  note('dessinerObjet', o.type);
  ctx.fillStyle = o.couleur || '#000';
  if ('x1' in o) ctx.fillRect(o.x1, o.y1, o.x2 - o.x1, o.y2 - o.y1);
}
export function peindreMasques(ctx, base, doc) {
  note('peindreMasques', doc.objets.filter((o) => o.type === 'masque').length);
  ctx.drawImage(base, 0, 0);
  ctx.fillStyle = '#000';
  for (const o of doc.objets) if (o.type === 'masque') ctx.fillRect(o.x1, o.y1, o.x2 - o.x1,
    o.y2 - o.y1);
}
const cadre = (doc) => doc.recadrage ?? { x: 0, y: 0, largeur: doc.largeur, hauteur: doc.hauteur };
export function composer(doc, fond) {
  note('composer');
  const c = cadre(doc);
  const t = document.createElement('canvas');
  t.width = c.largeur; t.height = c.hauteur;
  const ctx = t.getContext('2d', { willReadFrequently: true });
  ctx.drawImage(fond, -c.x, -c.y);
  for (const o of doc.objets) if (o.type !== 'masque') {
    ctx.setTransform(1, 0, 0, 1, -c.x, -c.y); dessinerObjet(ctx, doc, o);
  }
  const rgba = ctx.getImageData(0, 0, c.largeur, c.hauteur).data;
  t.width = 0;
  return { largeur: c.largeur, hauteur: c.hauteur, rgba };
}
export function peindreVue(ctx, doc, fond, vue, selection, options = {}) {
  note('peindreVue', { ...vue }, selection.slice(), options.ajout ? options.ajout.type : null,
    options.cacher || null);
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);
  const c = cadre(doc);
  ctx.imageSmoothingEnabled = vue.echelle < 1;
  ctx.imageSmoothingQuality = 'high';
  ctx.setTransform(vue.echelle, 0, 0, vue.echelle, vue.x, vue.y);
  ctx.drawImage(fond, c.x, c.y, c.largeur, c.hauteur, c.x, c.y, c.largeur, c.hauteur);
  for (const o of doc.objets) if (o.type !== 'masque' && o.id !== options.cacher) {
    dessinerObjet(ctx, doc, o);
  }
  if (options.ajout && options.ajout.type !== 'masque') dessinerObjet(ctx, doc, options.ajout);
  ctx.setTransform(1, 0, 0, 1, 0, 0);
}
`;

function serveur(doubles) {
  return http.createServer((q, r) => {
    const nom = decodeURIComponent(new URL(q.url, 'http://x').pathname);
    const entete = { 'content-security-policy': CSP };
    if (doubles && nom === '/editeur/document.js') {
      r.writeHead(200, { ...entete, 'content-type': 'text/javascript' });
      return r.end(DOUBLE_DOCUMENT);
    }
    if (doubles && nom === '/editeur/dessin.js') {
      r.writeHead(200, { ...entete, 'content-type': 'text/javascript' });
      return r.end(DOUBLE_DESSIN);
    }
    const p = path.join(RACINE, nom);
    if (!p.startsWith(RACINE) || !fs.existsSync(p)) {
      r.writeHead(404);
      return r.end();
    }
    r.writeHead(200, { ...entete, 'content-type': types[path.extname(p)] || 'text/plain' });
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
          window.__exports.push({
            numero,
            entetes: opt.headers,
            corps: Array.from(a.slice(0, 64)),
            longueur: a.byteLength,
            somme: a.reduce((s, v) => (s + v) % 1000000007, 0),
          });
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
  const srvPage = serveur(true);
  const srvReel = serveur(false);
  await new Promise((ok) => srvPage.listen(0, ok));
  await new Promise((ok) => srvReel.listen(0, ok));
  const url = (s) => `http://127.0.0.1:${s.address().port}/editeur/editeur.html`;
  const nav = await chromium.launch();
  let echecs = 0;
  const attente = [];
  const verifier = (ok, quoi, info = '') => {
    console.log(ok ? 'OK   ' : 'ECHEC', quoi, ok ? '' : info);
    if (!ok) echecs++;
  };

  async function page(o = {}) {
    const ctx = await nav.newContext({
      viewport: { width: o.largeur || 1200, height: o.hauteur || 800 },
      deviceScaleFactor: o.echelle || 1,
      colorScheme: o.theme || 'light',
      reducedMotion: o.reduit ? 'reduce' : 'no-preference',
    });
    const p = await ctx.newPage();
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
    await p.goto(url(o.reel ? srvReel : srvPage));
    await p
      .waitForFunction(() => window.__appels.some((a) => a[0] === 'editeur_affiche'), null, {
        timeout: 5000,
      })
      .catch(() => {});
    return p;
  }
  const appels = (p, nom) => p.evaluate((n) => __appels.filter((a) => !n || a[0] === n), nom);
  const vider = (p) =>
    p.evaluate(() => {
      __appels.length = 0;
      window.__modele = [];
      window.__dessin = [];
    });
  const modele = (p, nom) =>
    p.evaluate((n) => (window.__modele || []).filter((a) => a[0] === n), nom);
  const docu = (p) => p.evaluate(() => window.__ed.h.doc);
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
  const image = (p) =>
    p.evaluate(() => {
      const v = window.__ed.vue;
      return { ...v };
    });
  // Pixels de l'image → point CSS de la page.
  const point = async (p, x, y) => {
    const v = await image(p);
    return [(v.x + x * v.echelle) / v.dpr, (v.y + y * v.echelle) / v.dpr];
  };
  const glisser = async (p, a, b, opt = {}) => {
    const [x1, y1] = await point(p, a[0], a[1]);
    const [x2, y2] = await point(p, b[0], b[1]);
    if (opt.maj) await p.keyboard.down('Shift');
    await p.mouse.move(x1, y1);
    await p.mouse.down();
    for (let i = 1; i <= 6; i++)
      await p.mouse.move(x1 + ((x2 - x1) * i) / 6, y1 + ((y2 - y1) * i) / 6);
    await p.mouse.up();
    if (opt.maj) await p.keyboard.up('Shift');
    await uneImage(p);
  };
  const cliquer = async (p, x, y) => {
    const [a, b] = await point(p, x, y);
    await p.mouse.click(a, b);
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
    const peint = await p.evaluate(() =>
      (window.__dessin || []).some((d) => d[0] === 'peindreVue'),
    );
    verifier(
      affiches.length === 1 && JSON.stringify(affiches[0][1]) === '{"id":7}' && peint,
      '1 editeur_affiche une fois, après le premier dessin',
      JSON.stringify(affiches),
    );
    await attendreAppel(p, 'editeur_vue');
    const vue = await appels(p, 'editeur_vue');
    verifier(
      vue.length === 1 && vue[0][1].id === 7,
      '1 editeur_vue à la fin du placement',
      JSON.stringify(vue),
    );
    await p.context().close();
  }

  // Raccourci d'outil puis attente d'une image.
  const touche = async (p, k) => {
    await p.keyboard.press(k);
    await uneImage(p);
  };
  const outilChoisi = (p) =>
    p.$$eval('#outils [aria-pressed="true"]', (bs) => bs.map((b) => b.getAttribute('aria-label')));
  // Une variante par le menu du chevron (l'outil choisi d'abord, puis son menu).
  const variante = async (p, g, v) => {
    const b = `[data-outil="${g}"]`;
    if ((await p.$eval(b, (x) => x.getAttribute('aria-pressed'))) !== 'true') await p.click(b);
    if (!(await p.$eval('#menu-variantes', (m) => m.classList.contains('ouvert'))))
      await p.click(b);
    await p.click(`#menu-variantes [data-variante="${v}"]`);
  };
  const derniers = async (p, type) => (await docu(p)).objets.filter((o) => o.type === type);

  // ---------- 2. barre : outils, noms, touches ; rien des outils reportés ----------
  {
    const p = await page();
    const outils = await p.$$eval('[role="toolbar"][aria-label="Outils"] #outils button', (bs) =>
      bs.map((b) => [b.getAttribute('aria-label'), b.dataset.key || '']),
    );
    verifier(
      JSON.stringify(outils) ===
        JSON.stringify([
          ['Sélection', 'V'],
          ['Flèche', 'A'],
          ['Formes', 'R'],
          ['Dessin', 'P'],
          ['Texte', 'T'],
          ['Numéros', 'N'],
          ['Masquer', 'B'],
          ['Recadrer', 'C'],
        ]),
      '2 outils dans l’ordre du §0, noms et touches',
      JSON.stringify(outils),
    );
    const barres = await p.$$eval('[role="toolbar"]', (bs) =>
      bs.map((b) => b.getAttribute('aria-label')),
    );
    verifier(
      JSON.stringify(barres) === '["Outils","Zoom","Actions"]',
      '2 trois barres nommées',
      JSON.stringify(barres),
    );
    const tout = await p.evaluate(() => document.documentElement.outerHTML);
    const interdits = [
      'Projecteur',
      'Loupe',
      'Fond',
      'Épingler',
      'Mise en avant',
      'i-spot',
      'i-mag',
      'i-pin',
      'i-bg',
      'draggable',
    ];
    const trouves = interdits.filter((m) => tout.includes(m));
    verifier(
      trouves.length === 0,
      '2 aucun Projecteur, Loupe, Fond, Épingler, draggable',
      JSON.stringify(trouves),
    );
    verifier((await p.title()) === 'Annoter la capture', '2 titre « Annoter la capture »');
    const plus = await p.$$eval('#menu-plus [role="menuitem"]', (m) =>
      m.map((x) => x.textContent.trim()),
    );
    const ctxm = await p.$$eval('#menu-objet [role="menuitem"]', (m) =>
      m.map((x) => x.textContent.trim()),
    );
    verifier(
      JSON.stringify(plus) === '["Enregistrer sous…","Glisser"]' &&
        JSON.stringify(ctxm) === '["Dupliquer","Premier plan","Arrière-plan","Supprimer"]' &&
        (await p.$eval('#menu-objet', (m) => m.getAttribute('aria-label'))) === 'Forme',
      '2 menus Plus et Forme, textes exacts',
      JSON.stringify([plus, ctxm]),
    );
    // Variantes et rapports du recadrage.
    await p.click('[data-outil="formes"]');
    await p.click('[data-outil="formes"]');
    const vf = await p.$$eval('#menu-variantes [role="menuitemradio"]', (m) =>
      m.map((x) => x.textContent),
    );
    await p.click('[data-outil="recadrer"]');
    await p.click('[data-outil="recadrer"]');
    const vr = await p.$$eval('#menu-variantes .mi', (m) => m.map((x) => x.textContent));
    verifier(
      JSON.stringify(vf) === '["Rectangle","Ellipse","Ligne"]' &&
        JSON.stringify(vr) === '["Libre","1:1","4:3","16:9","Image entière"]',
      '2 variantes Formes et Recadrer',
      JSON.stringify([vf, vr]),
    );
    verifier(
      JSON.stringify(await outilChoisi(p)) === '["Recadrer"]',
      '2 défaut puis Recadrer choisi',
    );
    await p.context().close();
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
    const r = await p.evaluate(
      ({ alpha }) => {
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
        return {
          e: e.entetes,
          longueur: a.byteLength,
          attendu: o.length,
          ecarts,
          alphaFaux,
          alpha,
        };
      },
      { alpha },
    );
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
      `3 ${l}×${h} : en-têtes de l’export`,
      JSON.stringify(r.e),
    );
    await p.context().close();
  }

  // ---------- 4 et 5. gestes de chaque outil : le modèle reçoit les bons objets ----------
  {
    const p = await page();
    await p.keyboard.press('Control+1');
    await uneImage(p);
    verifier((await image(p)).echelle === 1, '4 Ctrl+1 : taille réelle');
    await touche(p, 'r');
    await vider(p);
    await glisser(p, [100, 100], [300, 200]);
    const rect = await derniers(p, 'rectangle');
    const mods = await modele(p, 'modifier');
    const sansId =
      rect.length && Object.fromEntries(Object.entries(rect[0]).filter(([k]) => k !== 'id'));
    verifier(
      rect.length === 1 &&
        JSON.stringify(sansId) ===
          JSON.stringify({
            type: 'rectangle',
            x1: 100,
            y1: 100,
            x2: 300,
            y2: 200,
            couleur: '#E5484D',
            epaisseur: 1,
            plein: false,
            arrondi: 10,
            opacite: 1,
          }) &&
        mods.length === 1,
      '4 rectangle (100,100)-(300,200) : objet, défauts du §3.3, un pas',
      JSON.stringify([rect, mods]),
    );
    verifier(
      JSON.stringify(await p.evaluate(() => __ed.selection)) === JSON.stringify([rect[0].id]),
      '4 le rectangle posé est choisi',
    );
    await glisser(p, [320, 10], [324, 14]);
    verifier((await derniers(p, 'rectangle')).length === 1, '4 rectangle de 4 px : rien de créé');
    await glisser(p, [20, 20], [120, 60], { maj: true });
    const carre = (await derniers(p, 'rectangle'))[1];
    verifier(
      carre && carre.x2 - carre.x1 === 100 && carre.y2 - carre.y1 === 100,
      '4 Maj : carré',
      JSON.stringify(carre),
    );
    await glisser(p, [300, 280], [150, 250]);
    const n = (await derniers(p, 'rectangle'))[2];
    verifier(
      n && n.x1 === 150 && n.y1 === 250 && n.x2 === 300 && n.y2 === 280,
      '4 rectangle tracé à rebours : normalisé',
      JSON.stringify(n),
    );

    await touche(p, 'a');
    await glisser(p, [10, 150], [110, 160], { maj: true });
    const f = (await derniers(p, 'fleche'))[0];
    const angle = f && (Math.atan2(f.y2 - f.y1, f.x2 - f.x1) * 180) / Math.PI;
    verifier(
      f &&
        f.cx === undefined &&
        f.double === false &&
        Math.abs(angle % 15) < 1e-6 &&
        f.epaisseur === 1 &&
        f.opacite === 1,
      '5 Flèche Droite, Maj : 15°',
      JSON.stringify(f),
    );
    await glisser(p, [10, 200], [15, 203]);
    verifier((await derniers(p, 'fleche')).length === 1, '5 flèche de 6 px : rien de créé');
    await variante(p, 'fleche', 'courbe');
    await glisser(p, [10, 220], [200, 220]);
    const fc = (await derniers(p, 'fleche'))[1];
    verifier(
      fc && typeof fc.cx === 'number' && typeof fc.cy === 'number' && !('courbe' in fc),
      '5 Flèche Courbe : point de contrôle',
      JSON.stringify(fc),
    );
    await variante(p, 'fleche', 'double');
    await glisser(p, [10, 240], [200, 240]);
    verifier((await derniers(p, 'fleche'))[2]?.double === true, '5 Flèche Double');
    await touche(p, 'l');
    await glisser(p, [210, 10], [390, 20]);
    const li = (await derniers(p, 'ligne'))[0];
    verifier(
      li && li.x1 === 210 && li.x2 === 390 && li.couleur === '#E5484D',
      '5 Ligne (L)',
      JSON.stringify(li),
    );
    await touche(p, 'e');
    await glisser(p, [210, 30], [260, 60]);
    verifier((await derniers(p, 'ellipse')).length === 1, '5 Ellipse (E)');
    await touche(p, 'p');
    await glisser(p, [20, 270], [200, 290]);
    const cr = (await derniers(p, 'crayon'))[0];
    verifier(
      cr && cr.points.length > 7 && cr.opacite === 1,
      '5 Crayon (P) lissé',
      JSON.stringify(cr && cr.points.length),
    );
    await touche(p, 'h');
    await glisser(p, [20, 295], [200, 295]);
    const su = (await derniers(p, 'surligneur'))[0];
    verifier(
      su && su.couleur === '#FFD43B' && !('opacite' in su),
      '5 Surligneur (H) : Jaune, sans opacité',
      JSON.stringify(su),
    );
    await touche(p, 'n');
    await cliquer(p, 350, 100);
    await cliquer(p, 350, 150);
    const nums = await derniers(p, 'numero');
    verifier(
      nums.length === 2 && nums[0].ordre === 1 && nums[1].ordre === 2 && nums[0].rayon === 16,
      '5 Numéros (N) : un clic, ordre 1 puis 2, rayon M',
      JSON.stringify(nums),
    );
    await vider(p);
    await touche(p, 'b');
    await glisser(p, [330, 200], [390, 260]);
    const ms = await derniers(p, 'masque');
    const des = await p.evaluate(() => window.__dessin || []);
    const peints = des.filter((d) => d[0] === 'peindreMasques').length;
    const ajouts = des.filter((d) => d[0] === 'peindreVue' && d[1][2] === 'masque').length;
    verifier(
      ms.length === 1 &&
        ms[0].sorte === 'pixeliser' &&
        ms[0].intensite === 1 &&
        peints === 1 &&
        ajouts >= 2,
      '5 Pixeliser (B) : aperçu par peindreVue (ajout), fond repeint une fois',
      JSON.stringify([ms, peints, ajouts]),
    );
    await vider(p);
    await p.evaluate(() => __ed.dessiner());
    await uneImage(p);
    const repeint = await p.evaluate(
      () => (window.__dessin || []).filter((d) => d[0] === 'peindreMasques').length,
    );
    verifier(repeint === 0, '5 fond non repeint sans changement de masque', String(repeint));
    for (const [v, sorte, y] of [
      ['flouter', 'flouter', 30],
      ['bloc', 'bloc', 70],
    ]) {
      await variante(p, 'masquer', v);
      await glisser(p, [330, y], [380, y + 30]);
      verifier((await derniers(p, 'masque')).pop()?.sorte === sorte, `5 Masquer ${v}`);
    }
    await touche(p, 'c');
    await glisser(p, [50.4, 40.4], [250.6, 190.6]);
    const d = await docu(p);
    verifier(
      JSON.stringify(d.recadrage) === '{"x":50,"y":40,"largeur":201,"hauteur":151}',
      '5 Recadrer (C) : recadrage entier',
      JSON.stringify(d.recadrage),
    );
    verifier(
      (await p.$eval('#zoom-ajuster', (b) => b.getAttribute('aria-pressed'))) === 'true',
      '5 recadrage : zoom à Ajuster',
    );
    await p.click('[data-outil="recadrer"]');
    await p.click('[data-act="image_entiere"]');
    verifier((await docu(p)).recadrage === null, '5 Image entière : recadrage retiré');
    verifier(p.erreurs.length === 0, '5 aucune erreur de console', JSON.stringify(p.erreurs));
    await p.context().close();
  }

  // ---------- 6. sélection, déplacement, poignées, menu, clavier sur la sélection ----------
  {
    const p = await page();
    await p.keyboard.press('Control+1');
    await touche(p, 'r');
    await glisser(p, [100, 100], [200, 160]);
    await glisser(p, [250, 100], [300, 160]);
    const [r1, r2] = await derniers(p, 'rectangle');
    await touche(p, 'v');
    await cliquer(p, 150, 100);
    verifier(
      JSON.stringify(await p.evaluate(() => __ed.selection)) === JSON.stringify([r1.id]),
      '6 clic : sélection',
    );
    await uneImage(p);
    const pv = await p.evaluate(() => window.__dessin.filter((x) => x[0] === 'peindreVue').pop());
    verifier(
      JSON.stringify(pv[1][1]) === JSON.stringify([r1.id]) &&
        Number.isInteger(pv[1][0].x) &&
        pv[1][0].echelle === 1 &&
        pv[1][0].dpr === 1,
      '6 peindreVue reçoit vue et sélection',
      JSON.stringify(pv),
    );
    const [kx, ky] = await point(p, 275, 100);
    await p.keyboard.down('Shift');
    await p.mouse.click(kx, ky);
    await p.keyboard.up('Shift');
    verifier((await p.evaluate(() => __ed.selection)).length === 2, '6 Maj+clic ajoute');
    await vider(p);
    await glisser(p, [150, 100], [170, 130]);
    const mods = await modele(p, 'modifier');
    const marques = new Set(mods.map((m) => m[1][0]));
    const [a1, a2] = await derniers(p, 'rectangle');
    verifier(
      mods.length > 1 &&
        marques.size === 1 &&
        [...marques][0] !== null &&
        a1.x1 === 120 &&
        a1.y1 === 130 &&
        a2.x1 === 270,
      '6 déplacer deux objets : un seul pas',
      JSON.stringify([mods.length, [...marques], a1, a2]),
    );
    await p.keyboard.press('Control+z');
    const apres = await derniers(p, 'rectangle');
    verifier(
      apres[0].x1 === 100 && apres[1].x1 === 250,
      '6 Ctrl+Z : les deux reviennent',
      JSON.stringify(apres),
    );
    await p.keyboard.press('Escape');
    await cliquer(p, 150, 100);
    await vider(p);
    await glisser(p, [200, 160], [240, 200]);
    const red = await modele(p, 'redimensionner');
    verifier(
      red.length > 0 && red[0][1][1] === 'se' && red[0][1][3] === 1,
      '6 poignée se : redimensionner, échelle de la vue',
      JSON.stringify(red[0]),
    );
    await vider(p);
    for (let i = 0; i < 3; i++) await p.keyboard.press('ArrowRight');
    await p.keyboard.press('Shift+ArrowDown');
    const pou = await modele(p, 'modifier');
    const o = (await derniers(p, 'rectangle'))[0];
    verifier(
      new Set(pou.map((m) => m[1][0])).size === 1 && o.x1 === 103 && o.y1 === 110,
      '6 flèches 1 px, Maj 10 px, un pas',
      JSON.stringify([pou, o]),
    );
    await p.keyboard.press('Control+d');
    verifier(
      (await derniers(p, 'rectangle')).length === 3 && (await modele(p, 'dupliquer')).length === 1,
      '6 Ctrl+D : dupliquer',
    );
    const [cx, cy] = await point(p, 150, 130);
    await p.mouse.click(cx, cy, { button: 'right' });
    await uneImage(p);
    verifier(
      await p.$eval('#menu-objet', (m) => m.classList.contains('ouvert')),
      '6 menu Forme ouvert',
    );
    await p.getByRole('menuitem', { name: 'Premier plan' }).click();
    const ord = await modele(p, 'ordonner');
    verifier(
      ord.length === 1 && ord[0][1][1] === true,
      '6 Premier plan → ordonner(devant)',
      JSON.stringify(ord),
    );
    await p.keyboard.press('Delete');
    verifier((await derniers(p, 'rectangle')).length === 2, '6 Suppr : supprimé');
    await cliquer(p, 275, 100);
    await p.keyboard.press('Backspace');
    verifier((await derniers(p, 'rectangle')).length === 1, '6 Retour arrière : supprimé');
    await glisser(p, [10, 10], [399, 299]);
    verifier((await p.evaluate(() => __ed.selection)).length === 1, '6 cadre de sélection');
    await p.keyboard.press('Escape');
    verifier(
      (await p.evaluate(() => __ed.selection)).length === 0 &&
        (await appels(p, 'editeur_fermer')).length === 0,
      '6 Échap : sélection vidée, rien fermé',
    );
    await p.context().close();
  }

  // ---------- 7. annuler, rétablir ----------
  {
    const p = await page();
    const etat = () =>
      p.$$eval('[data-act="annuler"],[data-act="retablir"]', (b) => b.map((x) => x.disabled));
    verifier(
      JSON.stringify(await etat()) === '[true,true]',
      '7 au départ : Annuler et Rétablir grisés',
    );
    await touche(p, 'r');
    await glisser(p, [50, 50], [150, 150]);
    verifier(JSON.stringify(await etat()) === '[false,true]', '7 après un tracé : Annuler actif');
    await vider(p);
    await p.keyboard.press('Control+z');
    verifier(
      (await modele(p, 'annuler')).length === 1 &&
        (await derniers(p, 'rectangle')).length === 0 &&
        JSON.stringify(await etat()) === '[true,false]',
      '7 Ctrl+Z : annuler',
    );
    await p.keyboard.press('Control+y');
    verifier((await derniers(p, 'rectangle')).length === 1, '7 Ctrl+Y : rétablir');
    await p.keyboard.press('Control+z');
    await p.keyboard.press('Control+Shift+z');
    verifier(
      (await modele(p, 'retablir')).length === 2 && (await derniers(p, 'rectangle')).length === 1,
      '7 Ctrl+Maj+Z : rétablir',
    );
    await p.click('[data-act="annuler"]');
    verifier((await derniers(p, 'rectangle')).length === 0, '7 bouton Annuler');
    await p.click('[data-act="retablir"]');
    verifier((await derniers(p, 'rectangle')).length === 1, '7 bouton Rétablir');
    const v = await p.evaluate(() => __ed.h.version);
    await attendreAppel(p, 'editeur_etat', { version: v });
    const et = (await appels(p, 'editeur_etat')).pop();
    verifier(
      et && et[1].id === 7 && et[1].version === v && et[1].vide === false,
      '7 editeur_etat : dernière version, non vide',
      JSON.stringify([et, v]),
    );
    await p.context().close();
  }
  // Règles du modèle (relecture de B) : pas d'annuler pendant un geste ; une marque par
  // geste ; fond repeint après annuler ; recadrage entier au bon rapport.
  {
    const p = await page();
    await p.keyboard.press('Control+1');
    await touche(p, 'r');
    await glisser(p, [50, 50], [150, 150]);
    await touche(p, 'v');
    const [ax, ay] = await point(p, 100, 50);
    await vider(p);
    await p.mouse.move(ax, ay);
    await p.mouse.down();
    await p.mouse.move(ax + 20, ay + 20);
    await p.keyboard.press('Control+z');
    await p.mouse.up();
    const pendant = await modele(p, 'annuler');
    await glisser(p, [120, 70], [140, 90]);
    const marques = [...new Set((await modele(p, 'modifier')).map((m) => m[1][0]))];
    verifier(
      pendant.length === 0 && marques.length === 2 && marques[0] !== marques[1],
      '7 annuler ignoré pendant un geste ; deux glissés : deux marques',
      JSON.stringify([pendant, marques]),
    );
    await p.keyboard.press('Control+z');
    const r = (await derniers(p, 'rectangle'))[0];
    verifier(
      r.x1 === 70 && r.y1 === 70,
      '7 deux glissés : deux pas d’annulation',
      JSON.stringify(r),
    );
    await touche(p, 'b');
    await glisser(p, [200, 200], [260, 260]);
    await vider(p);
    await p.keyboard.press('Control+z');
    await p.keyboard.press('Control+y');
    await uneImage(p);
    const fonds = await p.evaluate(
      () => (window.__dessin || []).filter((d) => d[0] === 'peindreMasques').length,
    );
    verifier(fonds === 2, '7 fond repeint après annuler et rétablir', String(fonds));
    for (const [nom, a, b] of [
      ['4:3', 4, 3],
      ['16:9', 16, 9],
      ['1:1', 1, 1],
    ]) {
      await p.click('[data-act="annuler"]').catch(() => {});
      await p.keyboard.press('c');
      await p.click('[data-outil="recadrer"]');
      await p.getByRole('menuitemradio', { name: nom }).click();
      await vider(p);
      await glisser(p, [380.3, 290.2], [10.6, 5.1]);
      const k = (await modele(p, 'recadrer')).pop();
      const c = k && k[1][0];
      verifier(
        c &&
          [c.x, c.y, c.largeur, c.hauteur].every(Number.isInteger) &&
          c.largeur * b === c.hauteur * a &&
          c.x >= 0 &&
          c.y >= 0 &&
          c.x + c.largeur <= 400 &&
          c.y + c.hauteur <= 300,
        `7 Recadrer ${nom} : cadre entier, dans l’image, au rapport`,
        JSON.stringify(c),
      );
      await p.keyboard.press('Control+z');
    }
    await p.context().close();
  }

  // ---------- 8. texte ----------
  {
    const p = await page();
    await p.keyboard.press('Control+1');
    await touche(p, 't');
    await cliquer(p, 100, 100);
    verifier(
      await p.$eval('#saisie', (s) => !s.hidden && document.activeElement === s),
      '8 clic Texte : zone de saisie en place, focus',
    );
    await p.keyboard.type('Ab rv');
    verifier(
      JSON.stringify(await outilChoisi(p)) === '["Texte"]',
      '8 aucune touche d’outil pendant la saisie',
    );
    await p.keyboard.press('Shift+Enter');
    await p.keyboard.type('x');
    await p.keyboard.press('Enter');
    const t = await derniers(p, 'texte');
    verifier(
      t.length === 1 &&
        t[0].texte === 'Ab rv\nx' &&
        t[0].taille === 28 &&
        t[0].gras === true &&
        t[0].style === 'contour' &&
        t[0].couleur === '#E5484D',
      '8 Entrée garde, Maj+Entrée à la ligne',
      JSON.stringify(t),
    );
    await cliquer(p, 300, 200);
    await p.keyboard.type('perdu');
    await p.keyboard.press('Escape');
    verifier(
      (await derniers(p, 'texte')).length === 1 && (await appels(p, 'editeur_fermer')).length === 0,
      '8 Échap abandonne (rien fermé)',
    );
    await cliquer(p, 300, 200);
    await p.keyboard.press('Enter');
    verifier((await derniers(p, 'texte')).length === 1, '8 texte vide : rien de gardé');
    await touche(p, 'v');
    const [dx, dy] = await point(p, t[0].x + 5, t[0].y + 5);
    await p.mouse.dblclick(dx, dy);
    await uneImage(p);
    verifier(
      await p.$eval('#saisie', (s) => !s.hidden && s.value === 'Ab rv\nx'),
      '8 double-clic rouvre',
    );
    await p.keyboard.press('Control+a');
    await p.keyboard.press('Delete');
    await p.keyboard.press('Enter');
    verifier((await derniers(p, 'texte')).length === 0, '8 texte vidé : supprimé');
    await p.context().close();
  }

  // ---------- 9. réglages de l'outil (§3.3) ----------
  {
    const p = await page();
    const lignes = () =>
      p.$$eval('#reglages-outil > .row', (rs) =>
        rs.map((r) => {
          const g =
            r.getAttribute('role') === 'radiogroup'
              ? r
              : r.querySelector('[role="radiogroup"]') || r.querySelector('input[type="range"]');
          const vis = r.querySelector('.lab');
          return (
            (g ? g.getAttribute('aria-label') : '?') +
            (vis ? '' : '*') +
            (r.querySelector('.gbtn') ? '+G' : '')
          );
        }),
      );
    const ouvrir = async () => {
      if (!(await p.$eval('#reglages-outil', (x) => x.classList.contains('ouvert')))) {
        await p.click('#bouton-reglages');
      }
    };
    const attendu = {
      // « * » : sans libellé visible (la ligne des couleurs n'en a pas, comme le design).
      a: ['Couleur*', 'Épaisseur', 'Opacité'],
      l: ['Couleur*', 'Épaisseur', 'Opacité'],
      p: ['Couleur*', 'Épaisseur', 'Opacité'],
      r: ['Couleur*', 'Épaisseur', 'Style', 'Arrondi', 'Opacité'],
      e: ['Couleur*', 'Épaisseur', 'Style', 'Opacité'],
      h: ['Couleur*', 'Épaisseur'],
      t: ['Couleur*', 'Taille*', 'Style*+G', 'Opacité*'],
      n: ['Couleur*', 'Taille', 'Opacité'],
      b: ['Intensité'],
    };
    for (const [k, l] of Object.entries(attendu)) {
      await p.evaluate(() => __ed.barre.fermerMenus());
      await touche(p, k);
      await ouvrir();
      const vu = await lignes();
      verifier(
        JSON.stringify(vu) === JSON.stringify(l),
        `9 réglages de « ${k} »`,
        JSON.stringify(vu),
      );
    }
    const couleurs = await p.evaluate(() => {
      __ed.barre.fermerMenus();
      return 0;
    });
    void couleurs;
    await touche(p, 'r');
    await ouvrir();
    const sw = await p.$$eval(
      '#reglages-outil [role="radiogroup"][aria-label="Couleur"] > *',
      (b) => b.map((x) => x.getAttribute('aria-label') || x.dataset.tip),
    );
    verifier(
      JSON.stringify(sw) ===
        JSON.stringify([
          'Rouge',
          'Orange',
          'Jaune',
          'Vert',
          'Bleu',
          'Violet',
          'Noir',
          'Blanc',
          'Autre couleur',
          'Pipette',
        ]),
      '9 ligne des couleurs',
      JSON.stringify(sw),
    );
    await p.keyboard.press('Escape');
    await variante(p, 'masquer', 'bloc');
    const bloc = await p.$eval('#bouton-reglages', (b) => b.disabled);
    await touche(p, 'c');
    const rec = await p.$eval('#bouton-reglages', (b) => b.disabled);
    await touche(p, 'v');
    const sel = await p.$eval('#bouton-reglages', (b) => b.disabled);
    verifier(bloc && rec && sel, '9 pastille inactive : Bloc noir, Recadrer, sélection vide');
    // Un changement sur un objet choisi : un pas ; une glissière tirée : un pas.
    await touche(p, 'r');
    await glisser(p, [50, 50], [150, 150]);
    await ouvrir();
    await vider(p);
    await p.click('#reglages-outil [aria-label="Bleu"]');
    let r = (await derniers(p, 'rectangle'))[0];
    verifier(
      r.couleur === '#0090FF' && (await modele(p, 'modifier')).length === 1,
      '9 couleur sur l’objet choisi : un pas',
      JSON.stringify(r),
    );
    await vider(p);
    const curseur = await p.$('#reglages-outil input[data-reglage="opacite"]');
    const b = await curseur.boundingBox();
    await p.mouse.move(b.x + b.width - 2, b.y + b.height / 2);
    await p.mouse.down();
    for (let i = 1; i <= 30; i++) await p.mouse.move(b.x + b.width - 2 - i * 3, b.y + b.height / 2);
    await p.mouse.up();
    r = (await derniers(p, 'rectangle'))[0];
    const mods = await modele(p, 'modifier');
    verifier(
      mods.length > 3 && new Set(mods.map((m) => m[1][0])).size === 1 && r.opacite < 1,
      '9 glissière Opacité tirée : un seul pas',
      JSON.stringify([mods.length, r.opacite]),
    );
    await p.click('#reglages-outil [aria-label="Plein"]');
    verifier((await derniers(p, 'rectangle'))[0].plein === true, '9 Style Plein');
    // Pipette : I, la couleur du pixel affiché ; Échap sort.
    await p.keyboard.press('Escape');
    await p.keyboard.press('i');
    verifier((await p.$eval('#scene', (s) => s.dataset.curseur)) === 'pick', '9 I : pipette');
    await p.keyboard.press('Escape');
    verifier(
      (await p.$eval('#scene', (s) => s.dataset.curseur)) !== 'pick' &&
        (await appels(p, 'editeur_fermer')).length === 0,
      '9 Échap : sort de la pipette',
    );
    await p.keyboard.press('i');
    await cliquer(p, 300, 250);
    const pris = await p.evaluate(() => __ed.prefs.couleur);
    verifier(
      /^#[0-9A-F]{6}$/.test(pris) && pris !== '#0090FF',
      '9 clic pipette : couleur prise',
      pris,
    );
    await p.context().close();
  }

  // ---------- 10. clavier du §5, raccourcis du navigateur ----------
  {
    const p = await page();
    for (const [k, nom] of [
      ['v', 'Sélection'],
      ['a', 'Flèche'],
      ['r', 'Formes'],
      ['e', 'Formes'],
      ['l', 'Formes'],
      ['p', 'Dessin'],
      ['h', 'Dessin'],
      ['t', 'Texte'],
      ['n', 'Numéros'],
      ['b', 'Masquer'],
      ['c', 'Recadrer'],
    ]) {
      await touche(p, k);
      verifier(
        JSON.stringify(await outilChoisi(p)) === JSON.stringify([nom]),
        `10 touche ${k.toUpperCase()}`,
      );
    }
    await p.evaluate(() => {
      window.__empeche = {};
      window.addEventListener('keydown', (e) => {
        if (e.key === 'Control' || e.key === 'Shift') return;
        window.__empeche[(e.ctrlKey ? 'Ctrl+' : '') + (e.shiftKey ? 'Maj+' : '') + e.key] =
          e.defaultPrevented;
      });
      window.__marque = 1;
    });
    for (const k of [
      'F5',
      'Control+r',
      'Control+p',
      'Control+f',
      'Control+u',
      'F7',
      'Control+s',
      'Control+Shift+s',
      'Control+0',
      'Control+Equal',
      'Control+Minus',
    ])
      await p.keyboard.press(k);
    const emp = await p.evaluate(() => window.__empeche);
    const nonEmp = Object.entries(emp)
      .filter(([, v]) => !v)
      .map(([k]) => k);
    verifier(
      nonEmp.length === 0 &&
        Object.keys(emp).length >= 10 &&
        (await p.evaluate(() => window.__marque === 1)),
      '10 F5, Ctrl+R, P, F, U, F7, S, 0, =, − : empêchés, page non rechargée',
      JSON.stringify(emp),
    );
    const ctxm = await p.evaluate(() => {
      const e = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
      document.querySelector('.pill').dispatchEvent(e);
      return e.defaultPrevented;
    });
    const roue = await p.evaluate(() => {
      const e = new WheelEvent('wheel', {
        bubbles: true,
        cancelable: true,
        ctrlKey: true,
        deltaY: -100,
      });
      document.body.dispatchEvent(e);
      return e.defaultPrevented;
    });
    verifier(ctxm && roue, '10 menu contextuel natif et Ctrl+molette du navigateur empêchés');
    // Ctrl+molette sur la scène : zoom à la pointe, le point sous la pointe ne bouge pas.
    await p.keyboard.press('Control+1');
    const v0 = await image(p);
    const [mx, my] = await point(p, 200, 150);
    await p.mouse.move(mx, my);
    await p.keyboard.down('Control');
    await p.mouse.wheel(0, -200);
    await p.keyboard.up('Control');
    await uneImage(p);
    const v1 = await image(p);
    const [nx, ny] = await point(p, 200, 150);
    verifier(
      v1.echelle > v0.echelle &&
        Math.abs(nx - mx) <= 1 &&
        Math.abs(ny - my) <= 1 &&
        (await p.evaluate(() => window.visualViewport.scale === 1 && devicePixelRatio === 1)),
      '10 Ctrl+molette : zoom à la pointe, pas celui du navigateur',
      JSON.stringify([v0, v1]),
    );
    await p.context().close();
  }

  // ---------- 11. zoom : Ajuster, Taille réelle, bornes, étiquette ----------
  {
    const p = await page({ charge: { largeur: 2400, hauteur: 1600 } });
    const etiquette = () => p.$eval('#etiquette-zoom', (e) => e.textContent);
    const v = await image(p);
    verifier(
      v.echelle < 1 &&
        (await p.$eval('#zoom-ajuster', (b) => b.getAttribute('aria-pressed'))) === 'true',
      '11 grande image : Ajuster < 100 %',
      JSON.stringify(v),
    );
    await p.click('#zoom-reel');
    verifier(
      (await image(p)).echelle === 1 &&
        (await etiquette()) === '100 %' &&
        (await p.$eval('#zoom-reel', (b) => b.getAttribute('aria-pressed'))) === 'true',
      '11 Taille réelle : 100 %, bouton enfoncé',
    );
    await p.click('#zoom-reel');
    verifier(
      (await image(p)).echelle === v.echelle,
      '11 Taille réelle déjà active : retour à Ajuster',
    );
    await p.keyboard.press('Control+1');
    await p.keyboard.press('Control+Equal');
    verifier(
      Math.abs((await image(p)).echelle - 1.25) < 1e-9 && (await etiquette()) === '125 %',
      '11 Ctrl+= : × 1,25',
      await etiquette(),
    );
    for (let i = 0; i < 30; i++) await p.keyboard.press('Control+Minus');
    verifier((await etiquette()) === '10 %', '11 borne basse 10 %', await etiquette());
    for (let i = 0; i < 30; i++) await p.keyboard.press('Control+Equal');
    verifier((await etiquette()) === '800 %', '11 borne haute 800 %', await etiquette());
    await p.keyboard.press('Control+0');
    verifier((await image(p)).echelle === v.echelle, '11 Ctrl+0 : Ajuster');
    await vider(p);
    await p.keyboard.press('Control+1');
    await attendreAppel(p, 'editeur_vue', { zoom: 1 });
    const v1 = (await appels(p, 'editeur_vue')).pop();
    verifier(v1 && v1[1].zoom === 1, '11 Ctrl+1 : editeur_vue zoom 1', JSON.stringify(v1));
    await vider(p);
    await p.keyboard.press('Control+0');
    await attendreAppel(p, 'editeur_vue');
    const ev = (await appels(p, 'editeur_vue')).pop();
    verifier(
      ev && Math.abs(ev[1].zoom - v.echelle) < 1e-9 && ev[1].largeur > 0,
      '11 editeur_vue après le zoom',
      JSON.stringify(ev),
    );
    // Espace tenu, le focus sur la scène : déplacer la vue.
    await p.keyboard.press('Control+1');
    await p.focus('#scene');
    const a = await image(p);
    await p.keyboard.down(' ');
    await p.mouse.move(500, 400);
    await p.mouse.down();
    await p.mouse.move(540, 420);
    await p.mouse.up();
    await p.keyboard.up(' ');
    const b = await image(p);
    verifier(
      b.x - a.x === 40 && b.y - a.y === 20 && (await docu(p)).objets.length === 0,
      '11 Espace tenu : la vue suit, rien de tracé',
      JSON.stringify([a, b]),
    );
    await p.mouse.move(500, 400);
    await p.mouse.down({ button: 'middle' });
    await p.mouse.move(490, 400);
    await p.mouse.up({ button: 'middle' });
    verifier((await image(p)).x - b.x === -10, '11 bouton du milieu : la vue suit');
    await p.context().close();
  }

  // ---------- 12. devicePixelRatio : un pixel de l'image par pixel de l'écran ----------
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
      let ecarts = 0;
      for (let i = 0; i < o.length; i++) if (d[i] !== o[i]) ecarts++;
      return {
        l: t.width,
        h: t.height,
        cl: t.clientWidth,
        ch: t.clientHeight,
        dpr: devicePixelRatio,
        v,
        ecarts,
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
        r.z === '100 %',
      `12 dpr ${echelle} : toile au pixel de l’écran, Taille réelle pixel pour pixel`,
      JSON.stringify(r),
    );
    await p.keyboard.press('Control+0');
    const aj = await image(p);
    verifier(aj.echelle <= 1, `12 dpr ${echelle} : Ajuster ≤ 100 %`);
    await p.context().close();
  }

  // ---------- 13. fermeture : dialogue ----------
  {
    const p = await page();
    await touche(p, 'r');
    await glisser(p, [50, 50], [150, 150]);
    await p.keyboard.press('Escape');
    await p.keyboard.press('Escape');
    await uneImage(p);
    const f = (await appels(p, 'editeur_fermer')).pop();
    const prefs = JSON.parse(f[1].preferences);
    verifier(
      f &&
        f[1].id === 7 &&
        f[1].force === false &&
        f[1].vide === false &&
        prefs.outil === 'formes' &&
        prefs.variantes.formes === 'rectangle',
      '13 Échap : editeur_fermer, préférences de l’outil',
      JSON.stringify(f),
    );
    const dlg = await p.evaluate(() => {
      const d = document.querySelector('[role="alertdialog"]');
      return {
        vu: getComputedStyle(document.querySelector('#voile')).visibility,
        focus: document.activeElement.textContent,
        titre: d.textContent.replace(/\s+/g, ' ').trim(),
      };
    });
    verifier(
      dlg.vu === 'visible' &&
        dlg.focus === 'Copier et fermer' &&
        dlg.titre === 'Fermer sans garder ? Copier et fermer Ne pas garder Continuer',
      '13 « demander » : dialogue, focus sur Copier et fermer, textes exacts',
      JSON.stringify(dlg),
    );
    const focus = () => p.evaluate(() => document.activeElement.textContent);
    await p.keyboard.press('Tab');
    const f1 = await focus();
    await p.keyboard.press('Tab');
    await p.keyboard.press('Tab');
    const f3 = await focus();
    await p.keyboard.press('Shift+Tab');
    verifier(
      f1 === 'Ne pas garder' && f3 === 'Copier et fermer' && (await focus()) === 'Continuer',
      '13 Tab boucle dans le dialogue',
    );
    await touche(p, 'v');
    verifier(
      JSON.stringify(await outilChoisi(p)) === '["Formes"]',
      '13 aucune touche d’outil sous le dialogue',
    );
    await p.keyboard.press('Escape');
    await uneImage(p);
    verifier(
      (await p.$eval('#voile', (v) => !v.classList.contains('ouvert'))) &&
        (await appels(p, 'editeur_garder')).length === 1,
      '13 Échap = Continuer (editeur_garder)',
    );
    await p.evaluate(() => __emettre('editeur:fermer_demande', { id: 7, raison: 'cadre' }));
    await uneImage(p);
    verifier(
      (await appels(p, 'editeur_fermer')).length === 2 &&
        (await p.$eval('#voile', (v) => v.classList.contains('ouvert'))),
      '13 fermer_demande (cadre) : dialogue',
    );
    await p.getByRole('button', { name: 'Ne pas garder' }).click();
    await uneImage(p);
    const g = (await appels(p, 'editeur_fermer')).pop();
    verifier(
      g[1].force === true && (await p.evaluate(() => __ed.fond === null)),
      '13 Ne pas garder : force, toiles relâchées',
      JSON.stringify(g),
    );
    await p.context().close();
  }
  {
    const p = await page();
    await touche(p, 'r');
    await glisser(p, [50, 50], [150, 150]);
    await p.keyboard.press('Escape');
    await p.keyboard.press('Escape');
    await uneImage(p);
    await vider(p);
    await p.getByRole('button', { name: 'Copier et fermer' }).click();
    await p.waitForFunction(() => window.__exports.length === 1);
    const e = await p.evaluate(() => window.__exports[0].entetes['x-editeur-but']);
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
    verifier(
      e === 'copier_fermer' &&
        (await p.$eval('#message', (m) => m.textContent)) === 'Impossible de copier' &&
        (await appels(p, 'editeur_fermer')).length === 0,
      '13 Copier et fermer en échec : la fenêtre reste, message',
    );
    await p.keyboard.press('Escape');
    await uneImage(p);
    await p.getByRole('button', { name: 'Copier et fermer' }).click();
    await p.waitForFunction(() => window.__exports.length === 2);
    await p.evaluate(() =>
      __emettre('editeur:resultat', {
        id: 7,
        numero: 2,
        but: 'copier_fermer',
        etat: 'fait',
        message: null,
      }),
    );
    await uneImage(p);
    const f = (await appels(p, 'editeur_fermer')).pop();
    verifier(
      f && f[1].force === false,
      '13 Copier et fermer réussi : editeur_fermer',
      JSON.stringify(f),
    );
    await p.context().close();
  }
  {
    const p = await page({ reponseFermer: 'fermee' });
    await p.keyboard.press('Escape');
    await uneImage(p);
    const f = (await appels(p, 'editeur_fermer')).pop();
    verifier(
      f &&
        f[1].vide === true &&
        f[1].force === false &&
        (await p.$eval('#voile', (v) => !v.classList.contains('ouvert'))),
      '13 vide : fermée sans dialogue',
    );
    await p.context().close();
  }
  // Attendu du cœur (rapport de A) : réponse à fermer_demande tout de suite (< 2 s) ;
  // « fermee » suivie d'un editeur:charger (capture en attente) : la page se recharge.
  {
    const p = await page({ reponseFermer: 'fermee' });
    await touche(p, 'r');
    await glisser(p, [50, 50], [150, 150]);
    await vider(p);
    const t0 = Date.now();
    await p.evaluate(() => {
      __TAURI__.core.invoke = ((f) => (c, a, o) => {
        const r = f(c, a, o);
        if (c === 'editeur_fermer') {
          window.__charge = { ...window.__charge, id: 9 };
          __emettre('editeur:charger', window.__charge);
        }
        return r;
      })(__TAURI__.core.invoke);
      __emettre('editeur:fermer_demande', { id: 7, raison: 'remplacer' });
    });
    await p.waitForFunction(
      () => __appels.some((a) => a[0] === 'editeur_affiche' && a[1].id === 9),
      null,
      { timeout: 2000 },
    );
    const r = await p.evaluate(() => ({
      f: __appels.filter((a) => a[0] === 'editeur_fermer').map((a) => a[1].id),
      img: __appels.filter((a) => a[0] === 'editeur_image').map((a) => a[1].id),
      fond: !!__ed.fond,
      n: __ed.h.doc.objets.length,
    }));
    verifier(
      Date.now() - t0 < 2000 && JSON.stringify(r) === '{"f":[7],"img":[9],"fond":true,"n":0}',
      '13 fermer_demande : editeur_fermer aussitôt ; « fermee » puis charger : nouveau document',
      JSON.stringify(r),
    );
    await touche(p, 'r');
    await vider(p);
    await glisser(p, [60, 60], [160, 160]);
    // Au plus 250 ms après la modification (marge de 150 ms pour l'essai).
    await p.waitForFunction(() => __appels.some((a) => a[0] === 'editeur_etat'), null, {
      timeout: 400,
    });
    const et = (await appels(p, 'editeur_etat')).pop();
    verifier(
      et && et[1].id === 9 && et[1].vide === false,
      '13 editeur_etat après la modification',
      JSON.stringify(et),
    );
    await p.context().close();
  }

  // ---------- 14. résultats, messages, remplacement (horloge de Playwright) ----------
  {
    const ctx = await nav.newContext({ viewport: { width: 1200, height: 800 } });
    await ctx.clock.install();
    const p = await ctx.newPage();
    p.erreurs = [];
    p.on('pageerror', (e) => p.erreurs.push(String(e)));
    await p.addInitScript(simulerCoeur, {
      charge: CHARGE,
      graine: 1,
      alpha: false,
      reponseFermer: 'demander',
    });
    await p.goto(url(srvPage));
    // Horloge arrêtée : seul runFor la fait avancer.
    await ctx.clock.pauseAt(Date.now() + 1000);
    await ctx.clock.runFor(200);
    const copier = () =>
      p.$eval('[data-act="copier"]', (b) => [
        b.classList.contains('fait'),
        b.querySelector('.l2').getAttribute('aria-hidden'),
      ]);
    await p.keyboard.press('Control+c');
    await ctx.clock.runFor(50);
    verifier(
      JSON.stringify(await copier()) === '[false,"true"]',
      '14 au clic : pas encore « Copié »',
    );
    await p.evaluate(() =>
      __emettre('editeur:resultat', {
        id: 7,
        numero: 1,
        but: 'copier',
        etat: 'fait',
        message: null,
      }),
    );
    await ctx.clock.runFor(1799);
    verifier(
      JSON.stringify(await copier()) === '[true,null]',
      '14 « Copié » au résultat, jusqu’à 1 800 ms',
    );
    await ctx.clock.runFor(2);
    verifier(
      JSON.stringify(await copier()) === '[false,"true"]',
      '14 « Copié » retiré après 1 800 ms',
    );
    await p.keyboard.press('Control+s');
    await ctx.clock.runFor(50);
    await p.evaluate(() =>
      __emettre('editeur:resultat', {
        id: 7,
        numero: 2,
        but: 'enregistrer',
        etat: 'deja_fait',
        message: null,
      }),
    );
    await ctx.clock.runFor(10);
    verifier(
      await p.$eval('[data-act="enregistrer"]', (b) => b.classList.contains('fait')),
      '14 Ctrl+S, déjà fait : « Enregistré »',
    );
    await p.keyboard.press('Control+Shift+s');
    await ctx.clock.runFor(50);
    const buts = await p.evaluate(() => window.__exports.map((e) => e.entetes['x-editeur-but']));
    verifier(
      JSON.stringify(buts) === '["copier","enregistrer","enregistrer_sous"]',
      '14 buts des exports',
      JSON.stringify(buts),
    );
    await p.evaluate(() =>
      __emettre('editeur:resultat', {
        id: 7,
        numero: 3,
        but: 'enregistrer_sous',
        etat: 'echec',
        message: 'Impossible d’enregistrer',
      }),
    );
    await ctx.clock.runFor(3999);
    const m1 = await p.$eval('#message', (m) => [
      m.getAttribute('role'),
      m.textContent,
      m.classList.contains('on'),
    ]);
    await ctx.clock.runFor(2);
    const m2 = await p.$eval('#message', (m) => m.classList.contains('on'));
    verifier(
      JSON.stringify(m1) === JSON.stringify(['status', 'Impossible d’enregistrer', true]) && !m2,
      '14 « Impossible d’enregistrer » 4 s (status)',
      JSON.stringify([m1, m2]),
    );
    // editeur:charger : tout est vidé, l'image redemandée.
    await p.keyboard.press('r');
    const [a, b] = await point(p, 50, 50);
    await p.mouse.move(a, b);
    await p.mouse.down();
    await p.mouse.move(a + 80, b + 80);
    await p.mouse.up();
    await p.keyboard.press('Control+1');
    await ctx.clock.runFor(50);
    await p.evaluate(() => {
      window.__charge = { ...window.__charge, id: 8, largeur: 200, hauteur: 100 };
      __emettre('editeur:charger', window.__charge);
    });
    await ctx.clock.runFor(200);
    const r = await p.evaluate(() => ({
      n: __ed.h.doc.objets.length,
      v: __ed.h.version,
      s: __ed.selection.length,
      mode: __ed.cadrage.mode,
      l: __ed.h.doc.largeur,
      img: __appels.filter((x) => x[0] === 'editeur_image').map((x) => x[1].id),
      outil: __ed.prefs.outil,
    }));
    const aff = await p.evaluate(() =>
      __appels.filter((x) => x[0] === 'editeur_affiche').map((x) => x[1].id),
    );
    verifier(
      JSON.stringify(aff) === '[7,8]',
      '14 editeur_affiche après le premier dessin de chaque document',
      JSON.stringify(aff),
    );
    verifier(
      r.n === 0 &&
        r.v === 0 &&
        r.s === 0 &&
        r.mode === 'ajuster' &&
        r.l === 200 &&
        JSON.stringify(r.img) === '[7,8]' &&
        r.outil === 'formes',
      '14 editeur:charger : document, historique, sélection, zoom vidés ; outil gardé',
      JSON.stringify(r),
    );
    verifier(p.erreurs.length === 0, '14 aucune erreur', JSON.stringify(p.erreurs));
    await ctx.close();
  }

  // ---------- 15. Glisser : 4 px → un export « glisser », 3 px : rien ----------
  {
    const p = await page();
    await p.click('#bouton-plus');
    await uneImage(p);
    const src = await p.$eval('#glisser-image', (i) => i.src);
    const b = await (await p.$('#glisser')).boundingBox();
    const x = b.x + b.width / 2;
    const y = b.y + b.height / 2;
    await p.mouse.move(x, y);
    await p.mouse.down();
    await p.mouse.move(x + 3, y);
    await p.mouse.up();
    const n3 = await p.evaluate(() => window.__exports.length);
    await p.mouse.move(x, y);
    await p.mouse.down();
    await p.mouse.move(x + 4, y);
    await p.mouse.up();
    await uneImage(p);
    const e = await p.evaluate(() => window.__exports.map((x) => x.entetes['x-editeur-but']));
    verifier(
      n3 === 0 &&
        JSON.stringify(e) === '["glisser"]' &&
        src.startsWith('blob:') &&
        (await p.$eval('#menu-plus', (m) => !m.classList.contains('ouvert'))),
      '15 Glisser : 3 px rien, 4 px un export, menu fermé, vignette blob:',
      JSON.stringify([n3, e, src]),
    );
    await p.evaluate(() => __emettre('editeur:glisser', { id: 7, etat: 'debut' }));
    verifier(
      (await p.$eval('#glisser', (g) => g.getAttribute('aria-disabled'))) === 'true',
      '15 glisser en cours : grisé',
    );
    await p.evaluate(() => __emettre('editeur:glisser', { id: 7, etat: 'fini' }));
    verifier(
      (await p.$eval('#glisser', (g) => g.getAttribute('aria-disabled'))) === 'false',
      '15 glisser fini',
    );
    await p.context().close();
  }

  // ---------- 16. animations réduites ----------
  for (const [quoi, o] of [
    ['charge reduites', { charge: { reduites: true } }],
    ['système', { reduit: true }],
  ]) {
    const p = await page(o);
    await p.click('#bouton-plus');
    const max = await p.evaluate(() => {
      let m = 0;
      for (const e of document.querySelectorAll('*')) {
        for (const d of getComputedStyle(e).transitionDuration.split(','))
          m = Math.max(m, parseFloat(d) * 1000);
      }
      return m;
    });
    verifier(max <= 1, `16 animations réduites (${quoi}) : aucune transition > 1 ms`, String(max));
    await p.context().close();
  }

  // ---------- 17. CSP, console, thèmes, 1200 × 800 et 640 × 440 ----------
  for (const theme of ['light', 'dark']) {
    for (const [l, h] of [
      [1200, 800],
      [640, 440],
    ]) {
      const p = await page({
        theme,
        largeur: l,
        hauteur: h,
        charge: { largeur: 1600, hauteur: 900 },
      });
      await p.keyboard.press('r');
      await glisser(p, [200, 200], [700, 500]);
      await p.click('#bouton-reglages');
      await p.waitForTimeout(300);
      await p.screenshot({ path: `${SORTIE}/editeur-${theme}-${l}x${h}.png` });
      const r = await p.evaluate(() => {
        const pills = [...document.querySelectorAll('#barre > .pill')].map((x) =>
          x.getBoundingClientRect(),
        );
        const pop = document.querySelector('#reglages-outil').getBoundingClientRect();
        return {
          deborde:
            pills.some((x) => x.left < 0 || x.right > innerWidth || x.top < 0) ||
            pop.left < 0 ||
            pop.right > innerWidth ||
            pop.top < 0,
          lignes: new Set(pills.map((x) => Math.round(x.bottom))).size,
          defile: document.documentElement.scrollWidth > innerWidth,
          fond: getComputedStyle(document.querySelector('#scene')).backgroundColor,
          csp: window.__csp || [],
        };
      });
      verifier(
        !r.deborde &&
          !r.defile &&
          r.csp.length === 0 &&
          p.erreurs.length === 0 &&
          (l === 640 ? r.lignes === 2 : r.lignes === 1),
        `17 ${theme} ${l}×${h} : rien ne déborde, ${l === 640 ? 'deux' : 'une'} ligne(s), CSP`,
        JSON.stringify([r, p.erreurs]),
      );
      verifier(
        r.fond === (theme === 'dark' ? 'rgb(20, 20, 20)' : 'rgb(228, 231, 236)'),
        `17 ${theme} : scène aux couleurs du thème`,
        r.fond,
      );
      await p.context().close();
    }
  }
  {
    const p = await page();
    await p.evaluate(() => __emettre('interface:theme', { theme: 'dark' }));
    const a = await p.$eval('#scene', (s) => getComputedStyle(s).backgroundColor);
    await p.evaluate(() => __emettre('interface:theme', { theme: null }));
    const b = await p.$eval('#scene', (s) => getComputedStyle(s).backgroundColor);
    verifier(
      a === 'rgb(20, 20, 20)' && b === 'rgb(228, 231, 236)',
      '17 interface:theme suivi',
      JSON.stringify([a, b]),
    );
    // Infobulle : nom et touche seulement, après 450 ms.
    const outil = await (await p.$('[data-outil="formes"]')).boundingBox();
    await p.mouse.move(outil.x + 10, outil.y + 10);
    await p.waitForTimeout(250);
    const avant = await p.$eval('#infobulle', (t) => t.classList.contains('on'));
    await p.waitForTimeout(400);
    const bulle = await p.$eval('#infobulle', (t) => [t.classList.contains('on'), t.textContent]);
    verifier(
      !avant && bulle[0] && bulle[1] === 'RectangleR',
      '17 infobulle : nom et touche, après 450 ms',
      JSON.stringify([avant, bulle]),
    );
    await p.context().close();
  }
  // Image de longueur fausse : la page la refuse (editeur_fermer avec force).
  {
    const ctx = await nav.newContext();
    const p = await ctx.newPage();
    await p.addInitScript(simulerCoeur, {
      charge: CHARGE,
      graine: 1,
      alpha: false,
      reponseFermer: 'demander',
    });
    await p.addInitScript(() => {
      const f = window.__image;
      window.__image = (c, g) => f(c, g).slice(1);
    });
    await p.goto(url(srvPage));
    await p.waitForFunction(() => window.__appels.some((a) => a[0] === 'editeur_fermer'));
    const f = await p.evaluate(() => __appels.filter((a) => a[0] === 'editeur_fermer').pop()[1]);
    verifier(
      f.force === true &&
        !(await p.evaluate(() => __appels.some((a) => a[0] === 'editeur_affiche'))),
      '1 image de longueur fausse : refusée (editeur_fermer force)',
      JSON.stringify(f),
    );
    await ctx.close();
  }

  // ======== Assemblage : les vrais document.js et dessin.js (corps du chantier B) ========
  const POINTS_B = [
    '3 export égal (vrai composer)',
    '4 rectangle : hors de la boîte identique, bord',
    '5 Pixeliser : blocs de 13 px',
    '6 Flouter : variance',
    '7 masque sous la flèche',
    '8 Annuler, Rétablir : exports égaux',
    '10 recadrage : taille de l’export',
    '12 dpr : pixel pour pixel (vrai peindreVue)',
    '19 composer 5760 × 1080, 50 objets',
  ];
  if (B_PROVISOIRE) {
    for (const x of POINTS_B) attente.push(`${x} (corps de B provisoires dans cette branche)`);
  } else {
    // Exporte (Ctrl+C) et rend le dernier corps sous forme de résumé calculé dans la page.
    const exporter = async (p) => {
      const n = await p.evaluate(() => window.__exports.length);
      await p.keyboard.press('Control+c');
      await p.waitForFunction((k) => window.__exports.length === k + 1, n);
      // Le résultat du cœur : un export à la fois, le suivant est alors permis.
      await p.evaluate(() => {
        const e = window.__exports[window.__exports.length - 1];
        __emettre('editeur:resultat', {
          id: +e.entetes['x-editeur-id'],
          numero: e.numero,
          but: e.entetes['x-editeur-but'],
          etat: 'fait',
          message: null,
        });
      });
      return p.evaluate(() => {
        window.__corps = (window.__corps || []).concat([window.__dernierCorps]);
        const e = window.__exports[window.__exports.length - 1].entetes;
        return {
          l: +e['x-editeur-largeur'],
          h: +e['x-editeur-hauteur'],
          n: window.__corps.length - 1,
        };
      });
    };
    const egaux = (p, a, b) =>
      p.evaluate(
        ([i, j]) => {
          const x = window.__corps[i],
            y = j < 0 ? window.__image(window.__charge, 3) : window.__corps[j];
          if (x.length !== y.length) return false;
          for (let k = 0; k < x.length; k++) if (x[k] !== y[k]) return false;
          return true;
        },
        [a, b],
      );
    const C = { largeur: 1024, hauteur: 768 };
    {
      const p = await page({ reel: true, charge: C, graine: 3 });
      await p.keyboard.press('Control+1');
      const e0 = await exporter(p);
      verifier(await egaux(p, e0.n, -1), '3 réel : export sans rien faire égal à l’image');
      await p.keyboard.press('r');
      await glisser(p, [100, 100], [300, 200]);
      const e1 = await exporter(p);
      const r4 = await p.evaluate(() => {
        const o = __ed.h.doc.objets[0];
        const b = __ed.modele.boite(o);
        const a = window.__corps[1],
          s = window.__image(window.__charge, 3);
        let hors = 0;
        for (let y = 0; y < 768; y++)
          for (let x = 0; x < 1024; x++) {
            if (
              x >= b.x - 1 &&
              x <= b.x + b.largeur + 1 &&
              y >= b.y - 1 &&
              y <= b.y + b.hauteur + 1
            )
              continue;
            const i = (y * 1024 + x) * 4;
            if (
              a[i] !== s[i] ||
              a[i + 1] !== s[i + 1] ||
              a[i + 2] !== s[i + 2] ||
              a[i + 3] !== s[i + 3]
            )
              hors++;
          }
        const i = (100 * 1024 + 200) * 4;
        return { hors, bord: [a[i], a[i + 1], a[i + 2]], b };
      });
      verifier(
        e1.l === 1024 &&
          r4.hors === 0 &&
          Math.abs(r4.bord[0] - 0xe5) <= 6 &&
          Math.abs(r4.bord[1] - 0x48) <= 6 &&
          Math.abs(r4.bord[2] - 0x4d) <= 6,
        '4 réel : hors de la boîte identique, bord haut #E5484D ± 6',
        JSON.stringify(r4),
      );
      await p.keyboard.press('a');
      await glisser(p, [400, 400], [700, 400]);
      await p.keyboard.press('b');
      await glisser(p, [380, 300], [720, 500]);
      const e2 = await exporter(p);
      const r57 = await p.evaluate(() => {
        const a = window.__corps[window.__corps.length - 1];
        const f = __ed.modele.boite(__ed.h.doc.objets.find((o) => o.type === 'fleche'));
        let unis = 0,
          blocs = 0;
        for (let by = 312; by + 13 <= 494; by += 13)
          for (let bx = 390; bx + 13 <= 715; bx += 13) {
            if (by < f.y + f.hauteur + 1 && by + 13 > f.y - 1) continue; // la flèche est dessus
            blocs++;
            const i0 = (by * 1024 + bx) * 4;
            let ok = true;
            for (let y = by; y < by + 13 && ok; y++)
              for (let x = bx; x < bx + 13; x++) {
                const i = (y * 1024 + x) * 4;
                if (a[i] !== a[i0] || a[i + 1] !== a[i0 + 1] || a[i + 2] !== a[i0 + 2]) {
                  ok = false;
                  break;
                }
              }
            if (ok) unis++;
          }
        const i = (400 * 1024 + 550) * 4;
        return { unis, blocs, fleche: [a[i], a[i + 1], a[i + 2]] };
      });
      verifier(
        r57.blocs > 50 && r57.unis === r57.blocs,
        '5 réel : Pixeliser Moyenne, blocs de 13 px unis alignés',
        JSON.stringify(r57),
      );
      verifier(
        Math.abs(r57.fleche[0] - 0xe5) <= 10 && Math.abs(r57.fleche[1] - 0x48) <= 10,
        '7 réel : masque posé sur une flèche, la flèche reste au-dessus',
        JSON.stringify(r57.fleche),
      );
      await p.keyboard.press('Control+z');
      await p.keyboard.press('Control+z');
      await p.keyboard.press('Control+z');
      const e3 = await exporter(p);
      await p.keyboard.press('Control+y');
      await p.keyboard.press('Control+y');
      await p.keyboard.press('Control+y');
      const e4 = await exporter(p);
      verifier(
        (await egaux(p, e3.n, -1)) && (await egaux(p, e4.n, e2.n)),
        '8 réel : Annuler → l’origine, Rétablir → le même export',
      );
      await p.keyboard.press('Control+0');
      await p.keyboard.press('c');
      await glisser(p, [10, 10], [210, 110]);
      const k = await p.evaluate(() => __ed.h.doc.recadrage);
      const e5 = await exporter(p);
      await p.click('[data-outil="recadrer"]');
      await p.click('[data-act="image_entiere"]');
      const e6 = await exporter(p);
      verifier(
        k &&
          e5.l === k.largeur &&
          e5.h === k.hauteur &&
          Math.abs(k.largeur - 200) <= 2 &&
          e6.l === 1024 &&
          e6.h === 768,
        '10 réel : export à la taille du cadre, Image entière : taille d’origine',
        JSON.stringify([k, e5, e6]),
      );
      verifier(p.erreurs.length === 0, '3-10 réel : aucune erreur', JSON.stringify(p.erreurs));
      await p.context().close();
    }
    {
      const p = await page({ reel: true, charge: { largeur: 600, hauteur: 400 }, graine: 3 });
      await p.click('[data-outil="masquer"]');
      await p.click('[data-outil="masquer"]');
      await p.click('#menu-variantes [data-variante="flouter"]');
      await glisser(p, [100, 100], [300, 300]);
      await exporter(p);
      const v = await p.evaluate(() => {
        const a = window.__corps[0],
          s = window.__image(window.__charge, 3);
        const variance = (t) => {
          let m = 0,
            m2 = 0,
            n = 0;
          for (let y = 120; y < 280; y++)
            for (let x = 120; x < 280; x++) {
              const g = t[(y * 600 + x) * 4 + 1];
              m += g;
              m2 += g * g;
              n++;
            }
          return m2 / n - (m / n) ** 2;
        };
        return [variance(a), variance(s)];
      });
      verifier(
        v[0] < 0.05 * v[1],
        '6 réel : Flouter Moyenne, variance < 5 % de l’origine',
        JSON.stringify(v),
      );
      await p.context().close();
    }
    for (const echelle of [1, 1.25, 1.5, 2]) {
      const p = await page({
        reel: true,
        echelle,
        charge: { largeur: 301, hauteur: 203 },
        graine: 5,
      });
      await p.keyboard.press('Control+1');
      await uneImage(p);
      const ecarts = await p.evaluate(() => {
        const v = __ed.vue;
        const d = __ed.ctx.getImageData(v.x, v.y, 301, 203).data;
        const o = window.__image(window.__charge, 5);
        let n = 0;
        for (let i = 0; i < o.length; i++) if (d[i] !== o[i]) n++;
        return n;
      });
      verifier(
        ecarts === 0,
        `12 réel dpr ${echelle} : Taille réelle pixel pour pixel`,
        String(ecarts),
      );
      await p.context().close();
    }
    {
      const p = await page({ reel: true, charge: { largeur: 5760, hauteur: 1080 }, graine: 3 });
      const ms = await p.evaluate(() => {
        const ed = __ed;
        const types = ['rectangle', 'ellipse', 'ligne', 'fleche', 'texte'];
        ed.modifier((doc) => {
          for (let i = 0; i < 50; i++) {
            const t = types[i % 5],
              x = 100 + i * 110,
              y = 100 + (i % 7) * 120;
            const o =
              t === 'texte'
                ? {
                    type: t,
                    x,
                    y,
                    texte: 'Texte ' + i,
                    couleur: '#E5484D',
                    taille: 28,
                    gras: true,
                    style: 'contour',
                    opacite: 1,
                  }
                : {
                    type: t,
                    x1: x,
                    y1: y,
                    x2: x + 90,
                    y2: y + 80,
                    couleur: '#0090FF',
                    epaisseur: 1,
                    plein: false,
                    arrondi: 10,
                    double: false,
                    opacite: 1,
                  };
            ed.modele.ajouter(doc, o);
          }
          ed.modele.ajouter(doc, {
            type: 'masque',
            x1: 10,
            y1: 10,
            x2: 900,
            y2: 600,
            sorte: 'pixeliser',
            intensite: 1,
          });
        });
        const t0 = performance.now();
        const r = ed.dessin.composer(ed.h.doc, ed.fond);
        return [performance.now() - t0, r.largeur];
      });
      console.log(`MESURE composer 5760×1080, 50 objets : ${Math.round(ms[0])} ms`);
      verifier(
        ms[1] === 5760 && ms[0] <= 1000,
        '19 réel : composer 5760 × 1080, 50 objets ≤ 1 000 ms',
        String(ms[0]),
      );
      await p.context().close();
    }
    // ---------- 20. reprises de la relecture, avec le vrai modèle ----------
    const objs = async (p) => (await docu(p)).objets;
    const saisieOuverte = (p) => p.evaluate(() => !document.querySelector('#saisie').hidden);
    // Ordre des appels au cœur (noms), depuis le dernier `vider`.
    const noms = async (p) => (await appels(p)).map((a) => a[0]);
    {
      const p = await page({ reel: true });
      await p.keyboard.press('Control+1');
      await uneImage(p);
      const v = await image(p);
      // Hors de l'image : la scène autour (5, 5) CSS est hors du cadre en Taille réelle.
      verifier(v.x / v.dpr > 20 && v.y / v.dpr > 20, '20 préalable : marge autour de l’image');
      await touche(p, 'r');
      await glisser(p, [100, 100], [200, 150]);
      const avant = await p.evaluate(() => __ed.selection.length);
      await p.mouse.move(5, 5);
      await p.mouse.down();
      await p.mouse.move(25, 25, { steps: 4 });
      await p.mouse.up();
      await uneImage(p);
      verifier(
        (await objs(p)).length === 1 &&
          avant === 1 &&
          (await p.evaluate(() => __ed.selection.length)) === 0,
        '20 cas 1 : appui hors de l’image : rien de créé, sélection vidée',
        JSON.stringify([avant, (await objs(p)).length]),
      );
      await touche(p, 'n');
      await p.mouse.click(5, 5);
      await uneImage(p);
      verifier((await objs(p)).length === 1, '20 cas 1 : Numéros hors de l’image : rien');
      // Commencé dans l'image, fini hors de la fenêtre : créé.
      await touche(p, 'r');
      const [hx, hy] = await point(p, 350, 250);
      await p.mouse.move(hx, hy);
      await p.mouse.down();
      await p.mouse.move(1199, 799, { steps: 5 });
      await p.mouse.up();
      await uneImage(p);
      const dehors = (await objs(p)).at(-1);
      verifier(
        (await objs(p)).length === 2 &&
          dehors.type === 'rectangle' &&
          Math.abs(dehors.x1 - 350) < 1 &&
          !(await p.evaluate(() => __ed.outils.gesteEnCours())),
        '20 cas 1 : geste commencé dans l’image, fini dehors : créé',
        JSON.stringify(dehors),
      );
      // Normaliser : un rectangle tiré vers le haut à gauche.
      await glisser(p, [300, 200], [250, 160]);
      const n = (await objs(p)).at(-1);
      verifier(
        n.x1 === 250 && n.x2 === 300 && n.y1 === 160 && n.y2 === 200,
        '20 normaliser : tiré vers le haut à gauche',
        JSON.stringify(n),
      );
      // Lisser : le sommet d'un V tracé au crayon est arrondi (Chaikin) et au 1/100.
      await touche(p, 'p');
      const [a1, b1] = await point(p, 40, 200);
      await p.mouse.move(a1, b1);
      await p.mouse.down();
      await p.mouse.move(a1 + 30, b1 + 60, { steps: 6 });
      await p.mouse.move(a1 + 60, b1, { steps: 6 });
      await p.mouse.up();
      await uneImage(p);
      const c = (await objs(p)).at(-1);
      const ys = c.points.map((q) => q[1]);
      verifier(
        c.type === 'crayon' &&
          Math.max(...ys) < 259 &&
          c.points.every((q) => q.every((k) => Math.round(k * 100) === k * 100)),
        '20 lisser : sommet du crayon adouci, points au 1/100',
        JSON.stringify([c.type, Math.max(...ys)]),
      );
      // Poignée « se » : redimensionner.
      await touche(p, 'v');
      await p.keyboard.press('Escape');
      await cliquer(p, 100, 125);
      const sel = await p.evaluate(() => __ed.selection.slice());
      const r0 = (await objs(p))[0];
      await glisser(p, [r0.x2, r0.y2], [r0.x2 + 40, r0.y2 + 30]);
      const r1 = (await objs(p))[0];
      verifier(
        sel.length === 1 &&
          sel[0] === r0.id &&
          r1.x1 === r0.x1 &&
          r1.y1 === r0.y1 &&
          Math.abs(r1.x2 - (r0.x2 + 40)) < 1 &&
          Math.abs(r1.y2 - (r0.y2 + 30)) < 1,
        '20 poignée se : redimensionner',
        JSON.stringify([sel, r0, r1]),
      );
      await p.context().close();
    }
    // Texte : place de la saisie, texte dessiné par la vue (`ajout`), clic ailleurs, Annuler.
    {
      const p = await page({ reel: true });
      await p.keyboard.press('Control+1');
      await uneImage(p);
      await touche(p, 'r');
      await glisser(p, [20, 20], [60, 60]);
      await touche(p, 't');
      await cliquer(p, 100, 100);
      const t = await p.evaluate(() => {
        const s = document.querySelector('#saisie');
        const r = s.getBoundingClientRect();
        return { x: r.left, y: r.top, couleur: getComputedStyle(s).color };
      });
      const ouverte = await saisieOuverte(p);
      const zone = () =>
        p.evaluate(() => {
          const v = __ed.vue;
          const d = __ed.ctx.getImageData(v.x + 100, v.y + 100, 80, 30).data;
          let s = 0;
          for (let i = 0; i < d.length; i++) s = (s + d[i] * (i + 1)) % 1000000007;
          return s;
        });
      const vide = await zone();
      await p.keyboard.type('MMM');
      await uneImage(p);
      verifier((await zone()) !== vide, '20 le texte en saisie est dessiné par la vue (ajout)');
      // Cas 2 : un clic ailleurs valide seulement (pas de nouvelle saisie).
      await cliquer(p, 300, 250);
      const o2 = await objs(p);
      verifier(
        o2.length === 2 && o2[1].texte === 'MMM' && !(await saisieOuverte(p)),
        '20 cas 2 : clic ailleurs pendant la saisie : validé, rien d’autre',
        JSON.stringify(o2.map((o) => o.texte || o.type)),
      );
      // La zone de saisie était au coin du texte posé (x, y de l'objet).
      const [ex, ey] = await point(p, o2[1].x, o2[1].y);
      verifier(
        ouverte &&
          Math.abs(t.x - ex) < 1 &&
          Math.abs(t.y - ey) < 1 &&
          t.couleur === 'rgba(0, 0, 0, 0)',
        '20 saisie à la place du texte, texte de la zone transparent',
        JSON.stringify([t, ex, ey]),
      );
      // Texte existant rouvert : `cacher` le retire de la vue, `ajout` le redessine une fois.
      // Sans sélection (son contour changerait la zone).
      await p.evaluate(() => {
        __ed.selection = [];
        __ed.dessiner();
      });
      await uneImage(p);
      const avec = await zone();
      await touche(p, 'v');
      const [dx, dy] = await point(p, 110, 110);
      await p.mouse.dblclick(dx, dy);
      await uneImage(p);
      verifier(
        (await saisieOuverte(p)) && (await zone()) === avec,
        '20 texte rouvert : dessiné une fois (cacher + ajout)',
        JSON.stringify([
          await saisieOuverte(p),
          avec,
          await zone(),
          await p.evaluate(() => [
            __ed.selection,
            __ed.outils.idEnEdition(),
            document.querySelector('#saisie').value,
          ]),
        ]),
      );
      await p.keyboard.press('Escape');
      // Cas 3 : Annuler pendant une saisie l'abandonne ; Rétablir ne fait rien.
      await p.keyboard.press('Control+z');
      await uneImage(p);
      verifier((await objs(p)).length === 1, '20 préalable : un pas à rétablir');
      await touche(p, 't');
      await cliquer(p, 200, 200);
      await p.keyboard.type('zz');
      const retablir = await p.$eval('[data-act="retablir"]', (b) => b.disabled);
      await p.evaluate(() => __ed.retablir());
      const apresRetablir = [(await objs(p)).length, await saisieOuverte(p)];
      await p.click('[data-act="annuler"]');
      await uneImage(p);
      verifier(
        retablir &&
          JSON.stringify(apresRetablir) === '[1,true]' &&
          (await objs(p)).length === 1 &&
          !(await saisieOuverte(p)) &&
          (await p.evaluate(() => __ed.h.futur.length)) === 1,
        '20 cas 3 : Annuler abandonne la saisie, Rétablir sans effet',
        JSON.stringify([retablir, apresRetablir]),
      );
      // L'avertissement de Chromium vient des lectures de cet essai (getImageData).
      const erreurs = p.erreurs.filter((x) => !x.includes('willReadFrequently'));
      verifier(erreurs.length === 0, '20 texte : console vide', JSON.stringify(erreurs));
      await p.context().close();
    }
    // Numéros : un numéro supprimé, les suivants renumérotés ; le suivant prend la suite.
    {
      const p = await page({ reel: true });
      await p.keyboard.press('Control+1');
      await uneImage(p);
      await touche(p, 'n');
      for (const x of [60, 120, 180]) await cliquer(p, x, 60);
      await touche(p, 'v');
      await cliquer(p, 120, 60);
      await p.keyboard.press('Delete');
      await touche(p, 'n');
      await cliquer(p, 240, 60);
      const rangs = await p.evaluate(() =>
        __ed.h.doc.objets.map((o) => __ed.modele.numeroAffiche(__ed.h.doc, o)),
      );
      verifier(
        JSON.stringify(rangs) === '[1,2,3]',
        '20 numéros : renumérotés après suppression',
        JSON.stringify(rangs),
      );
      await p.context().close();
    }
    // Cas 4 : un export à la fois ; Ctrl+C répété ; double clic.
    {
      const p = await page({ reel: true });
      await touche(p, 'r');
      await glisser(p, [20, 20], [60, 60]);
      const nb = () => p.evaluate(() => window.__exports.length);
      await p.keyboard.down('Control');
      await p.keyboard.down('c');
      await p.keyboard.down('c');
      await p.keyboard.down('c');
      await p.keyboard.up('c');
      await p.keyboard.up('Control');
      await p.dblclick('[data-act="copier"]');
      await p.keyboard.press('Control+s');
      await attendreAppel(p, 'editeur_exporter');
      await uneImage(p);
      await uneImage(p);
      verifier((await nb()) === 1, '20 cas 4 : un export à la fois', String(await nb()));
      await p.evaluate(() =>
        __emettre('editeur:resultat', { id: 7, numero: 1, but: 'copier', etat: 'fait' }),
      );
      await p.keyboard.press('Control+s');
      await p.waitForFunction(() => window.__exports.length === 2, null, { timeout: 3000 });
      verifier((await nb()) === 2, '20 cas 4 : après le résultat, l’export suivant');
      // Échec de l'appel : le verrou tombe aussi.
      await p.evaluate(() => {
        const i = window.__TAURI__.core.invoke;
        window.__TAURI__.core.invoke = (c, ...r) => {
          if (c !== 'editeur_exporter') return i(c, ...r);
          window.__TAURI__.core.invoke = i;
          return Promise.reject(new Error('refus'));
        };
        __emettre('editeur:resultat', { id: 7, numero: 2, but: 'enregistrer', etat: 'fait' });
      });
      await p.keyboard.press('Control+c');
      await p.waitForFunction(() => !__ed.exportEnCours(), null, { timeout: 3000 });
      await p.keyboard.press('Control+c');
      await p.waitForFunction(() => window.__exports.length === 3, null, { timeout: 3000 });
      verifier((await nb()) === 3, '20 cas 4 : échec de l’appel, le verrou tombe');
      await p.evaluate(() =>
        __emettre('editeur:resultat', { id: 7, numero: 3, but: 'copier', etat: 'fait' }),
      );
      // « Copier et fermer » cliqué deux fois : un export.
      await p.evaluate(() => __emettre('editeur:fermer_demande', { id: 7, raison: 'cadre' }));
      await p.waitForSelector('#voile.ouvert', { timeout: 3000 });
      await p.dblclick('[data-act="copier_fermer"]');
      await attendreAppel(p, 'editeur_exporter');
      await uneImage(p);
      await uneImage(p);
      const buts = await p.evaluate(() => __exports.map((e) => e.entetes['x-editeur-but']));
      verifier(
        buts.filter((b) => b === 'copier_fermer').length === 1 && buts.length === 4,
        '20 cas 4 : « Copier et fermer » deux fois : un export',
        JSON.stringify(buts),
      );
      await p.context().close();
    }
    // Décision : à chaque fermer_demande et avant tout export, le texte en saisie validé,
    // `editeur_etat` aussitôt, puis la réponse ou l'export.
    {
      const p = await page({ reel: true });
      await touche(p, 't');
      await cliquer(p, 100, 100);
      await p.keyboard.type('bonjour');
      await vider(p);
      await p.evaluate(() => __emettre('editeur:fermer_demande', { id: 7, raison: 'cadre' }));
      await attendreAppel(p, 'editeur_fermer');
      await p.waitForSelector('#voile.ouvert', { timeout: 3000 });
      const a = await appels(p);
      const v = await p.evaluate(() => __ed.h.version);
      const iE = a.findIndex((x) => x[0] === 'editeur_etat');
      const iF = a.findIndex((x) => x[0] === 'editeur_fermer');
      verifier(
        (await objs(p)).some((o) => o.texte === 'bonjour') &&
          iE >= 0 &&
          iE < iF &&
          a[iE][1].version === v &&
          a[iE][1].vide === false &&
          a[iF][1].version === v,
        '20 fermer_demande : texte validé, editeur_etat puis editeur_fermer',
        JSON.stringify(a.map((x) => x[0])),
      );
      await p.click('[data-act="continuer"]');
      // Le voile ne prend plus les clics : la scène est sous la pointe.
      const [sx, sy] = await point(p, 200, 200);
      await p.waitForFunction(
        ([x, y]) => document.elementFromPoint(x, y) === __ed.toile,
        [sx, sy],
        {
          timeout: 3000,
        },
      );
      await cliquer(p, 200, 200);
      await p.keyboard.type('encore');
      await vider(p);
      await p.click('[data-act="copier"]');
      await attendreAppel(p, 'editeur_exporter');
      const b = await appels(p);
      const w = await p.evaluate(() => __ed.h.version);
      const jE = b.findIndex((x) => x[0] === 'editeur_etat');
      const jX = b.findIndex((x) => x[0] === 'editeur_exporter');
      verifier(
        (await objs(p)).some((o) => o.texte === 'encore') &&
          jE >= 0 &&
          jE < jX &&
          b[jE][1].version === w &&
          b[jX][2]['x-editeur-version'] === String(w),
        '20 export : texte validé, editeur_etat puis editeur_exporter',
        JSON.stringify([
          b.map((x) => x[0]),
          await p.evaluate(() => [
            __ed.h.doc.objets.map((o) => o.texte),
            document.activeElement.id,
            document.activeElement.dataset.act,
            __ed.exportEnCours(),
            !!__ed.charge,
            !!__ed.h,
            !!__ed.fond,
            document.querySelector('[data-act="copier"]').disabled,
            document.querySelector('#voile').className,
            !document.querySelector('#saisie').hidden,
            __ed.prefs.outil,
          ]),
        ]),
      );
      verifier(
        (await noms(p)).filter((x) => x === 'editeur_exporter').length === 1,
        '20 export : un seul appel',
      );
      await p.context().close();
    }
  }

  await nav.close();
  srvPage.close();
  srvReel.close();
  for (const a of attente) console.log('EN ATTENTE', a);
  console.log(`ECHECS : ${echecs}`);
  process.exit(echecs ? 1 : 0);
})();
