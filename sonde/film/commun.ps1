# Aides communes de la sonde du film (contrat 0.7, partie I). À charger au début de chaque
# script :
#   . "$PSScriptRoot/commun.ps1"
#   if (Demarrer-Sonde -Nom 's1' -Titre '...') { try { ... } catch { Erreur-Sonde $_ } }
#   Terminer-Sonde
#
# Règles tenues par toutes les mesures (comme la sonde 0.6) :
# - aucune attente sans limite ;
# - tout ce qui est ouvert (processus, fenêtres, réglages) est fermé ou remis dans
#   Terminer-Sonde, même après une erreur ;
# - une mesure qui ne peut pas se faire écrit « NON MESURÉ : <raison> » (avec le HRESULT ou
#   le code de sortie quand il y en a un), jamais un silence ni un échec du job :
#   Terminer-Sonde sort toujours avec 0 ;
# - chaque résultat est une ligne « mesure | valeur | critère | verdict », écrite aussitôt
#   dans le journal du job (Write-Host) et, à la fin du script, dans
#   sonde-resultats/<nom>.md ; verdict : OUI / NON pour une question, « passe » / « échoue »
#   pour un critère du contrat, « à lire » pour un chiffre sans seuil.
# PowerShell 7.

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script:Ici = $PSScriptRoot
$script:Sonde = $null
$script:Pwsh = (Get-Process -Id $PID).Path
$script:N = $null
$script:T = $null
# Plafond des fichiers gardés en artefact (MP4, GIF, PNG), tous scripts confondus.
$script:BUDGET_FICHIERS = 150MB

# ---------------------------------------------------------------------------------------
# Résultats

function Dossier-Resultats {
    $d = if ($env:SONDE_RESULTATS) { $env:SONDE_RESULTATS } else { Join-Path (Get-Location) 'sonde-resultats' }
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    return (Resolve-Path -LiteralPath $d).Path
}

function Demarrer-Sonde {
    param([Parameter(Mandatory)][string]$Nom, [Parameter(Mandatory)][string]$Titre)
    $script:Sonde = [pscustomobject]@{
        Nom       = $Nom
        Titre     = $Titre
        Sortie    = (Dossier-Resultats)
        Sections  = [System.Collections.Generic.List[object]]::new()
        Processus = [System.Collections.Generic.List[object]]::new()
        Restaurer = [System.Collections.Generic.List[object]]::new()
        Chrono    = [System.Diagnostics.Stopwatch]::StartNew()
        Temp      = (Join-Path ([System.IO.Path]::GetTempPath()) ("breezyshot-film-$Nom-" + [guid]::NewGuid().ToString('N').Substring(0, 8)))
        Machine   = ''
    }
    New-Item -ItemType Directory -Force -Path $script:Sonde.Temp | Out-Null
    Write-Host "=== Sonde $Nom : $Titre ==="
    Section $Titre
    if (-not $IsWindows) {
        NonMesurable 'toutes les mesures' 'pas sous Windows (la sonde ne tourne que sur windows-latest)'
        return $false
    }
    if (-not ('BreezyShotFilm.Natif' -as [type])) {
        Add-Type -TypeDefinition (Get-Content -LiteralPath (Join-Path $script:Ici 'natif.cs') -Raw)
    }
    $script:N = [BreezyShotFilm.Natif]
    $script:T = [BreezyShotFilm.Temoin]
    try { [void]$script:N::SetProcessDpiAwarenessContext([IntPtr]::new(-4)) } catch { }
    Add-Type -AssemblyName System.Drawing, System.Windows.Forms
    $script:Sonde.Machine = Machine
    Write-Host "machine : $($script:Sonde.Machine)"
    return $true
}

# Édition, version et build de Windows, et la carte qui porte l'écran (contrat §1.0) : en
# tête de chaque résultat.
function Machine {
    $w = '?'; $c = '?'
    try {
        $cv = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion' -ErrorAction Stop
        $v = { param($n) if ($cv.PSObject.Properties[$n]) { $cv.$n } else { '?' } }
        $w = "$(& $v 'ProductName'), édition $(& $v 'EditionID'), version $(& $v 'DisplayVersion'), build $(& $v 'CurrentBuild').$(& $v 'UBR')"
    }
    catch { $w = "illisible ($($_.Exception.Message))" }
    try {
        $cartes = @(Get-CimInstance Win32_VideoController -ErrorAction Stop)
        $ecran = @($cartes | Where-Object { $_.CurrentHorizontalResolution })
        if ($ecran.Count -eq 0) { $ecran = $cartes }
        $c = ($ecran | ForEach-Object { "$($_.Name) (pilote $($_.DriverVersion), $($_.InstalledDisplayDrivers -split ',' | Select-Object -First 1), $($_.CurrentHorizontalResolution) × $($_.CurrentVerticalResolution))" }) -join ' ; '
    }
    catch { $c = "illisible ($($_.Exception.Message))" }
    return "Windows : $w ; carte de l'écran : $c"
}

