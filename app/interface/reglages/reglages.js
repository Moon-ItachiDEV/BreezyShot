// Fenêtre des Réglages (étape 10). La page montre et rapporte ; le cœur décide de tout
// (ARCHITECTURE.md §2.5) : chaque geste part au cœur, et la page affiche la vue qu'il
// renvoie (un interrupteur refusé revient à sa place parce que la vue le dit).
//
// Contrat avec le cœur (commandes permises à la seule fenêtre `reglages`) :
// - `reglages_lire` {} → Reponse, au chargement et au retour du focus (le cœur relit le
//   fichier) ;
// - `reglages_changer` { changement: { cle, valeur } } → Reponse (« impr_ecran » : valeur =
//   l'identifiant d'un choix de `vue.impr_ecran.choix`) ;
// - `reglages_apercu_nom` { modele } → { apercu } (rien n'est écrit) ;
// - `reglages_parcourir` {} → Reponse (sélecteur de dossier du système) ;
// - `reglages_parcourir_films` {} → Reponse (le même, pour le dossier des films, Pro) ;
// - `reglages_vider_historique` {} → Reponse (Pro : efface les captures de l'historique) ;
// - `reglages_saisie` { action, active } → Reponse (le cœur rend ses raccourcis au système
//   le temps de la saisie, sinon il les recevrait avant la page) ;
// - `reglages_raccourci` { action, saisie: { ctrl, maj, alt, win, cle, code } } → Reponse ;
// - `reglages_retablir` { action } → Reponse ;
// - `licence_activer` { texte } → { active } ou rejet (code du refus : la page dit
//   « Licence non valide », rien d'autre), `licence_retirer` {} → { active } : après l'un ou
//   l'autre, la page redemande sa vue (`reglages_lire`), qui dit `licence_active` et les
//   lignes de Réglages › Raccourcis en vigueur.
// Reponse = { vue, refus: { ligne, texte } | null }. Événements : `reglages:etat` (une vue),
// `reglages:rubrique` ("licence" : ouvrir Général et la ligne Licence, depuis la barre),
// `interface:theme` (commun/theme.js).
//
// Les éléments ne sont trouvés que par data-*, rôles et identifiants, jamais par une
// classe : l'habillage (reglages.css) peut changer sans toucher à ce fichier. Aucune donnée
// ne passe par innerHTML.

import { poserTheme, suivreTheme } from "../commun/theme.js";
import { picto } from "../commun/pictos.js";

// Pictogramme de chaque ligne de Réglages › Raccourcis : celui de l'entrée du menu de
// l'icône qui lance la même action.
const PICTO_ACTION = {
  zone: "capturer_zone",
  plein_ecran: "capturer_plein_ecran",
  fenetre: "capturer_fenetre",
  derniere_zone: "capturer_derniere_zone",
  defilante: "capturer_defilante",
  texte: "capturer_texte",
  video: "capturer_video",
  gif: "capturer_gif",
  historique: "historique",
};

const tauri = window.__TAURI__;
const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));

const ATTENTE_NOM_MS = 400;
const ATTENTE_CONFIRMATION_MS = 4000;
const THEMES = { systeme: null, clair: "light", sombre: "dark" };
const MODIFICATEURS = ["Control", "Shift", "Alt", "Meta", "AltGraph", "OS"];
const LICENCE_REFUSEE = "Licence non valide";

let vue = null;
let saisie = null; // action en cours de saisie
let attenteNom = null; // minuterie de l'écriture du nom

function appeler(commande, args = {}) {
  if (!tauri?.core?.invoke) return Promise.resolve(null);
  return Promise.resolve(tauri.core.invoke(commande, args)).catch((e) => {
    console.warn(`${commande} :`, e);
    return null;
  });
}

// Une réponse du cœur : la vue d'abord, puis le refus sur sa ligne.
function recevoir(reponse) {
  if (!reponse || !reponse.vue) return;
  afficher(reponse.vue);
  const r = reponse.refus;
  if (r && r.texte) {
    const ligne = $(`[data-reglage="${CSS.escape(String(r.ligne))}"]`);
    const p = ligne && $("[data-refus]", ligne);
    if (p) {
      p.textContent = r.texte;
      p.hidden = false;
    }
  }
}

function poserInterrupteur(nom, oui) {
  const b = $(`[role="switch"][data-controle="${nom}"]`);
  if (b) b.setAttribute("aria-checked", oui ? "true" : "false");
}

