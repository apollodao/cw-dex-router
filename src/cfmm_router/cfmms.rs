use cosmwasm_schema::cw_serde;
use cosmwasm_std::{StdError, StdResult};

use super::math::DecimalMath;

pub trait CFMM<T> {
    fn find_arb(&self, market_prices: &[T]) -> StdResult<(Vec<T>, Vec<T>)>;

    fn get_token_ids(&self) -> Vec<usize>;

    fn get_reserves(&self) -> Vec<T>;

    fn set_reserves(&mut self, reserves: &[T]);
}

#[cw_serde]
pub struct ProductTwoCoin<T> {
    // R::Vector{T}
    pub reserves: Vec<T>,
    // γ::T
    pub fee: T,
    // Ai::Vector{Int}                 # idx vector: jth coin in CFMM is Ai[j]
    pub token_ids: Vec<usize>,
}

impl<T> ProductTwoCoin<T>
where
    T: DecimalMath,
{
    pub fn new(reserves: Vec<T>, fee: T, idxs: Vec<usize>) -> StdResult<Self> {
        if reserves.len() != 2 || idxs.len() != 2 || idxs[0] == idxs[1] {
            return Err(StdError::generic_err("Invalid ProductTwoCoin"));
        }
        Ok(Self {
            reserves,
            fee,
            token_ids: idxs,
        })
    }

    // # See App. A of "An Analysis of Uniswap Markets"
    // @inline prod_arb_δ(m, r, k, γ) = max(sqrt(γ*m*k) - r, 0)/γ
    fn prod_arb_delta(market_price: T, reserve: T, constant_product: T, fee: T) -> StdResult<T> {
        let sqrt = (market_price
            .checked_mul(constant_product)?
            .checked_mul(fee)?)
        .sqrt();
        if reserve > sqrt {
            Ok(reserve.checked_sub(sqrt)?.checked_div(fee)?)
        } else {
            Ok(T::zero())
        }
    }

    // @inline prod_arb_λ(m, r, k, γ) = max(r - sqrt(k/(m*γ)), 0)
    fn prod_arb_lambda(market_price: T, reserve: T, constant_product: T, fee: T) -> StdResult<T> {
        let sqrt = (constant_product
            .checked_div(market_price)?
            .checked_div(fee)?)
        .sqrt();
        if reserve > sqrt {
            Ok(reserve.checked_sub(sqrt)?)
        } else {
            Ok(T::zero())
        }
    }
}

impl<T> CFMM<T> for ProductTwoCoin<T>
where
    T: DecimalMath,
{
    fn get_token_ids(&self) -> Vec<usize> {
        self.token_ids.clone()
    }

    // # Solves the maximum arbitrage problem for the two-coin constant product case.
    // # Assumes that v > 0 and γ > 0.
    // function find_arb!(Δ::VT, Λ::VT, cfmm::ProductTwoCoin{T}, v::VT) where {T, VT<:AbstractVector{T}}
    //     R, γ = cfmm.R, cfmm.γ
    //     k = R[1]*R[2]

    //     Δ[1] = prod_arb_δ(v[2]/v[1], R[1], k, γ)
    //     Δ[2] = prod_arb_δ(v[1]/v[2], R[2], k, γ)

    //     Λ[1] = prod_arb_λ(v[1]/v[2], R[1], k, γ)
    //     Λ[2] = prod_arb_λ(v[2]/v[1], R[2], k, γ)
    //     return nothing
    // end
    fn find_arb(&self, market_prices: &[T]) -> StdResult<(Vec<T>, Vec<T>)> {
        let constant_product = self.reserves[0].checked_mul(self.reserves[1])?;
        let delta = vec![
            Self::prod_arb_delta(
                market_prices[1].checked_div(market_prices[0])?,
                self.reserves[0],
                constant_product,
                self.fee,
            )?,
            Self::prod_arb_delta(
                market_prices[0].checked_div(market_prices[1])?,
                self.reserves[1],
                constant_product,
                self.fee,
            )?,
        ];
        let lambda = vec![
            Self::prod_arb_lambda(
                market_prices[0].checked_div(market_prices[1])?,
                self.reserves[0],
                constant_product,
                self.fee,
            )?,
            Self::prod_arb_lambda(
                market_prices[1].checked_div(market_prices[0])?,
                self.reserves[1],
                constant_product,
                self.fee,
            )?,
        ];
        Ok((delta, lambda))
    }

    fn get_reserves(&self) -> Vec<T> {
        self.reserves.clone()
    }

    fn set_reserves(&mut self, reserves: &[T]) {
        self.reserves = reserves.to_vec();
    }
}
