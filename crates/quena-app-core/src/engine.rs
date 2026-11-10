//! Capture engine: the proxy plus system integration (system proxy,
//! upstream detection, root CA management).

use crate::settings::{DecryptScope as SDecryptScope, Settings};
use crate::{AppCore, CaptureEngine, EngineStatus};
use anyhow::{Context, Result, anyhow};
use parking_lot::{Mutex, RwLock};
use quena_proxy::util::{Cidr, split_host_port, split_list};
use quena_proxy::{DecryptScope, Proxy, ProxyConfig, UpstreamResolver};
use quena_tls::CertAuthority;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

pub struct ProxyEngine {
    pub proxy: Arc<Proxy>,
    system_proxy: Arc<dyn SystemProxyControl>,
    ca: RwLock<Option<Arc<CertAuthority>>>,
    data_dir: PathBuf,
    state: Mutex<State>,
    rules: RwLock<Option<Arc<crate::rules::Rules>>>,
    /// Currently loaded PAC resolver (cached; rebuilt only when the source changes).
    pac: Mutex<Option<Arc<crate::pac::PacResolver>>>,
}

#[derive(Default)]
struct State {
    /// The port and exceptions last applied to the OS, independent of saved preferences.
    system_proxy: Option<SystemProxyConfig>,
    upstream: Option<String>,
    error: Option<String>,
    /// System proxy found before Quena took over (used as upstream).
    detected_upstream: Option<(String, u16)>,
    /// That proxy's exceptions: hosts the system reached directly, so Quena does too.
    detected_bypass: Vec<String>,
    pac_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SystemProxyConfig {
    port: u16,
    bypass: Vec<String>,
}

/// Keep OS changes at the integration boundary so lifecycle tests never change the host.
trait SystemProxyControl: Send + Sync {
    fn set(&self, port: u16, bypass: &[String], backup: &std::path::Path) -> quena_platform::Result<()>;
    fn restore(&self, backup: &std::path::Path) -> quena_platform::Result<bool>;
}

struct PlatformSystemProxy;

impl SystemProxyControl for PlatformSystemProxy {
    fn set(&self, port: u16, bypass: &[String], backup: &std::path::Path) -> quena_platform::Result<()> {
        quena_platform::set_system_proxy(port, bypass, backup)
    }

    fn restore(&self, backup: &std::path::Path) -> quena_platform::Result<bool> {
        quena_platform::restore_system_proxy(backup)
    }
}

/// Exceptions Quena writes into the system proxy settings while capturing. macOS and Linux
/// keep their usual defaults (Bonjour names, link-local addresses). Windows gets none: the
/// system default there is empty, and `<local>` ("bypass for local addresses") would let
/// intranet and VPN hosts without a dot pass Quena unseen.
/// Exceptions of the system proxy while Quena is it: link-local and `.local`, and the hosts
/// that do not go through Quena ([`crate::bypass`]).
fn system_bypass(s: &crate::settings::Settings) -> Vec<String> {
    let mut v: Vec<String> = if cfg!(windows) { vec![] } else { vec!["*.local".into(), "169.254/16".into()] };
    v.extend(crate::bypass::hosts(s));
    v
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaInfo {
    pub exists: bool,
    pub trusted: bool,
    pub sha256: String,
    pub path: String,
    pub pem: String,
    /// Common name of the CA certificate.
    pub name: String,
    pub issuer: String,
    /// End of validity (Unix seconds).
    pub not_after: i64,
    /// Certificates above the CA sent along (an imported intermediate CA).
    pub chain: usize,
    /// Quena created this CA (not imported).
    pub generated: bool,
}

/// Where an imported CA comes from: a PEM certificate and key, or a PKCS#12 file.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CaImport {
    pub cert_path: String,
    pub key_path: String,
    pub p12_path: String,
    pub password: String,
}

/// Export formats of the CA.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CaExportFormat {
    /// Certificate only, PEM.
    Pem,
    /// Certificate only, DER.
    Der,
    /// Certificate, chain and private key, password protected.
    P12,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub port: u16,
    pub allow_remote: bool,
    pub listening: bool,
    pub addresses: Vec<(String, String)>,
    pub ca_sha256: Option<String>,
}

fn backup_path(data: &std::path::Path) -> PathBuf {
    data.join("system-proxy-backup.json")
}

/// A panic on the main thread ends the app without the normal shutdown: restore the
/// system proxy first, so the machine is not left pointing at a proxy that is gone.
/// Panics on worker threads are caught and do not end the app, so they leave it alone.
pub fn install_panic_hook(data: &std::path::Path) {
    let backup = backup_path(data);
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if std::thread::current().name() == Some("main") && backup.exists() {
            let _ = quena_platform::restore_system_proxy(&backup);
        }
        prev(info);
    }));
}

