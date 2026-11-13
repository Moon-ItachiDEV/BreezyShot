const { chromium } = require('playwright');
const http = require('http'), fs = require('fs'), path = require('path');
// Essai de la page des Réglages dans Chromium (Playwright). Sort avec 1 si une vérification
// échoue. Copies d'écran dans $SORTIE (par défaut essais-resultats/pages).
//   node essais/pages/reglages.cjs
// Les éléments ne sont visés que par data-*, rôles et noms accessibles : l'habillage peut
// changer sans toucher à cet essai.
const RACINE = path.join(__dirname, '..', '..', 'app', 'interface');
const SORTIE = process.env.SORTIE || path.join(__dirname, '..', '..', 'essais-resultats', 'pages');
fs.mkdirSync(SORTIE, { recursive: true });
const CSP = Object.entries(JSON.parse(fs.readFileSync(path.join(__dirname, '..', '..', 'app', 'coeur', 'tauri.conf.json'), 'utf8')).app.security.csp)
  .map(([k, v]) => `${k} ${v}`).join('; ');
const types = { '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript' };
const serveur = http.createServer((q, r) => {
  const p = path.join(RACINE, decodeURIComponent(new URL(q.url, 'http://x').pathname));
  if (!p.startsWith(RACINE) || !fs.existsSync(p)) { r.writeHead(404); return r.end(); }
  r.writeHead(200, { 'content-type': types[path.extname(p)] || 'text/plain', 'content-security-policy': CSP });
  fs.createReadStream(p).pipe(r);
});

// La vue réelle de l'app assemblée et le vrai refus de la dernière sortie, écrits par le
// cœur (`cargo test vue_de_l_essai_de_page`, qui échoue si ce fichier n'est plus à jour).
// Seuls les raccourcis sont remplacés, pour essayer un raccourci absent et modifié.
const REEL = JSON.parse(fs.readFileSync(path.join(__dirname, 'vue-reglages.json'), 'utf8'));
const VUE = {
  ...REEL.vue,
  raccourcis: [
    { action: 'zone', libelle: 'Zone', touches: ['Ctrl', 'Maj', '1'], modifie: false, refus: null },
    { action: 'plein_ecran', libelle: 'Plein écran', touches: ['Ctrl', 'Maj', '2'], modifie: false, refus: null },
    { action: 'fenetre', libelle: 'Fenêtre', touches: [], modifie: true, refus: 'Aucun raccourci' },
    { action: 'derniere_zone', libelle: 'Refaire la dernière zone', touches: ['Ctrl', 'Maj', '4'], modifie: false, refus: null },
  ],
};
// Avec licence : la vue du cœur après « Activer » (défilante, historique, texte, une langue).
const VUE_PRO = REEL.vue_pro;
// La licence d'essai publique (avenant 0.5 « Licence », §2) : le cœur simulé l'accepte, il
// refuse tout autre texte avec le code `signature`, comme le vrai.
const LICENCE = fs.readFileSync(path.join(__dirname, '..', 'licence-essai.txt'), 'utf8').trim();
// Sans Gel (autre système) ni vignette : aucune ligne de la rubrique Capture.
const SANS_VIGNETTE = { ...VUE, lignes: ['copier', 'dossier', 'modele_nom', 'theme', 'demarrage', 'version'] };

