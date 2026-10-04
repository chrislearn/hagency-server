use dioxus::prelude::*;

#[component]
pub fn Card(#[props(default)] class: String, children: Element) -> Element {
    rsx! {
        div {
            class: "rounded-lg border text-card-foreground shadow-sm glass-panel {class}",
            {children}
        }
    }
}

#[component]
pub fn CardHeader(#[props(default)] class: String, children: Element) -> Element {
    rsx! {
        div {
            class: "flex flex-col space-y-1.5 p-6 {class}",
            {children}
        }
    }
}

#[component]
pub fn CardTitle(#[props(default)] class: String, children: Element) -> Element {
    rsx! {
        h3 {
            class: "text-2xl font-semibold leading-none tracking-tight {class}",
            {children}
        }
    }
}

#[component]
pub fn CardDescription(#[props(default)] class: String, children: Element) -> Element {
    rsx! {
        p {
            class: "text-sm text-muted-foreground {class}",
            {children}
        }
    }
}

#[component]
pub fn CardContent(#[props(default)] class: String, children: Element) -> Element {
    rsx! {
        div {
            class: "p-6 pt-0 {class}",
            {children}
        }
    }
}

#[component]
pub fn CardFooter(#[props(default)] class: String, children: Element) -> Element {
    rsx! {
        div {
            class: "flex items-center p-6 pt-0 {class}",
            {children}
        }
    }
}
