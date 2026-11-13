# S4 (contrat 0.6, §1.4) : trouver « la page » sous un clic, calculé pendant que l'écran
# figé est montré (imité par une fenêtre native toujours au-dessus, d'un autre processus,
# écartée de la liste comme l'app écarte ses fenêtres). Pour les 8 premières fenêtres de la
# liste (comme gel::fenetres_visibles) :
#   1  UI Automation : ElementFromHandle puis FindAll(Descendants, IsScrollPatternAvailable),
#      éléments VerticallyScrollable, leur BoundingRectangle ;
#   2  fenêtres filles (EnumChildWindows) : la plus profonde avec WS_VSCROLL ou de classe
#      Chrome_RenderWidgetHostHWND.
# Mesure : temps pour les 8 (médiane, maximum, sur 3 tours ; le premier à part) ; rectangle
# trouvé contre le vrai (témoins : leur zone ; Edge : la zone de rendu, sans la barre de
# titre ; Explorateur : la vue SHELLDLL_DefView) ; faux candidats. Et ElementFromPoint
# pendant l'écran figé : sur quelle fenêtre tombe-t-il ?
#   pwsh -NoProfile -File sonde/defilante/04-s4-pages.ps1

. "$PSScriptRoot/commun.ps1"

function Ecart([int[]]$A, [int[]]$B) {
    return [math]::Max([math]::Max([math]::Abs($A[0] - $B[0]), [math]::Abs($A[1] - $B[1])),
        [math]::Max([math]::Abs($A[0] + $A[2] - $B[0] - $B[2]), [math]::Abs($A[1] + $A[3] - $B[1] - $B[3])))
}

function Moyen1([IntPtr[]]$Fenetres) {
    $AE = [System.Windows.Automation.AutomationElement]
    $SP = [System.Windows.Automation.ScrollPattern]
    $cond = [System.Windows.Automation.PropertyCondition]::new($AE::IsScrollPatternAvailableProperty, $true)
    $rects = [System.Collections.Generic.List[object]]::new()
    $chrono = [System.Diagnostics.Stopwatch]::StartNew()
    $erreurs = 0
    foreach ($h in $Fenetres) {
        try {
            $e = $AE::FromHandle($h)
            foreach ($x in @($e.FindAll([System.Windows.Automation.TreeScope]::Descendants, $cond))) {
                $m = $null
                if ($x.TryGetCurrentPattern($SP::Pattern, [ref]$m) -and $m.Current.VerticallyScrollable) {
                    $b = $x.Current.BoundingRectangle
                    if (-not $b.IsEmpty) { $rects.Add([pscustomobject]@{ R = [int[]]@([int]$b.X, [int]$b.Y, [int]$b.Width, [int]$b.Height); F = $h; Quoi = $x.Current.ClassName }) }
                }
            }
        }
        catch { $erreurs++ }
    }
    return [pscustomobject]@{ Ms = $chrono.Elapsed.TotalMilliseconds; Rects = $rects; Erreurs = $erreurs }
}

function Moyen2([IntPtr[]]$Fenetres) {
    $rects = [System.Collections.Generic.List[object]]::new()
    $chrono = [System.Diagnostics.Stopwatch]::StartNew()
    foreach ($h in $Fenetres) {
        $meilleure = [IntPtr]::Zero; $prof = -1
        foreach ($f in $script:N::Filles($h)) {
            if (-not $script:N::IsWindowVisible($f)) { continue }
            $ok = (($script:N::Style($f) -band 0x00200000) -ne 0) -or ($script:N::Classe($f) -eq 'Chrome_RenderWidgetHostHWND')
            if (-not $ok) { continue }
            $p = $script:N::Profondeur($f, $h)
            if ($p -gt $prof) { $prof = $p; $meilleure = $f }
        }
        if ($meilleure -ne [IntPtr]::Zero) {
            $r = [BreezyShotSonde.Natif+RECT]::new()
            [void]$script:N::GetWindowRect($meilleure, [ref]$r)
            $rects.Add([pscustomobject]@{ R = [int[]]@($r.Left, $r.Top, $r.Right - $r.Left, $r.Bottom - $r.Top); F = $h; Quoi = $script:N::Classe($meilleure) })
        }
    }
    return [pscustomobject]@{ Ms = $chrono.Elapsed.TotalMilliseconds; Rects = $rects; Erreurs = 0 }
}

