//! Red: servidores DNS (IPv4 e IPv6) por adaptador, con DNS cifrado (DoH) para los
//! proveedores del catálogo.
//!
//! - API oficial de Windows (`SetInterfaceDnsSettings`): aplica en el momento, sin `netsh`.
//! - Solo adaptadores físicos conectados (Ethernet y Wi-Fi). VPN, túneles y virtuales nunca
//!   se tocan: manejan su propio DNS.
//! - Las IPs se validan acá; las plantillas DoH salen solo del catálogo (nunca de la interfaz).
//! - Para no dejar a nadie sin internet: prueba previa de cada servidor y, después de aplicar,
//!   vuelta atrás automática si la persona no confirma (la arma `lib.rs` con `arm`).

use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Segundos para confirmar que internet sigue andando antes de volver atrás solo.
pub const CONFIRM_SECS: u64 = 15;
/// Máximo de servidores por protocolo.
const MAX_SERVERS: usize = 2;
/// Dominio que se consulta en la prueba previa.
const TEST_NAME: &str = "www.microsoft.com";

// ───────────────────────────── Catálogo ─────────────────────────────

pub struct Provider {
    pub id: &'static str,
    pub name: &'static str,
    pub v4: [&'static str; 2],
    pub v6: [&'static str; 2],
    pub doh: &'static str,
}

/// Proveedores ofrecidos. Fijos en el binario: la interfaz solo elige por id.
pub const PROVIDERS: [Provider; 5] = [
    Provider {
        id: "cloudflare",
        name: "Cloudflare",
        v4: ["1.1.1.1", "1.0.0.1"],
        v6: ["2606:4700:4700::1111", "2606:4700:4700::1001"],
        doh: "https://cloudflare-dns.com/dns-query",
    },
    Provider {
        id: "cloudflare_malware",
        name: "Cloudflare (malware)",
        v4: ["1.1.1.2", "1.0.0.2"],
        v6: ["2606:4700:4700::1112", "2606:4700:4700::1002"],
        doh: "https://security.cloudflare-dns.com/dns-query",
    },
    Provider {
        id: "google",
        name: "Google",
        v4: ["8.8.8.8", "8.8.4.4"],
        v6: ["2001:4860:4860::8888", "2001:4860:4860::8844"],
        doh: "https://dns.google/dns-query",
    },
    Provider {
        id: "quad9",
        name: "Quad9",
        v4: ["9.9.9.9", "149.112.112.112"],
        v6: ["2620:fe::fe", "2620:fe::9"],
        doh: "https://dns.quad9.net/dns-query",
    },
    Provider {
        id: "adguard",
        name: "AdGuard",
        v4: ["94.140.14.14", "94.140.15.15"],
        v6: ["2a10:50c0::ad1:ff", "2a10:50c0::ad2:ff"],
        doh: "https://dns.adguard-dns.com/dns-query",
    },
];

/// Plantilla DoH del catálogo para una IP (solo proveedores conocidos).
fn doh_template(ip: &IpAddr) -> Option<&'static str> {
    PROVIDERS
        .iter()
        .find(|p| p.v4.iter().chain(&p.v6).any(|s| s.parse::<IpAddr>().ok().as_ref() == Some(ip)))
        .map(|p| p.doh)
}

/// Proveedor al que pertenecen todos los servidores (para mostrar y para el historial).
pub fn provider_of(servers: &[String]) -> Option<&'static Provider> {
    let ips: Vec<IpAddr> = servers.iter().filter_map(|s| s.parse().ok()).collect();
    if ips.is_empty() || ips.len() != servers.len() {
        return None;
    }
    PROVIDERS.iter().find(|p| {
        ips.iter().all(|ip| p.v4.iter().chain(&p.v6).any(|s| s.parse::<IpAddr>().ok().as_ref() == Some(ip)))
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderView {
    pub id: &'static str,
    pub name: &'static str,
    pub v4: Vec<&'static str>,
    pub v6: Vec<&'static str>,
}

// ───────────────────────────── Tipos ─────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DohMode {
    #[default]
    Off,
    /// Cifrado, y si falla, sin cifrar.
    Preferred,
    /// Solo cifrado.
    Required,
}

/// Configuración de un protocolo en un adaptador. `servers` vacío = Automático (DHCP).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DnsConfig {
    #[serde(default)]
    pub servers: Vec<String>,
    #[serde(default)]
    pub doh: DohMode,
}

/// Configuración de un adaptador (lo que se pide, lo que se guarda en el historial).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterDns {
    /// "{GUID}" tal como lo da Windows.
    pub guid: String,
    #[serde(default)]
    pub name: String,
    pub v4: DnsConfig,
    pub v6: DnsConfig,
}

