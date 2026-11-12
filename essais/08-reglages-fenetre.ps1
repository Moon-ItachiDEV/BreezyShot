# Étape 10 : la fenêtre des Réglages et le démarrage avec Windows (ARCHITECTURE.md §3.5,
# §4, §5, §6), sur le vrai .exe, avec de vrais clics et de vraies touches.
#
# (1) Démarrage : « demarrage: cle d'essai » (la vraie clé Run n'est ni lue ni écrite) ;
#     une valeur Run d'essai posée vers une autre copie est réécrite vers l'exe.
# (2) Ouverture par le VRAI menu de l'icône : l'essai envoie à la fenêtre cachée de l'icône
#     (classe « tray_icon_app » de l'app) le message que Windows lui envoie au clic droit,
#     puis lit le menu (#32768) par l'API des menus Win32 (MN_GETHMENU, GetMenuStringW,
#     GetMenuItemRect) et clique au centre de « Réglages ». Si le menu ne
#     s'ouvre pas, la vérification ÉCHOUE (jamais sautée). Fenêtre ordinaire : visible,
#     dans la barre des tâches (pas WS_EX_TOOLWINDOW).
# (3) Rouvrir : une seule fenêtre, ramenée devant.
# (3b) Rubrique Capture ouverte au départ (sans aucun clic d'onglet) : retardateur 3 s
#     écrit dans le fichier puis remis à Aucun ; Inclure le pointeur allumé puis éteint ;
#     aucun interrupteur « Loupe » (écartée de la version).
# (4) Copie seule sortie : l'éteindre est refusé (« Gardez au moins… », reste allumée).
#     (Rubrique « Après la capture » ouverte d'abord.) Vignette allumée par la fenêtre,
#     copie éteinte : Ctrl+Maj+2 : « sortie copie non
#     demandee », presse-papiers inchangé, vignette préparée au coin et à la durée du
#     réglage, partie au bout de 3 s ; l'éteindre, seule sortie, est refusé (second refus
#     voulu) ; copie rallumée puis vignette éteinte. (5) vérifie
#     ensuite qu'aucune vignette n'est plus préparée.
# (5) Raccourci du plein écran changé au clavier en Ctrl+Maj+7 : raccourcis suspendus puis
#     repris, fichier, Ctrl+Maj+7 capture, Ctrl+Maj+2 ne fait plus rien et est rendu à
#     Windows (une autre application peut le tenir).
# (5b) « Refaire la dernière zone » changée au clavier en Ctrl+Maj+9 (libre : le cas tient
#     Ctrl+Maj+1, 3, 4 et 7) : fichier, puis le menu de l'icône lu par l'API des menus :
#     l'entrée vaut « Refaire la dernière zone<tab>Ctrl + Maj + 9 » ; menu fermé par Échap.
# (6) Ctrl+Maj+8 tenu par une autre application : Windows lui donne la touche, la page ne
#     la reçoit pas : ni changement ni refus, la saisie reste ouverte ; Échap : rien ne
#     change, raccourcis repris (une autre application ne peut plus prendre Ctrl+Maj+7).
# (7) Lancer au démarrage sur la clé d'essai : écrit, retiré par l'essai, fenêtre rouverte
#     → éteint (état lu dans le système) ; allumé puis éteint → valeur absente. La vraie
#     clé Run est identique avant et après.
# (8) Enregistrement éteint : rien dans Images ; allumé par la fenêtre. Parcourir… : le
#     sélecteur de dossier du système, un dossier d'essai choisi, une capture y est
#     enregistrée ; le chemin n'apparaît jamais dans le journal.
# (9) Fichier changé de l'extérieur pendant que la fenêtre est ouverte : le changement
#     suivant le relit et ne l'écrase pas.
# (10) Thème Clair puis Sombre : le fond de la zone de contenu prend la couleur du jeton
#     --reglages-fond clair (#FAFAFA) puis sombre (#1C1C1C).
# (11) Fermer : la fenêtre disparaît, l'app tourne ; mémoire notée ; arrêt propre.
#
# Ce que la machine GitHub ne prouve pas : le vrai démarrage de Windows avec la vraie clé,
# la désactivation par le gestionnaire des tâches (tests purs de `etat_depuis`), un
# clavier AZERTY (tests purs de `notation_saisie`), plusieurs écrans.
# Interrupteurs : cherchés par leur motif Toggle (jamais leur libellé), cliqués au centre
# de leur propre rectangle s'ils sont à l'écran, et leur état UI Automation doit basculer.
# Un élément introuvable dit ce qui est vu. Hors Windows : seul le démarrage est vérifié.

param(
    [Parameter(Mandatory)][string]$Exe,
    [Parameter(Mandatory)][string]$Sortie
)

. "$PSScriptRoot/commun.ps1"
Initialiser-Essai -Nom '08-reglages-fenetre' -Exe $Exe -Sortie $Sortie | Out-Null

$script:Auxiliaires = [System.Collections.Generic.List[object]]::new()
$script:Pwsh = (Get-Process -Id $PID).Path
$CLE_ESSAI = 'HKCU:\Software\BreezyShot\Essais'
$CLE_RUN_ESSAI = 'HKCU:\Software\BreezyShot\Essais\Run'
$CLE_RUN = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'

# --- Aides (même forme que les essais 04 à 06) -----------------------------------------

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
        Start-Sleep -Milliseconds 50
    } while ([DateTime]::UtcNow -lt $limite)
    $vu = Compter-Motif -Motif $Motif -Dossier $Dossier
    throw "$Quoi : rien dans le journal en $DelaiS s (motif « $Motif », attendu $Fois fois, vu $vu)"
}

# Attend la ligne suivante d'un motif (comptée avant le geste).
function Attendre-Suivante([string]$Motif, [string]$Dossier, [int]$Avant, [string]$Quoi, [double]$DelaiS = 5) {
    return Attendre-Motif -Motif $Motif -Dossier $Dossier -DelaiS $DelaiS -Quoi $Quoi -Fois ($Avant + 1)
}

function Lire-Reglages([string]$Dossier) {
    return (Get-Content -LiteralPath (Join-Path $Dossier 'reglages/reglages.json') -Raw -Encoding utf8 | ConvertFrom-Json)
}

function Ecrire-Reglages([string]$Dossier, $Objet) {
    $d = Join-Path $Dossier 'reglages'
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    $texte = $Objet | ConvertTo-Json -Depth 6
    [System.IO.File]::WriteAllText((Join-Path $d 'reglages.json'), $texte, [System.Text.UTF8Encoding]::new($false))
}

# Réglages du cas : un raccourci par action, sans Impr. écran (pas d'annonce), la copie
# seule allumée : pas de vignette (elle couvrirait un coin et garderait l'état Vignette
# entre deux points), pas d'enregistrement. Le point 4 allume la vignette par la fenêtre
# le temps d'une capture, le point 8 allume l'enregistrement.
function Reglages-Du-Cas {
    return [ordered]@{
        version                   = 3
        raccourcis                = [ordered]@{ zone = @('Ctrl+Maj+1'); plein_ecran = @('Ctrl+Maj+2'); fenetre = @('Ctrl+Maj+3'); derniere_zone = @('Ctrl+Maj+4') }
        copier_apres_capture      = $true
        enregistrer_apres_capture = $false
        vignette                  = [ordered]@{ afficher = $false; duree_s = 3; coin = 'bas_gauche' }
        selection                 = [ordered]@{ loupe = $false }
        capture                   = [ordered]@{ retardateur_s = 0; pointeur = $false }
        dossier_enregistrement    = $null
        demarrer_avec_systeme     = $false
        modele_nom                = 'BreezyShot {date} à {heure}'
        animations                = 'systeme'
        theme                     = 'systeme'
    }
}

function Valeur-Run([string]$Cle) {
    $v = Get-ItemProperty -LiteralPath $Cle -Name 'BreezyShot' -ErrorAction SilentlyContinue
    if ($null -eq $v) { return $null }
    return [string]$v.BreezyShot
}

