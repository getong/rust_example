use std::{
  collections::HashMap,
  env, fs,
  path::{Path, PathBuf},
  sync::OnceLock,
};

use alloy::{primitives::Address, providers::Provider};
use eyre::Result;
use serde::Deserialize;
use tokio::time::{Duration, timeout};

pub mod abi_encode;
pub mod account;
pub mod account_error;
pub mod array;
pub mod array_remove_by_shifting;
pub mod array_replace_from_end;
pub mod assembly_error;
pub mod assembly_if;
pub mod assembly_loop;
pub mod assembly_math;
pub mod assembly_variable;
pub mod bitwise_ops;
pub mod callback;
pub mod calling_contract;
pub mod constants;
pub mod counter;
pub mod data_locations;
pub mod enum_basic;
pub mod enum_declaration_example;
pub mod enum_import;
pub mod ether_units;
pub mod ether_wallet;
pub mod event_basic;
pub mod event_driven_architecture;
pub mod event_subscription;
pub mod examples;
pub mod fallback_contract;
pub mod function_contract;
pub mod function_modifier;
pub mod function_selector;
pub mod gas;
pub mod gas_golf;
pub mod hash_function;
pub mod if_else;
pub mod immutable;
pub mod iterable_mapping;
pub mod loop_contract;
pub mod malicious_callback;
pub mod mapping;
pub mod multi_sig_wallet;
pub mod nested_mapping;
pub mod new_contract;
pub mod payable_contract;
pub mod primitives;
pub mod reentrancy_guard;
pub mod reentrancy_guard_transient;
pub mod sending_ether;
pub mod simple_storage;
pub mod struct_import_example;
pub mod test_contract;
pub mod test_merkle_proof;
pub mod test_storage;
pub mod test_transient_storage;
pub mod todos_struct_declaration;
pub mod todos_structs;
pub mod unchecked_math;
pub mod variables;
pub mod verify_signature;
pub mod view_and_pure;
pub mod visibility;
pub mod xyz;

type DeploymentMap = HashMap<String, Vec<Address>>;

static DEPLOYMENTS: OnceLock<Result<DeploymentMap, String>> = OnceLock::new();

#[derive(Deserialize)]
struct DeploymentManifest {
  contracts: HashMap<String, String>,
}

macro_rules! deployed_contract {
  ($provider:expr, $ty:ident, $key:expr, $label:literal) => {{
    match crate::contracts::deployed_address_or_skip($key, $label)? {
      Some(address) => {
        let contract = $ty::new(address, $provider);
        println!("[{}] using deployed: {}", $label, contract.address());
        Some(contract)
      }
      None => None,
    }
  }};
}

pub(crate) use deployed_contract;

pub(crate) fn deployed_address_or_skip(contract_id: &str, label: &str) -> Result<Option<Address>> {
  match deployed_address(contract_id)? {
    Some(address) => Ok(Some(address)),
    None => {
      println!("[skip] {label}: no deployed address found for contract id `{contract_id}`");
      Ok(None)
    }
  }
}

fn deployed_address(contract_id: &str) -> Result<Option<Address>> {
  let env_key = deployment_env_key(contract_id);
  if let Ok(raw) = env::var(&env_key) {
    return Ok(Some(raw.parse()?));
  }

  let deployments = DEPLOYMENTS
    .get_or_init(|| load_deployments().map_err(|err| err.to_string()))
    .as_ref()
    .map_err(|err| eyre::eyre!(err.clone()))?;

  Ok(
    deployments
      .get(contract_id)
      .and_then(|items| items.first())
      .copied(),
  )
}

fn deployment_env_key(contract_id: &str) -> String {
  let sanitized = contract_id
    .chars()
    .map(|ch| {
      if ch.is_ascii_alphanumeric() {
        ch.to_ascii_uppercase()
      } else {
        '_'
      }
    })
    .collect::<String>();
  format!("CONTRACT_ADDRESS_{sanitized}")
}

fn load_deployments() -> Result<DeploymentMap> {
  let path = deployments_manifest_path();
  if !path.exists() {
    return Ok(HashMap::new());
  }

  let content = fs::read_to_string(&path)?;
  let manifest: DeploymentManifest = serde_yaml::from_str(&content)?;

  let mut deployments = HashMap::new();
  for (contract_id, contract_address) in manifest.contracts {
    deployments
      .entry(contract_id)
      .or_insert_with(Vec::new)
      .push(contract_address.parse()?);
  }

  Ok(deployments)
}

fn deployments_manifest_path() -> PathBuf {
  if let Ok(path) = env::var("DEPLOYMENTS_YAML_PATH") {
    return PathBuf::from(path);
  }

  if let Ok(path) = env::var("DEPLOY_BROADCAST_PATH") {
    return PathBuf::from(path);
  }

  Path::new(env!("CARGO_MANIFEST_DIR")).join("deployments-latest.yaml")
}

fn fail_on_module_error() -> bool {
  matches!(
    env::var("FAIL_ON_MODULE_ERROR").ok().as_deref(),
    Some("1" | "true" | "TRUE" | "yes" | "YES")
  )
}

