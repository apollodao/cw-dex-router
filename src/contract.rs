use apollo_cw_asset::{Asset, AssetInfo, AssetInfoUnchecked, AssetList, AssetListUnchecked};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Env, Event, MessageInfo, Order,
    Response, StdError, StdResult, Uint128,
};
use cw2::set_contract_version;
use cw20::Cw20ReceiveMsg;

use crate::error::ContractError;
use crate::helpers::{receive_asset, receive_assets};
use crate::msg::{
    BestPathForPairResponse, CallbackMsg, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg,
    QueryMsg,
};
use crate::operations::{
    SwapOperation, SwapOperationsList, SwapOperationsListBase, SwapOperationsListUnchecked,
};
use crate::state::{ADMIN, PATHS};

const CONTRACT_NAME: &str = "crates.io:cw-dex-router";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    ADMIN.set(deps, Some(info.sender))?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::ExecuteSwapOperations {
            operations,
            offer_amount,
            minimum_receive,
            to,
        } => {
            let operations = operations.check(deps.as_ref())?;
            execute_swap_operations(
                deps,
                env,
                info.clone(),
                info.sender,
                operations,
                offer_amount,
                minimum_receive,
                to,
            )
        }
        // ExecuteMsg::BasketLiquidate {
        //     offer_assets,
        //     receive_asset,
        //     minimum_receive,
        //     to,
        // } => {
        //     let api = deps.api;
        //     basket_liquidate(
        //         deps,
        //         env,
        //         info,
        //         offer_assets.check(api)?,
        //         receive_asset.check(api)?,
        //         minimum_receive,
        //         to,
        //     )
        // }
        ExecuteMsg::SetPath {
            offer_asset,
            ask_asset,
            path,
            bidirectional,
        } => {
            let path = path.check(deps.as_ref())?;
            let api = deps.api;
            set_path(
                deps,
                info,
                offer_asset.check(api)?,
                ask_asset.check(api)?,
                path,
                bidirectional,
            )
        }
        ExecuteMsg::Callback(msg) => {
            if info.sender != env.contract.address {
                return Err(ContractError::Unauthorized);
            }
            match msg {
                CallbackMsg::ExecuteSwapOperation { operation, to } => {
                    execute_swap_operation(deps, env, operation, to)
                }
                CallbackMsg::AssertMinimumReceive {
                    asset_info,
                    prev_balance,
                    minimum_receive,
                    recipient,
                } => assert_minimum_receive(
                    deps,
                    asset_info,
                    prev_balance,
                    minimum_receive,
                    recipient,
                ),
            }
        }
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let sender = deps.api.addr_validate(&cw20_msg.sender)?;

    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::ExecuteSwapOperations {
            operations,
            minimum_receive,
            to,
        } => {
            let operations = operations.check(deps.as_ref())?;
            execute_swap_operations(
                deps,
                env,
                info,
                sender,
                operations,
                None,
                minimum_receive,
                to,
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn execute_swap_operations(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    sender: Addr,
    operations: SwapOperationsList,
    offer_amount: Option<Uint128>,
    minimum_receive: Option<Uint128>,
    to: Option<String>,
) -> Result<Response, ContractError> {
    //Validate input or use sender address if None
    let recipient = to.map_or(Ok(sender), |x| deps.api.addr_validate(&x))?;

    let target_asset_info = operations.to();
    let offer_asset_info = operations.from();

    // 1. Validate sent asset. We only do this if the passed in optional
    // `offer_amount` and in this case we do transfer from on it, given that
    // the offer asset is a CW20. Otherwise we assume the caller already sent
    // funds and in the first call of execute_swap_operation, we just use the
    // whole contracts balance.
    let mut msgs: Vec<CosmosMsg> = vec![];
    if let Some(offer_amount) = offer_amount {
        msgs.extend(receive_asset(
            &info,
            &env,
            &Asset::new(offer_asset_info, offer_amount),
        )?);
    };

    // 2. Loop and execute swap operations
    let mut msgs: Vec<CosmosMsg> = operations.into_execute_msgs(&env, recipient.clone())?;

    // 3. Assert min receive
    if let Some(minimum_receive) = minimum_receive {
        let recipient_balance =
            target_asset_info.query_balance(&deps.querier, recipient.clone())?;
        msgs.push(
            CallbackMsg::AssertMinimumReceive {
                asset_info: target_asset_info,
                prev_balance: recipient_balance,
                minimum_receive,
                recipient,
            }
            .into_cosmos_msg(&env)?,
        );
    }
    Ok(Response::new().add_messages(msgs))
}

pub fn execute_swap_operation(
    deps: DepsMut,
    env: Env,
    operation: SwapOperation,
    to: Addr,
) -> Result<Response, ContractError> {
    //We use all of the contracts balance.
    let offer_amount = operation
        .offer_asset_info
        .query_balance(&deps.querier, env.contract.address.to_string())?;

    if offer_amount.is_zero() {
        return Ok(Response::default());
    }

    let event = Event::new("apollo/cw-dex-router/callback_execute_swap_operation")
        .add_attribute("operation", format!("{:?}", operation))
        .add_attribute("offer_amount", offer_amount)
        .add_attribute("to", to.to_string());

    Ok(operation
        .to_cosmos_response(deps.as_ref(), &env, offer_amount, None, to)?
        .add_event(event))
}

pub fn assert_minimum_receive(
    deps: DepsMut,
    asset_info: AssetInfo,
    prev_balance: Uint128,
    minimum_receive: Uint128,
    recipient: Addr,
) -> Result<Response, ContractError> {
    let recipient_balance = asset_info.query_balance(&deps.querier, recipient)?;

    let received_amount = recipient_balance.checked_sub(prev_balance)?;

    if received_amount < minimum_receive {
        return Err(ContractError::FailedMinimumReceive);
    }
    Ok(Response::default())
}

pub fn set_path(
    deps: DepsMut,
    info: MessageInfo,
    offer_asset: AssetInfo,
    ask_asset: AssetInfo,
    path: SwapOperationsList,
    bidirectional: bool,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender)?;

    // Validate the path
    if path.from() != offer_asset || path.to() != ask_asset {
        return Err(ContractError::InvalidSwapOperations {
            operations: path.into(),
        });
    }

    // check if we have any exisiting items under the offer_asset, ask_asset pair
    // we are looking for the highest ID so we can increment it, this should be under Order::Descending in the first item
    let ps: Result<Vec<(u64, SwapOperationsList)>, StdError> = PATHS
        .prefix((offer_asset.clone().into(), ask_asset.clone().into()))
        .range(deps.storage, None, None, Order::Descending)
        .collect();
    let paths = ps?;
    let last_id = paths.first().map(|(val, _)| val).unwrap_or(&0);

    let new_id = last_id + 1;
    PATHS.save(
        deps.storage,
        ((&offer_asset).into(), (&ask_asset).into(), new_id),
        &path,
    )?;

    // reverse path and store if `bidirectional` is true
    if bidirectional {
        let ps: Result<Vec<(u64, SwapOperationsList)>, StdError> = PATHS
            .prefix((ask_asset.clone().into(), offer_asset.clone().into()))
            .range(deps.storage, None, None, Order::Descending)
            .collect();
        let paths = ps?;
        let last_id = paths.first().map(|(val, _)| val).unwrap_or(&0);

        let new_id = last_id + 1;
        PATHS.save(
            deps.storage,
            (ask_asset.into(), offer_asset.into(), new_id),
            &path.reverse(),
        )?;
    }

    Ok(Response::default())
}

// pub fn basket_liquidate(
//     deps: DepsMut,
//     env: Env,
//     info: MessageInfo,
//     offer_assets: AssetList,
//     receive_asset: AssetInfo,
//     minimum_receive: Option<Uint128>,
//     to: Option<String>,
// ) -> Result<Response, ContractError> {
//     //Validate input or use sender address if None
//     let recipient = to.map_or(Ok(info.sender.clone()), |x| deps.api.addr_validate(&x))?;

//     // 1. Assert offer_assets are sent or do TransferFrom on Cw20s
//     let receive_msgs = receive_assets(&info, &env, &offer_assets)?;

//     // 2. Loop over offer assets and for each:
//     // Fetch path and call ExecuteMsg::ExecuteSwapOperations
//     let mut msgs = offer_assets
//         .into_iter()
//         .try_fold(vec![], |mut msgs, asset| {
//             // TODO we should fetch and compare paths here
//             let path = PATHS
//                 .load(
//                     deps.storage,
//                     (asset.info.clone().into(), receive_asset.clone().into()),
//                 )
//                 .map_err(|_| ContractError::NoPathFound {
//                     offer: asset.info.to_string(),
//                     ask: receive_asset.to_string(),
//                 })?;
//             msgs.extend(path.into_execute_msgs(&env, recipient.clone())?);
//             Ok::<Vec<_>, ContractError>(msgs)
//         })?;

//     // 3. Assert min receive
//     if let Some(minimum_receive) = minimum_receive {
//         let recipient_balance = receive_asset.query_balance(&deps.querier, recipient.clone())?;
//         msgs.push(
//             CallbackMsg::AssertMinimumReceive {
//                 asset_info: receive_asset.clone(),
//                 prev_balance: recipient_balance,
//                 minimum_receive,
//                 recipient: recipient.clone(),
//             }
//             .into_cosmos_msg(&env)?,
//         );
//     }

//     let event = Event::new("apollo/cw-dex-router/basket_liquidate")
//         .add_attribute("offer_assets", offer_assets.to_string())
//         .add_attribute("receive_asset", receive_asset.to_string())
//         .add_attribute("minimum_receive", minimum_receive.unwrap_or_default())
//         .add_attribute("recipient", recipient);

//     Ok(Response::new()
//         .add_messages(receive_msgs)
//         .add_messages(msgs)
//         .add_event(event))
// }

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::SimulateSwapOperations {
            offer_amount,
            operations,
        } => to_binary(&simulate_swap_operations(deps, offer_amount, operations)?),
        // QueryMsg::SimulateBasketLiquidate {
        //     offer_assets,
        //     receive_asset,
        // } => to_binary(&simulate_basket_liquidate(
        //     deps,
        //     offer_assets,
        //     receive_asset,
        // )?),
        QueryMsg::PathsForPair {
            offer_asset,
            ask_asset,
        } => to_binary(&query_paths_for_pair(
            deps,
            offer_asset.check(deps.api)?,
            ask_asset.check(deps.api)?,
        )?),
        QueryMsg::BestPathForPair {
            offer_asset,
            offer_amount,
            ask_asset,
            exclude_paths,
        } => to_binary(&query_best_path_for_pair(
            deps,
            offer_amount,
            offer_asset.check(deps.api)?,
            ask_asset.check(deps.api)?,
            exclude_paths,
        )?),
        QueryMsg::SupportedOfferAssets { ask_asset } => {
            to_binary(&query_supported_offer_assets(deps, ask_asset)?)
        }
        QueryMsg::SupportedAskAssets { offer_asset } => {
            to_binary(&query_supported_ask_assets(deps, offer_asset)?)
        }
    }
}