try {
    $cas = Join-Path $script:Essai.Dossier 'cas'
    New-Item -ItemType Directory -Force -Path $cas | Out-Null
    Ecrire-Reglages -Dossier $cas -Objet (Reglages-Du-Cas)

    if (-not $IsWindows) {
        $p = Demarrer-App -Dossier $cas
        $null = Attendre-Motif -Motif 'application +pret$' -Dossier $cas -DelaiS 30 -Quoi 'démarrage'
        Verifier '(1) clé d''essai annoncée' ((Compter-Motif -Motif "demarrage: cle d'essai$" -Dossier $cas) -eq 1) | Out-Null
        $q = Quitter-App -DelaiS 10 -Dossier $cas
        Verifier '(11) --quitter → arret propre' ($q.Fini -and $p.WaitForExit(10000)) | Out-Null
        Write-Host '[saute] (2) à (10) : sautés hors Windows'
        return
    }

    Add-Type -AssemblyName UIAutomationClient
    Add-Type -AssemblyName UIAutomationTypes
    Add-Type -AssemblyName System.Drawing
    Add-Type -Namespace BreezyShotEssai -Name Reglages -MemberDefinition @'
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
public static extern bool IsWindow(IntPtr h);
[DllImport("user32.dll")]
public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")]
public static extern int GetWindowLong(IntPtr h, int index);
[DllImport("user32.dll", SetLastError = true)]
public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
[DllImport("user32.dll")]
static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
delegate bool Rappel(IntPtr h, IntPtr p);
[DllImport("user32.dll")]
static extern bool EnumWindows(Rappel f, IntPtr p);
[DllImport("user32.dll", CharSet = CharSet.Unicode)]
static extern int GetClassName(IntPtr h, System.Text.StringBuilder nom, int taille);
[DllImport("user32.dll", CharSet = CharSet.Unicode)]
static extern int GetWindowText(IntPtr h, System.Text.StringBuilder texte, int taille);
[StructLayout(LayoutKind.Sequential)]
public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
[DllImport("user32.dll")]
public static extern bool GetClientRect(IntPtr h, out RECT r);
[StructLayout(LayoutKind.Sequential)]
public struct POINT { public int X; public int Y; }
[DllImport("user32.dll")]
public static extern bool ClientToScreen(IntPtr h, ref POINT p);

const uint RELACHEE = 0x2;
static void Bas(byte vk) { keybd_event(vk, (byte)MapVirtualKey(vk, 0), 0u, UIntPtr.Zero); }
static void Haut(byte vk) { keybd_event(vk, (byte)MapVirtualKey(vk, 0), RELACHEE, UIntPtr.Zero); }
public static void Appuyer(byte[] mods, byte touche) {
    foreach (var m in mods) Bas(m);
    System.Threading.Thread.Sleep(20);
    Bas(touche);
    System.Threading.Thread.Sleep(30);
    Haut(touche);
    for (int i = mods.Length - 1; i >= 0; i--) Haut(mods[i]);
}
public static void Cliquer(int x, int y) {
    SetCursorPos(x, y);
    mouse_event(0x1, 0, 0, 0, UIntPtr.Zero);
    System.Threading.Thread.Sleep(30);
    mouse_event(0x2, 0, 0, 0, UIntPtr.Zero);
    System.Threading.Thread.Sleep(30);
    mouse_event(0x4, 0, 0, 0, UIntPtr.Zero);
}
// Fenêtres de premier niveau d'un processus, d'une classe donnée (toutes si classe vide).
public static IntPtr[] Fenetres(int pid, string classe) {
    var liste = new System.Collections.Generic.List<IntPtr>();
    EnumWindows((h, p) => {
        uint proprio; GetWindowThreadProcessId(h, out proprio);
        if ((int)proprio != pid) return true;
        var nom = new System.Text.StringBuilder(256);
        GetClassName(h, nom, nom.Capacity);
        if (classe == "" || nom.ToString() == classe) liste.Add(h);
        return true;
    }, IntPtr.Zero);
    return liste.ToArray();
}
// Fenêtres visibles de cette classe, tous processus confondus.
public static IntPtr[] Visibles(string classe) {
    var liste = new System.Collections.Generic.List<IntPtr>();
    EnumWindows((h, p) => {
        var nom = new System.Text.StringBuilder(256);
        if (IsWindowVisible(h) && GetClassName(h, nom, nom.Capacity) > 0 && nom.ToString() == classe) liste.Add(h);
        return true;
    }, IntPtr.Zero);
    return liste.ToArray();
}
// Menus Win32 : les HMENU se lisent d'un autre processus.
[DllImport("user32.dll")]
public static extern IntPtr SendMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
[DllImport("user32.dll")]
public static extern IntPtr GetDlgItem(IntPtr dialogue, int id);
[DllImport("user32.dll")]
public static extern bool SetForegroundWindow(IntPtr h);
[StructLayout(LayoutKind.Sequential)]
struct KEYBDINPUT { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
[StructLayout(LayoutKind.Sequential)]
struct MOUSEINPUT { public int dx; public int dy; public uint mouseData; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
[StructLayout(LayoutKind.Explicit)]
struct ENTREE_UNION { [FieldOffset(0)] public MOUSEINPUT mi; [FieldOffset(0)] public KEYBDINPUT ki; }
[StructLayout(LayoutKind.Sequential)]
struct ENTREE { public uint type; public ENTREE_UNION u; }
[DllImport("user32.dll", SetLastError = true)]
static extern uint SendInput(uint n, ENTREE[] entrees, int taille);
// Texte tapé au clavier, caractère par caractère (KEYEVENTF_UNICODE : sans disposition de
// clavier), dans le contrôle qui a le clavier. Rend le nombre de caractères acceptés.
public static int Taper(string texte) {
    int faits = 0;
    foreach (char c in texte) {
        var e = new ENTREE[2];
        e[0].type = 1; e[0].u.ki.wScan = c; e[0].u.ki.dwFlags = 0x4;
        e[1].type = 1; e[1].u.ki.wScan = c; e[1].u.ki.dwFlags = 0x4 | 0x2;
        if (SendInput(2, e, Marshal.SizeOf(typeof(ENTREE))) == 2) faits++;
        System.Threading.Thread.Sleep(4);
    }
    return faits;
}
[DllImport("user32.dll")]
public static extern int GetMenuItemCount(IntPtr menu);
[DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "GetMenuStringW")]
static extern int GetMenuString(IntPtr menu, uint item, System.Text.StringBuilder texte, int taille, uint drapeaux);
[DllImport("user32.dll")]
public static extern bool GetMenuItemRect(IntPtr h, IntPtr menu, uint item, out RECT r);
const uint MN_GETHMENU = 0x01E1;
const uint MF_BYPOSITION = 0x400;
// Le HMENU d'une fenêtre de menu (#32768), IntPtr.Zero si elle n'en donne pas.
public static IntPtr MenuDe(IntPtr fenetre) { return SendMessage(fenetre, MN_GETHMENU, IntPtr.Zero, IntPtr.Zero); }
// Texte complet de chaque position (tabulation et raccourci compris : le point (5b) les
// lit) ; "" pour un séparateur.
public static string[] Textes(IntPtr menu) {
    int n = GetMenuItemCount(menu);
    if (n < 0) return null;
    var textes = new string[n];
    for (int i = 0; i < n; i++) {
        var t = new System.Text.StringBuilder(256);
        GetMenuString(menu, (uint)i, t, t.Capacity, MF_BYPOSITION);
        textes[i] = t.ToString();
    }
    return textes;
}
public static string Titre(IntPtr h) {
    var t = new System.Text.StringBuilder(512);
    GetWindowText(h, t, t.Capacity);
    return t.ToString();
}
'@
    try { $null = [BreezyShotEssai.Reglages]::SetProcessDpiAwarenessContext([IntPtr]::new(-4)) } catch { }
    $AE = [System.Windows.Automation.AutomationElement]
    $CTRL_MAJ = [byte[]]@(0x11, 0x10)

    # Ce qu'UI Automation voit dans une fenêtre, pour le message d'un élément introuvable :
    # le nombre d'éléments et les noms (ceux de la page de l'app), 60 au plus.
    function Elements-Vus([IntPtr]$Racine) {
        try {
            $tous = $AE::FromHandle($Racine).FindAll([System.Windows.Automation.TreeScope]::Descendants,
                [System.Windows.Automation.Condition]::TrueCondition)
            $noms = @()
            foreach ($e in $tous) {
                $c = $e.Current
                if ($c.Name -ne '') { $noms += "$($c.ControlType.ProgrammaticName -replace '^ControlType\.', '') « $($c.Name) »" }
                if ($noms.Count -ge 60) { break }
            }
            return "$($tous.Count) éléments vus ; nommés : " + ($noms -join ', ')
        } catch { return "UI Automation en échec : $($_.Exception.Message)" }
    }
    $script:ErreurElement = ''
    # Type « interrupteur » pour Element : élément qui porte le motif Toggle.
    $INTERRUPTEUR = 'interrupteur'
    $RADIO = [System.Windows.Automation.ControlType]::RadioButton
    # Un élément de la fenêtre $Racine par son nom accessible (et son type), attendu.
    function Element([IntPtr]$Racine, [string]$Nom, $Type = $null, [double]$DelaiS = 5) {
        $script:ErreurElement = ''
        $cond = [System.Windows.Automation.PropertyCondition]::new($AE::NameProperty, $Nom)
        if ($Type -eq $INTERRUPTEUR) {
            # Un interrupteur (role=switch) : celui qui a le motif Toggle, jamais son libellé.
            $cond = [System.Windows.Automation.AndCondition]::new($cond,
                [System.Windows.Automation.PropertyCondition]::new($AE::IsTogglePatternAvailableProperty, $true))
        }
        elseif ($null -ne $Type) {
            $cond = [System.Windows.Automation.AndCondition]::new($cond,
                [System.Windows.Automation.PropertyCondition]::new($AE::ControlTypeProperty, $Type))
        }
        $limite = [DateTime]::UtcNow.AddSeconds($DelaiS)
        do {
            try {
                $e = $AE::FromHandle($Racine).FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
                if ($null -ne $e) { return $e }
            } catch { $script:ErreurElement = $_.Exception.Message }
            Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $limite)
        return $null
    }
    # Type, nom et rectangle d'un élément, pour les messages.
    function Decrire($e) {
        try {
            $c = $e.Current; $r = $c.BoundingRectangle
            $hors = if ($c.IsOffscreen) { ' (hors écran)' } else { '' }
            return "$($c.ControlType.ProgrammaticName) « $($c.Name) » en $([int]$r.X),$([int]$r.Y) $([int]$r.Width)x$([int]$r.Height)$hors"
        } catch { return "élément illisible : $($_.Exception.Message)" }
    }
    # Clic au centre de l'élément lui-même ; jamais sur un élément hors écran (rubrique non
    # affichée) ou sans rectangle.
    function Cliquer-Element($e, [string]$Quoi) {
        if ($null -eq $e) { throw "$Quoi : élément introuvable" }
        # Une ligne qui se déplie au-dessus (repli sous un interrupteur) fait glisser
        # l'élément : on clique quand son rectangle ne bouge plus (deux lectures à 120 ms).
        $c = $e.Current; $r = $c.BoundingRectangle
        $limite = [DateTime]::UtcNow.AddSeconds(2)
        do {
            Start-Sleep -Milliseconds 120
            $avant = $r; $c = $e.Current; $r = $c.BoundingRectangle
        } while (($r.X -ne $avant.X -or $r.Y -ne $avant.Y -or $r.Width -ne $avant.Width -or $r.Height -ne $avant.Height) -and [DateTime]::UtcNow -lt $limite)
        if ($c.IsOffscreen -or $r.IsEmpty -or $r.Width -le 0 -or $r.Height -le 0) {
            $d = Decrire $e
            Verifier "$Quoi : élément à l'écran" $false $d | Out-Null
            throw "$Quoi : élément hors écran ou sans rectangle ($d)"
        }
        [BreezyShotEssai.Reglages]::Cliquer([int]($r.X + $r.Width / 2), [int]($r.Y + $r.Height / 2))
        Start-Sleep -Milliseconds 150
    }
    function Etat-Toggle($e) {
        try { return [string]$e.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern).Current.ToggleState }
        catch { return 'sans etat' }
    }
    # Message d'un élément introuvable : nom et type cherchés, dernière erreur, ce qui est vu.
    function Introuvable([IntPtr]$Racine, [string]$Nom, $Type) {
        $t = if ($Type -eq $INTERRUPTEUR) { 'interrupteur (Toggle)' } elseif ($null -ne $Type) { $Type.ProgrammaticName } else { 'tout type' }
        $err = if ($script:ErreurElement -ne '') { " ; dernière erreur : $script:ErreurElement" } else { '' }
        return "« $Nom » ($t) introuvable$err ; $(Elements-Vus $Racine)"
    }
    function Cliquer-Nom([IntPtr]$Fenetre, [string]$Nom, $Type, [string]$Quoi) {
        $e = Element $Fenetre $Nom $Type
        if ($null -eq $e) {
            $d = Introuvable $Fenetre $Nom $Type
            Verifier "$Quoi : élément trouvé" $false $d | Out-Null
            throw "$Quoi : $d"
        }
        Cliquer-Element $e $Quoi
    }
    # Clic sur un interrupteur, puis preuve que son état UI Automation a basculé (avant
    # d'attendre le journal : un échec dira si c'est le clic ou le cœur).
    function Cliquer-Interrupteur([IntPtr]$Fenetre, [string]$Nom, [string]$Quoi) {
        $e = Element $Fenetre $Nom $INTERRUPTEUR
        if ($null -eq $e) {
            $d = Introuvable $Fenetre $Nom $INTERRUPTEUR
            Verifier "$Quoi : élément trouvé" $false $d | Out-Null
            throw "$Quoi : $d"
        }
        $avant = Etat-Toggle $e
        Cliquer-Element $e $Quoi
        $apres = $avant
        $limite = [DateTime]::UtcNow.AddSeconds(2)
        do {
            $apres = Etat-Toggle $e
            if ($apres -ne $avant) { break }
            Start-Sleep -Milliseconds 50
        } while ([DateTime]::UtcNow -lt $limite)
        $d = "$(Decrire $e) : $avant → $apres"
        if (-not (Verifier "$Quoi : l'interrupteur bascule" ($apres -ne $avant -and $apres -ne 'sans etat') $d)) {
            throw "$Quoi : l'interrupteur n'a pas basculé ($d)"
        }
    }
    function Etat-Interrupteur([IntPtr]$Fenetre, [string]$Nom) {
        $e = Element $Fenetre $Nom $INTERRUPTEUR
        if ($null -eq $e) { return "introuvable ($(Introuvable $Fenetre $Nom $INTERRUPTEUR))" }
        return (Etat-Toggle $e)
    }

    $reelAvant = Valeur-Run $CLE_RUN
    Remove-Item -LiteralPath $CLE_ESSAI -Recurse -Force -ErrorAction SilentlyContinue
    # (1) Une valeur Run (clé d'essai) laissée par une autre copie de l'exe : l'app la
    # réécrit vers elle-même au démarrage.
    New-Item -Path $CLE_RUN_ESSAI -Force | Out-Null
    New-ItemProperty -LiteralPath $CLE_RUN_ESSAI -Name 'BreezyShot' -Value '"C:\Ailleurs\breezyshot.exe"' -PropertyType String -Force | Out-Null

    # (1) Démarrage.
    $p = Demarrer-App -Dossier $cas
    $null = Attendre-Motif -Motif 'application +pret$' -Dossier $cas -DelaiS 30 -Quoi '(1) démarrage'
    $null = Attendre-Motif -Motif 'bandeau pret$' -Dossier $cas -DelaiS 20 -Quoi '(1) page du bandeau'
    # Sans clé d'essai, (7) écrirait sur la vraie clé Run : l'essai s'arrête.
    if (-not (Verifier "(1) « demarrage: cle d'essai »" ((Compter-Motif -Motif "demarrage: cle d'essai$" -Dossier $cas) -eq 1))) {
        throw "(1) pas de clé d'essai : arrêt avant tout geste sur le démarrage"
    }
    Verifier '(1) raccourci plein_ecran: Ctrl+Maj+2' ((Compter-Motif -Motif 'raccourci plein_ecran: Ctrl\+Maj\+2$' -Dossier $cas) -eq 1) | Out-Null
    $vu = Valeur-Run $CLE_RUN_ESSAI
    Verifier '(1) valeur Run d''une autre copie réécrite vers l''exe de l''essai' (((Compter-Motif -Motif 'demarrage avec le systeme: chemin mis a jour$' -Dossier $cas) -eq 1) -and ($vu -eq ('"' + $script:Essai.Exe + '"'))) "$vu" | Out-Null
    # Retirée : le point 7 part d'un démarrage éteint.
    Remove-ItemProperty -LiteralPath $CLE_RUN_ESSAI -Name 'BreezyShot' -ErrorAction SilentlyContinue

    # (2) Ouvrir par le vrai menu de l'icône.
    function Ouvrir-Par-Menu([string]$Quoi) {
        $icones = [BreezyShotEssai.Reglages]::Fenetres($p.Id, 'tray_icon_app')
        if ($icones.Length -lt 1) {
            Verifier "$Quoi : fenêtre de l'icône trouvée" $false 'aucune fenêtre tray_icon_app' | Out-Null
            throw "$Quoi : menu de l'icône impossible à ouvrir"
        }
        $avant = @([BreezyShotEssai.Reglages]::Visibles('#32768') | ForEach-Object { [long]$_ })
        $vus = Compter-Motif -Motif 'menu: reglages$' -Dossier $cas
        # WM_USER_TRAYICON (6002) avec WM_RBUTTONUP (0x205) : ce que Windows envoie au clic droit.
        foreach ($h in $icones) { [void][BreezyShotEssai.Reglages]::PostMessage($h, 6002, [IntPtr]::Zero, [IntPtr]0x205) }
        # Chaque fenêtre de menu nouvelle est lue par l'API des menus Win32 (MN_GETHMENU,
        # GetMenuStringW) : l'entrée est celle dont le texte vaut exactement « Réglages ».
        $nouveaux = @()
        $trouve = $null
        $lus = @{}
        $limite = [DateTime]::UtcNow.AddSeconds(3)
        do {
            $nouveaux = @([BreezyShotEssai.Reglages]::Visibles('#32768') | Where-Object { $avant -notcontains [long]$_ })
            foreach ($f in $nouveaux) {
                $hm = [BreezyShotEssai.Reglages]::MenuDe($f)
                if ($hm -eq [IntPtr]::Zero) { $lus["$f"] = 'pas de HMENU'; continue }
                $textes = [BreezyShotEssai.Reglages]::Textes($hm)
                if ($null -eq $textes) { $lus["$f"] = "HMENU $hm : GetMenuItemCount en échec"; continue }
                $lus["$f"] = "HMENU $hm, $($textes.Length) entrées : " + (($textes | ForEach-Object { "« $_ »" }) -join ', ')
                for ($i = 0; $i -lt $textes.Length; $i++) {
                    if ($textes[$i] -ceq 'Réglages') { $trouve = @{ Fenetre = $f; Menu = $hm; Position = $i }; break }
                }
                if ($null -ne $trouve) { break }
            }
            if ($null -eq $trouve) { Start-Sleep -Milliseconds 50 }
        } while ($null -eq $trouve -and [DateTime]::UtcNow -lt $limite)
        if (-not (Verifier "$Quoi : le menu de l'icône s'ouvre" ($nouveaux.Count -gt 0) "fenêtres #32768 nouvelles : $($nouveaux.Count), déjà là : $($avant.Count)")) {
            throw "$Quoi : le menu de l'icône ne s'ouvre pas"
        }
        $diag = (($lus.GetEnumerator() | ForEach-Object { "fenêtre $($_.Key) : $($_.Value)" }) -join ' | ')
        if (-not (Verifier "$Quoi : entrée « Réglages » au menu" ($null -ne $trouve) $diag)) { throw "$Quoi : pas d'entrée Réglages ($diag)" }
        $r = New-Object BreezyShotEssai.Reglages+RECT
        if (-not [BreezyShotEssai.Reglages]::GetMenuItemRect([IntPtr]::Zero, $trouve.Menu, [uint32]$trouve.Position, [ref]$r)) {
            Verifier "$Quoi : rectangle de l'entrée « Réglages »" $false "GetMenuItemRect en échec, position $($trouve.Position)" | Out-Null
            throw "$Quoi : rectangle de l'entrée Réglages illisible"
        }
        [BreezyShotEssai.Reglages]::Cliquer([int](($r.Left + $r.Right) / 2), [int](($r.Top + $r.Bottom) / 2))
        $null = Attendre-Suivante 'menu: reglages$' $cas $vus "$Quoi : menu: reglages (clic en $($r.Left),$($r.Top) $($r.Right - $r.Left)x$($r.Bottom - $r.Top), position $($trouve.Position))"
    }

    $ouvertes = Compter-Motif -Motif 'reglages fenetre \d+$' -Dossier $cas
    Ouvrir-Par-Menu '(2)'
    $l = Attendre-Suivante 'reglages fenetre (\d+)$' $cas $ouvertes '(2) fenêtre créée' 10
    if ($l -notmatch 'reglages fenetre (\d+)$') { throw "(2) ligne illisible : $l" }
    $fen = [IntPtr][long]$Matches[1]
    $null = Attendre-Motif -Motif 'reglages page prete$' -Dossier $cas -DelaiS 20 -Quoi '(2) page prête'
    $ex = [BreezyShotEssai.Reglages]::GetWindowLong($fen, -20)
    Verifier '(2) fenêtre visible, titre « Réglages de BreezyShot »' (([BreezyShotEssai.Reglages]::IsWindowVisible($fen)) -and ([BreezyShotEssai.Reglages]::Titre($fen) -eq 'Réglages de BreezyShot')) | Out-Null
    Verifier '(2) fenêtre ordinaire (pas WS_EX_TOOLWINDOW)' (($ex -band 0x80) -eq 0) ('0x{0:X}' -f $ex) | Out-Null

    # (3) Rouvrir : ramenée devant, une seule.
    $devant = Compter-Motif -Motif 'reglages ramenes devant$' -Dossier $cas
    Ouvrir-Par-Menu '(3)'
    $null = Attendre-Suivante 'reglages ramenes devant$' $cas $devant '(3) ramenée devant'
    Start-Sleep -Milliseconds 300
    $titres = @([BreezyShotEssai.Reglages]::Fenetres($p.Id, '') | Where-Object { [BreezyShotEssai.Reglages]::Titre($_) -eq 'Réglages de BreezyShot' })
    Verifier '(3) une seule fenêtre Réglages' ($titres.Count -eq 1) "$($titres.Count)" | Out-Null
    Verifier '(3) au premier plan' ([BreezyShotEssai.Reglages]::GetForegroundWindow() -eq $fen) | Out-Null

    function Presses { return (Compter-Motif -Motif 'raccourci presse: plein_ecran$' -Dossier $cas) }
    function Attendre-Repos([int]$Avant) { $null = Attendre-Suivante 'Decoupe -> Repos' $cas $Avant 'retour au repos' 10 }
    # Un raccourci tenu un instant par une autre application (tenir-raccourci.ps1) : rend
    # « tenu » ou « refuse », puis ferme l'aide (le raccourci est alors rendu).
    function Tenir-Ailleurs([int]$Vk, [int]$Mods, [string]$Nom) {
        $fichier = Join-Path $script:Essai.Dossier "tenu-$Nom.txt"
        $aide = Start-Process -FilePath $script:Pwsh -PassThru -WindowStyle Hidden -ArgumentList @('-NoProfile', '-File', (Join-Path $PSScriptRoot 'tenir-raccourci.ps1'), '-Vk', $Vk, '-Mods', $Mods, '-Fichier', $fichier)
        $script:Auxiliaires.Add($aide)
        $limite = [DateTime]::UtcNow.AddSeconds(30)
        while (-not (Test-Path -LiteralPath $fichier) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 100 }
        $lu = if (Test-Path -LiteralPath $fichier) { (Get-Content -LiteralPath $fichier -Raw).Trim() } else { 'pas de réponse en 30 s' }
        try { $aide.Kill(); $null = $aide.WaitForExit(5000) } catch { }
        return $lu
    }

    # (3b) Rubrique Capture, ouverte au départ : ses contrôles sont à l'écran sans aucun
    # clic d'onglet, ceux d'« Après la capture » ne le sont pas.
    $aucun = Element $fen 'Aucun' $RADIO
    $ouverte = ($null -ne $aucun) -and (-not $aucun.Current.IsOffscreen)
    Verifier '(3b) rubrique Capture ouverte au départ (« Aucun » du retardateur à l''écran)' $ouverte $(if ($null -eq $aucun) { Introuvable $fen 'Aucun' $RADIO } else { Decrire $aucun }) | Out-Null
    $copier = Element $fen 'Copier automatiquement' $INTERRUPTEUR 0.5
    Verifier '(3b) « Après la capture » fermée au départ' (($null -eq $copier) -or $copier.Current.IsOffscreen) $(if ($null -ne $copier) { Decrire $copier } else { '' }) | Out-Null
    # Retardateur : 3 s écrit, puis remis à Aucun (les captures suivantes partent tout de suite).
    Cliquer-Nom $fen '3 s' $RADIO '(3b) retardateur 3 s'
    $null = Attendre-Motif -Motif 'reglage change: retardateur 3$' -Dossier $cas -DelaiS 5 -Quoi '(3b) retardateur 3'
    Verifier '(3b) fichier : capture.retardateur_s = 3' ((Lire-Reglages $cas).capture.retardateur_s -eq 3) | Out-Null
    Cliquer-Nom $fen 'Aucun' $RADIO '(3b) retardateur Aucun'
    $null = Attendre-Motif -Motif 'reglage change: retardateur 0$' -Dossier $cas -DelaiS 5 -Quoi '(3b) retardateur 0'
    Verifier '(3b) fichier : capture.retardateur_s = 0' ((Lire-Reglages $cas).capture.retardateur_s -eq 0) | Out-Null
    # Inclure le pointeur : allumé puis éteint, chaque fois dans le fichier.
    Cliquer-Interrupteur $fen 'Inclure le pointeur' '(3b) interrupteur Inclure le pointeur'
    $null = Attendre-Motif -Motif 'reglage change: pointeur oui$' -Dossier $cas -DelaiS 5 -Quoi '(3b) pointeur oui'
    Verifier '(3b) fichier : capture.pointeur true' ((Lire-Reglages $cas).capture.pointeur -eq $true) | Out-Null
    Cliquer-Interrupteur $fen 'Inclure le pointeur' '(3b) interrupteur Inclure le pointeur (éteindre)'
    $null = Attendre-Motif -Motif 'reglage change: pointeur non$' -Dossier $cas -DelaiS 5 -Quoi '(3b) pointeur non'
    Verifier '(3b) fichier : capture.pointeur false' ((Lire-Reglages $cas).capture.pointeur -eq $false) | Out-Null
    # La loupe a été écartée (12/11/2026) : aucun interrupteur « Loupe » dans la page.
    $loupe = Element $fen 'Loupe' $INTERRUPTEUR 0.5
    Verifier '(3b) aucun interrupteur « Loupe » dans la page' ($null -eq $loupe) $(if ($null -ne $loupe) { Decrire $loupe } else { '' }) | Out-Null

    # (4) La copie est la seule sortie : l'éteindre est refusé, pour de vrai.
    Cliquer-Nom $fen 'Après la capture' ([System.Windows.Automation.ControlType]::TabItem) '(4) rubrique Après la capture'
    $refuses = Compter-Motif -Motif 'reglage refuse: copier \(aucune_sortie\)$' -Dossier $cas
    Cliquer-Nom $fen 'Copier automatiquement' $INTERRUPTEUR '(4) Copier, seule sortie'
    $null = Attendre-Suivante 'reglage refuse: copier \(aucune_sortie\)$' $cas $refuses '(4) refus de la dernière sortie'
    $texte = Element $fen 'Gardez au moins la copie, l''enregistrement ou la vignette'
    Verifier '(4) « Gardez au moins la copie, l''enregistrement ou la vignette » sur la ligne' ($null -ne $texte) $(if ($null -eq $texte) { Introuvable $fen 'Gardez au moins la copie, l''enregistrement ou la vignette' $null } else { '' }) | Out-Null
    Start-Sleep -Milliseconds 300
    $etat = Etat-Interrupteur $fen 'Copier automatiquement'
    Verifier '(4) refus : Copier reste allumé (interface et fichier)' (($etat -eq 'On') -and ((Lire-Reglages $cas).copier_apres_capture -eq $true)) $etat | Out-Null

    # (4) Vignette allumée par la fenêtre : la capture suivante la prépare (durée 3 s du
    # fichier) et elle part seule au bout du délai.
    Cliquer-Interrupteur $fen 'Vignette' '(4) interrupteur Vignette'
    $null = Attendre-Motif -Motif 'reglage change: vignette oui$' -Dossier $cas -DelaiS 5 -Quoi '(4) vignette oui'

    # (4) Copier éteint, capture sans copie, rallumé.
    $ecrits = Compter-Motif -Motif 'reglages ecrits$' -Dossier $cas
    Cliquer-Interrupteur $fen 'Copier automatiquement' '(4) interrupteur Copier'
    $null = Attendre-Suivante 'reglage change: copier non$' $cas 0 '(4) copier non'
    $null = Attendre-Suivante 'reglages ecrits$' $cas $ecrits '(4) écrit'
    Verifier '(4) fichier : copier_apres_capture false' ((Lire-Reglages $cas).copier_apres_capture -eq $false) | Out-Null
    Set-Clipboard -Value 'temoin-08'
    $sorties = Compter-Motif -Motif 'sortie copie ' -Dossier $cas
    $preparees = Compter-Motif -Motif 'vignette preparee \d+ ' -Dossier $cas
    $delais = Compter-Motif -Motif 'vignette fermee \d+ delai$' -Dossier $cas
    $finVignette = Compter-Motif -Motif 'Vignette -> Repos' -Dossier $cas
    [BreezyShotEssai.Reglages]::Appuyer($CTRL_MAJ, 0x32)
    $ls = Attendre-Suivante 'sortie copie ' $cas $sorties '(4) capture' 10
    Verifier '(4) Ctrl+Maj+2 : copie non demandée' ($ls -match 'sortie copie non demandee') $ls | Out-Null
    Verifier '(4) presse-papiers inchangé' ((Get-Clipboard -Raw) -eq 'temoin-08') | Out-Null
    $lv = Attendre-Suivante 'vignette preparee \d+ ' $cas $preparees '(4) vignette préparée (réglage allumé dans la fenêtre)' 10
    Verifier '(4) vignette au coin et à la durée du réglage' (($lv -match ' coin bas_gauche ') -and ($lv -match ' duree 3 ')) $lv | Out-Null
    $null = Attendre-Suivante 'vignette fermee \d+ delai$' $cas $delais '(4) vignette partie au bout de 3 s' 15
    $null = Attendre-Suivante 'Vignette -> Repos' $cas $finVignette '(4) retour au repos' 10
    # Second refus, voulu : la vignette est maintenant la seule sortie (copie et
    # enregistrement éteints) ; l'éteindre est refusé, elle reste allumée.
    $refuses = Compter-Motif -Motif 'reglage refuse: vignette \(aucune_sortie\)$' -Dossier $cas
    Cliquer-Nom $fen 'Vignette' $INTERRUPTEUR '(4) Vignette, seule sortie'
    $null = Attendre-Suivante 'reglage refuse: vignette \(aucune_sortie\)$' $cas $refuses '(4) second refus : dernière sortie'
    Start-Sleep -Milliseconds 300
    $etat = Etat-Interrupteur $fen 'Vignette'
    Verifier '(4) second refus : Vignette reste allumée (interface et fichier)' (($etat -eq 'On') -and ((Lire-Reglages $cas).vignette.afficher -eq $true)) $etat | Out-Null
    # Règle : chaque extinction laisse une sortie. Copie rallumée d'abord, puis vignette
    # éteinte.
    Cliquer-Interrupteur $fen 'Copier automatiquement' '(4) interrupteur Copier (rallumer)'
    $null = Attendre-Motif -Motif 'reglage change: copier oui$' -Dossier $cas -DelaiS 5 -Quoi '(4) copier oui'
    Cliquer-Interrupteur $fen 'Vignette' '(4) interrupteur Vignette (éteindre)'
    $null = Attendre-Motif -Motif 'reglage change: vignette non$' -Dossier $cas -DelaiS 5 -Quoi '(4) vignette non'

    # (5) Ctrl+Maj+7 pour le plein écran.
    Cliquer-Nom $fen 'Raccourcis' ([System.Windows.Automation.ControlType]::TabItem) '(5) rubrique Raccourcis'
    $susp = Compter-Motif -Motif 'raccourcis suspendus \(saisie plein_ecran\)$' -Dossier $cas
    $repris = Compter-Motif -Motif 'raccourcis repris$' -Dossier $cas
    Cliquer-Nom $fen 'Plein écran' ([System.Windows.Automation.ControlType]::Button) '(5) raccourci du plein écran'
    $null = Attendre-Suivante 'raccourcis suspendus \(saisie plein_ecran\)$' $cas $susp '(5) saisie'
    [BreezyShotEssai.Reglages]::Appuyer($CTRL_MAJ, 0x37)
    $null = Attendre-Motif -Motif 'raccourci plein_ecran change: Ctrl\+Maj\+7$' -Dossier $cas -DelaiS 5 -Quoi '(5) changé'
    $null = Attendre-Suivante 'raccourcis repris$' $cas $repris '(5) repris'
    Start-Sleep -Milliseconds 300
    $lu = @((Lire-Reglages $cas).raccourcis.plein_ecran)
    Verifier '(5) fichier : plein_ecran = [Ctrl+Maj+7]' (($lu.Count -eq 1) -and ($lu[0] -eq 'Ctrl+Maj+7')) ($lu -join ',') | Out-Null
    $n = Presses; $repos = Compter-Motif -Motif 'Decoupe -> Repos' -Dossier $cas
    $preparees = Compter-Motif -Motif 'vignette preparee \d+ ' -Dossier $cas
    [BreezyShotEssai.Reglages]::Appuyer($CTRL_MAJ, 0x37)
    $null = Attendre-Suivante 'raccourci presse: plein_ecran$' $cas $n '(5) Ctrl+Maj+7 capture'
    Attendre-Repos $repos
    Verifier '(5) Ctrl+Maj+7 lance la capture de l''écran' $true | Out-Null
    # Vignette éteinte dans la fenêtre au point 4 : plus de vignette, sans redémarrer.
    Verifier '(5) vignette éteinte : aucune « vignette preparee »' ((Compter-Motif -Motif 'vignette preparee \d+ ' -Dossier $cas) -eq $preparees) | Out-Null
    $n = Presses
    [BreezyShotEssai.Reglages]::Appuyer($CTRL_MAJ, 0x32)
    Start-Sleep -Milliseconds 1500
    Verifier '(5) Ctrl+Maj+2 ne fait plus rien' ((Presses) -eq $n) | Out-Null
    # L'ancien raccourci est rendu à Windows : une autre application peut le prendre.
    $t = Tenir-Ailleurs 0x32 6 'ctrl-maj-2'
    Verifier '(5) Ctrl+Maj+2 rendu à Windows (une autre application le tient)' ($t -eq 'tenu') $t | Out-Null

    # (5b) Ctrl+Maj+9 pour la dernière zone (même geste que (5)), puis le menu de l'icône.
    $lu = (Lire-Reglages $cas).raccourcis
    $pris = @('zone', 'plein_ecran', 'fenetre', 'derniere_zone') | ForEach-Object { @($lu.$_) } | Where-Object { $_ -eq 'Ctrl+Maj+9' }
    if (-not (Verifier '(5b) Ctrl+Maj+9 libre dans le fichier du cas' (@($pris).Count -eq 0))) { throw '(5b) Ctrl+Maj+9 déjà pris' }
    $susp = Compter-Motif -Motif 'raccourcis suspendus \(saisie derniere_zone\)$' -Dossier $cas
    $repris = Compter-Motif -Motif 'raccourcis repris$' -Dossier $cas
    Cliquer-Nom $fen 'Refaire la dernière zone' ([System.Windows.Automation.ControlType]::Button) '(5b) raccourci de la dernière zone'
    $null = Attendre-Suivante 'raccourcis suspendus \(saisie derniere_zone\)$' $cas $susp '(5b) saisie'
    [BreezyShotEssai.Reglages]::Appuyer($CTRL_MAJ, 0x39)
    $null = Attendre-Motif -Motif 'raccourci derniere_zone change: Ctrl\+Maj\+9$' -Dossier $cas -DelaiS 5 -Quoi '(5b) changé'
    $null = Attendre-Suivante 'raccourcis repris$' $cas $repris '(5b) repris'
    Start-Sleep -Milliseconds 300
    $lu = @((Lire-Reglages $cas).raccourcis.derniere_zone)
    Verifier '(5b) fichier : derniere_zone = [Ctrl+Maj+9]' ((@($lu).Count -eq 1) -and ($lu[0] -eq 'Ctrl+Maj+9')) ($lu -join ',') | Out-Null
    # Le menu de l'icône (même message que Ouvrir-Par-Menu), lu sans rien y cliquer.
    $icones = [BreezyShotEssai.Reglages]::Fenetres($p.Id, 'tray_icon_app')
    if (-not (Verifier '(5b) fenêtre de l''icône trouvée' ($icones.Length -ge 1) 'aucune fenêtre tray_icon_app')) { throw '(5b) menu de l''icône impossible à ouvrir' }
    $avant = @([BreezyShotEssai.Reglages]::Visibles('#32768') | ForEach-Object { [long]$_ })
    foreach ($h in $icones) { [void][BreezyShotEssai.Reglages]::PostMessage($h, 6002, [IntPtr]::Zero, [IntPtr]0x205) }
    $attendu = "Refaire la dernière zone`tCtrl + Maj + 9"
    $menu = [IntPtr]::Zero; $textes = @(); $lus = ''
    $limite = [DateTime]::UtcNow.AddSeconds(3)
    do {
        foreach ($f in @([BreezyShotEssai.Reglages]::Visibles('#32768') | Where-Object { $avant -notcontains [long]$_ })) {
            $hm = [BreezyShotEssai.Reglages]::MenuDe($f)
            if ($hm -eq [IntPtr]::Zero) { continue }
            $t = [BreezyShotEssai.Reglages]::Textes($hm)
            if ($null -ne $t -and $t.Length -gt 0) { $menu = $f; $textes = @($t); break }
        }
        if ($menu -eq [IntPtr]::Zero) { Start-Sleep -Milliseconds 50 }
    } while ($menu -eq [IntPtr]::Zero -and [DateTime]::UtcNow -lt $limite)
    $lus = ($textes | ForEach-Object { "« $($_ -replace "`t", '<tab>') »" }) -join ', '
    if (-not (Verifier '(5b) le menu de l''icône s''ouvre' ($menu -ne [IntPtr]::Zero))) { throw '(5b) le menu de l''icône ne s''ouvre pas' }
    $entrees = @($textes | Where-Object { $_ -like 'Refaire la dernière zone*' })
    Verifier '(5b) menu : « Refaire la dernière zone<tab>Ctrl + Maj + 9 »' ((@($entrees).Count -eq 1) -and ($entrees[0] -ceq $attendu)) $lus | Out-Null
    # Échap ferme le menu sans rien choisir.
    $menus = Compter-Motif -Motif 'menu: (quitter|reglages|capturer)' -Dossier $cas
    [BreezyShotEssai.Reglages]::Appuyer([byte[]]@(), 0x1B)
    $limite = [DateTime]::UtcNow.AddSeconds(2)
    while ((@([BreezyShotEssai.Reglages]::Visibles('#32768') | Where-Object { [long]$_ -eq [long]$menu }).Count -gt 0) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 50 }
    $ouvert = @([BreezyShotEssai.Reglages]::Visibles('#32768') | Where-Object { [long]$_ -eq [long]$menu }).Count -gt 0
    Start-Sleep -Milliseconds 300
    Verifier '(5b) Échap ferme le menu, aucune entrée choisie' ((-not $ouvert) -and ((Compter-Motif -Motif 'menu: (quitter|reglages|capturer)' -Dossier $cas) -eq $menus)) | Out-Null

    # (6) Ctrl+Maj+8 tenu ailleurs : la saisie ne le reçoit pas ; puis Échap.
    # L'aide tourne sans console : sa fenêtre couvrirait la ligne cliquée ensuite.
    $f = Join-Path $script:Essai.Dossier 'tenu-8.txt'
    $aide = Start-Process -FilePath $script:Pwsh -PassThru -WindowStyle Hidden -ArgumentList @('-NoProfile', '-File', (Join-Path $PSScriptRoot 'tenir-raccourci.ps1'), '-Vk', 0x38, '-Mods', 6, '-Fichier', $f)
    $script:Auxiliaires.Add($aide)
    $limite = [DateTime]::UtcNow.AddSeconds(30)
    while (-not (Test-Path -LiteralPath $f) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 100 }
    $tenu = (Test-Path -LiteralPath $f) -and ((Get-Content -LiteralPath $f -Raw).Trim() -eq 'tenu')
    if (-not (Verifier '(6) Ctrl+Maj+8 tenu par une autre application' $tenu)) { throw '(6) Ctrl+Maj+8 non tenu' }
    # Windows donne un raccourci global à l'application qui le tient, avant la fenêtre
    # active : la page ne reçoit pas la touche. La saisie reste donc ouverte, sans
    # changement ni refus ; Échap la ferme (preuve que la page avait bien le clavier).
    $susp = Compter-Motif -Motif 'raccourcis suspendus \(saisie plein_ecran\)$' -Dossier $cas
    $repris = Compter-Motif -Motif 'raccourcis repris$' -Dossier $cas
    $changes = Compter-Motif -Motif 'raccourci plein_ecran change: ' -Dossier $cas
    $refus = Compter-Motif -Motif 'raccourci plein_ecran refuse: ' -Dossier $cas
    Cliquer-Nom $fen 'Plein écran' ([System.Windows.Automation.ControlType]::Button) '(6) raccourci du plein écran'
    $null = Attendre-Suivante 'raccourcis suspendus \(saisie plein_ecran\)$' $cas $susp '(6) saisie'
    [BreezyShotEssai.Reglages]::Appuyer($CTRL_MAJ, 0x38)
    Start-Sleep -Milliseconds 1200
    $changesApres = Compter-Motif -Motif 'raccourci plein_ecran change: ' -Dossier $cas
    $refusApres = Compter-Motif -Motif 'raccourci plein_ecran refuse: ' -Dossier $cas
    $reprisApres = Compter-Motif -Motif 'raccourcis repris$' -Dossier $cas
    Verifier '(6) Ctrl+Maj+8 tenu ailleurs : ni changement ni refus, la saisie reste ouverte' (($changesApres -eq $changes) -and ($refusApres -eq $refus) -and ($reprisApres -eq $repris)) "changements $changes → $changesApres, refus $refus → $refusApres, repris $repris → $reprisApres" | Out-Null
    $lu = @((Lire-Reglages $cas).raccourcis.plein_ecran)
    Verifier '(6) fichier inchangé' (($lu.Count -eq 1) -and ($lu[0] -eq 'Ctrl+Maj+7')) ($lu -join ',') | Out-Null
    [BreezyShotEssai.Reglages]::Appuyer([byte[]]@(), 0x1B)
    $null = Attendre-Suivante 'raccourcis repris$' $cas $repris '(6) Échap : repris'
    $lu = @((Lire-Reglages $cas).raccourcis.plein_ecran)
    Verifier '(6) Échap : rien ne change' (($lu.Count -eq 1) -and ($lu[0] -eq 'Ctrl+Maj+7')) ($lu -join ',') | Out-Null
    $n = Presses; $repos = Compter-Motif -Motif 'Decoupe -> Repos' -Dossier $cas
    [BreezyShotEssai.Reglages]::Appuyer($CTRL_MAJ, 0x37)
    $null = Attendre-Suivante 'raccourci presse: plein_ecran$' $cas $n '(6) Ctrl+Maj+7 après Échap'
    Attendre-Repos $repos
    Verifier '(6) Ctrl+Maj+7 capture toujours' $true | Out-Null
    try { $aide.Kill() } catch { }
    # Repris après l'Échap, Ctrl+Maj+7 est bien retenu par l'app auprès de Windows.
    $t = Tenir-Ailleurs 0x37 6 'ctrl-maj-7'
    Verifier '(6) Ctrl+Maj+7 repris par l''app (une autre application ne peut pas le prendre)' ($t -eq 'refuse') $t | Out-Null

    # (7) Lancer au démarrage, sur la clé d'essai.
    $commande = '"' + $script:Essai.Exe + '"'
    Cliquer-Nom $fen 'Général' ([System.Windows.Automation.ControlType]::TabItem) '(7) rubrique Général'
    Cliquer-Interrupteur $fen 'Lancer au démarrage' '(7) interrupteur'
    $null = Attendre-Motif -Motif 'demarrage avec le systeme ecrit: oui$' -Dossier $cas -DelaiS 5 -Quoi '(7) écrit'
    Verifier '(7) clé d''essai : la valeur est l''exe entre guillemets' ((Valeur-Run $CLE_RUN_ESSAI) -eq $commande) "$(Valeur-Run $CLE_RUN_ESSAI)" | Out-Null
    Verifier '(7) fichier : demarrer_avec_systeme true' ((Lire-Reglages $cas).demarrer_avec_systeme -eq $true) | Out-Null
    Remove-ItemProperty -LiteralPath $CLE_RUN_ESSAI -Name 'BreezyShot'
    $fermes = Compter-Motif -Motif 'reglages fermes$' -Dossier $cas
    [void][BreezyShotEssai.Reglages]::PostMessage($fen, 0x10, [IntPtr]::Zero, [IntPtr]::Zero)
    $null = Attendre-Suivante 'reglages fermes$' $cas $fermes '(7) fermée'
    $ouvertes = Compter-Motif -Motif 'reglages fenetre \d+$' -Dossier $cas
    Ouvrir-Par-Menu '(7) réouverture'
    $l = Attendre-Suivante 'reglages fenetre (\d+)$' $cas $ouvertes '(7) fenêtre recréée' 10
    $null = $l -match 'reglages fenetre (\d+)$'; $fen = [IntPtr][long]$Matches[1]
    Verifier '(7) état relu dans le système : inactif' ((Derniere-Ligne 'demarrage avec le systeme: ' $cas) -match 'demarrage avec le systeme: inactif$') | Out-Null
    Cliquer-Nom $fen 'Général' ([System.Windows.Automation.ControlType]::TabItem) '(7) rubrique Général'
    Start-Sleep -Milliseconds 300
    Verifier '(7) interrupteur éteint (le fichier dit pourtant oui)' ((Etat-Interrupteur $fen 'Lancer au démarrage') -eq 'Off') (Etat-Interrupteur $fen 'Lancer au démarrage') | Out-Null
    Cliquer-Interrupteur $fen 'Lancer au démarrage' '(7) allumer'
    $null = Attendre-Motif -Motif 'demarrage avec le systeme ecrit: oui$' -Dossier $cas -DelaiS 5 -Quoi '(7) rallumé' -Fois 2
    Cliquer-Interrupteur $fen 'Lancer au démarrage' '(7) éteindre'
    $null = Attendre-Motif -Motif 'demarrage avec le systeme ecrit: non$' -Dossier $cas -DelaiS 5 -Quoi '(7) éteint'
    Verifier '(7) éteint : valeur absente de la clé d''essai' ($null -eq (Valeur-Run $CLE_RUN_ESSAI)) | Out-Null
    Verifier '(7) vraie clé Run identique' ((Valeur-Run $CLE_RUN) -eq $reelAvant) | Out-Null

    # (8) Parcourir… : un dossier d'essai choisi dans le sélecteur du système.
    $jeton = 'choisi-' + [guid]::NewGuid().ToString('N').Substring(0, 8)
    $choisi = Join-Path $script:Essai.Dossier $jeton
    New-Item -ItemType Directory -Force -Path $choisi | Out-Null
    Cliquer-Nom $fen 'Après la capture' ([System.Windows.Automation.ControlType]::TabItem) '(8) rubrique'
    # Enregistrement éteint jusqu'ici (fichier du cas) : aucune capture dans Images. Puis
    # allumé par la fenêtre : la capture suivante est enregistrée, sans redémarrer.
    $images = @(Get-ChildItem -LiteralPath (Join-Path $cas 'images') -Recurse -Filter '*.png' -ErrorAction SilentlyContinue)
    Verifier '(8) enregistrement éteint : aucune capture dans Images' ($images.Count -eq 0) "$($images.Count)" | Out-Null
    Cliquer-Interrupteur $fen 'Enregistrer automatiquement' '(8) interrupteur Enregistrer'
    $null = Attendre-Motif -Motif 'reglage change: enregistrer oui$' -Dossier $cas -DelaiS 5 -Quoi '(8) enregistrer oui'
    Verifier '(8) fichier : enregistrer_apres_capture true' ((Lire-Reglages $cas).enregistrer_apres_capture -eq $true) | Out-Null
    $dialoguesAvant = @([BreezyShotEssai.Reglages]::Fenetres($p.Id, '#32770') | Where-Object { [BreezyShotEssai.Reglages]::IsWindowVisible($_) }).Count
    Cliquer-Nom $fen 'Parcourir…' ([System.Windows.Automation.ControlType]::Button) '(8) Parcourir'
    # Le sélecteur de Windows met de 1 à plus de 13 s à s'ouvrir sur la machine de GitHub
    # (mesuré). Un clic parti pendant que les lignes Format et Nom se déplient peut aussi
    # tomber à côté : sans sélecteur au bout de 15 s, un second clic sur le bouton relu (sans
    # effet si le premier a porté : l'app n'ouvre qu'un sélecteur à la fois).
    $dialogue = [IntPtr]::Zero
    $clics = 1
    $chronoDialogue = [System.Diagnostics.Stopwatch]::StartNew()
    do {
        $d = @([BreezyShotEssai.Reglages]::Fenetres($p.Id, '#32770') | Where-Object { [BreezyShotEssai.Reglages]::IsWindowVisible($_) })
        if ($d.Count -gt $dialoguesAvant) { $dialogue = $d[0]; break }
        if ($clics -eq 1 -and $chronoDialogue.Elapsed.TotalSeconds -ge 15) {
            Cliquer-Nom $fen 'Parcourir…' ([System.Windows.Automation.ControlType]::Button) '(8) Parcourir, second clic'
            $clics = 2
        }
        Start-Sleep -Milliseconds 100
    } while ($chronoDialogue.Elapsed.TotalSeconds -lt 45)
    $ouParcourir = Decrire (Element $fen 'Parcourir…' ([System.Windows.Automation.ControlType]::Button))
    if (-not (Verifier '(8) le sélecteur de dossier s''ouvre' ($dialogue -ne [IntPtr]::Zero) "$clics clic(s) ; $ouParcourir")) { throw '(8) pas de sélecteur' }
    Noter-Mesure -Nom 'ouverture du sélecteur de dossier' -Valeur ([math]::Round($chronoDialogue.Elapsed.TotalSeconds, 1)) -Unite 's'
    # Champ « Dossier : » (1152) puis bouton de validation (1) du dialogue de Windows,
    # attendus. Introuvables : le message liste les identifiants (pas les noms, qui peuvent
    # être des noms de dossiers) des champs et boutons vus.
    function Par-Id([IntPtr]$Racine, [string]$Id, $Type, [string]$Quoi) {
        $cond = [System.Windows.Automation.PropertyCondition]::new($AE::AutomationIdProperty, $Id)
        if ($null -ne $Type) {
            $cond = [System.Windows.Automation.AndCondition]::new($cond,
                [System.Windows.Automation.PropertyCondition]::new($AE::ControlTypeProperty, $Type))
        }
        $err = ''
        $limite = [DateTime]::UtcNow.AddSeconds(5)
        do {
            try {
                $e = $AE::FromHandle($Racine).FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
                if ($null -ne $e) { return $e }
            } catch { $err = $_.Exception.Message }
            Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $limite)
        $ids = @()
        try {
            foreach ($x in $AE::FromHandle($Racine).FindAll([System.Windows.Automation.TreeScope]::Descendants,
                    [System.Windows.Automation.Condition]::TrueCondition)) {
                $c = $x.Current
                if ($c.ControlType -eq [System.Windows.Automation.ControlType]::Edit -or $c.ControlType -eq [System.Windows.Automation.ControlType]::Button) {
                    $ids += "$($c.ControlType.ProgrammaticName -replace '^ControlType\.', '') #$($c.AutomationId)"
                }
            }
        } catch { $ids += "UI Automation en échec : $($_.Exception.Message)" }
        $d = "identifiant $Id introuvable$(if ($err) { " ; dernière erreur : $err" }) ; champs et boutons vus : $($ids -join ', ')"
        Verifier "$Quoi : trouvé" $false $d | Out-Null
        throw "$Quoi : $d"
    }
    # Comme une personne : clic dans le champ « Dossier : », tout sélectionner, taper le
    # chemin, Entrée. (Un texte posé par WM_SETTEXT n'est pas pris par le sélecteur : il
    # rend alors son dossier de départ, vu sur la machine de GitHub.) Selon Windows, Entrée
    # valide le dossier tapé ou y entre seulement : si le sélecteur reste ouvert, son bouton
    # de validation (contrôle 1, IDOK) est pressé, et le dossier courant est le bon.
    $champ = Par-Id $dialogue '1152' $null '(8) champ du dossier'
    $null = [BreezyShotEssai.Reglages]::SetForegroundWindow($dialogue)
    Start-Sleep -Milliseconds 200
    $rc = $champ.Current.BoundingRectangle
    [BreezyShotEssai.Reglages]::Cliquer([int]($rc.X + $rc.Width / 2), [int]($rc.Y + $rc.Height / 2))
    Start-Sleep -Milliseconds 200
    [BreezyShotEssai.Reglages]::Appuyer([byte[]]@(0x11), 0x41)
    Start-Sleep -Milliseconds 100
    $tapes = [BreezyShotEssai.Reglages]::Taper($choisi)
    Start-Sleep -Milliseconds 400
    $lu = ''
    try { $lu = [string]$champ.Current.Name } catch { $lu = "illisible : $($_.Exception.Message)" }
    if (-not (Verifier '(8) dossier tapé dans le champ du sélecteur' ($tapes -eq $choisi.Length -and $lu.StartsWith($choisi, [System.StringComparison]::OrdinalIgnoreCase)) "$tapes caractère(s) sur $($choisi.Length) ; champ : « $lu » ; $(Decrire $champ)")) {
        [BreezyShotEssai.Reglages]::Appuyer([byte[]]@(), 0x1B)
        throw '(8) champ du dossier non écrit'
    }
    [BreezyShotEssai.Reglages]::Appuyer([byte[]]@(), 0x0D)
    $valide = 'Entrée'
    $limite = [DateTime]::UtcNow.AddSeconds(3)
    while ([BreezyShotEssai.Reglages]::IsWindowVisible($dialogue) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 100 }
    if ([BreezyShotEssai.Reglages]::IsWindowVisible($dialogue)) {
        $hOk = [BreezyShotEssai.Reglages]::GetDlgItem($dialogue, 1)
        if ($hOk -ne [IntPtr]::Zero -and [BreezyShotEssai.Reglages]::PostMessage($hOk, 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero)) { $valide = 'Entrée puis bouton de validation (BM_CLICK)' }
        elseif ([BreezyShotEssai.Reglages]::PostMessage($dialogue, 0x0111, [IntPtr]1, [IntPtr]::Zero)) { $valide = 'Entrée puis WM_COMMAND IDOK' }
        $limite = [DateTime]::UtcNow.AddSeconds(10)
        while ([BreezyShotEssai.Reglages]::IsWindowVisible($dialogue) -and [DateTime]::UtcNow -lt $limite) { Start-Sleep -Milliseconds 100 }
    }
    if (-not (Verifier '(8) le sélecteur se ferme' (-not [BreezyShotEssai.Reglages]::IsWindowVisible($dialogue)) $valide)) {
        $null = [BreezyShotEssai.Reglages]::PostMessage($dialogue, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
        throw '(8) sélecteur resté ouvert'
    }
    $null = Attendre-Motif -Motif 'reglage change: dossier$' -Dossier $cas -DelaiS 10 -Quoi '(8) dossier changé'
    # Le journal est écrit juste avant le fichier : le fichier est relu jusqu'à 5 s.
    $ecritFichier = $null
    $limite = [DateTime]::UtcNow.AddSeconds(5)
    do {
        $ecritFichier = (Lire-Reglages $cas).dossier_enregistrement
        if ($ecritFichier -eq $choisi) { break }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $limite)
    Verifier '(8) fichier : dossier choisi' ($ecritFichier -eq $choisi) "fichier : « $ecritFichier » ; attendu : « $choisi » ; $valide" | Out-Null
    $n = Presses; $repos = Compter-Motif -Motif 'Decoupe -> Repos' -Dossier $cas
    [BreezyShotEssai.Reglages]::Appuyer($CTRL_MAJ, 0x37)
    $null = Attendre-Suivante 'raccourci presse: plein_ecran$' $cas $n '(8) capture'
    Attendre-Repos $repos
    $pngs = @()
    $limite = [DateTime]::UtcNow.AddSeconds(5)
    do {
        $pngs = @(Get-ChildItem -LiteralPath $choisi -Filter '*.png' -ErrorAction SilentlyContinue)
        if ($pngs.Count -ge 1) { break }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $limite)
    $ailleurs = @(Get-ChildItem -LiteralPath (Join-Path $cas 'images') -Recurse -Filter '*.png' -ErrorAction SilentlyContinue).Count
    Verifier '(8) la capture est enregistrée dans le dossier choisi' ($pngs.Count -eq 1) "$($pngs.Count) dans le dossier choisi, $ailleurs dans Images" | Out-Null
    Verifier '(8) le chemin choisi n''est jamais au journal' ((Compter-Lignes -Texte $jeton -Dossier $cas) -eq 0) | Out-Null

    # (9) Fichier changé de l'extérieur pendant l'ouverture : jamais écrasé.
    $r = Lire-Reglages $cas
    $r.modele_nom = 'Externe {date}'
    Ecrire-Reglages -Dossier $cas -Objet $r
    $exterieurs = Compter-Motif -Motif 'reglages relus: changes exterieurs$' -Dossier $cas
    Cliquer-Interrupteur $fen 'Copier automatiquement' '(9) Copier'
    $null = Attendre-Suivante 'reglages relus: changes exterieurs$' $cas $exterieurs '(9) relu'
    Start-Sleep -Milliseconds 300
    $r = Lire-Reglages $cas
    Verifier '(9) changement extérieur gardé, nouveau changement écrit' (($r.modele_nom -eq 'Externe {date}') -and ($r.copier_apres_capture -eq $false)) "$($r.modele_nom) / $($r.copier_apres_capture)" | Out-Null
    Cliquer-Interrupteur $fen 'Copier automatiquement' '(9) Copier rallumé'

    # (10) Thème : le fond de la page prend le jeton --reglages-fond de l'habillage B, clair
    # (#FAFAFA) puis sombre (#1C1C1C). Pixel lu dans la zone de contenu (en bas à droite, sous les groupes),
    # jamais dans la colonne des rubriques ; lu en Clair d'abord pour prouver le changement.
    function Pixel-Contenu {
        $rc = New-Object BreezyShotEssai.Reglages+RECT
        $null = [BreezyShotEssai.Reglages]::GetClientRect($fen, [ref]$rc)
        $pt = New-Object BreezyShotEssai.Reglages+POINT
        $pt.X = $rc.Right - 40; $pt.Y = $rc.Bottom - 20
        $null = [BreezyShotEssai.Reglages]::ClientToScreen($fen, [ref]$pt)
        $bmp = [System.Drawing.Bitmap]::new(1, 1)
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        $g.CopyFromScreen($pt.X, $pt.Y, 0, 0, [System.Drawing.Size]::new(1, 1))
        $c = $bmp.GetPixel(0, 0); $g.Dispose(); $bmp.Dispose()
        return $c
    }
    function Proche($c, [int]$v) { return ([math]::Abs($c.R - $v) -le 6) -and ([math]::Abs($c.G - $v) -le 6) -and ([math]::Abs($c.B - $v) -le 6) }
    Cliquer-Nom $fen 'Général' ([System.Windows.Automation.ControlType]::TabItem) '(10) rubrique'
    Cliquer-Nom $fen 'Clair' $RADIO '(10) Clair'
    $null = Attendre-Motif -Motif 'theme interface: clair$' -Dossier $cas -DelaiS 5 -Quoi '(10) thème clair'
    Start-Sleep -Milliseconds 600
    $c = Pixel-Contenu
    Verifier '(10) fond clair (#FAFAFA ±6)' (Proche $c 0xFA) "$($c.R),$($c.G),$($c.B)" | Out-Null
    Cliquer-Nom $fen 'Sombre' $RADIO '(10) Sombre'
    $null = Attendre-Motif -Motif 'theme interface: sombre$' -Dossier $cas -DelaiS 5 -Quoi '(10) thème sombre'
    Start-Sleep -Milliseconds 600
    $c = Pixel-Contenu
    Verifier '(10) fond sombre (#1C1C1C ±6)' (Proche $c 0x1C) "$($c.R),$($c.G),$($c.B)" | Out-Null
    Cliquer-Nom $fen 'Système' $RADIO '(10) Système'

    # (11) Fermer : la fenêtre disparaît, l'app tourne.
    $fermes = Compter-Motif -Motif 'reglages fermes$' -Dossier $cas
    [void][BreezyShotEssai.Reglages]::PostMessage($fen, 0x10, [IntPtr]::Zero, [IntPtr]::Zero)
    $null = Attendre-Suivante 'reglages fermes$' $cas $fermes '(11) fermée'
    Start-Sleep -Milliseconds 500
    Verifier '(11) fenêtre détruite, app toujours là' ((-not [BreezyShotEssai.Reglages]::IsWindow($fen)) -and (-not $p.HasExited)) | Out-Null
    Noter-Mesure -Nom 'memoire privee apres fermeture des reglages' -Valeur (Memoire-Privee-Mo -Processus $p) -Unite 'Mo'
    $arrets = Compter-Motif -Motif 'arret propre$' -Dossier $cas
    $q = Quitter-App -DelaiS 10 -Dossier $cas
    Verifier '(11) --quitter → arret propre' ($q.Fini -and $p.WaitForExit(10000)) | Out-Null
    $null = Attendre-Suivante 'arret propre$' $cas $arrets '(11) arrêt'
}
catch {
    Verifier "essai interrompu : $_" $false | Out-Null
}
finally {
    foreach ($a in $script:Auxiliaires) { try { if (-not $a.HasExited) { $a.Kill() } } catch { } }
    if ($IsWindows) {
        Remove-Item -LiteralPath $CLE_ESSAI -Recurse -Force -ErrorAction SilentlyContinue
        Verifier 'vraie clé Run jamais touchée' ((Valeur-Run $CLE_RUN) -eq $reelAvant) | Out-Null
    }
    Terminer-Essai
}
