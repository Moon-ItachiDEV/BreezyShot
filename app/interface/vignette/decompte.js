// Décompte du retardateur dans la fenêtre du bandeau (contrat 0.4 §4.3). La page montre et
// rapporte, le cœur décide et cache la fenêtre (ARCHITECTURE.md §6).
//
// Contrat avec le cœur (branché par bandeau.js) :
// - `bandeau:decompte` { id, secondes, restant_ms, diametre, decalage: { x, y },
//   reduites } : montre le disque (diamètre en pixels CSS), le chiffre ceil(restant)
//   (3, 2, 1, jamais 0) et l'anneau qui avance. Le disque et sa croix sont déplacés de
//   `decalage` depuis le centre de la page, pour rester sur la cible quand la fenêtre a été
//   ramenée dans l'écran (le cœur le borne ; absent : centré). Il remplace la pilule d'un
//   message, sans `bandeau_termine`. Charge invalide : console.warn, rien ne change.
// - `bandeau:decompte_cache` { id } : la fenêtre est cachée, le disque est effacé.
// - Un `bandeau:afficher` efface le disque (bandeau.js appelle `effacer`).
// - `bandeau_zone` { id, x, y, largeur, hauteur } : rectangle de la croix seulement (pixels
//   CSS, relatif à la page), une fois, puis à chaque changement de taille. Le disque laisse
//   passer les clics.
// - `bandeau_action` { id, action: "annuler" } au clic de la croix, une seule fois.
// Animations réduites (`reduites` ou préférence du navigateur) : aucune animation, l'anneau
// avance d'un cran par seconde.

const ANNEAU = 2 * Math.PI * 48; // longueur du cercle de rayon 48 dans le dessin 100 × 100

/** Les images d'une entrée : fondu et échelle de départ `depart`. */
const entree = (depart) => [
  { opacity: 0, transform: `scale(${depart})` },
  { opacity: 1, transform: "none" },
];
const DIAMETRE_MAX_PAGE = 148; // le disque et sa croix tiennent dans les 220 px de la page
const DECALAGE_MAX = { x: 280, y: 110 }; // la moitié de la page de 560 × 220

const racine = document.getElementById("decompte");
const chiffre = racine.querySelector(".chiffre");
const progression = racine.querySelector(".progression");
const croix = racine.querySelector(".annuler");
const preference = matchMedia("(prefers-reduced-motion: reduce)");
progression.style.strokeDasharray = String(ANNEAU);

const jeton = (nom) => getComputedStyle(document.documentElement).getPropertyValue(nom).trim();
function duree(nom) {
  const v = jeton(nom);
  const n = parseFloat(v);
  if (!Number.isFinite(n)) return 0;
  return v.endsWith("ms") || !v.endsWith("s") ? n : n * 1000;
}
const nombre = (nom, defaut) => {
  const n = parseFloat(jeton(nom));
  return Number.isFinite(n) ? n : defaut;
};

/** Vérifie une charge `bandeau:decompte` ; null si elle doit être rejetée. */
export function valider(c) {
  const rejeter = (raison) => {
    console.warn(`decompte : charge rejetée (${raison})`);
    return null;
  };
  if (!c || typeof c !== "object") return rejeter("pas un objet");
  const idNombre = typeof c.id === "number" && Number.isFinite(c.id);
  const idValide = idNombre || (typeof c.id === "string" && c.id !== "");
  if (!idValide) return rejeter("id absent");
  if (!Number.isInteger(c.secondes) || c.secondes < 1 || c.secondes > 3600) {
    return rejeter("secondes");
  }
  if (typeof c.restant_ms !== "number" || !Number.isFinite(c.restant_ms) || c.restant_ms < 0) {
    return rejeter("restant_ms");
  }
  const d = c.diametre;
  if (typeof d !== "number" || !Number.isFinite(d) || d <= 0 || d > DIAMETRE_MAX_PAGE) {
    return rejeter("diametre");
  }
  if (c.reduites !== undefined && typeof c.reduites !== "boolean") return rejeter("reduites");
  const decalage = c.decalage ?? { x: 0, y: 0 };
  const coordonnee = (v, max) => typeof v === "number" && Number.isFinite(v) && Math.abs(v) <= max;
  const decalageValide =
    typeof decalage === "object" &&
    coordonnee(decalage.x, DECALAGE_MAX.x) &&
    coordonnee(decalage.y, DECALAGE_MAX.y);
  if (!decalageValide) {
    return rejeter("decalage");
  }
  return {
    id: c.id,
    total: c.secondes * 1000,
    restant: Math.min(c.restant_ms, c.secondes * 1000),
    diametre: d,
    decalage: { x: decalage.x, y: decalage.y },
    reduit: c.reduites === true || preference.matches,
  };
}

/**
 * Prépare le décompte. `appeler(commande, args)` parle au cœur ; `effacerMessage()` retire la
 * pilule du message en cours sans `bandeau_termine`. Rend { montrer, cacher, effacer }.
 */