pub fn simulate_swap_operations(
    deps: Deps,
    mut offer_amount: Uint128,
    operations: SwapOperationsListUnchecked,
) -> Result<Uint128, ContractError> {
    let operations = operations.check(deps)?;

    for operation in operations.into_iter() {
        let offer_asset = Asset::new(operation.offer_asset_info, offer_amount);

        offer_amount =
            operation
                .pool
                .as_trait()
                .simulate_swap(deps, offer_asset, operation.ask_asset_info)?;
    }

    Ok(offer_amount)
}

// todo, decide whether I care about basket liquidate in the router
// pub fn simulate_basket_liquidate(
//     deps: Deps,
//     offer_assets: AssetListUnchecked,
//     receive_asset: AssetInfoUnchecked,
// ) -> Result<Uint128, ContractError> {
//     let offer_assets = offer_assets.check(deps.api)?;
//     let receive_asset = receive_asset.check(deps.api)?;

//     let mut receive_amount = Uint128::zero();

//     // Loop over offer assets and fetch path for each
//     // for each set of paths between to assets, figure out what the best path is
//     let paths = offer_assets
//         .into_iter()
//         .map(|asset| {
//             Ok::<_, ContractError>((
//                 asset.clone(),
//                 query_paths_for_pair(deps, asset.info.clone(), receive_asset.clone())?,
//             ))
//         })
//         .collect::<Result<Vec<(Asset, SwapOperationsList, _)>, _>>()?;

