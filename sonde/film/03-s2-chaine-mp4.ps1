# S2 points 10, 11 et 13, puis le reste de S2 (3, 4, 5, 6, 9) (contrat 0.7, §1.2, ordre
# du §1.8) :
# 10. La chaîne complète à deux fils (prise, pointeur, copie, file bornée à 4 ; un fil seul
#     crée l'écrivain, convertit, écrit, finit), puis à un fil : BitBlt et duplication sur le
#     témoin -Rapide en 600 × 450 et 1024 × 768, puis une image de 2560 × 1440 faite par
#     l'exemple ; images écrites par seconde, valeurs k distinctes par seconde du fichier
#     relu, pas sautés, temps de chaque étape.
# 11. Préparation de l'écrivain, à froid (cinq processus) puis à chaud, matériel 1 puis 0.
# 13. MP4 fragmenté (MFCreateFMPEG4MediaSink) : accepté ; processus tué au milieu
#     (Stop-Process) : lisible jusqu'à la coupure par IMFSourceReader et par Edge ; le même
#     pour un MP4 ordinaire (attendu illisible) ; le fichier fini lu. Finalize à 5 et 60 min :
#     10-s2-finalize.ps1 (job à part).
# 3. Le codeur pris avec MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS à 1 puis à 0.
# 4. Vitesse : WriteSample à 30 i/s pendant 20 s, témoin simple, fixe et plein, 600 × 450 et
#    1024 × 768 ; processeur ; taille.
# 5. Réglages du codeur : mode qualité, débit moyen, images clés toutes les 2 s.
# 6. Justesse relue par IMFSourceReader (RGB32 stride positif, par défaut, négatif ; NV12
#    converti par l'exemple) : durée, images, sens, pastilles, k.
# 9. Le fichier dans Edge (WebView2) : duration, videoWidth, première image peinte.
# -Passe2 (contrat §1.10) : seulement ce qui reste à mesurer : 10 (600 × 450 à deux fils et à
# un fil, NV12 en 600 × 450, 1024 × 768 et 2560 × 1440), 13 (coupure, relue), 6 et 9.
#   pwsh -NoProfile -File sonde/film/03-s2-chaine-mp4.ps1 [-Passe2]

param([switch]$Passe2)

. "$PSScriptRoot/commun.ps1"

function Montrer-Chaine([string]$Nom, $R, [double]$Rythme, [double]$Seuil) {
    if ($R['ok'] -ne '1') { NonMesurable $Nom "$($R['erreur'])"; return }
    $kps = Nombre $R['relu_k_distincts_par_s']
    $crit = if ($Seuil -gt 0) { "au moins $Seuil images fraîches par seconde (témoin à $([math]::Round($Rythme, 1)) pas/s)" } else { 'à lire' }
    Resultat "$Nom : valeurs k distinctes par seconde dans le fichier relu" "$($R['relu_k_distincts_par_s']) ($($R['relu_k_distincts']) en $($R['duree_s']) s) ; relues $($R['relu_images']) images, $($R['relu_duree_s']) s ; k illisibles $($R['relu_k_illisibles']) ; reculs $($R['relu_k_reculs'])$(if ($R.ContainsKey('relu_refuse')) { " ; relecture refusée : $($R['relu_refuse'])" })" $crit $(if ($Seuil -gt 0) { Verdict ($kps -ge $Seuil) } else { 'à lire' })
    Resultat "$Nom : images" "prises $($R['images_prises']) ; écrites $($R['images_ecrites']) ($($R['ecrites_par_s']) par seconde) ; créneaux manqués (retard) $($R['creneaux_manques']) ; perdues file pleine $($R['file_pleine'])" 'à lire' 'à lire'
    Resultat "$Nom : temps par étape" "prise $(Stats $R['prise_ms']) ; pointeur $(Stats $R['pointeur_ms']) ; copie $(Stats $R['copie_ms']) ; conversion $(Stats $R['conversion_ms']) ; préparation de l'échantillon $(Stats $R['preparation_ms']) ; WriteSample $(Stats $R['writesample_ms']) ; codeur par image $(Stats $R['codeur_par_image_ms']) ; Finalize $(Ms $R['finalize_ms'])" 'à lire' 'à lire'
    Resultat "$Nom : processeur, taille" "$($R['processeur_pourcent_coeur']) % d'un cœur ; $(Mo $R['taille_octets'])" 'à lire' 'à lire'
}