pub fn proxy_config(s: &Settings, detected: Option<(String, u16)>, system_bypass: &[String], pac: Option<Arc<dyn UpstreamResolver>>) -> ProxyConfig {
    let mut upstream_bypass = split_list(&s.proxy.upstream_bypass);
    let upstream = if !s.proxy.manual_upstream.trim().is_empty() {
        let (h, p) = split_host_port(s.proxy.manual_upstream.trim(), 8080);
        Some((h, p))
    } else if s.proxy.use_system_upstream {
        // Hosts the system proxy's exceptions sent direct go direct from Quena too (Quena
        // sets its own, shorter exceptions, so it now sees these hosts).
        if detected.is_some() {
            upstream_bypass.extend(system_bypass.iter().map(|b| b.trim().to_string()).filter(|b| !b.is_empty()));
        }
        detected
    } else {
        None
    };
    ProxyConfig {
        port: s.proxy.port,
        allow_remote: s.proxy.allow_remote,
        remote_allowlist: Cidr::parse_list(&s.proxy.remote_allowlist),
        decrypt: s.https.decrypt,
        decrypt_scope: match s.https.scope {
            SDecryptScope::All => DecryptScope::All,
            SDecryptScope::Browsers => DecryptScope::Browsers,
            SDecryptScope::NonBrowsers => DecryptScope::NonBrowsers,
            SDecryptScope::Remote => DecryptScope::Remote,
        },
        // Hosts kept away from Quena: passed through undecrypted when a client sends them anyway.
        skip_decryption: split_list(&s.https.skip_decryption).into_iter().chain(crate::bypass::hosts(s)).collect(),
        ignore_cert_errors: s.https.ignore_cert_errors,
        ignore_cert_errors_hosts: split_list(&s.https.ignore_cert_errors_hosts),
        cert_warn_days: s.https.cert_warn_days,
        enable_http2: s.https.enable_http2,
        http2_downgrade_hosts: split_list(&s.https.http2_downgrade_hosts),
        upstream,
        upstream_bypass,
        pac,
        stream: s.stream,
        headers_only_hosts: split_list(&s.headers_only_hosts),
        headers_only_types: split_list(&s.headers_only_types),
        lossless: s.lossless_recording,
        auto_auth: s.auth.enabled,
        auto_auth_hosts: split_list(&s.auth.hosts),
        auto_auth_upstream: s.auth.upstream,
        auth_prefer: parse_prefer(&s.auth.prefer),
        throttle_bps: s.throttle_kbps.saturating_mul(1000) / 8,
        throttle_latency_ms: s.throttle_latency_ms,
        reverse: s
            .reverse_proxy
            .active()
            .filter_map(|e| e.to_route().map_err(|err| tracing::warn!(target: "quena", "reverse proxy {err}")).ok())
            .collect(),
        socks: s.socks.to_port(),
        transparent: s.transparent.to_port(),
        host_remap: s.host_remap.rules(),
    }
}

fn parse_prefer(s: &str) -> Vec<quena_proxy::Scheme> {
    let mut out = Vec::new();
    for t in s.split([';', ',', ' ']).map(|t| t.trim().to_ascii_lowercase()) {
        match t.as_str() {
            "negotiate" | "kerberos" => out.push(quena_proxy::Scheme::Negotiate),
            "ntlm" => out.push(quena_proxy::Scheme::Ntlm),
            "basic" => out.push(quena_proxy::Scheme::Basic),
            _ => {}
        }
    }
    if out.is_empty() {
        out = vec![quena_proxy::Scheme::Negotiate, quena_proxy::Scheme::Ntlm, quena_proxy::Scheme::Basic];
    }
    out
}

impl ProxyEngine {
    pub fn new(core: &Arc<AppCore>) -> Result<Arc<ProxyEngine>> {
        Self::with_system_proxy(core, Arc::new(PlatformSystemProxy))
    }

    fn with_system_proxy(core: &Arc<AppCore>, system_proxy: Arc<dyn SystemProxyControl>) -> Result<Arc<ProxyEngine>> {
        let data = core.paths.data.clone();
        // Browsers open many connections; the default soft limit (256 on macOS) is too low.
        if let Some(n) = quena_platform::raise_fd_limit(2 * quena_proxy::MAX_CLIENT_CONNECTIONS as u64 + 1024) {
            tracing::debug!(target: "quena", "open-file limit {n}");
        }
        // Crash recovery: a previous run left the system proxy pointing to us.
        match system_proxy.restore(&backup_path(&data)) {
            Ok(true) => tracing::warn!(target: "quena", "restored the system proxy left over by a previous run"),
            Ok(false) => {}
            Err(e) => tracing::error!(target: "quena", "restoring the system proxy failed: {e}"),
        }
        let s = core.settings();
        let ca = if s.https.decrypt || data.join(quena_tls::CA_CERT_FILE).exists() {
            Some(Arc::new(CertAuthority::load_or_create(&data).context("root CA")?))
        } else {
            None
        };
        let detected = detect_upstream(s.proxy.port);
        // The PAC file (possibly a download) is loaded by the first `apply`, i.e. when
        // capturing starts on its background thread, not while the app is starting up.
        let pac_slot: Mutex<Option<Arc<crate::pac::PacResolver>>> = Mutex::new(None);
        let proxy = Proxy::new(core.capture(), proxy_config(&s, detected.0.clone(), &detected.2, None), ca.clone()).map_err(|e| anyhow!("{e}"))?;
        proxy.shared.recorder.set_lossless(s.lossless_recording);
        if let Some(r) = &core.rules {
            proxy.set_interceptor(r.clone());
        }
        proxy.set_credential_resolver(Arc::new(crate::auth::AppCredentials::new(core.clone())));
        let e = Arc::new(ProxyEngine {
            proxy,
            system_proxy,
            ca: RwLock::new(ca),
            data_dir: data,
            state: Mutex::new(State { detected_upstream: detected.0, pac_url: detected.1, detected_bypass: detected.2, ..Default::default() }),
            rules: RwLock::new(core.rules.clone()),
            pac: pac_slot,
        });
        e.apply_client_certs(&s);
        Ok(e)
    }

