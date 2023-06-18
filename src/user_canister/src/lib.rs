use ic_cdk::*;
use ic_cdk::export::candid::{CandidType};
use ic_cdk::export::Principal;
use serde::Deserialize;
use std::cell::RefCell;

#[derive(Default, CandidType, Deserialize, Clone, Debug)]
struct User {
    name: String,
    age: u64,
    email: String
}

thread_local! {
    static USER_STORE: RefCell<User> = RefCell::new(User::default());
}

#[derive(CandidType, Deserialize)]
struct CreateUserArgs {
    user: User
}

#[derive(CandidType, Deserialize)]
struct CreateUserResult {
    user_id: Principal
}

#[ic_cdk::update]
async fn create_user(args: CreateUserArgs) -> Result<CreateUserResult, String> {
    let user_id = ic_cdk::id();
    let user = args.user;
    USER_STORE.with(|store| {
        store.replace(user);
    });
    Ok(CreateUserResult { user_id })
}

#[ic_cdk::query]
async fn get_user() -> Result<User, String> {
    Ok(USER_STORE.with(|store| store.borrow().clone()))
}

#[ic_cdk::query]
async fn get_user_name() -> Result<String, String> {
    Ok(USER_STORE.with(|store| store.borrow().name.clone()))
}