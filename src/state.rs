
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Uint128, Timestamp};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub admin: Addr,
    pub updater: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakeInfo {
    pub balance: Uint128,
    pub reward: Uint128,
    pub index: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardInfo {
    pub last_update: Timestamp,
    pub cur_sum_index: Uint128,
    pub rps: Uint128,
    pub total_stake: Uint128,
}

pub const USERS: Map<Addr, StakeInfo> = Map::new("users");
pub const CONFIG: Item<Config> = Item::new("config");
pub const REWARD: Item<RewardInfo> = Item::new("reward");
pub const ORACLE: Item<Uint128> = Item::new("oracle");



