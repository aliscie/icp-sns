use ic_cdk::api;
use ic_cdk::export::candid::{
    candid_method, CandidType, check_prog, Deserialize, export_service, IDLProg, TypeEnv,
};
use std::cell::RefCell;


#[ic_cdk::query]
fn greet(name: String) -> String {
    format!("Hello, {}!", name)
}

#[derive(Clone, CandidType, Deserialize)]
pub struct ChartTick {
    timestamp: u64,
    cycles: u64,
}

thread_local! {
    static CHART_TICKS: RefCell<Vec<ChartTick>> = Default::default();
}

fn update_chart() {
    let timestamp = api::time();
    let cycles = api::canister_balance();
    CHART_TICKS.with(|mut chart| chart.borrow_mut().push(ChartTick { timestamp, cycles }));
}

mod wallet {
    use ic_cdk::export::candid::{
        candid_method, CandidType, check_prog, Deserialize, export_service, IDLProg, TypeEnv,
    };
    // use ic_cdk::query;
    use std::convert::TryInto;

    use ic_cdk::*;
    use ic_cdk::export::candid::{Nat};
    use ic_cdk::export::Principal;
    use super::*;

    /***************************************************************************************************
             * Cycle Management
             **************************************************************************************************/
    #[derive(CandidType)]
    struct BalanceResult<TCycles> {
        amount: TCycles,
    }

    #[derive(CandidType, Deserialize)]
    struct SendCyclesArgs<TCycles> {
        canister: Principal,
        amount: TCycles,
    }

    /// Return the cycle balance of this canister.
    // #[query(guard = "is_custodian_or_controller", name = "wallet_balance")]
    #[candid_method(query)]
    #[ic_cdk::query]
    fn balance() -> BalanceResult<u64> {
        BalanceResult {
            amount: api::canister_balance128()
                .try_into()
                .expect("Balance exceeded a 64-bit value; call `wallet_balance128`"),
        }
    }

    // #[query(guard = "is_custodian_or_controller", name = "wallet_balance128")]
    #[candid_method(query)]
    #[ic_cdk::query]
    fn balance128() -> BalanceResult<u128> {
        BalanceResult {
            amount: api::canister_balance128(),
        }
    }


    /***************************************************************************************************
     * Managing Canister
     **************************************************************************************************/
    #[derive(CandidType, Clone, Deserialize)]
    struct CanisterSettings {
        // dfx versions <= 0.8.1 (or other wallet callers expecting version 0.1.0 of the wallet)
        // will set a controller (or not) in the the `controller` field:
        controller: Option<Principal>,

        // dfx versions >= 0.8.2 will set 0 or more controllers here:
        controllers: Option<Vec<Principal>>,

        compute_allocation: Option<Nat>,
        memory_allocation: Option<Nat>,
        freezing_threshold: Option<Nat>,
    }

    #[derive(CandidType, Clone, Deserialize)]
    struct CreateCanisterArgs<TCycles> {
        cycles: TCycles,
        settings: CanisterSettings,
    }

    #[derive(CandidType, Deserialize)]
    struct UpdateSettingsArgs {
        canister_id: Principal,
        settings: CanisterSettings,
    }

    #[derive(CandidType, Deserialize)]
    struct CreateResult {
        canister_id: Principal,
    }

    // #[update(guard = "is_custodian_or_controller", name = "wallet_create_canister")]
    #[candid_method(update)]
    #[ic_cdk::update]
    async fn create_canister(
        CreateCanisterArgs { cycles, settings }: CreateCanisterArgs<u64>,
    ) -> Result<CreateResult, String> {
        create_canister128(CreateCanisterArgs {
            cycles: cycles as u128,
            settings,
        })
            .await
    }

    async fn create_canister_call(args: CreateCanisterArgs<u128>) -> Result<CreateResult, String> {
        #[derive(CandidType)]
        struct In {
            settings: Option<CanisterSettings>,
        }
        let in_arg = In {
            settings: Some(normalize_canister_settings(args.settings)?),
        };

        let (create_result, ): (CreateResult, ) = match api::call::call_with_payment128(
            Principal::management_canister(),
            "create_canister",
            (in_arg, ),
            args.cycles,
        )
            .await
        {
            Ok(x) => x,
            Err((code, msg)) => {
                return Err(format!(
                    "An error happened during the call: {}: {}",
                    code as u8, msg
                ));
            }
        };

        // events::record(events::EventKind::CanisterCreated {
        //     canister: create_result.canister_id,
        //     cycles: args.cycles,
        // });
        Ok(create_result)
    }

