# S1 points 1, 2 et 4 (contrat 0.7, §1.1), et (a) à (e) pour les moyens 1 et 2 :
# 1. BitBlt (code de systeme::vivant, recopié) et 2. duplication DXGI (périphérique sur la
#    carte de l'écran de la zone, AcquireNextFrame à délai nul, rectangles sales croisés avec
#    la zone), sur la zone du témoin (600 × 450) et sur tout l'écran, 20 s à 30 puis à 15
#    images par seconde, sur le témoin -Rapide (k avance à chaque image composée ; son rythme
#    réel est noté d'abord) : temps par prise, prises par seconde, valeurs k distinctes par
#    seconde ; tout l'écran à 30 i/s pendant 100 s (3 000 images) avec la mémoire et les
#    objets GDI (e) et les pertes d'accès ; ce que rend la duplication quand rien ne bouge
#    dans la zone (délai dépassé, rectangles sales hors de la zone).
# 4. Ouverture à froid (cinq processus neufs) puis à chaud (le même processus) : carte,
#    D3D11CreateDevice, DuplicateOutput, première image ; BitBlt et sa première prise.
# (a) fenêtre de contrôle exclue absente de l'image au pixel près ; (b) pointeur dans
# l'image ; (c) coût de la lecture du pointeur ; (d) cadence avec la fenêtre montrée.
# WGC (points 3 et 5), l'accès perdu (6) et l'exemple pour un PC (7) : 09-s1-wgc-pc.ps1 et
# 08-s7-ecrans.ps1.
# -Passe2 (contrat §1.10) : seulement (b), le pointeur montré par les touches souris.
#   pwsh -NoProfile -File sonde/film/02-s1-images.ps1 [-Passe2]

param([switch]$Passe2)

. "$PSScriptRoot/commun.ps1"