function Section {
    param([Parameter(Mandatory)][string]$Titre)
    $s = [pscustomobject]@{ Titre = $Titre; Lignes = [System.Collections.Generic.List[object]]::new(); Notes = [System.Collections.Generic.List[string]]::new() }
    $script:Sonde.Sections.Add($s)
    Write-Host "--- $Titre"
}

function Texte-Md([object]$v) {
    if ($null -eq $v) { return '' }
    return ("$v" -replace '\|', '/' -replace "[`r`n]+", ' ')
}

function Resultat {
    param([Parameter(Mandatory)][string]$Mesure, [object]$Valeur, [string]$Critere = '', [string]$Verdict = '')
    $s = $script:Sonde.Sections[$script:Sonde.Sections.Count - 1]
    $s.Lignes.Add([pscustomobject]@{ Mesure = $Mesure; Valeur = (Texte-Md $Valeur); Critere = (Texte-Md $Critere); Verdict = $Verdict })
    Write-Host "  $Mesure : $Valeur$(if ($Critere) { " | critère : $Critere" })$(if ($Verdict) { " [$Verdict]" })"
}

function NonMesurable {
    param([Parameter(Mandatory)][string]$Mesure, [Parameter(Mandatory)][string]$Raison, [string]$Critere = '')
    Resultat -Mesure $Mesure -Valeur "NON MESURÉ : $Raison" -Critere $Critere -Verdict 'non mesuré'
}

function Note([string]$Texte) {
    $s = $script:Sonde.Sections[$script:Sonde.Sections.Count - 1]
    $s.Notes.Add((Texte-Md $Texte))
    Write-Host "  note : $Texte"
}

function Verdict([bool]$Ok) { if ($Ok) { 'passe' } else { 'échoue' } }
function OuiNon([bool]$Ok) { if ($Ok) { 'OUI' } else { 'NON' } }

function Erreur-Sonde($Erreur) {
    $pos = ''
    try { $pos = " (ligne $($Erreur.InvocationInfo.ScriptLineNumber))" } catch { }
    Section 'Arrêt du script sur une erreur'
    NonMesurable 'mesures restantes de ce script' "erreur du script : $($Erreur.Exception.Message)$pos"
}

