use num_traits::ToPrimitive;
use solana_program::program_error::ProgramError;
use solana_program::program_pack::IsInitialized;
use solana_program::system_instruction::create_account;
use solana_sdk::program::{invoke, invoke_signed};
use solana_sdk::system_instruction;

use {
    borsh::{BorshDeserialize, BorshSerialize},
    crate::{
        error::SolcloutError,
        instruction::SolcloutInstruction,
        state::{
            PREFIX, SolcloutCreator
        }
    },
    solana_program::{
        account_info::{AccountInfo, next_account_info},
        borsh::try_from_slice_unchecked,
        entrypoint::ProgramResult,
        msg,
        pubkey::Pubkey,
        sysvar::{rent::Rent},
    },
    spl_token::state::{Account, Mint},
};
use spl_token::native_mint;
use spl_token::solana_program::program_pack::Pack;

use crate::solana_program::sysvar::Sysvar;
use crate::state::SolcloutInstance;

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    input: &[u8],
) -> ProgramResult {
    let instruction = SolcloutInstruction::try_from_slice(input)?;
    match instruction {
        SolcloutInstruction::InitializeSolclout(args) => {
            msg!("Instruction: Initialize Solclout");
            process_initialize_solclout(program_id, accounts, args.token_program_id, args.nonce)
        }
        SolcloutInstruction::InitializeCreator(args) => {
            msg!("Instruction: Initialize Creator");
            process_initialize_creator(
                program_id,
                accounts,
                args.founder_reward_percentage,
                args.nonce
            )
        }
        SolcloutInstruction::BuyCreatorCoins(args) => {
            msg!("Instruction: Buy Creator Coins");
            process_buy_creator_coins(program_id, accounts, args.lamports)
        }
        SolcloutInstruction::SellCreatorCoins(args) => {
            msg!("Instruction: Sell Creator Coins");
            process_sell_creator_coins(program_id, accounts, args.lamports)
        }
    }
}

/// Unpacks a spl_token `Account`.
pub fn unpack_token_account(
    account_info: &AccountInfo,
    token_program_id: &Pubkey,
) -> Result<spl_token::state::Account, SolcloutError> {
    if account_info.owner != token_program_id {
        Err(SolcloutError::IncorrectTokenProgramId)
    } else {
        spl_token::state::Account::unpack(&account_info.data.borrow())
            .map_err(|_| SolcloutError::ExpectedAccount)
    }
}

/// Calculates the authority id by generating a program address.
pub fn authority_id(
    program_id: &Pubkey,
    source_id: &Pubkey,
    nonce: u8,
) -> Result<Pubkey, SolcloutError> {
    Pubkey::create_program_address(&[&source_id.to_bytes()[..32], &[nonce]], program_id)
        .or(Err(SolcloutError::InvalidProgramAddress))
}

fn process_initialize_solclout(program_id: &Pubkey, accounts: &[AccountInfo], token_program_id: Pubkey, nonce: u8) -> ProgramResult {
    let accounts_iter =  &mut accounts.into_iter();
    let solclout = next_account_info(accounts_iter)?;
    let solclout_storage_acc = next_account_info(accounts_iter)?;
    let authority_key = authority_id(program_id, solclout.key, nonce)?;
    let solclout_storage = unpack_token_account(solclout_storage_acc, &token_program_id)?;

    if solclout_storage.owner != authority_key {
        return Err(SolcloutError::InvalidStorageOwner.into());
    }

    if try_from_slice_unchecked::<SolcloutInstance>(&solclout.data.borrow())?.initialized {
        return Err(SolcloutError::AlreadyInitialized.into());
    }

    let solclout_instance = SolcloutInstance {
        solclout_token: solclout_storage.mint,
        solclout_storage: *solclout_storage_acc.key,
        token_program_id,
        initialized: true
    };
    solclout_instance.serialize(&mut *solclout.try_borrow_mut_data()?)?;

    Ok(())
}

