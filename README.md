# BreezyShot

Outil de capture d'écran pour Windows 11 (macOS ensuite), dans l'esprit de CleanShot X.
Écrit avec Tauri 2 : un cœur en Rust, une interface web sans bundler.

État : MVP, étape 1 « Socle » (icône près de l'horloge, instance unique, journal).

Licence : FSL-1.1-MIT, voir LICENSE.md.

## Où est quoi

- `app/coeur/` : la crate Rust `breezyshot` (dossier Tauri, `tauri.conf.json`).
  - `src/application/` : démarrage, table des états, journal, arguments, icône et menu.
  - `src/reglages/`, `src/raccourcis/`, `src/fenetres/` : réglages, raccourcis globaux, bandeau.
  - `src/systeme/` : tout ce qui dépend du système (`windows.rs`, `autre.rs`).
- `app/interface/` : les pages web (`vignette/bandeau.*`, `commun/jetons.css`).
- `app/package.json` : fige la version de l'outil Tauri et de Playwright, rien d'autre.
- `design/` : jetons (`jetons.json`, `jetons-complements.json`) et icônes du design system.
- `outils/generer-jetons.mjs` : génère `app/interface/commun/jetons.css` à partir des jetons.
- `essais/` : scénarios PowerShell joués sur la machine Windows de GitHub ; `essais/pages/` :
  essai de la page du bandeau dans Chromium.
- `docs/ARCHITECTURE.md` : le contrat du code. `docs/TRAVAIL-VALIDE.md` : les étapes validées.

## Construire et tester

```
cd app/coeur
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test

cd ..
npm ci
npx tauri build --no-bundle      # donne app/coeur/target/release/breezyshot.exe

cd ..
pwsh essais/lancer.ps1 -Exe app/coeur/target/release/breezyshot.exe

node outils/generer-jetons.mjs              # après un changement de design/jetons*.json
node outils/generer-jetons.mjs --verifier   # échoue si jetons.css n'est pas à jour
cd app && npx playwright install chromium && cd ..
NODE_PATH=app/node_modules node essais/pages/bandeau.cjs
```

`breezyshot.exe --quitter` ferme l'instance en cours. Le journal est dans
`%LOCALAPPDATA%\BreezyShot\Journal\breezyshot.log` ; avec la variable
`BREEZYSHOT_DOSSIER_ESSAI`, tout va dans ce dossier à la place.

GitHub Actions (`.github/workflows/app.yml`) refait tout cela sur Windows à chaque envoi,
plus l'essai de la page sur Linux et une vérification sur macOS. Le dossier `prototypes/` de la branche
`prototypes/test-vitesse` contient des prototypes jetables qui ne font pas partie de l'app.
