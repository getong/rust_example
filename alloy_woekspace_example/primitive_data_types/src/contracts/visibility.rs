use alloy::{providers::Provider, sol};
use eyre::Result;

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Base,
  "abi/Base.json"
);

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  Child,
  "abi/Child.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(base) = super::deployed_contract!(provider, Base, "Base", "Visibility::Base") else {
    return Ok(());
  };
  let Some(child) = super::deployed_contract!(provider, Child, "Child", "Visibility::Child") else {
    return Ok(());
  };
  println!(
    "[Visibility] base: {}, child: {}",
    base.address(),
    child.address()
  );

  let private_msg = base.testPrivateFunc().call().await?;
  let internal_msg = base.testInternalFunc().call().await?;
  let public_msg = base.publicFunc().call().await?;
  let child_internal_msg = child.testInternalFunc().call().await?;
  println!(
    "[Visibility] private={private_msg}, internal={internal_msg}, public={public_msg}, \
     child_internal={child_internal_msg}"
  );
  Ok(())
}
