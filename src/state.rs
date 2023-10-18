use apollo_cw_asset::AssetInfoKey;
use cw_controllers::Admin;
use cw_storage_plus::Map;

use crate::operations::SwapOperationsList;

/// As an MVP we hardcode paths for each tuple of assets (offer, ask).
/// In a future version we want to find the path that produces the highest
/// number of ask assets, but this will take some time to implement.
/// To support multiple paths between the same asset, we add an id field we increment per asset of the same
/// path
pub const PATHS: Map<(AssetInfoKey, AssetInfoKey, u64), SwapOperationsList> = Map::new("paths");
pub const ADMIN: Admin = Admin::new("admin");
