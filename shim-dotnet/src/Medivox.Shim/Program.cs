using Serilog;

namespace Medivox.Shim;

internal static class Program
{
    [STAThread]
    private static void Main()
    {
        Logging.Configure();
        try
        {
            ApplicationConfiguration.Initialize();
            Application.Run(new TrayApplicationContext());
        }
        catch (Exception ex)
        {
            Log.Fatal(ex, "Unerwarteter Absturz");
            throw;
        }
        finally
        {
            Log.CloseAndFlush();
        }
    }
}
