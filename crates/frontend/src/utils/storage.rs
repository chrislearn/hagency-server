use std::cell::RefCell;
use std::collections::HashMap;

use gloo_storage::{LocalStorage, SessionStorage, Storage};

thread_local! {
    static SESSION_SECRETS: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

fn session_only(key: &str) -> bool {
    matches!(
        key,
        "access_token" | "refresh_token" | "access_token_expires_at"
    )
}

pub fn get_item(key: &str) -> Option<String> {
    if session_only(key) {
        LocalStorage::delete(key);
        SessionStorage::delete(key);
        return SESSION_SECRETS.with(|values| values.borrow().get(key).cloned());
    }
    LocalStorage::get::<String>(key).ok()
}

pub fn set_item(key: &str, value: &str) {
    if session_only(key) {
        SESSION_SECRETS.with(|values| {
            values
                .borrow_mut()
                .insert(key.to_string(), value.to_string());
        });
        LocalStorage::delete(key);
        SessionStorage::delete(key);
    } else {
        let _ = LocalStorage::set(key, value.to_string());
    }
}

pub fn remove_item(key: &str) {
    LocalStorage::delete(key);
    SessionStorage::delete(key);
    SESSION_SECRETS.with(|values| {
        values.borrow_mut().remove(key);
    });
}

pub fn clear() {
    LocalStorage::clear();
    SessionStorage::clear();
    SESSION_SECRETS.with(|values| values.borrow_mut().clear());
}

// Nonsecret operation references are kept per tab for idempotent retries.
pub fn get_session_item(key: &str) -> Option<String> {
    SessionStorage::get(key).ok()
}
pub fn set_session_item(key: &str, value: &str) {
    let _ = SessionStorage::set(key, value);
}
pub fn remove_session_item(key: &str) {
    SessionStorage::delete(key);
}
