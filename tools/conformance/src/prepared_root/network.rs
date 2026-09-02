//! Bounded network and Netfilter policy observation.

use std::{
    fs::File,
    io::Read as _,
    os::fd::AsRawFd as _,
    thread,
    time::{Duration, Instant},
};

use nix::{
    errno::Errno,
    fcntl::{FcntlArg, OFlag, fcntl},
    sys::socket::{
        AddressFamily, MsgFlags, NetlinkAddr, SockFlag, SockProtocol, SockType,
        bind, getsockname, recvfrom, sendto, socket,
    },
};

use crate::digest;

use super::model::{DispatchBounds, Error, NetworkObservation};

const NFT_QUERY_DOMAIN: &[u8] = b"reovim-nftables-table-set/v1\0";
const NETLINK_HEADER_BYTES: usize = 16;
const NETLINK_DONE: u16 = 3;
const NETLINK_ERROR: u16 = 2;
const NETLINK_REQUEST: u16 = 1;
const NETLINK_MULTI: u16 = 2;
const NETLINK_DUMP: u16 = 0x0300;
const NETFILTER_SUBSYSTEM_NFTABLES: u16 = 10;
const NFT_GET_TABLE: u16 = 1;
const NFT_NEW_TABLE: u16 = 0;
const QUERY_SEQUENCE: u32 = 1;
const NETLINK_SOCKET_TABLE: &str = "/proc/net/netlink";
const NETLINK_SOCKET_COLUMNS: [&str; 10] = [
    "sk", "Eth", "Pid", "Groups", "Rmem", "Wmem", "Dump", "Locks", "Drops",
    "Inode",
];
const NON_PACKET_SOCKET_TABLES: &[&str] = &[
    "/proc/net/tcp",
    "/proc/net/tcp6",
    "/proc/net/udp",
    "/proc/net/udp6",
    "/proc/net/raw",
    "/proc/net/raw6",
    "/proc/net/icmp",
    "/proc/net/icmp6",
    "/proc/net/unix",
    NETLINK_SOCKET_TABLE,
];
const PACKET_SOCKET_TABLES: &[&str] = &["/proc/net/packet"];
const KERNEL_IPV6_ROUTE_COUNT: usize = 2;

#[cfg(test)]
type SocketTablePaths = &'static [&'static str];

pub(super) fn observe(
    bounds: DispatchBounds,
) -> Result<NetworkObservation, Error> {
    let interfaces = interfaces(bounds)?;
    if interfaces.as_slice() != ["lo"] {
        return Err(Error::new(
            "PREPARED_NETWORK_INTERFACE",
            format!("network namespace interfaces are {interfaces:?}"),
        ));
    }
    let ipv4_route_count = ipv4_routes(bounds)?;
    let (ipv6_route_count, kernel_route_count) = ipv6_routes(bounds)?;
    if ipv4_route_count != 0
        || ipv6_route_count != 0
        || kernel_route_count != KERNEL_IPV6_ROUTE_COUNT
    {
        return Err(Error::new(
            "PREPARED_NETWORK_ROUTE",
            "network namespace route state differs from the kernel baseline",
        ));
    }
    let (non_packet_socket_count, packet_socket_count) = sockets(bounds)?;
    if non_packet_socket_count != 0 || packet_socket_count != 0 {
        return Err(Error::new(
            "PREPARED_NETWORK_SOCKET",
            "network namespace contains an observed socket",
        ));
    }
    let policy_observation_digest = nftables_empty(bounds)?;
    Ok(NetworkObservation {
        interfaces,
        ipv4_route_count,
        ipv6_route_count,
        kernel_route_count,
        non_packet_socket_count,
        packet_socket_count,
        policy_observation_digest,
        policy_basis: "bounded-nftables-empty-table-dump".to_owned(),
        external_carrier_absent: true,
    })
}