pub async fn run_all(provider: &impl Provider) -> Result<()> {
  let mut failures = Vec::new();

  macro_rules! run_step {
    ($name:literal, $step:expr) => {
      match timeout(Duration::from_secs(30), $step).await {
        Ok(Ok(())) => println!("[ok] {}", $name),
        Ok(Err(err)) => {
          eprintln!("[failed] {}: {err:#}", $name);
          failures.push($name);
        }
        Err(_) => {
          eprintln!("[failed] {}: timed out after 30s", $name);
          failures.push($name);
        }
      }
    };
  }

  run_step!("AbiEncode", abi_encode::run(provider));
  run_step!("Account.sol::Error", account_error::run(provider));
  run_step!("Array", array::run(provider));
  run_step!(
    "ArrayRemoveByShifting",
    array_remove_by_shifting::run(provider)
  );
  run_step!("ArrayReplaceFromEnd", array_replace_from_end::run(provider));
  run_step!("AssemblyError", assembly_error::run(provider));
  run_step!("AssemblyIf", assembly_if::run(provider));
  run_step!("AssemblyLoop", assembly_loop::run(provider));
  run_step!("AssemblyMath", assembly_math::run(provider));
  run_step!("AssemblyVariable", assembly_variable::run(provider));
  run_step!("BitwiseOps", bitwise_ops::run(provider));
  run_step!("CallingContract", calling_contract::run(provider));
  run_step!("Constants", constants::run(provider));
  run_step!("Counter", counter::run(provider));
  run_step!("DataLocations", data_locations::run(provider));
  run_step!("EtherWallet", ether_wallet::run(provider));
  run_step!("Primitives", primitives::run(provider));
  run_step!("Enum.sol::Enum", enum_basic::run(provider));
  run_step!(
    "EnumDeclarationExample",
    enum_declaration_example::run(provider)
  );
  run_step!("EnumImport.sol::Enum", enum_import::run(provider));
  run_step!("Error.sol::Account", account::run(provider));
  run_step!("EtherUnits", ether_units::run(provider));
  run_step!("Events.sol::Event", event_basic::run(provider));
  run_step!(
    "EventDrivenArchitecture",
    event_driven_architecture::run(provider)
  );
  run_step!("EventSubscription", event_subscription::run(provider));
  run_step!("Fallback", fallback_contract::run(provider));
  run_step!("Function", function_contract::run(provider));
  run_step!("XYZ", xyz::run(provider));
  run_step!("FunctionModifier", function_modifier::run(provider));
  run_step!("FunctionSelector", function_selector::run(provider));
  run_step!("Gas", gas::run(provider));
  run_step!("GasGolf", gas_golf::run(provider));
  run_step!("Keccak256", hash_function::run(provider));
  run_step!("IfElse", if_else::run(provider));
  run_step!("Immutable", immutable::run(provider));
  run_step!("IterableMapping", iterable_mapping::run(provider));
  run_step!("Loop", loop_contract::run(provider));
  run_step!("Mapping", mapping::run(provider));
  run_step!("MultiSigWallet", multi_sig_wallet::run(provider));
  run_step!("NewContract", new_contract::run(provider));
  run_step!("NestedMapping", nested_mapping::run(provider));
  run_step!("Payable", payable_contract::run(provider));
  run_step!("SimpleStorage", simple_storage::run(provider));
  run_step!("SendingEther", sending_ether::run(provider));
  run_step!("Variables", variables::run(provider));
  run_step!(
    "StructDeclaration.sol::Todos",
    todos_struct_declaration::run(provider)
  );
  run_step!("StructImportExample", struct_import_example::run(provider));
  run_step!("Structs.sol::Todos", todos_structs::run(provider));
  run_step!("Callback", callback::run(provider));
  run_step!("TestStorage", test_storage::run(provider));
  run_step!("TestContract", test_contract::run(provider));
  run_step!("TestMerkleProof", test_merkle_proof::run(provider));
  run_step!(
    "TestTransientStorage",
    test_transient_storage::run(provider)
  );
  run_step!("MaliciousCallback", malicious_callback::run(provider));
  run_step!("ReentrancyGuard", reentrancy_guard::run(provider));
  run_step!(
    "ReentrancyGuardTransient",
    reentrancy_guard_transient::run(provider)
  );
  run_step!("Examples", examples::run(provider));
  run_step!("UncheckedMath", unchecked_math::run(provider));
  run_step!("VerifySignature", verify_signature::run(provider));
  run_step!("Visibility", visibility::run(provider));
  run_step!("ViewAndPure", view_and_pure::run(provider));

  if failures.is_empty() {
    Ok(())
  } else {
    let summary = format!(
      "{} module(s) failed: {}",
      failures.len(),
      failures.join(", ")
    );
    eprintln!("[summary] {summary}");

    if fail_on_module_error() {
      eyre::bail!("{summary}");
    }

    eprintln!(
      "[summary] continuing despite module failures. Set FAIL_ON_MODULE_ERROR=1 to exit with an \
       error."
    );
    Ok(())
  }
}
