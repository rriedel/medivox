using System.Runtime.InteropServices;

namespace Medivox.Shim;

/// <summary>
/// Injiziert Text als Unicode-Tastatureingaben in das aktuell fokussierte Fenster
/// (Python-Original: inject.py).
///
/// Funktioniert nicht, wenn das Zielfenster mit hoeheren Rechten (erhoeht/Administrator)
/// laeuft als dieser Prozess -- Windows UIPI blockiert das dann.
/// </summary>
internal static class TextInjector
{
    private const uint InputKeyboard = 1;
    private const uint KeyEventFUnicode = 0x0004;
    private const uint KeyEventFKeyUp = 0x0002;

    [StructLayout(LayoutKind.Sequential)]
    private struct MOUSEINPUT
    {
        public int Dx;
        public int Dy;
        public uint MouseData;
        public uint DwFlags;
        public uint Time;
        public IntPtr DwExtraInfo;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct KEYBDINPUT
    {
        public ushort WVk;
        public ushort WScan;
        public uint DwFlags;
        public uint Time;
        public IntPtr DwExtraInfo;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct HARDWAREINPUT
    {
        public uint UMsg;
        public ushort WParamL;
        public ushort WParamH;
    }

    // mi/hi werden nie benutzt, muessen aber Teil des Unions sein: Windows berechnet
    // sizeof(INPUT) anhand der echten API-Struktur (inkl. MOUSEINPUT/HARDWAREINPUT) und
    // verweigert den Aufruf mit ERROR_INVALID_PARAMETER, wenn cbSize davon abweicht.
    [StructLayout(LayoutKind.Explicit)]
    private struct InputUnion
    {
        [FieldOffset(0)] public MOUSEINPUT Mi;
        [FieldOffset(0)] public KEYBDINPUT Ki;
        [FieldOffset(0)] public HARDWAREINPUT Hi;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct INPUT
    {
        public uint Type;
        public InputUnion U;
    }

    [DllImport("user32.dll", SetLastError = true)]
    private static extern uint SendInput(uint nInputs, [In] INPUT[] pInputs, int cbSize);

    private static INPUT CharInput(char ch, bool keyUp)
    {
        // wVk muss 0 sein, wenn KEYEVENTF_UNICODE gesetzt ist; wScan traegt die UTF-16-Codeeinheit.
        var flags = KeyEventFUnicode | (keyUp ? KeyEventFKeyUp : 0);
        var ki = new KEYBDINPUT { WVk = 0, WScan = ch, DwFlags = flags, Time = 0, DwExtraInfo = IntPtr.Zero };
        return new INPUT { Type = InputKeyboard, U = new InputUnion { Ki = ki } };
    }

    public static void TypeText(string text)
    {
        if (string.IsNullOrEmpty(text))
        {
            return;
        }

        var inputs = new List<INPUT>(text.Length * 2);
        foreach (var c in text)
        {
            var ch = c == '\n' ? '\r' : c;
            inputs.Add(CharInput(ch, keyUp: false));
            inputs.Add(CharInput(ch, keyUp: true));
        }

        var array = inputs.ToArray();
        Marshal.SetLastSystemError(0);
        var sent = SendInput((uint)array.Length, array, Marshal.SizeOf<INPUT>());
        if (sent != array.Length)
        {
            var error = Marshal.GetLastWin32Error();
            throw new InvalidOperationException(
                $"SendInput hat nur {sent} von {array.Length} Eingaben zugestellt. Win32-Fehler: {error}");
        }
    }
}
