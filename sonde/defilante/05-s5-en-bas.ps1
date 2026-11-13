# S5 (contrat 0.6, §1.5) : savoir qu'on est en bas. Sur Edge, l'Explorateur et le témoin
# WPF (et Chrome s'il est là) :
# - VerticalScrollPercent arrive-t-il à 100 exactement en bas (atteint à la molette) ;
# - vaut-il -1 (UIA_ScrollPatternNoScroll) pour une page courte ;
# - après un cran demandé en bas : combien d'images de suite restent identiques (lissage,
#   rebond) sur une seconde ;
# - durée du lissage d'un cran au milieu de la page (contre FIN_EN_BAS = 350 ms) ;
# - longue.html charge en défilant (image paresseuse, 40 lignes ajoutées près du bas) : la
#   position recule-t-elle (en pixels, lus sur la bande ; en pour cent, lus par UIA).
#   pwsh -NoProfile -File sonde/defilante/05-s5-en-bas.ps1

. "$PSScriptRoot/commun.ps1"

# Un cran au centre de la zone, si rien ne couvre la cible (remise au premier plan d'abord) ;
# rend '' ou la raison du saut (écrite dans le journal par l'appelant).
$script:Raison = ''
function Cran($C) {
    $d = Degager $C
    if ($d) { $script:Raison = "cran sauté : sous le point $d"; return $false }
    $pt = Centre $C.Zone
    $script:N::Poser($pt[0], $pt[1])
    return ($script:N::Molette(-1, $script:N::MARQUE) -eq 1)
}

