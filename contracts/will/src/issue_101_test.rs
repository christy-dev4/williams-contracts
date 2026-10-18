#![cfg(test)]

//! Regression tests for issue #101: `check_in` rate-limits state updates so
//! consecutive check-ins within `MIN_CHECKIN_INTERVAL` do not write redundant
//! history entries.

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Env,
};

use crate::{
    Allocation, Beneficiary, WillContract, WillContractClient, WillStatus,
};

const DAY: u64 = 86_400;

fn advance(env: &Env, days: u64) {
    env.ledger().with_mut(|l| l.timestamp += days * DAY);
}

fn setup(env: &Env) -> (WillContractClient<'_>, Address, Address) {
    let owner = Address::generate(env);
    let sac = env.register_stellar_asset_contract_v2(owner.clone());
    let token_address = sac.address();
    StellarAssetClient::new(env, &token_address).mint(&owner, &10_000_000);

    let contract_id = env.register(WillContract, ());
    let client = WillContractClient::new(env, &contract_id);
    (client, owner, token_address)
}

fn create_will(
    env: &Env,
    client: &WillContractClient<'_>,
    owner: &Address,
    token_address: &Address,
    beneficiary: &Address,
) -> u64 {
    client.create_will(
        owner,
        &vec![env, (token_address.clone(), 1_000_000_i128)],
        &vec![
            env,
            Beneficiary {
                address: beneficiary.clone(),
                allocation: Allocation::Percentage(10_000),
            },
        ],
        &90,
        &7,
        &vec![env],
        &0,
        &None,
        &0,
    )
}

#[test]
fn check_in_rate_limits_redundant_updates() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let heir = Address::generate(&env);
    let (client, owner, token_address) = setup(&env);
    let will_id = create_will(&env, &client, &owner, &token_address, &heir);

    let first_checkin = env.ledger().timestamp();
    client.check_in(&will_id, &owner);
    let after_first = client.get_will(&will_id).last_checkin;
    assert_eq!(after_first, first_checkin);

    // Immediate second check-in within MIN_CHECKIN_INTERVAL must be a no-op.
    client.check_in(&will_id, &owner);
    let after_second = client.get_will(&will_id).last_checkin;
    assert_eq!(
        after_second, first_checkin,
        "consecutive check-in within the interval must not update last_checkin"
    );

    // Advancing past the interval allows another update.
    advance(&env, 2); // 2 days > 1 minute interval
    client.check_in(&will_id, &owner);
    let after_third = client.get_will(&will_id).last_checkin;
    assert!(after_third > first_checkin, "check-in after interval must update");
}
