# S6 (contrat 0.6, §1.6) : reconnaître la molette de l'utilisateur. Un autre processus
# (ecoute.ps1 + ecoute.cs) pose un crochet WH_MOUSE_LL dans un fil à boucle de messages, et
# l'entrée brute (RIDEV_INPUTSINK) sur le même fil. Ce script-ci envoie les crans
# (SendInput), une moitié avec la marque de la sonde dans dwExtraInfo, l'autre sans.
# Mesure : 200 crans sur 200 reçus par chaque voie ; marque lue ; drapeau LLMHF_INJECTED ;
# latence (QueryPerformanceCounter, commun aux processus) ; crochet toujours là après 60 s
# de défilement (un cran toutes les 50 ms), et 10 crans après.
#   pwsh -NoProfile -File sonde/defilante/03-s6-molette.ps1

. "$PSScriptRoot/commun.ps1"

function Lire-Detail([string]$Fichier) {
    $c = [System.Collections.Generic.List[object]]::new(); $r = [System.Collections.Generic.List[object]]::new()
    foreach ($l in @(Get-Content -LiteralPath $Fichier -ErrorAction SilentlyContinue)) {
        $m = $l -split ' '
        if ($m[0] -eq 'c' -and $m.Count -ge 5) { $c.Add([pscustomobject]@{ T = [long]$m[1]; D = [int]$m[2]; F = [uint32]$m[3]; X = [long]$m[4] }) }
        elseif ($m[0] -eq 'r' -and $m.Count -ge 4) { $r.Add([pscustomobject]@{ T = [long]$m[1]; D = [int]$m[2]; X = [long]$m[3] }) }
    }
    return [pscustomobject]@{ Crochet = $c; Brut = $r }
}

function Demander-Detail($O, [string]$Nom) {
    $f = Fichier-Temp "detail-$Nom.txt"
    Ecrire-Commande $O.Cmd "detail $f"
    if (-not (Attendre -DelaiS 10 -PasMs 100 -Condition { Test-Path -LiteralPath $f })) { throw "écoute : détail $Nom non écrit en 10 s" }
    return (Lire-Detail $f)
}

# Les crans reçus entre deux horodatages, rapprochés dans l'ordre des envois.
function Bilan-Voie([object[]]$Recus, [long[]]$Envois, [long[]]$Marques, [long]$Debut, [long]$Fin, [double]$Freq, [string]$Voie, [bool]$AvecDrapeaux) {
    $dans = @($Recus | Where-Object { $_.T -ge $Debut -and $_.T -le $Fin })
    $n = $Envois.Count
    $marque = 0; $sansMarque = 0; $injecte = 0; $lat = [System.Collections.Generic.List[double]]::new()
    for ($i = 0; $i -lt [math]::Min($dans.Count, $n); $i++) {
        if ($Marques[$i] -ne 0 -and $dans[$i].X -eq $Marques[$i]) { $marque++ }
        if ($Marques[$i] -eq 0 -and $dans[$i].X -eq 0) { $sansMarque++ }
        if ($AvecDrapeaux -and ($dans[$i].F -band 1)) { $injecte++ }
        $lat.Add(($dans[$i].T - $Envois[$i]) * 1000.0 / $Freq)
    }
    $avec = @($Marques | Where-Object { $_ -ne 0 }).Count
    return [pscustomobject]@{ Recus = $dans.Count; Envoyes = $n; Marque = $marque; Avec = $avec; SansMarque = $sansMarque; Sans = $n - $avec
        Injecte = $injecte; Lat = $lat.ToArray(); Voie = $Voie }
}

