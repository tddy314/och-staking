
#[cfg(not(feature = "library"))]

use cosmwasm_std::entry_point;
use cosmwasm_std::{Addr, Coin, Empty, StdAck, Uint128};
use cosmwasm_std::{Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, to_json_binary, BankMsg, CosmosMsg, WasmMsg, WasmQuery, QueryRequest};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, ViewAPRResponse, ViewRewardResponse, ViewStakeAmount, ViewBalance};

use cw2::set_contract_version;
use cw20::{self, Cw20ExecuteMsg};

use crate::state::{Config, StakeInfo, RewardInfo, CONFIG, USERS, REWARD, ORACLE};


const CONTRACT_NAME: &str = "crates.io:och-staking";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const year: Uint128 = Uint128::new(31536000);
const PRECISION: Uint128 = Uint128::new(10u128.pow(6));
const USDC_ADDRESS: &str = "orai14x647uadcp3wxav6vvjyq23vtwvkkhqnfy9w4vp77h36qx3gdmhq0ws7zh";


//_________________________________________________________________________

//_______________________________
//|                              |
//|                              |
//|         INSTANTIATE          |  
//|                              |
//|______________________________|

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    
    // Init Config
    let admin = msg.admin.unwrap_or(info.sender.to_string());
    let updater = msg.updater.unwrap_or(admin.clone());

    let validated_admin  = deps.api.addr_validate(&admin)?;
    let validated_updater = deps.api.addr_validate(&updater)?;

    let config = Config {
        admin: validated_admin.clone(),
        updater: validated_updater.clone(),
    };

    CONFIG.save(deps.storage, &config)?;

    // Init Reward
    let rps_init = Uint128::new(msg.rps.unwrap_or(0));
    
    let reward = RewardInfo {
        last_update: env.block.time,
        cur_sum_index: Uint128::new(0),
        rps: rps_init,
        total_stake: Uint128::new(0),
    };

    REWARD.save(deps.storage, &reward)?;

    //Init oracle
    let oracle_init = Uint128::new(msg.oracle.unwrap_or(0));
    ORACLE.save(deps.storage, &oracle_init)?;

    Ok(
        Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin", validated_admin.to_string())
        .add_attribute("updater", validated_updater.to_string())
    )
}


//_________________________________________________________________________

//_______________________________
//|                              |
//|                              |
//|          EXECUTE             |  
//|                              |
//|______________________________|

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Stake {  } => stake(deps, env, info),
        ExecuteMsg::Unstake { amount } => unstake(deps, env, info, amount),
        ExecuteMsg::UpdateCurSumIndex {  } => update_cur_sum_index(deps, env),
        ExecuteMsg::ClaimReward {  } => claim_reward(deps, env, info),
        ExecuteMsg::UpdateOracle { price } => update_oracle(deps, env, info, price),
        ExecuteMsg::UpdateUserReward { account } => update_user_reward(deps, env, info, account),
        ExecuteMsg::UpdateRewardPerSecond { new_rps } => update_rps(deps, env, info, new_rps),
    }
}


