# S1 points 3, 5 et 7 (contrat 0.7, §1.1) : Windows.Graphics.Capture, et l'exemple pour un PC.
# 3. Le cadre jaune part-il si GraphicsCaptureAccess.RequestAccessAsync(Borderless) est
#    attendu avant la session : trois sessions de 1,5 s sur l'écran entier (avec le cadre, le
#    témoin de la mesure ; sans cadre sans accès demandé ; sans cadre, accès demandé avant),
#    l'écran pris par BitBlt et par la duplication avant et pendant, pixels changés sur le
#    bord et pixels jaunes comptés. Le témoin et la fenêtre de contrôle sont fermés pendant
#    ce temps (une pastille jaune compterait comme cadre).
# 5. WGC de l'écran entier (CreateForMonitor), périphérique sur la carte de l'écran, zone
#    découpée ensuite : valeurs k distinctes par seconde sur le témoin -Rapide (zone et écran
#    entier, 30 puis 15 i/s, 20 s) ; exclusion (a) au pixel près ; pointeur avec
#    IsCursorCaptureEnabled vrai puis faux.
# 7. images --methode pc, tel qu'il tournera chez l'utilisateur (une ligne, rien à
#    installer) : lancé ici pour prouver qu'il va au bout ; son fichier de résultats gardé.
#   pwsh -NoProfile -File sonde/film/09-s1-wgc-pc.ps1

. "$PSScriptRoot/commun.ps1"

