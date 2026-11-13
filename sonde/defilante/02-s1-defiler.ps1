# S1 (contrat 0.6, §1.1) : faire défiler la fenêtre d'une autre app. Sur chaque cible
# (témoins WPF, WinForms, « molette seule », Edge, Chrome, Explorateur, Firefox et Electron
# s'ils sont là), chaque moyen, 30 commandes :
#   1a UI Automation SetScrollPercent (pas d'une demi-vue, calculé avec VerticalViewSize)
#   1b UI Automation Scroll(SmallIncrement)
#   2  molette SendInput (un cran, pointeur sur la zone, marque dans dwExtraInfo)
#   3  WM_MOUSEWHEEL posté à la fenêtre sous le point (pointeur ailleurs)
#   4a WM_VSCROLL SB_LINEDOWN, 4b SB_PAGEDOWN, à cette même fenêtre
#   5a touche Page suivante, 5b touche Bas (SendInput, cible au premier plan)
# Mesure : la page a-t-elle bougé (décalage vrai lu sur la bande des témoins et de
# longue.html, sinon décalage des lignes de deux images ; UI Automation quand elle existe),
# pixels par commande, temps jusqu'à une image stable (lissage), une sentinelle (deux
# fenêtres, à côté et sous la cible) a-t-elle bougé ; puis 10 commandes cible en arrière-plan
# (moyens 1a, 2, 3, 4a), et la molette avec « Faire défiler les fenêtres inactives » coupé
# (réglage remis à la fin). Avec Edge : UI Automation dès la première demande ?
# Pendant les moyens 1, 3, 4 et 5, le pointeur est posé sur la sentinelle (un message mal
# routé vers la fenêtre sous le pointeur s'y verrait).
#   pwsh -NoProfile -File sonde/defilante/02-s1-defiler.ps1

. "$PSScriptRoot/commun.ps1"

$N_COMMANDES = 30
$N_VARIANTE = 10
$MOYENS = [ordered]@{
    '1a' = 'UI Automation SetScrollPercent (demi-vue)'
    '1b' = 'UI Automation Scroll(SmallIncrement)'
    '2'  = 'molette SendInput'
    '3'  = 'WM_MOUSEWHEEL posté'
    '4a' = 'WM_VSCROLL SB_LINEDOWN'
    '4b' = 'WM_VSCROLL SB_PAGEDOWN'
    '5a' = 'touche Page suivante'
    '5b' = 'touche Bas'
}

# Envoie une commande ; rend '' si elle est partie, sinon « saute : … » ou « erreur : … ».
function Commande($C, [string]$Moyen) {
    $pt = Centre $C.Zone
    switch -Regex ($Moyen) {
        '^1' {
            if ($null -eq $C.Uia) { return 'erreur : pas de ScrollPattern' }
            try {
                if ($Moyen -eq '1a') {
                    $vs = [double]$C.Uia.Motif.Current.VerticalViewSize
                    $p = [double]$C.Uia.Motif.Current.VerticalScrollPercent
                    if ($vs -ge 100 -or $vs -le 0) { return "erreur : VerticalViewSize $vs" }
                    $nouveau = [math]::Min(100.0, [math]::Max(0.0, $p) + 50.0 * $vs / (100.0 - $vs))
                    $C.Uia.Motif.SetScrollPercent([System.Windows.Automation.ScrollPattern]::NoScroll, $nouveau)
                }
                else { $C.Uia.Motif.Scroll([System.Windows.Automation.ScrollAmount]::NoAmount, [System.Windows.Automation.ScrollAmount]::SmallIncrement) }
                return ''
            }
            catch { return "erreur : $($_.Exception.GetType().Name) $($_.Exception.Message)" }
        }
        '^2$' {
            $racine = $script:N::RacineSousPoint($pt[0], $pt[1])
            if ($racine -ne $C.Racine) { return "saute : sous le point $($script:N::Decrire($racine))" }
            $script:N::Poser($pt[0], $pt[1])
            if ($script:N::Molette(-1, $script:N::MARQUE) -ne 1) { return 'erreur : SendInput refusé' }
            return ''
        }
        '^3$' {
            $h = $script:N::SousPoint($pt[0], $pt[1])
            $e = $script:N::PosterMolette($h, -1, $pt[0], $pt[1])
            if ($e -ne 0) { return "erreur : PostMessage $e" }
            return ''
        }
        '^4' {
            $h = $script:N::SousPoint($pt[0], $pt[1])
            $e = $script:N::PosterDefilement($h, $(if ($Moyen -eq '4a') { 1 } else { 3 }))
            if ($e -ne 0) { return "erreur : PostMessage $e" }
            return ''
        }
        '^5' {
            if (-not $script:N::PremierPlan($C.Racine)) { return "saute : premier plan refusé ($($script:N::Decrire($script:N::GetForegroundWindow())))" }
            $vk = if ($Moyen -eq '5a') { 0x22 } else { 0x28 }
            if ($script:N::Touche($vk, $true) -ne 2) { return 'erreur : SendInput refusé' }
            return ''
        }
    }
    return "erreur : moyen $Moyen inconnu"
}