    pub(crate) fn ensure_ca(&self) -> Result<Arc<CertAuthority>> {
        if let Some(c) = self.ca.read().clone() {
            return Ok(c);
        }
        let ca = Arc::new(CertAuthority::load_or_create(&self.data_dir)?);
        *self.ca.write() = Some(ca.clone());
        self.proxy.set_ca(Some(ca.clone()));
        Ok(ca)
    }

    pub fn ca_info(&self) -> CaInfo {
        match self.ca.read().clone() {
            Some(ca) => {
                let f = ca.facts();
                CaInfo {
                    exists: true,
                    trusted: quena_platform::is_root_ca_trusted(&ca.cert_path()),
                    sha256: ca.sha256_fingerprint(),
                    path: ca.cert_path().display().to_string(),
                    pem: ca.cert_pem().to_string(),
                    name: ca.common_name(),
                    generated: f.common_name.as_deref() == Some(quena_tls::CA_COMMON_NAME) && f.subject.contains("Quena"),
                    issuer: f.issuer,
                    not_after: f.not_after,
                    chain: ca.chain_len(),
                }
            }
            None => CaInfo { exists: false, trusted: false, sha256: String::new(), path: String::new(), pem: String::new(), name: String::new(), issuer: String::new(), not_after: 0, chain: 0, generated: true },
        }
    }

    /// Install the root CA into the user's trust store (OS asks for confirmation).
    pub fn ca_trust(&self) -> Result<CaInfo> {
        let ca = self.ensure_ca()?;
        quena_platform::install_root_ca(&ca.cert_path()).map_err(|e| anyhow!("{e}"))?;
        tracing::info!(target: "quena", "root certificate trusted");
        Ok(self.ca_info())
    }

    /// Windows: trust the root CA for all users of the machine, or remove it from there
    /// (administrator rights; Windows asks).
    pub fn ca_machine(&self, trust: bool) -> Result<CaInfo> {
        let ca = self.ensure_ca()?;
        quena_platform::machine_root_ca(&ca.cert_path(), &ca.sha1_fingerprint(), trust).map_err(|e| anyhow!("{e}"))?;
        tracing::info!(target: "quena", "root certificate {} the local machine's store", if trust { "added to" } else { "removed from" });
        Ok(self.ca_info())
    }

    pub fn ca_remove(&self) -> Result<CaInfo> {
        if let Some(ca) = self.ca.read().clone() {
            quena_platform::remove_root_ca(&ca.cert_path(), &ca.sha1_fingerprint()).map_err(|e| anyhow!("{e}"))?;
            tracing::info!(target: "quena", "root certificate removed from the trust store");
            // Trusted for all users too (Windows): that store needs its own removal.
            if cfg!(windows) && quena_platform::is_root_ca_trusted(&ca.cert_path()) {
                return Err(anyhow!("removed for this user, but it is still trusted for all users of this computer: right-click \"Trust for all users…\" to remove it from there"));
            }
        }
        Ok(self.ca_info())
    }

    /// Create a new CA (the old one should be removed from the trust store first).
    pub fn ca_regenerate(&self) -> Result<CaInfo> {
        let ca = Arc::new(CertAuthority::regenerate(&self.data_dir)?);
        *self.ca.write() = Some(ca.clone());
        self.proxy.set_ca(Some(ca));
        Ok(self.ca_info())
    }

    pub fn ca_export(&self, path: PathBuf, format: CaExportFormat, password: &str) -> Result<()> {
        let ca = self.ensure_ca()?;
        match format {
            CaExportFormat::Der => std::fs::write(path, ca.cert_der())?,
            CaExportFormat::Pem => std::fs::write(path, ca.cert_pem())?,
            CaExportFormat::P12 => {
                let data = ca.to_p12(password)?;
                std::fs::write(&path, data)?;
                tracing::warn!(target: "quena", "root CA exported with its private key to {}", path.display());
            }
        }
        Ok(())
    }

