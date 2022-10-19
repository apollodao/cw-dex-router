#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult, Uint128,
};
use cw2::set_contract_version;
use cw20::Cw20ReceiveMsg;
use cw_asset::{Asset, AssetInfo, AssetList};

use crate::error::ContractError;
use crate::helpers::{receive_asset, receive_assets};
use crate::msg::{CallbackMsg, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::operations::{SwapOperation, SwapOperationsList, SwapOperationsListUnchecked};
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
            let api = deps.api;
            execute_swap_operations(
                deps,
                env,
                info.clone(),
                info.sender,
                operations.check(api)?,
                offer_amount,
                minimum_receive,
                to,
            )
        }
        ExecuteMsg::BasketLiquidate {
            offer_assets,
            receive_asset,
            minimum_receive,
            to,
        } => {
            let api = deps.api;
            basket_liquidate(
                deps,
                env,
                info,
                offer_assets.check(api, None)?,
                receive_asset.check(api, None)?,
                minimum_receive,
                to,
            )
        }
        ExecuteMsg::UpdatePath {
            offer_asset,
            ask_asset,
            path,
        } => {
            let api = deps.api;
            update_path(
                deps,
                info,
                offer_asset.check(api, None)?,
                ask_asset.check(api, None)?,
                path.check(api)?,
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

    let api = deps.api;

    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::ExecuteSwapOperations {
            operations,
            minimum_receive,
            to,
        } => execute_swap_operations(
            deps,
            env,
            info,
            sender,
            operations.check(api)?,
            None,
            minimum_receive,
            to,
        ),
    }
}

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
    let recipient = to.map_or(Ok(sender.clone()), |x| deps.api.addr_validate(&x))?;

    let target_asset_info = operations.0.last().unwrap().ask_asset_info.clone();
    let offer_asset_info = operations.0.first().unwrap().offer_asset_info.clone();

    //1. Validate sent asset. We only do this if the passed in optional `offer_amount`
    //   and in this case we do transfer from on it, given that the offer asset is
    //   a CW20. Otherwise we assume the caller already sent funds and in the first
    //   call of execute_swap_operation, we just use the whole contracts balance.
    let mut msgs: Vec<CosmosMsg> = vec![];
    if let Some(offer_amount) = offer_amount {
        msgs.extend(receive_asset(
            &info,
            &env,
            &Asset::new(offer_asset_info, offer_amount),
        )?);
    };

    //2. Loop and execute swap operations
    let mut msgs: Vec<CosmosMsg> = operations.into_execute_msgs(&env, recipient.clone())?;

    //3. Assert min receive
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
        .query_balance(&deps.querier, env.contract.address)?;

    operation.to_cosmos_response(deps.as_ref(), offer_amount, None, to)
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

pub fn update_path(
    deps: DepsMut,
    info: MessageInfo,
    offer_asset: AssetInfo,
    ask_asset: AssetInfo,
    path: SwapOperationsList,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender)?;

    PATHS.save(deps.storage, (offer_asset.into(), ask_asset.into()), &path)?;
    Ok(Response::default())
}

pub fn basket_liquidate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    offer_assets: AssetList,
    receive_asset: AssetInfo,
    minimum_receive: Option<Uint128>,
    to: Option<String>,
) -> Result<Response, ContractError> {
    //Validate input or use sender address if None
    let recipient = to.map_or(Ok(info.sender.clone()), |x| deps.api.addr_validate(&x))?;

    // 1. Assert offer_assets are sent or do TransferFrom on Cw20s
    let receive_msgs = receive_assets(&info, &env, &offer_assets)?;

    // 2. Loop over offer assets and for each:
    // Fetch path and call ExecuteMsg::ExecuteSwapOperations
    let mut msgs = offer_assets
        .into_iter()
        .try_fold(vec![], |mut msgs, asset| {
            let path = PATHS
                .load(
                    deps.storage,
                    (asset.info.clone().into(), receive_asset.clone().into()),
                )
                .map_err(|_| ContractError::NoPathFound)?;
            msgs.extend(path.into_execute_msgs(&env, recipient.clone())?);
            Ok::<Vec<_>, ContractError>(msgs)
        })?;

    //3. Assert min receive
    if let Some(minimum_receive) = minimum_receive {
        let recipient_balance = receive_asset.query_balance(&deps.querier, recipient.clone())?;
        msgs.push(
            CallbackMsg::AssertMinimumReceive {
                asset_info: receive_asset,
                prev_balance: recipient_balance,
                minimum_receive,
                recipient,
            }
            .into_cosmos_msg(&env)?,
        );
    }

    Ok(Response::new().add_messages(receive_msgs))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::SimulateSwapOperations {
            offer_amount,
            operations,
            sender,
        } => to_binary(&simulate_swap_operations(
            deps,
            offer_amount,
            operations,
            sender,
        )?),
    }
}

pub fn simulate_swap_operations(
    deps: Deps,
    mut offer_amount: Uint128,
    operations: SwapOperationsListUnchecked,
    sender: Option<String>,
) -> Result<Uint128, ContractError> {
    let operations = operations.check(deps.api)?;

    for operation in operations.0 {
        let offer_asset = Asset::new(operation.offer_asset_info, offer_amount);

        offer_amount = operation.pool.as_trait().simulate_swap(
            deps,
            offer_asset,
            operation.ask_asset_info,
            Uint128::zero(),
            sender.clone(),
        )?;
    }

    Ok(offer_amount)
}

//TODO: Write tests
#[cfg(test)]
mod tests {}