/// Lo que se guarda en el historial: cómo estaba y cómo quedó cada adaptador tocado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkChange {
    pub before: Vec<AdapterDns>,
    pub after: Vec<AdapterDns>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FamilyView {
    /// Configuración propia (vacía = automático).
    pub config: DnsConfig,
    /// Servidores que Windows usa de hecho (los propios o los del DHCP).
    pub effective: Vec<String>,
    /// Proveedor del catálogo, si los servidores propios son de uno.
    pub provider: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdapterView {
    pub guid: String,
    pub name: String,
    pub description: String,
    /// "ethernet" o "wifi".
    pub kind: &'static str,
    /// Tiene puerta de enlace (el que sale a internet).
    pub primary: bool,
    pub v4: FamilyView,
    pub v6: FamilyView,
}

#[derive(Debug, Clone, Serialize)]
pub struct PendingView {
    pub entry_id: u64,
    pub seconds_left: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetworkStatus {
    pub adapters: Vec<AdapterView>,
    pub providers: Vec<ProviderView>,
    /// Dominio de empresa al que está unido el equipo (cambiar el DNS lo rompería).
    pub domain: Option<String>,
    /// Hay DNS impuesto por una directiva (manda sobre lo de cada adaptador).
    pub policy: bool,
    /// Hay una VPN conectada (manda sobre el DNS mientras esté activa).
    pub vpn: bool,
    pub elevated: bool,
    pub pending: Option<PendingView>,
}

// ───────────────────────────── Validación ─────────────────────────────

/// Parsea y valida servidores de un protocolo. Devuelve las IPs normalizadas.
pub fn validate_servers(servers: &[String], v6: bool) -> Result<Vec<String>, String> {
    if servers.len() > MAX_SERVERS {
        return Err(t!("dns_too_many", n = MAX_SERVERS));
    }
    let mut out = Vec::new();
    for s in servers {
        let s = s.trim();
        let ip: IpAddr = s.parse().map_err(|_| t!("dns_bad_ip", ip = s))?;
        let ok = match ip {
            IpAddr::V4(a) if !v6 => valid_v4(&a),
            IpAddr::V6(a) if v6 => valid_v6(&a),
            _ => return Err(t!("dns_wrong_family", ip = s)),
        };
        if !ok {
            return Err(t!("dns_bad_ip", ip = s));
        }
        let norm = ip.to_string();
        if out.contains(&norm) {
            return Err(t!("dns_duplicate", ip = s));
        }
        out.push(norm);
    }
    Ok(out)
}

fn valid_v4(a: &Ipv4Addr) -> bool {
    !(a.is_unspecified() || a.is_broadcast() || a.is_multicast() || a.is_documentation())
}

fn valid_v6(a: &Ipv6Addr) -> bool {
    // Link-local (fe80::/10) necesita la zona del adaptador: no se acepta.
    let link_local = (a.segments()[0] & 0xffc0) == 0xfe80;
    !(a.is_unspecified() || a.is_multicast() || link_local || a.to_ipv4_mapped().is_some())
}

/// Sin servidores no hay cifrado; con servidores ajenos al catálogo, tampoco.
fn effective_doh(cfg: &DnsConfig) -> DohMode {
    if cfg.servers.iter().any(|s| s.parse().ok().as_ref().and_then(doh_template).is_some()) {
        cfg.doh
    } else {
        DohMode::Off
    }
}

fn normalize(cfg: &DnsConfig, v6: bool) -> Result<DnsConfig, String> {
    let servers = validate_servers(&cfg.servers, v6)?;
    let mut out = DnsConfig { servers, doh: cfg.doh };
    out.doh = effective_doh(&out);
    Ok(out)
}

/// Valida un pedido completo contra los adaptadores elegibles de este momento.
pub fn validate_request(req: &[AdapterDns], adapters: &[AdapterView]) -> Result<Vec<AdapterDns>, String> {
    if req.is_empty() {
        return Err(t!("dns_nothing"));
    }
    let mut out: Vec<AdapterDns> = Vec::new();
    for r in req {
        let a = adapters
            .iter()
            .find(|a| a.guid.eq_ignore_ascii_case(&r.guid))
            .ok_or_else(|| t!("dns_adapter_missing"))?;
        if out.iter().any(|o| o.guid == a.guid) {
            return Err(t!("dns_adapter_missing"));
        }
        out.push(AdapterDns {
            guid: a.guid.clone(),
            name: a.name.clone(),
            v4: normalize(&r.v4, false)?,
            v6: normalize(&r.v6, true)?,
        });
    }
    Ok(out)
}

// ───────────────────────────── Lectura ─────────────────────────────

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn guid_of(s: &str) -> Result<windows::core::GUID, String> {
    windows::core::GUID::try_from(s.trim_start_matches('{').trim_end_matches('}')).map_err(|_| t!("dns_adapter_missing"))
}

/// VPN, túneles y similares: nunca se tocan (manejan su propio DNS).
const VPN_WORDS: [&str; 17] = [
    "vpn", "wireguard", "wintun", "tap-", "tap ", "openvpn", "tunnel", "zerotier", "tailscale", "nordlynx", "proton",
    "fortinet", "anyconnect", "pangp", "loopback", "bluetooth", "miniport",
];

/// Adaptadores virtuales del lado del host (Hyper-V vEthernet, WSL, VirtualBox host-only,
/// VMware VMnet). Solo se ofrecen si son la salida a internet (p. ej. un conmutador externo de
/// Hyper-V). La placa de red de una PC que es ella misma una máquina virtual
/// ("Microsoft Hyper-V Network Adapter", vmxnet3) no es de estos y se trata como física.
const HOST_VIRTUAL_WORDS: [&str; 6] = ["vethernet", "virtual ethernet", "host-only", "vmnet", "wsl", "virtualbox"];

fn has_word(words: &[&str], description: &str, name: &str) -> bool {
    let d = format!("{} {}", description, name).to_ascii_lowercase();
    words.iter().any(|w| d.contains(w))
}

/// No se ofrece: VPN o túnel, o un virtual del host que no sale a internet.
fn excluded(description: &str, name: &str, gateway: bool) -> bool {
    has_word(&VPN_WORDS, description, name) || (!gateway && has_word(&HOST_VIRTUAL_WORDS, description, name))
}

struct RawAdapter {
    guid: String,
    name: String,
    description: String,
    if_type: u32,
    up: bool,
    gateway: bool,
    dns: Vec<IpAddr>,
}

fn sockaddr_ip(sa: &windows::Win32::Networking::WinSock::SOCKET_ADDRESS) -> Option<IpAddr> {
    use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6, SOCKADDR_IN, SOCKADDR_IN6};
    if sa.lpSockaddr.is_null() {
        return None;
    }
    unsafe {
        let family = (*sa.lpSockaddr).sa_family;
        if family == AF_INET && sa.iSockaddrLength as usize >= std::mem::size_of::<SOCKADDR_IN>() {
            let a = &*(sa.lpSockaddr as *const SOCKADDR_IN);
            Some(IpAddr::V4(Ipv4Addr::from(u32::from_be(a.sin_addr.S_un.S_addr))))
        } else if family == AF_INET6 && sa.iSockaddrLength as usize >= std::mem::size_of::<SOCKADDR_IN6>() {
            let a = &*(sa.lpSockaddr as *const SOCKADDR_IN6);
            Some(IpAddr::V6(Ipv6Addr::from(a.sin6_addr.u.Byte)))
        } else {
            None
        }
    }
}

fn raw_adapters() -> Vec<RawAdapter> {
    use windows::Win32::NetworkManagement::IpHelper::{
        GetAdaptersAddresses, GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_MULTICAST,
        IP_ADAPTER_ADDRESSES_LH,
    };
    use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows::Win32::Networking::WinSock::AF_UNSPEC;
    let flags = GAA_FLAG_INCLUDE_GATEWAYS | GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST;
    let mut size = 32 * 1024u32;
    let mut buf: Vec<u64> = Vec::new();
    for _ in 0..4 {
        buf = vec![0u64; (size as usize).div_ceil(8)];
        let rc = unsafe {
            GetAdaptersAddresses(AF_UNSPEC.0 as u32, flags, None, Some(buf.as_mut_ptr().cast()), &mut size)
        };
        match rc {
            0 => break,
            111 => continue, // ERROR_BUFFER_OVERFLOW: `size` ya trae lo necesario
            _ => return Vec::new(),
        }
    }
    let mut out = Vec::new();
    let mut p = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
    while !p.is_null() {
        let a = unsafe { &*p };
        let guid = unsafe { a.AdapterName.to_string() }.unwrap_or_default();
        let name = unsafe { a.FriendlyName.to_string() }.unwrap_or_default();
        let description = unsafe { a.Description.to_string() }.unwrap_or_default();
        let mut dns = Vec::new();
        let mut d = a.FirstDnsServerAddress;
        while !d.is_null() {
            let s = unsafe { &*d };
            if let Some(ip) = sockaddr_ip(&s.Address) {
                dns.push(ip);
            }
            d = s.Next;
        }
        out.push(RawAdapter {
            guid,
            name,
            description,
            if_type: a.IfType,
            up: a.OperStatus == IfOperStatusUp,
            gateway: !a.FirstGatewayAddress.is_null(),
            dns,
        });
        p = a.Next;
    }
    out
}

/// Ethernet o Wi-Fi, conectado y que no sea VPN ni virtual.
fn eligible_kind(a: &RawAdapter) -> Option<&'static str> {
    use windows::Win32::NetworkManagement::IpHelper::{IF_TYPE_ETHERNET_CSMACD, IF_TYPE_IEEE80211};
    if !a.up || excluded(&a.description, &a.name, a.gateway) {
        return None;
    }
    match a.if_type {
        IF_TYPE_ETHERNET_CSMACD => Some("ethernet"),
        IF_TYPE_IEEE80211 => Some("wifi"),
        _ => None,
    }
}

