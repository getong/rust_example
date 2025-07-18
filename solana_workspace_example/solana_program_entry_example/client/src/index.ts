import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
  clusterApiUrl,
  SystemProgram,
} from "@solana/web3.js";
import fs from "fs";
import path from "path";

// Configuration
const CLUSTER = "http://localhost:8899"; // Use local validator
const PROGRAM_ID = new PublicKey(
  "FP9Ui3292EvHcidbQPJcHqDQsstZP7Wb4uJEEQLS3Qos",
);
const WALLET_PATH = path.join(process.env.HOME!, "solana-wallets", "bob.json");

// Course data structure
interface CourseData {
  name: string;
  degree: string;
  institution: string;
  start_date: string;
}

// Instruction types
enum InstructionType {
  AddCourse = 0,
  UpdateCourse = 1,
  ReadCourse = 2,
  DeleteCourse = 3,
}

// Helper function to serialize course data
function serializeCourseData(course: CourseData, instructionType: InstructionType): Buffer {
  const nameBuffer = Buffer.from(course.name, "utf8");
  const degreeBuffer = Buffer.from(course.degree, "utf8");
  const institutionBuffer = Buffer.from(course.institution, "utf8");
  const startDateBuffer = Buffer.from(course.start_date, "utf8");

  if (instructionType === InstructionType.ReadCourse || instructionType === InstructionType.DeleteCourse) {
    // For read and delete, only serialize name and start_date
    const dataBuffer = Buffer.alloc(
      1 + // variant
      4 + nameBuffer.length + // name length + name
      4 + startDateBuffer.length // start_date length + start_date
    );

    let offset = 0;
    dataBuffer.writeUInt8(instructionType, offset);
    offset += 1;

    // Write name
    dataBuffer.writeUInt32LE(nameBuffer.length, offset);
    offset += 4;
    nameBuffer.copy(dataBuffer, offset);
    offset += nameBuffer.length;

    // Write start_date
    dataBuffer.writeUInt32LE(startDateBuffer.length, offset);
    offset += 4;
    startDateBuffer.copy(dataBuffer, offset);

    return dataBuffer;
  } else {
    // For add and update, serialize all fields
    const dataBuffer = Buffer.alloc(
      1 + // variant
      4 + nameBuffer.length + // name length + name
      4 + degreeBuffer.length + // degree length + degree
      4 + institutionBuffer.length + // institution length + institution
      4 + startDateBuffer.length // start_date length + start_date
    );

    let offset = 0;
    dataBuffer.writeUInt8(instructionType, offset);
    offset += 1;

    // Write name
    dataBuffer.writeUInt32LE(nameBuffer.length, offset);
    offset += 4;
    nameBuffer.copy(dataBuffer, offset);
    offset += nameBuffer.length;

    // Write degree
    dataBuffer.writeUInt32LE(degreeBuffer.length, offset);
    offset += 4;
    degreeBuffer.copy(dataBuffer, offset);
    offset += degreeBuffer.length;

    // Write institution
    dataBuffer.writeUInt32LE(institutionBuffer.length, offset);
    offset += 4;
    institutionBuffer.copy(dataBuffer, offset);
    offset += institutionBuffer.length;

    // Write start_date
    dataBuffer.writeUInt32LE(startDateBuffer.length, offset);
    offset += 4;
    startDateBuffer.copy(dataBuffer, offset);

    return dataBuffer;
  }
}

