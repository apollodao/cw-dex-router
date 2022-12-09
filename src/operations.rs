use crate::msg::CallbackMsg;
use crate::ContractError;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Api, CosmosMsg, Deps, Env, Response, StdResult, Uint128};
use cw_asset::{Asset, AssetInfo, AssetInfoBase};
use cw_dex::traits::Pool as PoolTrait;
use cw_dex::Pool;
use std::collections::HashSet;

#[cw_serde]
pub struct SwapOperationBase<T> {
    pub pool: Pool,
    pub offer_asset_info: AssetInfoBase<T>,
    pub ask_asset_info: AssetInfoBase<T>,
}

impl<T> SwapOperationBase<T> {
    pub fn new(
        pool: Pool,
        offer_asset_info: AssetInfoBase<T>,
        ask_asset_info: AssetInfoBase<T>,
    ) -> Self {
        Self {
            pool,
            offer_asset_info,
            ask_asset_info,
        }
    }
}

pub type SwapOperationUnchecked = SwapOperationBase<String>;

pub type SwapOperation = SwapOperationBase<Addr>;

impl SwapOperationUnchecked {
    pub fn check(&self, api: &dyn Api) -> StdResult<SwapOperation> {
        Ok(SwapOperation {
            ask_asset_info: self.ask_asset_info.check(api)?,
            offer_asset_info: self.offer_asset_info.check(api)?,
            pool: self.pool.clone(),
        })
    }
}

impl SwapOperation {
    pub fn to_cosmos_response(
        &self,
        deps: Deps,
        env: &Env,
        offer_amount: Uint128,
        minimum_receive: Option<Uint128>,
        recipient: Addr,
    ) -> Result<Response, ContractError> {
        let offer_asset = Asset::new(self.offer_asset_info.clone(), offer_amount);
        let minimum_receive = minimum_receive.unwrap_or(Uint128::one());

        let mut response = self.pool.swap(
            deps,
            env,
            offer_asset.clone(),
            self.ask_asset_info.clone(),
            minimum_receive,
        )?;

        if recipient != env.contract.address {
            // Simulate swap to know how much will be returned, then add message
            // to send tokens to recipient
            let receive_amount = self.pool.simulate_swap(
                deps,
                offer_asset,
                self.ask_asset_info.clone(),
                Some(env.contract.address.to_string()),
            )?;
            let receive_asset = Asset::new(self.ask_asset_info.clone(), receive_amount);
            response = response.add_message(receive_asset.transfer_msg(recipient)?);
        }

        Ok(response)
    }
}

impl From<&SwapOperation> for SwapOperationUnchecked {
    fn from(checked: &SwapOperation) -> Self {
        Self {
            ask_asset_info: checked.ask_asset_info.clone().into(),
            offer_asset_info: checked.offer_asset_info.clone().into(),
            pool: checked.pool.clone(),
        }
    }
}

#[cw_serde]
pub struct SwapOperationsListBase<T>(Vec<SwapOperationBase<T>>);

impl<T> IntoIterator for SwapOperationsListBase<T> {
    type Item = SwapOperationBase<T>;
    type IntoIter = std::vec::IntoIter<SwapOperationBase<T>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

pub type SwapOperationsListUnchecked = SwapOperationsListBase<String>;

pub type SwapOperationsList = SwapOperationsListBase<Addr>;

impl SwapOperationsListUnchecked {
    pub fn new(operations: Vec<SwapOperationUnchecked>) -> Self {
        Self(operations)
    }

    pub fn check(&self, api: &dyn Api) -> Result<SwapOperationsList, ContractError> {
        let mut assets_to_swap = HashSet::new();
        // should not visit the same asset more than once
        let mut visited_assets = HashSet::new();
        let operations = self
            .0
            .iter()
            .map(|x| x.check(api))
            .collect::<StdResult<Vec<_>>>()?;

        if operations.is_empty() {
            return Err(ContractError::MustProvideOperations);
        }

        let mut prev_ask_asset = operations.first().unwrap().ask_asset_info.clone();
        visited_assets.insert(operations.first().unwrap().offer_asset_info.to_string());
        visited_assets.insert(prev_ask_asset.to_string());
        assets_to_swap.insert(prev_ask_asset.to_string());
        for operation in operations.iter().skip(1) {
            if operation.offer_asset_info != prev_ask_asset
                || visited_assets.contains(&operation.ask_asset_info.to_string())
            {
                return Err(ContractError::InvalidSwapOperations { operations });
            }
            prev_ask_asset = operation.ask_asset_info.clone();
            assets_to_swap.remove(&operation.offer_asset_info.to_string());
            assets_to_swap.insert(operation.ask_asset_info.to_string());
            visited_assets.insert(operation.ask_asset_info.to_string());
        }

        // should be left with one asset after swap operations are complete
        if visited_assets.len() != 1 {
            return Err(ContractError::InvalidSwapOperations { operations });
        }

        Ok(SwapOperationsListBase(operations))
    }
}

impl SwapOperationsList {
    pub fn new(operations: Vec<SwapOperation>) -> Self {
        Self(operations)
    }

    pub fn reverse(&self) -> Self {
        let mut operations = self
            .0
            .to_vec()
            .into_iter()
            .map(|op| {
                let mut op = op.clone();
                let tmp = op.offer_asset_info.clone();
                op.offer_asset_info = op.ask_asset_info.clone();
                op.ask_asset_info = tmp;
                op.clone()
            })
            .collect::<Vec<SwapOperation>>();
        operations.reverse();
        Self::new(operations)
    }

    pub fn into_execute_msgs(
        &self,
        env: &Env,
        recipient: Addr,
    ) -> Result<Vec<CosmosMsg>, ContractError> {
        let operations_len = self.0.len();
        let mut msgs = vec![];
        for (i, operation) in self.0.iter().enumerate() {
            //Always send assets to self except for last operation
            let to = if i == operations_len - 1 {
                recipient.clone()
            } else {
                env.contract.address.clone()
            };
            msgs.push(
                CallbackMsg::ExecuteSwapOperation {
                    operation: operation.clone(),
                    to,
                }
                .into_cosmos_msg(env)?,
            )
        }
        Ok(msgs)
    }

    pub fn from(&self) -> AssetInfo {
        self.0.first().unwrap().offer_asset_info.clone()
    }

    pub fn to(&self) -> AssetInfo {
        self.0.last().unwrap().ask_asset_info.clone()
    }
}

impl From<SwapOperationsList> for Vec<SwapOperation> {
    fn from(operations: SwapOperationsList) -> Self {
        operations.0
    }
}

impl From<&SwapOperationsList> for SwapOperationsListUnchecked {
    fn from(checked: &SwapOperationsList) -> Self {
        Self(checked.0.iter().map(|x| x.into()).collect())
    }
}
impl From<SwapOperationsList> for SwapOperationsListUnchecked {
    fn from(checked: SwapOperationsList) -> Self {
        (&checked).into()
    }
}
