// @doc raw"""
//     BasketLiquidation(i, Δin)

// Liquidation objective for the routing problem,
// ```math
//     \Psi_i - \mathbf{I}(\Psi_{-i} + Δ^\mathrm{in}_{-i} = 0, ~ \Psi_i \geq 0),
// ```
// where `i` is the desired output token and `Δin` is the basket of tokens to be liquidated.
// """
// struct BasketLiquidation{T} <: Objective
//     i::Int
//     Δin::Vector{T}

//     function BasketLiquidation(i::Integer, Δin::Vector{T}) where {T<:AbstractFloat}
//         !(i > 0 && i <= length(Δin)) && throw(ArgumentError("Invalid index i"))
//         return new{T}(
//             i,
//             Δin,
//         )
//     end
// end
// BasketLiquidation(i::Integer, Δin::Vector{T}) where {T<:Real} = BasketLiquidation(i, Float64.(Δin))

// function f(obj::BasketLiquidation{T}, v) where {T}
//     if v[obj.i] >= 1.0
//         return sum(i -> (i == obj.i ? 0.0 : obj.Δin[i] * v[i]), 1:length(v))
//     end
//     return convert(T, Inf)
// end

// function grad!(g, obj::BasketLiquidation{T}, v) where {T}
//     if v[obj.i] >= 1.0
//         g .= obj.Δin
//         g[obj.i] = zero(T)
//     else
//         g .= convert(T, Inf)
//     end
//     return nothing
// end

// @inline function lower_limit(o::BasketLiquidation{T}) where {T}
//     ret = Vector{T}(undef, length(o.Δin))
//     fill!(ret, eps())
//     ret[o.i] = one(T) + eps()
//     return ret
// end
// @inline upper_limit(o::BasketLiquidation{T}) where {T} = convert(T, Inf) .+ zero(o.Δin)

use std::ops::Sub;

use super::math::DecimalMath;

#[cfg(test)]
use super::{cfmms::ProductTwoCoin, router::Router, signed_decimal256::SignedDecimal256};
use liblbfgs::{
    decimal::Decimal as FixedDecimal,
    decimal_math::{One, Zero},
};

pub trait Objective<T> {
    fn f(&self, v: &[T]) -> T;
    fn grad(&self, g: &mut [T], v: &[T]);
    fn lower_limit(&self) -> Vec<T>;
    fn upper_limit(&self) -> Vec<T>;
}

pub struct BasketLiquidation<T> {
    //The ID of the desired output token
    output_token_id: usize,
    //The basket of tokens to be liquidated
    delta_in: Vec<T>,
}

impl<T> BasketLiquidation<T>
where
    T: DecimalMath,
{
    pub fn new(output_token_id: usize, delta_in: Vec<T>) -> Self {
        Self {
            output_token_id,
            delta_in,
        }
    }
}

impl<T> Objective<T> for BasketLiquidation<T>
where
    T: DecimalMath,
{
    fn f(&self, v: &[T]) -> T {
        if v[self.output_token_id] >= T::one() {
            self.delta_in
                .iter()
                .enumerate()
                .map(|(i, delta)| {
                    if i == self.output_token_id {
                        T::zero()
                    } else {
                        *delta * v[i] //TODO: Make checked mul?
                    }
                })
                .sum()
        } else {
            T::infinity()
        }
    }

    fn grad(&self, g: &mut [T], v: &[T]) {
        if v[self.output_token_id] >= T::one() {
            for (i, delta) in self.delta_in.iter().enumerate() {
                g[i] = delta.clone();
            }
            g[self.output_token_id] = T::zero();
        } else {
            for i in 0..self.delta_in.len() {
                g[i] = T::infinity();
            }
        }
    }

    fn lower_limit(&self) -> Vec<T> {
        let mut ret = vec![T::zero(); self.delta_in.len()];
        ret[self.output_token_id] = T::one() + T::eps();
        ret
    }

    fn upper_limit(&self) -> Vec<T> {
        vec![T::infinity(); self.delta_in.len()]
    }
}

#[test]
fn test_basket_liquidate_signed_decimal256() {
    // ## Create CFMMs
    // cfmms = [
    //     ProductTwoCoin([1e3, 1e4], 0.997, [1, 2]),
    //     ProductTwoCoin([1e3, 1e2], 0.997, [2, 3]),
    //     ProductTwoCoin([1e3, 2e4], 0.997, [1, 3])
    // ]

    // ## We want to liquidate a basket of tokens 2 & 3 into token 1
    // Δin = [0, 1e1, 1e2]

    // ## Build a routing problem with liquidation objective
    // router = Router(
    //     BasketLiquidation(1, Δin),
    //     cfmms,
    //     maximum([maximum(cfmm.Ai) for cfmm in cfmms]),
    // )

    // ## Optimize!
    // route!(router)

    // ## Print results
    // Ψ = round.(Int, netflows(router))
    // println("Input Basket: $(round.(Int, Δin))")
    // println("Net trade: $Ψ")
    // println("Amount recieved: $(Ψ[1])")

    let cfmms = vec![
        ProductTwoCoin::<SignedDecimal256>::new(
            vec![1000.into(), 10000.into()],
            "0.997".into(),
            vec![1, 2],
        )
        .unwrap(),
        ProductTwoCoin::<SignedDecimal256>::new(
            vec![1000.into(), 100.into()],
            "0.997".into(),
            vec![2, 3],
        )
        .unwrap(),
        ProductTwoCoin::<SignedDecimal256>::new(
            vec![1000.into(), 20000.into()],
            "0.997".into(),
            vec![1, 3],
        )
        .unwrap(),
    ];

    let delta_in: Vec<SignedDecimal256> = vec![0.into(), 10.into(), 100.into()];

    let mut router = Router::<
        SignedDecimal256,
        BasketLiquidation<SignedDecimal256>,
        ProductTwoCoin<SignedDecimal256>,
    >::new(BasketLiquidation::new(1, delta_in.clone()), cfmms, 3);

    router.route().unwrap();

    let net_flows = router.net_flows().unwrap();
    println!("Input Basket: {:?}", delta_in);
    println!("Net trade: {:?}", net_flows);
    println!("Amount recieved: {:?}", net_flows[1]);
}

