using Serilog;
using Serilog.Events;

namespace Medivox.Shim;

internal static class Logging
{
    public static void Configure()
    {
        var level = Enum.TryParse<LogEventLevel>(ShimConfig.LogLevel, ignoreCase: true, out var parsed)
            ? parsed
            : LogEventLevel.Information;

        var logDir = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "Medivox", "logs");
        Directory.CreateDirectory(logDir);

        Log.Logger = new LoggerConfiguration()
            .MinimumLevel.Is(level)
            .WriteTo.Console(outputTemplate: "{Timestamp:yyyy-MM-dd HH:mm:ss} {Level:u3} {SourceContext}: {Message:lj}{NewLine}{Exception}")
            .WriteTo.File(
                Path.Combine(logDir, "shim-.log"),
                rollingInterval: RollingInterval.Day,
                retainedFileCountLimit: 14,
                outputTemplate: "{Timestamp:yyyy-MM-dd HH:mm:ss} {Level:u3} {SourceContext}: {Message:lj}{NewLine}{Exception}")
            .CreateLogger();
    }
}