fn vpn_up(raw: &[RawAdapter]) -> bool {
    use windows::Win32::NetworkManagement::IpHelper::{IF_TYPE_PPP, IF_TYPE_PROP_VIRTUAL, IF_TYPE_TUNNEL};
    raw.iter().any(|a| {
        let named = has_word(&VPN_WORDS[..14], &a.description, &a.name);
        a.up && a.gateway && (named || matches!(a.if_type, IF_TYPE_PPP | IF_TYPE_TUNNEL | IF_TYPE_PROP_VIRTUAL))
    })
}

/// Configuración propia de un protocolo en un adaptador (servidores y DoH), del registro.
///
/// Se lee del registro y no con `GetInterfaceDnsSettings`: esa función tira abajo el proceso
/// al pedirle IPv6 (comprobado en Windows 11 26300). Escribir sí se hace con la API.
pub fn read_config(guid: &str, v6: bool) -> Result<DnsConfig, String> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY};
    use winreg::RegKey;
    // Solo un GUID válido llega a armar una ruta del registro.
    let g = guid_of(guid)?;
    let id = format!("{{{g:?}}}");
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let tcpip = if v6 { "Tcpip6" } else { "Tcpip" };
    let path = format!(r"SYSTEM\CurrentControlSet\Services\{tcpip}\Parameters\Interfaces\{id}");
    let servers_raw: String = hklm
        .open_subkey_with_flags(&path, KEY_READ | KEY_WOW64_64KEY)
        .and_then(|k| k.get_value("NameServer"))
        .unwrap_or_default();
    let servers: Vec<String> = servers_raw
        .split([',', ' ', ';'])
        .filter(|x| !x.is_empty())
        .map(|x| x.parse::<IpAddr>().map(|ip| ip.to_string()).unwrap_or_else(|_| x.to_string()))
        .collect();
    // DoH: Dnscache\InterfaceSpecificParameters\{GUID}\DohInterfaceSettings\Doh(6)\<ip>\DohFlags
    let doh_path = format!(
        r"SYSTEM\CurrentControlSet\Services\Dnscache\InterfaceSpecificParameters\{id}\DohInterfaceSettings\{}",
        if v6 { "Doh6" } else { "Doh" }
    );
    let mut doh = DohMode::Off;
    if let Ok(k) = hklm.open_subkey_with_flags(&doh_path, KEY_READ | KEY_WOW64_64KEY) {
        for name in k.enum_keys().flatten() {
            let ip: Option<IpAddr> = name.parse().ok();
            if !ip.is_some_and(|ip| servers.contains(&ip.to_string())) {
                continue;
            }
            let flags: u64 = k
                .open_subkey_with_flags(&name, KEY_READ | KEY_WOW64_64KEY)
                .and_then(|s| s.get_value::<u64, _>("DohFlags").or_else(|_| s.get_value::<u32, _>("DohFlags").map(u64::from)))
                .unwrap_or(0);
            // Si hay mezcla, gana el más estricto.
            if let Some(mode) = doh_mode_from_flags(flags)
                && doh != DohMode::Required
            {
                doh = mode;
            }
        }
    }
    Ok(DnsConfig { servers, doh })
}

