#[cfg(feature = "osmosis")]
#[allow(unused_imports)]
#[allow(dead_code)]
mod osmosis_tests {
    use std::collections::HashMap;

    use std::str::FromStr;

    use cosmwasm_std::{Addr, Api};

    use cosmwasm_std::{Coin, CosmosMsg};

    use cosmwasm_std::{QuerierWrapper, StdError, StdResult, Uint128};

    use apollo_cw_asset::{AssetInfo, AssetInfoUnchecked};
    use cw_dex_osmosis::OsmosisPool;
    use cw_dex_router::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};

    use cw_dex_router::operations::{Pool, SwapOperation, SwapOperationsList};

    use cw_dex_router::helpers::{CwDexRouter, CwDexRouterUnchecked};

    use cw_it::cosmrs::Any;
    use cw_it::osmosis_std::types::cosmos::bank::v1beta1::QueryBalanceRequest;
    use cw_it::osmosis_test_tube::{Gamm, OsmosisTestApp};
    use cw_it::test_tube::{Bank, Module, RunnerResult, SigningAccount, Wasm};
    use cw_it::traits::CwItRunner;

    use cw_it::{self, Artifact, ContractType};

    use test_case::test_case;

    const ARTIFACTS_DIR: &str = "target/wasm32-unknown-unknown/release";

    pub type ConstPaths<'a> = &'a [((&'a str, &'a str), &'a [(u64, &'a str, &'a str)])];

    fn cw_dex_router_wasm_path() -> String {
        println!("{}/cw_dex_router.wasm", ARTIFACTS_DIR);
        format!("{}/cw_dex_router.wasm", ARTIFACTS_DIR)
    }

    fn cw_dex_router_contract() -> ContractType {
        ContractType::Artifact(Artifact::Local(cw_dex_router_wasm_path()))
    }

    fn upload_wasm_file<'a, R: CwItRunner<'a>>(
        runner: &'a R,
        contract: ContractType,
        signer: &SigningAccount,
    ) -> u64 {
        runner.store_code(contract, signer).unwrap()
    }

    fn instantiate_cw_dex_router<'a, R: CwItRunner<'a>>(
        runner: &'a R,
        signer: &SigningAccount,
        code_id: u64,
    ) -> RunnerResult<String> {
        let wasm = Wasm::new(runner);
        let contract_addr = wasm
            .instantiate(
                code_id,
                &InstantiateMsg {},
                None,
                Some("cw-dex-router"),
                &[],
                signer,
            )?
            .data
            .address;

        Ok(contract_addr)
    }

    /// Admin account is always the first account in the list
    fn setup() -> (OsmosisTestApp, Vec<SigningAccount>, u64) {
        let app = OsmosisTestApp::new();

        let accs = app
            .init_accounts(
                &[
                    Coin::new(1_000_000_000_000, UATOM),
                    Coin::new(1_000_000_000_000, UOSMO),
                    Coin::new(1_000_000_000_000, UION),
                ],
                10,
            )
            .unwrap();

        // Create pools and add liquidity for the paths defined as constants
        for path in &[
            UOSMO_UATOM_PATH.to_vec(),
            UION_UATOM_PATH.to_vec(),
            UOSMO_UATOM_UION_PATH.to_vec(),
        ] {
            for pool in path {
                let pool_liquidity = vec![
                    Coin {
                        denom: pool.1.to_string(),
                        amount: Uint128::from(1000000u128),
                    },
                    Coin {
                        denom: pool.2.to_string(),
                        amount: Uint128::from(1000000u128),
                    },
                ];
                create_basic_pool(&app, pool_liquidity, &accs[0]);
            }
        }

        let code_id = upload_wasm_file(&app, cw_dex_router_contract(), &accs[0]);

        (app, accs, code_id)
    }

    const UOSMO: &str = "uosmo";
    const UATOM: &str = "uatom";
    const UION: &str = "uion";

    const UOSMO_UATOM_PATH: &[(u64, &str, &str); 1] = &[(1, UOSMO, UATOM)];
    const UION_UATOM_PATH: &[(u64, &str, &str); 1] = &[(2, UION, UATOM)];
    const UOSMO_UATOM_UION_PATH: &[(u64, &str, &str); 2] = &[(1, UOSMO, UATOM), (2, UATOM, UION)];

    fn osmosis_swap_operations_list_from_vec(vec: &[(u64, &str, &str)]) -> SwapOperationsList {
        SwapOperationsList::new(
            vec.iter()
                .map(|(pool_id, from, to)| SwapOperation {
                    pool: Pool::Osmosis(OsmosisPool::unchecked(pool_id.to_owned())),
                    offer_asset_info: AssetInfo::Native(from.to_string()),
                    ask_asset_info: AssetInfo::Native(to.to_string()),
                })
                .collect(),
        )
    }

    fn set_paths<'a>(
        app: &'a impl CwItRunner<'a>,
        cw_dex_router_addr: &str,
        paths: ConstPaths,
        sender: &SigningAccount,
        bidirectional: bool,
    ) -> RunnerResult<()> {
        let wasm = Wasm::new(app);

        // Set paths
        for ((offer_asset, ask_asset), path) in paths {
            let offer_asset = AssetInfoUnchecked::Native(offer_asset.to_string());
            let ask_asset = AssetInfoUnchecked::Native(ask_asset.to_string());
            let path = osmosis_swap_operations_list_from_vec(path);
            wasm.execute(
                cw_dex_router_addr,
                &ExecuteMsg::SetPath {
                    offer_asset,
                    ask_asset,
                    path: path.into(),
                    bidirectional,
                },
                &[],
                sender,
            )?;
        }

        Ok(())
    }

    fn bank_balance_query<'a>(
        runner: &'a impl CwItRunner<'a>,
        address: String,
        denom: String,
    ) -> StdResult<Uint128> {
        let bank = Bank::new(runner);
        bank.query_balance(&QueryBalanceRequest { address, denom })
            .unwrap()
            .balance
            .map(|c| Uint128::from_str(&c.amount).unwrap())
            .ok_or(StdError::generic_err("Bank balance query failed"))
    }

    fn create_basic_pool<'a>(
        runner: &'a impl CwItRunner<'a>,
        pool_liquidity: Vec<Coin>,
        signer: &SigningAccount,
    ) -> OsmosisPool {
        let gamm = Gamm::new(runner);

        // Create 1:1 pool
        let pool_id = gamm
            .create_basic_pool(&pool_liquidity, signer)
            .unwrap()
            .data
            .pool_id;

        OsmosisPool::unchecked(pool_id)
    }

    #[test_case(&[((UOSMO, UATOM), UOSMO_UATOM_PATH)], false, UOSMO_UATOM_PATH, 1 => matches Err(_) ; "not admin")]
    #[test_case(&[((UOSMO, UATOM), UOSMO_UATOM_PATH)], false, UOSMO_UATOM_PATH, 0  ; "uosmo/uatom simple path")]
    #[test_case(&[((UOSMO, UION), UOSMO_UATOM_UION_PATH)], false, UOSMO_UATOM_UION_PATH, 0 ; "uosmo/uion two hops path")]
    #[test_case(&[((UOSMO, UATOM), UOSMO_UATOM_PATH)], true, UOSMO_UATOM_PATH, 0  ; "uosmo/uatom simple path bidirectional")]
    #[test_case(&[((UOSMO, UION), UOSMO_UATOM_UION_PATH)], true, UOSMO_UATOM_UION_PATH, 0 ; "uosmo/uion two hops path bidirectional")]
    #[test_case(&[((UOSMO, UION), &[(1337u64, UOSMO, UION)])], false, UOSMO_UATOM_UION_PATH, 0 => matches Err(_) ; "pool id does not exist")]
    #[test_case(&[((UION, UATOM), UOSMO_UATOM_PATH)], false, UOSMO_UATOM_UION_PATH, 0 => matches Err(_) ; "SwapOperation offer not in pool")]
    #[test_case(&[((UOSMO, UION), UOSMO_UATOM_PATH)], false, UOSMO_UATOM_UION_PATH, 0 => matches Err(_) ; "SwapOperation ask not in pool")]
    fn test_update_path_and_query_path_for_pair(
        paths: ConstPaths,
        bidirectional: bool,
        output_path: &[(u64, &str, &str)],
        sender_acc_nr: usize,
    ) -> RunnerResult<()> {
        let (app, accs, code_id) = setup();
        let wasm = Wasm::new(&app);

        let admin = &accs[0];
        let sender = &accs[sender_acc_nr];
        let cw_dex_router_addr = instantiate_cw_dex_router(&app, admin, code_id)?;

        // Set paths
        set_paths(&app, &cw_dex_router_addr, paths, sender, bidirectional)?;

        let expected_output_path = osmosis_swap_operations_list_from_vec(output_path);

        // Query path for pair
        let swap_operations: SwapOperationsList = wasm
            .query(
                &cw_dex_router_addr,
                &QueryMsg::PathForPair {
                    offer_asset: expected_output_path.from().into(),
                    ask_asset: expected_output_path.to().into(),
                },
            )
            .unwrap();

        assert_eq!(swap_operations, expected_output_path);

        if bidirectional {
            let swap_operations_reverse: SwapOperationsList = wasm
                .query(
                    &cw_dex_router_addr,
                    &QueryMsg::PathForPair {
                        offer_asset: expected_output_path.to().into(),
                        ask_asset: expected_output_path.from().into(),
                    },
                )
                .unwrap();
            assert_eq!(swap_operations_reverse, expected_output_path.reverse());
        }

        Ok(())
    }

    // Tests disabled due to breaking changes in cw-it
    // #[test_case(&[((UOSMO, UATOM), UOSMO_UATOM_PATH)], &[(UOSMO,
    // Uint128::from(1000u128))],         UATOM, None, None  ; "uosmo/uatom
    // simple path")] #[test_case(&[((UOSMO, UION), UOSMO_UATOM_UION_PATH)],
    // &[(UOSMO, Uint128::from(1000u128))],         UION, None, None ;
    // "uosmo/uion two hops path")] #[test_case(&[((UOSMO, UATOM),
    // UOSMO_UATOM_PATH), ((UION, UATOM), UION_UATOM_PATH)],         &[(UOSMO,
    // Uint128::from(1000u128)), (UION, Uint128::from(1000u128))],
    //         UATOM, None, None  ; "uosmo/uatom uion/uatom two liquidation paths")]
    // #[test_case(&[((UOSMO, UATOM), UOSMO_UATOM_PATH)], &[(UOSMO,
    // Uint128::from(1000u128))],         UATOM, Some(Uint128::from(989u128)),
    // None  ; "one path with min received")] #[test_case(&[((UOSMO, UATOM),
    // UOSMO_UATOM_PATH)], &[(UOSMO, Uint128::from(1000u128))], UATOM,
    //         Some(Uint128::from(990u128)), None => matches Err(_) ; "one path with
    // min received too low")] #[test_case(&[((UOSMO, UATOM), UOSMO_UATOM_PATH),
    // ((UION, UATOM), UION_UATOM_PATH)],         &[(UOSMO,
    // Uint128::from(1000u128)), (UION, Uint128::from(1000u128))],
    //         UATOM, Some(1978u128.into()), None  ; "two paths with min received")]
    // #[test_case(&[((UOSMO, UATOM), UOSMO_UATOM_PATH), ((UION, UATOM),
    // UION_UATOM_PATH)],         &[(UOSMO, Uint128::from(1000u128)), (UION,
    // Uint128::from(1000u128))],         UATOM, Some(1979u128.into()), None =>
    // matches Err(_) ; "two paths with min received too low")]
    // fn test_simulate_and_execute_basket_liquidate(
    //     paths: &[((&str, &str), &[(u64, &str, &str)])],
    //     offer_assets: &[(&str, Uint128)],
    //     receive_asset: &str,
    //     minimum_receive: Option<Uint128>,
    //     recipient_account_nr: Option<usize>,
    // ) -> RunnerResult<()> { let (app, api, accs, code_ids) = setup(); let admin =
    //   &accs[0]; let sender = &accs[1]; let recipient =
    //   recipient_account_nr.map(|nr| accs[nr].address());

    //     // Check input assets
    //     let offer_assets: AssetList = offer_assets
    //         .iter()
    //         .map(|(denom, amount)| {
    //             let asset_info = AssetInfoUnchecked::Native(denom.to_string())
    //                 .check(&api)
    //                 .unwrap();
    //             Asset::new(asset_info, *amount)
    //         })
    //         .collect::<Vec<_>>()
    //         .into();
    //     let receive_asset = AssetInfoUnchecked::Native(receive_asset.to_string())
    //         .check(&api)
    //         .unwrap();

    //     // Instantiate cw_dex_router
    //     let cw_dex_router =
    //         instantiate_cw_dex_router(&app, &api, admin,
    // code_ids["cw_dex_router.wasm"]).unwrap();

    //     // Set paths
    //     set_paths(&app, &api, &cw_dex_router, paths, admin, false).unwrap();

    //     // Create pools and add liquidity
    //     for path in paths {
    //         let pools = path.1;
    //         for pool in pools {
    //             let pool_liquidity = vec![
    //                 Coin {
    //                     denom: pool.1.to_string(),
    //                     amount: Uint128::from(1000000u128),
    //                 },
    //                 Coin {
    //                     denom: pool.2.to_string(),
    //                     amount: Uint128::from(1000000u128),
    //                 },
    //             ];
    //             let osmo_pool = create_basic_pool(&app, pool_liquidity, admin);
    //             println!("osmo pool: {:?}", osmo_pool);
    //             println!("pool: {:?}", pool);
    //         }
    //     }

    //     // Query all pools
    //     let gamm = Gamm::new(&app);
    //     let reserves = gamm.query_pool_reserves(1).unwrap();
    //     println!("reserves: {:?}", reserves);

    //     // Query recipient balance before swap
    //     let denom = match &receive_asset {
    //         AssetInfo::Native(denom) => denom.clone(),
    //         _ => panic!("Only native tokens are supported"),
    //     };
    //     let balance_before = bank_balance_query(
    //         &app,
    //         recipient.clone().unwrap_or(sender.address()),
    //         denom.clone(),
    //     )
    //     .unwrap();

    //     let bank = Bank::new(&app);
    //     let balances = bank
    //         .query_all_balances(&QueryAllBalancesRequest {
    //             address: recipient.clone().unwrap_or(sender.address()),
    //             pagination: None,
    //         })
    //         .unwrap();
    //     println!("balances before: {:?}", balances);

    //     // Simulate swap
    //     let querier = QuerierWrapper::new(&app);
    //     let expected_out = cw_dex_router
    //         .simulate_basket_liquidate(&querier, offer_assets.clone(),
    // &receive_asset)         .unwrap();
    //     println!("expected out: {:?}", expected_out);

    //     // Execute swap
    //     println!("offer_assets: {:?}", offer_assets);
    //     println!("receive_asset: {:?}", receive_asset);
    //     println!("minimum_receive: {:?}", minimum_receive);
    //     println!("recipient: {:?}", recipient);
    //     let liquidate_msgs = cw_dex_router
    //         .basket_liquidate_msgs(
    //             offer_assets,
    //             &receive_asset,
    //             minimum_receive,
    //             recipient.clone(),
    //         )
    //         .unwrap();
    //     println!("liquidate_msgs: {:?}", liquidate_msgs);
    //     println!("pre call");
    //     let res = app
    //         .execute_cosmos_msgs::<MsgExecuteContractResponse>(liquidate_msgs.
    // as_slice(), sender)?;     // print events
    //     println!("events: {:?}", res.events);
    //     println!("post call");

    //     // Query balance of recipient
    //     let balance_after =
    //         bank_balance_query(&app,
    // recipient.clone().unwrap_or(sender.address()), denom).unwrap();

    //     // query all balances
    //     let bank = Bank::new(&app);
    //     let balances = bank
    //         .query_all_balances(&QueryAllBalancesRequest {
    //             address: recipient.unwrap_or(sender.address()),
    //             pagination: None,
    //         })
    //         .unwrap();
    //     println!("balances after: {:?}", balances);

    //     // Check that simulation and execution are consistent
    //     assert_eq!(
    //         expected_out,
    //         balance_after.checked_sub(balance_before).unwrap()
    //     );

    //     Ok(())
    // }

    // #[test_case(&[((UOSMO, UATOM), UOSMO_UATOM_PATH)], UOSMO_UATOM_PATH,
    //         vec![Coin { denom: UOSMO.to_string(), amount: Uint128::new(1000u128)
    // }],         None, None  ; "uosmo/uatom simple path")]
    // #[test_case(&[((UOSMO, UION), UOSMO_UATOM_UION_PATH)], UOSMO_UATOM_UION_PATH,
    //         vec![Coin { denom: UOSMO.to_string(), amount: Uint128::new(1000u128)
    // }],         None, None  ; "uosmo/uatom + uion/uatom two paths")]
    // fn test_simulate_and_execute_swap_operations(
    //     paths: &[((&str, &str), &[(u64, &str, &str)])],
    //     swap_operations: &[(u64, &str, &str)],
    //     funds: Vec<Coin>,
    //     minimum_receive: Option<Uint128>,
    //     recipient_account_nr: Option<usize>,
    // ) -> RunnerResult<()> { let (app, api, accs, code_ids) = setup(); let admin =
    //   &accs[0]; let sender = &accs[1]; let recipient =
    //   recipient_account_nr.map(|i| accs[i].address());

    //     // Instantiate cw_dex_router
    //     let cw_dex_router =
    //         instantiate_cw_dex_router(&app, &api, admin,
    // code_ids["cw_dex_router.wasm"])?;

    //     // Set paths
    //     set_paths(&app, &api, &cw_dex_router, paths, admin, false)?;

    //     // Create pools and add liquidity
    //     for path in paths {
    //         let pools = path.1;
    //         for pool in pools {
    //             let pool_liquidity = vec![
    //                 Coin {
    //                     denom: pool.1.to_string(),
    //                     amount: Uint128::from(1000000u128),
    //                 },
    //                 Coin {
    //                     denom: pool.2.to_string(),
    //                     amount: Uint128::from(1000000u128),
    //                 },
    //             ];
    //             let osmo_pool = create_basic_pool(&app, pool_liquidity, admin);
    //             println!("osmo pool: {:?}", osmo_pool);
    //             println!("pool: {:?}", pool);
    //         }
    //     }

    //     // Simulate swap operations
    //     let operations = osmosis_swap_operations_list_from_vec(swap_operations);
    //     let expected_out = cw_dex_router.simulate_swap_operations(
    //         &QuerierWrapper::new(&app),
    //         funds[0].amount,
    //         &operations,
    //     )?;

    //     // Query out asset balances before swap
    //     let balance_before = bank_balance_query(
    //         &app,
    //         recipient.clone().unwrap_or(sender.address()),
    //         swap_operations.last().unwrap().2.to_string(),
    //     )?;

    //     // Execute swap operations
    //     // TODO: Do we need to test with offer_amount here?
    //     let msgs = cw_dex_router
    //         .execute_swap_operations_msg(
    //             &operations,
    //             None,
    //             minimum_receive,
    //             recipient.clone(),
    //             funds,
    //         )
    //         .unwrap();
    //     app.execute_cosmos_msgs::<Any>(&[msgs], sender)?;

    //     // Query out asset balances after swap
    //     let balance_after = bank_balance_query(
    //         &app,
    //         recipient.unwrap_or(sender.address()),
    //         swap_operations.last().unwrap().2.to_string(),
    //     )?;

    //     // Check that simulated and executed swap operations are equal
    //     assert_eq!(
    //         expected_out,
    //         balance_after.checked_sub(balance_before).unwrap()
    //     );

    //     Ok(())
    // }

    #[test_case(&[((UOSMO, UATOM), UOSMO_UATOM_PATH)], UOSMO, UATOM ; "uosmo/uatom simple path")]
    #[test_case(&[((UOSMO, UATOM), UOSMO_UATOM_PATH), ((UOSMO, UION), UOSMO_UATOM_UION_PATH)], UOSMO, UION ; "multiple paths")]
    fn test_supported_ask_and_offer_assets(
        paths: ConstPaths,
        offer_asset: &str,
        ask_asset: &str,
    ) -> RunnerResult<()> {
        let (app, accs, code_id) = setup();
        let admin = &accs[0];

        // Check input assets
        let offer_asset = AssetInfoUnchecked::Native(offer_asset.to_string());
        let ask_asset = AssetInfoUnchecked::Native(ask_asset.to_string());

        // Find expected offer and ask assets from paths
        let expected_offer_assets = paths
            .iter()
            .filter_map(|((offer, ask), _)| {
                if ask_asset == AssetInfoUnchecked::Native(ask.to_string()) {
                    Some(AssetInfo::Native(offer.to_string()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let expected_ask_assets = paths
            .iter()
            .filter_map(|((offer, ask), _)| {
                if offer_asset == AssetInfoUnchecked::Native(offer.to_string()) {
                    Some(AssetInfo::Native(ask.to_string()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        // Instantiate cw_dex_router
        let cw_dex_router_addr = instantiate_cw_dex_router(&app, admin, code_id)?;

        // Set paths
        set_paths(&app, &cw_dex_router_addr, paths, admin, false)?;

        // Create pools and add liquidity
        for path in paths {
            let pools = path.1;
            for pool in pools {
                let pool_liquidity = vec![
                    Coin {
                        denom: pool.1.to_string(),
                        amount: Uint128::from(1000000u128),
                    },
                    Coin {
                        denom: pool.2.to_string(),
                        amount: Uint128::from(1000000u128),
                    },
                ];
                let osmo_pool = create_basic_pool(&app, pool_liquidity, admin);
                println!("osmo pool: {:?}", osmo_pool);
                println!("pool: {:?}", pool);
            }
        }

        // Query supported offer assets
        let wasm = Wasm::new(&app);
        let supported_offer_assets: Vec<AssetInfo> = wasm.query(
            &cw_dex_router_addr,
            &QueryMsg::SupportedOfferAssets { ask_asset },
        )?;
        let supported_ask_assets: Vec<AssetInfo> = wasm.query(
            &cw_dex_router_addr,
            &QueryMsg::SupportedAskAssets { offer_asset },
        )?;

        println!("expected_offer_assets: {:?}", expected_offer_assets);
        println!("supported_offer_assets: {:?}", supported_offer_assets);
        println!("expected_ask_assets: {:?}", expected_ask_assets);
        println!("supported_ask_assets: {:?}", supported_ask_assets);

        // Check that supported offer and ask assets are equal to expected
        assert_eq!(supported_offer_assets, expected_offer_assets);
        assert_eq!(supported_ask_assets, expected_ask_assets);

        Ok(())
    }
}
