# dynamic_canisters
## goal

1. we I create a new canister let's say I created two user-canister one for "Ali" and one for "James" Each canister should have a query named  "who_am_i" so I should `dfx canister call <caniset id> who_am_i '()"` this should return a string like "James"
2. Also, the backend canister should store a list of all created canisters id in static storage and it should have the query `get_all_canisters`