/// Flags de Windows (`DNS_DOH_SERVER_SETTINGS_*`) → modo. None si no está activo.
fn doh_mode_from_flags(flags: u64) -> Option<DohMode> {
    use windows::Win32::NetworkManagement::IpHelper::{
        DNS_DOH_SERVER_SETTINGS_ENABLE, DNS_DOH_SERVER_SETTINGS_ENABLE_AUTO, DNS_DOH_SERVER_SETTINGS_FALLBACK_TO_UDP,
    };
    let f = flags as u32;
    if f & (DNS_DOH_SERVER_SETTINGS_ENABLE | DNS_DOH_SERVER_SETTINGS_ENABLE_AUTO) == 0 {
        return None;
    }
    Some(if f & DNS_DOH_SERVER_SETTINGS_FALLBACK_TO_UDP != 0 { DohMode::Preferred } else { DohMode::Required })
}

fn family_view(guid: &str, v6: bool, effective: &[IpAddr]) -> FamilyView {
    let config = read_config(guid, v6).unwrap_or_default();
    let provider = provider_of(&config.servers).map(|p| p.id);
    FamilyView {
        effective: effective.iter().filter(|ip| ip.is_ipv6() == v6).map(|ip| ip.to_string()).collect(),
        config,
        provider,
    }
}

/// Adaptadores que se pueden configurar, el principal primero.
pub fn adapters() -> Vec<AdapterView> {
    let mut out: Vec<AdapterView> = raw_adapters()
        .iter()
        .filter_map(|a| {
            let kind = eligible_kind(a)?;
            Some(AdapterView {
                guid: a.guid.clone(),
                name: a.name.clone(),
                description: a.description.clone(),
                kind,
                primary: a.gateway,
                v4: family_view(&a.guid, false, &a.dns),
                v6: family_view(&a.guid, true, &a.dns),
            })
        })
        .collect();
    out.sort_by_key(|a| !a.primary);
    out
}

/// Nombre del dominio de empresa, si el equipo está unido a uno.
fn domain() -> Option<String> {
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::NetworkManagement::NetManagement::{NetApiBufferFree, NetGetJoinInformation, NetSetupDomainName, NETSETUP_JOIN_STATUS};
    let mut name = PWSTR::null();
    let mut status = NETSETUP_JOIN_STATUS::default();
    let rc = unsafe { NetGetJoinInformation(PCWSTR::null(), &mut name, &mut status) };
    if rc != 0 {
        return None;
    }
    let s = unsafe { name.to_string() }.ok();
    unsafe { NetApiBufferFree(Some(name.0 as _)) };
    (status == NetSetupDomainName).then_some(s.unwrap_or_default())
}

/// DNS impuesto por directiva de grupo.
fn policy_dns() -> bool {
    use crate::catalog::Hive;
    matches!(
        crate::registry::read(Hive::HKLM, r"SOFTWARE\Policies\Microsoft\Windows NT\DNSClient", "NameServer"),
        Ok(Some(v)) if v.bytes.iter().any(|b| *b != 0)
    )
}

/// Lo que impide cambiar el DNS ahora (antes de probar o aplicar).
pub fn guard() -> Result<(), String> {
    if !crate::system::is_elevated() {
        return Err(t!("requires_admin"));
    }
    if let Some(d) = domain() {
        return Err(t!("dns_domain", domain = d));
    }
    if policy_dns() {
        return Err(t!("dns_policy"));
    }
    if has_pending() {
        return Err(t!("dns_pending"));
    }
    Ok(())
}

pub fn status() -> NetworkStatus {
    let raw = raw_adapters();
    NetworkStatus {
        vpn: vpn_up(&raw),
        adapters: adapters(),
        providers: PROVIDERS
            .iter()
            .map(|p| ProviderView { id: p.id, name: p.name, v4: p.v4.to_vec(), v6: p.v6.to_vec() })
            .collect(),
        domain: domain(),
        policy: policy_dns(),
        elevated: crate::system::is_elevated(),
        pending: pending_view(),
    }
}

// ───────────────────────────── Escritura ─────────────────────────────

