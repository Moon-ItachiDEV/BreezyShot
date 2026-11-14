# S2 points 1, 2, 8 et 12 (b, c) (contrat 0.7, §1.2, premiers de l'ordre du §1.8) :
# 1. MFStartup ; MFTEnumEx : codeurs H.264 (nom, matériel ou logiciel), codeur AAC, et les
#    décodeurs (pour la relecture). Sans codeur H.264 : Install-WindowsFeature
#    Server-Media-Foundation (durée, redémarrage demandé ou non), puis la même liste.
# 2. IMFSinkWriter vers un .mp4, entrée RGB32, sortie H.264, pour 600 × 450 … 16 × 16 :
#    accepté ou refusé, et avec quel code.
# 8. a. L'exemple `charge` (lié /DELAYLOAD:mfplat, mfreadwrite, mf, dxgi, d3d11) démarre,
#       ne liste aucune de ces DLL dans Get-Process -Module avant le premier appel, et les
#       liste après ; le témoin `charge-temoin` (même code, sans /DELAYLOAD) les liste dès le
#       lancement.
#    b. La garde (LoadLibraryExW(nom, LOAD_LIBRARY_SEARCH_SYSTEM32)) pour mfplat, mfreadwrite
#       et mf, avant et après l'installation si elle a lieu.
#    c. La DLL inventée bzs-absente.dll : appelée sans garde (code de sortie), avec la garde
#       (fin propre).
# 12. b. Les modules de `charge` avant tout appel sont gardés (charge-modules-avant.txt) :
#        l'étape des résultats les compare à la référence de l'app 0.6.1.
#     c. Après la recherche du codeur comme F32 (MFStartup, MFTEnumEx sans ActivateObject,
#        MFShutdown) : modules en plus, mémoire privée en plus, DLL du pilote graphique.
#   pwsh -NoProfile -File sonde/film/01-s2-codeur-charge.ps1

. "$PSScriptRoot/commun.ps1"

$script:SUIVIES = @('mfplat.dll', 'mfreadwrite.dll', 'mf.dll', 'dxgi.dll', 'd3d11.dll')

function Liste-Codeurs([string]$Quand) {
    $r = Jouer-Exemple 'mf' @('--etape', 'liste') 90
    if ($r['ok'] -ne '1') { NonMesurable "MFStartup et MFTEnumEx ($Quand)" "$($r['erreur'])"; return $r }
    Resultat "MFStartup ($Quand)" "OUI, $($r['mfstartup_ms']) ms" 'réussit' 'passe'
    foreach ($g in 'codeur_h264', 'codeur_aac', 'decodeur_h264', 'decodeur_aac') {
        if ($r.ContainsKey("$($g)_erreur")) { NonMesurable "$g ($Quand)" "$($r["$($g)_erreur"])"; continue }
        $n = [int]$r["$($g)_nombre"]
        $noms = @(); for ($i = 0; $i -lt $n; $i++) { if ($r.ContainsKey("$($g)_$i")) { $noms += $r["$($g)_$i"] } }
        $crit = if ($g -eq 'codeur_h264') { 'au moins un (sinon la voie MP4 ne se prouve pas ici, §16 R4)' } else { 'à lire' }
        $ver = if ($g -eq 'codeur_h264') { Verdict ($n -gt 0) } else { OuiNon ($n -gt 0) }
        Resultat "$g ($Quand)" "$n trouvé(s), dont $($r["$($g)_materiels"]) matériel(s)$(if ($noms) { " : $($noms -join ' / ')" })" $crit $ver
    }
    return $r
}

function Garde-PS([string]$Quand) {
    foreach ($d in 'mfplat.dll', 'mfreadwrite.dll', 'mf.dll') {
        $g = $script:Natif::Garde($d)
        Resultat "8b : garde LoadLibraryExW($d, LOAD_LIBRARY_SEARCH_SYSTEM32) ($Quand, depuis PowerShell)" $g 'à lire : 1 si Media Foundation est là' (OuiNon ($g -eq '1'))
    }
}

