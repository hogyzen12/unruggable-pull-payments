use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use dioxus::prelude::*;
use js_sys::Date;
use serde_json::to_string;
use sha2::{Digest, Sha256};

use crate::rpc::get_account_data_base64;
use crate::timed_delegation::{
    build_create_delegation_instructions, build_set_auth_instructions,
    build_withdraw_instructions, build_withdraw_message, decode_base64,
    derive_addresses, parse_nonce_from_state, DEFAULT_PROGRAM_ID, USDC_MINT,
};
use crate::wallet::{PasskeyEnv, WalletAdapter};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

const RPC_URL: &str = "https://johna-k3cr1v-fast-mainnet.helius-rpc.com";

#[component]
pub fn DelegationModal() -> Element {
    let mut wallet = use_signal(|| WalletAdapter::new());
    let mut wallet_address = use_signal(|| Option::<String>::None);
    let mut mode = use_signal(|| "delegate".to_string());
    let program_id = DEFAULT_PROGRAM_ID.to_string();
    let mint = USDC_MINT.to_string();
    let mut delegate_beneficiary = use_signal(|| String::new());
    let mut delegate_amount = use_signal(|| "10".to_string());
    let mut delegate_duration_hours = use_signal(|| "1".to_string());
    let mut status = use_signal(|| Option::<String>::None);
    let mut withdraw_beneficiary = use_signal(|| String::new());
    let mut withdraw_delegator = use_signal(|| String::new());
    let mut withdraw_amount = use_signal(|| "4.2".to_string());
    let mut auth_expiry_minutes = use_signal(|| "2".to_string());
    let mut delegation_status = use_signal(|| Option::<String>::None);
    let mut passkey_pubkey_b64 = use_signal(|| String::new());
    let mut passkey_cred_id_b64 = use_signal(|| String::new());
    let mut passkey_status = use_signal(|| Option::<String>::None);
    let mut passkey_env = use_signal(|| Option::<PasskeyEnv>::None);

    {
        let adapter = wallet.read().clone();
        let mut passkey_pubkey_b64 = passkey_pubkey_b64.clone();
        let mut passkey_cred_id_b64 = passkey_cred_id_b64.clone();
        let mut passkey_status = passkey_status.clone();
        let mut passkey_env = passkey_env.clone();
        use_effect(move || {
            let adapter = adapter.clone();
            spawn(async move {
                if let Ok(Some(passkey)) = adapter.get_stored_passkey().await {
                    passkey_pubkey_b64.set(passkey.pubkey_b64);
                    passkey_cred_id_b64.set(passkey.cred_id_b64);
                    passkey_status.set(Some("Passkey loaded".to_string()));
                }
                if let Ok(env) = adapter.passkey_env().await {
                    passkey_env.set(Some(env));
                }
            });
        });
    }

    let connect_wallet = move |_| {
        spawn(async move {
            match WalletAdapter::connect_wallet().await {
                Ok(address) => {
                    wallet_address.set(Some(address.clone()));
                    status.set(None);
                }
                Err(e) => status.set(Some(format!("Connect failed: {}", e))),
            }
        });
    };

    let program_id_for_create = program_id.clone();
    let mint_for_create = mint.clone();
    let create_delegation = move |_| {
        let program_id = program_id_for_create.clone();
        let mint = mint_for_create.clone();
        let beneficiary = delegate_beneficiary.read().trim().to_string();
        let max_amount = delegate_amount.read().clone();
        let duration_hours = delegate_duration_hours.read().clone();
        let passkey_pubkey_b64 = passkey_pubkey_b64.read().clone();
        let env = passkey_env.read().clone();
        let wallet_address = wallet_address.read().clone();
        spawn(async move {
            let Some(fee_payer) = wallet_address else {
                status.set(Some("Connect wallet first".to_string()));
                return;
            };
            if beneficiary.is_empty() {
                status.set(Some("Enter beneficiary pubkey".to_string()));
                return;
            }
            if let Some(env) = env {
                if env.in_app || !env.supported || !env.platform {
                    status.set(Some(format!(
                        "Passkeys require {} in the system browser. Tap Open in Browser.",
                        env.hint
                    )));
                    return;
                }
            }
            if passkey_pubkey_b64.is_empty() {
                status.set(Some("Register a passkey before delegating".to_string()));
                return;
            }
            let program_id = Pubkey::from_str(program_id.trim()).map_err(|e| e.to_string());
            let mint = Pubkey::from_str(mint.trim()).map_err(|e| e.to_string());
            let beneficiary = Pubkey::from_str(beneficiary.trim()).map_err(|e| e.to_string());
            let delegator = Pubkey::from_str(fee_payer.trim()).map_err(|e| e.to_string());
            if program_id.is_err() || mint.is_err() || beneficiary.is_err() || delegator.is_err() {
                status.set(Some("Invalid pubkey".to_string()));
                return;
            }
            let program_id = program_id.unwrap();
            let mint = mint.unwrap();
            let beneficiary = beneficiary.unwrap();
            let delegator = delegator.unwrap();

            let auth_pubkey = match decode_base64(&passkey_pubkey_b64) {
                Ok(bytes) => bytes,
                Err(e) => {
                    status.set(Some(e));
                    return;
                }
            };

            let max_amount = match parse_amount(&max_amount, 6) {
                Ok(v) => v,
                Err(e) => {
                    status.set(Some(e));
                    return;
                }
            };
            let duration_hours: i64 = duration_hours.parse().unwrap_or(1);
            let now = (Date::now() / 1000.0) as i64;
            let start_ts = now - 60;
            let end_ts = now + duration_hours * 3600;

            let (addrs, mut instructions) = match build_create_delegation_instructions(
                &program_id,
                &delegator,
                &beneficiary,
                &mint,
                max_amount,
                start_ts,
                end_ts,
            ) {
                Ok(v) => v,
                Err(e) => {
                    status.set(Some(e));
                    return;
                }
            };

            if let Ok(_) = get_account_data_base64(RPC_URL, &addrs.delegation_pda.to_string()).await {
                status.set(Some("Delegation already exists for this beneficiary".to_string()));
                return;
            }

            let (_, auth_ixs) = match build_set_auth_instructions(
                &program_id,
                &delegator,
                &beneficiary,
                &mint,
                &auth_pubkey,
            ) {
                Ok(v) => v,
                Err(e) => {
                    status.set(Some(e));
                    return;
                }
            };
            instructions.extend(auth_ixs);

            let json = match to_string(&instructions) {
                Ok(v) => v,
                Err(e) => {
                    status.set(Some(e.to_string()));
                    return;
                }
            };

            let adapter = wallet.read().clone();
            let sig = match adapter.send_instructions_json(RPC_URL, &fee_payer, &json).await {
                Ok(sig) => sig,
                Err(e) => {
                    status.set(Some(e));
                    return;
                }
            };
            withdraw_delegator.set(delegator.to_string());
            withdraw_beneficiary.set(beneficiary.to_string());
            status.set(Some(format!("Delegation + authenticator set: {}", sig)));
        });
    };

    let load_passkey = move |_| {
        let adapter = wallet.read().clone();
        spawn(async move {
            match adapter.get_stored_passkey().await {
                Ok(Some(passkey)) => {
                    passkey_pubkey_b64.set(passkey.pubkey_b64);
                    passkey_cred_id_b64.set(passkey.cred_id_b64);
                    passkey_status.set(Some("Passkey loaded".to_string()));
                }
                Ok(None) => passkey_status.set(Some("No stored passkey found".to_string())),
                Err(e) => passkey_status.set(Some(e)),
            }
        });
    };

    let register_passkey = move |_| {
        let adapter = wallet.read().clone();
        let env = passkey_env.read().clone();
        spawn(async move {
            if let Some(env) = env {
                if env.in_app || !env.supported || !env.platform {
                    passkey_status.set(Some(format!(
                        "Passkeys require {} in the system browser. Tap Open in Browser.",
                        env.hint
                    )));
                    return;
                }
            }
            match adapter.register_passkey().await {
                Ok(passkey) => {
                    passkey_pubkey_b64.set(passkey.pubkey_b64);
                    passkey_cred_id_b64.set(passkey.cred_id_b64);
                    passkey_status.set(Some("Passkey registered".to_string()));
                }
                Err(e) => passkey_status.set(Some(e)),
            }
        });
    };

    let open_in_browser = move |_| {
        let adapter = wallet.read().clone();
        spawn(async move {
            let _ = adapter
                .open_system_browser("https://pull.unruggable.io")
                .await;
        });
    };

    let program_id_for_auth = program_id.clone();
    let mint_for_auth = mint.clone();
    let set_authenticator = move |_| {
        let program_id = program_id_for_auth.clone();
        let mint = mint_for_auth.clone();
        let beneficiary = delegate_beneficiary.read().trim().to_string();
        let passkey_pubkey_b64 = passkey_pubkey_b64.read().clone();
        let wallet_address = wallet_address.read().clone();
        spawn(async move {
            let Some(delegator_str) = wallet_address else {
                status.set(Some("Connect delegator wallet first".to_string()));
                return;
            };
            if beneficiary.is_empty() {
                status.set(Some("Enter beneficiary pubkey first".to_string()));
                return;
            }
            if passkey_pubkey_b64.is_empty() {
                status.set(Some("Register a passkey first".to_string()));
                return;
            }
            let program_id = Pubkey::from_str(program_id.trim()).map_err(|e| e.to_string());
            let mint = Pubkey::from_str(mint.trim()).map_err(|e| e.to_string());
            let beneficiary = Pubkey::from_str(beneficiary.trim()).map_err(|e| e.to_string());
            let delegator = Pubkey::from_str(delegator_str.trim()).map_err(|e| e.to_string());
            if program_id.is_err() || mint.is_err() || beneficiary.is_err() || delegator.is_err() {
                status.set(Some("Invalid pubkey".to_string()));
                return;
            }
            let program_id = program_id.unwrap();
            let mint = mint.unwrap();
            let beneficiary = beneficiary.unwrap();
            let delegator = delegator.unwrap();

            let auth_pubkey = match decode_base64(&passkey_pubkey_b64) {
                Ok(bytes) => bytes,
                Err(e) => {
                    status.set(Some(e));
                    return;
                }
            };

            let (_, instructions) = match build_set_auth_instructions(
                &program_id,
                &delegator,
                &beneficiary,
                &mint,
                &auth_pubkey,
            ) {
                Ok(v) => v,
                Err(e) => {
                    status.set(Some(e));
                    return;
                }
            };

            let json = match to_string(&instructions) {
                Ok(v) => v,
                Err(e) => {
                    status.set(Some(e.to_string()));
                    return;
                }
            };

            let adapter = wallet.read().clone();
            let sig = match adapter.send_instructions_json(RPC_URL, &delegator_str, &json).await {
                Ok(sig) => sig,
                Err(e) => {
                    status.set(Some(e));
                    return;
                }
            };
            status.set(Some(format!("Authenticator set: {}", sig)));
        });
    };

    let program_id_for_check = program_id.clone();
    let mint_for_check = mint.clone();
    let check_delegation = move |_| {
        let program_id = program_id_for_check.clone();
        let mint = mint_for_check.clone();
        let beneficiary = withdraw_beneficiary.read().trim().to_string();
        let delegator = withdraw_delegator.read().trim().to_string();
        spawn(async move {
            if beneficiary.is_empty() || delegator.is_empty() {
                delegation_status.set(Some("Enter beneficiary + delegator pubkeys".to_string()));
                return;
            }
            let program_id = Pubkey::from_str(program_id.trim()).map_err(|e| e.to_string());
            let mint = Pubkey::from_str(mint.trim()).map_err(|e| e.to_string());
            let beneficiary = Pubkey::from_str(beneficiary.trim()).map_err(|e| e.to_string());
            let delegator = Pubkey::from_str(delegator.trim()).map_err(|e| e.to_string());
            if program_id.is_err() || mint.is_err() || beneficiary.is_err() || delegator.is_err() {
                delegation_status.set(Some("Invalid pubkey".to_string()));
                return;
            }
            let program_id = program_id.unwrap();
            let mint = mint.unwrap();
            let beneficiary = beneficiary.unwrap();
            let delegator = delegator.unwrap();

            match fetch_nonce(&program_id, &mint, &beneficiary, &delegator).await {
                Ok(n) => delegation_status.set(Some(format!("Delegation found (nonce {})", n))),
                Err(e) => delegation_status.set(Some(format!("Delegation not found: {}", e))),
            }
        });
    };

    let program_id_for_withdraw = program_id.clone();
    let mint_for_withdraw = mint.clone();
    let withdraw = move |_| {
        let program_id = program_id_for_withdraw.clone();
        let mint = mint_for_withdraw.clone();
        let beneficiary_input = withdraw_beneficiary.read().trim().to_string();
        let delegator = withdraw_delegator.read().trim().to_string();
        let withdraw_amount = withdraw_amount.read().clone();
        let auth_expiry_minutes = auth_expiry_minutes.read().clone();
        let passkey_pubkey_b64 = passkey_pubkey_b64.read().clone();
        let passkey_cred_id_b64 = passkey_cred_id_b64.read().clone();
        let wallet_address = wallet_address.read().clone();
        let env = passkey_env.read().clone();
        spawn(async move {
            let Some(beneficiary_wallet) = wallet_address else {
                status.set(Some("Connect beneficiary wallet first".to_string()));
                return;
            };

            if beneficiary_input.is_empty() {
                status.set(Some("Enter beneficiary pubkey".to_string()));
                return;
            }
            if beneficiary_wallet.trim() != beneficiary_input.trim() {
                status.set(Some("Connected wallet does not match beneficiary input".to_string()));
                return;
            }
            if delegator.is_empty() {
                status.set(Some("Enter delegator pubkey".to_string()));
                return;
            }
            if passkey_pubkey_b64.is_empty() || passkey_cred_id_b64.is_empty() {
                status.set(Some("Register/load a passkey first".to_string()));
                return;
            }
            if let Some(env) = env {
                if env.in_app || !env.supported || !env.platform {
                    status.set(Some(format!(
                        "Passkeys require {} in the system browser. Tap Open in Browser.",
                        env.hint
                    )));
                    return;
                }
            }

            let program_id = Pubkey::from_str(program_id.trim()).map_err(|e| e.to_string());
            let mint = Pubkey::from_str(mint.trim()).map_err(|e| e.to_string());
            let beneficiary = Pubkey::from_str(beneficiary_wallet.trim()).map_err(|e| e.to_string());
            let delegator = Pubkey::from_str(delegator.trim()).map_err(|e| e.to_string());
            if program_id.is_err() || mint.is_err() || beneficiary.is_err() || delegator.is_err() {
                status.set(Some("Invalid pubkey".to_string()));
                return;
            }
            let program_id = program_id.unwrap();
            let mint = mint.unwrap();
            let beneficiary = beneficiary.unwrap();
            let delegator = delegator.unwrap();

            let nonce_u64 = match fetch_nonce(&program_id, &mint, &beneficiary, &delegator).await {
                Ok(n) => n,
                Err(e) => {
                    status.set(Some(format!("Delegation not found or invalid: {}", e)));
                    return;
                }
            };

            let amount = match parse_amount(&withdraw_amount, 6) {
                Ok(v) => v,
                Err(e) => {
                    status.set(Some(e));
                    return;
                }
            };
            let expiry_minutes: i64 = auth_expiry_minutes.parse().unwrap_or(2);
            let now = (Date::now() / 1000.0) as i64;
            let auth_expiry_ts = now + expiry_minutes * 60;

            let addrs = derive_addresses(&program_id, &delegator, &beneficiary, &mint);
            let message = build_withdraw_message(
                &program_id,
                &addrs.delegation_pda,
                &addrs.source_ata,
                &addrs.destination_ata,
                amount,
                nonce_u64,
                auth_expiry_ts,
            );
            let mut hasher = Sha256::new();
            hasher.update(&message);
            let challenge = hasher.finalize();
            let challenge_b64 = B64.encode(challenge);

            let adapter = wallet.read().clone();
            let passkey_sig = match adapter
                .sign_passkey(&challenge_b64, &passkey_cred_id_b64)
                .await
            {
                Ok(sig) => sig,
                Err(e) => {
                    status.set(Some(e));
                    return;
                }
            };

            let signature = match decode_base64(&passkey_sig.signature_b64) {
                Ok(v) => v,
                Err(e) => {
                    status.set(Some(e));
                    return;
                }
            };
            let authenticator_data = match decode_base64(&passkey_sig.authenticator_data_b64) {
                Ok(v) => v,
                Err(e) => {
                    status.set(Some(e));
                    return;
                }
            };
            let client_data_json = match decode_base64(&passkey_sig.client_data_json_b64) {
                Ok(v) => v,
                Err(e) => {
                    status.set(Some(e));
                    return;
                }
            };
            let auth_pubkey = match decode_base64(&passkey_pubkey_b64) {
                Ok(v) => v,
                Err(e) => {
                    status.set(Some(e));
                    return;
                }
            };

            let (_, instructions) = match build_withdraw_instructions(
                &program_id,
                &delegator,
                &beneficiary,
                &mint,
                amount,
                nonce_u64,
                auth_expiry_ts,
                &auth_pubkey,
                &authenticator_data,
                &client_data_json,
                &signature,
            ) {
                Ok(v) => v,
                Err(e) => {
                    status.set(Some(e));
                    return;
                }
            };

            let json = match to_string(&instructions) {
                Ok(v) => v,
                Err(e) => {
                    status.set(Some(e.to_string()));
                    return;
                }
            };

            let sig = match adapter
                .send_instructions_json(RPC_URL, &beneficiary_wallet, &json)
                .await
            {
                Ok(sig) => sig,
                Err(e) => {
                    status.set(Some(e));
                    return;
                }
            };
            status.set(Some(format!("Withdraw sent: {}", sig)));
        });
    };

    let delegate_end_ts = {
        let now = (Date::now() / 1000.0) as i64;
        let duration_hours: i64 = delegate_duration_hours.read().parse().unwrap_or(1);
        now + duration_hours * 3600
    };
    let auth_expires_ts = {
        let now = (Date::now() / 1000.0) as i64;
        let expiry_minutes: i64 = auth_expiry_minutes.read().parse().unwrap_or(2);
        now + expiry_minutes * 60
    };

    rsx! {
        div {
            style: "max-width: 560px; margin: 0 auto; padding: 24px; background: linear-gradient(135deg, #1e293b 0%, #0f172a 100%); border-radius: 16px; box-shadow: 0 8px 32px rgba(0,0,0,0.4); border: 2px solid #334155;",
            h2 { style: "color: #e0e0e0; margin-bottom: 16px; font-size: 22px; text-align: center;", "Timed Delegation" }

            if wallet_address.read().is_none() {
                button {
                    onclick: connect_wallet,
                    style: "width: 100%; background: #0f172a; color: #e0e0e0; padding: 12px; border-radius: 12px; border: 2px solid #3b82f6; font-size: 14px; font-weight: 600;",
                    "Connect Wallet"
                }
            } else {
                div { style: "color: #94a3b8; margin-bottom: 16px; font-size: 12px; text-align: center;", 
                    "Connected: {wallet_address.read().as_ref().unwrap().chars().take(8).collect::<String>()}..."
                }
            }

            div { style: "display: flex; gap: 8px; margin-bottom: 16px;",
                button {
                    onclick: move |_| mode.set("delegate".to_string()),
                    style: if mode.read().as_str() == "delegate" { "flex:1;padding:10px;border-radius:10px;background:#1d4ed8;color:#fff;border:none;" } else { "flex:1;padding:10px;border-radius:10px;background:#0f172a;color:#94a3b8;border:1px solid #334155;" },
                    "Delegate"
                }
                button {
                    onclick: move |_| {
                        mode.set("withdraw".to_string());
                    },
                    style: if mode.read().as_str() == "withdraw" { "flex:1;padding:10px;border-radius:10px;background:#10b981;color:#fff;border:none;" } else { "flex:1;padding:10px;border-radius:10px;background:#0f172a;color:#94a3b8;border:1px solid #334155;" },
                    "Withdraw"
                }
            }

            if mode.read().as_str() == "delegate" {
                div { style: "display: grid; gap: 8px; margin-bottom: 16px;",
                    input { value: "{delegate_beneficiary}", oninput: move |e| delegate_beneficiary.set(e.value().clone()), placeholder: "Beneficiary Pubkey", style: "padding: 12px; border-radius: 10px; background: #0f172a; border: 1px solid #334155; color: #e0e0e0;" }
                    input { value: "{delegate_amount}", oninput: move |e| delegate_amount.set(e.value().clone()), placeholder: "USDC amount (e.g. 10)", style: "padding: 12px; border-radius: 10px; background: #0f172a; border: 1px solid #334155; color: #e0e0e0;" }
                    input { value: "{delegate_duration_hours}", oninput: move |e| delegate_duration_hours.set(e.value().clone()), placeholder: "Time limit (hours)", style: "padding: 12px; border-radius: 10px; background: #0f172a; border: 1px solid #334155; color: #e0e0e0;" }
                    div { style: "padding: 8px 10px; border-radius: 10px; background: #0b1220; border: 1px solid #334155; color: #94a3b8; font-size: 12px;",
                        "Ends: {format_ts(delegate_end_ts)}"
                    }
                    button { onclick: create_delegation, style: "padding: 14px; border-radius: 10px; background: #1d4ed8; color: #fff; border: none; font-weight: 600;", "Delegate USDC" }
                    div { style: "padding: 10px; border-radius: 12px; background: #0b1220; border: 1px solid #334155; display: grid; gap: 8px;",
                        div { style: "font-size: 12px; color: #94a3b8;", "Passkey is required and is set automatically when you delegate." }
                        if let Some(env) = passkey_env.read().as_ref() {
                            if env.in_app || !env.supported || !env.platform {
                                div { style: "padding: 10px; border-radius: 10px; background: #111827; border: 1px solid #ef4444; color: #fecaca; font-size: 12px;",
                                    "Passkeys require {env.hint} in the system browser. In-app wallet browsers often block passkeys."
                                }
                                button { onclick: open_in_browser, style: "padding: 10px; border-radius: 10px; background: #ef4444; color: #fff; border: none; font-weight: 600;", "Open in Browser" }
                            }
                        }
                        div { style: "display: flex; gap: 8px;",
                            button { onclick: register_passkey, style: "flex: 1; padding: 10px; border-radius: 10px; background: #0f172a; color: #cbd5f5; border: 1px solid #334155;", "Register Passkey" }
                            button { onclick: load_passkey, style: "flex: 1; padding: 10px; border-radius: 10px; background: #0f172a; color: #cbd5f5; border: 1px solid #334155;", "Load Passkey" }
                            button { onclick: set_authenticator, style: "flex: 1; padding: 10px; border-radius: 10px; background: #334155; color: #e2e8f0; border: none;", "Update Passkey" }
                        }
                        if let Some(msg) = passkey_status.read().as_ref() {
                            div { style: "font-size: 12px; color: #94a3b8;", "{msg}" }
                        }
                        if !passkey_pubkey_b64.read().is_empty() {
                            div { style: "font-size: 11px; color: #64748b;", "Pubkey: {passkey_pubkey_b64.read().chars().take(16).collect::<String>()}..." }
                        }
                    }
                }
            } else {
                div { style: "display: grid; gap: 8px; margin-bottom: 12px;",
                    div { style: "padding: 10px; border-radius: 10px; background: #0b1220; border: 1px solid #334155; color: #cbd5f5; font-size: 12px;",
                        "Connect as beneficiary, then withdraw using the passkey set by the delegator."
                    }
                    if let Some(env) = passkey_env.read().as_ref() {
                        if env.in_app || !env.supported || !env.platform {
                            div { style: "padding: 10px; border-radius: 10px; background: #111827; border: 1px solid #ef4444; color: #fecaca; font-size: 12px;",
                                "Passkeys require {env.hint} in the system browser. In-app wallet browsers often block passkeys."
                            }
                            button { onclick: open_in_browser, style: "padding: 10px; border-radius: 10px; background: #ef4444; color: #fff; border: none; font-weight: 600;", "Open in Browser" }
                        }
                    }
                    div { style: "display: flex; gap: 8px;",
                        button { onclick: move |_| {
                            if let Some(addr) = wallet_address.read().clone() {
                                withdraw_delegator.set(addr);
                            } else {
                                status.set(Some("Connect wallet first".to_string()));
                            }
                        }, style: "flex: 1; padding: 10px; border-radius: 10px; background: #0f172a; color: #94a3b8; border: 1px solid #334155;", "Use connected as delegator" }
                        button { onclick: move |_| {
                            if let Some(addr) = wallet_address.read().clone() {
                                withdraw_beneficiary.set(addr);
                            } else {
                                status.set(Some("Connect wallet first".to_string()));
                            }
                        }, style: "flex: 1; padding: 10px; border-radius: 10px; background: #0f172a; color: #94a3b8; border: 1px solid #334155;", "Use connected as beneficiary" }
                    }
                    input { value: "{withdraw_beneficiary}", oninput: move |e| withdraw_beneficiary.set(e.value().clone()), placeholder: "Beneficiary Pubkey (must match connected wallet)", style: "padding: 12px; border-radius: 10px; background: #0f172a; border: 1px solid #334155; color: #e0e0e0;" }
                    input { value: "{withdraw_delegator}", oninput: move |e| withdraw_delegator.set(e.value().clone()), placeholder: "Delegator Pubkey", style: "padding: 12px; border-radius: 10px; background: #0f172a; border: 1px solid #334155; color: #e0e0e0;" }
                    input { value: "{withdraw_amount}", oninput: move |e| withdraw_amount.set(e.value().clone()), placeholder: "Withdraw amount (USDC)", style: "padding: 12px; border-radius: 10px; background: #0f172a; border: 1px solid #334155; color: #e0e0e0;" }
                    input { value: "{auth_expiry_minutes}", oninput: move |e| auth_expiry_minutes.set(e.value().clone()), placeholder: "Auth expiry (minutes)", style: "padding: 12px; border-radius: 10px; background: #0f172a; border: 1px solid #334155; color: #e0e0e0;" }
                    div { style: "padding: 8px 10px; border-radius: 10px; background: #0b1220; border: 1px solid #334155; color: #94a3b8; font-size: 12px;",
                        "Auth expires: {format_ts(auth_expires_ts)}"
                    }
                    if let Some(msg) = delegation_status.read().as_ref() {
                        div { style: "padding: 8px 10px; border-radius: 10px; background: #0b1220; border: 1px solid #334155; color: #94a3b8; font-size: 12px;",
                            "{msg}"
                        }
                    }
                    div { style: "display: flex; gap: 8px;",
                        button { onclick: check_delegation, style: "flex: 1; padding: 12px; border-radius: 10px; background: #0f172a; color: #cbd5f5; border: 1px solid #334155;", "Check Delegation" }
                        button { onclick: withdraw, style: "flex: 1; padding: 12px; border-radius: 10px; background: #10b981; color: #fff; border: none;", "Withdraw" }
                    }
                }
            }

            if let Some(msg) = status.read().as_ref() {
                div { style: "margin-top: 12px; color: #fca5a5; font-size: 12px;", "{msg}" }
            }
        }
    }
}