// Helper function to derive PDA address
function derivePDAAddress(
  course: CourseData,
  programId: PublicKey,
): [PublicKey, number] {
  const [pda, bump] = PublicKey.findProgramAddressSync(
    [Buffer.from(course.name, "utf8"), Buffer.from(course.start_date, "utf8")],
    programId,
  );
  return [pda, bump];
}

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

  async callProgram(courseData: CourseData, instructionType: InstructionType): Promise<string> {
    // Serialize the course data
    const instructionData = serializeCourseData(courseData, instructionType);

    // Derive the PDA address
    const [pdaAddress, bump] = derivePDAAddress(courseData, PROGRAM_ID);

    console.log("PDA Address:", pdaAddress.toString());
    console.log("Bump seed:", bump);

    let keys;
    if (instructionType === InstructionType.AddCourse) {
      // For adding course, we need system program
      keys = [
        {
          pubkey: this.payer.publicKey,
          isSigner: true,
          isWritable: true,
        },
        {
          pubkey: pdaAddress,
          isSigner: false,
          isWritable: true,
        },
        {
          pubkey: SystemProgram.programId,
          isSigner: false,
          isWritable: false,
        },
      ];
    } else if (instructionType === InstructionType.ReadCourse) {
      // For reading, only need the PDA account
      keys = [
        {
          pubkey: pdaAddress,
          isSigner: false,
          isWritable: false,
        },
      ];
    } else {
      // For update and delete, need initializer and PDA
      keys = [
        {
          pubkey: this.payer.publicKey,
          isSigner: true,
          isWritable: true,
        },
        {
          pubkey: pdaAddress,
          isSigner: false,
          isWritable: true,
        },
      ];
    }

    const instruction = new TransactionInstruction({
      keys,
      programId: PROGRAM_ID,
      data: instructionData,
    });

    const transaction = new Transaction().add(instruction);

    const signature = await sendAndConfirmTransaction(
      this.connection,
      transaction,
      [this.payer],
    );

    return signature;
  }

  async addCourse(courseData: CourseData): Promise<string> {
    console.log("Adding course:", courseData.name);
    return await this.callProgram(courseData, InstructionType.AddCourse);
  }

  async updateCourse(courseData: CourseData): Promise<string> {
    console.log("Updating course:", courseData.name);
    return await this.callProgram(courseData, InstructionType.UpdateCourse);
  }

  async readCourse(courseData: CourseData): Promise<string> {
    console.log("Reading course:", courseData.name);
    return await this.callProgram(courseData, InstructionType.ReadCourse);
  }

  async deleteCourse(courseData: CourseData): Promise<string> {
    console.log("Deleting course:", courseData.name);
    return await this.callProgram(courseData, InstructionType.DeleteCourse);
  }

  async checkPDAAccount(courseData: CourseData): Promise<void> {
    const [pdaAddress] = derivePDAAddress(courseData, PROGRAM_ID);

    try {
      const accountInfo = await this.connection.getAccountInfo(pdaAddress);
      if (accountInfo) {
        console.log("PDA account exists!");
        console.log("Account data length:", accountInfo.data.length);
        console.log("Account owner:", accountInfo.owner.toString());
        console.log("Account lamports:", accountInfo.lamports);
      } else {
        console.log("PDA account does not exist yet");
      }
    } catch (error) {
      console.error("Error checking PDA account:", error);
    }
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

      // Create sample course data
      const courseData: CourseData = {
        name: "Computer Science",
        degree: "Bachelor of Science",
        institution: "MIT",
        start_date: "2024-09-01",
      };

      console.log("Course data:", courseData);

      // Demonstrate CRUD operations
      console.log("\n=== CRUD Operations Demo ===");

      // 1. CREATE - Add course
      console.log("\n1. Creating course...");
      try {
        const addSignature = await this.addCourse(courseData);
        console.log("Add course transaction signature:", addSignature);
        await this.checkPDAAccount(courseData);
      } catch (error) {
        console.log("Course might already exist:", error);
      }

      // 2. READ - Read course
      console.log("\n2. Reading course...");
      try {
        const readSignature = await this.readCourse(courseData);
        console.log("Read course transaction signature:", readSignature);
      } catch (error) {
        console.log("Error reading course:", error);
      }

      // 3. UPDATE - Update course
      console.log("\n3. Updating course...");
      const updatedCourseData: CourseData = {
        ...courseData,
        degree: "Master of Science", // Update degree
        institution: "Stanford University", // Update institution
      };
      try {
        const updateSignature = await this.updateCourse(updatedCourseData);
        console.log("Update course transaction signature:", updateSignature);
      } catch (error) {
        console.log("Error updating course:", error);
      }

      // 4. READ - Read updated course
      console.log("\n4. Reading updated course...");
      try {
        const readUpdatedSignature = await this.readCourse(courseData);
        console.log("Read updated course transaction signature:", readUpdatedSignature);
      } catch (error) {
        console.log("Error reading updated course:", error);
      }

      // 5. DELETE - Delete course
      console.log("\n5. Deleting course...");
      try {
        const deleteSignature = await this.deleteCourse(courseData);
        console.log("Delete course transaction signature:", deleteSignature);
        await this.checkPDAAccount(courseData);
      } catch (error) {
        console.log("Error deleting course:", error);
      }

      console.log("\n=== CRUD Operations Demo Complete ===");

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