#[test]
fn test_basket_liquidate() {
    let cfmms = vec![
        ProductTwoCoin::<FixedDecimal>::new(
            vec![1000.into(), 10000.into()],
            FixedDecimal::from_str("0.997").unwrap(),
            vec![0, 1],
        )
        .unwrap(),
        ProductTwoCoin::<FixedDecimal>::new(
            vec![1000.into(), 100.into()],
            FixedDecimal::from_str("0.997").unwrap(),
            vec![1, 2],
        )
        .unwrap(),
        ProductTwoCoin::<FixedDecimal>::new(
            vec![1000.into(), 20000.into()],
            FixedDecimal::from_str("0.997").unwrap(),
            vec![0, 2],
        )
        .unwrap(),
    ];

    let delta_in: Vec<FixedDecimal> = vec![0.into(), 10.into(), 100.into()];

    let mut router = Router::<
        FixedDecimal,
        BasketLiquidation<FixedDecimal>,
        ProductTwoCoin<FixedDecimal>,
    >::new(BasketLiquidation::new(0, delta_in.clone()), cfmms, 3);

    router.route().unwrap();

    let net_flows = router.net_flows().unwrap();
    println!("Input Basket: {:?}", delta_in);
    println!("Net trade: {:?}", net_flows);
    println!("Amount recieved: {:?}", net_flows[1]);
}

#[test]
fn test_lbfgs() {
    use liblbfgs::{default_evaluate, default_progress, lbfgs};

    fn assert_relative_eq(
        first: SignedDecimal256,
        second: SignedDecimal256,
        max_relative: SignedDecimal256,
    ) {
        let diff = if first > second {
            first.sub(second).abs()
        } else {
            second.sub(first).abs()
        };

        if diff > max_relative {
            panic!(
                "assert_relative_eq failed. first: {}, second: {}, max_relative: {}",
                first, second, max_relative
            );
        }
    }

    const N: usize = 100;

    // Initialize the variables
    let mut x = [SignedDecimal256::zero(); N];
    for i in (0..N).step_by(2) {
        x[i] = SignedDecimal256::from_str("1.2", false).unwrap();
        x[i + 1] = SignedDecimal256::one();
    }

    let prb = lbfgs()
        .minimize(&mut x, default_evaluate(), default_progress())
        .expect("lbfgs minimize");

    // Iteration 37:
    // fx = 0.0000000000000012832127771605377, x[0] = 0.9999999960382451, x[1] = 0.9999999917607568
    // xnorm = 9.999999938995018, gnorm = 0.0000009486547293218877, step = 1

    // let epsilon = SignedDecimal256::from_scientific("1e-4").unwrap();
    let epsilon = SignedDecimal256::from_str("0.0001", true).unwrap();

    assert_relative_eq(SignedDecimal256::zero(), prb.fx, epsilon);
    // assert_eq!(SignedDecimal256::ZERO, prb.fx);

    println!("First assert passed!");

    for i in 0..N {
        assert_relative_eq(SignedDecimal256::one(), x[i], epsilon);
    }
    // for i in 0..N {
    //     assert_eq!(SignedDecimal256::ONE, x[i]);
    // }

    println!("First assert passed!");

    // OWL-QN
    let prb = lbfgs()
        .with_orthantwise(SignedDecimal256::one(), 0, 99)
        .minimize(&mut x, default_evaluate(), default_progress())
        .expect("lbfgs owlqn minimize");

    // Iteration 171:
    // fx = 43.50249999999999, x[0] = 0.2500000069348678, x[1] = 0.057500004213084016
    // xnorm = 1.8806931246657475, gnorm = 0.00000112236896804755, step = 1

    // assert_relative_eq!(43.5025, prb.fx, epsilon = 1e-4);
    // assert_relative_eq!(0.2500, x[0], epsilon = 1e-4);
    // assert_relative_eq!(0.0575, x[1], epsilon = 1e-4);
    assert_relative_eq(
        SignedDecimal256::from_str("43.5025", true).unwrap(),
        prb.fx,
        epsilon,
    );
    assert_relative_eq(
        SignedDecimal256::from_str("0.2500", true).unwrap(),
        x[0],
        epsilon,
    );
    assert_relative_eq(
        SignedDecimal256::from_str("0.0575", true).unwrap(),
        x[1],
        epsilon,
    );
}
// rosenbrock:1 ends here
