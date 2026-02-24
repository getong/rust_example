import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SingleCounterProgram } from "../target/types/single_counter_program";
import { expect } from "chai";

describe("single_counter_program", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.counterProgram as Program<CounterProgram>;
  const user = provider.wallet;

  const [counterPda] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("counter")],
    program.programId,
  );

  it("First increment initializes shared counter and sets it to 1", async () => {
    await program.methods
      .increment()
      .accounts({
        user: user.publicKey,
      })
      .rpc();

    const counterAccount = await program.account.counter.fetch(counterPda);
    expect(counterAccount.count.toNumber()).to.equal(1);
  });

  it("A second user can operate on the same counter", async () => {
    const secondUser = anchor.web3.Keypair.generate();

    await program.methods
      .increment()
      .accounts({
        user: secondUser.publicKey,
      })
      .signers([secondUser])
      .rpc();

    const counterAccount = await program.account.counter.fetch(counterPda);
    expect(counterAccount.count.toNumber()).to.equal(2);
  });

  it("Decrements the shared counter", async () => {
    await program.methods
      .decrement()
      .accounts({
        user: user.publicKey,
      })
      .rpc();

    const counterAccount = await program.account.counter.fetch(counterPda);
    expect(counterAccount.count.toNumber()).to.equal(1);
  });
});
