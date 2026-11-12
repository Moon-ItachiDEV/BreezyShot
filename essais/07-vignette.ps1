# Étape 9 : la vignette, de bout en bout (ARCHITECTURE.md §11, contrat de l'étape 9 §10.1),
# sur le vrai .exe, avec de vraies entrées clavier et souris. Réglages écrits par l'essai :
# plein écran sur Ctrl+Maj+2, copie, pas d'enregistrement, vignette 3 s en bas à gauche,
# animations normales.
#
# Cas A (BREEZYSHOT_ESSAI_CAPTURABLE=1) : (1) démarrage : fenêtres de la vignette et du
# souffle créées, cachées, styles outil sans APPWINDOW, pages prêtes. (10) Glisser d'abord :
# une fenêtre cible (cible-glisser.ps1) reçoit exactement un fichier nommé au modèle, mêmes
# octets que « Récents » ; glisser tenu plus que la durée puis Échap : annulé, la vignette
# reste ; lâché sur la carte : annulé. (2) Capture : Decoupe -> Souffle, souffle vu visible
# puis caché, Souffle -> Vignette, carte au coin bas gauche de la zone utile à 12 px, aucun
# bandeau, presse-papiers, un PNG dans « Récents », aucun dans Images, premier plan
# inchangé. (3) WindowFromPoint : la vignette sur la carte, jamais à côté. (4) Délai de 3 s,
# carte peinte (copie d'écran différente après son départ). (5) Pause au survol. (6) Copier.
# (7) Enregistrer, puis « deja fait ». (8) Fermer. (9) Supprimer puis Annuler ; Supprimer
# après Enregistrer : les deux fichiers effacés après 4 s, rien d'autre. (11) Aucune
# fenêtre BreezyShot dans la photo suivante. (13) Seconde capture pendant le souffle
# ignorée. (15) Fenêtres et fils stables après 10 vignettes, mémoire notée. (16) --quitter
# avec une suppression en attente : fichier effacé, arret propre.
# Cas B (sans la variable) : (12) exclusion réelle des captures.
# Cas C, D, E : (14) réglages : vignette coupée, coin haut droit, animations réduites.
#
# Ce que la machine GitHub ne peut pas vérifier : plusieurs écrans et échelles mêlées, le
# dépôt dans une vraie appli (chat, mail), la durée « toujours » (tests purs), le rendu.

param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$Sortie
)

. "$PSScriptRoot/commun.ps1"
Initialiser-Essai -Nom '07-vignette' -Exe $Exe -Sortie $Sortie | Out-Null

$script:Auxiliaires = [System.Collections.Generic.List[object]]::new()
$script:Pwsh = (Get-Process -Id $PID).Path

# --- Aides (même forme que les essais 04 à 06) -------------------------------------------

function Dossier-Cas([string]$Nom) {
    $d = Join-Path $script:Essai.Dossier $Nom
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    return $d
}

function Lignes-Journal([string]$Dossier) {
    return @((Lire-Journal -Dossier $Dossier) -split "`n" | ForEach-Object { $_.TrimEnd("`r") } | Where-Object { $_ -ne '' })
}

function Derniere-Ligne([string]$Motif, [string]$Dossier) {
    $trouvee = $null
    foreach ($l in (Lignes-Journal $Dossier)) { if ($l -match $Motif) { $trouvee = $l } }
    return $trouvee
}

function Compter-Motif([string]$Motif, [string]$Dossier) {
    $n = 0
    foreach ($l in (Lignes-Journal $Dossier)) { if ($l -match $Motif) { $n++ } }
    return $n
}

function Attendre-Motif([string]$Motif, [string]$Dossier, [double]$DelaiS, [string]$Quoi, [int]$Fois = 1) {
    $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
    do {
        if ((Compter-Motif -Motif $Motif -Dossier $Dossier) -ge $Fois) {
            return (Derniere-Ligne -Motif $Motif -Dossier $Dossier)
        }
        Start-Sleep -Milliseconds 30
    } while ([DateTime]::UtcNow -lt $limite)
    $vu = Compter-Motif -Motif $Motif -Dossier $Dossier
    throw "$Quoi : rien dans le journal en $DelaiS s (motif « $Motif », attendu $Fois fois, vu $vu)"
}

function Lignes-Depuis([string]$Dossier, [int]$Debut) {
    $toutes = @(Lignes-Journal $Dossier)
    if ($Debut -ge $toutes.Count) { return @() }
    return @($toutes[$Debut..($toutes.Count - 1)])
}

function Fichiers-Png([string]$Dossier) {
    if (-not (Test-Path -LiteralPath $Dossier)) { return @() }
    return @(Get-ChildItem -LiteralPath $Dossier -File -Filter '*.png' | Sort-Object LastWriteTimeUtc, Name)
}

# Réglages du cas (version 2 du fichier, complète : rien à réécrire ; durée « toujours » non essayée ici).
function Ecrire-Reglages([string]$Dossier, [bool]$Afficher = $true, [string]$Coin = 'bas_gauche', [string]$Animations = 'normales', $DossierEnregistrement = $null) {
    $r = [ordered]@{
        version                   = 2
        raccourcis                = [ordered]@{ zone = @(); plein_ecran = @('Ctrl+Maj+2'); fenetre = @() }
        copier_apres_capture      = $true
        enregistrer_apres_capture = $false
        dossier_enregistrement    = $DossierEnregistrement
        vignette                  = [ordered]@{ afficher = $Afficher; duree_s = 3; coin = $Coin }
        animations                = $Animations
        selection                 = [ordered]@{ loupe = $false }
        demarrer_avec_systeme     = $false
        modele_nom                = 'BreezyShot {date} à {heure}'
        theme                     = 'systeme'
    }
    $d = Join-Path $Dossier 'reglages'
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    $r | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $d 'reglages.json') -Encoding utf8
}

function Demarrer-Cas([string]$Dossier, [string]$Quoi, [bool]$Capturable = $true) {
    if ($Capturable) { $env:BREEZYSHOT_ESSAI_CAPTURABLE = '1' }
    try { $proc = Demarrer-App -Dossier $Dossier }
    finally { Remove-Item Env:BREEZYSHOT_ESSAI_CAPTURABLE -ErrorAction SilentlyContinue }
    $null = Attendre-Motif -Motif 'application +pret$' -Dossier $Dossier -DelaiS 30 -Quoi "$Quoi : démarrage"
    $null = Attendre-Motif -Motif 'bandeau pret$' -Dossier $Dossier -DelaiS 20 -Quoi "$Quoi : page du bandeau"
    $null = Attendre-Motif -Motif 'vignette prete$' -Dossier $Dossier -DelaiS 20 -Quoi "$Quoi : page de la vignette"
    $null = Attendre-Motif -Motif 'souffle pret$' -Dossier $Dossier -DelaiS 20 -Quoi "$Quoi : page du souffle"
    return $proc
}

