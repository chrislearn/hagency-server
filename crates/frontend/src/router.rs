use dioxus::prelude::*;

use crate::api::auth;
use crate::components::layout::AppLayout;
use crate::pages;
use crate::utils::i18n::t;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[route("/login")]
    LoginPage {},

    #[route("/oauth/callback?:code&:state&:error&:error_description")]
    OAuthCallback {
        code: Option<String>,
        state: Option<String>,
        error: Option<String>,
        error_description: Option<String>,
    },

    #[layout(AuthenticatedLayout)]
        #[route("/")]
        Dashboard {},

        #[route("/hagency/projects")]
        HagencyProjects {},
        #[route("/hagency/agents")]
        HagencyAgents {},

        #[route("/users")]
        UserList {},
        #[route("/users/create")]
        UserCreate {},
        #[route("/users/known")]
        KnownUserList {},
        #[route("/users/known/:user_id")]
        KnownUserShow { user_id: String },
        #[route("/users/:user_id")]
        UserShow { user_id: String },

        #[route("/rooms")]
        RoomList {},
        #[route("/rooms/:room_id")]
        RoomShow { room_id: String },

        #[route("/media")]
        MediaList {},

        #[route("/reports")]
        ReportList {},
        #[route("/reports/:report_id")]
        ReportShow { report_id: String },

        #[route("/destinations")]
        DestinationList {},
        #[route("/destinations/:destination_id")]
        DestinationShow { destination_id: String },

        #[route("/registration-tokens")]
        RegistrationTokenList {},

        #[route("/appservices")]
        AppserviceList {},

        #[route("/auth-status")]
        AuthStatus {},

        #[route("/server-status")]
        ServerStatus {},

        #[route("/server-actions")]
        ServerActions {},

        #[route("/server-notices")]
        ServerNotices {},

        #[route("/server-notifications")]
        ServerNotifications {},

        #[route("/settings/notifications")]
        NotificationPreferences {},

        #[route("/pasion/accounts")]
        PasionAccounts {},
        #[route("/pasion/accounts/:user_id")]
        PasionAccountShow { user_id: String },
        #[route("/pasion/audit-log")]
        PasionAuditLog {},
        #[route("/pasion/oauth2-sessions")]
        PasionOAuth2Sessions {},
        #[route("/pasion/personal-sessions")]
        PasionPersonalSessions {},
        #[route("/pasion/connector-health")]
        PasionConnectorHealth {},
        #[route("/pasion/upstream-providers")]
        PasionUpstreamProviders {},
        #[route("/pasion/upstream-links")]
        PasionUpstreamLinks {},
        #[route("/pasion/notification-channels")]
        PasionNotificationChannels {},
        #[route("/pasion/notification-templates")]
        PasionNotificationTemplates {},
    #[end_layout]

    #[route("/:..route")]
    NotFound { route: Vec<String> },
}

#[component]
pub fn AppRouter() -> Element {
    rsx! {
        Router::<Route> {}
    }
}

/// Session-scoped admin verdict, shared across the whole router.
///
///   Some(true)  = the servers accepted this user as an administrator
///   Some(false) = verified non-admin (forbidden)
///   None        = not yet probed (or the probe failed)
///
/// `verify_admin()` is an HTTP round-trip, so in-session navigation reuses
/// this verdict. It only ever comes from a successful server probe: nothing
/// persisted client side can grant access, and a failed probe never counts
/// as admin. It is cleared on logout and after a 401 (see
/// `reset_admin_cache`), and flipped to `false` when a later 403 turns out
/// to mean the admin flag was revoked (see `auth::handle_forbidden`).
static ADMIN_VERDICT: GlobalSignal<Option<bool>> = GlobalSignal::new(|| None);

/// Last admin-probe failure, shown with a retry button instead of letting
/// the user in.
static ADMIN_PROBE_ERROR: GlobalSignal<Option<String>> = GlobalSignal::new(|| None);

/// Clear the cached admin verdict so the next navigation re-probes. Call this
/// when the session is invalidated (e.g. after a 401 or on logout).
pub fn reset_admin_cache() {
    *ADMIN_VERDICT.write() = None;
    *ADMIN_PROBE_ERROR.write() = None;
}

/// Record a verdict obtained from a server probe.
pub fn set_admin_verdict(is_admin: bool) {
    *ADMIN_VERDICT.write() = Some(is_admin);
    *ADMIN_PROBE_ERROR.write() = None;
}

pub fn is_server_admin() -> bool {
    *ADMIN_VERDICT.read() == Some(true)
}

