# S7 (contrat 0.6, §1.8) : mémoire et taille maximale de l'image finale. Pour 1 000 × 16 384,
# 2 560 × 9 765 (25 Mpx) et 1 000 × 30 000, l'exemple Rust memoire (une image par processus)
# fait l'image (bandes de couleurs, pas du bruit), l'encode en PNG comme `sortie` (png,
# RGBA, Fast), l'écrit, la copie par arboard ; ce script relit le presse-papiers dans un
# fil STA ; puis une vue Web (WebView2, exemple controle --genre apercu) décode chaque PNG
# et le dessine dans une <canvas> de sa taille.
# Les très grandes images restent dans le dossier temporaire (jamais dans sonde-resultats).
#   pwsh -NoProfile -File sonde/defilante/07-s7-memoire.ps1

. "$PSScriptRoot/commun.ps1"

# Lit l'image du presse-papiers dans un fil STA : { Ms, L, H } ou une erreur.
function Relire-PressePapiers {
    $rs = [runspacefactory]::CreateRunspace()
    $rs.ApartmentState = [System.Threading.ApartmentState]::STA
    $rs.Open()
    try {
        $ps = [powershell]::Create()
        $ps.Runspace = $rs
        [void]$ps.AddScript({
                Add-Type -AssemblyName System.Windows.Forms, System.Drawing
                $t = [System.Diagnostics.Stopwatch]::StartNew()
                $img = $null
                for ($i = 0; $i -lt 10 -and $null -eq $img; $i++) {
                    try { $img = [System.Windows.Forms.Clipboard]::GetImage() } catch { }
                    if ($null -eq $img) { Start-Sleep -Milliseconds 100 }
                }
                $ms = $t.Elapsed.TotalMilliseconds
                if ($null -eq $img) { return "aucune image ($([int]$ms) ms)" }
                $r = "$ms;$($img.Width);$($img.Height)"
                $img.Dispose()
                return $r
            })
        $h = $ps.BeginInvoke()
        if (-not $h.AsyncWaitHandle.WaitOne(60000)) { $ps.Stop(); return 'délai de 60 s dépassé' }
        $r = @($ps.EndInvoke($h))
        $ps.Dispose()
        return "$($r[-1])"
    }
    finally { $rs.Dispose() }
}

