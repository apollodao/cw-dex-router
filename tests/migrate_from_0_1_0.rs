#[cfg(feature = "osmosis")]
mod tests {
    use apollo_cw_asset::{AssetInfo, AssetInfoUnchecked, AssetUnchecked};
    use cosmwasm_std::{coin, Coin, Empty, Uint128};
    use cw_dex_router::msg::{ExecuteMsg, MigrateMsg};
    use cw_it::osmosis_std::types::cosmwasm::wasm::v1::{
        MsgMigrateContract, MsgMigrateContractResponse,
    };
    use cw_it::osmosis_test_tube::{Gamm, OsmosisTestApp};
    use cw_it::test_tube::{Account, Module, Runner, SigningAccount, Wasm};
    use cw_it::traits::CwItRunner;
    use cw_it::{Artifact, ContractType, OwnedTestRunner};

    const TEST_ARTIFACTS_DIR: &str = "tests/test_artifacts";

    const UOSMO: &str = "uosmo";
    const UATOM: &str = "uatom";
    const UION: &str = "uion";

    const UOSMO_UATOM_PATH: &[(u64, &str, &str); 1] = &[(1, UOSMO, UATOM)];
    const UION_UATOM_PATH: &[(u64, &str, &str); 2] = &[(2, UION, UOSMO), (1, UOSMO, UATOM)];

    #[allow(deprecated)]
    fn osmosis_swap_operations_list_from_vec(
        vec: &[(u64, &str, &str)],
    ) -> cw_dex_router_0_3::operations::SwapOperationsList {
        cw_dex_router_0_3::operations::SwapOperationsList::new(
            vec.iter()
                .map(
                    |(pool_id, from, to)| cw_dex_router_0_3::operations::SwapOperation {
                        pool: cw_dex::Pool::Osmosis(
                            cw_dex::implementations::osmosis::OsmosisPool::unchecked(
                                pool_id.to_owned(),
                            ),
                        ),
                        offer_asset_info: AssetInfo::Native(from.to_string()),
                        ask_asset_info: AssetInfo::Native(to.to_string()),
                    },
                )
                .collect(),
        )
    }

    #[allow(deprecated)]
    fn create_basic_pool<'a>(
        runner: &'a impl Runner<'a>,
        pool_liquidity: Vec<Coin>,
        signer: &SigningAccount,
    ) -> cw_dex::implementations::osmosis::OsmosisPool {
        let gamm = Gamm::new(runner);

        // Create 1:1 pool
        let pool_id = gamm
            .create_basic_pool(&pool_liquidity, signer)
            .unwrap()
            .data
            .pool_id;

        cw_dex::implementations::osmosis::OsmosisPool::unchecked(pool_id)
    }

    #[test]
    fn migrate_from_0_1_0() {
        let test_app = OsmosisTestApp::new();
        let runner = OwnedTestRunner::OsmosisTestApp(test_app);
        let wasm = Wasm::new(&runner);
        let admin = runner.init_default_account().unwrap();

        // Upload old wasm file
        let old_wasm = ContractType::Artifact(Artifact::Local(format!(
            "{}/{}.wasm",
            TEST_ARTIFACTS_DIR, "cw_dex_router_osmosis_0_1_0"
        )));
        let old_code_id = runner.store_code(old_wasm, &admin).unwrap();

        // Instantiate old contract
        let res = wasm
            .instantiate(
                old_code_id,
                &Empty {},
                Some(admin.address().as_str()),
                Some("Cw Dex Router"),
                &[],
                &admin,
            )
            .unwrap();
        let contract_addr = res.data.address;

        // Create pools
        let pools = vec![(UOSMO, UATOM), (UION, UOSMO)];
        for pool in pools {
            let pool_liquidity = vec![
                Coin {
                    denom: pool.0.to_string(),
                    amount: Uint128::from(1000000u128),
                },
                Coin {
                    denom: pool.1.to_string(),
                    amount: Uint128::from(1000000u128),
                },
            ];
            let osmo_pool = create_basic_pool(&runner, pool_liquidity, &admin);
            println!("osmo pool: {:?}", osmo_pool);
            println!("pool: {:?}", pool);
        }

        // Store two routes
        // OSMO -> ATOM
        let execute_msg = cw_dex_router_0_3::msg::ExecuteMsg::SetPath {
            offer_asset: AssetInfoUnchecked::Native(UOSMO.to_string()),
            ask_asset: AssetInfoUnchecked::Native(UATOM.to_string()),
            path: osmosis_swap_operations_list_from_vec(UOSMO_UATOM_PATH).into(),
            bidirectional: true,
        };
        wasm.execute(&contract_addr, &execute_msg, &[], &admin)
            .unwrap();
        // ION -> OSMO -> ATOM
        let execute_msg = cw_dex_router_0_3::msg::ExecuteMsg::SetPath {
            offer_asset: AssetInfoUnchecked::Native(UION.to_string()),
            ask_asset: AssetInfoUnchecked::Native(UATOM.to_string()),
            path: osmosis_swap_operations_list_from_vec(UION_UATOM_PATH).into(),
            bidirectional: true,
        };
        wasm.execute(&contract_addr, &execute_msg, &[], &admin)
            .unwrap();

        // Try basket liquidate swapping ION and OSMO to ATOM, should fail due to
        // overlapping paths bug
        let basket_liq_msg = ExecuteMsg::BasketLiquidate {
            offer_assets: vec![
                AssetUnchecked::new(
                    AssetInfoUnchecked::Native(UION.to_string()),
                    Uint128::new(1000000),
                ),
                AssetUnchecked::new(
                    AssetInfoUnchecked::Native(UOSMO.to_string()),
                    Uint128::new(1000000),
                ),
            ]
            .into(),
            receive_asset: AssetInfoUnchecked::Native(UATOM.to_string()),
            minimum_receive: None,
            to: None,
        };
        let res = wasm
            .execute(
                &contract_addr,
                &basket_liq_msg,
                &[coin(1000000, UION), coin(1000000, UOSMO)],
                &admin,
            )
            .unwrap_err();
        println!("res: {:?}", res);

        // Upload new wasm file
        let new_wasm = ContractType::Artifact(Artifact::Local(format!(
            "{}/{}.wasm",
            TEST_ARTIFACTS_DIR, "cw_dex_router_osmosis_0_3_0"
        )));
        let new_code_id = runner.store_code(new_wasm, &admin).unwrap();

        // Migrate contract
        let msg = MigrateMsg {};
        runner
            .execute::<_, MsgMigrateContractResponse>(
                MsgMigrateContract {
                    sender: admin.address(),
                    code_id: new_code_id,
                    msg: serde_json::to_vec(&msg).unwrap(),
                    contract: contract_addr.clone(),
                },
                "/cosmwasm.wasm.v1.MsgMigrateContract",
                &admin,
            )
            .unwrap();

        // Try basket liquidate swapping ION and OSMO to ATOM, should succeed
        let res = wasm
            .execute(
                &contract_addr,
                &basket_liq_msg,
                &[coin(1000000, UION), coin(1000000, UOSMO)],
                &admin,
            )
            .unwrap();
        res.events.iter().for_each(|event| {
            println!("event: {:?}", event);
        });
    }
}
