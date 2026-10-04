use dioxus::prelude::*;

#[component]
pub fn PageHeader(
    title: String,
    #[props(default)] description: String,
    #[props(default)] children: Element,
) -> Element {
    rsx! {
        div { class: "page-header",
            div { class: "space-y-1",
                h1 { class: "text-2xl font-bold tracking-tight", "{title}" }
                if !description.is_empty() {
                    p { class: "text-muted-foreground", "{description}" }
                }
            }
            div { class: "page-header-actions",
                {children}
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BreadcrumbItem {
    pub label: String,
    pub href: Option<String>,
}

#[component]
pub fn Breadcrumbs(items: Vec<BreadcrumbItem>) -> Element {
    rsx! {
        nav { class: "flex items-center space-x-1 text-sm text-muted-foreground mb-4",
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    span { class: "mx-1", "/" }
                }
                if let Some(ref href) = item.href {
                    // Router navigation, not a full page load: the access
                    // token only lives in memory and would be lost.
                    Link {
                        to: href.clone(),
                        class: "hover:text-foreground transition-colors",
                        "{item.label}"
                    }
                } else {
                    span { class: "text-foreground font-medium", "{item.label}" }
                }
            }
        }
    }
}
