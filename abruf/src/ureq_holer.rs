// Mär — an audio player for self-hosted open ecosystems
// Copyright (C) 2026 Laurenz Schröder
// SPDX-License-Identifier: GPL-3.0-or-later
// Additional terms apply — see LICENSE-ADDITIONAL-TERMS

//! The real `Holer` over `ureq`.
//!
//! It makes exactly one request and decides nothing a source could abuse:
//! redirects are returned, not followed (`netz::Netz` follows them, https
//! only, a bounded number); every status comes back as an answer; reading
//! stops one byte after the bound, counted after decoding; every request
//! has its time limit. TLS is rustls with the Mozilla roots compiled in
//! (`webpki-roots`), so the hardened image needs no certificate file.
//! **Certificate verification cannot be turned off here**: there is no
//! switch for it, and a witness holds that.

use std::time::Duration;

use ureq::Agent;

use crate::netz::{Anfrage, Antwort, HolFehler, Holer};

pub struct UreqHoler {
    agent: Agent,
}

fn agent(https_only: bool) -> Agent {
    let config = Agent::config_builder()
        .max_redirects(0)
        .http_status_as_error(false)
        .https_only(https_only)
        .save_redirect_history(false)
        // ureq takes ALL_PROXY, HTTPS_PROXY and HTTP_PROXY from the environment by
        // default; a container's environment must not turn the fetch aside.
        .proxy(None)
        .build();
    Agent::new_with_config(config)
}

impl UreqHoler {
    /// The holer of the program: https only.
    pub fn new() -> Self {
        UreqHoler { agent: agent(true) }
    }

    /// The same holer allowing plain http, for the witnesses' local server
    /// only (it speaks no TLS). Verification is the same.
    pub fn plain_http_for_witnesses() -> Self {
        UreqHoler { agent: agent(false) }
    }

    pub fn verifies_certificates(&self) -> bool {
        !self.agent.config().tls_config().disable_verification()
    }

    /// Whether a proxy is set (never: see `agent`).
    pub fn follows_a_proxy(&self) -> bool {
        self.agent.config().proxy().is_some()
    }

    pub fn https_only(&self) -> bool {
        self.agent.config().https_only()
    }
}

impl Default for UreqHoler {
    fn default() -> Self {
        Self::new()
    }
}

fn mapped(e: ureq::Error, max_bytes: usize) -> HolFehler {
    match e {
        ureq::Error::Timeout(_) => HolFehler::Zeit,
        ureq::Error::BodyExceedsLimit(_) => HolFehler::ZuGross(max_bytes + 1),
        other => HolFehler::Verbindung(other.to_string()),
    }
}

impl Holer for UreqHoler {
    fn hole(&mut self, a: &Anfrage) -> Result<Antwort, HolFehler> {
        let mut resp = self.agent
            .get(&a.url)
            .header("User-Agent", &a.user_agent)
            .config()
            .timeout_global(Some(Duration::from_secs(a.timeout_s)))
            .build()
            .call()
            .map_err(|e| mapped(e, a.max_bytes))?;
        let header = |name: &str| resp.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_string);
        let location = header("location");
        let retry_after_s = header("retry-after").and_then(|v| v.trim().parse().ok());
        let status = resp.status().as_u16();
        let koerper = resp.body_mut()
            .with_config()
            .limit(a.max_bytes as u64)
            .read_to_vec()
            .map_err(|e| mapped(e, a.max_bytes))?;
        Ok(Antwort { status, koerper, location, retry_after_s })
    }
}
