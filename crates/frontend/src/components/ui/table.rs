use dioxus::prelude::*;

#[component]
pub fn Table(#[props(default)] class: String, children: Element) -> Element {
    rsx! {
        div { class: "relative w-full overflow-auto",
            table {
                class: "w-full caption-bottom text-sm {class}",
                {children}
            }
        }
    }
}

#[component]
pub fn TableHeader(#[props(default)] class: String, children: Element) -> Element {
    rsx! {
        thead {
            class: "sticky top-0 bg-background z-10 [&_tr]:border-b {class}",
            {children}
        }
    }
}

#[component]
pub fn TableBody(#[props(default)] class: String, children: Element) -> Element {
    rsx! {
        tbody {
            class: "[&_tr:last-child]:border-0 {class}",
            {children}
        }
    }
}

#[component]
pub fn TableRow(#[props(default)] class: String, children: Element) -> Element {
    rsx! {
        tr {
            class: "border-b transition-colors hover:bg-muted/50 data-[state=selected]:bg-muted {class}",
            {children}
        }
    }
}

#[component]
pub fn TableHead(#[props(default)] class: String, children: Element) -> Element {
    rsx! {
        th {
            class: "h-12 px-4 text-left align-middle font-medium text-muted-foreground [&:has([role=checkbox])]:pr-0 {class}",
            {children}
        }
    }
}

#[component]
pub fn TableCell(
    #[props(default)] class: String,
    #[props(default)] colspan: i64,
    children: Element,
) -> Element {
    rsx! {
        td {
            class: "p-4 align-middle [&:has([role=checkbox])]:pr-0 {class}",
            colspan: if colspan > 0 { colspan },
            {children}
        }
    }
}

/// Placeholder row rendered inside `TableBody` when a list has no data.
///
/// Factors out the repeated `TableRow { td { colspan: N, ... } }` pattern
/// that every list page was hand-rolling on its "nothing to show" path.
#[component]
pub fn EmptyRow(colspan: i64, message: String) -> Element {
    rsx! {
        TableRow {
            td {
                class: "p-4 text-center py-8 text-muted-foreground",
                colspan,
                "{message}"
            }
        }
    }
}
