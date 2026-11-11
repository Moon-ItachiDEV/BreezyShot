# Aides communes aux essais BreezyShot. À charger au début de chaque essai :
#   . "$PSScriptRoot/commun.ps1"
#   Initialiser-Essai -Nom '01-socle' -Exe $Exe -Sortie $Sortie
#   try { ... Verifier ... } catch { ... } finally { Terminer-Essai }
#
# Chaque essai tourne dans son propre dossier temporaire, désigné à l'app par la variable
# BREEZYSHOT_DOSSIER_ESSAI : il ne touche jamais aux vrais dossiers de l'utilisateur.
# PowerShell 7 (pwsh) ; marche aussi sous Linux avec le binaire de développement.

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script:Essai = $null
$script:Verifications = [System.Collections.Generic.List[object]]::new()
$script:Mesures = [System.Collections.Generic.List[object]]::new()

# Nom du processus de l'app (breezyshot.exe sous Windows).
$script:NomProcessus = 'breezyshot'

function Initialiser-Essai {
    param(
        [Parameter(Mandatory)][string]$Nom,
        [Parameter(Mandatory)][string]$Exe,
        [Parameter(Mandatory)][string]$Sortie
    )
    $exeComplet = (Resolve-Path -LiteralPath $Exe).Path
    New-Item -ItemType Directory -Force -Path $Sortie | Out-Null
    $script:Essai = [pscustomobject]@{
        Nom     = $Nom
        Exe     = $exeComplet
        Sortie  = (Resolve-Path -LiteralPath $Sortie).Path
        Dossier = Nouveau-DossierEssai -Nom $Nom
    }
    Write-Host "=== Essai $Nom ==="
    Write-Host "app : $exeComplet"
    Write-Host "dossier d'essai : $($script:Essai.Dossier)"
    # Un essai part toujours d'une machine sans BreezyShot en cours.
    Arreter-Tout
    return $script:Essai
}

function Nouveau-DossierEssai {
    param([string]$Nom = 'essai')
    $chemin = Join-Path ([System.IO.Path]::GetTempPath()) ("breezyshot-$Nom-" + [guid]::NewGuid().ToString('N').Substring(0, 8))
    New-Item -ItemType Directory -Force -Path $chemin | Out-Null
    return $chemin
}

# Lance l'app avec BREEZYSHOT_DOSSIER_ESSAI pointé sur $Dossier. Rend le processus.
function Demarrer-App {
    param(
        [string]$Exe = $script:Essai.Exe,
        [string]$Dossier = $script:Essai.Dossier,
        [string[]]$Arguments = @()
    )
    $ancien = $env:BREEZYSHOT_DOSSIER_ESSAI
    $env:BREEZYSHOT_DOSSIER_ESSAI = $Dossier
    try {
        $options = @{ FilePath = $Exe; PassThru = $true }
        if ($Arguments.Count -gt 0) { $options.ArgumentList = $Arguments }
        $p = Start-Process @options
        # Lire Handle tout de suite garde le code de sortie disponible plus tard.
        $null = $p.Handle
        return $p
    }
    finally {
        $env:BREEZYSHOT_DOSSIER_ESSAI = $ancien
    }
}

# Lance l'app et attend qu'elle sorte. Rend Fini (sortie avant le délai), Code, DureeS.
function Lancer-Et-Attendre {
    param(
        [string[]]$Arguments = @(),
        [double]$DelaiS = 10,
        [string]$Exe = $script:Essai.Exe,
        [string]$Dossier = $script:Essai.Dossier
    )
    $chrono = [System.Diagnostics.Stopwatch]::StartNew()
    $p = Demarrer-App -Exe $Exe -Dossier $Dossier -Arguments $Arguments
    $fini = $p.WaitForExit([int]($DelaiS * 1000))
    $chrono.Stop()
    $code = $null
    if ($fini) {
        $p.WaitForExit()
        $code = $p.ExitCode
    }
    else {
        try { $p.Kill() } catch { }
    }
    return [pscustomobject]@{ Fini = $fini; Code = $code; DureeS = [math]::Round($chrono.Elapsed.TotalSeconds, 2) }
}

# Demande à l'instance en cours de se fermer (--quitter).
function Quitter-App {
    param([double]$DelaiS = 10, [string]$Dossier = $script:Essai.Dossier)
    return Lancer-Et-Attendre -Arguments @('--quitter') -DelaiS $DelaiS -Dossier $Dossier
}

function Chemin-Journal {
    param([string]$Dossier = $script:Essai.Dossier)
    return Join-Path $Dossier 'local/Journal/breezyshot.log'
}

# Lit le journal sans gêner l'app qui l'a ouvert en écriture.
function Lire-Journal {
    param([string]$Dossier = $script:Essai.Dossier)
    $chemin = Chemin-Journal -Dossier $Dossier
    if (-not (Test-Path -LiteralPath $chemin)) { return '' }
    try {
        $flux = [System.IO.FileStream]::new($chemin, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]'ReadWrite, Delete')
        try {
            $lecteur = [System.IO.StreamReader]::new($flux, [System.Text.Encoding]::UTF8)
            return $lecteur.ReadToEnd()
        }
        finally { $flux.Dispose() }
    }
    catch { return '' }
}