export function creerDecompte({ appeler, effacerMessage }) {
  // Le décompte affiché : { id, fin, total, reduit, dernier, minuterie, image, ferme, zone }
  let courant = null;

  function arreter() {
    if (!courant) return;
    clearTimeout(courant.minuterie);
    cancelAnimationFrame(courant.image);
    for (const a of racine.getAnimations({ subtree: true })) a.cancel();
  }

  function effacer() {
    arreter();
    courant = null;
    racine.hidden = true;
    racine.classList.remove("ferme", "reduit");
  }

  /** Rectangle de mise en page de la croix (sans transformation), relatif à la page. */
  function envoyerZone() {
    if (!courant || courant.ferme || racine.hidden) return;
    let x = 0;
    let y = 0;
    for (let n = croix; n; n = n.offsetParent) {
      x += n.offsetLeft;
      y += n.offsetTop;
    }
    const zone = { id: courant.id, x, y, largeur: croix.offsetWidth, hauteur: croix.offsetHeight };
    const cle = JSON.stringify(zone);
    if (cle === courant.zone) return;
    courant.zone = cle;
    appeler("bandeau_zone", zone);
  }
  new ResizeObserver(envoyerZone).observe(croix);
  addEventListener("resize", envoyerZone);

  function tic() {
    const lui = courant;
    if (!lui) return;
    const restant = Math.max(0, lui.fin - performance.now());
    const n = Math.max(1, Math.ceil(restant / 1000));
    if (n !== lui.dernier) {
      lui.dernier = n;
      chiffre.textContent = String(n);
      if (!lui.reduit) {
        const depart = jeton("--decompte-chiffre-echelle");
        chiffre.animate(entree(depart), {
          duration: duree("--duree-decompte-chiffre"),
          easing: jeton("--courbe-douce"),
        });
      }
    }
    // Réduites : l'anneau avance d'un cran par seconde, avec le chiffre.
    const ecoule = lui.reduit ? Math.max(0, lui.total - n * 1000) : lui.total - restant;
    const part = Math.min(ecoule, lui.total) / lui.total;
    progression.style.strokeDashoffset = String(-ANNEAU * part);
    if (restant <= 0) return; // reste sur « 1 » : le cœur cache la fenêtre
    if (lui.reduit) lui.minuterie = setTimeout(tic, restant % 1000 || 1000);
    else lui.image = requestAnimationFrame(tic);
  }

  function montrer(brut) {
    const d = valider(brut);
    if (!d) return;
    effacerMessage();
    arreter();
    courant = {
      id: d.id,
      fin: performance.now() + d.restant,
      total: d.total,
      reduit: d.reduit,
      dernier: -1,
      minuterie: 0,
      image: 0,
      ferme: false,
      zone: null,
    };
    racine.classList.remove("ferme");
    racine.classList.toggle("reduit", d.reduit);
    racine.style.setProperty("--decompte-diametre", `${d.diametre}px`);
    racine.style.setProperty("--decompte-dx", `${d.decalage.x}px`);
    racine.style.setProperty("--decompte-dy", `${d.decalage.y}px`);
    racine.hidden = false;
    if (!d.reduit) {
      const depart = nombre("--decompte-entree-echelle", 1);
      racine.animate(entree(depart), {
        duration: duree("--duree-ressort"),
        easing: jeton("--courbe-ressort"),
      });
    }
    tic();
    envoyerZone();
  }

  function cacher(charge) {
    if (courant && charge && charge.id === courant.id) effacer();
  }

  function annuler() {
    const lui = courant;
    if (!lui || lui.ferme || racine.hidden) return;
    lui.ferme = true; // les clics suivants sont ignorés ; le cœur cache la fenêtre
    racine.classList.add("ferme");
    appeler("bandeau_action", { id: lui.id, action: "annuler" });
  }

  // Comme les boutons du bandeau : la fenêtre n'a jamais le focus, on agit quand le bouton
  // gauche est relâché sur la croix où il a été appuyé, sans attendre « click ».
  let appuye = false;
  const surCroix = (e) => e.target instanceof Element && croix.contains(e.target);
  addEventListener(
    "pointerdown",
    (e) => {
      appuye = e.button === 0 && surCroix(e);
      if (racine.contains(e.target)) e.preventDefault(); // ne prend pas le focus
    },
    true,
  );
  addEventListener(
    "pointerup",
    (e) => {
      const avant = appuye;
      appuye = false;
      if (e.button === 0 && avant && surCroix(e)) annuler();
    },
    true,
  );
  addEventListener("pointercancel", () => (appuye = false), true);
  addEventListener("blur", () => (appuye = false));
  croix.addEventListener("click", (e) => {
    // Clic sans pointeur (lecteur d'écran) : rien avant, on agit ici.
    if (e.detail === 0) annuler();
  });

  return { montrer, cacher, effacer };
}
