# Version 0.7 « Vidéo et GIF » (contrat 0.7, §8, §9 et §15.3) : la vidéo et le GIF de bout
# en bout sur le vrai .exe, avec de vraies entrées clavier et souris, sur un témoin animé
# (temoin-anime.ps1, dessin de film-natif.cs) dont chaque image porte son compteur k, donc
# son heure (le témoin écrit « k <n> <ms> » à chaque pas, sur l'horloge de performance) :
# un film se relit image par image (MP4 : l'exemple lire_film ; GIF : System.Drawing) et se
# compare au témoin (durée, ordre et heure des images, couleurs, bords), le journal au §8,
# les temps et la mémoire au §9.
# Chaque attente lit une ligne nouvelle du journal (ou guette une fenêtre, un élément UI
# Automation, le fichier du témoin) avec une borne, et dit ce qu'elle attendait. Garde des
# essais 12 à 14 : un appui de souris ne part que si le point visé est sur une fenêtre de
# l'app ; une touche n'est qu'un raccourci global que l'app tient (Ctrl+Maj+6 et 7 une fois
# `raccourci video|gif: …` au journal, Échap seulement après `echap global reserve` sans
# libération depuis). Un point qui échoue ramène l'app au repos (Échap, le raccourci du film
# qui l'arrête, sinon l'app relancée) : les suivants restent valables.
#
# L0, sans licence (réglages v5) : `licence absente` ; aucune ligne `raccourci video`,
#     `raccourci gif`, `video disponible` ni `film:` ; menu de 9 positions sans « Vidéo » ni
#     « GIF » ; Ctrl+Maj+6 et 7 libres (tenus par tenir-raccourci.ps1) ; `--capturer video`
#     et `gif` → `--capturer: mode non construit` ; réglages passés en v6 (video
#     Ctrl+Maj+6, gif Ctrl+Maj+7) ; aucune fenêtre `film` ; modules 10 s après le
#     démarrage : rien de plus que la référence de la 0.6.1 (docs/sonde-film-modules-0.6.1.txt,
#     « sans licence »), aucun des huit surveillés ; mémoire (app et WebView2) au plus celle
#     de l'essai 13 sans licence + 8 Mo (l'essai 14 mesure après le menu : pas comparable).
# L1, licence (cas A) : `raccourci video: Ctrl+Maj+6`, `raccourci gif: Ctrl+Maj+7`, `video
#     disponible: oui` ; menu : « Vidéo », « GIF » dans leur groupe après « Copier le
#     texte » ; Réglages › Raccourcis dans le même ordre ; aucune fenêtre `film` au repos ;
#     modules 10 s après `video disponible` : rien hors de la référence « avec licence » et
#     des onze de F32 ; mémoire au repos ≤ L0 + 3 Mo (processus de l'app).
# Cas A (vidéo, témoin simple en 40,40 ; une sentinelle -Fixe en 660,-150 ; copie,
#     enregistrement, vignette « toujours ») : A1 Ctrl+Maj+6 et la visée ; A2 tracé de la
#     zone du témoin, styles de la fenêtre de contrôle, écran figé caché ≤ 450 ms, pilule
#     lue pendant le décompte (« Pause » inactif, « Arrêter », pas de « Micro ») ; A3
#     `film debut`, duplication, codeur logiciel, pas de son, Échap libéré avant, « Pause »
#     actif, la durée passe par 0:01 et 0:02, `pret` du premier film vidéo noté (≤ 3 000 ms) ;
#     A4 la souris 1 s sur la pastille jaune ; A5 « Arrêter » après 6 s, Finalisation,
#     vignette (« Enregistrement terminé », durée, boutons) ; A6 le MP4 relu (H.264, 600 ×
#     450, sans son, durée à 200 ms, ≥ 28 images par seconde, k sans recul, premier k après
#     le décompte, dernier à l'arrêt à 300 ms, pastilles ± 8, bords blancs, pointeur pendant
#     sa seconde et plus après) ; A7 fraîcheur sur le témoin -Rapide (10 s) ; A8
#     presse-papiers (le .mp4 rangé dans le dossier d'enregistrement lui-même, aucun lien
#     dans `Glisser`), « Récents » inchangé, `Films` vide
#     (choix U4 : le film est déplacé dans le dossier d'enregistrement) ; copie d'écran sans
#     cadre ni voile ni pilule (non capturable). Budgets du §9 sur les lignes `film debut` et
#     `chrono film`.
# Cas A9 (capturable) : cadre blanc hors de la zone, voile à ± 6 du jeton sur la sentinelle,
#     pilule peinte, la zone elle-même claire.
# Cas B (GIF) : Ctrl+Maj+7 (6 s), puis par le menu ; relu (600 × 450, boucle infinie, délais
#     = durée à 100 ms, k sans recul, pastilles exactes, bords) ; « GIF · 0:06 » ; un .gif
#     au presse-papiers ; témoin -Fixe 3 s : une ou deux images, 3 s, moins de 100 Ko.
# Cas C (pause, vidéo puis GIF) ; D (annulations : Échap, croix, Arrêter, session pendant le
#     décompte, Échap pendant la visée) ; E (gardes : Ctrl+Maj+2 et 7 ignorés pendant une
#     vidéo, menu « Quitter » seul, Ctrl+Maj+6 l'arrête ; Ctrl+Maj+6 ignoré pendant un GIF,
#     Ctrl+Maj+7 l'arrête ; --quitter pendant un film : `film arret: fichier range`, le
#     film dans le dossier d'enregistrement au nom du modèle, relu entier) ; F (80 × 60 agrandie à 120 × 120,
#     601 × 451 : vidéo 600 × 450, GIF 601 × 451 ; GIF laissé à la limite de 60 s, sur le
#     témoin simple, avec I) ; F125 (échelle 125 %, capturable, « non joué » si un témoin
#     lancé après le changement ne le suit pas) ; G (sans vignette, annonces) ; H (copie éteinte : Copier, Supprimer puis
#     Annuler, Supprimer puis le délai) ; I (vidéo de 30 s, GIF jusqu'à la limite : processeur
#     et mémoire ; 10 s après la vignette ; cinq films de suite) ; J (sans Media Foundation) ;
#     K (session, veille, écrans 800 × 600, disque virtuel de 300 Mo plein) ; L (app tuée
#     pendant une vidéo, relancée propre, le film coupé gardé dans `Films`).
# Fin de chaque cas : aucune ligne `fin deja rendue`, `fil arrete (panique)`, `ecran fige
# encore montre`, `source precedente non rendue`, `fil sans reponse a la fermeture` ;
# aucune ligne ERROR d'un film.
#
# Non vérifiable sur GitHub (un écran 1024 × 768 à 100 %, sans carte graphique, sans
# micro) : la liste de fin du §15.3 du contrat (micro, codeur matériel, duplication sur une
# vraie carte, 2K, 150 %, plusieurs écrans, vraie veille, UAC, arrêt de Windows, OneDrive,
# collage dans d'autres applis, lecteurs). Hors Windows : démarrage, `licence absente`,
# `--quitter`.

param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$Sortie
)

. "$PSScriptRoot/commun.ps1"
Initialiser-Essai -Nom '15-video-gif' -Exe $Exe -Sortie $Sortie | Out-Null

$script:Auxiliaires = [System.Collections.Generic.List[object]]::new()
$script:Pwsh = (Get-Process -Id $PID).Path
$script:proc = $null
$script:Cas = $null
# La licence d'essai, telle que rangée par le chantier L (une ligne, blancs retirés).
$LICENCE = (Get-Content -LiteralPath (Join-Path $PSScriptRoot 'licence-essai.txt') -Raw).Trim()
$CULTURE = [System.Globalization.CultureInfo]::InvariantCulture
$INTERDITES = 'film \d+: fin deja rendue|film: fil arrete \(panique\)|film: ecran fige encore montre|source precedente non rendue|film-sortie: fil interrompu|film: fil sans reponse a la fermeture'

# --- Aides du journal (même forme que les essais 13 et 14) --------------------------------

function Dossier-Cas([string]$Nom) {
    $d = Join-Path $script:Essai.Dossier $Nom
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    return $d
}
function Lignes-Journal([string]$Dossier) {
    return @((Lire-Journal -Dossier $Dossier) -split "`n" | ForEach-Object { $_.TrimEnd("`r") } | Where-Object { $_ -ne '' })
}
function Derniere-Ligne([string]$Motif, [string]$Dossier) {
    $trouvee = $null
    foreach ($l in (Lignes-Journal $Dossier)) { if ($l -match $Motif) { $trouvee = $l } }
    return $trouvee
}
function Compter-Motif([string]$Motif, [string]$Dossier) {
    $n = 0
    foreach ($l in (Lignes-Journal $Dossier)) { if ($l -match $Motif) { $n++ } }
    return $n
}
# Le message d'une ligne, sans horodatage, niveau ni module.
function Message([string]$Ligne) {
    if (-not $Ligne) { return '<aucune ligne>' }
    return ($Ligne -replace '^\S+\s+\S+\s+\S+\s+', '')
}
function Diagnostic-Journal([string]$Dossier) {
    $l = @(Lignes-Journal $Dossier | Where-Object { $_ -match 'licence|film|video|gif|selection|raccourci|menu|etat |echap|sortie|vignette|capture|bandeau|WARN|ERROR' } | Select-Object -Last 40 | ForEach-Object { Message $_ })
    if ($l.Count -eq 0) { return '(journal : aucune ligne utile)' }
    return 'journal : ' + ($l -join ' | ')
}
function Attendre-Motif([string]$Motif, [string]$Dossier, [double]$DelaiS, [string]$Quoi, [int]$Fois = 1) {
    $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
    do {
        if ((Compter-Motif $Motif $Dossier) -ge $Fois) { return (Derniere-Ligne $Motif $Dossier) }
        Start-Sleep -Milliseconds 40
    } while ([DateTime]::UtcNow -lt $limite)
    throw "$Quoi : rien dans le journal en $DelaiS s (motif « $Motif », attendu $Fois fois, vu $(Compter-Motif $Motif $Dossier)) ; $(Diagnostic-Journal $Dossier)"
}
# L'heure d'une ligne du journal (premier mot, `2026-11-11T18:40:12.345+01:00`).
function Heure-Ligne([string]$Ligne) {
    return [DateTimeOffset]::Parse(($Ligne -split '\s+')[0], $CULTURE)
}
# Le nombre de lignes du journal : les attentes d'un geste lisent les lignes venues après.
function Base-Journal([string]$Dossier) { return @(Lignes-Journal $Dossier).Count }
function Lignes-Apres([string]$Dossier, [int]$Base) {
    $t = @(Lignes-Journal $Dossier)
    if ($t.Count -lt $Base) { $Base = 0 }   # journal tourné (1 Mo) : tout le fichier neuf
    if ($t.Count -le $Base) { return @() }
    return @($t[$Base..($t.Count - 1)])
}
# La $Fois-ième ligne du motif après $Base : { Ligne, Index (rang après $Base) } ; $null.
function Trouver-Apres([string]$Motif, [string]$Dossier, [int]$Base, [int]$Fois = 1) {
    $t = @(Lignes-Apres $Dossier $Base)
    $n = 0
    for ($i = 0; $i -lt $t.Count; $i++) {
        if ($t[$i] -match $Motif) { $n++; if ($n -ge $Fois) { return [pscustomobject]@{ Ligne = $t[$i]; Index = $i } } }
    }
    return $null
}
function Toutes-Apres([string]$Motif, [string]$Dossier, [int]$Base) {
    return @(Lignes-Apres $Dossier $Base | Where-Object { $_ -match $Motif })
}
function Attendre-Apres([string]$Motif, [string]$Dossier, [int]$Base, [double]$DelaiS, [string]$Quoi, [int]$Fois = 1) {
    $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
    do {
        $r = Trouver-Apres $Motif $Dossier $Base $Fois
        if ($null -ne $r) { return $r }
        Start-Sleep -Milliseconds 40
    } while ([DateTime]::UtcNow -lt $limite)
    throw "$Quoi : rien de nouveau dans le journal en $DelaiS s (motif « $Motif ») ; $(Diagnostic-Journal $Dossier)"
}
function Nombre-Texte([string]$Texte) {
    $d = 0.0
    if ([double]::TryParse($Texte, [System.Globalization.NumberStyles]::Float, $CULTURE, [ref]$d)) { return $d }
    return [double]::NaN
}

# Réglages en version 6 (complets : rien à réécrire) ou 5 (sans `video` ni `gif` : l'app
# les passe en version 6). Vignette « toujours », en bas à droite, animations réduites.
function Ecrire-Reglages([string]$Dossier, [int]$Version = 6, [bool]$Copier = $true, [bool]$Vignette = $true) {
    $racc = [ordered]@{ zone = @(); plein_ecran = @('Ctrl+Maj+2'); fenetre = @(); derniere_zone = @(); historique = @(); texte = @(); defilante = @() }
    if ($Version -ge 6) { $racc['video'] = @('Ctrl+Maj+6'); $racc['gif'] = @('Ctrl+Maj+7') }
    $r = [ordered]@{
        version                   = $Version
        raccourcis                = $racc
        copier_apres_capture      = $Copier
        enregistrer_apres_capture = $true
        dossier_enregistrement    = (Join-Path $Dossier 'images')
        vignette                  = [ordered]@{ afficher = $Vignette; duree_s = 0; coin = 'bas_droite' }
        animations                = 'reduites'
        selection                 = [ordered]@{ loupe = $false }
        capture                   = [ordered]@{ retardateur_s = 0; pointeur = $false }
        demarrer_avec_systeme     = $false
        modele_nom                = 'BreezyShot {date} à {heure}'
        theme                     = 'systeme'
    }
    $d = Join-Path $Dossier 'reglages'
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $Dossier 'images') | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $d 'reglages.json'), ($r | ConvertTo-Json -Depth 5), [System.Text.UTF8Encoding]::new($false))
}
function Lire-Reglages([string]$Dossier) {
    return (Get-Content -LiteralPath (Join-Path $Dossier 'reglages/reglages.json') -Raw -Encoding utf8 | ConvertFrom-Json)
}
function Poser-Licence([string]$Dossier) {
    New-Item -ItemType Directory -Force -Path (Join-Path $Dossier 'reglages') | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $Dossier 'reglages/licence.txt'), $LICENCE, [System.Text.UTF8Encoding]::new($false))
}

function Demarrer-Cas([string]$Dossier, [string]$Quoi, [switch]$Capturable, [switch]$SansCodeur, [switch]$AvecLicence) {
    if ($Capturable) { $env:BREEZYSHOT_ESSAI_CAPTURABLE = '1' }
    if ($SansCodeur) { $env:BREEZYSHOT_ESSAI_SANS_CODEUR = '1' }
    $prets = Compter-Motif 'application +pret$' $Dossier
    try { $p = Demarrer-App -Dossier $Dossier }
    finally {
        Remove-Item Env:BREEZYSHOT_ESSAI_CAPTURABLE -ErrorAction SilentlyContinue
        Remove-Item Env:BREEZYSHOT_ESSAI_SANS_CODEUR -ErrorAction SilentlyContinue
    }
    if ($IsWindows) { [BreezyShotFilm.Natif]::PidApp = $p.Id }
    $null = Attendre-Motif 'application +pret$' $Dossier 30 "$Quoi : démarrage" ($prets + 1)
    if ($AvecLicence) { $null = Attendre-Motif 'licence active ' $Dossier 10 "$Quoi : licence active" }
    return $p
}
function Fermer-Cas($Processus, [string]$Dossier, [string]$Quoi) {
    $arrets = Compter-Motif 'arret propre$' $Dossier
    $q = Quitter-App -DelaiS 15 -Dossier $Dossier
    if (-not $q.Fini) { throw "$Quoi : --quitter n'a pas répondu en 15 s" }
    if (-not $Processus.WaitForExit(15000)) { throw "$Quoi : l'app ne s'est pas fermée en 15 s" }
    $null = Attendre-Motif 'arret propre$' $Dossier 3 "$Quoi : arrêt" ($arrets + 1)
}

# Un point de l'essai dans son propre try : un échec est noté, l'app revient au repos, et
# l'essai continue.
function Point([string]$Nom, [scriptblock]$Corps) {
    try { & $Corps }
    catch {
        Verifier $Nom $false "$_" | Out-Null
        if ($IsWindows) { Revenir-Au-Repos $Nom }
    }
}

# Fin d'un cas : aucune ligne qui ne doit jamais paraître (§8), aucune ERROR d'un film.
function Verifier-Interdites([string]$Dossier, [string]$Quoi) {
    $l = @(Lignes-Journal $Dossier | Where-Object { $_ -match $INTERDITES } | ForEach-Object { Message $_ })
    Verifier "$Quoi aucune ligne « fin deja rendue », « fil arrete (panique) », « ecran fige encore montre », « source precedente non rendue », « fil sans reponse a la fermeture »" ($l.Count -eq 0) ($l -join ' | ') | Out-Null
    $e = @(Lignes-Journal $Dossier | Where-Object { $_ -match '^\S+\s+ERROR\s' -and $_ -match 'film' } | ForEach-Object { Message $_ })
    Verifier "$Quoi aucune ligne ERROR d'un film" ($e.Count -eq 0) ($e -join ' | ') | Out-Null
    $w = @(Lignes-Journal $Dossier | Where-Object { $_ -match '^\S+\s+WARN\s+\S+\s+(film|--capturer)' } | ForEach-Object { Message $_ } | Select-Object -Unique)
    Noter-Mesure -Nom "$Quoi lignes WARN des films (notées)" -Valeur $(if ($w.Count) { $w -join ' | ' } else { 'aucune' })
}

