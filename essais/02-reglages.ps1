# Étape 2, réglages : fichier absent, abîmé, valeur invalide avec champ inconnu, version plus
# récente, fichier juste (ARCHITECTURE.md §4). Marche aussi sous Linux.

param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$Sortie
)

. "$PSScriptRoot/commun.ps1"
Initialiser-Essai -Nom '02-reglages' -Exe $Exe -Sortie $Sortie | Out-Null

$dossierReglages = Join-Path $script:Essai.Dossier 'reglages'
$fichier = Join-Path $dossierReglages 'reglages.json'
$utf8 = [System.Text.UTF8Encoding]::new($false)
# Nombre de démarrages complets déjà vus (ligne « pret » de l'Application).
$script:Demarrages = 0

function Ecrire-Reglages([string]$Texte) {
    New-Item -ItemType Directory -Force -Path $dossierReglages | Out-Null
    [System.IO.File]::WriteAllText($fichier, $Texte, $utf8)
}

function Empreinte([string]$Chemin) {
    return (Get-FileHash -LiteralPath $Chemin -Algorithm SHA256).Hash
}

# Un lancement complet : démarre, attend « pret », ferme par --quitter. Échoue clairement.
function Cycle([string]$Quoi) {
    $p = Demarrer-App
    try {
        $script:Demarrages++
        if (-not (Attendre-Ligne -Texte 'application pret' -Fois $script:Demarrages -DelaiS 30)) {
            throw "$Quoi : l'app n'a pas fini de démarrer en 30 s"
        }
        $q = Quitter-App -DelaiS 10
        if (-not $q.Fini) { throw "$Quoi : --quitter n'a pas répondu en 10 s" }
        if (-not $p.WaitForExit(10000)) { throw "$Quoi : l'app ne s'est pas fermée en 10 s" }
    }
    finally {
        if (-not $p.HasExited) { try { $p.Kill() } catch { } }
    }
}

