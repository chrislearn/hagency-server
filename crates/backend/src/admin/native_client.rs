//! User-authenticated enrollment. Shared secrets and embedded Matrix authority
//! stay in this process; native clients receive only their own Fleet custody.
use super::{api::Admin, *};
use reqwest::Method;
use salvo::prelude::*;

pub(crate) async fn route(
    admin: &Admin,
    req: &mut Request,
    path: &str,
    host: &str,
    origin: Option<&str>,
    bearer: &str,
) -> Result<(u16, Value)> {
    if host != super::api::host_port(&admin.public_origin)
        || origin.is_some()
        || req.headers().contains_key("cookie")
        || req.headers().get_all("host").iter().count() != 1
        || req.headers().get_all("authorization").iter().count() > 1
        || req.headers().contains_key("x-forwarded-host")
    {
        return Err(err(
            403,
            "native_origin_required",
            "Use the native client endpoint without browser credentials.",
        ));
    }
    let Some((revoke, secret)) = &admin.oauth_revocation else {
        return Err(err(
            503,
            "pasion_required",
            "Pasion delegated authentication is required.",
        ));
    };
    if path == "/_hagency/client/v1/discovery" && req.method() == salvo::http::Method::GET {
        return Ok((
            200,
            json!({"serverName":admin.server_name,"homeserver":admin.public_origin,"issuer":admin.public_origin.join("_pasion/").unwrap(),"selfService":admin.fleet_access.allow_self_service}),
        ));
    }
    if bearer.len() < 16 || bearer.len() > 4096 {
        return Err(err(401, "sign_in_required", "Sign in with Pasion."));
    }
    admin
        .rate(format!("client:token:{}", hash(bearer)), 60, 60000)
        .await?;
    let endpoint = revoke.join("introspect").unwrap();
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap();
    let mut response = client
        .post(endpoint)
        .bearer_auth(secret)
        .form(&[("token", bearer)])
        .send()
        .await
        .map_err(|_| err(503, "pasion_unreachable", "Pasion did not respond."))?;
    if !response.status().is_success() {
        return Err(err(
            503,
            "pasion_unreachable",
            "Token verification is unavailable.",
        ));
    }
    let mut raw = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| err(502, "invalid_identity", "Invalid Pasion response."))?
    {
        if raw.len() + chunk.len() > 65536 {
            return Err(err(502, "invalid_identity", "Invalid Pasion response."));
        }
        raw.extend_from_slice(&chunk);
    }
    let identity: Value = serde_json::from_slice(&raw)
        .map_err(|_| err(502, "invalid_identity", "Invalid Pasion response."))?;
    if identity["active"] != true
        || !identity["sub"].is_string()
        || !identity["username"].is_string()
        || !identity["scope"].as_str().is_some_and(|v| {
            v.split_whitespace().any(|s| {
                matches!(
                    s,
                    "urn:matrix:client:api:*" | "urn:matrix:org.matrix.msc2967.client:api:*"
                )
            })
        })
    {
        return Err(err(
            401,
            "sign_in_required",
            "An active Pasion user session is required.",
        ));
    }
    let who = admin
        .palpo
        .get("/_matrix/client/v3/account/whoami", bearer)
        .await?;
    let actor = admin.owner(&who["user_id"])?;
    if actor != format!("@{}:{}", s(&identity["username"]), admin.server_name)
        || who["is_guest"] == true
        || identity["client_id"].as_str().is_none_or(|v| v.is_empty())
    {
        return Err(err(403, "human_required", "Use an ordinary user account."));
    }
    let principal = json!({"userId":actor,"subject":identity["sub"],"clientId":identity["client_id"],"serverName":admin.server_name});
    if path == "/_hagency/client/v1/identity" && req.method() == salvo::http::Method::GET {
        return Ok((200, principal));
    }
    // Service authority is deliberately confined to this authenticated and
    // policy-checked enrollment adapter; browser/admin routes keep their guards.
    let mut service = admin.clone();
    let service_token = if admin.embedded_matrix {
        service.palpo = service.palpo.server_authority();
        ""
    } else {
        admin.retirement_token.as_deref().ok_or_else(|| {
            err(
                503,
                "service_authority_unavailable",
                "Server enrollment authority is unavailable.",
            )
        })?
    };
    let _guard = admin.mutations.lock().await;
    if path == "/_hagency/client/v1/fleets" && req.method() == salvo::http::Method::POST {
        if !admin.fleet_access.allow_self_service {
            return Err(err(
                403,
                "self_service_disabled",
                "The server has not enabled self-service enrollment.",
            ));
        }
        let input: Value = req
            .parse_json()
            .await
            .map_err(|_| err(400, "invalid_json", "A JSON object is required."))?;
        let object = input
            .as_object()
            .ok_or_else(|| err(400, "invalid_json", "A JSON object is required."))?;
        if object
            .keys()
            .any(|k| !matches!(k.as_str(), "installationId" | "name"))
        {
            return Err(err(
                400,
                "invalid_enrollment",
                "Only installationId and name are accepted.",
            ));
        }
        let installation = key(&input["installationId"], "Installation ID")?;
        let request = format!("client_{}", hash(format!("{actor}:{installation}")));
        let state = admin.store.snapshot().await;
        let existing = object_values(&state["fleets"])
            .iter()
            .any(|f| f["requestId"] == request && f["ownerMxid"] == actor);
        if !existing
            && object_values(&state["fleets"])
                .iter()
                .filter(|f| f["ownerMxid"] == actor && f["state"] != "revoked")
                .count()
                >= admin.fleet_access.max_per_user
        {
            return Err(err(
                403,
                "fleet_limit",
                "The account's Fleet limit has been reached.",
            ));
        }
        let fleet = service.fleet_create(&json!({"requestId":request,"name":input["name"],"ownerMxid":actor,"transportMode":"outbound"}),&actor,service_token).await?;
        let config = service.credentials(s(&fleet["id"]), &actor).await?;
        return Ok((
            201,
            json!({"identity":principal,"fleet":fleet,"configuration":config,"homeserver":admin.public_origin}),
        ));
    }
    let pattern =
        regex::Regex::new(r"^/_hagency/client/v1/fleets/(hf_[a-f0-9]{32})/connect$").unwrap();
    if let Some(c) = pattern.captures(path)
        && req.method() == salvo::http::Method::POST
    {
        service.fleet_owned(&c[1], &actor).await?;
        let result = service.connect(&c[1], &actor, bearer).await?;
        let status = if result["readiness"]["ready"] == true {
            200
        } else {
            202
        };
        return Ok((status, json!({"fleet":result})));
    }
    Err(err(404, "not_found", "Endpoint not found."))
}

