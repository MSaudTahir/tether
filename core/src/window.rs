//! Virtual display sizes for app windows, stored in the config as "WIDTHxHEIGHT/DPI".
//!
//! Sizes are window sizes in points (what the user sees), and DPI is per point: 160 makes one Android
//! dp one point, so apps look as big as the computer's own. `app_window` turns that into the display's
//! real pixels for the screen.

/// `width`/`height` 0 = the phone's own size; `dpi` 0 = 160.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
    pub dpi: u32,
}

/// A common window size in points, in landscape. `width` 0 means "same as the phone".
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WindowPreset {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// 160 (one dp per point); TextSize scales it.
    pub dpi: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TextSize {
    Smaller,
    Default,
    Larger,
}

impl TextSize {
    const ALL: [TextSize; 3] = [TextSize::Smaller, TextSize::Default, TextSize::Larger];

    fn scale(self, dpi: u32) -> u32 {
        let factor = match self {
            TextSize::Smaller => 0.8,
            TextSize::Default => 1.0,
            TextSize::Larger => 1.25,
        };
        (dpi as f64 * factor).round() as u32
    }
}

/// A window size in picker terms. `preset` is None for a custom size.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WindowChoice {
    pub preset: Option<WindowPreset>,
    pub portrait: bool,
    pub text_size: TextSize,
}

#[uniffi::export]
pub fn window_presets() -> Vec<WindowPreset> {
    [("Small", 960, 540, 160), ("Medium", 1280, 720, 160), ("Large", 1600, 900, 160), ("Same as Phone", 0, 0, 0)]
        .into_iter()
        .map(|(name, width, height, dpi)| WindowPreset { name: name.into(), width, height, dpi })
        .collect()
}

#[uniffi::export]
pub fn preset_window_size(preset: WindowPreset, portrait: bool, text_size: TextSize) -> WindowSize {
    let (width, height) = if portrait { (preset.height, preset.width) } else { (preset.width, preset.height) };
    WindowSize { width, height, dpi: text_size.scale(preset.dpi) }
}

/// Describes a size in terms of the presets, so pickers can show it.
#[uniffi::export]
pub fn window_choice(size: WindowSize) -> WindowChoice {
    let portrait = size.height > size.width;
    let preset = window_presets().into_iter().find_map(|preset| {
        TextSize::ALL
            .into_iter()
            .find(|&text_size| preset_window_size(preset.clone(), portrait, text_size) == size)
            .map(|text_size| (preset, text_size))
    });
    match preset {
        Some((preset, text_size)) => WindowChoice { preset: Some(preset), portrait, text_size },
        None => WindowChoice { preset: None, portrait, text_size: TextSize::Default },
    }
}

/// The usable part of the screen an app window opens on.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Screen {
    /// Points.
    pub width: u32,
    pub height: u32,
    /// Pixels per point (2 on Retina).
    pub scale: f32,
}

/// How to open an app window: the window's size in points, and the display behind it in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppWindow {
    pub width: u32,
    pub height: u32,
    pub display: WindowSize,
}

/// Fits the chosen size on the screen (keeping the app's layout, just smaller) and gives the display
/// the screen's pixel density so it's sharp. `phone` is the phone's screen in pixels and dpi, for
/// "Same as Phone": its size in dp becomes the window's size in points.
pub fn app_window(size: WindowSize, phone: Option<WindowSize>, screen: Screen) -> AppWindow {
    let (width, height, dpi) = match (size.width, phone) {
        (0, Some(phone)) if phone.dpi > 0 => {
            let dp = |px: u32| px as f32 * 160.0 / phone.dpi as f32;
            (dp(phone.width), dp(phone.height), 160.0)
        }
        (0, _) => (412.0, 915.0, 160.0), // a typical phone, in dp
        _ => (size.width as f32, size.height as f32, if size.dpi == 0 { 160.0 } else { size.dpi as f32 }),
    };
    let fit = (0.9 * screen.width as f32 / width).min(0.9 * screen.height as f32 / height).min(1.0);
    let scale = screen.scale.clamp(1.0, 3.0);
    let (width, height, dpi) = (width * fit, height * fit, dpi * fit);
    // Video encoders want sizes in multiples of 8.
    // Video encoders want sizes in multiples of 8; the window follows so pixels map 1:1.
    let px8 = |n: f32| ((n * scale / 8.0).round() as u32 * 8).max(8);
    let display = WindowSize { width: px8(width), height: px8(height), dpi: (dpi * scale).round() as u32 };
    let points = |px: u32| (px as f32 / scale).round() as u32;
    AppWindow { width: points(display.width), height: points(display.height), display }
}

