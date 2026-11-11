# BreezyShot

Outil de capture d'écran pour Windows 11 (macOS ensuite), dans l'esprit de CleanShot X.
Écrit avec Tauri 2 : un cœur en Rust, une interface web sans bundler.

État : MVP, étape 1 « Socle » (icône près de l'horloge, instance unique, journal).

Licence : FSL-1.1-MIT, voir LICENSE.md.

## Où est quoi

- `app/coeur/` : la crate Rust `breezyshot` (dossier Tauri, `tauri.conf.json`).
  - `src/application/` : démarrage, table des états, journal, arguments, icône et menu.
  - `src/systeme/` : tout ce qui dépend du système (`windows.rs`, `autre.rs`).
- `app/interface/` : les pages web (vides à l'étape 1).
- `app/package.json` : fige la version de l'outil Tauri, rien d'autre.
- `design/icones/` : icônes Moulinet copiées du design system.
- `essais/` : scénarios PowerShell joués sur la machine Windows de GitHub.
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
```

`breezyshot.exe --quitter` ferme l'instance en cours. Le journal est dans
`%LOCALAPPDATA%\BreezyShot\Journal\breezyshot.log` ; avec la variable
`BREEZYSHOT_DOSSIER_ESSAI`, tout va dans ce dossier à la place.

GitHub Actions (`.github/workflows/app.yml`) refait tout cela sur Windows à chaque envoi,
plus une vérification sur macOS. Le dossier `prototypes/` de la branche
`prototypes/test-vitesse` contient des prototypes jetables qui ne font pas partie de l'app.
