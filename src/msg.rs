use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{
    wasm_execute, Addr, Api, CosmosMsg, Deps, Empty, Env, Response, StdResult, Uint128,
};
use cw20::Cw20ReceiveMsg;
use cw_asset::{Asset, AssetInfo, AssetInfoBase};
use cw_dex::osmosis::OsmosisPool;
use cw_dex::Pool as PoolTrait;

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
    },
}

#[cw_serde]
pub enum MigrateMsg {}

/// An enum with all known variants that implement the cw_dex::Pool trait.
/// The ideal solution would of course instead be to use a trait object so that
/// the caller can pass in any type that implements the Pool trait, but trait
/// objects require us not to implement the Sized trait, which cw_serde requires.
#[cw_serde]
#[derive(Copy)]
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
pub struct SwapOperationBase<T> {
    pub pool: Pool,
    pub offer_asset_info: AssetInfoBase<T>,
    pub ask_asset_info: AssetInfoBase<T>,
}

pub type SwapOperationUnchecked = SwapOperationBase<String>;
pub type SwapOperation = SwapOperationBase<Addr>;

impl SwapOperationUnchecked {
    pub fn check(&self, api: &dyn Api) -> StdResult<SwapOperation> {
        Ok(SwapOperation {
            ask_asset_info: self.ask_asset_info.check(api, None)?,
            offer_asset_info: self.offer_asset_info.check(api, None)?,
            pool: self.pool,
        })
    }
}

impl SwapOperation {
    pub fn to_cosmos_response(
        &self,
        deps: Deps,
        offer_amount: Uint128,
        minimum_receive: Option<Uint128>,
        recipient: Addr,
    ) -> Result<Response, ContractError> {
        let offer_asset = Asset::new(self.offer_asset_info.clone(), offer_amount);
        let minimum_receive = minimum_receive.unwrap_or_default();
        Ok(self.pool.as_trait().swap(
            deps,
            offer_asset,
            self.ask_asset_info.clone(),
            minimum_receive,
            recipient,
        )?)
    }
}

#[cw_serde]
pub struct SwapOperationsListBase<T>(pub Vec<SwapOperationBase<T>>);

pub type SwapOperationsListUnchecked = SwapOperationsListBase<String>;
pub type SwapOperationsList = SwapOperationsListBase<Addr>;

impl SwapOperationsListUnchecked {
    pub fn check(&self, api: &dyn Api) -> Result<SwapOperationsList, ContractError> {
        let operations = self
            .0
            .iter()
            .map(|x| x.check(api))
            .collect::<StdResult<Vec<_>>>()?;

        if operations.len() < 1 {
            return Err(ContractError::MustProvideOperations);
        }

        let mut last_offer_asset = operations.first().unwrap().offer_asset_info.clone();
        for operation in operations.iter().skip(1) {
            if operation.ask_asset_info != last_offer_asset {
                return Err(ContractError::InvalidSwapOperations);
            }
            last_offer_asset = operation.offer_asset_info.clone();
        }

        Ok(SwapOperationsListBase(operations))
    }
}
