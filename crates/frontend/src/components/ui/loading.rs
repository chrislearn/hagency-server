use dioxus::prelude::*;

#[component]
pub fn LoadingSkeleton(#[props(default)] class: String) -> Element {
    rsx! {
        div { class: "space-y-4 {class}",
            div { class: "h-8 w-48 bg-muted animate-pulse rounded" }
            div { class: "space-y-2",
                div { class: "h-4 w-full bg-muted animate-pulse rounded" }
                div { class: "h-4 w-3/4 bg-muted animate-pulse rounded" }
                div { class: "h-4 w-1/2 bg-muted animate-pulse rounded" }
            }
        }
    }
}

#[component]
pub fn StatsSkeleton(#[props(default = 4)] count: u32) -> Element {
    rsx! {
        div { class: "grid gap-4 md:grid-cols-2 lg:grid-cols-4",
            for _ in 0..count {
                div { class: "rounded-lg border bg-card p-6 shadow-sm",
                    div { class: "h-4 w-24 bg-muted animate-pulse rounded mb-2" }
                    div { class: "h-8 w-16 bg-muted animate-pulse rounded" }
                }
            }
        }
    }
}

#[component]
pub fn PageSkeleton(#[props(default = 3)] card_count: u32) -> Element {
    rsx! {
        div { class: "space-y-6",
            div { class: "space-y-3",
                div { class: "h-8 w-48 bg-muted animate-pulse rounded" }
                div { class: "h-4 w-full max-w-[300px] bg-muted animate-pulse rounded" }
            }
            div { class: "grid gap-4 md:grid-cols-2 lg:grid-cols-4",
                for _ in 0..card_count {
                    div { class: "rounded-lg border bg-card p-6 shadow-sm space-y-3",
                        div { class: "h-4 w-24 bg-muted animate-pulse rounded" }
                        div { class: "h-8 w-16 bg-muted animate-pulse rounded" }
                        div { class: "h-4 w-3/4 bg-muted animate-pulse rounded" }
                    }
                }
            }
            div { class: "rounded-lg border bg-card p-6 shadow-sm space-y-3",
                div { class: "h-5 w-32 bg-muted animate-pulse rounded" }
                div { class: "h-4 w-full bg-muted animate-pulse rounded" }
                div { class: "h-4 w-full bg-muted animate-pulse rounded" }
                div { class: "h-4 w-3/4 bg-muted animate-pulse rounded" }
                div { class: "h-4 w-1/2 bg-muted animate-pulse rounded" }
            }
        }
    }
}

#[component]
pub fn Spinner(#[props(default)] class: String) -> Element {
    rsx! {
        svg {
            class: "animate-spin h-5 w-5 {class}",
            xmlns: "http://www.w3.org/2000/svg",
            fill: "none",
            view_box: "0 0 24 24",
            circle {
                class: "opacity-25",
                cx: "12",
                cy: "12",
                r: "10",
                stroke: "currentColor",
                stroke_width: "4",
            }
            path {
                class: "opacity-75",
                fill: "currentColor",
                d: "M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z",
            }
        }
    }
}
