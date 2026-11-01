using System;
using System.Linq;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.tether_core;

namespace Tether;

/// Browse the phone's storage: open, download, upload, rename, delete.
static class FilesView
{
    static readonly AppModel Model = AppModel.Shared;

    public static FrameworkElement Build(Device device)
    {
        var page = new StackPanel { Spacing = 12 };
        var folder = Model.CurrentFolder(device);

        var header = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        var up = IconButton("\uE74A", "Up one folder", () => Model.OpenFolder(Parent(folder), device));
        up.IsEnabled = folder != "/";
        header.Children.Add(up);
        header.Children.Add(Breadcrumbs(folder, device));
        if (Model.Transfers.GetValueOrDefault(device.serial) > 0) header.Children.Add(new ProgressRing { IsActive = true, Width = 18, Height = 18 });
        var hidden = new ToggleButton { Content = new FontIcon { Glyph = "\uE7B3", FontSize = 14 }, IsChecked = Model.ShowHiddenFiles };
        ToolTipService.SetToolTip(hidden, "Show hidden files");
        hidden.Click += (_, _) => { Model.ShowHiddenFiles = hidden.IsChecked == true; Model.LoadFiles(device); };
        header.Children.Add(hidden);
        header.Children.Add(IconButton("\uE8F4", "New folder", async () =>
        {
            if (await AskName(page.XamlRoot, "New Folder", "Create", "") is { } name) Model.MakeFolder(name, device);
        }));
        header.Children.Add(IconButton("\uE898", "Upload files from this PC (or drop them here)", async () =>
        {
            var picker = new Windows.Storage.Pickers.FileOpenPicker();
            picker.FileTypeFilter.Add("*");
            WinRT.Interop.InitializeWithWindow.Initialize(picker, WinRT.Interop.WindowNative.GetWindowHandle(App.Window));
            var files = await picker.PickMultipleFilesAsync();
            if (files.Count > 0) Model.Upload(files.Select(f => f.Path), device);
        }));
        header.Children.Add(IconButton("\uE72C", "Reload this folder", () => Model.LoadFiles(device)));
        page.Children.Add(header);

        if (Model.FilesError.TryGetValue(device.serial, out var error))
        {
            page.Children.Add(Dialogs.Note($"Can't open this folder: {error}"));
            return page;
        }
        var files = Model.Files.GetValueOrDefault(device.serial) ?? [];
        if (files.Length == 0)
        {
            page.Children.Add(Dialogs.Note("Empty folder. Drop files here to copy them to the phone."));
            return page;
        }
        var list = new ListView { SelectionMode = ListViewSelectionMode.None, IsItemClickEnabled = true };
        foreach (var item in files) list.Items.Add(Row(item, device));
        list.ItemClick += (_, e) => { if (e.ClickedItem is FrameworkElement { Tag: PhoneFile item }) Model.OpenFile(item, device); };
        page.Children.Add(list);
        return page;
    }

    static FrameworkElement Row(PhoneFile item, Device device)
    {
        var date = DateTimeOffset.FromUnixTimeSeconds(item.modified).ToLocalTime().ToString("g");
        var text = new StackPanel();
        text.Children.Add(new TextBlock { Text = item.name, TextTrimming = TextTrimming.CharacterEllipsis });
        text.Children.Add(Dialogs.Note(item.folder ? $"Folder · {date}" : $"{Size(item.size)} · {date}"));
        var menu = new MenuFlyout();
        void Add(string label, Action action) { var i = new MenuFlyoutItem { Text = label }; i.Click += (_, _) => action(); menu.Items.Add(i); }
        var row = new Grid { Tag = item, ColumnSpacing = 12, Padding = new Thickness(0, 4, 0, 4) };
        Add("Download to Downloads", () => Model.Download([item], device));
        Add("Rename…", async () => { if (await AskName(row.XamlRoot, "Rename", "Rename", item.name) is { } name && name != item.name) Model.Rename(item, name, device); });
        Add("Delete…", async () => { if (await Confirm(row.XamlRoot, $"Delete “{item.name}”?")) Model.Delete([item], device); });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.Children.Add(new FontIcon { Glyph = item.folder ? "\uE8B7" : "\uE8A5", FontSize = 18 });
        Grid.SetColumn(text, 1);
        row.Children.Add(text);
        var more = new Microsoft.UI.Xaml.Controls.Button { Content = new FontIcon { Glyph = "\uE712", FontSize = 14 }, Flyout = menu, Background = null, BorderThickness = new Thickness(0) };
        Grid.SetColumn(more, 2);
        row.Children.Add(more);
        return row;
    }

    /// "Phone › DCIM › Camera": click a part to go there.
    static FrameworkElement Breadcrumbs(string path, Device device)
    {
        var root = TetherCoreMethods.PhoneStorage();
        var inStorage = path == root || path.StartsWith(root + "/");
        var parts = (inStorage ? path[root.Length..] : path).Split('/', StringSplitOptions.RemoveEmptyEntries);
        var row = new StackPanel { Orientation = Orientation.Horizontal, VerticalAlignment = VerticalAlignment.Center };
        void Crumb(string label, string target, bool last)
        {
            var b = new HyperlinkButton { Content = label, FontWeight = last ? Microsoft.UI.Text.FontWeights.SemiBold : Microsoft.UI.Text.FontWeights.Normal };
            b.Click += (_, _) => Model.OpenFolder(target, device);
            row.Children.Add(b);
        }
        if (inStorage) Crumb("Phone", root, parts.Length == 0);
        for (var i = 0; i < parts.Length; i++)
        {
            if (inStorage || i > 0) row.Children.Add(new FontIcon { Glyph = "\uE76C", FontSize = 10, Opacity = 0.6 });
            Crumb(parts[i], (inStorage ? root : "") + "/" + string.Join("/", parts[..(i + 1)]), i == parts.Length - 1);
        }
        return row;
    }

    static string Parent(string folder)
    {
        var i = folder.LastIndexOf('/');
        return i <= 0 ? "/" : folder[..i];
    }

    static string Size(ulong count)
    {
        string[] units = ["bytes", "KB", "MB", "GB", "TB"];
        double value = count;
        var unit = 0;
        while (value >= 1000 && unit < units.Length - 1) { value /= 1000; unit++; }
        return $"{value:0.##} {units[unit]}";
    }

    static Microsoft.UI.Xaml.Controls.Button IconButton(string glyph, string tip, Action action)
    {
        var b = new Microsoft.UI.Xaml.Controls.Button { Content = new FontIcon { Glyph = glyph, FontSize = 14 } };
        ToolTipService.SetToolTip(b, tip);
        b.Click += (_, _) => action();
        return b;
    }

    static async Task<string?> AskName(XamlRoot root, string title, string action, string initial)
    {
        var box = new TextBox { Text = initial };
        var dialog = new ContentDialog { XamlRoot = root, Title = title, Content = box, PrimaryButtonText = action, CloseButtonText = "Cancel", DefaultButton = ContentDialogButton.Primary };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary) return null;
        var name = box.Text.Trim();
        return name == "" || name.Contains('/') ? null : name;
    }

    static async Task<bool> Confirm(XamlRoot root, string title)
    {
        var dialog = new ContentDialog
        {
            XamlRoot = root, Title = title, Content = "It's deleted from the phone right away; this can't be undone.",
            PrimaryButtonText = "Delete", CloseButtonText = "Cancel", DefaultButton = ContentDialogButton.Close,
        };
        return await dialog.ShowAsync() == ContentDialogResult.Primary;
    }
}
