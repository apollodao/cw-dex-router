#[cfg(test)]
pub mod initialize {
    use std::str::FromStr;

    use cosmwasm_std::{coin, Addr, Attribute, Coin, Decimal, Uint128};
    use cw_dex::osmosis::OsmosisPool;
    use osmosis_std::types::cosmos::base::v1beta1;
    use osmosis_std::types::osmosis::concentratedliquidity::v1beta1::{
        CreateConcentratedLiquidityPoolsProposal, Pool, PoolRecord, PoolsRequest,
    };
    use osmosis_std::types::osmosis::gamm::v1beta1::MsgJoinPool;
    use osmosis_std::types::osmosis::poolmanager::v1beta1::PoolType;
    use osmosis_std::types::osmosis::poolmanager::v1beta1::{
        MsgSwapExactAmountIn, SpotPriceRequest, SwapAmountInRoute,
    };
    use osmosis_std::types::osmosis::tokenfactory::v1beta1::QueryDenomsFromCreatorRequest;

    use osmosis_test_tube::osmosis_std::types::osmosis::gamm::poolmodels::balancer::v1beta1::MsgCreateBalancerPool;
    use osmosis_test_tube::osmosis_std::types::osmosis::gamm::v1beta1::PoolAsset;
    use osmosis_test_tube::Gamm;
    use osmosis_test_tube::{
        cosmrs::proto::traits::Message,
        osmosis_std::types::osmosis::concentratedliquidity::{
            poolmodel::concentrated::v1beta1::MsgCreateConcentratedPool, v1beta1::MsgCreatePosition,
        },
        Account, ConcentratedLiquidity, GovWithAppAccess, Module, OsmosisTestApp, PoolManager,
        SigningAccount, TokenFactory, Wasm,
    };

    use crate::msg::{ExecuteMsg, InstantiateMsg};
    use crate::operations::{
        SwapOperationBase, SwapOperationsListBase, SwapOperationsListUnchecked,
    };
    use crate::test_tube::helpers::{get_event_attributes_by_ty_and_key, sort_tokens};

    const ADMIN_BALANCE_AMOUNT: u128 = 340282366920938463463374607431768211455u128;
    const TOKENS_PROVIDED_AMOUNT: &str = "1000000000000";
    const DENOM_BASE: &str = "uatom";
    const DENOM_QUOTE: &str = "uosmo";

    pub struct PoolWithDenoms {
        pool: u64,
        denom0: String,
        denom1: String,
    }

    pub fn default_init() -> (OsmosisTestApp, Addr, Vec<PoolWithDenoms>, SigningAccount) {
        init_test_contract(
            "./test-tube-build/wasm32-unknown-unknown/release/cw_dex_router.wasm",
            &[
                Coin::new(ADMIN_BALANCE_AMOUNT, DENOM_BASE),
                Coin::new(ADMIN_BALANCE_AMOUNT, DENOM_QUOTE),
            ],
            vec![MsgCreateConcentratedPool {
                sender: "overwritten".to_string(),
                denom0: DENOM_BASE.to_string(),
                denom1: DENOM_QUOTE.to_string(),
                tick_spacing: 100,
                spread_factor: Decimal::from_str("0.01").unwrap().atomics().to_string(),
            }],
            vec![MsgCreateBalancerPool {
                sender: "overwritten".to_string(),
                pool_params: None,
                pool_assets: vec![
                    PoolAsset {
                        weight: "1".to_string(),
                        token: Some(
                            Coin {
                                denom: DENOM_BASE.to_string(),
                                amount: Uint128::from(1000000u128),
                            }
                            .into(),
                        ),
                    },
                    PoolAsset {
                        weight: "1".to_string(),
                        token: Some(
                            Coin {
                                denom: DENOM_QUOTE.to_string(),
                                amount: Uint128::from(1000000u128),
                            }
                            .into(),
                        ),
                    },
                ],
                future_pool_governor: "overwritten".to_string(),
            }],
        )
    }

