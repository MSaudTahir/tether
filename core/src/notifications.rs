//! The phone's current notifications, read from `dumpsys notification` (no app on the phone needed).

use crate::PhoneNotification;

// Notification.FLAG_*: skip what isn't a message for the user.
const FLAG_ONGOING_EVENT: u32 = 0x2;
const FLAG_FOREGROUND_SERVICE: u32 = 0x40;
const FLAG_GROUP_SUMMARY: u32 = 0x200;

/// Parses the "Notification List" of `dumpsys notification --noredact`, newest first.
pub fn parse(out: &str) -> Vec<PhoneNotification> {
    let lines: Vec<&str> = out
        .lines()
        .skip_while(|l| l.trim() != "Notification List:")
        .skip(1)
        // The list ends at the next section at its indent, e.g. "  Lights List:".
        .take_while(|l| !(l.starts_with("  ") && !l.starts_with("   ") && l.trim_end().ends_with(':')))
        .collect();

    let mut notifications = Vec::new();
    let starts: Vec<usize> = lines.iter().enumerate().filter(|(_, l)| l.trim_start().starts_with("NotificationRecord(")).map(|(i, _)| i).collect();
    for (n, &start) in starts.iter().enumerate() {
        let record = &lines[start..starts.get(n + 1).copied().unwrap_or(lines.len())];
        if let Some(notification) = parse_record(record) {
            notifications.push(notification);
        }
    }
    notifications.sort_by(|a, b| b.time.cmp(&a.time));
    notifications
}

fn parse_record(record: &[&str]) -> Option<PhoneNotification> {
    let field = |name: &str| record.iter().find_map(|l| l.trim().strip_prefix(name).map(str::trim));
    let package = record[0].split_once("pkg=")?.1.split_whitespace().next()?.to_string();
    let flags = field("flags=0x").and_then(|f| u32::from_str_radix(f, 16).ok()).unwrap_or(0);
    if flags & (FLAG_ONGOING_EVENT | FLAG_FOREGROUND_SERVICE | FLAG_GROUP_SUMMARY) != 0 {
        return None;
    }
    let title = extra(record, "android.title");
    let text = extra(record, "android.bigText").or_else(|| extra(record, "android.text"));
    if title.is_none() && text.is_none() {
        return None;
    }
    Some(PhoneNotification {
        key: field("key=")?.to_string(),
        package,
        app_name: String::new(),
        icon_path: None,
        title: title.unwrap_or_default(),
        text: text.unwrap_or_default(),
        time: field("when=").and_then(|w| w.parse().ok()).unwrap_or(0),
    })
}

/// An extra like `android.title=String (Hello)`; the value can run over several lines.
fn extra(record: &[&str], name: &str) -> Option<String> {
    let prefix = format!("{name}=");
    let start = record.iter().position(|l| l.trim_start().starts_with(&prefix))?;
    let value = record[start].trim_start()[prefix.len()..].trim_end();
    // "String (...)", "SpannableString (...)"; anything else (null, numbers, ...) isn't text.
    let (kind, rest) = value.split_once(" (")?;
    if !kind.ends_with("String") {
        return None;
    }
    let mut text = rest.to_string();
    let mut next = start + 1;
    while !text.ends_with(')') && next < record.len() {
        text.push('\n');
        text.push_str(record[next].trim_end());
        next += 1;
    }
    let text = text.strip_suffix(')').unwrap_or(&text).trim().to_string();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DUMP: &str = "Current Notification Manager state:
  Notification List:
    NotificationRecord(0x0d08a06e: pkg=com.instagram.android user=UserHandle{0} id=64278 tag=direct importance=4 key=0|com.instagram.android|64278|direct|10270: Notification(channel=ig_direct))
      uid=10270 userId=0
      flags=0x11
      key=0|com.instagram.android|64278|direct|10270
      notification=
            when=1793524329459
            extras={
                android.title=String (Alishba)
                android.subText=null
                android.text=String (Reacted to your message)
            }
    NotificationRecord(0x0c5219d2: pkg=com.spotify.music user=UserHandle{0} id=1 tag=null importance=2 key=0|com.spotify.music|1|null|10100: Notification(channel=playback))
      flags=0x62
      key=0|com.spotify.music|1|null|10100
      notification=
            when=1793524000000
            extras={
                android.title=String (Some Song)
            }
    NotificationRecord(0x0c5219d3: pkg=com.google.android.gm user=UserHandle{0} id=7 tag=null importance=3 key=0|com.google.android.gm|7|null|10120: Notification(channel=mail))
      flags=0x10
      key=0|com.google.android.gm|7|null|10120
      notification=
            when=1793524400000
            extras={
                android.title=SpannableString (Weekly report)
                android.text=String (short)
                android.bigText=SpannableString (Line one
line two)
            }
  Lights List:
    NotificationRecord(0x1: pkg=not.a.notification key=x)
";

    #[test]
    fn parses_active_notifications() {
        let list = parse(DUMP);
        assert_eq!(list.len(), 2, "{list:?}"); // the ongoing Spotify one is skipped
        assert_eq!(list[0].package, "com.google.android.gm"); // newest first
        assert_eq!(list[0].title, "Weekly report");
        assert_eq!(list[0].text, "Line one\nline two");
        assert_eq!(list[1].key, "0|com.instagram.android|64278|direct|10270");
        assert_eq!((list[1].title.as_str(), list[1].text.as_str()), ("Alishba", "Reacted to your message"));
        assert_eq!(list[1].time, 1793524329459);
        assert!(parse("garbage").is_empty());
    }
}
