#![cfg(test)]

//! Regression tests for issue #102: `request_grace_extension` stacks
//! additional grace days on a `Triggered` will without resetting the
//! check-in cycle, capped at `MAX_GRACE_EXTENSION_DAYS`.

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Env,
};

use crate::{
    Allocation, Beneficiary, WillContract, WillContractClient, WillError, WillStatus,
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

/// Drives a will into `Triggered` by advancing past the check-in period and
/// invoking `trigger_will` as a non-owner caller.
fn trigger_will(env: &Env, client: &WillContractClient<'_>, will_id: u64) {
    env.ledger().with_mut(|l| l.timestamp += 91 * DAY);
    client.trigger_will(&will_id);
}

#[test]
fn request_grace_extension_stacks_without_status_reset() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let heir = Address::generate(&env);
    let (client, owner, token_address) = setup(&env);
    let will_id = create_will(&env, &client, &owner, &token_address, &heir);

    trigger_will(&env, &client, will_id);
    assert_eq!(client.get_will(&will_id).status, WillStatus::Triggered);

    // Request a 7-day extension while triggered.
    client.request_grace_extension(&will_id, &owner, 7);

    let will = client.get_will(&will_id);
    assert_eq!(will.status, WillStatus::Triggered, "status stays Triggered");
    assert_eq!(
        will.grace_extension_days, 7,
        "extension is recorded"
    );
}

#[test]
fn request_grace_extension_rejects_zero_days() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let heir = Address::generate(&env);
    let (client, owner, token_address) = setup(&env);
    let will_id = create_will(&env, &client, &owner, &token_address, &heir);

    trigger_will(&env, &client, will_id);

    let err = client
        .try_request_grace_extension(&will_id, &owner, 0)
        .unwrap_err()
        .unwrap();
    assert_eq!(
        err,
        WillError::InvalidPeriod.into(),
        "zero-day extension is rejected"
    );
}

#[test]
fn request_grace_extension_rejects_above_max() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let heir = Address::generate(&env);
    let (client, owner, token_address) = setup(&env);
    let will_id = create_will(&env, &client, &owner, &token_address, &heir);

    trigger_will(&env, &client, will_id);

    let err = client
        .try_request_grace_extension(&will_id, &owner, 31)
        .unwrap_err()
        .unwrap();
    assert_eq!(
        err,
        WillError::InvalidPeriod.into(),
        "extension above MAX_GRACE_EXTENSION_DAYS is rejected"
    );
}

#[test]
fn request_grace_extension_rejects_after_grace_expired() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let heir = Address::generate(&env);
    let (client, owner, token_address) = setup(&env);
    let will_id = create_will(&env, &client, &owner, &token_address, &heir);

    trigger_will(&env, &client, will_id);

    // Advance past the grace period (7 days).
    advance(&env, 8);

    let err = client
        .try_request_grace_extension(&will_id, &owner, 7)
        .unwrap_err()
        .unwrap();
    assert_eq!(
        err,
        WillError::GracePeriodExpired.into(),
        "extension after grace expiry is rejected"
    );
}
