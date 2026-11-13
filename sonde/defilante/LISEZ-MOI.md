# Sonde de la capture défilante (contrat 0.6, partie I) — provisoire, jamais fusionnée

Jouée par `.github/workflows/sonde-defilante.yml` sur `windows-latest`, dans l'ordre du §1.10 :
- `00-inventaire.ps1` : Windows, écran et échelle, carte graphique, navigateurs, WebView2, .NET, session, molette.
- `01-s9-s2-controle-vivant.ps1` : fenêtre de contrôle native et WebView2 (`rust/examples/controle.rs`) : molette à travers elle (S9 a, b), création pendant l'écran figé (S9 c), absente des images et visible sans l'exclusion (S2 a), BitBlt, DXGI et WGC (`vivant.rs`) : temps, ressources, GPU, pointeur, clignotement (S2 b à g).
- `02-s1-defiler.ps1` : moyens 1a à 5b sur chaque cible (dont l'app Paramètres, WinUI), 30 commandes, sentinelle, arrière-plan, survol coupé, Edge à la première demande (S1) ; une cible couverte par une autre fenêtre est « non mesurable » avec le nom de cette fenêtre.
- `03-s6-molette.ps1` (crochet et entrée brute d'un autre processus, `ecoute.cs`), `04-s4-pages.ps1` (page sous un clic), `05-s5-en-bas.ps1` (bas, page courte, lissage, chargement en défilant).
- `06-s3-suites.ps1` (suites PNG + `index.csv` avec le décalage vrai), `07-s7-memoire.ps1` (`memoire.rs` : PNG, copie, pic de mémoire, aperçu WebView2), `08-s8-echelles.ps1` (vraie échelle si Windows l'accepte, LayoutTransform 1,25 et 1,5, écrans), puis `09-remettre-echelle.ps1` (étape `if: always()`).
- Aides : `commun.ps1`, `natif.cs` (entrées, BitBlt, pixels), `temoin.ps1` (WPF, WinForms, molette seule, sentinelle, écran figé), `pages/longue.html`, `electron/` (version figée).

Lire : le journal du job suffit (chaque ligne y porte mesure, valeur, critère et verdict ; S3 y résume chaque suite) ; sinon le résumé du job, ou l'artefact `sonde-defilante-resultats` (`<question>.md`, PNG, `s3/<cas>/index.csv`). Chaque ligne : mesure | valeur | critère du contrat | verdict (« passe » seulement si l'effet est constaté sur les images ou par UI Automation ; « non mesurable : raison » sinon). Le décalage vrai se lit sur la bande de 8 px à gauche des témoins et de `longue.html` (couleur = numéro de ligne, `natif.cs` `CouleurLigne`).
