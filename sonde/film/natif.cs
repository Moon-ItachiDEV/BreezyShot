// Aides natives de la sonde du film (contrat 0.7, partie I), chargées par Add-Type
// (PowerShell 7). Aucune dépendance hors de .NET : les images sont des byte[] BGRA de haut
// en bas, largeur l.
//
// Temoin : le même dessin que sonde/film/rust/src/lib.rs (module temoin), au pixel près :
// mêmes constantes, mêmes couleurs. La fenêtre WPF de la sonde le peint ; la relecture des
// GIF et des images prises le lit.
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;

namespace BreezyShotFilm
{
    public static class Temoin
    {
        public const int L = 600, H = 450;
        public const int CARRES = 16, CARRE = 32, CARRE_PAS = 36, CARRE_X = 12, CARRE_Y = 12;
        public const int BANDE_Y = 60, BANDE_H = 40;
        public const int PASTILLE = 48, PASTILLE_Y = 390;
        public static readonly int[] PASTILLES_X = { 20, 90, 160, 230 };
        public static readonly int[,] PASTILLES = { { 200, 60, 60 }, { 60, 170, 80 }, { 60, 90, 200 }, { 230, 200, 60 } };

        // genre : 0 simple, 1 fixe, 2 plein.
        public static int[] Couleur(int k)
        {
            return new int[] { (k * 67 + 40) & 255, (k * 29 + 120) & 255, (k * 151 + 200) & 255 };
        }

        static int[] Fond(int x, int y, int k, int genre)
        {
            if (genre == 2) return new int[] { (x + 3 * k) & 255, (y + 2 * k) & 255, ((x + y) / 2 + 5 * k) & 255 };
            if (y % 30 < 2) return new int[] { 225, 228, 234 };
            if (y >= 120 && y < 360 && (y % 30) >= 10 && (y % 30) < 20)
            {
                int rangee = y / 30;
                int fin = 20 + 100 + (rangee * 73) % 400;
                if (x >= 20 && x < fin) return new int[] { 60, 60, 60 };
            }
            return new int[] { 255, 255, 255 };
        }

        public static int[] Pixel(int x, int y, int k, int genre)
        {
            if (genre == 1) k = 0;
            if (y >= CARRE_Y && y < CARRE_Y + CARRE)
                for (int i = 0; i < CARRES; i++)
                {
                    int x0 = CARRE_X + i * CARRE_PAS;
                    if (x >= x0 && x < x0 + CARRE)
                        return ((k >> (CARRES - 1 - i)) & 1) == 1 ? new int[] { 0, 0, 0 } : new int[] { 255, 255, 255 };
                }
            if (y >= BANDE_Y && y < BANDE_Y + BANDE_H) return Couleur(k);
            if (y >= PASTILLE_Y && y < PASTILLE_Y + PASTILLE)
                for (int i = 0; i < 4; i++)
                    if (x >= PASTILLES_X[i] && x < PASTILLES_X[i] + PASTILLE) return new int[] { PASTILLES[i, 0], PASTILLES[i, 1], PASTILLES[i, 2] };
            return Fond(x, y, k, genre);
        }

        /// Peint tout le témoin dans `b` (BGRA, L × H, pas de L × 4).
        public static void Peindre(byte[] b, int k, int genre) { PeindreLignes(b, k, genre, H); }

        /// Ce qui change d'un pas à l'autre : le haut (carrés et bande) pour le témoin simple,
        /// tout pour le plein. Rend le nombre de lignes peintes.
        public static int PeindrePas(byte[] b, int k, int genre)
        {
            int n = genre == 2 ? H : BANDE_Y + BANDE_H;
            PeindreLignes(b, k, genre, n);
            return n;
        }

        static void PeindreLignes(byte[] b, int k, int genre, int lignes)
        {
            for (int y = 0; y < lignes; y++)
                for (int x = 0; x < L; x++)
                {
                    int[] c = Pixel(x, y, k, genre);
                    int i = (y * L + x) * 4;
                    b[i] = (byte)c[2]; b[i + 1] = (byte)c[1]; b[i + 2] = (byte)c[0]; b[i + 3] = 255;
                }
        }

        static bool Moyenne(byte[] a, int l, int cx, int cy, int n, double[] m)
        {
            int h = a.Length / 4 / l; double r = 0, v = 0, bl = 0; int c = 0;
            for (int y = cy - n / 2; y < cy + n / 2; y++)
                for (int x = cx - n / 2; x < cx + n / 2; x++)
                {
                    if (x < 0 || y < 0 || x >= l || y >= h) return false;
                    int i = (y * l + x) * 4;
                    r += a[i + 2]; v += a[i + 1]; bl += a[i]; c++;
                }
            if (c == 0) return false;
            m[0] = r / c; m[1] = v / c; m[2] = bl / c;
            return true;
        }