/// Escribe la configuración de un protocolo. `servers` vacío = Automático (DHCP).
fn write_config(guid: &str, v6: bool, cfg: &DnsConfig) -> Result<(), String> {
    use windows::core::PWSTR;
    use windows::Win32::NetworkManagement::IpHelper::{
        SetInterfaceDnsSettings, DNS_DOH_SERVER_SETTINGS, DNS_DOH_SERVER_SETTINGS_ENABLE,
        DNS_DOH_SERVER_SETTINGS_FALLBACK_TO_UDP, DNS_INTERFACE_SETTINGS, DNS_INTERFACE_SETTINGS3,
        DNS_INTERFACE_SETTINGS_VERSION3, DNS_SERVER_PROPERTY, DNS_SERVER_PROPERTY_TYPES, DNS_SERVER_PROPERTY_VERSION1,
        DNS_SETTING_DOH, DNS_SETTING_IPV6, DNS_SETTING_NAMESERVER, DnsServerDohProperty,
    };
    let g = guid_of(guid)?;
    let mut ns = wide(&cfg.servers.join(","));
    // Plantillas y propiedades DoH (solo servidores del catálogo).
    let mode = effective_doh(cfg);
    let templates: Vec<(u32, Vec<u16>)> = if mode == DohMode::Off {
        Vec::new()
    } else {
        cfg.servers
            .iter()
            .enumerate()
            .filter_map(|(i, s)| Some((i as u32, wide(doh_template(&s.parse().ok()?)?))))
            .collect()
    };
    let flags = DNS_DOH_SERVER_SETTINGS_ENABLE
        | if mode == DohMode::Preferred { DNS_DOH_SERVER_SETTINGS_FALLBACK_TO_UDP } else { 0 };
    let mut templates = templates;
    let mut doh: Vec<DNS_DOH_SERVER_SETTINGS> = templates
        .iter_mut()
        .map(|(_, t)| DNS_DOH_SERVER_SETTINGS { Template: PWSTR(t.as_mut_ptr()), Flags: flags as u64 })
        .collect();
    let mut props: Vec<DNS_SERVER_PROPERTY> = templates
        .iter()
        .zip(doh.iter_mut())
        .map(|((i, _), d)| DNS_SERVER_PROPERTY {
            Version: DNS_SERVER_PROPERTY_VERSION1,
            ServerIndex: *i,
            Type: DnsServerDohProperty,
            Property: DNS_SERVER_PROPERTY_TYPES { DohSettings: d },
        })
        .collect();
    let family = if v6 { DNS_SETTING_IPV6 as u64 } else { 0 };
    let set = |with_doh: bool, props: &mut Vec<DNS_SERVER_PROPERTY>, ns: &mut Vec<u16>| {
        let s = DNS_INTERFACE_SETTINGS3 {
            Version: DNS_INTERFACE_SETTINGS_VERSION3,
            Flags: family | DNS_SETTING_NAMESERVER as u64 | if with_doh { DNS_SETTING_DOH as u64 } else { 0 },
            NameServer: PWSTR(ns.as_mut_ptr()),
            cServerProperties: if with_doh { props.len() as u32 } else { 0 },
            ServerProperties: if with_doh && !props.is_empty() { props.as_mut_ptr() } else { std::ptr::null_mut() },
            ..Default::default()
        };
        unsafe { SetInterfaceDnsSettings(g, (&s as *const DNS_INTERFACE_SETTINGS3).cast::<DNS_INTERFACE_SETTINGS>()) }.0
    };
    let mut rc = set(true, &mut props, &mut ns);
    // Si esta versión de Windows no acepta limpiar DoH sin propiedades, al menos los servidores.
    if rc == 87 && mode == DohMode::Off {
        rc = set(false, &mut props, &mut ns);
    }
    match rc {
        0 => Ok(()),
        5 => Err(t!("requires_admin")),
        code => Err(t!("dns_write_failed", code = code)),
    }
}

/// Vacía la caché de DNS de Windows para que el cambio se note enseguida.
fn flush_cache() {
    use windows::core::{s, w};
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32};
    unsafe {
        let Ok(lib) = LoadLibraryExW(w!("dnsapi.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) else { return };
        if let Some(f) = GetProcAddress(lib, s!("DnsFlushResolverCache")) {
            let flush: extern "system" fn() -> i32 = std::mem::transmute(f);
            flush();
        }
    }
}

/// Aplica una configuración ya validada. Si algo falla, deja todo como estaba.
///
/// Antes de tocar nada se comprueba que lo que hay ahora se pueda volver a escribir tal cual:
/// si no, no habría cómo deshacer (ni la vuelta atrás automática podría restaurarlo).
pub fn apply(target: &[AdapterDns]) -> Result<NetworkChange, String> {
    let mut before = Vec::new();
    for t in target {
        let b = AdapterDns {
            guid: t.guid.clone(),
            name: t.name.clone(),
            v4: read_config(&t.guid, false)?,
            v6: read_config(&t.guid, true)?,
        };
        if !restorable(&b) {
            return Err(t!("dns_unrestorable", adapter = b.name));
        }
        before.push(b);
    }
    let mut done: Vec<(&str, bool)> = Vec::new();
    for t in target {
        for (v6, cfg) in [(false, &t.v4), (true, &t.v6)] {
            if let Err(e) = write_config(&t.guid, v6, cfg) {
                for (guid, v6) in done.iter().rev() {
                    if let Some(b) = before.iter().find(|b| b.guid == *guid) {
                        let _ = write_config(guid, *v6, if *v6 { &b.v6 } else { &b.v4 });
                    }
                }
                flush_cache();
                return Err(e);
            }
            done.push((&t.guid, v6));
        }
    }
    flush_cache();
    let after = target
        .iter()
        .map(|t| AdapterDns {
            guid: t.guid.clone(),
            name: t.name.clone(),
            v4: read_config(&t.guid, false).unwrap_or_else(|_| t.v4.clone()),
            v6: read_config(&t.guid, true).unwrap_or_else(|_| t.v6.clone()),
        })
        .collect();
    Ok(NetworkChange { before, after })
}

/// Máximo de servidores al restaurar lo que había (Windows admite más que los 2 que ofrece P-Rune).
const MAX_SAVED: usize = 16;

/// Un servidor guardado para restaurar: una IP de la familia correcta. A diferencia de lo que
/// se pide desde la interfaz, acepta lo que Windows ya tenía (más de 2, link-local con zona
/// "fe80::1%12", rangos de documentación), porque restaurar es volver a lo de antes.
fn saved_server_ok(s: &str, v6: bool) -> bool {
    let ip = if v6 {
        let (addr, zone) = s.split_once('%').unwrap_or((s, ""));
        zone.chars().all(|c| c.is_ascii_digit()) && addr.parse::<Ipv6Addr>().is_ok()
    } else {
        s.parse::<Ipv4Addr>().is_ok()
    };
    ip && !s.is_empty()
}

fn restorable(a: &AdapterDns) -> bool {
    [(&a.v4, false), (&a.v6, true)]
        .iter()
        .all(|(c, v6)| c.servers.len() <= MAX_SAVED && c.servers.iter().all(|s| saved_server_ok(s, *v6)))
}

/// Para deshacer: valida lo guardado (GUID de un adaptador que existe, IPs de la familia
/// correcta) antes de volver a escribirlo.
pub fn validate_saved(list: &[AdapterDns]) -> Result<(), String> {
    let known: Vec<String> = raw_adapters().into_iter().map(|a| a.guid).collect();
    for a in list {
        if !known.iter().any(|k| k.eq_ignore_ascii_case(&a.guid)) {
            return Err(t!("dns_adapter_missing"));
        }
        if !restorable(a) {
            return Err(t!("dns_unrestorable", adapter = a.name));
        }
    }
    Ok(())
}

pub fn restore(list: &[AdapterDns]) -> Result<(), String> {
    let mut err = None;
    for a in list {
        for (v6, cfg) in [(false, &a.v4), (true, &a.v6)] {
            if let Err(e) = write_config(&a.guid, v6, cfg) {
                err.get_or_insert(e);
            }
        }
    }
    flush_cache();
    err.map_or(Ok(()), Err)
}

/// Cómo está ahora cada adaptador de la lista (para detectar cambios posteriores).
pub fn current(list: &[AdapterDns]) -> Vec<AdapterDns> {
    list.iter()
        .map(|a| AdapterDns {
            guid: a.guid.clone(),
            name: a.name.clone(),
            v4: read_config(&a.guid, false).unwrap_or_default(),
            v6: read_config(&a.guid, true).unwrap_or_default(),
        })
        .collect()
}

/// Etiqueta para el historial: "Wi-Fi: Cloudflare" o "Wi-Fi: Automático".
pub fn label(change: &NetworkChange) -> String {
    let parts: Vec<String> = change
        .after
        .iter()
        .map(|a| {
            let all: Vec<String> = a.v4.servers.iter().chain(&a.v6.servers).cloned().collect();
            let what = if all.is_empty() {
                t!("dns_auto")
            } else {
                provider_of(&all).map_or_else(|| all.join(", "), |p| p.name.to_string())
            };
            format!("{}: {what}", a.name)
        })
        .collect();
    t!("label_dns", what = parts.join(" · "))
}

// ───────────────────────────── Prueba previa ─────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct TestResult {
    pub server: String,
    pub doh: bool,
    pub ok: bool,
    pub ms: Option<u64>,
}