pub(super) fn validate_observation(
    value: &NetworkObservation,
) -> Result<(), Error> {
    let exact = value.interfaces == ["lo"]
        && value.ipv4_route_count == 0
        && value.ipv6_route_count == 0
        && value.kernel_route_count == KERNEL_IPV6_ROUTE_COUNT
        && value.non_packet_socket_count == 0
        && value.packet_socket_count == 0
        && value.policy_observation_digest == digest::bytes(NFT_QUERY_DOMAIN)
        && value.policy_basis == "bounded-nftables-empty-table-dump"
        && value.external_carrier_absent;
    if exact {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_NETWORK_OBSERVATION",
        "retained network observation is incomplete or inconsistent",
    ))
}

fn interfaces(bounds: DispatchBounds) -> Result<Vec<String>, Error> {
    let contents = read_proc("/proc/net/dev", bounds.result_bytes)?;
    let text = utf8(&contents, "PREPARED_NETWORK_DEV_UTF8")?;
    let mut values = Vec::new();
    for line in text.lines().skip(2) {
        let Some((name, _)) = line.split_once(':') else {
            return Err(Error::new(
                "PREPARED_NETWORK_DEV_FORMAT",
                "interface row lacks its colon delimiter",
            ));
        };
        let name = name.trim();
        validate_name(name, bounds)?;
        values.push(name.to_owned());
    }
    values.sort();
    values.dedup();
    Ok(values)
}

fn ipv4_routes(bounds: DispatchBounds) -> Result<usize, Error> {
    let contents = read_proc("/proc/net/route", bounds.result_bytes)?;
    let text = utf8(&contents, "PREPARED_NETWORK_ROUTE_UTF8")?;
    let mut count = 0_usize;
    for line in text.lines().skip(1) {
        let interface =
            line.split_ascii_whitespace().next().ok_or_else(|| {
                Error::new(
                    "PREPARED_NETWORK_ROUTE_FORMAT",
                    "IPv4 route row is empty",
                )
            })?;
        if interface != "lo" {
            return Err(Error::new(
                "PREPARED_NETWORK_ROUTE_EXTERNAL",
                format!("IPv4 route uses interface {interface:?}"),
            ));
        }
        count = bounded_count(count, bounds.json_items, "IPv4 routes")?;
    }
    Ok(count)
}

fn ipv6_routes(bounds: DispatchBounds) -> Result<(usize, usize), Error> {
    let contents = read_proc("/proc/net/ipv6_route", bounds.result_bytes)?;
    ipv6_route_rows(&contents, bounds)
}

fn ipv6_route_rows(
    contents: &[u8],
    bounds: DispatchBounds,
) -> Result<(usize, usize), Error> {
    let text = utf8(contents, "PREPARED_NETWORK_ROUTE6_UTF8")?;
    let mut external = 0_usize;
    let mut kernel = 0_usize;
    for line in text.lines() {
        let columns = line.split_ascii_whitespace().collect::<Vec<_>>();
        if columns.len() != 10 {
            return Err(Error::new(
                "PREPARED_NETWORK_ROUTE6_FORMAT",
                "IPv6 route row does not have ten fields",
            ));
        }
        if columns[9] != "lo" {
            return Err(Error::new(
                "PREPARED_NETWORK_ROUTE_EXTERNAL",
                format!("IPv6 route uses interface {:?}", columns[9]),
            ));
        }
        if kernel_ipv6_route(&columns) {
            kernel =
                bounded_count(kernel, bounds.json_items, "kernel IPv6 routes")?;
        } else {
            external = bounded_count(
                external,
                bounds.json_items,
                "external IPv6 routes",
            )?;
        }
    }
    Ok((external, kernel))
}

fn kernel_ipv6_route(columns: &[&str]) -> bool {
    columns
        == [
            "00000000000000000000000000000000",
            "00",
            "00000000000000000000000000000000",
            "00",
            "00000000000000000000000000000000",
            "ffffffff",
            "00000001",
            "00000000",
            "00200200",
            "lo",
        ]
}

