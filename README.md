### Prerequisites
This project requires an installation of:

- [x] The Rust toolchain (e.g. cargo).
- [x] [sns-cli](https://github.com/dfinity/ic)
- [x] [quill](https://github.com/dfinity/quill)
- [x] [didc.](https://github.com/dfinity/candid/tree/master/tools/didc)
- [x] Install the [IC SDK](../developer-docs/setup/install/index.mdx).


Begin by opening a terminal window.

 ### Step 1: Navigate into the folder containing the project's files and start a local instance of the Internet Computer with the command:

```
cd icp_sns
dfx start --background
```
 ### Step 2: Deploy icp_sns:

```
dfx deploy
```

 ### Step 3: Register new user by calling canister method with new user data:

```
dfx canister call dynamic_canisters_backend signup_new_user "(record { user = record { name = \"James Fury\"; age = 28:nat64; email = \"
dragon99steel@gmail.com\" };})"
```

You should see the output similar to following one:

```
(
  variant {
    Ok = record { canister_id = principal "ajuq4-ruaaa-aaaaa-qaaga-cai" }
  },
)
```

 ### Step 4: List registered user canisters and confirm that they are using dynamically created canisters:

```
dfx canister call dynamic_canisters_backend get_user_canisters '()'
```

You should see the output:

```
(
  vec {
    principal "ajuq4-ruaaa-aaaaa-qaaga-cai";
  },
)
```

 ### Step 5: Fetch user data from any user canister using `who_am_i` method:

```
dfx canister call dynamic_canisters_backend who_am_i "(principal \"ajuq4-ruaaa-aaaaa-qaaga-cai\")"
```

Output:

```
(
  variant {
    Ok = record {
      age = 28 : nat64;
      name = "James Fury";
      email = "dragon99steel@gmail.com";
    }
  },
)
```

 ### Step 6: Deploy testflight SNS and store the developer neuron ID:

```
sns deploy-testflight
```

After that, you will get your own developer neuron ID which is similar to following output. You will need it in the next step.

Output:

```
Developer neuron IDs:
594fd5d8dce3e793c3e421e1b87d55247627f8a63473047671f7f5ccc48eda63
```

 ### Step 7: Let's make a proposal to update existing user canister data. We can use `update_user_canister.sh` shell script to perform above operation:

```
./scripts/sns/proposals/update_user_canister.sh <Developer_Neuron_ID> <Username> <User_Canister_ID> <User_Age> <User_Name> <User_Email>
```

e.g.
```
./scripts/sns/proposals/update_user_canister.sh 5ee58180b48d54ca91f394a42e4c036d43a82e1095e4ff5275e0cb14c2140abc administrator bw4dl-smaaa-aaaaa-qaacq-cai 22 James dragon1227@outlook.com
```

### Resources
- [ic-cdk](https://docs.rs/ic-cdk/latest/ic_cdk/)
- [ic-cdk-macros](https://docs.rs/ic-cdk-macros)
- [JavaScript API reference](https://erxue-5aaaa-aaaab-qaagq-cai.ic0.app/)