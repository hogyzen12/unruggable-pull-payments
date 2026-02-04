use serde::Deserialize;
use serde_wasm_bindgen::from_value;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

// Simplified wallet adapter for browser wallets
#[derive(Clone, Debug)]
pub struct WalletAdapter {
    connected: bool,
    public_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PasskeyRegistration {
    #[serde(rename = "credIdB64")]
    pub cred_id_b64: String,
    #[serde(rename = "pubkeyB64")]
    pub pubkey_b64: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PasskeySignature {
    #[serde(rename = "authenticatorDataB64")]
    pub authenticator_data_b64: String,
    #[serde(rename = "clientDataJsonB64")]
    pub client_data_json_b64: String,
    #[serde(rename = "signatureB64")]
    pub signature_b64: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PasskeyEnv {
    pub supported: bool,
    pub platform: bool,
    #[serde(rename = "inApp")]
    pub in_app: bool,
    pub hint: String,
}

impl WalletAdapter {
    pub fn new() -> Self {
        Self {
            connected: false,
            public_key: None,
        }
    }

    pub fn is_installed() -> bool {
        // Check if window.solana exists
        if let Some(window) = web_sys::window() {
            let solana = js_sys::Reflect::get(&window, &JsValue::from_str("solana"));
            return solana.is_ok() && !solana.unwrap().is_undefined();
        }
        false
    }

    pub async fn connect_wallet() -> Result<String, String> {
        let window = web_sys::window().ok_or("window not available")?;
        let solana = js_sys::Reflect::get(&window, &JsValue::from_str("solana"))
            .map_err(|_| "Wallet not found")?;

        if solana.is_undefined() {
            return Err("Solana wallet not installed".to_string());
        }

        // Call solana.connect()
        let connect_fn = js_sys::Reflect::get(&solana, &JsValue::from_str("connect"))
            .map_err(|_| "Connect method not found")?;
        
        let connect_fn: js_sys::Function = connect_fn
            .dyn_into()
            .map_err(|_| "Connect is not a function")?;

        let result = connect_fn.call0(&solana);
        
        // Wait for promise
        let promise: js_sys::Promise = result
            .map_err(|_| "Connect failed")?
            .dyn_into()
            .map_err(|_| "Connect didn't return promise")?;

        let _result = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(|_| "Connection rejected")?;

        // Get publicKey
        let pubkey = js_sys::Reflect::get(&solana, &JsValue::from_str("publicKey"))
            .map_err(|_| "Failed to get publicKey")?;

        let tostring_fn = js_sys::Reflect::get(&pubkey, &JsValue::from_str("toString"))
            .map_err(|_| "toString not found")?;
        
        let tostring_fn: js_sys::Function = tostring_fn
            .dyn_into()
            .map_err(|_| "toString is not a function")?;

        let pubkey_str = tostring_fn
            .call0(&pubkey)
            .map_err(|_| "toString failed")?
            .as_string()
            .ok_or("publicKey toString didn't return string")?;

        Ok(pubkey_str)
    }

    pub async fn connect(&mut self) -> Result<String, String> {
        let pubkey_str = Self::connect_wallet().await?;
        self.connected = true;
        self.public_key = Some(pubkey_str.clone());
        Ok(pubkey_str)
    }

    pub async fn disconnect(&mut self) -> Result<(), String> {
        self.connected = false;
        self.public_key = None;
        Ok(())
    }

    pub fn get_public_key(&self) -> Option<String> {
        self.public_key.clone()
    }

    pub async fn sign_message_base64(&self, message_b64: &str) -> Result<String, String> {
        let window = web_sys::window().ok_or("window not available")?;
        let td = js_sys::Reflect::get(&window, &JsValue::from_str("td"))
            .map_err(|_| "td helper not found")?;
        let sign_fn = js_sys::Reflect::get(&td, &JsValue::from_str("signMessage"))
            .map_err(|_| "signMessage not found")?;
        let sign_fn: js_sys::Function = sign_fn
            .dyn_into()
            .map_err(|_| "signMessage is not a function")?;

        let result = sign_fn
            .call1(&td, &JsValue::from_str(message_b64))
            .map_err(|_| "signMessage failed")?;
        let promise: js_sys::Promise = result
            .dyn_into()
            .map_err(|_| "signMessage didn't return promise")?;
        let sig = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(js_error_to_string)?;
        sig.as_string()
            .ok_or("signature not a string".to_string())
    }

    pub async fn register_passkey(&self) -> Result<PasskeyRegistration, String> {
        let window = web_sys::window().ok_or("window not available")?;
        let td = js_sys::Reflect::get(&window, &JsValue::from_str("td"))
            .map_err(|_| "td helper not found")?;
        let register_fn = js_sys::Reflect::get(&td, &JsValue::from_str("registerPasskey"))
            .map_err(|_| "registerPasskey not found")?;
        let register_fn: js_sys::Function = register_fn
            .dyn_into()
            .map_err(|_| "registerPasskey is not a function")?;
        let result = register_fn
            .call0(&td)
            .map_err(|_| "registerPasskey failed")?;
        let promise: js_sys::Promise = result
            .dyn_into()
            .map_err(|_| "registerPasskey didn't return promise")?;
        let value = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(js_error_to_string)?;
        from_value(value).map_err(|e| e.to_string())
    }

    pub async fn get_stored_passkey(&self) -> Result<Option<PasskeyRegistration>, String> {
        let window = web_sys::window().ok_or("window not available")?;
        let td = js_sys::Reflect::get(&window, &JsValue::from_str("td"))
            .map_err(|_| "td helper not found")?;
        let get_fn = js_sys::Reflect::get(&td, &JsValue::from_str("getStoredPasskey"))
            .map_err(|_| "getStoredPasskey not found")?;
        let get_fn: js_sys::Function = get_fn
            .dyn_into()
            .map_err(|_| "getStoredPasskey is not a function")?;
        let value = get_fn
            .call0(&td)
            .map_err(|_| "getStoredPasskey failed")?;
        if value.is_null() || value.is_undefined() {
            return Ok(None);
        }
        from_value(value).map(Some).map_err(|e| e.to_string())
    }

    pub async fn passkey_env(&self) -> Result<PasskeyEnv, String> {
        let window = web_sys::window().ok_or("window not available")?;
        let td = js_sys::Reflect::get(&window, &JsValue::from_str("td"))
            .map_err(|_| "td helper not found")?;
        let env_fn = js_sys::Reflect::get(&td, &JsValue::from_str("passkeyEnv"))
            .map_err(|_| "passkeyEnv not found")?;
        let env_fn: js_sys::Function = env_fn
            .dyn_into()
            .map_err(|_| "passkeyEnv is not a function")?;
        let result = env_fn.call0(&td).map_err(|_| "passkeyEnv failed")?;
        if result.is_instance_of::<js_sys::Promise>() {
            let promise: js_sys::Promise = result
                .dyn_into()
                .map_err(|_| "passkeyEnv didn't return promise")?;
            let value = wasm_bindgen_futures::JsFuture::from(promise)
                .await
                .map_err(js_error_to_string)?;
            return from_value(value).map_err(|e| e.to_string());
        }
        from_value(result).map_err(|e| e.to_string())
    }

    pub async fn open_system_browser(&self, url: &str) -> Result<(), String> {
        let window = web_sys::window().ok_or("window not available")?;
        let td = js_sys::Reflect::get(&window, &JsValue::from_str("td"))
            .map_err(|_| "td helper not found")?;
        let open_fn = js_sys::Reflect::get(&td, &JsValue::from_str("openSystemBrowser"))
            .map_err(|_| "openSystemBrowser not found")?;
        let open_fn: js_sys::Function = open_fn
            .dyn_into()
            .map_err(|_| "openSystemBrowser is not a function")?;
        open_fn
            .call1(&td, &JsValue::from_str(url))
            .map_err(|_| "openSystemBrowser failed")?;
        Ok(())
    }

    pub async fn sign_passkey(
        &self,
        challenge_b64: &str,
        cred_id_b64: &str,
    ) -> Result<PasskeySignature, String> {
        let window = web_sys::window().ok_or("window not available")?;
        let td = js_sys::Reflect::get(&window, &JsValue::from_str("td"))
            .map_err(|_| "td helper not found")?;
        let sign_fn = js_sys::Reflect::get(&td, &JsValue::from_str("signPasskey"))
            .map_err(|_| "signPasskey not found")?;
        let sign_fn: js_sys::Function = sign_fn
            .dyn_into()
            .map_err(|_| "signPasskey is not a function")?;
        let result = sign_fn
            .call2(&td, &JsValue::from_str(challenge_b64), &JsValue::from_str(cred_id_b64))
            .map_err(|_| "signPasskey failed")?;
        let promise: js_sys::Promise = result
            .dyn_into()
            .map_err(|_| "signPasskey didn't return promise")?;
        let value = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(js_error_to_string)?;
        from_value(value).map_err(|e| e.to_string())
    }

    pub async fn send_instructions_json(
        &self,
        rpc_url: &str,
        fee_payer: &str,
        instructions_json: &str,
    ) -> Result<String, String> {
        let window = web_sys::window().ok_or("window not available")?;
        let td = js_sys::Reflect::get(&window, &JsValue::from_str("td"))
            .map_err(|_| "td helper not found")?;
        let send_fn = js_sys::Reflect::get(&td, &JsValue::from_str("sendInstructions"))
            .map_err(|_| "sendInstructions not found")?;
        let send_fn: js_sys::Function = send_fn
            .dyn_into()
            .map_err(|_| "sendInstructions is not a function")?;

        let result = send_fn
            .call3(
                &td,
                &JsValue::from_str(rpc_url),
                &JsValue::from_str(fee_payer),
                &JsValue::from_str(instructions_json),
            )
            .map_err(|_| "sendInstructions failed")?;
        let promise: js_sys::Promise = result
            .dyn_into()
            .map_err(|_| "sendInstructions didn't return promise")?;
        let sig = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(js_error_to_string)?;
        sig.as_string().ok_or("signature not a string".to_string())
    }
}

impl Default for WalletAdapter {
    fn default() -> Self {
        Self::new()
    }
}

fn js_error_to_string(e: JsValue) -> String {
    if let Some(s) = e.as_string() {
        return s;
    }
    if let Ok(msg) = js_sys::Reflect::get(&e, &JsValue::from_str("message")) {
        if let Some(s) = msg.as_string() {
            return s;
        }
    }
    if let Ok(s) = js_sys::JSON::stringify(&e) {
        if let Some(s) = s.as_string() {
            return s;
        }
    }
    "request rejected".to_string()
}
