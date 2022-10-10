use std::{
    fmt::{Display, Formatter},
    iter::Sum,
    ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign},
    str::FromStr,
};

use cosmwasm_std::{Decimal256, DivideByZeroError, OverflowError, StdError, StdResult};
use liblbfgs::{
    decimal_math::{Abs, BfgsMath, FromInt, IsSignPositive, One, Sqrt, Zero},
    vector_math::VectorMath,
};

use super::math::{
    CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, DecimalMath, Infinity, MachineEpsilon,
};

#[derive(Copy, Clone, Debug, Eq, Ord, Default)]
pub struct SignedDecimal256 {
    pub value: Decimal256,
    pub sign: bool,
}

impl DecimalMath for SignedDecimal256 {}
impl BfgsMath for SignedDecimal256 {}

impl VectorMath<SignedDecimal256> for [SignedDecimal256] {
    /// y += c*x
    fn vecadd(&mut self, x: &[SignedDecimal256], c: SignedDecimal256) {
        for (y, x) in self.iter_mut().zip(x) {
            *y += &c * x;
        }
    }

    /// s = y.dot(x)
    fn vecdot(&self, other: &[SignedDecimal256]) -> SignedDecimal256 {
        self.iter().zip(other).map(|(x, y)| x * y).sum()
    }

    /// y *= c
    fn vecscale(&mut self, c: SignedDecimal256) {
        for y in self.iter_mut() {
            *y *= c;
        }
    }

    /// y = x
    fn veccpy(&mut self, x: &[SignedDecimal256]) {
        for (v, x) in self.iter_mut().zip(x) {
            *v = *x;
        }
    }

    /// y = -x
    fn vecncpy(&mut self, x: &[SignedDecimal256]) {
        for (v, x) in self.iter_mut().zip(x) {
            *v = -x;
        }
    }

    /// z = x - y
    fn vecdiff(&mut self, x: &[SignedDecimal256], y: &[SignedDecimal256]) {
        for ((z, x), y) in self.iter_mut().zip(x).zip(y) {
            *z = x - y;
        }
    }

    /// ||x||
    fn vec2norm(&self) -> SignedDecimal256 {
        let n2 = self.vecdot(&self);
        n2.sqrt()
    }

    /// 1/||x||
    fn vec2norminv(&self) -> SignedDecimal256 {
        SignedDecimal256::one() / self.vec2norm()
    }
}

impl SignedDecimal256 {
    pub fn new(value: Decimal256, sign: bool) -> Self {
        Self { value, sign }
    }

    pub fn from_str(value: &str, sign: bool) -> Result<Self, String> {
        let value = Decimal256::from_str(value).map_err(|e| e.to_string())?;
        Ok(Self { value, sign })
    }

    pub fn from_u128(value: u128) -> Self {
        Self {
            value: Decimal256::from_ratio(value, 1u128),
            sign: true,
        }
    }

    pub fn from_i128(value: i128) -> Self {
        let sign = value >= 0;
        Self {
            value: Decimal256::from_ratio(value.abs() as u128, 1u128),
            sign,
        }
    }

    pub fn to_string(&self) -> String {
        if self.sign {
            self.value.to_string()
        } else {
            format!("-{}", self.value.to_string())
        }
    }

    pub fn to_decimal(&self) -> StdResult<Decimal256> {
        if self.sign {
            Ok(self.value)
        } else {
            Err(StdError::generic_err("Negative value"))
        }
    }

    pub fn is_zero(&self) -> bool {
        self.value.is_zero()
    }

    pub fn is_positive(&self) -> bool {
        self.sign
    }

    pub fn is_negative(&self) -> bool {
        !self.sign
    }

    pub fn abs(&self) -> Self {
        Self {
            value: self.value,
            sign: true,
        }
    }
}

impl From<i32> for SignedDecimal256 {
    fn from(value: i32) -> Self {
        Self::from_i128(value as i128)
    }
}

impl From<u128> for SignedDecimal256 {
    fn from(value: u128) -> Self {
        Self::from_u128(value)
    }
}

impl From<i128> for SignedDecimal256 {
    fn from(value: i128) -> Self {
        Self::from_i128(value)
    }
}

impl From<&str> for SignedDecimal256 {
    fn from(value: &str) -> Self {
        Self::from_str(value, true).unwrap()
    }
}

impl Mul for SignedDecimal256 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            value: self.value * rhs.value,
            sign: self.sign == rhs.sign,
        }
    }
}

impl Mul for &SignedDecimal256 {
    type Output = SignedDecimal256;