    pub fn init_test_contract(
        filename: &str,
        admin_balance: &[Coin],
        cl_pools: Vec<MsgCreateConcentratedPool>,
        gamm_pools: Vec<MsgCreateBalancerPool>,
    ) -> (OsmosisTestApp, Addr, Vec<PoolWithDenoms>, SigningAccount) {
        // Create new osmosis appchain instance
        let app = OsmosisTestApp::new();
        let pm = PoolManager::new(&app);
        let cl = ConcentratedLiquidity::new(&app);
        let wasm = Wasm::new(&app);

        // Create new account with initial funds
        let admin = app.init_account(admin_balance).unwrap();

        // Load compiled wasm bytecode
        let wasm_byte_code = std::fs::read(filename).unwrap();
        let code_id = wasm
            .store_code(&wasm_byte_code, None, &admin)
            .unwrap()
            .data
            .code_id;

        let mut pools: Vec<PoolWithDenoms> = vec![];

        let gov = GovWithAppAccess::new(&app);
        for pool in cl_pools {
            // Setup a dummy CL pool to work with
            gov.propose_and_execute(
                CreateConcentratedLiquidityPoolsProposal::TYPE_URL.to_string(),
                CreateConcentratedLiquidityPoolsProposal {
                    title: "CL Pool".to_string(),
                    description: "So that we can trade it".to_string(),
                    pool_records: vec![PoolRecord {
                        denom0: pool.denom0,
                        denom1: pool.denom1,
                        tick_spacing: pool.tick_spacing,
                        spread_factor: pool.spread_factor,
                    }],
                },
                admin.address(),
                &admin,
            )
            .unwrap();

            // Get just created pool information by querying all the pools, and taking the first one
            let pools_response = cl.query_pools(&PoolsRequest { pagination: None }).unwrap();
            let pool: Pool = Pool::decode(pools_response.pools[0].value.as_slice()).unwrap();

            let tokens_provided = vec![
                v1beta1::Coin {
                    denom: DENOM_BASE.to_string(),
                    amount: TOKENS_PROVIDED_AMOUNT.to_string(),
                },
                v1beta1::Coin {
                    denom: DENOM_QUOTE.to_string(),
                    amount: TOKENS_PROVIDED_AMOUNT.to_string(),
                },
            ];
            // Create a first position in the pool with the admin user
            cl.create_position(
                MsgCreatePosition {
                    pool_id: pool.id,
                    sender: admin.address(),
                    lower_tick: -5000000, // 0.5 spot price
                    upper_tick: 500000,   // 1.5 spot price
                    tokens_provided: tokens_provided.clone(),
                    token_min_amount0: "1".to_string(),
                    token_min_amount1: "1".to_string(),
                },
                &admin,
            )
            .unwrap();

            // Get and assert spot price is 1.0
            let spot_price = pm
                .query_spot_price(&SpotPriceRequest {
                    base_asset_denom: tokens_provided[0].denom.to_string(),
                    quote_asset_denom: tokens_provided[1].denom.to_string(),
                    pool_id: pool.id,
                })
                .unwrap();
            assert_eq!(spot_price.spot_price, "1.000000000000000000");

            pools.push(PoolWithDenoms {
                pool: pool.id,
                denom0: pool.token0,
                denom1: pool.token1,
            });
        }

        for pool in gamm_pools {
            // Create a new pool
            let gamm = Gamm::new(&app);
            let response = gamm
                .create_basic_pool(
                    &[
                        Coin {
                            denom: pool.pool_assets[0]
                                .token
                                .as_ref()
                                .unwrap()
                                .denom
                                .to_string(),
                            amount: Uint128::from_str(
                                &pool.pool_assets[0].token.as_ref().unwrap().amount,
                            )
                            .unwrap(),
                        },
                        Coin {
                            denom: pool.pool_assets[1]
                                .token
                                .as_ref()
                                .unwrap()
                                .denom
                                .to_string(),
                            amount: Uint128::from_str(
                                &pool.pool_assets[1].token.as_ref().unwrap().amount,
                            )
                            .unwrap(),
                        },
                    ],
                    &admin,
                )
                .unwrap();

            let ty = "pool_created";
            let keys = vec!["pool_id"];
            let pool_id: u64 = response
                .events
                .iter()
                .filter(|event| event.ty == ty)
                .flat_map(|event| event.attributes.clone())
                .filter(|attribute| keys.contains(&attribute.key.as_str()))
                .collect::<Vec<Attribute>>()
                .first()
                .unwrap()
                .value
                .parse()
                .unwrap();
            // println!("Gamm pool Result: {:?} {:?}", response, pool_id);

            let add_liq = MsgJoinPool {
                sender: admin.address().to_string(),
                pool_id: pool_id.clone(),
                share_out_amount: "1".to_string(),
                token_in_maxs: vec![
                    Coin {
                        denom: pool.pool_assets[0]
                            .token
                            .as_ref()
                            .unwrap()
                            .denom
                            .to_string(),
                        amount: Uint128::from_str(
                            &pool.pool_assets[0].token.as_ref().unwrap().amount,
                        )
                        .unwrap(),
                    }
                    .into(),
                    Coin {
                        denom: pool.pool_assets[1]
                            .token
                            .as_ref()
                            .unwrap()
                            .denom
                            .to_string(),
                        amount: Uint128::from_str(
                            &pool.pool_assets[1].token.as_ref().unwrap().amount,
                        )
                        .unwrap(),
                    }
                    .into(),
                ],
            };

            pools.push(PoolWithDenoms {
                pool: pool_id,
                denom0: DENOM_BASE.to_string(),
                denom1: DENOM_QUOTE.to_string(),
            })
        }

        // Instantiate vault
        let contract = wasm
            .instantiate(
                code_id,
                &InstantiateMsg {},
                Some(admin.address().as_str()),
                Some("cw-dex-router"),
                &[],
                &admin,
            )
            .unwrap();

        // // Sort tokens alphabetically by denom name or Osmosis will return an error
        // tokens_provided.sort_by(|a, b| a.denom.cmp(&b.denom)); // can't use helpers.rs::sort_tokens() due to different Coin type

        // // Increment the app time for twaps to function, this is needed to do not fail on querying a twap for a timeframe higher than the chain existence
        // app.increase_time(1000000);

        (app, Addr::unchecked(contract.data.address), pools, admin)
    }