fn process_initialize_creator(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    founder_reward_percentage: u16,
    nonce: u8
) -> ProgramResult {
    let accounts_iter =  &mut accounts.into_iter();
    let mut account = next_account_info(accounts_iter)?;
    let solclout_instance = next_account_info(accounts_iter)?;
    let solclout_instance_data: SolcloutInstance = try_from_slice_unchecked(&solclout_instance.data.borrow())?;

    let founder_rewards_account = next_account_info(accounts_iter)?;
    let founder_rewards_account_data = Account::unpack(&founder_rewards_account.data.borrow())?;
    let authority = authority_id(program_id, account.key, nonce)?;
    let creator_mint = next_account_info(accounts_iter)?;

    if solclout_instance.owner != *program_id {
        return Err(SolcloutError::InvalidSolcloutInstanceOwner).into();
    }

    if *creator_mint.owner != solclout_instance_data.token_program_id {
        return Err(SolcloutError::AccountWrongTokenProgram.into());
    }

    let creator_mint_data = Mint::unpack(*creator_mint.data.borrow())?;
    if creator_mint_data.mint_authority.unwrap() != authority {
        return Err(SolcloutError::InvalidMintAuthority.into());
    }

    if creator_mint_data.freeze_authority.unwrap() != authority {
        return Err(SolcloutError::InvalidFreezeAuthority.into());
    }

    if try_from_slice_unchecked::<SolcloutCreator>(&account.data.borrow())?.initialized {
        return Err(SolcloutError::AlreadyInitialized.into());
    }

    if *founder_rewards_account.owner != solclout_instance_data.token_program_id {
        return Err(SolcloutError::AccountWrongTokenProgram.into());
    }


    if founder_rewards_account_data.mint != *creator_mint.key {
        return Err(SolcloutError::InvalidFounderRewardsAccountType.into());
    }

    if !account.is_signer {
        return Err(SolcloutError::MissingSigner.into())
    }

    let new_account_data = SolcloutCreator {
        creator_token: *creator_mint.key,
        solclout_instance: *solclout_instance.key,
        founder_rewards_account: *founder_rewards_account.key,
        founder_reward_percentage,
        initialized: true,
        authority_nonce: nonce
    };
    new_account_data.serialize(&mut *account.try_borrow_mut_data()?)?;

    Ok(())
}


/// Price is 0.003 * supply^2.
/// But since we're buying multiple, the total price is
/// Intregral[(curr_supply, end_supply), 0.003 * supply^2.]
/// This is 0.001 * (end_supply^3 - curr_supply^3)
/// Since both are in lamports, we need to divide again by lamports^3 then multiply by lamports
/// to get back to lamports output.
fn price(supply: u64, lamports: u64) -> u64 {
    let numerator: u128 = (((lamports + supply) as u128).pow(3) - (supply as u128).pow(3));
    let denominator: u128 = (1000 * (10_u128.pow(native_mint::DECIMALS as u32)).pow(2)) as u128;
    (numerator / denominator) as u64
}

fn process_buy_creator_coins(program_id: &Pubkey, accounts: &[AccountInfo], lamports: u64) -> ProgramResult {
    let accounts_iter =  &mut accounts.into_iter();
    let solclout_instance = next_account_info(accounts_iter)?;
    let creator = next_account_info(accounts_iter)?;
    let creator_mint = next_account_info(accounts_iter)?;
    let purchaser = next_account_info(accounts_iter)?;
    let destination = next_account_info(accounts_iter)?;
    let creator_mint_data = Mint::unpack(*creator_mint.data.borrow())?;
    let (solclout_storage_account_key, _) = Pubkey::find_program_address(&[creator.key.as_ref()], program_id);

    let solclout_instance_data: SolcloutInstance = try_from_slice_unchecked(*solclout_instance.data.borrow())?;
    let token_program_id = solclout_instance_data.token_program_id;
    let creator_data: SolcloutCreator = try_from_slice_unchecked(*creator.data.borrow())?;
    let creator_mint_key = creator_data.creator_token;
    let authority = authority_id(program_id, solclout_instance.key, creator_data.authority_nonce)?;

    if creator_mint_key != *creator_mint.key {
        return Err(SolcloutError::InvalidCreatorMint.into());
    }

    if creator_data.solclout_instance != *solclout_instance.key {
        return Err(SolcloutError::SolcloutInstanceMismatch.into());
    }

    if creator.owner != *program_id {
        return Err(SolcloutError::InvalidCreatorOwner).into();
    }

    if solclout_instance.owner != *program_id {
        return Err(SolcloutError::InvalidSolcloutInstanceOwner).into();
    }

    let price = price(creator_mint_data.supply, lamports);
    let founder_cut = 10000 * lamports / (creator_data.founder_reward_percentage as u64);
    let purchaser_cut = lamports - founder_cut;

    // Suck their money into solclout
    let pay_money = spl_token::instruction::transfer(
        purchaser.owner,
        purchaser.key,
        &solclout_storage_account_key,
        purchaser.key,
        &[],
        price
    )?;
    invoke_signed(&pay_money, accounts, &[])?;

    let authority_seed = &[&solclout_instance.key.to_bytes()[..32], &[creator_data.authority_nonce]];
    // Mint the required lamports
    let give_founder_cut = spl_token::instruction::mint_to(
        &token_program_id,
        &creator_mint_key,
        &creator_data.creator_token,
        &authority,
        &[&authority],
        founder_cut
    )?;
    invoke_signed(&give_founder_cut, accounts, &[authority_seed]);
    let give_purchaser_cut = spl_token::instruction::mint_to(
        &token_program_id,
        &creator_mint_key,
        &destination.key,
        &authority,
        &[&authority],
        purchaser_cut
    )?;
    invoke_signed(&give_purchaser_cut, accounts, &[authority_seed]);

    Ok(())
}