    fn mul(self, rhs: Self) -> Self::Output {
        SignedDecimal256 {
            value: self.value * rhs.value,
            sign: self.sign == rhs.sign,
        }
    }
}

impl MulAssign for SignedDecimal256 {
    fn mul_assign(&mut self, rhs: Self) {
        self.value *= rhs.value;
        self.sign = self.sign == rhs.sign;
    }
}

impl Div for SignedDecimal256 {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        Self {
            value: self.value / rhs.value,
            sign: self.sign == rhs.sign,
        }
    }
}

impl DivAssign for SignedDecimal256 {
    fn div_assign(&mut self, rhs: Self) {
        self.value /= rhs.value;
        self.sign = self.sign == rhs.sign;
    }
}

impl Add for SignedDecimal256 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        if self.sign == rhs.sign {
            Self {
                value: self.value + rhs.value,
                sign: self.sign,
            }
        } else if self.value >= rhs.value {
            Self {
                value: self.value - rhs.value,
                sign: self.sign,
            }
        } else {
            Self {
                value: rhs.value - self.value,
                sign: rhs.sign,
            }
        }
    }
}

impl AddAssign for SignedDecimal256 {
    fn add_assign(&mut self, rhs: Self) {
        if self.sign == rhs.sign {
            self.value += rhs.value;
        } else if self.value >= rhs.value {
            self.value -= rhs.value;
        } else {
            self.value = rhs.value - self.value;
            self.sign = rhs.sign;
        }
    }
}

impl Sub for SignedDecimal256 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        if self.sign == rhs.sign {
            if self.value >= rhs.value {
                Self {
                    value: self.value - rhs.value,
                    sign: self.sign,
                }
            } else {
                Self {
                    value: rhs.value - self.value,
                    sign: !self.sign,
                }
            }
        } else {
            Self {
                value: self.value + rhs.value,
                sign: self.sign,
            }
        }
    }
}

impl Sub for &SignedDecimal256 {
    type Output = SignedDecimal256;

    fn sub(self, rhs: Self) -> Self::Output {
        if self.sign == rhs.sign {
            if self.value >= rhs.value {
                SignedDecimal256 {
                    value: self.value - rhs.value,
                    sign: self.sign,
                }
            } else {
                SignedDecimal256 {
                    value: rhs.value - self.value,
                    sign: !self.sign,
                }
            }
        } else {
            SignedDecimal256 {
                value: self.value + rhs.value,
                sign: self.sign,
            }
        }
    }
}

impl SubAssign for SignedDecimal256 {
    fn sub_assign(&mut self, rhs: Self) {
        self.value -= rhs.value;
    }
}

impl PartialEq for SignedDecimal256 {
    fn eq(&self, other: &Self) -> bool {
        // Zero is equal to zero even if sign is opposite.
        if self.value.is_zero() && other.value.is_zero() {
            true
        } else {
            self.sign == other.sign && self.value == other.value
        }
    }
}

impl PartialOrd for SignedDecimal256 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.sign == other.sign {
            self.value.partial_cmp(&other.value)
        } else if self.sign {
            Some(std::cmp::Ordering::Greater)
        } else {
            Some(std::cmp::Ordering::Less)
        }
    }
}

impl FromStr for SignedDecimal256 {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let sign = s.starts_with('-');
        let value =
            Decimal256::from_str(if sign { &s[1..] } else { s }).map_err(|e| e.to_string())?;
        Ok(Self { value, sign })
    }
}

impl Display for SignedDecimal256 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl Abs for SignedDecimal256 {
    fn abs(self) -> Self {
        Self {
            value: self.value,
            sign: true,
        }
    }
}

impl Neg for SignedDecimal256 {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            value: self.value,
            sign: !self.sign,
        }
    }
}

impl Neg for &SignedDecimal256 {
    type Output = SignedDecimal256;

    fn neg(self) -> Self::Output {
        SignedDecimal256 {
            value: self.value,
            sign: !self.sign,
        }
    }
}

impl IsSignPositive for SignedDecimal256 {
    fn is_sign_positive(&self) -> bool {
        self.sign
    }
}

impl FromInt for SignedDecimal256 {
    fn from_i16(i: i16) -> Result<Self, anyhow::Error>
    where
        Self: Sized,
    {
        Ok(Self::from_i128(i as i128))
    }
}

impl Sum for SignedDecimal256 {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::zero(), |acc, x| acc + x)
    }
}

impl MachineEpsilon for SignedDecimal256 {
    fn eps() -> Self {
        Self {
            // TODO. Different value?
            value: Decimal256::zero(),
            sign: true,
        }
    }
}

