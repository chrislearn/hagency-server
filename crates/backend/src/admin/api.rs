use super::*;
use crate::config::{Config, QueueConfig};
use salvo::{http::header, prelude::*};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::Mutex;
use url::Url;

#[derive(Clone)]
pub struct Admin {
    pub(crate) store: Arc<Store>,
    pub(crate) palpo: Upstream,
    pub(crate) operations: Arc<hagency_operations::api::App>,
    pub(crate) server_name: String,
    pub(crate) public_origin: Url,
    pub(crate) callback_origins: Vec<String>,
    pub(crate) queue: QueueConfig,
    pub(crate) retirement_token: Option<String>,
    pub(crate) account_config: Option<Value>,
    pub(crate) accounts_ready: Arc<std::sync::atomic::AtomicBool>,
    pub(crate) account_error: Arc<Mutex<Option<String>>>,
    sessions: Arc<Mutex<HashMap<String, Session>>>,
    rates: Arc<Mutex<HashMap<String, (u32, i64)>>>,
    pub(crate) mutations: Arc<Mutex<()>>,
    session_ttl: i64,
    read_timeout: Duration,
    oauth_revocation: Option<(Url, String)>,
}
#[derive(Clone)]
struct Session {
    token: String,
    user_id: String,
    csrf: String,
    is_admin: bool,
    expires: i64,
}
impl Admin {
    pub async fn new(conf: &Config, store: Arc<Store>) -> anyhow::Result<Self> {
        let account_config = match &conf.account_config {
            Some(p) => Some(serde_json::from_str::<Value>(&std::fs::read_to_string(p)?)?),
            None => None,
        };
        let retirement_token = match &conf.retirement_admin_token_file {
            Some(p) => Some(std::fs::read_to_string(p)?.trim().into()),
            None => account_config
                .as_ref()
                .and_then(|c| c["adminToken"].as_str().map(str::to_string)),
        };
        let operations = hagency_operations::api::App::new(
            hagency_operations::matrix::Matrix::new(
                conf.internal_origin().as_str(),
                conf.matrix.server_name.to_string().try_into()?,
            )?,
            store.shared.clone(),
            conf.public_origin.as_str(),
            900000,
        )
        .await?
        .with_transport(conf.public_origin.as_str(), conf.internal_origin().as_str())?
        .with_limits(hagency_operations::outbound::Limits {
            lease_ms: conf.queue.lease_ms as u64,
            records: conf.queue.max_records as u64,
            pending: conf.queue.max_pending as u64,
            bytes: conf.queue.max_bytes as u64,
        })?;
        let this = Self {
            store,
            operations,
            palpo: Upstream::new(conf.internal_origin()),
            server_name: conf.matrix.server_name.to_string(),
            public_origin: conf.public_origin.clone(),
            callback_origins: conf
                .callback_origins
                .iter()
                .map(|u| u.origin().ascii_serialization())
                .collect(),
            queue: conf.queue.clone(),
            retirement_token,
            account_config,
            accounts_ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            account_error: Arc::new(Mutex::new(None)),
            sessions: Default::default(),
            rates: Default::default(),
            mutations: Default::default(),
            session_ttl: conf.session_ttl_ms.unwrap_or(1800000) as i64,
            read_timeout: Duration::from_millis(conf.read_timeout_ms.unwrap_or(8000)),
            oauth_revocation: if conf.pasion.as_ref().is_some_and(|p| p.delegate_matrix_auth) {
                Some((
                    conf.internal_origin().join("_pasion/oauth2/revoke")?,
                    conf.matrix
                        .admin
                        .mas_secret
                        .clone()
                        .ok_or_else(|| anyhow::anyhow!("missing Pasion shared secret"))?,
                ))
            } else {
                None
            },
        };
        let binding = json!({"serverName":this.server_name,"palpoOrigin":this.palpo.url.origin().ascii_serialization()});
        this.store
            .change(|st| {
                if st["serverBinding"].is_object() && st["serverBinding"] != binding {
                    return Err(err(
                        409,
                        "server_binding_conflict",
                        "Admin storage belongs to a different server.",
                    ));
                }
                st["serverBinding"] = binding;
                Ok(())
            })
            .await?;
        this.account_setup().await?;
        Ok(this)
    }
    pub fn start_notifications(
        &self,
        conf: &Config,
    ) -> anyhow::Result<Option<tokio::task::JoinHandle<()>>> {
        let Some(n) = &conf.action_notifications else {
            return Ok(None);
        };
        let worker = hagency_operations::notifications::Notifications::new(
            self.operations.clone(),
            n.bot_mxid.clone(),
            std::fs::read_to_string(&n.token_file)?.trim().to_owned(),
            conf.public_origin.to_string(),
        )?;
        Ok(Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(15));
            loop {
                interval.tick().await;
                if let Err(error) = worker.tick().await {
                    tracing::warn!(code=%error.code,"action notification delivery deferred");
                }
            }
        })))
    }
    pub fn router(&self) -> Router {
        Router::new()
            .push(hagency_operations::api::miniapp_router(
                self.operations.clone(),
            ))
            .push(Router::with_path("api/{**path}").goal(self.clone()))
    }
    pub fn with_upstream(mut self, url: Url) -> anyhow::Result<Self> {
        let operations = Arc::get_mut(&mut self.operations)
            .ok_or_else(|| anyhow::anyhow!("configure the upstream before cloning Admin"))?;
        operations.matrix = hagency_operations::matrix::Matrix::new(
            url.as_str(),
            self.server_name.clone().try_into()?,
        )?;
        self.palpo = Upstream::new(url);
        Ok(self)
    }
    async fn rate(&self, key: String, max: u32, window: i64) -> Result<()> {
        let mut rates = self.rates.lock().await;
        rates.retain(|_, (_, until)| *until > millis());
        let rate = rates.entry(key).or_insert((0, millis() + window));
        rate.0 += 1;
        if rate.0 > max {
            Err(err(
                429,
                "rate_limit",
                "Too many attempts. Try again later.",
            ))
        } else {
            Ok(())
        }
    }
    fn cookie(&self, id: &str, clear: bool) -> String {
        format!(
            "palpo_admin={id}; Path=/; HttpOnly; SameSite=Strict{}; Max-Age={}",
            if self.public_origin.scheme() == "https" {
                "; Secure"
            } else {
                ""
            },
            if clear { 0 } else { self.session_ttl / 1000 }
        )
    }
    async fn browser_session(
        &self,
        req: &Request,
        res: &mut Response,
        token: String,
        user: String,
        is_admin: bool,
    ) -> Result<Value> {
        let id = secret();
        let csrf = secret();
        let previous = req
            .headers()
            .get(header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .and_then(|cookies| {
                cookies
                    .split(';')
                    .find_map(|p| p.trim().strip_prefix("palpo_admin="))
            });
        let old = if let Some(previous) = previous {
            self.sessions.lock().await.get(previous).cloned()
        } else {
            None
        };
        if let (Some((endpoint, shared_secret)), Some(old)) = (&self.oauth_revocation, old)
            && old.token != token
        {
            // Revoke only after the replacement token is fully verified. Calling
            // Matrix logout here would delete a device shared by both grants.
            let revoked = reqwest::Client::new()
                .post(endpoint.clone())
                .timeout(Duration::from_secs(10))
                .bearer_auth(shared_secret)
                .form(&[
                    ("token", old.token),
                    ("token_type_hint", "access_token".into()),
                ])
                .send()
                .await
                .map_err(|_| {
                    err(
                        502,
                        "authorization_handover_failed",
                        "Could not replace your previous authorization. Retry sign-in.",
                    )
                })?;
            if !revoked.status().is_success() {
                return Err(err(
                    502,
                    "authorization_handover_failed",
                    "Could not replace your previous authorization. Retry sign-in.",
                ));
            }
        }
        let mut sessions = self.sessions.lock().await;
        sessions.retain(|_, s| s.expires > millis());
        // Replace the cookie session instead of leaving a previous identity active.
        if let Some(previous) = previous {
            sessions.remove(previous);
        }
        sessions.insert(
            id.clone(),
            Session {
                token,
                user_id: user.clone(),
                csrf: csrf.clone(),
                is_admin,
                expires: millis() + self.session_ttl,
            },
        );
        drop(sessions);
        res.add_header(header::SET_COOKIE, self.cookie(&id, false), true)
            .unwrap();
        Ok(
            json!({"userId":user,"csrf":csrf,"isAdmin":is_admin,"serverName":self.server_name,
            "callbackOrigins":if is_admin { self.callback_origins.clone() } else { vec![] },"outboundAvailable":true}),
        )
    }
    async fn body(req: &mut Request, limit: usize) -> Result<Value> {
        if req
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .split(';')
            .next()
            != Some("application/json")
        {
            return Err(err(415, "json_required", "Use application/json."));
        }
        req.set_secure_max_size(limit);
        let raw = req.payload().await.map_err(|_| {
            err(
                413,
                "body_too_large",
                "Request body exceeds the endpoint limit.",
            )
        })?;
        let value: Value = serde_json::from_slice(raw)
            .map_err(|_| err(400, "invalid_json", "A JSON object is required."))?;
        if !value.is_object() {
            return Err(err(400, "invalid_json", "A JSON object is required."));
        }
        Ok(value)
    }
    async fn route(&self, req: &mut Request, res: &mut Response) -> Result<(u16, Value)> {
        let path = req.uri().path().to_string();
        let method = req.method().as_str().to_string();
        let host = req
            .headers()
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let origin = req
            .headers()
            .get(header::ORIGIN)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let bearer = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .unwrap_or("")
            .to_string();
        let machine =
            regex::Regex::new(r"^/api/fleet/v2/(hf_[a-f0-9]{32})/(poll|ack|updates|retire-agent)$")
                .unwrap();
        let relay=regex::Regex::new(r"^/api/relay/v2/(hf_[a-f0-9]{32})/(?:_matrix/app/v1/)?(transactions|users|rooms)/([^/]+)$").unwrap();
        let m = machine.captures(&path);
        let r = relay.captures(&path);
        if m.is_some() || r.is_some() {
            let caps = m.as_ref().or(r.as_ref()).unwrap();
            let id = caps.get(1).unwrap().as_str();
            let action = caps.get(2).unwrap().as_str();
            let expected = if m.is_some() {
                self.public_origin.origin().ascii_serialization()
            } else {
                self.palpo.url.origin().ascii_serialization()
            };
            let expected = Url::parse(&expected).unwrap();
            if host != host_port(&expected) || origin.is_some() {
                return Err(err(
                    403,
                    "host_forbidden",
                    "Machine endpoints require their fixed origin and no browser Origin.",
                ));
            }
            let token = if bearer.is_empty() && r.is_some() {
                req.query::<String>("access_token").unwrap_or_default()
            } else {
                bearer
            };
            let generation = req
                .headers()
                .get("x-hagency-generation")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            let f = self
                .authenticate(id, &token, &generation, r.is_some())
                .await?;
            if m.is_some() && action == "poll" && method == "GET" {
                return Ok((
                    200,
                    self.poll(
                        id,
                        &token,
                        &generation,
                        &req.query::<String>("lane").unwrap_or_default(),
                        &req.query::<String>("consumer").unwrap_or_default(),
                        &req.query::<String>("wait").unwrap_or("25000".into()),
                    )
                    .await?,
                ));
            }
            if m.is_some()
                && method == "POST"
                && ["ack", "updates", "retire-agent"].contains(&action)
            {
                let input = Self::body(
                    req,
                    if action == "updates" {
                        1024 * 1024
                    } else {
                        16384
                    },
                )
                .await?;
                if action == "ack" {
                    return Ok((200, self.ack(id, &token, &generation, &input).await?));
                }
                let _guard = self.mutations.lock().await;
                self.authenticate(id, &token, &generation, false).await?;
                return Ok((
                    200,
                    if action == "updates" {
                        self.updates(id, &token, &generation, &input).await?
                    } else {
                        self.retire_allocated(id, &input).await?
                    },
                ));
            }
            if r.is_some() && action == "transactions" && method == "PUT" {
                let input = Self::body(req, 1024 * 1024).await?;
                let tid = decode(caps.get(3).unwrap().as_str())?;
                return Ok((200, self.transaction(id, &token, &tid, &input).await?));
            }
            if r.is_some() && method == "GET" && ["users", "rooms"].contains(&action) {
                let identity = decode(caps.get(3).unwrap().as_str())?;
                let known = action == "users"
                    && self.in_namespace(&f, &identity)
                    && (f["representativeMxid"] == identity
                        || object_values(&f["agents"])
                            .iter()
                            .any(|a| a["mxid"] == identity && a["state"] == "registered"));
                return Ok((
                    if known { 200 } else { 404 },
                    if known {
                        json!({})
                    } else {
                        json!({"errcode":"M_NOT_FOUND","error":"No registered identity exists."})
                    },
                ));
            }
            return Err(err(
                405,
                "method_not_allowed",
                "Unsupported machine operation.",
            ));
        }
        if host != host_port(&self.public_origin) {
            return Err(err(
                403,
                "host_forbidden",
                "Unexpected admin application host.",
            ));
        }
        let mutation = !["GET", "HEAD"].contains(&method.as_str());
        let pair = regex::Regex::new(r"^/api/pair/(hf_[a-f0-9]{32})$").unwrap();
        let pair = pair.captures(&path);
        if mutation
            && pair.is_none()
            && origin.as_deref() != Some(&self.public_origin.origin().ascii_serialization())
        {
            return Err(err(
                403,
                "origin_forbidden",
                "A same-origin request is required.",
            ));
        }
        if path == "/api/account-access" && method == "GET" {
            return Ok((200, self.account_public()));
        }
        if ["/api/account-requests", "/api/account-requests/status"].contains(&path.as_str())
            && method == "POST"
        {
            let input = Self::body(req, 16384).await?;
            let status = path.ends_with("/status");
            self.rate(
                format!("account:{}:{status}", address(req)),
                if status { 180 } else { 30 },
                if status { 60000 } else { 3600000 },
            )
            .await?;
            return Ok((
                if status { 200 } else { 202 },
                json!({"request":if status{self.account_status(&input).await?}else{self.account_submit(&input).await?}}),
            ));
        }
        if path == "/api/login/token" && method == "POST" {
            self.rate(format!("token-login:{}", address(req)), 120, 900000)
                .await?;
            Self::body(req, 16384).await?;
            if bearer.is_empty() || bearer.len() > 16384 {
                return Err(err(
                    401,
                    "sign_in_required",
                    "A valid Matrix access token is required.",
                ));
            }
            let me = self
                .palpo
                .get("/_matrix/client/v3/account/whoami", &bearer)
                .await?;
            if me["is_guest"] == true {
                return Err(err(
                    403,
                    "identity_mismatch",
                    "Guest accounts cannot manage projects.",
                ));
            }
            let user = self.owner(&me["user_id"])?;
            let is_admin = match self.palpo.require_admin(&bearer).await {
                Ok(()) => true,
                Err(e) if e.status == 403 => false,
                Err(e) => return Err(e),
            };
            return Ok((
                200,
                self.browser_session(req, res, bearer, user, is_admin)
                    .await?,
            ));
        }
        if path == "/api/login" && method == "POST" {
            let address = address(req);
            self.rate(format!("login:{address}"), 10, 900000).await?;
            let input = Self::body(req, 16384).await?;
            let user = self.owner(&input["username"])?;
            let password = input["password"]
                .as_str()
                .filter(|p| !p.is_empty() && p.len() <= 4096)
                .ok_or_else(|| err(400, "invalid_login", "Password is required."))?;
            let login=self.palpo.post("/_matrix/client/v3/login","",&json!({"type":"m.login.password","identifier":{"type":"m.id.user","user":user},"password":password,"initial_device_display_name":"Palpo web administration"})).await?;
            let token = s(&login["access_token"]).to_string();
            let verified = async {
                if token.is_empty() || login["user_id"] != user {
                    return Err(err(
                        502,
                        "invalid_login",
                        "Palpo did not verify the requested identity.",
                    ));
                }
                match self.palpo.require_admin(&token).await {
                    Ok(()) => Ok(true),
                    Err(e) if e.status == 403 => Ok(false),
                    Err(e) => Err(e),
                }
            }
            .await;
            let is_admin = match verified {
                Ok(v) => v,
                Err(e) => {
                    if !token.is_empty() {
                        let _ = self
                            .palpo
                            .post("/_matrix/client/v3/logout", &token, &json!({}))
                            .await;
                    }
                    return Err(e);
                }
            };
            self.rates.lock().await.remove(&format!("login:{address}"));
            return Ok((
                200,
                self.browser_session(req, res, token, user, is_admin)
                    .await?,
            ));
        }
        if let Some(pair) = pair
            && method == "POST"
        {
            if origin
                .as_deref()
                .is_some_and(|o| o != self.public_origin.origin().ascii_serialization())
            {
                return Err(err(
                    403,
                    "origin_forbidden",
                    "Cross-origin pairing is not allowed.",
                ));
            }
            if bearer.is_empty() {
                return Err(err(
                    401,
                    "owner_token_required",
                    "Pairing requires the owner Matrix access token.",
                ));
            }
            let me = self
                .palpo
                .get("/_matrix/client/v3/account/whoami", &bearer)
                .await?;
            if me["is_guest"] == true || !me["user_id"].is_string() {
                return Err(err(
                    403,
                    "identity_mismatch",
                    "A non-guest Matrix identity is required.",
                ));
            }
            let _guard = self.mutations.lock().await;
            return Ok((
                200,
                self.credentials(pair.get(1).unwrap().as_str(), s(&me["user_id"]))
                    .await?,
            ));
        }
        let sid = req
            .headers()
            .get(header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .split(';')
            .find_map(|p| p.trim().strip_prefix("palpo_admin="))
            .unwrap_or("")
            .to_string();
        let session = {
            let mut sessions = self.sessions.lock().await;
            sessions.retain(|_, v| v.expires > millis());
            sessions.get(&sid).cloned().ok_or_else(|| {
                err(
                    401,
                    "sign_in_required",
                    "Sign in with your local Matrix account.",
                )
            })?
        };
        if mutation
            && !same(
                req.headers()
                    .get("x-csrf-token")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or(""),
                &session.csrf,
            )
        {
            return Err(err(
                403,
                "csrf_forbidden",
                "Refresh the session before trying again.",
            ));
        }
        if path == "/api/logout" && method == "POST" {
            self.palpo
                .post("/_matrix/client/v3/logout", &session.token, &json!({}))
                .await?;
            self.sessions.lock().await.remove(&sid);
            res.add_header(header::SET_COOKIE, self.cookie("", true), true)
                .unwrap();
            return Ok((200, json!({})));
        }
        let identity = self
            .palpo
            .get("/_matrix/client/v3/account/whoami", &session.token)
            .await;
        let identity = match identity {
            Ok(v) => v,
            Err(e) if e.status == 401 => {
                self.sessions.lock().await.remove(&sid);
                return Err(err(401, "sign_in_required", "Your Matrix session expired."));
            }
            Err(e) => return Err(e),
        };
        if identity["user_id"] != session.user_id || identity["is_guest"] == true {
            return Err(err(
                403,
                "identity_mismatch",
                "Matrix session identity changed.",
            ));
        }
        let actor = session.user_id.as_str();
        let token = session.token.as_str();
        if path == "/api/operations/call" && method == "POST" {
            let input = Self::body(req, 16384).await?;
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Call {
                service: String,
                args: Value,
            }
            let call: Call = serde_json::from_value(input)
                .map_err(|_| err(400, "invalid_arguments", "A service and args are required."))?;
            // Origin, cookie, CSRF and current Matrix identity have already
            // been checked. Business policy is shared with the native adapter.
            return Ok((
                200,
                self.operations
                    .browser_call(token, &call.service, call.args)
                    .await?,
            ));
        }
        if method == "GET"
            && ["/api/requests", "/api/catalog", "/api/projects"].contains(&path.as_str())
        {
            let data = match path.as_str() {
                "/api/requests" => self.requests(actor, token).await?,
                "/api/catalog" => self.catalog(actor).await?,
                _ => self.projects(actor, token).await?,
            };
            if self
                .sessions
                .lock()
                .await
                .get(&sid)
                .is_none_or(|s| s.expires <= millis())
            {
                return Err(err(401, "sign_in_required", "Your session expired."));
            }
            return Ok((
                200,
                match path.as_str() {
                    "/api/catalog" => json!({"fleets":data}),
                    "/api/requests" => json!({"requests":data}),
                    _ => json!({"projects":data}),
                },
            ));
        }
        if path == "/api/session" && method == "GET" {
            if session.is_admin {
                self.palpo.require_admin(token).await?;
            }
            return Ok((
                200,
                json!({"userId":actor,"csrf":session.csrf,"isAdmin":session.is_admin,"serverName":self.server_name,"callbackOrigins":if session.is_admin{self.callback_origins.clone()}else{vec![]},"outboundAvailable":true}),
            ));
        }
        if path == "/api/my/fleets" && method == "GET" {
            return Ok((
                200,
                json!({"fleets":object_values(&self.store.snapshot().await["fleets"]).iter().filter(|f|f["ownerMxid"]==actor).map(super::fleet::public_fleet).collect::<Vec<_>>()}),
            ));
        }
        let owned =
            regex::Regex::new(r"^/api/my/fleets/(hf_[a-f0-9]{32})/(pair|connect)$").unwrap();
        if let Some(c) = owned.captures(&path)
            && method == "POST"
        {
            Self::body(req, 16384).await?;
            let _g = self.mutations.lock().await;
            let result = if &c[2] == "pair" {
                self.credentials(&c[1], actor).await?
            } else {
                self.connect(&c[1], actor, token).await?
            };
            return Ok((
                if &c[2] == "connect"
                    && result["transport"]["mode"] == "outbound"
                    && result["readiness"]["ready"] != true
                {
                    202
                } else {
                    200
                },
                result,
            ));
        }
        if ["/api/projects", "/api/requests"].contains(&path.as_str()) && method == "POST" {
            let input = Self::body(req, 16384).await?;
            let _g = self.mutations.lock().await;
            return Ok((
                201,
                if path == "/api/projects" {
                    json!({"project":self.create_project(&input,actor,token).await?})
                } else {
                    json!({"request":self.request(&input,actor,token).await?})
                },
            ));
        }
        self.palpo.require_admin(token).await?;
        if path == "/api/account-requests" && method == "GET" {
            return Ok((200, self.account_admin_view().await));
        }
        if path == "/api/fleets" && method == "GET" {
            return Ok((
                200,
                json!({"fleets":object_values(&self.store.snapshot().await["fleets"]).iter().map(super::fleet::public_fleet).collect::<Vec<_>>()}),
            ));
        }
        if path == "/api/audit" && method == "GET" {
            return Ok((
                200,
                json!({"events":self.store.snapshot().await["audit"].as_array().unwrap().iter().rev().take(200).cloned().collect::<Vec<_>>()}),
            ));
        }
        if path == "/api/fleets" && method == "POST" {
            let input = Self::body(req, 16384).await?;
            let _g = self.mutations.lock().await;
            return Ok((
                201,
                json!({"fleet":self.fleet_create(&input,actor,token).await?}),
            ));
        }
        let routes=regex::Regex::new(r"^/api/fleets/(hf_[a-f0-9]{32})(?:/(install|pause|resume|revoke|agents|outbound)(?:/([a-z0-9_]+)(?:/(retire))?)?)?$").unwrap();
        if let Some(c) = routes.captures(&path) {
            let id = &c[1];
            self.store.fleet(id).await?;
            let action = c.get(2).map(|m| m.as_str()).unwrap_or("");
            let aid = c.get(3).map(|m| m.as_str());
            let retire = c.get(4).is_some();
            if action.is_empty() && method == "GET" {
                return Ok((
                    200,
                    json!({"fleet":super::fleet::public_fleet(&self.store.fleet(id).await?)}),
                ));
            }
            if action == "outbound" && method == "GET" {
                return Ok((
                    200,
                    json!({"queue":self.usage_in(&self.store.snapshot().await,id)}),
                ));
            }
            if action == "agents" && aid.is_none() && method == "GET" {
                return Ok((200, json!({"agents":self.agents(id,token).await?})));
            }
            if method == "POST" || method == "PATCH" {
                let input = Self::body(req, 16384).await?;
                let _g = self.mutations.lock().await;
                return match (action, aid, retire, method.as_str()) {
                    ("install", None, false, "POST") => Ok((
                        200,
                        json!({"fleet":self.fleet_install(id,actor,token).await?}),
                    )),
                    ("pause" | "resume" | "revoke", None, false, "POST") => Ok((
                        200,
                        json!({"fleet":self.fleet_state(id,action,actor,token).await?}),
                    )),
                    ("outbound", None, false, "POST") => Ok((
                        200,
                        json!({"fleet":self.migrate(id,&input,actor,token).await?}),
                    )),
                    ("agents", None, false, "POST") => Ok((
                        201,
                        json!({"agent":self.agent_create(id,&input,actor,token).await?}),
                    )),
                    ("agents", Some(a), false, "PATCH") => Ok((
                        200,
                        json!({"agent":self.agent_update(id,a,&input,actor,token).await?}),
                    )),
                    ("agents", Some(a), true, "POST") => Ok((
                        200,
                        json!({"agent":self.agent_retire(id,a,actor,token).await?}),
                    )),
                    _ => Err(err(404, "not_found", "Endpoint not found.")),
                };
            }
        }
        Err(err(404, "not_found", "Endpoint not found."))
    }
}
#[handler]
impl Admin {
    async fn handle(&self, req: &mut Request, res: &mut Response) {
        for (name, value) in [
            ("cache-control", "no-store"),
            (
                "content-security-policy",
                "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'",
            ),
            ("x-content-type-options", "nosniff"),
            ("referrer-policy", "no-referrer"),
        ] {
            res.add_header(name, value, true).unwrap();
        }
        let read = req.method() == salvo::http::Method::GET
            && matches!(
                req.uri().path(),
                "/api/requests" | "/api/catalog" | "/api/projects"
            );
        let result = if read {
            tokio::time::timeout(self.read_timeout, self.route(req, res))
                .await
                .unwrap_or_else(|_| {
                    Err(err(
                        504,
                        "read_timeout",
                        "Status verification timed out. Refresh to verify the current state.",
                    ))
                })
        } else {
            self.route(req, res).await
        };
        match result {
            Ok((status, data)) => {
                res.status_code(StatusCode::from_u16(status).unwrap());
                res.render(Json(data));
            }
            Err(e) => render_error(res, e),
        }
    }
}
fn render_error(res: &mut Response, e: ApiError) {
    res.status_code(StatusCode::from_u16(e.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR));
    res.render(Json(json!({"code":e.code,"error":e.message})));
}
fn host_port(url: &Url) -> String {
    url[url::Position::BeforeHost..url::Position::AfterPort].to_string()
}
fn address(req: &Request) -> String {
    req.remote_addr()
        .as_ipv4()
        .map(|a| a.ip().to_string())
        .or_else(|| req.remote_addr().as_ipv6().map(|a| a.ip().to_string()))
        .unwrap_or_else(|| "unknown".into())
}
fn decode(v: &str) -> Result<String> {
    url::Url::parse(&format!("http://localhost/?v={}", v.replace('+', "%2B")))
        .ok()
        .and_then(|u| u.query_pairs().next().map(|(_, v)| v.into_owned()))
        .ok_or_else(|| err(400, "invalid_path", "Invalid path encoding."))
}
