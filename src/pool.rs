use std::ops::Deref;

use cosmwasm_schema::cw_serde;
use cw_dex::traits::Pool as PoolTrait;

#[cw_serde]
pub enum Pool {
    #[cfg(feature = "osmosis")]
    Osmosis(cw_dex_osmosis::OsmosisPool),
    #[cfg(feature = "astroport")]
    Astroport(cw_dex_astroport::AstroportPool),
}

impl Deref for Pool {
    type Target = dyn PoolTrait;

    fn deref(&self) -> &Self::Target {
        #[allow(unreachable_patterns)]
        match self {
            #[cfg(feature = "osmosis")]
            Pool::Osmosis(pool) => pool as &dyn PoolTrait,
            #[cfg(feature = "astroport")]
            Pool::Astroport(pool) => pool as &dyn PoolTrait,
            _ => panic!("No pool feature enabled"),
        }
    }
}
