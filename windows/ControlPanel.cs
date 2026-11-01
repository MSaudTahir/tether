using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;
using uniffi.tether_core;

namespace Tether;

/// The device page's Controls row and Details card.
static class ControlPanel
{
    static readonly AppModel Model = AppModel.Shared;

    /// Buttons and switches that act on the phone without opening a window.
    public static FrameworkElement Controls(Device device, DeviceInfo info)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        row.Children.Add(Button("\uE722", "Screenshot", "Save a screenshot to the Desktop", () => Model.Screenshot(device)));
        var installing = Model.Installing.Contains(device.serial);
        var install = Button("\uE896", installing ? "Installing…" : "Install APK…", "Install an app from an .apk file. You can also drop .apk files on this window.",
            async () => { if (await PickApks() is { Count: > 0 } apks) Model.Install(apks, device); });
        install.IsEnabled = !installing;
        row.Children.Add(install);
        row.Children.Add(Separator());
        row.Children.Add(Key(device, DeviceKey.Back, "\uE72B", "Back"));
        row.Children.Add(Key(device, DeviceKey.Home, "\uE80F", "Home"));
        row.Children.Add(Key(device, DeviceKey.RecentApps, "\uE8A9", "Recent apps"));
        row.Children.Add(Key(device, DeviceKey.VolumeDown, "\uE993", "Volume down"));
        row.Children.Add(Key(device, DeviceKey.VolumeUp, "\uE995", "Volume up"));
        row.Children.Add(Key(device, DeviceKey.Power, info.screenOn ? "\uE72E" : "\uE706", info.screenOn ? "Lock screen" : "Wake screen"));
        row.Children.Add(Separator());
        row.Children.Add(Toggle("\uE708", "Dark Mode", "Switch the phone's dark theme", info.darkMode, on => Model.SetDarkMode(on, device)));
        row.Children.Add(Toggle("\uE7E8", "Stay Awake", "Keep the screen on while charging", info.stayAwake, on => Model.SetStayAwake(on, device)));

        var restart = new DropDownButton { Content = Label("\uE777", "Restart") };
        var menu = new MenuFlyout { Placement = FlyoutPlacementMode.BottomEdgeAlignedRight };
        foreach (var (title, mode) in new[] { ("Restart", RebootMode.Normal), ("Restart to Recovery", RebootMode.Recovery), ("Restart to Bootloader", RebootMode.Bootloader) })
        {
            var item = new MenuFlyoutItem { Text = title };
            item.Click += async (_, _) => await ConfirmReboot(device, info, mode, restart.XamlRoot);
            menu.Items.Add(item);
        }
        restart.Flyout = menu;

        var grid = new Grid { ColumnSpacing = 8 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.Children.Add(new ScrollViewer { Content = row, HorizontalScrollBarVisibility = ScrollBarVisibility.Auto, VerticalScrollBarVisibility = ScrollBarVisibility.Disabled });
        Grid.SetColumn(restart, 1);
        grid.Children.Add(restart);
        return Card(grid);
    }