impl Sqrt for SignedDecimal256 {
    fn sqrt(&self) -> Self {
        Self {
            value: self.value.sqrt(),
            sign: true,
        }
    }
}

impl Infinity for SignedDecimal256 {
    fn infinity() -> Self {
        Self {
            value: Decimal256::MAX,
            sign: true,
        }
    }
}

impl One for SignedDecimal256 {
    fn one() -> Self {
        Self {
            value: Decimal256::one(),
            sign: true,
        }
    }
}

impl Zero for SignedDecimal256 {
    fn zero() -> Self {
        Self {
            value: Decimal256::zero(),
            sign: true,
        }
    }
}

impl From<Decimal256> for SignedDecimal256 {
    fn from(value: Decimal256) -> Self {
        Self { value, sign: true }
    }
}

impl CheckedDiv for SignedDecimal256 {
    fn checked_div(self, rhs: Self) -> Result<Self, StdError> {
        if rhs.is_zero() {
            return Err(StdError::DivideByZero {
                source: DivideByZeroError {
                    operand: String::from("SignedfDEcimal256 checked_div"),
                },
            });
        } else {
            Ok(Self {
                value: self.value.checked_div(rhs.value).map_err(|e| {
                    StdError::generic_err(format!("SignedDecimal256 checked_div: {}", e))
                })?,
                sign: self.sign == rhs.sign,
            })
        }
    }
}

impl CheckedDiv<usize> for SignedDecimal256 {
    fn checked_div(self, rhs: usize) -> Result<Self, StdError> {
        self.checked_div(Self::from_u128(rhs as u128))
    }
}

impl CheckedAdd for SignedDecimal256 {
    fn checked_add(self, rhs: Self) -> Result<Self, OverflowError>
    where
        Self: Sized,
    {
        if self.sign == rhs.sign {
            Ok(Self {
                value: self.value.checked_add(rhs.value)?,
                sign: self.sign,
            })
        } else if self.value >= rhs.value {
            Ok(Self {
                value: self.value.checked_sub(rhs.value)?,
                sign: self.sign,
            })
        } else {
            Ok(Self {
                value: rhs.value.checked_sub(self.value)?,
                sign: rhs.sign,
            })
        }
    }
}

impl CheckedSub for SignedDecimal256 {
    fn checked_sub(self, rhs: Self) -> Result<Self, OverflowError>
    where
        Self: Sized,
    {
        if self.sign != rhs.sign {
            Ok(Self {
                value: self.value.checked_add(rhs.value)?,
                sign: self.sign,
            })
        } else if self.value >= rhs.value {
            Ok(Self {
                value: self.value.checked_sub(rhs.value)?,
                sign: self.sign,
            })
        } else {
            Ok(Self {
                value: rhs.value.checked_sub(self.value)?,
                sign: !self.sign,
            })
        }
    }
}

impl CheckedMul for SignedDecimal256 {
    fn checked_mul(self, rhs: Self) -> Result<Self, OverflowError>
    where
        Self: Sized,
    {
        Ok(Self {
            value: self.value.checked_mul(rhs.value)?,
            sign: self.sign == rhs.sign,
        })
    }
}

#[test]
fn test_add() {
    let a = SignedDecimal256::from_i128(100);
    let b = SignedDecimal256::from_i128(100);
    let c = SignedDecimal256::from_i128(200);
    assert_eq!(a + b, c);

    let a = SignedDecimal256::from_i128(-100);
    let b = SignedDecimal256::from_i128(200);
    let c = SignedDecimal256::from_i128(100);
    assert_eq!(a + b, c);

    let a = SignedDecimal256::from_i128(-100);
    let b = SignedDecimal256::from_i128(100);
    let c = SignedDecimal256::zero();
    assert_eq!(a + b, c);

    let a = SignedDecimal256::from_i128(200);
    let b = SignedDecimal256::from_i128(-200);
    let c = SignedDecimal256::zero();
    assert_eq!(a + b, c);

    let a = SignedDecimal256::from_i128(500);
    let b = SignedDecimal256::from_i128(-200);
    let c = SignedDecimal256::from_i128(300);
    assert_eq!(a + b, c);

    let a = SignedDecimal256::from_i128(-500);
    let b = SignedDecimal256::from_i128(-500);
    let c = SignedDecimal256::from_i128(-1000);
    assert_eq!(a + b, c);
}

