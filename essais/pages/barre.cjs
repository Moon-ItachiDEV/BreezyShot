const { chromium } = require('playwright');
const http = require('http'), fs = require('fs'), path = require('path');
// Essai de la page de la barre de capture (design/capture, `.launcher`) dans Chromium
// (Playwright) : boutons montrés selon la charge reçue (`barre_pret`, puis `barre:ouvrir`),
// mesures égales à celles du cœur (fenetres/barre.rs), taille rendue (`barre_taille`), choix
// envoyé, menus Vidéo/GIF et Retardateur, clavier, Échap, clic dans la marge, mouvement
// réduit, CSP. Captures claire et sombre pour relecture. Sort avec 1 si une vérification
// échoue.
//   node essais/pages/barre.cjs
const RACINE = path.join(__dirname, '..', '..', 'app', 'interface');
const SORTIE = process.env.SORTIE || path.join(__dirname, '..', '..', 'essais-resultats', 'pages');
fs.mkdirSync(SORTIE, { recursive: true });
const CSP = Object.entries(JSON.parse(fs.readFileSync(path.join(__dirname, '..', '..', 'app', 'coeur', 'tauri.conf.json'), 'utf8')).app.security.csp)
  .map(([k, v]) => `${k} ${v}`).join('; ');
// Mesures du cœur, lues dans fenetres/barre.rs.
const RS = fs.readFileSync(path.join(__dirname, '..', '..', 'app', 'coeur', 'src', 'fenetres', 'barre.rs'), 'utf8');
const mesure = (nom) => Number(RS.match(new RegExp(`pub const ${nom}: f64 = ([0-9.]+);`))[1]);
const [HAUT, MARGE, MARGE_BAS, HAUTEUR] = ['HAUT', 'MARGE', 'MARGE_BAS', 'HAUTEUR_BARRE'].map(mesure);
const LARGEUR_FENETRE = 640;
const types = { '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript' };
const serveur = http.createServer((q, r) => {
  const p = path.join(RACINE, decodeURIComponent(new URL(q.url, 'http://x').pathname));
  if (!p.startsWith(RACINE) || !fs.existsSync(p)) { r.writeHead(404); return r.end(); }
  const entetes = { 'content-type': types[path.extname(p)] || 'text/plain' };
  if (q.url.includes('csp=1')) entetes['content-security-policy'] = CSP;
  r.writeHead(200, entetes); fs.createReadStream(p).pipe(r);
});
const BASE = ['capturer_zone', 'capturer_plein_ecran', 'capturer_fenetre', 'reglages'];
const PRO = ['capturer_zone', 'capturer_plein_ecran', 'capturer_fenetre', 'capturer_defilante', 'capturer_video', 'capturer_gif', 'reglages'];
const charge = (boutons, retardateur_s = 0) => ({ boutons, retardateur_s, retardateurs: [0, 3, 5, 10] });
const CURSEURS = ['M3 6.5h5.5M13.5 6.5H17M3 13.5h2.5M10.5 13.5H17', 'circle 11 6.5 2.2', 'circle 8 13.5 2.2'];
(async () => {
  await new Promise(ok => serveur.listen(0, ok));
  const base = `http://127.0.0.1:${serveur.address().port}/barre/barre.html?csp=1`;
  const nav = await chromium.launch();
  const erreurs = [];
  let echecs = 0;
  const verifier = (ok, quoi, info = '') => { console.log(ok ? 'OK   ' : 'ECHEC', quoi, info); if (!ok) echecs++; };
  const json = JSON.stringify;
  async function page(o = {}) {
    const ctx = await nav.newContext({
      viewport: { width: LARGEUR_FENETRE, height: Math.round(HAUT + HAUTEUR + MARGE_BAS + 80) },
      deviceScaleFactor: o.echelle || 1, colorScheme: o.sombre ? 'dark' : 'light', reducedMotion: o.reduit ? 'reduce' : 'no-preference',
    });
    const p = await ctx.newPage();
    p.on('pageerror', e => erreurs.push(String(e)));
    p.on('console', m => { if (m.type() === 'error' || (m.type() === 'warning' && !o.bruitPermis)) erreurs.push(m.text()); });
    await p.addInitScript(([charge, refus]) => {
      window.__refus = []; document.addEventListener('securitypolicyviolation', e => window.__refus.push(e.violatedDirective));
      window.__appels = []; const ecoutes = {}; let refuses = 0;
      window.__TAURI__ = {
        core: { invoke: (c, a) => {
          window.__appels.push([c, a]);
          if (c === 'barre_pret' && refuses < refus) { refuses++; return Promise.reject('pas encore en place'); }
          if (c === 'barre_pret') return Promise.resolve(charge);
          if (c === 'barre_retardateur') return Promise.resolve(a.secondes);
          return Promise.resolve(null);
        } },
        event: { listen: (n, f) => { ecoutes[n] = f; return Promise.resolve(() => {}); } },
      };
      window.__emettre = (n, payload) => ecoutes[n]({ payload });
    }, [o.charge || charge(PRO), o.refus || 0]);
    await p.goto(base);
    await p.waitForTimeout(300);
    return p;
  }
  const appels = (p, nom) => p.evaluate(n => window.__appels.filter(a => !n || a[0] === n), nom);
  const derniere = async (p, nom) => (await appels(p, nom)).at(-1)?.[1];
  const ordre = (p) => p.evaluate(() => [...document.getElementById('barre').children].map(e =>
    e.classList.contains('marque') ? 'logo' : e.getAttribute('role') === 'separator' ? '|' : e.dataset.id));
  const focus = (p) => p.evaluate(() => document.activeElement?.dataset?.id || document.activeElement?.dataset?.secondes || null);

  const p = await page();
  verifier((await appels(p, 'barre_pret')).length === 1, 'barre_pret une fois');
  verifier(json(await ordre(p)) === json(['logo', 'capturer_zone', 'capturer_plein_ecran', 'capturer_fenetre', '|', 'capturer_defilante', 'film', 'retardateur', '|', 'reglages']),
    'avec licence : logo, Zone, Plein écran, Fenêtre | Capture défilante, Vidéo/GIF, Retardateur | Réglages', json(await ordre(p)));
  const vu = await p.evaluate(() => {
    const b = document.getElementById('barre');
    const r = b.getBoundingClientRect();
    const modes = [...b.querySelectorAll('.mode')];
    return {
      carte: [r.left, r.top, r.width, r.height],
      libelles: modes.map(m => [m.dataset.id, m.querySelector('.libelle')?.textContent || null, m.getAttribute('aria-label'), m.title]),
      pictos: modes.every(m => m.querySelectorAll('svg *').length > 0),
      hauteurs: [...new Set(modes.map(m => m.getBoundingClientRect().height))],
      role: b.getAttribute('role'),
      fond: getComputedStyle(document.body).backgroundColor,
    };
  });
  verifier(vu.role === 'toolbar', 'role toolbar');
  verifier(vu.carte[1] === HAUT && Math.abs(vu.carte[0] + vu.carte[2] / 2 - LARGEUR_FENETRE / 2) <= 0.5, `carte à ${HAUT} px du haut, centrée`, json(vu.carte));
  verifier(vu.carte[3] === HAUTEUR, `carte de ${HAUTEUR} px de haut (celle du cœur)`, json(vu.carte));
  verifier(vu.hauteurs.length === 1 && vu.hauteurs[0] === 36, 'boutons de 36 px', json(vu.hauteurs));
  verifier(vu.pictos, 'un pictogramme sur chaque bouton');
  verifier(json(vu.libelles.slice(0, 3).map(x => [x[1], x[2], x[3]])) === json([['Zone', null, ''], ['Plein écran', null, ''], ['Fenêtre', null, '']]), 'Zone, Plein écran, Fenêtre : libellés, sans info-bulle', json(vu.libelles));
  verifier(json(vu.libelles.slice(3).map(x => [x[2], x[3]])) === json([['Capture défilante', 'Capture défilante'], ["Enregistrer l'écran", "Enregistrer l'écran"], ['Retardateur', 'Retardateur'], ['Réglages', 'Réglages']]),
    'les autres : le nom seul, en info-bulle et pour les lecteurs', json(vu.libelles.slice(3)));
  verifier(vu.fond === 'rgba(0, 0, 0, 0)', 'fond de la page transparent', vu.fond);
  // L'ombre de la carte (et des menus) tient dans la marge de la fenêtre : MARGE sur les
  // côtés, MARGE_BAS en bas, HAUT en haut.
  const ombres = await p.evaluate(() => [...document.querySelectorAll('.carte')].map(e => getComputedStyle(e).boxShadow));
  const debords = ombres.map(o => o.split(/,(?![^(]*\))/).map(c => {
    const [x, y, flou, etale] = (c.match(/-?[\d.]+px/g) || []).map(parseFloat);
    return { cote: Math.abs(x) + flou + (etale || 0), bas: y + flou + (etale || 0), haut: -y + flou + (etale || 0) };
  }));
  const pire = debords.flat().reduce((m, d) => ({ cote: Math.max(m.cote, d.cote), bas: Math.max(m.bas, d.bas), haut: Math.max(m.haut, d.haut) }), { cote: 0, bas: 0, haut: 0 });
  verifier(ombres.length === 3 && pire.cote <= MARGE && pire.bas <= MARGE_BAS && pire.haut <= HAUT, `ombre dans la marge (côtés ${MARGE}, bas ${MARGE_BAS}, haut ${HAUT} px)`, JSON.stringify(pire));
  const t0 = await derniere(p, 'barre_taille');
  verifier(t0 && t0.hauteur === HAUTEUR && t0.largeur === Math.ceil(vu.carte[2]), 'barre_taille : la carte mesurée', json(t0));
  // Réglages : l'icône « Curseurs », jamais un engrenage.
  const reg = await p.evaluate(() => {
    const s = document.querySelector('[data-id=reglages] svg');
    const cs = getComputedStyle(s);
    return {
      formes: [...s.children].map(e => e.tagName === 'path' ? e.getAttribute('d') : `${e.tagName} ${e.getAttribute('cx')} ${e.getAttribute('cy')} ${e.getAttribute('r')}`),
      vue: s.getAttribute('viewBox'), trait: cs.strokeWidth, bouts: cs.strokeLinecap, angles: cs.strokeLinejoin, fond: cs.fill,
      engrenage: document.documentElement.outerHTML.includes('gear'),
    };
  });
  verifier(json(reg.formes) === json(CURSEURS) && reg.vue === '0 0 20 20', 'Réglages : icône « Curseurs » (viewBox 0 0 20 20)', json(reg));
  verifier(reg.trait === '1.4px' && reg.bouts === 'round' && reg.angles === 'round' && reg.fond === 'none' && !reg.engrenage, 'trait 1.4, bouts et angles arrondis, sans remplissage, aucun engrenage', json(reg));
  for (const [nom, sombre] of [['clair', false], ['sombre', true]]) {
    for (const echelle of [1, 2]) {
      const q = await page({ sombre, echelle, charge: charge(PRO, 3) });
      await q.mouse.move(LARGEUR_FENETRE / 2 - 120, HAUT + 20);
      await q.waitForTimeout(250);
      await q.screenshot({ path: path.join(SORTIE, `barre-${nom}-${echelle}x.png`), omitBackground: true });
      if (echelle === 2) {
        await q.click('[data-id=retardateur]');
        await q.waitForTimeout(300);
        await q.screenshot({ path: path.join(SORTIE, `barre-retardateur-${nom}.png`), omitBackground: true });
      }
      await q.context().close();
    }
  }
  {
    // Thèmes : le texte suit le système, et l'attribut posé par le cœur.
    const couleur = (q) => q.evaluate(() => getComputedStyle(document.querySelector('[data-id=capturer_zone]')).color);
    const c = await page(); const s = await page({ sombre: true });
    const [clair, sombre] = [await couleur(c), await couleur(s)];
    await s.evaluate(() => { document.documentElement.dataset.theme = 'light'; });
    const force = await couleur(s);
    verifier(clair !== sombre && force === clair, 'thème clair et sombre, et data-theme du cœur', json([clair, sombre, force]));
    await c.context().close(); await s.context().close();
  }

  // Clic sur « Zone » : barre_choisi une seule fois, même au double clic.
  await p.getByRole('button', { name: 'Zone' }).dblclick();
  await p.waitForTimeout(50);
  verifier(json(await appels(p, 'barre_choisi')) === json([['barre_choisi', { id: 'capturer_zone' }]]), 'clic sur Zone : barre_choisi { id: capturer_zone }, une fois');

  // Cachée : la barre se vide.
  await p.evaluate(() => __emettre('barre:vider'));
  verifier(await p.evaluate(() => document.getElementById('barre').hidden && !document.querySelector('.mode')), 'barre:vider : barre cachée, sans bouton');

  // Nouvelle ouverture : menu Vidéo/GIF.
  await p.evaluate((c) => __emettre('barre:ouvrir', c), charge(PRO, 0));
  await p.waitForTimeout(50);
  verifier(await focus(p) === null, 'ouverte : aucun bouton au focus', await focus(p));
  await p.click('[data-id=film]');
  await p.waitForTimeout(50);
  const film = await p.evaluate(() => {
    const v = document.getElementById('volet-film');
    return { montre: !v.hidden, etendu: document.querySelector('[data-id=film]').getAttribute('aria-expanded'),
      elements: [...v.querySelectorAll('[role=menuitem]')].map(e => [e.dataset.id, e.textContent, e.querySelectorAll('svg *').length > 0]),
      bas: v.offsetTop + v.offsetHeight };
  });
  verifier(film.montre && film.etendu === 'true' && json(film.elements) === json([['capturer_video', 'Vidéo', true], ['capturer_gif', 'GIF', true]]), 'Vidéo/GIF : menu Vidéo, GIF', json(film));
  const t1 = await derniere(p, 'barre_taille');
  verifier(t1 && t1.hauteur === film.bas && t1.hauteur > HAUTEUR, 'menu ouvert : barre_taille avec le menu', json(t1));
  await p.click('[data-id=capturer_gif]');
  await p.waitForTimeout(50);
  verifier(json((await appels(p, 'barre_choisi')).map(a => a[1].id)) === json(['capturer_zone', 'capturer_gif']), 'clic sur GIF : barre_choisi { id: capturer_gif }');

  // Retardateur : même réglage que Réglages › Capture ; la barre reste ouverte.
  await p.evaluate((c) => __emettre('barre:ouvrir', c), charge(PRO, 0));
  await p.click('[data-id=retardateur]');
  await p.waitForTimeout(50);
  const choix = await p.evaluate(() => [...document.querySelectorAll('#volet-retardateur [role=radio]')].map(c => [c.textContent, c.getAttribute('aria-checked')]));
  verifier(json(choix) === json([['Aucun', 'true'], ['3 s', 'false'], ['5 s', 'false'], ['10 s', 'false']]), 'Retardateur : Aucun, 3 s, 5 s, 10 s ; Aucun coché', json(choix));
  verifier(await focus(p) === '0', 'menu ouvert : le choix coché au focus', await focus(p));
  await p.getByRole('radio', { name: '5 s' }).click();
  await p.waitForTimeout(50);
  const arme = await p.evaluate(() => {
    const b = document.querySelector('[data-id=retardateur]');
    return { arme: b.classList.contains('arme'), badge: b.querySelector('.badge').textContent, nom: b.getAttribute('aria-label'), titre: b.title,
      volet: document.getElementById('volet-retardateur').hidden, transforme: getComputedStyle(b.querySelector('.badge')).transform };
  });
  verifier(json(await derniere(p, 'barre_retardateur')) === json({ secondes: 5 }), 'choix 5 s : barre_retardateur { secondes: 5 }');
  verifier(arme.arme && arme.badge === '5' && arme.nom === 'Retardateur, 5 s' && arme.titre === 'Retardateur' && arme.volet, 'badge « 5 », menu refermé, info-bulle : le nom seul', json(arme));
  verifier((await appels(p, 'barre_fermer')).length === 0 && (await appels(p, 'barre_choisi')).length === 2 && await p.evaluate(() => !document.getElementById('barre').hidden), 'la barre reste ouverte');
  verifier(await focus(p) === 'retardateur', 'le focus revient au bouton Retardateur', await focus(p));
  verifier((await derniere(p, 'barre_taille')).hauteur === HAUTEUR, 'menu fermé : barre_taille sans le menu', json(await derniere(p, 'barre_taille')));
  // Le badge suit le réglage reçu à l'ouverture (7 s, du fichier : aucun choix coché).
  await p.evaluate((c) => __emettre('barre:ouvrir', c), charge(PRO, 7));
  const sept = await p.evaluate(() => [document.querySelector('[data-id=retardateur] .badge').textContent,
    [...document.querySelectorAll('#volet-retardateur [aria-checked=true]')].length]);
  verifier(json(sept) === json(['7', 0]), 'retardateur 7 s reçu : badge « 7 », aucun choix coché', json(sept));
  await p.evaluate((c) => __emettre('barre:ouvrir', c), charge(PRO, 3));

  // Clavier : flèches, Tab, Début, Fin, Entrée, Espace ; Flèche bas ouvre un menu.
  await p.keyboard.press('ArrowRight');
  verifier(await focus(p) === 'capturer_zone', 'Flèche droite : le premier bouton', await focus(p));
  await p.keyboard.press('ArrowLeft');
  verifier(await focus(p) === 'reglages', 'Flèche gauche depuis le premier : le dernier', await focus(p));
  await p.keyboard.press('Tab');
  verifier(await focus(p) === 'capturer_zone', 'Tab depuis le dernier : le premier', await focus(p));
  await p.keyboard.press('Shift+Tab');
  verifier(await focus(p) === 'reglages', 'Maj + Tab depuis le premier : le dernier', await focus(p));
  await p.keyboard.press('Home');
  await p.keyboard.press('Tab');
  await p.keyboard.press('ArrowRight');
  await p.keyboard.press('ArrowRight');
  await p.keyboard.press('ArrowRight');
  await p.keyboard.press('ArrowRight');
  verifier(await focus(p) === 'retardateur', 'Début, Tab et flèches : d\'un bouton à l\'autre', await focus(p));
  await p.keyboard.press('ArrowDown');
  verifier(await focus(p) === '3' && await p.evaluate(() => !document.getElementById('volet-retardateur').hidden), 'Flèche bas : le menu du retardateur, 3 s au focus', await focus(p));
  await p.keyboard.press('ArrowRight');
  await p.keyboard.press('ArrowRight');
  await p.keyboard.press('ArrowRight');
  verifier(await focus(p) === '0', 'flèches dans le menu : en boucle', await focus(p));
  await p.keyboard.press('Escape');
  verifier(await focus(p) === 'retardateur' && (await appels(p, 'barre_fermer')).length === 0, 'Échap menu ouvert : le menu seul se ferme', await focus(p));
  await p.keyboard.press('Enter');
  await p.keyboard.press('End');
  verifier(await focus(p) === '10', 'Entrée : le menu ; Fin : le dernier choix', await focus(p));
  await p.keyboard.press('Enter');
  await p.waitForTimeout(50);
  verifier(json(await derniere(p, 'barre_retardateur')) === json({ secondes: 10 }) && await focus(p) === 'retardateur', 'Entrée sur 10 s : barre_retardateur { secondes: 10 }', await focus(p));
  await p.keyboard.press('ArrowLeft');
  await p.keyboard.press('ArrowLeft');
  await p.keyboard.press('ArrowLeft');
  await p.keyboard.press('ArrowLeft');
  verifier(await focus(p) === 'capturer_plein_ecran', 'flèche gauche : Plein écran', await focus(p));
  await p.keyboard.press('Enter');
  await p.waitForTimeout(50);
  verifier((await derniere(p, 'barre_choisi'))?.id === 'capturer_plein_ecran', 'Entrée : barre_choisi { id: capturer_plein_ecran }');
  await p.evaluate((c) => __emettre('barre:ouvrir', c), charge(PRO, 0));
  await p.keyboard.press('ArrowRight');
  await p.keyboard.press('ArrowRight');
  await p.keyboard.press('ArrowRight');
  await p.keyboard.press(' ');
  await p.waitForTimeout(50);
  verifier((await derniere(p, 'barre_choisi'))?.id === 'capturer_fenetre', 'Espace : barre_choisi { id: capturer_fenetre }');
  // Échap ferme la barre sans rien choisir.
  await p.evaluate((c) => __emettre('barre:ouvrir', c), charge(PRO, 0));
  const nChoix = (await appels(p, 'barre_choisi')).length;
  await p.keyboard.press('Escape');
  await p.waitForTimeout(50);
  verifier((await appels(p, 'barre_fermer')).length === 1 && (await appels(p, 'barre_choisi')).length === nChoix, 'Échap : barre_fermer, rien de choisi');
  // Clic dans la marge transparente : la barre se ferme ; sur la carte hors bouton : non.
  await p.mouse.click(LARGEUR_FENETRE / 2, HAUT + HAUTEUR + 6);
  await p.waitForTimeout(50);
  const logo = await p.evaluate(() => { const r = document.querySelector('.marque').getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; });
  await p.mouse.click(logo[0], logo[1]);
  await p.waitForTimeout(50);
  verifier((await appels(p, 'barre_fermer')).length === 2, 'clic dans la marge : barre_fermer ; sur le logo : rien');
  // Réglages : choisi comme l'entrée du menu.
  await p.getByRole('button', { name: 'Réglages' }).click();
  await p.waitForTimeout(50);
  verifier((await derniere(p, 'barre_choisi'))?.id === 'reglages', 'clic sur Réglages : barre_choisi { id: reglages }');
  // Charge mal formée ou identifiant inconnu : ignoré sans casser la page.
  await p.evaluate(() => __emettre('barre:ouvrir', { boutons: [3, 'quitter', 'capturer_zone'], retardateur_s: null }));
  verifier(json(await ordre(p)) === json(['logo', 'capturer_zone']), 'identifiant inconnu ignoré, sans retardateur (null)', json(await ordre(p)));
  await p.evaluate(() => __emettre('barre:ouvrir', { boutons: 'x' }));
  verifier(json(await ordre(p)) === json(['logo', 'capturer_zone']), 'charge mal formée ignorée');
  await p.context().close();

  {
    // Sans licence : Zone, Plein écran, Fenêtre | Retardateur | Réglages.
    const q = await page({ charge: charge(BASE, 0) });
    verifier(json(await ordre(q)) === json(['logo', 'capturer_zone', 'capturer_plein_ecran', 'capturer_fenetre', '|', 'retardateur', '|', 'reglages']), 'sans licence : rien de Pro', json(await ordre(q)));
    await q.context().close();
    // Sans Media Foundation : GIF sans Vidéo, le menu n'a que GIF.
    const r = await page({ charge: charge(PRO.filter(b => b !== 'capturer_video'), 0) });
    await r.click('[data-id=film]');
    const g = await r.evaluate(() => [...document.querySelectorAll('#volet-film [role=menuitem]')].map(e => e.dataset.id));
    verifier(json(g) === json(['capturer_gif']), 'sans Media Foundation : GIF seul au menu', json(g));
    await r.context().close();
    // Sans retardateur (capture indisponible) ni Pro : un seul séparateur.
    const s = await page({ charge: charge(BASE, null) });
    verifier(json(await ordre(s)) === json(['logo', 'capturer_zone', 'capturer_plein_ecran', 'capturer_fenetre', '|', 'reglages']), 'sans retardateur : Réglages après un séparateur', json(await ordre(s)));
    await s.context().close();
  }
  {
    // Mouvement réduit : ni ressort, ni montée, ni animation du menu.
    const q = await page({ reduit: true });
    await q.click('[data-id=retardateur]');
    const m = await q.evaluate(() => ({
      volet: getComputedStyle(document.getElementById('volet-retardateur')).animationName,
      mode: getComputedStyle(document.querySelector('.mode')).transitionProperty,
      badge: getComputedStyle(document.querySelector('.badge')).transitionDuration,
    }));
    await q.hover('[data-id=capturer_zone]');
    await q.waitForTimeout(300);
    const survol = await q.evaluate(() => getComputedStyle(document.querySelector('[data-id=capturer_zone]')).transform);
    verifier(m.volet === 'none' && !m.mode.includes('transform') && m.badge === '0s' && survol === 'none', 'mouvement réduit respecté', json({ ...m, survol }));
    await q.context().close();
  }
  {
    // Cœur pas encore en place : deux refus, boutons reçus au troisième essai.
    const q = await page({ refus: 2 });
    await q.waitForTimeout(400);
    const n = (await appels(q, 'barre_pret')).length;
    const b = await q.evaluate(() => document.querySelectorAll('.mode').length);
    verifier(n === 3 && b === 7, 'barre_pret rejeté deux fois : retenté, boutons au troisième', `${n} appel(s), ${b} bouton(s)`);
    await q.context().close();
  }

  verifier(erreurs.length === 0, 'aucune erreur ni avertissement dans la console', erreurs.join(' | '));
  const csp = await (async () => { const q = await page(); await q.click('[data-id=film]'); const r = await q.evaluate(() => window.__refus); await q.context().close(); return r; })();
  verifier(csp.length === 0, 'aucune violation de la CSP', JSON.stringify(csp));
  await nav.close();
  serveur.close();
  console.log(echecs ? `${echecs} échec(s)` : 'tout est vert');
  process.exit(echecs ? 1 : 0);
})().catch(e => { console.error(e); process.exit(1); });
