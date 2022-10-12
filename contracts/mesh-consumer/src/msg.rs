use std::collections::HashMap;

use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Coin, Decimal};

use crate::state::Config;

#[cw_serde]
pub struct InstantiateMsg {
    pub provider: ProviderInfo,
    pub remote_to_local_exchange_rate: Decimal,
    pub meta_staking_contract_address: String,
}

#[cw_serde]
pub struct ProviderInfo {
    pub port_id: String,
    pub connection_id: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    MeshConsumerRecieveRewardsMsg {
        rewards_by_validator: HashMap<String, Coin>,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    // Return configuration info
    #[returns(Config)]
    Config {},
}
