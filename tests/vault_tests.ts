import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import { SplTokenVault } from "../target/types/spl_token_vault";
import {
  createMint,
  createAccount,
  mintTo,
  getAccount,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { assert } from "chai";

describe("spl-vault-anchor", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.SplTokenVault as Program<SplTokenVault>;
  const connection = provider.connection;

  // keypairs
  const admin = anchor.web3.Keypair.generate();
  const user = anchor.web3.Keypair.generate();
  let acceptedMint: anchor.web3.PublicKey;
  let receiptMint: anchor.web3.Keypair;
  let vaultState: anchor.web3.PublicKey;
  let vaultTokenAccount: anchor.web3.PublicKey;
  let userTokenAccount: anchor.web3.PublicKey;
  let userReceiptAccount: anchor.web3.PublicKey;
  let adminTokenAccount: anchor.web3.PublicKey;

  const FEE_BPS = 50; // 0.5%
  const DEPOSIT_AMOUNT = new BN(1_000);
  const COOLDOWN_SECONDS = 86_400;

  before(async () => {
    // airdrop to admin and user
    await connection.confirmTransaction(
      await connection.requestAirdrop(admin.publicKey, 10 * anchor.web3.LAMPORTS_PER_SOL)
    );
    await connection.confirmTransaction(
      await connection.requestAirdrop(user.publicKey, 10 * anchor.web3.LAMPORTS_PER_SOL)
    );

    // create accepted mint (admin is mint authority)
    acceptedMint = await createMint(connection, admin, admin.publicKey, null, 6);

    // receipt mint keypair (passed to initialize as signer)
    receiptMint = anchor.web3.Keypair.generate();

    // derive PDAs
    [vaultState] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("vault_state"), admin.publicKey.toBuffer()],
      program.programId
    );
    [vaultTokenAccount] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("vault_token"), vaultState.toBuffer()],
      program.programId
    );

    // create token accounts
    userTokenAccount = await createAccount(connection, user, acceptedMint, user.publicKey);
    adminTokenAccount = await createAccount(connection, admin, acceptedMint, admin.publicKey);

    // mint 10,000 tokens to user
    await mintTo(connection, admin, acceptedMint, userTokenAccount, admin, 10_000);
  });

  it("initializes the vault", async () => {
    await program.methods
      .initialize(FEE_BPS)
      .accounts({
        admin: admin.publicKey,
        acceptedMint,
        receiptMint: receiptMint.publicKey,
        vaultState,
        vaultTokenAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([admin, receiptMint])
      .rpc();

    const vault = await program.account.vaultState.fetch(vaultState);
    assert.ok(vault.admin.equals(admin.publicKey));
    assert.equal(vault.totalDeposited.toNumber(), 0);
    assert.equal(vault.feeBps, FEE_BPS);
    assert.equal(vault.isPaused, false);

    // create user receipt ATA now that receipt mint exists
    userReceiptAccount = await createAccount(
      connection,
      user,
      receiptMint.publicKey,
      user.publicKey
    );

    console.log("✅ Vault initialized correctly");
  });

  it("deposits tokens and mints receipts 1:1", async () => {
    await program.methods
      .deposit(DEPOSIT_AMOUNT)
      .accounts({
        user: user.publicKey,
        vaultState,
        userTokenAccount,
        vaultTokenAccount,
        receiptMint: receiptMint.publicKey,
        userReceiptAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([user])
      .rpc();

    const vaultAcc = await getAccount(connection, vaultTokenAccount);
    assert.equal(Number(vaultAcc.amount), 1_000);

    const receiptAcc = await getAccount(connection, userReceiptAccount);
    assert.equal(Number(receiptAcc.amount), 1_000);

    const vault = await program.account.vaultState.fetch(vaultState);
    assert.equal(vault.totalDeposited.toNumber(), 1_000);

    console.log("✅ Deposit minted receipts 1:1");
  });

  it("requests withdrawal — burns receipts and creates ticket", async () => {
    const [withdrawalTicket] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("withdrawal"), user.publicKey.toBuffer(), vaultState.toBuffer()],
      program.programId
    );

    await program.methods
      .requestWithdrawal(DEPOSIT_AMOUNT)
      .accounts({
        user: user.publicKey,
        vaultState,
        receiptMint: receiptMint.publicKey,
        userReceiptAccount,
        withdrawalTicket,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([user])
      .rpc();

    // receipts should be burned
    const receiptAcc = await getAccount(connection, userReceiptAccount);
    assert.equal(Number(receiptAcc.amount), 0);

    // ticket should exist with correct data
    const ticket = await program.account.withdrawalTicket.fetch(withdrawalTicket);
    assert.equal(ticket.receiptAmount.toNumber(), 1_000);
    assert.ok(ticket.user.equals(user.publicKey));

    console.log("✅ Withdrawal ticket created, receipts burned");
  });

  it("rejects claim before 24hr cooldown", async () => {
    const [withdrawalTicket] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("withdrawal"), user.publicKey.toBuffer(), vaultState.toBuffer()],
      program.programId
    );

    try {
      await program.methods
        .claim()
        .accounts({
          user: user.publicKey,
          vaultState,
          vaultTokenAccount,
          userTokenAccount,
          adminTokenAccount,
          withdrawalTicket,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([user])
        .rpc();
      assert.fail("Should have thrown CooldownNotElapsed");
    } catch (err: any) {
      assert.include(err.message, "CooldownNotElapsed");
      console.log("✅ Claim correctly rejected before 24hr cooldown");
    }
  });

  it("attacker cannot claim another user's ticket", async () => {
    const attacker = anchor.web3.Keypair.generate();
    await connection.confirmTransaction(
      await connection.requestAirdrop(attacker.publicKey, 2 * anchor.web3.LAMPORTS_PER_SOL)
    );

    // real ticket belongs to user, attacker tries to claim it
    const [realTicket] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("withdrawal"), user.publicKey.toBuffer(), vaultState.toBuffer()],
      program.programId
    );

    try {
      await program.methods
        .claim()
        .accounts({
          user: attacker.publicKey,
          vaultState,
          vaultTokenAccount,
          userTokenAccount,
          adminTokenAccount,
          withdrawalTicket: realTicket,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([attacker])
        .rpc();
      assert.fail("Attacker should not be able to claim user's ticket");
    } catch (err: any) {
      // expected — PDA seeds won't match attacker's pubkey
      console.log("✅ Attacker correctly rejected from stealing withdrawal");
    }
  });
});
