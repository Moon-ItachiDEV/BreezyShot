const { chromium } = require('playwright');
const http = require('http'), fs = require('fs'), path = require('path');
// Essai de la page du menu de l'icône (piste A « Liste ») dans Chromium (Playwright) :
// lignes reçues de `menu_pret` puis de `menu:ouvrir`, mesures égales à celles du cœur
// (fenetres/menu.rs), clic, clavier, Échap, CSP. Captures claire et sombre pour relecture.
// Sort avec 1 si une vérification échoue.
//   node essais/pages/menu.cjs
const RACINE = path.join(__dirname, '..', '..', 'app', 'interface');
const SORTIE = process.env.SORTIE || path.join(__dirname, '..', '..', 'essais-resultats', 'pages');
fs.mkdirSync(SORTIE, { recursive: true });
const CSP = Object.entries(JSON.parse(fs.readFileSync(path.join(__dirname, '..', '..', 'app', 'coeur', 'tauri.conf.json'), 'utf8')).app.security.csp)
  .map(([k, v]) => `${k} ${v}`).join('; ');
// Mesures du cœur, lues dans fenetres/menu.rs.
const RS = fs.readFileSync(path.join(__dirname, '..', '..', 'app', 'coeur', 'src', 'fenetres', 'menu.rs'), 'utf8');
const mesure = (nom) => Number(RS.match(new RegExp(`pub const ${nom}: f64 = ([0-9.]+);`))[1]);
const [LARGEUR, LIGNE, SEP, CADRE, MARGE] = ['LARGEUR', 'HAUTEUR_LIGNE', 'HAUTEUR_SEPARATEUR', 'CADRE', 'MARGE'].map(mesure);
const types = { '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript' };
const serveur = http.createServer((q, r) => {
  const p = path.join(RACINE, decodeURIComponent(new URL(q.url, 'http://x').pathname));
  if (!p.startsWith(RACINE) || !fs.existsSync(p)) { r.writeHead(404); return r.end(); }
  const entetes = { 'content-type': types[path.extname(p)] || 'text/plain' };
  if (q.url.includes('csp=1')) entetes['content-security-policy'] = CSP;
  r.writeHead(200, entetes); fs.createReadStream(p).pipe(r);
});
const LIGNES = [
  { id: 'capturer_zone', libelle: 'Capturer une zone', raccourci: 'Ctrl + Maj + 1', separateur: false },
  { id: 'capturer_plein_ecran', libelle: "Capturer l'écran", raccourci: 'Ctrl + Maj + 2', separateur: false },
  { id: 'capturer_fenetre', libelle: 'Capturer une fenêtre', raccourci: 'Ctrl + Maj + 3', separateur: false },
  { id: '', libelle: '', raccourci: null, separateur: true },
  { id: 'capturer_derniere_zone', libelle: 'Refaire la dernière zone', raccourci: 'Ctrl + Maj + 4', separateur: false },
  { id: 'capturer_defilante', libelle: 'Capture défilante', raccourci: 'Ctrl + Maj + 5', separateur: false },
  { id: 'capturer_texte', libelle: 'Copier le texte', raccourci: 'Ctrl + Maj + T', separateur: false },
  { id: '', libelle: '', raccourci: null, separateur: true },
  { id: 'historique', libelle: 'Historique', raccourci: 'Ctrl + Maj + H', separateur: false },
  { id: 'reglages', libelle: 'Réglages', raccourci: null, separateur: false },
  { id: '', libelle: '', raccourci: null, separateur: true },
  { id: 'quitter', libelle: 'Quitter BreezyShot', raccourci: null, separateur: false },
];
const hauteur = (lignes) => { const n = lignes.filter(l => l.separateur).length; return CADRE + (lignes.length - n) * LIGNE + n * SEP; };
const HAUTEUR = hauteur(LIGNES);
// Avec licence : le troisième groupe, Vidéo et GIF, entre deux séparateurs.
const LIGNES_FILM = [
  ...LIGNES.slice(0, 7),
  { id: '', libelle: '', raccourci: null, separateur: true },
  { id: 'capturer_video', libelle: 'Vidéo', raccourci: 'Ctrl + Maj + 6', separateur: false },
  { id: 'capturer_gif', libelle: 'GIF', raccourci: 'Ctrl + Maj + 7', separateur: false },
  { id: '', libelle: '', raccourci: null, separateur: true },
  ...LIGNES.slice(8),
];
(async () => {
  await new Promise(ok => serveur.listen(0, ok));
  const base = `http://127.0.0.1:${serveur.address().port}/menu/menu.html?csp=1`;
  const nav = await chromium.launch();
  const erreurs = [];
  let echecs = 0;
  const verifier = (ok, quoi, info = '') => { console.log(ok ? 'OK   ' : 'ECHEC', quoi, info); if (!ok) echecs++; };
  async function page(o = {}) {
    const ctx = await nav.newContext({ viewport: { width: LARGEUR + 2 * MARGE, height: Math.round(hauteur(o.lignes || LIGNES) + 2 * MARGE) }, deviceScaleFactor: o.echelle || 1, colorScheme: o.sombre ? 'dark' : 'light' });
    const p = await ctx.newPage();
    p.on('pageerror', e => erreurs.push(String(e)));
    p.on('console', m => { if (m.type() === 'error' || (m.type() === 'warning' && !o.bruitPermis)) erreurs.push(m.text()); });
    await p.addInitScript(([lignes, refus]) => {
      window.__refus = []; document.addEventListener('securitypolicyviolation', e => window.__refus.push(e.violatedDirective));
      window.__appels = []; const ecoutes = {}; let refuses = 0;
      window.__TAURI__ = {
        core: { invoke: (c, a) => {
          window.__appels.push([c, a]);
          if (c === 'menu_pret' && refuses < refus) { refuses++; return Promise.reject('pas encore en place'); }
          return Promise.resolve(c === 'menu_pret' ? { lignes } : null);
        } },
        event: { listen: (n, f) => { ecoutes[n] = f; return Promise.resolve(() => {}); } },
      };
      window.__emettre = (n, payload) => ecoutes[n]({ payload });
    }, [o.lignes || LIGNES, o.refus || 0]);
    await p.goto(base);
    await p.waitForTimeout(300);
    return p;
  }
  const appels = (p, nom) => p.evaluate(n => window.__appels.filter(a => !n || a[0] === n), nom);

  const p = await page();
  verifier((await appels(p, 'menu_pret')).length === 1, 'menu_pret une fois');
  const mesurer = (q) => q.evaluate(() => {
    const m = document.getElementById('menu');
    const l = [...m.querySelectorAll('.ligne')];
    const r = m.getBoundingClientRect();
    const dernier = m.lastElementChild.getBoundingClientRect();
    const cs = getComputedStyle(m);
    return {
      lignes: l.map(b => [b.dataset.id, b.querySelector('.libelle').textContent, b.querySelector('.raccourci')?.textContent || null, b.getAttribute('role'), b.querySelectorAll('svg *').length > 0]),
      seps: m.querySelectorAll('[role=separator]').length,
      carte: [r.left, r.top, r.width],
      contenu: dernier.bottom - r.top + parseFloat(cs.paddingBottom) + parseFloat(cs.borderBottomWidth),
      hauteurs: [...new Set(l.map(b => b.getBoundingClientRect().height))],
      raccourciAligne: [...new Set(l.filter(b => b.querySelector('.raccourci')).map(b => Math.round(b.querySelector('.raccourci').getBoundingClientRect().right)))],
      engrenage: document.body.innerHTML.includes('gear'),
      defilantePointillee: !!m.querySelector('[data-id=capturer_defilante] svg path[stroke-dasharray]'),
      ordre: [...m.children].map(e => e.getAttribute('role') === 'separator' ? '|' : e.dataset.id),
    };
  });
  const vu = await mesurer(p);
  verifier(vu.lignes.length === 9 && vu.seps === 3, '9 lignes, 3 séparateurs', JSON.stringify(vu.lignes.map(x => x[0])));
  verifier(vu.lignes.every(x => x[3] === 'menuitem' && x[4]), 'chaque ligne : role menuitem et pictogramme');
  verifier(vu.lignes[4][0] === 'capturer_defilante' && vu.defilantePointillee, 'Capture défilante : cinquième, pictogramme au bas pointillé', JSON.stringify(vu.lignes.map(x => x[0])));
  verifier(JSON.stringify(vu.lignes.map(x => [x[1], x[2]])) === JSON.stringify(LIGNES.filter(l => !l.separateur).map(l => [l.libelle, l.raccourci])), 'libellés et raccourcis reçus, tels quels');
  verifier(vu.carte[0] === MARGE && vu.carte[1] === MARGE && vu.carte[2] === LARGEUR, `carte à ${MARGE} px du bord, ${LARGEUR} px de large`, JSON.stringify(vu.carte));
  verifier(Math.abs(vu.contenu - HAUTEUR) < 0.5, `hauteur du contenu = celle du cœur (${HAUTEUR} px)`, String(vu.contenu));
  verifier(vu.hauteurs.length === 1 && vu.hauteurs[0] === LIGNE, `lignes de ${LIGNE} px`, JSON.stringify(vu.hauteurs));
  verifier(vu.raccourciAligne.length === 1, 'raccourcis alignés à droite', JSON.stringify(vu.raccourciAligne));
  for (const [nom, sombre] of [['clair', false], ['sombre', true]]) {
    for (const echelle of [1, 2]) {
      const q = await page({ sombre, echelle });
      await q.mouse.move(MARGE + 100, MARGE + 4 + LIGNE * 4 + SEP + 10);
      await q.waitForTimeout(200);
      await q.screenshot({ path: path.join(SORTIE, `menu-${nom}-${echelle}x.png`), omitBackground: true });
      await q.context().close();
    }
  }

  {
    // Avec licence : 11 lignes, 4 séparateurs ; Vidéo et GIF forment le troisième groupe.
    const q = await page({ lignes: LIGNES_FILM });
    const v = await mesurer(q);
    verifier(v.lignes.length === 11 && v.seps === 4, 'avec licence : 11 lignes, 4 séparateurs', JSON.stringify(v.ordre));
    const i = v.ordre.indexOf('capturer_video');
    verifier(v.ordre[i - 1] === '|' && v.ordre[i + 1] === 'capturer_gif' && v.ordre[i + 2] === '|', 'Vidéo puis GIF, entre deux séparateurs', JSON.stringify(v.ordre));
    verifier(v.lignes.every(x => x[3] === 'menuitem' && x[4]), 'avec licence : un pictogramme sur chaque ligne');
    verifier(JSON.stringify(v.lignes.map(x => [x[1], x[2]])) === JSON.stringify(LIGNES_FILM.filter(l => !l.separateur).map(l => [l.libelle, l.raccourci])), 'Vidéo « Ctrl + Maj + 6 », GIF « Ctrl + Maj + 7 », tels quels');
    const pictos = await q.evaluate(() => ['capturer_video', 'capturer_gif', 'capturer_zone'].map(id => document.querySelector(`[data-id=${id}] svg`).innerHTML));
    verifier(new Set(pictos).size === 3, 'pictogrammes Vidéo et GIF propres, distincts de la zone');
    const H = hauteur(LIGNES_FILM);
    verifier(Math.abs(v.contenu - H) < 0.5, `avec licence : hauteur du contenu = celle du cœur (${H} px)`, String(v.contenu));
    verifier(v.raccourciAligne.length === 1, 'avec licence : raccourcis alignés à droite', JSON.stringify(v.raccourciAligne));
    await q.context().close();
    for (const [nom, sombre] of [['clair', false], ['sombre', true]]) {
      const r = await page({ sombre, echelle: 2, lignes: LIGNES_FILM });
      await r.mouse.move(MARGE + 100, MARGE + 4 + LIGNE * 6 + SEP * 2 + 10);
      await r.waitForTimeout(200);
      await r.screenshot({ path: path.join(SORTIE, `menu-film-${nom}.png`), omitBackground: true });
      await r.context().close();
    }
  }

  // Clic sur « Réglages » : menu_choisi une seule fois, même au double clic.
  await p.getByRole('menuitem', { name: 'Réglages' }).dblclick();
  await p.waitForTimeout(50);
  verifier(JSON.stringify(await appels(p, 'menu_choisi')) === JSON.stringify([['menu_choisi', { id: 'reglages' }]]), 'clic sur Réglages : menu_choisi { id: reglages }, une fois');

  // Caché : la carte se vide (rien d'ancien à la prochaine ouverture).
  await p.evaluate(() => __emettre('menu:vider'));
  verifier((await p.evaluate(() => document.getElementById('menu').children.length)) === 0, 'menu:vider : carte vide');
  // Nouvelle ouverture, au clavier : Flèche bas prend la première ligne, puis flèches, Entrée.
  await p.evaluate((lignes) => __emettre('menu:ouvrir', { lignes }), LIGNES);
  await p.waitForTimeout(50);
  const focus = () => p.evaluate(() => document.activeElement?.dataset?.id || null);
  verifier(await focus() === null, 'ouvert : aucune ligne au focus', await focus());
  await p.keyboard.press('ArrowDown');
  verifier(await focus() === 'capturer_zone', 'Flèche bas : la première ligne', await focus());
  await p.keyboard.press('ArrowUp');
  verifier(await focus() === 'quitter', 'Flèche haut depuis la première : la dernière', await focus());
  await p.keyboard.press('ArrowDown');
  await p.keyboard.press('ArrowDown');
  await p.keyboard.press('End');
  await p.keyboard.press('Home');
  await p.keyboard.press('ArrowDown');
  await p.keyboard.press('ArrowDown');
  await p.keyboard.press('ArrowDown');
  verifier(await focus() === 'capturer_derniere_zone', 'Flèches et Début : les séparateurs sont sautés', await focus());
  await p.keyboard.press('Enter');
  await p.waitForTimeout(50);
  const choix = (await appels(p, 'menu_choisi')).map(a => a[1].id);
  verifier(JSON.stringify(choix) === JSON.stringify(['reglages', 'capturer_derniere_zone']), 'Entrée : menu_choisi de la ligne au focus', JSON.stringify(choix));
  // Échap ferme sans choisir.
  await p.evaluate((lignes) => __emettre('menu:ouvrir', { lignes }), LIGNES);
  await p.keyboard.press('Escape');
  await p.waitForTimeout(50);
  verifier((await appels(p, 'menu_fermer')).length === 1 && (await appels(p, 'menu_choisi')).length === 2, 'Échap : menu_fermer, rien de choisi');
  // Pendant une défilante, le cœur n'envoie que Quitter : une seule ligne.
  await p.evaluate(() => __emettre('menu:ouvrir', { lignes: [{ id: 'quitter', libelle: 'Quitter BreezyShot', raccourci: null, separateur: false }] }));
  verifier(JSON.stringify(await p.evaluate(() => [...document.querySelectorAll('.ligne')].map(b => b.dataset.id))) === '["quitter"]', 'pendant une défilante : Quitter seul');
  // Ligne mal formée : ignorée sans casser la page.
  await p.evaluate(() => __emettre('menu:ouvrir', { lignes: [{ id: 3 }, { id: 'reglages', libelle: 'Réglages', raccourci: null, separateur: false }] }));
  verifier((await p.evaluate(() => document.querySelectorAll('.ligne').length)) === 1, 'ligne mal formée ignorée');
  await p.context().close();

  {
    // Cœur pas encore en place : deux refus, lignes reçues au troisième essai.
    const q = await page({ refus: 2 });
    await q.waitForTimeout(400);
    const n = (await appels(q, 'menu_pret')).length;
    const l = await q.evaluate(() => document.querySelectorAll('.ligne').length);
    verifier(n === 3 && l === 9, 'menu_pret rejeté deux fois : retenté, lignes au troisième', `${n} appel(s), ${l} ligne(s)`);
    await q.context().close();
  }

  verifier(erreurs.length === 0, 'aucune erreur ni avertissement dans la console', erreurs.join(' | '));
  const csp = await (async () => { const q = await page(); const r = await q.evaluate(() => window.__refus); await q.context().close(); return r; })();
  verifier(csp.length === 0, 'aucune violation de la CSP', JSON.stringify(csp));
  await nav.close();
  serveur.close();
  console.log(echecs ? `${echecs} échec(s)` : 'tout est vert');
  process.exit(echecs ? 1 : 0);
})().catch(e => { console.error(e); process.exit(1); });
