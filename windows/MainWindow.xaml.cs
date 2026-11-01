using System;
using System.Collections.Generic;
using System.Linq;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using uniffi.tether_core;

namespace Tether;

public sealed partial class MainWindow : Window
{
    static readonly Dictionary<string, string> StateHints = new()
    {
        ["unauthorized"] = "Unlock the phone and accept the \"Allow USB debugging?\" prompt.",
        ["offline"] = "The device is offline. Try reconnecting it.",
    };

    readonly AppModel model = AppModel.Shared;
    readonly SetupView Setup = new() { Visibility = Visibility.Collapsed };
    /// What the app grid currently shows, so it's only rebuilt when that changes.
    (DeviceApp[]? apps, string search) shownApps;

    public MainWindow()
    {
        InitializeComponent();
        SetupHost.Content = Setup;
        SectionList.SelectedIndex = 0; // Overview (selecting it in XAML fires before the panels exist)
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(TitleBar);
        AppWindow.SetIcon("Tether.ico");
        AppWindow.Resize(new Windows.Graphics.SizeInt32(1180, 820));
        WindowSizeButton.Flyout = new Flyout { Content = new WindowSizeForm(), Placement = Microsoft.UI.Xaml.Controls.Primitives.FlyoutPlacementMode.BottomEdgeAlignedRight };

        model.Changed += Render;
        model.Alert += message => _ = Dialogs.ShowError(Content.XamlRoot, message);
        Closed += (_, _) => model.Core.StopSessions();
        Render();
        model.Start();
    }

    void Render()
    {
        if (Content.XamlRoot is { } root)
        {
            var scale = root.RasterizationScale;
            var area = Microsoft.UI.Windowing.DisplayArea.GetFromWindowId(AppWindow.Id, Microsoft.UI.Windowing.DisplayAreaFallback.Primary).WorkArea;
            // scrcpy sizes windows in physical pixels on Windows.
            model.Screen = new Screen((uint)(area.Width / scale), (uint)(area.Height / scale), (float)scale, true);
        }
        Setup.Visibility = model.NeedsSetup ? Visibility.Visible : Visibility.Collapsed;
        Main.Visibility = Toolbar.Visibility = model.NeedsSetup ? Visibility.Collapsed : Visibility.Visible;

        UpdateButton.Visibility = model.Update != null ? Visibility.Visible : Visibility.Collapsed;
        UpdateButton.Content = model.InstallingUpdate ? "Updating scrcpy…" : $"Update scrcpy to {model.Update?.version}";
        UpdateButton.IsEnabled = !model.InstallingUpdate;

        RenderDevices();
        var device = model.SelectedDevice;
        RefreshButton.IsEnabled = ToolbarMirror.IsEnabled = device?.state == "device";
        if (device == null)
        {
            ShowEmpty("\uE8EA", "No Devices", "Plug in a phone with USB debugging on, or pair one over Wi-Fi.", pair: true);
        }
        else if (device.state != "device")
        {
            ShowEmpty("\uE72E", device.label, StateHints.GetValueOrDefault(device.state, $"The device is {device.state}."), pair: false);
        }
        else
        {
            Empty.Visibility = Visibility.Collapsed;
            Detail.Visibility = Visibility.Visible;
            RenderDevice(device);
        }
    }

    void ShowEmpty(string glyph, string title, string text, bool pair)
    {
        Detail.Visibility = Visibility.Collapsed;
        Empty.Visibility = Visibility.Visible;
        EmptyIcon.Glyph = glyph;
        EmptyTitle.Text = title;
        EmptyText.Text = text;
        EmptyPair.Visibility = pair ? Visibility.Visible : Visibility.Collapsed;
    }