if (Demarrer-Sonde -Nom 's5' -Titre 'S5 : savoir qu''on est en bas' -QuotaMo 1) {
    try {
        foreach ($g in 'edge', 'chrome', 'explorateur', 'wpf') {
            Section "S5 : $g"
            $c = $null
            try {
                $c = Ouvrir-Cible $g
                if ($c.Info) { Note $c.Info }
                $d = Degager $c
                if ($d) { Noter-Obstacles $c; throw "cible couverte par $d" }
                # Même recherche que S1 et S4 (commun.ps1), avec l'attente que Chromium demande.
                $c.Uia = Uia-Defilable $c
                if ($null -eq $c.Uia) { NonMesurable "$g : ScrollPattern" 'aucun élément défilable trouvé par UI Automation en 2,5 s (sous le centre ni depuis la fenêtre)' }
                else { Resultat "$g : ScrollPattern" "trouvé ($($c.Uia.Par)) en $(Ms $c.Uia.Ms) ; vue $('{0:0.#}' -f $c.Uia.Motif.Current.VerticalViewSize) %" 'trouvé' 'passe' }
                # Lissage au milieu : 10 crans.
                if ($null -ne $c.Uia) { try { $c.Uia.Motif.SetScrollPercent([System.Windows.Automation.ScrollPattern]::NoScroll, 30.0) } catch { } }
                $img = (Attendre-Stable $c 1500).Image
                $lis = [System.Collections.Generic.List[double]]::new()
                for ($i = 0; $i -lt 10; $i++) {
                    $chrono = [System.Diagnostics.Stopwatch]::StartNew()
                    if (-not (Cran $c)) { Note $script:Raison; break }
                    $st = Attendre-Stable $c 1500 $chrono
                    $m = Mouvement $c $img $st.Image
                    if (-not [double]::IsNaN($m.Px) -and $m.Px -gt 0) { $lis.Add($st.Ms) }
                    $img = $st.Image
                }
                if ($lis.Count -gt 0) {
                    $mx = ($lis | Measure-Object -Maximum).Maximum
                    Resultat 'lissage d''un cran (molette), du cran à l''image stable' "médiane $(Ms (Mediane $lis.ToArray())), max $(Ms $mx) sur $($lis.Count) crans" 'max ≤ 350 ms : FIN_EN_BAS gardé' (Verdict ($mx -le 350))
                }
                else { NonMesurable 'lissage d''un cran' 'la molette n''a pas fait bouger la page' }

                # En bas, à la molette.
                if ($null -ne $c.Uia) { try { $c.Uia.Motif.SetScrollPercent([System.Windows.Automation.ScrollPattern]::NoScroll, 97.0) } catch { } }
                $img = (Attendre-Stable $c 1500).Image
                $immobiles = 0; $crans = 0
                while ($immobiles -lt 2 -and $crans -lt 400) {
                    if (-not (Cran $c)) { Note $script:Raison; break }
                    $crans++
                    $st = Attendre-Stable $c 1000
                    $m = Mouvement $c $img $st.Image
                    if (-not [double]::IsNaN($m.Px) -and [math]::Abs($m.Px) -lt 0.5) { $immobiles++ } else { $immobiles = 0 }
                    $img = $st.Image
                }
                $pc = Uia-Pourcent $c.Uia
                if ($immobiles -lt 2) { NonMesurable 'bas atteint à la molette' "pas d'image immobile après $crans crans" }
                elseif ([double]::IsNaN($pc)) { NonMesurable 'VerticalScrollPercent en bas' 'illisible (pas de ScrollPattern)' '≥ 99,9 (D15)' }
                else { Resultat 'VerticalScrollPercent en bas (atteint à la molette)' ('{0:R}' -f $pc) '≥ 99,9 (D15) ; 100 exactement attendu' (Verdict ($pc -ge 99.9)) }
                # Un cran de plus : une seconde d'images.
                $ref = Prendre $c.Zone
                $chrono = [System.Diagnostics.Stopwatch]::StartNew()
                [void](Cran $c)
                $memes = 0; $autres = 0; $suite = 0; $plusLongue = 0
                while ($chrono.Elapsed.TotalMilliseconds -lt 1000) {
                    $x = Prendre $c.Zone
                    $d = $script:N::Differences($ref, $x, $c.Zone[2], 0, 0, $c.Zone[2] - 24, $c.Zone[3])
                    if ($d -eq 0) { $memes++; $suite++; $plusLongue = [math]::Max($plusLongue, $suite) } else { $autres++; $suite = 0 }
                    Start-Sleep -Milliseconds 15
                }
                Resultat 'cran demandé en bas : images pendant 1 s' "$memes identiques à l'image d'avant, $autres différentes (plus longue suite identique : $plusLongue)" 'à lire (différentes : rebond, barre, animation)' 'à lire'
            }
            catch { NonMesurable "$g" "$($_.Exception.Message) (ligne $($_.InvocationInfo.ScriptLineNumber))" }
            finally { Fermer-Cible $c }

            # Page courte.
            $c = $null
            try {
                $c = Ouvrir-Cible $g -Courte
                $u = Uia-Defilable $c 2500 -Tous
                if ($null -eq $u) { NonMesurable 'page courte : VerticalScrollPercent' 'aucun ScrollPattern (sous le centre ni depuis la fenêtre, 2,5 s)' }
                else {
                    $v = $u.Motif.Current.VerticalScrollPercent
                    Resultat 'page courte : VerticalScrollPercent' "$v (VerticallyScrollable $($u.Motif.Current.VerticallyScrollable))" '-1 (UIA_ScrollPatternNoScroll)' (Verdict ($v -eq -1))
                }
            }
            catch { NonMesurable "$g page courte" $_.Exception.Message }
            finally { Fermer-Cible $c }
        }

        # Chargement en défilant : longue.html dans Edge.
        Section 'S5 : page qui charge en défilant (Edge, longue.html)'
        $c = $null
        try {
            $c = Ouvrir-Cible 'edge'
            if ($c.Info) { Note $c.Info }
            $c.Uia = Uia-Defilable $c
            if ($null -ne $c.Uia) { try { $c.Uia.Motif.SetScrollPercent([System.Windows.Automation.ScrollPattern]::NoScroll, 40.0) } catch { } }
            $st = Attendre-Stable $c 1500
            $h0 = Hauteur-Page $c
            $p0 = Position-Bande $c $st.Image; $u0 = Uia-Pourcent $c.Uia
            $reculPx = 0; $reculPc = 0; $croissances = 0; $immobiles = 0; $crans = 0; $hPrec = $h0
            $chrono = [System.Diagnostics.Stopwatch]::StartNew()
            while ($immobiles -lt 3 -and $crans -lt 300 -and $chrono.Elapsed.TotalSeconds -lt 90) {
                if (-not (Cran $c)) { Note $script:Raison; break }
                $crans++
                $avant = $st.Image
                $st = Attendre-Stable $c 800
                $p = Position-Bande $c $st.Image; $u = Uia-Pourcent $c.Uia; $h = Hauteur-Page $c
                if ($h -gt $hPrec) { $croissances++ }
                if (-not [double]::IsNaN($p) -and -not [double]::IsNaN($p0)) {
                    if ($p -lt $p0 - 0.5) { $reculPx++ }
                    if ([math]::Abs($p - $p0) -lt 0.5) { $immobiles++ } else { $immobiles = 0 }
                }
                elseif (-not $c.Bande) {
                    # Sans bande : le mouvement lu sur les images.
                    $mv = Mouvement $c $avant $st.Image
                    if (-not [double]::IsNaN($mv.Px) -and $mv.Px -lt -0.5) { $reculPx++ }
                    if (-not [double]::IsNaN($mv.Px) -and [math]::Abs($mv.Px) -lt 0.5) { $immobiles++ } else { $immobiles = 0 }
                }
                if (-not [double]::IsNaN($u) -and -not [double]::IsNaN($u0) -and $u -lt $u0 - 0.01) { $reculPc++ }
                $p0 = $p; $u0 = $u; $hPrec = $h
            }
            Resultat 'hauteur du document' "$h0 → $hPrec px CSS ($croissances croissance(s) vues en $crans crans)" 'à lire (image paresseuse, 40 lignes ajoutées)' 'à lire'
            Resultat 'la position recule-t-elle (pixels, bande)' "$reculPx fois" '0 attendu (les ajouts sont sous la vue)' (Verdict ($reculPx -eq 0))
            if ($null -eq $c.Uia) { NonMesurable 'le pourcentage recule-t-il (UI Automation)' 'pas de ScrollPattern sur Edge' }
            else { Resultat 'le pourcentage recule-t-il (UI Automation)' "$reculPc fois" 'à lire (un recul ferait croire D15 à une remontée)' 'à lire' }
            Resultat 'fin' "$(if ($immobiles -ge 3) { 'bas atteint' } else { 'arrêté avant le bas' }) ; UI Automation $('{0:0.###}' -f (Uia-Pourcent $c.Uia)) %" '≥ 99,9 en bas' (Verdict ($immobiles -ge 3 -and (Uia-Pourcent $c.Uia) -ge 99.9))
        }
        catch { NonMesurable 'Edge, chargement en défilant' $_.Exception.Message }
        finally { Fermer-Cible $c }

        Section 'Critère (rappel)'
        Note 'Avec un ScrollPattern, « en bas » = 99,9 % ou plus et une image immobile ; sans lui, deux images immobiles de suite après deux commandes envoyées (D9). FIN_EN_BAS (350 ms) gardé si aucun lissage ne dure plus.'
    }
    catch { Erreur-Sonde $_ }
}
Terminer-Sonde
