# Sección Red: DNS (IPv4/IPv6, con DoH) y proxy

Estado: etapa 1 (DNS + DoH) implementada en `src-tauri/src/network.rs`, probada como admin (aplicar y restaurar con DoH). Etapa 2 (proxy on/off) implementada en `src-tauri/src/proxy.rs` (solo cambia `INTERNET_PER_CONN_FLAGS`; lee `FLAGS_UI` y avisa con `SETTINGS_CHANGED` + `REFRESH`). Probada de verdad sin admin: alternar "Detectar automáticamente" y restaurar.

Notas de la implementación:
- Leer: del registro (`Tcpip`/`Tcpip6\...\Interfaces\{GUID}\NameServer` y `Dnscache\InterfaceSpecificParameters\{GUID}\DohInterfaceSettings\Doh(6)\<ip>\DohFlags`). `GetInterfaceDnsSettings` con `DNS_SETTING_IPV6` tira abajo el proceso (Windows 11 26300).
- Prueba previa: `DNS_CUSTOM_SERVER` exige el puerto en 0 (con 53 o 443 da error 87).
- El catálogo quedó como constante en Rust (`PROVIDERS`), no en un `.toml`.
- Filtro de adaptadores: la placa de una PC que es una VM ("Microsoft Hyper-V Network Adapter", vmxnet3) cuenta como física; los vEthernet del host solo si tienen puerta de enlace.

## Principios

- Igual que el resto de P-Rune: sin PowerShell ni `netsh`, catálogo fijo dentro del binario,
  validación en Rust, plan con diff antes de aplicar, historial con deshacer exacto, 9 idiomas,
  demo simulada en `mock.ts`.
- Nunca dejar al usuario sin internet: prueba previa + confirmación con vuelta atrás automática.
- No abrir puertas nuevas: nada de servidores DoH ni proxies cargados a mano.

## Etapa 1: DNS

### APIs (todas en `windows` 0.62, feature a sumar: `Win32_NetworkManagement_IpHelper`,
`Win32_NetworkManagement_Dns`; `NetGetJoinInformation` ya está en NetManagement)

| Para qué | API |
|---|---|
| Listar adaptadores | `GetAdaptersAddresses` (GUID, tipo, estado, gateway, nombre amigable) |
| Leer DNS actual | `GetInterfaceDnsSettings` (+ `FreeInterfaceDnsSettings`), flag `DNS_SETTING_IPV6` para v6 |
| Escribir DNS | `SetInterfaceDnsSettings`, `DNS_SETTING_NAMESERVER` (+ `DNS_SETTING_IPV6`) |
| DoH | `DNS_INTERFACE_SETTINGS3` + `DNS_SETTING_DOH`, `DNS_SERVER_PROPERTY` tipo `DnsServerDohProperty` con `DNS_DOH_SERVER_SETTINGS { Template, Flags }` (`ENABLE`, `FALLBACK_TO_UDP`) |
| Prueba previa | `DnsQueryEx` con `DNS_QUERY_REQUEST3` y `DNS_CUSTOM_SERVER` (`TYPE_UDP` o `TYPE_DOH` con la plantilla) |
| Equipo en dominio | `NetGetJoinInformation` (`NetSetupDomainName` → bloquear) |
| Vaciar caché | `DnsFlushResolverCache` (dnsapi, no está en el crate: declararla con `link!`) |
| DNS por directiva | leer `HKLM\SOFTWARE\Policies\Microsoft\Windows NT\DNSClient\NameServer` |

Vacío en `NameServer` = Automático (DHCP).

### Adaptadores

- Se muestran solo los conectados y físicos (Ethernet `IF_TYPE_ETHERNET_CSMACD`, Wi-Fi
  `IF_TYPE_IEEE80211`). Por defecto, el que tiene puerta de enlace.
- Excluidos y nunca tocados: loopback, túneles, VPN (WireGuard, TAP/OpenVPN, etc.) y virtuales
  (Hyper-V vEthernet, WSL, VirtualBox, VMware). Filtrar por tipo + descripción.
- Los comandos reciben el GUID; Rust lo valida contra la enumeración del momento.
- Aviso: el DNS queda en el adaptador (en Wi-Fi vale para todas las redes).

### Catálogo `src-tauri/catalog/dns.toml` (fijo en el binario)

| id | IPv4 | IPv6 | DoH |
|---|---|---|---|
| cloudflare | 1.1.1.1, 1.0.0.1 | 2606:4700:4700::1111, 2606:4700:4700::1001 | https://cloudflare-dns.com/dns-query |
| cloudflare_malware | 1.1.1.2, 1.0.0.2 | 2606:4700:4700::1112, 2606:4700:4700::1002 | https://security.cloudflare-dns.com/dns-query |
| google | 8.8.8.8, 8.8.4.4 | 2001:4860:4860::8888, 2001:4860:4860::8844 | https://dns.google/dns-query |
| quad9 | 9.9.9.9, 149.112.112.112 | 2620:fe::fe, 2620:fe::9 | https://dns.quad9.net/dns-query |
| adguard | 94.140.14.14, 94.140.15.15 | 2a10:50c0::ad1:ff, 2a10:50c0::ad2:ff | https://dns.adguard-dns.com/dns-query |