    /// Use an existing CA (e.g. the company's interception CA) instead of the current one.
    /// The current CA's files are kept as `*.bak-<time>` in the data directory.
    pub fn ca_import(&self, src: &CaImport) -> Result<CaInfo> {
        let read = |p: &str| std::fs::read(p.trim()).with_context(|| format!("cannot read {p}"));
        let m = if !src.p12_path.trim().is_empty() {
            quena_tls::ca_from_p12(&read(&src.p12_path)?, &src.password)?
        } else {
            if src.cert_path.trim().is_empty() || src.key_path.trim().is_empty() {
                return Err(anyhow!("choose the certificate and its private key, or a .p12 file"));
            }
            let cert = String::from_utf8_lossy(&read(&src.cert_path)?).into_owned();
            let key = String::from_utf8_lossy(&read(&src.key_path)?).into_owned();
            quena_tls::ca_from_pem(&cert, &key)?
        };
        let ca = Arc::new(CertAuthority::import(&self.data_dir, &m)?);
        tracing::info!(target: "quena", "root CA replaced by {}", ca.common_name());
        *self.ca.write() = Some(ca.clone());
        self.proxy.set_ca(Some(ca));
        Ok(self.ca_info())
    }

    pub fn device_info(&self, core: &AppCore) -> DeviceInfo {
        let s = core.settings();
        let listen = self.proxy.listen_addrs();
        DeviceInfo {
            port: listen.first().map(|a| a.port()).unwrap_or(s.proxy.port),
            allow_remote: s.proxy.allow_remote,
            listening: !listen.is_empty(),
            addresses: quena_platform::local_addresses(),
            ca_sha256: self.ca.read().as_ref().map(|c| c.sha256_fingerprint()),
        }
    }

    /// Attach/detach the computer without restarting listeners or closing device traffic.
    fn sync_system_proxy(&self, s: &Settings) -> Result<()> {
        let desired = self.proxy.listen_addrs().first().filter(|_| s.proxy.act_as_system_proxy)
            .map(|addr| SystemProxyConfig { port: addr.port(), bypass: system_bypass(s) });
        let unchanged = {
            let st = self.state.lock();
            st.system_proxy == desired && st.error.is_none()
        };
        if unchanged {
            return Ok(());
        }
        let Some(config) = desired else { return self.restore_system_proxy() };
        if let Err(e) = self.system_proxy.set(config.port, &config.bypass, &backup_path(&self.data_dir)) {
            // A failed attach can have changed some network services. Restore the backup,
            // retaining ownership if rollback fails so a later save/stop can retry.
            let restored = self.system_proxy.restore(&backup_path(&self.data_dir));
            let message = match &restored {
                Ok(_) => format!("system proxy: {e}"),
                Err(restore) => format!("system proxy: {e}; restore system proxy: {restore}"),
            };
            let mut st = self.state.lock();
            st.system_proxy = restored.is_err().then_some(config);
            st.error = Some(message.clone());
            return Err(anyhow!(message));
        }
        let mut st = self.state.lock();
        st.system_proxy = Some(config);
        st.error = None;
        Ok(())
    }

    fn restore_system_proxy(&self) -> Result<()> {
        if self.state.lock().system_proxy.is_none() {
            self.state.lock().error = None;
            return Ok(());
        }
        let result = self.system_proxy.restore(&backup_path(&self.data_dir))
            .context("restore system proxy")
            .and_then(|restored| if restored { Ok(()) } else { Err(anyhow!("restore system proxy: backup is missing")) });
        let mut st = self.state.lock();
        match &result {
            Ok(()) => {
                st.system_proxy = None;
                st.error = None;
            }
            Err(e) => st.error = Some(format!("{e:#}")),
        }
        result
    }

    fn apply(&self, core: &Arc<AppCore>) -> Result<()> {
        let s = core.settings();
        // HTTPS clients of a reverse proxy port get certificates from the root CA too.
        let reverse_tls = s.reverse_proxy.active().any(|e| e.client_protocol != crate::settings::ClientProtocol::Http);
        if s.https.decrypt || reverse_tls {
            self.ensure_ca()?;
        }
        let (detected, system_pac, detected_bypass) = {
            let st = self.state.lock();
            (st.detected_upstream.clone(), st.pac_url.clone(), st.detected_bypass.clone())
        };
        let pac = resolve_pac(&self.pac, &s, system_pac.as_deref());
        let cfg = proxy_config(&s, detected, &detected_bypass, pac);
        self.state.lock().upstream = cfg.upstream.as_ref().map(|(h, p)| format!("{h}:{p}"));
        self.proxy.shared.recorder.set_lossless(s.lossless_recording);
        self.apply_client_certs(&s);
        self.proxy.reconfigure(cfg).map_err(|e| anyhow!("{e}"))
    }

