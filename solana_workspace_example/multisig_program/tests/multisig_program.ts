import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { MultisigProgram } from "../target/types/multisig_program";
import { expect } from "chai";

describe("multisig_program", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.multisigProgram as Program<MultisigProgram>;
  const walletPubkey = provider.wallet.publicKey;
  const secondOwner = anchor.web3.Keypair.generate();
  const thirdOwner = anchor.web3.Keypair.generate();

  it("creates, approves, and executes a multisig transaction", async () => {
    const airdropSig = await provider.connection.requestAirdrop(
      secondOwner.publicKey,
      2 * anchor.web3.LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(airdropSig, "confirmed");

    const multisig = anchor.web3.Keypair.generate();
    const transaction = anchor.web3.Keypair.generate();
    const owners = [walletPubkey, secondOwner.publicKey, thirdOwner.publicKey];

    await program.methods
      .createMultisig(owners, new anchor.BN(2))
      .accounts({
        multisig: multisig.publicKey,
        payer: walletPubkey,
      })
      .signers([multisig])
      .rpc();

    const accountsMeta = [
      {
        pubkey: walletPubkey,
        isSigner: false,
        isWritable: false,
      },
    ];

    await program.methods
      .createTransaction(
        program.programId,
        accountsMeta,
        Buffer.from([1, 2, 3, 4])
      )
      .accounts({
        multisig: multisig.publicKey,
        transaction: transaction.publicKey,
        proposer: walletPubkey,
      })
      .signers([transaction])
      .rpc();

    let txState = await program.account.transaction.fetch(transaction.publicKey);
    expect(txState.didExecute).to.eq(false);

    await program.methods
      .approve()
      .accounts({
        multisig: multisig.publicKey,
        transaction: transaction.publicKey,
        owner: secondOwner.publicKey,
      })
      .signers([secondOwner])
      .rpc();

    await program.methods
      .executeTransaction()
      .accounts({
        multisig: multisig.publicKey,
        transaction: transaction.publicKey,
        executor: walletPubkey,
      })
      .rpc();

    txState = await program.account.transaction.fetch(transaction.publicKey);
    expect(txState.didExecute).to.eq(true);
  });
});
