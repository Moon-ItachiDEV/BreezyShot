// Page de l'installateur. Elle montre et rapporte ; le cœur décide (ARCHITECTURE §2.5).
//
// Contrat avec le cœur :
//  - écoute `installateur:etat` { etat:"accueil"|"installation"|"erreur"|"pret", cause, total, direct }
//  - écoute `installateur:progression` { octets, total } (avancement réel de la copie)
//  - écoute `installateur:envol` { duree } (le cœur déplace la fenêtre ; la page réduit la carte)
//  - écoute `installateur:fermer` {} (fondu de fermeture)
//  - `installateur_pret` {} une fois, l'écoute en place
//  - `installateur_action` { action } : "installer" | "reessayer" | "essayer" | "terminer"
//    | "fermer" | "reduire", une seule fois par appui ; un bouton hors de l'état courant est
//    sans effet (le cœur revérifie de toute façon).
//
// Sans window.__TAURI__ (aperçu dans un navigateur) : ?demo lance une installation simulée.

// --- Rythme de la barre (le seul endroit où l'ajuster) -------------------------------------
// Paliers (cible, durée en s) de la maquette validée : une courbe lissée, toujours croissante,
// qui ne dépasse jamais la part réellement copiée et reste sous 100 % tant que « prêt » n'est
// pas arrivé.
const STAGES = [[.16, .4], [.34, .45], [.52, .4], [.68, .45], [.8, .4], [.88, .5], [.92, .6], [1, .2]];

// --- Durées et courbes : lues dans jetons.css (aucune recopiée ici) ------------------------
const racine = getComputedStyle(document.documentElement);
const jeton = (nom) => racine.getPropertyValue("--" + nom).trim();
const ms = (nom) => {
  const v = parseFloat(jeton(nom));
  if (!Number.isFinite(v)) throw new Error("jeton absent : " + nom);
  return v;
};
const D = {
  reduite: ms("duree-reduite"),                       // --duree-reduite
  sortie: ms("duree-sortie"),                         // --duree-sortie
  versProgression: ms("duree-installateur-vers-progression"),
  versBarre: ms("duree-installateur-vers-barre"),
  anneau: ms("duree-installateur-anneau"),
  anneauReduit: ms("duree-installateur-anneau-reduit"),
  focus: ms("duree-installateur-focus"),
  arretMoulinet: ms("duree-installateur-moulinet-arret"),
  envol: ms("duree-envol-fenetre"),                   // aussi envol::DUREE_ENVOL (cœur)
};
const COURBE_ENVOL = jeton("courbe-envol");
const BARRE_PX = ms("installateur-barre");
window.__dureesInstallateur = D; // relu par l'essai de la page

const tauri = window.__TAURI__;
const RM = matchMedia("(prefers-reduced-motion: reduce)");
const rm = () => RM.matches;
const $ = (s) => document.querySelector(s);

const win = $("#win"), fill = $("#fill"), pbar = $("#pbar"), pctEl = $("#pct"), moEl = $("#mo"),
  cause = $("#cause");
const steps = [...document.querySelectorAll("[data-step]")];
const blades = () => $(".mark .logo .blades");

// --- Moulinet (repris de la maquette, pictogramme tiré d'une constante) --------------------
const PALE = "M98 98Q100 64 80 40.3A100 100 0 0 1 215.7 80Q192 100 158 98Z";
function logo(k) {
  const b = [0, 90, 180, 270];
  const pales = (attr) => b.map((r, i) => `<path d="${PALE}" transform="rotate(${r} 128 128)"${attr(i)}/>`).join("");
  // Couleurs par les classes (jetons logo-*) ; le masque reste noir et blanc (luminance).
  return `<svg class="moulinet" viewBox="0 0 256 256" aria-hidden="true"><defs>` +
    `<linearGradient id="ga${k}" gradientUnits="userSpaceOnUse" x1="20" y1="10" x2="236" y2="246">` +
    `<stop offset="0" class="ga0"/><stop offset=".55" class="ga1"/><stop offset="1" class="ga2"/></linearGradient>` +
    `<radialGradient id="gh${k}" cx=".25" cy=".12" r=".9"><stop offset="0" class="gh" stop-opacity=".38"/><stop offset=".6" class="gh" stop-opacity="0"/></radialGradient></defs>` +
    `<rect x="8" y="8" width="240" height="240" rx="58" fill="url(#ga${k})"/>` +
    `<rect x="8" y="8" width="240" height="240" rx="58" fill="url(#gh${k})"/>` +
    `<g transform="translate(128 128) scale(.8) translate(-128 -128)"><g class="blades">` +
    `<mask id="mk${k}" maskUnits="userSpaceOnUse" x="0" y="0" width="256" height="256">` +
    `<rect width="256" height="256" fill="#fff"/><g fill="none" stroke="#000" stroke-width="9" stroke-linejoin="round">` +
    pales(() => "") + `</g></mask>` +
    `<g mask="url(#mk${k})" class="pales">` + pales((i) => ` opacity="${[1, .86, .72, .58][i]}"`) + `</g>` +
    `</g></g></svg>`;
}
$("#icoLogo").innerHTML = logo("i");
$("#icoLogo svg").style.cssText = "width:var(--installateur-icone);height:var(--installateur-icone);display:block";
$("#welLogo").innerHTML = logo("w");
$("#spinLogo").innerHTML = logo("s");

