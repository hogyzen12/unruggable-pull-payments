use serde::{Deserialize, Serialize};
use gloo_net::http::Request;

const JUPITER_ULTRA_API: &str = "https://api.jup.ag/ultra/v1";
const JUPITER_API_KEY: &str = "ddbf7533-efd7-41a4-b794-59325ccbc383";

// Jupiter Ultra Order Response - EXACT copy from original app
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JupiterUltraOrderResponse {
    pub mode: String,
    #[serde(rename = "inputMint")]
    pub input_mint: String,
    #[serde(rename = "outputMint")]
    pub output_mint: String,
    #[serde(rename = "inAmount")]
    pub in_amount: String,
    #[serde(rename = "outAmount")]
    pub out_amount: String,
    #[serde(rename = "otherAmountThreshold")]
    pub other_amount_threshold: String,
    #[serde(rename = "swapMode")]
    pub swap_mode: String,
    #[serde(rename = "slippageBps")]
    pub slippage_bps: u16,
    #[serde(rename = "priceImpact")]
    pub price_impact: Option<f64>,
    #[serde(rename = "routePlan")]
    pub route_plan: Vec<serde_json::Value>,
    #[serde(rename = "feeBps")]
    pub fee_bps: u16,
    #[serde(rename = "prioritizationFeeLamports")]
    pub prioritization_fee_lamports: u64,
    pub router: String,
    pub transaction: Option<String>,
    pub gasless: bool,
    #[serde(rename = "requestId")]
    pub request_id: String,
    pub taker: Option<String>,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
}

pub struct JupiterClient;

impl JupiterClient {
    /// Get Jupiter Ultra order (quote + unsigned transaction)
    pub async fn get_quote(
        input_mint: &str,
        output_mint: &str,
        amount: u64,
        slippage_bps: u16,
        user_pubkey: Option<&str>,
    ) -> Result<JupiterUltraOrderResponse, String> {
        let mut url = format!(
            "{}/order?inputMint={}&outputMint={}&amount={}&slippageBps={}",
            JUPITER_ULTRA_API, input_mint, output_mint, amount, slippage_bps
        );

        if let Some(pubkey) = user_pubkey {
            url.push_str(&format!("&taker={}", pubkey));
        }

        log::info!("Fetching Jupiter Ultra order: {}", url);

        let response = Request::get(&url)
            .header("x-api-key", JUPITER_API_KEY)
            .send()
            .await
            .map_err(|e| format!("Jupiter request failed: {:?}", e))?;

        if !response.ok() {
            return Err(format!("Jupiter API error: {}", response.status()));
        }

        let order = response
            .json::<JupiterUltraOrderResponse>()
            .await
            .map_err(|e| format!("Failed to parse Jupiter response: {:?}", e))?;

        log::info!("Jupiter order received: {} -> {}", order.in_amount, order.out_amount);

        Ok(order)
    }
}