function poserChoix(nom, valeur) {
  for (const b of $$(`[data-controle="${nom}"] [role="radio"]`)) {
    const oui = b.dataset.valeur === String(valeur);
    b.setAttribute("aria-checked", oui ? "true" : "false");
    b.tabIndex = oui ? 0 : -1;
  }
  // Aucun coché (valeur venue du fichier, hors de la liste) : le premier reste atteignable.
  const radios = $$(`[data-controle="${nom}"] [role="radio"]`);
  if (radios.length && !radios.some((b) => b.tabIndex === 0)) radios[0].tabIndex = 0;
}

function texteDuree(s) {
  return s === 0 ? "Toujours" : `${s} s`;
}

function texteRetardateur(s) {
  return s === 0 ? "Aucun" : `${s} s`;
}

// Segments rendus depuis une liste de secondes de la vue (durées, retardateurs).
function poserSegments(nom, valeurs, texte) {
  const groupe = $(`[data-controle="${nom}"]`);
  const voulues = valeurs.map(String).join(",");
  if (groupe.dataset.valeurs === voulues) return;
  groupe.dataset.valeurs = voulues;
  groupe.replaceChildren(
    ...valeurs.map((d) => {
      const b = document.createElement("button");
      b.setAttribute("role", "radio");
      b.dataset.valeur = String(d);
      b.textContent = texte(d);
      return b;
    }),
  );
}

function texteGardes(n) {
  return n === 0 ? "Aucun" : String(n);
}

function poserRaccourcis(liste) {
  const conteneur = $('[data-liste="raccourcis"]');
  const modele = $('template[data-modele="raccourci"]');
  // Une action qui n'est plus en vigueur (licence retirée) perd sa ligne.
  const voulues = new Set(liste.map((r) => r.action));
  for (const l of $$('[data-reglage="raccourci"]', conteneur)) {
    if (!voulues.has(l.dataset.action)) {
      if (saisie === l.dataset.action) finirSaisie(true);
      l.remove();
    }
  }
  for (const r of liste) {
    let ligne = $(`[data-reglage="raccourci"][data-action="${CSS.escape(r.action)}"]`, conteneur);
    if (!ligne) {
      ligne = modele.content.firstElementChild.cloneNode(true);
      ligne.dataset.action = r.action;
      const id = `l-raccourci-${r.action}`;
      $('[data-valeur="libelle"]', ligne).id = id;
      $('[data-controle="saisie"]', ligne).setAttribute("aria-labelledby", id);
      const nomPicto = PICTO_ACTION[r.action];
      if (nomPicto) $("[data-picto]", ligne).append(picto(nomPicto));
      conteneur.append(ligne);
    }
    // Dans l'ordre de la vue ; une ligne déjà à sa place n'est pas déplacée (clic gardé).
    const place = conteneur.children[liste.indexOf(r)];
    if (place !== ligne) conteneur.insertBefore(ligne, place || null);
    $('[data-valeur="libelle"]', ligne).textContent = r.libelle;
    $('[data-controle="retablir"]', ligne).hidden = !r.modifie;
    const refus = $("[data-refus]", ligne);
    refus.textContent = r.refus || "";
    refus.hidden = !r.refus;
    if (saisie !== r.action) montrerTouches(ligne, r.touches);
  }
}

// Les touches déjà montrées ne sont pas redessinées : la vue est relue au retour du focus,
// et remplacer les <kbd> entre l'appui et le relâchement de la souris ferait perdre le clic
// (premier clic sur la fenêtre inactive).
function montrerTouches(ligne, touches) {
  const bouton = $('[data-controle="saisie"]', ligne);
  const cle = touches.join("\n");
  if (bouton.dataset.touches === cle) return;
  bouton.dataset.touches = cle;
  bouton.replaceChildren(
    ...touches.map((t) => {
      const k = document.createElement("kbd");
      k.textContent = t;
      return k;
    }),
  );
  bouton.setAttribute("aria-description", touches.join(" + "));
}

