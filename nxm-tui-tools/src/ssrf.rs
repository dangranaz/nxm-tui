//! SSRF protection for `web_fetch` — address classification and host validation.
//!
//! Blocks requests to non-routable/sensitive addresses (loopback, link-local,
//! private-use, metadata endpoint, unspecified) and non-HTTP(S) schemes.
//! Also validates redirect targets to prevent DNS rebinding.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};

use anyhow::{Context, Result};
use reqwest::Url;
use tracing::{info, warn};

use crate::TARGET;

/// Check if an IP address is blocked by SSRF policy.
///
/// Returns `true` for:
/// - Loopback (127.0.0.0/8, ::1)
/// - Link-local (169.254.0.0/16, fe80::/10)
/// - Private-use (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, fc00::/7)
/// - Metadata endpoint (169.254.169.254)
/// - Unspecified (0.0.0.0, ::)
///
/// ```
/// use std::net::IpAddr;
/// use std::str::FromStr;
/// use nxm_tui_tools::is_blocked_addr;
///
/// assert!(is_blocked_addr(IpAddr::from_str("127.0.0.1").unwrap()));
/// assert!(is_blocked_addr(IpAddr::from_str("10.0.0.1").unwrap()));
/// assert!(is_blocked_addr(IpAddr::from_str("192.168.1.1").unwrap()));
/// assert!(is_blocked_addr(IpAddr::from_str("169.254.169.254").unwrap()));
/// assert!(!is_blocked_addr(IpAddr::from_str("93.184.216.34").unwrap())); // example.org
/// ```
pub fn is_blocked_addr(ip: IpAddr) -> bool {
    tracing::debug!(target: crate::TARGET, "is_blocked_addr check ip={}", ip);
    let blocked = match ip {
        IpAddr::V4(ipv4) => is_blocked_ipv4(ipv4),
        IpAddr::V6(ipv6) => is_blocked_ipv6(ipv6),
    };
    if blocked {
        tracing::debug!(target: crate::TARGET, "is_blocked_addr: BLOCKED ip={}", ip);
    }
    blocked
}

fn is_blocked_ipv4(ip: Ipv4Addr) -> bool {
    // Loopback: 127.0.0.0/8
    if ip.is_loopback() {
        return true;
    }
    // Link-local: 169.254.0.0/16 (includes metadata endpoint 169.254.169.254)
    if ip.is_link_local() {
        return true;
    }
    // Private-use: 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
    if ip.is_private() {
        return true;
    }
    // Unspecified: 0.0.0.0
    if ip.is_unspecified() {
        return true;
    }
    false
}

fn is_blocked_ipv6(ip: Ipv6Addr) -> bool {
    // Native IPv6 loopback (::1) and unspecified (::) must be checked FIRST,
    // because to_ipv4() also matches the deprecated IPv4-compat form (::a.b.c.d)
    // which would incorrectly map ::1 to 0.0.0.1 (not loopback in IPv4).
    if ip.is_loopback() {
        return true;
    }
    if ip.is_unspecified() {
        return true;
    }

    // Handle IPv4-mapped addresses (::ffff:a.b.c.d) only.
    // These are the only ones that should be classified via IPv4 logic.
    // Check for the mapped prefix (first 80 bits zero, next 16 bits 0xffff).
    let segments = ip.segments();
    if segments[0] == 0
        && segments[1] == 0
        && segments[2] == 0
        && segments[3] == 0
        && segments[4] == 0
        && segments[5] == 0xffff
    {
        // Extract the embedded IPv4 address from the last 32 bits
        let v4 = Ipv4Addr::new(
            (segments[6] >> 8) as u8,
            (segments[6] & 0xff) as u8,
            (segments[7] >> 8) as u8,
            (segments[7] & 0xff) as u8,
        );
        return is_blocked_ipv4(v4);
    }

    // Link-local: fe80::/10
    if ip.is_unicast_link_local() {
        return true;
    }
    // ULA (private): fc00::/7
    if ip.segments()[0] & 0xfe00 == 0xfc00 {
        return true;
    }
    false
}

/// Check if a URL scheme is allowed (only http/https).
///
/// ```
/// use reqwest::Url;
/// use nxm_tui_tools::is_allowed_scheme;
///
/// assert!(is_allowed_scheme(&Url::parse("http://example.com").unwrap()));
/// assert!(is_allowed_scheme(&Url::parse("https://example.com").unwrap()));
/// assert!(!is_allowed_scheme(&Url::parse("ftp://example.com").unwrap()));
/// assert!(!is_allowed_scheme(&Url::parse("file:///etc/passwd").unwrap()));
/// ```
pub fn is_allowed_scheme(url: &Url) -> bool {
    let allowed = matches!(url.scheme(), "http" | "https");
    tracing::debug!(target: crate::TARGET, "is_allowed_scheme scheme={} allowed={}", url.scheme(), allowed);
    allowed
}

