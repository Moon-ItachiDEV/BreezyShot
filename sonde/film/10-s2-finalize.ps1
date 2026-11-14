# S2 points 7 et 13 (contrat 0.7, §1.2) : Finalize sur de longs films, un job par cas.
#   -Cas ordinaire   MP4 ordinaire : 10 s, 60 s, 5 min, 60 min
#   -Cas moov        MP4 avec MF_MPEG4SINK_MOOV_BEFORE_MDAT : les mêmes durées
#   -Cas fragmente   MP4 fragmenté (MFCreateFMPEG4MediaSink) : 5 min et 60 min
# Images de 600 × 450 (témoin simple) écrites aussi vite que le codeur les prend (pas en
# temps réel : 60 min de film ne tiennent pas dans un job sinon). Pour chaque durée : temps
# d'écriture, WriteSample, Finalize, taille, place disque en plus pendant Finalize (l'index
# au début réécrit le fichier), ordre des boîtes (moov avant mdat ?), puis le fichier fini
# relu par IMFSourceReader (durée, images, valeurs k).
# Si la vitesse mesurée sur 5 min annonce plus que le budget du job (SONDE_BUDGET_LONG_S,
# 1500 s par défaut) pour 60 min, la dernière durée passe à 20 min, et le résultat le dit.
#   pwsh -NoProfile -File sonde/film/10-s2-finalize.ps1 -Cas ordinaire

param([ValidateSet('ordinaire', 'moov', 'fragmente')][string]$Cas = 'ordinaire')

. "$PSScriptRoot/commun.ps1"

$titres = @{ ordinaire = 'MP4 ordinaire'; moov = 'MP4, index au début (MOOV_BEFORE_MDAT)'; fragmente = 'MP4 fragmenté' }
if (Demarrer-Sonde -Nom "10-s2-finalize-$Cas" -Titre "S2 points 7 et 13 : Finalize sur de longs films, $($titres[$Cas])") {
    try {
        $plus = switch ($Cas) { 'moov' { @('--moov-debut') } 'fragmente' { @('--fragmente') } default { @() } }
        $durees = if ($Cas -eq 'fragmente') { @(300, 3600) } else { @(10, 60, 300, 3600) }
        $budget = if ($env:SONDE_BUDGET_LONG_S) { Nombre $env:SONDE_BUDGET_LONG_S } else { 1500.0 }
        $vitesse = [double]::NaN
        $chrono = [System.Diagnostics.Stopwatch]::StartNew()
        Section "S2 (7) : Finalize, $($titres[$Cas])"
        foreach ($s in $durees) {
            $secondes = $s
            if ($s -eq 3600) {
                $reste = $budget - $chrono.Elapsed.TotalSeconds
                $estime = if ([double]::IsNaN($vitesse)) { [double]::PositiveInfinity } else { 3600 * 30 / $vitesse * 1.15 + 120 }
                if ($estime -gt $reste) {
                    $secondes = 1200
                    Note ("60 min ramenées à 20 min faute de temps : vitesse mesurée {0:0} images par seconde, 60 min estimées à {1:0} s pour {2:0} s restantes du budget du job." -f $vitesse, $estime, $reste)
                }
            }
            $f = Fichier-Temp "finalize-$Cas-$secondes.mp4"
            $delai = if ([double]::IsNaN($vitesse)) { 1800 } else { [math]::Min(3000, $secondes * 30 / $vitesse * 2 + 300) }
            $r = Jouer-Exemple 'mf' (@('--etape', 'finaliser', '--secondes', "$secondes", '--sortie', "`"$f`"") + $plus) $delai
            $nom = if ($secondes -ge 60) { "$($secondes / 60) min" } else { "$secondes s" }
            if ($r['ok'] -ne '1') { NonMesurable "$nom : écriture et Finalize" "$($r['erreur'])"; Remove-Item -LiteralPath $f -Force -ErrorAction SilentlyContinue; continue }
            $ec = Nombre $r['ecriture_s']
            if ($ec -gt 0) { $vitesse = (Nombre $r['images']) / $ec }
            Resultat "$nom : écriture ($($r['images']) images)" ("{0} s, {1:0} images par seconde ; WriteSample {2}" -f $r['ecriture_s'], ((Nombre $r['images']) / [math]::Max(0.001, $ec)), (Stats $r['writesample_ms'])) 'à lire' 'à lire'
            Resultat "$nom : Finalize" (Ms $r['finalize_ms']) 'à lire (le contrat le budgète à la fin du film)' 'à lire'
            Resultat "$nom : taille" "$(Mo $r['taille_octets']) ($(Mo $r['taille_avant_finalize']) avant Finalize)" 'à lire' 'à lire'
            Resultat "$nom : place disque pendant Finalize" "$(Mo $r['disque_en_plus_pendant_finalize']) en plus au pic ; $($r['disque_pendant_finalize'])" 'à lire (l''index au début réécrit le fichier : jusqu''à sa taille en plus)' 'à lire'
            $moovAttendu = $Cas -eq 'moov'
            Resultat "$nom : boîtes, moov avant mdat" "$($r['moov_avant_mdat']) ; boîtes $($r['boites'])" $(if ($moovAttendu) { '1 (demandé)' } else { 'à lire' }) $(if ($moovAttendu) { OuiNon ("$($r['moov_avant_mdat'])" -eq '1') } else { 'à lire' })
            $l = Jouer-Exemple 'mf' @('--etape', 'relire', '--fichier', "`"$f`"") ([math]::Max(120, $secondes / 4))
            if ($l['ok'] -ne '1') { NonMesurable "$nom : fichier fini relu" "$($l['erreur'])" }
            else {
                $dur = Nombre $l['relu_duree_s']
                $ok = "$($l['lisible'])" -eq '1' -and [math]::Abs($dur - $secondes) -le 0.5 -and (Nombre $l['relu_images']) -ge ($secondes * 30 - 2)
                Resultat "$nom : fichier fini relu (IMFSourceReader)" "lisible $($l['lisible']) ; durée $($l['relu_duree_s']) s ; images $($l['relu_images']) ; dernier horodatage $($l['relu_dernier_horodatage_s']) s ; k distincts $($l['relu_k_distincts'])$(if ($l.ContainsKey('lecture_refusee')) { " ; refus : $($l['lecture_refusee'])" })" "lisible, $secondes s à 0,5 s près, $($secondes * 30) images" (OuiNon $ok)
            }
            Remove-Item -LiteralPath $f -Force -ErrorAction SilentlyContinue
        }
        Resultat 'Durée de ce job de mesure' ("{0:0} s" -f $chrono.Elapsed.TotalSeconds) 'à lire' 'à lire'
        if ($Cas -eq 'fragmente') { Note 'Le processus tué au milieu (lisible jusqu''à la coupure) : 03-s2-chaine-mp4.ps1, S2 (13).' }
    }
    catch { Erreur-Sonde $_ }
}
Terminer-Sonde
