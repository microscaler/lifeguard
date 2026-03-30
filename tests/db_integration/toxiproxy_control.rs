//! Optional [Toxiproxy](https://github.com/Shopify/toxiproxy) HTTP API for CI fault injection.
//!
//! Set `TOXIPROXY_API` (e.g. `http://127.0.0.1:8474`) when using
//! [`.github/docker/docker-compose.yml`](../../.github/docker/docker-compose.yml) so integration tests can
//! toggle the `postgres_replica` proxy. The API listens on **8474**; Postgres through the proxy is on host **6547** in CI Compose (avoids Kind/Tilt replica-0 on **6544**).

/// Proxy name in `.github/docker/toxiproxy.json`.
pub const POSTGRES_REPLICA_PROXY: &str = "postgres_replica";

/// Base URL with no trailing slash, e.g. `http://127.0.0.1:8474`.
///
/// Unset or empty → callers should skip Toxiproxy-dependent tests (e.g. local Kind without Toxiproxy).
pub fn api_base_from_env() -> Option<String> {
    let s = std::env::var("TOXIPROXY_API").ok()?;
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.trim_end_matches('/').to_string())
    }
}

fn join(base: &str, path: &str) -> String {
    format!("{}/{}", base.trim_end_matches('/'), path.trim_start_matches('/'))
}

/// `POST /reset` — enable all proxies and remove all toxics ([Toxiproxy HTTP API](https://github.com/Shopify/toxiproxy#http-api)).
pub fn reset_all(api_base: &str) -> Result<(), String> {
    let url = join(api_base, "reset");
    let resp = ureq::post(&url).call().map_err(|e| e.to_string())?;
    if (200..300).contains(&resp.status()) {
        Ok(())
    } else {
        Err(format!("POST /reset -> HTTP {}", resp.status()))
    }
}

/// `PATCH /proxies/{name}` with `{"enabled": enabled}` (Toxiproxy 2.x deprecates POST for updates).
pub fn set_proxy_enabled(api_base: &str, proxy_name: &str, enabled: bool) -> Result<(), String> {
    let url = join(api_base, &format!("proxies/{proxy_name}"));
    let body = serde_json::json!({ "enabled": enabled });
    let body_str = serde_json::to_string(&body).map_err(|e| e.to_string())?;
    let resp = ureq::patch(&url)
        .set("Content-Type", "application/json")
        .send_string(&body_str)
        .map_err(|e| e.to_string())?;
    if (200..300).contains(&resp.status()) {
        Ok(())
    } else {
        Err(format!("PATCH {url} -> HTTP {}", resp.status()))
    }
}
