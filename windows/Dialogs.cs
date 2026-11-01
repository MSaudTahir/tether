using System;
using System.Collections.Generic;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.Storage.Pickers;

namespace Tether;

static class Dialogs
{
    static readonly AppModel Model = AppModel.Shared;

    public static async Task ShowError(XamlRoot root, string message)
    {
        var dialog = new ContentDialog
        {
            XamlRoot = root, Title = "Something Went Wrong", CloseButtonText = "OK",
            Content = new TextBlock { Text = message, TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true },
        };
        await ShowOne(dialog);
    }

    public static async Task ShowPair(XamlRoot root)
    {
        var pairAddress = new TextBox { Header = "Pairing IP:port", PlaceholderText = "192.168.1.20:37099" };
        var code = new TextBox { Header = "Pairing code", PlaceholderText = "123456" };
        var connect = new TextBox { Header = "IP:port", PlaceholderText = "192.168.1.20:41235" };
        var error = new TextBlock { TextWrapping = TextWrapping.Wrap, Foreground = Resource<Microsoft.UI.Xaml.Media.Brush>("SystemFillColorCriticalBrush") };
        var content = new StackPanel { Spacing = 12, Width = 400 };
        content.Children.Add(Heading("Pair"));
        content.Children.Add(pairAddress);
        content.Children.Add(code);
        content.Children.Add(Note("On the phone: Developer options → Wireless debugging → Pair device with pairing code. The phone and this PC must be on the same network."));
        content.Children.Add(Heading("Connect"));
        content.Children.Add(connect);
        content.Children.Add(Note("The IP address & port shown on the Wireless debugging screen. Already paired? Just fill this in."));
        content.Children.Add(error);

        var dialog = new ContentDialog
        {
            XamlRoot = root, Title = "Pair over Wi-Fi", Content = content,
            PrimaryButtonText = "Connect", CloseButtonText = "Cancel", DefaultButton = ContentDialogButton.Primary,
        };
        dialog.PrimaryButtonClick += async (_, args) =>
        {
            var deferral = args.GetDeferral();
            dialog.IsPrimaryButtonEnabled = false;
            error.Text = "";
            try
            {
                await Model.Pair(pairAddress.Text.Trim(), code.Text.Trim(), connect.Text.Trim());
            }
            catch (Exception e)
            {
                error.Text = e.Message;
                args.Cancel = true;
            }
            dialog.IsPrimaryButtonEnabled = true;
            deferral.Complete();
        };
        await ShowOne(dialog);
    }

    public static async Task ShowSettings(XamlRoot root)
    {
        var content = new StackPanel { Spacing = 20, Width = 480 };
        var dialog = new ContentDialog { XamlRoot = root, Title = "Settings", Content = new ScrollViewer { Content = content }, CloseButtonText = "Done" };

        var adb = new StackPanel { Spacing = 8 };
        adb.Children.Add(Heading("adb"));
        adb.Children.Add(new AdbPicker());
        content.Children.Add(adb);

        var mouse = new StackPanel { Spacing = 4 };
        mouse.Children.Add(Heading("Mouse"));
        var capture = new CheckBox { Content = "Capture the mouse", IsChecked = Model.Config.captureMouse };
        capture.Click += (_, _) => Model.SetCaptureMouse(capture.IsChecked == true);
        mouse.Children.Add(capture);
        mouse.Children.Add(Note("Sends the mouse as a real mouse, for games and apps that capture the pointer (Android 8+). Press Left Alt to release it. App windows turn on the phone's \"Force desktop mode\" developer option so the mouse appears in them."));
        content.Children.Add(mouse);

        var scrcpy = new StackPanel { Spacing = 8 };
        scrcpy.Children.Add(Heading("scrcpy"));
        var path = new TextBlock { TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true };
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var useDownloaded = new Button { Content = "Use Downloaded scrcpy" };
        var choose = new Button { Content = "Choose scrcpy…" };
        buttons.Children.Add(useDownloaded);
        buttons.Children.Add(choose);
        scrcpy.Children.Add(path);
        scrcpy.Children.Add(buttons);
        content.Children.Add(scrcpy);
        void RenderScrcpy()
        {
            path.Text = $"In use: {Model.Core.ScrcpyPath() ?? "Not found"}";
            useDownloaded.Visibility = Model.Config.scrcpy != "" && Model.Core.BundledScrcpyVersion() != null ? Visibility.Visible : Visibility.Collapsed;
        }
        RenderScrcpy();
        useDownloaded.Click += (_, _) => { Model.Save(Model.Config with { scrcpy = "" }); RenderScrcpy(); };
        choose.Click += async (_, _) =>
        {
            if (await PickExe() is { } file) { Model.Save(Model.Config with { scrcpy = file }); RenderScrcpy(); }
        };

        var setup = new Button { Content = "Run Setup Again" };
        setup.Click += (_, _) => { dialog.Hide(); Model.RunSetupAgain(); };
        content.Children.Add(setup);

        await ShowOne(dialog);
    }

