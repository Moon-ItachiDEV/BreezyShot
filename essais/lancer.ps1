# Joue tous les essais essais/NN-*.ps1 dans l'ordre (01-socle … 14-defilante), chacun dans
# son propre pwsh. Les aides (temoin*.ps1, tenir-raccourci.ps1, cible-glisser.ps1) ne sont
# pas des essais.
# Sort avec 1 si un essai échoue. Écrit un résumé lisible, aussi dans $env:GITHUB_STEP_SUMMARY.
#
#   pwsh essais/lancer.ps1 -Exe app/coeur/target/release/breezyshot.exe [-Sortie essais-resultats]

param(
    [Parameter(Mandatory)][string]$Exe,
    [string]$Sortie = 'essais-resultats',
    # Un essai qui n'a pas fini dans ce délai est arrêté (lui et tout ce qu'il a lancé) et
    # compte comme échoué : les suivants se jouent quand même. L'essai 13 (historique, texte
    # et licence, six à sept minutes s'il réussit) et l'essai 14 (capture défilante, une
    # douzaine de défilantes, Edge et l'Explorateur) ont au moins 900 s.
    [int]$LimiteS = 600
)

$LIMITES_PROPRES = @{ '13-historique-texte' = 900; '14-defilante' = 900 }

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$exeComplet = (Resolve-Path -LiteralPath $Exe).Path
New-Item -ItemType Directory -Force -Path $Sortie | Out-Null
$sortieComplete = (Resolve-Path -LiteralPath $Sortie).Path
$pwsh = (Get-Process -Id $PID).Path

$essais = @(Get-ChildItem -LiteralPath $PSScriptRoot -File |
    Where-Object { $_.Name -match '^\d\d-.+\.ps1$' } |
    Sort-Object Name)
if ($essais.Count -eq 0) { throw "aucun essai trouvé dans $PSScriptRoot" }

$bilans = [System.Collections.Generic.List[object]]::new()
foreach ($fichier in $essais) {
    $nom = [System.IO.Path]::GetFileNameWithoutExtension($fichier.Name)
    $limite = if ($LIMITES_PROPRES.ContainsKey($nom)) { [math]::Max($LimiteS, $LIMITES_PROPRES[$nom]) } else { $LimiteS }
    $chrono = [System.Diagnostics.Stopwatch]::StartNew()
    $avant = @(Get-Process | ForEach-Object { $_.Id })
    $json = Join-Path $sortieComplete "$nom.json"
    Remove-Item -LiteralPath $json -Force -ErrorAction SilentlyContinue   # jamais un vieux bilan
    $arguments = @('-NoProfile', '-File', "`"$($fichier.FullName)`"",
        '-Exe', "`"$exeComplet`"", '-Sortie', "`"$sortieComplete`"")
    $processus = Start-Process -FilePath $pwsh -ArgumentList $arguments -NoNewWindow -PassThru
    $null = $processus.Handle   # sans quoi ExitCode reste vide après la fin
    if ($processus.WaitForExit($limite * 1000)) {
        $code = $processus.ExitCode
    }
    else {
        Write-Host "$nom : pas fini en $limite s, arrêté."
        try { $processus.Kill($true) } catch { Write-Host "$nom : arrêt impossible ($_)" }
        $code = -1
        # L'essai arrêté n'a pas rangé les journaux de l'app : ils sont pris dans son dossier.
        $temp = [System.IO.Path]::GetTempPath()
        foreach ($d in @(Get-ChildItem -LiteralPath $temp -Directory -Filter "breezyshot-$nom-*" -ErrorAction SilentlyContinue)) {
            $journaux = @(Get-ChildItem -LiteralPath $d.FullName -Directory -Recurse -Depth 3 -Filter 'Journal' -ErrorAction SilentlyContinue |
                Where-Object { $_.Parent.Name -eq 'local' })
            foreach ($j in $journaux) {
                $cas = [System.IO.Path]::GetRelativePath($d.FullName, $j.Parent.Parent.FullName)
                $ici = Join-Path $sortieComplete "journaux/$nom"
                if ($cas -ne '.') { $ici = Join-Path $ici $cas }
                New-Item -ItemType Directory -Force -Path $ici | Out-Null
                Copy-Item -Path (Join-Path $j.FullName '*') -Destination $ici -Force -ErrorAction SilentlyContinue
            }
        }
    }
    $chrono.Stop()
    # Rien de ce qu'un essai a lancé ne doit lui survivre : l'app, une aide, ou les vues Web
    # de l'app (WebView2 dit pour qui il tourne dans sa ligne de commande ; celui d'un autre
    # programme n'est pas touché). L'essai suivant attend qu'ils soient partis.
    $restes = @(Get-Process | Where-Object {
            $_.Id -ne $PID -and $_.Id -notin $avant -and
            $_.ProcessName -match '^(breezyshot.*|pwsh|msedgewebview2)$'
        } | Where-Object {
            if ($_.ProcessName -ne 'msedgewebview2') { return $true }
            $ligne = ''
            try { $ligne = "$((Get-CimInstance Win32_Process -Filter "ProcessId = $($_.Id)" -ErrorAction Stop).CommandLine)" } catch { }
            return $ligne -match '--webview-exe-name=breezyshot'
        })
    if ($restes.Count -gt 0) {
        $noms = ($restes | ForEach-Object { "$($_.ProcessName) ($($_.Id))" }) -join ', '
        Write-Host "$nom : processus restés ouverts, arrêtés : $noms"
        $restes | ForEach-Object { try { $_.Kill(); $null = $_.WaitForExit(5000) } catch { } }
    }
    $bilan = $null
    if (Test-Path -LiteralPath $json) { $bilan = Get-Content -LiteralPath $json -Raw | ConvertFrom-Json }
    $bilans.Add([pscustomobject]@{
            Nom    = $nom
            Reussi = ($code -eq 0) -and ($null -ne $bilan) -and $bilan.Reussi
            Code   = $code
            DureeS = [math]::Round($chrono.Elapsed.TotalSeconds, 1)
            LimiteS = $limite
            Bilan  = $bilan
        })
}

