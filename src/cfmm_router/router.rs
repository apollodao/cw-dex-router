use std::{fmt, str::FromStr};

use anyhow::Error;
use cosmwasm_std::StdResult;

use crate::ContractError;

use super::{cfmms::CFMM, math::DecimalMath, objectives::Objective};
use liblbfgs::{default_progress, lbfgs, vector_math::VectorMath};

// struct Router{O,T}
//     objective::O
//     cfmms::Vector{CFMM{T}}
//     Δs::Vector{AbstractVector{T}}
//     Λs::Vector{AbstractVector{T}}
//     v::Vector{T}
// end

// """
//     Router(objective, cfmms, n_tokens)

// Constructs a router that finds a set of trades `(router.Δs, router.Λs)` through `cfmms`
// which maximizes `objective`. The number of tokens `n_tokens` must be specified.
// """
// function Router(objective::O, cfmms::Vector{C}, n_tokens) where {T,O<:Objective,C<:CFMM{T}}
//     V = Vector{T}
//     VT = Vector{Vector{typeof(objective).parameters[1]}}
//     Δs = VT()
//     Λs = VT()

//     for c in cfmms
//         push!(Δs, zero(c.R))
//         push!(Λs, zero(c.R))
//     end

//     return Router{O,T}(
//         objective,
//         cfmms,
//         Δs,
//         Λs,
//         zeros(T, n_tokens)
//     )
// end
// Router(objective, n_tokens) = Router(objective, Vector{CFMM{Float64}}(), n_tokens)

// function find_arb!(r::Router, v)
//     Threads.@threads for i in 1:length(r.Δs)
//         find_arb!(r.Δs[i], r.Λs[i], r.cfmms[i], v[r.cfmms[i].Ai])
//     end
// end

// @doc raw"""
//     route!(r::Router)

// Solves the routing problem,
// ```math
// \begin{array}{ll}
// \text{maximize}     & U(\Psi) \\
// \text{subject to}   & \Psi = \sum_{i=1}^m A_i(\Lambda_i - \Delta_i) \\
// & \phi_i(R_i + \gamma_i\Delta_i - \Lambda_i) \geq \phi_i(R_i), \quad i = 1, \dots, m \\
// &\Delta_i \geq 0, \quad \Lambda_i \geq 0, \quad i = 1, \dots, m.
// \end{array}
// ```
// Overwrites `r.Δs` and `r.Λs`.
// """
// function route!(r::R; v=nothing, verbose=false, m=5) where {R<:Router}
//     # Optimizer set up
//     optimizer = L_BFGS_B(length(r.v), 17)
//     if isnothing(v)
//         r.v .= ones(length(r.v)) / length(r.v) # We should use the initial marginal price here
//     else
//         r.v .= v
//     end

//     bounds = zeros(3, length(r.v))
//     bounds[1, :] .= 2
//     bounds[2, :] .= lower_limit(r.objective)
//     bounds[3, :] .= upper_limit(r.objective)

//     # Objective function
//     function fn(v)
//         if !all(v .== r.v)
//             find_arb!(r, v)
//             r.v .= v
//         end

//         acc = 0.0

//         for (Δ, Λ, c) in zip(r.Δs, r.Λs, r.cfmms)
//             acc += @views dot(Λ, v[c.Ai]) - dot(Δ, v[c.Ai])
//         end

//         return f(r.objective, v) + acc
//     end

//     # Derivative of objective function
//     function g!(G, v)
//         G .= 0

//         if !all(v .== r.v)
//             find_arb!(r, v)
//             r.v .= v
//         end
//         grad!(G, r.objective, v)

//         for (Δ, Λ, c) in zip(r.Δs, r.Λs, r.cfmms)
//             @views G[c.Ai] .+= Λ .- Δ
//         end

//     end

//     find_arb!(r, r.v)
//     _, v = optimizer(fn, g!, r.v, bounds, m=m, factr=1e1, pgtol=1e-5, iprint=verbose ? 1 : -1, maxfun=15000, maxiter=15000)
//     r.v .= v
//     find_arb!(r, v)
// end

// # ----- Convenience functions
// function netflows!(ψ, r::Router)
//     fill!(ψ, 0)

//     for (Δ, Λ, c) in zip(r.Δs, r.Λs, r.cfmms)
//         ψ[c.Ai] += Λ - Δ
//     end

//     return nothing
// end

// function netflows(r::Router)
//     ψ = zero(r.v)
//     netflows!(ψ, r)
//     return ψ
// end