    /// The sidebar's phone picker: the selected phone, and a menu to switch or pair one.
    void RenderDevices()
    {
        var device = model.SelectedDevice;
        PickerTitle.Text = device != null ? model.Name(device) : "No Phone";
        PickerSubtitle.Text = device != null ? Subtitle(device) : "Connect or pair a phone";
        var menu = new MenuFlyout();
        foreach (var d in model.Devices)
        {
            var item = new ToggleMenuFlyoutItem { Text = model.Name(d), IsChecked = d.serial == model.Selection };
            item.Click += (_, _) => model.Select(d.serial);
            menu.Items.Add(item);
        }
        if (model.Devices.Length > 0) menu.Items.Add(new MenuFlyoutSeparator());
        var pair = new MenuFlyoutItem { Text = "Pair over Wi-Fi…", Icon = new FontIcon { Glyph = "\uE701" } };
        pair.Click += (_, _) => _ = Dialogs.ShowPair(Content.XamlRoot);
        menu.Items.Add(pair);
        DevicePicker.Flyout = menu;
        SectionList.Visibility = device?.state == "device" ? Visibility.Visible : Visibility.Collapsed;
        var count = model.Selection is { } serial ? model.Notifications.GetValueOrDefault(serial)?.Length ?? 0 : 0;
        NotificationCount.Text = count > 0 ? count.ToString() : "";
    }

    string Subtitle(Device device)
    {
        var transport = device.wireless ? "Wi-Fi" : "USB";
        if (device.state != "device") return $"{transport} · {device.state}";
        return model.Info.TryGetValue(device.serial, out var info) ? $"{transport} · {info.battery}%" : transport;
    }

    void RenderDevice(Device device)
    {
        model.Info.TryGetValue(device.serial, out var info);
        DeviceName.Text = model.Name(device);
        DeviceSubtitle.Text = info != null ? $"{info.model} · Android {info.android}" : "";
        Pills.Children.Clear();
        if (info != null) Pills.Children.Add(Pill(BatteryGlyph(info), $"{info.battery}% · {Capitalized(info.power)}"));
        Pills.Children.Add(Pill(device.wireless ? "\uE701" : "\uE88E", device.wireless ? "Wi-Fi" : "USB"));
        Pills.Children.Add(Pill("\uE8EC", device.shortSerial));
        WirelessButton.Visibility = device.wireless ? Visibility.Collapsed : Visibility.Visible;
        CaptureMouse.IsChecked = model.Config.captureMouse;
        WindowSizeButton.Content = TetherCoreMethods.DescribeWindowSize(model.WindowSize);

        ControlsHost.Content = info != null ? ControlPanel.Controls(device, info) : null;
        ClipboardHost.Content = info != null ? ControlPanel.Clipboard(device) : null;
        DetailsHost.Content = info != null ? ControlPanel.Details(info) : null;
        var notifications = model.Notifications.GetValueOrDefault(device.serial) ?? [];
        NotificationsHost.Content = ControlPanel.Notifications(device, notifications);
        FilesHost.Content = FilesView.Build(device);
        PhotosHost.Content = PhotosView.Build(device);

        var supported = (info?.sdk ?? uint.MaxValue) >= TetherCoreMethods.VirtualDisplayMinSdk();
        AppsHint.Text = supported ? "Click an app to open it in its own window." : "App windows need Android 10 or newer.";
        AppGrid.IsEnabled = supported;

        model.Apps.TryGetValue(device.serial, out var apps);
        model.AppErrors.TryGetValue(device.serial, out var error);
        AppCount.Text = apps != null ? apps.Length.ToString() : "";
        var loading = model.LoadingApps.Contains(device.serial) && apps == null;
        AppsLoading.IsActive = loading;
        AppsLoading.Visibility = loading ? Visibility.Visible : Visibility.Collapsed;
        AppsError.Visibility = error != null ? Visibility.Visible : Visibility.Collapsed;
        AppsErrorText.Text = error ?? "";
        AppGrid.Visibility = !loading && error == null ? Visibility.Visible : Visibility.Collapsed;
        RenderApps(apps);
    }

    void RenderApps(DeviceApp[]? apps)
    {
        var search = Search.Text.Trim();
        if (ReferenceEquals(shownApps.apps, apps) && shownApps.search == search) return;
        shownApps = (apps, search);
        AppGrid.ItemsSource = (apps ?? [])
            .Where(app => search == "" || app.name.Contains(search, StringComparison.OrdinalIgnoreCase)
                || app.package.Contains(search, StringComparison.OrdinalIgnoreCase))
            .Select(app => { var tile = AppTile(app); tile.ContextFlyout = AppMenu(app); return tile; })
            .ToList();
    }

