// Titan API types for browser client
use serde::{Deserialize, Serialize};

// Simplified response matching Dflow/Jupiter format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TitanQuoteResponse {
    #[serde(rename = "outAmount")]
    pub out_amount: String,
    #[serde(rename = "inAmount")]
    pub in_amount: String,
}