# DLL de pilote graphique connues : celles que Win32_VideoController annonce, et des noms
# de pilotes courants (Intel, NVIDIA, AMD).
function Dll-Pilotes {
    $v = [System.Collections.Generic.HashSet[string]]::new()
    try {
        foreach ($c in @(Get-CimInstance Win32_VideoController -ErrorAction Stop)) {
            foreach ($x in "$($c.InstalledDisplayDrivers)" -split ',') { $n = (Split-Path -Leaf $x.Trim()).ToLowerInvariant(); if ($n) { [void]$v.Add($(if ($n.EndsWith('.dll')) { $n } else { "$n.dll" })) } }
        }
    }
    catch { }
    foreach ($n in 'igdumdim64.dll', 'igd10iumd64.dll', 'igd12umd64.dll', 'igc64.dll', 'nvwgf2umx.dll', 'nvldumdx.dll', 'nvd3dumx.dll', 'amdxc64.dll', 'atidxx64.dll', 'aticfx64.dll', 'amdihk64.dll') { [void]$v.Add($n) }
    return $v
}

function Modules-Externes([int]$Id) {
    return @((Get-Process -Id $Id -Module -ErrorAction Stop) | ForEach-Object { $_.ModuleName.ToLowerInvariant() } | Sort-Object -Unique)
}