fn sockets(bounds: DispatchBounds) -> Result<(usize, usize), Error> {
    socket_counts(bounds, read_proc)
}

fn socket_counts<Read>(
    bounds: DispatchBounds,
    mut read: Read,
) -> Result<(usize, usize), Error>
where
    Read: FnMut(&str, usize) -> Result<Vec<u8>, Error>,
{
    let mut byte_count = 0_usize;
    let mut total_count = 0_usize;
    let non_packet = socket_count(
        NON_PACKET_SOCKET_TABLES,
        bounds,
        &mut byte_count,
        &mut total_count,
        &mut read,
    )?;
    let packet = socket_count(
        PACKET_SOCKET_TABLES,
        bounds,
        &mut byte_count,
        &mut total_count,
        &mut read,
    )?;
    Ok((non_packet, packet))
}

fn socket_count<Read>(
    tables: &[&str],
    bounds: DispatchBounds,
    byte_count: &mut usize,
    total_count: &mut usize,
    read: &mut Read,
) -> Result<usize, Error>
where
    Read: FnMut(&str, usize) -> Result<Vec<u8>, Error>,
{
    let mut group_count = 0_usize;
    for path in tables {
        let contents = read(path, bounds.result_bytes)?;
        *byte_count =
            add_socket_bytes(*byte_count, contents.len(), bounds.result_bytes)?;
        let rows = socket_rows(path, &contents, bounds.json_items)?;
        group_count = add_socket_rows(group_count, rows, bounds.json_items)?;
        *total_count = add_socket_rows(*total_count, rows, bounds.json_items)?;
    }
    Ok(group_count)
}

fn socket_rows(
    path: &str,
    contents: &[u8],
    limit: usize,
) -> Result<usize, Error> {
    let text = std::str::from_utf8(contents).map_err(|failure| {
        Error::at("PREPARED_NETWORK_SOCKET_UTF8", path, failure.to_string())
    })?;
    let mut lines = text.lines();
    let header = lines.next().ok_or_else(|| socket_format(path))?;
    validate_socket_header(path, header)?;
    let mut count = 0_usize;
    for line in lines {
        if line.trim().is_empty() {
            return Err(socket_format(path));
        }
        if socket_row_is_counted(path, line)? {
            count = add_socket_rows(count, 1, limit)?;
        }
    }
    Ok(count)
}

fn validate_socket_header(path: &str, header: &str) -> Result<(), Error> {
    if path != NETLINK_SOCKET_TABLE && !header.trim().is_empty() {
        return Ok(());
    }
    let columns = header.split_ascii_whitespace().collect::<Vec<_>>();
    if columns == NETLINK_SOCKET_COLUMNS {
        return Ok(());
    }
    Err(socket_format(path))
}

fn socket_row_is_counted(path: &str, row: &str) -> Result<bool, Error> {
    if path != NETLINK_SOCKET_TABLE {
        return Ok(true);
    }
    netlink_row_is_userspace(path, row)
}

fn netlink_row_is_userspace(path: &str, row: &str) -> Result<bool, Error> {
    let columns = row.split_ascii_whitespace().collect::<Vec<_>>();
    if columns.len() != NETLINK_SOCKET_COLUMNS.len() {
        return Err(socket_format(path));
    }
    parse_socket_column(path, columns[0], 16)?;
    parse_socket_column(path, columns[1], 10)?;
    let pid = parse_socket_column(path, columns[2], 10)?;
    parse_socket_column(path, columns[3], 16)?;
    for column in &columns[4..] {
        parse_socket_column(path, column, 10)?;
    }

    // PID 0 is a kernel control endpoint present in a fresh network
    // namespace, not a user or external-carrier socket.
    Ok(pid != 0)
}

fn parse_socket_column(
    path: &str,
    value: &str,
    radix: u32,
) -> Result<u64, Error> {
    u64::from_str_radix(value, radix).map_err(|_| socket_format(path))
}