#[test]
fn test_sub() {
    let a = SignedDecimal256::from_i128(100);
    let b = SignedDecimal256::from_i128(100);
    let c = SignedDecimal256::zero();
    assert_eq!(a - b, c);

    let a = SignedDecimal256::from_i128(-100);
    let b = SignedDecimal256::from_i128(200);
    let c = SignedDecimal256::from_i128(-300);
    assert_eq!(a - b, c);

    let a = SignedDecimal256::from_i128(-100);
    let b = SignedDecimal256::from_i128(100);
    let c = SignedDecimal256::from_i128(-200);
    assert_eq!(a - b, c);

    let a = SignedDecimal256::from_i128(200);
    let b = SignedDecimal256::from_i128(-200);
    let c = SignedDecimal256::from_i128(400);
    assert_eq!(a - b, c);

    let a = SignedDecimal256::from_i128(500);
    let b = SignedDecimal256::from_i128(-200);
    let c = SignedDecimal256::from_i128(700);
    assert_eq!(a - b, c);

    let a = SignedDecimal256::from_i128(-500);
    let b = SignedDecimal256::from_i128(-500);
    let c = SignedDecimal256::zero();
    assert_eq!(a - b, c);
}

#[test]
fn test_div() {
    let a = SignedDecimal256::from_i128(100);
    let b = SignedDecimal256::from_i128(100);
    let c = SignedDecimal256::from_i128(1);
    assert_eq!(a / b, c);

    let a = SignedDecimal256::from_i128(-100);
    let b = SignedDecimal256::from_i128(200);
    let c = SignedDecimal256::from_str("0.5", false).unwrap();
    assert_eq!(a / b, c);

    let a = SignedDecimal256::from_i128(-100);
    let b = SignedDecimal256::from_i128(100);
    let c = SignedDecimal256::from_i128(-1);
    assert_eq!(a / b, c);

    let a = SignedDecimal256::from_i128(200);
    let b = SignedDecimal256::from_i128(-200);
    let c = SignedDecimal256::from_i128(-1);
    assert_eq!(a / b, c);

    let a = SignedDecimal256::from_i128(500);
    let b = SignedDecimal256::from_i128(-200);
    let c = SignedDecimal256::from_str("2.5", false).unwrap();
    assert_eq!(a / b, c);

    let a = SignedDecimal256::from_i128(-500);
    let b = SignedDecimal256::from_i128(-500);
    let c = SignedDecimal256::from_i128(1);
    assert_eq!(a / b, c);
}

#[test]
fn test_mul() {
    let a = SignedDecimal256::from_i128(100);
    let b = SignedDecimal256::from_i128(100);
    let c = SignedDecimal256::from_i128(10000);
    assert_eq!(a * b, c);

    let a = SignedDecimal256::from_i128(-100);
    let b = SignedDecimal256::from_i128(200);
    let c = SignedDecimal256::from_i128(-20000);
    assert_eq!(a * b, c);

    let a = SignedDecimal256::from_i128(-100);
    let b = SignedDecimal256::from_i128(100);
    let c = SignedDecimal256::from_i128(-10000);
    assert_eq!(a * b, c);

    let a = SignedDecimal256::from_i128(200);
    let b = SignedDecimal256::from_i128(-200);
    let c = SignedDecimal256::from_i128(-40000);
    assert_eq!(a * b, c);

    let a = SignedDecimal256::from_i128(500);
    let b = SignedDecimal256::from_i128(-200);
    let c = SignedDecimal256::from_i128(-100000);
    assert_eq!(a * b, c);

    let a = SignedDecimal256::from_i128(-500);
    let b = SignedDecimal256::from_i128(-500);
    let c = SignedDecimal256::from_i128(250000);
    assert_eq!(a * b, c);
}

#[test]
fn test_checked_add() {
    let a = SignedDecimal256::from_i128(100);
    let b = SignedDecimal256::from_i128(100);
    let c = SignedDecimal256::from_i128(200);
    assert_eq!(a.checked_add(b), Ok(c));

    let a = SignedDecimal256::from_i128(-100);
    let b = SignedDecimal256::from_i128(200);
    let c = SignedDecimal256::from_i128(100);
    assert_eq!(a.checked_add(b), Ok(c));

    let a = SignedDecimal256::from_i128(-100);
    let b = SignedDecimal256::from_i128(100);
    let c = SignedDecimal256::zero();
    assert_eq!(a.checked_add(b), Ok(c));

    let a = SignedDecimal256::from_i128(200);
    let b = SignedDecimal256::from_i128(-200);
    let c = SignedDecimal256::zero();
    assert_eq!(a.checked_add(b), Ok(c));

    let a = SignedDecimal256::from_i128(500);
    let b = SignedDecimal256::from_i128(-200);
    let c = SignedDecimal256::from_i128(300);
    assert_eq!(a.checked_add(b), Ok(c));

    let a = SignedDecimal256::from_i128(-500);
    let b = SignedDecimal256::from_i128(-500);
    let c = SignedDecimal256::from_i128(-1000);
    assert_eq!(a.checked_add(b), Ok(c));
}

