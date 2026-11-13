// Copier le texte dans l'éditeur (contrat 0.5, §2.4, §5.6) : bouton « Texte » de la barre du
// haut, balayage de l'image, calque « Choisir le texte » (boîtes des mots, barre flottante,
// clavier). La page ne reçoit jamais le texte : seulement les boîtes des mots, en pixels de
// l'image composée (le cadre du recadrage). Elle rend des indices de mots.
//
// Avec le cœur : `editeur_exporter` de but `texte` (l'image composée, R7) → numéro ;
// `editeur:texte` { id, numero, etat: "lu" | "vide" | "echec", mots: [{ x, y, largeur,
// hauteur, ligne }] } ; `editeur_copier_texte` { id, numero, debut, fin } (indices inclus) ;
// `editeur:texte_copie` { id, numero, etat: "fait" | "echec" }.
//
// Le bouton n'existe que si la charge le dit (`texte: true`, R10) : sinon rien n'est créé.
// Tant que le calque est ouvert, tout le clavier est à lui, où que soit le focus (écoute en
// capture sur `window`) : Échap ne ferme que lui, Ctrl + C copie le texte choisi (jamais
// l'image), les touches des outils et du zoom sont sans effet, Tab reste dans le calque et sa
// barre. « Enregistrer » et « Copier » de la barre du haut sont grisés ; « Fermer » ferme
// d'abord le calque. La fenêtre redimensionnée replace les boîtes (`placer`).

const SVG = "http://www.w3.org/2000/svg";
const MARGE_TOUCHE = 2; // pixels CSS autour d'une boîte, comme le design
const PRES_MAX = 60; // tirer hors d'un mot : le mot le plus proche, jusqu'à cette distance

function picto(id) {
  const s = document.createElementNS(SVG, "svg");
  s.setAttribute("class", "ico");
  s.setAttribute("aria-hidden", "true");
  const u = document.createElementNS(SVG, "use");
  u.setAttribute("href", `#${id}`);
  s.append(u);
  return s;
}
function bouton(classe, cle, texte) {
  const b = document.createElement("button");
  b.type = "button";
  b.className = classe;
  b.dataset.texte = cle;
  if (texte) b.textContent = texte;
  return b;
}

// Boîtes reçues : entiers positifs, sinon la liste est refusée (lue comme un échec).
function mots(liste) {
  if (!Array.isArray(liste)) return null;
  const ok = (v) => Number.isFinite(v) && v >= 0;
  const r = [];
  for (const m of liste) {
    if (!m || ![m.x, m.y, m.largeur, m.hauteur, m.ligne].every(ok)) return null;
    r.push({ x: m.x, y: m.y, largeur: m.largeur, hauteur: m.hauteur, ligne: m.ligne });
  }
  return r;
}

