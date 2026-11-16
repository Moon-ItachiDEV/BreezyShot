const { chromium } = require('playwright');
const http = require('http');
const fs = require('fs');
const path = require('path');
// Essai de la page de l'éditeur pour les fichiers .breezy (0.7.8) dans Chromium : la
// réouverture d'un fichier (annotations lues, jugées, prises pour document de départ),
// l'enregistrement (corps et en-têtes), l'entrée de menu Pro éteinte sans licence, et un
// fichier hostile. Le cœur est simulé ; le modèle et la page sont les vrais. Sort avec 1 si
// une vérification échoue.
//   node essais/pages/editeur-breezy-page.cjs
const RACINE = path.join(__dirname, '..', '..', 'app', 'interface');
const CONF = path.join(__dirname, '..', '..', 'app', 'coeur', 'tauri.conf.json');
const CSP = Object.entries(JSON.parse(fs.readFileSync(CONF, 'utf8')).app.security.csp)
  .map(([k, v]) => `${k} ${v}`).join('; ');
const types = { '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript' };

function serveur() {
  return http.createServer((q, r) => {
    const nom = decodeURIComponent(new URL(q.url, 'http://x').pathname);
    const p = path.join(RACINE, nom);
    if (!p.startsWith(RACINE) || !fs.existsSync(p)) { r.writeHead(404); return r.end(); }
    r.writeHead(200, { 'content-security-policy': CSP, 'content-type': types[path.extname(p)] || 'text/plain' });
    fs.createReadStream(p).pipe(r);
  });
}

function simulerCoeur({ charge, annotations, reponseBreezy }) {
  window.__essaiEditeur = true;
  window.__appels = [];
  window.__ecouteurs = {};
  window.__charge = charge;
  window.__TAURI__ = {
    core: {
      invoke: (c, a, opt) => {
        if (c === 'editeur_breezy_enregistrer') {
          window.__appels.push([c, { corps: new TextDecoder().decode(a), entetes: opt.headers }]);
          return reponseBreezy === 'pro' ? Promise.reject('pro') : Promise.resolve(42);
        }
        window.__appels.push([c, a]);
        if (c === 'editeur_pret') return Promise.resolve(window.__charge);
        if (c === 'editeur_image') {
          const n = window.__charge.largeur * window.__charge.hauteur * 4;
          const o = new Uint8Array(n).fill(200);
          return Promise.resolve(o.buffer);
        }
        if (c === 'editeur_annotations') {
          return annotations === null ? Promise.reject('id')
            : Promise.resolve(new TextEncoder().encode(annotations).buffer);
        }
        return Promise.resolve(null);
      },
    },
    event: { listen: (n, f) => { window.__ecouteurs[n] = f; return Promise.resolve(() => {}); } },
  };
  window.__emettre = (n, payload) => window.__ecouteurs[n] && window.__ecouteurs[n]({ payload });
}

const CHARGE = { id: 9, largeur: 400, hauteur: 300, transparence: false, reduites: true,
  preferences: null, texte: false, pro: true, depuis_breezy: true };
const DOC = { largeur: 400, hauteur: 300, suivant: 3, recadrage: null, objets: [
  { id: 1, type: 'fleche', x1: 20, y1: 20, x2: 200, y2: 120, double: false, couleur: '#E5484D', epaisseur: 1, opacite: 1 },
  { id: 2, type: 'texte', x: 40, y: 200, texte: 'Bonjour', couleur: '#1A1A1A', taille: 28, gras: false, style: 'simple', opacite: 1 },
] };