/// Resolve a hostname to IP addresses and check if any is blocked.
///
/// Returns `Ok(())` if all resolved addresses are allowed.
/// Returns `Err` if resolution fails or any address is blocked.
///
/// ```
/// use nxm_tui_tools::validate_host;
///
/// // Public host should pass (requires network DNS)
/// // validate_host("example.com").expect("public host allowed");
/// // Private host should fail
/// // validate_host("localhost").expect_err("loopback blocked");
/// ```
pub fn validate_host(host: &str) -> Result<()> {
    let addrs: Vec<SocketAddr> = (host, 0)
        .to_socket_addrs()
        .with_context(|| format!("resolve host `{host}`"))?
        .collect();
    if addrs.is_empty() {
        anyhow::bail!("host `{host}` resolved to no addresses");
    }
    for addr in &addrs {
        if is_blocked_addr(addr.ip()) {
            warn!(
                target: TARGET,
                "SSRF block: host `{host}` resolves to blocked address `{}`",
                addr.ip()
            );
            anyhow::bail!("host `{host}` resolves to blocked address `{}`", addr.ip());
        }
    }
    Ok(())
}

/// Validate a URL for SSRF safety: scheme + host resolution.
///
/// Returns the validated URL if allowed, or an error describing the block.
///
/// ```
/// use nxm_tui_tools::validate_url;
///
/// assert!(validate_url("http://example.com").is_ok());
/// assert!(validate_url("https://example.com").is_ok());
/// assert!(validate_url("ftp://example.com").is_err());
/// assert!(validate_url("file:///etc/passwd").is_err());
/// ```
pub fn validate_url(url_str: &str) -> Result<Url> {
    let url = Url::parse(url_str).context("parse URL")?;
    if !is_allowed_scheme(&url) {
        warn!(
            target: TARGET,
            "SSRF block: disallowed scheme `{}` for URL `{}`",
            url.scheme(),
            url_str
        );
        anyhow::bail!("scheme `{}` not allowed (only http/https)", url.scheme());
    }
    let host = url.host_str().context("URL has no host")?;
    validate_host(host).with_context(|| format!("validate host `{host}`"))?;
    Ok(url)
}

/// Maximum number of redirects to follow.
const MAX_REDIRECTS: usize = 10;

/// Perform a GET request with SSRF protection on redirects.
///
/// Uses a custom redirect policy that validates each hop's host.
/// The initial URL is also validated before the first request.
/// If `NEXUM_WEBFETCH_ALLOW_LOCAL=1`, the guard is completely disabled
/// (for local development only).
///
/// ```
/// use nxm_tui_tools::fetch_with_ssrf_guard;
///
/// // Public URL should work (requires network)
/// // let resp = fetch_with_ssrf_guard("http://example.com").expect("public fetch");
/// // assert!(resp.status().is_success());
/// // Loopback should be blocked without allow-local
/// // let res = fetch_with_ssrf_guard("http://127.0.0.1:9999");
/// // assert!(res.is_err());
/// ```
pub fn fetch_with_ssrf_guard(url_str: &str) -> Result<reqwest::blocking::Response> {
    // Check allow-list env var for dev — completely disables the guard
    if std::env::var("NEXUM_WEBFETCH_ALLOW_LOCAL").is_ok() {
        info!(target: TARGET, "SSRF guard completely disabled via NEXUM_WEBFETCH_ALLOW_LOCAL");
        return Ok(reqwest::blocking::get(url_str)?);
    }

    // Validate initial URL
    let mut url = validate_url(url_str).context("initial URL validation")?;

    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("build HTTP client")?;

    for hop in 0..MAX_REDIRECTS {
        info!(target: TARGET, "web_fetch GET hop={} url={}", hop, url);
        let resp = client.get(url.as_str()).send()?;

        // If not a redirect, return the response
        if !resp.status().is_redirection() {
            return Ok(resp);
        }

        // Follow redirect: validate the Location header
        let location = resp
            .headers()
            .get(reqwest::header::LOCATION)
            .context("redirect missing Location header")?
            .to_str()
            .context("Location header not valid UTF-8")?;

        // Resolve relative redirect
        url = url.join(location).context("resolve redirect URL")?;

        // Validate redirect target
        validate_url(url.as_str())
            .with_context(|| format!("redirect to `{}` blocked by SSRF guard", url))?;
    }

    anyhow::bail!("too many redirects (>{MAX_REDIRECTS})")
}
