use cosmwasm_std::{OverflowError, StdError};
use cw_controllers::AdminError;
use cw_dex::CwDexError;
use thiserror::Error;

use crate::operations::SwapOperation;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    CwDexError(#[from] CwDexError),

    #[error("{0}")]
    Overflow(#[from] OverflowError),

    #[error("{0}")]
    AdminError(#[from] AdminError),

    #[error("Incorrect amount of native token sent. You don't need to pass in offer_amount if using native tokens.")]
    IncorrectNativeAmountSent,

    #[error("Unsupported asset type. Only native and cw20 tokens are supported.")]
    UnsupportedAssetType,

    #[error("No swap operations provided")]
    MustProvideOperations,

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Invalid swap operations: {operations:?}")]
    InvalidSwapOperations { operations: Vec<SwapOperation> },

    #[error("Did not receive minimum amount")]
    FailedMinimumReceive,

    #[error("No path found for assets {offer:?} -> {ask:?}")]
    NoPathFound { offer: String, ask: String },
}

impl From<ContractError> for StdError {
    fn from(x: ContractError) -> Self {
        Self::generic_err(x.to_string())
    }
}
