#![cfg(test)]

//! Regression tests for issue #103: non-zero `confirmation_delay_seconds`
//! must satisfy `MIN_CONFIRMATION_DELAY` (300s); trivial micro-delays are
//! rejected with `WillError::InvalidPeriod`.

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Env,
};

use crate::{
    Allocation, Beneficiary, WillContract, WillContractClient, WillError, WillStatus,
};

fn setup(env: &Env) -> (WillContractClient<'_>, Address, Address) {
    let owner = Address::generate(env);
    let sac = env.register_stellar_asset_contract_v2(owner.clone());
    let token_address = sac.address();
    StellarAssetClient::new(env, &token_address).mint(&owner, &10_000_000);

    let contract_id = env.register(WillContract, ());
    let client = WillContractClient::new(env, &contract_id);
    (client, owner, token_address)
}

fn make_create_call<'a>(
    client: &WillContractClient<'a>,
    env: &Env,
    owner: &Address,
    token_address: &Address,
    beneficiary: &Address,
    delay_seconds: u64,
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
        &Some(delay_seconds),
        &0,
    )
}

#[test]
fn create_will_rejects_sub_threshold_confirmation_delay() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let heir = Address::generate(&env);
    let (client, owner, token_address) = setup(&env);

    let err = client
        .try_create_will(
            &owner,
            &vec![env, (token_address.clone(), 1_000_000_i128)],
            &vec![
                env,
                Beneficiary {
                    address: heir.clone(),
                    allocation: Allocation::Percentage(10_000),
                },
            ],
            &90,
            &7,
            &vec![env],
            &0,
            &Some(5), // 5 seconds — below MIN_CONFIRMATION_DELAY (300s)
            &0,
        )
        .unwrap_err()
        .unwrap();
    assert_eq!(
        err,
        WillError::InvalidPeriod.into(),
        "confirmation delay of 5s must be rejected"
    );
}

#[test]
fn create_will_accepts_zero_delay() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let heir = Address::generate(&env);
    let (client, owner, token_address) = setup(&env);

    // Zero delay is explicitly allowed — will starts Active.
    let will_id = make_create_call(&client, &env, &owner, &token_address, &heir, 0);
    assert_eq!(
        client.get_will(&will_id).status,
        WillStatus::Active,
        "zero-delay will starts Active"
    );
}

#[test]
fn create_will_accepts_delay_at_threshold() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let heir = Address::generate(&env);
    let (client, owner, token_address) = setup(&env);

    // Exactly MIN_CONFIRMATION_DELAY (300s) is accepted.
    let will_id = make_create_call(&client, &env, &owner, &token_address, &heir, 300);
    assert_eq!(
        client.get_will(&will_id).status,
        WillStatus::PendingConfirmation,
        "delay at threshold starts PendingConfirmation"
    );
}
