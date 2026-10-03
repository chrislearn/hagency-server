use hagency_server::config::Config;
#[test]
fn unified_config_and_signing_key_survive_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, include_str!("../config.example.toml")).unwrap();
    let mut conf = Config::load(&path).unwrap();
    conf.prepare_signing_key().unwrap();
    let key = conf.matrix.keypair.clone().unwrap();
    let mut reopened = Config::load(&path).unwrap();
    reopened.prepare_signing_key().unwrap();
    assert_eq!(
        reopened.matrix.keypair.as_ref().unwrap().document,
        key.document
    );
    assert_eq!(
        reopened.matrix.keypair.as_ref().unwrap().version,
        key.version
    );
    assert_eq!(conf.internal_origin().as_str(), "http://127.0.0.1:8088/");
}
#[test]
fn public_http_and_mistyped_host_configuration_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let original = include_str!("../config.example.toml");
    std::fs::write(
        &path,
        original.replace(
            "public_origin = \"http://127.0.0.1:8088\"",
            "public_origin = \"http://public.invalid\"",
        ),
    )
    .unwrap();
    assert!(Config::load(&path).is_err());
    std::fs::write(&path, format!("lissten = \"127.0.0.1:8088\"\n{original}")).unwrap();
    assert!(Config::load(&path).is_err());
}

#[test]
fn databases_are_distinct_and_required_before_initialization() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let original = include_str!("../config.example.toml");
    let conf = toml::from_str::<Config>(original).unwrap();
    assert!(conf.database_url.ends_with("/hagency"));
    assert!(conf.matrix.db.url.ends_with("/palpo"));
    assert!(
        conf.pasion
            .as_ref()
            .unwrap()
            .database_url
            .ends_with("/pasion")
    );
    for (from, to) in [
        ("palpo", "hagency"),
        ("pasion", "palpo"),
        ("pasion", "hagency"),
    ] {
        let shared = original.replace(
            &format!("postgres://hagency:hagency_dev@127.0.0.1:55438/{from}"),
            &format!("postgresql://hagency:hagency_dev@localhost:55438/{to}"),
        );
        std::fs::write(&path, shared).unwrap();
        let error = match Config::load(&path) {
            Ok(_) => panic!("shared databases were accepted"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("different database names"));
        assert!(!dir.path().join("data").exists());
    }
    let missing = original.replace(
        "database_url = \"postgres://hagency:hagency_dev@127.0.0.1:55438/hagency\"",
        "",
    );
    assert!(toml::from_str::<Config>(&missing).is_err());
    for url in ["sqlite://admin.db", "postgres://localhost"] {
        let invalid = original.replace(&conf.database_url, url);
        std::fs::write(&path, invalid).unwrap();
        assert!(Config::load(&path).is_err());
    }
}
