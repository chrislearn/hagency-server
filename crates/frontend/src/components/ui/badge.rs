use dioxus::prelude::*;

#[derive(Debug, Clone, PartialEq, Default)]
pub enum BadgeVariant {
    #[default]
    Default,
    Secondary,
    Destructive,
    Outline,
    Success,
}

#[component]
pub fn Badge(
    #[props(default)] variant: BadgeVariant,
    #[props(default)] class: String,
    children: Element,
) -> Element {
    let variant_class = match variant {
        BadgeVariant::Default => "bg-primary text-primary-foreground hover:bg-primary/80",
        BadgeVariant::Secondary => "bg-secondary text-secondary-foreground hover:bg-secondary/80",
        BadgeVariant::Destructive => {
            "bg-destructive text-destructive-foreground hover:bg-destructive/80"
        }
        BadgeVariant::Outline => "text-foreground border",
        BadgeVariant::Success => "bg-green-500/10 text-green-600 dark:text-green-400",
    };

    rsx! {
        div {
            class: "inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-semibold transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 {variant_class} {class}",
            {children}
        }
    }
}