(async () => {
  await new Promise(ok => serveur.listen(0, ok));
  const base = `http://127.0.0.1:${serveur.address().port}/reglages/reglages.html`;
  const nav = await chromium.launch();
  let echecs = 0;
  const verifier = (ok, quoi, info = '') => { console.log(ok ? 'OK   ' : 'ECHEC', quoi, info); if (!ok) echecs++; };

  // Page avec un cœur simulé : `__reponses[commande]` (fonction des arguments) donne la
  // réponse ; tous les appels sont notés dans `__appels`.
  async function page(o = {}) {
    const ctx = await nav.newContext({ viewport: { width: o.largeur || 1024, height: o.hauteur || 768 }, deviceScaleFactor: o.echelle || 1, colorScheme: o.theme || 'light' });
    const p = await ctx.newPage();
    p.erreurs = [];
    p.on('pageerror', e => p.erreurs.push(String(e)));
    p.on('console', m => { if (m.type() === 'error' || m.type() === 'warning') p.erreurs.push(m.text()); });
    await p.addInitScript(({ vue, vueSans, vuePro, licence }) => {
      window.__appels = []; window.__ecouteurs = {}; window.__vue = vue; window.__refus = null;
      window.__suspendu = null; window.__refusees = [];
      document.addEventListener('securitypolicyviolation', e => (window.__csp = (window.__csp || []).concat(e.violatedDirective)));
      window.__TAURI__ = {
        core: { invoke: (c, a) => {
          window.__appels.push([c, a]);
          if (c === 'reglages_apercu_nom') return Promise.resolve({ apercu: `aperçu:${a.modele}` });
          if (c === 'licence_activer') {
            if (a.texte.trim() !== licence) return Promise.reject('signature');
            window.__vue = vuePro; return Promise.resolve({ active: true });
          }
          if (c === 'licence_retirer') {
            window.__vue = vueSans; return Promise.resolve({ active: false });
          }
          // Comme le cœur (`page_reglages::saisie_permise`) : une action hors vigueur est
          // refusée, sauf la fin d'une saisie, qui passe toujours.
          if (['reglages_saisie', 'reglages_raccourci', 'reglages_retablir'].includes(c)) {
            const connue = window.__vue.raccourcis.some(r => r.action === a.action);
            if (!connue && !(c === 'reglages_saisie' && a.active === false)) {
              window.__refusees.push([c, a.action]);
              const refus = { ligne: 'raccourci', texte: '' };
              return Promise.resolve({ vue: window.__vue, refus });
            }
            if (c === 'reglages_saisie') window.__suspendu = a.active ? a.action : null;
          }
          const refus = window.__refus; window.__refus = null;
          return Promise.resolve({ vue: window.__vue, refus });
        } },
        event: { listen: (n, f) => { window.__ecouteurs[n] = f; return Promise.resolve(() => {}); } },
      };
      window.__emettre = (n, payload) => window.__ecouteurs[n]({ payload });
    }, { vue: o.vue || VUE, vueSans: REEL.vue, vuePro: VUE_PRO, licence: LICENCE });
    await p.goto(base);
    await p.waitForTimeout(150);
    return p;
  }
  const appels = (p) => p.evaluate(() => __appels);
  const vider = (p) => p.evaluate(() => { __appels.length = 0; });
  const dernier = async (p, nom) => (await appels(p)).filter(a => a[0] === nom).pop();
  const visible = (p, s) => p.$eval(s, e => !e.hidden && !!e.offsetParent).catch(() => false);
  const onglet = (p, nom) => p.getByRole('tab', { name: nom }).click();

  // 1. Chargement, CSP, captures clair et sombre, fenêtre étroite.
  for (const theme of ['light', 'dark']) {
    const p = await page({ theme });
    const premier = (await appels(p))[0];
    verifier(premier && premier[0] === 'reglages_lire', `1 ${theme} : reglages_lire au chargement`, JSON.stringify(premier));
    const csp = await p.evaluate(() => window.__csp || []);
    const style = await p.$eval('[role="switch"][data-controle="copier"]', e => getComputedStyle(e).borderRadius);
    verifier(csp.length === 0 && style !== '0px', `1 ${theme} : CSP respectée, styles chargés`, JSON.stringify(csp));
    await p.screenshot({ path: `${SORTIE}/reglages-capture-${theme}.png` });
    await onglet(p, 'Après la capture'); await p.screenshot({ path: `${SORTIE}/reglages-apres-${theme}.png` });
    await onglet(p, 'Raccourcis'); await p.screenshot({ path: `${SORTIE}/reglages-raccourcis-${theme}.png` });
    await onglet(p, 'Général'); await p.screenshot({ path: `${SORTIE}/reglages-general-${theme}.png` });
    verifier(p.erreurs.length === 0, `1 ${theme} : aucune erreur de console`, JSON.stringify(p.erreurs));
    await p.context().close();
  }
  {
    const p = await page({ largeur: 480, hauteur: 700 });
    const larg = await p.evaluate(() => [document.documentElement.scrollWidth, innerWidth]);
    verifier(larg[0] <= larg[1], '1 480 px : pas de défilement horizontal', JSON.stringify(larg));
    await p.screenshot({ path: `${SORTIE}/reglages-etroit.png` });
    await p.context().close();
  }

  // 2. Lignes montrées : celles de la vue, rien d'autre ; sans ses lignes, pas de rubrique
  // « Capture », et « Après la capture » s'ouvre à sa place.
  {
    const p = await page({ vue: SANS_VIGNETTE });
    const montrees = await p.$$eval('[data-reglage]:not([data-reglage="raccourci"])', ls => ls.filter(l => !l.hidden).map(l => l.dataset.reglage));
    verifier(JSON.stringify([...montrees].sort()) === JSON.stringify([...SANS_VIGNETTE.lignes].sort()), '2 sans vignette : seules les lignes de la vue', JSON.stringify(montrees));
    const onglets = await p.$$eval('[role="tab"]', os => os.filter(o => !o.hidden).map(o => o.textContent.trim()));
    verifier(JSON.stringify(onglets) === JSON.stringify(['Après la capture', 'Raccourcis', 'Général']), '2 trois rubriques, pas de Capture', JSON.stringify(onglets));
    const ouvert = await p.$$eval('[role="tab"][aria-selected="true"]', os => os.map(o => o.textContent.trim()));
    const sections = await p.$$eval('section[role="tabpanel"]', ss => ss.filter(s => !s.hidden).map(s => s.dataset.rubrique));
    verifier(JSON.stringify(ouvert) === '["Après la capture"]' && JSON.stringify(sections) === '["apres"]'
      && await visible(p, '[data-reglage="copier"]'), '2 Capture cachée : « Après la capture » ouverte', JSON.stringify([ouvert, sections]));
    const interdits = await p.$$('[data-reglage="barre"],[data-reglage="format"],[data-reglage="mise_a_jour"]');
    verifier(interdits.length === 0, '2 aucun contrôle sans effet dans la page (barre, format, mise à jour)');
    await p.context().close();
  }

  // 2 bis. Vue réelle (vignette construite) : toutes ses lignes montrées, dont la vignette,
  // sa durée, son coin et « Enregistrer automatiquement ».
  {
    const p = await page();
    const montrees = await p.$$eval('[data-reglage]:not([data-reglage="raccourci"])', ls => ls.filter(l => !l.hidden).map(l => l.dataset.reglage));
    verifier(JSON.stringify([...montrees].sort()) === JSON.stringify([...REEL.vue.lignes].sort()) && ['vignette', 'vignette_duree', 'vignette_coin', 'enregistrer'].every(l => montrees.includes(l)),
      '2 vue réelle : lignes du cœur, vignette et enregistrement compris', JSON.stringify(montrees));
    const durees = await p.$$eval('[data-controle="vignette_duree"] [role="radio"]', bs => bs.map(b => [b.textContent, b.getAttribute('aria-checked')]));
    const attendues = REEL.vue.durees.map(d => [d === 0 ? 'Toujours' : `${d} s`, String(d === REEL.vue.vignette.duree_s)]);
    verifier(JSON.stringify(durees) === JSON.stringify(attendues), '2 durées du cœur, la sienne cochée', JSON.stringify(durees));
    const coin = await p.$$eval('[data-controle="vignette_coin"] [aria-checked="true"]', bs => bs.map(b => b.dataset.valeur));
    verifier(JSON.stringify(coin) === JSON.stringify([REEL.vue.vignette.coin]), '2 coin du cœur coché', JSON.stringify(coin));
    const inter = await p.$$eval('[role="switch"]', bs => Object.fromEntries(bs.map(b => [b.dataset.controle, b.getAttribute('aria-checked')])));
    verifier(inter.vignette === String(REEL.vue.vignette.afficher) && inter.enregistrer === String(REEL.vue.enregistrer), '2 interrupteurs vignette et enregistrement d’après la vue', JSON.stringify(inter));
    // Habillage : le nom se replie (inerte) sous l'enregistrement éteint, la durée et le coin
    // sous la vignette éteinte ; allumés, ils reviennent.
    const repli = () => p.$$eval('[data-repli]', rs => rs.map(r => [r.dataset.repli, r.hasAttribute('data-replie'), r.inert]));
    verifier(JSON.stringify(await repli()) === JSON.stringify([['enregistrer', true, true], ['vignette', false, false]]), '2 repli : nom replié sous l’enregistrement éteint', JSON.stringify(await repli()));
    await p.evaluate(() => __emettre('reglages:etat', { ...__vue, enregistrer: true, vignette: { ...__vue.vignette, afficher: false } }));
    await p.waitForTimeout(50);
    verifier(JSON.stringify(await repli()) === JSON.stringify([['enregistrer', false, false], ['vignette', true, true]]), '2 repli : nom déplié, durée et coin repliés sous la vignette éteinte', JSON.stringify(await repli()));
    await p.evaluate(() => __emettre('reglages:etat', __vue)); await p.waitForTimeout(50);
    await onglet(p, 'Après la capture');
    await p.getByRole('switch', { name: 'Enregistrer automatiquement' }).click(); await p.waitForTimeout(50);
    verifier(JSON.stringify((await dernier(p, 'reglages_changer'))?.[1]) === JSON.stringify({ changement: { cle: 'enregistrer', valeur: !REEL.vue.enregistrer } }), '2 Enregistrer → reglages_changer enregistrer');
    await p.context().close();
  }

  // 3. Interrupteur : envoie le changement ; un refus (le vrai, rendu par le cœur) remet
  // l'interrupteur et dit son texte sur la ligne.
  {
    const p = await page();
    await onglet(p, 'Après la capture');
    await p.evaluate((r) => { window.__refus = r; }, REEL.refus_copier);
    await p.getByRole('switch', { name: 'Copier automatiquement' }).click(); await p.waitForTimeout(100);
    const a = await dernier(p, 'reglages_changer');
    verifier(JSON.stringify(a?.[1]) === JSON.stringify({ changement: { cle: 'copier', valeur: false } }), '3 copier → reglages_changer { copier, false }', JSON.stringify(a));
    const etat = await p.getByRole('switch', { name: 'Copier automatiquement' }).getAttribute('aria-checked');
    const refus = await p.$eval('[data-reglage="copier"] [data-refus]', e => [e.hidden, e.textContent]);
    verifier(etat === 'true' && !refus[0] && refus[1] === REEL.refus_copier.texte && refus[1].startsWith('Gardez au moins') && !refus[1].includes('\n'),
      '3 refus réel « Gardez au moins… » : interrupteur remis, une ligne', JSON.stringify([etat, refus]));
    await p.getByRole('switch', { name: 'Vignette' }).click(); await p.waitForTimeout(50);
    verifier(JSON.stringify((await dernier(p, 'reglages_changer'))[1]) === JSON.stringify({ changement: { cle: 'vignette_afficher', valeur: false } }), '3 vignette → vignette_afficher');
    await p.context().close();
  }

  // 4. Choix : durée (Toujours = 0), coin à la souris et aux flèches, thème ; durée hors liste.
  {
    const p = await page();
    await onglet(p, 'Après la capture');
    await p.getByRole('radio', { name: 'Toujours' }).click(); await p.waitForTimeout(50);
    verifier(JSON.stringify((await dernier(p, 'reglages_changer'))[1]) === JSON.stringify({ changement: { cle: 'vignette_duree', valeur: 0 } }), '4 Toujours → vignette_duree 0');
    await p.getByRole('radio', { name: 'En haut à droite' }).click(); await p.waitForTimeout(50);
    verifier(JSON.stringify((await dernier(p, 'reglages_changer'))[1]) === JSON.stringify({ changement: { cle: 'vignette_coin', valeur: 'haut_droite' } }), '4 coin à la souris');
    await p.getByRole('radio', { name: 'En bas à gauche' }).focus(); await p.keyboard.press('ArrowRight'); await p.waitForTimeout(50);
    verifier(JSON.stringify((await dernier(p, 'reglages_changer'))[1]) === JSON.stringify({ changement: { cle: 'vignette_coin', valeur: 'bas_droite' } }), '4 coin aux flèches');
    await onglet(p, 'Général');
    await p.getByRole('radio', { name: 'Sombre' }).click(); await p.waitForTimeout(50);
    verifier(JSON.stringify((await dernier(p, 'reglages_changer'))[1]) === JSON.stringify({ changement: { cle: 'theme', valeur: 'sombre' } }), '4 thème Sombre');
    await p.evaluate(() => { __vue = { ...__vue, vignette: { ...__vue.vignette, duree_s: 12 } }; __emettre('reglages:etat', __vue); });
    await onglet(p, 'Après la capture');
    const coches = await p.$$eval('[data-controle="vignette_duree"] [role="radio"][aria-checked="true"]', bs => bs.length);
    verifier(coches === 0, '4 durée 12 (du fichier) : aucun segment coché');
    await p.context().close();
  }

  // 5. Nom : aperçu à chaque frappe, écriture une fois après la pause ; texte jamais du HTML.
  {
    // Le nom se replie sous « Enregistrer automatiquement » éteint : la vue l'allume.
    const p = await page({ vue: { ...VUE, enregistrer: true } });
    await onglet(p, 'Après la capture');
    await vider(p);
    const champ = p.getByRole('textbox', { name: 'Nom des fichiers' });
    await champ.fill(''); await champ.type('<img src=x onerror="window.__pwn=1">', { delay: 5 });
    await p.waitForTimeout(100);
    let a = await appels(p);
    verifier(a.filter(x => x[0] === 'reglages_changer').length === 0 && a.filter(x => x[0] === 'reglages_apercu_nom').length > 5, '5 aperçus pendant la frappe, aucune écriture');
    const apercu = await p.$eval('[data-valeur="apercu_nom"]', e => [e.textContent, e.children.length]);
    verifier(apercu[0].startsWith('aperçu:<img') && apercu[1] === 0, '5 aperçu du cœur posé en texte', JSON.stringify(apercu));
    await p.waitForTimeout(500);
    a = (await appels(p)).filter(x => x[0] === 'reglages_changer');
    verifier(a.length === 1 && a[0][1].changement.cle === 'modele_nom' && a[0][1].changement.valeur.startsWith('<img'), '5 une seule écriture 400 ms après', JSON.stringify(a));
    const pwn = await p.evaluate(() => window.__pwn);
    verifier(!pwn, '5 aucun HTML interprété');
    await champ.press('End'); await p.getByRole('button', { name: 'Insérer' }).click();
    await p.getByRole('menuitem', { name: 'Heure' }).click(); await p.waitForTimeout(50);
    const v = await champ.inputValue();
    verifier(v.endsWith(' {heure}'), '5 Insérer › Heure ajoute {heure}', v);
    await p.context().close();
  }

  // 6. Parcourir.
  {
    const p = await page();
    await onglet(p, 'Après la capture');
    await p.evaluate(() => { __vue = { ...__vue, dossier: 'D: › Captures' }; });
    await p.getByRole('button', { name: 'Parcourir…' }).click(); await p.waitForTimeout(100);
    const a = await dernier(p, 'reglages_parcourir');
    const dossier = await p.$eval('[data-valeur="dossier"]', e => e.textContent);
    verifier(!!a && dossier === 'D: › Captures', '6 Parcourir → reglages_parcourir, dossier de la vue', dossier);
    await p.context().close();
  }

  // 7. Saisie d'un raccourci.
  {
    const p = await page();
    await onglet(p, 'Raccourcis');
    const bouton = () => p.getByRole('button', { name: 'Plein écran' });
    await vider(p);
    await bouton().click(); await p.waitForTimeout(50);
    let a = await appels(p);
    verifier(JSON.stringify(a[0]) === JSON.stringify(['reglages_saisie', { action: 'plein_ecran', active: true }]), '7 clic → reglages_saisie active', JSON.stringify(a));
    await p.keyboard.down('Control'); await p.keyboard.down('Shift'); await p.waitForTimeout(30);
    a = await appels(p);
    verifier(!a.some(x => x[0] === 'reglages_raccourci'), '7 modificateurs seuls : rien n’est envoyé');
    const tenus = await p.$$eval('[data-action="plein_ecran"] [data-controle="saisie"] kbd', ks => ks.map(k => k.textContent));
    verifier(JSON.stringify(tenus) === JSON.stringify(['Ctrl', 'Maj']), '7 modificateurs tenus montrés', JSON.stringify(tenus));
    // Une touche « A » à la place physique de Q (clavier AZERTY).
    await p.evaluate(() => window.dispatchEvent(new KeyboardEvent('keydown', { key: 'A', code: 'KeyQ', ctrlKey: true, shiftKey: true, bubbles: true })));
    await p.keyboard.up('Shift'); await p.keyboard.up('Control');
    await p.waitForTimeout(50);
    a = await dernier(p, 'reglages_raccourci');
    verifier(JSON.stringify(a?.[1]) === JSON.stringify({ action: 'plein_ecran', saisie: { ctrl: true, maj: true, alt: false, win: false, cle: 'A', code: 'KeyQ' } }), '7 touche envoyée brute (key et code)', JSON.stringify(a));
    // Échap annule ; la perte du focus aussi.
    await vider(p);
    await bouton().click(); await p.keyboard.press('Escape'); await p.waitForTimeout(50);
    a = await appels(p);
    verifier(JSON.stringify(a.map(x => x[0])) === JSON.stringify(['reglages_saisie', 'reglages_saisie']) && a[1][1].active === false, '7 Échap : fin de saisie, aucune touche envoyée', JSON.stringify(a));
    await vider(p);
    await bouton().click(); await p.getByRole('tab', { name: 'Général' }).focus(); await p.waitForTimeout(50);
    a = await appels(p);
    verifier(a.length === 2 && a[1][1].active === false && !a.some(x => x[0] === 'reglages_raccourci'), '7 perte du focus : fin de saisie', JSON.stringify(a));
    await onglet(p, 'Raccourcis');
    // Filet des 15 s : la vue poussée dit la saisie finie, la page sort de « Appuyez… ».
    await vider(p);
    await bouton().click(); await p.waitForTimeout(50);
    const avantFilet = await p.$eval('[data-action="plein_ecran"]', l => [l.dataset.saisie, l.querySelector('[data-controle="saisie"]').textContent]);
    await p.evaluate(() => __emettre('reglages:etat', { ...__vue, saisie_en_cours: false }));
    await p.waitForTimeout(50);
    const apresFilet = await p.$eval('[data-action="plein_ecran"]', l => [l.dataset.saisie ?? null, l.querySelector('[data-controle="saisie"]').textContent, l.querySelector('[data-controle="saisie"]').getAttribute('aria-pressed')]);
    a = await appels(p);
    verifier(avantFilet[0] === 'oui' && apresFilet[0] === null && !apresFilet[1].includes('Appuyez') && apresFilet[2] === 'false' && a.length === 1,
      '7 filet : vue poussée sans saisie → « Appuyez… » retiré, cœur non rappelé', JSON.stringify([avantFilet, apresFilet, a]));
    await bouton().click(); await p.keyboard.press('Escape'); await p.waitForTimeout(50);
    // La vue est relue au retour du focus : si elle arrive entre l'appui et le relâchement
    // de la souris (clic sur une fenêtre inactive), le clic doit quand même ouvrir la saisie.
    await vider(p);
    const boite = await bouton().boundingBox();
    const touche = await p.$eval('[data-action="plein_ecran"] [data-controle="saisie"] kbd', (k) => {
      const r = k.getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2];
    });
    await p.mouse.move(touche[0], touche[1]); await p.mouse.down();
    await p.evaluate(() => __emettre('reglages:etat', __vue)); await p.waitForTimeout(30);
    await p.mouse.up(); await p.waitForTimeout(50);
    a = await appels(p);
    verifier(boite && JSON.stringify(a[0]) === JSON.stringify(['reglages_saisie', { action: 'plein_ecran', active: true }]),
      '7 vue reçue pendant le clic (fenêtre inactive) : la saisie s’ouvre quand même', JSON.stringify(a));
    await p.keyboard.press('Escape'); await p.waitForTimeout(50);
    // Impr. écran : seulement au relâchement.
    await vider(p);
    await bouton().click();
    await p.evaluate(() => window.dispatchEvent(new KeyboardEvent('keyup', { key: 'PrintScreen', code: 'PrintScreen', bubbles: true })));
    await p.waitForTimeout(50);
    a = await dernier(p, 'reglages_raccourci');
    verifier(a?.[1]?.saisie?.code === 'PrintScreen', '7 Impr. écran envoyée au relâchement', JSON.stringify(a));
    // Refus et Rétablir viennent de la vue.
    const refus = await p.$eval('[data-action="fenetre"] [data-refus]', e => [e.hidden, e.textContent]);
    verifier(!refus[0] && refus[1] === 'Aucun raccourci', '7 refus de la vue sur sa ligne', JSON.stringify(refus));
    const retablir = await p.$$eval('[data-reglage="raccourci"]', ls => ls.map(l => l.querySelector('[data-controle="retablir"]').hidden));
    verifier(JSON.stringify(retablir) === JSON.stringify([true, true, false, true]), '7 Rétablir seulement si modifié', JSON.stringify(retablir));
    await p.getByRole('button', { name: 'Rétablir' }).click(); await p.waitForTimeout(50);
    verifier(JSON.stringify((await dernier(p, 'reglages_retablir'))?.[1]) === JSON.stringify({ action: 'fenetre' }), '7 Rétablir → reglages_retablir');
    const touches = await p.$$eval('[data-action="zone"] kbd', ks => ks.map(k => k.textContent));
    verifier(JSON.stringify(touches) === JSON.stringify(['Ctrl', 'Maj', '1']), '7 touches de la vue', JSON.stringify(touches));
    await p.context().close();
  }

  // 8. État et thème poussés par le cœur.
  {
    const p = await page();
    await onglet(p, 'Après la capture');
    await p.evaluate(() => __emettre('reglages:etat', { ...__vue, copier: false, alerte: 'Réglages non enregistrés' }));
    const copier = await p.getByRole('switch', { name: 'Copier automatiquement' }).getAttribute('aria-checked');
    const alerte = await p.$eval('[data-alerte]', e => [e.hidden, e.textContent]);
    verifier(copier === 'false' && !alerte[0] && alerte[1] === 'Réglages non enregistrés', '8 reglages:etat remplace l’affichage', JSON.stringify([copier, alerte]));
    await p.evaluate(() => __emettre('interface:theme', { theme: 'dark' }));
    const t1 = await p.evaluate(() => document.documentElement.dataset.theme);
    await p.evaluate(() => __emettre('interface:theme', { theme: null }));
    const t2 = await p.evaluate(() => document.documentElement.dataset.theme);
    await p.evaluate(() => __emettre('interface:theme', { theme: '<x>' }));
    const t3 = await p.evaluate(() => document.documentElement.dataset.theme);
    verifier(t1 === 'dark' && t2 === undefined && t3 === undefined, '8 interface:theme pose et retire data-theme', JSON.stringify([t1, t2, t3]));
    const fond = async () => p.evaluate(() => getComputedStyle(document.body).backgroundColor);
    await p.evaluate(() => __emettre('interface:theme', { theme: 'dark' })); const sombre = await fond();
    await p.evaluate(() => __emettre('interface:theme', { theme: 'light' })); const clair = await fond();
    verifier(sombre !== clair, '8 le fond suit le thème', `${sombre} / ${clair}`);
    await vider(p);
    await p.evaluate(() => window.dispatchEvent(new Event('focus')));
    verifier((await appels(p)).some(x => x[0] === 'reglages_lire'), '8 retour du focus → reglages_lire');
    await p.context().close();
  }

  // 9. Version exacte de la vue, noms accessibles.
  {
    const p = await page();
    await onglet(p, 'Général');
    const version = await p.$eval('[data-valeur="version"]', e => e.textContent);
    verifier(version === `BreezyShot ${VUE.version}`, '9 version de la vue, telle quelle', version);
    const sansNom = await p.evaluate(() => Array.from(document.querySelectorAll('button, input, [role="radiogroup"]'))
      .filter(e => e.offsetParent && !(e.getAttribute('aria-label') || e.getAttribute('aria-labelledby') || e.labels?.length || e.textContent.trim()))
      .map(e => e.outerHTML.slice(0, 80)));
    verifier(sansNom.length === 0, '9 chaque contrôle visible a un nom', JSON.stringify(sansNom));
    await p.context().close();
  }

  // 10. Rubrique Capture : premier onglet, ouvert au départ ; retardateur, pointeur.
  {
    const p = await page();
    const onglets = await p.$$eval('[role="tab"]', os => os.filter(o => !o.hidden).map(o => [o.textContent.trim(), o.getAttribute('aria-selected')]));
    verifier(JSON.stringify(onglets) === JSON.stringify([['Capture', 'true'], ['Après la capture', 'false'], ['Raccourcis', 'false'], ['Général', 'false']]),
      '10 Capture en premier, ouverte', JSON.stringify(onglets));
    const sections = await p.$$eval('section[role="tabpanel"]', ss => ss.filter(s => !s.hidden).map(s => s.dataset.rubrique));
    verifier(JSON.stringify(sections) === '["capture"]', '10 seule la rubrique Capture est montrée', JSON.stringify(sections));
    const lignes = await p.$$eval('[data-rubrique="capture"] [data-reglage]', ls => ls.filter(l => !l.hidden && l.offsetParent).map(l => l.dataset.reglage));
    verifier(JSON.stringify(lignes) === '["retardateur","pointeur"]', '10 deux lignes d’après la vue, dans l’ordre', JSON.stringify(lignes));
    const libelles = await p.$$eval('[data-rubrique="capture"] .ligne .libelle', ls => ls.map(l => l.textContent));
    verifier(JSON.stringify(libelles) === JSON.stringify(['Retardateur', 'Inclure le pointeur']), '10 libellés du design', JSON.stringify(libelles));
    const segments = () => p.$$eval('[data-controle="retardateur"] [role="radio"]', bs => bs.map(b => [b.textContent, b.getAttribute('aria-checked')]));
    verifier(JSON.stringify(await segments()) === JSON.stringify([['Aucun', 'true'], ['3 s', 'false'], ['5 s', 'false'], ['10 s', 'false']]),
      '10 segments de vue.retardateurs, « Aucun » coché', JSON.stringify(await segments()));
    const inter = await p.$$eval('[data-rubrique="capture"] [role="switch"]', bs => bs.map(b => [b.dataset.controle, b.getAttribute('aria-checked')]));
    verifier(JSON.stringify(inter) === JSON.stringify([['pointeur', 'false']]), '10 pointeur éteint d’après la vue', JSON.stringify(inter));
    await vider(p);
    await p.getByRole('radio', { name: '5 s' }).click(); await p.waitForTimeout(50);
    verifier(JSON.stringify((await dernier(p, 'reglages_changer'))?.[1]) === JSON.stringify({ changement: { cle: 'retardateur', valeur: 5 } }), '10 « 5 s » → retardateur 5 (un nombre)');
    await p.getByRole('radio', { name: 'Aucun' }).focus(); await p.keyboard.press('ArrowRight'); await p.waitForTimeout(50);
    verifier(JSON.stringify((await dernier(p, 'reglages_changer'))?.[1]) === JSON.stringify({ changement: { cle: 'retardateur', valeur: 3 } }), '10 flèche droite depuis « Aucun » → retardateur 3');
    await p.getByRole('radio', { name: 'Aucun' }).focus(); await p.keyboard.press('ArrowLeft'); await p.waitForTimeout(50);
    verifier(JSON.stringify((await dernier(p, 'reglages_changer'))?.[1]) === JSON.stringify({ changement: { cle: 'retardateur', valeur: 10 } }), '10 flèche gauche depuis « Aucun » → retardateur 10');
    // Valeur du fichier hors de la liste : aucun segment coché, le premier reste atteignable.
    // Le cœur garde 7 (chaque réponse le redit) : les flèches partent bien de 7.
    await p.evaluate(() => { window.__vueAvant7 = __vue; __vue = { ...__vue, capture: { ...__vue.capture, retardateur_s: 7 } }; __emettre('reglages:etat', __vue); });
    await p.waitForTimeout(50);
    const sept = await p.$$eval('[data-controle="retardateur"] [role="radio"]', bs => bs.map(b => [b.getAttribute('aria-checked'), b.tabIndex]));
    const pastille = await p.$eval('[data-controle="retardateur"]', g => g.hasAttribute('data-choisi'));
    verifier(sept.every(x => x[0] === 'false') && sept[0][1] === 0 && !pastille, '10 retardateur 7 : aucun segment coché, aucune pastille', JSON.stringify([sept, pastille]));
    // Flèches depuis la valeur 7 : le segment atteignable (le premier) sert de départ.
    await p.evaluate(() => document.querySelector('[data-controle="retardateur"] [tabindex="0"]').focus());
    await p.keyboard.press('ArrowRight'); await p.waitForTimeout(50);
    const droite = (await dernier(p, 'reglages_changer'))?.[1];
    await p.evaluate(() => document.querySelector('[data-controle="retardateur"] [tabindex="0"]').focus());
    await p.keyboard.press('ArrowLeft'); await p.waitForTimeout(50);
    const gauche = (await dernier(p, 'reglages_changer'))?.[1];
    verifier(JSON.stringify([droite, gauche]) === JSON.stringify([{ changement: { cle: 'retardateur', valeur: 3 } }, { changement: { cle: 'retardateur', valeur: 10 } }]),
      '10 retardateur 7 : flèche droite → 3, flèche gauche → 10', JSON.stringify([droite, gauche]));
    await p.evaluate(() => { __vue = window.__vueAvant7; });
    await p.evaluate(() => __emettre('reglages:etat', { ...__vue, capture: { retardateur_s: 10, pointeur: true } })); await p.waitForTimeout(50);
    const allume = [await segments(), await p.$$eval('[data-rubrique="capture"] [role="switch"]', bs => bs.map(b => b.getAttribute('aria-checked')))];
    verifier(allume[0][3][1] === 'true' && JSON.stringify(allume[1]) === '["true"]', '10 vue poussée : « 10 s », pointeur allumé', JSON.stringify(allume));
    await p.evaluate(() => __emettre('reglages:etat', __vue)); await p.waitForTimeout(50);
    // Interrupteurs : bons appels ; refusés (sans texte), la vue les remet éteints.
    for (const [nom, cle] of [['Inclure le pointeur', 'pointeur']]) {
      await vider(p);
      await p.evaluate((l) => { window.__refus = { ligne: l, texte: '' }; }, cle);
      await p.getByRole('switch', { name: nom }).click(); await p.waitForTimeout(50);
      const a = (await appels(p)).filter(x => x[0] === 'reglages_changer');
      const etat = await p.getByRole('switch', { name: nom }).getAttribute('aria-checked');
      const refus = await p.$eval(`[data-reglage="${cle}"] [data-refus]`, e => e.hidden);
      verifier(a.length === 1 && JSON.stringify(a[0][1]) === JSON.stringify({ changement: { cle, valeur: true } }) && etat === 'false' && refus,
        `10 ${nom} → ${cle} true une fois ; refus : remis par la vue, sans texte`, JSON.stringify([a, etat, refus]));
    }
    // La vue sans les lignes de la Capture (Gel absent) pendant qu'elle est ouverte :
    // l'onglet se cache et « Après la capture » s'ouvre.
    await p.evaluate((l) => __emettre('reglages:etat', { ...__vue, lignes: l }), SANS_VIGNETTE.lignes); await p.waitForTimeout(50);
    const apres = await p.$$eval('[role="tab"]', os => os.filter(o => !o.hidden && o.getAttribute('aria-selected') === 'true').map(o => o.dataset.rubrique));
    const cache = await p.$eval('[role="tab"][data-rubrique="capture"]', o => o.hidden);
    verifier(cache && JSON.stringify(apres) === '["apres"]' && await visible(p, '[data-reglage="copier"]'), '10 Capture vidée : onglet caché, « Après la capture » ouverte', JSON.stringify([cache, apres]));
    await p.context().close();
  }
  // Flèches sur les onglets : Capture en premier (Gauche depuis Capture mène à Général) ;
  // Capture cachée, les flèches la sautent.
  {
    const p = await page();
    const ouvert = () => p.evaluate(() => [document.activeElement.dataset.rubrique, document.querySelector('[role="tab"][aria-selected="true"]').dataset.rubrique,
      [...document.querySelectorAll('section[role="tabpanel"]')].filter(s => !s.hidden).map(s => s.dataset.rubrique).join()]);
    await p.getByRole('tab', { name: 'Capture', exact: true }).focus();
    await p.keyboard.press('ArrowLeft');
    const g = await ouvert();
    await p.keyboard.press('ArrowRight');
    const d = await ouvert();
    await p.keyboard.press('ArrowRight');
    const d2 = await ouvert();
    verifier(JSON.stringify([g, d, d2]) === JSON.stringify([['general', 'general', 'general'], ['capture', 'capture', 'capture'], ['apres', 'apres', 'apres']]),
      '10 onglets : Gauche depuis Capture → Général, Droite → Capture puis Après', JSON.stringify([g, d, d2]));
    await p.evaluate((l) => __emettre('reglages:etat', { ...__vue, lignes: l }), SANS_VIGNETTE.lignes); await p.waitForTimeout(50);
    await p.getByRole('tab', { name: 'Après la capture' }).focus();
    await p.keyboard.press('ArrowLeft');
    const g3 = await ouvert();
    await p.keyboard.press('ArrowRight');
    const d3 = await ouvert();
    verifier(JSON.stringify([g3, d3]) === JSON.stringify([['general', 'general', 'general'], ['apres', 'apres', 'apres']]),
      '10 Capture cachée : les flèches la sautent (Après ↔ Général)', JSON.stringify([g3, d3]));
    await p.context().close();
  }
  {
    // Les raccourcis réels du cœur : quatre lignes, la dernière zone comprise.
    const p = await page({ vue: REEL.vue });
    await onglet(p, 'Raccourcis');
    const lignes = await p.$$eval('[data-reglage="raccourci"]', ls => ls.map(l => [l.dataset.action, l.querySelector('[data-valeur="libelle"]').textContent, [...l.querySelectorAll('kbd')].map(k => k.textContent).join('+')]));
    verifier(lignes.length === 4 && JSON.stringify(lignes[3]) === JSON.stringify(['derniere_zone', 'Refaire la dernière zone', 'Ctrl+Maj+4']),
      '10 quatre raccourcis, « Refaire la dernière zone » en Ctrl + Maj + 4', JSON.stringify(lignes));
    await p.context().close();
  }
  // 11. Licence (avenant 0.5 « Licence », §1) : dernière ligne de Général.
  const ligneLicence = (p) => p.$eval('[data-reglage="licence"]', (l) => {
    const vu = (e) => !!e && !e.closest('[hidden]') && !!e.offsetParent;
    const champ = l.querySelector('[data-controle="licence_texte"]');
    const refus = l.querySelector('[data-refus]');
    const activer = l.querySelector('[data-controle="activer"]');
    return {
      texte: l.innerText.split('\n').map(t => t.trim()).filter(Boolean),
      champ: vu(champ), valeur: champ.value, indicatif: champ.placeholder,
      activer: vu(activer),
      inactif: activer.disabled,
      retirer: vu(l.querySelector('[data-controle="retirer"]')),
      refus: vu(refus) ? refus.textContent : null,
      focus: document.activeElement?.dataset?.controle ?? null,
    };
  });
  const raccourcisMontres = (p) => p.$$eval('[data-reglage="raccourci"]', ls => ls.map(l => [
    l.dataset.action,
    l.querySelector('[data-valeur="libelle"]').textContent,
    [...l.querySelectorAll('kbd')].map(k => k.textContent).join('+'),
    l.querySelector('[data-refus]').hidden,
  ]));
  const quatre = JSON.stringify([
    ['zone', 'Zone', 'Ctrl+Maj+1', true],
    ['plein_ecran', 'Plein écran', 'Ctrl+Maj+2', true],
    ['fenetre', 'Fenêtre', 'Ctrl+Maj+3', true],
    ['derniere_zone', 'Refaire la dernière zone', 'Ctrl+Maj+4', true],
  ]);
  const sept = JSON.stringify(JSON.parse(quatre).concat([
    ['defilante', 'Capture défilante', 'Ctrl+Maj+5', true],
    ['historique', 'Historique', 'Ctrl+Maj+H', true],
    ['texte', 'Copier le texte', 'Ctrl+Maj+T', true],
  ]));
  // Couleur d'erreur des champs des Réglages (jeton --danger), lue sur un témoin.
  const danger = (p) => p.evaluate(() => {
    const t = document.createElement('span');
    t.style.color = 'var(--danger)';
    document.body.append(t);
    const c = getComputedStyle(t).color;
    t.remove();
    return c;
  });
  const json = JSON.stringify;
  for (const theme of ['light', 'dark']) {
    const p = await page({ theme, vue: REEL.vue });
    await onglet(p, 'Général');
    // Sans licence : le champ « Coller la licence » et « Activer » inactif, rien d'autre.
    let l = await ligneLicence(p);
    verifier(json(l.texte) === '["Licence","Activer"]' && l.champ && l.valeur === ''
      && l.indicatif === 'Coller la licence' && l.activer && l.inactif && !l.retirer
      && l.refus === null, `11 ${theme} sans licence : champ, « Activer » inactif, rien d'autre`,
    json(l));
    const ordre = await p.$$eval('[data-rubrique="general"] [data-reglage]',
      ls => ls.filter(e => !e.hidden).map(e => e.dataset.reglage));
    verifier(json(ordre) === json(['version', 'theme', 'demarrage', 'licence']),
      `11 ${theme} Général : logo et version, Thème, Lancer au démarrage, Licence`,
      json(ordre));
    const nom = await p.getByRole('textbox', { name: 'Licence' }).count();
    verifier(nom === 1, `11 ${theme} le champ s'appelle « Licence »`);
    await p.screenshot({ path: `${SORTIE}/reglages-licence-sans-${theme}.png` });
    // Bouton inactif tant que le champ est vide (des espaces ne comptent pas).
    const champ = p.getByRole('textbox', { name: 'Licence' });
    const activer = p.getByRole('button', { name: 'Activer' });
    await champ.fill('   ');
    const espaces = (await ligneLicence(p)).inactif;
    await activer.click({ force: true }); await p.waitForTimeout(50);
    await champ.fill('x'); const rempli = (await ligneLicence(p)).inactif;
    await champ.fill(''); const vide = (await ligneLicence(p)).inactif;
    const aucun = !(await appels(p)).some(a => a[0] === 'licence_activer');
    verifier(espaces && !rempli && vide && aucun,
      `11 ${theme} « Activer » inactif champ vide ou fait d'espaces, actif sinon, aucun appel`,
      json([espaces, rempli, vide]));
    // Refusée : le champ garde le texte collé, prend l'allure d'erreur, « Licence non valide ».
    const faux = LICENCE.slice(0, -2) + 'AA';
    await champ.fill(faux);
    await vider(p);
    await activer.click(); await p.waitForTimeout(100);
    l = await ligneLicence(p);
    const a = await appels(p);
    await p.evaluate(() => Promise.all(document.getAnimations().map(x => x.finished)));
    const bord = await p.$eval('[data-reglage="licence"] .champ',
      e => getComputedStyle(e).borderTopColor);
    verifier(json(a) === json([['licence_activer', { texte: faux }]])
      && l.refus === 'Licence non valide' && l.valeur === faux && l.champ && !l.retirer
      && bord === await danger(p) && json(l.texte) === '["Licence","Licence non valide","Activer"]',
    `11 ${theme} refusée : « Licence non valide », champ gardé, allure d'erreur`,
    json([a, l, bord]));
    await p.screenshot({ path: `${SORTIE}/reglages-licence-refusee-${theme}.png` });
    // Une vue relue (retour du focus) ne l'efface pas ; changer le texte, si.
    await p.evaluate(() => __emettre('reglages:etat', __vue)); await p.waitForTimeout(30);
    const garde = await ligneLicence(p);
    await champ.press('End'); await champ.press('Backspace');
    const efface = await ligneLicence(p);
    verifier(garde.refus === 'Licence non valide' && garde.valeur === faux && efface.refus === null,
      `11 ${theme} refus gardé par une vue relue, retiré à la frappe`,
      json([garde.refus, efface.refus]));
    // Entrée dans le champ : même chemin que le bouton.
    await vider(p);
    await champ.press('Enter'); await p.waitForTimeout(50);
    const entree = (await appels(p)).filter(x => x[0] === 'licence_activer').length;
    verifier(entree === 1 && (await ligneLicence(p)).refus === 'Licence non valide',
      `11 ${theme} Entrée → licence_activer, refusée`);
    // Acceptée : « Pro activé » et « Retirer », la vue redemandée, les trois raccourcis
    // apparaissent ; ni numéro ni date dans la page.
    await champ.fill(`  ${LICENCE} `);
    await vider(p);
    await activer.click(); await p.waitForTimeout(100);
    l = await ligneLicence(p);
    let noms = (await appels(p)).map(x => x[0]);
    verifier(json(noms) === '["licence_activer","reglages_lire"]',
      `11 ${theme} acceptée : licence_activer puis reglages_lire`, json(noms));
    verifier(json(l.texte) === '["Licence","Pro activé","Retirer"]' && !l.champ && !l.activer
      && l.retirer && l.refus === null && l.valeur === '' && l.focus === 'retirer',
    `11 ${theme} acceptée : « Pro activé », « Retirer » (focus), rien d'autre`, json(l));
    const corps = await p.$eval('[data-rubrique="general"][role="tabpanel"]', e => e.innerText);
    verifier(!/ESSAI|0001|BS1\.|\b20\d\d\b/.test(corps),
      `11 ${theme} ni numéro, ni date, ni texte de licence dans la page`);
    await p.screenshot({ path: `${SORTIE}/reglages-licence-active-${theme}.png` });
    await onglet(p, 'Raccourcis');
    let montres = await raccourcisMontres(p);
    verifier(json(montres) === sept,
      `11 ${theme} acceptée : « Capture défilante » en cinquième, « Historique » et « Copier le texte » après`,
      json(montres));
    await p.screenshot({ path: `${SORTIE}/reglages-raccourcis-pro-${theme}.png` });
    // Retirée : la vue redemandée, le champ revient vide (focus), les trois lignes partent.
    await onglet(p, 'Général');
    await vider(p);
    await p.getByRole('button', { name: 'Retirer' }).click(); await p.waitForTimeout(100);
    noms = (await appels(p)).map(x => x[0]);
    l = await ligneLicence(p);
    verifier(json(noms) === '["licence_retirer","reglages_lire"]'
      && json(l.texte) === '["Licence","Activer"]' && l.champ && l.valeur === '' && l.inactif
      && !l.retirer && l.focus === 'licence_texte',
    `11 ${theme} retirée : licence_retirer puis reglages_lire, champ vide (focus), inactif`,
    json([noms, l]));
    await onglet(p, 'Raccourcis');
    montres = await raccourcisMontres(p);
    verifier(json(montres) === quatre, `11 ${theme} retirée : les quatre raccourcis de la 0.4`,
      json(montres));
    const csp = await p.evaluate(() => window.__csp || []);
    verifier(csp.length === 0 && p.erreurs.length === 0,
      `11 ${theme} aucune violation de CSP, aucune erreur de console`, json([csp, p.erreurs]));
    await p.context().close();
  }
  {
    // Ouverte avec une licence déjà en place ; fenêtre étroite, sans et avec licence.
    const p = await page({ vue: VUE_PRO, largeur: 480, hauteur: 700 });
    await onglet(p, 'Général');
    const l = await ligneLicence(p);
    const largeurs = () => p.evaluate(() => [document.documentElement.scrollWidth, innerWidth]);
    const larg1 = await largeurs();
    await p.evaluate((v) => __emettre('reglages:etat', v), REEL.vue); await p.waitForTimeout(30);
    const larg2 = await largeurs();
    const boite = await p.$eval('[data-reglage="licence"] .champ',
      e => [e.getBoundingClientRect().right, innerWidth]);
    verifier(json(l.texte) === '["Licence","Pro activé","Retirer"]' && larg1[0] <= larg1[1]
      && larg2[0] <= larg2[1] && boite[0] <= boite[1],
    '11 480 px : licence ouverte active, rien ne déborde avec ou sans licence',
    json([l.texte, larg1, larg2, boite]));
    await p.screenshot({ path: `${SORTIE}/reglages-licence-etroit.png` });
    await p.context().close();
  }

  // 12. Raccourcis de la défilante, de l'historique et du texte (§7.2) : présents seulement quand l'action est
  // en vigueur ; saisie et Rétablir comme les autres ; une ligne qui part pendant sa saisie
  // la termine. Le cœur simulé refuse une action qui n'est pas en vigueur, comme le vrai.
  {
    const p = await page({ vue: VUE_PRO });
    await onglet(p, 'Raccourcis');
    let montres = await raccourcisMontres(p);
    verifier(json(montres) === sept,
      '12 vue réelle avec licence : sept lignes, Ctrl + Maj + 5, Ctrl + Maj + H et Ctrl + Maj + T', json(montres));
    // « Capture défilante » : saisie, touche envoyée, refus dit sous la ligne, « Aucun
    // raccourci », Rétablir, comme les autres lignes.
    await vider(p);
    await p.getByRole('button', { name: 'Capture défilante' }).click(); await p.waitForTimeout(30);
    await p.keyboard.press('Control+Alt+Digit5'); await p.waitForTimeout(50);
    let ad = await appels(p);
    const toucheD = ad.find(x => x[0] === 'reglages_raccourci')?.[1];
    verifier(json(ad[0]) === json(['reglages_saisie', { action: 'defilante', active: true }])
      && toucheD?.action === 'defilante' && toucheD?.saisie?.code === 'Digit5' && toucheD?.saisie?.ctrl && toucheD?.saisie?.alt,
    '12 « Capture défilante » : saisie puis touche envoyée pour `defilante`', json(ad));
    await p.evaluate(() => __emettre('reglages:etat', {
      ...__vue,
      raccourcis: __vue.raccourcis.map(r => (r.action === 'defilante'
        ? { ...r, touches: [], modifie: true, refus: 'Aucun raccourci' } : r)),
    }));
    await p.waitForTimeout(30);
    const ligneD = await p.$eval('[data-reglage="raccourci"][data-action="defilante"]', l => [
      l.querySelectorAll('kbd').length, l.querySelector('[data-refus]').hidden,
      l.querySelector('[data-refus]').textContent,
      !l.querySelector('[data-controle="retablir"]').hidden,
      [...l.parentElement.children].indexOf(l),
    ]);
    verifier(json(ligneD) === json([0, false, 'Aucun raccourci', true, 4]),
      '12 « Capture défilante » sans raccourci : « Aucun raccourci », Rétablir, cinquième', json(ligneD));
    await vider(p);
    await p.getByRole('button', { name: 'Rétablir' }).click(); await p.waitForTimeout(30);
    verifier(json((await dernier(p, 'reglages_retablir'))?.[1]) === json({ action: 'defilante' }),
      '12 Rétablir de « Capture défilante » → reglages_retablir defilante');
    await p.evaluate(() => __emettre('reglages:etat', __vue)); await p.waitForTimeout(30);
    await vider(p);
    await p.getByRole('button', { name: 'Copier le texte' }).click(); await p.waitForTimeout(30);
    await p.keyboard.press('Control+Alt+KeyY'); await p.waitForTimeout(50);
    const a = await appels(p);
    const touche = a.find(x => x[0] === 'reglages_raccourci')?.[1];
    verifier(json(a[0]) === json(['reglages_saisie', { action: 'texte', active: true }])
      && touche?.action === 'texte' && touche?.saisie?.code === 'KeyY',
    '12 « Copier le texte » : saisie puis touche envoyée pour `texte`', json(a));
    await p.evaluate(() => __emettre('reglages:etat', {
      ...__vue,
      raccourcis: __vue.raccourcis.map(r => (r.action === 'historique'
        ? { ...r, modifie: true } : r)),
    }));
    await vider(p);
    await p.getByRole('button', { name: 'Rétablir' }).click(); await p.waitForTimeout(30);
    verifier(json((await dernier(p, 'reglages_retablir'))?.[1]) === json({ action: 'historique' }),
      '12 Rétablir de « Historique » → reglages_retablir historique');
    // Sans langue de lecture : « Capture défilante » et « Historique » après la dernière zone.
    await p.evaluate(() => __emettre('reglages:etat', {
      ...__vue, raccourcis: __vue.raccourcis.filter(r => r.action !== 'texte'),
    }));
    const six = await raccourcisMontres(p);
    verifier(json(six) === json(JSON.parse(sept).slice(0, 6)),
      '12 sans langue : six lignes, pas de « Copier le texte »', json(six));
    // La licence retirée pendant la saisie de « Historique » : la ligne part, la saisie est
    // rendue au cœur, qui reprend les raccourcis (fin de saisie jamais refusée).
    await p.evaluate(() => __emettre('reglages:etat', __vue));
    await p.getByRole('button', { name: 'Historique' }).click(); await p.waitForTimeout(30);
    const suspenduAvant = await p.evaluate(() => window.__suspendu);
    await vider(p);
    await p.evaluate((v) => {
      __vue = v; __emettre('reglages:etat', { ...v, saisie_en_cours: true });
    }, REEL.vue);
    await p.waitForTimeout(30);
    const fin = await appels(p);
    const apres = await p.evaluate(() => [window.__suspendu, window.__refusees]);
    montres = await raccourcisMontres(p);
    verifier(json(montres) === quatre && suspenduAvant === 'historique'
      && json(fin) === json([['reglages_saisie', { action: 'historique', active: false }]])
      && json(apres) === '[null,[]]',
    '12 ligne partie pendant sa saisie : saisie rendue au cœur et acceptée, quatre lignes',
    json([suspenduAvant, fin, apres]));
    // Une action qui n'est plus en vigueur est refusée par le cœur simulé : la page n'en
    // envoie plus (aucun bouton), un appel forcé l'est.
    await p.evaluate(() => window.__TAURI__.core.invoke('reglages_retablir',
      { action: 'historique' }));
    const refusees = await p.evaluate(() => window.__refusees);
    verifier(json(refusees) === json([['reglages_retablir', 'historique']])
      && await p.getByRole('button', { name: 'Historique' }).count() === 0,
    '12 action hors vigueur : refusée par le cœur simulé, aucun bouton', json(refusees));
    // Elles reviennent dans l'ordre de la vue, sans doublon.
    await p.evaluate((v) => { __vue = v; __emettre('reglages:etat', v); }, VUE_PRO);
    await p.waitForTimeout(30);
    verifier(json(await raccourcisMontres(p)) === sept,
      '12 lignes revenues dans l’ordre, sans doublon');
    verifier(p.erreurs.length === 0, '12 aucune erreur de console', json(p.erreurs));
    await p.context().close();
  }

  // Photos de la rubrique Capture à la taille de la fenêtre (680 × 512, échelle 2).
  for (const theme of ['light', 'dark']) {
    const p = await page({ theme, largeur: 680, hauteur: 512, echelle: 2 });
    await p.screenshot({ path: `${SORTIE}/reglages-capture-680-${theme}.png` });
    verifier(p.erreurs.length === 0, `10 ${theme} 680 × 512 : aucune erreur de console`, JSON.stringify(p.erreurs));
    await p.context().close();
  }

  console.log('ECHECS :', echecs);
  await nav.close(); serveur.close();
  process.exitCode = echecs === 0 ? 0 : 1;
})().catch((e) => { console.error('essai interrompu :', e); process.exit(1); });
