# S2 point 12 (a) (contrat 0.7, §1.2) : les modules de l'app 0.6.1 au repos, la référence.
# L'app de main (construite par le job app061) démarre dans un dossier d'essai
# (BREEZYSHOT_DOSSIER_ESSAI) ; 10 s après « pret » dans son journal, Get-Process -Module de
# son processus ; puis --quitter. Deux fois : sans licence, puis avec licence.txt posé avant
# le démarrage (essais/licence-essai.txt, UTF-8 sans BOM, journal : « licence active »).
# Écrit sonde-resultats/sonde-film-modules-0.6.1.txt (une ligne « nom<TAB>chemin » par
# module, triées, sous « [sans licence] » et « [avec licence] ») : la référence de l'essai
# 15, à ranger dans docs/sonde-film-modules-0.6.1.txt. Y trouve-t-on déjà dxgi, d3d11,
# mfplat, mmdevapi ?
#   pwsh -NoProfile -File sonde/film/11-s2-modules-061.ps1 -Exe <breezyshot.exe> -Licence <licence-essai.txt>

param([string]$Exe = '', [string]$Licence = '')

. "$PSScriptRoot/commun.ps1"

$surveillees = 'dxgi.dll', 'd3d11.dll', 'mfplat.dll', 'mfreadwrite.dll', 'mf.dll', 'mmdevapi.dll', 'audioses.dll', 'msmpeg2enc.dll'

function Journal-App([string]$Dossier) {
    $f = Join-Path $Dossier 'local/Journal/breezyshot.log'
    if (-not (Test-Path -LiteralPath $f)) { return '' }
    try {
        $fs = [System.IO.FileStream]::new($f, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]'ReadWrite, Delete')
        try { return [System.IO.StreamReader]::new($fs, [System.Text.Encoding]::UTF8).ReadToEnd() } finally { $fs.Dispose() }
    }
    catch { return '' }
}

function Lancer-App([string]$Dossier, [string[]]$Arguments = @()) {
    $ancien = $env:BREEZYSHOT_DOSSIER_ESSAI
    $env:BREEZYSHOT_DOSSIER_ESSAI = $Dossier
    try {
        $o = @{ FilePath = $Exe; PassThru = $true }
        if ($Arguments.Count) { $o.ArgumentList = $Arguments }
        $p = Start-Process @o
        $null = $p.Handle
        return $p
    }
    finally { $env:BREEZYSHOT_DOSSIER_ESSAI = $ancien }
}

# Un démarrage : rend { Modules (nom → chemin), Pret, Licence (ligne du journal), Prive,
# Raison }.
function Modules-Au-Repos([string]$Cas, [switch]$AvecLicence) {
    $d = Fichier-Temp "app061-$Cas"
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    if ($AvecLicence) {
        New-Item -ItemType Directory -Force -Path (Join-Path $d 'reglages') | Out-Null
        $l = "$(Get-Content -LiteralPath $Licence -Raw -Encoding utf8)".Trim()
        [System.IO.File]::WriteAllText((Join-Path $d 'reglages/licence.txt'), $l, [System.Text.UTF8Encoding]::new($false))
    }
    $o = [pscustomobject]@{ Modules = [ordered]@{}; Pret = $false; Licence = ''; Prive = 0L; Raison = ''; Webview = 0 }
    $p = Garder-Processus (Lancer-App $d)
    try {
        $o.Pret = Attendre -DelaiS 60 -PasMs 250 -Condition { (Journal-App $d) -match '(?m)application +pret\s*$' -or $p.HasExited }
        if ($p.HasExited) { $o.Raison = "sortie, code $(Code-Sortie $p.ExitCode)"; return $o }
        if (-not $o.Pret) { $o.Raison = '« pret » absent du journal en 60 s (mesure faite quand même)' }
        Start-Sleep -Seconds 10
        $p.Refresh()
        $o.Prive = $p.PrivateMemorySize64
        $mods = @(Get-Process -Id $p.Id -Module -ErrorAction Stop | Sort-Object { $_.ModuleName.ToLowerInvariant() })
        foreach ($m in $mods) { $o.Modules[$m.ModuleName.ToLowerInvariant()] = $m.FileName }
        $o.Licence = @((Journal-App $d) -split "`r?`n" | Where-Object { $_ -match 'licence' } | Select-Object -Last 1) -join ''
        $o.Webview = @(Get-CimInstance Win32_Process -Filter "Name = 'msedgewebview2.exe'" -ErrorAction SilentlyContinue | Where-Object { "$($_.CommandLine)".Contains($d) }).Count
    }
    catch { $o.Raison = "$($_.Exception.Message)" }
    finally {
        $q = Lancer-App $d @('--quitter')
        $null = $q.WaitForExit(15000)
        if (-not $p.WaitForExit(15000)) { try { $p.Kill($true) } catch { } }
    }
    return $o
}