        /// k lu sur les carrés (le témoin en ox, oy), ou -1.
        public static int LireK(byte[] a, int l, int ox, int oy)
        {
            int k = 0; double[] m = new double[3];
            for (int i = 0; i < CARRES; i++)
            {
                if (!Moyenne(a, l, ox + CARRE_X + i * CARRE_PAS + CARRE / 2, oy + CARRE_Y + CARRE / 2, 12, m)) return -1;
                double lum = 0.2126 * m[0] + 0.7152 * m[1] + 0.0722 * m[2];
                int bit;
                if (lum < 80) bit = 1; else if (lum > 175) bit = 0; else return -1;
                k = (k << 1) | bit;
            }
            return k;
        }

        /// Écart maximal par canal de chacune des quatre pastilles (moyenne 16 × 16 au centre),
        /// ou null hors de l'image.
        public static double[] EcartsPastilles(byte[] a, int l, int ox, int oy)
        {
            double[] e = new double[4]; double[] m = new double[3];
            for (int i = 0; i < 4; i++)
            {
                if (!Moyenne(a, l, ox + PASTILLES_X[i] + PASTILLE / 2, oy + PASTILLE_Y + PASTILLE / 2, 16, m)) return null;
                double x = 0;
                for (int c = 0; c < 3; c++) x = Math.Max(x, Math.Abs(m[c] - PASTILLES[i, c]));
                e[i] = x;
            }
            return e;
        }

        /// Pixels exacts (au bit près) des quatre pastilles entières, sur 4 × 48 × 48.
        public static int PastillesExactes(byte[] a, int l, int ox, int oy)
        {
            int h = a.Length / 4 / l, n = 0;
            for (int i = 0; i < 4; i++)
                for (int y = 0; y < PASTILLE; y++)
                    for (int x = 0; x < PASTILLE; x++)
                    {
                        int px = ox + PASTILLES_X[i] + x, py = oy + PASTILLE_Y + y;
                        if (px < 0 || py < 0 || px >= l || py >= h) continue;
                        int j = (py * l + px) * 4;
                        if (a[j + 2] == PASTILLES[i, 0] && a[j + 1] == PASTILLES[i, 1] && a[j] == PASTILLES[i, 2]) n++;
                    }
            return n;
        }

        /// Pixels d'alpha 0 (format 32 bpp ARGB).
        public static int Transparents(byte[] a)
        {
            int n = 0;
            for (int i = 3; i < a.Length; i += 4) if (a[i] == 0) n++;
            return n;
        }
    }