fn sockaddr_bytes(ip: &IpAddr, port: u16) -> [i8; 32] {
    let mut b = [0u8; 32];
    match ip {
        IpAddr::V4(a) => {
            b[0..2].copy_from_slice(&2u16.to_le_bytes());
            b[2..4].copy_from_slice(&port.to_be_bytes());
            b[4..8].copy_from_slice(&a.octets());
        }
        IpAddr::V6(a) => {
            b[0..2].copy_from_slice(&23u16.to_le_bytes());
            b[2..4].copy_from_slice(&port.to_be_bytes());
            b[8..24].copy_from_slice(&a.octets());
        }
    }
    b.map(|x| x as i8)
}

/// Una consulta real a un servidor (por UDP, o por DoH si tiene plantilla y se pide).
fn query(ip: IpAddr, doh: bool) -> bool {
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::NetworkManagement::Dns::{
        DnsFree, DnsQueryEx, DnsFreeRecordList, DNS_CUSTOM_SERVER, DNS_CUSTOM_SERVER_0, DNS_CUSTOM_SERVER_1,
        DNS_CUSTOM_SERVER_TYPE_DOH, DNS_CUSTOM_SERVER_TYPE_UDP, DNS_QUERY_BYPASS_CACHE, DNS_QUERY_NO_HOSTS_FILE,
        DNS_QUERY_REQUEST, DNS_QUERY_REQUEST3, DNS_QUERY_REQUEST_VERSION3, DNS_QUERY_RESULT, DNS_QUERY_RESULTS_VERSION1,
        DNS_TYPE_A, DNS_TYPE_AAAA,
    };
    let mut template = doh.then(|| doh_template(&ip)).flatten().map(wide);
    let mut server = DNS_CUSTOM_SERVER {
        dwServerType: if template.is_some() { DNS_CUSTOM_SERVER_TYPE_DOH } else { DNS_CUSTOM_SERVER_TYPE_UDP },
        ullFlags: 0,
        Anonymous1: DNS_CUSTOM_SERVER_0 {
            pwszTemplate: template.as_mut().map_or(PWSTR::null(), |t| PWSTR(t.as_mut_ptr())),
        },
        Anonymous2: DNS_CUSTOM_SERVER_1 { MaxSa: sockaddr_bytes(&ip, 0) },
    };
    let name = wide(TEST_NAME);
    let req = DNS_QUERY_REQUEST3 {
        Version: DNS_QUERY_REQUEST_VERSION3.0,
        QueryName: PCWSTR(name.as_ptr()),
        QueryType: if ip.is_ipv6() { DNS_TYPE_AAAA.0 } else { DNS_TYPE_A.0 },
        QueryOptions: (DNS_QUERY_BYPASS_CACHE.0 | DNS_QUERY_NO_HOSTS_FILE.0) as u64,
        cCustomServers: 1,
        pCustomServers: &mut server,
        ..Default::default()
    };
    let mut res = DNS_QUERY_RESULT { Version: DNS_QUERY_RESULTS_VERSION1.0, ..Default::default() };
    let rc = unsafe { DnsQueryEx((&req as *const DNS_QUERY_REQUEST3).cast::<DNS_QUERY_REQUEST>(), &mut res, None) };
    if !res.pQueryRecords.is_null() {
        unsafe { DnsFree(Some(res.pQueryRecords as *const _), DnsFreeRecordList) };
    }
    rc == 0 && res.QueryStatus == 0
}

