use serde::Deserialize;
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
};
use url::Url;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub listen: SocketAddr,
    pub public_origin: Url,
    #[serde(default)]
    pub public_dir: Option<PathBuf>,
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    #[serde(default)]
    pub callback_origins: Vec<Url>,
    #[serde(default)]
    pub session_ttl_ms: Option<u64>,
    #[serde(default)]
    pub read_timeout_ms: Option<u64>,
    #[serde(default)]
    pub queue: QueueConfig,
    #[serde(default)]
    pub account_config: Option<PathBuf>,
    #[serde(default)]
    pub retirement_admin_token_file: Option<PathBuf>,
    pub matrix: palpo::config::ServerConfig,
}
#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct QueueConfig {
    pub max_pending: usize,
    pub max_records: usize,
    pub max_bytes: usize,
    pub lease_ms: i64,
}
impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_pending: 1000,
            max_records: 10000,
            max_bytes: 16 * 1024 * 1024,
            lease_ms: 30000,
        }
    }
}
fn default_data_dir() -> PathBuf {
    PathBuf::from("data")
}
impl Config {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let canonical_path = path.as_ref().canonicalize()?;
        let path = canonical_path.as_path();
        let mut conf: Self = toml::from_str(&std::fs::read_to_string(path)?)?;
        let base = path.parent().unwrap_or(Path::new("."));
        for file in [
            &mut conf.public_dir,
            &mut conf.account_config,
            &mut conf.retirement_admin_token_file,
        ] {
            if let Some(p) = file
                && p.is_relative()
            {
                *p = base.join(&*p);
            }
        }
        if conf.data_dir.is_relative() {
            conf.data_dir = base.join(&conf.data_dir);
        }
        if let palpo::config::StorageConfig::Fs { root } = &mut conf.matrix.storage
            && Path::new(root).is_relative()
        {
            *root = base.join(&*root).to_string_lossy().into_owned();
        }
        conf.validate()?;
        Ok(conf)
    }
    pub fn prepare_signing_key(&mut self) -> anyhow::Result<()> {
        use base64::Engine;
        if self.matrix.keypair.is_some() {
            return Ok(());
        }
        std::fs::create_dir_all(&self.data_dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.data_dir, std::fs::Permissions::from_mode(0o700))?;
        }
        let path = self.data_dir.join("matrix-signing-key.json");
        let pair = if path.exists() {
            serde_json::from_slice::<serde_json::Value>(&std::fs::read(&path)?)?
        } else {
            let der = palpo::core::signatures::Ed25519KeyPair::generate()?;
            let pair = serde_json::json!({"document":base64::engine::general_purpose::STANDARD.encode(der),"version":uuid::Uuid::new_v4().simple().to_string()[..8].to_string()});
            let tmp = self
                .data_dir
                .join(format!(".signing-key-{}", uuid::Uuid::new_v4()));
            let mut opts = std::fs::OpenOptions::new();
            opts.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            use std::io::Write;
            let mut file = opts.open(&tmp)?;
            file.write_all(&serde_json::to_vec(&pair)?)?;
            file.sync_all()?;
            // An atomic hard link avoids replacing a key created by a concurrent startup.
            match std::fs::hard_link(&tmp, &path) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(e) => return Err(e.into()),
            }
            std::fs::remove_file(tmp)?;
            serde_json::from_slice::<serde_json::Value>(&std::fs::read(&path)?)?
        };
        let document = pair["document"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("invalid signing key document"))?
            .to_string();
        let version = pair["version"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("invalid signing key version"))?
            .to_string();
        let der = base64::engine::general_purpose::STANDARD.decode(&document)?;
        palpo::core::signatures::Ed25519KeyPair::from_der(&der, version.clone())?;
        self.matrix.keypair = Some(palpo::config::KeypairConfig { document, version });
        Ok(())
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        origin(&self.public_origin, true)?;
        for url in &self.callback_origins {
            origin(url, false)?;
        }
        anyhow::ensure!(
            self.queue.max_pending > 0
                && self.queue.max_records > 0
                && self.queue.max_bytes > 0
                && self.queue.lease_ms > 0,
            "queue limits must be positive"
        );
        anyhow::ensure!(
            self.session_ttl_ms.unwrap_or(1800000) > 0 && self.read_timeout_ms.unwrap_or(8000) > 0,
            "timeouts must be positive"
        );
        anyhow::ensure!(
            !self.listen.ip().is_unspecified()
                || !self.public_origin.host_str().unwrap_or("").is_empty(),
            "public origin is required"
        );
        anyhow::ensure!(
            !self.matrix.admin.console_automatic,
            "embedded Palpo does not support an automatic interactive console"
        );
        self.matrix.check()?;
        Ok(())
    }
    pub fn internal_origin(&self) -> Url {
        let ip = if self.listen.ip().is_unspecified() {
            if self.listen.is_ipv6() {
                "[::1]".into()
            } else {
                "127.0.0.1".into()
            }
        } else if self.listen.is_ipv6() {
            format!("[{}]", self.listen.ip())
        } else {
            self.listen.ip().to_string()
        };
        Url::parse(&format!("http://{ip}:{}/", self.listen.port())).unwrap()
    }
}
pub fn origin(url: &Url, public: bool) -> anyhow::Result<()> {
    anyhow::ensure!(
        ["http", "https"].contains(&url.scheme())
            && url.username().is_empty()
            && url.password().is_none()
            && url.path() == "/"
            && url.query().is_none()
            && url.fragment().is_none(),
        "use a credential-free HTTP(S) origin"
    );
    anyhow::ensure!(
        !public
            || url.scheme() == "https"
            || matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")),
        "public origins require HTTPS"
    );
    Ok(())
}