    /// Load client certificates (mTLS) from the configured PEM files into the
    /// upstream TLS clients. Called on every (re)configure.
    fn apply_client_certs(&self, s: &Settings) {
        let tls = &self.proxy.shared.tls_clients;
        tls.clear_client_certs();
        for c in &s.https.client_certs {
            if c.host.trim().is_empty() || c.cert_path.trim().is_empty() {
                continue;
            }
            let chain = match std::fs::read_to_string(&c.cert_path) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(target: "quena", "client cert {} unreadable: {e}", c.cert_path);
                    continue;
                }
            };
            let key_path = if c.key_path.trim().is_empty() { &c.cert_path } else { &c.key_path };
            let key = match std::fs::read_to_string(key_path) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(target: "quena", "client key {key_path} unreadable: {e}");
                    continue;
                }
            };
            if let Err(e) = tls.add_client_cert(&c.host, &chain, &key) {
                tracing::warn!(target: "quena", "client cert for {} rejected: {e}", c.host);
            }
        }
    }
}

/// Read the current system proxy (before Quena overrides it).
/// Decide the effective PAC source and (re)build the resolver, caching it so the
/// engine is only recompiled when the source URL/path changes. A manual upstream
/// proxy disables PAC (the static upstream wins).
fn resolve_pac(
    slot: &Mutex<Option<Arc<crate::pac::PacResolver>>>,
    s: &Settings,
    system_pac: Option<&str>,
) -> Option<Arc<dyn UpstreamResolver>> {
    if !s.proxy.manual_upstream.trim().is_empty() {
        *slot.lock() = None;
        return None;
    }
    let manual = s.proxy.pac_url.trim();
    let location = if !manual.is_empty() {
        Some(manual.to_string())
    } else if s.proxy.use_system_pac {
        system_pac.map(|p| p.to_string())
    } else {
        None
    };
    let Some(location) = location else {
        *slot.lock() = None;
        return None;
    };
    if let Some(cur) = slot.lock().as_ref() {
        if cur.source_id == location {
            return Some(cur.clone() as Arc<dyn UpstreamResolver>);
        }
    }
    let source = match crate::pac::load_pac_source(&location) {
        Ok(src) => src,
        Err(e) => {
            tracing::warn!(target: "quena", "PAC load from {location} failed: {e}; using direct/static upstream");
            *slot.lock() = None;
            return None;
        }
    };
    let resolver = crate::pac::PacResolver::new(location.clone(), &source);
    if let Some(err) = resolver.error() {
        tracing::warn!(target: "quena", "PAC script {location} is invalid: {err}; using direct/static upstream");
        *slot.lock() = None;
        return None;
    }
    tracing::info!(target: "quena", "PAC loaded from {location}");
    *slot.lock() = Some(resolver.clone());
    Some(resolver as Arc<dyn UpstreamResolver>)
}

fn detect_upstream(own_port: u16) -> (Option<(String, u16)>, Option<String>, Vec<String>) {
    match quena_platform::system_proxy() {
        Ok(p) if p.points_to(own_port) => (None, None, vec![]),
        Ok(p) => {
            let up = p.https.clone().or(p.http.clone());
            if let Some((h, port)) = &up {
                tracing::info!(target: "quena", "upstream gateway detected: {h}:{port}");
            }
            if let Some(pac) = &p.pac_url {
                tracing::info!(target: "quena", "the system uses a proxy auto-config script ({pac}); Quena will evaluate it when \"use system PAC\" is enabled");
            }
            (up, p.pac_url, p.exceptions)
        }
        Err(e) => {
            tracing::debug!("system proxy detection failed: {e}");
            (None, None, vec![])
        }
    }
}

impl CaptureEngine for ProxyEngine {
    fn start(&self, core: &Arc<AppCore>) -> Result<()> {
        if core.settings().proxy.bypass_vpn {
            crate::bypass::refresh_vpn();
        }
        self.apply(core)?;
        match self.proxy.start() {
            Ok(a) => a,
            Err(e) => {
                self.state.lock().error = Some(e.to_string());
                return Err(anyhow!("{e}"));
            }
        };
        self.state.lock().error = None;
        if let Err(e) = self.sync_system_proxy(&core.settings()) {
            // Explicitly configured clients can still use the listener.
            tracing::warn!(target: "quena", "could not set the system proxy: {e:#}");
        }
        Ok(())
    }

    fn stop(&self, _core: &Arc<AppCore>) -> Result<()> {
        // Keep serving clients if restoring the OS fails, rather than leaving it pointing
        // at a closed port. The saved backup and runtime state allow another attempt.
        self.restore_system_proxy()?;
        self.proxy.stop();
        Ok(())
    }