export function installerTexte(ed, { appeler, message, reduites }) {
  const fenetre = document.getElementById("fenetre");
  const enregistrer = document.querySelector('#barre-haut [data-act="enregistrer"]');
  const bCopier = document.querySelector('#barre-haut [data-act="copier"]');
  const bFermer = document.querySelector('#barre-haut [data-act="fermer"]');
  let bTexte = null;
  // La lecture en cours ou le choix : { numero, recu, balaye, phase, calque, ... }, ou null.
  let l = null;
  let retour = null; // focus rendu à la sortie
  const premiers = new Map(); // `editeur:texte` reçus avant le numéro de leur export

  // ---------- bouton « Texte » ----------
  function poserBouton(voulu) {
    if (voulu && !bTexte) {
      bTexte = bouton("rb lib", "lancer", "");
      bTexte.title = "Copier le texte";
      const s = document.createElement("span");
      s.textContent = "Texte";
      bTexte.append(picto("i-ocr"), s);
      bTexte.addEventListener("click", () => lancer());
      enregistrer.before(bTexte);
    } else if (!voulu && bTexte) {
      bTexte.remove();
      bTexte = null;
    }
    synchroniser();
  }
  function synchroniser() {
    if (bTexte) bTexte.disabled = ed.exportEnCours();
    enregistrer.disabled = !!l;
    bCopier.disabled = !!l;
  }
  // « Fermer » de la barre du haut pendant le calque : le calque se ferme d'abord, puis le
  // geste suit son cours (editeur.js).
  bFermer.addEventListener("click", () => fermer(false), true);

  // ---------- géométrie : pixels de l'image composée → pixels CSS du calque ----------
  function geometrie() {
    const c = ed.modele.cadre(ed.h.doc);
    const { x, y, echelle, dpr } = ed.vue;
    const t = ed.toile.getBoundingClientRect();
    const k = l.calque.getBoundingClientRect();
    const ox = t.left - k.left + (x + c.x * echelle) / dpr;
    const oy = t.top - k.top + (y + c.y * echelle) / dpr;
    const s = echelle / dpr; // pixels CSS par pixel de l'image
    return { ox, oy, s, l: c.largeur * s, h: c.hauteur * s, kl: k.width, kh: k.height };
  }
  function poser(n, x, y, w, h) {
    n.style.left = `${x}px`;
    n.style.top = `${y}px`;
    n.style.width = `${w}px`;
    n.style.height = `${h}px`;
  }
  function placer() {
    if (!l || !ed.h) return;
    const g = geometrie();
    // La zone balayée : la partie visible de l'image.
    const x0 = Math.max(0, g.ox);
    const y0 = Math.max(0, g.oy);
    const x1 = Math.min(g.kl, g.ox + g.l);
    const y1 = Math.min(g.kh, g.oy + g.h);
    poser(l.zone, x0, y0, Math.max(0, x1 - x0), Math.max(0, y1 - y0));
    if (l.phase !== "choix") return;
    l.mots.forEach((m, i) => {
      poser(l.boites[i], g.ox + m.x * g.s, g.oy + m.y * g.s, m.largeur * g.s, m.hauteur * g.s);
    });
    // La barre : dans le bas de la partie visible de l'image, centrée, dans le calque.
    const bw = l.barre.offsetWidth;
    const bh = l.barre.offsetHeight;
    const marge = 8;
    const haut = Math.min(y1, g.kh) - bh - marge;
    const gauche = (x0 + x1) / 2 - bw / 2;
    l.barre.style.left = `${Math.max(marge, Math.min(g.kl - bw - marge, gauche))}px`;
    l.barre.style.top = `${Math.max(marge, Math.min(g.kh - bh - marge, haut))}px`;
  }

  // ---------- calque ----------
  function creerCalque() {
    const calque = document.createElement("div");
    calque.className = "calque-texte";
    calque.tabIndex = -1;
    calque.setAttribute("role", "application");
    calque.setAttribute("aria-label", "Copier le texte");
    const zone = document.createElement("div");
    zone.className = "balayage";
    const bande = document.createElement("i");
    bande.className = "bande";
    zone.append(bande);
    calque.append(zone);
    fenetre.append(calque);
    return { calque, zone };
  }

  function fermer(rendre = true) {
    if (!l) return;
    const o = l;
    l = null;
    clearTimeout(o.minuterie);
    window.removeEventListener("keydown", o.ecoute, true);
    o.calque.remove();
    synchroniser();
    if (rendre) {
      const r = retour?.isConnected ? retour : ed.scene;
      r.focus({ preventScroll: true });
    }
    retour = null;
  }

  async function lancer() {
    if (!ed.charge || !ed.h || !ed.fond || ed.exportEnCours()) return;
    fermer(false);
    ed.barre?.fermerMenus?.(false);
    retour = document.activeElement;
    const { calque, zone } = creerCalque();
    const lu = { calque, zone, phase: "balayage", numero: null, recu: null, balaye: false };
    l = lu;
    lu.ecoute = garder(calque);
    synchroniser();
    placer();
    calque.focus({ preventScroll: true });
    // Balayage : 400 ms (respiration, bande, teinte) ; animations réduites : 120 ms, sans
    // bande (règle commune des animations réduites de la page).
    const reduit = reduites();
    calque.classList.toggle("reduit", reduit);
    zone.classList.add("joue");
    const duree = parseFloat(getComputedStyle(zone).getPropertyValue("--duree-balayage")) || 0;
    lu.minuterie = setTimeout(() => {
      if (l !== lu) return;
      lu.balaye = true;
      zone.classList.remove("joue");
      suite();
    }, duree);
    const numero = await ed.exporter("texte");
    synchroniser();
    if (l !== lu) {
      // Calque fermé avant le numéro : un résultat déjà reçu finit l'export.
      if (numero != null && premiers.has(numero)) terminerExport(numero);
      premiers.clear();
      return;
    }
    if (numero == null) {
      fermer();
      message("Impossible de lire le texte");
      return;
    }
    lu.numero = numero;
    const avant = premiers.get(numero);
    premiers.clear();
    if (avant) {
      terminerExport(numero);
      lu.recu = avant;
    }
    suite();
  }

  // Le résultat de la lecture : montré quand le balayage est fini.
  function recu(r) {
    if (!r || !ed.charge || r.id !== ed.charge.id || !Number.isSafeInteger(r.numero)) return;
    // Arrivé avant le numéro de l'export : gardé pour `lancer`.
    if (!terminerExport(r.numero)) {
      if (premiers.size > 8) premiers.clear();
      premiers.set(r.numero, r);
      return;
    }
    if (l && r.numero === l.numero) l.recu = r;
    suite();
  }
  // L'export de but `texte` finit à `editeur:texte` (pas d'`editeur:resultat`).
  function terminerExport(numero) {
    const e = ed.exports.get(numero);
    if (!e || e.but !== "texte") return false;
    ed.exports.delete(numero);
    ed.finExport();
    return true;
  }
  function suite() {
    if (!l || !l.balaye || !l.recu || l.phase !== "balayage") return;
    const r = l.recu;
    const liste = r.etat === "lu" ? mots(r.mots) : null;
    if (r.etat === "lu" && liste && liste.length) {
      choisir(liste);
    } else if (r.etat === "vide" || (r.etat === "lu" && liste && !liste.length)) {
      fermer();
      message("Aucun texte trouvé", true);
    } else {
      fermer();
      message("Impossible de lire le texte");
    }
  }

  // ---------- choix des mots ----------
  function choisir(liste) {
    const o = l;
    o.phase = "choix";
    o.mots = liste;
    o.ancre = null;
    o.fin = null;
    o.curseur = 0;
    o.clavier = false;
    o.calque.classList.add("choix");
    o.calque.setAttribute("aria-label", "Choisir le texte");
    const boites = document.createElement("div");
    boites.className = "mots";
    o.boites = liste.map(() => {
      const b = document.createElement("i");
      b.className = "mot";
      boites.append(b);
      return b;
    });
    const barre = document.createElement("div");
    barre.className = "barre-texte";
    barre.setAttribute("role", "toolbar");
    barre.setAttribute("aria-label", "Texte");
    o.copier = bouton("bt accent", "copier", "Copier");
    const tout = bouton("bt", "tout", "Tout copier");
    const croix = bouton("rb petit", "fermer", "");
    croix.setAttribute("aria-label", "Fermer");
    croix.title = "Fermer";
    croix.append(picto("i-x"));
    barre.append(o.copier, tout, croix);
    o.barre = barre;
    o.calque.append(boites, barre);
    barre.addEventListener("click", (e) => {
      const b = e.target.closest("button");
      if (!b || b.disabled) return;
      if (b.dataset.texte === "copier") copier(false);
      else if (b.dataset.texte === "tout") copier(true);
      else fermer();
    });
    barre.addEventListener("pointerdown", (e) => e.stopPropagation());
    placer();
    peindre();
    o.calque.focus({ preventScroll: true });
  }

  const choix = () => {
    if (!l || l.ancre === null) return null;
    return [Math.min(l.ancre, l.fin), Math.max(l.ancre, l.fin)];
  };
  function peindre() {
    if (!l || l.phase !== "choix") return;
    const c = choix();
    const curseur = l.clavier && document.activeElement === l.calque;
    l.boites.forEach((b, i) => {
      b.classList.toggle("choisi", !!c && i >= c[0] && i <= c[1]);
      b.classList.toggle("curseur", curseur && i === l.curseur);
      b.classList.toggle("survol", i === l.survol);
    });
    l.copier.disabled = !c;
  }

  function copier(tout) {
    if (!l || l.phase !== "choix") return;
    const c = tout ? [0, l.mots.length - 1] : choix();
    if (!c) return;
    appeler("editeur_copier_texte", {
      id: ed.charge.id,
      numero: l.numero,
      debut: c[0],
      fin: c[1],
    }).catch((e) => {
      console.info("editeur_copier_texte :", e);
      message("Impossible de copier");
    });
    fermer();
  }
  function copie(r) {
    if (!r || !ed.charge || r.id !== ed.charge.id) return;
    if (r.etat === "fait") message("Texte copié", false);
    else message("Impossible de copier");
  }

  // Le mot sous le point (pixels CSS de la page) ; `pres` : à défaut, le plus proche.
  function sous(cx, cy, pres) {
    const g = geometrie();
    const k = l.calque.getBoundingClientRect();
    const px = cx - k.left;
    const py = cy - k.top;
    let mieux = -1;
    let distance = Infinity;
    for (let i = 0; i < l.mots.length; i++) {
      const m = l.mots[i];
      const x = g.ox + m.x * g.s;
      const y = g.oy + m.y * g.s;
      const w = m.largeur * g.s;
      const h = m.hauteur * g.s;
      const M = MARGE_TOUCHE;
      if (px >= x - M && px <= x + w + M && py >= y - M && py <= y + h + M) return i;
      if (pres) {
        const dy = Math.max(0, Math.abs(py - (y + h / 2)) - h / 2);
        const dx = Math.max(0, Math.abs(px - (x + w / 2)) - w / 2);
        const d = dy * 4 + dx;
        if (d < distance) {
          distance = d;
          mieux = i;
        }
      }
    }
    return pres && distance < PRES_MAX ? mieux : -1;
  }

  // Le mot de la ligne voisine (sens +1 ou -1) le plus proche en x, sinon le même.
  function ligneVoisine(i, sens) {
    const ms = l.mots;
    let j = i;
    while (j >= 0 && j < ms.length && ms[j].ligne === ms[i].ligne) j += sens;
    if (j < 0 || j >= ms.length) return i;
    const autre = ms[j].ligne;
    let mieux = j;
    let d = Infinity;
    for (let q = j; q >= 0 && q < ms.length && ms[q].ligne === autre; q += sens) {
      const e = Math.abs(ms[q].x - ms[i].x);
      if (e < d) {
        d = e;
        mieux = q;
      }
    }
    return mieux;
  }

  // Le calque garde tout : ni la vue, ni les gestes, ni les raccourcis de l'éditeur.
  function garder(calque) {
    let tire = false;
    calque.addEventListener("wheel", (e) => e.preventDefault(), { passive: false });
    calque.addEventListener("pointerdown", (e) => {
      e.preventDefault();
      if (!l || l.calque !== calque || l.phase !== "choix" || e.button !== 0) return;
      l.clavier = false;
      calque.focus({ preventScroll: true });
      const i = sous(e.clientX, e.clientY, false);
      if (i < 0) l.ancre = l.fin = null;
      else {
        l.ancre = l.fin = i;
        l.curseur = i;
        tire = true;
        try {
          calque.setPointerCapture(e.pointerId);
        } catch {
          // Sans capture, le tirer marche tant que la souris reste dans la fenêtre.
        }
      }
      peindre();
    });
    calque.addEventListener("pointermove", (e) => {
      if (!l || l.calque !== calque || l.phase !== "choix") return;
      const survol = sous(e.clientX, e.clientY, false);
      if (!tire) {
        if (survol !== l.survol) {
          l.survol = survol;
          peindre();
        }
        return;
      }
      const i = sous(e.clientX, e.clientY, true);
      if (i >= 0 && i !== l.fin) {
        l.fin = i;
        l.curseur = i;
      }
      l.survol = survol;
      peindre();
    });
    const lacher = () => (tire = false);
    calque.addEventListener("pointerup", lacher);
    calque.addEventListener("pointercancel", lacher);
    calque.addEventListener("lostpointercapture", lacher);
    calque.addEventListener("focus", peindre);
    calque.addEventListener("blur", peindre);
    // Tout le clavier, où que soit le focus : rien ne passe aux raccourcis de l'éditeur.
    const ecoute = (e) => {
      // Alt (Alt + F4, menus du système) : rien n'est annulé, comme dans l'éditeur.
      if (e.altKey) return;
      e.stopImmediatePropagation();
      if (!l || l.calque !== calque) return;
      const k = e.key;
      const ctl = e.ctrlKey || e.metaKey;
      const t = document.activeElement;
      const surBouton = !!l.barre && t !== calque && l.barre.contains(t);
      if (k === "Escape") {
        e.preventDefault();
        fermer();
        return;
      }
      if (k === "Tab") {
        // Le calque, puis les boutons de sa barre, en boucle.
        e.preventDefault();
        const ordre = [calque, ...(l.barre ? [...l.barre.querySelectorAll("button")] : [])];
        const places = ordre.filter((b) => !b.disabled);
        const i = places.indexOf(t);
        const pas = e.shiftKey ? places.length - 1 : 1;
        places[i < 0 ? 0 : (i + pas) % places.length].focus({ preventScroll: true });
        peindre();
        return;
      }
      if (l.phase !== "choix") {
        e.preventDefault();
        return;
      }
      if (ctl && k.toLowerCase() === "c") {
        e.preventDefault();
        copier(false);
        return;
      }
      // Entrée et Espace activent le bouton de la barre qui a le focus.
      if (surBouton && !ctl && (k === "Enter" || k === " ")) return;
      if (!surBouton && t !== calque) calque.focus({ preventScroll: true });
      const n = l.mots.length;
      l.clavier = true;
      if (ctl && k.toLowerCase() === "a") {
        l.ancre = 0;
        l.fin = n - 1;
      } else if (k.startsWith("Arrow")) {
        const cur = l.curseur;
        let nx = cur;
        if (k === "ArrowRight") nx = cur + 1;
        else if (k === "ArrowLeft") nx = cur - 1;
        else nx = ligneVoisine(cur, k === "ArrowDown" ? 1 : -1);
        nx = Math.max(0, Math.min(n - 1, nx));
        l.curseur = nx;
        if (e.shiftKey) {
          if (l.ancre === null) l.ancre = cur;
          l.fin = nx;
        } else {
          l.ancre = l.fin = null;
        }
      } else if (k === " ") {
        if (l.ancre === null) l.ancre = l.fin = l.curseur;
      } else if (k === "Enter") {
        e.preventDefault();
        copier(!choix());
        return;
      } else if (k === "Home" || k === "End") {
        const cur = l.curseur;
        l.curseur = k === "Home" ? 0 : n - 1;
        if (!e.shiftKey) l.ancre = l.fin = null;
        else {
          if (l.ancre === null) l.ancre = cur;
          l.fin = l.curseur;
        }
      }
      e.preventDefault();
      peindre();
    };
    window.addEventListener("keydown", ecoute, true);
    return ecoute;
  }

  return {
    poserBouton,
    synchroniser,
    lancer,
    placer,
    recu,
    copie,
    ouvert: () => !!l,
    // Toute ouverture d'un menu ou d'une bulle (menu « Fermer » demandé par le cœur compris)
    // ferme d'abord le calque, sans lui rendre le focus.
    fermer: () => fermer(false),
    remiseAZero: () => {
      premiers.clear();
      fermer(false);
    },
  };
}