    /// Lets the user choose an .exe; null if cancelled.
    public static async Task<string?> PickExe()
    {
        var picker = new FileOpenPicker();
        picker.FileTypeFilter.Add(".exe");
        WinRT.Interop.InitializeWithWindow.Initialize(picker, WinRT.Interop.WindowNative.GetWindowHandle(App.Window));
        return (await picker.PickSingleFileAsync())?.Path;
    }

    static bool showing;

    /// WinUI allows one dialog at a time; later ones (e.g. a second error) are dropped.
    static async Task ShowOne(ContentDialog dialog)
    {
        if (showing) return;
        showing = true;
        try { await dialog.ShowAsync(); }
        finally { showing = false; }
    }

    public static TextBlock Heading(string text) => new() { Text = text, Style = Resource<Style>("BodyStrongTextBlockStyle") };

    public static TextBlock Note(string text) => new()
    {
        Text = text, TextWrapping = TextWrapping.Wrap, FontSize = 12, Foreground = Resource<Microsoft.UI.Xaml.Media.Brush>("TextFillColorSecondaryBrush"),
    };

    public static T Resource<T>(string key) => (T)Application.Current.Resources[key];
}

/// Lets the user pick which adb to use; saves immediately.
sealed class AdbPicker : StackPanel
{
    static readonly Dictionary<string, string> Names = new() { ["studio"] = "Android Studio", ["system"] = "System", ["bundled"] = "Bundled with scrcpy" };
    readonly AppModel model = AppModel.Shared;
    readonly RadioButtons choices = new();
    readonly List<string> values = [];

    public AdbPicker()
    {
        Spacing = 8;
        Children.Add(choices);
        var other = new HyperlinkButton { Content = "Choose another adb…" };
        other.Click += async (_, _) => { if (await Dialogs.PickExe() is { } file) { Set(file); Render(); } };
        Children.Add(other);
        choices.SelectionChanged += (_, _) =>
        {
            if (choices.SelectedIndex >= 0 && values[choices.SelectedIndex] != model.Config.adb) Set(values[choices.SelectedIndex]);
        };
        Render();
    }

    void Render()
    {
        var current = model.Config.adb;
        choices.Items.Clear();
        values.Clear();
        Add("auto", "Automatic (recommended)", model.Core.ResolveAdb("auto"));
        foreach (var candidate in model.Core.AdbCandidates())
            Add(candidate.source, Names.GetValueOrDefault(candidate.source, candidate.source), candidate.path);
        if (!values.Contains(current)) Add(current, "Custom", current);
        choices.SelectedIndex = values.IndexOf(current);
    }

    void Add(string value, string title, string? path)
    {
        values.Add(value);
        var item = new StackPanel { Spacing = 1 };
        item.Children.Add(new TextBlock { Text = title });
        item.Children.Add(Dialogs.Note(path ?? "Not found"));
        choices.Items.Add(item);
    }

    void Set(string choice) => model.Save(model.Config with { adb = choice });
}
