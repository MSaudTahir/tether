using System;
using System.Linq;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Tether;

/// First-run setup: get scrcpy, pick an adb, connect a phone.
public sealed class SetupView : UserControl
{
    readonly AppModel model = AppModel.Shared;
    readonly StackPanel dots = new() { Orientation = Orientation.Horizontal, Spacing = 8, HorizontalAlignment = HorizontalAlignment.Center };
    readonly ContentControl body = new() { HorizontalContentAlignment = HorizontalAlignment.Stretch };
    int step = -1;

    // scrcpy step
    readonly TextBlock scrcpyStatus = new() { TextWrapping = TextWrapping.Wrap, HorizontalAlignment = HorizontalAlignment.Center, TextAlignment = TextAlignment.Center };
    readonly ProgressRing downloading = new() { Width = 24, Height = 24 };
    readonly Button retry = new() { Content = "Try Again", HorizontalAlignment = HorizontalAlignment.Center };
    bool scrcpyReady;
    // connect step
    readonly TextBlock connectStatus = new() { HorizontalAlignment = HorizontalAlignment.Center, Style = Dialogs.Resource<Style>("BodyStrongTextBlockStyle") };

    public SetupView()
    {
        var card = new StackPanel { Spacing = 24, MaxWidth = 560, Padding = new Thickness(32), CornerRadius = new CornerRadius(16) };
        card.Background = Dialogs.Resource<Brush>("CardBackgroundFillColorDefaultBrush");
        card.BorderBrush = Dialogs.Resource<Brush>("CardStrokeColorDefaultBrush");
        card.BorderThickness = new Thickness(1);
        card.Children.Add(dots);
        card.Children.Add(body);
        Content = new ScrollViewer { Content = new Grid { Padding = new Thickness(40), VerticalAlignment = VerticalAlignment.Center, Children = { card } } };

        retry.Click += async (_, _) => await Download();
        model.Changed += () => { if (step == 2) RenderConnectStatus(); };
        // Starts over each time setup is shown.
        RegisterPropertyChangedCallback(VisibilityProperty, (_, _) => { if (Visibility == Visibility.Visible) Show(0); });
        Loaded += (_, _) => { if (Visibility == Visibility.Visible && step < 0) Show(0); };
    }

    void Show(int index)
    {
        step = index;
        dots.Children.Clear();
        for (var i = 0; i < 3; i++)
        {
            dots.Children.Add(new Border
            {
                Width = 36, Height = 5, CornerRadius = new CornerRadius(2.5),
                Background = Dialogs.Resource<Brush>(i <= step ? "AccentFillColorDefaultBrush" : "ControlStrongFillColorDisabledBrush"),
            });
        }
        body.Content = index switch { 0 => ScrcpyStep(), 1 => AdbStep(), _ => ConnectStep() };
    }

    static StackPanel Header(string glyph, string title, string subtitle)
    {
        var header = new StackPanel { Spacing = 10 };
        header.Children.Add(new FontIcon { Glyph = glyph, FontSize = 44, Foreground = Dialogs.Resource<Brush>("AccentTextFillColorPrimaryBrush") });
        header.Children.Add(new TextBlock { Text = title, Style = Dialogs.Resource<Style>("TitleTextBlockStyle"), HorizontalAlignment = HorizontalAlignment.Center });
        header.Children.Add(new TextBlock
        {
            Text = subtitle, TextWrapping = TextWrapping.Wrap, TextAlignment = TextAlignment.Center, HorizontalAlignment = HorizontalAlignment.Center,
            Foreground = Dialogs.Resource<Brush>("TextFillColorSecondaryBrush"),
        });
        return header;
    }

    static Grid Buttons(Button left, Button right)
    {
        var grid = new Grid();
        right.HorizontalAlignment = HorizontalAlignment.Right;
        right.Style = Dialogs.Resource<Style>("AccentButtonStyle");
        grid.Children.Add(left);
        grid.Children.Add(right);
        return grid;
    }

