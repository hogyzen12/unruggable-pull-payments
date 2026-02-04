use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::Serialize;
use sha2::{Digest, Sha256};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program, sysvar,
};
use std::str::FromStr;

pub const DEFAULT_PROGRAM_ID: &str = "de1gMWmVGZxacWBjpa6HqCfRG9fxcmkGqGdZKJVq5H9";
pub const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
pub const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
pub const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
pub const SECP256R1_PROGRAM_ID: &str = "Secp256r1SigVerify1111111111111111111111111";

#[derive(Clone, Debug, Serialize)]
pub struct JsInstruction {
    pub program_id: String,
    pub keys: Vec<JsAccountMeta>,
    pub data: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct JsAccountMeta {
    pub pubkey: String,
    pub is_signer: bool,
    pub is_writable: bool,
}

pub struct DelegationAddresses {
    pub source_ata: Pubkey,
    pub destination_ata: Pubkey,
    pub config_pda: Pubkey,
    pub delegation_pda: Pubkey,
    pub delegate_pda: Pubkey,
    pub auth_pda: Pubkey,
}

pub fn derive_addresses(
    program_id: &Pubkey,
    delegator: &Pubkey,
    beneficiary: &Pubkey,
    mint: &Pubkey,
) -> DelegationAddresses {
    let source_ata = associated_token_address(delegator, mint);
    let destination_ata = associated_token_address(beneficiary, mint);
    let (config_pda, _) = Pubkey::find_program_address(&[b"config"], program_id);
    let (delegation_pda, _) = Pubkey::find_program_address(
        &[b"delegation", source_ata.as_ref(), beneficiary.as_ref()],
        program_id,
    );
    let (delegate_pda, _) = Pubkey::find_program_address(&[b"delegate", delegation_pda.as_ref()], program_id);
    let (auth_pda, _) = Pubkey::find_program_address(&[b"auth", delegation_pda.as_ref()], program_id);

    DelegationAddresses {
        source_ata,
        destination_ata,
        config_pda,
        delegation_pda,
        delegate_pda,
        auth_pda,
    }
}

pub fn build_create_delegation_instructions(
    program_id: &Pubkey,
    delegator: &Pubkey,
    beneficiary: &Pubkey,
    mint: &Pubkey,
    max_amount: u64,
    start_ts: i64,
    end_ts: i64,
) -> Result<(DelegationAddresses, Vec<JsInstruction>), String> {
    let addrs = derive_addresses(program_id, delegator, beneficiary, mint);

    let create_source_ata =
        build_create_ata_idempotent_ix(delegator, delegator, &addrs.source_ata, mint);
    let create_destination_ata =
        build_create_ata_idempotent_ix(delegator, beneficiary, &addrs.destination_ata, mint);

    let approve_ix = Instruction {
        program_id: Pubkey::from_str(TOKEN_PROGRAM_ID).unwrap(),
        accounts: vec![
            AccountMeta::new(addrs.source_ata, false),
            AccountMeta::new_readonly(addrs.delegate_pda, false),
            AccountMeta::new_readonly(*delegator, true),
        ],
        data: build_token_approve_data(max_amount),
    };

    let mut data = Vec::with_capacity(1 + 8 + 8 + 8);
    data.push(2);
    data.extend_from_slice(&start_ts.to_le_bytes());
    data.extend_from_slice(&end_ts.to_le_bytes());
    data.extend_from_slice(&max_amount.to_le_bytes());

    let create_ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*delegator, true),
            AccountMeta::new_readonly(*beneficiary, false),
            AccountMeta::new(addrs.source_ata, false),
            AccountMeta::new(addrs.delegation_pda, false),
            AccountMeta::new_readonly(addrs.delegate_pda, false),
            AccountMeta::new_readonly(addrs.config_pda, false),
            AccountMeta::new_readonly(Pubkey::from_str(TOKEN_PROGRAM_ID).unwrap(), false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(sysvar::rent::id(), false),
        ],
        data,
    };

    let instructions = vec![
        instruction_to_js(&create_source_ata),
        instruction_to_js(&create_destination_ata),
        instruction_to_js(&approve_ix),
        instruction_to_js(&create_ix),
    ];
    Ok((addrs, instructions))
}