if (Demarrer-Sonde -Nom 's4' -Titre 'S4 : trouver la page sous un clic' -QuotaMo 1) {
    try {
        $wa = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
        $h2 = [int]($wa.Height / 2) - 4
        $places = [ordered]@{
            wpf         = @(0, 0, 505, $h2)
            winforms    = @(515, 0, 505, $h2)
            explorateur = @(0, ($h2 + 8), 505, $h2)
            edge        = @(515, ($h2 + 8), 505, $h2)
        }
        $verites = [ordered]@{}
        $cibles = @()
        foreach ($g in $places.Keys) {
            try {
                $c = Ouvrir-Cible $g $places[$g]
                $cibles += $c
                $v = $c.Zone
                if ($g -eq 'explorateur') {
                    foreach ($h in $script:N::Filles($c.Racine)) {
                        if ($script:N::Classe($h) -eq 'SHELLDLL_DefView') {
                            $r = [BreezyShotSonde.Natif+RECT]::new(); [void]$script:N::GetWindowRect($h, [ref]$r)
                            $v = [int[]]@($r.Left, $r.Top, $r.Right - $r.Left, $r.Bottom - $r.Top); break
                        }
                    }
                }
                $verites[$g] = $v
            }
            catch { Section "S4 : cible $g"; NonMesurable "cible $g" $_.Exception.Message }
        }
        Start-Sleep -Milliseconds 800
        $ff = Fichier-Temp 'fige.txt'
        $fige = Lancer-Pwsh 'temoin.ps1' @('-Genre', 'fige', '-Fichier', "`"$ff`"")
        if (-not (Attendre -DelaiS 30 -Condition { (Lire-Etat $ff).ContainsKey('hwnd') })) { throw 'écran figé imité pas prêt en 30 s' }
        Start-Sleep -Milliseconds 500
        $liste = @($script:N::FenetresVisibles($fige.Id) | Select-Object -First 8)

        Section 'S4 : liste des fenêtres (8 premières, écran figé montré)'
        $i = 0
        foreach ($h in $liste) { $i++; $b = $script:N::Bord($h); Resultat "fenêtre $i" "$($script:N::Decrire($h)) ($($b.Left),$($b.Top) → $($b.Right),$($b.Bottom))" '' '' }
        foreach ($g in $verites.Keys) { Note "vrai rectangle $g : $(Rect-Texte $verites[$g])" }

        foreach ($moyen in 1, 2) {
            Section "S4 : moyen $moyen ($(if ($moyen -eq 1) { 'UI Automation, ScrollPattern' } else { 'fenêtres filles' }))"
            $tours = @()
            for ($t = 0; $t -lt 3; $t++) { $tours += $(if ($moyen -eq 1) { Moyen1 $liste } else { Moyen2 $liste }) }
            $ms = [double[]]@($tours | ForEach-Object { $_.Ms })
            $med = Mediane $ms
            $max = ($ms | Measure-Object -Maximum).Maximum
            Resultat 'temps pour les 8 fenêtres' "premier tour $(Ms $ms[0]) ; médiane des 3 $(Ms $med) ; max $(Ms $max)$(if ($tours[0].Erreurs) { " ; $($tours[0].Erreurs) fenêtre(s) en erreur" })" '< 300 ms après la photo' (Verdict ($max -lt 300))
            $trouves = $tours[-1].Rects
            $justes = 0; $attendus = 0
            foreach ($g in $verites.Keys) {
                $attendus++
                $meilleur = $null; $e = [int]::MaxValue
                foreach ($x in $trouves) { $d = Ecart $x.R $verites[$g]; if ($d -lt $e) { $e = $d; $meilleur = $x } }
                if ($null -eq $meilleur) { Resultat "rectangle $g" 'aucun trouvé' 'juste à 2 px près' 'échoue'; continue }
                $ok = $e -le 2
                if ($ok) { $justes++ }
                Resultat "rectangle $g" "$(Rect-Texte $meilleur.R) ($($meilleur.Quoi)), écart $e px contre $(Rect-Texte $verites[$g])" 'juste à 2 px près' (Verdict $ok)
            }
            $faux = @($trouves | Where-Object { $x = $_; -not @($verites.Values | Where-Object { (Ecart $x.R $_) -le 2 }) }).Count
            Resultat 'faux candidats' "$faux sur $($trouves.Count) rectangles trouvés" 'à lire (un faux candidat entouré de bleu tromperait)' 'à lire'
            foreach ($x in @($trouves | Where-Object { $y = $_; -not @($verites.Values | Where-Object { (Ecart $y.R $_) -le 2 }) } | Select-Object -First 6)) {
                Note "faux candidat : $(Rect-Texte $x.R) ($($x.Quoi)) dans $($script:N::Decrire($x.F))"
            }
        }

        Section 'S4 : ElementFromPoint pendant l''écran figé'
        if ($verites.Contains('edge')) {
            $pt = Centre $verites['edge']
            try {
                $e = [System.Windows.Automation.AutomationElement]::FromPoint([System.Windows.Point]::new($pt[0], $pt[1]))
                $pidE = $e.Current.ProcessId
                Resultat 'élément sous le centre de la page Edge' "processus $pidE ($(try { (Get-Process -Id $pidE).ProcessName } catch { '?' })), classe $($e.Current.ClassName)" 'supposition forte : l''écran figé (processus de l''app)' (Verdict ($pidE -eq $fige.Id))
            }
            catch { NonMesurable 'ElementFromPoint' $_.Exception.Message }
        }
        else { NonMesurable 'ElementFromPoint' 'Edge absent' }
        Note 'Critère : moyen 1 si la liste est prête en moins de 300 ms après la photo et juste à 2 px près sur Edge, Explorateur et les témoins ; sinon 2 puis 1 en complément. Liste vide ou en retard : le clic ne vise rien et la sélection continue (D3).'
        Note 'Le vrai rectangle de l''Explorateur est pris comme la vue SHELLDLL_DefView (en-têtes de colonnes compris) : un écart de la hauteur de l''en-tête est à lire, pas un échec du moyen.'
        try { $fige.Kill($true) } catch { }
        foreach ($c in $cibles) { Fermer-Cible $c }
    }
    catch { Erreur-Sonde $_ }
}
Terminer-Sonde
