pub fn format_timestamp(ts_ms: u64) -> String {
    if ts_ms == 0 {
        return "-".to_string();
    }
    let secs = (ts_ms / 1000) as i64;
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "-".to_string())
}

/// Like [`format_timestamp`] but without seconds (`YYYY-MM-DD HH:MM`).
pub fn format_timestamp_short(ts_ms: u64) -> String {
    if ts_ms == 0 {
        return "-".to_string();
    }
    let secs = (ts_ms / 1000) as i64;
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "-".to_string())
}

pub fn get_time_since(ts_ms: u64) -> String {
    if ts_ms == 0 {
        return "-".to_string();
    }

    let now = chrono::Utc::now().timestamp_millis() as u64;
    if ts_ms > now {
        return "just now".to_string();
    }

    let diff_secs = (now - ts_ms) / 1000;

    if diff_secs < 60 {
        return "just now".to_string();
    }

    let minutes = diff_secs / 60;
    if minutes < 60 {
        return if minutes == 1 {
            "1 minute ago".to_string()
        } else {
            format!("{minutes} minutes ago")
        };
    }

    let hours = minutes / 60;
    if hours < 24 {
        return if hours == 1 {
            "1 hour ago".to_string()
        } else {
            format!("{hours} hours ago")
        };
    }

    let days = hours / 24;
    if days < 30 {
        return if days == 1 {
            "1 day ago".to_string()
        } else {
            format!("{days} days ago")
        };
    }

    let months = days / 30;
    if months < 12 {
        return if months == 1 {
            "1 month ago".to_string()
        } else {
            format!("{months} months ago")
        };
    }

    let years = months / 12;
    if years == 1 {
        "1 year ago".to_string()
    } else {
        format!("{years} years ago")
    }
}

pub fn format_duration_ms(ms: u64) -> String {
    let secs = ms / 1000;
    if secs >= 3600 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}