    fn status(&self) -> EngineStatus {
        let st = self.state.lock();
        let cfg = self.proxy.shared.cfg();
        EngineStatus {
            capturing: self.proxy.is_running(),
            listen: self.proxy.listen_addrs().iter().map(|a| a.to_string()).collect(),
            system_proxy: st.system_proxy.is_some(),
            decrypting: cfg.decrypt && self.ca.read().is_some(),
            upstream: st.upstream.clone(),
            error: st.error.clone(),
            breakpoints: self.rules.read().as_ref().map(|r| r.breakpoints().labels()).unwrap_or_default(),
            paused: self.rules.read().as_ref().map(|r| r.paused().len()).unwrap_or(0),
            autoresponder: self.rules.read().as_ref().is_some_and(|r| r.autoresponder_active()),
            rewrite: self.rules.read().as_ref().is_some_and(|r| r.rewrite.active()),
            listeners: self.proxy.listener_status(),
        }
    }

    fn reconfigure(&self, core: &Arc<AppCore>) -> Result<()> {
        let result = self.apply(core).and_then(|_| self.sync_system_proxy(&core.settings()));
        if let Err(e) = &result {
            self.state.lock().error = Some(format!("{e:#}"));
        }
        result
    }

    fn capture_changed(&self, core: &Arc<AppCore>) {
        self.proxy.set_capture(core.capture());
    }

    fn shutdown(&self, core: &Arc<AppCore>) {
        let _ = self.stop(core);
    }
}