if (Demarrer-Sonde -Nom '09-s1-wgc-pc' -Titre 'S1 points 3, 5 et 7 : Windows.Graphics.Capture et l''exemple pour un PC') {
    $t = $null; $c = $null
    try {
        $ecran = Ecran
        $loin = @(($ecran[0] + $ecran[2] - 8), ($ecran[1] + 8))
        $script:N::Poser($loin[0], $loin[1])

        Section 'S1 (3) : le cadre jaune de WGC (écran entier, 1,5 s de session)'
        $cadres = @{}
        foreach ($cas in @(
                @{ Nom = 'avec le cadre (témoin de la mesure)'; A = @('--avec-cadre') },
                @{ Nom = 'IsBorderRequired = false, sans accès demandé'; A = @() },
                @{ Nom = 'IsBorderRequired = false, RequestAccessAsync(Borderless) attendu avant'; A = @('--acces') })) {
            $r = Jouer-Exemple 'images' (@('--methode', 'wgc-cadre') + $cas.A) 60
            if ($r['ok'] -ne '1') { NonMesurable "WGC $($cas.Nom)" "$($r['erreur'])"; continue }
            $j = @("$($r['gdi_jaunes_avant_pendant'])" -split ';' | ForEach-Object { Nombre $_ })
            $bord = Nombre $r['gdi_bord_change']
            $vu = ($bord -gt 200) -or ($j.Count -eq 2 -and ($j[1] - $j[0]) -gt 200)
            $cadres[$cas.Nom] = $vu
            $acces = if ($r.ContainsKey('wgc_acces_sans_cadre')) { " ; accès sans cadre : $($r['wgc_acces_sans_cadre'])" } else { '' }
            Resultat "WGC $($cas.Nom)" "pris en charge $($r['wgc_pris_en_charge'])$acces ; SetIsBorderRequired(false) $($r['wgc_sans_cadre']) ; IsBorderRequired relu $($r['wgc_cadre_relu']) ; images en 1,5 s $($r['wgc_images_1500ms']) ; BitBlt : pixels du bord (6 px) changés $bord, pixels jaunes avant;pendant $($r['gdi_jaunes_avant_pendant']) ; duplication : bord $($r['dxgi_bord_change']), jaunes $($r['dxgi_jaunes_avant_pendant'])" 'à lire : cadre vu ou non' "cadre vu : $(OuiNon $vu)"
            if ($r.ContainsKey('wgc_appareil_materiel_refuse')) { Note "WGC $($cas.Nom) : appareil matériel refusé, WARP pris ($($r['wgc_appareil_materiel_refuse']))" }
        }
        $cle = 'IsBorderRequired = false, RequestAccessAsync(Borderless) attendu avant'
        if ($cadres.ContainsKey($cle)) {
            Resultat 'Réponse du point 3 : le cadre part-il avec l''accès demandé avant' "$(OuiNon (-not $cadres[$cle]))" 'noté seulement (attendu sur GitHub : NON, le cadre reste)' 'à lire'
        }
        if ($cadres.ContainsKey('avec le cadre (témoin de la mesure)') -and -not $cadres['avec le cadre (témoin de la mesure)']) {
            Note 'Le cadre n''est pas vu même quand il est demandé : la mesure ne le voit pas sur cette machine (BitBlt et la duplication ne prennent pas la fenêtre du cadre), les deux autres lignes ne prouvent donc rien.'
        }

        Section 'S1 (5) : WGC de l''écran entier, zone découpée, sur le témoin -Rapide'
        $t = Ouvrir-Temoin -Genre simple -X 100 -Y 100 -Rapide
        Start-Sleep -Seconds 4
        $ry = Rythme-Temoin $t 3
        Resultat 'Rythme du témoin -Rapide' "$([math]::Round($ry.ParS, 1)) pas par seconde" 'à lire' 'à lire'
        $zt = $t.Zone
        foreach ($fps in 30, 15) { $null = Prise-Temoin $t "wgc, 600 × 450, $fps i/s, 20 s" 'wgc' $zt $fps 20 }
        foreach ($fps in 30, 15) { $null = Prise-Temoin $t "wgc, tout l'écran, $fps i/s, 20 s" 'wgc' $ecran $fps 20 }
        Fermer-Temoin $t; $t = $null

        Section 'S1 (5) : WGC, (a) fenêtre de contrôle exclue, absente de l''image au pixel près (témoin fixe, tout l''écran)'
        $t = Ouvrir-Temoin -Genre fixe -X 100 -Y 100
        $zt = $t.Zone
        $bas = 48
        Note "Comparaisons hors des $bas dernières lignes (la barre des tâches, son horloge)."
        $ref1 = Image 'wgc' $ecran 'wgc-ref1'
        if ($null -eq $ref1.Image) { NonMesurable 'wgc : image sans la fenêtre' $ref1.Raison }
        else {
            $avec = $null; $vue = $null
            try {
                $c = Ouvrir-Controle -Zone $zt
                Start-Sleep -Milliseconds 700
                $avec = Image 'wgc' $ecran 'wgc-avec'
                Ecrire-Commande $c.Cmd 'affinite 0'
                $ok = Attendre -DelaiS 5 -Condition { "$((Lire-Etat $c.Etat)['affinite'])" -eq '0' }
                Start-Sleep -Milliseconds 500
                $vue = Image 'wgc' $ecran 'wgc-non-exclue'
                if (-not $ok) { Note 'wgc : affinité 0 non relue en 5 s (témoin à lire avec prudence)' }
            }
            catch { NonMesurable 'wgc : fenêtre de contrôle' "$($_.Exception.Message)" }
            finally { Fermer-Controle $c; $c = $null }
            Start-Sleep -Milliseconds 700
            $ref2 = Image 'wgc' $ecran 'wgc-ref2'
            $l = $ref1.Image.L
            $bruit = if ($ref2.Image) { $script:N::DifferencesSansBas($ref1.Image.Octets, $ref2.Image.Octets, $l, $bas) } else { "? ($($ref2.Raison))" }
            Resultat 'wgc : bruit (avant / après, sans la fenêtre)' "$bruit pixel(s)" '0' (Verdict ("$bruit" -eq '0'))
            if ($vue -and $vue.Image) {
                $d = $script:N::DifferencesSansBas($ref1.Image.Octets, $vue.Image.Octets, $l, $bas)
                Resultat 'wgc : fenêtre non exclue (affinité 0, témoin de la mesure)' "$d pixel(s) changés" 'plus de 0' (Verdict ($d -gt 0))
            }
            if ($avec -and $avec.Image) {
                $d = $script:N::DifferencesSansBas($ref1.Image.Octets, $avec.Image.Octets, $l, $bas)
                Resultat 'wgc : (a) fenêtre exclue (affinité 17) montrée sur tout l''écran' "$d pixel(s) changés" '0 (absente au pixel près)' (Verdict ($d -eq 0))
            }
            elseif ($avec) { NonMesurable 'wgc : image avec la fenêtre exclue' $avec.Raison }
        }

        Section 'S1 (5) : WGC, pointeur avec IsCursorCaptureEnabled vrai puis faux (témoin fixe)'
        $milieu = @(($zt[0] + 300), ($zt[1] + 225))
        $script:N::Poser($loin[0], $loin[1]); Start-Sleep -Milliseconds 300
        $hors = Image 'wgc' $zt 'wgc-pointeur-hors' @('--pointeur-wgc', '1')
        $script:N::Poser($milieu[0], $milieu[1]); Start-Sleep -Milliseconds 300
        $montre = $script:N::PointeurMontre()
        $vrai = Image 'wgc' $zt 'wgc-pointeur-vrai' @('--pointeur-wgc', '1')
        $faux = Image 'wgc' $zt 'wgc-pointeur-faux' @('--pointeur-wgc', '0')
        $script:N::Poser($loin[0], $loin[1])
        Resultat 'Pointeur montré par Windows' (OuiNon $montre) 'OUI (sinon la mesure ne prouve rien)' (OuiNon $montre)
        if ($hors.Image -and $vrai.Image) { Resultat 'IsCursorCaptureEnabled vrai : pixels changés, pointeur au milieu puis hors de la zone' "$($script:N::Differences($hors.Image.Octets, $vrai.Image.Octets))" 'plus de 0 : le pointeur est dans l''image' 'à lire' }
        else { NonMesurable 'IsCursorCaptureEnabled vrai' ("$($hors.Raison) $($vrai.Raison)".Trim()) }
        if ($hors.Image -and $faux.Image) {
            $d = $script:N::Differences($hors.Image.Octets, $faux.Image.Octets)
            Resultat 'IsCursorCaptureEnabled faux : pixels changés, pointeur au milieu puis hors de la zone' "$d" '0 : le pointeur n''est pas dans l''image' $(if ($montre) { Verdict ($d -eq 0) } else { 'non mesuré' })
        }
        else { NonMesurable 'IsCursorCaptureEnabled faux' ("$($hors.Raison) $($faux.Raison)".Trim()) }
        Fermer-Temoin $t; $t = $null

        Section 'S1 (7) : images --methode pc, l''exemple pour un PC sous Windows 11, lancé ici'
        $f = Fichier-Temp 'sonde-film-pc-resultats.txt'
        $chrono = [System.Diagnostics.Stopwatch]::StartNew()
        $r = Jouer-Exemple 'images' @('--methode', 'pc', '--sortie', "`"$f`"") 600
        $s = [int]$chrono.Elapsed.TotalSeconds
        if ($r['ok'] -ne '1') { NonMesurable 'images --methode pc' "$($r['erreur']) (après $s s)" }
        else {
            $lignes = @(Get-Content -LiteralPath $f -Encoding utf8)
            $erreurs = @($lignes | Where-Object { $_ -match '\berreur\b' })
            Resultat 'Va au bout, d''une ligne, sans rien installer' "OUI en $s s ; $($lignes.Count) lignes ; $($erreurs.Count) ligne(s) d'erreur" 'OUI' 'OUI'
            foreach ($l in @($lignes | Where-Object { $_ -match 'k_distincts_par_s|\[temoin|wgc_cadre_relu|gdi_bord_change|pixels différents|_total_ms' })) { Resultat 'pc' $l 'à lire' }
            foreach ($l in $erreurs) { Resultat 'pc : erreur' $l 'à lire' }
            Note "Fichier gardé : $(Garder-Fichier $f 's1-pc-resultats.txt') ; l'exécutable est publié dans l'artefact « sonde-film-pc » avec son LISEZMOI."
        }
    }
    catch { Erreur-Sonde $_ }
    finally { Fermer-Controle $c; Fermer-Temoin $t }
}
Terminer-Sonde