if (Demarrer-Sonde -Nom '02-s1-images' -Titre 'S1 points 1, 2 et 4 : BitBlt et duplication DXGI à la cadence d''une vidéo') {
    $t = $null; $c = $null; $t2 = $null
    try {
        Section 'S1 (2) : cartes et écrans vus par DXGI'
        $r = Jouer-Exemple 'images' @('--methode', 'adaptateurs') 30
        if ($r['ok'] -ne '1') { NonMesurable 'CreateDXGIFactory1 / EnumAdapters1' "$($r['erreur'])" }
        else {
            Resultat 'Cartes' "$($r['adaptateurs'])" 'à lire'
            foreach ($k in @($r.Keys | Where-Object { $_ -like 'adaptateur_*' -or $_ -like 'ecran_*' } | Sort-Object)) { Resultat $k $r[$k] 'à lire' }
        }
        $ecran = Ecran
        Resultat 'Écran principal' (Rect-Texte $ecran) '1024 × 768 attendu'
        $loin = @(($ecran[0] + $ecran[2] - 8), ($ecran[1] + 8))
        $script:Natif::Poser($loin[0], $loin[1])

        if (-not $Passe2) {
            Section 'Témoin -Rapide : son rythme réel (k à chaque image composée, CompositionTarget.Rendering)'
            $t = Ouvrir-Temoin -Genre simple -X 100 -Y 100 -Rapide
            Start-Sleep -Seconds 6
            $ry = Rythme-Temoin $t 5
            Resultat 'Rythme du témoin -Rapide (5 s)' "$([math]::Round($ry.ParS, 1)) pas par seconde ; écart entre deux pas $(Stats $ry.Ecarts)" 'à lire : 30 ou plus avec une carte graphique ; sinon les critères se lisent par rapport à lui' 'à lire'
            Resultat 'Seuil du critère S1' "$(Seuil-K $ry.ParS) valeurs k distinctes par seconde" '28, ou 93 % du rythme du témoin s''il est sous 30' 'à lire'
            $zt = $t.Zone
            Resultat 'Zone du témoin (pixels réels)' (Rect-Texte $zt) '100,100,600,450'

            Section 'S1 (1, 2) : cadence sur le témoin -Rapide'
            foreach ($m in 'bitblt', 'dxgi') {
                foreach ($fps in 30, 15) { $null = Prise-Temoin $t "$m, 600 × 450, $fps i/s, 20 s" $m $zt $fps 20 }
                $null = Prise-Temoin $t "$m, tout l'écran, 30 i/s, 100 s (3 000 images)" $m $ecran 30 100 @('--ressources') -Critere:($m -eq 'dxgi')
                $null = Prise-Temoin $t "$m, tout l'écran, 15 i/s, 20 s" $m $ecran 15 20
            }

            Section 'S1 (c) : coût de la lecture du pointeur (systeme::gel::lire_pointeur, recopiée)'
            $r = Jouer-Exemple 'images' @('--methode', 'pointeur', '--n', '300') 60
            if ($r['ok'] -ne '1') { NonMesurable 'lecture du pointeur' "$($r['erreur'])" }
            else {
                Resultat 'GetCursorInfo (300 lectures)' (Stats $r['pointeur_getcursorinfo_ms']) 'à lire'
                Resultat 'GetCursorInfo et forme (CopyIcon, GetIconInfo, DrawIconEx)' (Stats $r['pointeur_forme_ms']) 'à lire'
                Resultat 'Pointeur montré pendant les lectures' "$($r['pointeur_montre'])" 'à lire (forme forcée sur la flèche s''il est caché)'
            }
            foreach ($m in 'bitblt', 'dxgi') { $null = Prise-Temoin $t "$m, 600 × 450, 30 i/s, 10 s, pointeur lu à chaque image" $m $zt 30 10 @('--pointeur') }

            Section 'S1 (d) : cadence avec la fenêtre de contrôle montrée sur tout l''écran'
            Note 'À comparer aux lignes « tout l''écran, 30 i/s » sans elle (plus haut) ; S5 s''y rapporte aussi.'
            try {
                $c = Ouvrir-Controle -Zone $zt
                $ec = Lire-Etat $c.Etat
                Resultat 'Fenêtre de contrôle' "affinité $($ec['affinite']) ; styles étendus $($ec['exstyle']) ; traversée $($ec['traverse'])" 'affinité 17 (WDA_EXCLUDEFROMCAPTURE)' (Verdict ("$($ec['affinite'])" -eq '17'))
                foreach ($m in 'bitblt', 'dxgi') { $null = Prise-Temoin $t "$m, tout l'écran, 30 i/s, 20 s, fenêtre de contrôle montrée" $m $ecran 30 20 -Critere:($m -eq 'dxgi') }
            }
            catch { NonMesurable 'cadence avec la fenêtre de contrôle' "$($_.Exception.Message)" }
            finally { Fermer-Controle $c; $c = $null }
            Fermer-Temoin $t; $t = $null

            Section 'S1 (4) : ouverture, à froid (cinq processus neufs) puis à chaud (le même processus)'
            $z = @(100, 100, 600, 450)
            $v = @{ froid_dxgi = @(); chaud_dxgi = @(); froid_bitblt = @(); chaud_bitblt = @() }
            for ($i = 1; $i -le 5; $i++) {
                $r = Jouer-Exemple 'images' @('--methode', 'ouverture', '--zone', (Rect-Texte $z)) 60
                if ($r['ok'] -ne '1') { NonMesurable "ouverture $i" "$($r['erreur'])"; continue }
                foreach ($k in 'froid_dxgi', 'chaud_dxgi', 'froid_bitblt', 'chaud_bitblt') {
                    Resultat "ouverture $i, $($k -replace '_', ' ')" "$($r[$k])" 'à lire' 'à lire'
                    if ($r.ContainsKey("$($k)_total_ms")) { $v[$k] += (Nombre $r["$($k)_total_ms"]) }
                }
            }
            foreach ($k in 'froid_dxgi', 'chaud_dxgi', 'froid_bitblt', 'chaud_bitblt') {
                if ($v[$k].Count) { Resultat "ouverture et première image, $($k -replace '_', ' ') (médiane de $($v[$k].Count))" (Ms (Mediane $v[$k])) 'à lire (ce que F9 doit faire tenir dans le décompte)' 'à lire' }
            }

            Section 'S1 (2) : ce que rend la duplication quand rien ne bouge dans la zone'
            Note 'Témoin fixe en (0, 0) ; un témoin simple (k toutes les 100 ms) en (424, 318), hors de la zone mesurée (0, 0, 200, 150) : l''écran change, pas la zone.'
            $t = Ouvrir-Temoin -Genre fixe -X 0 -Y 0
            $t2 = Ouvrir-Temoin -Genre simple -X 424 -Y 318
            Start-Sleep -Milliseconds 800
            foreach ($att in 0, 100) {
                $r = Jouer-Exemple 'images' @('--methode', 'dxgi-repos', '--zone', '0,0,200,150', '--duree', '5', '--attente-ms', "$att") 60
                if ($r['ok'] -ne '1') { NonMesurable "duplication au repos (délai $att ms)" "$($r['erreur'])"; continue }
                Resultat "AcquireNextFrame, délai $att ms, 5 s" "$($r['repos_appels']) appels ; délai dépassé $($r['repos_delais_depasses']) ; images de l'écran neuves $($r['repos_ecran_neuves']) ; dont zone intacte (rectangles sales hors de la zone) $($r['repos_zone_intacte']) ; images neuves rendues pour la zone $($r['repos_images_neuves']) ; pointeur seul $($r['repos_pointeur_seul']) ; sans métadonnées $($r['repos_sans_metadonnees']) ; pertes $($r['repos_pertes_acces'])" 'zone : aucune image neuve ; l''écran : environ 10 par seconde' (Verdict ((Nombre $r['repos_images_neuves']) -eq 0))
                Resultat "Temps d'un appel (délai $att ms)" (Stats $r['repos_acquisition_ms']) 'à lire'
            }
            Fermer-Temoin $t2; $t2 = $null
            Fermer-Temoin $t; $t = $null

            Section 'S1 (a) : fenêtre de contrôle exclue, absente de l''image au pixel près (témoin fixe, tout l''écran)'
            $t = Ouvrir-Temoin -Genre fixe -X 100 -Y 100
            $zt = $t.Zone
            $bas = 48
            Note "Comparaisons hors des $bas dernières lignes (la barre des tâches, son horloge) ; le pointeur est loin du témoin."
            foreach ($m in 'bitblt', 'dxgi') {
                $ref1 = Image $m $ecran "$m-ref1"
                if ($null -eq $ref1.Image) { NonMesurable "$m : image sans la fenêtre" $ref1.Raison; continue }
                $avec = $null; $vue = $null
                try {
                    $c = Ouvrir-Controle -Zone $zt
                    Start-Sleep -Milliseconds 700
                    $avec = Image $m $ecran "$m-avec"
                    Ecrire-Commande $c.Cmd 'affinite 0'
                    $ok = Attendre -DelaiS 5 -Condition { "$((Lire-Etat $c.Etat)['affinite'])" -eq '0' }
                    Start-Sleep -Milliseconds 500
                    $vue = Image $m $ecran "$m-non-exclue"
                    if (-not $ok) { Note "$m : affinité 0 non relue en 5 s (témoin à lire avec prudence)" }
                }
                catch { NonMesurable "$m : fenêtre de contrôle" "$($_.Exception.Message)" }
                finally { Fermer-Controle $c; $c = $null }
                Start-Sleep -Milliseconds 700
                $ref2 = Image $m $ecran "$m-ref2"
                $l = $ref1.Image.L
                $bruit = if ($ref2.Image) { $script:Natif::DifferencesSansBas($ref1.Image.Octets, $ref2.Image.Octets, $l, $bas) } else { "? ($($ref2.Raison))" }
                Resultat "$m : bruit (avant / après, sans la fenêtre)" "$bruit pixel(s)" '0' (Verdict ("$bruit" -eq '0'))
                if ($vue -and $vue.Image) {
                    $d = $script:Natif::DifferencesSansBas($ref1.Image.Octets, $vue.Image.Octets, $l, $bas)
                    Resultat "$m : fenêtre non exclue (affinité 0, témoin de la mesure)" "$d pixel(s) changés" 'plus de 0 (sinon la mesure ne voit rien)' (Verdict ($d -gt 0))
                    Note "$m : image non exclue gardée : $(Garder-Fichier $vue.Fichier "s1-a-$m-non-exclue.png")"
                }
                if ($avec -and $avec.Image) {
                    $d = $script:Natif::DifferencesSansBas($ref1.Image.Octets, $avec.Image.Octets, $l, $bas)
                    Resultat "$m : (a) fenêtre exclue (affinité 17) montrée sur tout l'écran" "$d pixel(s) changés" '0 (absente au pixel près)' (Verdict ($d -eq 0))
                }
                elseif ($avec) { NonMesurable "$m : image avec la fenêtre exclue" $avec.Raison }
            }
        }

        if ($Passe2) { $t = Ouvrir-Temoin -Genre fixe -X 100 -Y 100; $zt = $t.Zone }
        Section 'S1 (b) : le pointeur est-il dans l''image (témoin fixe)'
        $milieu = @(($zt[0] + 300), ($zt[1] + 225))
        $pm = Montrer-Pointeur $milieu[0] $milieu[1]
        Resultat 'Pointeur montré par Windows avant la mesure' $pm.Detail 'OUI (sinon la mesure ne prouve rien)' (OuiNon $pm.Montre)
        foreach ($m in 'bitblt', 'dxgi') {
            $script:Natif::Poser($loin[0], $loin[1]); Start-Sleep -Milliseconds 300
            $hors = Image $m $zt "$m-pointeur-hors"
            $script:Natif::Poser($milieu[0], $milieu[1]); Start-Sleep -Milliseconds 300
            $montre = $script:Natif::PointeurMontre()
            $dans = Image $m $zt "$m-pointeur-dans"
            $script:Natif::Poser($loin[0], $loin[1])
            if ($null -eq $hors.Image -or $null -eq $dans.Image) { NonMesurable "$m : pointeur" ("$($hors.Raison) $($dans.Raison)".Trim()); continue }
            $d = $script:Natif::Differences($hors.Image.Octets, $dans.Image.Octets)
            Resultat "$m : pixels changés, pointeur au milieu de la zone puis hors d'elle" "$d (pointeur montré par Windows : $(OuiNon $montre))" '0 : le pointeur n''est pas dans l''image' $(if ($montre) { Verdict ($d -eq 0) } else { 'non mesuré' })
            if (-not $montre) { Note "$m : Windows cache le pointeur sans souris branchée : la mesure ne prouve rien (dite non mesurée)." }
        }
    }
    catch { Erreur-Sonde $_ }
    finally { Fermer-Controle $c; Fermer-Temoin $t; Fermer-Temoin $t2 }
}
Terminer-Sonde