if (Demarrer-Sonde -Nom 's6' -Titre 'S6 : reconnaître la molette de l''utilisateur' -QuotaMo 0) {
    $o = $null
    try {
        try {
            $d = Get-ItemProperty 'HKCU:\Control Panel\Desktop' -ErrorAction Stop
            Note "LowLevelHooksTimeout (HKCU) : $(if ($d.PSObject.Properties['LowLevelHooksTimeout']) { "$($d.LowLevelHooksTimeout) ms" } else { 'absent (défaut du système)' })"
        }
        catch { }
        $c = Ouvrir-Cible 'wpf'
        $pt = Centre $c.Zone
        $etat = Fichier-Temp 'ecoute.txt'
        $cmd = Fichier-Temp 'ecoute.cmd'
        $p = Lancer-Pwsh 'ecoute.ps1' @('-Etat', "`"$etat`"", '-Commandes', "`"$cmd`"")
        $o = [pscustomobject]@{ P = $p; Etat = $etat; Cmd = $cmd }
        if (-not (Attendre -DelaiS 40 -Condition { (Lire-Etat $etat)['pret'] -eq '1' })) { throw 'écoute pas prête en 40 s' }
        $e = Lire-Etat $etat
        $freq = [double]$e.frequence
        Section 'S6 : les deux voies'
        Resultat 'crochet WH_MOUSE_LL posé (autre processus, fil à boucle de messages)' $(if ($e.crochet -eq '1') { 'oui' } else { "non : $($e.crochet_erreur)" }) 'oui' (Verdict ($e.crochet -eq '1'))
        Resultat 'entrée brute (RIDEV_INPUTSINK)' $(if (-not $e.brut_erreur) { 'enregistrée' } else { "non : $($e.brut_erreur)" }) 'à lire' ''
        if ([math]::Abs($freq - [System.Diagnostics.Stopwatch]::Frequency) -gt 1) { Note "fréquences QPC différentes ($freq contre $([System.Diagnostics.Stopwatch]::Frequency)) : latences fausses" }

        # 200 crans, un sur deux marqué, 20 ms d'écart ; sens alterné par paquets de 20.
        $script:N::Poser($pt[0], $pt[1])
        Start-Sleep -Milliseconds 300
        $envois = [long[]]::new(200); $marques = [long[]]::new(200)
        $debut = [System.Diagnostics.Stopwatch]::GetTimestamp()
        for ($i = 0; $i -lt 200; $i++) {
            $sens = if ([int][math]::Floor($i / 20) % 2 -eq 0) { -1 } else { 1 }
            $marques[$i] = if ($i % 2 -eq 0) { [long]$script:N::MARQUE } else { 0 }
            $envois[$i] = [System.Diagnostics.Stopwatch]::GetTimestamp()
            [void]$script:N::Molette($sens, $marques[$i])
            Start-Sleep -Milliseconds 20
        }
        $fin = [System.Diagnostics.Stopwatch]::GetTimestamp()
        Start-Sleep -Milliseconds 600
        $det = Demander-Detail $o 'serie'
        $fin2 = $fin + [long]($freq * 0.5)
        foreach ($voie in @(@{ N = 'crochet WH_MOUSE_LL'; L = $det.Crochet; F = $true }, @{ N = 'entrée brute'; L = $det.Brut; F = $false })) {
            $b = Bilan-Voie @($voie.L) $envois $marques $debut $fin2 $freq $voie.N $voie.F
            Section "S6 : 200 crans envoyés par un autre processus, $($b.Voie)"
            Resultat 'crans reçus' "$($b.Recus)/$($b.Envoyes)" '200/200' (Verdict ($b.Recus -eq 200))
            Resultat 'marque lue (dwExtraInfo / ulExtraInformation)' "$($b.Marque)/$($b.Avec) crans marqués la portent ; $($b.SansMarque)/$($b.Sans) crans sans marque lus à 0" 'toutes' (Verdict ($b.Marque -eq $b.Avec -and $b.SansMarque -eq $b.Sans -and $b.Recus -eq 200))
            if ($voie.F) { Resultat 'drapeau LLMHF_INJECTED' "$($b.Injecte)/$($b.Recus)" 'à lire (tous injectés : il ne distingue pas l''app de l''essai)' 'à lire' }
            if ($b.Lat.Count -gt 0) {
                Resultat 'latence envoi → réception' "médiane $(Ms (Mediane $b.Lat)), 95e centile $(Ms (Centile $b.Lat 0.95)), max $(Ms (($b.Lat | Measure-Object -Maximum).Maximum))" 'à lire' 'à lire'
            }
        }

        # 60 s de défilement, puis 10 crans.
        Section 'S6 : 60 s de défilement (un cran toutes les 50 ms), crochet toujours là ?'
        $debut = [System.Diagnostics.Stopwatch]::GetTimestamp()
        $liste = [System.Collections.Generic.List[long]]::new(); $mq = [System.Collections.Generic.List[long]]::new()
        $chrono = [System.Diagnostics.Stopwatch]::StartNew()
        $i = 0
        while ($chrono.Elapsed.TotalSeconds -lt 60) {
            $sens = if ([int][math]::Floor($i / 30) % 2 -eq 0) { -1 } else { 1 }
            $m = if ($i % 2 -eq 0) { [long]$script:N::MARQUE } else { 0 }
            $liste.Add([System.Diagnostics.Stopwatch]::GetTimestamp()); $mq.Add($m)
            [void]$script:N::Molette($sens, $m)
            Start-Sleep -Milliseconds 50
            $i++
        }
        $milieu = [System.Diagnostics.Stopwatch]::GetTimestamp()
        Start-Sleep -Milliseconds 500
        $apres = [System.Collections.Generic.List[long]]::new()
        for ($k = 0; $k -lt 10; $k++) { $apres.Add([System.Diagnostics.Stopwatch]::GetTimestamp()); [void]$script:N::Molette(-1, $script:N::MARQUE); Start-Sleep -Milliseconds 50 }
        $fin = [System.Diagnostics.Stopwatch]::GetTimestamp()
        Start-Sleep -Milliseconds 600
        $det = Demander-Detail $o 'longue'
        foreach ($voie in @(@{ N = 'crochet WH_MOUSE_LL'; L = $det.Crochet }, @{ N = 'entrée brute'; L = $det.Brut })) {
            $l = @($voie.L)
            $pendant = @($l | Where-Object { $_.T -ge $debut -and $_.T -lt $milieu + [long]($freq * 0.4) }).Count
            $derniers = @($l | Where-Object { $_.T -ge $milieu + [long]($freq * 0.4) -and $_.T -le $fin + [long]($freq * 0.5) }).Count
            # Tranches de 5 s : une tranche vide après des tranches pleines = crochet retiré.
            $tranches = @(for ($t = 0; $t -lt 12; $t++) {
                    $a = $debut + [long]($freq * 5 * $t); $z = $a + [long]($freq * 5)
                    @($l | Where-Object { $_.T -ge $a -and $_.T -lt $z }).Count
                })
            Resultat "$($voie.N) : crans reçus pendant 60 s" "$pendant/$($liste.Count) ; par tranche de 5 s : $($tranches -join ', ')" "$($liste.Count)/$($liste.Count)" (Verdict ($pendant -eq $liste.Count))
            Resultat "$($voie.N) : 10 crans après les 60 s" "$derniers/10" '10/10 (crochet pas retiré)' (Verdict ($derniers -eq 10))
        }
        $e = Lire-Etat $etat
        Note "Fin : crochet $($e.crochet), crans notés $($e.crans_crochet) (crochet) et $($e.crans_brut) (brut)."

        Section 'S6 : ce que la machine ne peut pas montrer'
        NonMesurable 'molette physique de l''utilisateur : dwExtraInfo' 'aucune souris physique sur la machine de GitHub ; à vérifier chez l''utilisateur (supposition : 0, ou une valeur du pilote, jamais la marque)'
        Note 'Critère : le crochet si 200 sur 200 et marque lue ; sinon l''entrée brute (alors les crans d''un autre processus ne sont pas vus : l''essai 14 ne pourrait plus prouver la reprise, à dire au responsable).'
        Fermer-Cible $c
    }
    catch { Erreur-Sonde $_ }
    finally {
        if ($null -ne $o) {
            try { Ecrire-Commande $o.Cmd 'quitter' } catch { }
            if (-not $o.P.WaitForExit(5000)) { try { $o.P.Kill($true) } catch { } }
        }
    }
}
Terminer-Sonde
