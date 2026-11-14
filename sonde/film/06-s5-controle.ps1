# S5 (contrat 0.7, §1.5) : la fenêtre de contrôle du film pendant plusieurs minutes.
# La fenêtre (exemple controle --genre film : WebView2 transparente sur tout l'écran,
# toujours au-dessus, sans focus, exclue des captures, traversée par les clics sauf sur la
# pilule, point qui « respire » et durée qui change chaque seconde) est posée sur une fenêtre
# cible (temoin.ps1 -Genre cible) qui compte les clics, les crans de molette et les touches
# reçus.
# - Part de processeur des processus WebView2 de la page (et de l'hôte, à part), en % d'un
#   cœur : avec l'animation pendant 5 min, sans elle (--animation 0) pendant 2 min, et avec
#   « Réduire les animations » (SPI_SETCLIENTAREAANIMATION à 0, que Chromium lit pour
#   prefers-reduced-motion) pendant 2 min ; un relevé toutes les 30 s.
# - Mémoire privée des mêmes processus toutes les 30 s.
# - Clic, molette et touche loin de la pilule : arrivent-ils à la cible ; clic sur la pilule :
#   arrive-t-il à la page, et la cible garde-t-elle le premier plan.
# - La cadence de S1 avec et sans la fenêtre : 02-s1-images.ps1, (d).
#   pwsh -NoProfile -File sonde/film/06-s5-controle.ps1

. "$PSScriptRoot/commun.ps1"