# Nombre de lignes du journal qui contiennent $Texte (texte exact, pas une expression).
function Compter-Lignes {
    param([Parameter(Mandatory)][string]$Texte, [string]$Dossier = $script:Essai.Dossier)
    $n = 0
    foreach ($ligne in ((Lire-Journal -Dossier $Dossier) -split "`n")) {
        if ($ligne.Contains($Texte)) { $n++ }
    }
    return $n
}

# Attend qu'au moins $Fois lignes contiennent $Texte. Rend $true si c'est arrivé à temps.
function Attendre-Ligne {
    param(
        [Parameter(Mandatory)][string]$Texte,
        [double]$DelaiS = 10,
        [int]$Fois = 1,
        [string]$Dossier = $script:Essai.Dossier
    )
    $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
    do {
        if ((Compter-Lignes -Texte $Texte -Dossier $Dossier) -ge $Fois) { return $true }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $limite)
    return $false
}

# Les processus BreezyShot en cours.
function Processus-App {
    return @(Get-Process -Name $script:NomProcessus -ErrorAction SilentlyContinue)
}

# Mémoire privée (Mo) du processus et de tous ses descendants (processus du moteur web).
function Memoire-Privee-Mo {
    param([Parameter(Mandatory)][System.Diagnostics.Process]$Processus)
    $tous = @(Get-Process -ErrorAction SilentlyContinue)
    $enfants = @{}
    foreach ($q in $tous) {
        try { $parent = $q.Parent } catch { $parent = $null }
        if ($null -ne $parent) {
            if (-not $enfants.ContainsKey($parent.Id)) { $enfants[$parent.Id] = [System.Collections.Generic.List[object]]::new() }
            $enfants[$parent.Id].Add($q)
        }
    }
    $Processus.Refresh()
    $total = [long]$Processus.PrivateMemorySize64
    $a_voir = [System.Collections.Generic.Queue[int]]::new()
    $a_voir.Enqueue($Processus.Id)
    $vus = @{ $Processus.Id = $true }
    while ($a_voir.Count -gt 0) {
        $id = $a_voir.Dequeue()
        if (-not $enfants.ContainsKey($id)) { continue }
        foreach ($e in $enfants[$id]) {
            if ($vus.ContainsKey($e.Id)) { continue }
            $vus[$e.Id] = $true
            $total += [long]$e.PrivateMemorySize64
            $a_voir.Enqueue($e.Id)
        }
    }
    return [math]::Round($total / 1MB, 1)
}

# Ferme de force tout BreezyShot restant (nettoyage, jamais une vérification).
function Arreter-Tout {
    foreach ($p in (Processus-App)) {
        try { $p.Kill(); $null = $p.WaitForExit(5000) } catch { }
    }
}

# Note une vérification. Rend la condition, pour enchaîner si besoin.
function Verifier {
    param(
        [Parameter(Mandatory)][string]$Nom,
        [Parameter(Mandatory)][bool]$Condition,
        [string]$Detail = ''
    )
    $script:Verifications.Add([pscustomobject]@{ Nom = $Nom; Reussi = $Condition; Detail = $Detail })
    $etiquette = if ($Condition) { '[ok]    ' } else { '[ECHEC] ' }
    $suite = if ($Detail) { " ($Detail)" } else { '' }
    Write-Host "$etiquette$Nom$suite"
    return $Condition
}

# Note une mesure pour le résumé (mémoire, chrono…).
function Noter-Mesure {
    param([Parameter(Mandatory)][string]$Nom, [Parameter(Mandatory)]$Valeur, [string]$Unite = '')
    $script:Mesures.Add([pscustomobject]@{ Nom = $Nom; Valeur = $Valeur; Unite = $Unite })
    Write-Host "[mesure] $Nom : $Valeur $Unite"
}

# Range le journal et le bilan dans $Sortie, ferme ce qui reste, sort avec 0 ou 1.
function Terminer-Essai {
    Arreter-Tout
    $e = $script:Essai
    $code = 1
    if ($null -ne $e) {
        $dossierJournaux = Join-Path $e.Sortie "journaux/$($e.Nom)"
        New-Item -ItemType Directory -Force -Path $dossierJournaux | Out-Null
        $journaux = Join-Path $e.Dossier 'local/Journal'
        if (Test-Path -LiteralPath $journaux) {
            Copy-Item -Path (Join-Path $journaux '*') -Destination $dossierJournaux -Force
        }
        $echecs = @($script:Verifications | Where-Object { -not $_.Reussi }).Count
        $reussi = ($script:Verifications.Count -gt 0) -and ($echecs -eq 0)
        $bilan = [pscustomobject]@{
            Nom           = $e.Nom
            Reussi        = $reussi
            Verifications = @($script:Verifications)
            Mesures       = @($script:Mesures)
        }
        $bilan | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $e.Sortie "$($e.Nom).json") -Encoding utf8
        Remove-Item -LiteralPath $e.Dossier -Recurse -Force -ErrorAction SilentlyContinue
        if ($reussi) { $code = 0 }
        Write-Host "=== $($e.Nom) : $(if ($reussi) { 'réussi' } else { "échoué ($echecs vérification(s))" }) ==="
    }
    exit $code
}
