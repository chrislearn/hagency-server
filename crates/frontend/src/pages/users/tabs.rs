//! Tabs switching the Users page between this server's accounts and every
//! user the server has seen in its rooms.

use dioxus::prelude::*;

use crate::router::Route;
use crate::utils::i18n::t;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsersTab {
    Accounts,
    Known,
}

#[component]
pub fn UsersTabs(active: UsersTab) -> Element {
    let tabs = [
        (
            UsersTab::Accounts,
            t("users.tab_accounts"),
            Route::UserList {},
        ),
        (
            UsersTab::Known,
            t("users.tab_known"),
            Route::KnownUserList {},
        ),
    ];
    rsx! {
        div { class: "flex border-b",
            for (tab, label, route) in tabs {
                Link {
                    key: "{label}",
                    to: route,
                    class: if tab == active {
                        "px-4 py-2 text-sm font-medium border-b-2 border-primary text-primary"
                    } else {
                        "px-4 py-2 text-sm font-medium text-muted-foreground hover:text-foreground"
                    },
                    "{label}"
                }
            }
        }
    }
}
