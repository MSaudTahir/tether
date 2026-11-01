using Microsoft.UI.Xaml;

namespace Tether;

public partial class App : Application
{
    public static Window Window { get; private set; } = null!;

    public App() => InitializeComponent();

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        Window = new MainWindow();
        Window.Activate();
    }
}
