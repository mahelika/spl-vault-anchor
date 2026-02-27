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

