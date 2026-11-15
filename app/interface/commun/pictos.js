// Pictogrammes 20 × 20 du menu de l'icône (design/menu-icone), d'après l'identifiant d'une
// entrée du menu ; repris sur les lignes de Réglages › Raccourcis. Tirés de constantes,
// jamais de innerHTML. Le trait et la couleur viennent de la page (fill: none ;
// stroke: currentColor).

export const PICTOS = {
  capturer_zone: [
    "M3 7V4.5A1.5 1.5 0 0 1 4.5 3H7M13 3h2.5A1.5 1.5 0 0 1 17 4.5V7M17 13v2.5a1.5 1.5 0 0 1-1.5 1.5H13M7 17H4.5A1.5 1.5 0 0 1 3 15.5V13",
    "M10 7.5v5M7.5 10h5",
  ],
  capturer_plein_ecran: ["rect 2.5 3.5 15 10 1.5", "M7 16.5h6M10 13.5v3"],
  capturer_fenetre: ["rect 2.5 4 15 12 1.5", "M2.5 7.5h15"],
  capturer_defilante: [
    "rect 4 2.5 12 8.5 1.5",
    "M4 13.5v1.5a1.5 1.5 0 0 0 1.5 1.5h9a1.5 1.5 0 0 0 1.5-1.5v-1.5",
    "M10 5.5v6.5M7.8 9.8 10 12l2.2-2.2",
  ],
  capturer_derniere_zone: ["rect 2.5 3.5 11 9 1.5 pointille", "M17.5 8v5.5a2 2 0 0 1-2 2H9", "M11 13.5 9 15.5l2 2"],
  capturer_texte: [
    "M3 6.5v-2A1.5 1.5 0 0 1 4.5 3h2M13.5 3h2A1.5 1.5 0 0 1 17 4.5v2M17 13.5v2a1.5 1.5 0 0 1-1.5 1.5h-2M6.5 17h-2A1.5 1.5 0 0 1 3 15.5v-2",
    "M7 7h6M10 7v6.5",
  ],
  // Vidéo et GIF (0.7) : i-video et i-gif de design/capture.
  capturer_video: ["rect 2.5 5 10.5 10 2", "M13 9l4.5-2.5v7L13 11z"],
  capturer_gif: [
    "rect 2.5 4.5 15 11 2",
    "M8.2 8.3H6.6a.9.9 0 0 0-.9.9v1.6a.9.9 0 0 0 .9.9h1.6v-1.6H7.3M10.4 8.3v3.4M12.6 11.7V8.3h2M12.6 10h1.6",
  ],
  historique: ["M3.2 10a6.8 6.8 0 1 0 2-4.8", "M3.2 3v3.4h3.4", "M10 6.5V10l2.5 1.6"],
  // Icône « Curseurs » (choix du 13/11/2026) : jamais d'engrenage.
  reglages: ["M3 6.5h5.5M13.5 6.5H17M3 13.5h2.5M10.5 13.5H17", "circle 11 6.5 2.2", "circle 8 13.5 2.2"],
  quitter: ["M10 3v6.5", "M6.2 5.4a6 6 0 1 0 7.6 0"],
};

const SVG = "http://www.w3.org/2000/svg";
export function picto(id) {
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("viewBox", "0 0 20 20");
  svg.setAttribute("aria-hidden", "true");
  for (const forme of PICTOS[id] || []) {
    const [genre, ...v] = forme.split(" ");
    let el;
    if (genre === "rect") {
      el = document.createElementNS(SVG, "rect");
      const [x, y, l, h, r, style] = v;
      el.setAttribute("x", x);
      el.setAttribute("y", y);
      el.setAttribute("width", l);
      el.setAttribute("height", h);
      el.setAttribute("rx", r);
      if (style === "pointille") el.setAttribute("stroke-dasharray", "2 1.8");
    } else if (genre === "circle") {
      el = document.createElementNS(SVG, "circle");
      el.setAttribute("cx", v[0]);
      el.setAttribute("cy", v[1]);
      el.setAttribute("r", v[2]);
    } else {
      el = document.createElementNS(SVG, "path");
      el.setAttribute("d", forme);
      // Le bas de la capture défilante : en pointillé.
      if (id === "capturer_defilante" && forme.startsWith("M4 13.5")) el.setAttribute("stroke-dasharray", "2 2");
    }
    svg.append(el);
  }
  return svg;
}