// function update_reserves!(r::Router)
//     for (Δ, Λ, c) in zip(r.Δs, r.Λs, r.cfmms)
//         c.R .+= Δ - Λ
//     end

//     return nothing
// end

pub struct Router<T, O, C> {
    pub cfmms: Vec<C>,
    pub objective: O,
    pub market_prices: Vec<T>,
    pub token_sales: Vec<Vec<T>>,
    pub token_buys: Vec<Vec<T>>,
}

impl<T, O, C> Router<T, O, C>
where
    T: DecimalMath,
    O: Objective<T>,
    C: CFMM<T>,
    [T]: VectorMath<T>,
    <T as FromStr>::Err: fmt::Debug,
{
    pub fn new(objective: O, cfmms: Vec<C>, n_tokens: usize) -> Self {
        println!("cfmms length: {}", cfmms.len());
        let token_sales = vec![vec![T::zero(); cfmms.len()]; cfmms.len()];
        let token_buys = vec![vec![T::zero(); cfmms.len()]; cfmms.len()];
        let initial_guess = vec![T::zero(); n_tokens];

        Self {
            cfmms,
            objective,
            market_prices: initial_guess,
            token_sales,
            token_buys,
        }
    }

    pub fn find_arb(&mut self, v: &[T]) -> StdResult<()> {
        for (i, cfmm) in self.cfmms.iter().enumerate() {
            let (token_sale, token_buy) = cfmm.find_arb(&v)?;
            self.token_sales[i] = token_sale;
            self.token_buys[i] = token_buy;
        }
        Ok(())
    }

    pub fn route(&mut self) -> Result<(), ContractError> {
        // let mut optimizer = L_BFGS_B::new(self.market_prices.len(), 17);

        // We should use the initial marginal price here
        self.market_prices =
            vec![T::one().checked_div(self.market_prices.len())?; self.market_prices.len()];

        let mut temp_market_prices = self.market_prices.clone();
        self.find_arb(&temp_market_prices)?;

        // Objective function
        let f = |x: &[T], y: &mut [T]| {
            for i in 0..x.len() {
                println!("x[{}] = {}", i, x[i]);
            }
            for i in 0..y.len() {
                println!("y[{}] = {}", i, y[i]);
            }

            self.find_arb(x)?;

            let mut acc = T::zero();

            self.objective.grad(y, &self.market_prices);

            for (i, cfmm) in self.cfmms.iter().enumerate() {
                let token_sale = &self.token_sales[i];
                let token_buy = &self.token_buys[i];

                let tokens = self
                    .market_prices
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| cfmm.get_token_ids().contains(i))
                    .map(|(_, x)| x.clone())
                    .collect::<Vec<T>>();
                acc = acc.checked_add(
                    token_sale
                        .vecdot(&tokens)
                        .checked_sub(token_buy.vecdot(&tokens))?,
                )?;
                println!("acc: {}", acc);

                for j in cfmm.get_token_ids() {
                    println!("y[j]: {}", y[j]);
                    y[j] = y[j].checked_add(token_sale[j].checked_sub(token_buy[j])?)?;
                }
            }

            let out = self.objective.f(x) + acc;
            Ok::<T, Error>(out)
        };

        for i in 0..temp_market_prices.len() {
            println!("market_prices[{}]: {}", i, temp_market_prices[i]);
        }

        let _report = lbfgs().minimize(&mut temp_market_prices, f, default_progress())?;

        self.market_prices = temp_market_prices.clone();

        self.find_arb(&temp_market_prices)?;

        Ok(())
    }

    pub fn net_flows(&self) -> StdResult<Vec<T>> {
        let mut netflows = vec![T::zero(); self.market_prices.len()];
        for (i, cfmm) in self.cfmms.iter().enumerate() {
            let token_sale = &self.token_sales[i];
            let token_buy = &self.token_buys[i];

            for j in cfmm.get_token_ids() {
                netflows[j] = netflows[j].checked_add(token_sale[j].checked_sub(token_buy[j])?)?;
            }
        }
        Ok(netflows)
    }

    pub fn update_reserves(&mut self) -> StdResult<()> {
        for (i, cfmm) in self.cfmms.iter_mut().enumerate() {
            let token_sale = &self.token_sales[i];
            let token_buy = &self.token_buys[i];

            let mut reserves = cfmm.get_reserves().clone();

            for j in cfmm.get_token_ids() {
                reserves[j] = reserves[j].checked_add(token_sale[j].checked_sub(token_buy[j])?)?;
            }

            cfmm.set_reserves(&reserves);
        }
        Ok(())
    }
}
