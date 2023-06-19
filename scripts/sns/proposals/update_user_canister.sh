# eg './update_user_canister.sh 5ee58180b48d54ca91f394a42e4c036d43a82e1095e4ff5275e0cb14c2140abc administrator bw4dl-smaaa-aaaaa-qaacq-cai 22 James dragon1227@outlook.com'

# Set current directory to the directory this script is in
SCRIPT=$(readlink -f "$0")
SCRIPT_DIR=$(dirname "$SCRIPT")
cd $SCRIPT_DIR

printf 'SCRIPT_DIR: %s\n' "$SCRIPT_DIR"

# Update the settings for the dynamic_canisters_backend
dfx canister update-settings --add-controller $(dfx canister id sns_root) dynamic_canisters_backend

# Extract the information
DEVELOPER_NEURON_ID=$1
PEM_FILE="/home/$2/.config/dfx/identity/$(dfx identity whoami)/identity.pem"
CID=$(dfx canister id dynamic_canisters_backend)
USER_CANISTER_ID=$3
USER_AGE=$4
USER_NAME=$5
USER_EMAIL=$6
IC_URL="https://192.168.146.128:3000"
PAYLOAD=$(didc encode '("'$USER_CANISTER_ID'", record { user=record {age='$USER_AGE':nat64; name="'$USER_NAME'":text; email="'$USER_EMAIL'":text;};})' --format blob)

printf 'DEVELOPER_NEURON_ID: %s\n' "$DEVELOPER_NEURON_ID"
printf 'PEM_FILE: %s\n' "$PEM_FILE"
printf 'CID: %s\n' "$CID"
printf 'USER_CANISTER_ID: %s\n' "$USER_CANISTER_ID"
printf 'USER_AGE: %s\n' "$USER_AGE"
printf 'USER_NAME: %s\n' "$USER_NAME"
printf 'USER_EMAIL: %s\n' "$USER_EMAIL"

# Make the proposal using quill
quill sns --canister-ids-file ../../../sns_canister_ids.json --pem-file $PEM_FILE make-proposal --proposal "(record { title=\"Register SNS user update generic function.\"; url=\"https://example.com/\"; summary=\"This proposal registers SNS user update generic functions.\"; action=opt variant {AddGenericNervousSystemFunction = record {id=1000:nat64; name=\"MyGenericFunctions\"; description=null; function_type=opt variant {GenericNervousSystemFunction=record{validator_canister_id=opt principal\"$CID\"; target_canister_id=opt principal\"$CID\"; validator_method_name=opt\"sns_update_user_canister_validate\"; target_method_name=opt\"sns_update_user_canister\"}}}}})" $DEVELOPER_NEURON_ID > register-generic-functions.json
quill send register-generic-functions.json

quill sns --canister-ids-file ../../../sns_canister_ids.json --pem-file $PEM_FILE make-proposal --proposal "(record { title=\"Execute SNS update user function.\"; summary=\"This proposal executes SNS update user function.\"; action=opt variant {ExecuteGenericNervousSystemFunction = record {function_id=1000:nat64; payload=$PAYLOAD}}})" $DEVELOPER_NEURON_ID > execute-update-user-function.json
quill send execute-update-user-function.json