fn add_socket_bytes(
    total: usize,
    add: usize,
    limit: usize,
) -> Result<usize, Error> {
    let total = total.checked_add(add).ok_or_else(|| {
        Error::new(
            "PREPARED_NETWORK_SOCKET_BYTE_OVERFLOW",
            "socket observation byte count overflow",
        )
    })?;
    if total <= limit {
        return Ok(total);
    }
    Err(Error::new(
        "PREPARED_NETWORK_SOCKET_BYTE_LIMIT",
        "socket observations exceed the aggregate byte limit",
    ))
}

fn add_socket_rows(
    total: usize,
    add: usize,
    limit: usize,
) -> Result<usize, Error> {
    let total = total.checked_add(add).ok_or_else(|| {
        Error::new("PREPARED_NETWORK_SOCKET_OVERFLOW", "socket count overflow")
    })?;
    if total <= limit {
        return Ok(total);
    }
    Err(Error::new(
        "PREPARED_NETWORK_SOCKET_LIMIT",
        "socket count exceeds the observation limit",
    ))
}

fn socket_format(path: &str) -> Error {
    Error::at(
        "PREPARED_NETWORK_SOCKET_FORMAT",
        path,
        "socket table is empty or contains an empty row",
    )
}

fn validate_name(name: &str, bounds: DispatchBounds) -> Result<(), Error> {
    let valid = !name.is_empty()
        && name.len() <= 64
        && name.chars().all(|value| {
            value.is_ascii_alphanumeric() || matches!(value, '_' | '-' | '.')
        });
    if valid && name.len() <= bounds.result_bytes {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_NETWORK_INTERFACE_FORMAT",
        format!("invalid interface name {name:?}"),
    ))
}

fn nftables_empty(bounds: DispatchBounds) -> Result<String, Error> {
    let socket = socket(
        AddressFamily::Netlink,
        SockType::Raw,
        SockFlag::SOCK_CLOEXEC,
        SockProtocol::NetlinkNetFilter,
    )
    .map_err(netlink_unavailable)?;
    bind(socket.as_raw_fd(), &NetlinkAddr::new(0, 0))
        .map_err(netlink_unavailable)?;
    let local = getsockname::<NetlinkAddr>(socket.as_raw_fd())
        .map_err(netlink_unavailable)?;
    if local.pid() == 0 || local.groups() != 0 {
        return Err(Error::new(
            "PREPARED_NETWORK_POLICY_LOCAL_IDENTITY",
            "Netfilter socket lacks one unicast userspace port ID",
        ));
    }
    set_nonblocking(&socket)?;
    let request = nft_table_request();
    let sent = sendto(
        socket.as_raw_fd(),
        &request,
        &NetlinkAddr::new(0, 0),
        MsgFlags::empty(),
    )
    .map_err(netlink_unavailable)?;
    if sent != request.len() {
        return Err(Error::new(
            "PREPARED_NETWORK_POLICY_SEND",
            "Netfilter query was partially sent",
        ));
    }
    receive_nft_dump(socket.as_raw_fd(), local.pid(), bounds)?;
    Ok(digest::bytes(NFT_QUERY_DOMAIN))
}

fn nft_table_request() -> [u8; 20] {
    let mut request = [0_u8; 20];
    request[0..4].copy_from_slice(&20_u32.to_ne_bytes());
    let kind = (NETFILTER_SUBSYSTEM_NFTABLES << 8) | NFT_GET_TABLE;
    request[4..6].copy_from_slice(&kind.to_ne_bytes());
    let flags = NETLINK_REQUEST | NETLINK_DUMP;
    request[6..8].copy_from_slice(&flags.to_ne_bytes());
    request[8..12].copy_from_slice(&QUERY_SEQUENCE.to_ne_bytes());
    request
}