fn stake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {

    if info.funds.len() == 0 {
        return Err(ContractError::WrongStakeAmount{});
    }

    //Get user info

    let account = info.sender;

    if info.funds.iter().any(|coin| coin.denom != "orai") {
        return Err(ContractError::WrongNativeToken{});
    }

    let user_stake = Uint128::new(info.funds.iter().map(|coin| coin.amount.u128()).sum());
    
    if user_stake == Uint128::zero() {
        return Err(ContractError::WrongStakeAmount{});
    }
    
    let user_stake_info = USERS.may_load(deps.storage, account.clone())?;
    
    //Update cur_sum_index
    let mut reward_info = REWARD.load(deps.storage)?;
    let cur = env.block.time;
    let time_passed: u64 = cur.seconds() - reward_info.last_update.seconds();

    if reward_info.total_stake > Uint128::zero() && time_passed > 0 {
        let reward: Uint128 = reward_info.rps * Uint128::from(time_passed);
        reward_info.cur_sum_index = reward_info.cur_sum_index + (reward * PRECISION) / reward_info.total_stake;
    }

    reward_info.last_update = cur;

    //Update User Stake

    match user_stake_info {
        Some(mut stake_info) => {
            //Update user_reward
            stake_info.reward = stake_info.reward + (reward_info.cur_sum_index - stake_info.index) * stake_info.balance / PRECISION;
            stake_info.index = reward_info.cur_sum_index;
            //Update user_balance
            stake_info.balance = stake_info.balance + user_stake;

            USERS.save(deps.storage, account, &stake_info)?;
        },
        None => {
            let stake_info = StakeInfo {
                balance: user_stake,
                reward: Uint128::zero(),
                index: reward_info.cur_sum_index,
            };

            USERS.save(deps.storage, account, &stake_info)?;
        }
    }

    reward_info.total_stake = reward_info.total_stake + user_stake;
    REWARD.save(deps.storage, &reward_info)?;

    Ok(Response::new())
}

fn unstake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: u128,
) -> Result<Response, ContractError> {

    //Get user_unstake_info 
    let account = info.sender;
    let address = account.clone().to_string();
    let unstake_amount = Uint128::from(amount);
    let mut user_stake_info = USERS.load(deps.storage, account.clone())?;

    if user_stake_info.balance < unstake_amount {
        return Err(ContractError::NotEnoughBalance{});
    }

    //get reward_info && update
    let mut reward_info = REWARD.load(deps.storage)?;
    let cur = env.block.time;
    let time_passed = cur.seconds() - reward_info.last_update.seconds();

    if reward_info.total_stake > Uint128::zero() && time_passed > 0 {
        let reward: Uint128 = reward_info.rps * Uint128::from(time_passed);
        reward_info.cur_sum_index = reward_info.cur_sum_index + (reward * PRECISION) / reward_info.total_stake;
    }

    reward_info.last_update = cur;

    //update user's reward
    user_stake_info.reward = user_stake_info.reward + (reward_info.cur_sum_index - user_stake_info.index) * user_stake_info.balance / PRECISION;
    user_stake_info.index = reward_info.cur_sum_index;
    //update user's balance
    user_stake_info.balance = user_stake_info.balance - unstake_amount;
    reward_info.total_stake = reward_info.total_stake - unstake_amount;
    //Transfer to user
    let send_msg: CosmosMsg<Empty> = CosmosMsg::Bank(BankMsg::Send {
        to_address: address,
        amount: vec![Coin{
            denom: "orai".to_string(),
            amount: unstake_amount,
        }],
    });

    //Save
    USERS.save(deps.storage, account, &user_stake_info)?;
    REWARD.save(deps.storage, &reward_info)?;

    Ok(Response::new().add_message(send_msg))
}

fn update_cur_sum_index(
    deps: DepsMut,
    env: Env,
) -> Result<Response, ContractError> {
    let mut reward_info = REWARD.load(deps.storage)?;
    let cur = env.block.time;
    let time_passed: u64 = cur.seconds() - reward_info.last_update.seconds();

    if reward_info.total_stake > Uint128::zero() && time_passed > 0 {
        let reward: Uint128 = reward_info.rps * Uint128::from(time_passed);
        reward_info.cur_sum_index = reward_info.cur_sum_index + (reward * PRECISION) / reward_info.total_stake;
    }

    reward_info.last_update = cur;
    REWARD.save(deps.storage, &reward_info)?;
    
    Ok(Response::new())
}

