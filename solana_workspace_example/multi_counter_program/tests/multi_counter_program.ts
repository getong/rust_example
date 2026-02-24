import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { MultiCounterProgram } from "../target/types/multi_counter_program";

describe("multi_counter_program", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.multiCounterProgram as Program<MultiCounterProgram>;

  it("Is initialized!", async () => {
    // Add your test here.
    const tx = await program.methods.initialize().rpc();
    console.log("Your transaction signature", tx);
  });
});
