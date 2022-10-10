use cosmwasm_std::{OverflowError, OverflowOperation, StdError, StdResult};
use liblbfgs::decimal::Decimal as FixedDecimal;

use super::math::{
    CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, DecimalMath, Infinity, MachineEpsilon,
};

impl DecimalMath for FixedDecimal {}

impl Infinity for FixedDecimal {
    fn infinity() -> Self {
        FixedDecimal::MAX - FixedDecimal::from_num(1000000000)
    }
}

impl MachineEpsilon for FixedDecimal {
    fn eps() -> Self {
        FixedDecimal::ZERO
    }
}

impl CheckedDiv<usize> for FixedDecimal {
    fn checked_div(self, rhs: usize) -> StdResult<Self> {
        self.checked_div(FixedDecimal::from_num(rhs))
            .ok_or(StdError::generic_err("Divide by zero"))
    }
}

impl CheckedDiv for FixedDecimal {
    fn checked_div(self, rhs: Self) -> Result<Self, StdError> {
        self.checked_div(rhs)
            .ok_or(StdError::generic_err("Divide by zero"))
    }
}

impl CheckedSub for FixedDecimal {
    fn checked_sub(self, rhs: Self) -> Result<Self, OverflowError> {
        self.checked_sub(rhs)
            .ok_or(OverflowError::new(OverflowOperation::Sub, self, rhs))
    }
}

impl CheckedAdd for FixedDecimal {
    fn checked_add(self, rhs: Self) -> Result<Self, OverflowError> {
        self.checked_add(rhs)
            .ok_or(OverflowError::new(OverflowOperation::Add, self, rhs))
    }
}

impl CheckedMul for FixedDecimal {
    fn checked_mul(self, rhs: Self) -> Result<Self, OverflowError> {
        self.checked_mul(rhs)
            .ok_or(OverflowError::new(OverflowOperation::Mul, self, rhs))
    }
}