# Résumé en Markdown.
$lignes = [System.Collections.Generic.List[string]]::new()
$lignes.Add('## Essais BreezyShot')
$lignes.Add('')
$lignes.Add('| Essai | Résultat | Durée |')
$lignes.Add('| --- | --- | --- |')
foreach ($b in $bilans) {
    $resultat = if ($b.Reussi) { 'réussi' } else { "**échoué** (code $($b.Code))" }
    $lignes.Add("| $($b.Nom) | $resultat | $($b.DureeS) s |")
}
foreach ($b in $bilans) {
    $lignes.Add('')
    $lignes.Add("### $($b.Nom)")
    $lignes.Add('')
    if ($null -eq $b.Bilan) {
        $cause = if ($b.Code -eq -1) { "arrêté après $($b.LimiteS) s" } else { "code $($b.Code)" }
        $lignes.Add("Pas de bilan : l'essai s'est arrêté avant la fin ($cause).")
        continue
    }
    foreach ($v in @($b.Bilan.Verifications)) {
        $marque = if ($v.Reussi) { 'ok' } else { '**ÉCHEC**' }
        $detail = if ($v.Detail) { " ($($v.Detail))" } else { '' }
        $lignes.Add("- $marque : $($v.Nom)$detail")
    }
    $mesures = @($b.Bilan.Mesures)
    if ($mesures.Count -gt 0) {
        $lignes.Add('')
        $lignes.Add('| Mesure | Valeur |')
        $lignes.Add('| --- | --- |')
        foreach ($m in $mesures) { $lignes.Add("| $($m.Nom) | $($m.Valeur) $($m.Unite) |") }
    }
}
$texte = $lignes -join "`n"
Write-Host ''
Write-Host $texte
Set-Content -LiteralPath (Join-Path $sortieComplete 'resume.md') -Value $texte -Encoding utf8
if ($env:GITHUB_STEP_SUMMARY) {
    Add-Content -LiteralPath $env:GITHUB_STEP_SUMMARY -Value $texte -Encoding utf8
}

$echecs = @($bilans | Where-Object { -not $_.Reussi }).Count
if ($echecs -gt 0) {
    Write-Host "$echecs essai(s) échoué(s)."
    exit 1
}
Write-Host 'Tous les essais ont réussi.'
exit 0
