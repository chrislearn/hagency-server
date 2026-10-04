//! Pasion shares the process/listener, but never Palpo's database migrations.
use crate::config::Config;
use anyhow::Context;
use figment::{Figment, providers::Serialized};
use pasion_config::{AppConfig, ConfigurationSection, HttpResource};
use rand_core::SeedableRng;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub const MOUNT: &str = "/_pasion/";

#[derive(Clone)]
pub struct PasionConfig {
    pub database_url: String,
    pub resources_dir: PathBuf,
    pub delegate_matrix_auth: bool,
    pub settings: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmbeddingConfig {
    #[serde(default = "default_resources")]
    resources_dir: PathBuf,
    #[serde(default)]
    delegate_matrix_auth: bool,
}
fn default_resources() -> PathBuf {
    "../../resources/pasion".into()
}
impl PasionConfig {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path
            .as_ref()
            .canonicalize()
            .context("cannot open pasion_config")?;
        let base = path.parent().unwrap();
        let mut settings: Value = toml::from_str(&std::fs::read_to_string(&path)?)
            .context("invalid Pasion TOML configuration")?;
        let embedding = settings
            .as_object_mut()
            .context("Pasion configuration must be a table")?
            .remove("hagency")
            .unwrap_or_else(|| json!({}));
        let embedding: EmbeddingConfig = serde_json::from_value(embedding)
            .context("invalid Pasion [hagency] embedding options")?;
        let database_url = settings["database"]["uri"]
            .as_str()
            .context("Pasion configuration requires [database].uri")?
            .to_owned();
        crate::config::resolve_file_references(&mut settings, base);
        Ok(Self {
            database_url,
            resources_dir: crate::config::resolve(base, &embedding.resources_dir),
            delegate_matrix_auth: embedding.delegate_matrix_auth,
            settings,
        })
    }
    pub fn validate(&self, host: &Config) -> anyhow::Result<()> {
        let auth_db = crate::config::postgres_database(&self.database_url, "Pasion database.uri")?;
        let matrix_db = crate::config::postgres_database(&host.matrix.db.url, "matrix.db.url")?;
        let admin_db = crate::config::postgres_database(&host.database_url, "database_url")?;
        // Different hostname spellings may still refer to the same server.
        anyhow::ensure!(
            auth_db.path() != matrix_db.path() && auth_db.path() != admin_db.path(),
            "Hagency, Palpo and Pasion must use different database names"
        );
        let settings = self
            .settings
            .as_object()
            .context("Pasion configuration must be a table")?;
        for key in ["http", "matrix", "secrets", "templates", "storage"] {
            anyhow::ensure!(
                !settings.contains_key(key),
                "Pasion {key} is managed by hagency-server"
            );
        }
        anyhow::ensure!(
            !settings
                .get("policy")
                .is_some_and(|p| p.get("cedar_policy_file").is_some()),
            "Pasion policy files are managed through resources_dir"
        );
        anyhow::ensure!(
            !self.delegate_matrix_auth || host.account_config.is_none(),
            "legacy web-admin account approval cannot be combined with Pasion delegated registration"
        );
        Ok(())
    }
    pub fn resources(&self) -> anyhow::Result<Vec<HttpResource>> {
        let public = self.resources_dir.join("public");
        let public = camino::Utf8PathBuf::from_path_buf(public)
            .map_err(|_| anyhow::anyhow!("Pasion asset path must be UTF-8"))?;
        anyhow::ensure!(
            pasion_backend::server::discover_frontend_script(&public).is_some(),
            "Pasion frontend is missing; run just prepare-pasion"
        );
        Ok(vec![
            HttpResource::Discovery,
            HttpResource::Human,
            HttpResource::OAuth,
            HttpResource::RestApi {
                playground: false,
                undocumented_oauth2_access: false,
            },
            HttpResource::AdminApi,
            HttpResource::Health,
            HttpResource::Assets { path: public },
        ])
    }
}

/// Persist random OAuth signing/encryption keys before initializing either
/// component. URLs and the Matrix shared secret are wired from one host config.
pub async fn prepare(host: &mut Config) -> anyhow::Result<Option<Figment>> {
    let Some(conf) = host.pasion.clone() else {
        return Ok(None);
    };
    conf.resources()?;
    std::fs::create_dir_all(&host.data_dir)?;
    let path = host.data_dir.join("pasion-secrets.json");
    if !path.exists() {
        let mut rng = rand_chacha::ChaChaRng::from_entropy();
        let generated = pasion_config::RootConfig::generate(&mut rng).await?;
        let generated = serde_json::to_value(generated)?;
        write_private_once(
            &path,
            &json!({ "secrets": generated["secrets"], "matrix_secret": generated["matrix"]["secret"] }),
        )?;
    }
    let saved: Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    let secret = host.matrix.admin.mas_secret.clone().unwrap_or(
        saved["matrix_secret"]
            .as_str()
            .context("invalid persisted Pasion shared secret")?
            .to_owned(),
    );
    anyhow::ensure!(
        !secret.trim().is_empty(),
        "Matrix/Pasion shared secret must not be empty"
    );
    host.matrix.admin.mas_secret = Some(secret.clone());
    let public = host.public_origin.join(MOUNT)?;
    let internal = host.internal_origin().join(MOUNT)?;
    if conf.delegate_matrix_auth {
        anyhow::ensure!(
            host.matrix.delegated_auth.is_none(),
            "remove matrix.delegated_auth; pasion.delegate_matrix_auth manages it"
        );
        host.matrix.delegated_auth = Some(palpo::config::DelegatedAuthConfig {
            enable: true,
            issuer: Some(public.to_string()),
            introspection_endpoint: Some(internal.join("oauth2/introspect")?.to_string()),
            password_login_endpoint: Some(
                internal
                    .join("api/internal/matrix/password-login")?
                    .to_string(),
            ),
            account_management_url: Some(public.join("settings")?.to_string()),
            introspection_cache_ttl: 300,
            ..Default::default()
        });
    }
    let root = conf.resources_dir;
    let mut settings = conf.settings;
    // The host owns the listener, issuer, homeserver and key lifecycle.
    // Native database pool/TLS options stay in Pasion's own file.
    for (k,v) in json!({
        "http": {"public_base":public,"issuer":public,"listeners":[{"binds":[{"address":host.listen.to_string()}],"resources":[{"name":"assets","path":root.join("public")}]}],"trusted_proxies":[]},
        "matrix":{"kind":"palpo","homeserver":host.matrix.server_name,"endpoint":host.internal_origin(),"secret":secret},
        "secrets":saved["secrets"],
        "templates":{"path":root.join("templates"),"translations_path":root.join("translations")},
        "storage":{"backend":"fs","root":host.data_dir.join("pasion-media")}
    }).as_object().unwrap() { settings[k]=v.clone(); }
    if settings.get("policy").is_none() {
        settings["policy"] = json!({});
    }
    settings["policy"]["cedar_policy_file"] = json!(root.join("cedar/default.cedar"));
    let figment = Figment::new().merge(Serialized::defaults(settings));
    AppConfig::extract(&figment).map_err(anyhow::Error::from_boxed)?;
    Ok(Some(figment))
}

fn write_private_once(path: &Path, value: &Value) -> anyhow::Result<()> {
    use std::io::Write;
    let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| -> anyhow::Result<()> {
        let mut file = options.open(&tmp)?;
        file.write_all(&serde_json::to_vec(value)?)?;
        file.sync_all()?;
        match std::fs::hard_link(&tmp, path) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e.into()),
        }
        Ok(())
    })();
    let _ = std::fs::remove_file(tmp);
    result
}