function afficher(v) {
  vue = v;
  poserTheme(THEMES[v.theme] ?? null);
  const montrees = new Set(v.lignes);
  for (const l of $$("[data-reglage]")) {
    if (l.dataset.reglage === "raccourci") continue;
    l.hidden = !montrees.has(l.dataset.reglage);
  }
  for (const p of $$("[data-refus]")) {
    if (p.closest('[data-reglage="raccourci"]')) continue;
    p.textContent = "";
    p.hidden = true;
  }
  // Le refus d'Impr. écran vient de la vue (une autre application tient la touche).
  if (v.impr_ecran?.refus) {
    const p = $('[data-reglage="impr_ecran"] [data-refus]');
    p.textContent = v.impr_ecran.refus;
    p.hidden = false;
  }
  // Une rubrique sans ligne montrée disparaît, onglet compris.
  for (const s of $$('section[data-rubrique]')) {
    const vide = s.dataset.rubrique !== "raccourcis" && !$$("[data-reglage]", s).some((l) => !l.hidden);
    $(`[role="tab"][data-rubrique="${s.dataset.rubrique}"]`).hidden = vide;
  }
  // L'onglet ouvert caché (Capture sans Gel) : le premier onglet visible s'ouvre.
  const ouvert = $('[role="tab"][aria-selected="true"]');
  if (!ouvert || ouvert.hidden) {
    const premier = $$('[role="tab"]').find((o) => !o.hidden);
    if (premier) montrerRubrique(premier.dataset.rubrique);
  }
  poserChoix("format_image", v.format_image);
  poserInterrupteur("editeur_apres", v.editeur_apres === true);
  poserInterrupteur("son_capture", v.son_capture === true);
  poserChoix("fluidite", v.film.fluidite);
  poserInterrupteur("son_systeme", v.film.son_systeme);
  poserInterrupteur("clics_visibles", v.film.clics_visibles);
  $('[data-valeur="film_dossier"]').textContent = v.film.dossier ?? "Comme les images";
  $('[data-controle="film_dossier_defaut"]').hidden = v.film.dossier === null;
  poserSegments("historique_gardes", v.historique.choix, texteGardes);
  poserChoix("historique_gardes", v.historique.gardes);
  poserInterrupteur("pointeur", v.capture.pointeur);
  poserSegments("retardateur", v.retardateurs, texteRetardateur);
  poserChoix("retardateur", v.capture.retardateur_s);
  poserInterrupteur("copier", v.copier);
  poserInterrupteur("enregistrer", v.enregistrer);
  poserInterrupteur("vignette", v.vignette.afficher);
  poserInterrupteur("demarrage", v.demarrage === "oui");
  poserListeImpr(v.impr_ecran);
  poserSegments("vignette_duree", v.durees, texteDuree);
  poserChoix("vignette_duree", v.vignette.duree_s);
  poserChoix("vignette_coin", v.vignette.coin);
  poserChoix("theme", v.theme);
  for (const nom of ["vignette_duree", "vignette_coin"]) {
    $(`[data-reglage="${nom}"]`).toggleAttribute("data-inactif", !v.vignette.afficher);
  }
  $('[data-valeur="dossier"]').textContent = v.dossier;
  const champ = $('[data-controle="modele_nom"]');
  if (document.activeElement !== champ && attenteNom === null) champ.value = v.modele_nom;
  $('[data-valeur="apercu_nom"]').textContent = v.apercu_nom;
  // Bas du volet : le numéro seul à l'écran (le nom est en haut du volet) ; le texte, lu
  // aussi par un lecteur d'écran, reste « BreezyShot <version> ».
  const nomApp = document.createElement("span");
  nomApp.className = "cache";
  nomApp.textContent = "BreezyShot ";
  const numero = document.createElement("span");
  numero.className = "numero";
  numero.textContent = v.version;
  $('[data-valeur="version"]').replaceChildren(nomApp, numero);
  poserRaccourcis(v.raccourcis);
  poserLicence(v.licence_active === true);
  const alerte = $("[data-alerte]");
  alerte.textContent = v.alerte || "";
  alerte.hidden = !v.alerte;
  appliquerRubrique();
}

// Une rubrique demandée par le cœur (`reglages:rubrique`, barre de capture : un bouton Pro
// éteint) : « licence » ouvre Général, montre la ligne Licence et met le focus sur son
// contrôle. Gardée jusqu'à ce qu'une vue soit affichée, puis appliquée une seule fois.
let rubriqueDemandee = null;
function appliquerRubrique() {
  if (rubriqueDemandee !== "licence" || !vue) return;
  rubriqueDemandee = null;
  montrerRubrique("general");
  const ligne = $('[data-reglage="licence"]');
  if (!ligne || ligne.hidden) return;
  ligne.scrollIntoView({ block: "center" });
  const cible = vue.licence_active === true ? boutonRetirer : champLicence;
  if (!cible.closest("[hidden]")) cible.focus();
}

