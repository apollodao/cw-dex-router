use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{wasm_execute, Addr, CosmosMsg, Empty, Env, Uint128};
use cw20::Cw20ReceiveMsg;
use cw_asset::{AssetInfo, AssetInfoUnchecked, AssetListUnchecked};

use crate::operations::{SwapOperation, SwapOperationsListUnchecked};
use crate::ContractError;

pub type InstantiateMsg = Empty;

#[cw_serde]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    ExecuteSwapOperations {
        operations: SwapOperationsListUnchecked,
        /// Optional because we only need the information if the user wants to
        /// swap a Cw20 with TransferFrom
        offer_amount: Option<Uint128>,
        minimum_receive: Option<Uint128>,
        to: Option<String>,
    },
    BasketLiquidate {
        offer_assets: AssetListUnchecked,
        receive_asset: AssetInfoUnchecked,
        minimum_receive: Option<Uint128>,
        to: Option<String>,
    },
    UpdatePath {
        offer_asset: AssetInfoUnchecked,
        ask_asset: AssetInfoUnchecked,
        path: SwapOperationsListUnchecked,
    },
    Callback(CallbackMsg),
}

#[cw_serde]
pub enum CallbackMsg {
    ExecuteSwapOperation {
        operation: SwapOperation,
        to: Addr,
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
        operations: SwapOperationsListUnchecked,
        minimum_receive: Option<Uint128>,
        to: Option<String>,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Uint128)]
    SimulateSwapOperations {
        offer_amount: Uint128,
        operations: SwapOperationsListUnchecked,
        // For some reason osmosis requires this to simulate a swap...
        // TODO: Find a way around this and remove it from the API.
        sender: Option<String>,
    },

    /// Returns all the assets from which there are paths to a given ask asset.
    #[returns(Vec<AssetInfo>)]
    SupportedOfferAssets { ask_asset: AssetInfoUnchecked },
}

#[cw_serde]
pub enum MigrateMsg {}
