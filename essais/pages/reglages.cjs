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
  ],
};
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
    const ctx = await nav.newContext({ viewport: { width: o.largeur || 1024, height: o.hauteur || 768 }, colorScheme: o.theme || 'light' });
    const p = await ctx.newPage();
    p.erreurs = [];
    p.on('pageerror', e => p.erreurs.push(String(e)));
    p.on('console', m => { if (m.type() === 'error' || m.type() === 'warning') p.erreurs.push(m.text()); });
    await p.addInitScript(({ vue }) => {
      window.__appels = []; window.__ecouteurs = {}; window.__vue = vue; window.__refus = null;
      document.addEventListener('securitypolicyviolation', e => (window.__csp = (window.__csp || []).concat(e.violatedDirective)));
      window.__TAURI__ = {
        core: { invoke: (c, a) => {
          window.__appels.push([c, a]);
          if (c === 'reglages_apercu_nom') return Promise.resolve({ apercu: `aperçu:${a.modele}` });
          const refus = window.__refus; window.__refus = null;
          return Promise.resolve({ vue: window.__vue, refus });
        } },
        event: { listen: (n, f) => { window.__ecouteurs[n] = f; return Promise.resolve(() => {}); } },
      };
      window.__emettre = (n, payload) => window.__ecouteurs[n]({ payload });
    }, { vue: o.vue || VUE });
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
    await p.screenshot({ path: `${SORTIE}/reglages-apres-${theme}.png` });
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

  // 2. Lignes montrées : celles de la vue, rien d'autre ; pas de rubrique « Capture ».
  {
    const p = await page({ vue: SANS_VIGNETTE });
    const montrees = await p.$$eval('[data-reglage]:not([data-reglage="raccourci"])', ls => ls.filter(l => !l.hidden).map(l => l.dataset.reglage));
    verifier(JSON.stringify(montrees) === JSON.stringify(SANS_VIGNETTE.lignes), '2 sans vignette : seules les lignes de la vue', JSON.stringify(montrees));
    const onglets = await p.$$eval('[role="tab"]', os => os.filter(o => !o.hidden).map(o => o.textContent.trim()));
    verifier(JSON.stringify(onglets) === JSON.stringify(['Après la capture', 'Raccourcis', 'Général']), '2 trois rubriques, pas de Capture', JSON.stringify(onglets));
    const interdits = await p.$$('[data-reglage="loupe"],[data-reglage="retardateur"],[data-reglage="pointeur"],[data-reglage="format"]');
    verifier(interdits.length === 0, '2 aucun contrôle non construit dans la page');
    await p.context().close();
  }

  // 2 bis. Vue réelle (vignette construite) : toutes ses lignes montrées, dont la vignette,
  // sa durée, son coin et « Enregistrer automatiquement ».
  {
    const p = await page();
    const montrees = await p.$$eval('[data-reglage]:not([data-reglage="raccourci"])', ls => ls.filter(l => !l.hidden).map(l => l.dataset.reglage));
    verifier(JSON.stringify(montrees) === JSON.stringify(REEL.vue.lignes) && ['vignette', 'vignette_duree', 'vignette_coin', 'enregistrer'].every(l => montrees.includes(l)),
      '2 vue réelle : lignes du cœur, vignette et enregistrement compris', JSON.stringify(montrees));
    const durees = await p.$$eval('[data-controle="vignette_duree"] [role="radio"]', bs => bs.map(b => [b.textContent, b.getAttribute('aria-checked')]));
    const attendues = REEL.vue.durees.map(d => [d === 0 ? 'Toujours' : `${d} s`, String(d === REEL.vue.vignette.duree_s)]);
    verifier(JSON.stringify(durees) === JSON.stringify(attendues), '2 durées du cœur, la sienne cochée', JSON.stringify(durees));
    const coin = await p.$$eval('[data-controle="vignette_coin"] [aria-checked="true"]', bs => bs.map(b => b.dataset.valeur));
    verifier(JSON.stringify(coin) === JSON.stringify([REEL.vue.vignette.coin]), '2 coin du cœur coché', JSON.stringify(coin));
    const inter = await p.$$eval('[role="switch"]', bs => Object.fromEntries(bs.map(b => [b.dataset.controle, b.getAttribute('aria-checked')])));
    verifier(inter.vignette === String(REEL.vue.vignette.afficher) && inter.enregistrer === String(REEL.vue.enregistrer), '2 interrupteurs vignette et enregistrement d’après la vue', JSON.stringify(inter));
    await p.getByRole('switch', { name: 'Enregistrer automatiquement' }).click(); await p.waitForTimeout(50);
    verifier(JSON.stringify((await dernier(p, 'reglages_changer'))?.[1]) === JSON.stringify({ changement: { cle: 'enregistrer', valeur: !REEL.vue.enregistrer } }), '2 Enregistrer → reglages_changer enregistrer');
    await p.context().close();
  }

  // 3. Interrupteur : envoie le changement ; un refus (le vrai, rendu par le cœur) remet
  // l'interrupteur et dit son texte sur la ligne.
  {
    const p = await page();
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
    const p = await page();
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
    verifier(JSON.stringify(retablir) === JSON.stringify([true, true, false]), '7 Rétablir seulement si modifié', JSON.stringify(retablir));
    await p.getByRole('button', { name: 'Rétablir' }).click(); await p.waitForTimeout(50);
    verifier(JSON.stringify((await dernier(p, 'reglages_retablir'))?.[1]) === JSON.stringify({ action: 'fenetre' }), '7 Rétablir → reglages_retablir');
    const touches = await p.$$eval('[data-action="zone"] kbd', ks => ks.map(k => k.textContent));
    verifier(JSON.stringify(touches) === JSON.stringify(['Ctrl', 'Maj', '1']), '7 touches de la vue', JSON.stringify(touches));
    await p.context().close();
  }

  // 8. État et thème poussés par le cœur.
  {
    const p = await page();
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

  console.log('ECHECS :', echecs);
  await nav.close(); serveur.close();
  process.exitCode = echecs === 0 ? 0 : 1;
})().catch((e) => { console.error('essai interrompu :', e); process.exit(1); });