    #[test]
    fn default_init_works() {
        let (app, contract_address, pools, admin) = default_init();
        let wasm = Wasm::new(&app);
        let cl = ConcentratedLiquidity::new(&app);
        let tf = TokenFactory::new(&app);
        let pm = PoolManager::new(&app);

        for pool in pools {
            let response = wasm
                .execute(
                    &contract_address.to_string(),
                    &ExecuteMsg::SetPath {
                        offer_asset: apollo_cw_asset::AssetInfoBase::Native(pool.denom0.clone()),
                        ask_asset: apollo_cw_asset::AssetInfoBase::Native(pool.denom1.clone()),
                        path: SwapOperationsListUnchecked::new(vec![SwapOperationBase {
                            pool: cw_dex::Pool::Osmosis(OsmosisPool::unchecked(pool.pool.clone())),
                            offer_asset_info: apollo_cw_asset::AssetInfoBase::Native(
                                pool.denom0.clone(),
                            ),
                            ask_asset_info: apollo_cw_asset::AssetInfoBase::Native(pool.denom1)
                                .clone(),
                        }])
                        .into(),
                        bidirectional: true,
                    },
                    &[],
                    &admin,
                )
                .unwrap();

            println!("SetPath Result: {:?}", response);
        }
    }

    // #[test]
    // #[ignore]
    // fn default_init_works() {
    //     let (app, contract_address, cl_pool_id, admin) = default_init();
    //     let wasm = Wasm::new(&app);
    //     let cl = ConcentratedLiquidity::new(&app);
    //     let tf = TokenFactory::new(&app);
    //     let pm = PoolManager::new(&app);

    //     let pools = cl.query_pools(&PoolsRequest { pagination: None }).unwrap();
    //     let pool = Pool::decode(pools.pools[0].value.as_slice()).unwrap();

    //     let resp = wasm
    //         .query::<QueryMsg, PoolResponse>(
    //             contract_address.as_str(),
    //             &QueryMsg::VaultExtension(ExtensionQueryMsg::ConcentratedLiquidity(
    //                 ClQueryMsg::Pool {},
    //             )),
    //         )
    //         .unwrap();

    //     assert_eq!(resp.pool_config.pool_id, pool.id);
    //     assert_eq!(resp.pool_config.token0, pool.token0);
    //     assert_eq!(resp.pool_config.token1, pool.token1);

    //     let resp = wasm
    //         .query::<QueryMsg, VaultInfoResponse>(contract_address.as_str(), &QueryMsg::Info {})
    //         .unwrap();

    //     assert_eq!(resp.tokens, vec![pool.token0, pool.token1]);
    //     assert_eq!(
    //         resp.vault_token,
    //         tf.query_denoms_from_creator(&QueryDenomsFromCreatorRequest {
    //             creator: contract_address.to_string()
    //         })
    //         .unwrap()
    //         .denoms[0]
    //     );

    //     // Create Alice account
    //     let alice = app
    //         .init_account(&[
    //             Coin::new(1_000_000_000_000, DENOM_BASE),
    //             Coin::new(1_000_000_000_000, DENOM_QUOTE),
    //         ])
    //         .unwrap();

    //     // Swap some funds as Alice to move the pool's curent tick
    //     pm.swap_exact_amount_in(
    //         MsgSwapExactAmountIn {
    //             sender: alice.address(),
    //             routes: vec![SwapAmountInRoute {
    //                 pool_id: cl_pool_id,
    //                 token_out_denom: DENOM_BASE.to_string(),
    //             }],
    //             token_in: Some(v1beta1::Coin {
    //                 denom: DENOM_QUOTE.to_string(),
    //                 amount: "1000".to_string(),
    //             }),
    //             token_out_min_amount: "1".to_string(),
    //         },
    //         &alice,
    //     )
    //     .unwrap();

    //     // Increment the app time for twaps to function
    //     app.increase_time(1000000);

    //     // Update range of vault as Admin
    //     wasm.execute(
    //         contract_address.as_str(),
    //         &ExecuteMsg::VaultExtension(crate::msg::ExtensionExecuteMsg::ModifyRange(
    //             ModifyRangeMsg {
    //                 lower_price: Decimal::from_str("0.993").unwrap(),
    //                 upper_price: Decimal::from_str("1.002").unwrap(),
    //                 max_slippage: Decimal::bps(9500),
    //                 ratio_of_swappable_funds_to_use: Decimal::one(),
    //                 twap_window_seconds: 45,
    //             },
    //         )),
    //         &[],
    //         &admin,
    //     )
    //     .unwrap();
    // }
}
