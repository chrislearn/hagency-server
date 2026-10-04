use dioxus::prelude::*;

use super::icons::Icon;

#[component]
pub fn EmptyState(
    icon: String,
    title: String,
    description: String,
    #[props(default)] action_label: Option<String>,
    #[props(default)] action_href: Option<String>,
) -> Element {
    rsx! {
        div { class: "flex flex-col items-center justify-center py-12 px-4",
            div { class: "rounded-full bg-muted p-4 mb-4",
                Icon { name: icon, class: "h-8 w-8 text-muted-foreground".to_string() }
            }
            h3 { class: "text-lg font-semibold mb-1", "{title}" }
            p { class: "text-sm text-muted-foreground mb-4 text-center max-w-sm", "{description}" }
            if let (Some(label), Some(href)) = (action_label, action_href) {
                Link {
                    to: "{href}",
                    class: "inline-flex items-center justify-center whitespace-nowrap rounded-lg text-sm font-medium h-9 px-4 py-2 btn-gradient",
                    "{label}"
                }
            }
        }
    }
}
