use dioxus::prelude::*;

use crate::utils::date::{format_timestamp, get_time_since};

/// Renders a timestamp as a human-friendly relative string
/// (e.g. "1 day ago") with a tooltip showing the absolute date
/// on hover.
#[component]
pub fn RelativeTime(ts_ms: u64, #[props(default)] class: String) -> Element {
    if ts_ms == 0 {
        return rsx! { span { class: "{class}", "-" } };
    }

    let relative = get_time_since(ts_ms);
    let absolute = format_timestamp(ts_ms);

    rsx! {
        span { class: "{class}", title: "{absolute}", "{relative}" }
    }
}