/// The host's embedded Matrix capability. Only this adapter enables it, after
/// verifying the human and server enrollment policy. No HTTP bypass is mounted.
pub(crate) async fn embedded_admin(
    path: &str,
    method: &Method,
    body: Option<&Value>,
) -> Option<Result<(u16, Value)>> {
    let root = "/_palpo/admin/v1/appservices";
    if path == root || path.starts_with(&format!("{root}/")) {
        Some(async {
            let failure = |_|err(503,"matrix_unavailable","Embedded Matrix registration failed.");
            if path == root && *method == Method::GET {
                let records = palpo::appservice::list_all_registrations().await.map_err(failure)?;
                return Ok((200,json!({"appservices":records.iter().map(|(r,_)|json!({"id":r.id})).collect::<Vec<_>>()})));
            }
            if path == root && *method == Method::POST {
                let registration: palpo::core::appservice::Registration = serde_json::from_value(body.cloned().unwrap_or(Value::Null)).map_err(|_|err(400,"invalid_registration","Invalid App Service registration."))?;
                let id = palpo::appservice::register_appservice(registration).await.map_err(failure)?;
                return Ok((200,json!({"id":id})));
            }
            if *method == Method::GET {
                let id = path.strip_prefix(&format!("{root}/")).unwrap();
                let registration = palpo::appservice::get_registration(id).await.map_err(failure)?.ok_or_else(||err(404,"not_found","App Service not found."))?;
                let disabled = palpo::appservice::list_all_registrations().await.map_err(failure)?.iter().find(|(r,_)|r.id == id).is_some_and(|(_,disabled)|*disabled);
                let mut value = serde_json::to_value(registration).unwrap(); value["disabled"] = json!(disabled);
                return Ok((200,value));
            }
            Err(err(403,"service_operation_forbidden","Enrollment cannot perform this operation."))
        }.await)
    } else if let Some(mxid) = path.strip_prefix("/_palpo/admin/v2/users/") {
        Some(async {
            if *method != Method::GET { return Err(err(403,"service_operation_forbidden","Enrollment cannot modify users.")); }
            let id = percent_encoding::percent_decode_str(mxid).decode_utf8().map_err(|_|err(400,"invalid_user","Invalid user."))?;
            let id = palpo::core::OwnedUserId::try_from(id.as_ref()).map_err(|_|err(400,"invalid_user","Invalid user."))?;
            if !palpo::data::user::user_exists(&id).await.map_err(|_|err(503,"matrix_unavailable","User lookup failed."))? { return Err(err(404,"not_found","User not found.")); }
            let user = palpo::data::user::get_user(&id).await.map_err(|_|err(503,"matrix_unavailable","User lookup failed."))?;
            Ok((200,json!({"name":user.id,"appservice_id":user.appservice_id,"deactivated":user.deactivated_at.is_some(),"locked":user.locked_at.is_some()})))
        }.await)
    } else {
        None
    }
}
