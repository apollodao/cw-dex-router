use crate::msg::CallbackMsg;
use crate::ContractError;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Api, CosmosMsg, Deps, Env, Response, StdResult, Uint128};
use cw_asset::{Asset, AssetInfoBase};
use cw_dex::traits::Pool as PoolTrait;
use cw_dex::Pool;

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
            pool: self.pool.clone(),
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
        Ok(self.pool.swap(
            deps,
            offer_asset,
            self.ask_asset_info.clone(),
            minimum_receive,
            recipient,
        )?)
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

impl SwapOperationsList {
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
                .into_cosmos_msg(&env)?,
            )
        }
        Ok(msgs)
    }
}

impl From<&SwapOperationsList> for SwapOperationsListUnchecked {
    fn from(checked: &SwapOperationsList) -> Self {
        Self(checked.0.iter().map(|x| x.into()).collect())
    }
}

impl From<Vec<SwapOperation>> for SwapOperationsList {
    fn from(x: Vec<SwapOperation>) -> Self {
        Self(x)
    }
}
