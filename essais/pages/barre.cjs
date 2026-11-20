const { chromium } = require('playwright');
const http = require('http'), fs = require('fs'), path = require('path');
// Essai de la page de la barre de capture « Galet » (design/barre-capture, piste A) dans
// Chromium (Playwright) : boutons montrés selon la charge reçue (`barre_pret`, puis
// `barre:ouvrir`), mesures égales à celles du cœur (fenetres/barre.rs), taille rendue
// (`barre_taille`, étage du retardateur compris), choix envoyé, boutons Pro éteints, nom au
// survol, étage du retardateur, délai écrit dans le cadran (le « 10 » en traits), clavier,
// Échap, clic dans la marge, mouvement réduit, CSP. Captures claire et sombre pour relecture.
// Sort avec 1 si une vérification échoue.
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
const CAPTURES = ['capturer_zone', 'capturer_plein_ecran', 'capturer_fenetre', 'capturer_pipette'];
const FILMS = ['capturer_defilante', 'capturer_video', 'capturer_gif'];
const TOUT = [...CAPTURES, ...FILMS, 'reglages'];
const charge = (boutons, retardateur_s = 0, verrouilles = []) => ({ boutons, verrouilles, retardateur_s, retardateurs: [0, 3, 5, 10] });
const AVEC = charge(TOUT, 0);
const SANS = charge(TOUT, 0, FILMS);
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
    p.on('console', m => { if (m.type() === 'error' || m.type() === 'warning') erreurs.push(m.text()); });
    await p.addInitScript(([charge, refus]) => {
      window.__refus = []; document.addEventListener('securitypolicyviolation', e => window.__refus.push(e.violatedDirective));
      window.__appels = []; const ecoutes = {}; let refuses = 0;
      window.__TAURI__ = {
        core: { invoke: (c, a) => {
          window.__appels.push([c, a]);
          if (c === 'barre_pret' && refuses < refus) { refuses++; return Promise.reject('pas encore en place'); }
          if (c === 'barre_pret') return Promise.resolve(charge);
          if (c === 'barre_retardateur') return window.__lent ? new Promise(r => setTimeout(() => r(a.secondes), 300)) : Promise.resolve(a.secondes);
          return Promise.resolve(null);
        } },
        event: { listen: (n, f) => { ecoutes[n] = f; return Promise.resolve(() => {}); } },
      };
      window.__emettre = (n, payload) => ecoutes[n]({ payload });
    }, [o.charge || AVEC, o.refus || 0]);
    await p.goto(base);
    await p.waitForTimeout(400);
    return p;
  }
  const appels = (p, nom) => p.evaluate(n => window.__appels.filter(a => !n || a[0] === n), nom);
  const derniere = async (p, nom) => (await appels(p, nom)).at(-1)?.[1];
  const ordre = (p) => p.evaluate(() => [...document.querySelectorAll('#barre .rang > *')].map(e =>
    e.getAttribute('role') === 'separator' ? '|' : e.classList.contains('puce') ? 'puce' : e.dataset.id));
  const focus = (p) => p.evaluate(() => document.activeElement?.dataset?.id || document.activeElement?.dataset?.secondes || null);
  const ouvrir = async (p, c) => { await p.evaluate((x) => __emettre('barre:ouvrir', x), c); await p.waitForTimeout(80); };
  const nomVu = (p) => p.evaluate(() => { const n = document.getElementById('nom'); return n.classList.contains('vu') ? n.textContent : null; });

  // ---- Avec licence --------------------------------------------------------------
  const p = await page();
  verifier((await appels(p, 'barre_pret')).length === 1, 'barre_pret une fois');
  verifier(json(await ordre(p)) === json(['capturer_zone', 'capturer_plein_ecran', 'capturer_fenetre', 'capturer_pipette', 'retardateur', '|', 'capturer_defilante', 'capturer_video', 'capturer_gif', '|', 'reglages']),
    'avec licence : Zone, Plein écran, Fenêtre, Pipette, Retardateur | Défilante, Vidéo, GIF | Réglages, sans logo ni puce', json(await ordre(p)));
  const vu = await p.evaluate(() => {
    const b = document.getElementById('barre');
    const r = b.getBoundingClientRect();
    const boutons = [...b.querySelectorAll('.b')];
    return {
      carte: [r.left, r.top, r.width, r.height],
      rayon: getComputedStyle(b).borderTopLeftRadius,
      noms: boutons.map(m => [m.dataset.id, m.getAttribute('aria-label'), m.title, m.textContent]),
      pictos: boutons.every(m => m.querySelectorAll('svg *').length > 0),
      tailles: [...new Set(boutons.map(m => `${m.getBoundingClientRect().width}x${m.getBoundingClientRect().height}`))],
      pictoTaille: [...new Set(boutons.map(m => `${m.querySelector('svg').getBoundingClientRect().width}`))],
      role: b.getAttribute('role'),
      fond: getComputedStyle(document.body).backgroundColor,
      logo: !!document.querySelector('.logo, .marque'),
      volets: document.querySelectorAll('.volet').length,
    };
  });
  verifier(vu.role === 'toolbar', 'role toolbar');
  verifier(vu.carte[1] === HAUT && Math.abs(vu.carte[0] + vu.carte[2] / 2 - LARGEUR_FENETRE / 2) <= 0.5, `carte à ${HAUT} px du haut, centrée`, json(vu.carte));
  verifier(vu.carte[3] === HAUTEUR, `carte de ${HAUTEUR} px de haut (celle du cœur)`, json(vu.carte));
  verifier(vu.rayon === '20px', 'carte au rayon 20', vu.rayon);
  verifier(json(vu.tailles) === json(['44x44']) && json(vu.pictoTaille) === json(['22']), 'boutons de 44 px, pictogrammes de 22 px', json([vu.tailles, vu.pictoTaille]));
  verifier(vu.pictos && !vu.logo && vu.volets === 0, 'un pictogramme sur chaque bouton ; ni logo ni menu volant');
  verifier(json(vu.noms.map(x => [x[1], x[2], x[3]])) === json([['Zone', '', ''], ['Plein écran', '', ''], ['Fenêtre', '', ''], ['Pipette de couleur', '', ''], ['Retardateur', '', ''],
    ['Capture défilante', '', ''], ['Vidéo', '', ''], ['GIF', '', ''], ['Réglages', '', '']]),
    'pictogrammes seuls : le nom en aria-label, sans mot ni info-bulle (title)', json(vu.noms));
  verifier(vu.fond === 'rgba(0, 0, 0, 0)', 'fond de la page transparent', vu.fond);
  // L'ombre de la carte et de l'étiquette du nom tient dans la marge de la fenêtre.
  const ombres = await p.evaluate(() => [...document.querySelectorAll('.carte, .nom')].map(e => getComputedStyle(e).boxShadow));
  const debords = ombres.map((o, i) => o.split(/,(?![^(]*\))/).map(c => {
    const [x, y, flou, etale] = (c.match(/-?[\d.]+px/g) || []).map(parseFloat);
    return { cote: Math.abs(x) + flou + (etale || 0), bas: y + flou + (etale || 0), haut: -y + flou + (etale || 0), i };
  }));
  const pire = debords.flat().reduce((m, d) => ({ cote: Math.max(m.cote, d.cote), bas: Math.max(m.bas, d.bas), haut: Math.max(m.haut, d.haut) }), { cote: 0, bas: 0, haut: 0 });
  // L'étiquette : 8 px sous la carte, 26 px de haut, puis son ombre ; la carte : son ombre.
  const nomBas = 8 + 26 + debords[1].reduce((m, d) => Math.max(m, d.bas), 0);
  verifier(pire.cote <= MARGE && pire.bas <= MARGE_BAS && nomBas <= MARGE_BAS, `ombres dans la marge (côtés ${MARGE}, bas ${MARGE_BAS})`, json({ ...pire, nomBas }));
  verifier(pire.haut - HAUT <= 2.5, `ombre du haut : au plus 2,5 px de queue au-dessus de la fenêtre (HAUT ${HAUT})`, json(pire));
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
  verifier(reg.trait === '1.45px' && reg.bouts === 'round' && reg.angles === 'round' && reg.fond === 'none' && !reg.engrenage, 'trait 1,45, bouts et angles arrondis, sans remplissage, aucun engrenage', json(reg));

  // Nom au survol : sous la barre, centré sur le bouton ; Réglages ne dépasse pas la marge.
  await p.hover('[data-id=capturer_plein_ecran]');
  await p.waitForTimeout(300);
  const survol = await p.evaluate(() => {
    const n = document.getElementById('nom').getBoundingClientRect();
    const b = document.querySelector('[data-id=capturer_plein_ecran]').getBoundingClientRect();
    const c = document.getElementById('barre').getBoundingClientRect();
    return { texte: document.getElementById('nom').textContent, centre: Math.abs(n.left + n.width / 2 - (b.left + b.width / 2)), ecart: n.top - c.bottom, bas: n.bottom - c.bottom };
  });
  verifier(await nomVu(p) === 'Plein écran' && survol.centre <= 1 && Math.abs(survol.ecart - 8) <= 1, 'survol : « Plein écran » sous le bouton, centré, à 8 px', json(survol));
  await p.hover('[data-id=reglages]');
  await p.waitForTimeout(300);
  const dernier = await p.evaluate(() => {
    const n = document.getElementById('nom').getBoundingClientRect();
    const c = document.getElementById('barre').getBoundingClientRect();
    return { debord: n.right - c.right, gauche: n.left - c.left };
  });
  verifier(await nomVu(p) === 'Réglages' && dernier.debord <= MARGE - 14, 'survol de Réglages : l\'étiquette déborde de la carte de moins de la marge moins son ombre', json(dernier));
  await p.hover('[data-id=capturer_zone]');
  await p.waitForTimeout(300);
  const premier = await p.evaluate(() => { const n = document.getElementById('nom').getBoundingClientRect(); const c = document.getElementById('barre').getBoundingClientRect(); return c.left - n.left; });
  verifier(await nomVu(p) === 'Zone' && premier <= MARGE - 14, 'survol de Zone : l\'étiquette ne dépasse pas à gauche', String(premier));
  await p.mouse.move(2, 2);
  await p.waitForTimeout(200);
  verifier(await nomVu(p) === null, 'souris hors de la barre : plus de nom');
  await p.keyboard.press('Tab');
  await p.waitForTimeout(200);
  verifier(await nomVu(p) === 'Zone', 'focus clavier : le nom du bouton', String(await nomVu(p)));
  await p.keyboard.press('ArrowRight');
  await p.waitForTimeout(250);
  verifier(await nomVu(p) === 'Plein écran', 'flèche droite : le nom suit le focus', String(await nomVu(p)));
  await ouvrir(p, AVEC);

  // Captures claire et sombre pour relecture.
  for (const [nom, sombre] of [['clair', false], ['sombre', true]]) {
    for (const echelle of [1, 2]) {
      const q = await page({ sombre, echelle, charge: charge(TOUT, 3, FILMS) });
      await q.mouse.move(LARGEUR_FENETRE / 2 - 120, HAUT + 20);
      await q.waitForTimeout(300);
      await q.screenshot({ path: path.join(SORTIE, `barre-${nom}-${echelle}x.png`), omitBackground: true });
      if (echelle === 2) {
        await q.click('[data-id=retardateur]');
        await q.waitForTimeout(500);
        await q.screenshot({ path: path.join(SORTIE, `barre-etage-${nom}.png`), omitBackground: true });
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

  // Cachée : la barre se vide, le nom aussi.
  await p.hover('[data-id=capturer_zone]');
  await p.evaluate(() => __emettre('barre:vider'));
  verifier(await p.evaluate(() => document.getElementById('barre').hidden && !document.querySelector('.b') && !document.getElementById('nom').classList.contains('vu')), 'barre:vider : barre cachée, sans bouton ni nom');

  // Vidéo et GIF : deux boutons directs, un clic = barre_choisi.
  await ouvrir(p, AVEC);
  verifier(await focus(p) === null, 'ouverte : aucun bouton au focus', await focus(p));
  await p.click('[data-id=capturer_video]');
  await p.waitForTimeout(50);
  await ouvrir(p, AVEC);
  await p.click('[data-id=capturer_gif]');
  await p.waitForTimeout(50);
  await ouvrir(p, AVEC);
  await p.click('[data-id=capturer_defilante]');
  await p.waitForTimeout(50);
  verifier(json((await appels(p, 'barre_choisi')).map(a => a[1].id)) === json(['capturer_zone', 'capturer_video', 'capturer_gif', 'capturer_defilante']), 'Vidéo, GIF, Défilante : trois boutons directs, barre_choisi de chacun');

  // ---- Étage du retardateur ---------------------------------------------------------
  await ouvrir(p, AVEC);
  const avantEtage = (await appels(p, 'barre_taille')).length;
  await p.click('[data-id=retardateur]');
  await p.waitForTimeout(500);
  const etage = await p.evaluate(() => {
    const e = document.getElementById('etage-retardateur');
    const c = document.getElementById('barre').getBoundingClientRect();
    return { ouvert: e.classList.contains('ouvert'), etendu: document.querySelector('[data-id=retardateur]').getAttribute('aria-expanded'),
      choix: [...e.querySelectorAll('[role=radio]')].map(x => [x.textContent, x.getAttribute('aria-checked')]),
      hauteur: c.height, largeurChoix: [...e.querySelectorAll('.choix')].map(x => Math.round(x.getBoundingClientRect().width)),
      largeurEtage: Math.round(e.querySelector('.etage-rang').getBoundingClientRect().width) };
  });
  verifier(etage.ouvert && etage.etendu === 'true' && json(etage.choix) === json([['Aucun', 'true'], ['3 s', 'false'], ['5 s', 'false'], ['10 s', 'false']]), 'Retardateur : l\'étage Aucun, 3 s, 5 s, 10 s ; Aucun coché', json(etage));
  verifier(new Set(etage.largeurChoix).size === 1, 'quatre choix de même largeur, sur toute la largeur de la carte', json(etage));
  const tailles = (await appels(p, 'barre_taille')).slice(avantEtage);
  verifier(tailles.length === 1 && tailles[0][1].hauteur === Math.round(etage.hauteur) && tailles[0][1].hauteur > HAUTEUR, 'étage ouvert : barre_taille à la hauteur finale, une seule fois', json(tailles));
  verifier(await focus(p) === '0', 'étage ouvert : le choix coché au focus', await focus(p));
  await p.getByRole('radio', { name: '5 s' }).click();
  await p.waitForTimeout(80);
  const arme = await p.evaluate(() => {
    const b = document.querySelector('[data-id=retardateur]');
    return { arme: b.classList.contains('arme'), nom: b.getAttribute('aria-label'), titre: b.title, badge: !!b.querySelector('.badge'),
      chiffres: b.querySelectorAll('.chiffres > *').length, texte: b.querySelectorAll('svg text').length,
      couleur: getComputedStyle(b).color, ouvert: document.getElementById('etage-retardateur').classList.contains('ouvert') };
  });
  verifier(json(await derniere(p, 'barre_retardateur')) === json({ secondes: 5 }), 'choix 5 s : barre_retardateur { secondes: 5 }');
  verifier(arme.arme && arme.nom === 'Retardateur, 5 s' && arme.titre === '' && !arme.badge && arme.chiffres === 1 && arme.texte === 0, 'cadran armé : le « 5 » en traits (pas de texte, pas de badge)', json(arme));
  verifier(arme.ouvert, 'la pastille se pose avant que l\'étage se referme (170 ms)');
  await p.waitForTimeout(700);
  const ferme = await p.evaluate(() => ({ ouvert: document.getElementById('etage-retardateur').classList.contains('ouvert'), hauteur: document.getElementById('barre').getBoundingClientRect().height }));
  verifier(!ferme.ouvert && ferme.hauteur === HAUTEUR, 'étage refermé : la carte reprend ses 58 px', json(ferme));
  verifier((await derniere(p, 'barre_taille')).hauteur === HAUTEUR, 'étage fermé : barre_taille rétrécit une fois l\'étage replié', json(await derniere(p, 'barre_taille')));
  verifier((await appels(p, 'barre_fermer')).length === 0 && await p.evaluate(() => !document.getElementById('barre').hidden), 'la barre reste ouverte');
  verifier(await focus(p) === 'retardateur', 'le focus revient au bouton Retardateur', await focus(p));
  // Le délai reçu à l'ouverture : 10 s (deux chiffres, en traits, dans le cadran) ; 7 s : texte.
  await ouvrir(p, charge(TOUT, 10));
  const dix = await p.evaluate(() => {
    const b = document.querySelector('[data-id=retardateur]');
    const c = b.querySelector('circle').getBoundingClientRect();
    const g = b.querySelector('.chiffres').getBoundingClientRect();
    return { chiffres: b.querySelectorAll('.chiffres > *').length, dedans: g.left > c.left && g.right < c.right && g.top > c.top && g.bottom < c.bottom,
      largeur: g.width, hauteur: g.height, cadran: c.width, coches: [...document.querySelectorAll('.choix[aria-checked=true]')].map(x => x.textContent) };
  });
  verifier(dix.chiffres === 2 && dix.dedans && dix.hauteur >= 7 && dix.coches.join() === '10 s', 'cadran « 10 » : deux chiffres en traits, dans le cadran, 7 px de haut au moins', json(dix));
  await ouvrir(p, charge(TOUT, 7));
  const sept = await p.evaluate(() => [document.querySelectorAll('[data-id=retardateur] svg text').length, [...document.querySelectorAll('.choix[aria-checked=true]')].length]);
  verifier(json(sept) === json([1, 0]), 'délai 7 s (du fichier) : écrit en texte, aucun choix coché', json(sept));
  await ouvrir(p, charge(TOUT, 3));

  // ---- Clavier ------------------------------------------------------------------
  await p.keyboard.press('ArrowRight');
  verifier(await focus(p) === 'capturer_zone', 'Flèche droite : le premier bouton', await focus(p));
  await p.keyboard.press('ArrowLeft');
  verifier(await focus(p) === 'reglages', 'Flèche gauche depuis le premier : le dernier', await focus(p));
  await p.keyboard.press('Tab');
  verifier(await focus(p) === 'capturer_zone', 'Tab depuis le dernier : le premier', await focus(p));
  await p.keyboard.press('Shift+Tab');
  verifier(await focus(p) === 'reglages', 'Maj + Tab depuis le premier : le dernier', await focus(p));
  await p.keyboard.press('Home');
  verifier(await focus(p) === 'capturer_zone', 'Début : le premier');
  await p.keyboard.press('ArrowRight'); await p.keyboard.press('ArrowRight'); await p.keyboard.press('ArrowRight'); await p.keyboard.press('ArrowRight');
  verifier(await focus(p) === 'retardateur', 'flèches : d\'un bouton à l\'autre, Retardateur au cinquième', await focus(p));
  await p.keyboard.press('ArrowDown');
  await p.waitForTimeout(500);
  verifier(await focus(p) === '3' && await p.evaluate(() => document.getElementById('etage-retardateur').classList.contains('ouvert')), 'Flèche bas : l\'étage du retardateur, 3 s au focus', await focus(p));
  await p.keyboard.press('ArrowRight'); await p.keyboard.press('ArrowRight'); await p.keyboard.press('ArrowRight');
  verifier(await focus(p) === '0', 'flèches dans l\'étage : en boucle', await focus(p));
  await p.keyboard.press('Escape');
  await p.waitForTimeout(100);
  verifier(await focus(p) === 'retardateur' && (await appels(p, 'barre_fermer')).length === 0, 'Échap étage ouvert : l\'étage seul se ferme', await focus(p));
  await p.keyboard.press('Enter');
  await p.waitForTimeout(500);
  await p.keyboard.press('End');
  verifier(await focus(p) === '10', 'Entrée : l\'étage ; Fin : le dernier choix', await focus(p));
  await p.keyboard.press('Enter');
  await p.waitForTimeout(300);
  verifier(json(await derniere(p, 'barre_retardateur')) === json({ secondes: 10 }) && await focus(p) === 'retardateur', 'Entrée sur 10 s : barre_retardateur { secondes: 10 }, focus au bouton', await focus(p));
  await p.keyboard.press('ArrowLeft'); await p.keyboard.press('ArrowLeft'); await p.keyboard.press('ArrowLeft');
  verifier(await focus(p) === 'capturer_plein_ecran', 'flèche gauche : Plein écran', await focus(p));
  await p.keyboard.press('Enter');
  await p.waitForTimeout(50);
  verifier((await derniere(p, 'barre_choisi'))?.id === 'capturer_plein_ecran', 'Entrée : barre_choisi { id: capturer_plein_ecran }');
  await ouvrir(p, charge(TOUT, 0));
  await p.keyboard.press('ArrowRight'); await p.keyboard.press('ArrowRight'); await p.keyboard.press('ArrowRight');
  await p.keyboard.press(' ');
  await p.waitForTimeout(50);
  verifier((await derniere(p, 'barre_choisi'))?.id === 'capturer_fenetre', 'Espace : barre_choisi { id: capturer_fenetre }');
  // Échap ferme la barre sans rien choisir.
  await ouvrir(p, charge(TOUT, 0));
  const nChoix = (await appels(p, 'barre_choisi')).length;
  await p.keyboard.press('Escape');
  await p.waitForTimeout(50);
  verifier((await appels(p, 'barre_fermer')).length === 1 && (await appels(p, 'barre_choisi')).length === nChoix, 'Échap : barre_fermer, rien de choisi');
  // Clic dans la marge transparente : la barre se ferme ; sur la carte hors bouton : non.
  await p.mouse.click(LARGEUR_FENETRE / 2, HAUT + HAUTEUR + 6);
  await p.waitForTimeout(50);
  const filet = await p.evaluate(() => { const r = document.querySelector('.filet').getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; });
  await p.mouse.click(filet[0], filet[1]);
  await p.waitForTimeout(50);
  verifier((await appels(p, 'barre_fermer')).length === 2, 'clic dans la marge : barre_fermer ; sur un filet de la carte : rien');
  // Un clic dans la marge, étage ouvert : l'étage se replie et la barre se ferme.
  await p.click('[data-id=retardateur]');
  await p.waitForTimeout(400);
  await p.mouse.click(LARGEUR_FENETRE / 2, HAUT + 160);
  await p.waitForTimeout(500);
  verifier((await appels(p, 'barre_fermer')).length === 3 && !(await p.evaluate(() => document.getElementById('etage-retardateur').classList.contains('ouvert'))), 'clic dans la marge, étage ouvert : l\'étage se replie, barre_fermer');
  // Réglages : choisi comme l'entrée du menu.
  await ouvrir(p, AVEC);
  await p.getByRole('button', { name: 'Réglages' }).click();
  await p.waitForTimeout(50);
  verifier((await derniere(p, 'barre_choisi'))?.id === 'reglages', 'clic sur Réglages : barre_choisi { id: reglages }');
  // Charge mal formée ou identifiant inconnu : ignoré sans casser la page.
  await p.evaluate(() => __emettre('barre:ouvrir', { boutons: [3, 'quitter', 'capturer_zone'], retardateur_s: null }));
  verifier(json(await ordre(p)) === json(['capturer_zone']), 'identifiant inconnu ignoré, sans retardateur (null)', json(await ordre(p)));
  await p.evaluate(() => __emettre('barre:ouvrir', { boutons: 'x' }));
  verifier(json(await ordre(p)) === json(['capturer_zone']), 'charge mal formée ignorée');
  // Une réponse tardive d'une ouverture passée ne touche pas la barre suivante.
  await ouvrir(p, AVEC);
  await p.evaluate(() => { window.__lent = true; });
  await p.click('[data-id=retardateur]');
  await p.waitForTimeout(400);
  await p.getByRole('radio', { name: '5 s' }).click();
  await p.evaluate(() => __emettre('barre:vider'));
  await ouvrir(p, AVEC);
  await p.waitForTimeout(500);
  const tardive = await p.evaluate(() => document.querySelector('[data-id=retardateur]').classList.contains('arme'));
  await p.evaluate(() => { window.__lent = false; });
  verifier(!tardive, 'réponse de barre_retardateur arrivée après une nouvelle ouverture : ignorée (cadran non armé)');
  // La minuterie de fermeture (170 ms) d'une ouverture passée ne referme pas l'étage de la suivante.
  await p.click('[data-id=retardateur]');
  await p.waitForTimeout(400);
  await p.getByRole('radio', { name: '3 s' }).click();
  // Nouvelle ouverture et clic sur Retardateur dans le même instant, avant les 170 ms.
  await p.evaluate((c) => { __emettre('barre:ouvrir', c); document.querySelector('[data-id=retardateur]').click(); }, AVEC);
  await p.waitForTimeout(600);
  verifier(await p.evaluate(() => document.getElementById('etage-retardateur').classList.contains('ouvert')), 'minuterie de fermeture annulée par la nouvelle ouverture : l\'étage rouvert reste ouvert');
  // Barre:vider avant la fin de la minuterie : aucune taille périmée, barre toujours cachée.
  await p.getByRole('radio', { name: '3 s' }).click();
  await p.evaluate(() => __emettre('barre:vider'));
  const nTaille = (await appels(p, 'barre_taille')).length;
  await p.waitForTimeout(600);
  verifier((await appels(p, 'barre_taille')).length === nTaille && await p.evaluate(() => document.getElementById('barre').hidden), 'barre:vider avant la fin de la minuterie : aucune taille périmée, barre toujours cachée');
  // L'étiquette est vidée avec la barre : aucun nom fantôme à la réouverture.
  await ouvrir(p, AVEC);
  await p.hover('[data-id=capturer_zone]');
  await p.waitForTimeout(250);
  await p.evaluate(() => __emettre('barre:vider'));
  await p.mouse.move(2, 2);
  await ouvrir(p, AVEC);
  const fantome = await p.evaluate(() => { const n = document.getElementById('nom'); return [n.textContent, n.classList.contains('vu'), getComputedStyle(n).opacity]; });
  verifier(fantome[0] === '' && !fantome[1] && fantome[2] === '0', 'réouverture : l\'étiquette du nom est vide et invisible', json(fantome));
  // Étage ouvert : pas de nom au survol d'un autre bouton ; Tab replie l'étage, focus au bouton ; la taille n'est rendue qu'une fois l'étage replié.
  await p.click('[data-id=retardateur]');
  await p.waitForTimeout(500);
  await p.hover('[data-id=capturer_zone]');
  await p.waitForTimeout(250);
  verifier(await nomVu(p) === null, 'étage ouvert : aucun nom au survol d\'un bouton');
  await p.keyboard.press('Tab');
  await p.waitForTimeout(50);
  verifier(await focus(p) === 'retardateur' && !(await p.evaluate(() => document.getElementById('etage-retardateur').classList.contains('ouvert'))), 'Tab dans l\'étage : l\'étage se replie, le focus revient au bouton Retardateur', await focus(p));
  const nAvant = (await appels(p, 'barre_taille')).length;
  await p.click('[data-id=retardateur]');
  await p.waitForTimeout(500);
  const nOuvert = (await appels(p, 'barre_taille')).length;
  await p.getByRole('radio', { name: '5 s' }).click();
  await p.waitForTimeout(170 + 130);
  const pendant = (await appels(p, 'barre_taille')).length;
  await p.waitForTimeout(500);
  const apres = await appels(p, 'barre_taille');
  verifier(nOuvert === nAvant + 1 && pendant === nOuvert && apres.length === nOuvert + 1 && apres.at(-1)[1].hauteur === HAUTEUR, 'fermeture : la fenêtre ne rétrécit qu\'après le repli de l\'étage (une taille, 58 px)', json([nAvant, nOuvert, pendant, apres.length]));
  await p.context().close();

  // ---- Sans licence ---------------------------------------------------------------
  {
    const q = await page({ charge: SANS });
    verifier(json(await ordre(q)) === json(['capturer_zone', 'capturer_plein_ecran', 'capturer_fenetre', 'capturer_pipette', 'retardateur', '|', 'capturer_defilante', 'capturer_video', 'capturer_gif', 'puce', '|', 'reglages']),
      'sans licence : Défilante, Vidéo, GIF restent, une seule puce « Pro » après GIF', json(await ordre(q)));
    const e = await q.evaluate(() => {
      const pros = [...document.querySelectorAll('.b.pro')];
      return { ids: pros.map(b => b.dataset.id), noms: pros.map(b => b.getAttribute('aria-label')), descriptions: pros.map(b => b.getAttribute('aria-description')),
        inactifs: pros.map(b => b.disabled || b.getAttribute('aria-disabled')), opacite: pros.map(b => getComputedStyle(b.querySelector('svg')).opacity),
        puce: document.querySelector('.puce').textContent, puces: document.querySelectorAll('.puce').length,
        normal: getComputedStyle(document.querySelector('[data-id=capturer_zone] svg')).opacity };
    });
    verifier(json(e.ids) === json(FILMS) && json(e.noms) === json(['Capture défilante, Pro', 'Vidéo, Pro', 'GIF, Pro']) && e.puces === 1 && e.puce === 'Pro', 'boutons Pro éteints nommés « …, Pro », une seule puce', json(e));
    verifier(e.opacite.every(o => o === '0.5') && e.normal === '1' && e.inactifs.every(x => !x) && e.descriptions.every(d => d === 'Ouvre la licence'), 'éteints à 0,5 mais ni disabled ni aria-disabled, description « Ouvre la licence »', json(e));
    // Un clic sur un bouton éteint : barre_choisi { id } (le cœur ouvre la licence).
    await q.click('[data-id=capturer_video]');
    await q.waitForTimeout(50);
    verifier(json(await derniere(q, 'barre_choisi')) === json({ id: 'capturer_video' }), 'clic sur Vidéo éteinte : barre_choisi { id: capturer_video } (le cœur ouvre Réglages › Licence)');
    // Au clavier aussi.
    await ouvrir(q, SANS);
    await q.keyboard.press('End'); await q.keyboard.press('ArrowLeft');
    verifier(await focus(q) === 'capturer_gif', 'bouton éteint atteignable au clavier', await focus(q));
    await q.keyboard.press('Enter');
    await q.waitForTimeout(50);
    verifier((await derniere(q, 'barre_choisi'))?.id === 'capturer_gif', 'Entrée sur GIF éteint : barre_choisi { id: capturer_gif }');
    await q.context().close();
    // Sans Media Foundation : GIF seul (Vidéo absente), éteint, avec la puce.
    const r = await page({ charge: charge(TOUT.filter(b => b !== 'capturer_video'), 0, ['capturer_defilante', 'capturer_gif']) });
    verifier(json(await ordre(r)) === json(['capturer_zone', 'capturer_plein_ecran', 'capturer_fenetre', 'capturer_pipette', 'retardateur', '|', 'capturer_defilante', 'capturer_gif', 'puce', '|', 'reglages']), 'sans codeur vidéo : GIF sans Vidéo', json(await ordre(r)));
    await r.context().close();
    // Sans retardateur (capture indisponible) et sans Pro (le système ne le permet pas).
    const s = await page({ charge: charge([...CAPTURES, 'reglages'], null) });
    verifier(json(await ordre(s)) === json(['capturer_zone', 'capturer_plein_ecran', 'capturer_fenetre', 'capturer_pipette', '|', 'reglages']), 'sans retardateur ni Pro : Réglages après un filet', json(await ordre(s)));
    await s.context().close();
    // Aucun choix de retardateur à proposer : pas de bouton qui ouvre du vide.
    const t = await page({ charge: { boutons: TOUT, verrouilles: [], retardateur_s: 0, retardateurs: [] } });
    verifier(!(await ordre(t)).includes('retardateur'), 'aucun délai proposé : pas de bouton Retardateur');
    await t.context().close();
  }
  {
    // Mouvement réduit : ni ressort, ni montée ; l'étage s'ouvre et la taille part quand même.
    const q = await page({ reduit: true });
    const m = await q.evaluate(() => ({
      bouton: getComputedStyle(document.querySelector('.b')).transitionProperty,
      etage: getComputedStyle(document.getElementById('etage-retardateur')).transitionProperty,
    }));
    await q.click('[data-id=retardateur]');
    await q.waitForTimeout(400);
    const o = await q.evaluate(() => document.getElementById('etage-retardateur').classList.contains('ouvert'));
    const survol = 'none';
    verifier(!m.bouton.includes('transform') && m.etage === 'opacity' && survol === 'none' && o && (await derniere(q, 'barre_taille')).hauteur > HAUTEUR, 'mouvement réduit respecté, étage ouvert, taille annoncée', json({ ...m, survol, o }));
    await q.context().close();
  }
  {
    // Cœur pas encore en place : deux refus, boutons reçus au troisième essai.
    const q = await page({ refus: 2 });
    await q.waitForTimeout(400);
    const n = (await appels(q, 'barre_pret')).length;
    const b = await q.evaluate(() => document.querySelectorAll('.b').length);
    verifier(n === 3 && b === 9, 'barre_pret rejeté deux fois : retenté, boutons au troisième', `${n} appel(s), ${b} bouton(s)`);
    await q.context().close();
  }

  verifier(erreurs.length === 0, 'aucune erreur ni avertissement dans la console', erreurs.join(' | '));
  const csp = await (async () => { const q = await page(); await q.click('[data-id=retardateur]'); const r = await q.evaluate(() => window.__refus); await q.context().close(); return r; })();
  verifier(csp.length === 0, 'aucune violation de la CSP', JSON.stringify(csp));
  await nav.close();
  serveur.close();
  console.log(echecs ? `${echecs} échec(s)` : 'tout est vert');
  process.exit(echecs ? 1 : 0);
})().catch(e => { console.error(e); process.exit(1); });
