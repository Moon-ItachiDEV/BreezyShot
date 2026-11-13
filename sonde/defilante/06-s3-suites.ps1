# S3 (contrat 0.6, §1.7) : la sonde ne code pas la mesure du décalage, elle ENREGISTRE
# des suites d'images réelles avec, pour chaque image, le décalage vrai :
#   wpf          témoin WPF, pas d'une demi-vue (UI Automation), décalage lu sur la bande
#   edge-100/125/150  longue.html (en-tête collant, canvas et animation en haut), facteur
#                forcé par --force-device-scale-factor, décalage lu sur la bande (intervalle
#                [bas, haut) en pixels réels : plus large que 1 px quand l'échelle n'est pas 1)
#   explorateur  300 fichiers en Détails, décalage vrai estimé par UI Automation
#   edge-lissage longue.html, un cran de molette puis une image toutes les 33 ms pendant
#                400 ms (images prises pendant le lissage)
# Rend sonde-resultats/s3/<cas>/NNN.png et index.csv (image ; temps_ms ; bande_bas ;
# bande_haut ; uia_pourcent ; decalage_images ; lignes_egales), et la mesure : les lignes de
# deux images successives sont-elles égales au bit près une fois décalées (sans l'en-tête
# collant ni la barre de défilement), et à la tolérance de D12.
#   pwsh -NoProfile -File sonde/defilante/06-s3-suites.ps1

. "$PSScriptRoot/commun.ps1"

if (Demarrer-Sonde -Nom 's3' -Titre 'S3 : suites d''images avec leur décalage vrai' -QuotaMo 24) {
    try {
        $cas = @(
            @{ Nom = 'wpf'; Genre = 'wpf'; E = 1.0; N = 20; Haut = 0 },
            @{ Nom = 'edge-100'; Genre = 'edge'; E = 1.0; N = 15; Haut = 56 },
            @{ Nom = 'edge-125'; Genre = 'edge'; E = 1.25; N = 15; Haut = 70 },
            @{ Nom = 'edge-150'; Genre = 'edge'; E = 1.5; N = 15; Haut = 84 },
            @{ Nom = 'explorateur'; Genre = 'explorateur'; E = 1.0; N = 15; Haut = 30 },
            @{ Nom = 'edge-lissage'; Genre = 'edge'; E = 1.0; N = 48; Haut = 56; Lissage = $true }
        )
        foreach ($k in $cas) {
            Section "S3 : $($k.Nom)"
            $c = $null
            try {
                $c = Ouvrir-Cible $k.Genre -Echelle $k.E
                if ($c.Info) { Note $c.Info }
                if ($k.E -ne 1.0 -and $k.Genre -eq 'edge') {
                    # L'échelle forcée se voit-elle ? Hauteur de l'en-tête collant (56 px CSS) sur la bande.
                    Note "échelle demandée $($k.E) ; zone $(Rect-Texte $c.Zone)"
                }
                $c.Uia = Uia-Defilable $c
                if ($k.ContainsKey('Lissage')) {
                    $pt = Centre $c.Zone
                    $pas = { $script:N::Poser($pt[0], $pt[1]); [void]$script:N::Molette(-1, $script:N::MARQUE) }
                    $images = Suite $k.Nom $c $k.N $pas -Lissage
                }
                else {
                    if ($null -eq $c.Uia) { NonMesurable "$($k.Nom) : suite" 'pas de ScrollPattern (le pas d''une demi-vue passe par UI Automation)'; continue }
                    $pas = { Pas-UIA $c }
                    $images = Suite $k.Nom $c $k.N $pas
                }
                Mesure-Suite $k.Nom $c $images $k.Haut
                if ($k.Genre -eq 'explorateur' -and $null -ne $c.Uia) {
                    $vs = [double]$c.Uia.Motif.Current.VerticalViewSize
                    Note "explorateur : décalage vrai en pixels ≈ pourcent / 100 × hauteur de la vue × (100 / VerticalViewSize − 1), VerticalViewSize = $('{0:0.###}' -f $vs) %, hauteur de la zone $($c.Zone[3]) px (en-têtes de colonnes compris)."
                }
            }
            catch { NonMesurable $k.Nom "$($_.Exception.Message) (ligne $($_.InvocationInfo.ScriptLineNumber))" }
            finally { Fermer-Cible $c }
        }
        Note "Taille des résultats de S3 : $('{0:0.0}' -f ($script:Sonde.Ecrits / 1MB)) Mo (quota 24 Mo ; plafond de la sonde 40 Mo)."
        Note 'Le chantier C en tire ses cas d''essai (quelques dizaines d''images recadrées, 2 Mo au plus dans le dépôt). Critère du chantier C : décalage exact sur 100 % des images WPF et au moins 99 % des images Edge ; 15 ms au plus par mesure pour 1024 × 700 en release.'
    }
    catch { Erreur-Sonde $_ }
}
Terminer-Sonde
