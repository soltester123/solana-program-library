use {
    borsh::{BorshDeserialize, BorshSerialize},
    solana_program::{
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        sysvar,
    },
};

#[repr(C)]
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
/// Args for initialize
pub struct InitializeSolcloutArgs {
    pub token_program_id: Pubkey,
    /// Nonce used to derive authority program address
    pub nonce: u8
}

#[repr(C)]
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
/// Args for initialize
pub struct InitializeCreatorArgs {
    /// Percentage of purchases that go to the founder
    /// Percentage Value is (founder_reward_percentage / 10,000) * 100
    pub founder_reward_percentage: u16,
    /// Nonce used to derive authority program address
    pub nonce: u8
}

#[repr(C)]
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
pub struct BuyCreatorCoinsArgs {
    pub lamports: u64, // Number of lamports to purchase, since creator coins use the same decimal as sol
}

#[repr(C)]
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
pub struct SellCreatorCoinsArgs {
    pub lamports: u64, // Number of lamports to sell, since creator coins use the same decimal as sol
}

/// Instructions supported by the Solclout program.
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub enum SolcloutInstruction {
    /// Initialize Solclout. Must provide an authority over the solclout token acct that is a PDA
    /// of this program. This will give the program full authority over the account.
    ///
    ///   0. `[writable, signer]` New Solclout instance to create. Should be able to hold state::
    ///   2. `[]` solclout token Account. Must be non zero, with owner `create_program_address(&[Solclout instance account])`
    InitializeSolclout(InitializeSolcloutArgs),

    /// Initialize a new solclout account. Note that you must already have created the mint,
    /// founder rewards account, and authority. The authority is a PDA of this program that gives it
    /// full authority of the creator coin mint. No coins will be minted outside of this program
    ///
    ///   0. `[writable signer]`  Solclout account, initialized by system::create_account with this program
    ///                     as the owner
    ///   1  `[]` Solclout instance.
    ///   2. `[]` Founder rewards account, token program as owner, initialized in spl-token with creator coin mint.
    ///   3. `[]` creator coin with mint and freeze authority set to `create_program_address(&[Solclout account])`, with nonce specified in the args
    InitializeCreator(InitializeCreatorArgs),

    /// Buy creator coins
    ///   0. `[]` Solclout instance
    ///   1. `[]` Solclout Creator to purchase creator coins of. This should be an initialized acct in solclout
    ///   2. `[]` Solclout Creator coin mint
    ///   3. `[signer]`  Purchasing account, this is an account owned by the token program with
    ///                            the solclout mint
    ///   4. `[]`  Destination account, this is an account owned by the token program with
    ///                            the creator mint
    BuyCreatorCoins(BuyCreatorCoinsArgs),

    /// Sell creator coins
    ///   0. `[]` Account to sell creator coins of. This should be an initialized acct in solclout
    ///   1. `[writeable signer]`  Selling account, this is an account owned by the token program with
    ///                            the creator coin mint
    ///   2. `[]`  Destination account, owned by the token program with the solclout coin mint
    SellCreatorCoins(SellCreatorCoinsArgs),
}

/// Creates an InitializeSolclout instruction
pub fn initialize_solclout(
    program_id: &Pubkey,
    solclout_instance: &Pubkey,
    solclout_storage_account: &Pubkey,
    token_program_id: &Pubkey,
    nonce: u8
) -> Instruction {
    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*solclout_instance, true),
            AccountMeta::new_readonly(*solclout_storage_account, false),
        ],
        data: SolcloutInstruction::InitializeSolclout(InitializeSolcloutArgs {
            token_program_id: *token_program_id,
            nonce
        })
            .try_to_vec()
            .unwrap(),
    }
}

/// Creates an InitializeCreator instruction
pub fn initialize_creator(
    program_id: &Pubkey,
    solclout_account: &Pubkey,
    solclout_instance: &Pubkey,
    founder_rewards_account: &Pubkey,
    creator_mint: &Pubkey,
    founder_reward_percentage: u16,
    nonce: u8
) -> Instruction {
    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*solclout_account, true),
            AccountMeta::new_readonly(*solclout_instance, false),
            AccountMeta::new_readonly(*founder_rewards_account, false),
            AccountMeta::new_readonly(*creator_mint, false),
        ],
        data: SolcloutInstruction::InitializeCreator(InitializeCreatorArgs {
            founder_reward_percentage,
            nonce
        })
        .try_to_vec()
        .unwrap(),
    }
}
