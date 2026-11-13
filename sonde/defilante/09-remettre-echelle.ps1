# Étape à part du workflow (if: always()) : remet l'échelle de l'écran que S8 a changée,
# même si S8 a été arrêté (délai, erreur). L'échelle d'origine (pas relatif, 0 =
# recommandée) est lue dans breezyshot-sonde-echelle.txt ($env:RUNNER_TEMP, sinon le
# dossier temporaire) ; sans ce fichier, l'échelle recommandée (0) est remise si elle n'est
# pas déjà là. N'échoue jamais.
#   pwsh -NoProfile -File sonde/defilante/09-remettre-echelle.ps1

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
try {
    if (-not $IsWindows) { 'pas sous Windows : rien à remettre'; exit 0 }
    if (-not ('BreezyShotSonde.Natif' -as [type])) {
        Add-Type -TypeDefinition (Get-Content -LiteralPath (Join-Path $PSScriptRoot 'natif.cs') -Raw)
    }
    $NATIF = [BreezyShotSonde.Natif]
    try { [void]$NATIF::SetProcessDpiAwarenessContext([IntPtr]::new(-4)) } catch { }
    $f = Join-Path $(if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [System.IO.Path]::GetTempPath() }) 'breezyshot-sonde-echelle.txt'
    $voulu = 0
    if (Test-Path -LiteralPath $f) { $voulu = [int](Get-Content -LiteralPath $f -Raw).Trim() }
    $e = $NATIF::EchelleRelative()
    if ($e[0] -ne 0) { "échelle illisible (code $($e[0])) : rien remis"; exit 0 }
    "échelle relative lue $($e[2]) ; voulue $voulu ; DPI effectif $($NATIF::DpiEcran(10, 10))"
    if ($e[2] -ne $voulu) {
        $code = $NATIF::FixerEchelleRelative($voulu)
        $limite = [DateTime]::UtcNow.AddSeconds(8)
        while ([DateTime]::UtcNow -lt $limite -and $NATIF::EchelleRelative()[2] -ne $voulu) { Start-Sleep -Milliseconds 250 }
        "remise : code $code ; échelle relative relue $($NATIF::EchelleRelative()[2]) ; DPI effectif $($NATIF::DpiEcran(10, 10))"
    }
    else { 'échelle déjà à l''origine' }
}
catch { "remise de l'échelle : $($_.Exception.Message)" }
exit 0