fn parse_amount(value: &str, decimals: u8) -> Result<u64, String> {
    let mut parts = value.split('.');
    let whole = parts.next().unwrap_or("0");
    let frac = parts.next();
    if parts.next().is_some() {
        return Err("invalid amount format".to_string());
    }

    let whole_val: u64 = whole.parse().map_err(|_| "invalid number")?;
    let scale = 10u64.pow(decimals as u32);
    let mut amount = whole_val
        .checked_mul(scale)
        .ok_or("amount overflow")?;

    if let Some(frac_str) = frac {
        if frac_str.len() > decimals as usize {
            return Err("too many decimal places".to_string());
        }
        let frac_val: u64 = if frac_str.is_empty() { 0 } else { frac_str.parse().map_err(|_| "invalid fraction")? };
        let frac_scale = 10u64.pow((decimals as usize - frac_str.len()) as u32);
        amount = amount
            .checked_add(frac_val * frac_scale)
            .ok_or("amount overflow")?;
    }

    Ok(amount)
}

fn format_ts(ts: i64) -> String {
    let date = Date::new(&wasm_bindgen::JsValue::from_f64((ts as f64) * 1000.0));
    date.to_string().into()
}

async fn fetch_nonce(
    program_id: &Pubkey,
    mint: &Pubkey,
    beneficiary: &Pubkey,
    delegator: &Pubkey,
) -> Result<u64, String> {
    let addrs = derive_addresses(program_id, delegator, beneficiary, mint);
    let data_b64 =
        get_account_data_base64(RPC_URL, &addrs.delegation_pda.to_string()).await?;
    let data = decode_base64(&data_b64)?;
    parse_nonce_from_state(&data)
}
