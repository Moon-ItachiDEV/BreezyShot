# S3 (contrat 0.7, §1.3) : le micro.
# - Le service audio tourne-t-il ; les points de capture actifs
#   (IMMDeviceEnumerator::EnumAudioEndpoints(eCapture)) ; GetDefaultAudioEndpoint(eCapture,
#   eConsole) (attendu : introuvable).
# - Micro virtuel : pas installé (voir la note).
# - Sans micro : une sinusoïde de 440 Hz (PCM 48 kHz, mono, 16 bits) pendant 10 s dans la
#   piste AAC du même IMFSinkWriter que la vidéo, 3 s de silence au milieu (3,5 → 6,5 s) ;
#   relue : la piste existe, sa durée égale celle de la vidéo à 50 ms près, la fréquence est
#   retrouvée, le silence est à sa place.
#   pwsh -NoProfile -File sonde/film/05-s3-micro.ps1

. "$PSScriptRoot/commun.ps1"

if (Demarrer-Sonde -Nom '05-s3-micro' -Titre 'S3 : le micro, et la piste son AAC') {
    try {
        Section 'S3 : service audio et points de capture'
        foreach ($s in 'Audiosrv', 'AudioEndpointBuilder') {
            $x = Get-Service -Name $s -ErrorAction SilentlyContinue
            Resultat "Service $s" $(if ($x) { "$($x.Status), démarrage $($x.StartType)" } else { 'absent' }) 'à lire' (OuiNon ($x -and "$($x.Status)" -eq 'Running'))
        }
        $r = Jouer-Exemple 'mf' @('--etape', 'micro') 60
        if ($r['ok'] -ne '1') { NonMesurable 'IMMDeviceEnumerator' "$($r['erreur'])" }
        else {
            foreach ($nom in 'capture', 'rendu') {
                Resultat "Points de $nom actifs (EnumAudioEndpoints, DEVICE_STATE_ACTIVE)" "$($r["$($nom)_actifs"])" 'à lire' 'à lire'
                $tous = @($r.Keys | Where-Object { $_ -match "^$($nom)_\d+$" } | Sort-Object | ForEach-Object { $r[$_] })
                Resultat "Points de $nom, tous états" "$($r["$($nom)_tous_etats"])$(if ($tous.Count) { ' : ' + ($tous -join ' / ') })" 'à lire' 'à lire'
                Resultat "GetDefaultAudioEndpoint($(if ($nom -eq 'capture') { 'eCapture' } else { 'eRender' }), eConsole)" "$($r["$($nom)_defaut"])" $(if ($nom -eq 'capture') { 'attendu : introuvable (supposition)' } else { 'à lire' }) 'à lire'
            }
        }
        Note 'Micro virtuel : non installé. Il n''existe pas de micro virtuel dans Windows Server ; ceux qu''on trouve (VB-Audio Virtual Cable, Virtual Audio Cable, Scream) sont des pilotes de tiers, sous licence propriétaire ou à part, dont l''installation demande de faire confiance à un pilote signé par un tiers : hors des licences permises (MIT, Apache-2.0, BSD, Zlib) et hors de la règle « rien à installer » de la sonde. Abandonné sans l''essayer, comme le contrat le permet (une heure au plus).'
        Note 'Le refus d''accès au micro par les réglages de confidentialité de Windows ne se simule pas ici, et le temps d''ouverture d''un vrai micro ne se mesure pas : liste de tests de l''utilisateur.'

        Section 'S3 : piste son AAC dans le même IMFSinkWriter (440 Hz, 10 s, silence de 3,5 à 6,5 s)'
        $f = Join-Path $script:Sonde.Temp 'son.mp4'
        $r = Jouer-Exemple 'mf' @('--etape', 'son', '--sortie', "`"$f`"") 120
        if ($r['ok'] -ne '1') { NonMesurable 'piste son' "$($r['erreur'])" }
        else {
            Resultat 'Piste son relue' "$($r['piste_son'])" 'présente' (Verdict ("$($r['piste_son'])" -like 'présente*'))
            $e = Nombre $r['ecart_son_video_ms']
            Resultat 'Durée du son et de la vidéo' "son $($r['son_duree_s']) s ($($r['son_echantillons']) échantillons, premier horodatage $($r['son_premier_horodatage_ms']) ms) ; vidéo $($r['video_duree_s']) s ; écart $e ms" 'égales à 50 ms près' (Verdict ([math]::Abs($e) -le 50))
            $fr = Nombre $r['frequence_hz']
            Resultat 'Fréquence retrouvée (passages par zéro, 0,5 → 2,5 s)' "$($r['frequence_hz']) Hz" '440 Hz à 2 % près' (Verdict ([math]::Abs($fr - 440) -le 8.8))
            $si = "$($r['silence_s'])" -split ';'
            Resultat 'Silence (fenêtres de 10 ms sous 1 % de la pleine échelle)' "de $($si[0]) à $($si[1]) s" '3,5 → 6,5 s à 50 ms près' (Verdict ([math]::Abs((Nombre $si[0]) - 3.5) -le 0.05 -and [math]::Abs((Nombre $si[1]) - 6.5) -le 0.05))
            Resultat 'Taille' (Mo $r['taille_octets']) 'à lire' 'à lire'
            Note "Fichier gardé : $(Garder-Fichier $f 's3-son.mp4')"
        }
    }
    catch { Erreur-Sonde $_ }
}
Terminer-Sonde
