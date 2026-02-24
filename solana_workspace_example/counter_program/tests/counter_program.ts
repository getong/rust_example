import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { CounterProgram } from "../target/types/counter_program";
import { expect } from "chai";
describe("counter_program", () => {
  // Configure the client
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.CounterProgram as Program<CounterProgram>;
  const user = provider.wallet;
  // Generate a new keypair for our counter account
  const counterKeypair = anchor.web3.Keypair.generate();
  it("Initializes with count 0", async () => {
    // Call initialize instruction
    await program.methods
      .initialize()
      .accounts({
        counter: counterKeypair.publicKey,
        user: user.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
    })
      .signers([counterKeypair])
      .rpc();
    // Fetch the account and check the count
    const counterAccount = await program.account.counter.fetch(counterKeypair.publicKey);
    expect(counterAccount.count.toNumber()).to.equal(0);
    expect(counterAccount.authority.toString()).to.equal(user.publicKey.toString());
  });
  it("Increments the counter", async () => {
    // Call increment instruction
    await program.methods
      .increment()
      .accounts({
        counter: counterKeypair.publicKey,
        authority: user.publicKey,
    })
      .rpc();
    // Fetch the account and check the count
    const counterAccount = await program.account.counter.fetch(counterKeypair.publicKey);
    expect(counterAccount.count.toNumber()).to.equal(1);
  });
  it("Decrements the counter", async () => {
    // Call decrement instruction
    await program.methods
      .decrement()
      .accounts({
        counter: counterKeypair.publicKey,
        authority: user.publicKey,
    })
      .rpc();
    // Fetch the account and check the count
    const counterAccount = await program.account.counter.fetch(counterKeypair.publicKey);
    expect(counterAccount.count.toNumber()).to.equal(0);
  });
});
