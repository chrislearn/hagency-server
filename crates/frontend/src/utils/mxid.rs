use std::sync::OnceLock;

use crate::utils::storage;

fn mxid_regex() -> &'static regex_lite::Regex {
    static RE: OnceLock<regex_lite::Regex> = OnceLock::new();
    RE.get_or_init(|| regex_lite::Regex::new(r"^@[^@:]+:[^\s/@?#]+$").expect("static mxid regex"))
}

pub fn is_mxid(id: &str) -> bool {
    mxid_regex().is_match(id)
}

pub fn return_mxid(input: &str) -> String {
    if is_mxid(input) {
        return input.to_string();
    }

    let home_server = storage::get_item("server_name").unwrap_or_else(|| {
        storage::get_item("user_id")
            .and_then(|id| id.split_once(':').map(|(_, server)| server.to_owned()))
            .unwrap_or_default()
    });

    let localpart = input.strip_prefix('@').unwrap_or(input);

    format!("@{localpart}:{home_server}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_matrix_ids_keep_ports_and_ipv6_servers() {
        for id in [
            "@admin:localhost:8088",
            "@user:example.test",
            "@user:[::1]:8088",
        ] {
            assert!(is_mxid(id));
            assert_eq!(return_mxid(id), id);
        }
        for id in [
            "admin",
            "@user",
            "@user:http://example.test",
            "@user:bad server",
        ] {
            assert!(!is_mxid(id));
        }
    }
}
