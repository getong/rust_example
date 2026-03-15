use sha3::{Digest, Keccak256};
use sui_sdk::{
  SuiClient, SuiClientBuilder,
  rpc_types::{SuiObjectDataOptions, SuiRawData},
  types::{
    base_types::ObjectID,
    object::Owner,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{ObjectArg, SharedObjectMutability},
  },
};
use wormhole_sui_sdk::{
  bytes32::Bytes32,
  external_address::ExternalAddress,
  vaa::VAA,
};

const DEFAULT_SUI_RPC_URL: &str = "https://fullnode.mainnet.sui.io:443";

#[derive(Debug, Clone)]
struct SignedVaaSignature {
  guardian_index: u8,
  recovery_id: u8,
  r: [u8; 32],
  s: [u8; 32],
}

#[derive(Debug, Clone)]
struct ParsedSignedVaa {
  version: u8,
  guardian_set_index: u32,
  signatures: Vec<SignedVaaSignature>,
  timestamp: u32,
  nonce: u32,
  emitter_chain: u16,
  emitter_address: [u8; 32],
  sequence: u64,
  consistency_level: u8,
  payload: Vec<u8>,
  body: Vec<u8>,
  raw_bytes: Vec<u8>,
  digest: [u8; 32],
}

fn optional_env(name: &str) -> Option<String> {
  std::env::var(name)
    .ok()
    .map(|value| value.trim().to_owned())
    .filter(|value| !value.is_empty())
}

fn encode_hex_prefixed(bytes: &[u8]) -> String {
  format!("0x{}", hex::encode(bytes))
}

fn parse_u8(bytes: &[u8], cursor: &mut usize) -> anyhow::Result<u8> {
  if *cursor + 1 > bytes.len() {
    anyhow::bail!("unexpected end of VAA while reading u8 at offset {}", *cursor);
  }
  let value = bytes[*cursor];
  *cursor += 1;
  Ok(value)
}

fn parse_array<const N: usize>(bytes: &[u8], cursor: &mut usize) -> anyhow::Result<[u8; N]> {
  if *cursor + N > bytes.len() {
    anyhow::bail!(
      "unexpected end of VAA while reading {N} bytes at offset {}",
      *cursor
    );
  }
  let value: [u8; N] = bytes[*cursor..*cursor + N]
    .try_into()
    .map_err(|_| anyhow::anyhow!("failed to read {N} bytes from VAA"))?;
  *cursor += N;
  Ok(value)
}

fn parse_u16_be(bytes: &[u8], cursor: &mut usize) -> anyhow::Result<u16> {
  Ok(u16::from_be_bytes(parse_array(bytes, cursor)?))
}

fn parse_u32_be(bytes: &[u8], cursor: &mut usize) -> anyhow::Result<u32> {
  Ok(u32::from_be_bytes(parse_array(bytes, cursor)?))
}

fn parse_u64_be(bytes: &[u8], cursor: &mut usize) -> anyhow::Result<u64> {
  Ok(u64::from_be_bytes(parse_array(bytes, cursor)?))
}

fn double_keccak(bytes: &[u8]) -> [u8; 32] {
  let first = Keccak256::digest(bytes);
  let second = Keccak256::digest(first);
  second.into()
}

fn bytes32(bytes: [u8; 32]) -> Bytes32 {
  Bytes32::new(bytes.to_vec().into())
}

fn parse_signed_vaa(raw_bytes: Vec<u8>) -> anyhow::Result<ParsedSignedVaa> {
  let mut cursor = 0usize;

  let version = parse_u8(&raw_bytes, &mut cursor)?;
  let guardian_set_index = parse_u32_be(&raw_bytes, &mut cursor)?;
  let signatures_len = parse_u8(&raw_bytes, &mut cursor)? as usize;

  let mut signatures = Vec::with_capacity(signatures_len);
  for _ in 0..signatures_len {
    let guardian_index = parse_u8(&raw_bytes, &mut cursor)?;
    let r = parse_array::<32>(&raw_bytes, &mut cursor)?;
    let s = parse_array::<32>(&raw_bytes, &mut cursor)?;
    let recovery_id = parse_u8(&raw_bytes, &mut cursor)?;
    signatures.push(SignedVaaSignature {
      guardian_index,
      recovery_id,
      r,
      s,
    });
  }

  if raw_bytes.len() < cursor + 51 {
    anyhow::bail!(
      "signed VAA is too short: expected at least {} bytes, got {}",
      cursor + 51,
      raw_bytes.len()
    );
  }

  let body = raw_bytes[cursor..].to_vec();
  let mut body_cursor = 0usize;
  let timestamp = parse_u32_be(&body, &mut body_cursor)?;
  let nonce = parse_u32_be(&body, &mut body_cursor)?;
  let emitter_chain = parse_u16_be(&body, &mut body_cursor)?;
  let emitter_address = parse_array::<32>(&body, &mut body_cursor)?;
  let sequence = parse_u64_be(&body, &mut body_cursor)?;
  let consistency_level = parse_u8(&body, &mut body_cursor)?;
  let payload = body[body_cursor..].to_vec();
  let digest = double_keccak(&body);

  Ok(ParsedSignedVaa {
    version,
    guardian_set_index,
    signatures,
    timestamp,
    nonce,
    emitter_chain,
    emitter_address,
    sequence,
    consistency_level,
    payload,
    body,
    raw_bytes,
    digest,
  })
}

