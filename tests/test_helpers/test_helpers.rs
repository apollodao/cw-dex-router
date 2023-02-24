use apollo_cw_asset::{AssetInfo, AssetList};
use cosmwasm_std::Uint128;
use cw_dex_router::{msg::QueryMsg, operations::SwapOperationsList};
use osmosis_test_tube::{Module, Runner, RunnerResult, Wasm};

pub fn query_path_for_pair<'a, R>(
    app: &'a R,
    contract_addr: &str,
    offer_asset: &AssetInfo,
    ask_asset: &AssetInfo,
) -> RunnerResult<SwapOperationsList>
where
    R: Runner<'a>,
{
    let wasm = Wasm::new(app);
    wasm.query::<_, SwapOperationsList>(
        contract_addr,
        &QueryMsg::PathForPair {
            offer_asset: offer_asset.to_owned().into(),
            ask_asset: ask_asset.to_owned().into(),
        },
    )
}

pub fn simulate_basket_liquidate<'a, R>(
    app: &'a R,
    contract_addr: &str,
    offer_assets: AssetList,
    receive_asset: &AssetInfo,
    sender: Option<String>,
) -> RunnerResult<Uint128>
where
    R: Runner<'a>,
{
    let wasm = Wasm::new(app);
    wasm.query::<_, Uint128>(
        contract_addr,
        &QueryMsg::SimulateBasketLiquidate {
            offer_assets: offer_assets.into(),
            receive_asset: receive_asset.to_owned().into(),
            sender,
        },
    )
}

pub fn simulate_swap_operations<'a, R>(
    app: &'a R,
    contract_addr: &str,
    offer_amount: Uint128,
    operations: &SwapOperationsList,
    sender: Option<String>,
) -> RunnerResult<Uint128>
where
    R: Runner<'a>,
{
    let wasm = Wasm::new(app);
    wasm.query::<_, Uint128>(
        contract_addr,
        &QueryMsg::SimulateSwapOperations {
            offer_amount,
            operations: operations.into(),
            sender,
        },
    )
}

pub fn query_supported_offer_assets<'a, R>(
    app: &'a R,
    contract_addr: &str,
    ask_asset: &AssetInfo,
) -> RunnerResult<Vec<AssetInfo>>
where
    R: Runner<'a>,
{
    let wasm = Wasm::new(app);
    wasm.query::<_, Vec<AssetInfo>>(
        contract_addr,
        &QueryMsg::SupportedOfferAssets {
            ask_asset: ask_asset.to_owned().into(),
        },
    )
}

pub fn query_supported_ask_assets<'a, R>(
    app: &'a R,
    contract_addr: &str,
    offer_asset: &AssetInfo,
) -> RunnerResult<Vec<AssetInfo>>
where
    R: Runner<'a>,
{
    let wasm = Wasm::new(app);
    wasm.query::<_, Vec<AssetInfo>>(
        contract_addr,
        &QueryMsg::SupportedAskAssets {
            offer_asset: offer_asset.to_owned().into(),
        },
    )
}
