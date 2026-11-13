# S9 et S2 ensemble (contrat 0.6, §1.2, §1.3, ordre du §1.10) :
# - S9 : une fenêtre de contrôle (toujours au-dessus, transparente, sans focus, exclue des
#   captures, WS_EX_LAYERED | WS_EX_TRANSPARENT), en deux variantes (Win32 native en
#   couches ; vraie WebView2 par tao/wry comme l'app), couvre l'écran. (a) la molette
#   (SendInput, pointeur sur la zone) atteint-elle la cible, sur chaque cible ; (b) sur la
#   pilule, va-t-elle à la fenêtre de contrôle ; (c) temps de création jusqu'à la page
#   prête pendant qu'un « écran figé » natif toujours au-dessus est montré, et rien devant.
# - S2 : (a) la fenêtre de contrôle est absente des images (au pixel près contre une image
#   sans elle), et visible quand l'exclusion est retirée (sinon (a) ne prouverait rien) ;
#   (b) temps par image, médiane et 95e centile sur 200 images, 600 × 400 et 1024 × 700 ;
#   (c) objets GDI et mémoire sur 1 000 images ; (d) contenu GPU (canvas WebGL, animation
#   CSS) non noir ; (e) pointeur absent ; (f) clignotement du pointeur avec CAPTUREBLT ;
#   (g) DXGI et Windows.Graphics.Capture sur la machine.
#   pwsh -NoProfile -File sonde/defilante/01-s9-s2-controle-vivant.ps1

. "$PSScriptRoot/commun.ps1"

$WDA_EXCLUDE = 17
$PILULE = @(740, 660, 180, 44)

function Controle-Demarrer([string]$Genre, [int[]]$Zone) {
    $etat = Fichier-Temp "controle-$Genre-$([guid]::NewGuid().ToString('N').Substring(0, 6)).txt"
    $cmd = "$etat.cmd"
    $p = Lancer-Exemple 'controle' @('--genre', $Genre, '--etat', "`"$etat`"", '--commandes', "`"$cmd`"",
        '--zone', (Rect-Texte $Zone), '--pilule', (Rect-Texte $PILULE))
    if ($null -eq $p) { throw 'exemple controle absent (non construit)' }
    $o = [pscustomobject]@{ Genre = $Genre; P = $p; Etat = $etat; Cmd = $cmd }
    # dom_ms n'est écrit que par la variante WebView2 (la page a répondu) ; la native est
    # prête dès qu'elle est visible (premier passage : 7 × 40 s perdues à attendre dom_ms).
    $ok = Attendre -DelaiS 40 -PasMs 50 -Condition { $e = Lire-Etat $etat; $e['visible'] -eq '1' -and ($Genre -ne 'webview' -or ($e['dom_ms'] -as [int]) -gt 0) }
    if (-not $ok) {
        $e = Lire-Etat $etat
        Controle-Arreter $o
        throw "fenêtre de contrôle $Genre pas prête en 40 s ($(if ($e.ContainsKey('erreur')) { $e.erreur } else { 'aucune erreur dite' }))"
    }
    Start-Sleep -Milliseconds 300
    return $o
}
function Controle-Etat($O) { return (Lire-Etat $O.Etat) }
function Controle-Commande($O, [string]$Texte, [scriptblock]$Fait) {
    Ecrire-Commande $O.Cmd $Texte
    return (Attendre -DelaiS 5 -PasMs 50 -Condition { $e = Lire-Etat $O.Etat; & $Fait $e })
}
function Controle-Arreter($O) {
    if ($null -eq $O) { return }
    try { Ecrire-Commande $O.Cmd 'quitter' } catch { }
    if (-not $O.P.WaitForExit(5000)) { try { $O.P.Kill($true) } catch { } }
}

# n crans vers le bas (SendInput, marque de la sonde) au point (x, y) ; rend les mouvements
# de la cible mesurés après chaque cran.
function Crans($C, [int]$N, [int]$X, [int]$Y) {
    $px = [System.Collections.Generic.List[double]]::new()
    $avant = (Attendre-Stable $C 600).Image
    for ($i = 0; $i -lt $N; $i++) {
        $script:N::Poser($X, $Y)
        Start-Sleep -Milliseconds 30
        [void]$script:N::Molette(-1, $script:N::MARQUE)
        $apres = (Attendre-Stable $C 800).Image
        $m = Mouvement $C $avant $apres
        $px.Add($m.Px)
        $avant = $apres
    }
    return , $px
}