if (Demarrer-Sonde -Nom '11-s2-modules-061' -Titre 'S2 point 12 (a) : modules de l''app 0.6.1 au repos (la référence)') {
    try {
        if (-not $Exe -or -not (Test-Path -LiteralPath $Exe)) { NonMesurable 'app 0.6.1' "exécutable absent ($Exe) : le job app061 a-t-il construit main ?" }
        elseif (-not $Licence -or -not (Test-Path -LiteralPath $Licence)) { NonMesurable 'app 0.6.1' "licence d'essai absente ($Licence)" }
        else {
            $v = (Get-Item -LiteralPath $Exe).VersionInfo
            Resultat 'Exécutable' "$Exe ; version $($v.ProductVersion) ; $(Mo (Get-Item -LiteralPath $Exe).Length)$(if ($env:SONDE_MAIN_SHA) { " ; main $env:SONDE_MAIN_SHA" })" '0.6.1' 'à lire'
            $sortie = [System.Collections.Generic.List[string]]::new()
            $sortie.Add("# BreezyShot $($v.ProductVersion)$(if ($env:SONDE_MAIN_SHA) { " (main $env:SONDE_MAIN_SHA)" }) : modules du processus (Get-Process -Module), 10 s après « pret », dossier d'essai.")
            $sortie.Add("# $($script:Sonde.Machine)")
            $sortie.Add('# Une ligne « nom<TAB>chemin » par module, noms en minuscules, triés.')
            $listes = @{}
            foreach ($cas in 'sans licence', 'avec licence') {
                Section "S2 (12a) : $cas"
                $o = Modules-Au-Repos ($cas -replace ' ', '-') -AvecLicence:($cas -eq 'avec licence')
                if ($o.Modules.Count -eq 0) { NonMesurable "modules $cas" "$($o.Raison)"; continue }
                $listes[$cas] = $o.Modules
                Resultat 'Démarrage' "pret dans le journal : $(OuiNon $o.Pret)$(if ($o.Raison) { " ; $($o.Raison)" }) ; journal : $($o.Licence)" $(if ($cas -eq 'avec licence') { '« licence active »' } else { '« licence absente »' }) (OuiNon ($o.Licence -match $(if ($cas -eq 'avec licence') { 'licence active' } else { 'licence absente' })))
                Resultat 'Modules du processus' "$($o.Modules.Count) ; mémoire privée $(Mo $o.Prive) ; processus WebView2 de l'app $($o.Webview)" 'à lire' 'à lire'
                foreach ($s in $surveillees) { Resultat "Déjà chargé : $s" "$(OuiNon $o.Modules.Contains($s))$(if ($o.Modules.Contains($s)) { " ($($o.Modules[$s]))" })" 'à lire (point 12a)' (OuiNon $o.Modules.Contains($s)) }
                $sortie.Add('')
                $sortie.Add("[$cas]")
                foreach ($k in $o.Modules.Keys) { $sortie.Add("$k`t$($o.Modules[$k])") }
            }
            if ($listes.Count -eq 2) {
                $plus = @($listes['avec licence'].Keys | Where-Object { -not $listes['sans licence'].Contains($_) })
                $moins = @($listes['sans licence'].Keys | Where-Object { -not $listes['avec licence'].Contains($_) })
                Section 'S2 (12a) : écart entre les deux'
                Resultat 'Modules en plus avec la licence' "$($plus.Count)$(if ($plus.Count) { ' : ' + ($plus -join ', ') })" 'à lire' 'à lire'
                Resultat 'Modules en moins avec la licence' "$($moins.Count)$(if ($moins.Count) { ' : ' + ($moins -join ', ') })" 'à lire' 'à lire'
            }
            $f = Join-Path $script:Sonde.Sortie 'sonde-film-modules-0.6.1.txt'
            [System.IO.File]::WriteAllLines($f, $sortie, [System.Text.UTF8Encoding]::new($false))
            # En clair dans le journal, entre deux lignes repères, pour se recopier sans artefact.
            Write-Host '----- DEBUT sonde-film-modules-0.6.1.txt -----'
            foreach ($l in $sortie) { Write-Host $l }
            Write-Host '----- FIN sonde-film-modules-0.6.1.txt -----'
            Note "Référence écrite : $f (à ranger dans docs/sonde-film-modules-0.6.1.txt ; la comparaison 12 (b) est faite par 12-resultats.ps1)."
        }
    }
    catch { Erreur-Sonde $_ }
}
Terminer-Sonde