fn claim_reward(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    //Get user
    let account = info.sender;
    let user_address = account.clone().to_string();

    //update cur sum index
    let mut reward_info = REWARD.load(deps.storage)?;
    let cur = env.block.time;
    let time_passed = cur.seconds() - reward_info.last_update.seconds();

    if reward_info.total_stake > Uint128::zero() && time_passed > 0 {
        let reward: Uint128 = reward_info.rps * Uint128::from(time_passed);
        reward_info.cur_sum_index = reward_info.cur_sum_index + (reward * PRECISION) / reward_info.total_stake;
    }

    reward_info.last_update = cur;
    REWARD.save(deps.storage, &reward_info)?;
    //Update user reward
    let mut user = USERS.load(deps.storage, account.clone())?;
    let claim_reward = user.reward + (reward_info.cur_sum_index - user.index) * user.balance / PRECISION;
    user.index = reward_info.cur_sum_index;
    user.reward = Uint128::zero();
    USERS.save(deps.storage, account, &user)?;

    let msg = WasmMsg::Execute { 
        contract_addr: USDC_ADDRESS.to_string(), 
        msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
            recipient: user_address,
            amount: claim_reward,
        })?, 
        funds: vec![], 
    };

    Ok(Response::new().add_message(msg))
}

fn update_oracle(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    price: u128,
) -> Result<Response, ContractError> {
    let account = info.sender;
    let config = CONFIG.load(deps.storage)?;
    
    if config.admin != account {
        return Err(ContractError::Unauthorized{});
    }

    let p = Uint128::from(price);
    ORACLE.save(deps.storage, &p)?;

    Ok(Response::new())
}

fn update_user_reward(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    account: String,
) -> Result<Response, ContractError> {
    let acc_addr = deps.api.addr_validate(&account)?;

    //update cur sum index
    let mut reward_info = REWARD.load(deps.storage)?;
    let cur = env.block.time;
    let time_passed: u64 = cur.seconds() - reward_info.last_update.seconds();

    if reward_info.total_stake > Uint128::zero() && time_passed > 0 {
        let reward: Uint128 = reward_info.rps * Uint128::from(time_passed);
        reward_info.cur_sum_index = reward_info.cur_sum_index + (reward * PRECISION) / reward_info.total_stake;
    }

    reward_info.last_update = cur;
    REWARD.save(deps.storage, &reward_info)?;
    

    //update user reward

    let user = USERS.may_load(deps.storage, acc_addr.clone())?;

    match user {
        Some(mut stake_info) => {
            stake_info.reward = stake_info.reward + (reward_info.cur_sum_index - stake_info.index) * stake_info.balance / PRECISION;
            stake_info.index = reward_info.cur_sum_index;
            USERS.save(deps.storage, acc_addr, &stake_info)?;
        },
        None => {
            let stake_info = StakeInfo {
                balance: Uint128::zero(),
                reward: Uint128::zero(),
                index: reward_info.cur_sum_index,
            };
            USERS.save(deps.storage, acc_addr, &stake_info)?;
        }
    };

    Ok(Response::new())
}


fn update_rps(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    new_rps: u128,
) -> Result<Response, ContractError> {
    
    let config = CONFIG.load(deps.storage)?;

    if info.sender != config.updater {
        return  Err(ContractError::Unauthorized { });
    }

    //update cur sum index
    let mut reward_info = REWARD.load(deps.storage)?;
    let cur = env.block.time;
    let time_passed: u64 = cur.seconds() - reward_info.last_update.seconds();

    if reward_info.total_stake > Uint128::zero() && time_passed > 0 {
        let reward: Uint128 = reward_info.rps * Uint128::from(time_passed);
        reward_info.cur_sum_index = reward_info.cur_sum_index + (reward * PRECISION) / reward_info.total_stake;
    }

    reward_info.last_update = cur;
    reward_info.rps = Uint128::from(new_rps);

    REWARD.save(deps.storage, &reward_info)?;
    
    Ok(Response::new())
}

//_________________________________________________________________________

//_______________________________
//|                              |
//|                              |
//|            QUERY             |  
//|                              |
//|______________________________|

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::ViewAPR {} => view_apr(deps, env),
        QueryMsg::ViewReward { account } => view_reward(deps, env, account),
        QueryMsg::CheckStakeAmount { address } => check_stake_amount(deps, env, address),
    }
}

