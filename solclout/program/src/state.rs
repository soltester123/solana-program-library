use {
    borsh::{BorshDeserialize, BorshSerialize},
    solana_program::pubkey::Pubkey,
};

/// prefix used for PDAs to avoid certain collision attacks (https://en.wikipedia.org/wiki/Collision_attack#Chosen-prefix_collision_attack)
pub const PREFIX: &str = "solclout";

#[repr(C)]
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
pub struct SolcloutInstance {
    /// Solclout token mint pubkey that can be traded for creator tokens
    pub solclout_token: Pubkey,
    /// Account to hold solclout after people buy
    pub solclout_storage: Pubkey,

    pub token_program_id: Pubkey,
    pub initialized: bool
}

#[repr(C)]
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
pub struct SolcloutCreator {
    /// Fields not updatable by the user
    /// The creator token mint pubkey
    pub creator_token: Pubkey,
    /// Solclout token mint pubkey that can be traded for this creator token
    pub solclout_instance: Pubkey,
    /// Destination for founder rewards
    pub founder_rewards_account: Pubkey,
    /// Percentage of purchases that go to the founder
    /// Percentage Value is (founder_reward_percentage / 10,000) * 100
    pub founder_reward_percentage: u16,
    pub initialized: bool,
    pub authority_nonce: u8,
}

const UTF8_BYTES: usize = 4;

impl SolcloutCreator {
    pub const LEN: usize = 32 * 3 + 2 + 1 + 1;
}

impl SolcloutInstance {
    pub const LEN: usize = 32 * 3 + 2 + 1;
}
