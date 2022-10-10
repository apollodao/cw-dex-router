use std::{
    iter::Sum,
    ops::{Add, Mul},
};

use cosmwasm_std::{OverflowError, StdError};

use liblbfgs::decimal_math::BfgsMath;

pub trait DecimalMath:
    Infinity
    + MachineEpsilon
    + CheckedMul
    + PartialOrd
    + CheckedSub
    + CheckedDiv
    + CheckedAdd
    + Copy
    + Sum
    + Mul<Output = Self>
    + Add<Output = Self>
    + CheckedDiv<usize>
    + BfgsMath
{
}

pub trait Infinity {
    fn infinity() -> Self;
}

pub trait CheckedMul<Rhs = Self>
where
    Self: Sized,
{
    fn checked_mul(self, rhs: Rhs) -> Result<Self, OverflowError>;
}

pub trait CheckedSub<Rhs = Self> {
    fn checked_sub(self, rhs: Rhs) -> Result<Self, OverflowError>
    where
        Self: Sized;
}

pub trait CheckedDiv<Rhs = Self> {
    fn checked_div(self, rhs: Rhs) -> Result<Self, StdError>
    where
        Self: Sized;
}

pub trait CheckedAdd<Rhs = Self> {
    fn checked_add(self, rhs: Rhs) -> Result<Self, OverflowError>
    where
        Self: Sized;
}

pub trait MachineEpsilon {
    fn eps() -> Self;
}