// Licence : le champ et « Activer » sans licence, « Pro activé » et « Retirer » avec. Le
// texte collé reste dans le champ tant qu'il n'est pas accepté.
const champLicence = $('[data-controle="licence_texte"]');
const boutonActiver = $('[data-controle="activer"]');
const boutonRetirer = $('[data-controle="retirer"]');
let licenceEnCours = false;
// Le dernier texte essayé a été refusé : dit jusqu'à ce que le champ change (une vue relue
// au retour du focus ne l'efface pas).
let licenceRefusee = false;
function poserLicence(active) {
  $('[data-etat-licence="sans"]').hidden = active;
  $('[data-etat-licence="avec"]').hidden = !active;
  $('[data-valeur="pro"]').hidden = !active;
  if (active) {
    champLicence.value = "";
    licenceRefusee = false;
  }
  const p = $('[data-reglage="licence"] [data-refus]');
  p.textContent = licenceRefusee ? LICENCE_REFUSEE : "";
  p.hidden = !licenceRefusee;
  boutonActiver.disabled = licenceEnCours || champLicence.value.trim() === "";
}
// Après Activer ou Retirer : la vue est redemandée, puis le focus va au contrôle montré.
function apresLicence(cible) {
  return appeler("reglages_lire").then((r) => {
    licenceEnCours = false;
    recevoir(r);
    if (vue) poserLicence(vue.licence_active === true);
    if (!cible.closest("[hidden]")) cible.focus();
  });
}
function activerLicence() {
  const texte = champLicence.value;
  if (licenceEnCours || texte.trim() === "" || !tauri?.core?.invoke) return;
  licenceEnCours = true;
  boutonActiver.disabled = true;
  Promise.resolve()
    .then(() => tauri.core.invoke("licence_activer", { texte }))
    .then(
      () => apresLicence(boutonRetirer),
      () => {
        licenceEnCours = false;
        licenceRefusee = true;
        poserLicence(false);
      },
    );
}
champLicence.addEventListener("input", () => {
  licenceRefusee = false;
  poserLicence(false);
});
champLicence.addEventListener("keydown", (e) => {
  if (e.key === "Enter") activerLicence();
});
boutonActiver.addEventListener("click", activerLicence);
boutonRetirer.addEventListener("click", () => {
  if (licenceEnCours) return;
  licenceEnCours = true;
  appeler("licence_retirer").then(() => apresLicence(champLicence));
});

function changer(cle, valeur) {
  return appeler("reglages_changer", { changement: { cle, valeur } }).then(recevoir);
}