    public static class Natif
    {
        [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }
        [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
        [StructLayout(LayoutKind.Sequential)] struct MOUSEINPUT { public int dx; public int dy; public int mouseData; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
        [StructLayout(LayoutKind.Sequential)] struct KEYBDINPUT { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
        [StructLayout(LayoutKind.Explicit)] struct UNION { [FieldOffset(0)] public MOUSEINPUT mi; [FieldOffset(0)] public KEYBDINPUT ki; }
        [StructLayout(LayoutKind.Sequential)] struct INPUT { public uint type; public UNION u; }
        [StructLayout(LayoutKind.Sequential)] public struct CURSORINFO { public int cbSize; public int flags; public IntPtr hCursor; public POINT pt; }

        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
        public struct DEVMODE
        {
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmDeviceName;
            public short dmSpecVersion; public short dmDriverVersion; public short dmSize; public short dmDriverExtra;
            public int dmFields; public int dmPositionX; public int dmPositionY; public int dmDisplayOrientation; public int dmDisplayFixedOutput;
            public short dmColor; public short dmDuplex; public short dmYResolution; public short dmTTOption; public short dmCollate;
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmFormName;
            public short dmLogPixels; public int dmBitsPerPel; public int dmPelsWidth; public int dmPelsHeight;
            public int dmDisplayFlags; public int dmDisplayFrequency; public int dmICMMethod; public int dmICMIntent;
            public int dmMediaType; public int dmDitherType; public int dmReserved1; public int dmReserved2;
            public int dmPanningWidth; public int dmPanningHeight;
        }

        delegate bool Rappel(IntPtr h, IntPtr p);

        [DllImport("user32.dll", SetLastError = true)] public static extern bool SetProcessDpiAwarenessContext(IntPtr contexte);
        [DllImport("user32.dll")] static extern uint SendInput(uint n, INPUT[] entrees, int taille);
        [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
        [DllImport("user32.dll")] public static extern bool GetCursorInfo(ref CURSORINFO c);
        [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
        [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
        [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
        [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
        [DllImport("user32.dll")] static extern bool AllowSetForegroundWindow(int pid);
        [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder t, int n);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder t, int n);
        [DllImport("user32.dll")] static extern bool EnumWindows(Rappel f, IntPtr p);
        [DllImport("user32.dll")] public static extern int GetSystemMetrics(int i);
        [DllImport("user32.dll")] public static extern uint GetDpiForSystem();
        [DllImport("user32.dll", SetLastError = true)] static extern bool SystemParametersInfo(uint action, uint ui, ref int pv, uint ini);
        [DllImport("user32.dll", SetLastError = true, EntryPoint = "SystemParametersInfoW")] static extern bool SystemParametersInfoValeur(uint action, uint ui, IntPtr pv, uint ini);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern bool EnumDisplaySettings(string nom, int mode, ref DEVMODE dm);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int ChangeDisplaySettingsEx(string nom, ref DEVMODE dm, IntPtr h, uint drapeaux, IntPtr p);
        [DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "ChangeDisplaySettingsExW")] static extern int ChangeDisplaySettingsExNul(string nom, IntPtr dm, IntPtr h, uint drapeaux, IntPtr p);
        [DllImport("user32.dll")] static extern int GetDisplayConfigBufferSizes(uint drapeaux, out uint chemins, out uint modes);
        [DllImport("user32.dll")] static extern int QueryDisplayConfig(uint drapeaux, ref uint chemins, IntPtr tChemins, ref uint modes, IntPtr tModes, IntPtr topologie);
        [DllImport("user32.dll")] static extern int DisplayConfigGetDeviceInfo(IntPtr paquet);
        [DllImport("user32.dll")] static extern int DisplayConfigSetDeviceInfo(IntPtr paquet);
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern IntPtr LoadLibraryExW(string nom, IntPtr f, uint drapeaux);
        [DllImport("kernel32.dll")] static extern bool FreeLibrary(IntPtr m);
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern bool GetDiskFreeSpaceExW(string dossier, out ulong libre, out ulong total, out ulong toutLibre);
        [DllImport("shcore.dll")] static extern int GetDpiForMonitor(IntPtr ecran, int genre, out uint x, out uint y);
        [DllImport("user32.dll")] static extern IntPtr MonitorFromPoint(POINT p, uint drapeaux);

        // ---------------------------------------------------------------------------------
        // Fenêtres

        public static int Pid(IntPtr h) { uint p; GetWindowThreadProcessId(h, out p); return (int)p; }
        public static string Classe(IntPtr h) { var t = new StringBuilder(256); GetClassName(h, t, t.Capacity); return t.ToString(); }
        public static string Titre(IntPtr h) { var t = new StringBuilder(512); GetWindowText(h, t, t.Capacity); return t.ToString(); }

        public static string Decrire(IntPtr h)
        {
            if (h == IntPtr.Zero) return "aucune";
            string nom = "?";
            try { nom = Process.GetProcessById(Pid(h)).ProcessName; } catch { }
            string t = Titre(h);
            if (t.Length > 50) t = t.Substring(0, 50);
            return Classe(h) + " « " + t + " » (" + nom + ")";
        }

        /// Fenêtres de premier niveau visibles d'un processus (0 : toutes), du premier plan
        /// vers le fond.
        public static IntPtr[] Fenetres(int pid)
        {
            var l = new List<IntPtr>();
            EnumWindows((h, p) => { if (IsWindowVisible(h) && (pid == 0 || Pid(h) == pid)) l.Add(h); return true; }, IntPtr.Zero);
            return l.ToArray();
        }

        // ---------------------------------------------------------------------------------
        // Entrées (SendInput)

        static uint Envoyer(INPUT[] i) { return SendInput((uint)i.Length, i, Marshal.SizeOf(typeof(INPUT))); }

        public static void Poser(int x, int y)
        {
            SetCursorPos(x, y);
            INPUT i = new INPUT(); i.type = 0; i.u.mi.dwFlags = 0x1; Envoyer(new INPUT[] { i });
        }

        /// Un clic gauche au point (x, y) ; rend le nombre d'entrées acceptées (2).
        public static uint Clic(int x, int y)
        {
            Poser(x, y);
            INPUT a = new INPUT(); a.type = 0; a.u.mi.dwFlags = 0x2;
            INPUT b = new INPUT(); b.type = 0; b.u.mi.dwFlags = 0x4;
            return Envoyer(new INPUT[] { a, b });
        }

        public static uint Molette(int crans)
        {
            INPUT i = new INPUT(); i.type = 0; i.u.mi.dwFlags = 0x0800; i.u.mi.mouseData = crans * 120;
            return Envoyer(new INPUT[] { i });
        }

        public static uint Touche(ushort vk)
        {
            INPUT a = new INPUT(); a.type = 1; a.u.ki.wVk = vk;
            INPUT b = a; b.u.ki.dwFlags = 2;
            return Envoyer(new INPUT[] { a, b });
        }

        /// Ctrl + touche.
        public static uint Raccourci(ushort vk)
        {
            INPUT c = new INPUT(); c.type = 1; c.u.ki.wVk = 0x11;
            INPUT a = new INPUT(); a.type = 1; a.u.ki.wVk = vk;
            INPUT b = a; b.u.ki.dwFlags = 2;
            INPUT d = c; d.u.ki.dwFlags = 2;
            return Envoyer(new INPUT[] { c, a, b, d });
        }

        public static bool PremierPlan(IntPtr h)
        {
            for (int essai = 0; essai < 3; essai++)
            {
                if (GetForegroundWindow() == h) return true;
                AllowSetForegroundWindow(-1);
                if (essai == 1) Touche(0x12);
                BringWindowToTop(h);
                SetForegroundWindow(h);
                System.Threading.Thread.Sleep(80);
            }
            return GetForegroundWindow() == h;
        }

        public static bool PointeurMontre()
        {
            CURSORINFO c = new CURSORINFO(); c.cbSize = Marshal.SizeOf(typeof(CURSORINFO));
            return GetCursorInfo(ref c) && (c.flags & 1) != 0;
        }

        // ---------------------------------------------------------------------------------
        // Réglages

        /// SPI_GETCLIENTAREAANIMATION (Chromium s'en sert pour prefers-reduced-motion).
        public static int AnimationsClient()
        {
            int v = 0;
            return SystemParametersInfo(0x1042, 0, ref v, 0) ? v : -1 - Marshal.GetLastWin32Error();
        }

        /// SPI_SETCLIENTAREAANIMATION (valeur dans pvParam), SPIF_SENDCHANGE ; rend la valeur relue.
        public static int FixerAnimationsClient(bool oui)
        {
            SystemParametersInfoValeur(0x1043, 0, new IntPtr(oui ? 1 : 0), 0x2);
            return AnimationsClient();
        }

        // ---------------------------------------------------------------------------------
        // Écran : modes (EnumDisplaySettings / ChangeDisplaySettingsEx) et échelle (DisplayConfig)

        static DEVMODE Nouveau() { DEVMODE d = new DEVMODE(); d.dmSize = (short)Marshal.SizeOf(typeof(DEVMODE)); return d; }

        /// Mode courant « LxH@Hz bpp ».
        public static string ModeCourant()
        {
            DEVMODE d = Nouveau();
            if (!EnumDisplaySettings(null, -1, ref d)) return "illisible";
            return d.dmPelsWidth + "x" + d.dmPelsHeight + "@" + d.dmDisplayFrequency + " " + d.dmBitsPerPel + "bpp";
        }

        /// Tous les modes distincts « LxH » de l'écran principal.
        public static string[] Modes()
        {
            var v = new SortedSet<string>(StringComparer.Ordinal);
            for (int i = 0; i < 2000; i++)
            {
                DEVMODE d = Nouveau();
                if (!EnumDisplaySettings(null, i, ref d)) break;
                v.Add(d.dmPelsWidth.ToString("D5") + "x" + d.dmPelsHeight);
            }
            var r = new List<string>();
            foreach (string s in v) r.Add(s.TrimStart('0'));
            return r.ToArray();
        }

        /// Pose le mode l × h (dynamique, sans l'écrire au registre) ; rend le code
        /// DISP_CHANGE (0 : fait). `essai` : CDS_TEST seulement.
        public static int ChangerMode(int l, int h, bool essai)
        {
            DEVMODE d = Nouveau();
            if (!EnumDisplaySettings(null, -1, ref d)) return -100;
            d.dmPelsWidth = l; d.dmPelsHeight = h; d.dmFields = 0x80000 | 0x100000;
            return ChangeDisplaySettingsEx(null, ref d, IntPtr.Zero, essai ? 0x2u : 0u, IntPtr.Zero);
        }

        /// Remet le mode du registre (celui d'avant tout changement dynamique).
        public static int RemettreMode() { return ChangeDisplaySettingsExNul(null, IntPtr.Zero, IntPtr.Zero, 0, IntPtr.Zero); }

        static int[] PremiereSource()
        {
            uint nc, nm;
            if (GetDisplayConfigBufferSizes(2, out nc, out nm) != 0 || nc == 0) return null;
            IntPtr c = Marshal.AllocHGlobal((int)nc * 72), m = Marshal.AllocHGlobal((int)nm * 64);
            try
            {
                if (QueryDisplayConfig(2, ref nc, c, ref nm, m, IntPtr.Zero) != 0) return null;
                return new int[] { Marshal.ReadInt32(c, 0), Marshal.ReadInt32(c, 4), Marshal.ReadInt32(c, 8) };
            }
            finally { Marshal.FreeHGlobal(c); Marshal.FreeHGlobal(m); }
        }

        /// {code, min, actuel, max} : pas relatifs à l'échelle recommandée (0 = recommandée).
        public static int[] EchelleRelative()
        {
            int[] s = PremiereSource();
            if (s == null) return new int[] { -1, 0, 0, 0 };
            IntPtr p = Marshal.AllocHGlobal(32);
            try
            {
                Marshal.WriteInt32(p, 0, -3); Marshal.WriteInt32(p, 4, 32);
                Marshal.WriteInt32(p, 8, s[0]); Marshal.WriteInt32(p, 12, s[1]); Marshal.WriteInt32(p, 16, s[2]);
                int r = DisplayConfigGetDeviceInfo(p);
                return new int[] { r, Marshal.ReadInt32(p, 20), Marshal.ReadInt32(p, 24), Marshal.ReadInt32(p, 28) };
            }
            finally { Marshal.FreeHGlobal(p); }
        }

        public static int FixerEchelleRelative(int rel)
        {
            int[] s = PremiereSource();
            if (s == null) return -1;
            IntPtr p = Marshal.AllocHGlobal(24);
            try
            {
                Marshal.WriteInt32(p, 0, -4); Marshal.WriteInt32(p, 4, 24);
                Marshal.WriteInt32(p, 8, s[0]); Marshal.WriteInt32(p, 12, s[1]); Marshal.WriteInt32(p, 16, s[2]);
                Marshal.WriteInt32(p, 20, rel);
                return DisplayConfigSetDeviceInfo(p);
            }
            finally { Marshal.FreeHGlobal(p); }
        }

        /// La garde de F31 : LoadLibraryExW(nom, LOAD_LIBRARY_SEARCH_SYSTEM32) ; « 1 » ou
        /// « 0 (code) » ; le module est rendu ensuite.
        public static string Garde(string nom)
        {
            IntPtr m = LoadLibraryExW(nom, IntPtr.Zero, 0x800);
            if (m == IntPtr.Zero) return "0 (GetLastError " + Marshal.GetLastWin32Error() + ")";
            FreeLibrary(m);
            return "1";
        }

        /// {libre pour l'appelant, total} du volume qui porte `dossier` (point de montage
        /// compris) ; {-1, GetLastError} en cas d'échec.
        public static long[] Disque(string dossier)
        {
            ulong l, t, tl;
            if (!GetDiskFreeSpaceExW(dossier, out l, out t, out tl)) return new long[] { -1, Marshal.GetLastWin32Error() };
            return new long[] { (long)l, (long)t };
        }

        public static uint DpiEcran(int x, int y)
        {
            POINT p; p.X = x; p.Y = y;
            uint dx, dy;
            return GetDpiForMonitor(MonitorFromPoint(p, 2), 0, out dx, out dy) == 0 ? dx : 0;
        }

        // ---------------------------------------------------------------------------------
        // Pixels

        /// Pixels différents (B, V ou R) entre deux images de même taille ; -1 si les tailles
        /// diffèrent.
        public static int Differences(byte[] a, byte[] b)
        {
            if (a == null || b == null || a.Length != b.Length) return -1;
            int n = 0;
            for (int i = 0; i < a.Length; i += 4)
                if (a[i] != b[i] || a[i + 1] != b[i + 1] || a[i + 2] != b[i + 2]) n++;
            return n;
        }

        /// Différences hors des `bas` dernières lignes (la barre des tâches et son horloge).
        public static int DifferencesSansBas(byte[] a, byte[] b, int l, int bas)
        {
            if (a == null || b == null || a.Length != b.Length) return -1;
            int h = a.Length / 4 / l, n = 0;
            for (int i = 0; i < Math.Max(0, h - bas) * l * 4; i += 4)
                if (a[i] != b[i] || a[i + 1] != b[i + 1] || a[i + 2] != b[i + 2]) n++;
            return n;
        }
    }
}