# Remonte en haut (UI Automation, sinon un moyen propre à la cible). Rend vrai si la
# position lue (bande ou UI Automation) est en haut.
function Ramener-EnHaut($C) {
    $pt = Centre $C.Zone
    if ($null -ne $C.Uia) {
        try { $C.Uia.Motif.SetScrollPercent([System.Windows.Automation.ScrollPattern]::NoScroll, 0.0) } catch { }
    }
    else {
        switch ($C.Genre) {
            'winforms' { [void]$script:N::PosterDefilement($C.Defile, 6) }
            'molette' { for ($i = 0; $i -lt 150; $i++) { [void]$script:N::PosterMolette($C.Defile, 1, $pt[0], $pt[1]) } }
            default {
                if ($script:N::RacineSousPoint($pt[0], $pt[1]) -eq $C.Racine) {
                    $script:N::Poser($pt[0], $pt[1])
                    for ($i = 0; $i -lt 60; $i++) { [void]$script:N::Molette(5, $script:N::MARQUE); Start-Sleep -Milliseconds 5 }
                }
            }
        }
    }
    $st = Attendre-Stable $C 1500
    $p = Position-Bande $C $st.Image
    if (-not [double]::IsNaN($p)) { return ($p -le 0.5) }
    $u = Uia-Pourcent $C.Uia
    if (-not [double]::IsNaN($u)) { return ($u -le 0.1) }
    return $true
}

# Une série de commandes ; rend ses comptes.
function Serie($C, $S, [string]$Moyen, [int]$N, [switch]$ArrierePlan) {
    $r = [pscustomobject]@{ Moyen = $Moyen; Envoyees = 0; Bouge = 0; Immobile = 0; Recul = 0; Bas = 0; Sautees = 0
        Erreurs = [System.Collections.Generic.List[string]]::new(); Px = [System.Collections.Generic.List[double]]::new()
        Lissage = [System.Collections.Generic.List[double]]::new(); Instables = 0; Sentinelle = ''; UiaSuit = 'sans UI Automation'; Par = ''; PremierPlan = '' }
    if ($Moyen -like '1*' -and $null -eq $C.Uia) { $r.Erreurs.Add('pas de ScrollPattern'); return $r }
    $haut = Ramener-EnHaut $C
    if (-not $haut) { $r.Erreurs.Add('remise en haut non vérifiée') }
    $sp = Centre $S.Zone
    if ($ArrierePlan) {
        if (-not $script:N::PremierPlan($S.Racine)) { $r.Erreurs.Add('arrière-plan : la sentinelle n''a pas pris le premier plan'); return $r }
    }
    elseif ($Moyen -ne '1a' -and $Moyen -ne '1b') { [void]$script:N::PremierPlan($C.Racine) }
    if ($Moyen -ne '2') { $script:N::Poser($sp[0], $sp[1]) }
    $s0 = Etat-Sentinelle $S
    $u0 = Uia-Pourcent $C.Uia
    $img = (Attendre-Stable $C 1000).Image
    $essais = 0
    while ($r.Envoyees -lt $N -and $essais -lt 3 * $N) {
        $essais++
        if ($ArrierePlan -and $script:N::GetForegroundWindow() -eq $C.Racine) { $r.PremierPlan = 'la cible a repris le premier plan' }
        $chrono = [System.Diagnostics.Stopwatch]::StartNew()
        $e = Commande $C $Moyen
        if ($e -like 'saute*') { $r.Sautees++; $r.Envoyees++; $r.Erreurs.Add($e); continue }
        if ($e) { $r.Erreurs.Add($e); $r.Envoyees++; continue }
        $st = Attendre-Stable $C 900 $chrono
        if (-not $st.Stable) { $r.Instables++ }
        $m = Mouvement $C $img $st.Image
        $r.Par = $m.Par
        if (-not [double]::IsNaN($m.Px) -and $m.Px -gt 0.5) { $r.Bouge++; $r.Px.Add($m.Px); $r.Lissage.Add($st.Ms); $r.Envoyees++; $img = $st.Image }
        elseif (-not [double]::IsNaN($m.Px) -and $m.Px -lt -0.5) { $r.Recul++; $r.Envoyees++; $img = $st.Image }
        elseif (En-Bas $C $st.Image) {
            # En bas : la commande ne compte pas ; on remonte et on continue.
            $r.Bas++
            [void](Ramener-EnHaut $C)
            if ($Moyen -ne '2') { $script:N::Poser($sp[0], $sp[1]) }
            if ($ArrierePlan) { [void]$script:N::PremierPlan($S.Racine) }
            $img = (Attendre-Stable $C 1000).Image
        }
        else { $r.Immobile++; $r.Envoyees++; $img = $st.Image }
    }
    $s1 = Etat-Sentinelle $S
    $r.Sentinelle = if ($s0 -eq $s1) { 'immobile' } else { "BOUGÉE ($s0 → $s1)" }
    if ($null -ne $C.Uia) {
        $u1 = Uia-Pourcent $C.Uia
        $r.UiaSuit = "UI Automation $('{0:0.##}' -f $u0) → $('{0:0.##}' -f $u1) %"
    }
    return $r
}

