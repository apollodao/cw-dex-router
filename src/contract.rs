#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, Response, StdResult, Uint128,
};
use cw2::set_contract_version;
use cw20::Cw20ReceiveMsg;
use cw_asset::{Asset, AssetInfo, AssetInfoBase};

use crate::error::ContractError;
use crate::msg::{
    CallbackMsg, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, SwapOperation,
    SwapOperationsListUnchecked,
};

const CONTRACT_NAME: &str = "crates.io:cw-dex-router";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
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
            max_spread,
        } => execute_swap_operations(
            deps,
            env,
            info.clone(),
            info.sender,
            operations,
            offer_amount,
            minimum_receive,
            to,
            max_spread,
        ),
        ExecuteMsg::Callback(msg) => {
            if info.sender != env.contract.address {
                return Err(ContractError::Unauthorized);
            }
            match msg {
                CallbackMsg::ExecuteSwapOperation {
                    operation,
                    to,
                    max_spread,
                } => execute_swap_operation(deps, env, operation, to, max_spread),
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
            max_spread,
        } => execute_swap_operations(
            deps,
            env,
            info,
            sender,
            operations,
            None,
            minimum_receive,
            to,
            max_spread,
        ),
    }
}

pub fn execute_swap_operations(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    sender: Addr,
    operations: SwapOperationsListUnchecked,
    offer_amount: Option<Uint128>,
    minimum_receive: Option<Uint128>,
    to: Option<String>,
    max_spread: Option<Decimal>,
) -> Result<Response, ContractError> {
    //Validate input or use sender address if None
    let recipient = to.map_or(Ok(sender.clone()), |x| deps.api.addr_validate(&x))?;

    //1. Validate operations
    let operations = operations.check(deps.api)?;
    let operations_len = operations.0.len();

    let target_asset_info = operations.0.last().unwrap().ask_asset_info.clone();
    let offer_asset_info = operations.0.first().unwrap().offer_asset_info.clone();

    //2. Validate sent asset. We only do this if the passed in optional `offer_amount`
    //   and in this case we do transfer from on it, given that the offer asset is
    //   a CW20. Otherwise we assume the caller already sent funds and in the first
    //   call of execute_swap_operation, we just use the whole contracts balance.
    let mut msgs: Vec<CosmosMsg> = vec![];
    if let Some(offer_amount) = offer_amount {
        match offer_asset_info {
            AssetInfoBase::Cw20(_) => {
                let msg = Asset::new(offer_asset_info, offer_amount)
                    .transfer_from_msg(sender, env.contract.address.to_string())?;
                msgs.push(msg);
            }
            AssetInfoBase::Native(denom) => {
                if !info.funds.contains(&Coin::new(offer_amount.into(), denom)) {
                    return Err(ContractError::IncorrectNativeAmountSent);
                }
            }
            _ => return Err(ContractError::UnsupportedAssetType),
        }
    };

    //2. Loop and execute swap operations
    let mut msgs: Vec<CosmosMsg> = operations
        .0
        .into_iter()
        .enumerate()
        .map(|(i, operation)| {
            //Always send assets to self except for last operation
            let to = if i == operations_len - 1 {
                recipient.clone()
            } else {
                env.contract.address.clone()
            };
            CallbackMsg::ExecuteSwapOperation {
                operation,
                to,
                max_spread,
            }
            .into_cosmos_msg(&env)
        })
        .collect::<Result<Vec<_>, _>>()?;

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
    _max_spread: Option<Decimal>, //TODO: Use max spread
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

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::SimulateSwapOperations {
            offer_amount,
            operations,
        } => to_binary(&simulate_swap_operations(
            deps,
            env,
            offer_amount,
            operations,
        )?),
    }
}

pub fn simulate_swap_operations(
    _deps: Deps,
    _env: Env,
    offer_amount: Uint128,
    operations: SwapOperationsListUnchecked,
) -> Result<Uint128, ContractError> {
    let mut _offer_amount = offer_amount;
    for _operation in operations.0 {
        // TODO: Must add simulate_swap on Pool Trait
        // operation.pool.as_trait().simulate_swap(deps, offer, ask, recipient)
    }

    Ok(Uint128::default())
}

//TODO: Write tests
#[cfg(test)]
mod tests {}
