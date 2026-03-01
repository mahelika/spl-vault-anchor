use anchor_lang::{prelude::Pubkey, system_program, AnchorDeserialize, InstructionData, ToAccountMetas};
use litesvm::LiteSVM;
use solana_sdk::{
    instruction::Instruction,
    signature::{Keypair, Signer},
    transaction::Transaction,
    program_pack::Pack,
};
use spl_token::state::{Account as TokenAccount, Mint};
use spl_associated_token_account::get_associated_token_address;


// helpers

fn program_id()-> Pubkey {
    spl_vault_anchor::ID
}

fn vault_state_pda(admin: &Pubkey) -> (Pubkey, u8){
    Pubkey::find_program_address(&[b"vault_state", admin.as_ref()], &program_id())
}

fn vault_token_pda(vault_state: &Pubkey) -> (Pubkey, u8){
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
    ).unwrap();

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
    let ix = spl_token::instruction::mint_to(
        &spl_token::id(),
        mint,
        dest,
        &payer.pubkey(),
        &[],
        amount,
    ).unwrap();

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
        svm.add_program_from_file(program_id(), "../../target/deploy/spl_vault_anchor.so").unwrap();

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
    fn initialize(&mut self, fee_bps: u16){
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
            data: spl_vault_anchor::instruction::Initialize{fee_bps}.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.admin.pubkey()),
            &[&self.admin, &self.receipt_mint_kp],
            self.svm.latest_blockhash(),
        );
        self.svm.send_transaction(tx).unwrap();

        //create user receip ata
        self.user_receipt_ata = create_ata(
            &mut self.svm,
            &self.admin,
            &self.user.pubkey(),
            &self.receipt_mint_kp.pubkey(),
        );
    }
}