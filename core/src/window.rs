//! Virtual display sizes for app windows, stored in the config as "WIDTHxHEIGHT/DPI".

/// `width`/`height` 0 = the device's own size; `dpi` 0 = scrcpy's default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
    pub dpi: u32,
}

/// A common size, in landscape. `width` 0 means "same as the phone".
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WindowPreset {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// A comfortable density for this size; TextSize scales it.
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
    [("HD", 1280, 720, 200), ("Full HD", 1920, 1080, 280), ("QHD", 2560, 1440, 360), ("Same as Phone", 0, 0, 0)]
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
        let full_hd = window_presets().into_iter().find(|p| p.name == "Full HD").unwrap();
        let portrait = preset_window_size(full_hd.clone(), true, TextSize::Larger);
        assert_eq!(portrait, WindowSize { width: 1080, height: 1920, dpi: 350 });
        assert_eq!(window_choice(portrait), WindowChoice { preset: Some(full_hd), portrait: true, text_size: TextSize::Larger });
        assert_eq!(describe_window_size(portrait), "Full HD · Portrait");
        assert_eq!(describe_window_size(parse("").unwrap()), "Same as Phone");
        assert_eq!(window_choice(parse("1600x1000/240").unwrap()).preset, None);
        assert_eq!(describe_window_size(parse("1600x1000/240").unwrap()), "1600 × 1000 · 240 dpi");
    }
}
