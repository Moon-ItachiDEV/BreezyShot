#!/usr/bin/env node
// Génère app/interface/commun/jetons.css à partir de design/jetons.json (copie du design
// system) et de design/jetons-complements.json (valeurs validées ailleurs, avec leur origine).
// Node seul, aucune dépendance. Usage : node outils/generer-jetons.mjs [--verifier]
// Avec --verifier, n'écrit rien et sort en erreur si jetons.css n'est pas à jour.

import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const racine = join(dirname(fileURLToPath(import.meta.url)), "..");
const SOURCE = join(racine, "design", "jetons.json");
const COMPLEMENTS = join(racine, "design", "jetons-complements.json");
const SORTIE = join(racine, "app", "interface", "commun", "jetons.css");

const lire = (chemin) => JSON.parse(readFileSync(chemin, "utf8"));

/** Fusionne les sections { tokens: [...] } : un jeton de même nom est remplacé. */
function fusionner(base, ajout) {
  const resultat = structuredClone(base);
  for (const [section, contenu] of Object.entries(ajout)) {
    if (!contenu || !Array.isArray(contenu.tokens)) continue;
    resultat[section] ??= { tokens: [] };
    resultat[section].tokens ??= [];
    for (const jeton of contenu.tokens) {
      const i = resultat[section].tokens.findIndex((j) => j.name === jeton.name);
      if (i >= 0) resultat[section].tokens[i] = jeton;
      else resultat[section].tokens.push(jeton);
    }
  }
  return resultat;
}

const nomValide = (nom) => {
  if (!/^[a-z0-9-]+$/.test(nom)) throw new Error(`nom de jeton invalide : ${nom}`);
  return nom;
};
const estTheme = (v) => v && typeof v === "object" && "light" in v && "dark" in v;

function generer(j) {
  const clair = [];
  const sombre = [];
  const fixes = [];

  // Couleurs puis ombres : valeurs par thème.
  for (const section of ["color", "shadow"]) {
    for (const t of j[section]?.tokens ?? []) {
      const nom = nomValide(t.name);
      if (estTheme(t.value)) {
        clair.push(`  --${nom}: ${t.value.light};`);
        if (t.value.dark !== t.value.light) sombre.push(`  --${nom}: ${t.value.dark};`);
      } else {
        clair.push(`  --${nom}: ${t.value};`);
      }
    }
  }

  // Typographie.
  for (const [famille, pile] of Object.entries(j.type?.families ?? {})) {
    fixes.push(`  --police-${nomValide(famille)}: ${pile};`);
  }
  for (const groupe of j.type?.groups ?? []) {
    for (const s of groupe.styles ?? []) {
      const nom = nomValide(s.name);
      fixes.push(`  --texte-${nom}-taille: ${s.fontSize};`);
      fixes.push(`  --texte-${nom}-interligne: ${s.lineHeight};`);
      fixes.push(`  --texte-${nom}-graisse: ${s.fontWeight};`);
      fixes.push(`  --texte-${nom}-police: var(--police-${nomValide(groupe.family ?? "ui")});`);
    }
  }

  // Tout le reste (espacements, rayons, durées, courbes, dimensions…) : valeurs fixes.
  const dejaFait = new Set(["name", "version", "color", "shadow", "type", "note"]);
  for (const [section, contenu] of Object.entries(j)) {
    if (dejaFait.has(section) || !Array.isArray(contenu?.tokens)) continue;
    for (const t of contenu.tokens) {
      const nom = nomValide(t.name);
      if (estTheme(t.value)) {
        clair.push(`  --${nom}: ${t.value.light};`);
        if (t.value.dark !== t.value.light) sombre.push(`  --${nom}: ${t.value.dark};`);
      } else {
        fixes.push(`  --${nom}: ${t.value};`);
      }
    }
  }

  const sombreBloc = sombre.join("\n");
  return `/* Fichier généré par outils/generer-jetons.mjs à partir de design/jetons.json et
   design/jetons-complements.json. Ne pas modifier à la main : relancer le générateur. */

:root {
${fixes.join("\n")}
${clair.join("\n")}
}

@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
${sombreBloc.replace(/^/gm, "  ")}
  }
}

:root[data-theme="dark"] {
${sombreBloc}
}
`;
}

const jetons = fusionner(lire(SOURCE), existsSync(COMPLEMENTS) ? lire(COMPLEMENTS) : {});
const css = generer(jetons);

if (process.argv.includes("--verifier")) {
  // Fins de ligne Windows (git autocrlf) : comparées comme des \n.
  const actuel = existsSync(SORTIE) ? readFileSync(SORTIE, "utf8").replace(/\r\n/g, "\n") : "";
  if (actuel !== css) {
    console.error("jetons.css n'est pas à jour : lancer node outils/generer-jetons.mjs");
    process.exit(1);
  }
  console.log("jetons.css est à jour");
} else {
  writeFileSync(SORTIE, css);
  console.log(`écrit : ${SORTIE}`);
}