/// "" (device size), "WxH", "/DPI" or "WxH/DPI".
#[uniffi::export]
pub fn parse_window_size(text: String) -> Option<WindowSize> {
    let number = |s: &str| if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) { s.parse().ok() } else { None };
    let (dims, dpi) = text.trim().split_once('/').map_or((text.trim(), None), |(d, p)| (d, Some(p)));
    let (width, height) = if dims.is_empty() {
        (0, 0)
    } else {
        let (w, h) = dims.split_once('x')?;
        (number(w)?, number(h)?)
    };
    let dpi = match dpi {
        Some(dpi) => number(dpi)?,
        None => 0,
    };
    Some(WindowSize { width, height, dpi })
}

#[uniffi::export]
pub fn format_window_size(size: WindowSize) -> String {
    let mut text = String::new();
    if size.width > 0 && size.height > 0 {
        text += &format!("{}x{}", size.width, size.height);
    }
    if size.dpi > 0 {
        text += &format!("/{}", size.dpi);
    }
    text
}

/// A short label, e.g. "Full HD · Portrait" or "1600 × 1000 · 240 dpi".
#[uniffi::export]
pub fn describe_window_size(size: WindowSize) -> String {
    let choice = window_choice(size);
    match choice.preset {
        Some(preset) if preset.width == 0 => preset.name,
        Some(preset) => format!("{} · {}", preset.name, if choice.portrait { "Portrait" } else { "Landscape" }),
        None if size.width == 0 => format!("Same as Phone · {} dpi", size.dpi),
        None if size.dpi == 0 => format!("{} × {}", size.width, size.height),
        None => format!("{} × {} · {} dpi", size.width, size.height, size.dpi),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Option<WindowSize> {
        parse_window_size(text.into())
    }

    #[test]
    fn parse_and_format() {
        for text in ["", "1920x1080", "1920x1080/280", "/240"] {
            assert_eq!(format_window_size(parse(text).unwrap()), text);
        }
        for bad in ["1920", "1920x", "x1080", "1920x1080/", "abc", "1920x1080/2a"] {
            assert!(parse(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn choices() {
        let medium = window_presets().into_iter().find(|p| p.name == "Medium").unwrap();
        let portrait = preset_window_size(medium.clone(), true, TextSize::Larger);
        assert_eq!(portrait, WindowSize { width: 720, height: 1280, dpi: 200 });
        assert_eq!(window_choice(portrait), WindowChoice { preset: Some(medium), portrait: true, text_size: TextSize::Larger });
        assert_eq!(describe_window_size(portrait), "Medium · Portrait");
        assert_eq!(describe_window_size(parse("").unwrap()), "Same as Phone");
        assert_eq!(window_choice(parse("1600x1000/240").unwrap()).preset, None);
        assert_eq!(describe_window_size(parse("1600x1000/240").unwrap()), "1600 × 1000 · 240 dpi");
    }

    #[test]
    fn app_windows() {
        let retina = Screen { width: 1512, height: 875, scale: 2.0 };
        // Fits: one dp per point, twice the pixels.
        let small = app_window(parse("960x540/160").unwrap(), None, retina);
        assert_eq!(small, AppWindow { width: 960, height: 540, display: WindowSize { width: 1920, height: 1080, dpi: 320 } });
        // Too big for the screen: shrunk to 90% of its width, same layout.
        let large = app_window(parse("1600x900/160").unwrap(), None, retina);
        assert_eq!((large.width, large.height), (1360, 764));
        assert_eq!(large.display, WindowSize { width: 2720, height: 1528, dpi: 272 });
        // The phone (1080 × 2400 at 440 dpi) is 393 × 873 dp.
        let phone = WindowSize { width: 1080, height: 2400, dpi: 440 };
        let same = app_window(parse("").unwrap(), Some(phone), retina);
        assert_eq!((same.width, same.height), (356, 788));
        assert_eq!(same.display.width, 712);
    }
}