pub fn build_set_auth_instructions(
    program_id: &Pubkey,
    delegator: &Pubkey,
    beneficiary: &Pubkey,
    mint: &Pubkey,
    auth_pubkey: &[u8],
) -> Result<(DelegationAddresses, Vec<JsInstruction>), String> {
    if auth_pubkey.len() != 33 {
        return Err("auth pubkey must be 33 bytes".to_string());
    }
    let addrs = derive_addresses(program_id, delegator, beneficiary, mint);

    let mut data = Vec::with_capacity(1 + 33);
    data.push(6);
    data.extend_from_slice(auth_pubkey);

    let set_auth_ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new_readonly(*delegator, true),
            AccountMeta::new(addrs.delegation_pda, false),
            AccountMeta::new(addrs.auth_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(sysvar::rent::id(), false),
        ],
        data,
    };

    Ok((addrs, vec![instruction_to_js(&set_auth_ix)]))
}

pub fn build_withdraw_instructions(
    program_id: &Pubkey,
    delegator: &Pubkey,
    beneficiary: &Pubkey,
    mint: &Pubkey,
    amount: u64,
    nonce: u64,
    auth_expiry_ts: i64,
    auth_pubkey: &[u8],
    authenticator_data: &[u8],
    client_data_json: &[u8],
    signature: &[u8],
) -> Result<(DelegationAddresses, Vec<JsInstruction>), String> {
    if signature.len() != 64 {
        return Err("signature must be 64 bytes".to_string());
    }
    if auth_pubkey.len() != 33 {
        return Err("auth pubkey must be 33 bytes".to_string());
    }

    let addrs = derive_addresses(program_id, delegator, beneficiary, mint);
    let message = build_withdraw_message(
        program_id,
        &addrs.delegation_pda,
        &addrs.source_ata,
        &addrs.destination_ata,
        amount,
        nonce,
        auth_expiry_ts,
    );

    let webauthn_message = build_webauthn_message(authenticator_data, client_data_json);
    let secp_ix = build_secp256r1_instruction(auth_pubkey, signature, &webauthn_message)?;

    let mut data = Vec::with_capacity(1 + 8 + 8 + 2 + 2 + authenticator_data.len() + client_data_json.len());
    data.push(3);
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&auth_expiry_ts.to_le_bytes());
    let auth_len: u16 = authenticator_data
        .len()
        .try_into()
        .map_err(|_| "authenticator_data too long".to_string())?;
    let client_len: u16 = client_data_json
        .len()
        .try_into()
        .map_err(|_| "client_data_json too long".to_string())?;
    data.extend_from_slice(&auth_len.to_le_bytes());
    data.extend_from_slice(&client_len.to_le_bytes());
    data.extend_from_slice(authenticator_data);
    data.extend_from_slice(client_data_json);

    let withdraw_ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new_readonly(*beneficiary, true),
            AccountMeta::new(addrs.delegation_pda, false),
            AccountMeta::new_readonly(addrs.auth_pda, false),
            AccountMeta::new_readonly(addrs.delegate_pda, false),
            AccountMeta::new(addrs.source_ata, false),
            AccountMeta::new(addrs.destination_ata, false),
            AccountMeta::new_readonly(addrs.config_pda, false),
            AccountMeta::new_readonly(Pubkey::from_str(TOKEN_PROGRAM_ID).unwrap(), false),
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new_readonly(sysvar::instructions::id(), false),
        ],
        data,
    };

    let instructions = vec![instruction_to_js(&secp_ix), instruction_to_js(&withdraw_ix)];
    Ok((addrs, instructions))
}

pub fn build_withdraw_message(
    program_id: &Pubkey,
    delegation_pda: &Pubkey,
    source_token_account: &Pubkey,
    destination_token_account: &Pubkey,
    amount: u64,
    nonce: u64,
    auth_expiry_ts: i64,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(256);
    out.extend_from_slice(b"TDv1");
    out.push(b'|');
    push_hex(&mut out, program_id.as_ref());
    out.push(b'|');
    push_hex(&mut out, delegation_pda.as_ref());
    out.push(b'|');
    push_hex(&mut out, source_token_account.as_ref());
    out.push(b'|');
    push_hex(&mut out, destination_token_account.as_ref());
    out.push(b'|');
    push_hex(&mut out, &amount.to_le_bytes());
    out.push(b'|');
    push_hex(&mut out, &nonce.to_le_bytes());
    out.push(b'|');
    push_hex(&mut out, &auth_expiry_ts.to_le_bytes());
    out
}

fn push_hex(out: &mut Vec<u8>, bytes: &[u8]) {
    for &b in bytes {
        out.push(nibble_to_hex(b >> 4));
        out.push(nibble_to_hex(b & 0x0f));
    }
}

