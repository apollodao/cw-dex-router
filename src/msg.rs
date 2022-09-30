use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{wasm_execute, Addr, CosmosMsg, Decimal, Deps, Empty, Env, Response, Uint128};
use cw20::Cw20ReceiveMsg;
use cw_asset::{Asset, AssetInfo};
use cw_dex::osmosis::OsmosisPool;
use cw_dex::Pool as PoolTrait;

use crate::ContractError;

pub type InstantiateMsg = Empty;

#[cw_serde]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    ExecuteSwapOperations {
        operations: Vec<SwapOperation>,
        /// Optional because we only need the information if the user wants to
        /// swap a Cw20 with TransferFrom
        offer_amount: Option<Uint128>,
        minimum_receive: Option<Uint128>,
        to: Option<String>,
        /// TODO: Doc comment. Diff to min receive?
        max_spread: Option<Decimal>,
    },
    Callback(CallbackMsg),
}

#[cw_serde]
pub enum CallbackMsg {
    ExecuteSwapOperation {
        operation: SwapOperation,
        to: Addr,
        max_spread: Option<Decimal>,
    },
    AssertMinimumReceive {
        asset_info: AssetInfo,
        prev_balance: Uint128,
        minimum_receive: Uint128,
        recipient: Addr,
    },
}

impl CallbackMsg {
    pub fn into_cosmos_msg(&self, env: &Env) -> Result<CosmosMsg, ContractError> {
        Ok(wasm_execute(env.contract.address.to_string(), &self, vec![])?.into())
    }
}

#[cw_serde]
pub enum Cw20HookMsg {
    ExecuteSwapOperations {
        operations: Vec<SwapOperation>,
        /// Optional because we only need the information if the user wants to
        /// swap a Cw20 with TransferFrom
        minimum_receive: Option<Uint128>,
        to: Option<String>,
        /// TODO: Doc comment. Diff to min receive?
        max_spread: Option<Decimal>,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Uint128)]
    SimulateSwapOperations {
        offer_amount: Uint128,
        operations: Vec<SwapOperation>,
    },
}

#[cw_serde]
pub enum MigrateMsg {}

/// An enum with all known variants that implement the cw_dex::Pool trait.
/// The ideal solution would of course instead be to use a trait object so that
/// the caller can pass in any type that implements the Pool trait, but trait
/// objects require us not to implement the Sized trait, which cw_serde requires.
#[cw_serde]
pub enum Pool {
    Osmosis(OsmosisPool),
}

impl Pool {
    pub fn as_trait(&self) -> &dyn PoolTrait {
        match self {
            Pool::Osmosis(x) => x as &dyn PoolTrait,
        }
    }
}

#[cw_serde]
pub struct SwapOperation {
    pub pool: Pool,
    pub offer_asset_info: AssetInfo,
    pub ask_asset_info: AssetInfo,
}

impl SwapOperation {
    pub fn to_cosmos_response(
        &self,
        deps: Deps,
        offer_amount: Uint128,
        minimum_receive: Option<Uint128>,
        recipient: Addr,
    ) -> Result<Response, ContractError> {
        let offer = Asset::new(self.offer_asset_info.clone(), offer_amount);
        let ask = Asset::new(
            self.ask_asset_info.clone(),
            minimum_receive.unwrap_or_default(), //TODO: Should swap on pool trait really take an asset? Maybe better with separate min_receive parameter
        );
        Ok(self.pool.as_trait().swap(deps, offer, ask, recipient)?)
    }
}