if (Demarrer-Sonde -Nom 's9-s2' -Titre 'S9 et S2 : fenêtre de contrôle, molette, images vivantes' -QuotaMo 5) {
    try {
        $wa = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
        $ECRAN = [int[]]@($wa.X, $wa.Y, $wa.Width, $wa.Height)
        $REPOS = @([int]($wa.Right - 6), [int]($wa.Bottom - 6))
        $sent = Ouvrir-Sentinelle @(712, 40, 300, 600)
        $premier = $script:N::GetForegroundWindow()
        $genres = @('natif', 'webview')
        if (-not (Exemple 'controle')) { NonMesurable 'S9 et S2 (a)' 'exemple Rust controle absent (étape de construction échouée ?)' ; $genres = @() }

        # ---------------------------------------------------------------- S2 (a) et S9 (a) (b)
        $cibles = @('wpf', 'winforms', 'molette', 'edge', 'chrome', 'explorateur', 'firefox')
        $premiere = $true
        foreach ($g in $cibles) {
            Section "S9 (a) (b) : molette à travers la fenêtre de contrôle, cible $g"
            $c = $null
            try { $c = Ouvrir-Cible $g @(0, 0, 700, 720) } catch { NonMesurable "cible $g" $_.Exception.Message; continue }
            if ($c.Info) { Note $c.Info }
            $centre = Centre $c.Zone
            foreach ($v in $genres) {
                $o = $null
                try {
                    $script:N::Poser($REPOS[0], $REPOS[1])
                    if ($premiere -and $g -eq 'wpf') {
                        # S2 (a) sur la cible WPF immobile : références sans la fenêtre, par moyen.
                        $refs = @{}
                        $refs['bitblt'] = Prendre $ECRAN
                        $refs['captureblt'] = Prendre $ECRAN -CaptureBlt
                        foreach ($m in 'dxgi', 'wgc-ecran') {
                            $f = Fichier-Temp "ref-$m.png"
                            $r = Jouer-Exemple 'vivant' @('--methode', $m, '--zone', (Rect-Texte $ECRAN), '--images', '3', '--intervalle', '50', '--png', "`"$f`"") 30
                            if ($r['ok'] -eq '1' -and (Test-Path $f)) { $refs[$m] = (Lire-Png $f).O } else { $refs[$m] = "$($r['erreur'])" }
                        }
                    }
                    $tDebut = [System.Diagnostics.Stopwatch]::StartNew()
                    $o = Controle-Demarrer $v $c.Zone
                    $e = Controle-Etat $o
                    $style = [Convert]::ToInt64($e.exstyle.Substring(2), 16)
                    $attendu = 0x80000 -bor 0x20 -bor 0x8 -bor 0x80 -bor 0x8000000
                    Resultat "$v : fenêtre prête" "page qui répond $($e.dom_ms) ms, première image $($e.pret_ms) ms après le lancement du processus (0 : pas encore)" 'à lire' 'à lire'
                    Resultat "$v : style étendu" "$($e.exstyle) (LAYERED, TRANSPARENT, TOPMOST, TOOLWINDOW, NOACTIVATE attendus)" 'tous présents' (Verdict (($style -band $attendu) -eq $attendu))
                    Resultat "$v : affinité relue" $e.affinite "17 (WDA_EXCLUDEFROMCAPTURE)" (Verdict ($e.affinite -eq '17'))
                    $fg = $script:N::GetForegroundWindow()
                    Resultat "$v : premier plan" ($script:N::Decrire($fg)) 'jamais la fenêtre de contrôle' (Verdict ($script:N::Pid($fg) -ne $o.P.Id))
                    if ($e.ContainsKey('erreur')) { Note "$v : erreurs dites par la fenêtre : $($e.erreur)" }

                    if ($premiere -and $g -eq 'wpf') {
                        Section "S2 (a) : la fenêtre de contrôle $v absente des images"
                        # Preuve qu'elle se dessine : exclusion retirée, elle doit se voir.
                        $retiree = Controle-Commande $o 'affinite 0' { param($x) $x['affinite'] -eq '0' }
                        Start-Sleep -Milliseconds 300
                        $vue = Prendre $ECRAN
                        $dv = $script:N::Differences($refs['bitblt'], $vue, $ECRAN[2], 0, 0, $ECRAN[2], $ECRAN[3])
                        $png = Enregistrer-Png $vue $ECRAN[2] $ECRAN[3] "s2-controle-$v-sans-exclusion.png"
                        if (-not $retiree) { NonMesurable "$v : visible sans l'exclusion" 'la commande affinite 0 n''a pas été appliquée en 5 s' }
                        else { Resultat "$v : visible sans l'exclusion (BitBlt)" "$dv pixels différents de l'image sans la fenêtre$(if ($png) { " ($png)" })" '> 0 (sinon (a) ne prouve rien)' (Verdict ($dv -gt 1000)) }
                        $remise = Controle-Commande $o "affinite $WDA_EXCLUDE" { param($x) $x['affinite'] -eq '17' }
                        Start-Sleep -Milliseconds 300
                        $imgs = @{ bitblt = (Prendre $ECRAN); captureblt = (Prendre $ECRAN -CaptureBlt) }
                        foreach ($m in 'dxgi', 'wgc-ecran') {
                            $f = Fichier-Temp "exclue-$m-$v.png"
                            $r = Jouer-Exemple 'vivant' @('--methode', $m, '--zone', (Rect-Texte $ECRAN), '--images', '3', '--intervalle', '50', '--png', "`"$f`"") 30
                            if ($r['ok'] -eq '1' -and (Test-Path $f)) { $imgs[$m] = (Lire-Png $f).O } else { $imgs[$m] = "$($r['erreur'])" }
                        }
                        foreach ($m in 'bitblt', 'captureblt', 'dxgi', 'wgc-ecran') {
                            if (-not $remise) { NonMesurable "$v, $m : absente" 'exclusion non remise'; continue }
                            if ($refs[$m] -isnot [byte[]] -or $imgs[$m] -isnot [byte[]]) {
                                $raison = if ($refs[$m] -isnot [byte[]]) { $refs[$m] } else { $imgs[$m] }
                                NonMesurable "$v, $m : absente de l'image" "moyen indisponible : $raison"
                                continue
                            }
                            $d = $script:N::Differences($refs[$m], $imgs[$m], $ECRAN[2], 0, 0, $ECRAN[2], $ECRAN[3])
                            if ($d -ne 0) { $null = Enregistrer-Png $imgs[$m] $ECRAN[2] $ECRAN[3] "s2-controle-$v-exclue-$m.png" }
                            Resultat "$v, $m : absente de l'image" "$d pixels différents de l'image sans la fenêtre (zone utile $($ECRAN[2]) × $($ECRAN[3]))" '0 (au pixel près)' (Verdict ($d -eq 0))
                        }
                        Section "S9 (a) (b) : molette à travers la fenêtre de contrôle, cible $g (suite)"
                    }

                    # (a) molette sur la zone, à travers la fenêtre de contrôle (qui ne compte
                    # pas : WindowFromPoint passe à travers une fenêtre WS_EX_TRANSPARENT).
                    $couverte = Degager $c
                    if ($couverte) { Noter-Obstacles $c; NonMesurable "$v, (a) et (b) ($g)" "cible couverte par $couverte"; continue }
                    $avantCtrl = [int](Controle-Etat $o).molettes
                    $sAvant = Etat-Sentinelle $sent
                    $px = Crans $c 5 $centre[0] $centre[1]
                    $bouges = @($px | Where-Object { -not [double]::IsNaN($_) -and $_ -gt 0 }).Count
                    $recues = [int](Controle-Etat $o).molettes - $avantCtrl
                    $sApres = Etat-Sentinelle $sent
                    Resultat "$v, (a) molette sur la zone ($g)" "page bougée $bouges/5 ($(($px | ForEach-Object { if ([double]::IsNaN($_)) { '?' } else { $_ } }) -join ', ') px) ; reçues par la fenêtre de contrôle : $recues ; sentinelle $sAvant → $sApres" '5/5, 0 reçue, sentinelle immobile' (Verdict ($bouges -eq 5 -and $recues -eq 0 -and $sAvant -eq $sApres))

                    # (b) molette sur la pilule : la fenêtre de contrôle doit la recevoir.
                    $pc = Centre $PILULE
                    $script:N::Poser($pc[0], $pc[1])
                    $prise = Attendre -DelaiS 2 -PasMs 30 -Condition { (Lire-Etat $o.Etat)['traverse'] -eq '0' }
                    $avantP = [int](Controle-Etat $o).molettes_pilule
                    $avantImg = (Attendre-Stable $c 600).Image
                    $sAvant = Etat-Sentinelle $sent
                    for ($i = 0; $i -lt 5; $i++) { $script:N::Poser($pc[0], $pc[1]); [void]$script:N::Molette(-1, $script:N::MARQUE); Start-Sleep -Milliseconds 120 }
                    Start-Sleep -Milliseconds 300
                    $recuesP = [int](Controle-Etat $o).molettes_pilule - $avantP
                    $m = Mouvement $c $avantImg (Attendre-Stable $c 600).Image
                    $sApres = Etat-Sentinelle $sent
                    Resultat "$v, (b) molette sur la pilule ($g)" "fenêtre rendue cliquable : $(if ($prise) { 'oui' } else { 'non en 2 s' }) ; reçues : $recuesP/5 ; cible : $($m.Px) px ; sentinelle $sAvant → $sApres" '5/5 reçues, cible et sentinelle immobiles' (Verdict ($prise -and $recuesP -eq 5 -and $m.Px -eq 0 -and $sAvant -eq $sApres))
                    $script:N::Poser($REPOS[0], $REPOS[1])
                    $rendue = Attendre -DelaiS 2 -PasMs 30 -Condition { (Lire-Etat $o.Etat)['traverse'] -eq '1' }
                    if (-not $rendue) { Note "$v : la fenêtre n'est pas redevenue traversée 2 s après le départ de la souris" }
                }
                catch { NonMesurable "$v sur $g" $_.Exception.Message }
                finally { Controle-Arreter $o }
            }
            if ($g -eq 'wpf') { $premiere = $false }
            Fermer-Cible $c
        }

        # ---------------------------------------------------------------- S9 (c)
        Section 'S9 (c) : création de la fenêtre de contrôle WebView2 pendant l''écran figé'
        if (-not (Exemple 'controle')) { NonMesurable 'S9 (c)' 'exemple controle absent' }
        else {
            $ff = Fichier-Temp 'fige.txt'
            $fige = Lancer-Pwsh 'temoin.ps1' @('-Genre', 'fige', '-Fichier', "`"$ff`"")
            if (-not (Attendre -DelaiS 30 -Condition { (Lire-Etat $ff).ContainsKey('hwnd') })) { NonMesurable 'S9 (c)' 'écran figé imité pas prêt en 30 s' }
            else {
                $hf = [IntPtr]::new([long](Lire-Etat $ff).hwnd)
                Start-Sleep -Milliseconds 500
                foreach ($mode in @(@{ Nom = 'avec une vue Web déjà vivante (gardien)'; A = @('--nombre', '20') }, @{ Nom = 'sans autre vue Web vivante'; A = @('--nombre', '10', '--sans-gardien') })) {
                    $etat = Fichier-Temp "creations-$([guid]::NewGuid().ToString('N').Substring(0, 6)).txt"
                    $p = Lancer-Exemple 'controle' (@('--genre', 'creations', '--etat', "`"$etat`"") + $mode.A)
                    $devant = 0; $echantillons = 0
                    $limite = [DateTime]::UtcNow.AddSeconds(120)
                    while (-not $p.HasExited -and [DateTime]::UtcNow -lt $limite) {
                        # Une fenêtre du processus de contrôle, visible, au-dessus de l'écran figé ?
                        foreach ($h in $script:N::Fenetres(0)) {
                            if ($h -eq $hf) { break }
                            if ($script:N::Pid($h) -eq $p.Id) { $devant++; break }
                        }
                        $echantillons++
                        Start-Sleep -Milliseconds 15
                    }
                    if (-not $p.HasExited) { try { $p.Kill($true) } catch { } }
                    $e = Lire-Etat $etat
                    $lignes = @(Get-Content -LiteralPath $etat -ErrorAction SilentlyContinue | Where-Object { $_ -like 'creation *' })
                    $ms = [System.Collections.Generic.List[double]]::new(); $img = [System.Collections.Generic.List[double]]::new(); $vues = 0; $echecs = 0; $sansImage = 0
                    foreach ($l in $lignes) {
                        if ($l -match 'dom_ms ([\d.]+) pret_ms (\S+) visible_avant_pret (\d)') {
                            $ms.Add([double]$Matches[1])
                            if ($Matches[2] -eq 'absent') { $sansImage++ } else { $img.Add([double]$Matches[2]) }
                            if ($Matches[3] -eq '1') { $vues++ }
                        }
                        else { $echecs++ }
                    }
                    if ($ms.Count -eq 0) { NonMesurable "création ($($mode.Nom))" "aucune page qui répond ($(if ($e.ContainsKey('erreur')) { $e.erreur } else { "$echecs échec(s)" }))"; continue }
                    $suivantes = @($ms | Select-Object -Skip 1)
                    $med = Mediane $ms.ToArray()
                    Resultat "création → page qui répond ($($mode.Nom))" "première (à froid) $(Ms $ms[0]) ; médiane des $($ms.Count) $(Ms $med) ; médiane des suivantes $(Ms (Mediane $suivantes)) ; max $(Ms (($ms | Measure-Object -Maximum).Maximum)) ; échecs $echecs" 'médiane < 500 ms : création par capture (R1)' (Verdict ($med -lt 500))
                    Resultat "création → première image dessinée, fenêtre cachée ($($mode.Nom))" "$(if ($img.Count) { "médiane $(Ms (Mediane $img.ToArray()))" } else { 'aucune' }) ; sans image en 3 s : $sansImage" 'à lire (une page cachée peut ne pas dessiner)' 'à lire'
                    Resultat "rien devant l'écran figé ($($mode.Nom))" "fenêtre visible pendant la création : $vues fois ; une fenêtre du processus au-dessus de l'écran figé dans $devant échantillons sur $echantillons" '0 et 0' (Verdict ($vues -eq 0 -and $devant -eq 0))
                }
                Note 'La fenêtre de contrôle étant exclue des captures, « rien devant » se lit par l''ordre des fenêtres (EnumWindows) et IsWindowVisible, pas sur une image.'

                # À froid : 20 créations, chacune dans un processus neuf sans autre vue Web (ce
                # que voit la première capture défilante si l'app n'a pas encore de vue Web).
                $froid = [System.Collections.Generic.List[double]]::new(); $lance = [System.Collections.Generic.List[double]]::new()
                $mur = [System.Collections.Generic.List[double]]::new(); $echecsF = 0; $vuesF = 0
                $limiteF = [DateTime]::UtcNow.AddSeconds(150)
                for ($k = 0; $k -lt 20 -and [DateTime]::UtcNow -lt $limiteF; $k++) {
                    $etat = Fichier-Temp "froid-$k.txt"
                    $chronoF = [System.Diagnostics.Stopwatch]::StartNew()
                    $p = Lancer-Exemple 'controle' @('--genre', 'creations', '--etat', "`"$etat`"", '--nombre', '1', '--sans-gardien')
                    $tenu = @{ L = $null }
                    $null = Attendre -DelaiS 15 -PasMs 20 -Condition { $x = @((Lire-Etat $etat).Keys | Where-Object { $_ -eq 'creation' }); if ($x.Count -gt 0) { $tenu.L = "creation $((Lire-Etat $etat)['creation'])"; $true } else { $false } }
                    $ligne = $tenu.L
                    $tMur = $chronoF.Elapsed.TotalMilliseconds
                    if (-not $p.WaitForExit(5000)) { try { $p.Kill($true) } catch { } }
                    if ($null -eq $ligne) { $e = Lire-Etat $etat; if ($e.ContainsKey('creation')) { $ligne = "creation $($e['creation'])" } elseif ($e.ContainsKey('erreur')) { Note "à froid, processus $($k + 1) : $($e['erreur'])" } }
                    if ("$ligne" -match 'dom_ms ([\d.]+) pret_ms (\S+) visible_avant_pret (\d)(?: lancement_dom_ms ([\d.]+))?') {
                        $froid.Add([double]$Matches[1]); $mur.Add($tMur)
                        if ($Matches[4]) { $lance.Add([double]$Matches[4]) }
                        if ($Matches[3] -eq '1') { $vuesF++ }
                    }
                    else { $echecsF++ }
                }
                if ($froid.Count -eq 0) { NonMesurable 'création à froid, processus neuf' "aucune page qui répond ($echecsF échec(s))" }
                else {
                    $fa = $froid.ToArray(); $la = $lance.ToArray(); $ma = $mur.ToArray()
                    $sur = @($fa | Where-Object { $_ -ge 500 }).Count
                    Resultat "création à froid dans un processus neuf ($($froid.Count) processus)" "fenêtre → page qui répond : première $(Ms $fa[0]) ; médiane $(Ms (Mediane $fa)) ; 95e centile $(Ms (Centile $fa 0.95)) ; max $(Ms (($fa | Measure-Object -Maximum).Maximum)) ; ≥ 500 ms : $sur/$($froid.Count) ; échecs $echecsF" 'moins de 500 ms (R1), à froid : ce que voit la première capture' $(if ($sur -eq 0) { 'passe' } else { 'échoue' })
                    if ($la.Count -gt 0) { Resultat 'à froid : lancement du processus → page qui répond' "médiane $(Ms (Mediane $la)) ; max $(Ms (($la | Measure-Object -Maximum).Maximum)) (environnement WebView2 compris)" 'à lire (l''app est déjà lancée : borne haute)' 'à lire' }
                    Resultat 'à froid : Start-Process → ligne écrite (vu de la sonde)' "médiane $(Ms (Mediane $ma)) ; max $(Ms (($ma | Measure-Object -Maximum).Maximum))" 'à lire' 'à lire'
                    Resultat 'à froid : fenêtre visible avant la page' "$vuesF fois" '0' (Verdict ($vuesF -eq 0))
                }
            }
            try { $fige.Kill($true) } catch { }
        }

        # ---------------------------------------------------------------- S2 (b) à (g)
        Section 'S2 (b) à (g) : images vivantes sur Edge (canvas WebGL et animation CSS)'
        $c = $null
        try { $c = Ouvrir-Cible 'edge' @(0, 0, $wa.Width, $wa.Height) } catch { NonMesurable 'Edge pour S2' $_.Exception.Message }
        if ($null -ne $c) {
            $rendu = if ($script:N::Titre($c.Racine) -match ' r=(\w+)') { $Matches[1] } else { '?' }
            Resultat 'rendu de la canvas dans Edge' $rendu 'webgl (sinon (d) ne teste que la 2D)' (Verdict ($rendu -eq 'webgl'))
            $Z1 = [int[]]@(40, 80, 600, 400)
            $Z2 = [int[]]@(0, 0, [math]::Min(1024, $wa.Width), [math]::Min(700, $wa.Height))
            $methodes = 'bitblt', 'captureblt', 'dxgi', 'wgc-ecran'
            $dispo = @{}
            foreach ($m in $methodes) {
                foreach ($z in @($Z1, $Z2)) {
                    $r = Jouer-Exemple 'vivant' @('--methode', $m, '--zone', (Rect-Texte $z), '--images', '200', '--intervalle', '16') 90
                    $nomZ = "$($z[2]) × $($z[3])"
                    if ($r['ok'] -ne '1') { NonMesurable "(b) $m, $nomZ" "$($r['erreur'])"; $dispo[$m] = "$($r['erreur'])"; continue }
                    $dispo[$m] = 'oui'
                    $med = [double]$r.mediane_ms
                    $crit = if ($m -eq 'bitblt' -and $z -eq $Z2) { 'médiane ≤ 25 ms en 1024 × 700' } else { 'à lire' }
                    $ver = if ($m -eq 'bitblt' -and $z -eq $Z2) { Verdict ($med -le 25) } else { '' }
                    Resultat "(b) $m, $nomZ, 200 images" "médiane $(Ms $med), 95e centile $(Ms ([double]$r.p95_ms)), max $(Ms ([double]$r.max_ms)) ; images neuves $($r.neuves)/200 (médiane des neuves $(Ms ([double]$r.mediane_neuves_ms)))" $crit $ver
                    if ($r.ContainsKey('adaptateur')) { Note "$m : adaptateur $($r.adaptateur)" }
                    foreach ($k in 'wgc_sans_cadre', 'wgc_sans_pointeur', 'wgc_acces_sans_cadre', 'wgc_pris_en_charge', 'wgc_appareil_materiel_refuse') { if ($r.ContainsKey($k) -and $z -eq $Z1) { Note "$m : $k = $($r[$k])" } }
                }
            }
            # Fenêtre seule (WGC) : la fenêtre Edge.
            $r = Jouer-Exemple 'vivant' @('--methode', 'wgc-fenetre', '--hwnd', "$([long]$c.Racine)", '--zone', (Rect-Texte $Z1), '--images', '200', '--intervalle', '16') 90
            if ($r['ok'] -eq '1') { Resultat '(b) wgc-fenetre (Edge), 600 × 400' "médiane $(Ms ([double]$r.mediane_ms)), 95e centile $(Ms ([double]$r.p95_ms)), neuves $($r.neuves)/200" 'à lire' }
            else { NonMesurable '(b) wgc-fenetre (Edge)' "$($r['erreur'])" }

            # (c) 1 000 images : objets GDI et mémoire.
            $r = Jouer-Exemple 'vivant' @('--methode', 'bitblt', '--zone', (Rect-Texte $Z2), '--images', '1000', '--intervalle', '0', '--ressources') 120
            if ($r['ok'] -eq '1') {
                $dg = [int]$r.gdi_apres - [int]$r.gdi_avant
                $dm = ([double]$r.prive_apres - [double]$r.prive_avant) / 1MB
                # Critère du contrat : objets GDI et mémoire « stables ». Lu comme : GDI
                # identiques, et la mémoire ne monte plus pendant la seconde moitié (une fuite
                # monte à chaque image ; un palier du début n'en est pas une). Plus de seuil
                # propre à la sonde (premier passage : « + 1 Mo au plus », pas dans le contrat).
                $q = @("$($r['prive_quarts'])" -split ';' | Where-Object { $_ } | ForEach-Object { [double]$_ })
                $gq = "$($r['gdi_quarts'])"
                $pente = if ($q.Count -ge 2) { ([double]$r.prive_apres - $q[1]) / 1MB } else { [double]::NaN }
                $stable = $dg -eq 0 -and -not [double]::IsNaN($pente) -and $pente -le 0.25
                Resultat '(c) bitblt, 1 000 images 1024 × 700' "objets GDI $($r.gdi_avant) → $($r.gdi_apres) (aux quarts : $gq) ; mémoire privée $('{0:0.0}' -f ([double]$r.prive_avant / 1MB)) → $('{0:0.0}' -f ([double]$r.prive_apres / 1MB)) Mo (aux quarts : $(($q | ForEach-Object { '{0:0.0}' -f ($_ / 1MB) }) -join ' ; ') Mo ; seconde moitié $('{0:+0.00;-0.00}' -f $pente) Mo) ; médiane $(Ms ([double]$r.mediane_ms))" 'GDI et mémoire stables (contrat) : GDI identiques, mémoire sans pente sur la seconde moitié (≤ 0,25 Mo)' $(if ($stable) { 'passe' } elseif ($dg -ne 0) { 'échoue' } else { 'à lire' })
            }
            else { NonMesurable '(c) bitblt, 1 000 images' "$($r['erreur'])" }

            # (b) en détail : d'où viennent les 32 ms de BitBlt (premier passage : même médiane
            # à 600 × 400 qu'à 1024 × 700).
            $r = Jouer-Exemple 'vivant' @('--methode', 'bitblt-detail', '--zone', (Rect-Texte $Z2)) 60
            if ($r['ok'] -ne '1') { NonMesurable '(b) bitblt en détail' "$($r['erreur'])" }
            else {
                Resultat '(b) DwmFlush seul (20 appels)' "médiane $(Ms ([double]$r.dwmflush_mediane_ms))$(if ($r.ContainsKey('dwmflush_erreur')) { " ; erreur $($r.dwmflush_erreur)" })" 'à lire : ≈ une image de l''écran (le moniteur Hyper-V est à 1 Hz d''après l''inventaire)' 'à lire'
                foreach ($k in @($r.Keys | Where-Object { $_ -like 'detail_*' } | Sort-Object)) {
                    $val = "$($r[$k])"
                    if ($val -match '^([\d.]+);([\d.]+);([\d.]+)$') { $val = "médiane $(Ms ([double]$Matches[1])), 95e centile $(Ms ([double]$Matches[2])), max $(Ms ([double]$Matches[3])) sur 20" }
                    else { $val = Ms ([double]$val) }
                    Resultat "(b) bitblt $($k.Substring(7) -replace '_', ' ')" $val 'à lire : un temps fixe quelle que soit la taille, et qui tombe après DwmFlush, serait une attente de composition' 'à lire'
                }
            }

            # (d) contenu GPU : la canvas et les carrés animés (coordonnées CSS de longue.html).
            $zv = [int[]]@(($c.Zone[0] + 24), ($c.Zone[1] + 64), 408, 120)
            foreach ($m in $methodes) {
                $d = Join-Path $script:Sonde.Temp "gpu-$m"
                New-Item -ItemType Directory -Force -Path $d | Out-Null
                $r = Jouer-Exemple 'vivant' @('--methode', $m, '--zone', (Rect-Texte $zv), '--images', '5', '--intervalle', '120', '--suite', "`"$d`"", '--chaque', '1') 60
                $fichiers = @(Get-ChildItem -LiteralPath $d -Filter '*.png' -ErrorAction SilentlyContinue | Sort-Object Name)
                if ($r['ok'] -ne '1' -or $fichiers.Count -lt 2) { NonMesurable "(d) $m : contenu GPU" "$($r['erreur'])"; continue }
                $imgs = @($fichiers | ForEach-Object { Lire-Png $_.FullName })
                $noir = $script:N::PartNoire($imgs[0].O, $imgs[0].L, 0, 0, 240, 120)
                $change = 0
                for ($i = 1; $i -lt $imgs.Count; $i++) { if (-not $script:N::Egales($imgs[$i - 1].O, $imgs[$i].O)) { $change++ } }
                $png = Enregistrer-Png $imgs[0].O $imgs[0].L $imgs[0].H "s2-gpu-$m.png"
                Resultat "(d) $m : canvas et animation CSS" ("canvas noire à {0:P0} ; images qui changent : $change/$($imgs.Count - 1)$(if ($png) { " ($png)" })" -f $noir) 'non noire (< 50 %) et animée' (Verdict ($noir -lt 0.5 -and $change -gt 0))
            }

            # (e) pointeur : sur une partie immobile de la page, puis loin. Témoin : WGC avec le
            # pointeur laissé dans l'image doit voir une différence, sinon le test ne prouve rien
            # (le pointeur est caché sur une machine sans souris).
            $zp = [int[]]@(($c.Zone[0] + 120), ($c.Zone[1] + 320), 300, 200)
            Note "(e) pointeur montré (GetCursorInfo) : $($script:N::PointeurMontre())"
            $temoinPointeur = -1
            foreach ($m in @($methodes) + 'wgc-ecran-avec-pointeur') {
                $images = @{}
                foreach ($ou in 'dedans', 'dehors') {
                    if ($ou -eq 'dedans') { $script:N::Poser($zp[0] + 150, $zp[1] + 100) } else { $script:N::Poser($REPOS[0], $REPOS[1]) }
                    Start-Sleep -Milliseconds 250
                    if ($m -eq 'bitblt' -or $m -eq 'captureblt') { $images[$ou] = Prendre $zp -CaptureBlt:($m -eq 'captureblt') }
                    else {
                        $f = Fichier-Temp "pointeur-$m-$ou.png"
                        $aa = @('--methode', ($m -replace '-avec-pointeur$', ''), '--zone', (Rect-Texte $zp), '--images', '3', '--intervalle', '50', '--png', "`"$f`"")
                        if ($m -like '*-avec-pointeur') { $aa += '--avec-pointeur' }
                        $r = Jouer-Exemple 'vivant' $aa 30
                        if ($r['ok'] -eq '1' -and (Test-Path $f)) { $images[$ou] = (Lire-Png $f).O } else { $images[$ou] = $null }
                    }
                }
                if ($null -eq $images['dedans'] -or $null -eq $images['dehors']) { NonMesurable "(e) $m : pointeur absent" 'moyen indisponible'; continue }
                $d = $script:N::Differences($images['dedans'], $images['dehors'], $zp[2], 0, 0, $zp[2], $zp[3])
                if ($m -like '*-avec-pointeur') { $temoinPointeur = $d; Resultat "(e) témoin : WGC avec le pointeur laissé" "$d pixels différents" '> 0 (sinon le test (e) ne voit pas le pointeur)' $(if ($d -gt 0) { 'passe' } else { 'non mesurable' }) }
                else { Resultat "(e) $m : pointeur absent de l'image" "$d pixels différents (pointeur dans la zone contre loin)" '0' (Verdict ($d -eq 0)) }
            }
            if ($temoinPointeur -eq 0) { Note '(e) le témoin ne voit pas le pointeur : les « 0 » ci-dessus ne prouvent rien sur cette machine (pointeur caché sans souris) ; à vérifier chez l''utilisateur.' }

            # (f) clignotement du pointeur avec CAPTUREBLT, vu par GetCursorInfo et par DXGI.
            $script:N::Poser($zp[0] + 150, $zp[1] + 100)
            foreach ($cb in $false, $true) {
                $a = @('--methode', 'clignotement', '--zone', (Rect-Texte $Z2), '--duree-ms', '3000')
                if ($cb) { $a += '--captureblt' }
                $r = Jouer-Exemple 'vivant' $a 30
                $nom = if ($cb) { 'SRCCOPY | CAPTUREBLT' } else { 'SRCCOPY seul' }
                if ($r['ok'] -ne '1') { NonMesurable "(f) $nom" "$($r['erreur'])"; continue }
                $dx = if ($r.ContainsKey('dxgi_pointeur_maj')) { "DXGI : $($r.dxgi_pointeur_maj) mises à jour du pointeur, $($r.dxgi_pointeur_invisible) invisible" } else { "DXGI : $($r['dxgi_erreur'])" }
                Resultat "(f) $nom, 3 s de copies 1024 × 700" "$($r.copies) copies ; pointeur caché dans $($r.lectures_pointeur_cache) lectures sur $($r.lectures_pointeur) (GetCursorInfo) ; $dx" 'à lire (supposition : CAPTUREBLT fait clignoter le pointeur)' 'à lire'
            }
            Note '(f) Une lecture « caché » de GetCursorInfo ou un pointeur invisible pour DXGI pendant les copies, et pas sans CAPTUREBLT, serait le clignotement. Zéro partout ne prouve pas son absence à l''œil : à regarder chez l''utilisateur.'

            # Cadre jaune de Windows.Graphics.Capture : avec le cadre demandé (témoin) puis
            # avec IsBorderRequired = false, l'écran est copié pendant la session par BitBlt et
            # par DXGI ; on compte les pixels jaunes et les pixels changés sur les 6 px du bord
            # de l'écran (là où le cadre d'une capture d'écran se dessine).
            $bord = 6
            $cadres = @{}
            foreach ($avecCadre in $true, $false) {
                $nomC = if ($avecCadre) { 'cadre demandé (témoin)' } else { 'IsBorderRequired = false' }
                $avant = Prendre $ECRAN
                $fd0 = Fichier-Temp "ecran-avant-$avecCadre.png"
                $r0 = Jouer-Exemple 'vivant' @('--methode', 'dxgi', '--zone', (Rect-Texte $ECRAN), '--images', '3', '--intervalle', '30', '--png', "`"$fd0`"") 30
                $a = @('--methode', 'wgc-ecran', '--zone', (Rect-Texte $Z1), '--images', '300', '--intervalle', '16')
                if ($avecCadre) { $a += '--avec-cadre' }
                $p = Lancer-Exemple 'vivant' $a
                if ($null -eq $p) { NonMesurable "(g) cadre WGC ($nomC)" 'exemple vivant absent'; continue }
                Start-Sleep -Milliseconds 1500
                $pendant = Prendre $ECRAN
                $fd1 = Fichier-Temp "ecran-pendant-$avecCadre.png"
                $r1 = Jouer-Exemple 'vivant' @('--methode', 'dxgi', '--zone', (Rect-Texte $ECRAN), '--images', '3', '--intervalle', '30', '--png', "`"$fd1`"") 30
                $null = $p.WaitForExit(20000)
                $LE = $ECRAN[2]; $HE = $ECRAN[3]
                $bords = { param($x, $y) (($script:N::Differences($x, $y, $LE, 0, 0, $LE, $bord)) + ($script:N::Differences($x, $y, $LE, 0, ($HE - $bord), $LE, $HE)) + ($script:N::Differences($x, $y, $LE, 0, $bord, $bord, ($HE - $bord))) + ($script:N::Differences($x, $y, $LE, ($LE - $bord), $bord, $LE, ($HE - $bord)))) }
                $gdi = "GDI : jaunes $($script:N::PixelsProches($avant, 255, 220, 0, 50)) avant, $($script:N::PixelsProches($pendant, 255, 220, 0, 50)) pendant ; bord changé $(& $bords $avant $pendant) px"
                $dx = 'DXGI : indisponible'
                if ($r0['ok'] -eq '1' -and $r1['ok'] -eq '1' -and (Test-Path $fd0) -and (Test-Path $fd1)) {
                    $i0 = Lire-Png $fd0; $i1 = Lire-Png $fd1
                    if ($i0.L -eq $LE -and $i1.L -eq $LE -and $i0.H -eq $HE -and $i1.H -eq $HE) {
                        $cadres[$avecCadre] = & $bords $i0.O $i1.O
                        $dx = "DXGI : jaunes $($script:N::PixelsProches($i0.O, 255, 220, 0, 50)) avant, $($script:N::PixelsProches($i1.O, 255, 220, 0, 50)) pendant ; bord changé $($cadres[$avecCadre]) px"
                    }
                }
                Resultat "(g) cadre jaune pendant une session WGC, $nomC" "$gdi ; $dx" $(if ($avecCadre) { 'témoin : > 0 attendu (sinon le test ne voit pas le cadre)' } else { '0 (pas de cadre)' }) 'à lire'
                if (-not $avecCadre) { $null = Enregistrer-Png $pendant $ECRAN[2] $ECRAN[3] 's2-pendant-wgc.png' }
            }
            if ($cadres.ContainsKey($true) -and $cadres.ContainsKey($false)) {
                $vt = $cadres[$true]; $vs = $cadres[$false]
                Resultat '(g) cadre jaune absent avec IsBorderRequired = false' "bord changé (DXGI) : $vt px avec le cadre demandé, $vs px sans" 'témoin > 0 et 0 sans' $(if ($vt -gt 0 -and $vs -eq 0) { 'passe' } elseif ($vt -eq 0) { 'non mesurable' } else { 'échoue' })
                if ($vt -eq 0) { Note '(g) le témoin ne voit aucun cadre (ni GDI ni DXGI) : le cadre est peut-être exclu des copies sur cette machine ; à regarder chez l''utilisateur.' }
            }
            # WGC : ce que dit la session (une app non empaquetée peut-elle retirer le cadre et le
            # pointeur, temps jusqu'à la première image).
            $r = Jouer-Exemple 'vivant' @('--methode', 'wgc-ecran', '--zone', (Rect-Texte $Z1), '--images', '5', '--intervalle', '16') 30
            if ($r['ok'] -eq '1') {
                Resultat '(g) WGC : IsBorderRequired = false accepté (app non empaquetée)' "$($r['wgc_sans_cadre']) ; relu $($r['wgc_cadre_relu']) ; accès « sans cadre » : $($r['wgc_acces_sans_cadre'])" '1 (accepté) et relu 0' (Verdict ("$($r['wgc_sans_cadre'])" -eq '1' -and "$($r['wgc_cadre_relu'])" -eq '0'))
                Resultat '(g) WGC : IsCursorCaptureEnabled = false accepté' "$($r['wgc_sans_pointeur'])" '1' (Verdict ("$($r['wgc_sans_pointeur'])" -eq '1'))
                Resultat '(g) WGC : temps jusqu''à la première image' "$($r['wgc_premiere_image_ms']) ms après StartCapture ; ouverture complète $($r['ouverture_ms']) ms" 'à lire' 'à lire'
            }
            else { NonMesurable '(g) WGC : session' "$($r['erreur'])" }
            foreach ($m in $methodes) { Resultat "(g) $m disponible" $(if ($dispo.ContainsKey($m)) { $dispo[$m] } else { 'non essayé' }) 'à lire' }
            Fermer-Cible $c
        }

        Section 'Critères (rappel du contrat)'
        Note 'S2 : moyen 1 (BitBlt) retenu s''il passe (a), (c), (d), (e) et si sa médiane est ≤ 25 ms en 1024 × 700 ; avec CAPTUREBLT seulement si sans lui une fenêtre visible manque ; sinon moyen 3 (WGC).'
        Note 'S9 : (a) doit passer sur chaque cible, sinon le dessin pendant le défilement change (quatre fenêtres natives autour de la zone). (c) page prête en moins de 500 ms : création par capture (R1) ; au-delà, fenêtre préparée avec la licence.'
        $fin = $script:N::GetForegroundWindow()
        Note "Premier plan au début : $($script:N::Decrire($premier)) ; à la fin : $($script:N::Decrire($fin))."
    }
    catch { Erreur-Sonde $_ }
}
Terminer-Sonde