fn process_sell_creator_coins(program_id: &Pubkey, accounts: &[AccountInfo], lamports: u64) -> ProgramResult {
    todo!()
}

#[cfg(test)]
mod tests {
    use solana_program::{
        account_info::IntoAccountInfo, clock::Epoch, instruction::Instruction, sysvar::rent,
    };
    use solana_program::rent::Rent;
    use solana_sdk::account::{Account as SolanaAccount, create_account_for_test, create_is_signer_account_infos, ReadableAccount};
    use solana_sdk::program_option::COption;

    use spl_token::solana_program::program_pack::Pack;
    use spl_token::state::AccountState;

    use crate::instruction::*;

    use super::*;

    fn do_process_instruction(
        instruction: Instruction,
        accounts: Vec<&mut SolanaAccount>,
    ) -> ProgramResult {
        let mut meta = instruction
            .accounts
            .iter()
            .zip(accounts)
            .map(|(account_meta, account)| (&account_meta.pubkey, account_meta.is_signer, account))
            .collect::<Vec<_>>();

        let account_infos = create_is_signer_account_infos(&mut meta);
        process_instruction(&instruction.program_id, &account_infos, &instruction.data)
    }

    fn rent_sysvar() -> SolanaAccount {
        create_account_for_test(&Rent::default())
    }

    fn get_account(space: usize, owner: &Pubkey) -> (Pubkey, SolanaAccount) {
        let key = Pubkey::new_unique();
        (key, SolanaAccount::new(0, space, owner))
    }

    fn initialize_spl_account(
        account: &mut SolanaAccount,
        token_program_id: &Pubkey,
        mint: &Pubkey,
        owner: &Pubkey
    ) {
        let mut account_data = vec![0; Account::get_packed_len()];
        Account::pack(Account {
            mint: *mint,
            owner: *owner,
            amount: 20,
            delegate: COption::None,
            state: AccountState::Initialized,
            is_native: COption::None,
            delegated_amount: 0,
            close_authority: COption::None
        }, &mut account_data);
        account.data = account_data;
    }

    #[test]
    fn test_initialize_solclout() {
        let program_id = Pubkey::new_unique();
        let (instance_key, mut instance) = get_account(SolcloutInstance::LEN as usize, &program_id);
        let account_seeds = &[
            &instance_key.to_bytes()[..32]
        ];
        let (authority_key, nonce) = Pubkey::find_program_address(account_seeds, &program_id);
        let (mint_key, mut mint) = get_account(SolcloutInstance::LEN as usize, &program_id);
        let token_program_id = Pubkey::new_unique();
        let (account_key, mut account) = get_account(Account::LEN as usize, &token_program_id);
        initialize_spl_account(&mut account, &token_program_id, &mint_key, &authority_key);

        assert_eq!(
            Ok(()),
            do_process_instruction(
                initialize_solclout(
                    &program_id,
                    &instance_key,
                    &account_key,
                    &token_program_id,
                    nonce
                ),
                vec![&mut instance, &mut account],
            )
        );

        let mut instance_data: SolcloutInstance = try_from_slice_unchecked::<SolcloutInstance>(&instance.data).unwrap();
        assert_eq!(instance_data.token_program_id, token_program_id);
        assert_eq!(instance_data.initialized, true);
        assert_eq!(instance_data.solclout_storage, account_key);
        assert_eq!(instance_data.solclout_token, mint_key);
    }