// Liste « Touche Impr. écran » : un bouton qui ouvre une liste d'options. Flèches, Début et
// Fin déplacent le focus, Entrée ou Espace choisit, Échap ferme et rend le focus au bouton,
// Tab ferme ; un clic ailleurs ferme aussi. Les options viennent de la vue (`impr_ecran`).
const boutonImpr = $('[data-controle="impr_ecran"]');
const listeImpr = $('[data-menu="impr_ecran"]');
const optionsImpr = () => $$('[role="option"]', listeImpr);
function poserListeImpr(l) {
  if (!l) return;
  const cle = JSON.stringify(l.choix);
  if (listeImpr.dataset.choix !== cle) {
    const ouverte = !listeImpr.hidden;
    const focus = document.activeElement?.dataset?.valeur;
    listeImpr.dataset.choix = cle;
    listeImpr.replaceChildren(
      ...l.choix.flatMap((c, i) => {
        const o = document.createElement("div");
        o.setAttribute("role", "option");
        o.tabIndex = -1;
        o.dataset.valeur = c.id;
        o.textContent = c.libelle;
        if (i === 0 || l.choix[i - 1].groupe === c.groupe) return [o];
        const trait = document.createElement("div");
        trait.className = "trait";
        trait.setAttribute("aria-hidden", "true");
        return [trait, o];
      }),
    );
    // Liste ouverte : le focus suit l'option qu'il avait ; si elle a disparu (licence retirée),
    // la liste se ferme et le focus revient au bouton.
    if (ouverte) {
      const meme = focus && optionsImpr().find((o) => o.dataset.valeur === focus);
      if (meme) meme.focus();
      else {
        ouvrirListeImpr(false);
        boutonImpr.focus();
      }
    }
  }
  for (const o of optionsImpr()) {
    o.setAttribute("aria-selected", o.dataset.valeur === l.valeur ? "true" : "false");
  }
  $('[data-valeur="impr_ecran"]').textContent =
    l.choix.find((c) => c.id === l.valeur)?.libelle ?? "";
}
function ouvrirListeImpr(oui) {
  listeImpr.hidden = !oui;
  boutonImpr.setAttribute("aria-expanded", oui ? "true" : "false");
  if (!oui) return;
  // Bornée à la place sous le bouton : au-delà, la liste défile.
  listeImpr.style.maxHeight = "";
  const place = innerHeight - listeImpr.getBoundingClientRect().top - 8;
  if (listeImpr.offsetHeight > place) listeImpr.style.maxHeight = `${Math.max(place, 96)}px`;
  const options = optionsImpr();
  (options.find((o) => o.getAttribute("aria-selected") === "true") ?? options[0])?.focus();
}
function choisirImpr(valeur) {
  ouvrirListeImpr(false);
  boutonImpr.focus();
  if (vue) changer("impr_ecran", valeur);
}
boutonImpr.addEventListener("click", (e) => {
  e.stopPropagation();
  ouvrirListeImpr(listeImpr.hidden);
});
boutonImpr.addEventListener("keydown", (e) => {
  if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
  e.preventDefault();
  if (listeImpr.hidden) ouvrirListeImpr(true);
});
listeImpr.addEventListener("click", (e) => {
  const o = e.target.closest('[role="option"]');
  if (!o) return;
  e.stopPropagation();
  choisirImpr(o.dataset.valeur);
});
listeImpr.addEventListener("keydown", (e) => {
  const options = optionsImpr();
  const i = options.indexOf(document.activeElement);
  const pas = { ArrowDown: 1, ArrowUp: -1 }[e.key];
  if (pas) {
    e.preventDefault();
    options[(i + pas + options.length) % options.length].focus();
  } else if (e.key === "Home" || e.key === "End") {
    e.preventDefault();
    options[e.key === "Home" ? 0 : options.length - 1].focus();
  } else if (e.key === "Enter" || e.key === " ") {
    e.preventDefault();
    if (i >= 0) choisirImpr(options[i].dataset.valeur);
  } else if (e.key === "Escape") {
    e.preventDefault();
    e.stopPropagation();
    ouvrirListeImpr(false);
    boutonImpr.focus();
  } else if (e.key === "Tab") {
    e.preventDefault();
    ouvrirListeImpr(false);
    boutonImpr.focus();
  }
});
document.addEventListener("click", (e) => {
  if (!listeImpr.hidden && !listeImpr.contains(e.target)) ouvrirListeImpr(false);
});
window.addEventListener("blur", () => ouvrirListeImpr(false));

// Interrupteurs.
for (const b of $$('[role="switch"]')) {
  b.addEventListener("click", () => {
    if (!vue) return;
    const nom = b.dataset.controle;
    changer(nom === "vignette" ? "vignette_afficher" : nom, b.getAttribute("aria-checked") !== "true");
  });
}

// Groupes de choix : clic et flèches.
const CLES_CHOIX = {
  format_image: "format_image",
  fluidite: "fluidite",
  historique_gardes: "historique_gardes",
  retardateur: "retardateur",
  vignette_duree: "vignette_duree",
  vignette_coin: "vignette_coin",
  theme: "theme",
};
const CHOIX_NOMBRES = ["retardateur", "vignette_duree", "fluidite", "historique_gardes"];
function valeurChoix(nom, b) {
  return CHOIX_NOMBRES.includes(nom) ? Number(b.dataset.valeur) : b.dataset.valeur;
}
for (const groupe of $$('[role="radiogroup"]')) {
  const nom = groupe.dataset.controle;
  groupe.addEventListener("click", (e) => {
    const b = e.target.closest('[role="radio"]');
    if (b && vue) changer(CLES_CHOIX[nom], valeurChoix(nom, b));
  });
  groupe.addEventListener("keydown", (e) => {
    const radios = $$('[role="radio"]', groupe);
    const i = radios.indexOf(document.activeElement);
    const pas = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[e.key];
    if (i < 0 || !pas || !vue) return;
    e.preventDefault();
    const b = radios[(i + pas + radios.length) % radios.length];
    changer(CLES_CHOIX[nom], valeurChoix(nom, b)).then(() => b.focus());
  });
}

