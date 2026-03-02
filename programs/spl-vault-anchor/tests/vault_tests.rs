use anchor_lang::{
    prelude::Pubkey, system_program, AnchorDeserialize, InstructionData, ToAccountMetas,
};
use litesvm::LiteSVM;
use solana_sdk::{
    instruction::Instruction,
    program_pack::Pack,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use spl_associated_token_account::get_associated_token_address;
use spl_token::state::{Account as TokenAccount, Mint};

// helpers

fn program_id() -> Pubkey {
    spl_vault_anchor::ID
}

fn vault_state_pda(admin: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"vault_state", admin.as_ref()], &program_id())
}

fn vault_token_pda(vault_state: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"vault_token", vault_state.as_ref()], &program_id())
}

fn withdrawal_ticket_pda(user: &Pubkey, vault_state: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"withdrawal", user.as_ref(), vault_state.as_ref()],
        &program_id(),
    )
}

//create a mint, return its pubkey
fn create_mint(svm: &mut LiteSVM, payer: &Keypair, decimals: u8) -> Pubkey {
    let mint_kp = Keypair::new();
    let rent = svm.minimum_balance_for_rent_exemption(Mint::LEN);
    let create_ix = solana_sdk::system_instruction::create_account(
        &payer.pubkey(),
        &mint_kp.pubkey(),
        rent,
        Mint::LEN as u64,
        &spl_token::id(),
    );

    let init_ix = spl_token::instruction::initialize_mint(
        &spl_token::id(),
        &mint_kp.pubkey(),
        &payer.pubkey(),
        None,
        decimals,
    )
    .unwrap();

    let tx = Transaction::new_signed_with_payer(
        &[create_ix, init_ix],
        Some(&payer.pubkey()),
        &[payer, &mint_kp],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();
    mint_kp.pubkey()
}

//create an ata for owner, return its pubkey
fn create_ata(svm: &mut LiteSVM, payer: &Keypair, owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    let ata = get_associated_token_address(owner, mint);
    let ix = spl_associated_token_account::instruction::create_associated_token_account(
        &payer.pubkey(),
        owner,
        mint,
        &spl_token::id(),
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx).unwrap();
    ata
}

//mint tokens to a token acc
fn mint_to(svm: &mut LiteSVM, payer: &Keypair, mint: &Pubkey, dest: &Pubkey, amount: u64) {
    let ix =
        spl_token::instruction::mint_to(&spl_token::id(), mint, dest, &payer.pubkey(), &[], amount)
            .unwrap();

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx).unwrap();
}

//read token acc bal
fn token_balance(svm: &LiteSVM, account: Pubkey) -> u64 {
    let data = svm.get_account(account).unwrap().data;
    TokenAccount::unpack(&data).unwrap().amount
}

// setup

struct TestContext {
    svm: LiteSVM,
    admin: Keypair,
    user: Keypair,
    accepted_mint: Pubkey,
    receipt_mint_kp: Keypair,
    vault_state: Pubkey,
    vault_token_account: Pubkey,
    user_token_ata: Pubkey,
    user_receipt_ata: Pubkey,
    admin_token_ata: Pubkey,
}
impl TestContext {
    fn new() -> Self {
        let mut svm = LiteSVM::new();
        svm.add_program_from_file(program_id(), "../../target/deploy/spl_vault_anchor.so")
            .unwrap();

        let admin = Keypair::new();
        let user = Keypair::new();
        svm.airdrop(&admin.pubkey(), 10_000_000_000).unwrap();
        svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

        // create accepted mint (admin is mint authority)
        let accepted_mint = create_mint(&mut svm, &admin, 6);

        //receipt mint keupair (passed to initialize)
        let receipt_mint_kp = Keypair::new();

        // derive pdas
        let (vault_state, _) = vault_state_pda(&admin.pubkey());
        let (vault_token_account, _) = vault_token_pda(&vault_state);

        //create atas
        let user_token_ata = create_ata(&mut svm, &admin, &user.pubkey(), &accepted_mint);
        let admin_token_ata = create_ata(&mut svm, &admin, &admin.pubkey(), &accepted_mint);

        //mint 10,000 tokrn to user
        mint_to(&mut svm, &admin, &accepted_mint, &user_token_ata, 10_000);

        TestContext {
            svm,
            admin,
            user,
            accepted_mint,
            receipt_mint_kp,
            vault_state,
            vault_token_account,
            user_token_ata,
            user_receipt_ata: Pubkey::default(), //set after initialize
            admin_token_ata,
        }
    }

