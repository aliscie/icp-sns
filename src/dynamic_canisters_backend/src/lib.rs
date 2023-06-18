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

mod user {
    use ic_cdk::api::management_canister::http_request::{http_request, CanisterHttpRequestArgument, HttpMethod};
    use ic_cdk::export::candid::{CandidType, Principal, Nat};
    use ic_cdk::{api, query, update};
    use std::cell::RefCell;
    use serde::{Serialize, Deserialize};
    
    thread_local! {
        static USER_CANISTERS: RefCell<Vec<Principal>> = Default::default();
    }
    
    static USER_CANISTER_WASM_MODULE_URL: &str = "https://localhost:3000/user_canister.wasm";

    #[derive(Default, PartialEq, Eq, Serialize, CandidType, Deserialize, Clone, Debug)]
    struct User {
        name: String,
        age: u64,
        email: String
    }
    #[derive(CandidType, Serialize, Deserialize)]
    struct CreateUserArgs {
        user: User
    }

    #[derive(CandidType, Clone, Deserialize)]
    struct UserCanisterSettings {
        controllers: Option<Vec<Principal>>,
        compute_allocation: Option<Nat>,
        memory_allocation: Option<Nat>,
        freezing_threshold: Option<Nat>,
    }

    #[derive(CandidType, Clone, Deserialize)]
    struct UserCreateCanisterArgs<T> {
        cycles: T,
        settings: UserCanisterSettings,
    }

    #[derive(CandidType, Deserialize)]
    struct UserCreateCanisterResult {
        canister_id: Principal,
    }

    #[derive(Debug, CandidType, Deserialize)]
    struct QueryError {
        message: String,
    }

    #[update(name = "user_create_canister")]
    async fn create_canister(
        UserCreateCanisterArgs { cycles, settings}: UserCreateCanisterArgs<u64>
    ) -> Result<UserCreateCanisterResult, String> {
        create_canister128(UserCreateCanisterArgs {
            cycles: cycles as u128,
            settings,
        }).await
    }

    #[update(name = "user_create_canister128")]
    async fn create_canister128(
        mut args: UserCreateCanisterArgs<u128>,
    ) -> Result<UserCreateCanisterResult, String> {
        let mut settings = args.settings;
        let mut controllers = settings.controllers.unwrap_or(vec![]);
        if controllers.is_empty() {
            controllers.push(ic_cdk::api::caller());
            controllers.push(ic_cdk::api::id());
        }
        settings.controllers = Some(controllers.clone());
        args.settings = settings;
        let create_canister_result = create_canister_call(args).await?;

        Ok(create_canister_result)
    }

    #[update(name = "signup_new_user")]
    async fn signup_new_user(user_args: CreateUserArgs) -> Result<UserCreateCanisterResult, String> {
        let mut settings = UserCanisterSettings {
            controllers: Some(vec![ic_cdk::api::caller(), ic_cdk::api::id()]),
            compute_allocation: None,
            memory_allocation: None,
            freezing_threshold: None,
        };
        let mut controllers = settings.controllers.unwrap_or(vec![]);
        if controllers.is_empty() {
            controllers.push(ic_cdk::api::caller());
            controllers.push(ic_cdk::api::id());
        }
        settings.controllers = Some(controllers.clone());
        let args = UserCreateCanisterArgs {
            cycles: 100_000_000_000,
            settings,
        };
        let create_canister_result = create_canister_call(args).await?;

        install_user(&create_canister_result.canister_id, get_wasm_content(USER_CANISTER_WASM_MODULE_URL.to_string()).await?).await?;
        match api::call::call(create_canister_result.canister_id, "create_user", (user_args,)).await {
            Ok(x) => x,
            Err((code, msg)) => {
                return Err(format!(
                    "An error happened during the call: {}: {}",
                    code as u8, msg
                ))
            }
        };

        USER_CANISTERS.with(|canisters| canisters.borrow_mut().push(create_canister_result.canister_id.clone()));

        Ok(create_canister_result)
    }

    #[ic_cdk::update]
    async fn get_wasm_content(url: String) -> Result<Vec<u8>, String> {
        let request_headers = vec![];
        
        let request = CanisterHttpRequestArgument {
            url: url.to_string(),
            method: HttpMethod::GET,
            body: None,               //optional for request
            max_response_bytes: None, //optional for request
            transform: None,          //optional for request
            headers: request_headers,
        };

        match http_request(request).await {
            Ok((response,)) => {
                // Return the content of the response body as Vec<u8>
                Ok(response.body)
            }
            Err((r, m)) => {
                let message =
                    format!("The http_request resulted into error. RejectionCode: {r:?}, Error: {m}");
    
                //Return the error as a string and end the method
                Err(message)
            }
        }
    }

    async fn create_canister_call(args: UserCreateCanisterArgs<u128>) -> Result<UserCreateCanisterResult, String> {
        #[derive(CandidType)]
        struct CreateCanisterArgument {
            settings: Option<UserCanisterSettings>,
        }

        let create_canister_arg = CreateCanisterArgument {
            settings: Some(args.settings),
        };

        let (create_result,): (UserCreateCanisterResult,) = match api::call::call_with_payment128(
            Principal::management_canister(),
            "create_canister",
            (create_canister_arg,),
            args.cycles,
        )
        .await {
            Ok(r) => r,
            Err((code, msg)) => return Err(format!("Error while creating a canister: {}: {}", code as u8, msg)),
        };

        Ok(create_result)
    }

    async fn install_user(canister_id: &Principal, wasm_module: Vec<u8>) -> Result<(), String> {
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
            (install_config,),
        )
        .await
        {
            Ok(x) => x,
            Err((code, msg)) => {
                return Err(format!(
                    "An error happened during the call: {}: {}",
                    code as u8, msg
                ))
            }
        };

        #[derive(Default, CandidType, Deserialize, Clone, Debug)]
        struct User {
            name: String,
            age: u64,
            email: String
        }

        #[derive(CandidType, Deserialize)]
        struct CreateUserArgs {
            user: User
        }

        let create_user_arg = CreateUserArgs {
            user: User {
                name: "John".to_string(),
                age: 30,
                email: "dragon99steel@gmail.com".to_string()
            }
        };

        match api::call::call(*canister_id, "create_user", (create_user_arg,)).await {
            Ok(x) => x,
            Err((code, msg)) => {
                return Err(format!(
                    "An error happened during the call: {}: {}",
                    code as u8, msg
                ))
            }
        };

        Ok(())
    }

    #[query(name = "get_user_canisters")]
    fn get_user_canisters() -> Vec<Principal> {
        USER_CANISTERS.with(|canisters| canisters.borrow().clone())
    }

    #[update(name = "who_am_i")]
    async fn get_user_canister_by_id(user_canister_id: Principal) -> Result<User, String> {
        let call_result = api::call::call::<_, (Result<User, String>, )>(user_canister_id, "get_user", (),)
                        .await
                        .map_err(|e| format!("Error calling get_period_range_realized_volatility: {:?}", e))?;
        call_result.0
                .map_err(|e| format!("Error calling get_period_range_realized_volatility: {:?}", e))        
    }
}