if (Demarrer-Sonde -Nom '03-s2-chaine-mp4' -Titre 'S2 points 10, 11, 13, puis 3, 4, 5, 6, 9 : la chaîne, la préparation, le MP4 fragmenté, le codeur') {
    $t = $null
    try {
        $d = $script:Sonde.Temp
        $ecran = Ecran

        Section 'S2 (10) : la chaîne complète, à deux fils puis à un fil (20 s, 30 i/s)'
        $t = Ouvrir-Temoin -Genre simple -X 100 -Y 100 -Rapide
        Start-Sleep -Seconds 3
        $zt = $t.Zone
        $tem = "$($zt[0]),$($zt[1])"
        $zones = @(@{ N = '600 × 450'; Z = $zt })
        if (-not $Passe2) { $zones += @{ N = '1024 × 768 (tout l''écran)'; Z = $ecran } }
        foreach ($src in 'bitblt', 'dxgi') {
            foreach ($z in $zones) {
                foreach ($fils in 2, 1) {
                    $f = Join-Path $d "chaine-$src-$fils.mp4"
                    $r = Jouer-Exemple 'mf' @('--etape', 'chaine', '--source', $src, '--zone', (Rect-Texte $z.Z), '--temoin', $tem, '--fils', "$fils", '--duree', '20', '--sortie', "`"$f`"") 120
                    $ry = (Rythme-Temoin $t 20).ParS
                    $seuil = if ($fils -eq 2 -and $z.N -eq '600 × 450') { if ($src -eq 'bitblt') { 24 } else { 28 } } else { 0 }
                    Montrer-Chaine "$src, $($z.N), $fils fil(s)" $r $ry $seuil
                }
            }
        }
        foreach ($src in 'bitblt', 'dxgi') {
            foreach ($z in @(@{ N = '600 × 450'; Z = $zt; F = '600' }, @{ N = '1024 × 768 (tout l''écran)'; Z = $ecran; F = 'ecran' })) {
                $f = Join-Path $d "chaine-$src-nv12-$($z.F).mp4"
                $r = Jouer-Exemple 'mf' @('--etape', 'chaine', '--source', $src, '--zone', (Rect-Texte $z.Z), '--temoin', $tem, '--fils', '2', '--entree', 'nv12', '--duree', '20', '--sortie', "`"$f`"") 120
                Montrer-Chaine "$src, $($z.N), 2 fils, entrée NV12 convertie par l'exemple" $r (Rythme-Temoin $t 20).ParS 0
            }
        }
        Fermer-Temoin $t; $t = $null
        foreach ($c in @(@{ Fils = 2; Nv12 = $false }, @{ Fils = 2; Nv12 = $true }, @{ Fils = 1; Nv12 = $false })) {
            $f = Join-Path $d "chaine-synthese-$($c.Fils)-$([int]$c.Nv12).mp4"
            $a = @('--etape', 'chaine', '--source', 'synthetique', '--taille', '2560x1440', '--fils', "$($c.Fils)", '--duree', '20', '--sortie', "`"$f`"")
            if ($c.Nv12) { $a += @('--entree', 'nv12') }
            $r = Jouer-Exemple 'mf' $a 180
            Montrer-Chaine "image de 2560 × 1440 faite par l'exemple (k à chaque créneau), $($c.Fils) fil(s)$(if ($c.Nv12) { ', entrée NV12 convertie par l''exemple' })" $r 30 0
        }
        Note 'Le critère (contrat §1.2) : à deux fils, au moins 24 images fraîches par seconde avec BitBlt et 28 avec la duplication en 600 × 450 ; un témoin sous 30 pas par seconde borne ce que la mesure peut voir.'

        if (-not $Passe2) {
            Section 'S2 (11) : préparation de l''écrivain, à froid (cinq processus) puis à chaud'
            foreach ($m in '1', '0') {
                $v = @{ froid = @(); chaud = @() }
                for ($i = 1; $i -le 5; $i++) {
                    $r = Jouer-Exemple 'mf' @('--etape', 'preparation', '--materiel', $m, '--dossier', "`"$d`"") 60
                    if ($r['ok'] -ne '1') { NonMesurable "préparation $i (matériel $m)" "$($r['erreur'])"; continue }
                    foreach ($tour in 'froid', 'chaud') {
                        Resultat "préparation $i, matériel $m, $tour" "$($r[$tour])" 'à lire' 'à lire'
                        $v[$tour] += (Nombre $r["$($tour)_total_ms"])
                    }
                }
                foreach ($tour in 'froid', 'chaud') { if ($v[$tour].Count) { Resultat "préparation jusqu'à la première image, matériel $m, $tour (médiane de $($v[$tour].Count))" (Ms (Mediane $v[$tour])) 'à lire (avec S1 point 4 : ce que F9 doit faire tenir dans le décompte, la borne de F11)' 'à lire' } }
            }

        }

        Section 'S2 (13) : MP4 fragmenté, et le processus tué au milieu (Stop-Process)'
        foreach ($cas in 'ordinaire', 'fragmente') {
            $f = Join-Path $d "coupe-$cas.mp4"
            $pr = Join-Path $d "coupe-$cas.progres"
            $a = @('--etape', 'finaliser', '--secondes', '60', '--temps-reel', '--progres', "`"$pr`"", '--sortie', "`"$f`"")
            if ($cas -eq 'fragmente') { $a += '--fragmente' }
            $p = Lancer-Exemple 'mf' $a
            if ($null -eq $p) { NonMesurable "$cas : coupure" 'exemple mf absent'; continue }
            $ok = Attendre -DelaiS 40 -PasMs 100 -Condition { (Nombre (Lire-Etat $pr)['images']) -ge 300 -or $p.HasExited }
            $n = (Lire-Etat $pr)['images']
            if (-not $ok -or $p.HasExited) {
                $out = Get-ChildItem -LiteralPath $script:Sonde.Temp -Filter 'mf-vivant-*.txt' | Sort-Object LastWriteTime | Select-Object -Last 1
                NonMesurable "$cas : écriture avant la coupure" "$(if ($p.HasExited) { "sorti, code $(Code-Sortie $p.ExitCode) : $(if ($out) { (Get-Content -LiteralPath $out.FullName -Raw) -replace "`r?`n", ' ' })" } else { "pas 300 images en 40 s ($n)" })"
                try { $p.Kill($true) } catch { }
                continue
            }
            Stop-Process -Id $p.Id -Force
            $null = $p.WaitForExit(5000)
            Start-Sleep -Milliseconds 300
            $taille = if (Test-Path -LiteralPath $f) { (Get-Item -LiteralPath $f).Length } else { 0 }
            Resultat "$cas : coupé par Stop-Process" "après $n images annoncées ; fichier $(Mo $taille)" 'à lire' 'à lire'
            $r = Jouer-Exemple 'mf' @('--etape', 'relire', '--fichier', "`"$f`"") 120
            $lisible = $r['lisible'] -eq '1' -and (Nombre $r['relu_images']) -gt 0
            $crit = if ($cas -eq 'fragmente') { 'lisible jusqu''à la coupure (à 1 s près)' } else { 'attendu illisible (l''index n''est écrit que par Finalize)' }
            Resultat "$cas : relu par IMFSourceReader après la coupure" "lisible $(OuiNon $lisible) ; $(if ($lisible) { "$($r['relu_images']) images, durée annoncée $($r['relu_duree_s']) s, dernier horodatage $($r['relu_dernier_horodatage_s']) s, k $($r['relu_k_premier_dernier'] -replace ';', ' à '), reculs $($r['relu_k_reculs'])$(if ($r.ContainsKey('relu_arret')) { ", arrêt : $($r['relu_arret'])" })" } else { "$($r['lecture_refusee'])$($r['erreur'])" }) ; boîtes $($r['boites'])" $crit $(if ($cas -eq 'fragmente') { Verdict ($lisible -and (Nombre $r['relu_images']) -ge ((Nombre $n) - 30)) } else { OuiNon $lisible })
            $e = Lire-Dans-Edge $f 'video'
            Resultat "$cas : ouvert dans Edge (WebView2) après la coupure" "durée $($e['duree_s']) s ; taille $($e['taille']) ; première image $($e['premiere_image_ms']) ms ; images en 2 s $($e['images_presentees_2s'])$(foreach ($k in 'erreur_media', 'play_refuse', 'erreur', 'delai') { if ($e.ContainsKey($k)) { " ; $k $($e[$k])" } })" $crit (OuiNon ($e.ContainsKey('premiere_image_ms')))
            Note "$cas coupé gardé : $(Garder-Fichier $f "s2-13-coupe-$cas.mp4")"
        }
        $tailles = @{}
        if (-not $Passe2) {
            foreach ($cas in 'ordinaire', 'fragmente') {
                $f = Join-Path $d "fini-$cas.mp4"
                $a = @('--etape', 'finaliser', '--secondes', '10', '--sortie', "`"$f`"")
                if ($cas -eq 'fragmente') { $a += '--fragmente' }
                $r = Jouer-Exemple 'mf' $a 120
                if ($r['ok'] -ne '1') { NonMesurable "$cas fini (10 s)" "$($r['erreur'])"; continue }
                $tailles[$cas] = Nombre $r['taille_octets']
                Resultat "$cas fini (10 s) : accepté, Finalize, taille" "accepté ; Finalize $(Ms $r['finalize_ms']) ; $(Mo $r['taille_octets']) ; boîtes $($r['boites'])" 'accepté' 'passe'
                $l = Jouer-Exemple 'mf' @('--etape', 'relire', '--fichier', "`"$f`"") 120
                Resultat "$cas fini : relu par IMFSourceReader" "lisible $(OuiNon ($l['lisible'] -eq '1')) ; $($l['relu_images']) images ; $($l['relu_duree_s']) s ; k reculs $($l['relu_k_reculs'])$($l['lecture_refusee'])" '300 images, 10 s' (Verdict ($l['lisible'] -eq '1' -and [math]::Abs((Nombre $l['relu_images']) - 300) -le 2))
                $e = Lire-Dans-Edge $f 'video'
                Resultat "$cas fini : ouvert dans Edge (WebView2)" "durée $($e['duree_s']) s ; taille $($e['taille']) ; première image $($e['premiere_image_ms']) ms ; images en 2 s $($e['images_presentees_2s'])$(foreach ($k in 'erreur_media', 'play_refuse', 'erreur', 'delai') { if ($e.ContainsKey($k)) { " ; $k $($e[$k])" } })" 'durée 10 s, image peinte' (Verdict ($e.ContainsKey('premiere_image_ms') -and [math]::Abs((Nombre $e['duree_s']) - 10) -le 0.2))
                Note "$cas fini gardé : $(Garder-Fichier $f "s2-13-fini-$cas.mp4")"
            }
            if ($tailles.Count -eq 2) { Resultat 'Taille fragmenté / ordinaire (10 s)' ('{0:0.000}' -f ($tailles['fragmente'] / [math]::Max(1, $tailles['ordinaire']))) 'à lire' 'à lire' }
            Note 'Le lecteur de Windows (Films et TV, Lecteur multimédia) ne se pilote pas ici sans personne, et n''est pas sur un Windows Server : noté, pour la liste de l''utilisateur.'

            Section 'S2 (3) : codeur pris, MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS à 1 puis à 0'
            $r = Jouer-Exemple 'mf' @('--etape', 'materiel', '--dossier', "`"$d`"") 120
            if ($r['ok'] -ne '1') { NonMesurable 'chaîne des transformations' "$($r['erreur'])" }
            foreach ($m in '1', '0') { if ($r.ContainsKey("materiel_$m")) { Resultat "matériel $m : transformations du flux vidéo (IMFSinkWriterEx)" $r["materiel_$m"] 'à lire (attendu ici : logiciel dans les deux cas)' 'à lire' } }

            Section 'S2 (4) : vitesse à 30 i/s pendant 20 s (WriteSample par image)'
            $fond = Fichier-Temp 'fond.png'
            try { Copie-Ecran $fond } catch { Note "copie de l'écran pour le fond : $($_.Exception.Message)"; $fond = $null }
            foreach ($taille in '600x450', '1024x768') {
                foreach ($g in 'simple', 'fixe', 'plein') {
                    $f = Join-Path $d "vitesse-$g-$taille.mp4"
                    $a = @('--etape', 'vitesse', '--genre', $g, '--taille', $taille, '--duree', '20', '--sortie', "`"$f`"")
                    if ($fond) { $a += @('--fond', "`"$fond`"") }
                    $r = Jouer-Exemple 'mf' $a 120
                    if ($r['ok'] -ne '1') { NonMesurable "vitesse $g $taille" "$($r['erreur'])"; continue }
                    $med = Stat-Mediane $r['writesample_ms']
                    $crit = if ($taille -eq '600x450') { 'médiane de WriteSample au plus 15 ms' } else { 'à lire' }
                    Resultat "vitesse $g $taille : WriteSample" "$(Stats $r['writesample_ms']) ; préparation $(Stats $r['preparation_ms']) ; images en retard $($r['images_en_retard']) / $($r['images'])" $crit $(if ($taille -eq '600x450') { Verdict ($med -le 15) } else { 'à lire' })
                    Resultat "vitesse $g $taille : processeur, Finalize, taille" "$($r['processeur_pourcent_coeur']) % d'un cœur ; Finalize $(Ms $r['finalize_ms']) ; mémoire privée au plus $($r['prive_pic_mo']) Mo ; fichier $(Mo $r['taille_octets'])" 'à lire' 'à lire'
                    if ($g -ne 'fixe' -and $taille -eq '600x450') { Note "vitesse $g $taille gardé : $(Garder-Fichier $f "s2-4-vitesse-$g-$taille.mp4")" }
                }
        }

        Section 'S2 (5) : réglages du codeur'
        $r = Jouer-Exemple 'mf' @('--etape', 'reglages', '--dossier', "`"$d`"") 180
        if ($r['ok'] -ne '1') { NonMesurable 'réglages' "$($r['erreur'])" }
        foreach ($k in 'reglages_defaut_debit_moyen', 'reglages_qualite_par_parametres', 'reglages_gop_2s_par_parametres', 'icodecapi', 'icodecapi_mode_debit', 'icodecapi_qualite', 'icodecapi_gop', 'reglages_icodecapi') {
            if (-not $r.ContainsKey($k)) { continue }
            $v = "$($r[$k])"
            $ver = 'à lire'
            if ($k -like '*gop*' -or $k -eq 'reglages_icodecapi') {
                if ($v -match '\[([0-9, ]*)\]') { $cles = @($Matches[1] -split ',' | Where-Object { $_.Trim() } | ForEach-Object { [int]$_ }); $ver = OuiNon ($cles.Count -ge 2 -and @($cles | Where-Object { $_ % 60 -ne 0 }).Count -eq 0) }
            }
            elseif ($k -like '*qualite*' -or $k -eq 'icodecapi_mode_debit') { $ver = OuiNon ($v -match 'accept') }
            Resultat $k $v 'à lire : accepté ? images clés toutes les 60 images (2 s) pour le GOP' $ver
        }

        }

        Section 'S2 (6) : justesse, relu par IMFSourceReader (10 s, 300 images)'
        Note 'Le lecteur relit la géométrie (taille du tampon, stride, MF_MT_MINIMUM_DISPLAY_APERTURE) à chaque changement de type signalé par ReadSample, et copie la zone d''affichage ligne par ligne avec le pas réel ; les images relues 0, 1 et 150 sont gardées en PNG.'
        $cas6 = @(
            @{ K = 'positif'; N = 'RGB32, stride positif'; A = @('--stride', 'positif'); Edge = $true },
            @{ K = 'defaut'; N = 'RGB32, stride non posé'; A = @('--stride', 'defaut'); Edge = $false },
            @{ K = 'negatif'; N = 'RGB32, stride négatif'; A = @('--stride', 'negatif'); Edge = $false },
            @{ K = 'nv12'; N = 'NV12 converti par l''exemple'; A = @('--entree', 'nv12'); Edge = $true },
            @{ K = '602x452'; N = 'RGB32, stride positif, 602 × 452 (côtés non multiples de 16)'; A = @('--stride', 'positif', '--taille', '602x452'); Edge = $true })
        foreach ($cas in $cas6) {
            $f = Join-Path $d "justesse-$($cas.K).mp4"
            $png = Join-Path $d "justesse-$($cas.K).png"
            $pref = Join-Path $d "relue-$($cas.K)"
            $r = Jouer-Exemple 'mf' (@('--etape', 'justesse', '--sortie', "`"$f`"", '--png', "`"$png`"", '--png-relues', "`"$pref`"") + $cas.A) 180
            if ($r['ok'] -ne '1') { NonMesurable "justesse $($cas.N)" "$($r['erreur'])"; continue }
            $duree = Nombre $r['relu_duree_s']; $n = Nombre $r['relu_images']
            Resultat "$($cas.N) : durée et images relues" "$($r['relu_duree_s']) s ; $($r['relu_images']) images ; taille écrite $($r['taille_ecrite']), relue $($r['relu_taille']) ; stride relu $($r['relu_stride']) ; premier horodatage $($r['relu_premier_horodatage_ms']) ms" '10 s à 100 ms près, 300 images à 2 près' (Verdict ([math]::Abs($duree - 10) -le 0.1 -and [math]::Abs($n - 300) -le 2))
            Resultat "$($cas.N) : géométrie relue (à l'ouverture et à chaque changement de type)" "$($r['relu_geometrie']) ; changements de type $($r['relu_changements_de_type'])" 'à lire' 'à lire'
            Resultat "$($cas.N) : taille relue égale à la taille écrite" "$($r['relu_geometrie_egale'])" '1' (OuiNon ("$($r['relu_geometrie_egale'])" -eq '1'))
            Resultat "$($cas.N) : sens de l'image" "à l'endroit $($r['images_a_l_endroit']) ; à l'envers $($r['images_a_l_envers']) ; k illisible $($r['images_k_illisible']) (témoin en $($r['temoin_dans_l_image']))" 'toutes à l''endroit' (Verdict ((Nombre $r['images_a_l_envers']) -eq 0 -and (Nombre $r['images_a_l_endroit']) -ge ($n - 2)))
            $px = "$($r['pastilles_ecart_max_par_canal'])" -split ';'
            Resultat "$($cas.N) : pastilles (écart maximal par canal : médiane;95e;max)" "$($r['pastilles_ecart_max_par_canal']) ; bande $($r['bande_ecart_max_par_canal'])" '± 8 par canal (BT.709 déclarée)' (Verdict ((Nombre $px[-1]) -le 8))
            Resultat "$($cas.N) : valeurs k" "justes $($r['images_k_juste']) / $n ; reculs $($r['k_reculs']) ; dernière $($r['k_dernier'])" 'toutes justes, aucun recul' (Verdict ((Nombre $r['k_reculs']) -eq 0 -and (Nombre $r['images_k_juste']) -ge ($n - 2)))
            $pix = @($r.Keys | Where-Object { $_ -like 'pixels_image_*' } | Sort-Object { [int]($_ -replace '\D', '') } | ForEach-Object { "$($_ -replace 'pixels_', '') : $($r[$_])" })
            if ($pix.Count) { Resultat "$($cas.N) : au pixel près, contre l'image écrite" "$($pix -join ' / ') ; meilleur décalage $($r['pixels_decalage_meilleur'])" 'décalage 0,0 ; écart moyen faible (codage avec pertes)' (OuiNon ("$($r['pixels_decalage_meilleur'])" -like '0,0*')) }
            Resultat "$($cas.N) : temps" "WriteSample $(Stats $r['writesample_ms'])$(if ($r.ContainsKey('conversion_nv12_ms')) { " ; conversion RGBA → NV12 $(Stats $r['conversion_nv12_ms'])" }) ; fichier $(Mo $r['taille_octets'])" 'à lire' 'à lire'
            $gardes = @("$($r['relu_png'])" -split ' ; ' | Where-Object { $_ -and (Test-Path -LiteralPath $_) } | ForEach-Object { Garder-Fichier $_ "s2-6-$($cas.K)-$(Split-Path -Leaf $_)" })
            Note "$($cas.N) : images relues gardées : $(if ($gardes.Count) { $gardes -join ', ' } else { "aucune ($($r['relu_png']))" })"
            if ($cas.Edge) {
                Note "$($cas.N) gardé : $(Garder-Fichier $f "s2-6-justesse-$($cas.K).mp4")"
                $tem = if ($r['temoin_dans_l_image']) { "$($r['temoin_dans_l_image'])" } else { '0,0' }
                $e = Lire-Dans-Edge $f 'video' $tem
                $ec = Ecart-Couleur $e['pastille'] @(200, 60, 60)
                $ta = "$($r['taille_ecrite'])"
                Resultat "S2 (9) $($cas.N) dans Edge : duration, videoWidth × videoHeight" "$($e['duree_s']) s ; $($e['taille'])$(foreach ($k in 'erreur_media', 'play_refuse', 'erreur', 'delai') { if ($e.ContainsKey($k)) { " ; $k $($e[$k])" } })" "10 s, $ta" (Verdict ([math]::Abs((Nombre $e['duree_s']) - 10) -le 0.2 -and "$($e['taille'])" -eq $ta))
                Resultat "S2 (9) $($cas.N) dans Edge : première image peinte" "$($e['premiere_image_ms']) ms après le chargement ; première pastille lue $($e['pastille']) (écart $ec) ; images présentées en 2 s $($e['images_presentees_2s'])" 'peinte, pastille à ± 8' (Verdict ($e.ContainsKey('premiere_image_ms') -and $ec -le 8))
            }
        }
    }
    catch { Erreur-Sonde $_ }
    finally { Fermer-Temoin $t }
}
Terminer-Sonde