function Fermer-Cas($Processus, [string]$Dossier, [string]$Quoi) {
    $arrets = Compter-Motif -Motif 'arret propre$' -Dossier $Dossier
    $q = Quitter-App -DelaiS 10 -Dossier $Dossier
    if (-not $q.Fini) { throw "$Quoi : --quitter n'a pas répondu en 10 s" }
    if (-not $Processus.WaitForExit(10000)) { throw "$Quoi : l'app ne s'est pas fermée en 10 s" }
    $null = Attendre-Motif -Motif 'arret propre$' -Dossier $Dossier -DelaiS 3 -Quoi "$Quoi : arrêt" -Fois ($arrets + 1)
    Verifier "$Quoi : --quitter → arret propre" $true | Out-Null
}

try {
    if (-not $IsWindows) {
        # --- Hors Windows : démarrage seulement --------------------------------------------
        $commun = Dossier-Cas 'commun'
        Ecrire-Reglages -Dossier $commun
        $proc = Demarrer-Cas -Dossier $commun -Quoi 'commun'
        Verifier '(1) pages de la vignette et du souffle prêtes' $true | Out-Null
        Fermer-Cas $proc $commun 'commun'
        Write-Host '[saute] (2) à (16) : sautés hors Windows'
    }
    else {
        Add-Type -Namespace BreezyShotEssai -Name Vignette -MemberDefinition @'
[DllImport("user32.dll")]
static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
[DllImport("user32.dll")]
static extern uint MapVirtualKey(uint code, uint type);
[DllImport("user32.dll")]
static extern void mouse_event(uint flags, int dx, int dy, uint data, UIntPtr extra);
[DllImport("user32.dll")]
public static extern bool SetCursorPos(int x, int y);
[DllImport("user32.dll", SetLastError = true)]
public static extern bool SetProcessDpiAwarenessContext(IntPtr contexte);
[DllImport("user32.dll")]
public static extern bool IsWindowVisible(IntPtr h);
[DllImport("user32.dll")]
public static extern uint GetClipboardSequenceNumber();
[DllImport("user32.dll")]
public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")]
public static extern IntPtr GetWindowLongPtr(IntPtr h, int index);
[StructLayout(LayoutKind.Sequential)]
public struct POINT { public int X; public int Y; }
[DllImport("user32.dll")]
static extern IntPtr WindowFromPoint(POINT p);
[DllImport("user32.dll")]
static extern IntPtr GetAncestor(IntPtr h, uint quoi);
[StructLayout(LayoutKind.Sequential)]
public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
[DllImport("user32.dll")]
public static extern bool GetWindowRect(IntPtr h, out RECT r);
[DllImport("user32.dll")]
static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
delegate bool Rappel(IntPtr h, IntPtr p);
[DllImport("user32.dll")]
static extern bool EnumWindows(Rappel f, IntPtr p);

const uint RELACHEE = 0x2;
static void Bas(byte vk) { keybd_event(vk, (byte)MapVirtualKey(vk, 0), 0u, UIntPtr.Zero); }
static void Haut(byte vk) { keybd_event(vk, (byte)MapVirtualKey(vk, 0), RELACHEE, UIntPtr.Zero); }
public static void Appuyer(byte[] mods, byte touche) {
    foreach (var m in mods) Bas(m);
    Bas(touche);
    System.Threading.Thread.Sleep(30);
    Haut(touche);
    for (int i = mods.Length - 1; i >= 0; i--) Haut(mods[i]);
}
// Appuie en guettant la fenêtre donnée toutes les 2 ms pendant dureeMs : rend le nombre de
// relevés où elle était visible.
public static int AppuyerEnGuettant(byte[] mods, byte touche, IntPtr guettee, int dureeMs) {
    int vues = 0;
    bool fin = false;
    var guet = new System.Threading.Thread(() => {
        while (!System.Threading.Volatile.Read(ref fin)) {
            if (IsWindowVisible(guettee)) System.Threading.Interlocked.Increment(ref vues);
            System.Threading.Thread.Sleep(2);
        }
    });
    guet.IsBackground = true;
    guet.Start();
    System.Threading.Thread.Sleep(20);
    Appuyer(mods, touche);
    System.Threading.Thread.Sleep(dureeMs);
    System.Threading.Volatile.Write(ref fin, true);
    guet.Join();
    return vues;
}
// Pose le curseur et envoie un vrai mouvement d'entrée à cet endroit.
public static void Poser(int x, int y) {
    SetCursorPos(x, y);
    mouse_event(0x1, 0, 0, 0, UIntPtr.Zero);
}
public static void BoutonBas() { mouse_event(0x2, 0, 0, 0, UIntPtr.Zero); }
public static void BoutonHaut() { mouse_event(0x4, 0, 0, 0, UIntPtr.Zero); }
public static void Cliquer(int x, int y) {
    Poser(x, y);
    System.Threading.Thread.Sleep(40);
    BoutonBas();
    System.Threading.Thread.Sleep(60);
    BoutonHaut();
}
// Déplace le curseur de (x0, y0) à (x1, y1) en $pas étapes de $ms ms.
public static void Deplacer(int x0, int y0, int x1, int y1, int pas, int ms) {
    for (int i = 1; i <= pas; i++) {
        Poser(x0 + (x1 - x0) * i / pas, y0 + (y1 - y0) * i / pas);
        System.Threading.Thread.Sleep(ms);
    }
}
public static void Touche(byte vk) { Bas(vk); System.Threading.Thread.Sleep(30); Haut(vk); }
// La fenêtre racine sous le point.
public static IntPtr RacineSous(int x, int y) {
    var p = new POINT { X = x, Y = y };
    var h = WindowFromPoint(p);
    return h == IntPtr.Zero ? IntPtr.Zero : GetAncestor(h, 2);
}
public static IntPtr[] FenetresDe(int[] pids) {
    var liste = new System.Collections.Generic.List<IntPtr>();
    var ensemble = new System.Collections.Generic.HashSet<int>(pids);
    EnumWindows((h, p) => {
        uint pid; GetWindowThreadProcessId(h, out pid);
        if (ensemble.Contains((int)pid)) liste.Add(h);
        return true;
    }, IntPtr.Zero);
    return liste.ToArray();
}
'@
        try { [void][BreezyShotEssai.Vignette]::SetProcessDpiAwarenessContext([IntPtr]::new(-4)) } catch { }
        Add-Type -AssemblyName System.Drawing
        Add-Type -AssemblyName System.Windows.Forms
        $V = [BreezyShotEssai.Vignette]

        $MODS = [byte[]]@(0x11, 0x10)   # Ctrl, Maj
        $TOUCHE_2 = [byte]0x32
        $ECHAP = [byte]0x1B
        $utile = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
        $principal = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
        Write-Host "écran principal $principal ; zone utile $utile"
        # La souris au repos : au milieu de la zone utile, loin de la carte et de la cible.
        $REPOS_X = [int]($utile.X + $utile.Width / 2)
        $REPOS_Y = [int]($utile.Y + $utile.Height / 2)
        $MODELE = '^BreezyShot \d{4}-\d\d-\d\d à \d\d\.\d\d\.\d\d( \(\d+\))?\.png$'

        function Hwnd-Journal([string]$Motif, [string]$Dossier) {
            $l = Attendre-Motif -Motif "$Motif (\d+)$" -Dossier $Dossier -DelaiS 5 -Quoi $Motif
            if ($l -notmatch "$Motif (\d+)$") { throw "ligne illisible : $l" }
            return [IntPtr][long]$Matches[1]
        }

        # « … <x>,<y> <l>x<h> » en fin de ligne → { X, Y, L, H, CX, CY }.
        function Lire-Rect([string]$Ligne) {
            if ($Ligne -notmatch '(-?\d+),(-?\d+) (\d+)x(\d+)$') { throw "rectangle illisible : $Ligne" }
            $r = [pscustomobject]@{ X = [int]$Matches[1]; Y = [int]$Matches[2]; L = [int]$Matches[3]; H = [int]$Matches[4] }
            $r | Add-Member CX ([int]($r.X + $r.L / 2))
            $r | Add-Member CY ([int]($r.Y + $r.H / 2))
            return $r
        }

        function Copie-Ecran($R) {
            $b = [System.Drawing.Bitmap]::new($R.L, $R.H)
            $g = [System.Drawing.Graphics]::FromImage($b)
            try { $g.CopyFromScreen($R.X, $R.Y, 0, 0, [System.Drawing.Size]::new($R.L, $R.H)) }
            finally { $g.Dispose() }
            return $b
        }

        # Part des pixels (pris sur une grille) qui diffèrent de plus de $Seuil par canal.
        function Part-Differente($A, $B, [int]$Seuil = 6) {
            $n = 0; $d = 0
            for ($y = 2; $y -lt [math]::Min($A.Height, $B.Height) - 2; $y += 4) {
                for ($x = 2; $x -lt [math]::Min($A.Width, $B.Width) - 2; $x += 4) {
                    $p = $A.GetPixel($x, $y); $q = $B.GetPixel($x, $y); $n++
                    if ([math]::Abs($p.R - $q.R) -gt $Seuil -or [math]::Abs($p.G - $q.G) -gt $Seuil -or [math]::Abs($p.B - $q.B) -gt $Seuil) { $d++ }
                }
            }
            if ($n -eq 0) { return 1.0 }
            return $d / $n
        }

        function Lire-Png([string]$Chemin) {
            $flux = [System.IO.MemoryStream]::new([System.IO.File]::ReadAllBytes($Chemin))
            try {
                $img = [System.Drawing.Bitmap]::new($flux)
                try { return [System.Drawing.Bitmap]::new($img) } finally { $img.Dispose() }
            }
            finally { $flux.Dispose() }
        }

        # Le presse-papiers, dans un fil STA : lire l'image, ou y mettre du texte.
        function PressePapiers([scriptblock]$Script) {
            $rs = [runspacefactory]::CreateRunspace()
            $rs.ApartmentState = [System.Threading.ApartmentState]::STA
            $rs.Open()
            try {
                $ps = [powershell]::Create()
                $ps.Runspace = $rs
                [void]$ps.AddScript($Script)
                try { $r = @($ps.Invoke()) } finally { $ps.Dispose() }
                foreach ($x in $r) { if ($null -ne $x) { return $x.psobject.BaseObject } }
                return $null
            }
            finally { $rs.Dispose() }
        }
        $LIRE_IMAGE = {
            Add-Type -AssemblyName System.Windows.Forms, System.Drawing
            for ($i = 0; $i -lt 10; $i++) {
                try {
                    $img = [System.Windows.Forms.Clipboard]::GetImage()
                    if ($null -ne $img) { $t = "$($img.Width)x$($img.Height)"; $img.Dispose(); return $t }
                } catch { }
                Start-Sleep -Milliseconds 100
            }
            return $null
        }
        $METTRE_TEXTE = {
            Add-Type -AssemblyName System.Windows.Forms
            for ($i = 0; $i -lt 10; $i++) {
                try { [System.Windows.Forms.Clipboard]::SetText('texte de l''essai'); return 'ok' } catch { Start-Sleep -Milliseconds 100 }
            }
            return $null
        }

        # Une capture plein écran avec vignette : rend { Id, Vues (souffle vu visible),
        # Debut (lignes avant), Carte (rect réel), Seq, PremierPlan }.
        function Capturer([string]$Dossier, [string]$Quoi, [IntPtr]$Souffle) {
            $V::Poser($REPOS_X, $REPOS_Y)
            $debut = @(Lignes-Journal $Dossier).Count
            $entrees = Compter-Motif -Motif 'vignette entree \d+$' -Dossier $Dossier
            $seq = $V::GetClipboardSequenceNumber()
            $avant = $V::GetForegroundWindow()
            $vues = $V::AppuyerEnGuettant($MODS, $TOUCHE_2, $Souffle, 900)
            $l = Attendre-Motif -Motif 'vignette entree (\d+)$' -Dossier $Dossier -DelaiS 10 -Quoi "$Quoi : entrée de la vignette" -Fois ($entrees + 1)
            $null = $l -match 'vignette entree (\d+)$'
            $id = [int]$Matches[1]
            $zone = Lire-Rect (Attendre-Motif -Motif "vignette zone $id " -Dossier $Dossier -DelaiS 2 -Quoi "$Quoi : zone")
            Start-Sleep -Milliseconds 350   # entrée finie
            return [pscustomobject]@{ Id = $id; Vues = $vues; Debut = $debut; Carte = $zone; Seq = $seq; PremierPlan = $avant }
        }

        # Survole la carte (boutons montrés, fenêtre cliquable), puis clique le bouton.
        function Cliquer-Bouton([string]$Dossier, $C, [string]$Action) {
            $b = Lire-Rect (Derniere-Ligne -Motif "vignette bouton $($C.Id) $Action " -Dossier $Dossier)
            $V::Poser($C.Carte.CX, $C.Carte.CY)
            Start-Sleep -Milliseconds 450
            $V::Cliquer($b.CX, $b.CY)
        }

        function Attendre-Fermee([string]$Dossier, $C, [string]$Raison, [double]$DelaiS = 6) {
            return Attendre-Motif -Motif "vignette fermee $($C.Id) $Raison$" -Dossier $Dossier -DelaiS $DelaiS -Quoi "fermée ($Raison)"
        }

        function Lancer-Aide([string]$Script, [string[]]$Arguments, [string]$Fichier) {
            $aide = Start-Process -FilePath $script:Pwsh -PassThru -ArgumentList (@('-NoProfile', '-File', (Join-Path $PSScriptRoot $Script)) + $Arguments)
            $script:Auxiliaires.Add($aide)
            $limite = [DateTime]::UtcNow.AddSeconds(30)
            while (-not (Test-Path -LiteralPath $Fichier)) {
                if ([DateTime]::UtcNow -ge $limite) { throw "$Script : aucun HWND en 30 s" }
                Start-Sleep -Milliseconds 100
            }
            Start-Sleep -Milliseconds 400
            return [IntPtr][long](Get-Content -LiteralPath $Fichier -Raw).Trim()
        }

        try {
            # ===================== Cas A =========================================================
            $da = Dossier-Cas 'a'
            Ecrire-Reglages -Dossier $da
            $reglagesAvant = Get-Content -LiteralPath (Join-Path $da 'reglages/reglages.json') -Raw
            $recents = Join-Path $da 'local/Recents'
            $images = Join-Path $da 'images'
            $proc = Demarrer-Cas -Dossier $da -Quoi '(A)'

            # (1) Les deux fenêtres, cachées, outil, sans APPWINDOW ni activation.
            $hv = Hwnd-Journal 'vignette fenetre' $da
            $hs = Hwnd-Journal 'souffle fenetre' $da
            $hb = Hwnd-Journal 'bandeau fenetre' $da
            foreach ($f in @(@('vignette', $hv), @('souffle', $hs))) {
                $ex = [long]$V::GetWindowLongPtr($f[1], -20)
                $outil = (($ex -band 0x80) -ne 0) -and (($ex -band 0x08000000) -ne 0) -and (($ex -band 0x40000) -eq 0)
                Verifier "(1) $($f[0]) : cachée, outil, sans activation ni APPWINDOW" ((-not $V::IsWindowVisible($f[1])) -and $outil) ('0x{0:X}' -f $ex) | Out-Null
            }
            Verifier '(1) exclusion coupée pour l''essai' (((Compter-Motif -Motif 'vignette exclue des captures: non \(essai\)' -Dossier $da) -eq 1) -and ((Compter-Motif -Motif 'souffle exclu des captures: non \(essai\)' -Dossier $da) -eq 1)) | Out-Null

            # (10) Glisser, en premier : la cible en haut à droite de la zone utile.
            $COTE_CIBLE = 300
            $cx0 = [int]($utile.Right - $COTE_CIBLE - 20); $cy0 = [int]($utile.Y + 20)
            $depots = Join-Path $script:Essai.Dossier 'depots.txt'
            $hc = Lancer-Aide 'cible-glisser.ps1' @('-X', $cx0, '-Y', $cy0, '-Cote', $COTE_CIBLE, '-FichierHwnd', (Join-Path $script:Essai.Dossier 'cible.txt'), '-FichierDepots', $depots) (Join-Path $script:Essai.Dossier 'cible.txt')
            $ciX = $cx0 + [int]($COTE_CIBLE / 2); $ciY = $cy0 + [int]($COTE_CIBLE / 2)
            Write-Host "cible $hc à $cx0,$cy0"

            $g = Capturer $da '(10) glisser' $hs
            $V::Poser($g.Carte.CX, $g.Carte.CY)
            Start-Sleep -Milliseconds 450
            $V::BoutonBas()
            $V::Deplacer($g.Carte.CX, $g.Carte.CY, $g.Carte.CX + 12, $g.Carte.CY - 4, 6, 15)
            $debutGlisser = Attendre-Motif -Motif "vignette glisser $($g.Id) debut$" -Dossier $da -DelaiS 3 -Quoi '(10) début du glisser'
            $V::Deplacer($g.Carte.CX + 12, $g.Carte.CY - 4, $ciX, $ciY, 25, 20)
            Start-Sleep -Milliseconds 300
            $V::BoutonHaut()
            $null = Attendre-Motif -Motif "vignette glisser $($g.Id) (depose|annule|refuse.*)$" -Dossier $da -DelaiS 5 -Quoi '(10) fin du glisser'
            $limite = [DateTime]::UtcNow.AddSeconds(5)
            while (-not (Test-Path -LiteralPath $depots) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 100 }
            $recus = @()
            if (Test-Path -LiteralPath $depots) { $recus = @(Get-Content -LiteralPath $depots | Where-Object { $_ -ne '' }) }
            $dernierRecent = @(Fichiers-Png $recents)[-1]
            Verifier '(10) la cible a reçu exactement un fichier' ($recus.Count -eq 1) "$($recus.Count) chemin(s)" | Out-Null
            if ($recus.Count -ge 1) {
                $nom = [System.IO.Path]::GetFileName($recus[0])
                Verifier '(10) nommé au modèle' ($nom -match $MODELE) $nom | Out-Null
                $memes = [System.Linq.Enumerable]::SequenceEqual([System.IO.File]::ReadAllBytes($recus[0]), [System.IO.File]::ReadAllBytes($dernierRecent.FullName))
                Verifier '(10) mêmes octets que « Récents »' $memes | Out-Null
            }
            Verifier '(10) vignette glisser depose, GlisserReussi' (((Compter-Motif -Motif "vignette glisser $($g.Id) depose$" -Dossier $da) -eq 1) -and ((Compter-Motif -Motif 'etat Vignette -> Repos \(GlisserReussi\)' -Dossier $da) -eq 1)) | Out-Null
            $null = Attendre-Motif -Motif "vignette terminee $($g.Id)$" -Dossier $da -DelaiS 3 -Quoi '(10) départ'

            # Glisser tenu plus que la durée (3 s), puis Échap : annulé, la vignette reste.
            $g = Capturer $da '(10) Échap' $hs
            $V::Poser($g.Carte.CX, $g.Carte.CY)
            Start-Sleep -Milliseconds 450
            $V::BoutonBas()
            $V::Deplacer($g.Carte.CX, $g.Carte.CY, $g.Carte.CX + 12, $g.Carte.CY, 6, 15)
            $null = Attendre-Motif -Motif "vignette glisser $($g.Id) debut$" -Dossier $da -DelaiS 3 -Quoi '(10) début (Échap)'
            $V::Deplacer($g.Carte.CX + 12, $g.Carte.CY, $REPOS_X, $REPOS_Y, 15, 20)
            Start-Sleep -Milliseconds 4000
            $V::Touche($ECHAP)
            Start-Sleep -Milliseconds 200
            $V::BoutonHaut()
            $null = Attendre-Motif -Motif "vignette glisser $($g.Id) (depose|annule|refuse.*)$" -Dossier $da -DelaiS 5 -Quoi '(10) fin (Échap)'
            Verifier '(10) Échap : annulé, pause pendant le glisser' (((Compter-Motif -Motif "vignette glisser $($g.Id) annule$" -Dossier $da) -eq 1) -and ((Compter-Motif -Motif "vignette minuterie $($g.Id) pause glisser$" -Dossier $da) -eq 1)) | Out-Null
            Start-Sleep -Milliseconds 500
            Verifier '(10) Échap : la vignette reste après plus que sa durée' ((Compter-Motif -Motif "vignette fermee $($g.Id) " -Dossier $da) -eq 0) | Out-Null
            # Lâché sur la carte elle-même : annulé.
            $V::Poser($g.Carte.CX, $g.Carte.CY)
            Start-Sleep -Milliseconds 450
            $V::BoutonBas()
            $V::Deplacer($g.Carte.CX, $g.Carte.CY, $g.Carte.CX + 30, $g.Carte.CY, 10, 20)
            $null = Attendre-Motif -Motif "vignette glisser $($g.Id) debut$" -Dossier $da -DelaiS 3 -Quoi '(10) début (sur soi)' -Fois 2
            $V::Deplacer($g.Carte.CX + 30, $g.Carte.CY, $g.Carte.CX - 10, $g.Carte.CY, 10, 20)
            Start-Sleep -Milliseconds 300
            $V::BoutonHaut()
            $null = Attendre-Motif -Motif "vignette glisser $($g.Id) annule$" -Dossier $da -DelaiS 5 -Quoi '(10) lâché sur la carte : annulé' -Fois 2
            Verifier '(10) lâché sur la carte : annulé, un seul dépôt reçu en tout' (@(Get-Content -LiteralPath $depots | Where-Object { $_ -ne '' }).Count -eq 1) | Out-Null
            $V::Poser($REPOS_X, $REPOS_Y)
            $null = Attendre-Fermee $da $g 'delai'

            # (2) Une capture ordinaire.
            $pngImagesAvant = @(Fichiers-Png $images).Count
            $recentsAvant = @(Fichiers-Png $recents).Count
            $c = Capturer $da '(2)' $hs
            $lignes = @(Lignes-Depuis $da $c.Debut)
            $texte = $lignes -join "`n"
            Verifier '(2) Decoupe -> Souffle (ImagePrete), Souffle -> Vignette' ($texte.Contains('etat Decoupe -> Souffle (ImagePrete)') -and $texte.Contains('etat Souffle -> Vignette (SouffleTermine)')) | Out-Null
            $joue = $lignes | Where-Object { $_ -match "souffle joue $($c.Id) depart " } | Select-Object -First 1
            Verifier '(2) souffle joué depuis l''écran entier' ($null -ne $joue -and $joue -match "depart $($principal.X),$($principal.Y) $($principal.Width)x$($principal.Height) arrivee") "$joue" | Out-Null
            Verifier '(2) fenêtre du souffle vue visible, puis cachée' (($c.Vues -gt 0) -and -not $V::IsWindowVisible($hs)) "relevés $($c.Vues)" | Out-Null
            $marge = 12
            Verifier '(2) carte au coin bas gauche de la zone utile, à 12 px (± 1)' (([math]::Abs($c.Carte.X - ($utile.X + $marge)) -le 1) -and ([math]::Abs(($c.Carte.Y + $c.Carte.H) - ($utile.Bottom - $marge)) -le 1)) "carte $($c.Carte.X),$($c.Carte.Y) $($c.Carte.L)x$($c.Carte.H), zone utile $utile" | Out-Null
            Verifier '(2) aucun bandeau pour cette capture' (@($lignes | Where-Object { $_ -match 'bandeau affiche ' }).Count -eq 0) | Out-Null
            Verifier '(2) presse-papiers = l''image' (($V::GetClipboardSequenceNumber() -ne $c.Seq) -and ((PressePapiers $LIRE_IMAGE) -eq "$($principal.Width)x$($principal.Height)")) | Out-Null
            Verifier '(2) un PNG de plus dans « Récents », aucun dans Images' ((@(Fichiers-Png $recents).Count -eq $recentsAvant + 1) -and (@(Fichiers-Png $images).Count -eq $pngImagesAvant)) | Out-Null
            Verifier '(2) premier plan inchangé' ($V::GetForegroundWindow() -eq $c.PremierPlan) | Out-Null
            Noter-Mesure -Nom 'mémoire privée, vignette montrée' -Valeur (Memoire-Privee-Mo -Processus $proc) -Unite 'Mo'
            $chrono = Derniere-Ligne -Motif 'chrono vignette relache-souffle ' -Dossier $da
            if ($chrono) { Noter-Mesure -Nom 'chrono vignette' -Valeur ($chrono -replace '^.*chrono vignette ', '') }

            # (3) WindowFromPoint : la vignette sur la carte seulement.
            $V::Poser($c.Carte.CX, $c.Carte.CY)
            Start-Sleep -Milliseconds 300
            $sur = $V::RacineSous($c.Carte.CX, $c.Carte.CY)
            $fen = [BreezyShotEssai.Vignette+RECT]::new()
            $null = $V::GetWindowRect($hv, [ref]$fen)
            $aCoteX = [int](($fen.Left + $fen.Right) / 2); $aCoteY = $fen.Top + 4
            $V::Poser($aCoteX, $aCoteY)
            Start-Sleep -Milliseconds 300
            $acote = $V::RacineSous($aCoteX, $aCoteY)
            Verifier '(3) souris sur la carte : la vignette ; à côté, dans sa fenêtre : jamais elle' (($sur -eq $hv) -and ($acote -ne $hv)) "sur $sur, à côté $acote, vignette $hv" | Out-Null

            # (4) Délai de 3 s (souris ailleurs) ; la carte était peinte.
            $V::Poser($REPOS_X, $REPOS_Y)
            $peinte = Copie-Ecran $c.Carte
            $t0 = [DateTime]::UtcNow
            $null = Attendre-Fermee $da $c 'delai'
            $null = Attendre-Motif -Motif "vignette terminee $($c.Id)$" -Dossier $da -DelaiS 2 -Quoi '(4) terminée'
            Start-Sleep -Milliseconds 200
            $vide = Copie-Ecran $c.Carte
            $part = Part-Differente $peinte $vide
            Verifier '(4) la carte était peinte (copie d''écran différente après son départ)' ($part -gt 0.5) ("{0:P0} des points" -f $part) | Out-Null
            Verifier '(4) Vignette -> Repos (DelaiEcoule), fenêtre cachée' (((Compter-Motif -Motif 'etat Vignette -> Repos \(DelaiEcoule\)' -Dossier $da) -ge 1) -and -not $V::IsWindowVisible($hv)) | Out-Null

            # (5) Pause au survol : 5 s sur la carte, toujours là ; dehors, fermée ensuite.
            $c = Capturer $da '(5)' $hs
            $V::Poser($c.Carte.CX, $c.Carte.CY)
            Start-Sleep -Milliseconds 5000
            Verifier '(5) 5 s sur la carte : toujours là, minuterie en pause' (((Compter-Motif -Motif "vignette fermee $($c.Id) " -Dossier $da) -eq 0) -and ((Compter-Motif -Motif "vignette minuterie $($c.Id) pause survol$" -Dossier $da) -ge 1)) | Out-Null
            $V::Poser($REPOS_X, $REPOS_Y)
            $null = Attendre-Motif -Motif "vignette minuterie $($c.Id) reprise$" -Dossier $da -DelaiS 2 -Quoi '(5) reprise'
            $null = Attendre-Fermee $da $c 'delai' 5

            # (6) Copier : presse-papiers remplacé par du texte, puis le bouton.
            $c = Capturer $da '(6)' $hs
            Verifier '(6) presse-papiers remplacé par du texte' ((PressePapiers $METTRE_TEXTE) -eq 'ok') | Out-Null
            Cliquer-Bouton $da $c 'copier'
            $null = Attendre-Motif -Motif "vignette copie $($c.Id) (faite|echec)$" -Dossier $da -DelaiS 5 -Quoi '(6) copie'
            Verifier '(6) copie faite, image au presse-papiers, vignette là' (((Compter-Motif -Motif "vignette copie $($c.Id) faite$" -Dossier $da) -eq 1) -and ((PressePapiers $LIRE_IMAGE) -eq "$($principal.Width)x$($principal.Height)") -and ((Compter-Motif -Motif "vignette fermee $($c.Id) " -Dossier $da) -eq 0)) | Out-Null

            # (7) Enregistrer, puis « deja fait ».
            Cliquer-Bouton $da $c 'enregistrer'
            $null = Attendre-Motif -Motif "vignette enregistrement $($c.Id) (fait|echec)$" -Dossier $da -DelaiS 5 -Quoi '(7) enregistrement'
            $enreg = @(Fichiers-Png $images)
            Verifier '(7) un PNG dans Images, nommé au modèle' (($enreg.Count -eq $pngImagesAvant + 1) -and ($enreg[-1].Name -match $MODELE)) "$(@($enreg | ForEach-Object Name) -join ', ')" | Out-Null
            if ($enreg.Count -ge 1) {
                $a1 = Lire-Png $enreg[-1].FullName; $a2 = Lire-Png (@(Fichiers-Png $recents)[-1].FullName)
                Verifier '(7) mêmes pixels que « Récents »' (($a1.Width -eq $a2.Width) -and ($a1.Height -eq $a2.Height) -and ((Part-Differente $a1 $a2 0) -eq 0)) | Out-Null
            }
            Cliquer-Bouton $da $c 'enregistrer'
            $null = Attendre-Motif -Motif "vignette enregistrement $($c.Id) deja fait$" -Dossier $da -DelaiS 3 -Quoi '(7) deja fait'
            Verifier '(7) second clic : toujours un seul fichier' (@(Fichiers-Png $images).Count -eq $pngImagesAvant + 1) | Out-Null

            # (8) Fermer : la capture reste dans « Récents ».
            $nRecents = @(Fichiers-Png $recents).Count
            Cliquer-Bouton $da $c 'fermer'
            $null = Attendre-Fermee $da $c 'fermer' 3
            Verifier '(8) Vignette -> Repos (VignetteFermee), « Récents » intact' (((Compter-Motif -Motif 'etat Vignette -> Repos \(VignetteFermee\)' -Dossier $da) -eq 1) -and (@(Fichiers-Png $recents).Count -eq $nRecents)) | Out-Null
            $V::Poser($REPOS_X, $REPOS_Y)
            Start-Sleep -Milliseconds 500

            # (9) Supprimer puis Annuler ; puis Enregistrer, Supprimer, 4 s : effacés.
            $c = Capturer $da '(9)' $hs
            $nRecents = @(Fichiers-Png $recents).Count
            Cliquer-Bouton $da $c 'supprimer'
            $null = Attendre-Motif -Motif "vignette suppression $($c.Id) en attente$" -Dossier $da -DelaiS 3 -Quoi '(9) en attente'
            Start-Sleep -Milliseconds 400
            $V::Cliquer($c.Carte.CX, $c.Carte.CY)   # « Annuler » est au centre de la carte
            $null = Attendre-Motif -Motif "vignette suppression $($c.Id) annulee$" -Dossier $da -DelaiS 3 -Quoi '(9) annulée'
            Verifier '(9) annulée : fichier toujours là' (@(Fichiers-Png $recents).Count -eq $nRecents) | Out-Null
            $nImages = @(Fichiers-Png $images).Count
            Cliquer-Bouton $da $c 'enregistrer'
            $null = Attendre-Motif -Motif "vignette enregistrement $($c.Id) fait$" -Dossier $da -DelaiS 5 -Quoi '(9) enregistrée'
            Cliquer-Bouton $da $c 'supprimer'
            $null = Attendre-Motif -Motif "vignette suppression $($c.Id) en attente$" -Dossier $da -DelaiS 3 -Quoi '(9) en attente (2)' -Fois 2
            $V::Poser($REPOS_X, $REPOS_Y)
            $null = Attendre-Motif -Motif "vignette suppression $($c.Id) faite " -Dossier $da -DelaiS 7 -Quoi '(9) suppression faite'
            Verifier '(9) CaptureSupprimee, « Récents » et fichier enregistré effacés' (((Compter-Motif -Motif 'etat Vignette -> Repos \(CaptureSupprimee\)' -Dossier $da) -eq 1) -and ((Compter-Motif -Motif "vignette suppression $($c.Id) faite recent ok fichier ok$" -Dossier $da) -eq 1)) | Out-Null
            Verifier '(9) aucun autre fichier touché' ((@(Fichiers-Png $recents).Count -eq $nRecents - 1) -and (@(Fichiers-Png $images).Count -eq $nImages)) "Récents $(@(Fichiers-Png $recents).Count) (avant $nRecents), Images $(@(Fichiers-Png $images).Count) (avant $nImages)" | Out-Null

            # (11) Aucune fenêtre BreezyShot dans la photo suivante.
            $null = Attendre-Motif -Motif "vignette terminee $($c.Id)$" -Dossier $da -DelaiS 3 -Quoi '(11) repos'
            $ref = Capturer $da '(11) référence' $hs
            $V::Poser($REPOS_X, $REPOS_Y)
            $null = Attendre-Fermee $da $ref 'delai'
            Start-Sleep -Milliseconds 300
            $pngRef = @(Fichiers-Png $recents)[-1].FullName
            $c = Capturer $da '(11)' $hs
            $captures = Compter-Motif -Motif 'vignette entree \d+$' -Dossier $da
            $V::Appuyer($MODS, $TOUCHE_2)
            $null = Attendre-Fermee $da $c 'capture' 5
            $null = Attendre-Motif -Motif 'vignette entree \d+$' -Dossier $da -DelaiS 10 -Quoi '(11) seconde vignette' -Fois ($captures + 1)
            Verifier '(11) Vignette -> Gel' ((Compter-Motif -Motif 'etat Vignette -> Gel' -Dossier $da) -ge 1) | Out-Null
            $imgRef = Lire-Png $pngRef
            $imgNouv = Lire-Png (@(Fichiers-Png $recents)[-1].FullName)
            $zoneImage = [pscustomobject]@{ X = $c.Carte.X - $principal.X; Y = $c.Carte.Y - $principal.Y; L = $c.Carte.L; H = $c.Carte.H }
            $crop = { param($i, $z) $i.Clone([System.Drawing.Rectangle]::new($z.X, $z.Y, $z.L, $z.H), $i.PixelFormat) }
            $partPhoto = Part-Differente (& $crop $imgRef $zoneImage) (& $crop $imgNouv $zoneImage)
            Verifier '(11) zone de la carte : les pixels de la référence (± 6)' ($partPhoto -eq 0) ("{0:P1} des points différents" -f $partPhoto) | Out-Null
            $V::Poser($REPOS_X, $REPOS_Y)
            $dernier = [int]((Derniere-Ligne -Motif 'vignette entree \d+$' -Dossier $da) -replace '^.*vignette entree ', '')
            $null = Attendre-Motif -Motif "vignette fermee $dernier delai$" -Dossier $da -DelaiS 6 -Quoi '(11) fin'

            # (13) Seconde capture pendant le souffle : ignorée.
            $ignores = Compter-Motif -Motif 'capture deja en cours: appui ignore' -Dossier $da
            $souffles = Compter-Motif -Motif 'etat Decoupe -> Souffle' -Dossier $da
            $V::Appuyer($MODS, $TOUCHE_2)
            $null = Attendre-Motif -Motif 'etat Decoupe -> Souffle' -Dossier $da -DelaiS 5 -Quoi '(13) souffle' -Fois ($souffles + 1)
            $V::Appuyer($MODS, $TOUCHE_2)
            Start-Sleep -Milliseconds 800
            $etat = Derniere-Ligne -Motif 'etat Souffle -> Vignette' -Dossier $da
            Verifier '(13) appui pendant le souffle : ignoré' ((Compter-Motif -Motif 'capture deja en cours: appui ignore' -Dossier $da) -eq $ignores + 1) | Out-Null
            $dernier = [int]((Derniere-Ligne -Motif 'vignette entree \d+$' -Dossier $da) -replace '^.*vignette entree ', '')
            $null = Attendre-Motif -Motif "vignette fermee $dernier delai$" -Dossier $da -DelaiS 6 -Quoi '(13) fin'
            Start-Sleep -Milliseconds 500

            # (15) Fenêtres et fils stables après 10 vignettes ; mémoire au repos notée.
            $proc.Refresh()
            $fenAvant = $V::FenetresDe([int[]]@($proc.Id)).Length
            $filsAvant = $proc.Threads.Count
            for ($i = 0; $i -lt 10; $i++) {
                $n = Compter-Motif -Motif 'vignette entree \d+$' -Dossier $da
                $V::Appuyer($MODS, $TOUCHE_2)
                $null = Attendre-Motif -Motif 'vignette entree \d+$' -Dossier $da -DelaiS 10 -Quoi "(15) vignette $i" -Fois ($n + 1)
                Start-Sleep -Milliseconds 300
            }
            $dernier = [int]((Derniere-Ligne -Motif 'vignette entree \d+$' -Dossier $da) -replace '^.*vignette entree ', '')
            $null = Attendre-Motif -Motif "vignette terminee $dernier$" -Dossier $da -DelaiS 6 -Quoi '(15) fin'
            Start-Sleep -Milliseconds 1500
            $proc.Refresh()
            $fenApres = $V::FenetresDe([int[]]@($proc.Id)).Length
            $filsApres = $proc.Threads.Count
            Noter-Mesure -Nom 'fenêtres du processus avant / après 10 vignettes' -Valeur "$fenAvant / $fenApres"
            Noter-Mesure -Nom 'fils avant / après 10 vignettes' -Valeur "$filsAvant / $filsApres"
            Noter-Mesure -Nom 'mémoire privée au repos après 10 vignettes' -Valeur (Memoire-Privee-Mo -Processus $proc) -Unite 'Mo'
            Verifier '(15) fenêtres stables (± 2)' ([math]::Abs($fenApres - $fenAvant) -le 2) "$fenAvant → $fenApres" | Out-Null
            Verifier '(15) fils stables (± 4)' ([math]::Abs($filsApres - $filsAvant) -le 4) "$filsAvant → $filsApres" | Out-Null
            Verifier '(2) réglages non réécrits' ((Get-Content -LiteralPath (Join-Path $da 'reglages/reglages.json') -Raw) -eq $reglagesAvant) | Out-Null

            # (16) --quitter avec une suppression en attente : effacée, arret propre.
            $c = Capturer $da '(16)' $hs
            $fichier16 = @(Fichiers-Png $recents)[-1].FullName
            Cliquer-Bouton $da $c 'supprimer'
            $null = Attendre-Motif -Motif "vignette suppression $($c.Id) en attente$" -Dossier $da -DelaiS 3 -Quoi '(16) en attente'
            Fermer-Cas $proc $da '(16)'
            Verifier '(16) fichier effacé, vignette fermee arret' ((-not (Test-Path -LiteralPath $fichier16)) -and ((Compter-Motif -Motif "vignette fermee $($c.Id) arret$" -Dossier $da) -eq 1) -and ((Compter-Motif -Motif "vignette suppression $($c.Id) faite recent ok" -Dossier $da) -eq 1)) | Out-Null
            Verifier '(16) aucune instance restée en vie' (@(Processus-App).Count -eq 0) | Out-Null

            # ===================== Cas B : exclusion réelle =====================================
            $db = Dossier-Cas 'b'
            Ecrire-Reglages -Dossier $db
            $proc = Demarrer-Cas -Dossier $db -Quoi '(B)' -Capturable $false
            Verifier '(12) vignette et souffle exclus des captures' (((Compter-Motif -Motif 'vignette exclue des captures: oui$' -Dossier $db) -eq 1) -and ((Compter-Motif -Motif 'souffle exclu des captures: oui$' -Dossier $db) -eq 1)) | Out-Null
            $hsB = Hwnd-Journal 'souffle fenetre' $db
            $hvB = Hwnd-Journal 'vignette fenetre' $db
            $c = Capturer $db '(12)' $hsB
            $avecVignette = Copie-Ecran $c.Carte
            $null = Attendre-Fermee $db $c 'delai'
            Start-Sleep -Milliseconds 300
            $sansVignette = Copie-Ecran $c.Carte
            $partExcl = Part-Differente $avecVignette $sansVignette
            Verifier '(12) la copie d''écran de l''essai ne voit pas la carte' ($partExcl -lt 0.02) ("{0:P1} des points différents" -f $partExcl) | Out-Null
            Fermer-Cas $proc $db '(B)'

            # ===================== Cas C, D, E : réglages =======================================
            $dc = Dossier-Cas 'c'
            Ecrire-Reglages -Dossier $dc -Afficher $false
            $proc = Demarrer-Cas -Dossier $dc -Quoi '(C)'
            $V::Appuyer($MODS, $TOUCHE_2)
            $null = Attendre-Motif -Motif 'bandeau affiche \d+ reussite ' -Dossier $dc -DelaiS 10 -Quoi '(14) bandeau'
            Verifier '(14) vignette coupée : ImagePreteSansVignette, « Copié dans le presse-papiers », aucun souffle' (((Compter-Motif -Motif 'etat Decoupe -> Repos \(ImagePreteSansVignette\)' -Dossier $dc) -eq 1) -and ((Compter-Motif -Motif 'sortie annonce reussite: Copié dans le presse-papiers$' -Dossier $dc) -eq 1) -and ((Compter-Motif -Motif 'souffle ' -Dossier $dc) -eq (Compter-Motif -Motif 'souffle (fenetre|exclu|pret)' -Dossier $dc))) | Out-Null
            Fermer-Cas $proc $dc '(C)'

            $dd = Dossier-Cas 'd'
            Ecrire-Reglages -Dossier $dd -Coin 'haut_droite'
            $proc = Demarrer-Cas -Dossier $dd -Quoi '(D)'
            $c = Capturer $dd '(14) haut droite' (Hwnd-Journal 'souffle fenetre' $dd)
            Verifier '(14) coin haut droit de la zone utile, à 12 px (± 1)' (([math]::Abs(($c.Carte.X + $c.Carte.L) - ($utile.Right - 12)) -le 1) -and ([math]::Abs($c.Carte.Y - ($utile.Y + 12)) -le 1)) "carte $($c.Carte.X),$($c.Carte.Y) $($c.Carte.L)x$($c.Carte.H)" | Out-Null
            Fermer-Cas $proc $dd '(D)'

            $de = Dossier-Cas 'e'
            Ecrire-Reglages -Dossier $de -Animations 'reduites'
            $proc = Demarrer-Cas -Dossier $de -Quoi '(E)'
            $c = Capturer $de '(14) réduites' (Hwnd-Journal 'souffle fenetre' $de)
            Verifier '(14) animations réduites : souffle sauté, jamais montré' (((Compter-Motif -Motif "souffle saute $($c.Id): animations reduites$" -Dossier $de) -eq 1) -and ($c.Vues -eq 0)) "relevés $($c.Vues)" | Out-Null
            Fermer-Cas $proc $de '(E)'
        }
        finally {
            foreach ($a in $script:Auxiliaires) { try { $a.Kill() } catch { } }
        }
    }
}
catch {
    Verifier "essai interrompu : $_" $false | Out-Null
    Write-Host $_.ScriptStackTrace
}
finally {
    Terminer-Essai
}