pub fn pac_url(e: &ProxyEngine) -> Option<String> {
    e.state.lock().pac_url.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::time::Duration;

    #[derive(Default)]
    struct FakeProxyState {
        current: Option<SystemProxyConfig>,
        backup: Option<Option<SystemProxyConfig>>,
        fail_set: bool,
        fail_restore: bool,
        set_calls: usize,
    }

    #[derive(Default)]
    struct FakeSystemProxy(Mutex<FakeProxyState>);

    impl SystemProxyControl for FakeSystemProxy {
        fn set(&self, port: u16, bypass: &[String], _backup: &std::path::Path) -> quena_platform::Result<()> {
            let mut st = self.0.lock();
            let previous = st.current.clone();
            st.backup.get_or_insert(previous);
            st.current = Some(SystemProxyConfig { port, bypass: bypass.to_vec() });
            st.set_calls += 1;
            // Model a partial OS change before failure, rather than a harmless no-op.
            if std::mem::take(&mut st.fail_set) {
                return Err(quena_platform::PlatformError::Command("attach denied".into()));
            }
            Ok(())
        }

        fn restore(&self, _backup: &std::path::Path) -> quena_platform::Result<bool> {
            let mut st = self.0.lock();
            if std::mem::take(&mut st.fail_restore) {
                return Err(quena_platform::PlatformError::Command("restore denied".into()));
            }
            let Some(previous) = st.backup.take() else { return Ok(false) };
            st.current = previous;
            Ok(true)
        }
    }

    struct TestProxy {
        core: Arc<AppCore>,
        engine: Arc<ProxyEngine>,
        system: Arc<FakeSystemProxy>,
        original: SystemProxyConfig,
        _dir: tempfile::TempDir,
    }

    impl TestProxy {
        fn new(system_proxy: bool) -> Self {
            let dir = tempfile::tempdir().unwrap();
            let mut settings = Settings::default();
            settings.proxy.port = 0;
            settings.proxy.allow_remote = true;
            settings.proxy.act_as_system_proxy = system_proxy;
            settings.proxy.capture_on_startup = false;
            settings.proxy.use_system_upstream = false;
            settings.proxy.use_system_pac = false;
            settings.save(&dir.path().join("settings.json")).unwrap();
            let core = AppCore::new(crate::Paths::at(dir.path().to_path_buf()), crate::logbuf::LogBuffer::new(100)).unwrap();
            let original = SystemProxyConfig { port: 3128, bypass: vec!["*.company.example".into()] };
            let system = Arc::new(FakeSystemProxy(Mutex::new(FakeProxyState { current: Some(original.clone()), ..Default::default() })));
            let engine = ProxyEngine::with_system_proxy(&core, system.clone()).unwrap();
            core.set_proxy_engine(engine.clone());
            Self { core, engine, system, original, _dir: dir }
        }

        fn assert_preference(&self, on: bool) {
            assert_eq!(self.core.settings().proxy.act_as_system_proxy, on);
            assert_eq!(Settings::load(&self.core.paths.settings).proxy.act_as_system_proxy, on);
        }

        fn assert_original_proxy(&self) {
            assert_eq!(self.system.0.lock().current.as_ref(), Some(&self.original));
        }

        /// An established CONNECT tunnel stands in for a phone's persistent connection.
        fn device_tunnel(&self) -> TcpStream {
            let origin = TcpListener::bind("127.0.0.1:0").unwrap();
            let target = origin.local_addr().unwrap();
            std::thread::spawn(move || {
                let (mut stream, _) = origin.accept().unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
                let mut buf = [0; 256];
                while let Ok(n) = stream.read(&mut buf) {
                    if n == 0 || stream.write_all(&buf[..n]).is_err() { break }
                }
            });
            let addr = self.engine.proxy.listen_addrs().into_iter().find(|a| a.is_ipv4()).unwrap();
            let mut client = TcpStream::connect(addr).unwrap();
            client.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            client.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
            write!(client, "CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n\r\n").unwrap();
            let mut head = Vec::new();
            while !head.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                client.read_exact(&mut byte).unwrap();
                head.push(byte[0]);
                assert!(head.len() < 4096);
            }
            assert!(String::from_utf8_lossy(&head).starts_with("HTTP/1.1 200"));
            assert_tunnel_works(&mut client);
            client
        }
    }

    impl Drop for TestProxy {
        fn drop(&mut self) { self.core.shutdown(); }
    }

    fn assert_tunnel_works(client: &mut TcpStream) {
        client.write_all(b"phone traffic").unwrap();
        let mut reply = [0; 13];
        client.read_exact(&mut reply).unwrap();
        assert_eq!(&reply, b"phone traffic");
    }

    #[test]
    fn system_proxy_settings_apply_live_without_interrupting_devices() {
        let env = TestProxy::new(true);
        env.core.start_capture().unwrap();
        let listeners = env.engine.status().listen;
        let mut device = env.device_tunnel();
        let mut settings = env.core.settings();
        settings.proxy.act_as_system_proxy = false;
        env.core.update_settings(settings).unwrap();
        assert!(!env.engine.status().system_proxy);
        env.assert_preference(false);
        env.assert_original_proxy();
        assert!(env.engine.status().capturing);
        assert_eq!(env.engine.status().listen, listeners);
        assert_tunnel_works(&mut device);

        let mut settings = env.core.settings();
        settings.proxy.act_as_system_proxy = true;
        env.core.update_settings(settings).unwrap();
        assert!(env.engine.status().system_proxy);
        assert_eq!(env.engine.status().listen, listeners);
        assert_tunnel_works(&mut device);
        env.assert_preference(true);
        env.core.stop_capture().unwrap();
        env.assert_original_proxy();
    }

    #[test]
    fn f12_persists_local_capture_and_keeps_device_connections_alive() {
        let env = TestProxy::new(false);
        env.core.start_capture().unwrap();
        let listeners = env.engine.status().listen;
        let mut device = env.device_tunnel();
        for expected in [true, false, true, false] {
            assert_eq!(env.core.toggle_system_proxy().unwrap(), expected);
            env.assert_preference(expected);
            assert_eq!(env.engine.status().system_proxy, expected);
            assert!(env.engine.status().capturing);
            assert_eq!(env.engine.status().listen, listeners);
            assert_tunnel_works(&mut device);
        }
        env.assert_original_proxy();
        // A newly connected explicit client still works with local capture off.
        assert_tunnel_works(&mut env.device_tunnel());
    }

    #[test]
    fn saving_system_proxy_while_stopped_does_not_start_listener_but_f12_does() {
        let env = TestProxy::new(false);
        let mut settings = env.core.settings();
        settings.proxy.act_as_system_proxy = true;
        env.core.update_settings(settings).unwrap();
        assert!(!env.engine.status().capturing);
        assert!(!env.engine.status().system_proxy);
        assert_eq!(env.system.0.lock().set_calls, 0);
        env.assert_original_proxy();
        assert!(env.core.toggle_system_proxy().unwrap());
        assert!(env.engine.status().capturing);
        assert!(env.engine.status().system_proxy);
        env.assert_preference(true);
        env.core.stop_capture().unwrap();
        env.assert_original_proxy();
    }

    #[test]
    fn system_proxy_port_and_bypass_follow_listener_changes() {
        let env = TestProxy::new(true);
        env.core.start_capture().unwrap();
        let mut settings = env.core.settings();
        let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
        settings.proxy.port = reservation.local_addr().unwrap().port();
        settings.proxy.bypass_hosts = "skip.example.com".into();
        drop(reservation);
        env.core.update_settings(settings.clone()).unwrap();
        {
            let st = env.system.0.lock();
            let applied = st.current.as_ref().unwrap();
            assert_eq!(applied.port, settings.proxy.port);
            assert!(applied.bypass.contains(&"skip.example.com".into()));
        }
        env.core.stop_capture().unwrap();
        env.assert_original_proxy();
    }

    #[test]
    fn failed_system_proxy_restore_keeps_runtime_state_and_identical_save_retries() {
        let env = TestProxy::new(true);
        env.core.start_capture().unwrap();
        let mut device = env.device_tunnel();
        env.system.0.lock().fail_restore = true;
        let mut settings = env.core.settings();
        settings.proxy.act_as_system_proxy = false;
        assert!(env.core.update_settings(settings.clone()).is_err());
        env.assert_preference(false);
        assert!(env.engine.status().system_proxy);
        assert!(env.engine.status().error.is_some());
        assert_tunnel_works(&mut device);
        env.core.update_settings(settings).unwrap();
        assert!(!env.engine.status().system_proxy);
        assert!(env.engine.status().error.is_none());
        env.assert_original_proxy();
        assert_tunnel_works(&mut device);
    }

    #[test]
    fn failed_system_proxy_attach_rolls_back_and_identical_save_retries() {
        let env = TestProxy::new(false);
        env.core.start_capture().unwrap();
        env.system.0.lock().fail_set = true;
        let mut settings = env.core.settings();
        settings.proxy.act_as_system_proxy = true;
        assert!(env.core.update_settings(settings.clone()).is_err());
        env.assert_preference(true);
        assert!(!env.engine.status().system_proxy);
        assert!(env.engine.status().error.is_some());
        env.assert_original_proxy();
        env.core.update_settings(settings).unwrap();
        assert!(env.engine.status().system_proxy);
        assert!(env.engine.status().error.is_none());
        env.core.stop_capture().unwrap();
        env.assert_original_proxy();
    }

    #[test]
    fn failed_system_proxy_rollback_retains_backup_until_retry() {
        let env = TestProxy::new(false);
        env.core.start_capture().unwrap();
        {
            let mut st = env.system.0.lock();
            st.fail_set = true;
            st.fail_restore = true;
        }
        assert!(env.core.toggle_system_proxy().is_err());
        assert!(env.engine.status().system_proxy);
        assert!(env.engine.status().error.is_some());
        // A second F12 turns local capture off, using the retained original backup.
        assert!(!env.core.toggle_system_proxy().unwrap());
        env.assert_preference(false);
        env.assert_original_proxy();
        assert!(env.engine.status().capturing);
    }

    #[test]
    fn f12_reports_system_proxy_errors_when_starting_the_listener() {
        for rollback_fails in [false, true] {
            let env = TestProxy::new(false);
            {
                let mut st = env.system.0.lock();
                st.fail_set = true;
                st.fail_restore = rollback_fails;
            }
            assert!(env.core.toggle_system_proxy().is_err());
            assert!(env.engine.status().capturing);
            assert!(env.engine.status().error.is_some());
            env.assert_preference(true);
            env.core.update_settings(env.core.settings()).unwrap();
            assert!(env.engine.status().system_proxy);
            assert!(env.engine.status().error.is_none());
            env.core.stop_capture().unwrap();
            env.assert_original_proxy();
        }
    }

    #[test]
    fn failed_system_proxy_restore_does_not_leave_the_os_pointing_at_a_closed_listener() {
        let env = TestProxy::new(true);
        env.core.start_capture().unwrap();
        env.system.0.lock().fail_restore = true;
        assert!(env.core.stop_capture().is_err());
        assert!(env.engine.status().capturing);
        assert!(env.engine.status().system_proxy);
        env.core.stop_capture().unwrap();
        assert!(!env.engine.status().capturing);
        assert!(!env.engine.status().system_proxy);
        env.assert_original_proxy();
    }

    #[test]
    fn f12_does_not_change_runtime_or_preferences_when_saving_fails() {
        let env = TestProxy::new(false);
        env.core.start_capture().unwrap();
        // Block the atomic save's temporary file without changing the saved preference.
        std::fs::create_dir(env.core.paths.settings.with_extension("json.tmp")).unwrap();
        assert!(env.core.toggle_system_proxy().is_err());
        env.assert_preference(false);
        assert!(!env.engine.status().system_proxy);
        assert!(env.engine.status().capturing);
        assert_eq!(env.system.0.lock().set_calls, 0);
        env.assert_original_proxy();
    }

    #[test]
    fn system_exceptions_go_direct() {
        let mut s = Settings::default();
        let corp = Some(("proxy.corp".to_string(), 8080));
        let exc = vec!["<local>".to_string(), "*.corp.example".to_string(), " ".to_string()];
        let cfg = proxy_config(&s, corp.clone(), &exc, None);
        assert_eq!(cfg.upstream_for("appserver:80"), None);
        assert_eq!(cfg.upstream_for("wiki.corp.example:443"), None);
        assert_eq!(cfg.upstream_for("example.com:443"), corp);
        // A manual upstream comes with its own bypass list.
        s.proxy.manual_upstream = "gw.example:3128".into();
        let cfg = proxy_config(&s, corp, &exc, None);
        assert_eq!(cfg.upstream_for("appserver:80"), Some(("gw.example".to_string(), 3128)));
    }

    #[test]
    fn windows_keeps_local_addresses_in_capture() {
        let mut s = crate::settings::Settings::default();
        s.proxy.bypass_apple = false;
        let b = system_bypass(&s);
        assert!(!b.iter().any(|e| e == "<local>"));
        assert_eq!(b.is_empty(), cfg!(windows));
        // Hosts kept away from Quena are exceptions too.
        s.proxy.bypass_hosts = "login.example.com".into();
        assert!(system_bypass(&s).contains(&"login.example.com".to_string()));
        // … and pass Quena undecrypted when a client sends them anyway.
        let cfg = proxy_config(&s, None, &[], None);
        assert!(cfg.skip_decryption.contains(&"login.example.com".to_string()));
    }
}
