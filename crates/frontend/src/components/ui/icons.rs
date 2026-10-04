use dioxus::prelude::*;

#[component]
pub fn Icon(name: String, #[props(default = "h-4 w-4".to_string())] class: String) -> Element {
    match name.as_str() {
        "users" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" }
                circle { cx: "9", cy: "7", r: "4" }
                path { d: "M22 21v-2a4 4 0 0 0-3-3.87" }
                path { d: "M16 3.13a4 4 0 0 1 0 7.75" }
            }
        },
        "message-square" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" }
            }
        },
        "flag" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M4 15s1-1 4-1 5 2 8 2 4-1 4-1V3s-1 1-4 1-5-2-8-2-4 1-4 1z" }
                line { x1: "4", x2: "4", y1: "22", y2: "15" }
            }
        },
        "server" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                rect { width: "20", height: "8", x: "2", y: "2", rx: "2", ry: "2" }
                rect { width: "20", height: "8", x: "2", y: "14", rx: "2", ry: "2" }
                line { x1: "6", x2: "6.01", y1: "6", y2: "6" }
                line { x1: "6", x2: "6.01", y1: "18", y2: "18" }
            }
        },
        "image" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                rect { width: "18", height: "18", x: "3", y: "3", rx: "2", ry: "2" }
                circle { cx: "9", cy: "9", r: "2" }
                path { d: "m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21" }
            }
        },
        "globe" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                circle { cx: "12", cy: "12", r: "10" }
                path { d: "M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20" }
                path { d: "M2 12h20" }
            }
        },
        "key" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "m15.5 7.5 2.3 2.3a1 1 0 0 0 1.4 0l2.1-2.1a1 1 0 0 0 0-1.4L19 4" }
                path { d: "m21 2-9.6 9.6" }
                circle { cx: "7.5", cy: "15.5", r: "5.5" }
            }
        },
        "shield" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z" }
            }
        },
        "activity" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M22 12h-2.48a2 2 0 0 0-1.93 1.46l-2.35 8.36a.25.25 0 0 1-.48 0L9.24 2.18a.25.25 0 0 0-.48 0l-2.35 8.36A2 2 0 0 1 4.49 12H2" }
            }
        },
        "layout-dashboard" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                rect { width: "7", height: "9", x: "3", y: "3", rx: "1" }
                rect { width: "7", height: "5", x: "14", y: "3", rx: "1" }
                rect { width: "7", height: "9", x: "14", y: "12", rx: "1" }
                rect { width: "7", height: "5", x: "3", y: "16", rx: "1" }
            }
        },
        "log-out" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" }
                polyline { points: "16 17 21 12 16 7" }
                line { x1: "21", x2: "9", y1: "12", y2: "12" }
            }
        },
        "plus" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M5 12h14" }
                path { d: "M12 5v14" }
            }
        },
        "lock" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                rect { width: "18", height: "11", x: "3", y: "11", rx: "2", ry: "2" }
                path { d: "M7 11V7a5 5 0 0 1 10 0v4" }
            }
        },
        "moon" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9z" }
            }
        },
        "sun" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                circle { cx: "12", cy: "12", r: "4" }
                path { d: "M12 2v2" }
                path { d: "M12 20v2" }
                path { d: "m4.93 4.93 1.41 1.41" }
                path { d: "m17.66 17.66 1.41 1.41" }
                path { d: "M2 12h2" }
                path { d: "M20 12h2" }
                path { d: "m6.34 17.66-1.41 1.41" }
                path { d: "m19.07 4.93-1.41 1.41" }
            }
        },
        "upload" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" }
                polyline { points: "17 8 12 3 7 8" }
                line { x1: "12", x2: "12", y1: "3", y2: "15" }
            }
        },
        "download" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" }
                polyline { points: "7 10 12 15 17 10" }
                line { x1: "12", x2: "12", y1: "15", y2: "3" }
            }
        },
        "refresh-cw" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                polyline { points: "23 4 23 10 17 10" }
                polyline { points: "1 20 1 14 7 14" }
                path { d: "M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" }
            }
        },
        "alert-triangle" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z" }
                line { x1: "12", x2: "12", y1: "9", y2: "13" }
                line { x1: "12", x2: "12.01", y1: "17", y2: "17" }
            }
        },
        "info" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                circle { cx: "12", cy: "12", r: "10" }
                path { d: "M12 16v-4" }
                path { d: "M12 8h.01" }
            }
        },
        "code" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                polyline { points: "16 18 22 12 16 6" }
                polyline { points: "8 6 2 12 8 18" }
            }
        },
        "x" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M18 6 6 18" }
                path { d: "m6 6 12 12" }
            }
        },
        "edit" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z" }
                path { d: "m15 5 4 4" }
            }
        },
        "trash" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M3 6h18" }
                path { d: "M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" }
                path { d: "M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" }
            }
        },
        "clock" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                circle { cx: "12", cy: "12", r: "10" }
                polyline { points: "12 6 12 12 16 14" }
            }
        },
        "hash" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                line { x1: "4", x2: "20", y1: "9", y2: "9" }
                line { x1: "4", x2: "20", y1: "15", y2: "15" }
                line { x1: "10", x2: "8", y1: "3", y2: "21" }
                line { x1: "16", x2: "14", y1: "3", y2: "21" }
            }
        },
        "user" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2" }
                circle { cx: "12", cy: "7", r: "4" }
            }
        },
        "user-check" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" }
                circle { cx: "9", cy: "7", r: "4" }
                polyline { points: "16 11 18 13 22 9" }
            }
        },
        "monitor" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                rect { width: "20", height: "14", x: "2", y: "3", rx: "2" }
                line { x1: "8", x2: "16", y1: "21", y2: "21" }
                line { x1: "12", x2: "12", y1: "17", y2: "21" }
            }
        },
        "layers" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "m12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83Z" }
                path { d: "m22.4 10.08-8.58 3.91a2 2 0 0 1-1.66 0l-8.58-3.9" }
                path { d: "m22.4 14.08-8.58 3.91a2 2 0 0 1-1.66 0l-8.58-3.9" }
            }
        },
        "search" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                circle { cx: "11", cy: "11", r: "8" }
                path { d: "m21 21-4.3-4.3" }
            }
        },
        "columns" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                rect { width: "18", height: "18", x: "3", y: "3", rx: "2" }
                path { d: "M12 3v18" }
            }
        },
        "megaphone" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "m3 11 18-5v12L3 14v-3z" }
                path { d: "M11.6 16.8a3 3 0 1 1-5.8-1.6" }
            }
        },
        "bell" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9" }
                path { d: "M10.3 21a1.94 1.94 0 0 0 3.4 0" }
            }
        },
        "scroll-text" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M15 12h-5" }
                path { d: "M15 8h-5" }
                path { d: "M19 17V5a2 2 0 0 0-2-2H4" }
                path { d: "M8 21h12a2 2 0 0 0 2-2v-1a1 1 0 0 0-1-1H11a1 1 0 0 0-1 1v1a2 2 0 1 1-4 0V5a2 2 0 1 0-4 0v2a1 1 0 0 0 1 1h3" }
            }
        },
        "fingerprint" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M12 10a2 2 0 0 0-2 2c0 1.02-.1 2.51-.26 4" }
                path { d: "M14 13.12c0 2.38 0 6.38-1 8.88" }
                path { d: "M17.29 21.02c.12-.6.43-2.3.5-3.02" }
                path { d: "M2 12a10 10 0 0 1 18-6" }
                path { d: "M2 16h.01" }
                path { d: "M21.8 16c.2-2 .131-5.354 0-6" }
                path { d: "M5 19.5C5.5 18 6 15 6 12a6 6 0 0 1 .34-2" }
                path { d: "M8.65 22c.21-.66.45-1.32.57-2" }
                path { d: "M9 6.8a6 6 0 0 1 9 5.2v2" }
            }
        },
        "link" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" }
                path { d: "M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" }
            }
        },
        "heart-pulse" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M19 14c1.49-1.46 3-3.21 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.76 0-3 .5-4.5 2-1.5-1.5-2.74-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.29 1.51 4.04 3 5.5l7 7Z" }
                path { d: "M3.22 12H9.5l.5-1 2 4.5 2-7 1.5 3.5h5.27" }
            }
        },
        "file-lock" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" }
                path { d: "M14 2v4a2 2 0 0 0 2 2h4" }
                rect { width: "8", height: "5", x: "8", y: "12", rx: "1" }
                path { d: "M10 12V10a2 2 0 1 1 4 0v2" }
            }
        },
        "mail" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                rect { width: "20", height: "16", x: "2", y: "4", rx: "2" }
                path { d: "m22 7-8.97 5.7a1.94 1.94 0 0 1-2.06 0L2 7" }
            }
        },
        "settings" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" }
                circle { cx: "12", cy: "12", r: "3" }
            }
        },
        "plug" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M12 22v-5" }
                path { d: "M9 8V2" }
                path { d: "M15 8V2" }
                path { d: "M18 8v5a4 4 0 0 1-4 4h-4a4 4 0 0 1-4-4V8Z" }
            }
        },
        "send" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M22 2 11 13" }
                path { d: "m22 2-7 20-4-9-9-4Z" }
            }
        },
        "gamepad-2" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                line { x1: "6", x2: "10", y1: "11", y2: "11" }
                line { x1: "8", x2: "8", y1: "9", y2: "13" }
                line { x1: "15", x2: "15.01", y1: "12", y2: "12" }
                line { x1: "18", x2: "18.01", y1: "10", y2: "10" }
                path { d: "M17.32 5H6.68a4 4 0 0 0-3.978 3.59c-.006.052-.01.101-.017.152C2.604 9.416 2 14.456 2 16a3 3 0 0 0 3 3c1 0 1.5-.5 2-1l1.414-1.414A2 2 0 0 1 9.828 16h4.344a2 2 0 0 1 1.414.586L17 18c.5.5 1 1 2 1a3 3 0 0 0 3-3c0-1.545-.604-6.584-.685-7.258A4 4 0 0 0 17.32 5z" }
            }
        },
        "message-circle" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M7.9 20A9 9 0 1 0 4 16.1L2 22Z" }
            }
        },
        "briefcase" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                rect { width: "20", height: "14", x: "2", y: "7", rx: "2", ry: "2" }
                path { d: "M16 21V5a2 2 0 0 0-2-2h-4a2 2 0 0 0-2 2v16" }
            }
        },
        "terminal" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                polyline { points: "4 17 10 11 4 5" }
                line { x1: "12", x2: "20", y1: "19", y2: "19" }
            }
        },
        "wrench" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z" }
            }
        },
        "chevron-left" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "m15 18-6-6 6-6" }
            }
        },
        "chevron-right" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "m9 18 6-6-6-6" }
            }
        },
        "chevron-down" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "m6 9 6 6 6-6" }
            }
        },
        "copy" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                rect { width: "14", height: "14", x: "8", y: "8", rx: "2", ry: "2" }
                path { d: "M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2" }
            }
        },
        "webhook" => rsx! {
            svg {
                class: "{class}",
                xmlns: "http://www.w3.org/2000/svg",
                width: "24", height: "24",
                view_box: "0 0 24 24",
                fill: "none", stroke: "currentColor",
                stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                path { d: "M18 16.98h-5.99c-1.1 0-1.95.94-2.48 1.9A4 4 0 0 1 2 17c.01-.7.2-1.4.57-2" }
                path { d: "m6 17 3.13-5.78c.53-.97.1-2.18-.5-3.1a4 4 0 1 1 6.89-4.06" }
                path { d: "m12 6 3.13 5.73C15.66 12.7 16.9 13 18 13a4 4 0 0 1 0 8" }
            }
        },
        _ => rsx! {
            span { class: "{class}", "?" }
        },
    }
}