// --- État de la page -----------------------------------------------------------------------
let etat = "idle";          // idle, starting, installing, error, closing-ring, done, flying, closed
let total = 0;              // octets à copier
let reel = 0;               // part réellement copiée (0 à 1)
let pret = false;           // le cœur a dit « prêt »
let p = 0, elapsed = 0, lastFrame = performance.now(), shown = -1;
let cur = 0;
const timers = [];
const later = (f, delai) => timers.push(setTimeout(f, delai));
// Un minuteur posé dans un état ne fait rien si la page en est sortie entre-temps (une erreur
// reçue pendant le départ, un « prêt », une fermeture…).
const laterEn = (depuis, f, delai) => later(() => { if (etat === depuis) f(); }, delai);

const set = (s) => { etat = s; win.dataset.s = s; };
const smooth = (u) => u * u * (3 - 2 * u);
function cibleCourbe(t) {
  let from = 0;
  for (const [to, d] of STAGES) { if (t < d) return from + (to - from) * smooth(t / d); t -= d; from = to; }
  return 1;
}

// --- Écrans --------------------------------------------------------------------------------
function show(n) {
  steps.forEach((s, i) => { s.hidden = i !== n; s.classList.remove("in-r", "out-l"); });
  cur = n; void steps[n].offsetWidth; steps[n].classList.add("in-r");
}
function go(n) {
  const a = steps[cur], b = steps[n];
  a.classList.remove("in-r"); a.classList.add("out-l");
  later(() => {
    a.hidden = true; a.classList.remove("out-l");
    b.hidden = false; b.classList.remove("in-r"); void b.offsetWidth; b.classList.add("in-r");
    cur = n;
  }, rm() ? D.reduite : D.sortie);
}

// --- Moulinet piloté image par image -------------------------------------------------------
let angle = 0, vel = 0, spinning = false, settle = null;
const backOut = (u) => { const c1 = 1.3, c3 = c1 + 1; return 1 + c3 * Math.pow(u - 1, 3) + c1 * Math.pow(u - 1, 2); };
function stopSpin() {
  spinning = false;
  if (rm()) { vel = 0; return; }
  const cible = Math.ceil((angle + Math.max(60, vel * 0.09)) / 90) * 90;
  settle = { t0: performance.now(), d: D.arretMoulinet, a0: angle, da: cible - angle }; vel = 0;
}
function frame(t) {
  const dt = Math.min(0.05, (t - lastFrame) / 1000); lastFrame = t;
  if (spinning) { vel += (900 - vel) * Math.min(1, dt * 2.2); angle += vel * dt; }
  else if (settle) { const u = Math.min(1, (t - settle.t0) / settle.d); angle = settle.a0 + settle.da * backOut(u); if (u >= 1) { settle = null; angle %= 360; } }
  const g = blades();
  if (g) g.setAttribute("transform", `rotate(${angle.toFixed(2)} 128 128)`);
  if (etat === "installing") {
    elapsed += dt;
    // Cible du design, jamais au-dessus de la part copiée, jamais 100 % avant « prêt » ;
    // une fois « prêt » reçu, la barre file jusqu'au bout quel que soit le temps écoulé.
    let cible = pret ? 1 : Math.min(cibleCourbe(elapsed), Math.min(reel, 0.995));
    const k = 1 - Math.exp(-dt * (pret && cible >= 1 ? 14 : 6));
    p = Math.min(1, Math.max(p, p + (cible - p) * k));
    if (pret && p > 0.997) p = 1;
    if (pret && p >= 1) finir();
  }
  const px = Math.round(p * BARRE_PX * 100) / 100;
  if (px !== shown) {
    shown = px;
    fill.style.width = (p * 100) + "%";
    pbar.setAttribute("aria-valuenow", Math.round(p * 100));
    pctEl.textContent = Math.floor(p * 100) + "\u00a0%";
    const mo = (v) => (v / (1 << 20)).toFixed(1).replace(".", ",");
    moEl.textContent = `${mo(p * total)}\u00a0Mo sur ${mo(total)}\u00a0Mo`;
  }
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);

// --- Parcours ------------------------------------------------------------------------------
function accueil(t) { total = t; reel = 0; pret = false; p = 0; elapsed = 0; shown = -1; set("idle"); show(0); $("#install").focus(); }

function installer() {
  if (etat !== "idle") return;
  set("starting");
  laterEn("starting", () => {
    go(1);
    laterEn("starting", lancerBarre, rm() ? D.reduite : D.versBarre);
  }, rm() ? D.reduite : D.versProgression);
}
function lancerBarre() {
  reel = 0; pret = false; p = 0; elapsed = 0; shown = -1;
  set("installing"); spinning = !rm(); settle = null;
}