    static FrameworkElement AppTile(DeviceApp app)
    {
        var tile = new StackPanel { Width = 96, Spacing = 6, Padding = new Thickness(4, 8, 4, 4), Tag = app };
        tile.Children.Add(AppIcon(app));
        tile.Children.Add(new TextBlock
        {
            Text = app.name, TextAlignment = TextAlignment.Center, TextWrapping = TextWrapping.Wrap,
            MaxLines = 2, TextTrimming = TextTrimming.CharacterEllipsis, Height = 36, FontSize = 13,
        });
        ToolTipService.SetToolTip(tile, app.package);
        return tile;
    }

    /// Right-click actions for an app.
    MenuFlyout AppMenu(DeviceApp app)
    {
        var menu = new MenuFlyout();
        void Add(string text, Action<Device> action)
        {
            var item = new MenuFlyoutItem { Text = text };
            item.Click += (_, _) => { if (model.SelectedDevice is { } d) action(d); };
            menu.Items.Add(item);
        }
        Add("Open in Window", d => model.Open(app, d));
        Add("App Info on Phone", d => model.ShowAppInfo(app, d));
        menu.Items.Add(new MenuFlyoutSeparator());
        Add("Force Stop", d => model.ForceStop(app, d));
        Add("Clear Data…", async d =>
        {
            if (await Confirm($"Clear {app.name}'s data?", "Its files, settings and sign-ins are deleted from the phone, like \"Clear storage\" in Settings.", "Clear Data")) model.ClearData(app, d);
        });
        if (!app.system)
            Add("Uninstall…", async d =>
            {
                if (await Confirm($"Uninstall {app.name}?", "It's removed from the phone along with its data.", "Uninstall")) model.Uninstall(app, d);
            });
        return menu;
    }

    async System.Threading.Tasks.Task<bool> Confirm(string title, string text, string action)
    {
        var dialog = new ContentDialog { XamlRoot = Content.XamlRoot, Title = title, Content = text, PrimaryButtonText = action, CloseButtonText = "Cancel", DefaultButton = ContentDialogButton.Close };
        return await dialog.ShowAsync() == ContentDialogResult.Primary;
    }

    /// The app's cached icon, or a letter on a color derived from its name.
    internal static FrameworkElement AppIcon(DeviceApp app, double size = 56)
    {
        if (app.iconPath != null)
        {
            return new Image { Width = size, Height = size, Source = new BitmapImage(new Uri(app.iconPath)) { DecodePixelWidth = (int)size * 2 } };
        }
        var hue = app.package.Aggregate(0, (hash, c) => (hash * 31 + c) % 360);
        var letter = new Border { Width = size, Height = size, CornerRadius = new CornerRadius(size / 4), Background = new SolidColorBrush(FromHsv(hue, 0.45, 0.8)) };
        letter.Child = new TextBlock
        {
            Text = app.name[..1].ToUpperInvariant(), FontSize = size / 2.2, FontWeight = Microsoft.UI.Text.FontWeights.Bold,
            Foreground = new SolidColorBrush(Microsoft.UI.Colors.White), HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center,
        };
        return letter;
    }

    static Windows.UI.Color FromHsv(int hue, double s, double v)
    {
        double c = v * s, x = c * (1 - Math.Abs(hue / 60.0 % 2 - 1)), m = v - c;
        var (r, g, b) = (hue / 60) switch { 0 => (c, x, 0.0), 1 => (x, c, 0.0), 2 => (0.0, c, x), 3 => (0.0, x, c), 4 => (x, 0.0, c), _ => (c, 0.0, x) };
        return Windows.UI.Color.FromArgb(255, (byte)((r + m) * 255), (byte)((g + m) * 255), (byte)((b + m) * 255));
    }