fn check_stake_amount(
    deps: Deps,
    _env: Env,
    address: String,
) -> StdResult<Binary> {
    let user = USERS.may_load(deps.storage, deps.api.addr_validate(&address)?)?;
    match user {
        None => to_json_binary(&ViewStakeAmount{balance: Uint128::zero()}),
        Some(stake) => to_json_binary(&ViewStakeAmount{balance: stake.balance}),
    }
}



fn view_apr(
    deps: Deps,
    _env: Env,
) -> StdResult<Binary> {
    let reward_info = REWARD.load(deps.storage)?;
    let price = ORACLE.load(deps.storage)?;
    let ts = reward_info.rps * year;
    let ms = reward_info.total_stake * price;
    let apr = ts * Uint128::new(100) / ms ;
    to_json_binary(&ViewAPRResponse{apr})
}

fn view_reward(
    deps: Deps,
    env: Env,
    address: String,
) -> StdResult<Binary> {
    let account = deps.api.addr_validate(&address)?;
    let mut reward_info = REWARD.load(deps.storage)?;
    let cur = env.block.time;
    let time_passed: u64 = cur.seconds() - reward_info.last_update.seconds();

    if reward_info.total_stake > Uint128::zero() && time_passed > 0 {
        let reward: Uint128 = reward_info.rps * Uint128::from(time_passed);
        reward_info.cur_sum_index = reward_info.cur_sum_index + (reward * PRECISION) / reward_info.total_stake;
    }

    let user_info = USERS.may_load(deps.storage, account)?;

    match user_info {
        Some(mut stake_info) => {
            stake_info.reward = stake_info.reward + (reward_info.cur_sum_index - stake_info.index) * stake_info.balance / PRECISION;
            to_json_binary(&ViewRewardResponse{reward: stake_info.reward})
        },
        None => {
            to_json_binary(&ViewRewardResponse{reward: Uint128::zero()})
        }
    }

}




#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    unimplemented!()
}



#[cfg(test)]
mod tests {
    use std::panic::Location;

    use cosmwasm_std::{attr, from_binary, Api, Empty, Querier, QuerierWrapper, Timestamp};
    use cosmwasm_std::testing::{mock_dependencies, mock_dependencies_with_balance, mock_env, mock_info};
    use cw_multi_test::Bank;
    use serde::de::value::UsizeDeserializer;
    use crate::contract::{execute, instantiate, query, stake};
    use crate::msg::{
         ExecuteMsg, InstantiateMsg, QueryMsg, ViewAPRResponse, ViewBalance, ViewRewardResponse, ViewStakeAmount
    };
    use crate::state::{CONFIG, REWARD, USERS, ORACLE, StakeInfo, RewardInfo, Config};

    use cosmwasm_std::{coin, coins, Addr, BankMsg, CosmosMsg, BankQuery, QueryRequest, Coin};

    pub const address1: &str = "orai1kwyeufzwmwgqwy2aa7ycv4wxaglwd38lkepa05";
    pub const address2: &str = "orai1acsj7emfhkcn8vzjrm8j8qkdh3czdgutpxdent";

