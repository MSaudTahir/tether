using System.Linq;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.tether_core;

namespace Tether;

/// Picks the virtual display size for app windows: a preset, portrait/landscape, text size, or a custom size.
sealed class WindowSizeForm : StackPanel
{
    readonly AppModel model = AppModel.Shared;
    readonly WindowPreset[] presets = TetherCoreMethods.WindowPresets();
    readonly RadioButtons size = new() { Header = "Size" };
    readonly RadioButtons orientation = new() { Header = "Orientation", MaxColumns = 2 };
    readonly RadioButtons textSize = new() { Header = "Text size", MaxColumns = 3 };
    readonly TextBox custom = new() { PlaceholderText = "1600x1000/240", Width = 200 };
    readonly TextBlock footer = new() { TextWrapping = TextWrapping.Wrap, FontSize = 12 };
    readonly CheckBox resizable = new() { Content = "Resizable: the app relayouts when you resize its window" };
    bool rendering;

    public WindowSizeForm()
    {
        Width = 320;
        Spacing = 16;
        foreach (var preset in presets)
        {
            size.Items.Add(preset.width == 0 ? preset.name : $"{preset.name}  ({preset.width} × {preset.height})");
        }
        size.Items.Add("Custom");
        orientation.Items.Add("Landscape");
        orientation.Items.Add("Portrait");
        textSize.Items.Add("Smaller");
        textSize.Items.Add("Default");
        textSize.Items.Add("Larger");

        var set = new Button { Content = "Set" };
        set.Click += (_, _) => SetCustom();
        custom.KeyDown += (_, e) => { if (e.Key == Windows.System.VirtualKey.Enter) SetCustom(); };
        var customRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        customRow.Children.Add(custom);
        customRow.Children.Add(set);
        var customSection = new StackPanel { Spacing = 8 };
        customSection.Children.Add(new TextBlock { Text = "Custom size" });
        customSection.Children.Add(customRow);

        Children.Add(size);
        Children.Add(orientation);
        Children.Add(textSize);
        Children.Add(resizable);
        Children.Add(customSection);
        Children.Add(footer);

        size.SelectionChanged += (_, _) => Changed(() =>
        {
            var choice = TetherCoreMethods.WindowChoice(model.WindowSize);
            if (size.SelectedIndex >= 0 && size.SelectedIndex < presets.Length)
                model.SetWindowSize(TetherCoreMethods.PresetWindowSize(presets[size.SelectedIndex], choice.portrait, choice.textSize));
        });
        orientation.SelectionChanged += (_, _) => Changed(() => SetOrientation(portrait: orientation.SelectedIndex == 1));
        textSize.SelectionChanged += (_, _) => Changed(() =>
        {
            var choice = TetherCoreMethods.WindowChoice(model.WindowSize);
            if (choice.preset is { } preset)
                model.SetWindowSize(TetherCoreMethods.PresetWindowSize(preset, choice.portrait, (TextSize)textSize.SelectedIndex));
        });
        resizable.Click += (_, _) => model.Save(model.Config with { resizableWindows = resizable.IsChecked == true });
        model.Changed += Render;
        Render();
        Footer(error: false);
    }

    void Changed(System.Action action)
    {
        if (!rendering) action();
    }

    void Render()
    {
        rendering = true;
        var current = model.WindowSize;
        var choice = TetherCoreMethods.WindowChoice(current);
        var sameAsPhone = choice.preset?.width == 0;
        size.SelectedIndex = choice.preset is { } preset ? presets.ToList().FindIndex(p => p.name == preset.name) : presets.Length;
        orientation.SelectedIndex = choice.portrait ? 1 : 0;
        orientation.IsEnabled = !sameAsPhone && current.width != 0;
        textSize.SelectedIndex = (int)choice.textSize;
        textSize.IsEnabled = choice.preset != null && !sameAsPhone;
        resizable.IsChecked = model.Config.resizableWindows;
        if (FocusManager.GetFocusedElement(XamlRoot) != custom) custom.Text = model.Config.windowSize;
        rendering = false;
    }

    void SetOrientation(bool portrait)
    {
        var current = model.WindowSize;
        var choice = TetherCoreMethods.WindowChoice(current);
        if (choice.preset is { } preset)
            model.SetWindowSize(TetherCoreMethods.PresetWindowSize(preset, portrait, choice.textSize));
        else if (portrait != current.height > current.width)
            model.SetWindowSize(new WindowSize(current.height, current.width, current.dpi));
    }

    void SetCustom()
    {
        var parsed = TetherCoreMethods.ParseWindowSize(custom.Text);
        Footer(error: parsed == null);
        if (parsed != null) model.SetWindowSize(parsed);
    }

    void Footer(bool error)
    {
        footer.Text = error ? "Use WIDTHxHEIGHT/DPI, e.g. 1600x1000/240." : "Portrait suits most phone apps. Smaller text fits more on screen.";
        footer.Foreground = (Microsoft.UI.Xaml.Media.Brush)Application.Current.Resources[error ? "SystemFillColorCriticalBrush" : "TextFillColorSecondaryBrush"];
    }
}
