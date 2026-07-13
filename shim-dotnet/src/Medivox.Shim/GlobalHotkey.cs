using System.Runtime.InteropServices;

namespace Medivox.Shim;

/// <summary>
/// Duenner Wrapper um RegisterHotKey/UnregisterHotKey. Der eigentliche WM_HOTKEY-Empfang
/// passiert im WndProc-Override des versteckten Forms in TrayApplicationContext -- WinForms
/// stellt die Message-Loop bereits bereit, eine eigene GetMessage-Schleife ist nicht noetig.
/// </summary>
internal static class GlobalHotkey
{
    public const int WM_HOTKEY = 0x0312;
    public const int HotkeyId = 1;

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool RegisterHotKey(IntPtr hWnd, int id, uint fsModifiers, uint vk);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool UnregisterHotKey(IntPtr hWnd, int id);

    public static void Register(IntPtr hWnd, uint modifiers, uint vk)
    {
        if (!RegisterHotKey(hWnd, HotkeyId, modifiers, vk))
        {
            var error = Marshal.GetLastWin32Error();
            throw new InvalidOperationException(
                $"Hotkey konnte nicht registriert werden (evtl. bereits von einer anderen Anwendung belegt). Win32-Fehler: {error}");
        }
    }

    public static void Unregister(IntPtr hWnd)
    {
        UnregisterHotKey(hWnd, HotkeyId);
    }
}