    //call initialize and set user_receipt_ata
    fn initialize(&mut self, fee_bps: u16) {
        let ix = Instruction {
            program_id: program_id(),
            accounts: spl_vault_anchor::accounts::Initialize {
                admin: self.admin.pubkey(),
                accepted_mint: self.accepted_mint,
                receipt_mint: self.receipt_mint_kp.pubkey(),
                vault_state: self.vault_state,
                vault_token_account: self.vault_token_account,
                token_program: spl_token::id(),
                system_program: system_program::ID,
                rent: solana_sdk::sysvar::rent::id(),
            }
            .to_account_metas(None),
            data: spl_vault_anchor::instruction::Initialize { fee_bps }.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.admin.pubkey()),
            &[&self.admin, &self.receipt_mint_kp],
            self.svm.latest_blockhash(),
        );
        self.svm.send_transaction(tx).unwrap();

        //create user receip ata
        let admin_clone = Keypair::from_bytes(&self.admin.to_bytes()).unwrap();
        let receipt_mint = self.receipt_mint_kp.pubkey();
        let user = self.user.pubkey();

        self.user_receipt_ata = create_ata(&mut self.svm, &admin_clone, &user, &receipt_mint);
    }

    fn deposit(&mut self, amount: u64) {
        let ix = Instruction {
            program_id: program_id(),
            accounts: spl_vault_anchor::accounts::Deposit {
                user: self.user.pubkey(),
                vault_state: self.vault_state,
                user_token_account: self.user_token_ata,
                vault_token_account: self.vault_token_account,
                receipt_mint: self.receipt_mint_kp.pubkey(),
                user_receipt_account: self.user_receipt_ata,
                token_program: spl_token::id(),
            }
            .to_account_metas(None),
            data: spl_vault_anchor::instruction::Deposit { amount }.data(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.user.pubkey()),
            &[&self.user],
            self.svm.latest_blockhash(),
        );
        self.svm.send_transaction(tx).unwrap();
    }

    fn request_withdrawal(&mut self, receipt_amount: u64) -> Pubkey {
        let (ticket, _) = withdrawal_ticket_pda(&self.user.pubkey(), &self.vault_state);
        let ix = Instruction {
            program_id: program_id(),
            accounts: spl_vault_anchor::accounts::RequestWithdrawal {
                user: self.user.pubkey(),
                vault_state: self.vault_state,
                receipt_mint: self.receipt_mint_kp.pubkey(),
                user_receipt_account: self.user_receipt_ata,
                withdrawal_ticket: ticket,
                clock: solana_sdk::sysvar::clock::id(),
                token_program: spl_token::id(),
                system_program: system_program::ID,
            }
            .to_account_metas(None),
            data: spl_vault_anchor::instruction::RequestWithdrawal { receipt_amount }.data(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.user.pubkey()),
            &[&self.user],
            self.svm.latest_blockhash(),
        );
        self.svm.send_transaction(tx).unwrap();
        ticket
    }

    fn claim(&mut self) -> Result<(), litesvm::error::LiteSVMError> {
        let (ticket, _) = withdrawal_ticket_pda(&self.user.pubkey(), &self.vault_state);
        let ix = Instruction {
            program_id: program_id(),
            accounts: spl_vault_anchor::accounts::Claim {
                user: self.user.pubkey(),
                vault_state: self.vault_state,
                vault_token_account: self.vault_token_account,
                user_token_account: self.user_token_ata,
                admin_token_account: self.admin_token_ata,
                withdrawal_ticket: ticket,
                clock: solana_sdk::sysvar::clock::id(),
                token_program: spl_token::id(),
            }
            .to_account_metas(None),
            data: spl_vault_anchor::instruction::Claim {}.data(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.user.pubkey()),
            &[&self.user],
            self.svm.latest_blockhash(),
        );
        self.svm.send_transaction(tx).map(|_| ())
    }
}

// tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize_vault() {
        let mut ctx = TestContext::new();
        ctx.initialize(50); // 0.5% fee

        let vault_data = ctx.svm.get_account(&ctx.vault_state).unwrap().data;
        let vault: spl_vault_anchor::state::VaultState =
            AnchorDeserialize::deserialize(&mut &vault_data[8..]).unwrap();

        assert_eq!(vault.admin, ctx.admin.pubkey());
        assert_eq!(vault.total_deposited, 0);
        assert_eq!(vault.fee_bps, 50);
        assert!(!vault.is_paused);
        println!("✅ Vault initialized correctly");
    }

    #[test]
    fn test_deposit_mints_receipts() {
        let mut ctx = TestContext::new();
        ctx.initialize(50);
        ctx.deposit(1_000);

        assert_eq!(token_balance(&ctx.svm, &ctx.vault_token_account), 1_000);
        assert_eq!(token_balance(&ctx.svm, &ctx.user_receipt_ata), 1_000);

        let vault_data = ctx.svm.get_account(&ctx.vault_state).unwrap().data;
        let vault: spl_vault_anchor::state::VaultState =
            AnchorDeserialize::deserialize(&mut &vault_data[8..]).unwrap();
        assert_eq!(vault.total_deposited, 1_000);
        println!("✅ Deposit minted receipts 1:1");
    }

    #[test]
    fn test_withdrawal_request_burns_and_creates_ticket() {
        let mut ctx = TestContext::new();
        ctx.initialize(50);
        ctx.deposit(1_000);
        ctx.request_withdrawal(1_000);

        // receipts burned
        assert_eq!(token_balance(&ctx.svm, &ctx.user_receipt_ata), 0);

        // ticket exists
        let (ticket_pda, _) = withdrawal_ticket_pda(&ctx.user.pubkey(), &ctx.vault_state);
        let ticket_data = ctx.svm.get_account(&ticket_pda).unwrap().data;
        let ticket: spl_vault_anchor::state::WithdrawalTicket =
            AnchorDeserialize::deserialize(&mut &ticket_data[8..]).unwrap();
        assert_eq!(ticket.receipt_amount, 1_000);
        assert_eq!(ticket.user, ctx.user.pubkey());
        println!("✅ Withdrawal ticket created, receipts burned");
    }

    #[test]
    fn test_claim_before_cooldown_fails() {
        let mut ctx = TestContext::new();
        ctx.initialize(50);
        ctx.deposit(1_000);
        ctx.request_withdrawal(1_000);

        // try to claim immediately — must fail
        let result = ctx.claim();
        assert!(result.is_err(), "Claim should fail before cooldown");
        println!("✅ Claim correctly rejected before 24hr cooldown");
    }

    #[test]
    fn test_claim_after_cooldown_succeeds_with_fee() {
        let mut ctx = TestContext::new();
        ctx.initialize(50); // 0.5% fee
        ctx.deposit(1_000);
        ctx.request_withdrawal(1_000);

        // advance clock past 24hrs
        ctx.svm.warp_to_unix(
            ctx.svm.get_sysvar::<solana_sdk::sysvar::clock::Clock>().unix_timestamp
                + spl_vault_anchor::state::WithdrawalTicket::COOLDOWN_SECONDS
                + 1,
        );

        ctx.claim().unwrap();

        // fee = 1000 * 50 / 10000 = 5
        // user receives 995, admin receives 5
        assert_eq!(token_balance(&ctx.svm, &ctx.user_token_ata), 9_000 + 995); // started with 10_000, deposited 1_000
        assert_eq!(token_balance(&ctx.svm, &ctx.admin_token_ata), 5);
        println!("✅ Claim succeeded after cooldown with correct fee");
    }

    #[test]
    fn test_attacker_cannot_claim_other_users_ticket() {
        let mut ctx = TestContext::new();
        ctx.initialize(50);
        ctx.deposit(1_000);
        ctx.request_withdrawal(1_000);

        // attacker tries to derive user's ticket — gets wrong PDA
        let attacker = Keypair::new();
        ctx.svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();

        let (real_ticket, _) = withdrawal_ticket_pda(&ctx.user.pubkey(), &ctx.vault_state);

        // attacker passes real ticket address but signs with their own key
        // constraint = withdrawal_ticket.user == user.key() will reject this
        let ix = Instruction {
            program_id: program_id(),
            accounts: spl_vault_anchor::accounts::Claim {
                user: attacker.pubkey(), // attacker as user
                vault_state: ctx.vault_state,
                vault_token_account: ctx.vault_token_account,
                user_token_account: ctx.user_token_ata,
                admin_token_account: ctx.admin_token_ata,
                withdrawal_ticket: real_ticket, // real ticket
                clock: solana_sdk::sysvar::clock::id(),
                token_program: spl_token::id(),
            }
            .to_account_metas(None),
            data: spl_vault_anchor::instruction::Claim {}.data(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&attacker.pubkey()),
            &[&attacker],
            ctx.svm.latest_blockhash(),
        );
        let result = ctx.svm.send_transaction(tx);
        assert!(result.is_err(), "Attacker should not be able to claim user's ticket");
        println!("✅ Attacker correctly rejected from stealing withdrawal");
    }
}
