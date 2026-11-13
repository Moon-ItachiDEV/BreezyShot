# S8 (contrat 0.6, §1.9) : échelles 125 et 150 %, plusieurs écrans.
# 1. Vraie échelle : essai de changer l'échelle de l'écran sans fermer la session
#    (DisplayConfigSetDeviceInfo, paquet non documenté de l'app Paramètres), remise à la fin
#    quoi qu'il arrive ; si elle change, une suite d'images du témoin WPF et d'Edge.
# 2. Imitations : le témoin WPF sous une LayoutTransform de 1,25 et 1,5 (texte et bords à des
#    positions fractionnaires) ; Edge avec --force-device-scale-factor est dans S3.
# 3. Plusieurs écrans : nombre d'écrans de la machine ; non faisable sinon.
# L'échelle d'origine est aussi écrite dans $env:RUNNER_TEMP (ou le dossier temporaire),
# fichier breezyshot-sonde-echelle.txt : l'étape « Remettre l'échelle » du workflow
# (09-remettre-echelle.ps1, if: always()) la remet même si ce script est arrêté.
# Premier passage : `$n = …AllScreens.Count` écrasait `$script:N` (PowerShell ne distingue
# pas la casse) ; le `finally` levait et rien n'était écrit. La classe native est tenue ici
# dans $NATIF, et aucune variable du script ne s'appelle n.
#   pwsh -NoProfile -File sonde/defilante/08-s8-echelles.ps1

. "$PSScriptRoot/commun.ps1"

$PALIERS = 100, 125, 150, 175, 200, 225, 250, 300, 350, 400, 450, 500