function Ouvrir-Cible {
    $f = Fichier-Temp "cible-$([guid]::NewGuid().ToString('N').Substring(0, 6)).txt"
    $p = Lancer-Pwsh 'temoin.ps1' @('-Genre', 'cible', '-Fichier', "`"$f`"")
    if (-not (Attendre -DelaiS 40 -Condition { (Lire-Etat $f).ContainsKey('clics') })) { throw "fenêtre cible : pas prête en 40 s$(if ($p.HasExited) { " (sortie, code $(Code-Sortie $p.ExitCode))" })" }
    $e = Lire-Etat $f
    return [pscustomobject]@{ P = $p; Hwnd = [IntPtr]::new([long]$e.hwnd); Fichier = $f }
}

# Suit les processus d'une fenêtre de contrôle pendant -DureeS : processeur (delta de
# TotalProcessorTime, par processus connu au relevé précédent) et mémoire privée, un relevé
# toutes les -PasS. Rend { Page (% d'un cœur, processus WebView2), Hote, Tranches (%),
# Memoires (octets, page + hôte), Processus (nombre au dernier relevé), Duree }.
function Suivre-Controle($O, [double]$DureeS, [double]$PasS = 30) {
    Start-Sleep -Seconds 5
    $hote = $O.P.Id
    $connus = @{}
    $releve = {
        $v = @{}
        foreach ($i in (Pids-Controle $O)) {
            try { $q = [System.Diagnostics.Process]::GetProcessById($i); $v[$i] = [pscustomobject]@{ Cpu = $q.TotalProcessorTime.TotalSeconds; Prive = $q.PrivateMemorySize64 } } catch { }
        }
        return $v
    }
    $connus = & $releve
    $chrono = [System.Diagnostics.Stopwatch]::StartNew()
    $page = 0.0; $hoteCpu = 0.0
    $tranches = [System.Collections.Generic.List[double]]::new()
    $memoires = [System.Collections.Generic.List[double]]::new()
    $avant = 0.0
    $memoires.Add([double](($connus.Values | Measure-Object -Property Prive -Sum).Sum))
    while ($chrono.Elapsed.TotalSeconds -lt $DureeS) {
        $reste = $DureeS - $chrono.Elapsed.TotalSeconds
        Start-Sleep -Milliseconds ([int]([math]::Min($PasS, [math]::Max(0.5, $reste)) * 1000))
        $t = $chrono.Elapsed.TotalSeconds
        $v = & $releve
        $tranche = 0.0
        foreach ($i in $v.Keys) {
            if (-not $connus.ContainsKey($i)) { continue }
            $d = [math]::Max(0, $v[$i].Cpu - $connus[$i].Cpu)
            if ($i -eq $hote) { $hoteCpu += $d } else { $page += $d; $tranche += $d }
        }
        $tranches.Add(100 * $tranche / [math]::Max(0.001, $t - $avant))
        $memoires.Add([double](($v.Values | Measure-Object -Property Prive -Sum).Sum))
        $connus = $v
        $avant = $t
    }
    $duree = $chrono.Elapsed.TotalSeconds
    return [pscustomobject]@{
        Page = 100 * $page / $duree; Hote = 100 * $hoteCpu / $duree; Tranches = $tranches; Memoires = $memoires
        Processus = $connus.Count; Duree = $duree
    }
}

function Montrer-Suivi([string]$Nom, $S, [switch]$Critere) {
    $tr = ($S.Tranches | ForEach-Object { '{0:0.0}' -f $_ }) -join ' / '
    Resultat "$Nom : processeur de la page (processus WebView2)" ('{0:0.00} % d''un cœur en moyenne sur {1:0} s (par 30 s : {2})' -f $S.Page, $S.Duree, $tr) $(if ($Critere) { 'au plus 3 % d''un cœur en moyenne' } else { 'à lire (comparer à l''animation)' }) $(if ($Critere) { Verdict ($S.Page -le 3) } else { 'à lire' })
    Resultat "$Nom : processeur de l'hôte (exemple controle : tao, wry, surveillance de la pilule toutes les 30 ms)" ('{0:0.00} % d''un cœur' -f $S.Hote) 'à lire'
    Resultat "$Nom : processus suivis" "$($S.Processus) (hôte compris)" 'à lire'
    $m = $S.Memoires
    if ($m.Count -ge 3) {
        $premier = $m[1]; $dernier = $m[$m.Count - 1]
        $max = ($m | Measure-Object -Maximum).Maximum
        $suite = ($m | ForEach-Object { Mo $_ }) -join ' / '
        Resultat "$Nom : mémoire privée (page + hôte), toutes les 30 s" "$suite ; pic $(Mo $max) ; dérive $(Mo ($dernier - $premier)) entre le premier et le dernier relevé après 30 s" 'stable : dérive au plus 5 Mo (seuil de la sonde, supposition)' (OuiNon ([math]::Abs($dernier - $premier) -le 5MB))
    }
    else { NonMesurable "$Nom : mémoire privée" 'moins de trois relevés' }
}

if (Demarrer-Sonde -Nom '06-s5-controle' -Titre 'S5 : la fenêtre de contrôle du film pendant plusieurs minutes') {
    $cible = $null; $c = $null
    try {
        $e = Ecran
        $zone = [int[]]@(($e[0] + 100), ($e[1] + 100), 600, 450)
        $script:Anim0 = $script:Natif::AnimationsClient()

        Resultat 'SPI_GETCLIENTAREAANIMATION au départ' "$script:Anim0" '1 (animations permises)' 'à lire'
        $script:Sonde.Restaurer.Add({ if ($script:Anim0 -ge 0) { [void]$script:Natif::FixerAnimationsClient([bool]$script:Anim0) } })

        # Le processus de la cible est fermé par Terminer-Sonde (Lancer-Pwsh le garde).
        $cible = Ouvrir-Cible

        Section 'S5 : contrôle de la mesure du processeur (page qui calcule exprès 10 ms toutes les 20 ms, 30 s)'
        $c = Ouvrir-Controle -Zone $zone -Calcul
        $s = Suivre-Controle $c 30 10
        $ok = $s.Page -ge 10
        Resultat 'Processeur des processus WebView2 d''une page qui calcule' ('{0:0.0} % d''un cœur (hôte {1:0.0} %, {2} processus suivis)' -f $s.Page, $s.Hote, $s.Processus) 'au moins 10 % (attendu vers 50 %) : sinon la mesure ne voit pas la page' (OuiNon $ok)
        $script:MesureCpuSure = $ok
        Fermer-Controle $c; $c = $null

        Section 'S5 : avec l''animation (point qui respire), 5 min'
        # La machine démarre avec les animations coupées : elles sont permises pour cette passe.
        $v = $script:Natif::FixerAnimationsClient($true)
        Resultat 'SPI_SETCLIENTAREAANIMATION 1, relu' "$v" '1' (OuiNon ($v -eq 1))
        $c = Ouvrir-Controle -Zone $zone
        $ec = Lire-Etat $c.Etat
        $anime = "$($ec['reduit'])" -eq '0'
        Resultat 'Fenêtre prête' "DOM en $($ec['dom_ms']) ms ; affinité $($ec['affinite']) (17 : WDA_EXCLUDEFROMCAPTURE) ; style étendu $($ec['exstyle']) ; traversée $($ec['traverse']) ; prefers-reduced-motion $($ec['reduit'])" 'affinité 17, traversée 1, reduced-motion 0' (OuiNon ("$($ec['affinite'])" -eq '17' -and "$($ec['traverse'])" -eq '1' -and $anime))
        $s = Suivre-Controle $c 300
        if ($anime -and $script:MesureCpuSure) { Montrer-Suivi 'avec l''animation' $s -Critere }
        else {
            Montrer-Suivi 'avec l''animation (chiffres seulement)' $s
            NonMesurable 'critère des 3 % avec l''animation' "$(if (-not $anime) { "la page voit prefers-reduced-motion à $($ec['reduit']) : le point ne respire pas" } else { 'la mesure du processeur n''a pas vu la page qui calcule' })" 'au plus 3 % d''un cœur en moyenne'
        }
        $ec = Lire-Etat $c.Etat
        Resultat 'Durée affichée par la page (secondes comptées par la page)' "$($ec['secondes_page']) s pour $([math]::Round((Nombre $ec['depuis_ms']) / 1000)) s de vie" 'à 2 s près' (OuiNon ([math]::Abs((Nombre $ec['secondes_page']) - (Nombre $ec['depuis_ms']) / 1000) -le 3))
        if ($s.Page -gt 3) { Note 'Au-delà de 3 % : le point reste rouge sans respirer (contrat §1.5), ce qui change ce que l''utilisateur voit ; à lui dire.' }

        Section 'S5 : entrées vers la fenêtre du dessous (fenêtre de contrôle avec l''animation montrée)'
        $pp = $script:Natif::PremierPlan($cible.Hwnd)
        Resultat 'Cible au premier plan (SetForegroundWindow)' (OuiNon $pp) 'OUI' (OuiNon $pp)
        $x = $e[0] + 400; $y = $e[1] + 300
        $a0 = Lire-Etat $cible.Fichier; $k0 = Lire-Etat $c.Etat
        $n1 = $script:Natif::Clic($x, $y); Start-Sleep -Milliseconds 400
        $n2 = 0; foreach ($i in 1..3) { $n2 += $script:Natif::Molette(-1); Start-Sleep -Milliseconds 120 }
        Start-Sleep -Milliseconds 300
        $n3 = $script:Natif::Touche(0x41); Start-Sleep -Milliseconds 500
        $a1 = Lire-Etat $cible.Fichier; $k1 = Lire-Etat $c.Etat
        $d = { param($A, $B, $Cle) (Nombre $B[$Cle]) - (Nombre $A[$Cle]) }
        Resultat "Clic gauche en ($x, $y), loin de la pilule (SendInput : $n1 sur 2)" "cible +$(& $d $a0 $a1 'clics') ; page +$(& $d $k0 $k1 'clics')" 'cible +1, page +0' (OuiNon ((& $d $a0 $a1 'clics') -eq 1 -and (& $d $k0 $k1 'clics') -eq 0))
        Resultat "Trois crans de molette au même endroit (SendInput : $n2 sur 3)" "cible +$(& $d $a0 $a1 'molettes') ; page +$(& $d $k0 $k1 'molettes')" 'cible +3, page +0' (OuiNon ((& $d $a0 $a1 'molettes') -eq 3 -and (& $d $k0 $k1 'molettes') -eq 0))
        Resultat "Touche A (SendInput : $n3 sur 2)" "cible +$(& $d $a0 $a1 'touches') ; page +$(& $d $k0 $k1 'touches')" 'cible +1, page +0' (OuiNon ((& $d $a0 $a1 'touches') -eq 1 -and (& $d $k0 $k1 'touches') -eq 0))
        Resultat 'Cible encore au premier plan après les trois' "$($a1['premier_plan'])" '1' (OuiNon ("$($a1['premier_plan'])" -eq '1'))
        # Sur la pilule (position par défaut de l'exemple : centrée, 120 px au-dessus du bas).
        $px = $e[0] + [int]($e[2] / 2); $py = $e[1] + $e[3] - 120 + 24
        $script:Natif::Poser($px, $py)
        $ok = Attendre -DelaiS 2 -PasMs 50 -Condition { "$((Lire-Etat $c.Etat)['traverse'])" -eq '0' }
        Resultat "Pointeur sur la pilule ($px, $py) : la fenêtre cesse d'être traversée" "$(OuiNon $ok) (traverse $((Lire-Etat $c.Etat)['traverse']))" 'OUI en moins de 2 s' (OuiNon $ok)
        $b0 = Lire-Etat $cible.Fichier; $m0 = Lire-Etat $c.Etat
        $n4 = $script:Natif::Clic($px, $py); Start-Sleep -Milliseconds 500
        $b1 = Lire-Etat $cible.Fichier; $m1 = Lire-Etat $c.Etat
        Resultat "Clic sur la pilule (SendInput : $n4 sur 2)" "page +$(& $d $m0 $m1 'clics') ; cible +$(& $d $b0 $b1 'clics')" 'page +1, cible +0' (OuiNon ((& $d $m0 $m1 'clics') -eq 1 -and (& $d $b0 $b1 'clics') -eq 0))
        Resultat 'Premier plan après le clic sur la pilule' "cible au premier plan : $($b1['premier_plan']) ; fenêtre au premier plan : $($script:Natif::Decrire($script:Natif::GetForegroundWindow()))" 'à lire (la pilule ne devrait pas prendre le focus)' 'à lire'
        $script:Natif::Poser(($e[0] + $e[2] - 8), ($e[1] + 8))
        $ok = Attendre -DelaiS 2 -PasMs 50 -Condition { "$((Lire-Etat $c.Etat)['traverse'])" -eq '1' }
        Resultat 'Pointeur reparti : la fenêtre est de nouveau traversée' (OuiNon $ok) 'OUI' (OuiNon $ok)
        Fermer-Controle $c; $c = $null

        Section 'S5 : sans l''animation (--animation 0 : le point reste rouge), 2 min'
        $c = Ouvrir-Controle -Zone $zone -SansAnimation
        $s = Suivre-Controle $c 120
        Montrer-Suivi 'sans l''animation' $s
        Fermer-Controle $c; $c = $null

        Section 'S5 : avec « Réduire les animations » (SPI_SETCLIENTAREAANIMATION 0), 2 min'
        $v = $script:Natif::FixerAnimationsClient($false)
        Resultat 'SPI_SETCLIENTAREAANIMATION 0, relu' "$v" '0' (OuiNon ($v -eq 0))
        $c = Ouvrir-Controle -Zone $zone
        $ec = Lire-Etat $c.Etat
        Resultat 'La page voit prefers-reduced-motion: reduce' "reduit $($ec['reduit'])" '1' (OuiNon ("$($ec['reduit'])" -eq '1'))
        $s = Suivre-Controle $c 120
        Montrer-Suivi 'avec « Réduire les animations »' $s
        Fermer-Controle $c; $c = $null
        if ($script:Anim0 -ge 0) { [void]$script:Natif::FixerAnimationsClient([bool]$script:Anim0) }
        Note 'La page « avec l''animation » est mesurée 5 min comme le contrat le dit ; les deux autres 2 min chacune, pour tenir le job sous 40 min : la comparaison se lit sur la moyenne et les tranches de 30 s.'
        Note 'La cadence de S1 avec et sans la fenêtre montrée : 02-s1-images.ps1, section (d).'
    }
    catch { Erreur-Sonde $_ }
    finally { Fermer-Controle $c }
}
Terminer-Sonde
