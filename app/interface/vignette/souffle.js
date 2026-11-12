// Souffle : le flash sur la zone capturée, puis l'image qui s'envole jusqu'à la carte
// (ARCHITECTURE.md §11). La page joue, le cœur montre et cache la fenêtre.
//
// Contrat avec le cœur :
// - `souffle:jouer` { id, depart, arrivee } (rectangles { x, y, largeur, hauteur } en pixels
//   réels, relatifs à la fenêtre) : lit l'aperçu (`vignette_image` { id, quoi: "apercu" }),
//   joue le flash et l'envol, puis `souffle_termine` { id }. La dernière image reste jusqu'à
//   ce que le cœur cache la fenêtre. Sans image lisible : `souffle_termine` tout de suite.
// - `souffle:effacer` { id } : la page se vide (fenêtre déjà cachée).
// - `souffle_pret` {} une fois l'écoute en place (thème compris : l'ombre de l'envol suit
//   `interface:theme`, voir commun/theme.js).

import { suivreTheme } from "../commun/theme.js";

const tauri = window.__TAURI__;
const flash = document.getElementById("flash");
const envol = document.getElementById("envol");

const jeton = (nom) => getComputedStyle(document.documentElement).getPropertyValue(nom).trim();
function duree(nom) {
  const v = jeton(nom);
  const n = parseFloat(v);
  if (!Number.isFinite(n)) return 0;
  return v.endsWith("ms") || !v.endsWith("s") ? n : n * 1000;
}

function appeler(commande, args) {
  if (!tauri?.core?.invoke) return Promise.resolve(undefined);
  try {
    return Promise.resolve(tauri.core.invoke(commande, args)).catch((e) => {
      console.warn(`${commande} :`, e);
      return undefined;
    });
  } catch (e) {
    console.warn(`${commande} :`, e);
    return Promise.resolve(undefined);
  }
}

// Dit « prête » au cœur. Un refus (sous Windows, la page peut parler avant que le cœur ait
// fini de se mettre en place) est retenté sans bruit toutes les 100 ms, 10 s au plus.
const ESSAIS_PRET = 100;
function direPret(commande, reste = ESSAIS_PRET) {
  new Promise((ok) => ok(tauri.core.invoke(commande, {}))).catch((e) => {
    if (reste > 1) setTimeout(() => direPret(commande, reste - 1), 100);
    else console.warn(`${commande} : refusé ${ESSAIS_PRET} fois :`, e);
  });
}

/** Un rectangle reçu, en pixels CSS ; null s'il n'est pas valable. */
function rect(r) {
  const ok = (v) => typeof v === "number" && Number.isFinite(v);
  if (
    !r ||
    !ok(r.x) ||
    !ok(r.y) ||
    !ok(r.largeur) ||
    !ok(r.hauteur) ||
    r.largeur <= 0 ||
    r.hauteur <= 0
  )
    return null;
  const d = window.devicePixelRatio || 1;
  return { x: r.x / d, y: r.y / d, l: r.largeur / d, h: r.hauteur / d };
}

// L'envol en cours : { id, url, animations }
let courant = null;

function effacer() {
  if (courant) {
    for (const a of courant.animations) a.cancel();
    if (courant.url) URL.revokeObjectURL(courant.url);
  }
  courant = null;
  flash.hidden = true;
  envol.hidden = true;
  envol.removeAttribute("src");
}

export async function jouer(charge) {
  const depart = rect(charge?.depart);
  const arrivee = rect(charge?.arrivee);
  if (typeof charge?.id !== "number" || !depart || !arrivee) {
    console.warn("souffle : charge rejetée");
    return;
  }
  effacer();
  const lui = { id: charge.id, url: null, animations: [] };
  courant = lui;
  const finir = () => {
    if (lui === courant && !lui.fini) {
      lui.fini = true;
      appeler("souffle_termine", { id: lui.id });
    }
  };
  const octets = await appeler("vignette_image", { id: lui.id, quoi: "apercu" });
  if (lui !== courant) return;
  if (!octets) return finir();
  lui.url = URL.createObjectURL(
    new Blob([Array.isArray(octets) ? new Uint8Array(octets) : octets], { type: "image/png" }),
  );
  envol.src = lui.url;
  try {
    await envol.decode();
  } catch {
    return finir();
  }
  if (lui !== courant) return;

  for (const [el, r] of [
    [flash, depart],
    [envol, depart],
  ]) {
    el.style.left = `${r.x}px`;
    el.style.top = `${r.y}px`;
    el.style.width = `${r.l}px`;
    el.style.height = `${r.h}px`;
    el.hidden = false;
  }
  lui.animations.push(
    flash.animate([{ opacity: 0.55 }, { opacity: 0 }], {
      duration: duree("--duree-souffle-flash"),
      easing: jeton("--courbe-douce"),
      fill: "forwards",
    }),
  );
  const dx = arrivee.x - depart.x;
  const dy = arrivee.y - depart.y;
  const kx = arrivee.l / depart.l;
  const ky = arrivee.h / depart.h;
  const arc = parseFloat(jeton("--vignette-arc")) || 0;
  const rayon = jeton("--vignette-rayon");
  const a = envol.animate(
    [
      { transform: "translate(0px, 0px) scale(1)", borderRadius: "2px" },
      { transform: "translate(0px, 0px) scale(.95)", offset: 0.24 },
      {
        transform:
          `translate(${dx * 0.5}px, ${dy * 0.5 - arc}px) ` +
          `scale(${(0.95 + kx) / 2}, ${(0.95 + ky) / 2})`,
        offset: 0.62,
      },
      { transform: `translate(${dx}px, ${dy}px) scale(${kx}, ${ky})`, borderRadius: rayon },
    ],
    { duration: duree("--duree-envol"), easing: jeton("--courbe-envol"), fill: "forwards" },
  );
  lui.animations.push(a);
  a.onfinish = finir;
  a.oncancel = finir;
}

if (tauri?.event?.listen) {
  Promise.all([
    tauri.event.listen("souffle:jouer", (e) => jouer(e?.payload)),
    tauri.event.listen("souffle:effacer", () => effacer()),
    suivreTheme(),
  ]).then(
    () => direPret("souffle_pret"),
    (e) => console.warn("souffle : écoute impossible :", e),
  );
} else {
  window.souffleApercu = { jouer, effacer };
}
