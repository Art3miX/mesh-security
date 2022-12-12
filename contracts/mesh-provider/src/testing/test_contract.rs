use std::str::FromStr;

use cosmwasm_std::{
    coins,
    testing::{mock_env, mock_info},
    to_binary, Decimal, IbcMsg, Uint128, WasmMsg,
};
use mesh_apis::ClaimProviderMsg;
use mesh_ibc::ProviderMsg;
use mesh_testing::constants::{
    CHANNEL_ID, DELEGATOR_ADDR, LOCKUP_ADDR, REWARDS_IBC_DENOM, VALIDATOR,
};

use crate::{
    contract::execute,
    ibc::build_timeout,
    msg::ExecuteMsg,
    state::{Validator, CHANNEL, CONFIG, VALIDATORS},
    testing::utils::{executes::execute_slash, helpers::add_validator, queries::query_validators},
};

use super::utils::{
    executes::execute_claim_rewards, helpers::add_rewards, queries::query_provider_config,
    setup::setup_with_contract, setup_unit::setup_unit,
};

#[test]
fn test_execute_slash() {
    let (mut app, mesh_provider_addr) = setup_with_contract();

    let provider_config = query_provider_config(&app, mesh_provider_addr.as_str()).unwrap();
    let mesh_slasher_addr = provider_config.slasher.unwrap();

    // Add validator to contract (mimic ibc update call)
    add_validator(&mut app, mesh_provider_addr.clone());

    let validators = query_validators(&app, mesh_provider_addr.as_str(), None, None).unwrap();
    assert_eq!(validators.validators.len(), 1);

    // Do a slash
    execute_slash(
        &mut app,
        mesh_slasher_addr.as_str(),
        mesh_provider_addr.as_str(),
        VALIDATOR,
        "0.1",
    )
    .unwrap();

    // Multiplier was 1, we slashed by 0.1 (10%), new multiplier should be 0.9.
    let validators = query_validators(&app, mesh_provider_addr.as_str(), None, None).unwrap();
    assert_eq!(
        validators.validators[0].multiplier,
        Decimal::from_str("0.9").unwrap()
    );
}

#[test]
fn test_claim_rewards() {
    let (mut app, mesh_provider_addr) = setup_with_contract();

    // Add rewards (after calculation, we added 1000 ibc_coins)
    add_rewards(&mut app, mesh_provider_addr.clone());

    execute_claim_rewards(&mut app, mesh_provider_addr.as_str(), VALIDATOR).unwrap();

    let balance = app.wrap().query_all_balances(DELEGATOR_ADDR).unwrap();

    assert_eq!(balance, coins(1000, REWARDS_IBC_DENOM))
}

#[test]
fn test_unbond() {
    let (mut deps, _) = setup_unit(None);

    // First stake
    let info = mock_info(LOCKUP_ADDR, &[]);

    VALIDATORS
        .save(deps.as_mut().storage, VALIDATOR, &Validator::default())
        .unwrap();
    CHANNEL
        .save(deps.as_mut().storage, &CHANNEL_ID.to_string())
        .unwrap();
    execute(
        deps.as_mut(),
        mock_env(),
        info,
        ExecuteMsg::ReceiveClaim {
            owner: DELEGATOR_ADDR.to_string(),
            amount: Uint128::new(1000),
            validator: VALIDATOR.to_string(),
        },
    )
    .unwrap();

    // To unstake the delegetor need to send the request
    let info = mock_info(DELEGATOR_ADDR, &[]);

    execute(
        deps.as_mut(),
        mock_env(),
        info.clone(),
        ExecuteMsg::Unstake {
            amount: Uint128::new(1000),
            validator: VALIDATOR.to_string(),
        },
    )
    .unwrap();

    // Update block
    let unbound_period = CONFIG.load(deps.as_mut().storage).unwrap().unbonding_period;
    let mut env = mock_env();
    env.block.time = env.block.time.plus_seconds(unbound_period + 1);

    let res = execute(deps.as_mut(), env, info, ExecuteMsg::Unbond {}).unwrap();

    assert_eq!(
        res.messages[0].msg,
        WasmMsg::Execute {
            contract_addr: LOCKUP_ADDR.to_string(),
            msg: to_binary(&ClaimProviderMsg::SlashClaim {
                owner: DELEGATOR_ADDR.to_string(),
                amount: Uint128::new(1000),
            })
            .unwrap(),
            funds: vec![]
        }
        .into()
    );
}

#[test]
fn test_recieve_claim() {
    let (mut deps, _) = setup_unit(None);

    // Only lockup can send ReceiveClaim
    let info = mock_info(LOCKUP_ADDR, &[]);

    VALIDATORS
        .save(deps.as_mut().storage, VALIDATOR, &Validator::default())
        .unwrap();
    CHANNEL
        .save(deps.as_mut().storage, &CHANNEL_ID.to_string())
        .unwrap();
    let res = execute(
        deps.as_mut(),
        mock_env(),
        info,
        ExecuteMsg::ReceiveClaim {
            owner: DELEGATOR_ADDR.to_string(),
            amount: Uint128::new(1000),
            validator: VALIDATOR.to_string(),
        },
    )
    .unwrap();

    assert_eq!(
        res.messages[0].msg,
        IbcMsg::SendPacket {
            channel_id: CHANNEL_ID.to_string(),
            data: to_binary(&ProviderMsg::Stake {
                validator: VALIDATOR.to_string(),
                amount: Uint128::new(1000),
                key: DELEGATOR_ADDR.to_string()
            })
            .unwrap(),
            timeout: build_timeout(deps.as_ref(), &mock_env()).unwrap(),
        }
        .into()
    )
}

#[test]
fn test_unstake() {
    let (mut deps, _) = setup_unit(None);

    // First stake
    let info = mock_info(LOCKUP_ADDR, &[]);

    VALIDATORS
        .save(deps.as_mut().storage, VALIDATOR, &Validator::default())
        .unwrap();
    CHANNEL
        .save(deps.as_mut().storage, &CHANNEL_ID.to_string())
        .unwrap();
    execute(
        deps.as_mut(),
        mock_env(),
        info,
        ExecuteMsg::ReceiveClaim {
            owner: DELEGATOR_ADDR.to_string(),
            amount: Uint128::new(1000),
            validator: VALIDATOR.to_string(),
        },
    )
    .unwrap();

    // To unstake the delegetor need to send the request (???)
    let info = mock_info(DELEGATOR_ADDR, &[]);

    let res = execute(
        deps.as_mut(),
        mock_env(),
        info,
        ExecuteMsg::Unstake {
            amount: Uint128::new(1000),
            validator: VALIDATOR.to_string(),
        },
    )
    .unwrap();

    // No slash, so only 1 msg should exist
    assert_eq!(res.messages.len(), 1);
    assert_eq!(
        res.messages[0].msg,
        IbcMsg::SendPacket {
            channel_id: CHANNEL_ID.to_string(),
            data: to_binary(&ProviderMsg::Unstake {
                validator: VALIDATOR.to_string(),
                amount: Uint128::new(1000),
                key: DELEGATOR_ADDR.to_string()
            })
            .unwrap(),
            timeout: build_timeout(deps.as_ref(), &mock_env()).unwrap(),
        }
        .into()
    )
}
