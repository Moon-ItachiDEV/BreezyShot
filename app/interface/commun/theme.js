// Thème des pages (étape 10). Le cœur décide ; la page ne fait que poser l'attribut que
// lisent les jetons (`commun/jetons.css` : `:root[data-theme="light"|"dark"]`).
//
// Contrat : le cœur envoie `interface:theme` { theme: "light" | "dark" | null } à toutes
// les pages quand le réglage change, et à une page qui vient de dire qu'elle est prête.
// `null` (ou toute autre valeur) : l'attribut est retiré, la page suit le système.
//
// Utilisation : `import { suivreTheme } from "../commun/theme.js";` puis attendre
// `suivreTheme()` avant d'annoncer que la page est prête.

export function poserTheme(theme) {
  const racine = document.documentElement;
  if (theme === "light" || theme === "dark") racine.dataset.theme = theme;
  else delete racine.dataset.theme;
}

export function suivreTheme() {
  const ecoute = window.__TAURI__?.event?.listen;
  if (!ecoute) return Promise.resolve();
  return Promise.resolve(ecoute("interface:theme", (e) => poserTheme(e?.payload?.theme))).catch((e) =>
    console.warn("theme : écoute impossible :", e),
  );
}
