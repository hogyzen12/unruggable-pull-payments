// Titan client - uses hermes HTTP proxy for quotes
use super::types::*;
use gloo_net::http::Request;

const HERMES_TITAN_ENDPOINT: &str = "https://hermes-titan-proxy.fly.dev/api/titan/quote";

pub struct TitanClient;

impl TitanClient {
    pub fn new() -> Self {
        Self
    }

    pub async fn get_quote(
        &mut self,
        input_mint: &str,
        output_mint: &str,
        amount: u64,
        slippage_bps: u16,
    ) -> Result<TitanQuoteResponse, String> {
        log::info!("Fetching Titan quote via hermes HTTP proxy");

        let request_body = serde_json::json!({
            "inputMint": input_mint,
            "outputMint": output_mint,
            "amount": amount.to_string(),
            "slippageBps": slippage_bps
        });

        let response = Request::post(HERMES_TITAN_ENDPOINT)
            .json(&request_body)
            .map_err(|e| format!("Failed to build Titan request: {:?}", e))?
            .send()
            .await
            .map_err(|e| format!("Titan request failed: {:?}", e))?;

        if !response.ok() {
            return Err(format!("Titan API error: {}", response.status()));
        }

        let quote = response
            .json::<TitanQuoteResponse>()
            .await
            .map_err(|e| format!("Failed to parse Titan response: {:?}", e))?;

        log::info!("Titan quote received: {} -> {}", quote.in_amount, quote.out_amount);

        Ok(quote)
    }
}

impl Default for TitanClient {
    fn default() -> Self {
        Self::new()
    }
}