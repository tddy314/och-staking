use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Only ORAI is acceptable")]
    WrongNativeToken{},

    #[error("Staked amount must be positive")]
    WrongStakeAmount{},

    #[error("Not enough balance")]
    NotEnoughBalance{},
}