function Resume-Serie($R, [int]$N) {
    if ($R.Envoyees -eq 0) { return "aucune commande partie ($(($R.Erreurs | Select-Object -Unique) -join ' ; '))" }
    $px = $R.Px.ToArray(); $li = $R.Lissage.ToArray()
    $t = "bougé $($R.Bouge)/$($R.Envoyees)"
    if ($px.Count -gt 0) { $t += " ; $('{0:0.#}' -f (Mediane $px)) px par commande ($('{0:0.#}' -f ($px | Measure-Object -Minimum).Minimum) à $('{0:0.#}' -f ($px | Measure-Object -Maximum).Maximum), $($R.Par))" }
    if ($li.Count -gt 0) { $t += " ; stable après $(Ms (Mediane $li)) (max $(Ms (($li | Measure-Object -Maximum).Maximum)))" }
    if ($R.Immobile) { $t += " ; immobile $($R.Immobile)" }
    if ($R.Recul) { $t += " ; à l'envers $($R.Recul)" }
    if ($R.Sautees) { $t += " ; sautées (garde) $($R.Sautees)" }
    if ($R.Bas) { $t += " ; bas atteint $($R.Bas) fois" }
    if ($R.Instables) { $t += " ; jamais stable en 900 ms : $($R.Instables)" }
    $t += " ; sentinelle $($R.Sentinelle) ; $($R.UiaSuit)"
    $err = @($R.Erreurs | Where-Object { $_ -notlike 'saute*' } | Select-Object -Unique)
    if ($err.Count -gt 0) { $t += " ; erreurs : $($err -join ' ; ')" }
    if ($R.PremierPlan) { $t += " ; $($R.PremierPlan)" }
    return $t
}

function Passe($R, [int]$N) {
    $seuil = [math]::Ceiling($N * 29 / 30)
    return ($R.Envoyees -ge $N -and $R.Bouge -ge $seuil -and $R.Sentinelle -eq 'immobile')
}

