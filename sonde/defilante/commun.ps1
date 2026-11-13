# Aides communes de la sonde de la capture défilante. À charger au début de chaque script :
#   . "$PSScriptRoot/commun.ps1"
#   if (Demarrer-Sonde -Nom 's1' -Titre '...') { try { ... } catch { Erreur-Sonde $_ } }
#   Terminer-Sonde
#
# Règles tenues par toutes les mesures :
# - aucune attente sans limite (Attendre a toujours un délai) ;
# - tout ce qui est ouvert (processus, fenêtres, réglages) est fermé ou remis dans
#   Terminer-Sonde, même après une erreur ;
# - une mesure qui ne peut pas se faire écrit « non mesurable : <raison> », jamais un
#   échec du job : Terminer-Sonde sort toujours avec 0 ;
# - chaque résultat est une ligne « mesure | valeur | critère | verdict » ; « passe »
#   n'est écrit que si l'effet a été constaté (images ou UI Automation).
# PowerShell 7. Les témoins, la sentinelle et les aides tournent dans d'autres processus.

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script:Ici = $PSScriptRoot
$script:Sonde = $null
$script:BUDGET_TOTAL = 40MB
$script:Pwsh = (Get-Process -Id $PID).Path
$script:N = $null
$script:Prises = @{}

# ---------------------------------------------------------------------------------------
# Résultats

function Demarrer-Sonde {
    param([Parameter(Mandatory)][string]$Nom, [Parameter(Mandatory)][string]$Titre, [double]$QuotaMo = 2)
    $sortie = if ($env:SONDE_RESULTATS) { $env:SONDE_RESULTATS } else { Join-Path (Get-Location) 'sonde-resultats' }
    New-Item -ItemType Directory -Force -Path $sortie | Out-Null
    $script:Sonde = [pscustomobject]@{
        Nom       = $Nom
        Titre     = $Titre
        Sortie    = (Resolve-Path -LiteralPath $sortie).Path
        Sections  = [System.Collections.Generic.List[object]]::new()
        Processus = [System.Collections.Generic.List[object]]::new()
        Restaurer = [System.Collections.Generic.List[object]]::new()
        Fermer    = [System.Collections.Generic.List[object]]::new()
        Chrono    = [System.Diagnostics.Stopwatch]::StartNew()
        Quota     = [long]($QuotaMo * 1MB)
        Ecrits    = [long]0
        Temp      = (Join-Path ([System.IO.Path]::GetTempPath()) ("breezyshot-sonde-$Nom-" + [guid]::NewGuid().ToString('N').Substring(0, 8)))
    }
    New-Item -ItemType Directory -Force -Path $script:Sonde.Temp | Out-Null
    Write-Host "=== Sonde $Nom : $Titre ==="
    Section $Titre
    if (-not $IsWindows) {
        NonMesurable 'toutes les mesures' 'pas sous Windows (la sonde ne tourne que sur windows-latest)'
        return $false
    }
    if (-not ('BreezyShotSonde.Natif' -as [type])) {
        Add-Type -TypeDefinition (Get-Content -LiteralPath (Join-Path $script:Ici 'natif.cs') -Raw)
    }
    $script:N = [BreezyShotSonde.Natif]
    try { [void]$script:N::SetProcessDpiAwarenessContext([IntPtr]::new(-4)) } catch { }
    Add-Type -AssemblyName System.Drawing, System.Windows.Forms, WindowsBase, UIAutomationClient, UIAutomationTypes
    return $true
}

function Section {
    param([Parameter(Mandatory)][string]$Titre, [string]$Texte = '')
    $s = [pscustomobject]@{ Titre = $Titre; Texte = $Texte; Lignes = [System.Collections.Generic.List[object]]::new(); Notes = [System.Collections.Generic.List[string]]::new() }
    $script:Sonde.Sections.Add($s)
    Write-Host "--- $Titre"
}

function Texte-Md([object]$v) {
    if ($null -eq $v) { return '' }
    return ("$v" -replace '\|', '/' -replace "[`r`n]+", ' ')
}

# Une ligne de résultat. Verdict : « passe », « échoue », « à lire », « non mesurable »…
function Resultat {
    param([Parameter(Mandatory)][string]$Mesure, [object]$Valeur, [string]$Critere = '', [string]$Verdict = '')
    $s = $script:Sonde.Sections[$script:Sonde.Sections.Count - 1]
    $s.Lignes.Add([pscustomobject]@{ Mesure = $Mesure; Valeur = (Texte-Md $Valeur); Critere = (Texte-Md $Critere); Verdict = $Verdict })
    # Le journal du job est la seule lecture sûre (les artefacts peuvent ne pas être relus) :
    # la ligne y porte aussi le critère.
    Write-Host "  $Mesure : $Valeur$(if ($Critere) { " | critère : $Critere" })$(if ($Verdict) { " [$Verdict]" })"
}

function NonMesurable {
    param([Parameter(Mandatory)][string]$Mesure, [Parameter(Mandatory)][string]$Raison, [string]$Critere = '')
    Resultat -Mesure $Mesure -Valeur "non mesurable : $Raison" -Critere $Critere -Verdict 'non mesurable'
}

function Note([string]$Texte) {
    $s = $script:Sonde.Sections[$script:Sonde.Sections.Count - 1]
    $s.Notes.Add((Texte-Md $Texte))
    Write-Host "  note : $Texte"
}

# Verdict d'un booléen : « passe » / « échoue ».
function Verdict([bool]$Ok) { if ($Ok) { 'passe' } else { 'échoue' } }

function Erreur-Sonde($Erreur) {
    $pos = ''
    try { $pos = " (ligne $($Erreur.InvocationInfo.ScriptLineNumber))" } catch { }
    Section 'Arrêt de la sonde sur une erreur'
    NonMesurable 'mesures restantes de ce script' "erreur du script : $($Erreur.Exception.Message)$pos"
}

function Terminer-Sonde {
    if ($null -eq $script:Sonde) { exit 0 }
    foreach ($f in @($script:Sonde.Fermer)) { try { & $f } catch { Write-Host "fermeture : $_" } }
    foreach ($r in @($script:Sonde.Restaurer)) { try { & $r } catch { Write-Host "remise en l'état : $_" } }
    foreach ($p in @($script:Sonde.Processus)) {
        try { if (-not $p.HasExited) { $p.Kill($true); $null = $p.WaitForExit(5000) } } catch { }
    }
    foreach ($c in $script:Prises.Values) { try { $c.Dispose() } catch { } }
    $script:Prises = @{}
    try { Remove-Item -LiteralPath $script:Sonde.Temp -Recurse -Force -ErrorAction SilentlyContinue } catch { }
    $md = [System.Collections.Generic.List[string]]::new()
    $md.Add("## Sonde $($script:Sonde.Nom) : $($script:Sonde.Titre)")
    $md.Add('')
    $md.Add("Durée : $([math]::Round($script:Sonde.Chrono.Elapsed.TotalSeconds)) s.")
    foreach ($s in $script:Sonde.Sections) {
        $md.Add('')
        $md.Add("### $($s.Titre)")
        if ($s.Texte) { $md.Add(''); $md.Add($s.Texte) }
        if ($s.Lignes.Count -gt 0) {
            $md.Add('')
            $md.Add('| Mesure | Valeur | Critère | Verdict |')
            $md.Add('| --- | --- | --- | --- |')
            foreach ($l in $s.Lignes) { $md.Add("| $(Texte-Md $l.Mesure) | $($l.Valeur) | $($l.Critere) | $($l.Verdict) |") }
        }
        if ($s.Notes.Count -gt 0) {
            $md.Add('')
            foreach ($n in $s.Notes) { $md.Add("- $n") }
        }
    }
    $texte = $md -join "`n"
    $fichier = Join-Path $script:Sonde.Sortie "$($script:Sonde.Nom).md"
    Set-Content -LiteralPath $fichier -Value $texte -Encoding utf8
    if ($env:GITHUB_STEP_SUMMARY) { Add-Content -LiteralPath $env:GITHUB_STEP_SUMMARY -Value ($texte + "`n") -Encoding utf8 }
    Write-Host "résultats : $fichier"
    exit 0
}