/// Prueba cada servidor pedido (con un tope de tiempo por servidor).
pub fn test(target: &[AdapterDns]) -> Vec<TestResult> {
    let mut seen: Vec<(String, bool)> = Vec::new();
    for t in target {
        for cfg in [&t.v4, &t.v6] {
            let doh = effective_doh(cfg) != DohMode::Off;
            for s in &cfg.servers {
                let use_doh = doh && s.parse().ok().as_ref().and_then(doh_template).is_some();
                if !seen.contains(&(s.clone(), use_doh)) {
                    seen.push((s.clone(), use_doh));
                }
            }
        }
    }
    let handles: Vec<_> = seen
        .into_iter()
        .map(|(s, doh)| {
            let (tx, rx) = std::sync::mpsc::channel();
            let ip: Option<IpAddr> = s.parse().ok();
            std::thread::spawn(move || {
                let start = Instant::now();
                let ok = ip.is_some_and(|ip| query(ip, doh));
                let _ = tx.send((ok, start.elapsed().as_millis() as u64));
            });
            (s, doh, rx)
        })
        .collect();
    handles
        .into_iter()
        .map(|(server, doh, rx)| match rx.recv_timeout(Duration::from_secs(4)) {
            Ok((ok, ms)) => TestResult { server, doh, ok, ms: ok.then_some(ms) },
            Err(_) => TestResult { server, doh, ok: false, ms: None },
        })
        .collect()
}

// ───────────────────────────── Confirmación ─────────────────────────────

