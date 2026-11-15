# Étape 1, socle : démarrage, échelle par écran, icône, instance unique (la relance ouvre la
# barre de capture), --quitter.

param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$Sortie
)

. "$PSScriptRoot/commun.ps1"
Initialiser-Essai -Nom '01-socle' -Exe $Exe -Sortie $Sortie | Out-Null

try {
    # 0. Sous Windows : le manifeste (Per-Monitor V2) est bien intégré au .exe.
    if ($IsWindows) {
        $octets = [System.IO.File]::ReadAllBytes($script:Essai.Exe)
        $integre = [System.Text.Encoding]::ASCII.GetString($octets).Contains('PerMonitorV2') -or
            [System.Text.Encoding]::Unicode.GetString($octets).Contains('PerMonitorV2')
        Verifier 'manifeste PerMonitorV2 intégré au .exe' $integre | Out-Null
    }

    # 1. L'app démarre et écrit son journal.
    $premiere = Demarrer-App
    $demarree = Verifier "l'app démarre" (Attendre-Ligne -Texte 'demarrage BreezyShot ' -DelaiS 30)
    if (-not $demarree) { throw "l'app n'a pas démarré" }

    # 2. Conscience d'échelle par écran v2 (Windows) ; sans objet ailleurs.
    $attendu = if ($IsWindows) { 'echelle par ecran v2: oui' } else { 'echelle par ecran v2: sans objet' }
    Verifier "ligne « $attendu »" (Attendre-Ligne -Texte $attendu -DelaiS 10) | Out-Null

    # 3. Icône dans la zone de notification.
    Verifier 'icone installee' (Attendre-Ligne -Texte 'icone installee' -DelaiS 15) | Out-Null
    # « visible » se trouve dans « icone visible » comme dans « icone invisible ».
    $null = Attendre-Ligne -Texte 'visible' -DelaiS 10
    $visible = (Compter-Lignes -Texte 'icone visible') -ge 1
    if ($IsWindows) {
        Verifier "icone visible (dans la zone de notification)" $visible | Out-Null
    }
    else {
        Write-Host "[note]  icone visible : $visible (non exigé hors Windows)"
    }
    Verifier 'démarrage complet (pret)' (Attendre-Ligne -Texte ' pret' -FinDeLigne -DelaiS 15) | Out-Null

    # 4. Mémoire privée au repos, après quelques secondes sans rien faire.
    Start-Sleep -Seconds 5
    Noter-Mesure -Nom 'mémoire privée au repos' -Valeur (Memoire-Privee-Mo -Processus $premiere) -Unite 'Mo'

    # 5. Une seconde instance sort avec 0 en moins de 10 s ; la première ouvre la barre de
    # capture ; un clic sur « Zone » ouvre la sélection ; un seul processus.
    $seconde = Lancer-Et-Attendre -DelaiS 10 -LaisserVivant
    # Compter tout de suite, avant de tuer quoi que ce soit : une seconde instance restée
    # en vie au-delà du délai compte comme un second processus.
    $restants = @(Processus-App)
    Verifier 'seconde instance sortie en moins de 10 s' $seconde.Fini "$($seconde.DureeS) s" | Out-Null
    Verifier 'seconde instance : code de sortie 0' ($seconde.Code -eq 0) "code $($seconde.Code)" | Out-Null
    Verifier 'la première écrit « seconde instance: barre de capture »' (Attendre-Ligne -Texte 'seconde instance: barre de capture' -DelaiS 5) | Out-Null
    if ($IsWindows) {
        # La barre montrée, ses boutons lus ; sans licence, rien de Pro.
        $barre = Attendre-Barre -Dossier $script:Essai.Dossier -Quoi 'seconde instance'
        $M = [BreezyShotEssai.MenuIcone]
        Verifier 'la seconde instance ouvre la barre de capture' ($M::IsWindowVisible($barre.Fenetre)) "boutons : $($barre.Noms -join ', ')" | Out-Null
        $attendus = @('Zone', 'Plein écran', 'Fenêtre', 'Retardateur', 'Réglages')
        Verifier 'barre : Zone, Plein écran, Fenêtre, Retardateur, Réglages (sans licence)' (($barre.Noms -join '|') -eq ($attendus -join '|')) ($barre.Noms -join ', ') | Out-Null
        # Un clic sur « Zone » : la barre se ferme, la sélection s'ouvre (après ATTENTE_MENU).
        Cliquer-Bouton-Barre $barre 'Zone' 'seconde instance'
        Verifier 'clic sur Zone : « barre choisi capturer_zone »' (Attendre-Ligne -Texte 'barre choisi capturer_zone' -DelaiS 5) | Out-Null
        Verifier 'la barre se ferme au choix' (Attendre-Ligne -Texte 'barre fermee' -FinDeLigne -DelaiS 5) | Out-Null
        Verifier 'puis « menu: capturer zone »' (Attendre-Ligne -Texte 'menu: capturer zone' -DelaiS 5) | Out-Null
        # La sélection vraiment montrée (pas « selection montree … impossible »), puis Échap
        # la referme et l'app revient au repos avant la suite.
        $montree = $false; $repos = $false
        $limite = [DateTime]::UtcNow.AddSeconds(10)
        do {
            $montree = (Lire-Journal) -match 'selection montree \d+ voulu'
            if (-not $montree) { Start-Sleep -Milliseconds 100 }
        } while (-not $montree -and [DateTime]::UtcNow -lt $limite)
        Verifier 'Zone ouvre la sélection et sa barre' $montree | Out-Null
        Verifier 'la barre de capture n''est plus montrée' (-not $M::IsWindowVisible($barre.Fenetre)) | Out-Null
        $reposAvant = Compter-Lignes -Texte 'etat Selection -> Repos'
        Start-Sleep -Milliseconds 300
        $M::Touche(0x1B)
        $repos = Attendre-Ligne -Texte 'etat Selection -> Repos' -Fois ($reposAvant + 1) -DelaiS 5
        Verifier 'Échap referme la sélection (retour au repos)' $repos | Out-Null

        # Relancée de nouveau : la barre se rouvre ; Échap la ferme sans rien lancer.
        $encore = Lancer-Et-Attendre -DelaiS 10
        Verifier 'troisième lancement : code de sortie 0' ($encore.Fini -and $encore.Code -eq 0) "code $($encore.Code)" | Out-Null
        $barre = Attendre-Barre -Dossier $script:Essai.Dossier -Quoi 'troisième lancement' -Fois 2
        $fermees = Compter-Lignes -Texte 'barre fermee' -FinDeLigne
        # Un clic sur le logo (aucune action) lui donne le clavier et la laisse ouverte. Le
        # bouton « Zone » mesure 36 px logiques ; le logo est centré 18 px à sa gauche, la
        # carte finit 5 px sous lui, et la marge de l'ombre 16 px plus bas.
        $r = $barre.Boutons['Zone'].Current.BoundingRectangle
        $k = $r.Height / 36
        $x = [int]($r.X - 18 * $k); $y = [int]($r.Y + $r.Height / 2)
        if ($M::RacineSous($x, $y) -ne $barre.Fenetre) { throw "troisième lancement : le logo de la barre n'est pas sous le point $x,$y" }
        $M::Cliquer($x, $y)
        Start-Sleep -Milliseconds 300
        Verifier 'un clic sur le logo laisse la barre ouverte' (($M::IsWindowVisible($barre.Fenetre)) -and ((Compter-Lignes -Texte 'barre fermee' -FinDeLigne) -eq $fermees)) | Out-Null
        $limite = [DateTime]::UtcNow.AddSeconds(1)
        while ($M::GetForegroundWindow() -ne $barre.Fenetre -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 30 }
        if ($M::GetForegroundWindow() -eq $barre.Fenetre) {
            $M::Touche(0x1B)
            Verifier 'Échap ferme la barre' (Attendre-Ligne -Texte 'barre fermee' -FinDeLigne -Fois ($fermees + 1) -DelaiS 5) | Out-Null
        }
        else {
            # Windows a refusé le premier plan : jamais de touche hors des fenêtres de l'app.
            # Un clic hors de la carte (dans la marge de son ombre, au-dessous d'elle) la
            # ferme aussi.
            Write-Host "[note]  barre pas au premier plan : fermée par un clic dans sa marge"
            $x = [int]($r.X + $r.Width / 2); $y = [int]($r.Y + $r.Height + 12 * $k)
            if ($M::RacineSous($x, $y) -ne $barre.Fenetre) { throw "troisième lancement : la marge de la barre n'est pas sous le point $x,$y" }
            $M::Cliquer($x, $y)
            Verifier 'un clic hors de la carte ferme la barre' (Attendre-Ligne -Texte 'barre fermee' -FinDeLigne -Fois ($fermees + 1) -DelaiS 5) | Out-Null
        }
        Verifier 'rien de lancé : un seul « barre choisi »' ((Compter-Lignes -Texte 'barre choisi') -eq 1) | Out-Null
    }
    Verifier 'un seul processus BreezyShot' ($restants.Count -eq 1) "$($restants.Count) processus" | Out-Null
    if (-not $seconde.Fini) { try { $seconde.Processus.Kill() } catch { } }
    Verifier 'la première instance tourne toujours' (-not $premiere.HasExited) | Out-Null

    # 6. --quitter ferme la première en moins de 10 s, avec « arret propre ».
    $chrono = [System.Diagnostics.Stopwatch]::StartNew()
    $quitter = Quitter-App -DelaiS 10
    Verifier '--quitter : code de sortie 0' ($quitter.Fini -and $quitter.Code -eq 0) "code $($quitter.Code), $($quitter.DureeS) s" | Out-Null
    $reste = [math]::Max(0, 10000 - [int]$chrono.ElapsedMilliseconds)
    $fermee = $premiere.WaitForExit($reste)
    $chrono.Stop()
    Verifier 'la première instance se ferme en moins de 10 s' $fermee "$([math]::Round($chrono.Elapsed.TotalSeconds, 2)) s" | Out-Null
    if ($fermee) {
        $premiere.WaitForExit()
        Verifier 'la première instance sort avec le code 0' ($premiere.ExitCode -eq 0) "code $($premiere.ExitCode)" | Out-Null
    }
    Verifier 'ligne « arret propre »' (Attendre-Ligne -Texte 'arret propre' -DelaiS 3) | Out-Null

    # 7. --quitter sans instance en cours : sort vite avec 0.
    Start-Sleep -Milliseconds 500
    Verifier 'plus aucun processus BreezyShot' (@(Processus-App).Count -eq 0) | Out-Null
    $seul = Quitter-App -DelaiS 10
    Verifier '--quitter sans instance : sort en moins de 5 s' ($seul.Fini -and $seul.DureeS -lt 5) "$($seul.DureeS) s" | Out-Null
    Verifier '--quitter sans instance : code de sortie 0' ($seul.Code -eq 0) "code $($seul.Code)" | Out-Null
    Verifier 'une seule ligne « arret propre » au total' ((Compter-Lignes -Texte 'arret propre') -eq 1) | Out-Null
}
catch {
    Verifier "déroulement de l'essai" $false "$_" | Out-Null
}
finally {
    Terminer-Essai
}