    // #[update(
    // guard = "is_custodian_or_controller",
    // name = "wallet_create_canister128"
    // )]
    #[candid_method(update)]
    #[ic_cdk::update]
    async fn create_canister128(
        mut args: CreateCanisterArgs<u128>,
    ) -> Result<CreateResult, String> {
        let mut settings = normalize_canister_settings(args.settings)?;
        let controllers = settings
            .controllers
            .get_or_insert_with(|| Vec::with_capacity(2));
        if controllers.is_empty() {
            controllers.push(ic_cdk::api::caller());
            controllers.push(ic_cdk::api::id());
        }
        args.settings = settings;
        let create_result = create_canister_call(args).await?;
        super::update_chart();
        Ok(create_result)
    }

    // Make it so the controller or controllers are stored only in the controllers field.
    fn normalize_canister_settings(settings: CanisterSettings) -> Result<CanisterSettings, String> {
        // Agent <= 0.8.0, dfx <= 0.8.1 will send controller
        // Agents >= 0.9.0, dfx >= 0.8.2 will send controllers
        // The management canister will accept either controller or controllers, but not both.
        match (&settings.controller, &settings.controllers) {
            (Some(_), Some(_)) => {
                Err("CanisterSettings cannot have both controller and controllers set.".to_string())
            }
            (Some(controller), None) => Ok(CanisterSettings {
                controller: None,
                controllers: Some(vec![*controller]),
                ..settings
            }),
            _ => Ok(settings),
        }
    }


    async fn install_wallet(canister_id: &Principal, wasm_module: Vec<u8>) -> Result<(), String> {
        // Install Wasm
        #[derive(CandidType, Deserialize)]
        enum InstallMode {
            #[serde(rename = "install")]
            Install,
            #[serde(rename = "reinstall")]
            Reinstall,
            #[serde(rename = "upgrade")]
            Upgrade,
        }

        #[derive(CandidType, Deserialize)]
        struct CanisterInstall {
            mode: InstallMode,
            canister_id: Principal,
            #[serde(with = "serde_bytes")]
            wasm_module: Vec<u8>,
            arg: Vec<u8>,
        }

        let install_config = CanisterInstall {
            mode: InstallMode::Install,
            canister_id: *canister_id,
            wasm_module: wasm_module.clone(),
            arg: b" ".to_vec(),
        };

        match api::call::call(
            Principal::management_canister(),
            "install_code",
            (install_config, ),
        )
            .await
        {
            Ok(x) => x,
            Err((code, msg)) => {
                return Err(format!(
                    "An error happened during the call: {}: {}",
                    code as u8, msg
                ));
            }
        };

        // events::record(events::EventKind::WalletDeployed {
        //     canister: *canister_id,
        // });
        #[derive(CandidType, Deserialize)]
        struct WalletStoreWASMArgs {
            #[serde(with = "serde_bytes")]
            wasm_module: Vec<u8>,
        }

        // Store wallet wasm
        let store_args = WalletStoreWASMArgs { wasm_module };
        match api::call::call(*canister_id, "wallet_store_wallet_wasm", (store_args, )).await {
            Ok(x) => x,
            Err((code, msg)) => {
                return Err(format!(
                    "An error happened during the call: {}: {}",
                    code as u8, msg
                ));
            }
        };
        Ok(())
    }


    #[cfg(test)]
    mod tests {
        use std::borrow::Cow;
        use std::env;
        use std::fs::{create_dir_all, write};
        use std::path::Path;
        use std::path::PathBuf;
        use candid::export_service;

        use ic_cdk::{api, update};
        use ic_cdk::api::management_canister::main::CanisterSettings;
        // use ic_cdk::export::candid::{
        //     candid_method, CandidType, check_prog, Deserialize, export_service, IDLProg, TypeEnv,
        // };
        use ic_cdk::export::candid::Principal;
        use crate::wallet::BalanceResult;
        use crate::wallet::CreateCanisterArgs;
        use crate::wallet::CreateResult;
        // use super::*;

        #[test]
        fn save_candid_2() {
            #[ic_cdk_macros::query(name = "__get_candid_interface_tmp_hack")]
            fn export_candid() -> String {
                ic_cdk::export::candid::export_service!();
                __export_service()
            }

            let dir: PathBuf = env::current_dir().unwrap();
            let canister_name: Cow<str> = dir.file_name().unwrap().to_string_lossy();

            match create_dir_all(&dir) {
                Ok(_) => println!("Successfully created directory"),
                Err(e) => println!("Failed to create directory: {}", e),
            }

            let res = write(dir.join(format!("{:?}.did", canister_name).replace("\"", "")), export_candid());
            println!("-------- Wrote to {:?}", dir);
            println!("-------- res {:?}", canister_name);
        }
    }
}