struct Pending {
    entry_id: u64,
    deadline: Instant,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

/// Un cambio de DNS a la vez: comprobar, aplicar, guardar y armar la vuelta atrás van juntos.
pub static APPLY_LOCK: Mutex<()> = Mutex::new(());

/// Deja pendiente de confirmar la entrada recién aplicada.
pub fn arm(entry_id: u64) {
    *PENDING.lock().unwrap() =
        Some(Pending { entry_id, deadline: Instant::now() + Duration::from_secs(CONFIRM_SECS) });
}

/// Saca la entrada de "pendiente" si es la que está; devuelve si lo era.
pub fn take(entry_id: u64) -> bool {
    let mut p = PENDING.lock().unwrap();
    if p.as_ref().is_some_and(|x| x.entry_id == entry_id) {
        *p = None;
        true
    } else {
        false
    }
}

pub fn has_pending() -> bool {
    PENDING.lock().unwrap().is_some()
}

fn pending_view() -> Option<PendingView> {
    PENDING.lock().unwrap().as_ref().map(|p| PendingView {
        entry_id: p.entry_id,
        seconds_left: p.deadline.saturating_duration_since(Instant::now()).as_secs(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn catalogo_valido() {
        for p in &PROVIDERS {
            validate_servers(&v(&p.v4), false).unwrap_or_else(|e| panic!("{}: {e}", p.id));
            validate_servers(&v(&p.v6), true).unwrap_or_else(|e| panic!("{}: {e}", p.id));
            assert!(p.doh.starts_with("https://") && !p.doh.contains(' '), "{}", p.id);
        }
        let mut ids: Vec<_> = PROVIDERS.iter().map(|p| p.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), PROVIDERS.len());
    }

    #[test]
    fn valida_ips() {
        assert_eq!(validate_servers(&v(&[" 1.1.1.1 ", "8.8.8.8"]), false).unwrap(), v(&["1.1.1.1", "8.8.8.8"]));
        assert_eq!(validate_servers(&v(&["2606:4700:4700:0::1111"]), true).unwrap(), v(&["2606:4700:4700::1111"]));
        assert!(validate_servers(&v(&["127.0.0.1"]), false).is_ok(), "loopback se permite (Pi-hole local)");
        for bad in ["0.0.0.0", "255.255.255.255", "224.0.0.1", "1.1.1.1:53", "dns.google", "1.1.1", "", "1.1.1.1 8.8.8.8"] {
            assert!(validate_servers(&v(&[bad]), false).is_err(), "{bad}");
        }
        for bad in ["::", "ff02::1", "fe80::1", "fe80::1%12", "::ffff:1.1.1.1"] {
            assert!(validate_servers(&v(&[bad]), true).is_err(), "{bad}");
        }
        assert!(validate_servers(&v(&["1.1.1.1"]), true).is_err(), "v4 en el campo v6");
        assert!(validate_servers(&v(&["2620:fe::fe"]), false).is_err(), "v6 en el campo v4");
        assert!(validate_servers(&v(&["1.1.1.1", "1.1.1.1"]), false).is_err(), "duplicado");
        assert!(validate_servers(&v(&["1.1.1.1", "1.0.0.1", "8.8.8.8"]), false).is_err(), "más de 2");
    }

    #[test]
    fn doh_solo_con_el_catalogo() {
        let cf = DnsConfig { servers: v(&["1.1.1.1"]), doh: DohMode::Required };
        assert_eq!(effective_doh(&cf), DohMode::Required);
        let otro = DnsConfig { servers: v(&["192.168.1.10"]), doh: DohMode::Required };
        assert_eq!(effective_doh(&otro), DohMode::Off);
        let auto = DnsConfig { servers: vec![], doh: DohMode::Preferred };
        assert_eq!(effective_doh(&auto), DohMode::Off);
        assert_eq!(doh_template(&"2620:fe::9".parse().unwrap()), Some("https://dns.quad9.net/dns-query"));
    }

    #[test]
    fn proveedor_por_servidores() {
        assert_eq!(provider_of(&v(&["1.1.1.1", "2606:4700:4700::1001"])).map(|p| p.id), Some("cloudflare"));
        assert_eq!(provider_of(&v(&["1.1.1.1", "8.8.8.8"])).map(|p| p.id), None);
        assert!(provider_of(&[]).is_none());
    }

    fn view(guid: &str) -> AdapterView {
        let f = || FamilyView { config: DnsConfig::default(), effective: vec![], provider: None };
        AdapterView { guid: guid.into(), name: "Wi-Fi".into(), description: String::new(), kind: "wifi", primary: true, v4: f(), v6: f() }
    }

    #[test]
    fn pedido_solo_sobre_adaptadores_elegibles() {
        let adapters = [view("{AAAAAAAA-0000-0000-0000-000000000001}")];
        let ok = AdapterDns {
            guid: "{aaaaaaaa-0000-0000-0000-000000000001}".into(),
            name: "lo que diga la interfaz".into(),
            v4: DnsConfig { servers: v(&["1.1.1.1"]), doh: DohMode::Preferred },
            v6: DnsConfig::default(),
        };
        let r = validate_request(std::slice::from_ref(&ok), &adapters).unwrap();
        assert_eq!(r[0].guid, "{AAAAAAAA-0000-0000-0000-000000000001}");
        assert_eq!(r[0].name, "Wi-Fi", "el nombre sale de Windows, no de la interfaz");
        let mut otro = ok.clone();
        otro.guid = "{BBBBBBBB-0000-0000-0000-000000000001}".into();
        assert!(validate_request(&[otro], &adapters).is_err());
        assert!(validate_request(&[ok.clone(), ok], &adapters).is_err(), "adaptador repetido");
        assert!(validate_request(&[], &adapters).is_err());
    }

    #[test]
    fn filtra_virtuales() {
        // Virtuales del host sin salida a internet: fuera.
        assert!(excluded("Hyper-V Virtual Ethernet Adapter", "vEthernet (WSL)", false));
        assert!(excluded("VirtualBox Host-Only Ethernet Adapter", "Ethernet 3", false));
        // Conmutador externo de Hyper-V: es la salida a internet del equipo.
        assert!(!excluded("Hyper-V Virtual Ethernet Adapter", "vEthernet (Externo)", true));
        // VPN: fuera siempre, aunque tenga puerta de enlace.
        assert!(excluded("WireGuard Tunnel", "wg0", true));
        assert!(excluded("TAP-Windows Adapter V9", "OpenVPN", true));
        // Placas físicas y la de una PC que es una máquina virtual.
        assert!(!excluded("Intel(R) Wi-Fi 6 AX201 160MHz", "Wi-Fi", true));
        assert!(!excluded("Realtek PCIe GbE Family Controller", "Ethernet", true));
        assert!(!excluded("Microsoft Hyper-V Network Adapter", "Ethernet", true));
        assert!(!excluded("vmxnet3 Ethernet Adapter", "Ethernet0", true));
    }

    #[test]
    fn sockaddr() {
        let b = sockaddr_bytes(&"1.1.1.1".parse().unwrap(), 53);
        assert_eq!(&b[..8].iter().map(|x| *x as u8).collect::<Vec<_>>(), &[2, 0, 0, 53, 1, 1, 1, 1]);
        let b6 = sockaddr_bytes(&"2620:fe::fe".parse().unwrap(), 443);
        assert_eq!(b6[0] as u8, 23);
        assert_eq!(&b6[2..4].iter().map(|x| *x as u8).collect::<Vec<_>>(), &[1, 187]);
        assert_eq!(b6[8] as u8, 0x26);
    }

    #[test]
    fn restaurar_acepta_lo_que_windows_ya_tenia() {
        let a = |v4: &[&str], v6: &[&str]| AdapterDns {
            guid: "{X}".into(),
            name: "Ethernet".into(),
            v4: DnsConfig { servers: v(v4), doh: DohMode::Off },
            v6: DnsConfig { servers: v(v6), doh: DohMode::Off },
        };
        // Más de 2 servidores, link-local con zona y documentación: Windows los acepta.
        assert!(restorable(&a(&["1.1.1.1", "8.8.8.8", "9.9.9.9"], &["fe80::1%12"])));
        assert!(restorable(&a(&["192.0.2.1"], &[])));
        assert!(restorable(&a(&[], &[])), "automático");
        // Lo que no es una IP de esa familia, no.
        assert!(!restorable(&a(&["dns.google"], &[])));
        assert!(!restorable(&a(&["2620:fe::fe"], &[])));
        assert!(!restorable(&a(&[], &["1.1.1.1"])));
        assert!(!restorable(&a(&[], &["fe80::1%eth0"])));
        let muchos: Vec<String> = (1..=17).map(|i| format!("10.0.0.{i}")).collect();
        let muchos: Vec<&str> = muchos.iter().map(String::as_str).collect();
        assert!(!restorable(&a(&muchos, &[])));
    }

    #[test]
    fn confirmacion() {
        arm(42);
        assert!(has_pending());
        assert!(!take(7));
        assert!(take(42));
        assert!(!has_pending());
    }

    /// Lectura real (no necesita admin): lista adaptadores y su DNS.
    #[test]
    #[ignore]
    fn lectura_real() {
        let s = status();
        println!("{}", serde_json::to_string_pretty(&s).unwrap());
        assert!(!s.adapters.is_empty());
    }

    /// Aplicar y restaurar de verdad en el adaptador principal (necesita admin):
    /// `cargo test network::tests::aplicar_y_restaurar -- --ignored --nocapture` en una terminal elevada.
    #[test]
    #[ignore]
    fn aplicar_y_restaurar() {
        assert!(crate::system::is_elevated(), "correr como administrador");
        let a = adapters().into_iter().find(|a| a.primary).expect("sin adaptador principal");
        let cf = &PROVIDERS[0];
        let want = AdapterDns {
            guid: a.guid.clone(),
            name: a.name.clone(),
            v4: DnsConfig { servers: v(&cf.v4), doh: DohMode::Preferred },
            v6: DnsConfig { servers: v(&cf.v6), doh: DohMode::Preferred },
        };
        let change = apply(&[want]).expect("aplicar");
        let now = current(&change.before);
        println!("antes: {:?}\nahora: {:?}", change.before, now);
        assert_eq!(now[0].v4.servers, v(&cf.v4));
        assert_eq!(now[0].v6.servers, v(&cf.v6));
        assert_eq!(now[0].v4.doh, DohMode::Preferred, "DoH no quedó activo en IPv4");
        restore(&change.before).expect("restaurar");
        assert_eq!(current(&change.before), change.before, "no volvió exactamente a como estaba");
    }

    /// Consultas reales a Cloudflare por UDP y por DoH.
    #[test]
    #[ignore]
    fn prueba_real() {
        let ip: IpAddr = "1.1.1.1".parse().unwrap();
        assert!(query(ip, false), "UDP");
        assert!(query(ip, true), "DoH");
    }
}