// Dossier.
$('[data-controle="parcourir"]').addEventListener("click", () => appeler("reglages_parcourir").then(recevoir));

// Dossier des films (Pro) : « Parcourir… », et le retour à « Comme les images ».
$('[data-controle="parcourir_films"]').addEventListener("click", () =>
  appeler("reglages_parcourir_films").then(recevoir));
$('[data-controle="film_dossier_defaut"]').addEventListener("click", () =>
  changer("film_dossier", "defaut"));

// Vider l'historique (Pro) : un premier clic demande « Confirmer », le second vide ; le
// bouton revient à « Vider » seul après un délai, une perte du focus ou un autre geste.
const boutonVider = $('[data-controle="historique_vider"]');
let minuteVider = null;
function reposerVider(texte = "Vider") {
  clearTimeout(minuteVider);
  minuteVider = null;
  boutonVider.textContent = texte;
  boutonVider.removeAttribute("data-confirme");
}
boutonVider.addEventListener("click", () => {
  if (!boutonVider.hasAttribute("data-confirme")) {
    boutonVider.textContent = "Confirmer";
    boutonVider.setAttribute("data-confirme", "");
    minuteVider = setTimeout(() => reposerVider(), ATTENTE_CONFIRMATION_MS);
    return;
  }
  reposerVider("Vidé");
  minuteVider = setTimeout(() => reposerVider(), ATTENTE_CONFIRMATION_MS);
  appeler("reglages_vider_historique").then(recevoir);
});
boutonVider.addEventListener("blur", () => reposerVider());

// Nom des fichiers : aperçu à chaque frappe, écriture après une pause et à la sortie du champ.
const champNom = $('[data-controle="modele_nom"]');
function ecrireNom() {
  if (attenteNom === null) return;
  clearTimeout(attenteNom);
  attenteNom = null;
  changer("modele_nom", champNom.value);
}
champNom.addEventListener("input", () => {
  appeler("reglages_apercu_nom", { modele: champNom.value }).then((r) => {
    if (r && typeof r.apercu === "string") $('[data-valeur="apercu_nom"]').textContent = r.apercu;
  });
  clearTimeout(attenteNom);
  attenteNom = setTimeout(ecrireNom, ATTENTE_NOM_MS);
});
champNom.addEventListener("blur", ecrireNom);
champNom.addEventListener("keydown", (e) => {
  if (e.key === "Enter") ecrireNom();
});

// Menu « Insérer » : Date, Heure.
const boutonInserer = $('[data-controle="inserer"]');
const menuInserer = $('[data-menu="inserer"]');
function ouvrirMenu(oui) {
  menuInserer.hidden = !oui;
  boutonInserer.setAttribute("aria-expanded", oui ? "true" : "false");
  if (oui) $('[role="menuitem"]', menuInserer).focus();
}
boutonInserer.addEventListener("click", (e) => {
  e.stopPropagation();
  ouvrirMenu(menuInserer.hidden);
});
menuInserer.addEventListener("click", (e) => {
  const b = e.target.closest("[data-jeton]");
  if (!b) return;
  e.stopPropagation();
  const debut = champNom.selectionStart ?? champNom.value.length;
  const fin = champNom.selectionEnd ?? debut;
  const avant = champNom.value.slice(0, debut);
  const jeton = (avant && !/\s$/.test(avant) ? " " : "") + b.dataset.jeton;
  champNom.value = avant + jeton + champNom.value.slice(fin);
  ouvrirMenu(false);
  champNom.focus();
  const p = (avant + jeton).length;
  champNom.setSelectionRange(p, p);
  champNom.dispatchEvent(new Event("input"));
});
menuInserer.addEventListener("keydown", (e) => {
  const items = $$('[role="menuitem"]', menuInserer);
  const i = items.indexOf(document.activeElement);
  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    items[(i + (e.key === "ArrowDown" ? 1 : -1) + items.length) % items.length].focus();
  } else if (e.key === "Escape") {
    e.preventDefault();
    e.stopPropagation();
    ouvrirMenu(false);
    boutonInserer.focus();
  }
});
document.addEventListener("click", (e) => {
  if (!menuInserer.hidden && !menuInserer.contains(e.target)) ouvrirMenu(false);
});

