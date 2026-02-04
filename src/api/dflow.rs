use serde::{Deserialize, Serialize};
use gloo_net::http::Request;

// Use hermes backend proxy to avoid CORS issues
const HERMES_DFLOW_ENDPOINT: &str = "https://hermes-titan-proxy.fly.dev/api/dflow/quote";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DflowQuoteResponse {
    #[serde(rename = "outAmount")]
    pub out_amount: String,
    #[serde(rename = "inAmount")]
    pub in_amount: String,
    #[serde(default)]
    pub routes: Vec<serde_json::Value>,
}

pub struct DflowClient;

impl DflowClient {
    /// Get Dflow quote via hermes proxy (uses GET with query params)
    pub async fn get_quote(
        input_mint: &str,
        output_mint: &str,
        amount: u64,
        slippage_bps: u16,
    ) -> Result<DflowQuoteResponse, String> {
        let url = format!(
            "{}?inputMint={}&outputMint={}&amount={}&slippageBps={}",
            HERMES_DFLOW_ENDPOINT, input_mint, output_mint, amount, slippage_bps
        );

        log::info!("Fetching Dflow quote via hermes proxy");

        let response = Request::get(&url)
            .send()
            .await
            .map_err(|e| format!("Dflow request failed: {:?}", e))?;

        if !response.ok() {
            return Err(format!("Dflow API error: {}", response.status()));
        }

        let quote = response
            .json::<DflowQuoteResponse>()
            .await
            .map_err(|e| format!("Failed to parse Dflow response: {:?}", e))?;

        log::info!("Dflow quote received: {} -> {}", quote.in_amount, quote.out_amount);

        Ok(quote)
    }
}