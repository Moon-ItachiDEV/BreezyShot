// Aides natives de l'essai 15 (video et GIF), chargees par Add-Type (PowerShell 7) dans
// essais/15-video-gif.ps1 et dans le temoin anime (essais/temoin-anime.ps1). Reprises de la
// sonde du film (sonde/film/natif.cs, contrat 0.7 partie I), qui les a prouvees sur la
// machine GitHub ; les gardes des essais 12 a 14 en plus. Les images sont des byte[] BGRA de
// haut en bas, largeur l. Fichier ASCII.
//
// Temoin : le dessin du temoin, au pixel pres (memes constantes, memes couleurs que la
// sonde) : 16 carres noirs ou blancs qui ecrivent le compteur k, une bande de la couleur
// Couleur(k), quatre pastilles de couleurs fixes, un fond de lignes et de barres.
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

        /// Peint tout le temoin dans `b` (BGRA, L x H, pas de L x 4).
        public static void Peindre(byte[] b, int k, int genre) { PeindreLignes(b, k, genre, H); }

        /// Ce qui change d'un pas a l'autre : le haut (carres et bande) pour le temoin simple,
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

        /// Les points ou l'essai lit un film (pixels du temoin) : les centres des 16 carres,
        /// des 4 pastilles, de la bande, puis 8 points blancs a 2 px des bords.
        public static int[] Points()
        {
            var p = new List<int>();
            for (int i = 0; i < CARRES; i++) { p.Add(CARRE_X + i * CARRE_PAS + CARRE / 2); p.Add(CARRE_Y + CARRE / 2); }
            for (int i = 0; i < 4; i++) { p.Add(PASTILLES_X[i] + PASTILLE / 2); p.Add(PASTILLE_Y + PASTILLE / 2); }
            p.Add(300); p.Add(BANDE_Y + BANDE_H / 2);
            int[] bords = { 2, 6, 597, 6, 2, 447, 597, 447, 300, 2, 300, 447, 2, 225, 597, 225 };
            p.AddRange(bords);
            return p.ToArray();
        }

        /// k lu sur 16 couleurs (r, v, b a la suite) des centres des carres, ou -1.
        public static int KDeCouleurs(int[] rvb)
        {
            if (rvb == null || rvb.Length < 48) return -1;
            int k = 0;
            for (int i = 0; i < CARRES; i++)
            {
                double lum = 0.2126 * rvb[3 * i] + 0.7152 * rvb[3 * i + 1] + 0.0722 * rvb[3 * i + 2];
                int bit;
                if (lum < 80) bit = 1; else if (lum > 175) bit = 0; else return -1;
                k = (k << 1) | bit;
            }
            return k;
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

        /// k lu sur les carres d'une image BGRA (le temoin en ox, oy), ou -1.
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

        /// Pixels exacts (au bit pres) des quatre pastilles entieres, sur 4 x 48 x 48.
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

        /// Ecart maximal par canal, sur les huit points des bords (Points(), apres la bande),
        /// entre l'image BGRA et le blanc du temoin ; -1 hors de l'image.
        public static int EcartBords(byte[] a, int l)
        {
            int h = a.Length / 4 / l, e = 0;
            int[] p = Points();
            for (int j = 2 * (CARRES + 5); j + 1 < p.Length; j += 2)
            {
                int x = p[j], y = p[j + 1];
                if (x >= l || y >= h) return -1;
                int i = (y * l + x) * 4;
                for (int c = 0; c < 3; c++) e = Math.Max(e, Math.Abs(255 - a[i + c]));
            }
            return e;
        }
    }

    public static class Natif
    {
        [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }
        [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
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
        [DllImport("user32.dll")] static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
        [DllImport("user32.dll")] static extern uint MapVirtualKey(uint code, uint type);
        [DllImport("user32.dll")] static extern void mouse_event(uint flags, int dx, int dy, uint data, UIntPtr extra);
        [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
        [DllImport("user32.dll")] static extern bool GetCursorPos(out POINT p);
        [DllImport("user32.dll")] static extern bool GetCursorInfo(ref CURSORINFO c);
        [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
        [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
        [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
        [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
        [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")] public static extern IntPtr GetWindowLongPtr(IntPtr h, int index);
        [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
        [DllImport("user32.dll")] public static extern uint GetClipboardSequenceNumber();
        [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder t, int n);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder t, int n);
        [DllImport("user32.dll")] static extern bool EnumWindows(Rappel f, IntPtr p);
        [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(POINT p);
        [DllImport("user32.dll")] static extern IntPtr GetAncestor(IntPtr h, uint quoi);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern bool EnumDisplaySettings(string nom, int mode, ref DEVMODE dm);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int ChangeDisplaySettingsEx(string nom, ref DEVMODE dm, IntPtr h, uint drapeaux, IntPtr p);
        [DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "ChangeDisplaySettingsExW")] static extern int ChangeDisplaySettingsExNul(string nom, IntPtr dm, IntPtr h, uint drapeaux, IntPtr p);
        [DllImport("user32.dll")] static extern int GetDisplayConfigBufferSizes(uint drapeaux, out uint chemins, out uint modes);
        [DllImport("user32.dll")] static extern int QueryDisplayConfig(uint drapeaux, ref uint chemins, IntPtr tChemins, ref uint modes, IntPtr tModes, IntPtr topologie);
        [DllImport("user32.dll")] static extern int DisplayConfigGetDeviceInfo(IntPtr paquet);
        [DllImport("user32.dll")] static extern int DisplayConfigSetDeviceInfo(IntPtr paquet);
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern bool GetDiskFreeSpaceExW(string dossier, out ulong libre, out ulong total, out ulong toutLibre);
        [DllImport("shcore.dll")] static extern int GetDpiForMonitor(IntPtr ecran, int genre, out uint x, out uint y);
        [DllImport("user32.dll")] static extern IntPtr MonitorFromPoint(POINT p, uint drapeaux);
        [DllImport("winmm.dll")] static extern uint timeBeginPeriod(uint ms);
        [DllImport("winmm.dll")] static extern uint timeEndPeriod(uint ms);

        // ---------------------------------------------------------------------------------
        // Fenetres

        public static int Pid(IntPtr h) { uint p; GetWindowThreadProcessId(h, out p); return (int)p; }
        public static string Classe(IntPtr h) { var t = new StringBuilder(256); GetClassName(h, t, t.Capacity); return t.ToString(); }
        public static string Titre(IntPtr h) { var t = new StringBuilder(512); GetWindowText(h, t, t.Capacity); return t.ToString(); }

        public static string Decrire(IntPtr h)
        {
            if (h == IntPtr.Zero) return "aucune fenetre";
            string nom = "?";
            try { nom = Process.GetProcessById(Pid(h)).ProcessName; } catch { }
            return Classe(h) + " (" + (long)h + "), processus " + nom;
        }

        /// Fenetres de premier niveau visibles d'un processus, du premier plan vers le fond.
        public static IntPtr[] Visibles(int pid)
        {
            var l = new List<IntPtr>();
            EnumWindows((h, p) => { if (IsWindowVisible(h) && Pid(h) == pid) l.Add(h); return true; }, IntPtr.Zero);
            return l.ToArray();
        }

        /// La fenetre de premier niveau sous le point (GA_ROOT).
        public static IntPtr Sous(int x, int y)
        {
            POINT p; p.X = x; p.Y = y;
            IntPtr h = WindowFromPoint(p);
            return h == IntPtr.Zero ? IntPtr.Zero : GetAncestor(h, 2);
        }

        /// Styles etendus (GWL_EXSTYLE).
        public static long StylesEtendus(IntPtr h) { return (long)GetWindowLongPtr(h, -20); }

        public static int[] Rect(IntPtr h)
        {
            RECT r;
            if (!GetWindowRect(h, out r)) return null;
            return new int[] { r.Left, r.Top, r.Right - r.Left, r.Bottom - r.Top };
        }

        // ---------------------------------------------------------------------------------
        // Entrees, avec les gardes des essais 12 a 14 : un appui de souris ne part que si le
        // point vise est sur une fenetre de l'app (PidApp) ou d'un temoin (PidsPermis) ; une
        // touche n'est qu'un raccourci global que l'app tient (RegisterHotKey : la touche ne
        // va a aucune fenetre), l'essai le verifie avant d'appeler AppuyerGlobal.

        public static int PidApp;
        public static int[] PidsPermis = new int[0];

        static bool Permise(IntPtr h)
        {
            if (h == IntPtr.Zero) return false;
            int pid = Pid(h);
            if (pid == 0) return false;
            if (PidApp != 0 && pid == PidApp) return true;
            foreach (int p in PidsPermis) if (p == pid) return true;
            return false;
        }

        public static bool SurApp(int x, int y) { IntPtr h = Sous(x, y); return PidApp != 0 && h != IntPtr.Zero && Pid(h) == PidApp; }

        const uint RELACHEE = 0x2;
        static void Bas(byte vk) { keybd_event(vk, (byte)MapVirtualKey(vk, 0), 0u, UIntPtr.Zero); }
        static void Haut(byte vk) { keybd_event(vk, (byte)MapVirtualKey(vk, 0), RELACHEE, UIntPtr.Zero); }
        public static void AppuyerGlobal(byte[] mods, byte touche)
        {
            foreach (var m in mods) Bas(m);
            Bas(touche);
            System.Threading.Thread.Sleep(30);
            Haut(touche);
            for (int i = mods.Length - 1; i >= 0; i--) Haut(mods[i]);
            System.Threading.Thread.Sleep(30);
        }

        public static void Poser(int x, int y) { SetCursorPos(x, y); mouse_event(0x1, 0, 0, 0, UIntPtr.Zero); }

        public static void BoutonBas()
        {
            POINT p; GetCursorPos(out p);
            IntPtr h = Sous(p.X, p.Y);
            if (!Permise(h))
                throw new InvalidOperationException("appui de souris en " + p.X + "," + p.Y + " non envoye : ce point n'est ni sur l'app ni sur un temoin (" + Decrire(h) + ")");
            mouse_event(0x2, 0, 0, 0, UIntPtr.Zero);
        }
        public static void BoutonHaut() { mouse_event(0x4, 0, 0, 0, UIntPtr.Zero); }

        public static void Cliquer(int x, int y)
        {
            Poser(x, y); System.Threading.Thread.Sleep(40);
            BoutonBas(); System.Threading.Thread.Sleep(60); BoutonHaut();
        }

        /// Pose la souris, puis attend que ce point soit sur une fenetre de l'app (la fenetre
        /// de controle ne prend les clics que lorsque sa surveillance a vu la souris sur la
        /// pilule).
        public static bool PoserSurApp(int x, int y, int ms)
        {
            Poser(x, y);
            DateTime fin = DateTime.UtcNow.AddMilliseconds(ms);
            do
            {
                if (SurApp(x, y)) return true;
                System.Threading.Thread.Sleep(20);
                Poser(x, y);
            } while (DateTime.UtcNow < fin);
            return false;
        }

        /// Glisser reel (zone de x0,y0 a x1,y1 bornes comprises), puis deux fenetres guettees
        /// toutes les 2 ms depuis le relacher : rend { ms ou `cachee` n'est plus visible, ms
        /// ou `montree` est visible } ; -1 si deja ainsi au relacher, maxMs + 1 si jamais.
        public static int[] TracerPuisGuetter(int x0, int y0, int x1, int y1, IntPtr cachee, IntPtr montree, int maxMs)
        {
            Poser(x0, y0); System.Threading.Thread.Sleep(80);
            BoutonBas(); System.Threading.Thread.Sleep(80);
            for (int i = 1; i <= 20; i++)
            {
                Poser(x0 + (x1 - x0) * i / 20, y0 + (y1 - y0) * i / 20);
                System.Threading.Thread.Sleep(15);
            }
            System.Threading.Thread.Sleep(80);
            BoutonHaut();
            var sw = Stopwatch.StartNew();
            int a = cachee == IntPtr.Zero || !IsWindowVisible(cachee) ? -1 : maxMs + 1;
            int b = montree != IntPtr.Zero && IsWindowVisible(montree) ? -1 : maxMs + 1;
            timeBeginPeriod(1);
            try
            {
                while (sw.ElapsedMilliseconds <= maxMs && (a > maxMs || b > maxMs))
                {
                    System.Threading.Thread.Sleep(1);
                    int t = (int)sw.ElapsedMilliseconds;
                    if (a > maxMs && !IsWindowVisible(cachee)) a = t;
                    if (b > maxMs && montree != IntPtr.Zero && IsWindowVisible(montree)) b = t;
                }
            }
            finally { timeEndPeriod(1); }
            return new int[] { a, b };
        }

        // ---------------------------------------------------------------------------------
        // Pointeur (l'astuce de l'essai 11 : sans souris branchee, les touches souris le
        // montrent)

        [StructLayout(LayoutKind.Sequential)]
        struct MOUSEKEYS { public uint cbSize; public uint flags; public uint maxSpeed; public uint timeToMax; public uint ctrlSpeed; public uint r1; public uint r2; }
        [DllImport("user32.dll", SetLastError = true, EntryPoint = "SystemParametersInfoW")] static extern bool SpiMouseKeys(uint action, uint param, ref MOUSEKEYS v, uint winIni);
        static bool mouseKeysLu = false;
        static MOUSEKEYS mouseKeysOrigine;

        public static bool AllumerMouseKeys()
        {
            var m = new MOUSEKEYS(); m.cbSize = (uint)Marshal.SizeOf(typeof(MOUSEKEYS));
            if (!SpiMouseKeys(0x36, m.cbSize, ref m, 0)) return false;
            if (!mouseKeysLu) { mouseKeysOrigine = m; mouseKeysLu = true; }
            m.flags |= 0x3;
            return SpiMouseKeys(0x37, m.cbSize, ref m, 0);
        }

        public static bool RemettreMouseKeys()
        {
            if (!mouseKeysLu) return true;
            var m = mouseKeysOrigine;
            bool ok = SpiMouseKeys(0x37, m.cbSize, ref m, 0);
            if (ok) mouseKeysLu = false;
            return ok;
        }

        /// Montre (CURSOR_SHOWING) et pas supprime (CURSOR_SUPPRESSED).
        public static bool PointeurMontre()
        {
            CURSORINFO c = new CURSORINFO(); c.cbSize = Marshal.SizeOf(typeof(CURSORINFO));
            return GetCursorInfo(ref c) && (c.flags & 1) != 0 && (c.flags & 2) == 0;
        }

        // ---------------------------------------------------------------------------------
        // Ecran : modes (EnumDisplaySettings / ChangeDisplaySettingsEx) et echelle
        // (DisplayConfig), comme la sonde (S7 points 1 et 2).

        static DEVMODE Nouveau() { DEVMODE d = new DEVMODE(); d.dmSize = (short)Marshal.SizeOf(typeof(DEVMODE)); return d; }

        public static string ModeCourant()
        {
            DEVMODE d = Nouveau();
            if (!EnumDisplaySettings(null, -1, ref d)) return "illisible";
            return d.dmPelsWidth + "x" + d.dmPelsHeight;
        }

        /// Pose le mode l x h (dynamique, sans l'ecrire au registre) ; rend le code
        /// DISP_CHANGE (0 : fait).
        public static int ChangerMode(int l, int h)
        {
            DEVMODE d = Nouveau();
            if (!EnumDisplaySettings(null, -1, ref d)) return -100;
            d.dmPelsWidth = l; d.dmPelsHeight = h; d.dmFields = 0x80000 | 0x100000;
            return ChangeDisplaySettingsEx(null, ref d, IntPtr.Zero, 0u, IntPtr.Zero);
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

        /// {code, min, actuel, max} : pas relatifs a l'echelle recommandee (0 = recommandee).
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

        public static uint DpiEcran(int x, int y)
        {
            POINT p; p.X = x; p.Y = y;
            uint dx, dy;
            return GetDpiForMonitor(MonitorFromPoint(p, 2), 0, out dx, out dy) == 0 ? dx : 0;
        }

        /// {libre pour l'appelant, total} du volume qui porte `dossier` (point de montage
        /// compris) ; {-1, GetLastError} en cas d'echec.
        public static long[] Disque(string dossier)
        {
            ulong l, t, tl;
            if (!GetDiskFreeSpaceExW(dossier, out l, out t, out tl)) return new long[] { -1, Marshal.GetLastWin32Error() };
            return new long[] { (long)l, (long)t };
        }

        // ---------------------------------------------------------------------------------
        // Pixels (BGRA)

        /// Part des points d'une grille de pas `pas` ou `b` vaut `a` assombri de `voile`
        /// (0,18 : le voile du film) a `seuil` pres par canal ; `voile` 0 : `b` egal a `a`.
        public static double PartVoilee(byte[] a, byte[] b, int l, double voile, int seuil, int pas)
        {
            if (a == null || b == null || a.Length != b.Length || l <= 0) return -1;
            int h = a.Length / 4 / l; long n = 0, ok = 0;
            for (int y = 0; y < h; y += pas)
                for (int x = 0; x < l; x += pas)
                {
                    int i = (y * l + x) * 4; n++;
                    bool bon = true;
                    for (int c = 0; c < 3; c++)
                        if (Math.Abs(a[i + c] * (1.0 - voile) - b[i + c]) > seuil) { bon = false; break; }
                    if (bon) ok++;
                }
            return n == 0 ? -1 : (double)ok / n;
        }

        /// Part des pixels qui different de plus de `seuil` sur un canal.
        public static double PartDifferente(byte[] a, byte[] b, int seuil)
        {
            if (a == null || b == null || a.Length != b.Length || a.Length == 0) return -1;
            long n = 0, d = 0;
            for (int i = 0; i + 3 < a.Length; i += 4)
            {
                n++;
                if (Math.Abs(a[i] - b[i]) > seuil || Math.Abs(a[i + 1] - b[i + 1]) > seuil || Math.Abs(a[i + 2] - b[i + 2]) > seuil) d++;
            }
            return (double)d / n;
        }
    }
}
