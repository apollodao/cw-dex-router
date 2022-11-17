#[cfg(test)]
mod unit_tests {
    use crate::contract::{basket_liquidate, update_path};
    use crate::msg::{CallbackMsg, ExecuteMsg};
    use crate::operations::{SwapOperation, SwapOperationsList};
    use crate::state::ADMIN;
    use cosmwasm_std::testing::{
        mock_dependencies, mock_dependencies_with_balances, mock_env, mock_info, MOCK_CONTRACT_ADDR,
    };
    use cosmwasm_std::{wasm_execute, Addr, Coin, CosmosMsg, ReplyOn, SubMsg, Uint128};
    use cw_asset::{Asset, AssetInfo, AssetList};
    use cw_dex::osmosis::OsmosisPool;
    use cw_dex::Pool;

    #[test]
    fn test_basket_liquidate() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let admin_info = mock_info("admin", &[]);

        // set admin
        ADMIN
            .set(deps.as_mut(), Some(admin_info.sender.to_owned()))
            .unwrap();

        // add swap path
        let add_swap_path_res = update_path(
            deps.as_mut(),
            admin_info.to_owned(),
            AssetInfo::native("osmo".to_string()),
            AssetInfo::native("usdc".to_string()),
            SwapOperationsList::from(vec![
                SwapOperation {
                    pool: Pool::Osmosis(OsmosisPool { pool_id: 1 }),
                    offer_asset_info: AssetInfo::native("osmo".to_string()),
                    ask_asset_info: AssetInfo::native("ion".to_string()),
                },
                SwapOperation {
                    pool: Pool::Osmosis(OsmosisPool { pool_id: 2 }),
                    offer_asset_info: AssetInfo::native("ion".to_string()),
                    ask_asset_info: AssetInfo::native("atom".to_string()),
                },
                SwapOperation {
                    pool: Pool::Osmosis(OsmosisPool { pool_id: 3 }),
                    offer_asset_info: AssetInfo::native("atom".to_string()),
                    ask_asset_info: AssetInfo::native("usdc".to_string()),
                },
            ]),
        );
        assert!(add_swap_path_res.is_ok());

        // execute basket liquidate
        let swap_res = basket_liquidate(
            deps.as_mut(),
            env,
            mock_info(
                "user",
                &[Coin {
                    denom: "osmo".to_string(),
                    amount: Uint128::new(1000000000),
                }],
            ),
            AssetList::from(vec![Asset::native(
                "osmo".to_string(),
                Uint128::new(1000000000),
            )]),
            AssetInfo::native("usdc".to_string()),
            Some(Uint128::new(2000000000)),
            None,
        );
        assert!(swap_res.is_ok());
        let swap_res = swap_res.unwrap();

        // check response messages
        assert_eq!(swap_res.messages.len(), 4);
        assert_eq!(
            swap_res.messages,
            vec![
                SubMsg {
                    id: 0,
                    msg: wasm_execute(
                        MOCK_CONTRACT_ADDR,
                        &ExecuteMsg::Callback(CallbackMsg::ExecuteSwapOperation {
                            operation: SwapOperation {
                                pool: Pool::Osmosis(OsmosisPool { pool_id: 1 }),
                                offer_asset_info: AssetInfo::native("osmo".to_string()),
                                ask_asset_info: AssetInfo::native("ion".to_string()),
                            },
                            to: Addr::unchecked(MOCK_CONTRACT_ADDR),
                        }),
                        vec![]
                    )
                    .unwrap()
                    .into(),
                    gas_limit: None,
                    reply_on: ReplyOn::Never
                },
                SubMsg {
                    id: 0,
                    msg: wasm_execute(
                        MOCK_CONTRACT_ADDR,
                        &ExecuteMsg::Callback(CallbackMsg::ExecuteSwapOperation {
                            operation: SwapOperation {
                                pool: Pool::Osmosis(OsmosisPool { pool_id: 2 }),
                                offer_asset_info: AssetInfo::native("ion".to_string()),
                                ask_asset_info: AssetInfo::native("atom".to_string()),
                            },
                            to: Addr::unchecked(MOCK_CONTRACT_ADDR),
                        }),
                        vec![]
                    )
                    .unwrap()
                    .into(),
                    gas_limit: None,
                    reply_on: ReplyOn::Never
                },
                SubMsg {
                    id: 0,
                    msg: wasm_execute(
                        MOCK_CONTRACT_ADDR,
                        &ExecuteMsg::Callback(CallbackMsg::ExecuteSwapOperation {
                            operation: SwapOperation {
                                pool: Pool::Osmosis(OsmosisPool { pool_id: 3 }),
                                offer_asset_info: AssetInfo::native("atom".to_string()),
                                ask_asset_info: AssetInfo::native("usdc".to_string()),
                            },
                            to: Addr::unchecked("user"),
                        }),
                        vec![]
                    )
                    .unwrap()
                    .into(),
                    gas_limit: None,
                    reply_on: ReplyOn::Never
                },
                SubMsg {
                    id: 0,
                    msg: wasm_execute(
                        MOCK_CONTRACT_ADDR,
                        &ExecuteMsg::Callback(CallbackMsg::AssertMinimumReceive {
                            asset_info: AssetInfo::native("usdc".to_string()),
                            prev_balance: Uint128::zero(),
                            minimum_receive: Uint128::new(2000000000),
                            recipient: Addr::unchecked("user"),
                        }),
                        vec![]
                    )
                    .unwrap()
                    .into(),
                    gas_limit: None,
                    reply_on: ReplyOn::Never
                },
            ]
        )
    }
}