function Terminer-Sonde {
    if ($null -eq $script:Sonde) { exit 0 }
    foreach ($r in @($script:Sonde.Restaurer)) { try { & $r } catch { Write-Host "remise en l'état : $_" } }
    foreach ($p in @($script:Sonde.Processus)) {
        try { if (-not $p.HasExited) { $p.Kill($true); $null = $p.WaitForExit(5000) } } catch { }
    }
    try { Remove-Item -LiteralPath $script:Sonde.Temp -Recurse -Force -ErrorAction SilentlyContinue } catch { }
    $md = [System.Collections.Generic.List[string]]::new()
    $md.Add("## Sonde $($script:Sonde.Nom) : $($script:Sonde.Titre)")
    $md.Add('')
    if ($script:Sonde.Machine) { $md.Add("Machine : $(Texte-Md $script:Sonde.Machine)."); $md.Add('') }
    $md.Add("Durée : $([math]::Round($script:Sonde.Chrono.Elapsed.TotalSeconds)) s.")
    foreach ($s in $script:Sonde.Sections) {
        $md.Add('')
        $md.Add("### $($s.Titre)")
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

function Lancer-Pwsh {
    param([Parameter(Mandatory)][string]$Script, [string[]]$Arguments = @())
    $a = @('-NoProfile', '-STA', '-File', "`"$(Join-Path $script:Ici $Script)`"") + $Arguments
    $p = Start-Process -FilePath $script:Pwsh -ArgumentList $a -NoNewWindow -PassThru
    $null = $p.Handle
    return (Garder-Processus $p)
}

# « cle valeur » par ligne → table (clés répétées : valeurs jointes par « ; »).
function Lire-Etat([string]$Fichier) {
    $t = @{}
    if (-not (Test-Path -LiteralPath $Fichier)) { return $t }
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

function Rect-Texte([int[]]$R) { return "$($R[0]),$($R[1]),$($R[2]),$($R[3])" }

function Fichier-Temp([string]$Nom) { return (Join-Path $script:Sonde.Temp $Nom) }

# Chemin d'un exemple Rust construit (target/release/examples), ou d'un binaire
# (target/release, -Binaire).
function Exemple([string]$Nom, [switch]$Binaire) {
    $d = if ($env:SONDE_CIBLE) { $env:SONDE_CIBLE } else { Join-Path $script:Ici 'rust/target/release' }
    $f = if ($Binaire) { Join-Path $d "$Nom.exe" } else { Join-Path $d "examples/$Nom.exe" }
    if (Test-Path -LiteralPath $f) { return $f }
    return $null
}

# Code de sortie lisible : décimal et hexadécimal (0xC0000135 : DLL introuvable…).
function Code-Sortie([int]$C) {
    $h = '0x{0:X8}' -f ([uint32]([int64]$C -band 0xFFFFFFFF))
    $connus = @{ '0xC0000135' = 'STATUS_DLL_NOT_FOUND'; '0xC0000139' = 'STATUS_ENTRYPOINT_NOT_FOUND'; '0xC0000005' = 'STATUS_ACCESS_VIOLATION'; '0xC000007B' = 'STATUS_INVALID_IMAGE_FORMAT'; '0xC0000409' = 'STATUS_STACK_BUFFER_OVERRUN'; '0xC06D007E' = 'module à la demande introuvable (delayimp)' }
    if ($connus.ContainsKey($h)) { return "$C ($h $($connus[$h]))" }
    return "$C ($h)"
}

# Lance un exemple Rust et attend sa fin (DelaiS au plus) ; rend la table de ses lignes
# « cle valeur » (sortie standard), plus « _fini », « _code » et « _ms ». Sans « ok 1 »,
# « erreur » dit pourquoi (ligne de l'exemple, sinon sa sortie d'erreur, sinon son code).
function Jouer-Exemple {
    param([Parameter(Mandatory)][string]$Nom, [string[]]$Arguments = @(), [double]$DelaiS = 60, [switch]$Binaire)
    $exe = Exemple $Nom -Binaire:$Binaire
    if (-not $exe) { return @{ ok = '0'; erreur = "exemple $Nom absent (étape de construction échouée ?)"; _fini = '0'; _code = '' } }
    $out = Join-Path $script:Sonde.Temp ("$Nom-" + [guid]::NewGuid().ToString('N').Substring(0, 6) + '.txt')
    $err = "$out.err"
    $chrono = [System.Diagnostics.Stopwatch]::StartNew()
    $p = Start-Process -FilePath $exe -ArgumentList $Arguments -NoNewWindow -PassThru -RedirectStandardOutput $out -RedirectStandardError $err
    $null = $p.Handle
    $fini = $p.WaitForExit([int]($DelaiS * 1000))
    if (-not $fini) { try { $p.Kill($true) } catch { } }
    $t = Lire-Etat $out
    $t['_fini'] = if ($fini) { '1' } else { '0' }
    $t['_code'] = if ($fini) { Code-Sortie $p.ExitCode } else { 'arrêté' }
    $t['_ms'] = [int]$chrono.Elapsed.TotalMilliseconds
    if (-not $fini) { $t['erreur'] = "pas fini en $DelaiS s" }
    if ($t['ok'] -ne '1' -and -not $t.ContainsKey('erreur')) {
        $e = ''
        if (Test-Path -LiteralPath $err) { $e = "$(Get-Content -LiteralPath $err -Raw -ErrorAction SilentlyContinue)".Trim() }
        $t['erreur'] = if ($e) { "code de sortie $($t['_code']) ; $(($e -split "`n")[-1])" } else { "code de sortie $($t['_code']), rien écrit" }
    }
    return $t
}

# Lance un exemple Rust qui reste vivant ; rend le processus.
function Lancer-Exemple {
    param([Parameter(Mandatory)][string]$Nom, [string[]]$Arguments = @(), [switch]$Binaire)
    $exe = Exemple $Nom -Binaire:$Binaire
    if (-not $exe) { return $null }
    $out = Join-Path $script:Sonde.Temp ("$Nom-vivant-" + [guid]::NewGuid().ToString('N').Substring(0, 6) + '.txt')
    $p = Start-Process -FilePath $exe -ArgumentList $Arguments -NoNewWindow -PassThru -RedirectStandardOutput $out -RedirectStandardError "$out.err"
    $null = $p.Handle
    return (Garder-Processus $p)
}

# Garde une copie d'un fichier produit (MP4, GIF, PNG) dans sonde-resultats/fichiers/ si le
# plafond le permet ; rend le chemin relatif gardé, ou une raison.
function Garder-Fichier([string]$Chemin, [string]$Nom = '') {
    if (-not (Test-Path -LiteralPath $Chemin)) { return '(fichier absent)' }
    if (-not $Nom) { $Nom = Split-Path -Leaf $Chemin }
    $d = Join-Path (Dossier-Resultats) 'fichiers'
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    $deja = 0L
    foreach ($f in @(Get-ChildItem -LiteralPath $d -File -ErrorAction SilentlyContinue)) { $deja += $f.Length }
    $t = (Get-Item -LiteralPath $Chemin).Length
    if ($deja + $t -gt $script:BUDGET_FICHIERS) { return "(non gardé : plafond de $([int]($script:BUDGET_FICHIERS / 1MB)) Mo des artefacts)" }
    Copy-Item -LiteralPath $Chemin -Destination (Join-Path $d $Nom) -Force
    return "fichiers/$Nom"
}

function Mediane([double[]]$V) {
    if ($null -eq $V -or $V.Count -eq 0) { return [double]::NaN }
    $s = @($V | Sort-Object)
    return [double]$s[[int][math]::Floor(($s.Count - 1) / 2)]
}
function Ms([object]$V) {
    $d = 0.0
    if (-not [double]::TryParse("$V", [System.Globalization.NumberStyles]::Float, [System.Globalization.CultureInfo]::InvariantCulture, [ref]$d) -or [double]::IsNaN($d)) { return '—' }
    return ('{0:0.##} ms' -f $d)
}
function Nombre([object]$V) {
    $d = 0.0
    if ([double]::TryParse("$V", [System.Globalization.NumberStyles]::Float, [System.Globalization.CultureInfo]::InvariantCulture, [ref]$d)) { return $d }
    return [double]::NaN
}
# « m;p;x » (médiane, 95e centile, max) → texte.
function Stats([object]$V) {
    $p = "$V" -split ';'
    if ($p.Count -lt 3) { return '—' }
    return "médiane $(Ms $p[0]), 95e centile $(Ms $p[1]), max $(Ms $p[2])"
}
function Stat-Mediane([object]$V) { return (Nombre (("$V" -split ';')[0])) }
function Mo([object]$Octets) { return ('{0:0.00} Mo' -f ((Nombre $Octets) / 1MB)) }

# ---------------------------------------------------------------------------------------
# Écran et témoin

function Ecran { $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds; return [int[]]@($b.X, $b.Y, $b.Width, $b.Height) }

# Copie de tout l'écran principal en PNG (fond des images plus grandes que le témoin).
function Copie-Ecran([string]$Chemin) {
    $e = Ecran
    $b = [System.Drawing.Bitmap]::new($e[2], $e[3])
    try {
        $g = [System.Drawing.Graphics]::FromImage($b)
        try { $g.CopyFromScreen($e[0], $e[1], 0, 0, [System.Drawing.Size]::new($e[2], $e[3])) } finally { $g.Dispose() }
        $b.Save($Chemin, [System.Drawing.Imaging.ImageFormat]::Png)
    }
    finally { $b.Dispose() }
}

# Ouvre le témoin WPF (temoin.ps1, son propre processus) ; rend { P, Hwnd, Zone, Fichier,
# Journal, Genre, Rapide }. -Rapide : k avance à chaque image composée.
function Ouvrir-Temoin {
    param([ValidateSet('simple', 'fixe', 'plein')][string]$Genre = 'simple', [int]$X = 100, [int]$Y = 100, [int]$PeriodeMs = 100, [switch]$Rapide)
    $f = Fichier-Temp "temoin-$Genre-$([guid]::NewGuid().ToString('N').Substring(0, 6)).txt"
    $a = @('-Genre', $Genre, '-X', $X, '-Y', $Y, '-PeriodeMs', $PeriodeMs, '-Fichier', "`"$f`"")
    if ($Rapide) { $a += '-Rapide' }
    $p = Lancer-Pwsh 'temoin.ps1' $a
    if (-not (Attendre -DelaiS 40 -Condition { (Lire-Etat $f).ContainsKey('zone') })) { throw "témoin $Genre : pas prêt en 40 s$(if ($p.HasExited) { " (sorti, code $(Code-Sortie $p.ExitCode))" })" }
    $e = Lire-Etat $f
    $z = [int[]]@("$($e.zone)" -split ',' | ForEach-Object { [int]$_ })
    Start-Sleep -Milliseconds 400
    return [pscustomobject]@{ P = $p; Hwnd = [IntPtr]::new([long]$e.hwnd); Zone = $z; Fichier = $f; Journal = "$($e['journal'])"; Genre = $Genre; Rapide = [bool]$Rapide }
}

# Pas du témoin (lignes « k <n> <ms> » de son journal) : rend { Pas, ParS, Ecarts (ms entre
# deux pas : médiane;95e;max), De, A } sur les -DureeS dernières secondes (0 : tout).
function Rythme-Temoin($T, [double]$DureeS = 0) {
    $r = [pscustomobject]@{ Pas = 0; ParS = [double]::NaN; Ecarts = ''; De = 0; A = 0 }
    if ($null -eq $T -or -not $T.Journal) { return $r }
    $l = @()
    try {
        $fs = [System.IO.FileStream]::new($T.Journal, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite -bor [System.IO.FileShare]::Delete)
        try { $l = @([System.IO.StreamReader]::new($fs).ReadToEnd() -split "`r?`n") } finally { $fs.Dispose() }
    }
    catch { return $r }
    $ms = [System.Collections.Generic.List[double]]::new()
    foreach ($x in $l) { $p = "$x".Trim() -split ' '; if ($p.Count -eq 3 -and $p[0] -eq 'k') { $ms.Add((Nombre $p[2])) } }
    if ($ms.Count -lt 3) { $r.Pas = $ms.Count; return $r }
    $fin = $ms[$ms.Count - 1]
    $deb = if ($DureeS -gt 0) { $fin - $DureeS * 1000 } else { $ms[1] }
    $v = @($ms | Where-Object { $_ -ge $deb })
    $e = [System.Collections.Generic.List[double]]::new()
    for ($i = 1; $i -lt $v.Count; $i++) { $e.Add($v[$i] - $v[$i - 1]) }
    $s = @($e | Sort-Object)
    $r.Pas = $v.Count
    $r.ParS = if ($v.Count -gt 1) { ($v.Count - 1) / (($v[$v.Count - 1] - $v[0]) / 1000) } else { [double]::NaN }
    if ($s.Count) { $r.Ecarts = "$(Mediane $s);$($s[[int][math]::Floor(0.95 * ($s.Count - 1))]);$($s[$s.Count - 1])" }
    $r.De = $v[0]; $r.A = $v[$v.Count - 1]
    return $r
}
function Fermer-Temoin($T) {
    if ($null -eq $T) { return }
    try { if (-not $T.P.HasExited) { $T.P.Kill($true); $null = $T.P.WaitForExit(5000) } } catch { }
}

# ---------------------------------------------------------------------------------------
# Fenêtre de contrôle (exemple controle --genre film)

function Ouvrir-Controle {
    param([int[]]$Zone, [switch]$SansAnimation)
    $etat = Fichier-Temp "controle-$([guid]::NewGuid().ToString('N').Substring(0, 6)).txt"
    $cmd = "$etat.cmd"
    $a = @('--genre', 'film', '--etat', "`"$etat`"", '--commandes', "`"$cmd`"", '--zone', (Rect-Texte $Zone), '--animation', $(if ($SansAnimation) { '0' } else { '1' }))
    $p = Lancer-Exemple 'controle' $a
    if ($null -eq $p) { throw 'exemple controle absent (étape de construction échouée ?)' }
    $o = [pscustomobject]@{ P = $p; Etat = $etat; Cmd = $cmd }
    $ok = Attendre -DelaiS 40 -PasMs 100 -Condition { $e = Lire-Etat $etat; $e['visible'] -eq '1' -and ($e['dom_ms'] -as [int]) -gt 0 }
    if (-not $ok) {
        $e = Lire-Etat $etat
        Fermer-Controle $o
        throw "fenêtre de contrôle pas prête en 40 s ($(if ($e.ContainsKey('erreur')) { $e.erreur } elseif ($p.HasExited) { "sortie, code $(Code-Sortie $p.ExitCode)" } else { 'aucune erreur dite' }))"
    }
    Start-Sleep -Milliseconds 500
    return $o
}
function Fermer-Controle($O) {
    if ($null -eq $O) { return }
    try { Ecrire-Commande $O.Cmd 'quitter' } catch { }
    if (-not $O.P.WaitForExit(5000)) { try { $O.P.Kill($true) } catch { } }
}

# Processus WebView2 d'une fenêtre de contrôle (par son dossier de données) et l'hôte.
function Pids-Controle($O) {
    $e = Lire-Etat $O.Etat
    $d = "$($e['donnees'])"
    $pids = [System.Collections.Generic.List[int]]::new()
    $pids.Add($O.P.Id)
    if ($d) {
        foreach ($w in @(Get-CimInstance Win32_Process -Filter "Name = 'msedgewebview2.exe'" -ErrorAction SilentlyContinue)) {
            if ("$($w.CommandLine)".Contains($d)) { $pids.Add([int]$w.ProcessId) }
        }
    }
    return , $pids
}

# Temps de processeur total (s) et mémoire privée (octets) d'une liste de processus.
function Mesure-Processus([int[]]$Pids) {
    $cpu = 0.0; $prive = 0L; $vivants = 0
    foreach ($i in $Pids) {
        try { $q = [System.Diagnostics.Process]::GetProcessById($i); $cpu += $q.TotalProcessorTime.TotalSeconds; $prive += $q.PrivateMemorySize64; $vivants++ } catch { }
    }
    return [pscustomobject]@{ Cpu = $cpu; Prive = $prive; Vivants = $vivants }
}

# PNG (ou toute image que System.Drawing lit) → { L, H, Octets } en BGRA, rangé de haut en bas.
function Lire-Png([string]$Chemin) {
    $b = [System.Drawing.Bitmap]::new($Chemin)
    try {
        $r = [System.Drawing.Rectangle]::new(0, 0, $b.Width, $b.Height)
        $d = $b.LockBits($r, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
        try {
            $o = [byte[]]::new($b.Width * $b.Height * 4)
            for ($y = 0; $y -lt $b.Height; $y++) {
                [System.Runtime.InteropServices.Marshal]::Copy([IntPtr]::Add($d.Scan0, $y * $d.Stride), $o, $y * $b.Width * 4, $b.Width * 4)
            }
        }
        finally { $b.UnlockBits($d) }
        return [pscustomobject]@{ L = $b.Width; H = $b.Height; Octets = $o }
    }
    finally { $b.Dispose() }
}

# Ouvre un fichier dans une vue WebView2 (le moteur d'Edge ; exemple controle --genre
# lecture) ; rend la table de ce que la page a dit (duree_s, taille, premiere_image_ms,
# pastille r,g,b,a, images_presentees_2s, erreur_media, erreur_decode…), plus « _fin ».
function Lire-Dans-Edge([string]$Fichier, [ValidateSet('video', 'gif')][string]$Type, [string]$Temoin = '0,0') {
    $etat = Fichier-Temp "lecture-$([guid]::NewGuid().ToString('N').Substring(0, 6)).txt"
    $p = Lancer-Exemple 'controle' @('--genre', 'lecture', '--etat', "`"$etat`"", '--fichier', "`"$Fichier`"", '--type', $Type, '--temoin', $Temoin)
    if ($null -eq $p) { return @{ erreur = 'exemple controle absent'; _fin = '0' } }
    $fini = Attendre -DelaiS 45 -PasMs 200 -Condition { (Lire-Etat $etat).ContainsKey('fin') -or $p.HasExited }
    $e = Lire-Etat $etat
    $e['_fin'] = if ($fini) { '1' } else { '0' }
    if (-not $p.WaitForExit(5000)) { try { $p.Kill($true) } catch { } }
    if (-not $e.ContainsKey('fin') -and -not $e.ContainsKey('erreur')) { $e['erreur'] = "rien dit en 45 s$(if ($p.HasExited) { ", sortie code $(Code-Sortie $p.ExitCode)" })" }
    return $e
}

# Écart maximal par canal entre « r,g,b,a » et une couleur attendue.
function Ecart-Couleur([string]$Lu, [int[]]$Attendu) {
    $v = @("$Lu" -split ',' | ForEach-Object { Nombre $_ })
    if ($v.Count -lt 3 -or [double]::IsNaN($v[0])) { return [double]::NaN }
    return ([math]::Max([math]::Max([math]::Abs($v[0] - $Attendu[0]), [math]::Abs($v[1] - $Attendu[1])), [math]::Abs($v[2] - $Attendu[2])))
}

# ---------------------------------------------------------------------------------------
# Prises de S1 (exemple images), pour 02-s1-images.ps1 et 09-s1-wgc-pc.ps1

function Prise([string]$Methode, [int[]]$Zone, [double]$Fps, [double]$Duree, [string[]]$Plus = @()) {
    $a = @('--methode', $Methode, '--zone', (Rect-Texte $Zone), '--fps', "$Fps", '--duree', "$Duree") + $Plus
    return (Jouer-Exemple 'images' $a ($Duree + 60))
}

# Seuil du critère (contrat §1.1) : 28 valeurs k par seconde, ou 93 % du rythme du témoin
# -Rapide s'il est sous 30.
function Seuil-K([double]$Rythme) {
    if ([double]::IsNaN($Rythme) -or $Rythme -ge 30) { return 28.0 }
    return [math]::Round(0.93 * $Rythme, 1)
}

# Une prise montrée. -Rythme : pas par seconde du témoin pendant la prise. -Critere : celui
# du contrat pour la duplication sur tout l'écran à 30 i/s.
function Montrer-Prise([string]$Nom, $R, [double]$Rythme = [double]::NaN, [switch]$Critere) {
    if ($R['ok'] -ne '1') { NonMesurable $Nom "$($R['erreur'])"; return $null }
    $m = "$($R['methode'])"
    foreach ($k in 'adaptateur', 'dxgi_ouverture_ms', 'ouverture_ms', 'wgc_acces_sans_cadre', 'wgc_sans_cadre', 'wgc_cadre_relu', 'wgc_pointeur_pose', 'wgc_taille', 'wgc_premiere_image_ms') { if ($R.ContainsKey($k)) { Note "$Nom : $k $($R[$k])" } }
    Resultat "$Nom : prises par seconde" "$($R['prises_par_s']) ($($R['prises']) en $($R['duree_s']) s, créneaux manqués $($R['creneaux_manques']) ; images neuves $($R['neuves_par_s']) par seconde)" "$($R['fps_demande']) demandées" 'à lire'
    $med = Stat-Mediane $R['prise_neuve_ms']
    Resultat "$Nom : temps par prise" "toutes : $(Stats $R['prise_ms']) ; images neuves : $(Stats $R['prise_neuve_ms'])" $(if ($Critere) { 'médiane (acquisition, copie, lecture) des images neuves au plus 8 ms' } else { 'à lire (la 0.6 : 32 ms de médiane pour BitBlt)' }) $(if ($Critere) { Verdict ($med -le 8) } else { 'à lire' })
    if ($m -eq 'dxgi') {
        Resultat "$Nom : étapes" "acquisition (délai nul) $(Stats $R['acquisition_ms']) ; copie $(Stats $R['copie_ms']) ; lecture $(Stats $R['lecture_ms'])" 'à lire'
        Resultat "$Nom : images de l'écran" "neuves $($R['dxgi_ecran_neuves']) ; dont zone intacte (rectangles sales hors de la zone) $($R['dxgi_zone_intacte']) ; sans métadonnées $($R['dxgi_sans_metadonnees']) ; pointeur seul $($R['dxgi_pointeur_seul']) ; délai dépassé $($R['dxgi_delai_depasse'])" 'à lire'
        Resultat "$Nom : pertes d'accès (DXGI_ERROR_ACCESS_LOST)" "$($R['pertes_acces']) ; autres erreurs $($R['dxgi_autres_erreurs']) ; reprises $(Stats $R['duplication_refaite_ms'])$(if ($R.ContainsKey('dxgi_journal')) { " ; journal : $($R['dxgi_journal'])" })" 'à lire'
    }
    elseif ($m -eq 'wgc') { Resultat "$Nom : étapes" "image du pool (TryGetNextFrame) $(Stats $R['acquisition_ms']) ; copie de la zone $(Stats $R['copie_ms']) ; lecture $(Stats $R['lecture_ms'])" 'à lire' }
    else { Resultat "$Nom : étapes" "copie (BitBlt + GdiFlush) $(Stats $R['copie_ms']) ; lecture $(Stats $R['lecture_ms'])" 'à lire' }
    $kps = Nombre $R['k_distincts_par_s']
    if ($R.ContainsKey('k_distincts')) {
        $seuil = Seuil-K $Rythme
        $attendu = [math]::Min((Nombre $R['fps_demande']), $(if ([double]::IsNaN($Rythme)) { 999 } else { $Rythme }))
        Resultat "$Nom : valeurs k distinctes par seconde" "$($R['k_distincts_par_s']) ($($R['k_distincts']) valeurs, de $($R['k_premier_dernier'] -replace ';', ' à ')) ; témoin $([math]::Round($Rythme, 1)) pas/s ; illisibles $($R['k_illisibles']) ; déchirées $($R['k_dechirees']) ; sautées $($R['k_sautes']) ; reculs $($R['k_reculs'])" $(if ($Critere) { "au moins $seuil (28, ou 93 % du rythme du témoin s'il est sous 30), aucun recul" } else { "à lire (au mieux $([math]::Round($attendu, 1)))" }) $(if ($Critere) { Verdict ($kps -ge $seuil -and (Nombre $R['k_reculs']) -eq 0) } else { 'à lire' })
    }
    Resultat "$Nom : processeur" "$($R['processeur_pourcent_coeur']) % d'un cœur" 'à lire'
    if ($R.ContainsKey('pointeur_ms')) { Resultat "$Nom : lecture du pointeur à chaque image" "$(Stats $R['pointeur_ms']) ; pointeur montré $($R['pointeur_montre'])" 'à lire' }
    if ($R.ContainsKey('gdi_avant')) {
        $dg = (Nombre $R['gdi_apres']) - (Nombre $R['gdi_avant'])
        $dp = (Nombre $R['prive_apres']) - (Nombre $R['prive_avant'])
        $q = @("$($R['prive_quarts'])" -split ';' | ForEach-Object { Mo $_ }) -join ' / '
        Resultat "$Nom : (e) mémoire privée et objets GDI sur $($R['prises']) prises" "privée $(Mo $R['prive_avant']) → $(Mo $R['prive_apres']) (quarts $q ; pic $(Mo $R['prive_pic'])) ; GDI $($R['gdi_avant']) → $($R['gdi_apres']) (quarts $($R['gdi_quarts'] -replace ';', ' / ')) ; USER $($R['user_avant']) → $($R['user_apres'])" 'stables : GDI à 2 près, privée à 4 Mo près' (Verdict ([math]::Abs($dg) -le 2 -and $dp -le 4MB))
    }
    if ($Critere) {
        $ok = $kps -ge (Seuil-K $Rythme) -and $med -le 8
        Resultat "$Nom : critère S1 (sans le point a, plus bas)" "$($R['k_distincts_par_s']) valeurs k par seconde, médiane $(Ms $med)" "au moins $(Seuil-K $Rythme) valeurs k par seconde et au plus 8 ms" (Verdict $ok)
    }
    return $R
}

# Une prise montrée, avec le rythme du témoin pendant ce temps.
function Prise-Temoin($T, [string]$Nom, [string]$Methode, [int[]]$Zone, [double]$Fps, [double]$Duree, [string[]]$Plus = @(), [switch]$Critere) {
    $r = Prise $Methode $Zone $Fps $Duree (@('--temoin', "$($T.Zone[0]),$($T.Zone[1])") + $Plus)
    $ry = Rythme-Temoin $T $Duree
    return (Montrer-Prise $Nom $r $ry.ParS -Critere:$Critere)
}

# PNG d'une prise (une image) par une méthode ; rend { Image, Raison, Fichier }.
function Image([string]$Methode, [int[]]$Zone, [string]$Nom, [string[]]$Plus = @()) {
    $f = Fichier-Temp "$Nom.png"
    $r = Jouer-Exemple 'images' (@('--methode', $Methode, '--zone', (Rect-Texte $Zone), '--fps', '10', '--duree', '0.5', '--png', "`"$f`"") + $Plus) 60
    if ($r['ok'] -ne '1' -or -not (Test-Path -LiteralPath $f)) { return [pscustomobject]@{ Image = $null; Raison = "$($r['erreur'])"; Fichier = $f } }
    return [pscustomobject]@{ Image = (Lire-Png $f); Raison = ''; Fichier = $f }
}