#[component]
fn AuthenticatedLayout() -> Element {
    let nav = use_navigator();
    let route = use_route::<Route>();

    // Re-check periodically too: a revoked admin who only uses pages that
    // keep answering (or sits idle) must not keep the dashboard open.
    use_future(|| async {
        loop {
            gloo_timers::future::sleep(std::time::Duration::from_secs(60)).await;
            if *ADMIN_VERDICT.peek() == Some(true) {
                auth::handle_forbidden();
            }
        }
    });

    let router = dioxus::prelude::router();
    let mut probe = use_resource(move || {
        let route = router.current::<Route>();
        async move {
            // A reload retains only the HttpOnly session, never an OAuth bearer.
            // Revalidate that session before rendering owner management pages.
            *ADMIN_PROBE_ERROR.write() = None;
            match auth::verify_admin().await {
                Ok(flag) => {
                    set_admin_verdict(flag);
                    if flag
                        && !auth::has_oauth_token()
                        && !matches!(route, Route::HagencyProjects {} | Route::HagencyAgents {})
                        && let Err(err) = auth::start_login().await
                    {
                        *ADMIN_PROBE_ERROR.write() = Some(err.message);
                    }
                }
                Err(err) if err.status == 401 => {
                    crate::api::browser_auth::clear_session();
                    nav.replace(Route::LoginPage {});
                }
                // Never fall back to "admin": without a positive answer from the
                // servers the dashboard stays closed.
                Err(err) => *ADMIN_PROBE_ERROR.write() = Some(err.message),
            }
        }
    });

    let verdict = *ADMIN_VERDICT.read();
    let probe_error = ADMIN_PROBE_ERROR.read().clone();
    let member_page = matches!(route, Route::HagencyProjects {} | Route::HagencyAgents {});
    if verdict == Some(true) && !member_page && !auth::has_oauth_token() {
        return rsx! { div { class: "p-6", "Restoring Pasion authorization..."
            if let Some(message) = probe_error { p { role: "alert", "{message}" }
                button { onclick: move |_| probe.restart(), "Retry" }
            }
        } };
    }
    match (verdict, probe_error) {
        (Some(true), _) => rsx! {
            AppLayout {
                Outlet::<Route> {}
            }
        },
        (Some(false), _) => {
            let member_page = matches!(route, Route::HagencyProjects {} | Route::HagencyAgents {});
            if matches!(route, Route::Dashboard {}) {
                nav.replace(Route::HagencyProjects {});
            }
            rsx! { AppLayout {
                if member_page { Outlet::<Route> {} } else { pages::not_authorized::NotAuthorizedPage {} }
            } }
        }
        (None, Some(message)) => rsx! {
            div { class: "flex min-h-screen items-center justify-center bg-background p-4",
                div { class: "w-full max-w-md space-y-4 text-center",
                    h1 { class: "text-2xl font-bold tracking-tight", "Unable to verify administrator access" }
                    p { class: "text-muted-foreground text-sm break-all", "{message}" }
                    crate::components::ui::button::Button {
                        class: "w-full".to_string(),
                        onclick: move |_| probe.restart(),
                        "Retry"
                    }
                    crate::components::ui::button::Button {
                        variant: crate::components::ui::button::ButtonVariant::Outline,
                        class: "w-full".to_string(),
                        onclick: move |_| {
                            spawn(async move {
                                let _ = auth::logout().await;
                                nav.replace(Route::LoginPage {});
                            });
                        },
                        {crate::utils::i18n::t("auth.sign_out")}
                    }
                }
            }
        },
        (None, None) => rsx! {
            div { class: "flex min-h-screen items-center justify-center",
                crate::components::ui::loading::Spinner { class: String::new() }
            }
        },
    }
}

#[component]
fn LoginPage() -> Element {
    rsx! {
        pages::login::LoginPage {}
    }
}

#[component]
fn HagencyProjects() -> Element {
    rsx! { pages::hagency::projects::Projects {} }
}
#[component]
fn HagencyAgents() -> Element {
    rsx! { pages::hagency::agents::Agents {} }
}

#[component]
fn OAuthCallback(
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
) -> Element {
    rsx! {
        pages::oauth_callback::OAuthCallback {
            code: code,
            state: state,
            error: error,
            error_description: error_description,
        }
    }
}

#[component]
fn Dashboard() -> Element {
    rsx! {
        pages::dashboard::Dashboard {}
    }
}

#[component]
fn UserList() -> Element {
    rsx! {
        pages::users::list::UserList {}
    }
}

