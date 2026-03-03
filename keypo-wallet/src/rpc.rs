use crate::error::{Error, Result};

/// Shared JSON-RPC POST helper used by both `BundlerClient` and `PaymasterClient`.
///
/// Builds a `{"jsonrpc":"2.0","id":1,"method":"...","params":...}` envelope,
/// POSTs via the provided `reqwest::Client`, and extracts `result` or maps the
/// error field to `Error::Other`. Callers wrap into domain-specific errors via
/// `.map_err()`.
pub(crate) async fn json_rpc_post(
    client: &reqwest::Client,
    url: &str,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    });

    let resp = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::Other(format!("RPC HTTP error: {e}")))?;

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| Error::Other(format!("RPC HTTP error: {e}")))?;

    if let Some(err) = json.get("error") {
        let code = err.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
        let message = err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown error");
        let data = err
            .get("data")
            .map(|d| format!(" {d}"))
            .unwrap_or_default();
        return Err(Error::Other(format!(
            "RPC error {code}: {message}{data}"
        )));
    }

    // Return `result` field (including JSON null) — caller handles null.
    Ok(json
        .get("result")
        .cloned()
        .unwrap_or(serde_json::Value::Null))
}
