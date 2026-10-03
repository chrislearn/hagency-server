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
