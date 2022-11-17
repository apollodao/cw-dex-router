use std::vec;

use cosmwasm_schema::cw_serde;
use cw20::{Cw20Coin, Cw20ExecuteMsg};
use cw_asset::{AssetInfo, AssetInfoBase, AssetList};

use cosmwasm_std::{
    to_binary, Addr, Api, Coin, CosmosMsg, QuerierWrapper, QueryRequest, StdResult, Uint128,
    WasmMsg, WasmQuery,
};

use crate::{
    msg::{ExecuteMsg, QueryMsg},
    operations::SwapOperationsList,
};

#[cw_serde]
pub struct CwDexRouterBase<T>(pub T);

pub type CwDexRouterUnchecked = CwDexRouterBase<String>;
pub type CwDexRouter = CwDexRouterBase<Addr>;

impl From<CwDexRouter> for CwDexRouterUnchecked {
    fn from(x: CwDexRouter) -> Self {
        CwDexRouterBase(x.0.to_string())
    }
}

impl CwDexRouterUnchecked {
    pub fn check(&self, api: &dyn Api) -> StdResult<CwDexRouter> {
        Ok(CwDexRouter::new(&api.addr_validate(&self.0)?))
    }
}

impl CwDexRouter {
    pub fn new(contract_addr: &Addr) -> Self {
        Self(contract_addr.clone())
    }

    pub fn addr(&self) -> Addr {
        self.0.clone()
    }

    pub fn call<T: Into<ExecuteMsg>>(&self, msg: T, funds: Vec<Coin>) -> StdResult<CosmosMsg> {
        let msg = to_binary(&msg.into())?;
        Ok(WasmMsg::Execute {
            contract_addr: self.addr().into(),
            msg,
            funds,
        }
        .into())
    }

    pub fn execute_swap_operations_msg(
        &self,
        operations: &SwapOperationsList,
        offer_amount: Option<Uint128>,
        minimum_receive: Option<Uint128>,
        to: Option<String>,
        funds: Vec<Coin>,
    ) -> StdResult<CosmosMsg> {
        self.call(
            ExecuteMsg::ExecuteSwapOperations {
                operations: operations.into(),
                offer_amount,
                minimum_receive,
                to,
            },
            funds,
        )
    }

    /// Returns message to call BasketLiquidate, as well as approve spend on any
    /// CW20s in `offer_assets`. Also takes care of sending native tokens in
    /// `offer_assets` to the contract via the funds field.
    pub fn basket_liquidate_msgs(
        &self,
        offer_assets: AssetList,
        receive_asset: &AssetInfo,
        minimum_receive: Option<Uint128>,
        to: Option<String>,
    ) -> StdResult<Vec<CosmosMsg>> {
        //Extract all native tokens to send in funds field.
        let funds: Vec<Coin> = offer_assets
            .into_iter()
            .filter_map(|x| match x.info {
                cw_asset::AssetInfoBase::Native(_) => Some(x.try_into()),
                _ => None,
            })
            .collect::<StdResult<Vec<_>>>()?;

        //Extract all cw20s and approve allowance to router.
        let mut msgs: Vec<CosmosMsg> = offer_assets
            .into_iter()
            .filter_map(|x| match &x.info {
                AssetInfoBase::Cw20(addr) => Some(Cw20Coin {
                    address: addr.to_string(),
                    amount: x.amount,
                }),
                _ => None,
            })
            .map(|x| {
                Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: x.address,
                    msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                        spender: self.addr().to_string(),
                        amount: x.amount,
                        expires: None,
                    })?,
                    funds: vec![],
                }))
            })
            .collect::<StdResult<Vec<_>>>()?;

        let swap_msg = self.call(
            ExecuteMsg::BasketLiquidate {
                offer_assets: offer_assets.into(),
                receive_asset: receive_asset.to_owned().into(),
                minimum_receive,
                to,
            },
            funds,
        )?;
        msgs.push(swap_msg);

        Ok(msgs)
    }

    pub fn update_path_msg(
        &self,
        offer_asset: AssetInfo,
        ask_aaset: AssetInfo,
        path: &SwapOperationsList,
    ) -> StdResult<CosmosMsg> {
        self.call(
            ExecuteMsg::UpdatePath {
                offer_asset: offer_asset.into(),
                ask_asset: ask_aaset.into(),
                path: path.into(),
            },
            vec![],
        )
    }

    pub fn simulate_swap_operations(
        &self,
        querier: &QuerierWrapper,
        offer_amount: Uint128,
        operations: &SwapOperationsList,
        sender: Option<String>,
    ) -> StdResult<Uint128> {
        querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: self.0.to_string(),
            msg: to_binary(&QueryMsg::SimulateSwapOperations {
                offer_amount,
                operations: operations.into(),
                sender,
            })?,
        }))
    }

    pub fn simulate_basket_liquidate(
        &self,
        querier: &QuerierWrapper,
        offer_assets: AssetList,
        receive_asset: &AssetInfo,
        sender: Option<String>,
    ) -> StdResult<Uint128> {
        querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: self.0.to_string(),
            msg: to_binary(&QueryMsg::SimulateBasketLiquidate {
                offer_assets: offer_assets.into(),
                receive_asset: receive_asset.to_owned().into(),
                sender,
            })?,
        }))
    }

    pub fn query_path_for_pair(
        &self,
        querier: &QuerierWrapper,
        offer_asset: &AssetInfo,
        ask_asset: &AssetInfo,
    ) -> StdResult<SwapOperationsList> {
        querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: self.0.to_string(),
            msg: to_binary(&QueryMsg::PathForPair {
                offer_asset: offer_asset.to_owned().into(),
                ask_asset: ask_asset.to_owned().into(),
            })?,
        }))
    }

    pub fn query_supported_offer_assets(
        &self,
        querier: &QuerierWrapper,
        ask_asset: &AssetInfo,
    ) -> StdResult<Vec<AssetInfo>> {
        querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: self.0.to_string(),
            msg: to_binary(&QueryMsg::SupportedOfferAssets {
                ask_asset: ask_asset.to_owned().into(),
            })?,
        }))
    }

    pub fn query_supported_ask_assets(
        &self,
        querier: &QuerierWrapper,
        offer_asset: &AssetInfo,
    ) -> StdResult<Vec<AssetInfo>> {
        querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: self.0.to_string(),
            msg: to_binary(&QueryMsg::SupportedAskAssets {
                offer_asset: offer_asset.to_owned().into(),
            })?,
        }))
    }
}
