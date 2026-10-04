//! Confirmation dialog for deactivating local Matrix accounts, with the
//! option to also erase their data. Shared by the user list and detail pages.

use dioxus::prelude::*;

use crate::api::users::AccountOwner;
use crate::components::ui::button::{Button, ButtonVariant};
use crate::components::ui::dialog::Modal;
use crate::components::ui::toast::{ToastVariant, show_toast};
use crate::utils::i18n::t;

/// Ask for confirmation before deactivating `user_ids`. `on_confirm` receives
/// whether the user's data should also be erased.
#[component]
pub fn DeactivateUserDialog(
    open: bool,
    user_ids: Vec<String>,
    on_confirm: EventHandler<bool>,
    on_cancel: EventHandler<()>,
) -> Element {
    let mut erase = use_signal(|| false);
    let has_pasion = crate::utils::storage::get_item("pasion_url").is_some();

    let target = match user_ids.as_slice() {
        [one] => one.clone(),
        many => format!("{} users", many.len()),
    };
    let pasion_note = if has_pasion {
        " Accounts with a Pasion account are deactivated in Pasion, which blocks sign-in and then deactivates them on the homeserver."
    } else {
        ""
    };

    rsx! {
        Modal {
            open,
            on_close: move |_| {
                erase.set(false);
                on_cancel.call(());
            },
            bg: "glass-panel".to_string(),
            div {
                role: "dialog",
                aria_modal: "true",
                div { class: "flex flex-col space-y-2 text-center sm:text-left",
                    h2 { class: "text-lg font-semibold", {t("users.deactivate_user")} }
                    p { class: "text-sm text-muted-foreground",
                        "Deactivate {target}? They are signed out everywhere, removed from their rooms and can no longer sign in. The user ID can never be registered again.{pasion_note}"
                    }
                }
                label { class: "flex items-center gap-2 text-sm mt-4",
                    input {
                        r#type: "checkbox",
                        class: "h-4 w-4 rounded border-gray-300",
                        checked: *erase.read(),
                        onchange: move |evt: FormEvent| erase.set(evt.checked()),
                    }
                    "Also erase their uploaded media (irreversible)"
                }
                div { class: "flex flex-col-reverse sm:flex-row sm:justify-end sm:space-x-2 mt-4",
                    Button {
                        variant: ButtonVariant::Outline,
                        autofocus: true,
                        onclick: move |_| {
                            erase.set(false);
                            on_cancel.call(());
                        },
                        {t("common.cancel")}
                    }
                    Button {
                        variant: ButtonVariant::Destructive,
                        onclick: move |_| {
                            let value = *erase.read();
                            erase.set(false);
                            on_confirm.call(value);
                        },
                        {t("users.deactivate")}
                    }
                }
            }
        }
    }
}

/// Toast for a finished single-account deactivation or reactivation.
pub fn show_account_change_toast(user_id: &str, deactivated: bool, owner: AccountOwner) {
    let message = match (deactivated, owner) {
        (true, AccountOwner::Pasion) => {
            format!("{user_id} deactivated in Pasion; the homeserver account follows shortly")
        }
        (true, AccountOwner::Homeserver) => format!("{user_id} deactivated"),
        (false, _) => format!("{user_id} reactivated"),
    };
    show_toast(&message, ToastVariant::Success);
}