if (Demarrer-Sonde -Nom 's8' -Titre 'S8 : échelles 125 et 150 %, plusieurs écrans' -QuotaMo 5) {
    $origine = $null
    $NATIF = $script:N
    $FICHIER_ORIGINE = Join-Path $(if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [System.IO.Path]::GetTempPath() }) 'breezyshot-sonde-echelle.txt'
    try {
        $wa = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
        Section 'S8 : vraie échelle de l''écran'
        $e = $script:N::EchelleRelative()
        if ($e[0] -ne 0) { NonMesurable 'lecture de l''échelle (DisplayConfigGetDeviceInfo)' "code $($e[0])" }
        else {
            $recommande = [math]::Abs($e[1])
            $actuel = $recommande + $e[2]
            $origine = $e[2]
            Set-Content -LiteralPath $FICHIER_ORIGINE -Value "$origine" -Encoding ascii
            $pmax = $PALIERS[[math]::Min($PALIERS.Count - 1, $recommande + $e[3])]
            Resultat 'échelle lue' "actuelle $($PALIERS[[math]::Min($PALIERS.Count - 1, $actuel)]) % ; recommandée $($PALIERS[$recommande]) % ; permise jusqu'à $pmax % (pas relatifs $($e[1]) à $($e[3]))" 'à lire' 'à lire'
            Resultat 'DPI effectif avant' ($script:N::DpiEcran($wa.X + 10, $wa.Y + 10)) '96 attendu (100 %)' ''
            $o = $origine
            $script:Sonde.Restaurer.Add({ [void]$NATIF::FixerEchelleRelative($o) }.GetNewClosure())
            foreach ($cible in 125, 150) {
                $k = [array]::IndexOf($PALIERS, $cible)
                $rel = $k - $recommande
                if ($rel -gt $e[3] -or $rel -lt $e[1]) { NonMesurable "passer à $cible %" "hors des paliers permis par Windows pour cet écran (max $pmax %)"; continue }
                $code = $script:N::FixerEchelleRelative($rel)
                $vu = 0
                $ok = Attendre -DelaiS 8 -PasMs 250 -Condition { $script:N::DpiEcran($wa.X + 10, $wa.Y + 10) -eq [int](96 * $cible / 100) }
                $vu = $script:N::DpiEcran($wa.X + 10, $wa.Y + 10)
                Resultat "passer à $cible % sans fermer la session" "code $code ; DPI effectif lu $vu" "$([int](96 * $cible / 100))" (Verdict ($code -eq 0 -and $ok))
                if ($code -eq 0 -and $ok) {
                    Start-Sleep -Seconds 2
                    foreach ($g in 'wpf', 'edge') {
                        $c = $null
                        try {
                            $c = Ouvrir-Cible $g
                            if ($g -eq 'edge') { $c.Echelle = $cible / 100; $col = $script:N::ColonneBande((Prendre $c.Zone), $c.Zone[2], $c.Zone[3], 24, 0, [math]::Min(200, $c.Zone[3])); if ($col -ge 0) { $c.Bande = $true; $c.Colonne = $col + 2 } }
                            if ($c.Info) { Note $c.Info }
                            $c.Uia = Uia-Defilable $c
                            Note "$g à $cible % : zone $(Rect-Texte $c.Zone), échelle lue par le témoin $($c.Echelle), pas par $(if ($null -ne $c.Uia) { "UI Automation ($($c.Uia.Par), $(Ms $c.Uia.Ms))" } else { 'la molette (pas de ScrollPattern)' })"
                            $pas = if ($null -ne $c.Uia) { { Pas-UIA $c } } else { { Pas-Molette $c 3 } }
                            $images = Suite "$g-$cible-vraie" $c 10 $pas
                            Mesure-Suite "$g-$cible-vraie" $c $images $(if ($g -eq 'edge') { [int](56 * $cible / 100) } else { 0 })
                        }
                        catch { NonMesurable "$g à $cible %" $_.Exception.Message }
                        finally { Fermer-Cible $c }
                    }
                }
                [void]$script:N::FixerEchelleRelative($origine)
                $remis = Attendre -DelaiS 8 -PasMs 250 -Condition { $script:N::DpiEcran($wa.X + 10, $wa.Y + 10) -eq 96 }
                if (-not $remis) { Note "échelle pas revenue à 96 ppp en 8 s après $cible % (lue $($script:N::DpiEcran($wa.X + 10, $wa.Y + 10)))" }
            }
        }
        NonMesurable 'DpiValue de HKCU\Control Panel\Desktop\PerMonitorSettings' 'demande une fermeture de session, impossible sans personne sur la machine : non essayé'

        Section 'S8 : imitation, témoin WPF sous une LayoutTransform'
        foreach ($f in 1.25, 1.5) {
            $c = $null
            try {
                $c = Ouvrir-Cible 'wpf' -Echelle $f
                $c.Uia = Uia-Defilable $c
                if ($null -eq $c.Uia) { NonMesurable "wpf × $f" 'pas de ScrollPattern'; continue }
                $images = Suite "wpf-transformee-$f" $c 12 { Pas-UIA $c }
                Mesure-Suite "wpf-transformee-$f" $c $images 0
            }
            catch { NonMesurable "wpf × $f" $_.Exception.Message }
            finally { Fermer-Cible $c }
        }
        Note 'Edge à 125 et 150 % (facteur forcé) : voir S3 (edge-125, edge-150).'

        Section 'S8 : plusieurs écrans'
        $nEcrans = [System.Windows.Forms.Screen]::AllScreens.Count
        if ($nEcrans -lt 2) { NonMesurable 'plusieurs écrans' "la machine n'a que $nEcrans écran : tests purs seulement (§12.1) et liste de l'utilisateur (maison : un écran 2K ; travail : trois écrans 1080p)" }
        else { Resultat 'écrans' $nEcrans 'à lire' 'à lire' }
    }
    catch { Erreur-Sonde $_ }
    finally {
        if ($null -ne $origine) {
            try {
                $code = $NATIF::FixerEchelleRelative($origine)
                $ok = Attendre -DelaiS 8 -PasMs 250 -Condition { $NATIF::DpiEcran($wa.X + 10, $wa.Y + 10) -eq 96 }
                Section 'S8 : remise de l''échelle'
                Resultat 'échelle remise à l''origine' "code $code ; DPI effectif lu $($NATIF::DpiEcran($wa.X + 10, $wa.Y + 10))" '96 (100 %)' (Verdict ($code -eq 0 -and $ok))
            }
            catch { Write-Host "remise de l'échelle : $($_.Exception.Message)" }
        }
    }
}
Terminer-Sonde
