use ark_bn254::{Bn254, Fr};
use ark_ed_on_bn254::{EdwardsConfig, Fr as EmbeddedScalarField};
use ark_ff::UniformRand;
use ark_mpc::{
  PARTY0, PARTY1,
  algebra::Scalar,
  test_helpers::{TestCurve, execute_mock_mpc},
};
use ark_std::rand::{SeedableRng, rngs::StdRng};
use jf_primitives::elgamal::KeyPair;
use mpc_plonk::proof_system::structs::Proof;
use mpc_relation::{PlonkCircuit, errors::CircuitError, traits::Circuit};

type SystemProof = Proof<Bn254>;
type SystemScalar = Scalar<TestCurve>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  println!("jellyfish / ark-mpc mini example");

  run_relation_example()?;
  run_elgamal_example();
  run_mock_mpc_example().await?;
  describe_plonk_type();

  Ok(())
}

fn run_relation_example() -> Result<(), CircuitError> {
  let mut circuit = PlonkCircuit::<Fr>::new_turbo_plonk();

  let lhs = circuit.create_variable(Fr::from(6_u64))?;
  let rhs = circuit.create_variable(Fr::from(7_u64))?;
  let product = circuit.mul(lhs, rhs)?;
  let expected = circuit.create_variable(Fr::from(42_u64))?;

  circuit.enforce_equal(product, expected)?;
  circuit.check_circuit_satisfiability(&[])?;

  println!(
    "mpc-relation: built and checked 6 * 7 == 42, gates={}, vars={}",
    circuit.num_gates(),
    circuit.num_vars()
  );

  Ok(())
}

fn run_elgamal_example() {
  let mut rng = StdRng::seed_from_u64(7);
  let keypair = KeyPair::<EdwardsConfig>::generate(&mut rng);
  let plaintext = vec![Fr::from(11_u64), Fr::from(22_u64), Fr::from(33_u64)];
  let randomness = EmbeddedScalarField::rand(&mut rng);

  let ciphertext = keypair
    .enc_key()
    .deterministic_encrypt(randomness, &plaintext);
  let decrypted = keypair.dec_key().decrypt(&ciphertext);

  assert_eq!(decrypted, plaintext);
  println!(
    "jf-primitives: encrypted/decrypted {} BN254 field elements",
    plaintext.len()
  );
}

async fn run_mock_mpc_example() -> Result<(), Box<dyn std::error::Error>> {
  let a = SystemScalar::new(Fr::from(19_u64));
  let b = SystemScalar::new(Fr::from(23_u64));

  let (party0_output, party1_output) = execute_mock_mpc(move |fabric| async move {
    let a_shared = fabric.share_scalar(a, PARTY0);
    let b_shared = fabric.share_scalar(b, PARTY1);

    (a_shared + b_shared).open_authenticated().await
  })
  .await;

  let opened0 = party0_output?;
  let opened1 = party1_output?;
  let expected = SystemScalar::new(Fr::from(42_u64));

  assert_eq!(opened0, expected);
  assert_eq!(opened1, expected);
  println!(
    "ark-mpc: both mock parties opened 19 + 23 = {}",
    opened0.inner()
  );

  Ok(())
}

fn describe_plonk_type() {
  println!(
    "mpc-plonk: proof type wired for BN254 is {}",
    std::any::type_name::<SystemProof>()
  );
}