try {
    # 1. Fichier absent : créé avec les valeurs par défaut.
    Cycle 'fichier absent'
    Verifier 'fichier absent : reglages.json créé' (Test-Path -LiteralPath $fichier) | Out-Null
    $lu = Get-Content -LiteralPath $fichier -Raw | ConvertFrom-Json
    $attenduZone = if ($IsMacOS) { 'Cmd+Maj+1' } else { 'Ctrl+Maj+1' }
    $attenduDerniere = if ($IsMacOS) { 'Cmd+Maj+7' } else { 'Ctrl+Maj+4' }
    Verifier 'valeurs par défaut : version 12' ($lu.version -eq 12) "version $($lu.version)" | Out-Null
    # 0.8.0 : Impr. écran n'est plus dans la liste de la zone, la cible est un champ à part.
    Verifier 'valeurs par défaut : Impr. écran ouvre la zone (0.8.0)' ($lu.impr_ecran_ouvre -eq 'zone') "impr_ecran_ouvre $($lu.impr_ecran_ouvre)" | Out-Null
    Verifier 'valeurs par défaut : Win + Maj + S laissé à Windows' ($null -eq $lu.win_maj_s_ouvre) "win_maj_s_ouvre $($lu.win_maj_s_ouvre)" | Out-Null
    # 0.7.7 : format PNG, éditeur non, film à 30 i/s sans son ni clics, 50 captures gardées.
    Verifier 'valeurs par défaut : format, éditeur, film et historique (0.7.7)' (($lu.format_image -eq 'png') -and ($lu.editeur_apres_capture -eq $false) -and ($lu.film.fluidite -eq 30) -and ($lu.film.son_systeme -eq $false) -and ($lu.film.clics_visibles -eq $false) -and ($null -eq $lu.film.dossier) -and ($lu.historique.gardes -eq 50)) "format $($lu.format_image), editeur $($lu.editeur_apres_capture), film $($lu.film | ConvertTo-Json -Compress), historique $($lu.historique.gardes)" | Out-Null
    Verifier 'valeurs par défaut : raccourcis de zone' (($lu.raccourcis.zone -join ',') -eq $attenduZone) ($lu.raccourcis.zone -join ',') | Out-Null
    Verifier 'valeurs par défaut : raccourci de la dernière zone' (($lu.raccourcis.derniere_zone -join ',') -eq $attenduDerniere) ($lu.raccourcis.derniere_zone -join ',') | Out-Null
    $attenduPipette = if ($IsMacOS) { 'Cmd+Maj+C' } else { 'Ctrl+Maj+C' }
    $pipette = if ($lu.raccourcis.PSObject.Properties.Name -contains 'pipette') { @($lu.raccourcis.pipette) -join ',' } else { '<absent>' }
    Verifier 'valeurs par défaut : raccourci de la pipette' ($pipette -eq $attenduPipette) $pipette | Out-Null
    $attenduHistorique = if ($IsMacOS) { 'Cmd+Maj+8' } else { 'Ctrl+Maj+H' }
    $attenduTexte = if ($IsMacOS) { 'Cmd+Maj+9' } else { 'Ctrl+Maj+T' }
    Verifier 'valeurs par défaut : raccourcis de l''historique et du texte' ((($lu.raccourcis.historique -join ',') -eq $attenduHistorique) -and (($lu.raccourcis.texte -join ',') -eq $attenduTexte)) "$($lu.raccourcis.historique -join ',') ; $($lu.raccourcis.texte -join ',')" | Out-Null
    $attenduDefilante = if ($IsMacOS) { 'Cmd+Maj+0' } else { 'Ctrl+Maj+5' }
    $defilante = if ($lu.raccourcis.PSObject.Properties.Name -contains 'defilante') { $lu.raccourcis.defilante -join ',' } else { '<absent>' }
    Verifier 'valeurs par défaut : raccourci de la capture défilante' ($defilante -eq $attenduDefilante) $defilante | Out-Null
    # 0.7 : vidéo et GIF (aucun raccourci sur le Mac, où le film n'existe pas).
    $attenduVideo = if ($IsMacOS) { '' } else { 'Ctrl+Maj+6' }
    $attenduGif = if ($IsMacOS) { '' } else { 'Ctrl+Maj+7' }
    $video = if ($lu.raccourcis.PSObject.Properties.Name -contains 'video') { @($lu.raccourcis.video) -join ',' } else { '<absent>' }
    $gif = if ($lu.raccourcis.PSObject.Properties.Name -contains 'gif') { @($lu.raccourcis.gif) -join ',' } else { '<absent>' }
    Verifier 'valeurs par défaut : raccourcis de la vidéo et du GIF' (($video -eq $attenduVideo) -and ($gif -eq $attenduGif)) "$video ; $gif" | Out-Null
    Verifier 'valeurs par défaut : capture sans retardateur, pointeur ni masquage du bureau' (($null -ne $lu.capture) -and ($lu.capture.retardateur_s -eq 0) -and ($lu.capture.pointeur -eq $false) -and ($lu.capture.masquer_icones_bureau -eq $false)) ($lu.capture | ConvertTo-Json -Compress) | Out-Null
    Verifier 'valeurs par défaut : vignette 6 s en bas à gauche' (($lu.vignette.duree_s -eq 6) -and ($lu.vignette.coin -eq 'bas_gauche')) | Out-Null
    Verifier 'ligne « reglages crees »' ((Compter-Lignes -Texte 'reglages crees avec les valeurs par defaut') -eq 1) | Out-Null
    $defaut = [System.IO.File]::ReadAllText($fichier, $utf8)

    # 2. Fichier juste : identique octet pour octet après un lancement.
    $avant = Empreinte $fichier
    Cycle 'fichier juste'
    Verifier 'fichier juste : identique octet pour octet' ((Empreinte $fichier) -eq $avant) | Out-Null

    # 3. Fichier abîmé : mis de côté sous reglages.abime-*.json, recréé, ligne au journal.
    $abime = '{ "version": 1, pas du json'
    Ecrire-Reglages $abime
    Cycle 'fichier abîmé'
    $sauvegardes = @(Get-ChildItem -LiteralPath $dossierReglages -Filter 'reglages.abime-*.json')
    Verifier 'fichier abîmé : une sauvegarde reglages.abime-*.json' ($sauvegardes.Count -eq 1) "$($sauvegardes.Count) fichier(s)" | Out-Null
    if ($sauvegardes.Count -ge 1) {
        $contenu = [System.IO.File]::ReadAllText($sauvegardes[0].FullName, $utf8)
        Verifier 'la sauvegarde garde le contenu abîmé' ($contenu -eq $abime) | Out-Null
    }
    $nouveau = [System.IO.File]::ReadAllText($fichier, $utf8)
    Verifier 'fichier abîmé : nouveau fichier par défaut' ($nouveau -eq $defaut) | Out-Null
    Verifier 'ligne « reglages illisibles »' ((Compter-Lignes -Texte 'reglages illisibles') -eq 1) | Out-Null

    # 4. Valeur invalide corrigée, champ inconnu conservé.
    $objet = $defaut | ConvertFrom-Json
    $objet.vignette.duree_s = 999
    $objet | Add-Member -NotePropertyName 'champ_futur' -NotePropertyValue ([pscustomobject]@{ garde = 'oui'; n = 3 })
    Ecrire-Reglages (($objet | ConvertTo-Json -Depth 10) + "`n")
    Cycle 'valeur invalide'
    $corrige = Get-Content -LiteralPath $fichier -Raw | ConvertFrom-Json
    Verifier 'valeur invalide : durée remise à 6 s' ($corrige.vignette.duree_s -eq 6) "duree_s $($corrige.vignette.duree_s)" | Out-Null
    $garde = ($corrige.PSObject.Properties.Name -contains 'champ_futur') -and ($corrige.champ_futur.garde -eq 'oui') -and ($corrige.champ_futur.n -eq 3)
    Verifier 'champ inconnu conservé' $garde | Out-Null
    Verifier 'ligne « valeur invalide pour vignette.duree_s »' ((Compter-Lignes -Texte 'valeur invalide pour vignette.duree_s') -eq 1) | Out-Null

    # 5. Version plus récente : lue au mieux, fichier identique octet pour octet.
    $futur = $defaut | ConvertFrom-Json
    $futur.version = 99
    $futur | Add-Member -NotePropertyName 'nouveaute' -NotePropertyValue 'v99'
    Ecrire-Reglages (($futur | ConvertTo-Json -Depth 10) + "`n")
    $avant = Empreinte $fichier
    Cycle 'version plus récente'
    Verifier 'version plus récente : identique octet pour octet' ((Empreinte $fichier) -eq $avant) | Out-Null
    Verifier 'ligne « plus recente que la version 12 connue »' ((Compter-Lignes -Texte 'plus recente que la version 12 connue') -eq 1) | Out-Null

    Verifier 'cinq démarrages complets' ($script:Demarrages -eq 5) | Out-Null
}
catch {
    Verifier "déroulement de l'essai" $false "$_" | Out-Null
}
finally {
    Terminer-Essai
}
