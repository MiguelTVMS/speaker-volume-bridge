//! Read the desktop clock preference without changing system settings.
#[cfg(target_os = "macos")]
pub fn locale() -> Option<String> {
    normalize_locale(
        &objc2_foundation::NSLocale::autoupdatingCurrentLocale()
            .localeIdentifier()
            .to_string(),
    )
}

#[cfg(windows)]
#[allow(unsafe_code)] // Win32 fills an owned UTF-16 buffer with the user's format locale.
pub fn locale() -> Option<String> {
    let mut buffer = [0_u16; 85];
    // SAFETY: the API receives a valid writable slice, including its terminator.
    let count = unsafe { windows::Win32::Globalization::GetUserDefaultLocaleName(&mut buffer) };
    if count <= 1 {
        return None;
    }
    normalize_locale(&String::from_utf16_lossy(
        &buffer[..usize::try_from(count).ok()? - 1],
    ))
}

#[cfg(target_os = "linux")]
pub fn locale() -> Option<String> {
    let raw = ["LC_ALL", "LC_TIME", "LANG"]
        .into_iter()
        .find_map(|key| std::env::var(key).ok().filter(|value| !value.is_empty()))?;
    normalize_locale(&raw)
}

#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
pub fn locale() -> Option<String> {
    None
}

fn normalize_locale(raw: &str) -> Option<String> {
    let (base, modifiers) = raw.split_once('@').unwrap_or((raw, ""));
    let mut parts: Vec<String> = base
        .split('.')
        .next()?
        .split(['_', '-'])
        .map(str::to_owned)
        .collect();
    let language = parts.first()?;
    if !(2..=8).contains(&language.len())
        || !language.bytes().all(|byte| byte.is_ascii_alphabetic())
        || matches!(language.as_str(), "C" | "POSIX")
        || parts.iter().skip(1).any(|part| {
            part.is_empty()
                || part.len() > 8
                || !part.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
    {
        return None;
    }
    // macOS can retain an English language locale while overriding its region.
    // Intl ignores the Unicode rg key, so make that region explicit in the tag.
    if let Some(region) = modifiers
        .split(';')
        .find_map(|modifier| modifier.strip_prefix("rg="))
        .and_then(|region| region.strip_suffix("zzzz"))
        .filter(|region| valid_region(region))
    {
        let region = region.to_ascii_uppercase();
        if let Some(index) = parts
            .iter()
            .skip(1)
            .take_while(|part| part.len() != 1)
            .position(|part| valid_region(part))
        {
            parts[index + 1] = region;
        } else {
            let index = if parts.get(1).is_some_and(|part| part.len() == 4) {
                2
            } else {
                1
            };
            parts.insert(index, region);
        }
    }
    Some(parts.join("-"))
}

fn valid_region(region: &str) -> bool {
    (region.len() == 2 && region.bytes().all(|byte| byte.is_ascii_alphabetic()))
        || (region.len() == 3 && region.bytes().all(|byte| byte.is_ascii_digit()))
}

#[cfg(target_os = "macos")]
pub fn hour12() -> Option<bool> {
    use objc2_foundation::{NSDateFormatter, NSLocale, NSString};
    let locale = NSLocale::autoupdatingCurrentLocale();
    let pattern = NSDateFormatter::dateFormatFromTemplate_options_locale(
        &NSString::from_str("j"),
        0,
        Some(&locale),
    )?;
    pattern_hour12(&pattern.to_string())
}

#[cfg(windows)]
#[allow(unsafe_code)] // Win32 fills an owned UTF-16 buffer; null locale means user defaults.
pub fn hour12() -> Option<bool> {
    use windows::{
        Win32::Globalization::{GetLocaleInfoEx, LOCALE_STIMEFORMAT},
        core::PCWSTR,
    };
    let mut buffer = [0_u16; 128];
    // SAFETY: the API receives a valid writable slice, including space for its terminator.
    let count = unsafe { GetLocaleInfoEx(PCWSTR::null(), LOCALE_STIMEFORMAT, Some(&mut buffer)) };
    if count <= 1 {
        return None;
    }
    pattern_hour12(&String::from_utf16_lossy(
        &buffer[..usize::try_from(count).ok()? - 1],
    ))
}

#[cfg(target_os = "linux")]
pub fn hour12() -> Option<bool> {
    use std::process::Command;
    // GNOME's explicit desktop preference takes precedence over LC_TIME.
    if std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase()
        .contains("gnome")
    {
        let preference = Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "clock-format"])
            .output()
            .ok();
        if let Some(output) = preference.filter(|output| output.status.success()) {
            match String::from_utf8_lossy(&output.stdout)
                .trim()
                .trim_matches('\'')
            {
                "12h" => return Some(true),
                "24h" => return Some(false),
                _ => {}
            }
        }
    }
    let output = Command::new("locale")
        .args(["-k", "LC_TIME"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let pattern = text.lines().find_map(|line| line.strip_prefix("t_fmt="))?;
    posix_hour12(pattern)
}

#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
pub fn hour12() -> Option<bool> {
    None
}

#[cfg(any(target_os = "macos", windows, test))]
fn pattern_hour12(pattern: &str) -> Option<bool> {
    let mut quoted = false;
    for ch in pattern.chars() {
        match ch {
            '\'' => quoted = !quoted,
            'H' | 'k' if !quoted => return Some(false),
            'h' | 'K' if !quoted => return Some(true),
            _ => {}
        }
    }
    None
}
#[cfg(any(target_os = "linux", test))]
fn posix_hour12(pattern: &str) -> Option<bool> {
    let mut chars = pattern.chars();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            continue;
        }
        let mut token = chars.next()?;
        if token == 'E' || token == 'O' {
            token = chars.next()?;
        }
        match token {
            'H' | 'k' | 'R' | 'T' => return Some(false),
            'I' | 'l' | 'r' => return Some(true),
            _ => {}
        }
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desktop_locale_preserves_regional_overrides_for_webview_formatting() {
        for (raw, expected) in [
            ("en_US@rg=ptzzzz", Some("en-PT")),
            ("en@rg=ptzzzz", Some("en-PT")),
            ("zh_Hans_CN@rg=hkzzzz", Some("zh-Hans-HK")),
            ("en-US", Some("en-US")),
            ("pt_PT.UTF-8", Some("pt-PT")),
            ("C.UTF-8", None),
            ("POSIX", None),
            ("", None),
            ("en_US/invalid", None),
        ] {
            assert_eq!(normalize_locale(raw).as_deref(), expected, "{raw}");
        }
    }
    #[test]
    fn native_patterns_preserve_clock_overrides() {
        assert_eq!(pattern_hour12("HH:mm:ss"), Some(false));
        assert_eq!(pattern_hour12("h:mm tt"), Some(true));
        assert_eq!(pattern_hour12("'hours' HH:mm"), Some(false));
        assert_eq!(posix_hour12("%r"), Some(true));
        assert_eq!(posix_hour12("%H:%M:%S"), Some(false));
        assert_eq!(posix_hour12("%OI:%M %p"), Some(true));
        assert_eq!(posix_hour12("%%H %I:%M"), Some(true));
    }
}