function erreur(texte) {
  cause.textContent = texte || "";
  set("error"); stopSpin();
  // Erreur reçue avant l'écran 2 (départ en cours) : la cause s'affiche sur l'écran 2.
  if (cur !== 1) show(1);
}

function finir() {
  if (etat === "done" || etat === "flying" || etat === "closing-ring") return;
  set("closing-ring"); stopSpin(); p = 1;
  laterEn("closing-ring", () => {
    go(2); set("done");
    laterEn("done", () => $("#done").focus(), D.focus);
  }, rm() ? D.anneauReduit : D.anneau);
}

function pretDirect() {
  // « Plus récente installée » : l'écran 3 sans passer par la progression.
  pret = true; set("done"); show(2); $("#done").focus();
}

function envol(duree) {
  if (etat === "flying") return;
  set("flying");
  const d = rm() ? D.reduite : (duree || D.envol);
  win.animate(
    rm() ? [{ opacity: 1 }, { opacity: 0 }]
      : [{ transform: "scale(1)", opacity: 1 }, { transform: "scale(.5)", opacity: .8, offset: .7 }, { transform: "scale(.03)", opacity: 0 }],
    { duration: d, easing: rm() ? "linear" : COURBE_ENVOL, fill: "forwards" }
  );
}

function fermer() {
  set("closed");
  timers.forEach(clearTimeout);
  win.animate([{ opacity: 1, transform: "scale(1)" }, { opacity: 0, transform: "scale(.96)" }],
    { duration: rm() ? D.reduite : D.sortie, fill: "forwards" }).onfinish = () => win.classList.add("gone");
}

// --- Liaison au cœur -----------------------------------------------------------------------
let envoye = null;
function action(nom) {
  if (envoye) tauri.core.invoke("installateur_action", { action: nom });
  else demoAction(nom);
}
// Un bouton caché (hors de l'état courant) n'envoie rien, même cliqué de force : la page
// passe tout de suite à l'écran suivant (un second clic est alors ignoré) et prévient le
// cœur une seule fois ; le cœur revérifie de son côté.
function lier(id, nom, avant) {
  const b = $(id);
  b.onclick = () => {
    if (getComputedStyle(b).display === "none") return;
    if (avant) avant();
    action(nom);
  };
}
lier("#install", "installer", () => installer());
lier("#retry", "reessayer", () => { if (etat === "error") lancerBarre(); });
lier("#try", "essayer");
lier("#done", "terminer");
$("#close").onclick = () => action("fermer");
$("#min").onclick = () => action("reduire");

function recevoirEtat(d) {
  if (!d || typeof d !== "object") return;
  total = Number(d.total) || total; shown = -1;
  if (d.etat === "accueil") accueil(total);
  else if (d.etat === "installation") { if (etat === "idle") installer(); }
  else if (d.etat === "erreur") erreur(String(d.cause || ""));
  else if (d.etat === "pret") { if (d.direct) pretDirect(); else { pret = true; if (etat !== "installing") finir(); } }
}
function recevoirProgression(d) {
  if (!d || typeof d !== "object" || !total) return;
  reel = Math.min(1, Math.max(0, (Number(d.octets) || 0) / total));
}

if (tauri) {
  envoye = true;
  Promise.all([
    tauri.event.listen("installateur:etat", (e) => recevoirEtat(e.payload)),
    tauri.event.listen("installateur:progression", (e) => recevoirProgression(e.payload)),
    tauri.event.listen("installateur:envol", (e) => envol(e.payload && e.payload.duree)),
    tauri.event.listen("installateur:fermer", () => fermer()),
  ]).then(() => tauri.core.invoke("installateur_pret", {}));
} else {
  // Aperçu sans Tauri : installation simulée, pour juger la page dans un navigateur.
  accueil(15_519_744);
  let demoEtat = "idle";
  window.demoAction = (nom) => {
    if (nom === "installer" || nom === "reessayer") { demoEtat = "install"; installer(); fausseCopie(); }
    else if (nom === "terminer" || nom === "fermer") envol(D.envol);
    else if (nom === "essayer") { /* rien en aperçu */ }
  };
  function fausseCopie() {
    reel = 0; pret = false;
    const t0 = performance.now(), dureeMs = new URLSearchParams(location.search).has("erreur") ? 600 : 2600;
    const pas = () => {
      const k = Math.min(1, (performance.now() - t0) / dureeMs);
      reel = k;
      if (new URLSearchParams(location.search).has("erreur") && k > .5) { erreur("La copie de BreezyShot est incomplète. Réessayez."); return; }
      if (k >= 1) { pret = true; return; }
      requestAnimationFrame(pas);
    };
    requestAnimationFrame(pas);
  }
  if (new URLSearchParams(location.search).has("demo")) window.demoAction("installer");
}