impl ParsedSignedVaa {
  fn into_sdk_vaa(self) -> VAA {
    VAA::new(
      self.guardian_set_index,
      self.timestamp,
      self.nonce,
      self.emitter_chain,
      ExternalAddress::new(bytes32(self.emitter_address)),
      self.sequence,
      self.consistency_level,
      self.payload.into(),
      bytes32(self.digest),
    )
  }
}

async fn fetch_object_arg(
  sui: &SuiClient,
  object_id: ObjectID,
  is_mutable: bool,
) -> anyhow::Result<ObjectArg> {
  let response = sui
    .read_api()
    .get_object_with_options(object_id, SuiObjectDataOptions::new().with_owner())
    .await?;

  let object = response
    .data
    .ok_or_else(|| anyhow::anyhow!("object {object_id} not found"))?;
  let object_ref = object.object_ref();
  let owner = object
    .owner
    .ok_or_else(|| anyhow::anyhow!("owner metadata missing for object {object_id}"))?;

  let object_arg = match owner {
    Owner::Shared {
      initial_shared_version,
    } => ObjectArg::SharedObject {
      id: object_id,
      initial_shared_version,
      mutability: if is_mutable {
        SharedObjectMutability::Mutable
      } else {
        SharedObjectMutability::Immutable
      },
    },
    Owner::ConsensusAddressOwner { start_version, .. } => ObjectArg::SharedObject {
      id: object_id,
      initial_shared_version: start_version,
      mutability: if is_mutable {
        SharedObjectMutability::Mutable
      } else {
        SharedObjectMutability::Immutable
      },
    },
    Owner::AddressOwner(_) | Owner::ObjectOwner(_) | Owner::Immutable => {
      ObjectArg::ImmOrOwnedObject(object_ref)
    }
  };

  Ok(object_arg)
}