    UIElement ScrcpyStep()
    {
        var panel = new StackPanel { Spacing = 20 };
        panel.Children.Add(Header("\uE7F4", "Get scrcpy", "Tether uses scrcpy to show your phone's screen. It's free and open source."));
        var status = new StackPanel { Spacing = 8, MinHeight = 44 };
        status.Children.Add(downloading);
        status.Children.Add(scrcpyStatus);
        status.Children.Add(retry);
        panel.Children.Add(status);

        var own = new Button { Content = "Use My Own scrcpy…" };
        var next = new Button { Content = "Continue" };
        own.Click += async (_, _) =>
        {
            if (await Dialogs.PickExe() is not { } file) return;
            model.Save(model.Core.Config() with { scrcpy = file });
            ScrcpyReady();
        };
        next.Click += (_, _) => Show(1);
        panel.Children.Add(Buttons(own, next));
        scrcpyChanged = () => next.IsEnabled = scrcpyReady;

        if (model.Core.BundledScrcpyVersion() != null || model.Core.Config().scrcpy != "") ScrcpyReady();
        else _ = Download();
        scrcpyChanged();
        return panel;
    }

    /// Enables Continue once scrcpy is ready.
    Action? scrcpyChanged;

    void ScrcpyReady()
    {
        scrcpyReady = true;
        downloading.IsActive = false;
        retry.Visibility = Visibility.Collapsed;
        var custom = model.Core.Config().scrcpy;
        scrcpyStatus.Text = custom != "" ? $"✓ Using {custom}" : $"✓ scrcpy {model.Core.BundledScrcpyVersion()} is ready.";
        scrcpyStatus.Foreground = Dialogs.Resource<Brush>("SystemFillColorSuccessBrush");
        scrcpyChanged?.Invoke();
    }

    async Task Download()
    {
        scrcpyReady = false;
        downloading.IsActive = true;
        retry.Visibility = Visibility.Collapsed;
        scrcpyStatus.Text = "Downloading the latest scrcpy…";
        scrcpyStatus.Foreground = Dialogs.Resource<Brush>("TextFillColorPrimaryBrush");
        scrcpyChanged?.Invoke();
        var core = model.Core;
        try
        {
            await Task.Run(() =>
            {
                var release = core.LatestRelease() ?? throw new Exception("There's no prebuilt scrcpy for this PC. Install it and choose it below.");
                core.InstallScrcpy(release);
            });
            model.Save(core.Config() with { scrcpy = "" });
            ScrcpyReady();
        }
        catch (Exception e)
        {
            downloading.IsActive = false;
            scrcpyStatus.Text = e.Message;
            scrcpyStatus.Foreground = Dialogs.Resource<Brush>("SystemFillColorCriticalBrush");
            retry.Visibility = Visibility.Visible;
        }
    }

    UIElement AdbStep()
    {
        var panel = new StackPanel { Spacing = 20 };
        panel.Children.Add(Header("\uE88E", "Choose adb", "adb is how your PC talks to the phone. If you use Android Studio, sharing its adb avoids conflicts."));
        panel.Children.Add(new AdbPicker());
        var back = new Button { Content = "Back" };
        var next = new Button { Content = "Continue" };
        back.Click += (_, _) => Show(0);
        next.Click += (_, _) => Show(2);
        panel.Children.Add(Buttons(back, next));
        return panel;
    }

    UIElement ConnectStep()
    {
        var panel = new StackPanel { Spacing = 20 };
        panel.Children.Add(Header("\uE8EA", "Connect Your Phone", "You can skip this and connect, or pair over Wi-Fi, later."));
        var steps = new StackPanel { Spacing = 10 };
        steps.Children.Add(new TextBlock { Text = "1.  On the phone, open Settings → About phone and tap Build number 7 times.", TextWrapping = TextWrapping.Wrap });
        steps.Children.Add(new TextBlock { Text = "2.  Go to Settings → System → Developer options and turn on USB debugging.", TextWrapping = TextWrapping.Wrap });
        steps.Children.Add(new TextBlock { Text = "3.  Plug the phone in and tap Allow on the \"Allow USB debugging?\" prompt.", TextWrapping = TextWrapping.Wrap });
        panel.Children.Add(steps);
        panel.Children.Add(connectStatus);
        RenderConnectStatus();
        var back = new Button { Content = "Back" };
        var finish = new Button { Content = "Finish" };
        back.Click += (_, _) => Show(1);
        finish.Click += (_, _) => model.FinishSetup();
        panel.Children.Add(Buttons(back, finish));
        return panel;
    }

    void RenderConnectStatus()
    {
        var ready = model.Devices.FirstOrDefault(d => d.state == "device");
        connectStatus.Foreground = Dialogs.Resource<Brush>(ready != null ? "SystemFillColorSuccessBrush" : "TextFillColorPrimaryBrush");
        connectStatus.Text = ready != null ? $"✓ {ready.label} is connected"
            : model.Devices.FirstOrDefault() is { } device ? $"Found {device.label}. Tap Allow on the phone."
            : "Waiting for a device…";
    }
}