#[test]
fn test_checked_sub() {
    let a = SignedDecimal256::from_i128(100);
    let b = SignedDecimal256::from_i128(100);
    let c = SignedDecimal256::zero();
    let res = a.checked_sub(b);
    assert_eq!(res, Ok(c));

    let a = SignedDecimal256::from_i128(-100);
    let b = SignedDecimal256::from_i128(200);
    let c = SignedDecimal256::from_i128(-300);
    let res = a.checked_sub(b).unwrap();
    assert_eq!(res, c);

    let a = SignedDecimal256::from_i128(-100);
    let b = SignedDecimal256::from_i128(100);
    let c = SignedDecimal256::from_i128(-200);
    assert_eq!(a.checked_sub(b), Ok(c));

    let a = SignedDecimal256::from_i128(200);
    let b = SignedDecimal256::from_i128(-200);
    let c = SignedDecimal256::from_i128(400);
    assert_eq!(a.checked_sub(b), Ok(c));

    let a = SignedDecimal256::from_i128(500);
    let b = SignedDecimal256::from_i128(-200);
    let c = SignedDecimal256::from_i128(700);
    assert_eq!(a.checked_sub(b), Ok(c));

    let a = SignedDecimal256::from_i128(-500);
    let b = SignedDecimal256::from_i128(-500);
    let c = SignedDecimal256::zero();
    assert_eq!(a.checked_sub(b), Ok(c));
}

#[test]
fn test_checked_mul() {
    let a = SignedDecimal256::from_i128(100);
    let b = SignedDecimal256::from_i128(100);
    let c = SignedDecimal256::from_i128(10000);
    assert_eq!(a.checked_mul(b), Ok(c));

    let a = SignedDecimal256::from_i128(-100);
    let b = SignedDecimal256::from_i128(200);
    let c = SignedDecimal256::from_i128(-20000);
    assert_eq!(a.checked_mul(b), Ok(c));

    let a = SignedDecimal256::from_i128(-100);
    let b = SignedDecimal256::from_i128(100);
    let c = SignedDecimal256::from_i128(-10000);
    assert_eq!(a.checked_mul(b), Ok(c));

    let a = SignedDecimal256::from_i128(200);
    let b = SignedDecimal256::from_i128(-200);
    let c = SignedDecimal256::from_i128(-40000);
    assert_eq!(a.checked_mul(b), Ok(c));

    let a = SignedDecimal256::from_i128(500);
    let b = SignedDecimal256::from_i128(-200);
    let c = SignedDecimal256::from_i128(-100000);
    assert_eq!(a.checked_mul(b), Ok(c));

    let a = SignedDecimal256::from_i128(-500);
    let b = SignedDecimal256::from_i128(-500);
    let c = SignedDecimal256::from_i128(250000);
    assert_eq!(a.checked_mul(b), Ok(c));
}

#[test]
fn test_checked_div() {
    let a = SignedDecimal256::from_i128(100);
    let b = SignedDecimal256::from_i128(100);
    let c = SignedDecimal256::from_i128(1);
    assert_eq!(a.checked_div(b), Ok(c));

    let a = SignedDecimal256::from_i128(-100);
    let b = SignedDecimal256::from_i128(200);
    let c = SignedDecimal256::from_str("0.5", false).unwrap();
    assert_eq!(a.checked_div(b), Ok(c));

    let a = SignedDecimal256::from_i128(-100);
    let b = SignedDecimal256::from_i128(100);
    let c = SignedDecimal256::from_i128(-1);
    assert_eq!(a.checked_div(b), Ok(c));

    let a = SignedDecimal256::from_i128(200);
    let b = SignedDecimal256::from_i128(-200);
    let c = SignedDecimal256::from_i128(-1);
    assert_eq!(a.checked_div(b), Ok(c));

    let a = SignedDecimal256::from_i128(500);
    let b = SignedDecimal256::from_i128(-200);
    let c = SignedDecimal256::from_str("2.5", false).unwrap();
    assert_eq!(a.checked_div(b), Ok(c));

    let a = SignedDecimal256::from_i128(-500);
    let b = SignedDecimal256::from_i128(-500);
    let c = SignedDecimal256::from_i128(1);
    assert_eq!(a.checked_div(b), Ok(c));
}
