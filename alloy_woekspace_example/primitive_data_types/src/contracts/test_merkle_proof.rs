use alloy::{
  primitives::{U256, b256},
  providers::Provider,
  sol,
};
use eyre::{Result, ensure};

sol!(
  #[allow(missing_docs)]
  #[sol(rpc)]
  TestMerkleProof,
  "abi/TestMerkleProof.json"
);

pub async fn run(provider: &impl Provider) -> Result<()> {
  let Some(contract) = super::deployed_contract!(
    provider,
    TestMerkleProof,
    "TestMerkleProof",
    "TestMerkleProof"
  ) else {
    return Ok(());
  };

  let root = contract.getRoot().call().await?;
  let leaf = b256!("0xdca3326ad7e8121bf9cf9c12333e6b2271abe823ec9edfe42f813b1e768fa57b");
  let proof = vec![
    b256!("0x8da9e1c820f9dbd1589fd6585872bc1063588625729e7ab0797cfc63a00bd950"),
    b256!("0x995788ffc103b987ad50f5e5707fd094419eb12d9552cc423bd0cd86a3861433"),
  ];
  let verified = contract
    .verify(proof, root, leaf, U256::from(2_u64))
    .call()
    .await?;
  ensure!(verified, "expected merkle proof verification to succeed");
  println!("[TestMerkleProof] root={root}, verified={verified}");
  Ok(())
}
