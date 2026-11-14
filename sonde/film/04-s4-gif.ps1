# S4 (contrat 0.7, §1.4) : le GIF par la crate gif 0.14.2 (et color_quant), au fil de l'eau,
# sur des suites de 10 s du témoin, en 600 × 450 et 1024 × 768 :
# 1. cas ordinaire (simple), fixe, pire cas (plein) ;
# 2. règle du contrat (rectangle changé, pixels inchangés transparents, palette locale) et
#    image entière ;
# 3. temps par image en quatre postes (différence, apprentissage de la palette, recherche de
#    l'indice de chaque pixel, écriture LZW), temps total, temps restant après la dernière
#    image (file bornée), taille, pic de mémoire privée ; à 10, 12,5 et 15 images par
#    seconde, vitesses 10, 20 et 30 (une sélection de la grille, dite ci-dessous) ;
# 4. justesse relue par System.Drawing : images, somme des délais (10,00 s ± 0,05), boucle
#    infinie, pastilles exactes (cas ordinaire), pixels opaques devenus transparents
#    (comptés, pire cas aussi) ; le fichier ouvert dans Edge (WebView2) ;
# 5. la quantification propre (NeuQuant sur un échantillon, table 5-6-5) sur le pire cas,
#    et l'écart de couleur à from_rgba_speed.
#   pwsh -NoProfile -File sonde/film/04-s4-gif.ps1

. "$PSScriptRoot/commun.ps1"

