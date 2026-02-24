import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SingleCounterProgram } from "../target/types/single_counter_program";
import { expect } from "chai";

describe("single_counter_program", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.singleCounterProgram as Program<SingleCounterProgram>;
  const user = provider.wallet;

  const [counterPda] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("counter")],
    program.programId,
  );

  it("Initializes shared counter to 0", async () => {
    await program.methods
      .initialize()
      .accounts({
        counter: counterPda,
        user: user.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    const counterAccount = await program.account.counter.fetch(counterPda);
    expect(counterAccount.count.toNumber()).to.equal(0);
  });

  it("Increments the shared counter", async () => {
    await program.methods
      .increment()
      .accounts({
        counter: counterPda,
        user: user.publicKey,
      })
      .rpc();

    const counterAccount = await program.account.counter.fetch(counterPda);
    expect(counterAccount.count.toNumber()).to.equal(1);
  });

  it("A second user can call initialize and operate on the same counter", async () => {
    const secondUser = anchor.web3.Keypair.generate();

    await program.methods
      .initialize()
      .accounts({
        counter: counterPda,
        user: secondUser.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([secondUser])
      .rpc();

    await program.methods
      .increment()
      .accounts({
        counter: counterPda,
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
        counter: counterPda,
        user: user.publicKey,
      })
      .rpc();

    const counterAccount = await program.account.counter.fetch(counterPda);
    expect(counterAccount.count.toNumber()).to.equal(1);
  });
});