Cada uno con descripción traducida (qué filtra, link a su política de privacidad). Verificar
direcciones y plantillas al implementar, y test que valide el catálogo (IPs parseables,
plantillas `https://`, sin duplicados).

### Opciones por adaptador y por protocolo (IPv4 e IPv6 por separado)

- Automático (DHCP).
- De la lista: completa v4 y v6 juntos; se puede destildar uno.
- Manual: hasta 2 servidores por protocolo. Sin DoH.
- Cifrado (solo con proveedores de la lista): Sin cifrar / **Cifrado preferido** (por defecto,
  `ENABLE | FALLBACK_TO_UDP`) / Solo cifrado (avanzado, `ENABLE`).

### Validación (en Rust)

- `Ipv4Addr` / `Ipv6Addr` estrictos: sin nombres, puertos ni espacios; v4 en campo v4, v6 en v6.
- Rechazar: no especificada (`0.0.0.0`, `::`), broadcast, multicast, v6 link-local (necesita
  zona). Loopback (`127.0.0.1`, `::1`) permitido con aviso (Pi-hole local, dnscrypt).
- Máximo 2 por protocolo. Plantilla DoH solo desde el catálogo, nunca desde la interfaz.

### Para no dejar al usuario sin internet

1. Prueba previa: `DnsQueryEx` de un dominio fijo (p. ej. `www.microsoft.com`) contra cada
   servidor elegido, ~2 s; con DoH, por DoH. Si falla: aviso y no se aplica salvo "Aplicar igual".
2. Aplicar → el backend arma un temporizador de 15 s y la interfaz muestra
   "¿Sigue funcionando internet? Mantener / Volver". Sin confirmación, el backend restaura solo
   (aunque la interfaz se cuelgue). Solo una confirmación pendiente a la vez.
3. Historial: entrada nueva tipo `network` (como `power`) con el estado exacto anterior de cada
   adaptador (servidores v4/v6 o DHCP, y flags DoH). Deshacer valida el GUID y las IPs igual que
   `validate_change`, y avisa si algo cambió después.

### Bloqueos y avisos

- Equipo unido a dominio: bloqueado con explicación.
- DNS por directiva: solo lectura ("lo administra tu organización").
- VPN conectada: aviso de que la VPN manda sobre el DNS mientras esté activa.
- Portales cautivos: aviso + botón rápido "Volver a Automático".
- Control parental del router/proveedor: aviso de que un DNS propio lo esquiva.
- Requiere admin (configuración del equipo; no depende de `foreign_user`).

### Interfaz

- Barra lateral: "Red" (vista `network`), también en Ctrl K.
- Tarjeta DNS: una fila por adaptador con estado actual v4/v6, origen (DHCP / manual /
  directiva) y cifrado. Editor con proveedor, manual y modo de cifrado.
- Diálogo de confirmación con cuenta regresiva.

### Comandos Tauri

`network_status`, `dns_plan`, `dns_test`, `dns_apply`, `dns_confirm`, `dns_revert`
(sin parámetros libres más allá de GUID, id de proveedor, IPs y modo; todo validado en Rust).

### Tests

Validación de IPs y catálogo, filtro de adaptadores (con datos de ejemplo), armado de
`DNS_INTERFACE_SETTINGS3`, entrada de historial y su validación. Ignorados: prueba real contra
Cloudflare por UDP y por DoH; aplicar y restaurar en un adaptador (necesita admin).

## Etapa 2: proxy (on/off de lo que ya existe)

- Por usuario: `HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings`
  (`ProxyEnable`, `ProxyServer`, `ProxyOverride`, `AutoConfigURL`) y detección automática.
- Aplicar con `InternetSetOptionW` + `INTERNET_OPTION_PER_CONNECTION_OPTION`, después
  `INTERNET_OPTION_SETTINGS_CHANGED` y `INTERNET_OPTION_REFRESH` (que las apps abiertas se enteren).
- Tres interruptores: proxy manual, script PAC, detección automática. Al apagar se guarda el
  valor para reactivarlo tal cual; todo pasa por el historial.
- No se crean ni editan proxies.
- Avisos: proxy a `127.0.0.1:puerto` suele ser de un programa (Fiddler, antivirus, VPN).
  Proxy por directiva: solo lectura. Proxy de WinHTTP: solo se muestra.
- `foreign_user`: bloqueado como los tweaks de HKCU.