# Relit un GIF par System.Drawing (GDI+) : images, délais, boucle, et sur chaque image (200
# au plus) k, pastilles exactes et pixels transparents.
function Relire-Gif([string]$Chemin, [int]$Ox, [int]$Oy) {
    $o = [pscustomobject]@{ Images = 0; DelaisCs = 0; Boucle = '?'; KReculs = 0; KIllisibles = 0; KPremier = -1; KDernier = -1; PastillesMin = -1; Transparents = 0; Composees = '?'; Erreur = '' }
    $img = $null
    try {
        $img = [System.Drawing.Image]::FromFile($Chemin)
        $dim = [System.Drawing.Imaging.FrameDimension]::new($img.FrameDimensionsList[0])
        $o.Images = $img.GetFrameCount($dim)
        try { $v = $img.GetPropertyItem(0x5100).Value; $s = 0; for ($i = 0; $i -lt $v.Length; $i += 4) { $s += [BitConverter]::ToInt32($v, $i) }; $o.DelaisCs = $s } catch { $o.Erreur += "délais illisibles ; " }
        try { $o.Boucle = [BitConverter]::ToUInt16($img.GetPropertyItem(0x5101).Value, 0) } catch { $o.Boucle = 'absente' }
        $prec = -1; $pmin = [int]::MaxValue
        for ($i = 0; $i -lt [math]::Min($o.Images, 200); $i++) {
            [void]$img.SelectActiveFrame($dim, $i)
            $b = [System.Drawing.Bitmap]::new($img.Width, $img.Height, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
            try {
                $g = [System.Drawing.Graphics]::FromImage($b)
                try { $g.CompositingMode = [System.Drawing.Drawing2D.CompositingMode]::SourceCopy; $g.DrawImage($img, 0, 0, $img.Width, $img.Height) } finally { $g.Dispose() }
                $r = [System.Drawing.Rectangle]::new(0, 0, $b.Width, $b.Height)
                $dd = $b.LockBits($r, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
                $oct = [byte[]]::new($b.Width * $b.Height * 4)
                try { [System.Runtime.InteropServices.Marshal]::Copy($dd.Scan0, $oct, 0, $oct.Length) } finally { $b.UnlockBits($dd) }
            }
            finally { $b.Dispose() }
            $k = $script:T::LireK($oct, $img.Width, $Ox, $Oy)
            if ($k -lt 0) { $o.KIllisibles++ } else {
                if ($o.KPremier -lt 0) { $o.KPremier = $k }
                if ($prec -ge 0 -and $k -lt $prec) { $o.KReculs++ }
                $prec = $k; $o.KDernier = $k
            }
            $p = $script:T::PastillesExactes($oct, $img.Width, $Ox, $Oy)
            if ($i -eq 1) { $o.Composees = OuiNon ($p -ge 9000) }
            $pmin = [math]::Min($pmin, $p)
            $o.Transparents += $script:T::Transparents($oct)
        }
        $o.PastillesMin = $pmin
    }
    catch { $o.Erreur += "$($_.Exception.Message)" }
    finally { if ($img) { $img.Dispose() } }
    return $o
}

function Gif([string]$Taille, [string]$Genre, [double]$Fps, [int]$Vitesse, [string]$Mode = 'difference', [string]$Quant = 'crate', [string[]]$Plus = @(), [switch]$Garder, [switch]$Critere1024, [switch]$CritereReste) {
    $nom = "$Genre $Taille, $Fps i/s, vitesse $Vitesse, $Mode$(if ($Quant -ne 'crate') { ", quantification $Quant" })$(if ($Plus -contains '--ecarts') { ', écarts' })"
    $f = Join-Path $script:Sonde.Temp ("gif-$Genre-$Taille-$Fps-$Vitesse-$Mode-$Quant$(if ($Plus) { '-e' }).gif" -replace ',', '_')
    $a = @('--taille', $Taille, '--genre', $Genre, '--fps', "$Fps", '--vitesse', "$Vitesse", '--mode', $Mode, '--quant', $Quant, '--duree', '10', '--sortie', "`"$f`"") + $Plus
    if ($script:Fond -and $Taille -ne '600x450') { $a += @('--fond', "`"$script:Fond`"") }
    $r = Jouer-Exemple 'gif' $a 120
    if ($r['ok'] -ne '1') { NonMesurable $nom "$($r['erreur'])"; return }
    $par = Stat-Mediane $r['par_image_ms']
    $reste = Nombre $r['reste_apres_derniere_ms']
    $crit = if ($Critere1024) { 'médiane par image au plus 40 ms (temps réel à 15 i/s)' } elseif ($CritereReste) { 'fini moins de 2 s après la dernière image' } else { 'à lire' }
    $ver = if ($Critere1024) { Verdict ($par -le 40) } elseif ($CritereReste) { Verdict ($reste -lt 2000) } else { 'à lire' }
    Resultat "$nom : par image (travail du codeur)" "$(Stats $r['par_image_ms']) ; reste après la dernière image $(Ms $reste) ; total $($r['total_s']) s ; images prises $($r['images_prises']), écrites $($r['images_ecrites'])" $crit $ver
    Resultat "$nom : quatre postes" "différence $(Stats $r['difference_ms']) ; apprentissage $(Stats $r['apprentissage_ms']) ; recherche par pixel $(Stats $r['recherche_ms']) ; écriture LZW $(Stats $r['ecriture_ms']) ; aire écrite (pixels, médiane;95e;max) $($r['aire_px'])" 'à lire' 'à lire'
    Resultat "$nom : file, taille, mémoire" "prise bloquée $($r['prise_bloquee_ms_total']) ms en tout ; images de la prise en retard $($r['prise_images_en_retard']) ; fichier $(Mo $r['taille_octets']) ; mémoire privée $($r['prive_repos_mo']) → pic $($r['prive_pic_mo']) Mo" 'à lire' 'à lire'
    $ot = Nombre $r['opaques_devenus_transparents']
    Resultat "$nom : pixels opaques devenus transparents (compté au codage)" "$ot$(if ($r.ContainsKey('identique_a_la_crate')) { " ; quantification refaite identique à from_rgba_speed : $(OuiNon ($r['identique_a_la_crate'] -eq '1'))" })" '0' (Verdict ($ot -eq 0))
    foreach ($k in 'ecart_source', 'ecart_source_from_rgba_speed', 'ecart_table_contre_from_rgba_speed') { if ($r.ContainsKey($k)) { Resultat "$nom : $k (écart de couleur par canal)" $r[$k] 'à lire' 'à lire' } }
    if ($Plus -contains '--ecarts') { return }
    $l, $h = $Taille -split 'x' | ForEach-Object { [int]$_ }
    $ox = [int][math]::Max(0, [math]::Floor(($l - 600) / 2)); $oy = [int][math]::Max(0, [math]::Floor(($h - 450) / 2))
    $g = Relire-Gif $f $ox $oy
    $attendues = [int][math]::Round(10 * $Fps)
    $okJ = $g.Images -gt 0 -and [math]::Abs($g.DelaisCs - 1000) -le 5 -and "$($g.Boucle)" -eq '0' -and $g.KReculs -eq 0
    if ($Genre -eq 'simple') { $okJ = $okJ -and $g.PastillesMin -ge 9216 }
    Resultat "$nom : relu par System.Drawing" "$($g.Images) images (créneaux $attendues) ; délais $($g.DelaisCs) cs ; boucle $($g.Boucle) (0 : infinie) ; k $($g.KPremier) à $($g.KDernier), reculs $($g.KReculs), illisibles $($g.KIllisibles) ; pastilles exactes au moins $($g.PastillesMin) / 9216 ; images composées par GDI+ $($g.Composees) ; pixels transparents lus $($g.Transparents)$(if ($g.Erreur) { " ; $($g.Erreur)" })" "délais 1000 cs ± 5, boucle 0, aucun recul$(if ($Genre -eq 'simple') { ', pastilles exactes' })" (Verdict $okJ)
    if ($Garder) {
        Note "$nom gardé : $(Garder-Fichier $f ("s4-" + (Split-Path -Leaf $f)))"
        if ($Taille -eq '600x450') {
            $e = Lire-Dans-Edge $f 'gif'
            $ec = Ecart-Couleur $e['pastille'] @(200, 60, 60)
            Resultat "$nom : ouvert dans Edge (WebView2)" "taille $($e['taille']) ; première pastille $($e['pastille']) (écart $ec)$(foreach ($k in 'erreur_decode', 'erreur', 'delai') { if ($e.ContainsKey($k)) { " ; $k $($e[$k])" } })" 'décodé, 600x450, pastille à ± 8' (Verdict ("$($e['taille'])" -eq '600x450' -and $ec -le 8))
        }
    }
}

if (Demarrer-Sonde -Nom '04-s4-gif' -Titre 'S4 : le GIF (gif 0.14.2, color_quant), au fil de l''eau') {
    try {
        $script:Fond = Fichier-Temp 'fond.png'
        try { Copie-Ecran $script:Fond } catch { $script:Fond = $null }
        Note 'Grille jouée (une sélection : la grille complète ferait 108 suites de 10 s en temps réel) : 15 i/s vitesse 10 sur les trois cas et les deux tailles ; vitesse 30 et 20 sur le simple et le plein ; image entière sur trois cas ; 10 et 12,5 i/s sur le 1024 simple et le 600 plein ; la quantification propre sur le plein.'

        Section 'S4 (1, 2, 3, 4) : 15 i/s, vitesse 10, règle du contrat'
        Gif '1024x768' 'simple' 15 10 -Garder -Critere1024
        Gif '600x450' 'simple' 15 10 -Garder
        Gif '600x450' 'plein' 15 10 -Garder -CritereReste
        Gif '1024x768' 'plein' 15 10
        Gif '600x450' 'fixe' 15 10
        Gif '1024x768' 'fixe' 15 10

        Section 'S4 (3) : vitesses 30 et 20'
        foreach ($t in '600x450', '1024x768') { foreach ($g in 'simple', 'plein') { Gif $t $g 15 30 } }
        Gif '1024x768' 'simple' 15 20
        Gif '600x450' 'plein' 15 20

        Section 'S4 (2) : image entière à chaque créneau (sans la règle)'
        Gif '600x450' 'simple' 15 10 'entier'
        Gif '1024x768' 'simple' 15 10 'entier'
        Gif '600x450' 'plein' 15 10 'entier'

        Section 'S4 (3) : 10 et 12,5 images par seconde'
        foreach ($fps in 10, 12.5) { Gif '1024x768' 'simple' $fps 10; Gif '600x450' 'plein' $fps 10 }

        Section 'S4 (5) : quantification propre (NeuQuant sur un échantillon, table 5-6-5, indice 255 transparent)'
        Gif '600x450' 'plein' 15 10 'difference' 'table' -Garder -CritereReste
        Gif '1024x768' 'plein' 15 10 'difference' 'table'
        Gif '600x450' 'plein' 15 10 'difference' 'table' @('--ecarts')
        Gif '600x450' 'plein' 15 10 'difference' 'crate' @('--ecarts')
    }
    catch { Erreur-Sonde $_ }
}
Terminer-Sonde