// Raccourcis : saisie au clavier.
const conteneurRaccourcis = $('[data-liste="raccourcis"]');
function ligneDe(action) {
  return $(`[data-reglage="raccourci"][data-action="${CSS.escape(action)}"]`, conteneurRaccourcis);
}
function commencerSaisie(action) {
  if (saisie) finirSaisie(true);
  saisie = action;
  const ligne = ligneDe(action);
  ligne.dataset.saisie = "oui";
  const bouton = $('[data-controle="saisie"]', ligne);
  bouton.setAttribute("aria-pressed", "true");
  delete bouton.dataset.touches;
  bouton.textContent = "Appuyez…";
  appeler("reglages_saisie", { action, active: true }).then(recevoir);
}
// Fin sans touche retenue (Échap, Tab, clic ailleurs) : `annuler` prévient le cœur.
function finirSaisie(annuler) {
  if (!saisie) return;
  const action = saisie;
  saisie = null;
  const ligne = ligneDe(action);
  if (ligne) {
    delete ligne.dataset.saisie;
    $('[data-controle="saisie"]', ligne).setAttribute("aria-pressed", "false");
  }
  if (annuler) appeler("reglages_saisie", { action, active: false }).then(recevoir);
  else if (vue) afficher(vue);
}
// Une vue poussée par le cœur (`reglages:etat`). Si elle dit que la saisie est finie
// (filet des 15 s), la page sort de « Appuyez… » sans rappeler le cœur.
function etatPousse(v) {
  if (saisie && v.saisie_en_cours === false) finirSaisie(false);
  afficher(v);
}
function envoyerTouche(e) {
  const action = saisie;
  finirSaisie(false);
  const touche = { ctrl: e.ctrlKey, maj: e.shiftKey, alt: e.altKey, win: e.metaKey, cle: e.key, code: e.code };
  appeler("reglages_raccourci", { action, saisie: touche }).then(recevoir);
}
conteneurRaccourcis.addEventListener("click", (e) => {
  const ligne = e.target.closest('[data-reglage="raccourci"]');
  if (!ligne) return;
  const action = ligne.dataset.action;
  if (e.target.closest('[data-controle="retablir"]')) {
    finirSaisie(true);
    appeler("reglages_retablir", { action }).then(recevoir);
  } else if (e.target.closest('[data-controle="saisie"]')) {
    if (saisie === action) finirSaisie(true);
    else commencerSaisie(action);
  }
});
conteneurRaccourcis.addEventListener("focusout", (e) => {
  if (saisie && e.target.closest?.('[data-controle="saisie"]')) finirSaisie(true);
});
window.addEventListener(
  "keydown",
  (e) => {
    if (!saisie) return;
    if (e.key === "Tab") {
      finirSaisie(true);
      return;
    }
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") {
      finirSaisie(true);
      return;
    }
    if (MODIFICATEURS.includes(e.key)) {
      const tenus = [];
      if (e.ctrlKey) tenus.push("Ctrl");
      if (e.shiftKey) tenus.push("Maj");
      if (e.altKey) tenus.push("Alt");
      if (e.metaKey) tenus.push("Win");
      const bouton = $('[data-controle="saisie"]', ligneDe(saisie));
      if (tenus.length) {
        delete bouton.dataset.touches;
        bouton.replaceChildren(
          ...tenus.map((t) => {
            const k = document.createElement("kbd");
            k.textContent = t;
            return k;
          }),
        );
      }
      return;
    }
    // Windows n'envoie pas d'appui pour Impr. écran, seulement le relâchement.
    if (e.code === "PrintScreen") return;
    envoyerTouche(e);
  },
  true,
);
window.addEventListener(
  "keyup",
  (e) => {
    if (saisie && e.code === "PrintScreen") {
      e.preventDefault();
      envoyerTouche(e);
    }
  },
  true,
);
window.addEventListener("blur", () => finirSaisie(true));

