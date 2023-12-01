use apollo_cw_asset::{AssetInfo, AssetInfoUnchecked, AssetListUnchecked};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{wasm_execute, Addr, CosmosMsg, Empty, Env, Uint128};
use cw20::Cw20ReceiveMsg;

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
    // BasketLiquidate {
    //     offer_assets: AssetListUnchecked,
    //     receive_asset: AssetInfoUnchecked,
    //     minimum_receive: Option<Uint128>,
    //     to: Option<String>,
    // },
    SetPath {
        offer_asset: AssetInfoUnchecked,
        ask_asset: AssetInfoUnchecked,
        path: SwapOperationsListUnchecked,
        bidirectional: bool,
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
        Ok(wasm_execute(
            env.contract.address.to_string(),
            &ExecuteMsg::Callback(self.clone()),
            vec![],
        )?
        .into())
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
pub struct BestPathForPairResponse {
    /// the operations that will be executed to perform the swap
    pub operations: crate::operations::SwapOperationsList,
    /// the amount of tokens that are expected to be received after the swap
    pub return_amount: Uint128,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Uint128)]
    SimulateSwapOperations {
        offer_amount: Uint128,
        operations: SwapOperationsListUnchecked,
    },

    // #[returns(Uint128)]
    // SimulateBasketLiquidate {
    //     offer_assets: AssetListUnchecked,
    //     receive_asset: AssetInfoUnchecked,
    // },
    /// Returns all the current path for a given (offer_asset, ask_asset) pair.
    #[returns(Vec<crate::operations::SwapOperationsList>)]
    PathsForPair {
        offer_asset: AssetInfoUnchecked,
        ask_asset: AssetInfoUnchecked,
    },
    /// finds the best path for a given (offer_asset, ask_asset) pair.
    /// if no path is found, returns None.
    #[returns(Option<BestPathForPairResponse>)]
    BestPathForPair {
        offer_asset: AssetInfoUnchecked,
        offer_amount: Uint128,
        ask_asset: AssetInfoUnchecked,
        exclude_paths: Option<Vec<u64>>,
    },

    /// Returns all the assets from which there are paths to a given ask asset.
    #[returns(Vec<AssetInfo>)]
    SupportedOfferAssets { ask_asset: AssetInfoUnchecked },

    /// Returns all the assets to which there are paths from a given offer
    /// asset.
    #[returns(Vec<AssetInfo>)]
    SupportedAskAssets { offer_asset: AssetInfoUnchecked },
}

#[cw_serde]
pub struct MigrateMsg {}
