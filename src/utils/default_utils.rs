pub const fn default_as_true() -> bool { true }

pub fn default_as_default() -> String { String::from("default") }

// Default delay values for resolving VOD or Series requests,
// used to prevent frequent requests that could trigger a provider ban.
pub const fn default_resolve_delay_secs() -> u16 { 2 }

// Default grace values to accommodate rapid channel changes and seek requests,
/// Returns the default grace period in milliseconds for handling rapid channel changes and seek requests.
///
/// This value helps prevent triggering strict maximum connection enforcement by allowing brief delays between requests.
///
/// # Examples
///
/// ```
/// let grace_period = default_grace_period_millis();
/// assert_eq!(grace_period, 500);
/// ```
pub const fn default_grace_period_millis() -> u64 { 500 }
/// Returns the default grace period timeout in seconds.
///
/// This value defines the maximum duration allowed for a grace period, typically used to accommodate rapid channel changes or seek requests without triggering connection enforcement.
///
/// # Examples
///
/// ```
/// let timeout = default_grace_period_timeout_secs();
/// assert_eq!(timeout, 10);
/// ```
pub const fn default_grace_period_timeout_secs() -> u64 { 10 }
/// Returns the default connection timeout in seconds, set to 6.
///
/// # Examples
///
/// ```
/// let timeout = default_connect_timeout_secs();
/// assert_eq!(timeout, 6);
/// ```
pub const fn default_connect_timeout_secs() -> u32 { 6 }