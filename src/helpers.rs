use std::vec;

use cw_asset::{Asset, AssetInfo, AssetList};

use cosmwasm_std::{Coin, CosmosMsg, Env, MessageInfo, StdError, StdResult};

// /// CwTemplateContract is a wrapper around Addr that provides a lot of helpers
// /// for working with this. Rename it to your contract name.
// #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
// pub struct CwTemplateContract(pub Addr);

// impl CwTemplateContract {
//     pub fn addr(&self) -> Addr {
//         self.0.clone()
//     }

//     pub fn call<T: Into<ExecuteMsg>>(&self, msg: T) -> StdResult<CosmosMsg> {
//         let msg = to_binary(&msg.into())?;
//         Ok(WasmMsg::Execute {
//             contract_addr: self.addr().into(),
//             msg,
//             funds: vec![],
//         }
//         .into())
//     }

//     /// Get Custom
//     pub fn custom_query<Q, T, CQ>(&self, querier: &Q, val: String) -> StdResult<CustomResponse>
//     where
//         Q: Querier,
//         T: Into<String>,
//         CQ: CustomQuery,
//     {
//         let msg = QueryMsg::CustomMsg { val };
//         let query = WasmQuery::Smart {
//             contract_addr: self.addr().into(),
//             msg: to_binary(&msg)?,
//         }
//         .into();
//         let res: CustomResponse = QuerierWrapper::<CQ>::new(querier).query(&query)?;
//         Ok(res)
//     }
// }

/// Assert that a specific native token in the form of an `Asset` was sent to the contract.
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
        _ => Err(StdError::generic_err("Unsupported asset type")),
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