    #[test]
    fn test() {
        let mut deps = mock_dependencies();
        let mut env = mock_env();


        let info = mock_info(address1, &vec![]);

        let user1= Addr::unchecked(address1);
        let user2= Addr::unchecked(address2);
        
        let amount = coins(100000, "orai");

        deps.querier.update_balance(&user1, amount.clone());

        deps.querier.update_balance(&user2, amount.clone());


        let msg = InstantiateMsg {
            admin: Some(address1.to_string()),
            updater: Some(address2.to_string()),
            rps: Some(2323),
            oracle: Some(2920000),
        };

        let res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();
        
        let config = CONFIG.load(deps.as_ref().storage).unwrap();
        eprintln!("{} and {}", config.admin, config.updater);

        let reward = REWARD.load(deps.as_ref().storage).unwrap();
        eprintln!("last update: {}", reward.last_update.seconds());
        eprintln!("rps: {}", reward.rps);

        //////////////////////////////////
        //     Stake   
        /////////////////////////////////
        let info = mock_info(address2, &coins(10, "orai"));
        let msg = ExecuteMsg::Stake {  };

        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        let user2_info = USERS.load(deps.as_ref().storage, user2.clone()).unwrap();

        eprintln!("user2's balance  {}", user2_info.balance);

        env.block.time = Timestamp::from_seconds(reward.last_update.seconds()+100); 
        //Check reward
        let msg = QueryMsg::ViewReward { account: address2.to_string() };
        let bin  = query(deps.as_ref(), env.clone(), msg).unwrap();
        let res: ViewRewardResponse = from_binary(&bin).unwrap();
        eprintln!("user2's reward: {}", res.reward);

        //view stake amount
        let msg = QueryMsg::CheckStakeAmount { address:address2.to_string() };
        let bin = query(deps.as_ref(), env.clone(), msg).unwrap();
        let res: ViewStakeAmount = from_binary(&bin).unwrap();
        eprintln!("user2's balance by query: {}", res.balance);

        ////////////////////////////////////////////////////
        let info = mock_info(address1, &coins(10, "orai"));
        let msg = ExecuteMsg::Stake {  };
        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
        let user1_info  = USERS.load(deps.as_ref().storage, user1.clone()).unwrap();
        eprintln!("user1's balance: {}", user1_info.balance);

        env.block.time = Timestamp::from_seconds(env.block.time.seconds()+100); 

        //check reward
        let msg = QueryMsg::ViewReward { account: address1.to_string() };
        let bin  = query(deps.as_ref(), env.clone(), msg).unwrap();
        let res: ViewRewardResponse = from_binary(&bin).unwrap();
        eprintln!("user1's reward: {}", res.reward);
        

        /*//change rps 

        // use address2 to update
        let info = mock_info(address2, &vec![]);
        let msg = ExecuteMsg::UpdateRewardPerSecond { new_rps: 1232 };
        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
        
        //view rps
        let reward = REWARD.load(deps.as_ref().storage).unwrap();
        eprintln!("rps: {}", reward.rps);
        //eprintln!("env.block.time  {}", env.block.time.seconds());
        eprintln!("last update: {}", reward.last_update.seconds());
        //view APR
        let msg = QueryMsg::ViewAPR {  };
        let bin = query(deps.as_ref(), env.clone(), msg).unwrap();
        let res: ViewAPRResponse = from_binary(&bin).unwrap();
        eprintln!("Apr: {}", res.apr);
        
        // stake more

        env.block.time = Timestamp::from_seconds(reward.last_update.seconds() + 10000);
        
        let info = mock_info(address2, &coins(30, "orai"));
        let msg = ExecuteMsg::Stake {  };
        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        let user = USERS.load(deps.as_ref().storage, user2.clone()).unwrap();
        eprintln!("user2's balance: {}", user.balance);
        eprintln!("user2's reward: {}", user.reward);
        //

        //unstake
        env.block.time = Timestamp::from_seconds(env.block.time.seconds() + 1000);

        let info = mock_info(address2, &vec![]);
        let msg = ExecuteMsg::Unstake { amount:20 };
        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        let user = USERS.load(deps.as_ref().storage, user2.clone()).unwrap();
        eprintln!("user2's balance: {}", user.balance);
        eprintln!("user2's reward: {}", user.reward);

        //view reward
        env.block.time = Timestamp::from_seconds(env.block.time.seconds() + 2000);
        let msg = QueryMsg::ViewReward { account: address2.to_string() };
        let bin = query(deps.as_ref(), env.clone(), msg).unwrap();
        let res:ViewRewardResponse = from_binary(&bin).unwrap();

        eprintln!("user2's balance: {}", res.reward);*/

    }
}