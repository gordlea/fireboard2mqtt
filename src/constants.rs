use constcat::concat;

pub const ONLINE: &str = "online";
pub const OFFLINE: &str = "offline";
pub const ON: &str = "on";
pub const OFF: &str = "off";

pub const FIREBOARD_DEVICELOG_UPDATE_INTERVAL_MINUTES: i64 = 5;

const CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const USER_AGENT: &str = concat!("fireboard2mqtt/", CRATE_VERSION);