#[component]
fn UserCreate() -> Element {
    if crate::utils::storage::get_item("oauth_enabled").as_deref() == Some("true") {
        return rsx! { div { class:"hg-page",
            h1 { "Create a server account" }
            p { "Pasion manages accounts, passwords and administrator roles for this server." }
            Link { to:Route::PasionAccounts {},class:"hg-link","Create an account in Account management" }
        } };
    }
    rsx! {
        pages::users::create::UserCreate {}
    }
}

#[component]
fn UserShow(user_id: String) -> Element {
    rsx! {
        pages::users::show::UserShow { user_id }
    }
}

#[component]
fn KnownUserList() -> Element {
    rsx! {
        pages::users::known::KnownUserList {}
    }
}

#[component]
fn KnownUserShow(user_id: String) -> Element {
    rsx! {
        pages::users::known_show::KnownUserShow { user_id }
    }
}

#[component]
fn RoomList() -> Element {
    rsx! {
        pages::rooms::list::RoomList {}
    }
}

#[component]
fn RoomShow(room_id: String) -> Element {
    rsx! {
        pages::rooms::show::RoomShow { room_id }
    }
}

#[component]
fn MediaList() -> Element {
    rsx! {
        pages::media::MediaList {}
    }
}

#[component]
fn ReportList() -> Element {
    rsx! {
        pages::reports::ReportList {}
    }
}

#[component]
fn ReportShow(report_id: String) -> Element {
    rsx! {
        pages::reports::ReportShow { report_id }
    }
}

#[component]
fn DestinationList() -> Element {
    rsx! {
        pages::destinations::DestinationList {}
    }
}

#[component]
fn DestinationShow(destination_id: String) -> Element {
    rsx! {
        pages::destinations::DestinationShow { destination_id }
    }
}

#[component]
fn RegistrationTokenList() -> Element {
    rsx! {
        pages::registration_tokens::RegistrationTokenList {}
    }
}

#[component]
fn AppserviceList() -> Element {
    rsx! {
        pages::appservices::AppserviceList {}
    }
}

#[component]
fn AuthStatus() -> Element {
    rsx! {
        pages::auth_status::AuthStatusPage {}
    }
}

#[component]
fn ServerStatus() -> Element {
    rsx! {
        pages::server_status::ServerStatus {}
    }
}

#[component]
fn ServerActions() -> Element {
    rsx! {
        pages::server_actions::ServerActions {}
    }
}

#[component]
fn ServerNotices() -> Element {
    rsx! {
        pages::server_notices::ServerNotices {}
    }
}

#[component]
fn ServerNotifications() -> Element {
    rsx! {
        pages::server_notifications::ServerNotifications {}
    }
}

#[component]
fn NotificationPreferences() -> Element {
    rsx! {
        pages::notification_preferences::NotificationPreferencesPage {}
    }
}

#[component]
fn PasionAccounts() -> Element {
    rsx! { pages::pasion::accounts::PasionAccountsPage {} }
}

#[component]
fn PasionAccountShow(user_id: String) -> Element {
    rsx! { pages::pasion::account_show::PasionAccountShowPage { user_id } }
}

#[component]
fn PasionAuditLog() -> Element {
    rsx! { pages::pasion::audit_log::AuditLogPage {} }
}

#[component]
fn PasionOAuth2Sessions() -> Element {
    rsx! { pages::pasion::oauth2_sessions::OAuth2SessionsPage {} }
}

#[component]
fn PasionPersonalSessions() -> Element {
    rsx! { pages::pasion::personal_sessions::PersonalSessionsPage {} }
}

#[component]
fn PasionConnectorHealth() -> Element {
    rsx! { pages::pasion::connector_health::ConnectorHealthPage {} }
}

#[component]
fn PasionUpstreamProviders() -> Element {
    rsx! { pages::pasion::upstream_providers::UpstreamProvidersPage {} }
}

#[component]
fn PasionUpstreamLinks() -> Element {
    rsx! { pages::pasion::upstream_links::UpstreamLinksPage {} }
}

#[component]
fn PasionNotificationChannels() -> Element {
    rsx! { pages::pasion::notification_channels::NotificationChannelsPage {} }
}

#[component]
fn PasionNotificationTemplates() -> Element {
    rsx! { pages::pasion::notification_templates::NotificationTemplatesPage {} }
}

#[component]
fn NotFound(route: Vec<String>) -> Element {
    rsx! {
        div { class: "flex items-center justify-center min-h-screen",
            div { class: "text-center",
                h1 { class: "text-4xl font-bold mb-4", {t("not_found.title")} }
                p { class: "text-muted-foreground mb-4",
                    {format!("{}: /{}", t("not_found.message"), route.join("/"))}
                }
                Link { to: Route::Dashboard {}, class: "text-primary hover:underline",
                    {t("not_found.go_dashboard")}
                }
            }
        }
    }
}