fn receive_nft_dump(
    fd: i32,
    local_port: u32,
    bounds: DispatchBounds,
) -> Result<(), Error> {
    let started = Instant::now();
    let mut bytes = 0_usize;
    let mut messages = 0_usize;
    let mut buffer = vec![0_u8; bounds.result_bytes.min(64 * 1024)];
    if buffer.len() < NETLINK_HEADER_BYTES {
        return Err(Error::new(
            "PREPARED_NETWORK_POLICY_LIMIT",
            "result byte bound is too small for Netlink",
        ));
    }
    loop {
        if started.elapsed() >= bounds.execution_deadline {
            return Err(Error::new(
                "PREPARED_NETWORK_POLICY_DEADLINE",
                "Netfilter query exceeded its deadline",
            ));
        }
        let outcome = receive_nft_step(
            fd,
            &mut buffer,
            &mut bytes,
            &mut messages,
            local_port,
            bounds,
        )?;
        if outcome == Some(DumpOutcome::Done) {
            return Ok(());
        }
    }
}

fn receive_nft_step(
    fd: i32,
    buffer: &mut [u8],
    bytes: &mut usize,
    messages: &mut usize,
    local_port: u32,
    bounds: DispatchBounds,
) -> Result<Option<DumpOutcome>, Error> {
    let received = match recvfrom::<NetlinkAddr>(fd, buffer) {
        Ok(value) => value,
        Err(Errno::EAGAIN) => {
            thread::sleep(Duration::from_millis(1));
            return Ok(None);
        }
        Err(failure) => return Err(netlink_unavailable(failure)),
    };
    let (count, address) = received;
    validate_kernel_address(address)?;
    if count == buffer.len() {
        return Err(Error::new(
            "PREPARED_NETWORK_POLICY_TRUNCATED",
            "Netfilter datagram reached the receive buffer bound",
        ));
    }
    *bytes = add_bounded(*bytes, count, bounds.result_bytes)?;
    parse_datagram(&buffer[..count], messages, local_port, bounds.json_items)
        .map(Some)
}