# ---------------------------------------------------------------------------------------
# Temps, processus, fichiers

function Attendre {
    param([Parameter(Mandatory)][scriptblock]$Condition, [double]$DelaiS = 10, [int]$PasMs = 100)
    $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
    while ($true) {
        $ok = $false
        try { $ok = [bool](& $Condition) } catch { $ok = $false }
        if ($ok) { return $true }
        if ([DateTime]::UtcNow -ge $limite) { return $false }
        Start-Sleep -Milliseconds $PasMs
    }
}

function Garder-Processus($P) { if ($null -ne $P) { $script:Sonde.Processus.Add($P) }; return $P }

# Un script de la sonde dans son propre pwsh (STA, sans nouvelle console).
function Lancer-Pwsh {
    param([Parameter(Mandatory)][string]$Script, [string[]]$Arguments = @())
    $args2 = @('-NoProfile', '-STA', '-File', "`"$(Join-Path $script:Ici $Script)`"") + $Arguments
    $p = Start-Process -FilePath $script:Pwsh -ArgumentList $args2 -NoNewWindow -PassThru
    $null = $p.Handle
    return (Garder-Processus $p)
}

# « cle valeur » par ligne → table (clés répétées : valeurs jointes par « ; »).
function Lire-Etat([string]$Fichier) {
    $t = @{}
    if (-not (Test-Path -LiteralPath $Fichier)) { return $t }
    # Lu en partage complet (lecture, écriture, suppression) : l'écrivain remplace le fichier
    # par renommage, ce qui échoue si un lecteur le tient sans FileShare.Delete (premier
    # passage : exception dans la minuterie de la sentinelle, boîte « Unhandled exception »).
    $lignes = @()
    try {
        $fs = [System.IO.FileStream]::new($Fichier, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read,
            [System.IO.FileShare]::ReadWrite -bor [System.IO.FileShare]::Delete)
        try { $lu = [System.IO.StreamReader]::new($fs).ReadToEnd() } finally { $fs.Dispose() }
        $lignes = @($lu -split "`r?`n")
    }
    catch { return $t }
    foreach ($l in $lignes) {
        $l = "$l".Trim()
        if (-not $l) { continue }
        $i = $l.IndexOf(' ')
        $k = if ($i -lt 0) { $l } else { $l.Substring(0, $i) }
        $v = if ($i -lt 0) { '' } else { $l.Substring($i + 1) }
        if ($t.ContainsKey($k)) { $t[$k] = "$($t[$k]) ; $v" } else { $t[$k] = $v }
    }
    return $t
}

function Ecrire-Commande([string]$Fichier, [string]$Texte) {
    $tmp = "$Fichier.tmp"
    Set-Content -LiteralPath $tmp -Value $Texte -Encoding ascii
    for ($i = 0; $i -lt 20; $i++) {
        try { [System.IO.File]::Move($tmp, $Fichier, $true); return }
        catch { Start-Sleep -Milliseconds 25 }
    }
    throw "commande non écrite (fichier tenu) : $Fichier"
}

function Rect([string]$Texte) {
    if ($Texte -notmatch '^\s*(-?\d+),(-?\d+),(\d+),(\d+)') { throw "rectangle illisible : $Texte" }
    return [int[]]@([int]$Matches[1], [int]$Matches[2], [int]$Matches[3], [int]$Matches[4])
}
function Rect-Texte([int[]]$R) { return "$($R[0]),$($R[1]),$($R[2]),$($R[3])" }
function Centre([int[]]$R) { return [int[]]@([int]($R[0] + $R[2] / 2), [int]($R[1] + $R[3] / 2)) }

# Chemin d'un exemple Rust construit (sonde/defilante/rust/target/release/examples).
function Exemple([string]$Nom) {
    $d = if ($env:SONDE_EXEMPLES) { $env:SONDE_EXEMPLES } else { Join-Path $script:Ici 'rust/target/release/examples' }
    $f = Join-Path $d "$Nom.exe"
    if (Test-Path -LiteralPath $f) { return $f }
    return $null
}

# Lance un exemple Rust et attend sa fin (DelaiS au plus) ; rend la table de ses lignes
# « cle valeur » (sortie standard), plus « _fini » et « _code ».
function Jouer-Exemple {
    param([Parameter(Mandatory)][string]$Nom, [string[]]$Arguments = @(), [double]$DelaiS = 60)
    $exe = Exemple $Nom
    if (-not $exe) { return @{ ok = '0'; erreur = "exemple $Nom absent (non construit)"; _fini = '0' } }
    $out = Join-Path $script:Sonde.Temp ("$Nom-" + [guid]::NewGuid().ToString('N').Substring(0, 6) + '.txt')
    $err = "$out.err"
    $p = Start-Process -FilePath $exe -ArgumentList $Arguments -NoNewWindow -PassThru -RedirectStandardOutput $out -RedirectStandardError $err
    $null = $p.Handle
    $fini = $p.WaitForExit([int]($DelaiS * 1000))
    if (-not $fini) { try { $p.Kill($true) } catch { } }
    $t = Lire-Etat $out
    $t['_fini'] = if ($fini) { '1' } else { '0' }
    $t['_code'] = if ($fini) { "$($p.ExitCode)" } else { 'arrêté' }
    if (-not $fini) { $t['erreur'] = "pas fini en $DelaiS s" }
    if (Test-Path -LiteralPath $err) {
        $e = (Get-Content -LiteralPath $err -Raw -ErrorAction SilentlyContinue)
        if ($e -and -not $t.ContainsKey('erreur') -and $t['ok'] -ne '1') { $t['erreur'] = ($e.Trim() -split "`n")[-1] }
    }
    return $t
}

# Lance un exemple Rust qui reste vivant (fenêtre de contrôle) ; rend le processus.
function Lancer-Exemple {
    param([Parameter(Mandatory)][string]$Nom, [string[]]$Arguments = @())
    $exe = Exemple $Nom
    if (-not $exe) { return $null }
    $out = Join-Path $script:Sonde.Temp ("$Nom-vivant-" + [guid]::NewGuid().ToString('N').Substring(0, 6) + '.txt')
    $p = Start-Process -FilePath $exe -ArgumentList $Arguments -NoNewWindow -PassThru -RedirectStandardOutput $out -RedirectStandardError "$out.err"
    $null = $p.Handle
    return (Garder-Processus $p)
}

function Fichier-Temp([string]$Nom) { return (Join-Path $script:Sonde.Temp $Nom) }

# ---------------------------------------------------------------------------------------
# Images

# Une image BGRA d'un rectangle de l'écran (moyen 1 de S2, DIB réutilisé par rectangle).
function Prendre {
    param([Parameter(Mandatory)][int[]]$R, [switch]$CaptureBlt)
    $cle = Rect-Texte $R
    if (-not $script:Prises.ContainsKey($cle)) {
        if ($script:Prises.Count -gt 16) { foreach ($c in $script:Prises.Values) { $c.Dispose() }; $script:Prises = @{} }
        $script:Prises[$cle] = [BreezyShotSonde.Prise]::new($R[0], $R[1], $R[2], $R[3])
    }
    $o = $script:Prises[$cle].Prendre([bool]$CaptureBlt)
    if ($null -eq $o) { throw "BitBlt a échoué (erreur $($script:Prises[$cle].DerniereErreur))" }
    return , $o
}

