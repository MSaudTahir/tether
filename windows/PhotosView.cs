using System;
using System.Collections.Generic;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using uniffi.tether_core;

namespace Tether;

/// The phone's newest photos and videos; click one to open it, right-click to download it.
static class PhotosView
{
    static readonly AppModel Model = AppModel.Shared;
    static readonly string[] Videos = [".mp4", ".mkv", ".webm", ".3gp", ".mov"];

    public static FrameworkElement Build(Device device)
    {
        var page = new StackPanel { Spacing = 12 };
        var header = new Grid();
        header.Children.Add(new TextBlock { Text = "Recent Photos", Style = Dialogs.Resource<Style>("SubtitleTextBlockStyle") });
        var refresh = new Microsoft.UI.Xaml.Controls.Button { Content = new FontIcon { Glyph = "\uE72C", FontSize = 14 }, HorizontalAlignment = HorizontalAlignment.Right };
        ToolTipService.SetToolTip(refresh, "Reload");
        refresh.Click += (_, _) => Model.LoadPhotos(device);
        header.Children.Add(refresh);
        page.Children.Add(header);
        page.Children.Add(Dialogs.Note("From the camera and screenshots. Click one to open it; right-click to download it."));

        var photos = Model.Photos.GetValueOrDefault(device.serial);
        if (photos == null) return page;
        if (photos.Length == 0)
        {
            page.Children.Add(Dialogs.Note("No photos yet. Photos taken on the phone show up here."));
            return page;
        }
        var grid = new GridView { SelectionMode = ListViewSelectionMode.None, IsItemClickEnabled = true };
        foreach (var photo in photos) grid.Items.Add(Tile(photo, device));
        grid.ItemClick += (_, e) => { if (e.ClickedItem is FrameworkElement { Tag: Photo photo }) Model.OpenFile(photo.file, device); };
        page.Children.Add(grid);
        return page;
    }

    static FrameworkElement Tile(Photo photo, Device device)
    {
        var tile = new Grid { Width = 150, Height = 150, Tag = photo, CornerRadius = new CornerRadius(8), Background = Dialogs.Resource<Brush>("CardBackgroundFillColorDefaultBrush") };
        if (photo.thumbnailPath != null)
            tile.Children.Add(new Image { Source = new BitmapImage(new Uri(photo.thumbnailPath)) { DecodePixelWidth = 300 }, Stretch = Stretch.UniformToFill });
        if (Array.Exists(Videos, e => photo.file.name.EndsWith(e, StringComparison.OrdinalIgnoreCase)))
            tile.Children.Add(new FontIcon { Glyph = "\uE768", FontSize = 22, Foreground = new SolidColorBrush(Microsoft.UI.Colors.White), HorizontalAlignment = HorizontalAlignment.Right, VerticalAlignment = VerticalAlignment.Bottom, Margin = new Thickness(6) });
        ToolTipService.SetToolTip(tile, photo.file.name);
        var menu = new MenuFlyout();
        var download = new MenuFlyoutItem { Text = "Download to Downloads" };
        download.Click += (_, _) => Model.Download([photo.file], device);
        menu.Items.Add(download);
        tile.ContextFlyout = menu;
        return tile;
    }
}
