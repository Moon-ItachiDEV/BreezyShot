// Habillage de la fenêtre des Réglages (piste B « Volet ») : rien que de l'affichage.
// Aucune commande, aucun changement de réglage : ce module lit les états aria-* que pose
// reglages.js et en tire
// - la place du repère d'accent devant la rubrique choisie (volet) et des pastilles des
//   choix (variables CSS) ;
// - le repli de ce qui dépend d'un interrupteur éteint (Nom des fichiers sous
//   « Enregistrer automatiquement », Durée et Coin sous « Vignette ») : `data-replie` et
//   `inert`, pour que le clavier n'entre pas dans une partie repliée.
// Sans lui, la page marche et reste lisible (repère et pastilles absents, rien de replié).

const $$ = (s, r = document) => [...r.querySelectorAll(s)];
const nav = document.querySelector('[role="tablist"]');

function sansAnimation(el, faire) {
  el.classList.add("sans-animation");
  faire();
  void el.offsetWidth;
  el.classList.remove("sans-animation");
}

function placerRepere(premiere) {
  const o = nav && nav.querySelector('[role="tab"][aria-selected="true"]');
  if (!o || !o.offsetParent) return;
  const poser = () => {
    nav.style.setProperty("--repere-y", `${o.offsetTop}px`);
    nav.style.setProperty("--repere-h", `${o.offsetHeight}px`);
  };
  if (premiere) sansAnimation(nav, poser);
  else poser();
}

function placerPastilles(premiere) {
  for (const g of $$(".choix")) {
    const b = g.querySelector('[role="radio"][aria-checked="true"]');
    g.toggleAttribute("data-choisi", !!b);
    if (!b || !g.offsetParent) continue;
    const poser = () => {
      g.style.setProperty("--pastille-x", `${b.offsetLeft}px`);
      g.style.setProperty("--pastille-l", `${b.offsetWidth}px`);
    };
    if (premiere || !g.hasAttribute("data-place")) sansAnimation(g, poser);
    else poser();
    g.setAttribute("data-place", "");
  }
}

function replier() {
  for (const r of $$("[data-repli]")) {
    const ligne = document.querySelector(`[data-reglage="${r.dataset.repli}"]`);
    const inter = ligne && ligne.querySelector('[role="switch"]');
    const replie = !!ligne && !ligne.hidden && inter?.getAttribute("aria-checked") === "false";
    r.toggleAttribute("data-replie", replie);
    r.inert = replie;
  }
}

function tout(premiere = false) {
  replier();
  placerRepere(premiere);
  placerPastilles(premiere);
}

new MutationObserver(() => tout()).observe(document.body, {
  subtree: true,
  childList: true,
  attributes: true,
  attributeFilter: ["aria-selected", "aria-checked", "hidden"],
});
addEventListener("resize", () => tout(true));
document.fonts?.ready.then(() => tout(true));
tout(true);