if (Demarrer-Sonde -Nom 's1' -Titre 'S1 : faire défiler la fenêtre d''une autre app' -QuotaMo 2) {
    $routageInitial = -99
    try {
        $sent = Ouvrir-Sentinelle @(712, 40, 300, 600)
        $routageInitial = $script:N::LireRoutageMolette()
        $ri = $routageInitial
        $NN = $script:N
        $script:Sonde.Restaurer.Add({ if ($ri -ge 0) { [void]$NN::FixerRoutageMolette($ri) } }.GetNewClosure())

        # Edge, UI Automation à la première demande (arbre d'accessibilité de Chromium).
        Section 'S1 : Edge et Chrome, UI Automation dès la première demande'
        foreach ($g in 'edge', 'chrome') {
            $c = $null
            try {
                $c = Ouvrir-Cible $g
                $chrono = [System.Diagnostics.Stopwatch]::StartNew()
                $u = Uia-Defilable $c
                $premier = $chrono.Elapsed.TotalMilliseconds
                if ($null -ne $u) { Resultat "$g : ScrollPattern à la première demande" "trouvé en $(Ms $premier)" 'trouvé dès la première demande' 'passe' }
                else {
                    $ok = Attendre -DelaiS 8 -PasMs 250 -Condition { $null -ne (Uia-Defilable $c) }
                    $t = $chrono.Elapsed.TotalMilliseconds
                    Resultat "$g : ScrollPattern à la première demande" $(if ($ok) { "absent à la première demande ($(Ms $premier)), trouvé après $(Ms $t)" } else { "absent pendant $(Ms $t)" }) 'trouvé dès la première demande' 'échoue'
                }
            }
            catch { NonMesurable "$g : première demande" $_.Exception.Message }
            finally { Fermer-Cible $c }
        }

        $bilan = [ordered]@{}
        $cibles = 'wpf', 'winforms', 'molette', 'edge', 'chrome', 'explorateur', 'firefox', 'electron'
        foreach ($g in $cibles) {
            Section "S1 : cible $g"
            $c = $null
            try { $c = Ouvrir-Cible $g } catch { NonMesurable "cible $g" $_.Exception.Message; $bilan[$g] = 'absente'; continue }
            try {
                if ($c.Info) { Note $c.Info }
                $c.Uia = Uia-Defilable $c
                Note "zone $(Rect-Texte $c.Zone) ; fenêtre sous le point : $($script:N::Decrire($script:N::SousPoint((Centre $c.Zone)[0], (Centre $c.Zone)[1]))) ; ScrollPattern : $(if ($null -ne $c.Uia) { "oui ($($c.Uia.Element.Current.ClassName), vue $('{0:0.#}' -f $c.Uia.Motif.Current.VerticalViewSize) %)" } else { 'non' }) ; décalage lu $(if ($c.Bande) { 'sur la bande' } else { 'sur les images' })"
                $retenus = [System.Collections.Generic.List[string]]::new()
                foreach ($m in $MOYENS.Keys) {
                    $r = Serie $c $sent $m $N_COMMANDES
                    $ok = Passe $r $N_COMMANDES
                    if ($ok) { $retenus.Add($m) }
                    Resultat "$g · $m $($MOYENS[$m])" (Resume-Serie $r $N_COMMANDES) "≥ 29/30, sentinelle immobile" (Verdict $ok)
                }
                foreach ($m in '1a', '2', '3', '4a') {
                    $r = Serie $c $sent $m $N_VARIANTE -ArrierePlan
                    Resultat "$g · $m en arrière-plan" (Resume-Serie $r $N_VARIANTE) "≥ 10/10, sentinelle immobile" (Verdict (Passe $r $N_VARIANTE))
                }
                # « Faire défiler les fenêtres inactives » coupé.
                if ($routageInitial -lt 0) { NonMesurable "$g · 2 survol coupé" "routage illisible ($routageInitial)" }
                else {
                    $lu = $script:N::FixerRoutageMolette(0)
                    if ($lu -ne 0) { NonMesurable "$g · 2 survol coupé" "SPI_SETMOUSEWHEELROUTING sans effet (relu $lu)" }
                    else {
                        $r = Serie $c $sent '2' $N_VARIANTE
                        Resultat "$g · 2 survol coupé, cible au premier plan" (Resume-Serie $r $N_VARIANTE) '≥ 10/10' (Verdict (Passe $r $N_VARIANTE))
                        $r = Serie $c $sent '2' $N_VARIANTE -ArrierePlan
                        Resultat "$g · 2 survol coupé, cible en arrière-plan" (Resume-Serie $r $N_VARIANTE) 'à lire (attendu : la molette va au premier plan, la sentinelle bouge)' 'à lire'
                    }
                    [void]$script:N::FixerRoutageMolette($routageInitial)
                }
                $bilan[$g] = if ($retenus.Count -gt 0) { $retenus -join ', ' } else { 'aucun' }
            }
            catch { NonMesurable "cible $g (suite)" "$($_.Exception.Message) (ligne $($_.InvocationInfo.ScriptLineNumber))" }
            finally { Fermer-Cible $c }
        }

        Section 'S1 : moyens qui passent, par cible'
        foreach ($k in $bilan.Keys) { Resultat $k $bilan[$k] 'ordre de D7 : les moyens qui passent ; 5 seulement si rien d''autre sur une cible nommée' 'à lire' }
        Note 'Critère : l''ordre des moyens de l''app (D7) est celui des moyens qui font bouger la page dans au moins 29 commandes sur 30 sans jamais bouger la sentinelle. Le moyen 5 n''est retenu que si aucun autre ne marche sur un navigateur ou l''Explorateur. Recommandation de départ, à confirmer : 1 puis 2, jamais 5.'
        Note 'UI Automation est lue ici par le client managé de .NET (System.Windows.Automation), pas par le client COM IUIAutomation que l''app utiliserait : un écart reste possible, surtout pour Chromium.'
    }
    catch { Erreur-Sonde $_ }
    finally { if ($routageInitial -ge 0) { [void]$script:N::FixerRoutageMolette($routageInitial) } }
}
Terminer-Sonde