function Taille-Resultats {
    $t = 0L
    foreach ($f in @(Get-ChildItem -LiteralPath $script:Sonde.Sortie -Recurse -File -ErrorAction SilentlyContinue)) { $t += $f.Length }
    return $t
}

# Enregistre une image BGRA en PNG dans sonde-resultats/ (sous-dossier possible). Rend le
# chemin relatif, ou $null si le quota du script ou le plafond de 40 Mo serait dépassé.
function Enregistrer-Png {
    param([Parameter(Mandatory)][byte[]]$Octets, [int]$L, [int]$H, [Parameter(Mandatory)][string]$Nom)
    if ($script:Sonde.Ecrits -ge $script:Sonde.Quota) { return $null }
    $chemin = Join-Path $script:Sonde.Sortie $Nom
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $chemin) | Out-Null
    $bmp = [System.Drawing.Bitmap]::new($L, $H, [System.Drawing.Imaging.PixelFormat]::Format32bppRgb)
    try {
        $d = $bmp.LockBits([System.Drawing.Rectangle]::new(0, 0, $L, $H), [System.Drawing.Imaging.ImageLockMode]::WriteOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppRgb)
        try {
            for ($y = 0; $y -lt $H; $y++) {
                [System.Runtime.InteropServices.Marshal]::Copy($Octets, $y * $L * 4, [IntPtr]::Add($d.Scan0, $y * $d.Stride), $L * 4)
            }
        }
        finally { $bmp.UnlockBits($d) }
        $bmp.Save($chemin, [System.Drawing.Imaging.ImageFormat]::Png)
    }
    finally { $bmp.Dispose() }
    $taille = (Get-Item -LiteralPath $chemin).Length
    if ($script:Sonde.Ecrits + $taille -gt $script:Sonde.Quota -or (Taille-Resultats) -gt $script:BUDGET_TOTAL) {
        Remove-Item -LiteralPath $chemin -Force
        $script:Sonde.Ecrits = $script:Sonde.Quota
        return $null
    }
    $script:Sonde.Ecrits += $taille
    return $Nom
}

