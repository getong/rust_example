import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
  clusterApiUrl,
} from "@solana/web3.js";
import fs from "fs";
import path from "path";

// Configuration
const CLUSTER = "http://localhost:8899"; // Use local validator
const PROGRAM_ID = new PublicKey(
  "FP9Ui3292EvHcidbQPJcHqDQsstZP7Wb4uJEEQLS3Qos",
);
const WALLET_PATH = path.join(process.env.HOME!, "solana-wallets", "bob.json");

class SolanaProgramClient {
  private connection: Connection;
  private payer!: Keypair; // <-- Add definite assignment assertion

  constructor() {
    this.connection = new Connection(CLUSTER, "confirmed");
    this.loadWallet();
  }

  private loadWallet() {
    try {
      const walletData = JSON.parse(fs.readFileSync(WALLET_PATH, "utf8"));
      this.payer = Keypair.fromSecretKey(new Uint8Array(walletData));
      console.log("Wallet loaded:", this.payer.publicKey.toString());
    } catch (error) {
      console.error("Error loading wallet:", error);
      process.exit(1);
    }
  }

  async validateConnection(): Promise<boolean> {
    try {
      const version = await this.connection.getVersion();
      console.log(
        "Connected to Solana cluster version:",
        version["solana-core"],
      );
      return true;
    } catch (error) {
      console.error("Failed to connect to Solana cluster at:", CLUSTER);
      console.error(
        "Make sure the local validator is running with: solana-test-validator",
      );
      return false;
    }
  }

  async getBalance(): Promise<number> {
    const balance = await this.connection.getBalance(this.payer.publicKey);
    return balance / 1e9; // Convert lamports to SOL
  }

  async callProgram(data: Buffer = Buffer.alloc(0)): Promise<string> {
    const instruction = new TransactionInstruction({
      keys: [
        {
          pubkey: this.payer.publicKey,
          isSigner: true,
          isWritable: false,
        },
      ],
      programId: PROGRAM_ID,
      data,
    });

    const transaction = new Transaction().add(instruction);

    const signature = await sendAndConfirmTransaction(
      this.connection,
      transaction,
      [this.payer],
    );

    return signature;
  }

  async run() {
    try {
      console.log("Connecting to Solana...");

      // Validate connection first
      const isConnected = await this.validateConnection();
      if (!isConnected) {
        process.exit(1);
      }

      console.log("Getting wallet balance...");
      const balance = await this.getBalance();
      console.log(`Wallet balance: ${balance} SOL`);

      // Check if wallet has sufficient balance
      if (balance === 0) {
        console.log("Wallet has no SOL. Requesting airdrop...");
        await this.requestAirdrop();
        // Get balance again after airdrop
        const newBalance = await this.getBalance();
        console.log(`New wallet balance: ${newBalance} SOL`);
      }

      console.log("Calling program...");
      const signature = await this.callProgram();
      console.log("Transaction signature:", signature);
    } catch (error) {
      console.error("Error:", error);
      console.error(
        "Make sure solana-test-validator is running and try again.",
      );
    }
  }

  async requestAirdrop(): Promise<void> {
    try {
      const signature = await this.connection.requestAirdrop(
        this.payer.publicKey,
        2 * 1e9, // 2 SOL
      );
      await this.connection.confirmTransaction(signature);
      console.log("Airdrop successful! Signature:", signature);
    } catch (error) {
      console.error("Airdrop failed:", error);
    }
  }
}

// Run the client
const client = new SolanaProgramClient();
client.run();