// Rubriques.
const onglets = $$('[role="tab"]');
function montrerRubrique(nom, focus) {
  if (saisie) finirSaisie(true);
  for (const o of onglets) {
    const oui = o.dataset.rubrique === nom;
    o.setAttribute("aria-selected", oui ? "true" : "false");
    o.tabIndex = oui ? 0 : -1;
    if (oui && focus) o.focus();
  }
  const avant = $('[role="tabpanel"]:not([hidden])');
  for (const s of $$('[role="tabpanel"]')) s.hidden = s.dataset.rubrique !== nom;
  // Une autre rubrique s'ouvre en haut.
  if (avant?.dataset.rubrique !== nom) $(".contenu").scrollTop = 0;
}
for (const o of onglets) o.addEventListener("click", () => montrerRubrique(o.dataset.rubrique));
$('[role="tablist"]').addEventListener("keydown", (e) => {
  const visibles = onglets.filter((o) => !o.hidden);
  const i = visibles.indexOf(document.activeElement);
  const pas = { ArrowDown: 1, ArrowRight: 1, ArrowUp: -1, ArrowLeft: -1 }[e.key];
  if (i < 0 || !pas) return;
  e.preventDefault();
  montrerRubrique(visibles[(i + pas + visibles.length) % visibles.length].dataset.rubrique, true);
});

// Pas de menu contextuel du navigateur.
addEventListener("contextmenu", (e) => e.preventDefault(), true);

// Branchement : Tauri si présent, sinon un aperçu fixe pour le navigateur.
if (tauri?.event?.listen) {
  Promise.all([
    tauri.event.listen("reglages:etat", (e) => e?.payload && etatPousse(e.payload)),
    tauri.event.listen("reglages:rubrique", (e) => {
      rubriqueDemandee = e?.payload === "licence" ? "licence" : null;
      appliquerRubrique();
    }),
    suivreTheme(),
  ]).then(
    () => appeler("reglages_lire").then(recevoir),
    (e) => console.warn("reglages : écoute impossible :", e),
  );
  window.addEventListener("focus", () => {
    if (!saisie) appeler("reglages_lire").then(recevoir);
  });
} else {
  afficher({
    lignes: ["retardateur", "pointeur", "copier", "vignette", "vignette_duree",
      "vignette_coin", "dossier", "format_image", "editeur_apres", "son_capture", "enregistrer", "modele_nom", "impr_ecran", "theme", "demarrage", "licence",
      "version"],
    capture: { retardateur_s: 0, pointeur: false },
    retardateurs: [0, 3, 5, 10],
    copier: true,
    enregistrer: false,
    format_image: "png",
    editeur_apres: false,
    son_capture: true,
    film: { fluidite: 30, fluidites: [30, 60], son_systeme: false, clics_visibles: false, dossier: null },
    historique: { gardes: 50, choix: [0, 25, 50, 100, 200] },
    vignette: { afficher: true, duree_s: 6, coin: "bas_gauche" },
    durees: [3, 6, 10, 0],
    dossier: "Images › BreezyShot",
    modele_nom: "BreezyShot {date} à {heure}",
    apercu_nom: "BreezyShot 2026-11-12 à 10.42.08.png",
    theme: "systeme",
    demarrage: "non",
    raccourcis: [
      { action: "zone", libelle: "Zone", touches: ["Ctrl", "Maj", "1"], modifie: false, refus: null },
      { action: "plein_ecran", libelle: "Plein écran", touches: ["Ctrl", "Maj", "2"], modifie: false, refus: null },
      { action: "fenetre", libelle: "Fenêtre", touches: ["Ctrl", "Maj", "3"], modifie: false, refus: null },
      {
        action: "derniere_zone",
        libelle: "Refaire la dernière zone",
        touches: ["Ctrl", "Maj", "4"],
        modifie: false,
        refus: null,
      },
    ],
    impr_ecran: {
      choix: [
        { id: "windows", libelle: "Outil Capture de Windows", groupe: 0 },
        { id: "barre", libelle: "Barre de capture", groupe: 1 },
        { id: "zone", libelle: "Zone", groupe: 1 },
        { id: "plein_ecran", libelle: "Plein écran", groupe: 1 },
        { id: "fenetre", libelle: "Fenêtre", groupe: 1 },
        { id: "derniere_zone", libelle: "Refaire la dernière zone", groupe: 1 },
      ],
      valeur: "zone",
      refus: null,
    },
    licence_active: false,
    version: "0.0.0",
    alerte: null,
    saisie_en_cours: false,
  });
}