    #[test]
    fn test_initialize_creator() {
        let program_id = Pubkey::new_unique();

        let account_key = Pubkey::new_unique();
        let mut account = SolanaAccount::new(0, SolcloutCreator::LEN as usize, &program_id);
        let solclout_instance_key = Pubkey::new_unique();
        let mut solclout_instance = SolanaAccount::new(0, SolcloutInstance::LEN as usize, &program_id);
        let token_program_id = Pubkey::new_unique();
        let founder_rewards_account_key = Pubkey::new_unique();
        let mut founder_rewards_account = SolanaAccount::new(0, 0, &token_program_id);
        let (authority_key, nonce) = Pubkey::find_program_address(&[&account_key.to_bytes()[..32]], &program_id);
        let creator_mint_key = Pubkey::new_unique();
        let mut creator_mint = SolanaAccount::new(0, Mint::LEN as usize, &token_program_id);
        let solclout_instance_data = SolcloutInstance {
            solclout_token: Pubkey::new_unique(),
            solclout_storage: Pubkey::new_unique(),
            token_program_id,
            initialized: true
        };
        let mut new_data = solclout_instance_data.try_to_vec().unwrap();
        solclout_instance.data = new_data;

        let mut creator_mint_data = vec![0; Mint::get_packed_len()];
        Mint::pack(Mint {
            mint_authority: COption::Some(authority_key),
            supply: 20,
            decimals: 5,
            is_initialized: true,
            freeze_authority: COption::Some(authority_key)
        }, &mut creator_mint_data);
        creator_mint.data = creator_mint_data;
        let mut founder_rewards_account_data = vec![0; Account::get_packed_len()];
        Account::pack(Account {
            mint: creator_mint_key,
            owner: account_key,
            amount: 20,
            delegate: COption::None,
            state: AccountState::Initialized,
            is_native: COption::None,
            delegated_amount: 0,
            close_authority: COption::None
        }, &mut founder_rewards_account_data);
        founder_rewards_account.data = founder_rewards_account_data;
        let acc = Account::unpack(&founder_rewards_account.data).unwrap();

        assert_eq!(
            Ok(()),
            do_process_instruction(
                initialize_creator(
                    &program_id,
                    &account_key,
                    &solclout_instance_key,
                    &founder_rewards_account_key,
                    &creator_mint_key,
                    1000,
                    nonce
                ),
                vec![&mut account, &mut solclout_instance, &mut founder_rewards_account, &mut creator_mint],
            )
        );

        let mut solclout_account: SolcloutCreator = try_from_slice_unchecked::<SolcloutCreator>(&account.data).unwrap();
        assert_eq!(solclout_account.founder_reward_percentage, 1000);
        assert_eq!(solclout_account.solclout_instance, solclout_instance_key);
        assert_eq!(solclout_account.creator_token, creator_mint_key);
        assert_eq!(solclout_account.founder_rewards_account, founder_rewards_account_key);
    }

    #[test]
    fn test_price() {
        assert_eq!(price(0, 1000000000), 1000000);
        assert_eq!(price(1000000000, 1000000000), 7000000);
    }

    #[test]
    fn test_buy() {
        let program_id = Pubkey::new_unique();
        let (solclout_instance_key, mut solclout_instance) = get_account(SolcloutInstance::LEN, &program_id);
        let solclout_instance_data = SolcloutInstance {
            solclout_token: Pubkey::new_unique(),
            solclout_storage: Pubkey::new_unique(),
            token_program_id,
            initialized: true
        };
        let mut new_data = solclout_instance_data.try_to_vec().unwrap();
        solclout_instance.data = new_data;
        let token_program_id = Pubkey::new_unique();

        let (creator_key, creator) = get_account(SolcloutCreator::LEN, &program_id);
        let (creator_mint_key, mut creator_mint) = get_account(Mint::LEN, &token_program_id);
        let (solclout_mint_key, mut solclout_mint) = get_account(Mint::LEN, &token_program_id);
        let (authority_key, nonce) = Pubkey::find_program_address(&[&creator_key.to_bytes()[..32]], &program_id);
        let mut creator_mint_data = vec![0; Mint::get_packed_len()];
        Mint::pack(Mint {
            mint_authority: COption::Some(authority_key),
            supply: 20,
            decimals: 5,
            is_initialized: true,
            freeze_authority: COption::Some(authority_key)
        }, &mut creator_mint_data);
        creator_mint.data = creator_mint_data;

        let (purchaser_key, mut purchaser) = get_account(Account::LEN, &token_program_id);
        let (destination_key, mut destination) = get_account(Account::LEN, &token_program_id);
        initialize_spl_account(&mut purchaser, &token_program_id, &solclout_mint_key, &purchaser_key);
        initialize_spl_account(&mut destination, &token_program_id, &creator_mint_key, &purchaser_key);

        assert_eq!(
            Ok(()),
            do_process_instruction(
                initialize_creator(
                    &program_id,
                    &account_key,
                    &solclout_instance_key,
                    &founder_rewards_account_key,
                    &creator_mint_key,
                    1000,
                    nonce
                ),
                vec![&mut account, &mut solclout_instance, &mut founder_rewards_account, &mut creator_mint],
            )
        );


    }
}
