# Rassemble les résultats de tous les jobs de la sonde du film dans sonde-film-resultats.md :
# en-tête (commit, run, date), un tableau des verdicts par script, la comparaison du S2
# point 12 (b) (modules de l'exemple charge avant tout appel, contre la référence 0.6.1),
# puis chaque fichier <script>.md tel quel. Le fichier est aussi recopié en clair dans le
# journal du job et, s'il tient, dans le résumé du job.
#   pwsh -NoProfile -File sonde/film/12-resultats.ps1 -Entree <dossier des artefacts> -Sortie sonde-film-resultats.md
# Tourne partout (aucune mesure ici).

param(
    [Parameter(Mandatory)][string]$Entree,
    [string]$Sortie = 'sonde-film-resultats.md'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Continue'

$md = [System.Collections.Generic.List[string]]::new()
$md.Add('# Sonde du film (BreezyShot 0.7 « Vidéo et GIF », contrat partie I) : résultats bruts')
$md.Add('')
$run = if ($env:GITHUB_SERVER_URL -and $env:GITHUB_RUN_ID) { "$env:GITHUB_SERVER_URL/$env:GITHUB_REPOSITORY/actions/runs/$env:GITHUB_RUN_ID" } else { 'hors GitHub' }
$md.Add("Commit $(if ($env:GITHUB_SHA) { $env:GITHUB_SHA } else { '?' }) ; run $run ; rassemblé le $([DateTime]::UtcNow.ToString('yyyy-MM-dd HH:mm')) UTC.")
$md.Add('')
$md.Add('Verdicts : « passe » / « échoue » pour un critère du contrat, OUI / NON pour une question, « à lire » pour un chiffre sans seuil, « non mesuré » avec sa raison.')

$fichiers = @()
if (Test-Path -LiteralPath $Entree) {
    $fichiers = @(Get-ChildItem -LiteralPath $Entree -Recurse -File -Filter '*.md' | Where-Object { $_.Name -match '^\d\d-' } | Sort-Object Name)
}

# Tableau des verdicts.
$md.Add('')
$md.Add('## Verdicts par script')
$md.Add('')
$md.Add('| Script | passe | échoue | OUI | NON | à lire | non mesuré |')
$md.Add('| --- | --- | --- | --- | --- | --- | --- |')
$attendus = '00-inventaire', '01-s2-codeur-charge', '02-s1-images', '03-s2-chaine-mp4', '04-s4-gif', '05-s3-micro', '06-s5-controle', '07-s6-presse-papiers', '08-s7-ecrans', '09-s1-wgc-pc', '10-s2-finalize-ordinaire', '10-s2-finalize-moov', '10-s2-finalize-fragmente', '11-s2-modules-061'
foreach ($f in $fichiers) {
    $c = @{ 'passe' = 0; 'échoue' = 0; 'OUI' = 0; 'NON' = 0; 'à lire' = 0; 'non mesuré' = 0 }
    foreach ($l in (Get-Content -LiteralPath $f.FullName -Encoding utf8)) {
        if ($l -notmatch '^\|' -or $l -match '^\| (Mesure|---) ') { continue }
        $v = ($l.TrimEnd('|', ' ') -split '\|')[-1].Trim()
        if ($v -match '^cadre vu : (OUI|NON)$') { $v = $Matches[1] }
        if ($c.ContainsKey($v)) { $c[$v]++ }
    }
    $md.Add("| $($f.BaseName) | $($c['passe']) | $($c['échoue']) | $($c['OUI']) | $($c['NON']) | $($c['à lire']) | $($c['non mesuré']) |")
}
$vus = @($fichiers | ForEach-Object { $_.BaseName })
$manquants = @($attendus | Where-Object { $_ -notin $vus })
if ($manquants.Count) {
    $md.Add('')
    $md.Add("Scripts sans résultat (job arrêté ou artefact absent : voir le journal du job) : $($manquants -join ', ').")
}

# S2 point 12 (b).
$md.Add('')
$md.Add('## S2 (12b) : l''exemple charge avant tout appel, contre la référence 0.6.1')
$md.Add('')
$ref = @(Get-ChildItem -LiteralPath $Entree -Recurse -File -Filter 'sonde-film-modules-0.6.1.txt' -ErrorAction SilentlyContinue | Select-Object -First 1)
$av = @(Get-ChildItem -LiteralPath $Entree -Recurse -File -Filter 'charge-modules-avant.txt' -ErrorAction SilentlyContinue | Select-Object -First 1)
if (-not $ref.Count -or -not $av.Count) {
    $md.Add("NON MESURÉ : $(if (-not $ref.Count) { 'référence 0.6.1 absente (job app061)' }) $(if (-not $av.Count) { 'charge-modules-avant.txt absent (01-s2-codeur-charge)' })")
}
else {
    $reference = [System.Collections.Generic.HashSet[string]]::new()
    $section = ''
    foreach ($l in (Get-Content -LiteralPath $ref[0].FullName -Encoding utf8)) {
        if ($l -match '^\[(.+)\]$') { $section = $Matches[1]; continue }
        if ($section -eq 'sans licence' -and $l -and $l -notmatch '^#') { [void]$reference.Add((($l -split "`t")[0]).Trim().ToLowerInvariant()) }
    }
    $charge = @(Get-Content -LiteralPath $av[0].FullName -Encoding utf8 | ForEach-Object { $_.Trim().ToLowerInvariant() } | Where-Object { $_ })
    $listees = 'mfplat.dll', 'mfreadwrite.dll', 'mf.dll', 'dxgi.dll', 'd3d11.dll', 'mmdevapi.dll', 'audioses.dll', 'msmpeg2enc.dll'
    $md.Add('| Mesure | Valeur | Critère | Verdict |')
    $md.Add('| --- | --- | --- | --- |')
    $md.Add("| Référence 0.6.1, sans licence | $($reference.Count) modules | | à lire |")
    $md.Add("| Exemple charge avant tout appel | $($charge.Count) modules | | à lire |")
    $fautes = @()
    foreach ($n in $listees) {
        $dans = $charge -contains $n
        $enRef = $reference.Contains($n)
        $ok = (-not $dans) -or $enRef
        if (-not $ok) { $fautes += $n }
        $md.Add("| $n | charge : $(if ($dans) { 'chargée' } else { 'absente' }) ; référence : $(if ($enRef) { 'chargée' } else { 'absente' }) | absente de charge, ou déjà dans la référence | $(if ($ok) { 'passe' } else { 'échoue' }) |")
    }
    $md.Add("| Critère 12 (b) | $(if ($fautes.Count) { 'en plus de la référence : ' + ($fautes -join ', ') } else { 'aucune DLL listée en plus de la référence' }) | aucune | $(if ($fautes.Count) { 'échoue' } else { 'passe' }) |")
    $autres = @($charge | Where-Object { -not $reference.Contains($_) })
    $md.Add("| Autres modules de charge absents de la référence (pour lire) | $(if ($autres.Count) { $autres -join ', ' } else { 'aucun' }) | | à lire |")
}

foreach ($f in $fichiers) {
    $md.Add('')
    $md.Add('---')
    $md.Add('')
    $md.Add((Get-Content -LiteralPath $f.FullName -Raw -Encoding utf8).TrimEnd())
}
$texte = ($md -join "`n") + "`n"
[System.IO.File]::WriteAllText($Sortie, $texte, [System.Text.UTF8Encoding]::new($false))
Write-Host '=================== sonde-film-resultats.md ==================='
Write-Host $texte
Write-Host '================================================================'
if ($env:GITHUB_STEP_SUMMARY) {
    $resume = if ($texte.Length -lt 900000) { $texte } else { $texte.Substring(0, 900000) + "`n`n(coupé : le fichier entier est dans l'artefact sonde-film-resultats)`n" }
    Add-Content -LiteralPath $env:GITHUB_STEP_SUMMARY -Value $resume -Encoding utf8
}
exit 0
