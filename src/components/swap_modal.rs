use dioxus::prelude::*;
use crate::api::{JupiterClient, DflowClient, TitanClient};
use crate::wallet::WalletAdapter;

const SOL_MINT: &str = "So11111111111111111111111111111111111111112";
const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

#[derive(Clone, PartialEq)]
pub struct Token {
    pub mint: String,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
}

impl Token {
    pub fn sol() -> Self {
        Self {
            mint: SOL_MINT.to_string(),
            symbol: "SOL".to_string(),
            name: "Solana".to_string(),
            decimals: 9,
        }
    }

    pub fn usdc() -> Self {
        Self {
            mint: USDC_MINT.to_string(),
            symbol: "USDC".to_string(),
            name: "USD Coin".to_string(),
            decimals: 6,
        }
    }
}

#[component]
pub fn SwapModal() -> Element {
    let mut wallet = use_signal(|| WalletAdapter::new());
    let mut wallet_address = use_signal(|| Option::<String>::None);
    let mut input_token = use_signal(|| Token::sol());
    let mut output_token = use_signal(|| Token::usdc());
    let mut input_amount = use_signal(|| String::from(""));
    let mut quote_loading = use_signal(|| false);
    let mut jupiter_quote = use_signal(|| Option::<String>::None);
    let mut dflow_quote = use_signal(|| Option::<String>::None);
    let mut titan_quote = use_signal(|| Option::<String>::None);
    let mut error_message = use_signal(|| Option::<String>::None);

    let connect_wallet = move |_| {
        spawn(async move {
            match WalletAdapter::connect_wallet().await {
                Ok(address) => {
                    log::info!("Wallet connected: {}", address);
                    wallet_address.set(Some(address));
                    error_message.set(None);
                }
                Err(e) => {
                    log::error!("Wallet connection failed: {}", e);
                    error_message.set(Some(format!("Failed to connect wallet: {}", e)));
                }
            }
        });
    };

    // Auto-fetch quotes when amount changes (with debouncing)
    use_effect(move || {
        let input_val = input_amount.read().clone();
        
        if input_val.is_empty() || wallet_address.read().is_none() {
            jupiter_quote.set(None);
            dflow_quote.set(None);
            titan_quote.set(None);
            return;
        }

        let amount: f64 = match input_val.parse() {
            Ok(v) if v > 0.0 => v,
            _ => return,
        };

        let input_tok = input_token.read().clone();
        let output_tok = output_token.read().clone();
        let lamports = (amount * 10_f64.powi(input_tok.decimals as i32)) as u64;

        quote_loading.set(true);
        error_message.set(None);

        spawn(async move {
            // Debounce - wait 500ms before fetching
            gloo_timers::future::sleep(std::time::Duration::from_millis(500)).await;
            
            // Get Jupiter quote with user pubkey
            let user_pk = wallet_address.read().clone();
            log::info!("Fetching Jupiter quote for {} {} -> {}", 
                lamports, input_tok.symbol, output_tok.symbol);
            
            match JupiterClient::get_quote(
                &input_tok.mint, 
                &output_tok.mint, 
                lamports, 
                50,
                user_pk.as_deref()
            ).await {
                Ok(quote) => {
                    log::info!("Jupiter raw response - in: {}, out: {}", 
                        quote.in_amount, quote.out_amount);
                    
                    let out_amount = quote.out_amount.parse::<u64>().unwrap_or(0);
                    let out_val = out_amount as f64 / 10_f64.powi(output_tok.decimals as i32);
                    jupiter_quote.set(Some(format!("{:.6} {}", out_val, output_tok.symbol)));
                    log::info!("Jupiter quote SUCCESS: {} {}", out_val, output_tok.symbol);
                }
                Err(e) => {
                    log::error!("Jupiter quote FAILED: {}", e);
                }
            }

            // Get Dflow quote via hermes proxy
            match DflowClient::get_quote(
                &input_tok.mint, 
                &output_tok.mint, 
                lamports, 
                50
            ).await {
                Ok(quote) => {
                    let out_amount = quote.out_amount.parse::<u64>().unwrap_or(0);
                    let out_val = out_amount as f64 / 10_f64.powi(output_tok.decimals as i32);
                    dflow_quote.set(Some(format!("{:.6} {}", out_val, output_tok.symbol)));
                    log::info!("Dflow quote: {} {}", out_val, output_tok.symbol);
                }
                Err(e) => {
                    log::error!("Dflow quote failed: {}", e);
                }
            }
            
            // Get Titan quote via hermes WebSocket proxy
            let mut titan = TitanClient::default();
            match titan.get_quote(&input_tok.mint, &output_tok.mint, lamports, 50).await {
                Ok(quote) => {
                    let out_amount = quote.out_amount.parse::<u64>().unwrap_or(0);
                    let out_val = out_amount as f64 / 10_f64.powi(output_tok.decimals as i32);
                    titan_quote.set(Some(format!("{:.6} {}", out_val, output_tok.symbol)));
                    log::info!("Titan quote: {} {}", out_val, output_tok.symbol);
                }
                Err(e) => {
                    log::error!("Titan quote failed: {}", e);
                }
            }

            quote_loading.set(false);
        });
    });

    let swap_tokens = move |_| {
        let temp = input_token.read().clone();
        input_token.set(output_token.read().clone());
        output_token.set(temp);
        jupiter_quote.set(None);
        dflow_quote.set(None);
        titan_quote.set(None);
    };

    rsx! {
        div {
            style: "max-width: 480px; margin: 0 auto; padding: 24px; background: linear-gradient(135deg, #1e293b 0%, #0f172a 100%); border-radius: 16px; box-shadow: 0 8px 32px rgba(0,0,0,0.4); border: 2px solid #334155;",

            h2 {
                style: "color: #e0e0e0; margin-bottom: 24px; font-size: 24px; text-align: center;",
                "Swap Tokens"
            }

            // Wallet connection
            if wallet_address.read().is_none() {
                div {
                    id: "connect-wallet-button",
                    style: "margin-bottom: 24px;",
                    
                    button {
                        onclick: connect_wallet,
                        style: "
                            width: 100%;
                            background: linear-gradient(135deg, #2a2a2a 0%, #1a1a1a 100%);
                            color: #e0e0e0;
                            padding: 16px;
                            border-radius: 12px;
                            border: 2px solid #3b82f6;
                            font-size: 16px;
                            font-weight: 600;
                            cursor: pointer;
                            transition: all 0.3s ease;
                        ",
                        "Connect Wallet"
                    }
                }
            } else {
                div {
                    style: "color: #94a3b8; margin-bottom: 24px; font-size: 14px; text-align: center;",
                    "Connected: {wallet_address.read().as_ref().unwrap().chars().take(8).collect::<String>()}..."
                }
            }

            // Input token
            div {
                style: "background: #0f172a; padding: 16px; border-radius: 12px; margin-bottom: 8px; border: 1px solid #334155;",
                
                div {
                    style: "display: flex; justify-content: space-between; margin-bottom: 8px;",
                    span { style: "color: #94a3b8; font-size: 14px;", "You pay" }
                    span { style: "color: #94a3b8; font-size: 14px;", "{input_token.read().symbol}" }
                }
                
                input {
                    r#type: "text",
                    value: "{input_amount}",
                    oninput: move |e| input_amount.set(e.value().clone()),
                    placeholder: "0.0",
                    style: "
                        width: 100%;
                        background: transparent;
                        border: none;
                        color: white;
                        font-size: 32px;
                        outline: none;
                    "
                }
            }

            // Swap button
            button {
                onclick: swap_tokens,
                style: "
                    width: 100%;
                    background: #334155;
                    color: white;
                    padding: 8px;
                    border-radius: 8px;
                    border: 1px solid #475569;
                    margin-bottom: 8px;
                    cursor: pointer;
                    transition: all 0.3s ease;
                ",
                "↓ Swap ↑"
            }

            // Output token with skeleton loader
            div {
                style: "background: #0f172a; padding: 16px; border-radius: 12px; margin-bottom: 16px; border: 1px solid #334155;",
                
                div {
                    style: "display: flex; justify-content: space-between; margin-bottom: 8px;",
                    span { style: "color: #94a3b8; font-size: 14px;", "You receive" }
                    span { style: "color: #94a3b8; font-size: 14px;", "{output_token.read().symbol}" }
                }
                
                if *quote_loading.read() {
                    // Skeleton loader with pulsing circles
                    div {
                        style: "display: flex; align-items: center; gap: 8px; padding: 8px 0;",
                        div {
                            class: "pulse-loader",
                            style: "
                                width: 12px;
                                height: 12px;
                                border-radius: 50%;
                                background: linear-gradient(135deg, #60a5fa 0%, #a78bfa 50%, #34d399 100%);
                            "
                        }
                        div {
                            class: "pulse-loader pulse-delay-1",
                            style: "
                                width: 12px;
                                height: 12px;
                                border-radius: 50%;
                                background: linear-gradient(135deg, #60a5fa 0%, #a78bfa 50%, #34d399 100%);
                            "
                        }
                        div {
                            class: "pulse-loader pulse-delay-2",
                            style: "
                                width: 12px;
                                height: 12px;
                                border-radius: 50%;
                                background: linear-gradient(135deg, #60a5fa 0%, #a78bfa 50%, #34d399 100%);
                            "
                        }
                        span { style: "color: #64748b; font-size: 16px; margin-left: 8px;", "Fetching best rates..." }
                    }
                } else {
                    div {
                        style: "color: white; font-size: 32px;",
                        {
                            if let Some(ref best_quote) = *jupiter_quote.read() {
                                rsx! { "{best_quote}" }
                            } else {
                                rsx! { "0.0" }
                            }
                        }
                    }
                }
            }

            // Quote comparison
            if jupiter_quote.read().is_some() || dflow_quote.read().is_some() || titan_quote.read().is_some() {
                div {
                    id: "quote-comparison-container",
                    style: "background: linear-gradient(135deg, #1e293b 0%, #0f172a 100%); padding: 12px; border-radius: 12px; margin-bottom: 16px; border: 2px solid #3b82f6;",
                    
                    div {
                        style: "color: #64748b; font-size: 12px; text-transform: uppercase; letter-spacing: 1px; margin-bottom: 8px;",
                        "Quote Comparison"
                    }
                    
                    if let Some(ref jup) = *jupiter_quote.read() {
                        div {
                            style: "
                                color: #e0e0e0;
                                font-size: 14px;
                                margin-bottom: 4px;
                                display: flex;
                                justify-content: space-between;
                                align-items: center;
                                padding: 4px 0;
                            ",
                            span { "Jupiter" }
                            span { style: "font-weight: 600;", "{jup}" }
                        }
                    }
                    
                    if let Some(ref df) = *dflow_quote.read() {
                        div {
                            style: "
                                color: #e0e0e0;
                                font-size: 14px;
                                margin-bottom: 4px;
                                display: flex;
                                justify-content: space-between;
                                align-items: center;
                                padding: 4px 0;
                            ",
                            span { "Dflow" }
                            span { style: "font-weight: 600;", "{df}" }
                        }
                    }
                    
                    if let Some(ref titan) = *titan_quote.read() {
                        div {
                            style: "
                                color: #e0e0e0;
                                font-size: 14px;
                                display: flex;
                                justify-content: space-between;
                                align-items: center;
                                padding: 4px 0;
                            ",
                            span { "Titan" }
                            span { style: "font-weight: 600;", "{titan}" }
                        }
                    }
                }
            }

            // Error message
            if let Some(ref err) = *error_message.read() {
                div {
                    style: "color: #ef4444; margin-bottom: 16px; font-size: 14px; padding: 12px; background: rgba(239, 68, 68, 0.1); border-radius: 8px; border: 1px solid #ef4444;",
                    "{err}"
                }
            }
        }
    }
}
