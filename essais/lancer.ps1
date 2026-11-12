# Joue tous les essais essais/NN-*.ps1 dans l'ordre (01-socle … 12-editeur), chacun dans
# son propre pwsh. Les aides (temoin.ps1, tenir-raccourci.ps1, cible-glisser.ps1) ne sont
# pas des essais.
# Sort avec 1 si un essai échoue. Écrit un résumé lisible, aussi dans $env:GITHUB_STEP_SUMMARY.
#
#   pwsh essais/lancer.ps1 -Exe app/coeur/target/release/breezyshot.exe [-Sortie essais-resultats]

param(
    [Parameter(Mandatory)][string]$Exe,
    [string]$Sortie = 'essais-resultats'
)

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
    $chrono = [System.Diagnostics.Stopwatch]::StartNew()
    & $pwsh -NoProfile -File $fichier.FullName -Exe $exeComplet -Sortie $sortieComplete
    $code = $LASTEXITCODE
    $chrono.Stop()
    $json = Join-Path $sortieComplete "$nom.json"
    $bilan = $null
    if (Test-Path -LiteralPath $json) { $bilan = Get-Content -LiteralPath $json -Raw | ConvertFrom-Json }
    $bilans.Add([pscustomobject]@{
            Nom    = $nom
            Reussi = ($code -eq 0) -and ($null -ne $bilan) -and $bilan.Reussi
            Code   = $code
            DureeS = [math]::Round($chrono.Elapsed.TotalSeconds, 1)
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
        $lignes.Add("Pas de bilan : l'essai s'est arrêté avant la fin (code $($b.Code)).")
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