(async () => {
  const srv = serveur();
  await new Promise((ok) => srv.listen(0, ok));
  const url = `http://127.0.0.1:${srv.address().port}/editeur/editeur.html`;
  const nav = await chromium.launch();
  let echecs = 0;
  const verifier = (ok, quoi, info = '') => {
    console.log(ok ? 'OK   ' : 'ECHEC', quoi, ok ? '' : info);
    if (!ok) echecs++;
  };
  async function ouvrir(o) {
    const ctx = await nav.newContext({ viewport: { width: 1200, height: 800 } });
    const p = await ctx.newPage();
    const erreurs = [];
    p.on('pageerror', (e) => erreurs.push(String(e)));
    await p.addInitScript(simulerCoeur, { reponseBreezy: null, ...o });
    await p.goto(url);
    await p.waitForFunction(() => window.__ed && window.__ed.h && window.__ed.fond, null, { timeout: 8000 });
    await p.waitForTimeout(150);
    p.erreurs = erreurs;
    return p;
  }
  const objets = (p) => p.evaluate(() => window.__ed.h.doc.objets.map((o) => [o.id, o.type]));

  // 1. Réouverture : les annotations sont le document de départ, version 0, rien d'annulable.
  let p = await ouvrir({ charge: CHARGE, annotations: JSON.stringify(DOC) });
  verifier(JSON.stringify(await objets(p)) === '[[1,"fleche"],[2,"texte"]]', 'les annotations du fichier sont le document', JSON.stringify(await objets(p)));
  verifier(await p.evaluate(() => window.__ed.h.version === 0 && window.__ed.h.passe.length === 0), 'version 0, rien à annuler');
  verifier(await p.evaluate(() => document.querySelector('[data-act="annuler"]').disabled), 'Annuler grisé');
  verifier((await p.evaluate(() => window.__appels.filter((a) => a[0] === 'editeur_annotations').length)) === 1, 'une seule lecture des annotations');
  verifier(p.erreurs.length === 0, 'aucune erreur de page', p.erreurs.join('|'));

  // 2. Menu : l'entrée est là, sans mention Pro avec licence.
  await p.locator('[data-menu="enregistrer"]').click();
  const entree = p.locator('[data-act="enregistrer_breezy"]');
  verifier(await entree.isVisible(), '« Enregistrer en .breezy » est dans le menu');
  verifier(!(await entree.evaluate((e) => e.classList.contains('pro'))) &&
    (await entree.getAttribute('aria-label')) === 'Enregistrer en .breezy', 'avec licence : ni éteinte ni mention');

  // 3. Enregistrer : corps = le document de la page, en-têtes id et version ; l'éditeur reste libre ensuite.
  await entree.click();
  await p.waitForFunction(() => window.__appels.some((a) => a[0] === 'editeur_breezy_enregistrer'));
  const appel = await p.evaluate(() => window.__appels.find((a) => a[0] === 'editeur_breezy_enregistrer')[1]);
  const envoye = JSON.parse(appel.corps);
  verifier(appel.entetes['x-editeur-id'] === '9' && appel.entetes['x-editeur-version'] === '0', 'en-têtes id et version', JSON.stringify(appel.entetes));
  verifier(envoye.largeur === 400 && envoye.objets.length === 2 && envoye.objets[1].texte === 'Bonjour', 'le corps est le document');
  verifier(await p.evaluate(() => window.__ed.exportEnCours()), 'un enregistrement est en cours (les autres sorties attendent)');
  await p.evaluate(() => window.__emettre('editeur:breezy', { id: 9, version: 0, etat: 'fait', message: null }));
  await p.waitForTimeout(100);
  verifier(!(await p.evaluate(() => window.__ed.exportEnCours())), 'la fin libère l\'éditeur');
  verifier((await p.locator('#message span').textContent()) === 'Enregistré en .breezy', 'message « Enregistré en .breezy »');
  verifier(await p.evaluate(() => window.__ed.exportee === 0), 'version enregistrée notée');
  // Échec : le titre du cœur s'affiche ; annulé : rien.
  await p.locator('[data-menu="enregistrer"]').click();
  await p.locator('[data-act="enregistrer_breezy"]').click();
  await p.evaluate(() => window.__emettre('editeur:breezy', { id: 9, version: 0, etat: 'echec', message: "Impossible d'enregistrer le .breezy" }));
  await p.waitForTimeout(100);
  verifier((await p.locator('#message span').textContent()) === "Impossible d'enregistrer le .breezy" &&
    await p.locator('#message').evaluate((e) => e.classList.contains('echec')), 'échec : le titre du cœur, en erreur');
  // Un résultat d'un autre document est ignoré.
  await p.evaluate(() => window.__emettre('editeur:breezy', { id: 77, version: 0, etat: 'fait', message: null }));
  verifier(p.erreurs.length === 0, 'aucune erreur de page (enregistrement)', p.erreurs.join('|'));
  await p.context().close();

  // 4. Sans licence : l'entrée est visible, éteinte, « Pro » ; le clic appelle le cœur (qui ouvre la licence).
  p = await ouvrir({ charge: { ...CHARGE, pro: false, depuis_breezy: false }, annotations: null, reponseBreezy: 'pro' });
  await p.locator('[data-menu="enregistrer"]').click();
  const sans = p.locator('[data-act="enregistrer_breezy"]');
  verifier(await sans.isVisible() && await sans.evaluate((e) => e.classList.contains('pro')) &&
    (await sans.getAttribute('aria-label')) === 'Enregistrer en .breezy, Pro', 'sans licence : visible, éteinte, nom accessible « , Pro »');
  verifier((await sans.evaluate((e) => getComputedStyle(e, '::after').content)) === '"Pro"', 'la mention Pro est peinte');
  await sans.click();
  await p.waitForFunction(() => window.__appels.some((a) => a[0] === 'editeur_breezy_enregistrer'));
  await p.waitForTimeout(100);
  verifier(!(await p.evaluate(() => window.__ed.exportEnCours())), 'refus « pro » : l\'éditeur n\'attend rien');
  verifier((await p.evaluate(() => window.__appels.filter((a) => a[0] === 'editeur_annotations').length)) === 0, 'capture neuve : aucune lecture d\'annotations');
  await p.context().close();

  // 5. Fichier hostile : types faux, objets inconnus, couleurs piégées, __proto__.
  const hostile = '{"objets":[{"type":"fleche","x1":"a","couleur":"url(x)"},{"type":"evil"},{"type":"texte","texte":"<img src=x onerror=alert(1)>","couleur":"red"}],"__proto__":{"x":1}}';
  p = await ouvrir({ charge: CHARGE, annotations: hostile });
  verifier(JSON.stringify(await objets(p)).startsWith('[[') && (await objets(p)).length === 2, 'fichier hostile : deux objets propres, un écarté', JSON.stringify(await objets(p)));
  verifier((await p.locator('#message span').allTextContents()).join('') === '', 'objet écarté : aucun message (sobriété)');
  verifier(await p.evaluate(() => window.__ed.h.doc.objets.every((o) => !o.couleur || /^#[0-9A-Fa-f]{6}$/.test(o.couleur))), 'couleurs piégées remplacées');
  verifier(p.erreurs.length === 0, 'aucune erreur de page (hostile)', p.erreurs.join('|'));
  await p.context().close();

  // 6. Annotations illisibles : l'image seule, un message d'échec, l'éditeur reste utilisable.
  for (const [nom, annotations] of [['pas du JSON', '{oups'], ['lecture refusée', null], ['pas un objet', '[1,2]']]) {
    p = await ouvrir({ charge: CHARGE, annotations });
    verifier((await objets(p)).length === 0 && (await p.locator('#message span').textContent()).startsWith('Impossible de lire') &&
      await p.locator('#message').evaluate((e) => e.classList.contains('echec')), `annotations illisibles (${nom}) : image seule et message`);
    await p.context().close();
  }

  await nav.close();
  srv.close();
  console.log(echecs ? `${echecs} vérification(s) en échec` : 'tout est bon');
  process.exit(echecs ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
