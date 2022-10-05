use cosmwasm_schema::cw_serde;
use cw_asset::AssetInfo;
use cw_controllers::Admin;
use cw_storage_plus::{Key, Map, PrimaryKey};

use crate::operations::SwapOperationsList;

/// As an MVP we hardcode paths for each tuple of assets (offer, ask).
/// In a future version we want to find the path that produces the highest number
/// of ask assets, but this will take some time to implement.
pub const PATHS: Map<AssetInfoPair, SwapOperationsList> = Map::new("paths");
pub const ADMIN: Admin = Admin::new("admin");

#[cw_serde]
pub struct AssetInfoPair {
    pub from: AssetInfo,
    pub to: AssetInfo,
    str_key: String,
}

impl AssetInfoPair {
    fn new(from: AssetInfo, to: AssetInfo) -> AssetInfoPair {
        AssetInfoPair {
            str_key: format!("{} -> {}", from, to),
            from,
            to,
        }
    }
}

impl From<(AssetInfo, AssetInfo)> for AssetInfoPair {
    fn from(tuple: (AssetInfo, AssetInfo)) -> Self {
        Self::new(tuple.0, tuple.1)
    }
}

impl<'a> PrimaryKey<'a> for AssetInfoPair {
    type Prefix = ();
    type SubPrefix = ();
    type Suffix = ();
    type SuperSuffix = ();

    fn key(&self) -> Vec<Key> {
        self.str_key.key()
    }
}