//     // Loop over paths and simulate swap operations
//     for (asset, path) in paths {
//         receive_amount += simulate_swap_operations(deps, asset.amount, path.into())?;
//     }

//     Ok(receive_amount)
// }

pub fn query_paths_for_pair(
    deps: Deps,
    offer_asset: AssetInfo,
    ask_asset: AssetInfo,
) -> Result<Vec<(u64, SwapOperationsList)>, ContractError> {
    let ps: StdResult<Vec<(u64, SwapOperationsList)>> = PATHS
        .prefix(((&offer_asset).into(), (&ask_asset).into()))
        .range(deps.storage, None, None, Order::Ascending)
        .collect();
    let paths = ps?;
    if paths.is_empty() {
        Err(ContractError::NoPathFound {
            offer: offer_asset.to_string(),
            ask: ask_asset.to_string(),
        })
    } else {
        Ok(paths)
    }
}

pub fn query_best_path_for_pair(
    deps: Deps,
    offer_amount: Uint128,
    offer_asset: AssetInfo,
    ask_asset: AssetInfo,
    exclude_paths: Option<Vec<u64>>,
) -> Result<Option<BestPathForPairResponse>, ContractError> {
    let paths = query_paths_for_pair(deps, offer_asset, ask_asset)?;
    let excluded = exclude_paths.unwrap_or(vec![]);
    let paths: Vec<(u64, SwapOperationsList)> = paths
        .into_iter()
        .filter(|(id, _)| excluded.contains(id))
        .collect();
    let swap_paths: Result<Vec<BestPathForPairResponse>, ContractError> = paths
        .into_iter()
        .map(|(id, swaps)| {
            let out = simulate_swap_operations(deps, offer_amount, swaps.clone().into())?;
            Ok(BestPathForPairResponse {
                operations: swaps,
                return_amount: out,
            })
        })
        .collect();

    let best_path = swap_paths?
        .into_iter()
        .max_by(|a, b| a.return_amount.cmp(&b.return_amount));

    Ok(best_path)
}

pub fn query_supported_offer_assets(
    deps: Deps,
    ask_asset: AssetInfoUnchecked,
) -> Result<Vec<AssetInfo>, ContractError> {
    let mut offer_assets: Vec<AssetInfo> = vec![];
    for x in PATHS.range(deps.storage, None, None, Order::Ascending) {
        let ((offer_asset, path_ask_asset, _), _) = x?;
        if path_ask_asset == ask_asset.check(deps.api)? {
            offer_assets.push(offer_asset.into());
        }
    }
    Ok(offer_assets)
}

pub fn query_supported_ask_assets(
    deps: Deps,
    offer_asset: AssetInfoUnchecked,
) -> Result<Vec<AssetInfo>, ContractError> {
    let mut ask_assets: Vec<AssetInfo> = vec![];
    for x in PATHS.range(deps.storage, None, None, Order::Ascending) {
        let ((path_offer_asset, ask_asset, _), _) = x?;
        if path_offer_asset == offer_asset.check(deps.api)? {
            ask_assets.push(ask_asset.into());
        }
    }
    Ok(ask_assets)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    Ok(Response::default())
}
