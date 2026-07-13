using System.Drawing;
using System.Drawing.Drawing2D;
using System.Runtime.InteropServices;
using System.Windows.Forms;
using Serilog;

namespace Medivox.Shim;

/// <summary>
/// Verkabelt Tray-Icon, Hotkey und Aufnahme/Transkriptions-Ablauf
/// (Python-Original: main.py + tray.py).
/// </summary>
internal sealed class TrayApplicationContext : ApplicationContext
{
    private static readonly ILogger Logger = Log.ForContext<TrayApplicationContext>();

    private readonly HotkeyWindow _hotkeyWindow;
    private readonly NotifyIcon _trayIcon;
    private readonly Icon _grayIcon;
    private readonly Icon _redIcon;
    private readonly IntPtr _grayHIcon;
    private readonly IntPtr _redHIcon;
    private readonly Recorder _recorder = new();
    private readonly EngineClient _engineClient = new();
    private bool _recording;

    public TrayApplicationContext()
    {
        _hotkeyWindow = new HotkeyWindow(OnHotkeyPressed);
        GlobalHotkey.Register(_hotkeyWindow.Handle, ShimConfig.HotkeyModifiers, ShimConfig.HotkeyVk);

        (_grayIcon, _grayHIcon) = CreateIcon(Color.Gray);
        (_redIcon, _redHIcon) = CreateIcon(Color.Red);

        var menu = new ContextMenuStrip();
        menu.Items.Add("Beenden", null, (_, _) => ExitThread());

        _trayIcon = new NotifyIcon
        {
            Icon = _grayIcon,
            Text = "medivox -- bereit",
            ContextMenuStrip = menu,
            Visible = true,
        };

        Logger.Information("Hotkey installed, ready for activation");
        Logger.Information("to stop, use tray icon!");
    }

    [DllImport("user32.dll")]
    private static extern bool DestroyIcon(IntPtr handle);

    private static (Icon icon, IntPtr hIcon) CreateIcon(Color color)
    {
        using var bitmap = new Bitmap(64, 64);
        using (var g = Graphics.FromImage(bitmap))
        {
            g.SmoothingMode = SmoothingMode.AntiAlias;
            using var brush = new SolidBrush(color);
            g.FillEllipse(brush, 8, 8, 48, 48);
        }
        var hIcon = bitmap.GetHicon();
        return (Icon.FromHandle(hIcon), hIcon);
    }

    private void OnHotkeyPressed()
    {
        if (!_recording)
        {
            _recording = true;
            SetRecordingUi(active: true);
            _recorder.Start();
            return;
        }

        _recording = false;
        SetRecordingUi(active: false);
        var audio = _recorder.Stop();
        if (audio.Length == 0)
        {
            return;
        }

        _ = HandleTranscriptionAsync(audio);
    }

    private async Task HandleTranscriptionAsync(float[] audio)
    {
        string text;
        try
        {
            text = await _engineClient.TranscribeAsync(audio);
        }
        catch (Exception ex)
        {
            Logger.Error(ex, "Transkription fehlgeschlagen");
            return;
        }

        Logger.Information("Transkriptionsergebnis: {Text}", text);
        if (string.IsNullOrEmpty(text))
        {
            return;
        }

        try
        {
            TextInjector.TypeText(text);
        }
        catch (Exception ex)
        {
            Logger.Error(ex, "Texteingabe fehlgeschlagen");
        }
    }

    private void SetRecordingUi(bool active)
    {
        _trayIcon.Icon = active ? _redIcon : _grayIcon;
        _trayIcon.Text = active ? "medivox -- Aufnahme laeuft" : "medivox -- bereit";
    }

    protected override void ExitThreadCore()
    {
        GlobalHotkey.Unregister(_hotkeyWindow.Handle);
        _hotkeyWindow.DestroyHandle();

        _trayIcon.Visible = false;
        _trayIcon.Dispose();
        _grayIcon.Dispose();
        _redIcon.Dispose();
        DestroyIcon(_grayHIcon);
        DestroyIcon(_redHIcon);

        _engineClient.Dispose();
        Log.CloseAndFlush();

        base.ExitThreadCore();
    }

    /// <summary>
    /// Reines Message-only-Fenster (HWND_MESSAGE) nur zum Empfang von WM_HOTKEY -- keine
    /// eigene GetMessage-Schleife noetig, WinForms betreibt die Message-Loop bereits ueber
    /// Application.Run.
    /// </summary>
    private sealed class HotkeyWindow : NativeWindow
    {
        private static readonly IntPtr HwndMessage = new(-3);

        private readonly Action _onHotkey;

        public HotkeyWindow(Action onHotkey)
        {
            _onHotkey = onHotkey;
            CreateHandle(new CreateParams { Parent = HwndMessage });
        }

        protected override void WndProc(ref Message m)
        {
            if (m.Msg == GlobalHotkey.WM_HOTKEY && m.WParam.ToInt32() == GlobalHotkey.HotkeyId)
            {
                _onHotkey();
            }
            base.WndProc(ref m);
        }
    }
}
