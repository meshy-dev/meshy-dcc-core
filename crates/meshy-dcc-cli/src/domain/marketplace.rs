//! Marketplace domain types re-exported from the shared `meshy-dcc-marketplace` crate.

pub use meshy_dcc_marketplace::{
    InstalledMarketplacePackage, MarketplaceHit, MarketplaceInspectResult,
    MarketplaceInstallResult, MarketplaceInstalledList, MarketplaceInstalledState,
    MarketplaceOutdatedList, MarketplaceSearchResult, MarketplaceSource, MarketplaceSourceConfig,
    MarketplaceSourceOrigin, MarketplaceUninstallResult, MarketplaceUpdateResult,
    OFFICIAL_MARKETPLACE_SOURCE, OutdatedMarketplacePackage, StoredMarketplaceSource,
    builtin_source, entry_targets_dcc, normalise_source,
};