function Charge([switch]$Temoin) {
    $nom = if ($Temoin) { 'charge-temoin (sans /DELAYLOAD, témoin)' } else { 'charge (/DELAYLOAD)' }
    $etat = Fichier-Temp "charge-$([int][bool]$Temoin).txt"
    $signal = "$etat.signal"
    $a = @('--etat', "`"$etat`"", '--signal', "`"$signal`"")
    $p = if ($Temoin) { Lancer-Exemple 'charge-temoin' $a -Binaire } else { Lancer-Exemple 'charge' $a }
    if ($null -eq $p) { NonMesurable $nom 'exécutable absent (étape de construction échouée ?)'; return }
    $demarre = Attendre -DelaiS 20 -PasMs 50 -Condition { (Lire-Etat $etat)['etape'] -eq 'avant' }
    if (-not $demarre) {
        $raison = if ($p.HasExited) { "sorti avant d'écrire, code $(Code-Sortie $p.ExitCode)" } else { 'rien écrit en 20 s' }
        Resultat "$nom : démarre" "NON ($raison)" 'démarre' 'échoue'
        return
    }
    Resultat "$nom : démarre" 'OUI' 'démarre' 'passe'
    $phases = [ordered]@{}
    foreach ($ph in 'avant', 'recherche', 'apres') {
        if ($ph -ne 'avant') {
            Ecrire-Commande $signal 'go'
            if (-not (Attendre -DelaiS 40 -PasMs 50 -Condition { (Lire-Etat $etat)['etape'] -eq $ph })) {
                NonMesurable "$nom : phase $ph" "rien écrit en 40 s$(if ($p.HasExited) { ", sorti code $(Code-Sortie $p.ExitCode)" })"
                break
            }
        }
        $e = Lire-Etat $etat
        $ext = try { Modules-Externes $p.Id } catch { @("illisible : $($_.Exception.Message)") }
        $phases[$ph] = [pscustomobject]@{ Etat = $e; Modules = $ext; Prive = (Nombre $e["prive_$ph"]) }
        if (-not $Temoin) {
            $f = Join-Path (Dossier-Resultats) "charge-modules-$ph.txt"
            Set-Content -LiteralPath $f -Value ($ext -join "`n") -Encoding utf8
        }
    }
    if ($phases.Contains('avant')) {
        $av = $phases['avant']
        $vus = @($av.Modules | Where-Object { $_ -in $script:SUIVIES })
        $attendu = if ($Temoin) { 'les DLL liées sans /DELAYLOAD chargées dès le lancement (sinon la mesure ne voit rien)' } else { 'aucune des cinq' }
        $ok = if ($Temoin) { $vus.Count -ge 1 } else { $vus.Count -eq 0 }
        Resultat "$nom : 8a, mfplat, mfreadwrite, mf, dxgi, d3d11 avant le premier appel (Get-Process -Module)" "$(if ($vus.Count) { $vus -join ', ' } else { 'aucune' }) ; vu de l'intérieur (EnumProcessModules) : $($av.Etat['suivies_avant']) ; $($av.Modules.Count) modules en tout" $attendu (Verdict $ok)
        if (-not $Temoin) { Note "Liste complète des modules avant tout appel : charge-modules-avant.txt (comparée à la référence 0.6.1 par l'étape des résultats, S2 point 12 b)." }
    }
    if ($phases.Contains('recherche')) {
        $re = $phases['recherche']; $e = $re.Etat
        $gardes = @('mfplat.dll', 'mfreadwrite.dll', 'mf.dll' | ForEach-Object { "$_ $($e["garde_$_"])" }) -join ' ; '
        Resultat "$nom : 8b, garde LoadLibraryExW(LOAD_LIBRARY_SEARCH_SYSTEM32)" $gardes 'les trois à 1 sur une machine avec Media Foundation' (Verdict ($gardes -notmatch ' 0'))
        $plus = @($re.Modules | Where-Object { $_ -notin $phases['avant'].Modules })
        $pilotes = Dll-Pilotes
        $dp = @($plus | Where-Object { $pilotes.Contains($_) })
        Resultat "$nom : 12c, après la garde et la recherche du codeur (MFStartup, MFTEnumEx sans activer, MFShutdown)" "codeurs H.264 $($e['recherche_codeurs_h264']) ; $($e['recherche_ms']) ms ; $($plus.Count) modules en plus : $($plus -join ', ') ; mémoire privée en plus $(Mo ($re.Prive - $phases['avant'].Prive))" 'à lire' 'à lire'
        Resultat "$nom : 12c, DLL du pilote graphique entrées dans le processus" "$(if ($dp.Count) { $dp -join ', ' } else { 'aucune' }) (pilotes connus : $(@($pilotes) -join ', '))" 'à lire' (OuiNon ($dp.Count -gt 0))
        if ("$($e['erreur'])") { Note "$nom : erreurs : $($e['erreur'])" }
    }
    if ($phases.Contains('apres')) {
        $ap = $phases['apres']; $e = $ap.Etat
        $vus = @($ap.Modules | Where-Object { $_ -in $script:SUIVIES })
        $manque = @($script:SUIVIES | Where-Object { $_ -notin $ap.Modules })
        Resultat "$nom : 8a, DLL après MFStartup, MFTEnumEx, MFCreateSinkWriterFromURL, CreateDXGIFactory1, D3D11CreateDevice" "$($vus -join ', ') ; absentes : $(if ($manque.Count) { $manque -join ', ' } else { 'aucune' }) ; écrivain $($e['sinkwriter']) ; dxgi $($e['dxgi']) ; d3d11 $($e['d3d11']) ; $($e['appels_ms']) ms$(if ("$($e['erreur'])") { " ; erreurs : $($e['erreur'])" })" 'mfplat, mfreadwrite, dxgi et d3d11 chargées (mf.dll : seulement si une de ses fonctions est importée)' (Verdict (@('mfplat.dll', 'mfreadwrite.dll', 'dxgi.dll', 'd3d11.dll' | Where-Object { $_ -notin $ap.Modules }).Count -eq 0))
    }
    Ecrire-Commande $signal 'quitter'
    if (-not $p.WaitForExit(10000)) { try { $p.Kill($true) } catch { } }
}

if (Demarrer-Sonde -Nom '01-s2-codeur-charge' -Titre 'S2 points 1, 2, 8 et 12 : codeur H.264, tailles, chargement à la demande, garde, modules') {
    try {
        Section 'S2 (1) : codeurs et décodeurs (MFTEnumEx)'
        $r = Liste-Codeurs 'avant toute installation'
        $nbCodeurs = [int]("$($r['codeur_h264_nombre'])" -as [int])
        Garde-PS 'avant toute installation'
        if ($r['ok'] -ne '1' -or $nbCodeurs -eq 0) {
            Section 'S2 (1) : Install-WindowsFeature Server-Media-Foundation'
            $ps = Get-Command powershell.exe -ErrorAction SilentlyContinue
            if (-not $ps) { NonMesurable 'installation' 'powershell.exe (5.1, module ServerManager) absent' }
            else {
                $sortie = Fichier-Temp 'installation.json'
                $chrono = [System.Diagnostics.Stopwatch]::StartNew()
                $cmd = "`$r = Install-WindowsFeature -Name Server-Media-Foundation -ErrorAction Stop; `$r | Select-Object Success, RestartNeeded, ExitCode, @{n='Features';e={(`$_.FeatureResult | ForEach-Object { `$_.Name }) -join ','}} | ConvertTo-Json | Set-Content -LiteralPath '$sortie' -Encoding utf8"
                $p = Start-Process -FilePath $ps.Source -ArgumentList '-NoProfile', '-Command', $cmd -NoNewWindow -PassThru -RedirectStandardError "$sortie.err"
                $null = $p.Handle
                $fini = $p.WaitForExit(600000)
                if (-not $fini) { try { $p.Kill($true) } catch { } }
                $s = [int]$chrono.Elapsed.TotalSeconds
                if (-not $fini) { Resultat 'Install-WindowsFeature' "NON fini en 600 s" 'sans redémarrage, dans le temps du workflow' 'échoue' }
                elseif (Test-Path -LiteralPath $sortie) {
                    $j = Get-Content -LiteralPath $sortie -Raw | ConvertFrom-Json
                    Resultat 'Install-WindowsFeature' "réussite $($j.Success) ; redémarrage demandé $($j.RestartNeeded) ; code $($j.ExitCode) ; $s s ; fonctionnalités $($j.Features)" 'réussit sans redémarrage' (Verdict ("$($j.Success)" -eq 'True' -and "$($j.RestartNeeded)" -ne 'Yes'))
                }
                else { Resultat 'Install-WindowsFeature' "échec (code $(Code-Sortie $p.ExitCode), $s s) : $("$(Get-Content -LiteralPath "$sortie.err" -Raw -ErrorAction SilentlyContinue)".Trim())" 'réussit sans redémarrage' 'échoue' }
                Section 'S2 (1) : codeurs après l''installation'
                $null = Liste-Codeurs 'après l''installation'
                Garde-PS 'après l''installation'
            }
        }
        else { Note 'Un codeur H.264 est là : Install-WindowsFeature n''est pas joué (la garde n''a donc été vue que sur une machine avec Media Foundation).' }

        Section 'S2 (2) : tailles acceptées par l''IMFSinkWriter (RGB32 → H.264, 3 images, Finalize)'
        $r = Jouer-Exemple 'mf' @('--etape', 'tailles', '--dossier', "`"$($script:Sonde.Temp)`"") 240
        if ($r['ok'] -ne '1') { NonMesurable 'tailles' "$($r['erreur'])" }
        foreach ($t in '600x450', '1024x768', '2560x1440', '3840x2160', '5120x2880', '601x451', '120x120', '64x64', '16x16') {
            $v = $r["taille_$t"]
            if (-not $v) { if ($r['ok'] -eq '1') { NonMesurable "taille $t" 'pas de ligne' }; continue }
            $crit = switch ($t) { '601x451' { 'à lire : côtés impairs (la zone de l''utilisateur peut l''être)' } '5120x2880' { 'à lire : au-delà du niveau 5.2 de H.264 (4096 × 2304)' } default { 'accepté' } }
            $ver = if ($crit -eq 'accepté') { Verdict ("$v" -like 'acceptée*') } else { OuiNon ("$v" -like 'acceptée*') }
            Resultat "taille $t" $v $crit $ver
        }

        Section 'S2 (8a, 8b, 12c) : chargement à la demande des DLL (Get-Process -Module à chaque phase)'
        Charge
        Charge -Temoin
        Note 'Le Windows « N » sans pack multimédia (les DLL absentes) ne se simule pas ici : la preuve est l''absence des DLL avant le premier appel (8a) et la DLL inventée (8c). L''exemple et le témoin ont le même code (sonde_film::charge) ; seul /DELAYLOAD (build.rs, sur les exemples) les sépare.'

        Section 'S2 (8c) : la bibliothèque absente (bzs-absente.dll, importée par raw-dylib, chargée à la demande)'
        $r = Jouer-Exemple 'charge' @('--absente', 'sans-garde') 30
        $mort = "$($r['_code'])" -notlike '0 *'
        Resultat 'Appel sans garde' "code de sortie $($r['_code']) ; lignes : $($r['absente_appel'])$(if ($r.ContainsKey('absente_rendu')) { " ; rendu $($r['absente_rendu'])" })" 'le processus s''arrête (exception du chargeur à la demande 0xC06D007E attendue)' (Verdict $mort)
        $r = Jouer-Exemple 'charge' @('--absente', 'garde') 30
        Resultat 'Avec la garde (LoadLibraryExW échoue, aucun appel)' "code de sortie $($r['_code']) ; garde : $($r['absente_garde']) ; appel : $($r['absente_appel'])" 'fin propre (code 0), aucun appel' (Verdict ("$($r['_code'])" -like '0 *' -and $r['ok'] -eq '1' -and "$($r['absente_appel'])" -eq 'non fait'))
    }
    catch { Erreur-Sonde $_ }
}
Terminer-Sonde
