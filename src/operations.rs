use crate::msg::CallbackMsg;
use crate::ContractError;
use apollo_cw_asset::{Asset, AssetInfo, AssetInfoBase};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, CosmosMsg, Deps, Env, Response, Uint128};
use cw_dex::traits::Pool as PoolTrait;
use cw_dex::Pool;

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
    pub fn check(&self, deps: Deps) -> Result<SwapOperation, ContractError> {
        let op = SwapOperation {
            ask_asset_info: self.ask_asset_info.check(deps.api)?,
            offer_asset_info: self.offer_asset_info.check(deps.api)?,
            pool: self.pool.clone(),
        };
        // validate pool assets
        let pool_assets = op.pool.pool_assets(deps)?;
        if !pool_assets.contains(&op.offer_asset_info) || !pool_assets.contains(&op.ask_asset_info)
        {
            Err(ContractError::InvalidSwapOperations {
                operations: vec![op],
            })
        } else {
            Ok(op)
        }
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

    pub fn check(&self, deps: Deps) -> Result<SwapOperationsList, ContractError> {
        let operations = self
            .0
            .iter()
            .map(|x| x.check(deps))
            .collect::<Result<Vec<_>, ContractError>>()?;

        if operations.is_empty() {
            return Err(ContractError::MustProvideOperations);
        }

        let mut prev_ask_asset = operations.first().unwrap().ask_asset_info.clone();
        for operation in operations.iter().skip(1) {
            if operation.offer_asset_info != prev_ask_asset {
                return Err(ContractError::InvalidSwapOperations { operations });
            }
            prev_ask_asset = operation.ask_asset_info.clone();
        }

        // Check that the path never swaps through the same pool twice
        let mut unique_pools = vec![];
        for operation in operations.iter() {
            if !unique_pools.contains(&operation.pool) {
                unique_pools.push(operation.pool.clone());
            } else {
                return Err(ContractError::InvalidSwapOperations { operations });
            }
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
            .iter()
            .cloned()
            .map(|op| {
                let mut op = op;
                let tmp = op.offer_asset_info.clone();
                op.offer_asset_info = op.ask_asset_info.clone();
                op.ask_asset_info = tmp;
                op
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

#[cfg(feature = "osmosis")]
#[cfg(test)]
mod unit_tests {
    use crate::operations::{SwapOperation, SwapOperationsList};
    use apollo_cw_asset::AssetInfo;
    use cw_dex::osmosis::OsmosisPool;
    use cw_dex::Pool;

    #[test]
    fn test_reverse() {
        let ops = SwapOperationsList::new(vec![
            SwapOperation::new(
                Pool::Osmosis(OsmosisPool::unchecked(1)),
                AssetInfo::Native("asset1".to_string()),
                AssetInfo::Native("asset2".to_string()),
            ),
            SwapOperation::new(
                Pool::Osmosis(OsmosisPool::unchecked(2)),
                AssetInfo::Native("asset2".to_string()),
                AssetInfo::Native("asset3".to_string()),
            ),
            SwapOperation::new(
                Pool::Osmosis(OsmosisPool::unchecked(3)),
                AssetInfo::Native("asset3".to_string()),
                AssetInfo::Native("asset4".to_string()),
            ),
        ]);

        let reversed = ops.reverse();

        assert_eq!(
            reversed,
            SwapOperationsList::new(vec![
                SwapOperation::new(
                    Pool::Osmosis(OsmosisPool::unchecked(3)),
                    AssetInfo::Native("asset4".to_string()),
                    AssetInfo::Native("asset3".to_string()),
                ),
                SwapOperation::new(
                    Pool::Osmosis(OsmosisPool::unchecked(2)),
                    AssetInfo::Native("asset3".to_string()),
                    AssetInfo::Native("asset2".to_string()),
                ),
                SwapOperation::new(
                    Pool::Osmosis(OsmosisPool::unchecked(1)),
                    AssetInfo::Native("asset2".to_string()),
                    AssetInfo::Native("asset1".to_string()),
                )
            ])
        )
    }
}
