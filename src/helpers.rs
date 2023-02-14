use std::vec;

use apollo_cw_asset::{Asset, AssetInfo, AssetInfoBase, AssetList};
use apollo_utils::assets::separate_natives_and_cw20s;
use cosmwasm_schema::cw_serde;
use cw20::{Cw20Coin, Cw20ExecuteMsg};

use cosmwasm_std::{
    to_binary, Addr, Api, Coin, CosmosMsg, Env, MessageInfo, QuerierWrapper, QueryRequest,
    StdError, StdResult, Uint128, WasmMsg, WasmQuery,
};

use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::operations::SwapOperationsList;

#[cw_serde]
pub struct CwDexRouterBase<T>(pub T);

pub type CwDexRouterUnchecked = CwDexRouterBase<String>;
pub type CwDexRouter = CwDexRouterBase<Addr>;

impl From<CwDexRouter> for CwDexRouterUnchecked {
    fn from(x: CwDexRouter) -> Self {
        CwDexRouterBase(x.0.to_string())
    }
}

impl<T> From<T> for CwDexRouterBase<T> {
    fn from(x: T) -> Self {
        CwDexRouterBase(x)
    }
}

impl CwDexRouterUnchecked {
    pub const fn new(addr: String) -> Self {
        CwDexRouterBase(addr)
    }

    pub fn check(&self, api: &dyn Api) -> StdResult<CwDexRouter> {
        Ok(CwDexRouter::new(&api.addr_validate(&self.0)?))
    }

    pub fn instantiate(
        code_id: u64,
        admin: Option<String>,
        label: Option<String>,
    ) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Instantiate {
            code_id,
            admin,
            msg: to_binary(&InstantiateMsg {})?,
            funds: vec![],
            label: label.unwrap_or_else(|| "cw-dex-router".to_string()),
        }))
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
        let (funds, _) = separate_natives_and_cw20s(&offer_assets);

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

    pub fn set_path_msg(
        &self,
        offer_asset: AssetInfo,
        ask_asset: AssetInfo,
        path: &SwapOperationsList,
        bidirectional: bool,
    ) -> StdResult<CosmosMsg> {
        self.call(
            ExecuteMsg::SetPath {
                offer_asset: offer_asset.into(),
                ask_asset: ask_asset.into(),
                path: path.into(),
                bidirectional,
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

/// Assert that a specific native token in the form of an `Asset` was sent to
/// the contract.
pub fn assert_native_token_received(info: &MessageInfo, asset: &Asset) -> StdResult<()> {
    let coin: Coin = asset.try_into()?;

    if !info.funds.contains(&coin) {
        return Err(StdError::generic_err(format!(
            "Assert native token receive failed for asset: {}",
            asset
        )));
    }
    Ok(())
}

/// Calls TransferFrom on an Asset if it is a Cw20. If it is a native we just
/// assert that the native token was already sent to the contract.
pub fn receive_asset(info: &MessageInfo, env: &Env, asset: &Asset) -> StdResult<Vec<CosmosMsg>> {
    match &asset.info {
        AssetInfo::Cw20(_coin) => {
            let msg =
                asset.transfer_from_msg(info.sender.clone(), env.contract.address.to_string())?;
            Ok(vec![msg])
        }
        AssetInfo::Native(_token) => {
            //Here we just assert that the native token was sent with the contract call
            assert_native_token_received(info, asset)?;
            Ok(vec![])
        }
    }
}

pub fn receive_assets(
    info: &MessageInfo,
    env: &Env,
    assets: &AssetList,
) -> StdResult<Vec<CosmosMsg>> {
    assets.into_iter().try_fold(vec![], |mut msgs, asset| {
        msgs.append(&mut receive_asset(info, env, asset)?);
        Ok(msgs)
    })
}
