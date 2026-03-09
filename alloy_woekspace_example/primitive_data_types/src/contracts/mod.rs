use alloy::providers::Provider;
use eyre::Result;
use tokio::time::{Duration, timeout};

pub mod account;
pub mod account_error;
pub mod array;
pub mod array_remove_by_shifting;
pub mod array_replace_from_end;
pub mod callback;
pub mod constants;
pub mod counter;
pub mod data_locations;
pub mod enum_basic;
pub mod enum_declaration_example;
pub mod enum_import;
pub mod ether_units;
pub mod event_basic;
pub mod event_driven_architecture;
pub mod event_subscription;
pub mod examples;
pub mod function_contract;
pub mod function_modifier;
pub mod gas;
pub mod if_else;
pub mod immutable;
pub mod loop_contract;
pub mod malicious_callback;
pub mod mapping;
pub mod nested_mapping;
pub mod primitives;
pub mod reentrancy_guard;
pub mod reentrancy_guard_transient;
pub mod simple_storage;
pub mod struct_import_example;
pub mod test_storage;
pub mod test_transient_storage;
pub mod todos_struct_declaration;
pub mod todos_structs;
pub mod variables;
pub mod view_and_pure;
pub mod xyz;

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

  run_step!("Account.sol::Error", account_error::run(provider));
  run_step!("Array", array::run(provider));
  run_step!(
    "ArrayRemoveByShifting",
    array_remove_by_shifting::run(provider)
  );
  run_step!("ArrayReplaceFromEnd", array_replace_from_end::run(provider));
  run_step!("Constants", constants::run(provider));
  run_step!("Counter", counter::run(provider));
  run_step!("DataLocations", data_locations::run(provider));
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
  run_step!("Function", function_contract::run(provider));
  run_step!("XYZ", xyz::run(provider));
  run_step!("FunctionModifier", function_modifier::run(provider));
  run_step!("Gas", gas::run(provider));
  run_step!("IfElse", if_else::run(provider));
  run_step!("Immutable", immutable::run(provider));
  run_step!("Loop", loop_contract::run(provider));
  run_step!("Mapping", mapping::run(provider));
  run_step!("NestedMapping", nested_mapping::run(provider));
  run_step!("SimpleStorage", simple_storage::run(provider));
  run_step!("Variables", variables::run(provider));
  run_step!(
    "StructDeclaration.sol::Todos",
    todos_struct_declaration::run(provider)
  );
  run_step!("StructImportExample", struct_import_example::run(provider));
  run_step!("Structs.sol::Todos", todos_structs::run(provider));
  run_step!("Callback", callback::run(provider));
  run_step!("TestStorage", test_storage::run(provider));
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
  run_step!("ViewAndPure", view_and_pure::run(provider));

  if failures.is_empty() {
    Ok(())
  } else {
    eyre::bail!(
      "{} module(s) failed: {}",
      failures.len(),
      failures.join(", ")
    );
  }
}
