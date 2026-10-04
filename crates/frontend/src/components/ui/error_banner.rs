use dioxus::prelude::*;

use crate::utils::i18n::t;

/// Shared inline error banner used on list/detail pages.
///
/// Renders the repeated `bg-destructive/10` panel with a muted error
/// message and an optional "Retry" link wired to `on_retry`. Replaces
/// the hand-rolled `div { class: "rounded-md bg-destructive/10 ..." }`
/// block that was duplicated across 20+ pages.
#[component]
pub fn ErrorBanner(
    message: String,
    #[props(default)] on_retry: Option<EventHandler<MouseEvent>>,
) -> Element {
    let retry_label = t("common.retry");
    let error_label = t("common.error");
    rsx! {
        div { class: "rounded-md bg-destructive/10 p-4",
            div { class: "flex items-center justify-between gap-4",
                p { class: "text-sm text-destructive",
                    "{error_label}: {message}"
                }
                if let Some(handler) = on_retry {
                    button {
                        class: "text-sm font-medium text-primary hover:underline shrink-0",
                        onclick: move |evt| handler.call(evt),
                        "{retry_label}"
                    }
                }
            }
        }
    }
}
