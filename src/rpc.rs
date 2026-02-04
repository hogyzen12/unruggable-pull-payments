use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct RpcRequest<'a, T> {
    jsonrpc: &'static str,
    id: u32,
    method: &'static str,
    params: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(borrow)]
    _phantom: Option<&'a ()>,
}

#[derive(Serialize)]
struct AccountInfoConfig {
    encoding: &'static str,
}

#[derive(Deserialize)]
struct RpcResponse<T> {
    result: Option<T>,
    error: Option<RpcError>,
}

#[derive(Deserialize)]
struct RpcError {
    message: String,
}

#[derive(Deserialize)]
struct AccountInfoResult {
    value: Option<AccountInfoValue>,
}

#[derive(Deserialize)]
struct AccountInfoValue {
    data: (String, String),
}

pub async fn get_account_data_base64(rpc_url: &str, pubkey: &str) -> Result<String, String> {
    let params = (pubkey, AccountInfoConfig { encoding: "base64" });
    let req = RpcRequest {
        jsonrpc: "2.0",
        id: 1,
        method: "getAccountInfo",
        params,
        _phantom: None,
    };

    let resp = Request::post(rpc_url)
        .json(&req)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let body: RpcResponse<AccountInfoResult> = resp.json().await.map_err(|e| e.to_string())?;
    if let Some(err) = body.error {
        return Err(err.message);
    }
    let value = body
        .result
        .and_then(|r| r.value)
        .ok_or("account not found")?;
    Ok(value.data.0)
}