if (Demarrer-Sonde -Nom 's7' -Titre 'S7 : mémoire et taille maximale de l''image finale' -QuotaMo 0) {
    try {
        $tailles = '1000x16384', '2560x9765', '1000x30000'
        $pngs = [System.Collections.Generic.List[string]]::new()
        foreach ($t in $tailles) {
            Section "S7 : $t"
            $f = Fichier-Temp "image-$t.png"
            $r = Jouer-Exemple 'memoire' @('--taille', $t, '--png', "`"$f`"") 180
            if ($r['ok'] -ne '1') { NonMesurable "$t : encodage et copie" "$($r['erreur'])"; continue }
            $pngs.Add($f)
            $est25 = $t -eq '2560x9765'
            $mo = [double]$r.pic_moins_repos_mo
            Resultat 'encodage PNG (en mémoire)' "$($r.encodage_ms) ms ; $('{0:0.0}' -f ([double]$r.png_octets / 1MB)) Mo" 'à lire' ''
            Resultat 'fichier (encodage + écriture)' "$($r.fichier_ms) ms" $(if ($est25) { '< 3 s à 25 Mpx' } else { 'à lire' }) $(if ($est25) { Verdict ([double]$r.fichier_ms -lt 3000) } else { '' })
            if ($r.ContainsKey('copie_ms')) {
                Resultat 'copie (arboard)' "$($r.copie_ms) ms" $(if ($est25) { '< 3 s à 25 Mpx' } else { 'à lire' }) $(if ($est25) { Verdict ([double]$r.copie_ms -lt 3000) } else { '' })
            }
            else { Resultat 'copie (arboard)' "échec : $($r['copie_erreur'])" '< 3 s à 25 Mpx' 'échoue' }
            Resultat 'pic de mémoire privée' ("repos $('{0:0}' -f ([double]$r.prive_repos / 1MB)) Mo, pic $('{0:0}' -f ([double]$r.prive_pic / 1MB)) Mo : + $mo Mo") $(if ($est25) { 'pic < repos + 350 Mo à 25 Mpx' } else { 'à lire' }) $(if ($est25) { Verdict ($mo -lt 350) } else { '' })
            if ($r.ContainsKey('copie_ms')) {
                $l = Relire-PressePapiers
                if ($l -match '^([\d.,]+);(\d+);(\d+)$') {
                    $dim = "$($Matches[2])x$($Matches[3])"
                    Resultat 'relue par un fil STA (Clipboard.GetImage)' "$('{0:0}' -f [double]($Matches[1] -replace ',', '.')) ms, $dim" "$t" (Verdict ($dim -eq $t))
                }
                else { Resultat 'relue par un fil STA' $l "$t" 'échoue' }
            }
        }

        Section 'S7 : aperçu dans une vue Web (WebView2)'
        if ($pngs.Count -eq 0) { NonMesurable 'aperçu WebView2' 'aucune image faite' }
        elseif (-not (Exemple 'controle')) { NonMesurable 'aperçu WebView2' 'exemple controle absent' }
        else {
            $etat = Fichier-Temp 'apercu.txt'
            $p = Lancer-Exemple 'controle' @('--genre', 'apercu', '--etat', "`"$etat`"", '--images', "`"$($pngs -join ';')`"")
            $fini = Attendre -DelaiS 120 -PasMs 250 -Condition { (Lire-Etat $etat).ContainsKey('fin') }
            if (-not $p.WaitForExit(5000)) { try { $p.Kill($true) } catch { } }
            $lignes = @(Get-Content -LiteralPath $etat -ErrorAction SilentlyContinue)
            for ($i = 0; $i -lt $pngs.Count; $i++) {
                $nom = [System.IO.Path]::GetFileNameWithoutExtension($pngs[$i]) -replace '^image-', ''
                $l = @($lignes | Where-Object { $_ -like "apercu $i *" })
                if ($l.Count -eq 0) { NonMesurable "aperçu $nom" $(if ($fini) { 'pas de réponse de la page' } else { 'page pas finie en 120 s' }); continue }
                $ok = $l[0] -match "apercu $i (\d+x\d+) decode_ms ([\d.]+) canvas (\d) canvas_ms ([\d.]+)"
                if ($ok) {
                    Resultat "aperçu $nom" "décodée $($Matches[1]) en $($Matches[2]) ms ; canvas de sa taille : $(if ($Matches[3] -eq '1') { 'dessinée, pixel du bas relu' } else { 'VIDE au bas' }) ($($Matches[4]) ms)" "décodée $nom, canvas dessinée" (Verdict ($Matches[1] -eq $nom -and $Matches[3] -eq '1'))
                }
                else { Resultat "aperçu $nom" ($l[0] -replace '^apercu \d+ ', '') "décodée $nom" 'échoue' }
            }
        }
        NonMesurable 'l''éditeur ouvre 1 000 × 16 384' 'demande l''app elle-même (essai 14, chantier de l''éditeur) ; la canvas ci-dessus en est l''approche'
        NonMesurable 'vignette et historique de l''app' 'demandent l''app ; la vue Web ci-dessus décode les mêmes PNG'
        Note 'Critère : la limite est celle de l''éditeur (D13 : 16 384 de haut, 25 000 000 de pixels) si, à 25 Mpx, copie et fichier finissent en moins de 3 s et le pic reste sous repos + 350 Mo ; sinon la limite baisse jusqu''à tenir. 30 000 px de haut n''est pas retenu (l''éditeur ne l''accepterait pas) : mesuré pour information.'
        Note 'Le pic est celui de l''exemple seul (image, PNG en mémoire, copie d''arboard), pas de l''app entière ; l''app y ajoute l''assemblage et l''aperçu.'
    }
    catch { Erreur-Sonde $_ }
}
Terminer-Sonde
