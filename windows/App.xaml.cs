using System;
using System.IO;
using Microsoft.UI.Xaml;

namespace Tether;

public partial class App : Application
{
    public static Window Window { get; private set; } = null!;

    /// Where crashes are written, for bug reports.
    static readonly string CrashLog = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Tether", "crash.log");

    public App()
    {
        InitializeComponent();
        UnhandledException += (_, e) => LogCrash(e.Exception);
        AppDomain.CurrentDomain.UnhandledException += (_, e) => LogCrash(e.ExceptionObject as Exception);
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        try
        {
            Window = new MainWindow();
            Window.Activate();
        }
        catch (Exception e)
        {
            LogCrash(e);
            throw;
        }
    }

    static void LogCrash(Exception? e)
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(CrashLog)!);
            File.AppendAllText(CrashLog, $"{DateTime.Now:u}\n{e}\n\n");
        }
        catch { }
    }
}