# Un PNG en octets BGRA : { L, H, O }.
function Lire-Png([string]$Chemin) {
    $img = [System.Drawing.Bitmap]::new($Chemin)
    try {
        $r = [System.Drawing.Rectangle]::new(0, 0, $img.Width, $img.Height)
        $d = $img.LockBits($r, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
        try {
            $o = [byte[]]::new($img.Width * $img.Height * 4)
            for ($y = 0; $y -lt $img.Height; $y++) {
                [System.Runtime.InteropServices.Marshal]::Copy([IntPtr]::Add($d.Scan0, $y * $d.Stride), $o, $y * $img.Width * 4, $img.Width * 4)
            }
        }
        finally { $img.UnlockBits($d) }
        return [pscustomobject]@{ L = $img.Width; H = $img.Height; O = $o }
    }
    finally { $img.Dispose() }
}

function Mediane([double[]]$V) {
    if ($null -eq $V -or $V.Count -eq 0) { return [double]::NaN }
    $s = @($V | Sort-Object)
    return [double]$s[[int][math]::Floor(($s.Count - 1) / 2)]
}
function Centile([double[]]$V, [double]$P) {
    if ($null -eq $V -or $V.Count -eq 0) { return [double]::NaN }
    $s = @($V | Sort-Object)
    return [double]$s[[int][math]::Round(($s.Count - 1) * $P)]
}
function Ms([double]$V) { if ([double]::IsNaN($V)) { return '—' }; return ('{0:0.#} ms' -f $V) }

# ---------------------------------------------------------------------------------------
# Pages témoins (copiées dans le dossier temporaire, avec l'image paresseuse)

function Preparer-Pages {
    $d = Join-Path $script:Sonde.Temp 'pages'
    if (Test-Path -LiteralPath (Join-Path $d 'longue.html')) { return $d }
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    Copy-Item -Path (Join-Path $script:Ici 'pages/*') -Destination $d -Force
    $b = [System.Drawing.Bitmap]::new(600, 800)
    $g = [System.Drawing.Graphics]::FromImage($b)
    for ($y = 0; $y -lt 800; $y += 20) {
        $c = [System.Drawing.Color]::FromArgb(255, (40 + $y / 4) % 256, (120 + $y / 3) % 256, 180)
        $pinceau = [System.Drawing.SolidBrush]::new($c)
        $g.FillRectangle($pinceau, 0, $y, 600, 20)
        $pinceau.Dispose()
    }
    $g.Dispose()
    $b.Save((Join-Path $d 'paresseuse.png'), [System.Drawing.Imaging.ImageFormat]::Png)
    $b.Dispose()
    return $d
}

function Url-Page([string]$Nom, [int]$Lignes = 800) {
    $d = Preparer-Pages
    $u = [System.Uri]::new((Join-Path $d 'longue.html')).AbsoluteUri
    return "$($u)?lignes=$Lignes#$Nom"
}

# ---------------------------------------------------------------------------------------
# Navigateurs et cibles

function Chemin-Navigateur([string]$Genre) {
    $pf = ${env:ProgramFiles}; $pf86 = ${env:ProgramFiles(x86)}
    $listes = @{
        edge    = @("$pf86\Microsoft\Edge\Application\msedge.exe", "$pf\Microsoft\Edge\Application\msedge.exe")
        chrome  = @("$pf\Google\Chrome\Application\chrome.exe", "$pf86\Google\Chrome\Application\chrome.exe")
        firefox = @("$pf\Mozilla Firefox\firefox.exe", "$pf86\Mozilla Firefox\firefox.exe")
    }
    if ($Genre -eq 'electron') {
        if ($env:SONDE_ELECTRON -and (Test-Path -LiteralPath $env:SONDE_ELECTRON)) { return $env:SONDE_ELECTRON }
        return $null
    }
    foreach ($c in @($listes[$Genre])) { if ($c -and (Test-Path -LiteralPath $c)) { return $c } }
    return $null
}

# La première fenêtre visible dont le titre contient $Titre (ou $null), DelaiS au plus.
function Trouver-Fenetre([string]$Titre, [double]$DelaiS = 20) {
    $trouvee = @{ H = [IntPtr]::Zero }
    $null = Attendre -DelaiS $DelaiS -PasMs 200 -Condition {
        foreach ($h in $script:N::Fenetres(0)) {
            if ($script:N::Titre($h).Contains($Titre)) { $trouvee.H = $h; return $true }
        }
        return $false
    }
    return $trouvee.H
}

# Hauteur du document (pixels CSS) dite par le titre de la page (« h=<n> »), ou 0.
function Hauteur-Page($C) {
    if ($script:N::Titre($C.Racine) -match ' h=(\d+)') { return [int]$Matches[1] }
    return 0
}

$script:Compteur = 0

# Ouvre une cible de S1 ; rend un objet { Genre, Nom, Racine, Defile, Zone, Bande, Colonne,
# Echelle, Contenu, Processus, Fermer } ou lève une erreur qui dit pourquoi.
#   Genre : wpf, winforms, molette, edge, chrome, firefox, electron, explorateur
function Ouvrir-Cible {
    param([Parameter(Mandatory)][string]$Genre, [int[]]$R = @(0, 0, 700, 720), [double]$Echelle = 1.0,
        [int]$Lignes = 800, [switch]$Courte)
    $script:Compteur++
    $nom = "$Genre-$($script:Compteur)"
    $c = [pscustomobject]@{ Genre = $Genre; Nom = $nom; Racine = [IntPtr]::Zero; Defile = [IntPtr]::Zero; Zone = $null
        Bande = $false; Colonne = 1; Echelle = $Echelle; Contenu = 0; Processus = $null; Fermer = $null; Uia = $null; Info = ''
        Pids = [System.Collections.Generic.List[int]]::new() }
    if ($Genre -in 'wpf', 'winforms', 'molette') {
        $f = Fichier-Temp "temoin-$nom.txt"
        $a = @('-Genre', $Genre, '-X', $R[0], '-Y', $R[1], '-L', $R[2], '-H', $R[3], '-Fichier', "`"$f`"")
        if ($Echelle -ne 1.0) { $a += @('-Echelle', "$Echelle") }
        if ($Courte) { $a += '-Courte' }
        $c.Processus = Lancer-Pwsh 'temoin.ps1' $a
        if (-not (Attendre -DelaiS 40 -Condition { (Lire-Etat $f).ContainsKey('zone') })) { throw "témoin $Genre : pas prêt en 40 s" }
        $e = Lire-Etat $f
        $c.Racine = [IntPtr]::new([long]$e.hwnd)
        $c.Defile = [IntPtr]::new([long]$e.defile)
        $c.Zone = Rect $e.zone
        $c.Contenu = [int]$e.contenu
        $c.Echelle = [double]$e.echelle
        $c.Bande = $true
        $c.Colonne = 2
        $p = $c.Processus
        $c.Fermer = { try { if (-not $p.HasExited) { $p.Kill($true); $null = $p.WaitForExit(5000) } } catch { } }.GetNewClosure()
    }
    elseif ($Genre -eq 'explorateur') {
        $d = Join-Path $script:Sonde.Temp "dossier-$nom"
        New-Item -ItemType Directory -Force -Path $d | Out-Null
        $n = if ($Courte) { 3 } else { 300 }
        for ($i = 1; $i -le $n; $i++) { [System.IO.File]::WriteAllBytes((Join-Path $d ('fichier-{0:D3}.txt' -f $i)), [byte[]]@()) }
        $shell = New-Object -ComObject Shell.Application
        Start-Process -FilePath 'explorer.exe' -ArgumentList "`"$d`""
        $trouve = @{ W = $null }
        $ok = Attendre -DelaiS 20 -PasMs 250 -Condition {
            foreach ($w in @($shell.Windows())) {
                try {
                    $chemin = [System.Uri]::new($w.LocationURL).LocalPath
                    if ($chemin.TrimEnd('\') -ieq $d.TrimEnd('\')) { $trouve.W = $w; return $true }
                }
                catch { }
            }
            return $false
        }
        if (-not $ok) { throw 'Explorateur : fenêtre du dossier introuvable en 20 s' }
        $fenetre = $trouve.W
        try { $fenetre.Document.CurrentViewMode = 4 } catch { $c.Info = "affichage Détails non posé ($($_.Exception.Message))" }
        $c.Racine = [IntPtr]::new([long]$fenetre.HWND)
        [void]$script:N::ShowWindow($c.Racine, 9)
        [void]$script:N::MoveWindow($c.Racine, $R[0], $R[1], $R[2], $R[3], $true)
        Start-Sleep -Milliseconds 800
        $w = $fenetre
        $c.Fermer = { try { $w.Quit() } catch { } }.GetNewClosure()
        # La liste des fichiers : l'élément UI Automation « UIItemsView », sinon la fenêtre DirectUIHWND.
        $c.Zone = Zone-Uia $c.Racine 'UIItemsView'
        if ($null -eq $c.Zone) {
            foreach ($h in $script:N::Filles($c.Racine)) {
                if ($script:N::Classe($h) -eq 'DirectUIHWND' -and $script:N::Classe($script:N::GetAncestor($h, 1)) -eq 'SHELLDLL_DefView') { $c.Zone = $script:N::Client($h); break }
            }
        }
        if ($null -eq $c.Zone) { throw 'Explorateur : liste des fichiers introuvable' }
        $c.Defile = $script:N::SousPoint((Centre $c.Zone)[0], (Centre $c.Zone)[1])
    }
    elseif ($Genre -eq 'parametres') {
        # L'app Paramètres (WinUI/XAML, hébergée par ApplicationFrameHost) sur la liste des
        # applications installées, longue sur la machine de GitHub.
        $exe = Join-Path $env:windir 'ImmersiveControlPanel\SystemSettings.exe'
        if (-not (Test-Path -LiteralPath $exe)) { throw 'app Paramètres absente de la machine' }
        foreach ($q in @(Get-Process -Name SystemSettings -ErrorAction SilentlyContinue)) { try { $q.Kill() } catch { } }
        Start-Sleep -Milliseconds 500
        Start-Process -FilePath 'explorer.exe' -ArgumentList 'ms-settings:appsfeatures'
        $trouve = @{ Cadre = [IntPtr]::Zero; Contenu = [IntPtr]::Zero; Pid = 0 }
        $ok = Attendre -DelaiS 30 -PasMs 300 -Condition {
            $ps = @(Get-Process -Name SystemSettings -ErrorAction SilentlyContinue)
            if ($ps.Count -eq 0) { return $false }
            foreach ($h in $script:N::Fenetres(0)) {
                if ($script:N::Classe($h) -ne 'ApplicationFrameWindow') { continue }
                foreach ($f in $script:N::Filles($h)) {
                    $pf = $script:N::Pid($f)
                    if (@($ps | Where-Object { $_.Id -eq $pf }).Count -gt 0) { $trouve.Cadre = $h; $trouve.Contenu = $f; $trouve.Pid = $pf; return $true }
                }
            }
            return $false
        }
        if (-not $ok) { throw 'Paramètres : fenêtre introuvable en 30 s' }
        $c.Racine = $trouve.Cadre
        $c.Defile = $trouve.Contenu
        $c.Pids.Add($script:N::Pid($c.Racine)); $c.Pids.Add($trouve.Pid)
        [void]$script:N::ShowWindow($c.Racine, 9)
        [void]$script:N::MoveWindow($c.Racine, $R[0], $R[1], $R[2], $R[3], $true)
        $cadre = $c.Racine; $pidP = $trouve.Pid; $NN = $script:N
        $c.Fermer = {
            try { [void]$NN::PostMessage($cadre, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) } catch { }
            Start-Sleep -Milliseconds 400
            try { $q = [System.Diagnostics.Process]::GetProcessById($pidP); if (-not $q.HasExited) { $q.Kill() } } catch { }
        }.GetNewClosure()
        $script:Sonde.Fermer.Add($c.Fermer)
        # La liste : le plus grand élément défilable de la fenêtre (attendu 15 s : la liste
        # des applications se remplit après l'ouverture).
        $z = @{ R = $null }
        $null = Attendre -DelaiS 15 -PasMs 500 -Condition {
            $u = Uia-Plus-Grand $c.Racine
            if ($null -ne $u) { $b = $u.Current.BoundingRectangle; $z.R = [int[]]@([int]$b.X, [int]$b.Y, [int]$b.Width, [int]$b.Height); return $true }
            return $false
        }
        if ($null -eq $z.R) { throw 'Paramètres : aucun élément défilable verticalement (UI Automation)' }
        $c.Zone = $z.R
        $c.Bande = $false
        $c.Info = 'Paramètres (WinUI) : pas de bande, décalage mesuré sur les images'
        $script:Sonde.Fermer.Remove($c.Fermer) | Out-Null
    }
    else {
        $exe = Chemin-Navigateur $Genre
        if (-not $exe) { throw "$Genre absent de la machine" }
        $url = Url-Page $nom -Lignes $(if ($Courte) { 10 } else { $Lignes })
        $profil = Join-Path $script:Sonde.Temp "profil-$nom"
        New-Item -ItemType Directory -Force -Path $profil | Out-Null
        $lw = [int]($R[2] / $Echelle); $lh = [int]($R[3] / $Echelle)
        if ($Genre -in 'edge', 'chrome') {
            $a = @("--app=$url", "--user-data-dir=`"$profil`"", "--window-position=$($R[0]),$($R[1])", "--window-size=$lw,$lh",
                '--no-first-run', '--no-default-browser-check', '--disable-sync', '--disable-extensions',
                '--disable-background-networking', '--disable-component-update', '--disable-default-apps',
                '--hide-crash-restore-bubble', '--disable-session-crashed-bubble', '--password-store=basic',
                '--disable-features=msEdgeSidebarV2,msHubApps,msImplicitSignin,EdgeCollections,Translate,msEdgeTranslate',
                '--disable-translate', '--lang=en-US', '--force-color-profile=srgb')
            if ($Echelle -ne 1.0) { $a += "--force-device-scale-factor=$Echelle" }
            # Traduction coupée aussi par la préférence du profil (premier passage : « Translate
            # page from French? » devant Edge).
            New-Item -ItemType Directory -Force -Path (Join-Path $profil 'Default') | Out-Null
            Set-Content -LiteralPath (Join-Path $profil 'Default\Preferences') -Encoding ascii -Value '{"translate":{"enabled":false},"browser":{"has_seen_welcome_page":true},"translate_blocked_languages":["fr","en"]}'

            $c.Processus = Garder-Processus (Start-Process -FilePath $exe -ArgumentList $a -PassThru)
        }
        elseif ($Genre -eq 'firefox') {
            @('user_pref("browser.aboutwelcome.enabled", false);', 'user_pref("browser.shell.checkDefaultBrowser", false);',
                'user_pref("datareporting.policy.dataSubmissionEnabled", false);', 'user_pref("toolkit.telemetry.reportingpolicy.firstRun", false);',
                'user_pref("browser.startup.homepage_override.mstone", "ignore");', 'user_pref("app.update.enabled", false);',
                'user_pref("app.normandy.enabled", false);', 'user_pref("browser.tabs.warnOnClose", false);',
                'user_pref("datareporting.healthreport.uploadEnabled", false);', 'user_pref("network.captive-portal-service.enabled", false);',
                'user_pref("gfx.color_management.mode", 0);', 'user_pref("browser.translations.enable", false);',
                'user_pref("browser.translations.automaticallyPopup", false);', 'user_pref("trailhead.firstrun.didSeeAboutWelcome", true);',
                'user_pref("browser.startup.page", 0);', 'user_pref("sidebar.revamp", false);') |
                Set-Content -LiteralPath (Join-Path $profil 'user.js') -Encoding ascii
            $a = @('-no-remote', '-new-instance', '-profile', "`"$profil`"", '-width', $R[2], '-height', $R[3], "`"$url`"")
            $c.Processus = Garder-Processus (Start-Process -FilePath $exe -ArgumentList $a -PassThru)
        }
        elseif ($Genre -eq 'electron') {
            $env:SONDE_URL = $url; $env:SONDE_RECT = Rect-Texte $R
            try {
                $c.Processus = Garder-Processus (Start-Process -FilePath $exe -ArgumentList @("`"$(Join-Path $script:Ici 'electron/main.js')`"", "--user-data-dir=`"$profil`"") -PassThru)
            }
            finally { Remove-Item Env:SONDE_URL, Env:SONDE_RECT -ErrorAction SilentlyContinue }
        }
        $c.Racine = Trouver-Fenetre "Longue page BreezyShot $nom h=" 30
        if ($c.Racine -eq [IntPtr]::Zero) { throw "$Genre : fenêtre de la page introuvable en 30 s" }
        $pidFenetre = $script:N::Pid($c.Racine)
        $c.Pids.Add($pidFenetre)
        [void]$script:N::MoveWindow($c.Racine, $R[0], $R[1], $R[2], $R[3], $true)
        Start-Sleep -Milliseconds 1500
        # Bulles et invites du navigateur (traduction, bienvenue…) : d'autres fenêtres de
        # premier niveau du même processus ; fermées et dites.
        $bulles = [System.Collections.Generic.List[string]]::new()
        foreach ($h in $script:N::Fenetres($pidFenetre)) {
            if ($h -eq $c.Racine) { continue }
            $bulles.Add($script:N::Decrire($h))
            [void]$script:N::PostMessage($h, 0x10, [IntPtr]::Zero, [IntPtr]::Zero)
        }
        if ($bulles.Count -gt 0) { Write-Host "  note : $Genre : fenêtres en plus fermées : $($bulles -join ' ; ')"; Start-Sleep -Milliseconds 300 }
        $proc = $c.Processus; $racine = $c.Racine; $NN = $script:N
        $c.Fermer = {
            try { [void]$NN::PostMessage($racine, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) } catch { }
            Start-Sleep -Milliseconds 500
            try { if ($proc -and -not $proc.HasExited) { $proc.Kill($true) } } catch { }
            try { $q = [System.Diagnostics.Process]::GetProcessById($pidFenetre); if (-not $q.HasExited) { $q.Kill($true) } } catch { }
        }.GetNewClosure()
        # La page : la fenêtre de rendu de Chromium, sinon l'élément Document d'UI Automation.
        foreach ($h in $script:N::Filles($c.Racine)) {
            if ($script:N::Classe($h) -eq 'Chrome_RenderWidgetHostHWND' -and $script:N::IsWindowVisible($h)) { $c.Zone = $script:N::Client($h); $c.Defile = $h; break }
        }
        if ($null -eq $c.Zone) { $c.Zone = Zone-Uia $c.Racine '' 'Document' }
        if ($null -eq $c.Zone) { throw "$Genre : zone de la page introuvable (ni Chrome_RenderWidgetHostHWND ni Document)" }
        if ($c.Defile -eq [IntPtr]::Zero) { $c.Defile = $script:N::SousPoint((Centre $c.Zone)[0], (Centre $c.Zone)[1]) }
        $c.Bande = $true
        $c.Contenu = 0
        $img = Prendre $c.Zone
        $col = $script:N::ColonneBande($img, $c.Zone[2], $c.Zone[3], 24, 0, [math]::Min(200, $c.Zone[3]))
        if ($col -lt 0) {
            $c.Bande = $false
            $y0 = [math]::Min([math]::Max(0, $c.Zone[3] - 1), [int](80 * $Echelle))
            $c.Info = "bande de la page illisible : décalage mesuré sur les images seulement ; couleurs RRVVBB lues aux colonnes 0, 3, 6, lignes $y0 à $($y0 + 3) : $($script:N::Pixels($img, $c.Zone[2], 0, $y0, 4)) / $($script:N::Pixels($img, $c.Zone[2], 3, $y0, 4)) / $($script:N::Pixels($img, $c.Zone[2], 6, $y0, 4)) (attendu : la marque de natif.cs CouleurLigne, V entre 40 et 7F, B entre A0 et AF)"
        }
        else { $c.Colonne = $col + [math]::Max(1, [int][math]::Floor($Echelle * 2)) }
    }
    if ($c.Pids.Count -eq 0 -and $c.Racine -ne [IntPtr]::Zero) { $c.Pids.Add($script:N::Pid($c.Racine)) }
    $script:Sonde.Fermer.Add($c.Fermer)
    return $c
}

# '' si le centre de la zone est bien sur la cible (fenêtre de premier niveau de la cible,
# ou fenêtre d'un de ses processus) ; sinon la description de la fenêtre qui la couvre.
function Devant($C, [int[]]$Point = $null) {
    $pt = if ($null -ne $Point) { $Point } else { Centre $C.Zone }
    $h = $script:N::SousPoint($pt[0], $pt[1])
    $r = $script:N::GetAncestor($h, 2)
    if ($r -eq $C.Racine) { return '' }
    if ($C.Pids -contains $script:N::Pid($h)) { return '' }
    return "$($script:N::Decrire($r)) au point $($pt[0]),$($pt[1])"
}

# Comme Devant, mais essaie d'abord de remettre la cible au premier plan (sauf -SansPremierPlan).
function Degager($C, [switch]$SansPremierPlan) {
    $d = Devant $C
    if (-not $d -or $SansPremierPlan) { return $d }
    [void]$script:N::PremierPlan($C.Racine)
    Start-Sleep -Milliseconds 150
    return (Devant $C)
}

# Écrit dans le journal toutes les fenêtres visibles d'autres processus que la sonde qui
# couvrent le centre de la zone (pour dire ce qui gêne).
function Noter-Obstacles($C) {
    $pt = Centre $C.Zone
    foreach ($h in $script:N::Fenetres(0)) {
        if ($h -eq $C.Racine) { break }
        $b = $script:N::Bord($h)
        if ($pt[0] -ge $b.Left -and $pt[0] -lt $b.Right -and $pt[1] -ge $b.Top -and $pt[1] -lt $b.Bottom) { Note "au-dessus de la cible $($C.Genre) : $($script:N::Decrire($h)) ($($b.Left),$($b.Top) → $($b.Right),$($b.Bottom))" }
    }
}

function Fermer-Cible($C) {
    if ($null -eq $C -or $null -eq $C.Fermer) { return }
    try { & $C.Fermer } catch { }
    [void]$script:Sonde.Fermer.Remove($C.Fermer)
    Start-Sleep -Milliseconds 300
}

# ---------------------------------------------------------------------------------------
# UI Automation (client managé de .NET : System.Windows.Automation)

function Zone-Uia([IntPtr]$Racine, [string]$Classe = '', [string]$Type = '') {
    try {
        $AE = [System.Windows.Automation.AutomationElement]
        $e = $AE::FromHandle($Racine)
        $cond = if ($Classe) { [System.Windows.Automation.PropertyCondition]::new($AE::ClassNameProperty, $Classe) }
        else { [System.Windows.Automation.PropertyCondition]::new($AE::ControlTypeProperty, [System.Windows.Automation.ControlType]::Document) }
        $t = $e.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
        if ($null -eq $t) { return $null }
        $b = $t.Current.BoundingRectangle
        if ($b.IsEmpty -or $b.Width -le 0) { return $null }
        return [int[]]@([int]$b.X, [int]$b.Y, [int]$b.Width, [int]$b.Height)
    }
    catch { return $null }
}

# Le plus grand élément de la fenêtre $Racine qui défile verticalement (ou $null).
function Uia-Plus-Grand([IntPtr]$Racine) {
    $AE = [System.Windows.Automation.AutomationElement]
    $SP = [System.Windows.Automation.ScrollPattern]
    try {
        $cond = [System.Windows.Automation.PropertyCondition]::new($AE::IsScrollPatternAvailableProperty, $true)
        $meilleur = $null; $aire = 0.0
        foreach ($x in @($AE::FromHandle($Racine).FindAll([System.Windows.Automation.TreeScope]::Descendants, $cond))) {
            $m = $null
            if (-not $x.TryGetCurrentPattern($SP::Pattern, [ref]$m) -or -not $m.Current.VerticallyScrollable) { continue }
            $b = $x.Current.BoundingRectangle
            if ($b.IsEmpty) { continue }
            if ($b.Width * $b.Height -gt $aire) { $aire = $b.Width * $b.Height; $meilleur = $x }
        }
        return $meilleur
    }
    catch { return $null }
}

# Une recherche : sous le centre de la zone (l'élément ou un parent avec un ScrollPattern,
# s'il est d'un processus de la cible), sinon depuis la fenêtre de la cible (le plus grand
# élément défilable dont le rectangle contient le centre).
function Uia-Chercher($C, [bool]$Tous) {
    $AE = [System.Windows.Automation.AutomationElement]
    $SP = [System.Windows.Automation.ScrollPattern]
    $pt = Centre $C.Zone
    $wp = [System.Windows.Point]::new($pt[0], $pt[1])
    try {
        $e = $AE::FromPoint($wp)
        if ($null -ne $e -and ($C.Pids.Count -eq 0 -or $C.Pids -contains $e.Current.ProcessId)) {
            $marche = [System.Windows.Automation.TreeWalker]::RawViewWalker
            for ($i = 0; $i -lt 40 -and $null -ne $e; $i++) {
                $m = $null
                if ($e.TryGetCurrentPattern($SP::Pattern, [ref]$m) -and ($Tous -or $m.Current.VerticallyScrollable)) {
                    return [pscustomobject]@{ Element = $e; Motif = $m; Par = 'sous le centre'; Ms = 0.0 }
                }
                $e = $marche.GetParent($e)
            }
        }
    }
    catch { }
    try {
        $cond = [System.Windows.Automation.PropertyCondition]::new($AE::IsScrollPatternAvailableProperty, $true)
        $meilleur = $null; $motif = $null; $aire = 0.0
        foreach ($x in @($AE::FromHandle($C.Racine).FindAll([System.Windows.Automation.TreeScope]::Descendants, $cond))) {
            $m = $null
            if (-not $x.TryGetCurrentPattern($SP::Pattern, [ref]$m)) { continue }
            if (-not $Tous -and -not $m.Current.VerticallyScrollable) { continue }
            $b = $x.Current.BoundingRectangle
            if ($b.IsEmpty -or -not $b.Contains($wp)) { continue }
            if ($b.Width * $b.Height -gt $aire) { $aire = $b.Width * $b.Height; $meilleur = $x; $motif = $m }
        }
        if ($null -ne $meilleur) { return [pscustomobject]@{ Element = $meilleur; Motif = $motif; Par = 'depuis la fenêtre'; Ms = 0.0 } }
    }
    catch { }
    return $null
}

# L'élément qui défile pour la cible (ScrollPattern défilable verticalement ; -Tous : même
# non défilable, pour une page courte) : { Element, Motif, Par, Ms } ou $null. Une seule
# fonction pour tous les scripts. Chromium n'ouvre son arbre d'accessibilité qu'à la
# première demande et le rend 0,3 à 0,4 s plus tard (premier passage) : la recherche est
# répétée toutes les 150 ms jusqu'à AttenteMs ; Ms dit quand elle a abouti.
function Uia-Defilable($C, [int]$AttenteMs = 2500, [switch]$Tous) {
    $chrono = [System.Diagnostics.Stopwatch]::StartNew()
    while ($true) {
        $u = Uia-Chercher $C ([bool]$Tous)
        if ($null -ne $u) { $u.Ms = $chrono.Elapsed.TotalMilliseconds; return $u }
        if ($chrono.Elapsed.TotalMilliseconds -ge $AttenteMs) { return $null }
        Start-Sleep -Milliseconds 150
    }
}

function Uia-Pourcent($U) {
    if ($null -eq $U) { return [double]::NaN }
    try { return [double]$U.Motif.Current.VerticalScrollPercent } catch { return [double]::NaN }
}

# ---------------------------------------------------------------------------------------
# Position et mouvement d'une cible

# Prend des images toutes les ~15 ms jusqu'à deux égales de suite (MaxMs au plus), en C#
# (Prise.Stable). Rend { Image, Ms (du départ à la première des deux égales), Stable, Images }.
function Attendre-Stable($C, [int]$MaxMs = 800, $Depart = $null) {
    $chrono = if ($Depart) { $Depart } else { [System.Diagnostics.Stopwatch]::StartNew() }
    $null = Prendre $C.Zone
    $p = $script:Prises[(Rect-Texte $C.Zone)]
    $ms = 0.0; $stable = $false; $n = 0
    $col = if ($C.Bande) { $C.Colonne } else { -1 }
    $img = $p.Stable($col, [math]::Max(1, $C.Zone[2] - 24), $MaxMs, $chrono, [ref]$ms, [ref]$stable, [ref]$n)
    if ($null -eq $img) { throw "BitBlt a échoué (erreur $($p.DerniereErreur))" }
    return [pscustomobject]@{ Image = $img; Ms = $ms; Stable = $stable; Images = $n }
}

# Décalage vrai (pixels réels) lu sur la bande, ou NaN.
function Position-Bande($C, [byte[]]$Img) {
    if (-not $C.Bande) { return [double]::NaN }
    $r = $script:N::DecalageBande($Img, $C.Zone[2], $C.Colonne, 0, [math]::Min(240, $C.Zone[3]), $C.Echelle)
    if ($null -eq $r) { return [double]::NaN }
    return [double]$r[0]
}

# Pixels parcourus entre deux images (positif : la page est descendue). Par la bande si elle
# se lit, sinon par la comparaison des lignes. Rend { Px, Par }.
function Mouvement($C, [byte[]]$A, [byte[]]$B) {
    $pa = Position-Bande $C $A; $pb = Position-Bande $C $B
    if (-not [double]::IsNaN($pa) -and -not [double]::IsNaN($pb)) { return [pscustomobject]@{ Px = [math]::Round($pb - $pa, 2); Par = 'bande' } }
    $l = $C.Zone[2]; $h = $C.Zone[3]
    $d = $script:N::Decalage($A, $B, $l, $h, 0, [math]::Max(1, $l - 24), [math]::Max(1, $h - 40))
    if ($d[1] -le 0 -or $d[1] * 4 -lt $d[2]) { return [pscustomobject]@{ Px = [double]::NaN; Par = 'images (aucun recouvrement)' } }
    return [pscustomobject]@{ Px = [double]$d[0]; Par = 'images' }
}

# Vrai si la cible paraît en bas : UI Automation ≥ 99,9 %, ou la bande au bout du contenu.
function En-Bas($C, [byte[]]$Img) {
    if ($null -ne $C.Uia) {
        $p = Uia-Pourcent $C.Uia
        if (-not [double]::IsNaN($p) -and $p -ge 99.9) { return $true }
    }
    if ($C.Bande -and $C.Contenu -gt 0) {
        $p = Position-Bande $C $Img
        if (-not [double]::IsNaN($p) -and $p + $C.Zone[3] -ge $C.Contenu - 2) { return $true }
    }
    if ($C.Bande -and $C.Genre -notin 'wpf', 'winforms', 'molette') {
        # Page : la dernière ligne de la zone porte la dernière ligne du document.
        $hp = Hauteur-Page $C
        $bas = $script:N::LireLigne($Img, $C.Zone[2], $C.Colonne, $C.Zone[3] - 1)
        if ($hp -gt 0 -and $bas -ge $hp - 2) { return $true }
    }
    return $false
}

# ---------------------------------------------------------------------------------------
# Sentinelle

function Ouvrir-Sentinelle([int[]]$R = @(712, 40, 300, 600)) {
    $f = Fichier-Temp 'sentinelle.txt'
    $p = Lancer-Pwsh 'temoin.ps1' @('-Genre', 'sentinelle', '-X', $R[0], '-Y', $R[1], '-L', $R[2], '-H', $R[3], '-Fichier', "`"$f`"")
    if (-not (Attendre -DelaiS 40 -Condition { (Lire-Etat $f).ContainsKey('molettes') })) { throw 'sentinelle : pas prête en 40 s' }
    $e = Lire-Etat $f
    $s = [pscustomobject]@{ Fichier = $f; Processus = $p; Racine = [IntPtr]::new([long]$e.hwnd); Zone = (Rect $e.zone) }
    return $s
}

function Etat-Sentinelle($S) {
    Start-Sleep -Milliseconds 220
    $e = Lire-Etat $S.Fichier
    if (-not $e.ContainsKey('molettes')) { Start-Sleep -Milliseconds 150; $e = Lire-Etat $S.Fichier }
    return "$($e['molettes'])/$($e['defilements'])/$($e['touches'])/$($e['position'])"
}

# ---------------------------------------------------------------------------------------
# Suites d'images (S3, S8)

function Pas-UIA($C) {
    $vs = [double]$C.Uia.Motif.Current.VerticalViewSize
    $p = [double]$C.Uia.Motif.Current.VerticalScrollPercent
    if ($vs -ge 100 -or $vs -le 0) { throw "VerticalViewSize $vs" }
    $C.Uia.Motif.SetScrollPercent([System.Windows.Automation.ScrollPattern]::NoScroll, [math]::Min(100.0, [math]::Max(0.0, $p) + 50.0 * $vs / (100.0 - $vs)))
}

# Enregistre une suite. $Pas : scriptblock qui fait avancer la page. Rend les images.
function Suite([string]$Cas, $C, [int]$N, [scriptblock]$Pas, [switch]$Lissage) {
    $dossier = "$($script:Sonde.Nom)/$Cas"
    $lignes = [System.Collections.Generic.List[string]]::new()
    $lignes.Add('image;temps_ms;bande_bas;bande_haut;uia_pourcent;decalage_images;lignes_egales')
    $images = [System.Collections.Generic.List[object]]::new()
    $chrono = [System.Diagnostics.Stopwatch]::StartNew()
    $l = $C.Zone[2]; $h = $C.Zone[3]
    $etat = @{ Prec = $null; Gardees = 0 }
    $ajouter = {
        param([byte[]]$img)
        $t = $chrono.Elapsed.TotalMilliseconds
        $b = if ($C.Bande) { $script:N::DecalageBande($img, $l, $C.Colonne, 0, [math]::Min(240, $h), $C.Echelle) } else { $null }
        $u = Uia-Pourcent $C.Uia
        $di = ''; $eg = ''
        if ($null -ne $etat.Prec) {
            $d = $script:N::Decalage($etat.Prec, $img, $l, $h, 0, [math]::Max(1, $l - 24), [math]::Max(1, $h - 40))
            $di = $d[0]; $eg = "$($d[1])/$($d[2])"
        }
        $nom = '{0:D3}.png' -f $images.Count
        $png = Enregistrer-Png $img $l $h "$dossier/$nom"
        if ($png) { $etat.Gardees++ }
        $lignes.Add("$(if ($png) { $nom } else { '(non gardée : quota)' });$('{0:0.0}' -f $t);$(if ($b) { '{0:0.###}' -f $b[0] });$(if ($b) { '{0:0.###}' -f $b[1] });$(if (-not [double]::IsNaN($u)) { '{0:0.####}' -f $u });$di;$eg")
        $images.Add([pscustomobject]@{ O = $img; Bande = $b; Uia = $u })
        $etat.Prec = $img
    }
    $st = Attendre-Stable $C 1500
    & $ajouter $st.Image
    for ($i = 1; $i -lt $N; $i++) {
        try { & $Pas } catch { Note "$Cas : pas $i refusé ($($_.Exception.Message))"; break }
        if ($Lissage) {
            $t0 = [System.Diagnostics.Stopwatch]::StartNew()
            while ($t0.Elapsed.TotalMilliseconds -lt 400 -and $images.Count -lt $N) { Start-Sleep -Milliseconds 33; & $ajouter (Prendre $C.Zone) }
            $i = $images.Count - 1
        }
        else { $st = Attendre-Stable $C 1200; & $ajouter $st.Image }
    }
    $csv = Join-Path $script:Sonde.Sortie "$dossier/index.csv"
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $csv) | Out-Null
    Set-Content -LiteralPath $csv -Value $lignes -Encoding utf8
    Note "$Cas : $($images.Count) images ($($etat.Gardees) gardées en PNG), $l × $h, $dossier/index.csv"
    return $images
}

# Un pas de suite par la molette (pages web sans ScrollPattern) : $Crans crans au centre,
# si rien ne couvre la cible.
function Pas-Molette($C, [int]$Crans = 3) {
    $d = Devant $C
    if ($d) { throw "molette sautée : $d" }
    $pt = Centre $C.Zone
    $script:N::Poser($pt[0], $pt[1])
    for ($i = 0; $i -lt $Crans; $i++) { [void]$script:N::Molette(-1, $script:N::MARQUE); Start-Sleep -Milliseconds 20 }
}

# Égalité au bit près des lignes de deux images successives, décalées du décalage trouvé
# sur les images (et comparé au décalage vrai de la bande quand il est lisible) ; tout le
# résumé est écrit dans le journal (les PNG peuvent ne pas être relus) : nombre d'images,
# pas vrai, paires égales au bit près, et pour les autres combien de pixels diffèrent et où
# (une ou deux lignes, un bord, une zone, tout le texte), avec l'écart maximal par canal.
function Mesure-Suite([string]$Cas, $C, $Images, [int]$Haut) {
    $l = $C.Zone[2]; $h = $C.Zone[3]
    $paires = 0; $exactes = 0; $tol = 0; $accord = 0; $avecBande = 0; $lignesEx = 0L; $lignesTot = 0L; $pixEx = 0L; $pixTot = 0L
    $vrais = [System.Collections.Generic.List[double]]::new()
    $diffs = [System.Collections.Generic.List[double]]::new(); $touchees = [System.Collections.Generic.List[double]]::new()
    $ecartMax = 0; $plus8 = 0L; $genres = [ordered]@{ 'une ou deux lignes' = 0; 'bord (4 premières ou dernières lignes)' = 0; 'une zone (< 30 % des lignes)' = 0; 'tout le texte (≥ 30 % des lignes)' = 0 }
    $detail = [System.Collections.Generic.List[string]]::new()
    $immobiles = 0
    for ($i = 1; $i -lt $Images.Count; $i++) {
        $a = $Images[$i - 1]; $b = $Images[$i]
        $d = $script:N::Decalage($a.O, $b.O, $l, $h, 0, [math]::Max(1, $l - 24), [math]::Max(1, $h - 40))
        $vrai = [double]::NaN
        if ($null -ne $a.Bande -and $null -ne $b.Bande) { $vrai = (($b.Bande[0] + $b.Bande[1]) - ($a.Bande[0] + $a.Bande[1])) / 2; $vrais.Add($vrai) }
        if ($d[0] -eq 0) { $immobiles++; continue }
        $paires++
        if (-not [double]::IsNaN($vrai)) {
            $avecBande++
            if ([math]::Abs($vrai - $d[0]) -le ($C.Echelle - 1 + 0.5)) { $accord++ }
        }
        $r = $script:N::Comparer($a.O, $b.O, $l, $h, $d[0], $Haut, $h, 0, [math]::Max(1, $l - 24))
        $lignesEx += $r[0]; $lignesTot += $r[2]; $pixEx += $r[3]; $pixTot += $r[5]
        if ($r[2] -gt 0 -and $r[1] * 100 -ge $r[2] * 97) { $tol++ }
        $p = $script:N::Repartition($a.O, $b.O, $l, $h, $d[0], $Haut, $h, 0, [math]::Max(1, $l - 24))
        $vu = if ([double]::IsNaN($vrai)) { '?' } else { '{0:0.##}' -f $vrai }
        if ($r[2] -gt 0 -and $p[0] -eq 0) { $exactes++; $detail.Add("$($i):d=$($d[0])/vrai=$vu/égale"); continue }
        $diffs.Add([double]$p[0]); $touchees.Add([double]$p[1] / [math]::Max(1, $p[10]))
        $ecartMax = [math]::Max($ecartMax, [int]$p[8]); $plus8 += $p[9]
        $g = if ($p[1] -le 2) { 'une ou deux lignes' } elseif ($p[11] * 2 -ge $p[0]) { 'bord (4 premières ou dernières lignes)' } elseif ($p[1] * 10 -lt $p[10] * 3) { 'une zone (< 30 % des lignes)' } else { 'tout le texte (≥ 30 % des lignes)' }
        $genres[$g]++
        $detail.Add("$($i):d=$($d[0])/vrai=$vu/$($p[0]) px sur $($p[1]) lignes ($($p[2]) groupes, x $($p[4])-$($p[6]), y $($p[5])-$($p[7]), écart max $($p[8]))")
    }
    $pasVrai = if ($vrais.Count -gt 0) { $v = $vrais.ToArray(); "pas vrai (bande) médiane $('{0:0.##}' -f (Mediane $v)) px, de $('{0:0.##}' -f ($v | Measure-Object -Minimum).Minimum) à $('{0:0.##}' -f ($v | Measure-Object -Maximum).Maximum)" } else { 'pas vrai non lu (pas de bande)' }
    Resultat "$Cas : suite" "$($Images.Count) images de $l × $h ; $pasVrai ; paires immobiles $immobiles" 'à lire' 'à lire'
    if ($paires -eq 0) { NonMesurable "$Cas : égalité au bit près" 'aucune paire d''images décalées'; return }
    Resultat "$Cas : paires au bit près" "$exactes/$paires paires entièrement égales une fois décalées (sous les $Haut premières lignes, sans les 24 colonnes de droite) ; lignes égales $('{0:P2}' -f ($lignesEx / [math]::Max(1, $lignesTot))) ; pixels égaux $('{0:P3}' -f ($pixEx / [math]::Max(1, $pixTot)))" 'chantier C : décalage exact sur 100 % des images WPF, ≥ 99 % des images Edge' 'à lire'
    if ($diffs.Count -gt 0) {
        $dv = $diffs.ToArray(); $tv = $touchees.ToArray()
        Resultat "$Cas : pixels différents des $($diffs.Count) paires non égales" "médiane $([int](Mediane $dv)) px (max $([int](($dv | Measure-Object -Maximum).Maximum))) ; lignes touchées médiane $('{0:P1}' -f (Mediane $tv)) ; écart maximal sur un canal $ecartMax ; pixels à plus de ± 8 : $plus8 ; où : $(($genres.Keys | ForEach-Object { "$_ $($genres[$_])" }) -join ', ')" 'à lire : la tolérance que l''algorithme devra avoir' 'à lire'
    }
    Resultat "$Cas : paires à la tolérance de D12" "$tol/$paires (97 % des lignes égales à ± 8 sur 90 % des pixels)" 'à lire' 'à lire'
    if ($avecBande -gt 0) { Resultat "$Cas : décalage des images contre le vrai (bande)" "$accord/$avecBande d'accord" 'tous' (Verdict ($accord -eq $avecBande)) }
    Note "$Cas, paire par paire (i:d images/vrai/différences) : $(($detail | Select-Object -First 50) -join ' ; ')"
}