try {
    if (-not $IsWindows) {
        # --- Hors Windows : démarrage seulement -------------------------------------------
        $d = Dossier-Cas 'linux'
        Ecrire-Reglages $d 6
        $script:proc = Demarrer-Cas $d 'hors Windows'
        Verifier 'démarrage, licence absente' ((Compter-Motif 'licence absente' $d) -ge 1) | Out-Null
        Fermer-Cas $script:proc $d 'hors Windows'
        Verifier '--quitter → arret propre' $true | Out-Null
        Write-Host '[saute] le reste : sauté hors Windows'
    }
    else {
        Add-Type -AssemblyName UIAutomationClient
        Add-Type -AssemblyName UIAutomationTypes
        Add-Type -AssemblyName System.Drawing
        Add-Type -AssemblyName System.Windows.Forms
        Add-Type -TypeDefinition (Get-Content -LiteralPath (Join-Path $PSScriptRoot 'film-natif.cs') -Raw)
        # Noms longs : PowerShell confond $N et $n, $T et $t.
        $script:Natif = [BreezyShotFilm.Natif]
        $script:Temoin = [BreezyShotFilm.Temoin]
        try { [void]$script:Natif::SetProcessDpiAwarenessContext([IntPtr]::new(-4)) } catch { }
        $AE = [System.Windows.Automation.AutomationElement]
        $CT = [System.Windows.Automation.ControlType]

        $MODS = [byte[]]@(0x11, 0x10)          # Ctrl, Maj
        $AUCUN = [byte[]]@()
        $VK_2 = [byte]0x32; $VK_6 = [byte]0x36; $VK_7 = [byte]0x37; $ECHAP = [byte]0x1B
        $MOD_CTRL_MAJ = 6                       # RegisterHotKey : MOD_CONTROL | MOD_SHIFT
        $WS_EX = @{ TOPMOST = 0x8; TRANSPARENT = 0x20; TOOLWINDOW = 0x80; APPWINDOW = 0x40000; NOACTIVATE = 0x08000000 }
        $PASTILLES = @(@(200, 60, 60), @(60, 170, 80), @(60, 90, 200), @(230, 200, 60))
        $utile = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
        $ecran = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
        # Le témoin en 40,40 (zone 40..639 × 40..489) ; la sentinelle -Fixe en 660,-150 (vue de
        # 660..1023 × 0..299) ; la pilule sous la zone, à droite ; la vignette en bas à droite.
        $TX = 40; $TY = 40
        $SX = 660; $SY = -150
        # Souris au repos : à droite, sous la sentinelle, loin de la zone et de la pilule.
        $REPOS_X = [int]($utile.Right - 12)
        $REPOS_Y = [int]($utile.Top + $utile.Height / 2 + 20)
        # Le pointeur de A4 : sur la pastille jaune (230..277 × 390..437 du témoin), loin de
        # son centre ; ses quatre points de lecture sont dans le blanc de la flèche.
        $POINTEUR = @(232, 392)
        $POINTS_POINTEUR = @(233, 396, 234, 399, 235, 402, 233, 402)
        # Les bords exacts de l'image (rangées et colonnes 0 et dernière, au milieu).
        $POINTS_BORDS = @(0, 225, 599, 225, 300, 0, 300, 449)
        Write-Host "écran $ecran ; zone utile $utile ; mode $($script:Natif::ModeCourant()) ; DPI $($script:Natif::DpiEcran(1, 1))"

        # Le lecteur MP4 (chantier B) : BREEZYSHOT_LIRE_FILM (la CI), sinon à côté du .exe.
        $script:LireFilm = $env:BREEZYSHOT_LIRE_FILM
        if (-not $script:LireFilm -or -not (Test-Path -LiteralPath $script:LireFilm)) {
            $cibles = Split-Path -Parent (Split-Path -Parent $script:Essai.Exe)
            foreach ($c in @((Join-Path $cibles 'debug/examples/lire_film.exe'), (Join-Path $cibles 'release/examples/lire_film.exe'))) {
                if (Test-Path -LiteralPath $c) { $script:LireFilm = $c; break }
            }
        }
        Write-Host "lecteur MP4 : $(if ($script:LireFilm) { $script:LireFilm } else { 'introuvable' })"

        # --- Rectangles, images, horloge ----------------------------------------------------
        # « … <x>,<y> <l>x<h> … » → { X, Y, L, H, CX, CY }.
        function Lire-Rect([string]$Texte) {
            if ($Texte -notmatch '(-?\d+),(-?\d+) (\d+)x(\d+)') { throw "rectangle illisible : $Texte" }
            $r = [pscustomobject]@{ X = [int]$Matches[1]; Y = [int]$Matches[2]; L = [int]$Matches[3]; H = [int]$Matches[4] }
            $r | Add-Member CX ([int]($r.X + [math]::Floor($r.L / 2)))
            $r | Add-Member CY ([int]($r.Y + [math]::Floor($r.H / 2)))
            return $r
        }
        function Hwnd-Journal([string]$Motif, [string]$Dossier, [int]$Fois = 1, [double]$DelaiS = 10) {
            $l = Attendre-Motif "$Motif (\d+)$" $Dossier $DelaiS $Motif $Fois
            $null = $l -match "$Motif (\d+)$"
            return [IntPtr][long]$Matches[1]
        }
        function Octets-Image($Bitmap) {
            $r = [System.Drawing.Rectangle]::new(0, 0, $Bitmap.Width, $Bitmap.Height)
            $d = $Bitmap.LockBits($r, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
            try {
                $o = [byte[]]::new($d.Stride * $Bitmap.Height)
                [System.Runtime.InteropServices.Marshal]::Copy($d.Scan0, $o, 0, $o.Length)
            }
            finally { $Bitmap.UnlockBits($d) }
            return [pscustomobject]@{ L = $Bitmap.Width; H = $Bitmap.Height; O = $o }
        }
        function Copie-Ecran([int]$Gx, [int]$Gy, [int]$Larg, [int]$Haute) {
            $b = [System.Drawing.Bitmap]::new($Larg, $Haute, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
            try {
                $g = [System.Drawing.Graphics]::FromImage($b)
                try { $g.CopyFromScreen($Gx, $Gy, 0, 0, [System.Drawing.Size]::new($Larg, $Haute)) } finally { $g.Dispose() }
                return Octets-Image $b
            }
            finally { $b.Dispose() }
        }
        # Le pixel (x, y) d'une copie : { R, V, B }.
        function Pixel-Copie($Img, [int]$X, [int]$Y) {
            $i = ($Y * $Img.L + $X) * 4
            return @([int]$Img.O[$i + 2], [int]$Img.O[$i + 1], [int]$Img.O[$i])
        }
        $script:TicsMs = [double][System.Diagnostics.Stopwatch]::Frequency / 1000.0
        function Qpc-Maintenant { return [System.Diagnostics.Stopwatch]::GetTimestamp() / $script:TicsMs }
        # L'heure d'une ligne du journal sur l'horloge de performance (celle du témoin).
        function Qpc-Ligne([string]$Ligne) {
            $q = Qpc-Maintenant
            $u = [DateTimeOffset]::Now.ToUnixTimeMilliseconds()
            return [double]((Heure-Ligne $Ligne).ToUnixTimeMilliseconds() + ($q - $u))
        }
        function Nouveau-Nom { return [guid]::NewGuid().ToString('N').Substring(0, 8) }
        function Fichiers-De([string]$Dossier, [string]$Filtre = '*') {
            if (-not (Test-Path -LiteralPath $Dossier)) { return @() }
            return @(Get-ChildItem -LiteralPath $Dossier -File -Filter $Filtre -ErrorAction SilentlyContinue | Sort-Object LastWriteTimeUtc | ForEach-Object { $_.FullName })
        }
        function Empreinte([string]$Chemin) { return (Get-FileHash -LiteralPath $Chemin -Algorithm SHA256).Hash }

        # --- Témoins ---------------------------------------------------------------------------
        # Lance temoin-anime.ps1 (son propre processus) et attend son état : { Nom, Aide, Hwnd,
        # Pid, X, Y, L, H, Echelle, Journal, Genre }.
        function Lancer-Temoin([string]$Nom, [string]$Genre = 'simple', [int]$X = $TX, [int]$Y = $TY, [switch]$Rapide) {
            $f = Join-Path $script:Essai.Dossier "temoin-$Nom-$(Nouveau-Nom).txt"
            $a = @('-NoProfile', '-File', "`"$(Join-Path $PSScriptRoot 'temoin-anime.ps1')`"", '-Genre', $Genre, '-X', "$X", '-Y', "$Y", '-Fichier', "`"$f`"")
            if ($Rapide) { $a += '-Rapide' }
            $aide = Start-Process -FilePath $script:Pwsh -PassThru -NoNewWindow -ArgumentList $a
            $script:Auxiliaires.Add($aide)
            $limite = [DateTime]::UtcNow.AddSeconds(40)
            $etat = @{}
            do {
                if (Test-Path -LiteralPath $f) {
                    $etat = @{}
                    try { foreach ($l in [System.IO.File]::ReadAllLines($f)) { $p = $l -split ' ', 2; if ($p.Count -eq 2) { $etat[$p[0]] = $p[1] } } } catch { }
                    if ($etat.ContainsKey('rapide')) { break }
                }
                if ($aide.HasExited) { throw "témoin $Nom : sorti avant d'écrire son état" }
                Start-Sleep -Milliseconds 100
            } while ([DateTime]::UtcNow -lt $limite)
            if (-not $etat.ContainsKey('rapide')) { throw "témoin $Nom : aucun état écrit en 40 s" }
            $z = @("$($etat['zone'])" -split ',' | ForEach-Object { [int]$_ })
            $t = [pscustomobject]@{ Nom = $Nom; Aide = $aide; Hwnd = [IntPtr][long]$etat['hwnd']; Pid = [int]$etat['pid']; X = $z[0]; Y = $z[1]; L = $z[2]; H = $z[3]
                Echelle = (Nombre-Texte $etat['echelle']); Journal = $etat['journal']; Genre = $Genre; Rapide = [bool]$Rapide }
            Start-Sleep -Milliseconds 500
            Write-Host "témoin $Nom ($Genre$(if ($Rapide) { ', rapide' })) : zone $($t.X),$($t.Y) $($t.L)x$($t.H), échelle $($t.Echelle), fenêtre $($script:Natif::Rect($t.Hwnd) -join ',')"
            return $t
        }
        function Arreter-Temoin($Tem) {
            if ($null -eq $Tem) { return }
            try { if (-not $Tem.Aide.HasExited) { $Tem.Aide.Kill(); $null = $Tem.Aide.WaitForExit(5000) } } catch { }
        }
        # Les pas du témoin (« k <n> <ms> ») : { K, Ms }.
        function Pas-Temoin($Tem) {
            $ks = [System.Collections.Generic.List[int]]::new()
            $ms = [System.Collections.Generic.List[double]]::new()
            try {
                $fs = [System.IO.FileStream]::new($Tem.Journal, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite -bor [System.IO.FileShare]::Delete)
                try { $texte = [System.IO.StreamReader]::new($fs).ReadToEnd() } finally { $fs.Dispose() }
                foreach ($l in ($texte -split "`r?`n")) {
                    if ($l -match '^k (\d+) ([\d.]+)$') { $ks.Add([int]$Matches[1]); $ms.Add((Nombre-Texte $Matches[2])) }
                }
            }
            catch { }
            return [pscustomobject]@{ K = $ks; Ms = $ms }
        }
        # Le k montré à l'heure $Qpc (le dernier pas écrit avant), -1 avant le premier.
        function K-A($Pas, [double]$Qpc) {
            $k = -1
            for ($i = 0; $i -lt $Pas.Ms.Count; $i++) { if ($Pas.Ms[$i] -le $Qpc) { $k = $Pas.K[$i] } else { break } }
            return $k
        }
        # Pas du témoin par seconde entre deux heures.
        function Rythme-Temoin($Pas, [double]$De, [double]$A) {
            $n = 0
            for ($i = 0; $i -lt $Pas.Ms.Count; $i++) { if ($Pas.Ms[$i] -ge $De -and $Pas.Ms[$i] -le $A) { $n++ } }
            if ($A -le $De) { return [double]::NaN }
            return $n / (($A - $De) / 1000.0)
        }

        # --- Lecteurs ---------------------------------------------------------------------------
        # Le MP4 relu par lire_film aux points du témoin (16 carrés, 4 pastilles, bande, 8 bords
        # blancs), puis $Plus : { L, H, DureeMs, Images, Son, Video, Conteneur, Ks, Ts, Couleurs
        # (par image, la liste des [r, v, b]), EcartPastilles, EcartBords }.
        function Relire-Mp4([string]$Chemin, [string]$Quoi, [int[]]$Plus = @()) {
            if (-not $script:LireFilm) { throw "$Quoi : lecteur lire_film introuvable (BREEZYSHOT_LIRE_FILM)" }
            $pts = [System.Collections.Generic.List[int]]::new()
            foreach ($v in $script:Temoin::Points()) { $pts.Add($v) }
            foreach ($v in $Plus) { $pts.Add($v) }
            $paires = [System.Collections.Generic.List[string]]::new()
            for ($i = 0; $i -lt $pts.Count; $i += 2) { $paires.Add("$($pts[$i]),$($pts[$i + 1])") }
            $json = Join-Path $script:Essai.Dossier "lu-$(Nouveau-Nom).json"
            $p = Start-Process -FilePath $script:LireFilm -PassThru -NoNewWindow -ArgumentList @("`"$Chemin`"", '--points', "`"$($paires -join ';')`"", '--sortie', "`"$json`"")
            $null = $p.Handle
            if (-not $p.WaitForExit(120000)) { try { $p.Kill() } catch { }; throw "$Quoi : lire_film n'a pas fini en 120 s" }
            if (-not (Test-Path -LiteralPath $json)) { throw "$Quoi : lire_film n'a rien écrit (code $($p.ExitCode))" }
            $j = Get-Content -LiteralPath $json -Raw | ConvertFrom-Json
            if (-not $j.ok) { throw "$Quoi : lire_film : $($j.erreur)" }
            $detail = @($j.detail)
            $n = $detail.Count
            $ks = [int[]]::new($n); $ts = [double[]]::new($n)
            $couleurs = [object[]]::new($n)
            $ecartP = 0; $ecartB = 0
            for ($i = 0; $i -lt $n; $i++) {
                $d = $detail[$i]
                $ts[$i] = [double]$d.t_ms
                $coul = @($d.couleurs)
                $couleurs[$i] = $coul
                $rvb = [int[]]::new(48); $lisible = $true
                for ($q = 0; $q -lt 16; $q++) {
                    $x = $coul[$q]
                    if ($null -eq $x) { $lisible = $false; break }
                    $rvb[3 * $q] = [int]$x[0]; $rvb[3 * $q + 1] = [int]$x[1]; $rvb[3 * $q + 2] = [int]$x[2]
                }
                $ks[$i] = if ($lisible) { $script:Temoin::KDeCouleurs($rvb) } else { -1 }
                for ($q = 0; $q -lt 4; $q++) {
                    $x = $coul[16 + $q]
                    if ($null -eq $x) { $ecartP = 999; continue }
                    for ($c = 0; $c -lt 3; $c++) { $ecartP = [math]::Max($ecartP, [math]::Abs([int]$x[$c] - $PASTILLES[$q][$c])) }
                }
                for ($q = 21; $q -lt 29; $q++) {
                    $x = $coul[$q]
                    if ($null -eq $x) { $ecartB = 999; continue }
                    for ($c = 0; $c -lt 3; $c++) { $ecartB = [math]::Max($ecartB, 255 - [int]$x[$c]) }
                }
            }
            return [pscustomobject]@{ L = [int]$j.largeur; H = [int]$j.hauteur; DureeMs = [double]$j.duree_ms; Images = [int]$j.images; Son = [bool]$j.son
                Video = "$($j.video)"; Conteneur = "$($j.conteneur)"; Ks = $ks; Ts = $ts; Couleurs = $couleurs; EcartPastilles = $ecartP; EcartBords = $ecartB
                Octets = (Get-Item -LiteralPath $Chemin).Length }
        }
        # Le GIF relu par System.Drawing : { L, H, Images, DureeMs (somme des délais), Boucle,
        # Ks, Ts (début de chaque image), PastillesMin (pixels exacts sur 9 216), EcartBords }.
        function Relire-Gif([string]$Chemin, [string]$Quoi, [int]$MaxImages = 1500) {
            $img = [System.Drawing.Image]::FromFile($Chemin)
            try {
                $larg = $img.Width; $haut = $img.Height
                $dim = [System.Drawing.Imaging.FrameDimension]::new($img.FrameDimensionsList[0])
                $n = $img.GetFrameCount($dim)
                $v = $img.GetPropertyItem(0x5100).Value
                $delais = [int[]]::new($n)
                for ($i = 0; $i -lt $n; $i++) { $delais[$i] = 10 * [BitConverter]::ToInt32($v, 4 * $i) }
                $somme = 0; foreach ($x in $delais) { $somme += $x }
                $boucle = -1
                try { $boucle = [int][BitConverter]::ToUInt16($img.GetPropertyItem(0x5101).Value, 0) } catch { }
                $lus = [math]::Min($n, $MaxImages)
                $ks = [int[]]::new($lus); $ts = [double[]]::new($lus)
                $pmin = [int]::MaxValue; $bmax = 0; $t = 0.0
                $b = [System.Drawing.Bitmap]::new($larg, $haut, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
                $g = [System.Drawing.Graphics]::FromImage($b)
                try {
                    $g.CompositingMode = [System.Drawing.Drawing2D.CompositingMode]::SourceCopy
                    for ($i = 0; $i -lt $lus; $i++) {
                        [void]$img.SelectActiveFrame($dim, $i)
                        $g.DrawImage($img, 0, 0, $larg, $haut)
                        $o = (Octets-Image $b).O
                        $ts[$i] = $t; $t += $delais[$i]
                        if ($larg -ge 600 -and $haut -ge 450) {
                            $ks[$i] = $script:Temoin::LireK($o, $larg, 0, 0)
                            $pmin = [math]::Min($pmin, $script:Temoin::PastillesExactes($o, $larg, 0, 0))
                            $bmax = [math]::Max($bmax, $script:Temoin::EcartBords($o, $larg))
                        }
                        else { $ks[$i] = -1 }
                    }
                }
                finally { $g.Dispose(); $b.Dispose() }
            }
            finally { $img.Dispose() }
            return [pscustomobject]@{ L = $larg; H = $haut; Images = $n; DureeMs = $somme; Boucle = $boucle; Ks = $ks; Ts = $ts
                PastillesMin = $(if ($pmin -eq [int]::MaxValue) { -1 } else { $pmin }); EcartBords = $bmax; Octets = (Get-Item -LiteralPath $Chemin).Length }
        }

        # --- UI Automation (essais 13 et 14) -------------------------------------------------
        function Elements-Vus([IntPtr]$Racine) {
            try {
                $tous = $AE::FromHandle($Racine).FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
                $noms = @()
                foreach ($el in $tous) {
                    $c = $el.Current
                    if ($c.Name -ne '') { $noms += "$($c.ControlType.ProgrammaticName -replace '^ControlType\.', '') « $($c.Name) »" }
                    if ($noms.Count -ge 40) { break }
                }
                return "$($tous.Count) éléments vus ; nommés : " + ($noms -join ', ')
            } catch { return "UI Automation en échec : $($_.Exception.Message)" }
        }
        function Element($Racine, [string]$Nom, $Type = $null, [double]$DelaiS = 5) {
            $cond = [System.Windows.Automation.PropertyCondition]::new($AE::NameProperty, $Nom)
            if ($null -ne $Type) {
                $cond = [System.Windows.Automation.AndCondition]::new($cond, [System.Windows.Automation.PropertyCondition]::new($AE::ControlTypeProperty, $Type))
            }
            $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
            do {
                try {
                    $el = $AE::FromHandle([IntPtr]$Racine).FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
                    if ($null -ne $el) { return $el }
                } catch { }
                Start-Sleep -Milliseconds 100
            } while ([DateTime]::UtcNow -lt $limite)
            return $null
        }
        function Centre-Stable($el) {
            $r = $el.Current.BoundingRectangle
            $limite = [DateTime]::UtcNow.AddSeconds(2)
            do {
                Start-Sleep -Milliseconds 120
                $avant = $r; $r = $el.Current.BoundingRectangle
            } while (($r.X -ne $avant.X -or $r.Y -ne $avant.Y) -and [DateTime]::UtcNow -lt $limite)
            if ($r.IsEmpty -or $r.Width -le 0 -or $el.Current.IsOffscreen) { throw "« $($el.Current.Name) » hors écran ou sans rectangle" }
            return @([int]($r.X + $r.Width / 2), [int]($r.Y + $r.Height / 2))
        }
        # La durée affichée par la pilule (texte « 0:01 »), '' sinon.
        function Duree-Lue([IntPtr]$H) {
            try {
                $tous = $AE::FromHandle($H).FindAll([System.Windows.Automation.TreeScope]::Descendants,
                    [System.Windows.Automation.PropertyCondition]::new($AE::ControlTypeProperty, $CT::Text))
                foreach ($e in $tous) { $nm = $e.Current.Name; if ($nm -match '^\d+:\d\d$') { return $nm } }
            } catch { }
            return ''
        }
        function Ouvrir-Menu([string]$Quoi) { return (Ouvrir-Menu-Icone $script:proc.Id $script:Cas.Dossier $Quoi) }
        # Pendant un film, sans Échap : un second clic sur l'icône referme le menu.
        function Fermer-Menu-Par-Icone($Menu, [string]$Quoi) {
            $d = $script:Cas.Dossier
            $fermes = Compter-Motif 'menu ferme$' $d
            Start-Sleep -Milliseconds 400   # anti-rebond de l'icône (300 ms)
            Clic-Icone $Menu.IdProc $Quoi
            $null = Attendre-Motif 'menu ferme$' $d 3 "$Quoi : menu ferme (second clic sur l'icône)" ($fermes + 1)
            $limite = [DateTime]::UtcNow.AddSeconds(2)
            while ($script:Natif::IsWindowVisible($Menu.Fenetre) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 30 }
            if ($script:Natif::IsWindowVisible($Menu.Fenetre)) { throw "$Quoi : le menu est encore visible 2 s après « menu ferme »" }
        }
        # Un raccourci tenu un instant par l'essai : « tenu » si l'app ne le tient pas.
        function Tenir-Ailleurs([int]$Vk, [string]$Nom) {
            $fichier = Join-Path $script:Essai.Dossier "tenu-$Nom-$(Nouveau-Nom).txt"
            $aide = Start-Process -FilePath $script:Pwsh -PassThru -WindowStyle Hidden -ArgumentList @('-NoProfile', '-File', "`"$(Join-Path $PSScriptRoot 'tenir-raccourci.ps1')`"", '-Vk', $Vk, '-Mods', $MOD_CTRL_MAJ, '-Fichier', "`"$fichier`"")
            $script:Auxiliaires.Add($aide)
            $limite = [DateTime]::UtcNow.AddSeconds(30)
            while (-not (Test-Path -LiteralPath $fichier) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 100 }
            $lu = if (Test-Path -LiteralPath $fichier) { (Get-Content -LiteralPath $fichier -Raw).Trim() } else { 'pas de réponse en 30 s' }
            try { if (-not $aide.HasExited) { $aide.Kill(); $null = $aide.WaitForExit(5000) } } catch { }
            return $lu
        }
        # Les fenêtres de plomberie de Tauri (instance unique « …-sic » / « …-siw », cible des
        # événements de Tao) ne se montrent jamais à l'écran : elles ne comptent pas comme une
        # fenêtre de l'app, mais restent dites (Plomberie-Visible) si leur drapeau visible est mis.
        function Est-Plomberie([IntPtr]$H) {
            $c = $script:Natif::Classe($H); $t = $script:Natif::Titre($H)
            return (($c -like '*-sic') -and ($t -like '*-siw')) -or ($t -ceq 'Tao Thread Event Target')
        }
        function Visibles-App {
            return @($script:Natif::Visibles($script:proc.Id) | Where-Object { -not (Est-Plomberie $_) } | ForEach-Object { "$($script:Natif::Classe($_)) « $($script:Natif::Titre($_)) »" })
        }
        function Plomberie-Visible {
            return @($script:Natif::Visibles($script:proc.Id) | Where-Object { Est-Plomberie $_ } | ForEach-Object { "$($script:Natif::Classe($_)) « $($script:Natif::Titre($_)) » $(@($script:Natif::Rect($_)) -join ',')" })
        }
        # Le presse-papiers, dans un fil STA : la liste de fichiers ($null sans liste).
        function Fichiers-PressePapiers {
            $rs = [runspacefactory]::CreateRunspace()
            $rs.ApartmentState = [System.Threading.ApartmentState]::STA
            $rs.Open()
            try {
                $ps = [powershell]::Create()
                $ps.Runspace = $rs
                [void]$ps.AddScript({
                        Add-Type -AssemblyName System.Windows.Forms
                        for ($i = 0; $i -lt 10; $i++) {
                            try {
                                if ([System.Windows.Forms.Clipboard]::ContainsFileDropList()) {
                                    $l = [System.Windows.Forms.Clipboard]::GetFileDropList()
                                    return (@($l) -join "`n")
                                }
                            } catch { }
                            Start-Sleep -Milliseconds 100
                        }
                        return ''
                    })
                try { $r = @($ps.Invoke()) } finally { $ps.Dispose() }
                $texte = if ($r.Count -gt 0) { "$($r[0])" } else { '' }
                return @($texte -split "`n" | Where-Object { $_ -ne '' })
            }
            finally { $rs.Dispose() }
        }
        function Memoire-App {
            $script:proc.Refresh()
            return [math]::Round($script:proc.PrivateMemorySize64 / 1MB, 2)
        }
        function Modules-App {
            $p = Get-Process -Id $script:proc.Id
            return @($p.Modules | ForEach-Object { $_.ModuleName.ToLowerInvariant() } | Sort-Object -Unique)
        }
        function Reference-Modules([string]$Section) {
            $f = Join-Path $PSScriptRoot '../docs/sonde-film-modules-0.6.1.txt'
            $dans = $false; $noms = [System.Collections.Generic.List[string]]::new()
            foreach ($l in [System.IO.File]::ReadAllLines($f, [System.Text.Encoding]::UTF8)) {
                if ($l -match '^\[(.+)\]$') { $dans = ($Matches[1] -eq $Section); continue }
                if ($dans -and $l -match '^([^\t#]+)\t') { $noms.Add($Matches[1].Trim().ToLowerInvariant()) }
            }
            return @($noms)
        }
        # Sans licence, aucun des huit. Avec licence, au repos : rien hors de la référence
        # et des onze de F32 (mf.dll comprise : depuis les corrections, elle n'est plus
        # chargée à la demande par l'éditeur de liens, qui n'en garde que quatre — mfplat,
        # mfreadwrite, dxgi, d3d11 —, mais la garde de systeme::mp4 la charge elle-même
        # depuis System32 avec la recherche du codeur ; la liste reste une permission).
        $SURVEILLES = @('mfplat.dll', 'mfreadwrite.dll', 'mf.dll', 'dxgi.dll', 'd3d11.dll', 'mmdevapi.dll', 'audioses.dll', 'msmpeg2enc.dll')
        $ONZE_F32 = @('appxdeploymentclient.dll', 'cfgmgr32.dll', 'comppkgsup.dll', 'imagehlp.dll', 'mf.dll', 'mfplat.dll', 'mfreadwrite.dll', 'rtworkq.dll', 'windows.applicationmodel.dll', 'windows.media.dll', 'windows.staterepositoryps.dll')

        # --- L'app du cas ---------------------------------------------------------------------
        function Etat-App([string]$Dossier) {
            $l = Derniere-Ligne 'etat \w+ -> \w+ ' $Dossier
            if ($l -and $l -match 'etat \w+ -> (\w+) ') { return $Matches[1] }
            return 'Repos'
        }
        function Echap-Tenu([string]$Dossier) {
            return (Compter-Motif 'echap global reserve$' $Dossier) -gt (Compter-Motif 'echap global libere$' $Dossier)
        }
        # Un raccourci global de l'app, seulement s'il est tenu (dernière ligne `raccourci
        # <action>: Ctrl+Maj+…`, pas rendu depuis).
        function Appuyer-Raccourci([string]$Action, [byte]$Vk, [string]$Quoi) {
            $l = Derniere-Ligne "raccourci $Action(: \S+| rendu)$" $script:Cas.Dossier
            if (-not $l -or $l -notmatch "raccourci ${Action}: Ctrl\+Maj\+") { throw "$Quoi : le raccourci de « $Action » n'est pas tenu par l'app ($(Message $l)) : la touche n'est pas envoyée" }
            $script:Natif::AppuyerGlobal($MODS, $Vk)
        }
        function Appuyer-Film([string]$Sorte, [string]$Quoi) {
            Appuyer-Raccourci $Sorte $(if ($Sorte -eq 'video') { $VK_6 } else { $VK_7 }) $Quoi
        }
        function Appuyer-Echap([string]$Quoi) {
            $d = $script:Cas.Dossier
            $limite = [DateTime]::UtcNow.AddSeconds(3)
            while (-not (Echap-Tenu $d) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 30 }
            if (-not (Echap-Tenu $d)) { throw "$Quoi : Échap non tenu par l'app (réservation absente ou libérée) : la touche n'est pas envoyée ; $(Diagnostic-Journal $d)" }
            $script:Natif::AppuyerGlobal($AUCUN, $ECHAP)
        }
        # Lance l'app du cas en cours ($script:Cas) et attend ce qu'il lui faut.
        function Lancer-App-Du-Cas([string]$Quoi) {
            $cas = $script:Cas
            $d = $cas.Dossier
            $n = @{}
            foreach ($m in 'ecran fige fenetre \d+$', 'vignette fenetre \d+$', 'video disponible: ', 'raccourci video: ', 'raccourci gif: ') { $n[$m] = Compter-Motif $m $d }
            $script:proc = Demarrer-Cas $d $Quoi -Capturable:([bool]$cas.Capturable) -SansCodeur:([bool]$cas.SansCodeur) -AvecLicence
            $script:EcranFige = Hwnd-Journal 'ecran fige fenetre' $d ($n['ecran fige fenetre \d+$'] + 1)
            $script:FenetreVignette = [IntPtr]::Zero
            try { $script:FenetreVignette = Hwnd-Journal 'vignette fenetre' $d ($n['vignette fenetre \d+$'] + 1) 20 } catch { Write-Host "$Quoi : fenêtre de la vignette non dite ($_)" }
            $null = Attendre-Motif 'video disponible: ' $d 30 "$Quoi : recherche du codeur (video disponible)" ($n['video disponible: '] + 1)
            $null = Attendre-Motif 'raccourci gif: ' $d 10 "$Quoi : raccourci gif" ($n['raccourci gif: '] + 1)
            if (-not $cas.SansCodeur) { $null = Attendre-Motif 'raccourci video: ' $d 10 "$Quoi : raccourci video" ($n['raccourci video: '] + 1) }
        }
        # Un cas avec licence : dossier, réglages v6, licence, app lancée.
        function Ouvrir-Cas([string]$Nom, [string]$Quoi, [switch]$Capturable, [switch]$SansCodeur, [bool]$Copier = $true, [bool]$Vignette = $true) {
            $d = Dossier-Cas $Nom
            Ecrire-Reglages $d 6 $Copier $Vignette
            Poser-Licence $d
            $script:Cas = [pscustomobject]@{ Nom = $Nom; Dossier = $d; Images = (Join-Path $d 'images'); Films = (Join-Path $d 'local/Films')
                Recents = (Join-Path $d 'local/Recents'); Glisser = (Join-Path $d 'local/Glisser'); Capturable = [bool]$Capturable; SansCodeur = [bool]$SansCodeur
                Vignette = $Vignette; Temoin = $null }
            Lancer-App-Du-Cas $Quoi
            return $script:Cas
        }
        # L'app au repos (ou sa vignette) : rien d'autre ne peut commencer un film.
        function Attendre-Libre([string]$Quoi) {
            $d = $script:Cas.Dossier
            $limite = [DateTime]::UtcNow.AddSeconds(10)
            while ((Etat-App $d) -notin 'Repos', 'Vignette' -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 50 }
            if ((Etat-App $d) -notin 'Repos', 'Vignette') { throw "$Quoi : l'app n'est pas revenue au repos en 10 s (état $(Etat-App $d))" }
        }
        # Après l'échec d'un point : souris au repos, film arrêté (Échap s'il est tenu, sinon le
        # raccourci du film), sinon l'app relancée dans le dossier du cas.
        function Revenir-Au-Repos([string]$Quoi) {
            $cas = $script:Cas
            if ($null -eq $cas -or $null -eq $script:proc) { return }
            try {
                $script:Natif::BoutonHaut()
                $script:Natif::Poser($REPOS_X, $REPOS_Y)
                $dit = "état $(Etat-App $cas.Dossier)"
                $echap = $false; $arret = $false
                $limite = [DateTime]::UtcNow.AddSeconds(15)
                while (-not $script:proc.HasExited -and (Etat-App $cas.Dossier) -notin 'Repos', 'Vignette' -and [DateTime]::UtcNow -lt $limite) {
                    $etat = Etat-App $cas.Dossier
                    if (-not $echap -and (Echap-Tenu $cas.Dossier)) { $script:Natif::AppuyerGlobal($AUCUN, $ECHAP); $echap = $true; $dit += ', Échap global' }
                    elseif (-not $arret -and $etat -eq 'Enregistrement' -and -not (Echap-Tenu $cas.Dossier)) {
                        $z = Derniere-Ligne 'film (video|gif) zone ' $cas.Dossier
                        $sorte = if ($z -match 'film (video|gif) zone ') { $Matches[1] } else { 'video' }
                        try { Appuyer-Film $sorte "$Quoi (repos)" } catch { }
                        $arret = $true; $dit += ", raccourci du film ($sorte)"
                    }
                    Start-Sleep -Milliseconds 100
                }
                if ($script:proc.HasExited -or (Etat-App $cas.Dossier) -notin 'Repos', 'Vignette') {
                    $dit += ", app relancée (état $(Etat-App $cas.Dossier))"
                    Arreter-Tout
                    Lancer-App-Du-Cas "$Quoi (relance)"
                }
                Write-Host "[repos] $Quoi : retour au repos : $dit"
            }
            catch { Write-Host "[repos] $Quoi : retour au repos impossible : $_" }
        }

        # --- Un film, pas à pas ---------------------------------------------------------------
        # Clic réel sur un bouton de la pilule (UI Automation, souris vue sur la pilule) ;
        # rend l'heure du clic (horloge de performance).
        function Cliquer-Pilule($Film, [string]$Nom, [string]$Quoi) {
            $el = Element $Film.Hwnd $Nom $CT::Button 3
            if ($null -eq $el) { throw "$Quoi : « $Nom » introuvable dans la fenêtre de contrôle ; $(Elements-Vus $Film.Hwnd)" }
            $pt = Centre-Stable $el
            if (-not $script:Natif::PoserSurApp($pt[0], $pt[1], 2000)) { throw "$Quoi : souris sur « $Nom » : la fenêtre de contrôle ne prend pas le clic en 2 s ($($script:Natif::Decrire($script:Natif::Sous($pt[0], $pt[1]))))" }
            $q = Qpc-Maintenant
            $script:Natif::BoutonBas(); Start-Sleep -Milliseconds 60; $script:Natif::BoutonHaut()
            $script:Natif::Poser($REPOS_X, $REPOS_Y)
            return $q
        }
        # Survole la carte de la vignette (boutons montrés), puis clique le bouton $Action
        # (rectangle de `vignette bouton <id> <action>`).
        function Cliquer-Vignette([int]$Id, [string]$Action, [string]$Quoi) {
            $d = $script:Cas.Dossier
            $lb = Derniere-Ligne "vignette bouton $Id $Action " $d
            $lz = Derniere-Ligne "vignette zone $Id " $d
            if (-not $lb -or -not $lz) { throw "$Quoi : bouton « $Action » de la carte $Id inconnu du journal" }
            $b = Lire-Rect (Message $lb)
            $z = Lire-Rect (Message $lz)
            $script:Natif::Poser($z.CX, $z.CY)
            Start-Sleep -Milliseconds 450
            $script:Natif::Cliquer($b.CX, $b.CY)
            Start-Sleep -Milliseconds 100
            $script:Natif::Poser($REPOS_X, $REPOS_Y)
        }
        # Un film de bout en bout : raccourci ou menu, visée, tracé (par défaut la zone du
        # témoin), décompte, film, arrêt, Finalisation, sortie. $Arret : bouton | raccourci |
        # aucun (le film finit seul, ou $Pendant l'arrête et pose ArretQpc). $Annuler : visee |
        # echap | croix | arreter | session (le film s'arrête là). Rend le film (hashtable mué en
        # objet) : lignes, nombres, fichier enregistré.
        function Faire-Film {
            param(
                [string]$Sorte = 'video', [string]$Quoi = 'film', [string]$Par = 'raccourci',
                [int[]]$Trace = $null, [double]$DureeS = 2, [string]$Arret = 'raccourci',
                [scriptblock]$PendantDecompte = $null, [scriptblock]$Pendant = $null,
                [string]$Annuler = '', [double]$FinDelaiS = 15, [switch]$SansSortie
            )
            $cas = $script:Cas
            $d = $cas.Dossier
            Attendre-Libre $Quoi
            $film = @{ Sorte = $Sorte; Quoi = $Quoi; Dossier = $d }
            # Mode strict : toute clé lue plus loin existe ($null tant que le film n'y est pas).
            foreach ($k in @(
                    'Annule', 'ArretHeure', 'ArretQpc', 'Bandeau', 'Cause', 'Chrono', 'CodeMode',
                    'ControleMontreMs', 'CopiePilule', 'CopieZone', 'Copies', 'CopiesPendant',
                    'Debut', 'DebutHeure', 'DebutIndex', 'DebutQpc', 'Decompte',
                    'DecomptePremiere', 'Duree', 'DureeFigee', 'DureesLues', 'DureesPause',
                    'Enregistre', 'Etat', 'Exclue', 'Fichier', 'FigeCacheMs', 'Fin', 'FinAvant',
                    'FinIndex', 'FinQpc', 'Hwnd', 'Images', 'MenuLu', 'MenuSansChoix', 'MenuSeul',
                    'Mesure', 'Micro', 'PagePrete', 'PauseActif', 'PauseMs', 'PauseQpc', 'Pilule',
                    'PointeurMontre', 'PremierPlan', 'PremiereVideo', 'Pret', 'Quitter',
                    'QuitterHeure', 'Relache', 'Reprendre', 'RepriseQpc', 'Sautees', 'Selection',
                    'Sortie', 'SourisA', 'SourisDe', 'Styles', 'TueeHeure', 'UiaDecompte',
                    'UiaVus', 'VignetteHeure', 'Zone')) { $film[$k] = $null }
            $film.Vignette = -1; $film.Tuee = $false
            $film.Base = Base-Journal $d
            $film.ImagesAvant = @(Fichiers-De $cas.Images)
            $film.RecentsAvant = @(Fichiers-De $cas.Recents).Count
            $script:Natif::Poser($REPOS_X, $REPOS_Y)
            $film.PremierPlanAvant = $script:Natif::GetForegroundWindow()
            if ($Par -eq 'menu') {
                $m = Ouvrir-Menu "$Quoi : menu"
                Cliquer-Ligne-Menu $m $(if ($Sorte -eq 'video') { 'Vidéo' } else { 'GIF' }) "$Quoi : menu"
                $script:Natif::Poser($REPOS_X, $REPOS_Y)
                $null = Attendre-Apres "menu: capturer $Sorte$" $d $film.Base 5 "$Quoi : menu: capturer $Sorte"
            }
            else { Appuyer-Film $Sorte $Quoi }
            $null = Attendre-Apres "-> Gel \(DemandeCapture $Sorte\)$" $d $film.Base 5 "$Quoi : DemandeCapture $Sorte"
            $l = Attendre-Apres 'selection montree (\d+) .*mode film' $d $film.Base 5 "$Quoi : sélection en mode film"
            $null = $l.Ligne -match 'selection montree (\d+) '
            $film.Selection = [int]$Matches[1]
            $null = Attendre-Apres 'chrono selection prete ' $d $film.Base 5 "$Quoi : sélection prête"
            $l = Attendre-Apres 'film fenetre (\d+)$' $d $film.Base 5 "$Quoi : film fenetre"
            $null = $l.Ligne -match 'film fenetre (\d+)$'
            $film.Hwnd = [IntPtr][long]$Matches[1]
            $film.Exclue = Message (Attendre-Apres 'film exclue des captures: ' $d $film.Base 5 "$Quoi : exclusion des captures").Ligne
            $film.PagePrete = Message (Attendre-Apres 'film page prete en \d+ ms$' $d $film.Base 10 "$Quoi : film page prete").Ligne
            $null = Attendre-Apres 'echap global reserve$' $d $film.Base 3 "$Quoi : Échap réservé"
            if ($Annuler -eq 'visee') {
                Appuyer-Echap "$Quoi : Échap pendant la visée"
                $film.Annule = Message (Attendre-Apres 'etat Selection -> Repos \(SelectionAnnulee\)$' $d $film.Base 5 "$Quoi : sélection annulée").Ligne
                return [pscustomobject]$film
            }
            $tem = $cas.Temoin
            if ($null -eq $Trace) { $Trace = @($tem.X, $tem.Y, ($tem.X + $tem.L - 1), ($tem.Y + $tem.H - 1)) }
            $g = $script:Natif::TracerPuisGuetter($Trace[0], $Trace[1], $Trace[2], $Trace[3], $script:EcranFige, $film.Hwnd, 3000)
            $script:Natif::Poser($REPOS_X, $REPOS_Y)
            $film.FigeCacheMs = $g[0]; $film.ControleMontreMs = $g[1]
            $film.Zone = Message (Attendre-Apres "film $Sorte zone " $d $film.Base 5 "$Quoi : film $Sorte zone").Ligne
            $null = Attendre-Apres 'etat Selection -> Enregistrement \(EnregistrementChoisi\)$' $d $film.Base 5 "$Quoi : EnregistrementChoisi"
            $film.Decompte = Message (Attendre-Apres 'film decompte \d+ s$' $d $film.Base 5 "$Quoi : film decompte").Ligne
            $l = Attendre-Apres 'bandeau decompte (\d+) ecran ' $d $film.Base 5 "$Quoi : bandeau decompte"
            $null = $l.Ligne -match 'bandeau decompte (\d+) ecran '
            $film.Bandeau = [int]$Matches[1]
            if ($null -ne $PendantDecompte) { & $PendantDecompte $film }
            if ($Annuler) {
                switch ($Annuler) {
                    'echap' { Appuyer-Echap "$Quoi : Échap pendant le décompte" }
                    'croix' {
                        $lz = Attendre-Apres "bandeau zone $($film.Bandeau) (-?\d+),(-?\d+) (\d+)x(\d+)$" $d $film.Base 2 "$Quoi : bandeau zone de la croix"
                        $r = Lire-Rect (Message $lz.Ligne)
                        $script:Natif::Cliquer($r.CX, $r.CY)
                        $script:Natif::Poser($REPOS_X, $REPOS_Y)
                    }
                    'arreter' { $null = Cliquer-Pilule $film 'Arrêter' "$Quoi : Arrêter pendant le décompte" }
                    'session' { $null = $script:Natif::PostMessage($script:EcranFige, [uint32]0x2B1, [IntPtr]7, [IntPtr]::Zero) }
                }
                $film.Annule = Message (Attendre-Apres 'film annule decompte \(\w+\)$' $d $film.Base 5 "$Quoi : film annule").Ligne
                $null = Attendre-Apres 'etat Enregistrement -> Repos \(EnregistrementAnnule\)$' $d $film.Base 5 "$Quoi : EnregistrementAnnule"
                return [pscustomobject]$film
            }
            $l = Attendre-Apres 'film debut \(relache (\d+) ms, pret (\d+) ms, decompte-premiere (\d+) ms\)$' $d $film.Base 8 "$Quoi : film debut"
            $null = $l.Ligne -match 'film debut \(relache (\d+) ms, pret (\d+) ms, decompte-premiere (\d+) ms\)$'
            $film.Relache = [int]$Matches[1]; $film.Pret = [int]$Matches[2]; $film.DecomptePremiere = [int]$Matches[3]
            $film.Debut = Message $l.Ligne
            $film.DebutIndex = $l.Index
            $film.DebutQpc = Qpc-Ligne $l.Ligne
            $film.DebutHeure = Heure-Ligne $l.Ligne
            $film.PremiereVideo = ($Sorte -eq 'video') -and -not $script:VideoFaite
            if ($Sorte -eq 'video') { $script:VideoFaite = $true }
            $film.ArretQpc = $null
            $film.Tuee = $false
            if ($null -ne $Pendant) { & $Pendant $film }
            # Cas L : $Pendant a tué l'app, il n'y aura ni fin ni sortie.
            if ($film.Tuee) { return [pscustomobject]$film }
            if ($DureeS -gt 0) {
                $jusqua = $film.DebutHeure.AddSeconds($DureeS)
                while ([DateTimeOffset]::Now -lt $jusqua) { Start-Sleep -Milliseconds 20 }
            }
            switch ($Arret) {
                'bouton' { $film.ArretQpc = Cliquer-Pilule $film 'Arrêter' "$Quoi : Arrêter"; $film.ArretHeure = [DateTimeOffset]::Now }
                'raccourci' { $film.ArretQpc = Qpc-Maintenant; $film.ArretHeure = [DateTimeOffset]::Now; Appuyer-Film $Sorte "$Quoi : arrêt par le raccourci" }
                default { }
            }
            $l = Attendre-Apres 'film fin (\w+) duree (\d+) ms images (\d+) sautees (\d+)$' $d $film.Base $FinDelaiS "$Quoi : film fin"
            $null = $l.Ligne -match 'film fin (\w+) duree (\d+) ms images (\d+) sautees (\d+)$'
            $film.Cause = $Matches[1]; $film.Duree = [int]$Matches[2]; $film.Images = [int]$Matches[3]; $film.Sautees = [int]$Matches[4]
            $film.Fin = Message $l.Ligne
            $film.FinIndex = $l.Index
            $film.FinQpc = Qpc-Ligne $l.Ligne
            if ($null -eq $film.ArretQpc) { $film.ArretQpc = $film.FinQpc; $film.ArretHeure = Heure-Ligne $l.Ligne }
            if ($SansSortie) { return [pscustomobject]$film }
            $film.Fichier = Message (Attendre-Apres "film fichier $Sorte \d+ octets en \d+ ms$" $d $film.Base 15 "$Quoi : film fichier").Ligne
            $film.Chrono = Message (Attendre-Apres 'chrono film cadence ' $d $film.Base 5 "$Quoi : chrono film").Ligne
            $film.Etat = Message (Attendre-Apres 'etat Finalisation -> \w+ \(\w+\)$' $d $film.Base 10 "$Quoi : fin de la Finalisation").Ligne
            $film.Sortie = Message (Attendre-Apres 'sortie film copie .+ fichier .+ en \d+ ms$' $d $film.Base 30 "$Quoi : sortie film").Ligne
            $film.Vignette = -1
            if ($film.Etat -match 'Souffle') {
                $l = Attendre-Apres 'vignette entree (\d+)$' $d $film.Base 10 "$Quoi : vignette entree"
                $null = $l.Ligne -match 'vignette entree (\d+)$'
                $film.Vignette = [int]$Matches[1]
                $film.VignetteHeure = Heure-Ligne $l.Ligne
                $null = Attendre-Apres "vignette zone $($film.Vignette) " $d $film.Base 3 "$Quoi : vignette zone"
            }
            # Le film enregistré (choix U4 : toujours dans le dossier d'enregistrement).
            $ext = if ($Sorte -eq 'video') { '*.mp4' } else { '*.gif' }
            $film.Enregistre = $null
            $limite = [DateTime]::UtcNow.AddSeconds(5)
            do {
                $neufs = @(Fichiers-De $cas.Images $ext | Where-Object { $film.ImagesAvant -notcontains $_ })
                if ($neufs.Count -ge 1) { $film.Enregistre = $neufs[-1]; break }
                Start-Sleep -Milliseconds 100
            } while ([DateTime]::UtcNow -lt $limite)
            return [pscustomobject]$film
        }

        # --- Vérifications communes d'un film ----------------------------------------------
        # Les k relus contre le témoin : sans recul, lisibles, le premier à la fin du décompte
        # ou après, le dernier à l'arrêt à $MargeMs près.
        function Verifier-K([string]$Quoi, [int[]]$Ks, $Film, $Pas, [int]$MargeMs = 300) {
            $lus = @($Ks | Where-Object { $_ -ge 0 })
            $illisibles = $Ks.Count - $lus.Count
            $reculs = 0
            for ($i = 1; $i -lt $lus.Count; $i++) { if ($lus[$i] -lt $lus[$i - 1]) { $reculs++ } }
            Verifier "$Quoi les k ne reculent jamais, presque tous lisibles" (($lus.Count -gt 0) -and ($reculs -eq 0) -and ($illisibles -le [math]::Max(1, [math]::Floor($Ks.Count / 100)))) "$($Ks.Count) images, $illisibles illisibles, $reculs reculs" | Out-Null
            if ($lus.Count -eq 0) { return }
            $finDecompte = $Film.DebutQpc - $Film.DecomptePremiere
            $kDecompte = K-A $Pas ($finDecompte - 50)
            Verifier "$Quoi premier k celui de la fin du décompte ou après (rien du décompte filmé)" ($lus[0] -ge $kDecompte) "premier $($lus[0]), k à la fin du décompte $kDecompte" | Out-Null
            $kAvant = K-A $Pas ($Film.ArretQpc - $MargeMs)
            $kApres = K-A $Pas ($Film.ArretQpc + $MargeMs)
            Verifier "$Quoi dernier k celui de l'arrêt à $MargeMs ms près" (($lus[-1] -ge $kAvant) -and ($lus[-1] -le $kApres)) "dernier $($lus[-1]), témoin de $kAvant à $kApres autour de l'arrêt" | Out-Null
        }
        # Le MP4 enregistré relu : forme, durée, images, k, couleurs. Rend la lecture.
        function Verifier-Mp4([string]$Quoi, $Film, [int]$Largeur = 600, [int]$Hauteur = 450, [switch]$SansTemoin, [int[]]$Plus = @(), [int]$MargeMs = 300) {
            if (-not $Film.Enregistre) { throw "$Quoi : aucun .mp4 nouveau dans le dossier d'enregistrement" }
            $a = Relire-Mp4 $Film.Enregistre $Quoi $Plus
            Verifier "$Quoi fichier relu : MP4, H.264, $Largeur × $Hauteur, sans piste son" (($a.Conteneur -eq 'mp4') -and ($a.Video -eq 'h264') -and (-not $a.Son) -and ($a.L -eq $Largeur) -and ($a.H -eq $Hauteur)) "$($a.Conteneur), $($a.Video), $($a.L)x$($a.H), son $($a.Son)" | Out-Null
            Verifier "$Quoi durée du fichier = $($Film.Duree) ms à 200 ms près" ([math]::Abs($a.DureeMs - $Film.Duree) -le 200) "$($a.DureeMs) ms" | Out-Null
            $minimum = [math]::Ceiling(28 * $Film.Duree / 1000.0)
            Verifier "$Quoi au moins 28 images par seconde de film" ($a.Images -ge $minimum) "$($a.Images) images pour $($Film.Duree) ms (au moins $minimum)" | Out-Null
            Noter-Mesure -Nom "$Quoi taille du fichier" -Valeur ('{0:0.000}' -f ($a.Octets / 1MB)) -Unite 'Mo'
            if (-not $SansTemoin) {
                Verifier-K $Quoi $a.Ks $Film (Pas-Temoin $script:Cas.Temoin) $MargeMs
                Verifier "$Quoi pastilles à ± 8 (la pastille du haut en haut : image à l'endroit)" ($a.EcartPastilles -le 8) "écart maximal $($a.EcartPastilles)" | Out-Null
                Verifier "$Quoi bords de l'image : le blanc du témoin (ni cadre, ni voile, ni pilule)" ($a.EcartBords -le 24) "écart maximal au blanc $($a.EcartBords)" | Out-Null
            }
            return $a
        }
        function Verifier-Gif([string]$Quoi, $Film, [int]$Largeur = 600, [int]$Hauteur = 450, [switch]$SansTemoin, [int]$MargeMs = 300) {
            if (-not $Film.Enregistre) { throw "$Quoi : aucun .gif nouveau dans le dossier d'enregistrement" }
            $a = Relire-Gif $Film.Enregistre $Quoi
            Verifier "$Quoi fichier relu (System.Drawing) : $Largeur × $Hauteur, boucle infinie" (($a.L -eq $Largeur) -and ($a.H -eq $Hauteur) -and ($a.Boucle -eq 0)) "$($a.L)x$($a.H), boucle $($a.Boucle), $($a.Images) images" | Out-Null
            Verifier "$Quoi somme des délais = $($Film.Duree) ms à 100 ms près" ([math]::Abs($a.DureeMs - $Film.Duree) -le 100) "$($a.DureeMs) ms" | Out-Null
            Noter-Mesure -Nom "$Quoi taille du fichier" -Valeur ('{0:0.000}' -f ($a.Octets / 1MB)) -Unite 'Mo'
            if (-not $SansTemoin) {
                Verifier-K $Quoi $a.Ks $Film (Pas-Temoin $script:Cas.Temoin) $MargeMs
                Verifier "$Quoi pastilles exactes sur toutes les images" ($a.PastillesMin -eq 9216) "au moins $($a.PastillesMin) pixels exacts sur 9216" | Out-Null
                Verifier "$Quoi bords : le blanc du témoin" ($a.EcartBords -le 8) "écart maximal $($a.EcartBords)" | Out-Null
            }
            return $a
        }
        # Budgets du §9 sur `film debut` et `chrono film`.
        function Verifier-Budgets([string]$Quoi, $Film, [switch]$Long) {
            $src = Trouver-Apres 'film source (duplication|bitblt)' $Film.Dossier $Film.Base
            $dupl = ($null -ne $src) -and ($src.Ligne -match 'film source duplication')
            $c = $Film.Chrono
            if ($c -notmatch 'cadence ([\d.]+) prise-mediane (\S+) ecriture-mediane (\S+) file-max (\d+) arret-fichier (\d+)') { throw "$Quoi : ligne chrono illisible : $c" }
            $cadence = Nombre-Texte $Matches[1]; $prise = Nombre-Texte $Matches[2]; $ecriture = Nombre-Texte $Matches[3]; $arretFichier = [int]$Matches[5]
            $moyen = if ($dupl) { 'duplication' } else { 'BitBlt' }
            $pas = $Film.Images + $Film.Sautees
            $partSautees = if ($pas -gt 0) { $Film.Sautees / $pas } else { 0 }
            if ($Film.Sorte -eq 'video') {
                $cMin = if ($dupl) { 28 } else { 24 }; $pMax = if ($dupl) { 12 } else { 40 }; $eMax = 5; $aMax = 500; $sMax = if ($dupl) { 0.05 } else { 0.15 }
            }
            else { $cMin = 14; $pMax = if ($dupl) { 12 } else { 40 }; $eMax = 10; $aMax = 1000; $sMax = if ($dupl) { 0.05 } else { 0.15 } }
            Verifier "$Quoi cadence ≥ $cMin images par seconde ($moyen)" ($cadence -ge $cMin) $c | Out-Null
            Verifier "$Quoi pas sautés ≤ $([math]::Round($sMax * 100)) %" ($partSautees -le $sMax) "$($Film.Sautees) sur $pas" | Out-Null
            Verifier "$Quoi prise médiane ≤ $pMax ms ($moyen)" ((-not [double]::IsNaN($prise)) -and ($prise -le $pMax)) "$prise ms" | Out-Null
            Verifier "$Quoi écriture médiane ≤ $eMax ms" ((-not [double]::IsNaN($ecriture)) -and ($ecriture -le $eMax)) "$ecriture ms" | Out-Null
            Verifier "$Quoi fin → fichier fermé ≤ $aMax ms" ($arretFichier -le $aMax) "$arretFichier ms" | Out-Null
            if ($Film.PremiereVideo) {
                Noter-Mesure -Nom "$Quoi pret du premier film vidéo de la machine (relâchement → préparation finie, borne 3 000 ms)" -Valeur $Film.Pret -Unite 'ms'
                Verifier "$Quoi pret ≤ 3 000 ms (premier film vidéo de la machine)" ($Film.Pret -le 3000) "$($Film.Pret) ms" | Out-Null
            }
            else { Verifier "$Quoi pret ≤ 500 ms après le relâchement" ($Film.Pret -le 500) "$($Film.Pret) ms" | Out-Null }
            Verifier "$Quoi fin du décompte → première image ≤ 150 ms" ($Film.DecomptePremiere -le 150) "$($Film.DecomptePremiere) ms" | Out-Null
            Noter-Mesure -Nom "$Quoi film debut / chrono film" -Valeur "$($Film.Debut) ; $c ; $($Film.Sortie)"
        }
        # Lignes de film communes : Échap libéré avant `film debut`, jamais réservé ensuite.
        function Verifier-Echap([string]$Quoi, $Film) {
            $t = @(Lignes-Apres $Film.Dossier $Film.Base)
            $libere = -1; $reserveApres = 0
            for ($i = 0; $i -lt $t.Count; $i++) {
                if ($t[$i] -match 'echap global libere$' -and $libere -lt 0) { $libere = $i }
                if ($i -gt $Film.DebutIndex -and $i -lt $Film.FinIndex -and $t[$i] -match 'echap global reserve$') { $reserveApres++ }
            }
            Verifier "$Quoi `echap global libere` avant `film debut`, jamais `echap global reserve` pendant le film" (($libere -ge 0) -and ($libere -lt $Film.DebutIndex) -and ($reserveApres -eq 0)) "libéré au rang $libere, début au rang $($Film.DebutIndex), réservé pendant $reserveApres fois" | Out-Null
        }
        # La vignette d'un film : la carte nommée, la durée, les boutons (jamais Annoter).
        function Verifier-Vignette([string]$Quoi, $Film, [string]$Prefixe = '') {
            if ($Film.Vignette -lt 0) { throw "$Quoi : aucune vignette ($($Film.Etat))" }
            $noms = @(Toutes-Apres "vignette bouton $($Film.Vignette) (\w+) " $Film.Dossier $Film.Base | ForEach-Object { if ((Message $_) -match "vignette bouton \d+ (\w+) ") { $Matches[1] } } | Select-Object -Unique)
            $ok = ($noms -contains 'copier') -and ($noms -contains 'enregistrer') -and ($noms -contains 'supprimer') -and ($noms -notcontains 'annoter')
            Verifier "$Quoi vignette : boutons Copier, Enregistrer, Supprimer, pas d'Annoter" $ok ($noms -join ', ') | Out-Null
            $s = [int][math]::Floor([math]::Max(1000, $Film.Duree) / 1000)
            $duree = "$Prefixe$([math]::Floor($s / 60)):$('{0:00}' -f ($s % 60))"
            $carte = if ($script:FenetreVignette -ne [IntPtr]::Zero) { Element $script:FenetreVignette 'Enregistrement terminé' $null 3 } else { $null }
            $texte = if ($script:FenetreVignette -ne [IntPtr]::Zero) { Element $script:FenetreVignette $duree $null 2 } else { $null }
            Verifier "$Quoi vignette : carte « Enregistrement terminé », durée « $duree »" (($null -ne $carte) -and ($null -ne $texte)) $(if ($null -eq $carte -or $null -eq $texte) { Elements-Vus $script:FenetreVignette } else { '' }) | Out-Null
        }
        # Le presse-papiers : une liste d'un fichier de l'extension, le fichier rangé dans
        # le dossier d'enregistrement lui-même (corrections M1, M2 : rangé d'abord, copié
        # depuis Images, aucun lien dans Glisser). Rend le chemin listé.
        function Verifier-PressePapiers([string]$Quoi, $Film, [string]$Ext) {
            $l = @(Fichiers-PressePapiers)
            $meme = ($l.Count -eq 1) -and $Film.Enregistre -and ([System.IO.Path]::GetFullPath($l[0]) -ieq [System.IO.Path]::GetFullPath($Film.Enregistre))
            $ok = $meme -and ([System.IO.Path]::GetExtension($l[0]) -eq $Ext) -and (Test-Path -LiteralPath $l[0]) -and ((Empreinte $l[0]) -eq (Empreinte $Film.Enregistre))
            $liens = @(Fichiers-De $script:Cas.Glisser | Where-Object { [System.IO.Path]::GetExtension($_) -in '.mp4', '.gif' })
            Verifier "$Quoi presse-papiers : une liste d'un fichier $Ext, le fichier rangé dans le dossier d'enregistrement ; aucun lien de film dans Glisser" ($ok -and ($liens.Count -eq 0)) "$($l.Count) fichier(s) : $(@($l | ForEach-Object { $_ }) -join ', ') ; enregistré $($Film.Enregistre) ; Glisser $($liens.Count)" | Out-Null
            if ($l.Count -ge 1) { return $l[0] }
            return $null
        }
        # Processeur (% d'un cœur) et mémoire (pic) de l'app pendant $DureeS secondes.
        function Mesurer-Pendant([double]$DureeS) {
            $script:proc.Refresh()
            $cpu0 = $script:proc.TotalProcessorTime.TotalMilliseconds
            $t0 = [DateTime]::UtcNow
            $pic = 0.0; $picApp = 0.0
            $fin = $t0.AddSeconds($DureeS)
            while ([DateTime]::UtcNow -lt $fin) {
                $pic = [math]::Max($pic, (Memoire-Privee-Mo -Processus $script:proc))
                $picApp = [math]::Max($picApp, (Memoire-App))
                Start-Sleep -Milliseconds 1500
            }
            $script:proc.Refresh()
            $cpu = ($script:proc.TotalProcessorTime.TotalMilliseconds - $cpu0) / ([DateTime]::UtcNow - $t0).TotalMilliseconds * 100
            return [pscustomobject]@{ Cpu = [math]::Round($cpu, 1); Pic = $pic; PicApp = $picApp }
        }

        # diskpart (cas K4) : un script ASCII, sortie dans un fichier, attente bornée. Rend
        # Code, Fini, Ms, Texte. Le processus est tué au-delà du délai.
        function Diskpart([string[]]$Lignes, [double]$DelaiS = 120) {
            $nom = Nouveau-Nom
            $script_ = Join-Path $script:Essai.Dossier "diskpart-$nom.txt"
            $sortie = Join-Path $script:Essai.Dossier "diskpart-$nom.sortie.txt"
            [System.IO.File]::WriteAllLines($script_, $Lignes, [System.Text.Encoding]::ASCII)
            $chrono = [System.Diagnostics.Stopwatch]::StartNew()
            $p = Start-Process -FilePath 'diskpart.exe' -ArgumentList @('/s', "`"$script_`"") -PassThru -WindowStyle Hidden -RedirectStandardOutput $sortie
            $null = $p.Handle
            $fini = $p.WaitForExit([int]($DelaiS * 1000))
            $code = $null
            if ($fini) { $p.WaitForExit(); $code = $p.ExitCode } else { try { $p.Kill() } catch { } }
            $texte = ''
            try { $texte = (Get-Content -LiteralPath $sortie -Raw -ErrorAction Stop) -replace '\s+', ' ' } catch { }
            return [pscustomobject]@{ Code = $(if ($fini) { $code } else { -1 }); Fini = $fini; Ms = [math]::Round($chrono.Elapsed.TotalMilliseconds); Texte = $texte }
        }
        # Détache le disque virtuel du cas K4 (s'il est posé) et l'efface. Jamais d'exception.
        function Demonter-Disque {
            if (-not $script:Vhd) { return }
            $vhd = $script:Vhd
            $script:Vhd = $null
            try {
                if (Test-Path -LiteralPath $vhd) {
                    $x = Diskpart @("select vdisk file=`"$vhd`"", 'detach vdisk noerr') 60
                    Write-Host "(K4) disque virtuel détaché : code $($x.Code) en $($x.Ms) ms"
                    Remove-Item -LiteralPath $vhd -Force -ErrorAction SilentlyContinue
                }
                # Le point de montage vide laissé par le détachement (sans quoi le rangement
                # des artefacts le parcourrait).
                if ($script:Montage) { try { [System.IO.Directory]::Delete($script:Montage) } catch { } }
            }
            catch { Write-Host "(K4) démontage du disque virtuel : $_" }
        }

        $script:VideoFaite = $false
        $script:ReposL0 = $null; $script:ReposL0App = $null; $script:ReposA = $null; $script:ReposAApp = $null
        $script:EcranFige = [IntPtr]::Zero; $script:FenetreVignette = [IntPtr]::Zero
        $script:LCoupee = $false
        $script:ModeDepart = $script:Natif::ModeCourant()
        $script:EchelleDepart = $null
        $script:Vhd = $null
        $script:Montage = $null
        $mouseKeys = $script:Natif::AllumerMouseKeys()
        Write-Host "touches souris allumées : $mouseKeys ; pointeur montré : $($script:Natif::PointeurMontre())"
        try {
            # ===================== L0 : sans licence =====================================
            $d0 = Dossier-Cas 'l0'
            Ecrire-Reglages $d0 5
            $script:Cas = [pscustomobject]@{ Nom = 'l0'; Dossier = $d0; Images = (Join-Path $d0 'images'); Films = (Join-Path $d0 'local/Films'); Recents = (Join-Path $d0 'local/Recents')
                Glisser = (Join-Path $d0 'local/Glisser'); Capturable = $false; SansCodeur = $false; Vignette = $true; Temoin = $null }
            Point '(L0) sans licence' {
                $script:proc = Demarrer-Cas $d0 '(L0)'
                $pret = Heure-Ligne (Derniere-Ligne 'application +pret$' $d0)
                Verifier '(L0) licence absente' ((Compter-Motif 'licence absente' $d0) -ge 1) | Out-Null
                # 10 s après le démarrage : les modules, puis aucune ligne du film.
                while ([DateTimeOffset]::Now -lt $pret.AddSeconds(10)) { Start-Sleep -Milliseconds 100 }
                $mods = @(Modules-App)
                $ref = @(Reference-Modules 'sans licence')
                $plus = @($mods | Where-Object { $ref -notcontains $_ })
                $moins = @($ref | Where-Object { $mods -notcontains $_ })
                $vus = @($mods | Where-Object { $SURVEILLES -contains $_ })
                Verifier '(L0) modules 10 s après le démarrage : rien de plus que la référence 0.6.1 sans licence' ($plus.Count -eq 0) "$($mods.Count) modules ; en plus : $($plus -join ', ') ; absents de la référence : $($moins -join ', ')" | Out-Null
                Verifier '(L0) aucun des huit modules surveillés (mfplat, mfreadwrite, mf, dxgi, d3d11, mmdevapi, audioses, msmpeg2enc)' ($vus.Count -eq 0) ($vus -join ', ') | Out-Null
                $film = @(Lignes-Journal $d0 | Where-Object { $_ -match 'raccourci video|raccourci gif|video disponible|film:|film fenetre' } | ForEach-Object { Message $_ })
                Verifier '(L0) aucune ligne raccourci video, raccourci gif, video disponible, film: ni fenêtre film' ($film.Count -eq 0) ($film -join ' | ') | Out-Null
                $script:ReposL0App = Memoire-App
                $script:ReposL0 = Memoire-Privee-Mo -Processus $script:proc
                Noter-Mesure -Nom '(L0) mémoire privée au repos sans licence, processus de l''app (sonde 0.6.1 : 6,78 à 6,90 Mo)' -Valeur $script:ReposL0App -Unite 'Mo'
                Noter-Mesure -Nom '(L0) mémoire privée au repos sans licence (app et WebView2)' -Valeur $script:ReposL0 -Unite 'Mo'
                Verifier '(L0) mémoire du processus de l''app : celle de la 0.6.1 à ± 6 Mo (6,9 Mo)' ([math]::Abs($script:ReposL0App - 6.9) -le 6) "$($script:ReposL0App) Mo" | Out-Null
                # Référence comparable : l'essai 13 mesure lui aussi l'app sans licence au repos,
                # avant toute ouverture du menu (l'essai 14 mesure après le menu, page web du menu
                # chargée : 205 Mo contre 172). Borne haute seulement : plus bas n'est pas une
                # régression ; plus de 8 Mo au-dessus en est une.
                $j13 = Join-Path $script:Essai.Sortie '13-historique-texte.json'
                $ref = $null; $refDe = 'essai 13 de ce passage'
                if (Test-Path -LiteralPath $j13) {
                    try { $ref = @((Get-Content -LiteralPath $j13 -Raw | ConvertFrom-Json).Mesures | Where-Object { $_.Nom -eq 'memoire privee au repos sans licence (app et WebView2)' })[0].Valeur } catch { $ref = $null }
                }
                if ($null -eq $ref) { $ref = 174.1; $refDe = 'essai 13 mesuré au passage 65 (mesure de ce passage absente)' }
                $ref = [double]$ref
                Verifier '(L0) mémoire (app et WebView2) : au plus celle de l''essai 13 sans licence + 8 Mo' ($script:ReposL0 -le $ref + 8) "$($script:ReposL0) Mo ; référence $ref Mo ($refDe)" | Out-Null
                $m = Ouvrir-Menu '(L0)'
                $films = @($m.Textes | Where-Object { $_.StartsWith('Vidéo', [StringComparison]::Ordinal) -or $_.StartsWith('GIF', [StringComparison]::Ordinal) })
                Verifier '(L0) menu de la 0.6.1 sans licence : 9 positions, ni « Vidéo » ni « GIF »' (($films.Count -eq 0) -and ($m.Textes.Count -eq 9)) $m.Lus | Out-Null
                Fermer-Menu-Icone $m '(L0)'
                Verifier '(L0) Ctrl+Maj+6 libre (l''essai le tient puis le rend)' ((Tenir-Ailleurs 0x36 'six') -eq 'tenu') | Out-Null
                Verifier '(L0) Ctrl+Maj+7 libre' ((Tenir-Ailleurs 0x37 'sept') -eq 'tenu') | Out-Null
                foreach ($mode in 'video', 'gif') {
                    $recus = Compter-Motif "--capturer recu: $mode$" $d0
                    $non = Compter-Motif '--capturer: mode non construit$' $d0
                    $r = Lancer-Et-Attendre -Arguments @('--capturer', $mode) -DelaiS 15 -Dossier $d0
                    if (-not $r.Fini) { throw "(L0) --capturer $mode : la seconde instance ne sort pas en 15 s" }
                    $null = Attendre-Motif "--capturer recu: $mode$" $d0 5 "(L0) --capturer recu: $mode" ($recus + 1)
                    $null = Attendre-Motif '--capturer: mode non construit$' $d0 5 "(L0) --capturer $mode : mode non construit" ($non + 1)
                }
                Verifier '(L0) --capturer video et gif → « mode non construit », aucune capture' ((Compter-Motif 'DemandeCapture (video|gif)' $d0) -eq 0) | Out-Null
                $f = Lire-Reglages $d0
                $video = if ($f.raccourcis.PSObject.Properties.Name -contains 'video') { @($f.raccourcis.video) -join ',' } else { '<absent>' }
                $gif = if ($f.raccourcis.PSObject.Properties.Name -contains 'gif') { @($f.raccourcis.gif) -join ',' } else { '<absent>' }
                $mises = Compter-Motif 'reglages mis a niveau de la version 5 a la version 6$' $d0
                Verifier '(L0) réglages réécrits en v6 : video = Ctrl+Maj+6, gif = Ctrl+Maj+7, une ligne « de la version 5 a la version 6 »' (($f.version -eq 6) -and ($video -eq 'Ctrl+Maj+6') -and ($gif -eq 'Ctrl+Maj+7') -and ($mises -eq 1)) "version $($f.version), video $video, gif $gif, lignes $mises" | Out-Null
                Verifier '(L0) aucune fenêtre « film »' ((Compter-Motif 'film fenetre \d+' $d0) -eq 0) | Out-Null
                Fermer-Cas $script:proc $d0 '(L0)'
            }
            Arreter-Tout

            # ===================== Cas A : licence, non capturable ==========================
            $null = Ouvrir-Cas 'a' '(L1)'
            $da = $script:Cas.Dossier
            Point '(L1) avec licence' {
                $lv = Derniere-Ligne 'video disponible: ' $da
                Verifier '(L1) raccourci video: Ctrl+Maj+6, raccourci gif: Ctrl+Maj+7, video disponible: oui' (((Compter-Motif 'raccourci video: Ctrl\+Maj\+6$' $da) -eq 1) -and ((Compter-Motif 'raccourci gif: Ctrl\+Maj\+7$' $da) -eq 1) -and ($lv -match 'video disponible: oui$')) (Message $lv) | Out-Null
                while ([DateTimeOffset]::Now -lt (Heure-Ligne $lv).AddSeconds(10)) { Start-Sleep -Milliseconds 100 }
                $mods = @(Modules-App)
                $ref = @(Reference-Modules 'avec licence')
                $plus = @($mods | Where-Object { ($ref -notcontains $_) -and ($ONZE_F32 -notcontains $_) })
                $onze = @($ONZE_F32 | Where-Object { $mods -contains $_ })
                Verifier '(L1) modules 10 s après « video disponible » : rien hors de la référence 0.6.1 avec licence et des onze de F32' ($plus.Count -eq 0) "$($mods.Count) modules ; en plus : $($plus -join ', ') ; des onze : $($onze.Count) ($($onze -join ', '))" | Out-Null
                $script:ReposA = Memoire-Privee-Mo -Processus $script:proc
                $script:ReposAApp = Memoire-App
                Noter-Mesure -Nom '(L1) mémoire privée au repos avec licence, processus de l''app' -Valeur $script:ReposAApp -Unite 'Mo'
                Noter-Mesure -Nom '(L1) mémoire privée au repos avec licence (app et WebView2)' -Valeur $script:ReposA -Unite 'Mo'
                if ($null -ne $script:ReposL0App) {
                    Verifier '(L1) processus de l''app : au plus 3 Mo de plus que sans licence (recherche du codeur, §9)' (($script:ReposAApp - $script:ReposL0App) -le 3) "$($script:ReposAApp) Mo contre $($script:ReposL0App) Mo" | Out-Null
                }
                Verifier '(L1) aucune fenêtre « film » au repos' ((Compter-Motif 'film fenetre \d+' $da) -eq 0) | Out-Null
                $null = Attendre-Motif 'texte (langues|non pris en charge)' $da 30 '(L1) langues de lecture'
                $m = Ouvrir-Menu '(L1)'
                $t = @($m.Textes)
                $iv = [array]::IndexOf($t, "Vidéo`tCtrl + Maj + 6")
                $ok = ($iv -ge 2) -and ($t[$iv - 1] -eq '') -and ($t[$iv - 2].StartsWith('Copier le texte', [StringComparison]::Ordinal) -or $t[$iv - 2].StartsWith('Capture défilante', [StringComparison]::Ordinal)) -and ($t.Count -gt $iv + 2) -and ($t[$iv + 1] -ceq "GIF`tCtrl + Maj + 7") -and ($t[$iv + 2] -eq '')
                Verifier '(L1) menu : « Vidéo<tab>Ctrl + Maj + 6 », « GIF<tab>Ctrl + Maj + 7 » dans leur groupe, après « Copier le texte »' $ok $m.Lus | Out-Null
                Fermer-Menu-Icone $m '(L1)'
                # Réglages › Raccourcis : les lignes dans le même ordre.
                $vus = Compter-Motif 'menu: reglages$' $da
                $fen = Compter-Motif 'reglages fenetre \d+$' $da
                $pretes = Compter-Motif 'reglages page prete$' $da
                $m = Ouvrir-Menu '(L1) Réglages'
                Cliquer-Ligne-Menu $m 'Réglages' '(L1) Réglages'
                $script:Natif::Poser($REPOS_X, $REPOS_Y)
                $null = Attendre-Motif 'menu: reglages$' $da 5 '(L1) menu: reglages' ($vus + 1)
                $hr = Hwnd-Journal 'reglages fenetre' $da ($fen + 1)
                $null = Attendre-Motif 'reglages page prete$' $da 20 '(L1) page des Réglages' ($pretes + 1)
                Start-Sleep -Milliseconds 300
                $onglet = Element $hr 'Raccourcis' $CT::TabItem 5
                if ($null -eq $onglet) { throw "(L1) rubrique Raccourcis introuvable ; $(Elements-Vus $hr)" }
                $pt = Centre-Stable $onglet
                $script:Natif::Cliquer($pt[0], $pt[1])
                $script:Natif::Poser($REPOS_X, $REPOS_Y)
                $ys = @{}
                foreach ($nom in 'Capture défilante', 'Copier le texte', 'Vidéo', 'GIF', 'Historique') {
                    $el = Element $hr $nom $CT::Button 3
                    $ys[$nom] = if ($null -ne $el) { [int]$el.Current.BoundingRectangle.Y } else { -1 }
                }
                $ordre = ($ys['Vidéo'] -gt 0) -and ($ys['GIF'] -gt $ys['Vidéo']) -and ($ys['Vidéo'] -gt [math]::Max($ys['Copier le texte'], $ys['Capture défilante'])) -and (($ys['Historique'] -lt 0) -or ($ys['Historique'] -gt $ys['GIF']))
                Verifier '(L1) Réglages › Raccourcis : « Vidéo » puis « GIF », après « Copier le texte », avant « Historique »' $ordre "$(($ys.GetEnumerator() | Sort-Object Value | ForEach-Object { "$($_.Key) $($_.Value)" }) -join ' ; ')$(if (-not $ordre) { " ; $(Elements-Vus $hr)" })" | Out-Null
                $fermes = Compter-Motif 'reglages fermes$' $da
                $null = $script:Natif::PostMessage($hr, [uint32]0x10, [IntPtr]::Zero, [IntPtr]::Zero)
                $null = Attendre-Motif 'reglages fermes$' $da 10 '(L1) Réglages fermés' ($fermes + 1)
            }

            $script:Cas.Temoin = Lancer-Temoin 'a' 'simple'
            $sentinelle = Lancer-Temoin 'sentinelle' 'fixe' $SX $SY
            $tem = $script:Cas.Temoin
            if ([math]::Abs($tem.Echelle - 1.0) -gt 0.001 -or $tem.L -ne 600 -or $tem.H -ne 450) { throw "témoin : échelle $($tem.Echelle), zone $($tem.L)x$($tem.H) ; l'essai suppose un écran à 96 ppp" }
            # Régions de l'écran hors de la zone, lues avant et pendant un film : sous la zone
            # (la pilule), à droite de la zone, la sentinelle.
            $REG = @{ Bas = @(($TX - 10), ($TY + 450), 620, 80); Droite = @(($TX + 600), ($TY - 10), 18, 470); Sentinelle = @(($SX + 10), 10, 340, 280) }
            function Copier-Regions { $c = @{}; foreach ($k in $REG.Keys) { $r = $REG[$k]; $c[$k] = Copie-Ecran $r[0] $r[1] $r[2] $r[3] }; return $c }

            # (A1 à A6, A8) Une vidéo de 6 s, arrêtée par « Arrêter ».
            Point '(A) vidéo de 6 s' {
                $avant = Copier-Regions
                $fA = Faire-Film -Sorte video -Quoi '(A)' -DureeS 6 -Arret bouton -PendantDecompte {
                    param($F)
                    $F.Styles = $script:Natif::StylesEtendus($F.Hwnd)
                    $grp = Element $F.Hwnd 'Enregistrement' $CT::Group 2
                    $pause = Element $F.Hwnd 'Pause' $CT::Button 1
                    $arr = Element $F.Hwnd 'Arrêter' $CT::Button 1
                    $micro = Element $F.Hwnd 'Micro' $null 0.2
                    $F.UiaDecompte = [pscustomobject]@{ Groupe = ($null -ne $grp); PauseInactif = (($null -ne $pause) -and (-not $pause.Current.IsEnabled)); Arreter = ($null -ne $arr); Micro = ($null -ne $micro) }
                    if (-not ($F.UiaDecompte.Groupe -and $F.UiaDecompte.Arreter)) { $F.UiaVus = Elements-Vus $F.Hwnd }
                } -Pendant {
                    param($F)
                    $pause = Element $F.Hwnd 'Pause' $CT::Button 2
                    $F.PauseActif = ($null -ne $pause) -and $pause.Current.IsEnabled
                    $F.PremierPlan = $script:Natif::GetForegroundWindow()
                    $F.CopiesPendant = Copier-Regions
                    $vues = [System.Collections.Generic.List[string]]::new()
                    $jusqua = $F.DebutHeure.AddSeconds(3.3)
                    while ([DateTimeOffset]::Now -lt $jusqua) { $x = Duree-Lue $F.Hwnd; if ($x -and -not $vues.Contains($x)) { $vues.Add($x) }; Start-Sleep -Milliseconds 120 }
                    $F.DureesLues = @($vues)
                    # (A4) La souris 1 s sur la pastille jaune, puis dehors.
                    $script:Natif::Poser(($TX + $POINTEUR[0]), ($TY + $POINTEUR[1]))
                    Start-Sleep -Milliseconds 30
                    $F.SourisDe = Qpc-Maintenant
                    $F.PointeurMontre = $script:Natif::PointeurMontre()
                    Start-Sleep -Milliseconds 1000
                    $F.SourisA = Qpc-Maintenant
                    $script:Natif::Poser($REPOS_X, $REPOS_Y)
                }
                $script:FilmA = $fA
                # (A1)
                Verifier '(A1) Ctrl+Maj+6 : DemandeCapture video, sélection en mode film, film fenetre, film page prete' $true "$($fA.PagePrete)" | Out-Null
                # (A2)
                Verifier '(A2) film video zone = le témoin, agrandie non, image 600x450' ($fA.Zone -eq "film video zone $($tem.X),$($tem.Y) 600x450 agrandie non image 600x450") $fA.Zone | Out-Null
                $st = [long]$fA.Styles
                Verifier '(A2) fenêtre de contrôle : WS_EX_TOOLWINDOW, WS_EX_NOACTIVATE, sans WS_EX_APPWINDOW ; exclue des captures' ((($st -band $WS_EX.TOOLWINDOW) -ne 0) -and (($st -band $WS_EX.NOACTIVATE) -ne 0) -and (($st -band $WS_EX.APPWINDOW) -eq 0) -and ($fA.Exclue -eq 'film exclue des captures: oui')) ('styles 0x{0:X} ; {1}' -f $st, $fA.Exclue) | Out-Null
                Verifier '(A2) écran figé caché ≤ 450 ms après le relâchement' ($fA.FigeCacheMs -le 450) "$($fA.FigeCacheMs) ms (-1 : déjà caché)" | Out-Null
                Verifier '(A2) fenêtre de contrôle montrée ≤ 500 ms après le relâchement (page prête avant)' ($fA.ControleMontreMs -le 500) "$($fA.ControleMontreMs) ms ; $($fA.PagePrete)" | Out-Null
                Verifier '(A2) film decompte 3 s, bandeau decompte' ($fA.Decompte -eq 'film decompte 3 s') $fA.Decompte | Out-Null
                $u = $fA.UiaDecompte
                Verifier '(A2) pendant le décompte : groupe « Enregistrement », « Pause » inactif, « Arrêter », pas de « Micro »' ($u.Groupe -and $u.PauseInactif -and $u.Arreter -and -not $u.Micro) "groupe $($u.Groupe), pause inactif $($u.PauseInactif), arrêter $($u.Arreter), micro $($u.Micro) $($fA.UiaVus)" | Out-Null
                Verifier '(A2) le premier plan n''est jamais la fenêtre de contrôle' ($fA.PremierPlan -ne $fA.Hwnd) "avant $($script:Natif::Decrire($fA.PremierPlanAvant)) ; pendant $($script:Natif::Decrire($fA.PremierPlan))" | Out-Null
                # (A3)
                $src = Trouver-Apres 'film source ' $da $fA.Base
                $cod = Trouver-Apres 'film codeur ' $da $fA.Base
                $son = Trouver-Apres 'film son ' $da $fA.Base
                Verifier '(A3) film source duplication, film codeur logiciel …, film son non' (($null -ne $src) -and ((Message $src.Ligne) -eq 'film source duplication') -and ($null -ne $cod) -and ((Message $cod.Ligne) -match '^film codeur logiciel ') -and ($null -ne $son) -and ((Message $son.Ligne) -eq 'film son non')) "$(if ($src) { Message $src.Ligne }) ; $(if ($cod) { Message $cod.Ligne }) ; $(if ($son) { Message $son.Ligne })" | Out-Null
                Verifier-Echap '(A3)' $fA
                Verifier '(A3) « Pause » actif une fois le film commencé ; la durée lue passe par 0:01 puis 0:02' ($fA.PauseActif -and (@($fA.DureesLues) -contains '0:01') -and (@($fA.DureesLues) -contains '0:02')) "pause actif $($fA.PauseActif) ; lues : $(@($fA.DureesLues) -join ', ')" | Out-Null
                # (A5)
                Verifier '(A5) clic réel sur « Arrêter » : film fin bouton' ($fA.Cause -eq 'bouton') $fA.Fin | Out-Null
                $fin = Trouver-Apres 'etat Enregistrement -> Finalisation \(EnregistrementArrete\)$' $da $fA.Base
                Verifier '(A5) Enregistrement -> Finalisation, film fichier video, Finalisation -> Souffle (ImagePrete), sortie film copie faite fichier fait' (($null -ne $fin) -and ($fA.Etat -eq 'etat Finalisation -> Souffle (ImagePrete)') -and ($fA.Sortie -match '^sortie film copie faite fichier fait en ')) "$($fA.Fichier) ; $($fA.Etat) ; $($fA.Sortie)" | Out-Null
                Noter-Mesure -Nom '(A5) copie et enregistrement du film (hors du temps de la fin, F36)' -Valeur $fA.Sortie
                Verifier-Vignette '(A5)' $fA
                $ms = ($fA.VignetteHeure - $fA.ArretHeure).TotalMilliseconds
                Verifier '(A5) « Arrêter » → vignette montrée ≤ 2 000 ms' ($ms -le 2000) "$([math]::Round($ms)) ms" | Out-Null
                Verifier-Budgets '(A)' $fA
                # (A6)
                $lu = Verifier-Mp4 '(A6)' $fA -Plus ($POINTS_POINTEUR + $POINTS_BORDS)
                $exacts = 0
                foreach ($coul in $lu.Couleurs) { for ($q = 33; $q -lt 37; $q++) { $x = $coul[$q]; if ($null -ne $x -and ([int]$x[0] -ge 231 -and [int]$x[1] -ge 231 -and [int]$x[2] -ge 231)) { $exacts++ } } }
                Verifier '(A6) bords exacts de l''image (colonnes 0 et 599, rangées 0 et 449) : le blanc du témoin à 24 près' ($exacts -ge 4 * $lu.Couleurs.Count - 4) "$exacts points blancs sur $(4 * $lu.Couleurs.Count)" | Out-Null
                # Le pointeur : pendant sa seconde sur la pastille jaune, des pixels qui ne sont
                # pas ceux de la pastille ; plus après.
                $dans = 0; $nDans = 0; $apres = 0
                for ($i = 0; $i -lt $lu.Couleurs.Count; $i++) {
                    $q = $fA.DebutQpc + $lu.Ts[$i]
                    $autres = 0
                    for ($p = 29; $p -lt 33; $p++) { $x = $lu.Couleurs[$i][$p]; if ($null -ne $x -and ([math]::Abs([int]$x[2] - 60) -gt 60 -or [math]::Abs([int]$x[0] - 230) -gt 60)) { $autres++ } }
                    if ($q -ge $fA.SourisDe + 150 -and $q -le $fA.SourisA - 50) { $nDans++; if ($autres -ge 2) { $dans++ } }
                    elseif ($q -ge $fA.SourisA + 300) { if ($autres -ge 2) { $apres++ } }
                }
                if ($fA.PointeurMontre) {
                    Verifier '(A6) le pointeur dans les images pendant sa seconde sur la zone, plus après' (($nDans -gt 0) -and ($dans -ge [math]::Ceiling($nDans / 2)) -and ($apres -le 1)) "pendant : $dans images sur $nDans ; après : $apres" | Out-Null
                }
                else { Write-Host "[note] (A6) pointeur caché par Windows malgré les touches souris : non vérifiable (pendant : $dans sur $nDans, après : $apres)" ; Noter-Mesure -Nom '(A6) pointeur dans le film' -Valeur "non vérifiable : pointeur caché (pendant $dans sur $nDans, après $apres)" }
                # (A8)
                $null = Verifier-PressePapiers '(A8)' $fA '.mp4'
                Verifier '(A8) « Récents » : aucun fichier de plus ; <local>/Films vide (choix U4 : film déplacé)' ((@(Fichiers-De $script:Cas.Recents).Count -eq $fA.RecentsAvant) -and (@(Fichiers-De $script:Cas.Films).Count -eq 0)) "Récents $(@(Fichiers-De $script:Cas.Recents).Count) (avant $($fA.RecentsAvant)) ; Films $(@(Fichiers-De $script:Cas.Films).Count)" | Out-Null
                # (A9, négatif) La copie d'écran ne voit ni cadre, ni voile, ni pilule.
                $parts = @(foreach ($k in $REG.Keys) { "$k $([math]::Round($script:Natif::PartDifferente($avant[$k].O, $fA.CopiesPendant[$k].O, 6) * 100, 2)) %" })
                $max = 0.0; foreach ($k in $REG.Keys) { $max = [math]::Max($max, $script:Natif::PartDifferente($avant[$k].O, $fA.CopiesPendant[$k].O, 6)) }
                Verifier '(A9) sans la variable capturable, la copie d''écran ne voit pas la fenêtre de contrôle (autour de la zone, sous elle, sentinelle)' ($max -le 0.02) ($parts -join ' ; ') | Out-Null
            }

            # (B) GIF par Ctrl+Maj+7, 6 s.
            Point '(B) GIF de 6 s' {
                $fB = Faire-Film -Sorte gif -Quoi '(B)' -DureeS 6 -Arret bouton -PendantDecompte {
                    param($F)
                    $F.Micro = ($null -ne (Element $F.Hwnd 'Micro' $null 0.3))
                }
                $cod = Trouver-Apres 'film (codeur|son) ' $da $fB.Base
                Verifier '(B) GIF : ni film codeur ni film son ; jamais de « Micro »' (($null -eq $cod) -and (-not $fB.Micro)) "$(if ($cod) { Message $cod.Ligne })" | Out-Null
                Verifier '(B) film gif zone = le témoin, image 600x450 ; fin bouton ; sortie faite' (($fB.Zone -eq "film gif zone $($tem.X),$($tem.Y) 600x450 agrandie non image 600x450") -and ($fB.Cause -eq 'bouton') -and ($fB.Sortie -match '^sortie film copie faite fichier fait ')) "$($fB.Zone) ; $($fB.Fin) ; $($fB.Sortie)" | Out-Null
                Verifier-Echap '(B)' $fB
                Verifier-Budgets '(B)' $fB
                $null = Verifier-Gif '(B)' $fB
                Verifier-Vignette '(B)' $fB 'GIF · '
                $null = Verifier-PressePapiers '(B)' $fB '.gif'
            }
            Point '(B) GIF par le menu' {
                $fM = Faire-Film -Sorte gif -Quoi '(B menu)' -Par menu -DureeS 2 -Arret raccourci
                Verifier '(B) « GIF » cliqué au menu : menu: capturer gif, film fin bouton (Ctrl+Maj+7 l''arrête), un .gif enregistré' (($fM.Cause -eq 'bouton') -and $fM.Enregistre) "$($fM.Fin) ; $($fM.Sortie)" | Out-Null
            }

            # (C) Pause, vidéo puis GIF.
            foreach ($sorte in 'video', 'gif') {
                Point "(C) pause ($sorte)" {
                    $fC = Faire-Film -Sorte $sorte -Quoi "(C $sorte)" -DureeS 0 -Arret aucun -Pendant {
                        param($F)
                        while ([DateTimeOffset]::Now -lt $F.DebutHeure.AddSeconds(2)) { Start-Sleep -Milliseconds 20 }
                        $F.PauseQpc = Cliquer-Pilule $F 'Pause' "$($F.Quoi) : Pause"
                        $lp = Attendre-Apres 'film pause$' $F.Dossier $F.Base 3 "$($F.Quoi) : film pause"
                        $F.Reprendre = $null -ne (Element $F.Hwnd 'Reprendre' $CT::Button 2)
                        Start-Sleep -Milliseconds 300
                        $d1 = Duree-Lue $F.Hwnd
                        Start-Sleep -Milliseconds 3000
                        $d2 = Duree-Lue $F.Hwnd
                        $F.DureesPause = "$d1 → $d2"
                        $F.DureeFigee = ($d1 -ne '') -and ($d1 -eq $d2)
                        $F.RepriseQpc = Cliquer-Pilule $F 'Reprendre' "$($F.Quoi) : Reprendre"
                        $lr = Attendre-Apres 'film reprise$' $F.Dossier $F.Base 3 "$($F.Quoi) : film reprise"
                        $F.PauseMs = (Qpc-Ligne $lr.Ligne) - (Qpc-Ligne $lp.Ligne)
                        Start-Sleep -Milliseconds 2000
                        $F.ArretQpc = Cliquer-Pilule $F 'Arrêter' "$($F.Quoi) : Arrêter"
                        $F.ArretHeure = [DateTimeOffset]::Now
                    }
                    $q = $fC.Quoi
                    Verifier "$q film pause, « Reprendre » lu, la durée lue ne change pas pendant 3 s, film reprise" ($fC.Reprendre -and $fC.DureeFigee) "durée $($fC.DureesPause)" | Out-Null
                    $attendu = ($fC.ArretQpc - $fC.DebutQpc) - $fC.PauseMs
                    $a = if ($sorte -eq 'video') { Relire-Mp4 $fC.Enregistre $q } else { Relire-Gif $fC.Enregistre $q }
                    Verifier "$q durée du fichier = durée totale moins la pause, à 300 ms près" ([math]::Abs($a.DureeMs - $attendu) -le 300) "fichier $($a.DureeMs) ms ; attendu $([math]::Round($attendu)) ms (pause $([math]::Round($fC.PauseMs)) ms) ; $($fC.Fin)" | Out-Null
                    $lus = @($a.Ks | Where-Object { $_ -ge 0 })
                    $saut = 0; for ($i = 1; $i -lt $lus.Count; $i++) { $saut = [math]::Max($saut, $lus[$i] - $lus[$i - 1]) }
                    $mini = [math]::Floor($fC.PauseMs / 100) - 2
                    Verifier "$q dans les k, un saut d'au moins la longueur de la pause" ($saut -ge $mini) "plus grand saut $saut pas de 100 ms (au moins $mini)" | Out-Null
                }
            }

            # (D) Annulations.
            foreach ($cause in 'echap', 'croix', 'arreter', 'session') {
                Point "(D) annulation ($cause)" {
                    $fD = Faire-Film -Sorte video -Quoi "(D $cause)" -Annuler $cause
                    Start-Sleep -Milliseconds 300
                    $limite = [DateTime]::UtcNow.AddSeconds(3)
                    while (@(Visibles-App).Count -gt 0 -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 100 }
                    $vis = @(Visibles-App)
                    $films = @(Fichiers-De $script:Cas.Films)
                    Verifier "(D) $cause pendant le décompte : film annule decompte ($cause), EnregistrementAnnule, aucune fenêtre de l'app visible, aucun fichier dans Films" (($fD.Annule -eq "film annule decompte ($cause)") -and ($vis.Count -eq 0) -and ($films.Count -eq 0)) "$($fD.Annule) ; visibles : $($vis -join ', ') ; Films : $($films.Count)" | Out-Null
                }
            }
            Point '(D) Échap pendant la visée' {
                $fV = Faire-Film -Sorte gif -Quoi '(D visée)' -Annuler visee
                $limite = [DateTime]::UtcNow.AddSeconds(3)
                while ($script:Natif::IsWindow($fV.Hwnd) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 50 }
                Verifier '(D) Échap pendant la visée : sélection annulée, fenêtre film détruite, aucun fichier dans Films' ((-not $script:Natif::IsWindow($fV.Hwnd)) -and (@(Fichiers-De $script:Cas.Films).Count -eq 0)) "$($fV.Annule) ; fenêtre $(if ($script:Natif::IsWindow($fV.Hwnd)) { 'encore là' } else { 'détruite' })" | Out-Null
            }

            # (E) Gardes.
            Point '(E) gardes pendant une vidéo' {
                $fE = Faire-Film -Sorte video -Quoi '(E vidéo)' -DureeS 0 -Arret aucun -Pendant {
                    param($F)
                    Start-Sleep -Milliseconds 800
                    $dos = $F.Dossier
                    Appuyer-Raccourci 'plein_ecran' $VK_2 '(E) Ctrl+Maj+2'
                    $null = Attendre-Apres 'raccourci presse: plein_ecran$' $dos $F.Base 3 '(E) raccourci presse: plein_ecran'
                    $null = Attendre-Apres 'capture deja en cours: appui ignore$' $dos $F.Base 3 '(E) Ctrl+Maj+2 : appui ignore'
                    Appuyer-Film 'gif' '(E) Ctrl+Maj+7'
                    $null = Attendre-Apres 'raccourci presse: gif$' $dos $F.Base 3 '(E) raccourci presse: gif'
                    $null = Attendre-Apres 'capture deja en cours: appui ignore$' $dos $F.Base 3 '(E) Ctrl+Maj+7 : appui ignore' 2
                    $choisis = Compter-Motif '^\S+\s+\S+\s+\S+\s+menu: ' $dos
                    $mq = Ouvrir-Menu '(E) menu pendant le film'
                    $null = Attendre-Apres 'menu pendant le film: quitter seulement$' $dos $F.Base 3 '(E) menu pendant le film: quitter seulement'
                    $F.MenuSeul = (@($mq.Textes).Count -eq 1) -and ($mq.Textes[0] -ceq 'Quitter BreezyShot')
                    $F.MenuLu = $mq.Lus
                    Fermer-Menu-Par-Icone $mq '(E) menu pendant le film'
                    $F.MenuSansChoix = (Compter-Motif '^\S+\s+\S+\s+\S+\s+menu: ' $dos) -eq $choisis
                    $F.FinAvant = $null -ne (Trouver-Apres 'film fin ' $dos $F.Base)
                    $F.ArretQpc = Qpc-Maintenant
                    $F.ArretHeure = [DateTimeOffset]::Now
                    Appuyer-Film 'video' '(E) Ctrl+Maj+6'
                }
                Verifier '(E) Ctrl+Maj+2 et Ctrl+Maj+7 pendant une vidéo : appui ignore, le film continue' (-not $fE.FinAvant) | Out-Null
                Verifier '(E) menu de l''icône pendant le film : « Quitter BreezyShot » seul, refermé sans rien choisir' ($fE.MenuSeul -and $fE.MenuSansChoix) $fE.MenuLu | Out-Null
                Verifier '(E) Ctrl+Maj+6 pendant la vidéo l''arrête (choix B1) : film fin bouton' (($fE.Cause -eq 'bouton') -and ($null -ne (Trouver-Apres 'raccourci presse: video$' $da $fE.Base))) $fE.Fin | Out-Null
            }
            Point '(E) gardes pendant un GIF' {
                $fG = Faire-Film -Sorte gif -Quoi '(E GIF)' -DureeS 0 -Arret aucun -Pendant {
                    param($F)
                    Start-Sleep -Milliseconds 800
                    Appuyer-Film 'video' '(E) Ctrl+Maj+6 pendant le GIF'
                    $null = Attendre-Apres 'raccourci presse: video$' $F.Dossier $F.Base 3 '(E) raccourci presse: video'
                    $null = Attendre-Apres 'capture deja en cours: appui ignore$' $F.Dossier $F.Base 3 '(E) Ctrl+Maj+6 pendant le GIF : appui ignore'
                    Start-Sleep -Milliseconds 300
                    $F.FinAvant = $null -ne (Trouver-Apres 'film fin ' $F.Dossier $F.Base)
                    $F.ArretQpc = Qpc-Maintenant
                    $F.ArretHeure = [DateTimeOffset]::Now
                    Appuyer-Film 'gif' '(E) Ctrl+Maj+7'
                }
                Verifier '(E) Ctrl+Maj+6 pendant un GIF : appui ignore ; Ctrl+Maj+7 l''arrête : film fin bouton' ((-not $fG.FinAvant) -and ($fG.Cause -eq 'bouton')) $fG.Fin | Out-Null
            }

            # (F) Zones.
            Point '(F) zone de 80 × 60' {
                $x0 = $tem.X + 100; $y0 = $tem.Y + 100
                $fF = Faire-Film -Sorte video -Quoi '(F 80x60)' -Trace @($x0, $y0, ($x0 + 79), ($y0 + 59)) -DureeS 1 -Arret raccourci
                Verifier '(F) zone tracée de 80 × 60 : agrandie oui, image 120 × 120' ($fF.Zone -eq "film video zone $x0,$y0 80x60 agrandie oui image 120x120") $fF.Zone | Out-Null
                $null = Verifier-Mp4 '(F 80x60)' $fF 120 120 -SansTemoin
            }
            foreach ($sorte in 'video', 'gif') {
                Point "(F) zone de 601 × 451 ($sorte)" {
                    $fZ = Faire-Film -Sorte $sorte -Quoi "(F 601x451 $sorte)" -Trace @($tem.X, $tem.Y, ($tem.X + 600), ($tem.Y + 450)) -DureeS 1 -Arret raccourci
                    $img = if ($sorte -eq 'video') { '600x450' } else { '601x451' }
                    Verifier "(F) zone de 601 × 451 ($sorte) : image $img" ($fZ.Zone -eq "film $sorte zone $($tem.X),$($tem.Y) 601x451 agrandie non image $img") $fZ.Zone | Out-Null
                    $l, $h = $img -split 'x' | ForEach-Object { [int]$_ }
                    if ($sorte -eq 'video') { $null = Verifier-Mp4 "(F 601x451 $sorte)" $fZ $l $h -SansTemoin } else { $null = Verifier-Gif "(F 601x451 $sorte)" $fZ $l $h -SansTemoin }
                }
            }

            # (I) Cinq films de suite : mémoire, fils, poignées.
            Point '(I) cinq films de suite' {
                $suite = [System.Collections.Generic.List[object]]::new()
                for ($n = 1; $n -le 5; $n++) {
                    $fS = Faire-Film -Sorte video -Quoi "(I suite $n)" -DureeS 1 -Arret raccourci
                    Start-Sleep -Milliseconds 800
                    $script:proc.Refresh()
                    $suite.Add([pscustomobject]@{ Mo = (Memoire-Privee-Mo -Processus $script:proc); Fils = $script:proc.Threads.Count; Poignees = $script:proc.HandleCount })
                }
                $txt = ($suite | ForEach-Object { "$($_.Mo) Mo, $($_.Fils) fils, $($_.Poignees) poignées" }) -join ' ; '
                Noter-Mesure -Nom '(I) cinq films de suite' -Valeur $txt
                $dm = $suite[4].Mo - $suite[0].Mo; $df = $suite[4].Fils - $suite[0].Fils; $dp = $suite[4].Poignees - $suite[0].Poignees
                Verifier '(I) cinq films de suite : mémoire stable (≤ 2 Mo par film), fils et poignées stables' (($dm -le 8) -and ($df -le 2) -and ($dp -le 50)) "mémoire $dm Mo, fils $df, poignées $dp du premier au cinquième" | Out-Null
            }
            # (I) Vidéo de 30 s : processeur et mémoire ; 10 s après la vignette partie.
            Point '(I) vidéo de 30 s' {
                $fI = Faire-Film -Sorte video -Quoi '(I vidéo 30 s)' -DureeS 30 -Arret bouton -Pendant {
                    param($F)
                    Start-Sleep -Seconds 5
                    $F.Mesure = Mesurer-Pendant 22
                }
                $m = $fI.Mesure
                $plafond = $script:ReposA + 70 + 60 + 6 * (600 * 450 * 4 / 1MB)
                Noter-Mesure -Nom '(I) vidéo de 30 s : processeur de l''app, mémoire (pic app et WebView2 ; app seule)' -Valeur "$($m.Cpu) % d'un cœur ; $($m.Pic) Mo ; $($m.PicApp) Mo"
                Verifier '(I) vidéo de 30 s : processeur de l''app ≤ 50 % d''un cœur' ($m.Cpu -le 50) "$($m.Cpu) %" | Out-Null
                Verifier '(I) vidéo de 30 s : mémoire ≤ repos + 70 + 60 Mo + 6 images RGBA' ($m.Pic -le $plafond) "$($m.Pic) Mo (plafond $([math]::Round($plafond, 1)) Mo, repos $($script:ReposA) Mo)" | Out-Null
                Verifier-Budgets '(I vidéo 30 s)' $fI
                $null = Verifier-Mp4 '(I vidéo 30 s)' $fI
                $fermees = Compter-Motif "vignette fermee $($fI.Vignette) " $da
                Cliquer-Vignette $fI.Vignette 'fermer' '(I) Fermer la vignette'
                $null = Attendre-Motif "vignette fermee $($fI.Vignette) " $da 5 '(I) vignette fermée' ($fermees + 1)
                Start-Sleep -Seconds 10
                $apres = Memoire-Privee-Mo -Processus $script:proc
                Verifier '(I) 10 s après la vignette partie : mémoire ≤ repos + 10 Mo' ($apres -le $script:ReposA + 10) "$apres Mo (repos $($script:ReposA) Mo)" | Out-Null
            }
            # (I et F) GIF laissé à la limite de 60 s (témoin simple) : processeur et mémoire
            # sur 25 s, puis `film fin limite duree 60000`, sans message.
            Point '(F) GIF jusqu''à la limite' {
                $fL = Faire-Film -Sorte gif -Quoi '(F limite GIF)' -DureeS 0 -Arret aucun -FinDelaiS 75 -Pendant {
                    param($F)
                    Start-Sleep -Seconds 5
                    $F.Mesure = Mesurer-Pendant 25
                }
                $m = $fL.Mesure
                $plafond = $script:ReposA + 70 + 16 + 8 * (600 * 450 * 4 / 1MB)
                Noter-Mesure -Nom '(I) GIF : processeur de l''app, mémoire (pic app et WebView2 ; app seule)' -Valeur "$($m.Cpu) % d'un cœur ; $($m.Pic) Mo ; $($m.PicApp) Mo"
                Verifier '(I) GIF : processeur de l''app ≤ 30 % d''un cœur' ($m.Cpu -le 30) "$($m.Cpu) %" | Out-Null
                Verifier '(I) GIF : mémoire ≤ repos + 70 + 16 Mo + 8 images RGBA' ($m.Pic -le $plafond) "$($m.Pic) Mo (plafond $([math]::Round($plafond, 1)) Mo)" | Out-Null
                $msg = @(Toutes-Apres 'bandeau affiche ' $da $fL.Base)
                Verifier '(F) GIF laissé aller : film fin limite duree 60000 à 300 ms près, sans message' (($fL.Cause -eq 'limite') -and ([math]::Abs($fL.Duree - 60000) -le 300) -and ($msg.Count -eq 0)) "$($fL.Fin) ; messages $($msg.Count)" | Out-Null
                Verifier-Budgets '(F limite GIF)' $fL
                $null = Verifier-Gif '(F limite GIF)' $fL
            }

            # (K) Fins sans geste : session, veille, écrans.
            Point '(K1) session verrouillée' {
                $fK = Faire-Film -Sorte video -Quoi '(K1)' -DureeS 0 -Arret aucun -Pendant {
                    param($F)
                    Start-Sleep -Seconds 2
                    $F.ArretQpc = Qpc-Maintenant
                    $null = $script:Natif::PostMessage($script:EcranFige, [uint32]0x2B1, [IntPtr]7, [IntPtr]::Zero)
                }
                Verifier '(K1) WM_WTSSESSION_CHANGE (verrouillage) : film fin session, vignette' (($fK.Cause -eq 'session') -and ($fK.Vignette -ge 0)) "$($fK.Fin) ; $($fK.Etat)" | Out-Null
                $null = Verifier-Mp4 '(K1)' $fK
            }
            Point '(K2) veille annoncée' {
                $fK = Faire-Film -Sorte video -Quoi '(K2)' -DureeS 0 -Arret aucun -Pendant {
                    param($F)
                    Start-Sleep -Seconds 2
                    $F.ArretQpc = Qpc-Maintenant
                    $null = $script:Natif::PostMessage($script:EcranFige, [uint32]0x218, [IntPtr]4, [IntPtr]::Zero)
                }
                Verifier '(K2) WM_POWERBROADCAST (PBT_APMSUSPEND) : film fin veille' ($fK.Cause -eq 'veille') $fK.Fin | Out-Null
                $null = Verifier-Mp4 '(K2)' $fK -MargeMs 700
            }
            Point '(K3) écrans changés' {
                try {
                    $fK = Faire-Film -Sorte video -Quoi '(K3)' -DureeS 0 -Arret aucun -Pendant {
                        param($F)
                        Start-Sleep -Seconds 2
                        $F.ArretQpc = Qpc-Maintenant
                        $F.CodeMode = $script:Natif::ChangerMode(800, 600)
                    }
                    Verifier '(K3) mode passé à 800 × 600 : film fin ecrans' (($fK.CodeMode -eq 0) -and ($fK.Cause -eq 'ecrans')) "code $($fK.CodeMode) ; $($fK.Fin)" | Out-Null
                }
                finally {
                    if ($script:Natif::ModeCourant() -ne $script:ModeDepart) { [void]$script:Natif::RemettreMode() }
                    $limite = [DateTime]::UtcNow.AddSeconds(10)
                    while ($script:Natif::ModeCourant() -ne $script:ModeDepart -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 200 }
                    Write-Host "(K3) mode remis : $($script:Natif::ModeCourant())"
                    Start-Sleep -Seconds 2
                }
                Verifier '(K3) le mode d''affichage remis' ($script:Natif::ModeCourant() -eq $script:ModeDepart) $script:Natif::ModeCourant() | Out-Null
                $null = Verifier-Mp4 '(K3)' $fK -SansTemoin
            }
            Arreter-Temoin $sentinelle
            Arreter-Temoin $script:Cas.Temoin

            # (A7) Fraîcheur : témoin -Rapide, vidéo de 10 s.
            $script:Cas.Temoin = Lancer-Temoin 'rapide' 'simple' -Rapide
            Point '(A7) fraîcheur' {
                $fR = Faire-Film -Sorte video -Quoi '(A7)' -DureeS 10 -Arret raccourci
                $a = Relire-Mp4 $fR.Enregistre '(A7)'
                $pas = Pas-Temoin $script:Cas.Temoin
                $rythme = Rythme-Temoin $pas $fR.DebutQpc ($fR.DebutQpc + $fR.Duree)
                $secondes = [math]::Floor($a.DureeMs / 1000)
                $distincts = [System.Collections.Generic.List[int]]::new()
                for ($s = 0; $s -lt $secondes; $s++) {
                    $vus = [System.Collections.Generic.HashSet[int]]::new()
                    for ($i = 0; $i -lt $a.Ks.Count; $i++) { if ($a.Ts[$i] -ge 1000 * $s -and $a.Ts[$i] -lt 1000 * ($s + 1) -and $a.Ks[$i] -ge 0) { [void]$vus.Add($a.Ks[$i]) } }
                    $distincts.Add($vus.Count)
                }
                $moyenne = if ($distincts.Count) { ($distincts | Measure-Object -Average).Average } else { 0 }
                $seuil = 0.8 * [math]::Min(30, $rythme)
                Noter-Mesure -Nom '(A7) valeurs k distinctes par seconde (témoin -Rapide)' -Valeur "$([math]::Round($moyenne, 2)) (par seconde : $($distincts -join ', ')) ; témoin $([math]::Round($rythme, 2)) pas/s"
                Verifier '(A7) fraîcheur : valeurs k distinctes par seconde ≥ 80 % de la cadence (ou du rythme du témoin)' ($moyenne -ge $seuil) "$([math]::Round($moyenne, 2)) contre $([math]::Round($seuil, 2))" | Out-Null
                Noter-Mesure -Nom '(A7) taille du fichier, témoin -Rapide, 10 s' -Valeur ('{0:0.000}' -f ($a.Octets / 1MB)) -Unite 'Mo'
            }
            Arreter-Temoin $script:Cas.Temoin

            # (B) Témoin -Fixe : GIF de 3 s ; puis (E) --quitter pendant une vidéo.
            $script:Cas.Temoin = Lancer-Temoin 'fixe' 'fixe'
            Point '(B) GIF du témoin fixe' {
                $fX = Faire-Film -Sorte gif -Quoi '(B fixe)' -DureeS 3 -Arret raccourci
                $a = Relire-Gif $fX.Enregistre '(B fixe)'
                Verifier '(B) témoin -Fixe, 3 s : une ou deux images, délais de 3 s (à la durée près), moins de 100 Ko' (($a.Images -ge 1) -and ($a.Images -le 2) -and ([math]::Abs($a.DureeMs - $fX.Duree) -le 100) -and ([math]::Abs($fX.Duree - 3000) -le 300) -and ($a.Octets -lt 100KB)) "$($a.Images) images, $($a.DureeMs) ms (film $($fX.Duree) ms), $($a.Octets) octets" | Out-Null
            }
            Point '(E) --quitter pendant une vidéo' {
                # Correction B1 : le film arrêté par --quitter est rangé dans le dossier
                # d'enregistrement, au nom du modèle, sans copie (même disque ici) ; ni
                # presse-papiers, ni vignette, ni ligne `sortie film`.
                $seq = $script:Natif::GetClipboardSequenceNumber()
                $imagesAvant = @(Fichiers-De $script:Cas.Images '*.mp4')
                $filmsAvant = @(Fichiers-De $script:Cas.Films '*.mp4')
                $fQ = Faire-Film -Sorte video -Quoi '(E quitter)' -DureeS 0 -Arret aucun -SansSortie -Pendant {
                    param($F)
                    Start-Sleep -Milliseconds 1500
                    $F.QuitterHeure = [DateTimeOffset]::Now
                    $F.Quitter = Quitter-App -DelaiS 10 -Dossier $F.Dossier
                }
                $sorti = $script:proc.WaitForExit(6000)
                $la = Attendre-Apres 'arret propre$' $da $fQ.Base 3 '(E) arret propre'
                $ferme = Trouver-Apres 'film arret: fichier ferme en \d+ ms$' $da $fQ.Base
                $range = Trouver-Apres 'film arret: fichier (range$|a ranger au prochain lancement)' $da $fQ.Base
                $ms = ((Heure-Ligne $la.Ligne) - $fQ.QuitterHeure).TotalMilliseconds
                Verifier '(E) --quitter pendant le film : film fin arret, film arret: fichier ferme, arret propre en moins de 4 s' (($fQ.Cause -eq 'arret') -and ($null -ne $ferme) -and $sorti -and ($ms -lt 4000)) "$($fQ.Fin) ; $(if ($ferme) { Message $ferme.Ligne }) ; arret propre $([math]::Round($ms)) ms après --quitter" | Out-Null
                $neufs = @(Fichiers-De $script:Cas.Images '*.mp4' | Where-Object { $imagesAvant -notcontains $_ })
                $restes = @(Fichiers-De $script:Cas.Films '*.mp4' | Where-Object { $filmsAvant -notcontains $_ })
                $registre = Join-Path $script:Cas.Films 'a-ranger.txt'
                $note = if (Test-Path -LiteralPath $registre) { (Get-Content -LiteralPath $registre -Raw).Trim() } else { '' }
                $nom = if ($neufs.Count -eq 1) { Split-Path -Leaf $neufs[0] } else { '' }
                Verifier '(E) film arret: fichier range ; un .mp4 de plus dans le dossier d''enregistrement, au nom du modèle ; rien de plus dans Films, rien dans a-ranger.txt' (($null -ne $range) -and ((Message $range.Ligne) -eq 'film arret: fichier range') -and ($neufs.Count -eq 1) -and $nom.StartsWith('BreezyShot ', [StringComparison]::Ordinal) -and ($restes.Count -eq 0) -and ($note -eq '')) "$(if ($range) { Message $range.Ligne } else { 'aucune ligne de rangement' }) ; dossier : $($neufs.Count) nouveau(x) $nom ; Films : $($restes.Count) ; a-ranger.txt : « $note »" | Out-Null
                $sortie = @(Toutes-Apres 'sortie film ' $da $fQ.Base)
                Verifier '(E) aucune ligne sortie film, presse-papiers inchangé' (($sortie.Count -eq 0) -and ($script:Natif::GetClipboardSequenceNumber() -eq $seq)) "sortie $($sortie.Count)" | Out-Null
                if ($neufs.Count -ne 1) { throw "(E) $($neufs.Count) film(s) nouveau(x) dans le dossier d'enregistrement" }
                $a = Relire-Mp4 $neufs[0] '(E quitter)'
                Verifier '(E) le film rangé, relu, est entier (durée du film à 200 ms près)' ([math]::Abs($a.DureeMs - $fQ.Duree) -le 200) "$($a.DureeMs) ms, $($a.Images) images ; film $($fQ.Duree) ms" | Out-Null
            }
            Arreter-Temoin $script:Cas.Temoin
            Verifier-Interdites $da '(A)'
            Arreter-Tout

            # ===================== Cas A9 : capturable =====================================
            $null = Ouvrir-Cas 'a9' '(A9)' -Capturable
            $script:Cas.Temoin = Lancer-Temoin 'a9' 'simple'
            $sentinelle = Lancer-Temoin 'sentinelle-a9' 'fixe' $SX $SY
            $tem = $script:Cas.Temoin
            Point '(A9) fenêtre de contrôle peinte' {
                $avant = Copier-Regions
                $avantZone = Copie-Ecran ($tem.X - 8) ($tem.Y - 8) 616 466
                $fP = Faire-Film -Sorte video -Quoi '(A9)' -DureeS 3 -Arret raccourci -Pendant {
                    param($F)
                    Start-Sleep -Milliseconds 800
                    $F.Copies = Copier-Regions
                    $F.CopieZone = Copie-Ecran ($tem.X - 8) ($tem.Y - 8) 616 466
                    $grp = Element $F.Hwnd 'Enregistrement' $CT::Group 2
                    $F.Pilule = if ($null -ne $grp) { $grp.Current.BoundingRectangle } else { $null }
                    if ($null -ne $F.Pilule) { $r = $F.Pilule; $F.CopiePilule = Copie-Ecran ([int]$r.X + 2) ([int]$r.Y + 2) ([int]$r.Width - 4) ([int]$r.Height - 4) }
                }
                Verifier '(A9) film exclue des captures: non (essai)' ($fP.Exclue -eq 'film exclue des captures: non (essai)') $fP.Exclue | Out-Null
                # Le cadre : 2 px blancs à 2 et 3 px hors de la zone (milieu des quatre côtés).
                $z = $fP.CopieZone
                $o = 8
                $blancs = 0; $vus = @()
                foreach ($p in @(@(($o - 2), ($o + 225)), @(($o - 3), ($o + 225)), @(($o + 600 + 1), ($o + 225)), @(($o + 600 + 2), ($o + 225)), @(($o + 300), ($o - 2)), @(($o + 300), ($o - 3)), @(($o + 300), ($o + 450 + 1)), @(($o + 300), ($o + 450 + 2)))) {
                    $c = Pixel-Copie $z $p[0] $p[1]; $vus += ($c -join ',')
                    if ($c[0] -ge 245 -and $c[1] -ge 245 -and $c[2] -ge 245) { $blancs++ }
                }
                Verifier '(A9) cadre blanc de 2 px hors de la zone (quatre côtés)' ($blancs -eq 8) "points : $($vus -join ' ; ')" | Out-Null
                $dedans = $script:Natif::PartVoilee($avantZone.O, $z.O, 616, 0.0, 6, 4)
                Noter-Mesure -Nom '(A9) la zone et ses abords inchangés à ± 6 (témoin animé compris)' -Valeur "$([math]::Round($dedans * 100, 1)) %"
                $v = $script:Natif::PartVoilee($avant['Sentinelle'].O, $fP.Copies['Sentinelle'].O, $avant['Sentinelle'].L, 0.18, 6, 2)
                Verifier '(A9) voile sur le reste de l''écran : la sentinelle assombrie de 18 % à ± 6' ($v -ge 0.9) "$([math]::Round($v * 100, 1)) % des points" | Out-Null
                $pil = $fP.CopiePilule
                $fond = 0
                if ($null -ne $pil) { for ($i = 0; $i + 3 -lt $pil.O.Length; $i += 4) { if ([math]::Abs($pil.O[$i] - 249) -le 4 -and [math]::Abs($pil.O[$i + 1] - 249) -le 4 -and [math]::Abs($pil.O[$i + 2] - 249) -le 4) { $fond++ } } }
                $part = if ($null -ne $pil) { $fond / ($pil.O.Length / 4) } else { 0 }
                $place = ($null -ne $fP.Pilule) -and ($fP.Pilule.Y -ge $tem.Y + 450) -and ($fP.Pilule.Y -le $tem.Y + 450 + 40) -and ($fP.Pilule.Right -le $tem.X + 600 + 2)
                Verifier '(A9) pilule peinte sous la zone, à droite (fond #F9F9F9)' ($place -and ($part -ge 0.3)) "pilule $($fP.Pilule) ; fond $([math]::Round($part * 100, 1)) %" | Out-Null
                $null = Verifier-Mp4 '(A9)' $fP
            }
            Arreter-Temoin $sentinelle
            Arreter-Temoin $script:Cas.Temoin
            Verifier-Interdites $script:Cas.Dossier '(A9)'
            try { Fermer-Cas $script:proc $script:Cas.Dossier '(A9)' } catch { Write-Host "$_" }
            Arreter-Tout

            # ===================== Cas F125 : échelle de 125 % =============================
            Point '(F125) échelle de 125 %' {
                $er = @($script:Natif::EchelleRelative())
                if ($er[0] -ne 0 -or $er[3] -le $er[2]) { Noter-Mesure -Nom '(F125)' -Valeur "non joué : aucun pas d'échelle au-dessus (code $($er[0]), actuel $($er[2]), max $($er[3]))"; return }
                $script:EchelleDepart = $er[2]
                $c = $script:Natif::FixerEchelleRelative($er[2] + 1)
                if ($c -ne 0) { Noter-Mesure -Nom '(F125)' -Valeur "non joué : DisplayConfigSetDeviceInfo refuse (code $c ; relatif actuel $($er[2]), max $($er[3]))"; return }
                # Le DPI lu par ce processus (lancé avant le changement) peut rester ancien : on
                # attend 10 s au plus, puis c'est un processus neuf, le témoin, qui juge.
                $limite = [DateTime]::UtcNow.AddSeconds(10)
                while ($script:Natif::DpiEcran(1, 1) -ne 120 -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 200 }
                $dpi = $script:Natif::DpiEcran(1, 1)
                $apres = @($script:Natif::EchelleRelative())
                Start-Sleep -Seconds 1
                $tem = Lancer-Temoin 'f125' 'simple'
                $rect = @($script:Natif::Rect($tem.Hwnd))
                if ([math]::Abs($tem.Echelle - 1.25) -gt 0.01 -or $rect[2] -ne 600 -or $rect[3] -ne 450) {
                    Arreter-Temoin $tem
                    Noter-Mesure -Nom '(F125)' -Valeur "non joué : échelle non suivie (code $c, DPI lu ici $dpi, relatif lu $($apres[2]) ; témoin neuf : échelle $($tem.Echelle), fenêtre $($rect -join ','))"
                    return
                }
                Noter-Mesure -Nom '(F125) échelle suivie' -Valeur "DPI lu ici $dpi ; témoin neuf $($tem.Echelle)"
                $null = Ouvrir-Cas 'f125' '(F125)' -Capturable
                $script:Cas.Temoin = $tem
                $fS = Faire-Film -Sorte video -Quoi '(F125)' -DureeS 5 -Arret raccourci -Pendant {
                    param($F)
                    Start-Sleep -Milliseconds 800
                    $F.CopieZone = Copie-Ecran ($tem.X - 8) ($tem.Y - 8) 616 466
                    $grp = Element $F.Hwnd 'Enregistrement' $CT::Group 2
                    $F.Pilule = if ($null -ne $grp) { $grp.Current.BoundingRectangle } else { $null }
                }
                Verifier '(F125) zone et image en pixels réels au journal' ($fS.Zone -eq "film video zone $($tem.X),$($tem.Y) 600x450 agrandie non image 600x450") $fS.Zone | Out-Null
                $blancs = 0
                foreach ($p in @(@(6, 233), @(8 + 600 + 1, 233), @(308, 6), @(308, 8 + 450 + 1))) { $c = Pixel-Copie $fS.CopieZone $p[0] $p[1]; if ($c[0] -ge 240 -and $c[1] -ge 240 -and $c[2] -ge 240) { $blancs++ } }
                $place = ($null -ne $fS.Pilule) -and ($fS.Pilule.Y -ge $tem.Y + 450) -and ($fS.Pilule.Y -le $tem.Y + 450 + 50) -and ($fS.Pilule.Right -le $tem.X + 600 + 3)
                Verifier '(F125) cadre hors de la zone, pilule sous la zone à droite' (($blancs -eq 4) -and $place) "cadre $blancs/4 ; pilule $($fS.Pilule)" | Out-Null
                $null = Verifier-Mp4 '(F125)' $fS
                Arreter-Temoin $script:Cas.Temoin
                Verifier-Interdites $script:Cas.Dossier '(F125)'
                Fermer-Cas $script:proc $script:Cas.Dossier '(F125)'
            }
            Arreter-Tout
            if ($null -ne $script:EchelleDepart) {
                $x = @($script:Natif::EchelleRelative())
                if ($x[0] -eq 0 -and $x[2] -ne $script:EchelleDepart) { [void]$script:Natif::FixerEchelleRelative($script:EchelleDepart) }
                # Jugé sur la valeur relative relue (DisplayConfigGetDeviceInfo, à jour) : le DPI
                # lu par ce processus peut être resté ancien.
                $limite = [DateTime]::UtcNow.AddSeconds(5)
                do { $x = @($script:Natif::EchelleRelative()); if ($x[0] -eq 0 -and $x[2] -eq $script:EchelleDepart) { break }; Start-Sleep -Milliseconds 200 } while ([DateTime]::UtcNow -lt $limite)
                while ($script:Natif::DpiEcran(1, 1) -ne 96 -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 200 }
                Verifier '(F125) échelle remise à 100 %' (($x[0] -eq 0) -and ($x[2] -eq $script:EchelleDepart)) "relatif $($x[2]) (départ $($script:EchelleDepart)) ; DPI lu ici $($script:Natif::DpiEcran(1, 1))" | Out-Null
                $script:EchelleDepart = $null
                Start-Sleep -Seconds 1
            }

            # ===================== Cas G : sans vignette ===================================
            $null = Ouvrir-Cas 'g' '(G)' -Vignette $false
            $script:Cas.Temoin = Lancer-Temoin 'g' 'simple'
            foreach ($sorte in 'video', 'gif') {
                Point "(G) sans vignette ($sorte)" {
                    $fN = Faire-Film -Sorte $sorte -Quoi "(G $sorte)" -DureeS 2 -Arret raccourci
                    $annonce = if ($sorte -eq 'video') { 'Copiée et enregistrée' } else { 'Copié et enregistré' }
                    $la = Trouver-Apres "sortie annonce reussite: $annonce$" $script:Cas.Dossier $fN.Base
                    Verifier "(G) $sorte : Finalisation -> Repos (ImagePreteSansVignette), annonce « $annonce »" (($fN.Etat -eq 'etat Finalisation -> Repos (ImagePreteSansVignette)') -and ($null -ne $la)) "$($fN.Etat) ; $(Message (Derniere-Ligne 'sortie annonce ' $script:Cas.Dossier))" | Out-Null
                }
            }
            Arreter-Temoin $script:Cas.Temoin
            Verifier-Interdites $script:Cas.Dossier '(G)'
            try { Fermer-Cas $script:proc $script:Cas.Dossier '(G)' } catch { Write-Host "$_" }
            Arreter-Tout

            # ===================== Cas H : copie éteinte, Copier, Supprimer ================
            $null = Ouvrir-Cas 'h' '(H)' -Copier $false
            $script:Cas.Temoin = Lancer-Temoin 'h' 'simple'
            foreach ($sorte in 'video', 'gif') {
                Point "(H) vignette ($sorte)" {
                    $dh = $script:Cas.Dossier
                    $fH = Faire-Film -Sorte $sorte -Quoi "(H $sorte)" -DureeS 2 -Arret raccourci
                    $id = $fH.Vignette
                    Verifier "(H) $sorte : copie éteinte : sortie film copie non demandee fichier fait" ($fH.Sortie -match '^sortie film copie non demandee fichier fait ') $fH.Sortie | Out-Null
                    $fait = if ($sorte -eq 'video') { 'Copiée' } else { 'Copié' }
                    Cliquer-Vignette $id 'copier' "(H $sorte) Copier"
                    $null = Attendre-Motif "vignette copie $id faite$" $dh 5 "(H $sorte) vignette copie faite"
                    $bouton = Element $script:FenetreVignette $fait $null 2
                    $lien = Verifier-PressePapiers "(H $sorte) « Copier »" $fH $(if ($sorte -eq 'video') { '.mp4' } else { '.gif' })
                    Verifier "(H) $sorte : « Copier » sur un film non copié : le bouton dit « $fait »" ($null -ne $bouton) $(if ($null -eq $bouton) { Elements-Vus $script:FenetreVignette }) | Out-Null
                    $suppr = Compter-Motif "vignette suppression $id en attente$" $dh
                    Cliquer-Vignette $id 'supprimer' "(H $sorte) Supprimer"
                    $null = Attendre-Motif "vignette suppression $id en attente$" $dh 3 "(H $sorte) suppression en attente" ($suppr + 1)
                    Start-Sleep -Milliseconds 400
                    Cliquer-Vignette $id 'annuler' "(H $sorte) Annuler"
                    $null = Attendre-Motif "vignette suppression $id annulee$" $dh 3 "(H $sorte) suppression annulée"
                    Verifier "(H) $sorte : Supprimer puis Annuler : le fichier enregistré (celui du presse-papiers) encore là" ((Test-Path -LiteralPath $fH.Enregistre) -and $lien -and (Test-Path -LiteralPath $lien)) "presse-papiers $(if ($lien) { Split-Path -Leaf $lien })" | Out-Null
                    Cliquer-Vignette $id 'supprimer' "(H $sorte) Supprimer (2)"
                    $null = Attendre-Motif "vignette suppression $id en attente$" $dh 3 "(H $sorte) suppression en attente (2)" ($suppr + 2)
                    $lf = Attendre-Motif "vignette suppression $id faite " $dh 8 "(H $sorte) suppression faite"
                    Start-Sleep -Milliseconds 300
                    $restent = @(@($fH.Enregistre, $lien) | Where-Object { $_ -and (Test-Path -LiteralPath $_) })
                    Verifier "(H) $sorte : Supprimer puis la fin du délai : le fichier enregistré parti, rien dans Films" (($restent.Count -eq 0) -and (@(Fichiers-De $script:Cas.Films).Count -eq 0)) "$(Message $lf) ; restent : $($restent.Count) ; Films $(@(Fichiers-De $script:Cas.Films).Count)" | Out-Null
                }
            }
            Arreter-Temoin $script:Cas.Temoin
            Verifier-Interdites $script:Cas.Dossier '(H)'
            try { Fermer-Cas $script:proc $script:Cas.Dossier '(H)' } catch { Write-Host "$_" }
            Arreter-Tout

            # ===================== Cas J : sans Media Foundation ===========================
            Point '(J) sans Media Foundation' {
                $null = Ouvrir-Cas 'j' '(J)' -SansCodeur
                $dj = $script:Cas.Dossier
                $ok = ((Compter-Motif 'film: media foundation absente$' $dj) -ge 1) -and ((Compter-Motif 'video disponible: non$' $dj) -eq 1) -and ((Compter-Motif 'raccourci video' $dj) -eq 0)
                Verifier '(J) film: media foundation absente, video disponible: non, aucune ligne raccourci video' $ok (Message (Derniere-Ligne 'video disponible: ' $dj)) | Out-Null
                $m = Ouvrir-Menu '(J)'
                $avec = (@($m.Textes | Where-Object { $_.StartsWith('GIF', [StringComparison]::Ordinal) }).Count -eq 1) -and (@($m.Textes | Where-Object { $_.StartsWith('Vidéo', [StringComparison]::Ordinal) }).Count -eq 0)
                Fermer-Menu-Icone $m '(J)'
                Verifier '(J) le menu montre « GIF » sans « Vidéo »' $avec $m.Lus | Out-Null
                $non = Compter-Motif '--capturer: mode non construit$' $dj
                $r = Lancer-Et-Attendre -Arguments @('--capturer', 'video') -DelaiS 15 -Dossier $dj
                $null = Attendre-Motif '--capturer: mode non construit$' $dj 5 '(J) --capturer video : mode non construit' ($non + 1)
                Verifier '(J) --capturer video → mode non construit' ($r.Fini -and ((Compter-Motif 'DemandeCapture video' $dj) -eq 0)) | Out-Null
                $script:Cas.Temoin = Lancer-Temoin 'j' 'simple'
                $fJ = Faire-Film -Sorte gif -Quoi '(J GIF)' -DureeS 2 -Arret raccourci
                $null = Verifier-Gif '(J GIF)' $fJ
                Arreter-Temoin $script:Cas.Temoin
                Verifier-Interdites $dj '(J)'
                Fermer-Cas $script:proc $dj '(J)'
            }
            Arreter-Tout

            # ===================== Cas K4 : disque plein ===================================
            Point '(K4) disque plein' {
                $dk = Dossier-Cas 'k4'
                $films = Join-Path $dk 'local/Films'
                New-Item -ItemType Directory -Force -Path $films | Out-Null
                $script:Vhd = Join-Path $script:Essai.Dossier 'films-300.vhdx'
                $script:Montage = $films
                $x = Diskpart @(
                    "create vdisk file=`"$script:Vhd`" maximum=300 type=expandable",
                    "select vdisk file=`"$script:Vhd`"",
                    'attach vdisk',
                    'convert mbr noerr',
                    'create partition primary',
                    'format fs=ntfs quick label=FILMS300',
                    "assign mount=`"$films`""
                ) 180
                $libre = @($script:Natif::Disque("$films\"))
                if ($x.Code -ne 0 -or $libre[0] -le 0 -or $libre[1] -gt 320MB) { throw "(K4) disque virtuel non posé : code $($x.Code), libre $($libre[0]), total $($libre[1]) ; $($x.Texte)" }
                $marge = 200MB + 2MB
                $lest = [long]($libre[0] - $marge)
                if ($lest -gt 0) {
                    $fs = [System.IO.File]::Open((Join-Path $films 'lest.bin'), 'Create', 'Write', 'None')
                    try { $fs.SetLength($lest) } finally { $fs.Dispose() }
                }
                $libre = @($script:Natif::Disque("$films\"))
                Noter-Mesure -Nom '(K4) disque virtuel posé (diskpart) ; place libre avant le film' -Valeur "$($x.Ms) ms ; $([math]::Round($libre[0] / 1MB, 2)) Mo libres sur $([math]::Round($libre[1] / 1MB, 1)) Mo"
                Ecrire-Reglages $dk 6
                Poser-Licence $dk
                $script:Cas = [pscustomobject]@{ Nom = 'k4'; Dossier = $dk; Images = (Join-Path $dk 'images'); Films = $films; Recents = (Join-Path $dk 'local/Recents'); Glisser = (Join-Path $dk 'local/Glisser')
                    Capturable = $false; SansCodeur = $false; Vignette = $true; Temoin = $null }
                Lancer-App-Du-Cas '(K4)'
                $script:Cas.Temoin = Lancer-Temoin 'k4' 'plein'
                $fK = Faire-Film -Sorte video -Quoi '(K4)' -DureeS 0 -Arret aucun -FinDelaiS 75
                $s = ($fK.ArretHeure - $fK.DebutHeure).TotalSeconds
                $imp = @(Toutes-Apres 'film impossible' $dk $fK.Base)
                Verifier '(K4) disque à la marge : film fin disque en moins de 60 s, aucune ligne film impossible' (($fK.Cause -eq 'disque') -and ($s -lt 60) -and ($imp.Count -eq 0)) "$($fK.Fin) après $([math]::Round($s, 1)) s ; impossible $($imp.Count)" | Out-Null
                $a = Relire-Mp4 $fK.Enregistre '(K4)'
                Verifier '(K4) fichier relu entier (durée du film à 200 ms près)' ([math]::Abs($a.DureeMs - $fK.Duree) -le 200) "$($a.DureeMs) ms, $($a.Images) images, $([math]::Round($a.Octets / 1MB, 2)) Mo" | Out-Null
                Arreter-Temoin $script:Cas.Temoin
                Verifier-Interdites $dk '(K4)'
                Fermer-Cas $script:proc $dk '(K4)'
            }
            Arreter-Tout
            Demonter-Disque

            # ===================== Cas L : app tuée pendant une vidéo ======================
            Point '(L) coupure' {
                $null = Ouvrir-Cas 'l' '(L)'
                $dl = $script:Cas.Dossier
                $script:Cas.Temoin = Lancer-Temoin 'l' 'simple'
                $fT = Faire-Film -Sorte video -Quoi '(L)' -DureeS 0 -Arret aucun -SansSortie -FinDelaiS 1 -Pendant {
                    param($F)
                    Start-Sleep -Milliseconds 1500
                    $F.TueeHeure = [DateTimeOffset]::Now
                    Stop-Process -Id $script:proc.Id -Force
                    $F.TueeSortie = $script:proc.WaitForExit(5000)
                    $F.Tuee = $true
                    $script:LCoupee = $true
                }
                Start-Sleep -Milliseconds 300
                $fins = @(Toutes-Apres 'film fin ' $dl $fT.Base)
                Verifier '(L) vidéo commencée (film debut) puis app tuée (Stop-Process) : sortie en moins de 5 s, aucune ligne film fin' ($fT.TueeSortie -and ($fins.Count -eq 0)) "$($fT.Debut) ; sortie $($fT.TueeSortie) ; film fin $($fins.Count)" | Out-Null
            }
            Point '(L) relance après la coupure' {
                Arreter-Tout
                Start-Sleep -Milliseconds 500
                $dl = $script:Cas.Dossier
                $base = Base-Journal $dl
                Lancer-App-Du-Cas '(L) relance'
                Start-Sleep -Seconds 3
                $vis = @(Visibles-App)
                $plomberie = @(Plomberie-Visible)
                if ($plomberie.Count -gt 0) { Noter-Mesure -Nom '(L) fenêtres de plomberie de Tauri marquées visibles après la relance (pour l''auteur du cœur)' -Valeur ($plomberie -join ' ; ') }
                $msg = @(Toutes-Apres 'bandeau affiche ' $dl $base)
                $quoiL = if ($script:LCoupee) { 'après Stop-Process pendant une vidéo' } else { 'après Stop-Process (aucune vidéo coupée : le point précédent a échoué)' }
                Verifier "(L) relancée $quoiL : démarrage normal, sans fenêtre ni message" (($vis.Count -eq 0) -and ($msg.Count -eq 0)) "visibles : $($vis -join ', ') ; messages : $(@($msg | ForEach-Object { Message $_ }) -join ' | ')" | Out-Null
                # Le fichier d'un film tué n'est pas noté (il n'a jamais été fermé) : il reste
                # dans Films, et le nettoyage du lancement garde toujours le film le plus récent.
                $restes = @(Fichiers-De $script:Cas.Films '*.mp4')
                $ranges = Trouver-Apres 'films ranges au lancement: ' $dl $base
                if ($script:LCoupee) { Verifier '(L) le film coupé reste dans <local>/Films après le nettoyage du lancement (le plus récent est gardé)' ($restes.Count -ge 1) "$($restes.Count) fichier(s) ; $(if ($ranges) { Message $ranges.Ligne } else { 'aucune ligne films ranges au lancement' })" | Out-Null }
                else { Noter-Mesure -Nom '(L) film coupé gardé dans <local>/Films' -Valeur 'non joué : aucune vidéo coupée' }
                Noter-Mesure -Nom '(L) film laissé dans <local>/Films (non relu : R17, MP4 sans fin)' -Valeur "$($restes.Count) fichier(s) ; $(if ($ranges) { Message $ranges.Ligne } else { 'rien rangé au lancement' })"
                Arreter-Temoin $script:Cas.Temoin
                Verifier-Interdites $dl '(L)'
                Fermer-Cas $script:proc $dl '(L)'
            }
        }
        finally {
            try { $script:Natif::BoutonHaut() } catch { }
            if ($script:Natif::ModeCourant() -ne $script:ModeDepart) { [void]$script:Natif::RemettreMode() }
            if ($null -ne $script:EchelleDepart) { try { [void]$script:Natif::FixerEchelleRelative($script:EchelleDepart) } catch { } }
            [void]$script:Natif::RemettreMouseKeys()
            Arreter-Tout
            Demonter-Disque
        }
    }
}
catch {
    Verifier 'essai sans erreur inattendue' $false "$_" | Out-Null
}
finally {
    foreach ($a in $script:Auxiliaires) { try { if (-not $a.HasExited) { $a.Kill() } } catch { } }
    Terminer-Essai
}