fn nibble_to_hex(n: u8) -> u8 {
    match n {
        0..=9 => b'0' + n,
        _ => b'a' + (n - 10),
    }
}

pub fn encode_message_base64(message: &[u8]) -> String {
    B64.encode(message)
}

pub fn decode_base64(data: &str) -> Result<Vec<u8>, String> {
    B64.decode(data.as_bytes()).map_err(|e| e.to_string())
}

pub fn parse_nonce_from_state(data: &[u8]) -> Result<u64, String> {
    if data.len() < 164 {
        return Err("delegation account data too small".to_string());
    }
    let bytes: [u8; 8] = data[156..164].try_into().unwrap();
    Ok(u64::from_le_bytes(bytes))
}

pub fn instruction_to_js(ix: &Instruction) -> JsInstruction {
    JsInstruction {
        program_id: ix.program_id.to_string(),
        keys: ix
            .accounts
            .iter()
            .map(|meta| JsAccountMeta {
                pubkey: meta.pubkey.to_string(),
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            })
            .collect(),
        data: B64.encode(&ix.data),
    }
}

pub fn associated_token_address(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    let ata_program = Pubkey::from_str(ASSOCIATED_TOKEN_PROGRAM_ID).unwrap();
    let token_program = Pubkey::from_str(TOKEN_PROGRAM_ID).unwrap();
    Pubkey::find_program_address(
        &[owner.as_ref(), token_program.as_ref(), mint.as_ref()],
        &ata_program,
    )
    .0
}

fn build_webauthn_message(authenticator_data: &[u8], client_data_json: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(client_data_json);
    let client_hash = hasher.finalize();

    let mut out = Vec::with_capacity(authenticator_data.len() + 32);
    out.extend_from_slice(authenticator_data);
    out.extend_from_slice(&client_hash);
    out
}

fn build_secp256r1_instruction(
    pubkey: &[u8],
    signature: &[u8],
    message: &[u8],
) -> Result<Instruction, String> {
    let pubkey: &[u8; 33] = pubkey.try_into().map_err(|_| "pubkey must be 33 bytes".to_string())?;
    let signature: &[u8; 64] = signature.try_into().map_err(|_| "signature must be 64 bytes".to_string())?;

    let header_len = 2usize;
    let offsets_len = 14usize;
    let public_key_offset = (header_len + offsets_len) as u16;
    let signature_offset = public_key_offset + 33;
    let message_offset = signature_offset + 64;

    let msg_len: u16 = message
        .len()
        .try_into()
        .map_err(|_| "message too long".to_string())?;

    let mut data = Vec::with_capacity(header_len + offsets_len + 33 + 64 + message.len());
    data.push(1u8);
    data.push(0u8);
    data.extend_from_slice(&signature_offset.to_le_bytes());
    data.extend_from_slice(&u16::MAX.to_le_bytes());
    data.extend_from_slice(&public_key_offset.to_le_bytes());
    data.extend_from_slice(&u16::MAX.to_le_bytes());
    data.extend_from_slice(&message_offset.to_le_bytes());
    data.extend_from_slice(&msg_len.to_le_bytes());
    data.extend_from_slice(&u16::MAX.to_le_bytes());
    data.extend_from_slice(pubkey);
    data.extend_from_slice(signature);
    data.extend_from_slice(message);

    Ok(Instruction {
        program_id: Pubkey::from_str(SECP256R1_PROGRAM_ID).unwrap(),
        accounts: vec![],
        data,
    })
}

fn build_token_approve_data(amount: u64) -> Vec<u8> {
    // SPL Token Approve instruction = 4
    let mut data = Vec::with_capacity(1 + 8);
    data.push(4);
    data.extend_from_slice(&amount.to_le_bytes());
    data
}

fn build_create_ata_idempotent_ix(
    payer: &Pubkey,
    owner: &Pubkey,
    ata: &Pubkey,
    mint: &Pubkey,
) -> Instruction {
    let ata_program = Pubkey::from_str(ASSOCIATED_TOKEN_PROGRAM_ID).unwrap();
    let token_program = Pubkey::from_str(TOKEN_PROGRAM_ID).unwrap();
    Instruction {
        program_id: ata_program,
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new(*ata, false),
            AccountMeta::new_readonly(*owner, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(token_program, false),
        ],
        data: vec![1u8], // CreateIdempotent
    }
}