    /// Storage, memory, battery and hardware details.
    public static FrameworkElement Details(DeviceInfo info)
    {
        var left = new Grid { ColumnSpacing = 16, RowSpacing = 12 };
        var right = new Grid { ColumnSpacing = 16, RowSpacing = 12 };
        if (info.storageTotal > 0)
            AddMeter(left, "\uEDA2", "Storage", info.storageUsed, info.storageTotal, $"{Bytes(info.storageTotal - info.storageUsed)} free of {Bytes(info.storageTotal)}");
        if (info.memoryTotal > 0)
            AddMeter(left, "\uE964", "Memory", info.memoryTotal - info.memoryAvailable, info.memoryTotal, $"{Bytes(info.memoryAvailable)} available of {Bytes(info.memoryTotal)}");
        AddRow(left, "\uE83F", "Battery", Battery(info));
        if (info.chipset != "") AddRow(right, "\uE950", "Chipset", info.chipset);
        if (info.screen != "") AddRow(right, "\uE8EA", "Display", info.screen);
        AddRow(right, "\uE83D", "Android", Android(info));
        if (info.uptimeSeconds > 0) AddRow(right, "\uE823", "Up for", Uptime(info.uptimeSeconds));
        if (info.wifiAddress is { } address) AddRow(right, "\uE701", "Wi-Fi address", address);

        var columns = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 40 };
        columns.Children.Add(left);
        columns.Children.Add(right);
        return Card(columns);
    }

    /// The phone's current notifications, as separate cards like the phone shows them.
    public static FrameworkElement Notifications(Device device, PhoneNotification[] all)
    {
        if (all.Length == 0) return Dialogs.Note("No notifications. Notifications on the phone show up here.");
        var list = new StackPanel { Spacing = 10, MaxWidth = 720 };
        var ready = Model.Buddy.TryGetValue(device.serial, out var buddy) && buddy == BuddyState.Ready;
        if (Model.Buddy.ContainsKey(device.serial) && !ready) list.Children.Add(BuddyBanner(buddy, device));
        list.Children.Add(Dialogs.Note(ready
            ? "Click a notification to open it on the phone's screen."
            : "Click a notification to show it on the phone's screen and tap it there."));
        foreach (var notification in all)
        {
            var appName = notification.appName == "" ? notification.package : notification.appName;
            var when = notification.time > 0 ? " · " + DateTimeOffset.FromUnixTimeMilliseconds((long)notification.time).ToLocalTime().ToString("t") : "";
            var header = new Grid();
            var source = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            source.Children.Add(MainWindow.AppIcon(new DeviceApp(appName, notification.package, false, notification.iconPath), 20));
            source.Children.Add(Dialogs.Note(appName + when));
            header.Children.Add(source);
            var open = new HyperlinkButton { Content = "Open App", HorizontalAlignment = HorizontalAlignment.Right, Padding = new Thickness(4, 0, 4, 0) };
            ToolTipService.SetToolTip(open, $"Open {appName}");
            open.Click += (_, _) => Model.OpenApp(notification, device);
            header.Children.Add(open);

            var content = new StackPanel { Spacing = 4 };
            content.Children.Add(header);
            if (notification.title != "") content.Children.Add(new TextBlock { Text = notification.title, Style = Dialogs.Resource<Style>("BodyStrongTextBlockStyle"), TextTrimming = TextTrimming.CharacterEllipsis });
            if (notification.text != "") content.Children.Add(new TextBlock { Text = notification.text, TextWrapping = TextWrapping.Wrap, MaxLines = 4, TextTrimming = TextTrimming.CharacterEllipsis });
            var card = new Microsoft.UI.Xaml.Controls.Button
            {
                Content = content, HorizontalAlignment = HorizontalAlignment.Stretch, HorizontalContentAlignment = HorizontalAlignment.Stretch,
                Padding = new Thickness(14, 12, 14, 12), CornerRadius = new CornerRadius(12),
                Background = Dialogs.Resource<Brush>("CardBackgroundFillColorDefaultBrush"),
                BorderBrush = Dialogs.Resource<Brush>("CardStrokeColorDefaultBrush"),
            };
            ToolTipService.SetToolTip(card, "Open on the phone's screen");
            card.Click += (_, _) => Model.Show(notification, device);
            list.Children.Add(card);
        }
        return list;
    }

    /// Offers Tether Buddy, which lets a click run the notification's own action.
    static FrameworkElement BuddyBanner(BuddyState state, Device device)
    {
        var text = new StackPanel { Spacing = 2 };
        text.Children.Add(new TextBlock { Text = state == BuddyState.Outdated ? "Update Tether Buddy" : "Open notifications directly with Tether Buddy", Style = Dialogs.Resource<Style>("BodyStrongTextBlockStyle") });
        text.Children.Add(Dialogs.Note("A tiny app with no screens that Tether puts on the phone. With it, clicking a notification does what tapping it does, like opening that chat."));
        var installing = Model.InstallingBuddy.Contains(device.serial);
        var button = new Microsoft.UI.Xaml.Controls.Button
        {
            Content = installing ? "Installing…" : state switch { BuddyState.Outdated => "Update", BuddyState.Disabled => "Turn On", _ => "Install" },
            Style = Dialogs.Resource<Style>("AccentButtonStyle"), IsEnabled = !installing, VerticalAlignment = VerticalAlignment.Center,
        };
        button.Click += (_, _) => Model.InstallBuddy(device);
        var grid = new Grid { ColumnSpacing = 14 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.Children.Add(text);
        Grid.SetColumn(button, 1);
        grid.Children.Add(button);
        return Card(grid);
    }

    /// Moves text between this PC's and the phone's clipboards.
    public static FrameworkElement Clipboard(Device device)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var label = Label("\uE77F", "Clipboard");
        label.Opacity = 0.7;
        label.VerticalAlignment = VerticalAlignment.Center;
        row.Children.Add(label);
        row.Children.Add(Button("\uE898", "Send Clipboard to Phone", "Put this PC's clipboard text on the phone's clipboard", () => Model.SendClipboard(device)));
        row.Children.Add(Button("\uE8C8", "Copy Phone Clipboard", "Copy the phone's clipboard text to this PC", () => Model.CopyClipboard(device)));
        if (Model.ClipboardStatus.TryGetValue(device.serial, out var status))
            row.Children.Add(new TextBlock { Text = status, VerticalAlignment = VerticalAlignment.Center, Opacity = 0.7, TextWrapping = TextWrapping.Wrap, MaxWidth = 420 });
        return Card(row);
    }

    static string Battery(DeviceInfo info)
    {
        var parts = new List<string> { $"{info.battery}%", Capitalized(info.power) };
        if (info.batteryHealth != "") parts.Add($"{Capitalized(info.batteryHealth)} health");
        if (info.batteryTemperature > 0) parts.Add($"{info.batteryTemperature:0.0} °C");
        return string.Join(" · ", parts);
    }

    static string Android(DeviceInfo info) =>
        DateTime.TryParseExact(info.securityPatch, "yyyy-MM-dd", CultureInfo.InvariantCulture, DateTimeStyles.None, out var patch)
            ? $"Android {info.android} · Security patch {patch:MMMM yyyy}"
            : $"Android {info.android}";

    static string Uptime(ulong seconds)
    {
        var time = TimeSpan.FromSeconds(seconds);
        return time.TotalDays >= 1 ? $"{(int)time.TotalDays} days, {time.Hours} hours" : $"{time.Hours} hours, {time.Minutes} minutes";
    }

    static string Bytes(ulong count)
    {
        string[] units = ["bytes", "KB", "MB", "GB", "TB"];
        double value = count;
        var unit = 0;
        while (value >= 1000 && unit < units.Length - 1) { value /= 1000; unit++; }
        return $"{value:0.##} {units[unit]}";
    }

    static string Capitalized(string text) => text == "" ? text : char.ToUpper(text[0]) + text[1..];

    static void AddRow(Grid grid, string glyph, string title, string value)
    {
        var text = new TextBlock { Text = value, IsTextSelectionEnabled = true, TextWrapping = TextWrapping.Wrap };
        Add(grid, glyph, title, text);
    }

    static void AddMeter(Grid grid, string glyph, string title, ulong used, ulong total, string text)
    {
        var meter = new StackPanel { Spacing = 4 };
        var bar = new ProgressBar { Maximum = total, Value = used, Width = 240, HorizontalAlignment = HorizontalAlignment.Left };
        if ((double)used / total > 0.9) bar.Foreground = new SolidColorBrush(Microsoft.UI.Colors.Orange);
        meter.Children.Add(bar);
        meter.Children.Add(Dialogs.Note(text));
        Add(grid, glyph, title, meter);
    }

    static void Add(Grid grid, string glyph, string title, FrameworkElement value)
    {
        if (grid.ColumnDefinitions.Count == 0)
        {
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        }
        var row = grid.RowDefinitions.Count;
        grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        var label = Label(glyph, title);
        label.Opacity = 0.7;
        Grid.SetRow(label, row);
        Grid.SetRow(value, row);
        Grid.SetColumn(value, 1);
        grid.Children.Add(label);
        grid.Children.Add(value);
    }

    static async System.Threading.Tasks.Task ConfirmReboot(Device device, DeviceInfo info, RebootMode mode, XamlRoot root)
    {
        var dialog = new ContentDialog
        {
            XamlRoot = root, Title = $"Restart {info.name}?",
            Content = mode switch
            {
                RebootMode.Normal => "The phone will disconnect while it restarts.",
                RebootMode.Recovery => "The phone will start in recovery mode. Only do this if you know how to use it.",
                _ => "The phone will start in its bootloader. Only do this if you know how to use it.",
            },
            PrimaryButtonText = "Restart", CloseButtonText = "Cancel", DefaultButton = ContentDialogButton.Close,
        };
        if (await dialog.ShowAsync() == ContentDialogResult.Primary) Model.Reboot(device, mode);
    }

    static async System.Threading.Tasks.Task<List<string>> PickApks()
    {
        var picker = new Windows.Storage.Pickers.FileOpenPicker();
        picker.FileTypeFilter.Add(".apk");
        WinRT.Interop.InitializeWithWindow.Initialize(picker, WinRT.Interop.WindowNative.GetWindowHandle(App.Window));
        var files = await picker.PickMultipleFilesAsync();
        var paths = new List<string>();
        foreach (var file in files) paths.Add(file.Path);
        return paths;
    }

    static StackPanel Label(string glyph, string text)
    {
        var label = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        label.Children.Add(new FontIcon { Glyph = glyph, FontSize = 16 });
        label.Children.Add(new TextBlock { Text = text });
        return label;
    }

    static Microsoft.UI.Xaml.Controls.Button Button(string glyph, string text, string tip, Action action)
    {
        var button = new Microsoft.UI.Xaml.Controls.Button { Content = Label(glyph, text) };
        ToolTipService.SetToolTip(button, tip);
        button.Click += (_, _) => action();
        return button;
    }

    static Microsoft.UI.Xaml.Controls.Button Key(Device device, DeviceKey key, string glyph, string tip)
    {
        var button = new Microsoft.UI.Xaml.Controls.Button { Content = new FontIcon { Glyph = glyph, FontSize = 16 } };
        ToolTipService.SetToolTip(button, tip);
        button.Click += (_, _) => Model.Press(key, device);
        return button;
    }

    static ToggleButton Toggle(string glyph, string text, string tip, bool on, Action<bool> set)
    {
        var toggle = new ToggleButton { Content = Label(glyph, text), IsChecked = on };
        ToolTipService.SetToolTip(toggle, tip);
        toggle.Click += (_, _) => set(toggle.IsChecked == true);
        return toggle;
    }

    static Border Separator() => new() { Width = 1, Height = 20, Margin = new Thickness(4, 0, 4, 0), Background = Dialogs.Resource<Brush>("DividerStrokeColorDefaultBrush") };

    static Border Card(UIElement child) => new()
    {
        Child = child, Padding = new Thickness(20), CornerRadius = new CornerRadius(12), BorderThickness = new Thickness(1),
        Background = Dialogs.Resource<Brush>("CardBackgroundFillColorDefaultBrush"),
        BorderBrush = Dialogs.Resource<Brush>("CardStrokeColorDefaultBrush"),
    };
}