async fn fetch_vaa(
  sui: &SuiClient,
  object_id: ObjectID,
) -> anyhow::Result<(sui_sdk::types::base_types::SequenceNumber, String, VAA)> {
  let response = sui
    .read_api()
    .get_object_with_options(object_id, SuiObjectDataOptions::bcs_lossless())
    .await?;

  let object = response
    .data
    .ok_or_else(|| anyhow::anyhow!("object {object_id} not found"))?;
  let bcs = object
    .bcs
    .ok_or_else(|| anyhow::anyhow!("BCS data missing for object {object_id}"))?;

  let SuiRawData::MoveObject(raw_move) = bcs else {
    anyhow::bail!("object {object_id} is a package, not a Move object");
  };

  let type_name = raw_move.type_.to_string();
  let version = raw_move.version;
  let vaa = raw_move.deserialize::<VAA>()?;
  Ok((version, type_name, vaa))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let rpc_url = std::env::var("SUI_RPC_URL").unwrap_or_else(|_| DEFAULT_SUI_RPC_URL.to_owned());
  let wormhole_pkg_id = optional_env("WORMHOLE_PACKAGE_ID");
  let state_object_id = optional_env("WORMHOLE_STATE_OBJECT_ID");
  let vaa_hex = optional_env("WORMHOLE_VAA_HEX");
  let vaa_object_id = optional_env("WORMHOLE_VAA_OBJECT_ID");
  let state_is_mutable = std::env::var("WORMHOLE_STATE_MUTABLE")
    .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "True"))
    .unwrap_or(false);

  let sui = SuiClientBuilder::default()
    .build(&rpc_url)
    .await?;

  let wants_decode = vaa_object_id.is_some();
  let wants_signed_vaa_decode = vaa_hex.is_some();
  let wants_ptb = wormhole_pkg_id.is_some() || state_object_id.is_some();

  if !wants_decode && !wants_signed_vaa_decode && !wants_ptb {
    anyhow::bail!(
      "nothing to do. Set WORMHOLE_VAA_OBJECT_ID to decode an on-chain VAA object, or set WORMHOLE_VAA_HEX to decode a signed VAA and optionally combine it with WORMHOLE_PACKAGE_ID and WORMHOLE_STATE_OBJECT_ID to build a verify_vaa PTB."
    );
  }

  if let Some(vaa_object_id) = vaa_object_id {
    let (version, type_name, vaa) = fetch_vaa(&sui, vaa_object_id.parse()?).await?;
    println!("Decoded Wormhole VAA object {vaa_object_id} from {rpc_url}:");
    println!("  version: {version}");
    println!("  type: {type_name}");
    println!("  guardian_set_index: {}", vaa.guardian_set_index);
    println!("  timestamp: {}", vaa.timestamp);
    println!("  nonce: {}", vaa.nonce);
    println!("  emitter_chain: {}", vaa.emitter_chain);
    println!("  sequence: {}", vaa.sequence);
    println!("  consistency_level: {}", vaa.consistency_level);
    println!("  payload_len: {}", vaa.payload.len());
  }

  let parsed_signed_vaa = if let Some(vaa_hex) = vaa_hex.clone() {
    let parsed = parse_signed_vaa(hex::decode(vaa_hex)?)?;
    let signature_count = parsed.signatures.len();
    let first_guardian_index = parsed.signatures.first().map(|sig| sig.guardian_index);
    let first_recovery_id = parsed.signatures.first().map(|sig| sig.recovery_id);
    let first_signature_prefix = parsed.signatures.first().map(|sig| {
      format!(
        "{}{}",
        hex::encode(&sig.r[..4]),
        hex::encode(&sig.s[..4]),
      )
    });
    let body_len = parsed.body.len();
    let raw_len = parsed.raw_bytes.len();
    let sdk_vaa = parsed.clone().into_sdk_vaa();

    println!("Decoded signed Wormhole VAA from hex:");
    println!("  version: {}", parsed.version);
    println!("  guardian_set_index: {}", sdk_vaa.guardian_set_index);
    println!("  signatures: {signature_count}");
    println!("  emitter_chain: {}", sdk_vaa.emitter_chain);
    println!(
      "  emitter_address: {}",
      encode_hex_prefixed(&parsed.emitter_address)
    );
    println!("  sequence: {}", sdk_vaa.sequence);
    println!("  nonce: {}", sdk_vaa.nonce);
    println!("  timestamp: {}", sdk_vaa.timestamp);
    println!("  consistency_level: {}", sdk_vaa.consistency_level);
    println!("  payload_len: {}", sdk_vaa.payload.len());
    println!("  body_len: {body_len}");
    println!("  raw_len: {raw_len}");
    println!("  digest: {}", encode_hex_prefixed(&parsed.digest));
    if let Some(first_guardian_index) = first_guardian_index {
      println!("  first_signature.guardian_index: {first_guardian_index}");
    }
    if let Some(first_recovery_id) = first_recovery_id {
      println!("  first_signature.recovery_id: {first_recovery_id}");
    }
    if let Some(first_signature_prefix) = first_signature_prefix {
      println!("  first_signature.prefix: {first_signature_prefix}");
    }

    Some(parsed)
  } else {
    None
  };

  if wants_ptb {
    let wormhole_pkg_id = wormhole_pkg_id
      .ok_or_else(|| anyhow::anyhow!("missing environment variable WORMHOLE_PACKAGE_ID"))?;
    let state_object_id = state_object_id
      .ok_or_else(|| anyhow::anyhow!("missing environment variable WORMHOLE_STATE_OBJECT_ID"))?;
    let parsed_signed_vaa = parsed_signed_vaa
      .ok_or_else(|| anyhow::anyhow!("missing environment variable WORMHOLE_VAA_HEX"))?;

    let mut builder = ProgrammableTransactionBuilder::new();

    let state_object_arg =
      fetch_object_arg(&sui, state_object_id.parse()?, state_is_mutable).await?;
    let state_arg = builder.obj(state_object_arg)?;
    let vaa_arg = builder.pure(parsed_signed_vaa.raw_bytes.clone())?;

    builder.programmable_move_call(
      wormhole_pkg_id.parse()?,
      "vaa".parse()?,
      "verify_vaa".parse()?,
      vec![],
      vec![state_arg, vaa_arg],
    );

    let _ptb = builder.finish();

    // let tx_data = TransactionData::new_programmable(...);
    // let response = sui.quorum_driver_api().execute_transaction_block(...).await?;

    println!(
      "Wormhole PTB built successfully against {rpc_url} using package {wormhole_pkg_id} and state object {state_object_id}."
    );
    println!(
      "Generated verify_vaa arguments: state object {state_object_id}, signed VAA bytes {} bytes.",
      parsed_signed_vaa.raw_bytes.len()
    );
  }

  Ok(())
}
