// L'âge d'une capture, tel que le dit la pastille de la case et son nom accessible (contrat
// 0.5, §2.2 et §10). Pur, sans DOM : la règle `ago` de design/historique/index.html,
// recopiée telle quelle (arrondis compris). Écart assumé à la règle 5 comme `document.js` de
// l'éditeur : un affichage qui dépend de l'heure qui passe, testé sous Node.

const MINUTE = 60000;

/** « à l'instant », « il y a <n> min », « il y a <n> h », « hier », « il y a <n> j ». */
export function age(horodatageMs, maintenantMs) {
  const m = Math.round((maintenantMs - horodatageMs) / MINUTE);
  if (m < 1) return "à l'instant";
  if (m < 60) return `il y a ${m} min`;
  const h = Math.round(m / 60);
  if (h < 24) return `il y a ${h} h`;
  const j = Math.round(h / 24);
  return j < 2 ? "hier" : `il y a ${j} j`;
}

/** Le nom accessible d'une case : « Capture, il y a 3 min ». */
export function nomCase(horodatageMs, maintenantMs) {
  return `Capture, ${age(horodatageMs, maintenantMs)}`;
}
