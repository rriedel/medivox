namespace Medivox.Shim;

internal static class ShimConfig
{
    public const uint ModAlt = 0x0001;
    public const uint ModControl = 0x0002;
    public const uint ModShift = 0x0004;
    public const uint ModWin = 0x0008;

    public const uint VkSpace = 0x20;

    public static string EngineHost { get; } =
        Environment.GetEnvironmentVariable("MEDIVOX_ENGINE_HOST") ?? "127.0.0.1";

    public static int EnginePort { get; } =
        int.TryParse(Environment.GetEnvironmentVariable("MEDIVOX_ENGINE_PORT"), out var port)
            ? port
            : 8123;

    public const int SampleRate = 16000;

    // Standard-Hotkey zum Umschalten: Strg+Alt+Leertaste
    public const uint HotkeyModifiers = ModAlt | ModControl;
    public const uint HotkeyVk = VkSpace;

    public const int RequestTimeoutSeconds = 60;

    public static string LogLevel { get; } =
        Environment.GetEnvironmentVariable("MEDIVOX_LOG_LEVEL") ?? "Information";
}