fn validate_kernel_address(address: Option<NetlinkAddr>) -> Result<(), Error> {
    let address = address.ok_or_else(|| {
        Error::new(
            "PREPARED_NETWORK_POLICY_ADDRESS",
            "Netfilter response lacks a Netlink address",
        )
    })?;
    if address.pid() == 0 && address.groups() == 0 {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_NETWORK_POLICY_ADDRESS",
        "Netfilter response did not originate from the kernel",
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DumpOutcome {
    Continue,
    Done,
}

fn parse_datagram(
    bytes: &[u8],
    messages: &mut usize,
    local_port: u32,
    message_limit: usize,
) -> Result<DumpOutcome, Error> {
    let mut cursor = 0_usize;
    let mut outcome = DumpOutcome::Continue;
    while cursor < bytes.len() {
        let message = message_at(bytes, cursor)?;
        *messages =
            bounded_count(*messages, message_limit, "Netlink messages")?;
        outcome = parse_message(message, local_port)?;
        cursor = next_message(cursor, message.len(), bytes.len())?;
        if outcome == DumpOutcome::Done && cursor != bytes.len() {
            return Err(Error::new(
                "PREPARED_NETWORK_POLICY_TRAILING",
                "Netlink dump has messages after NLMSG_DONE",
            ));
        }
    }
    Ok(outcome)
}

fn message_at(bytes: &[u8], cursor: usize) -> Result<&[u8], Error> {
    let header = bytes.get(cursor..).ok_or_else(policy_framing)?;
    if header.len() < NETLINK_HEADER_BYTES {
        return Err(policy_framing());
    }
    let length = u32::from_ne_bytes(header[0..4].try_into().unwrap());
    let length = usize::try_from(length).map_err(|_| policy_framing())?;
    if length < NETLINK_HEADER_BYTES || length > header.len() {
        return Err(policy_framing());
    }
    Ok(&header[..length])
}

fn next_message(
    cursor: usize,
    length: usize,
    total: usize,
) -> Result<usize, Error> {
    let aligned = length
        .checked_add(3)
        .map(|value| value & !3)
        .ok_or_else(policy_framing)?;
    let next = cursor.checked_add(aligned).ok_or_else(policy_framing)?;
    if next <= total {
        return Ok(next);
    }
    Err(policy_framing())
}

fn parse_message(
    message: &[u8],
    local_port: u32,
) -> Result<DumpOutcome, Error> {
    let kind = u16::from_ne_bytes(message[4..6].try_into().unwrap());
    let flags = u16::from_ne_bytes(message[6..8].try_into().unwrap());
    let sequence = u32::from_ne_bytes(message[8..12].try_into().unwrap());
    let sender = u32::from_ne_bytes(message[12..16].try_into().unwrap());
    if sequence != QUERY_SEQUENCE || sender != local_port {
        return Err(Error::new(
            "PREPARED_NETWORK_POLICY_IDENTITY",
            "Netlink response sequence or destination port differs",
        ));
    }
    if kind == NETLINK_DONE {
        if flags & NETLINK_MULTI == 0 {
            return Err(Error::new(
                "PREPARED_NETWORK_POLICY_FLAGS",
                "Netfilter dump terminator lacks NLM_F_MULTI",
            ));
        }
        validate_done_payload(&message[NETLINK_HEADER_BYTES..])?;
        return Ok(DumpOutcome::Done);
    }
    if kind == NETLINK_ERROR {
        return Err(netlink_error(&message[NETLINK_HEADER_BYTES..]));
    }
    if flags & NETLINK_MULTI == 0 {
        return Err(Error::new(
            "PREPARED_NETWORK_POLICY_FLAGS",
            "Netfilter dump response lacks NLM_F_MULTI",
        ));
    }
    let table = (NETFILTER_SUBSYSTEM_NFTABLES << 8) | NFT_NEW_TABLE;
    if kind == table {
        return Err(Error::new(
            "PREPARED_NETWORK_POLICY_NONEMPTY",
            "network namespace contains an nftables table",
        ));
    }
    Err(Error::new(
        "PREPARED_NETWORK_POLICY_MESSAGE",
        format!("unexpected Netfilter message type {kind}"),
    ))
}

fn validate_done_payload(payload: &[u8]) -> Result<(), Error> {
    if payload.is_empty()
        || (payload.len() == 4
            && i32::from_ne_bytes(payload.try_into().unwrap()) == 0)
    {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_NETWORK_POLICY_DONE",
        "NLMSG_DONE contains a nonzero or malformed status",
    ))
}

fn netlink_error(payload: &[u8]) -> Error {
    if payload.len() < 4 {
        return policy_framing();
    }
    let code = i32::from_ne_bytes(payload[..4].try_into().unwrap());
    Error::new(
        "PREPARED_NETWORK_POLICY_ERROR",
        format!("Netfilter returned NLMSG_ERROR {code}"),
    )
}

fn policy_framing() -> Error {
    Error::new(
        "PREPARED_NETWORK_POLICY_FRAMING",
        "malformed or truncated Netfilter Netlink message",
    )
}

fn netlink_unavailable(failure: Errno) -> Error {
    Error::new("PREPARED_NETWORK_POLICY_UNAVAILABLE", failure.to_string())
}

fn set_nonblocking<Fd: std::os::fd::AsFd>(fd: &Fd) -> Result<(), Error> {
    let current = fcntl(fd, FcntlArg::F_GETFL).map_err(netlink_unavailable)?;
    let flags = nonblocking_flags(current);
    fcntl(fd, FcntlArg::F_SETFL(flags)).map(drop).map_err(netlink_unavailable)
}

fn nonblocking_flags(current: i32) -> OFlag {
    OFlag::from_bits_truncate(current) | OFlag::O_NONBLOCK
}

fn add_bounded(total: usize, add: usize, limit: usize) -> Result<usize, Error> {
    let total = total.checked_add(add).ok_or_else(|| {
        Error::new(
            "PREPARED_NETWORK_POLICY_BYTE_OVERFLOW",
            "Netfilter response byte count overflow",
        )
    })?;
    if total <= limit {
        return Ok(total);
    }
    Err(Error::new(
        "PREPARED_NETWORK_POLICY_BYTE_LIMIT",
        "Netfilter response exceeds its byte limit",
    ))
}

fn bounded_count(
    current: usize,
    limit: usize,
    subject: &str,
) -> Result<usize, Error> {
    let next = current.checked_add(1).ok_or_else(|| {
        Error::new(
            "PREPARED_NETWORK_COUNT_OVERFLOW",
            format!("{subject} count overflow"),
        )
    })?;
    if next <= limit {
        return Ok(next);
    }
    Err(Error::new(
        "PREPARED_NETWORK_COUNT_LIMIT",
        format!("{subject} exceed {limit}"),
    ))
}

fn read_proc(path: &str, limit: usize) -> Result<Vec<u8>, Error> {
    let file = File::open(path).map_err(|failure| {
        Error::at("PREPARED_PROC_OPEN", path, failure.to_string())
    })?;
    let take = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
    let mut bytes = Vec::new();
    file.take(take).read_to_end(&mut bytes).map_err(|failure| {
        Error::at("PREPARED_PROC_READ", path, failure.to_string())
    })?;
    if bytes.len() <= limit {
        return Ok(bytes);
    }
    Err(Error::at(
        "PREPARED_PROC_BYTE_LIMIT",
        path,
        format!("procfs input exceeds {limit} bytes"),
    ))
}

fn utf8<'a>(bytes: &'a [u8], code: &'static str) -> Result<&'a str, Error> {
    std::str::from_utf8(bytes)
        .map_err(|failure| Error::new(code, failure.to_string()))
}

#[cfg(test)]
pub(super) fn policy_request_for_test() -> [u8; 20] {
    nft_table_request()
}

#[cfg(test)]
pub(super) fn parse_policy_for_test(bytes: &[u8]) -> Result<bool, Error> {
    let mut messages = 0;
    parse_datagram(bytes, &mut messages, 7, 16)
        .map(|value| value == DumpOutcome::Done)
}

#[cfg(test)]
pub(super) fn ipv6_routes_for_test(
    bytes: &[u8],
    bounds: DispatchBounds,
) -> Result<(usize, usize), Error> {
    ipv6_route_rows(bytes, bounds)
}

#[cfg(test)]
pub(super) const fn non_packet_socket_tables_for_test() -> SocketTablePaths {
    NON_PACKET_SOCKET_TABLES
}

#[cfg(test)]
pub(super) const fn packet_socket_tables_for_test() -> SocketTablePaths {
    PACKET_SOCKET_TABLES
}

#[cfg(test)]
pub(super) fn socket_counts_for_test(
    bounds: DispatchBounds,
) -> Result<(usize, usize), Error> {
    let read = |path: &str, _: usize| Ok(socket_fixture(path));
    socket_counts(bounds, read)
}

#[cfg(test)]
fn socket_fixture(path: &str) -> Vec<u8> {
    if path == NETLINK_SOCKET_TABLE {
        return concat!(
            "sk Eth Pid Groups Rmem Wmem Dump Locks Drops Inode\n",
            "0000000000000001 0 7 00000000 0 0 0 2 0 1\n",
        )
        .as_bytes()
        .to_vec();
    }
    b"header\nrow\n".to_vec()
}

#[cfg(test)]
pub(super) fn socket_rows_for_test(
    path: &str,
    contents: &[u8],
    limit: usize,
) -> Result<usize, Error> {
    socket_rows(path, contents, limit)
}

#[cfg(test)]
pub(super) fn nonblocking_flags_for_test(current: i32) -> OFlag {
    nonblocking_flags(current)
}
