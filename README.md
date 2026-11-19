# BreezyShot

Outil de capture d'écran pour Windows 11 (macOS ensuite), dans l'esprit de CleanShot X.
Écrit avec Tauri 2 : un cœur en Rust, une interface web sans bundler.

État : version 0.8.5 — captures (zone, plein écran, fenêtre, dernière zone, retardateur),
barre de capture, vignette et glisser-déposer, éditeur d'annotation (avec fond, recadrage,
gomme au pixel, flèches courbées, flou/pixelisation/bloc noir, projecteur), réglages,
installateur ; fonctions Pro : historique, copie de texte (OCR et QR), capture défilante,
vidéo MP4 / GIF (micro, son système, clics visibles) et fichiers modifiables `.breezy`.

Licence : FSL-1.1-MIT, voir LICENSE.md.

## Où est quoi

- `app/coeur/` : la crate Rust `breezyshot` (dossier Tauri, `tauri.conf.json`).
  - `src/application/` : démarrage, table des états, journal, arguments, icône et menu,
    capture, vignette, barre, réglages, éditeur (`editeur.rs`, `geste_editeur.rs`,
    `fenetre_editeur.rs`), `.breezy`, historique, OCR, défilante, vidéo/GIF, veille.
  - `src/capture/`, `src/sortie/`, `src/reglages/`, `src/raccourcis/`, `src/fenetres/` :
    capture, encodage/export (PNG/JPG/WebP, presse-papiers, `.breezy`), réglages,
    raccourcis globaux et fenêtres.
  - `src/texte/`, `src/defilement/`, `src/film/`, `src/installation/`, `src/droits.rs`,
    `src/licence/` : logique pure des modules fonctionnels et de la licence hors ligne.
  - `src/systeme/` : tout ce qui dépend du système (`windows.rs`, `autre.rs`, sous-modules).
- `app/interface/` : les pages web sans bundler (`commun/`, `barre/`, `menu/`, `selection/`,
  `vignette/`, `editeur/`, `historique/`, `reglages/`, `defilante/`, `film/`, `installateur/`).
- `app/package.json` : fige la version de l'outil Tauri et de Playwright, rien d'autre.
- `design/` : jetons (`jetons.json`, `jetons-complements.json`) et icônes du design system.
- `outils/generer-jetons.mjs` : génère `app/interface/commun/jetons.css` à partir des jetons.
- `essais/` : scénarios PowerShell joués sur la machine Windows de GitHub ; `essais/pages/` :
  essais des pages et du modèle d'éditeur sous Node et Chromium (Playwright).
- `docs/ARCHITECTURE.md` : le contrat du code. `docs/TRAVAIL-VALIDE.md` : les étapes validées.
  `docs/ESSAI.md` : guide rapide d'essai.

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
for f in essais/pages/*.cjs; do NODE_PATH=app/node_modules node "$f"; done
```

`breezyshot.exe --quitter` ferme l'instance en cours. Le journal est dans
`%LOCALAPPDATA%\BreezyShot\Journal\breezyshot.log` ; avec la variable
`BREEZYSHOT_DOSSIER_ESSAI`, tout va dans ce dossier à la place.

GitHub Actions (`.github/workflows/app.yml`) refait tout cela sur Windows à chaque envoi,
plus les essais des pages sur Linux et une vérification sur macOS. Le dossier `prototypes/` de la branche
`prototypes/test-vitesse` contient des prototypes jetables qui ne font pas partie de l'app.