    static UIElement Pill(string glyph, string text)
    {
        var content = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        content.Children.Add(new FontIcon { Glyph = glyph, FontSize = 13 });
        content.Children.Add(new TextBlock { Text = text, IsTextSelectionEnabled = true });
        return new Border
        {
            Child = content, Padding = new Thickness(10, 4, 10, 4), CornerRadius = new CornerRadius(12),
            Background = Brush("SubtleFillColorSecondaryBrush"),
        };
    }

    static string BatteryGlyph(DeviceInfo info)
    {
        if (info.power.StartsWith("charging") || info.power.StartsWith("full")) return "\uEBB5";
        return ((char)(0xEBA0 + Math.Min(10, info.battery / 10))).ToString(); // Battery0..Battery10
    }

    static string Capitalized(string text) => text == "" ? text : char.ToUpper(text[0]) + text[1..];
    static Style Style(string key) => Dialogs.Resource<Style>(key);
    static Brush Brush(string key) => Dialogs.Resource<Brush>(key);

    // --- Events --------------------------------------------------------------

    void AppGrid_ItemClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is FrameworkElement { Tag: DeviceApp app } && model.SelectedDevice is { } device) model.Open(app, device);
    }

    void Detail_DragOver(object sender, DragEventArgs e)
    {
        if (e.DataView.Contains(Windows.ApplicationModel.DataTransfer.StandardDataFormats.StorageItems))
        {
            e.AcceptedOperation = Windows.ApplicationModel.DataTransfer.DataPackageOperation.Copy;
            e.DragUIOverride.Caption = "Install on the phone";
        }
    }

    /// Dropped files upload to the folder shown in Files; elsewhere, dropped .apk files are installed.
    async void Detail_Drop(object sender, DragEventArgs e)
    {
        if (model.SelectedDevice is not { } device) return;
        var paths = (await e.DataView.GetStorageItemsAsync()).Select(item => item.Path).ToList();
        if (FilesHost.Visibility == Visibility.Visible) { model.Upload(paths, device); return; }
        var apks = paths.Where(path => path.EndsWith(".apk", StringComparison.OrdinalIgnoreCase)).ToList();
        if (apks.Count > 0) model.Install(apks, device);
    }

    /// Shows the section selected in the sidebar.
    void SectionList_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (OverviewPanel == null) return; // still loading
        var section = (SectionList.SelectedItem as ListViewItem)?.Tag as string ?? "overview";
        OverviewPanel.Visibility = section == "overview" ? Visibility.Visible : Visibility.Collapsed;
        AppsPanel.Visibility = section == "apps" ? Visibility.Visible : Visibility.Collapsed;
        NotificationsHost.Visibility = section == "notifications" ? Visibility.Visible : Visibility.Collapsed;
        FilesHost.Visibility = section == "files" ? Visibility.Visible : Visibility.Collapsed;
        PhotosHost.Visibility = section == "photos" ? Visibility.Visible : Visibility.Collapsed;
        if (section == "photos" && model.SelectedDevice is { } shown && !model.Photos.ContainsKey(shown.serial)) model.LoadPhotos(shown);
        if (section == "files" && model.SelectedDevice is { } device && !model.Files.ContainsKey(device.serial)) model.LoadFiles(device);
    }

    void Search_TextChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args) => Render();
    void Mirror_Click(object sender, RoutedEventArgs e) { if (model.SelectedDevice is { } d) model.Mirror(d); }
    void Wireless_Click(object sender, RoutedEventArgs e) { if (model.SelectedDevice is { } d) model.GoWireless(d); }
    void CaptureMouse_Click(object sender, RoutedEventArgs e) => model.SetCaptureMouse(CaptureMouse.IsChecked == true);
    void Refresh_Click(object sender, RoutedEventArgs e) => model.Refresh();
    void Update_Click(object sender, RoutedEventArgs e) => model.InstallUpdate();
    async void Pair_Click(object sender, RoutedEventArgs e) => await Dialogs.ShowPair(Content.XamlRoot);
    async void Settings_Click(object sender, RoutedEventArgs e) => await Dialogs.ShowSettings(Content.XamlRoot);
}
