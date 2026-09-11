#![allow(clippy::future_not_send)]

use crate::capture::{self, Node};
use alloc::{format, string::String};
use core::fmt::Write as _;
use reovim_kernel_interface::{
    Connection, Contract, Message, RequestError, ResourceId,
};

pub async fn export(
    kernel: &Connection,
    destination: ResourceId,
    name: &str,
    nodes: &mut [Node],
) -> Result<String, RequestError> {
    let created =
        write(kernel, destination, "host.export.directory", name.as_bytes(), 0)
            .await?;
    if created.0 != "written" {
        return Ok(format!("partial\n{}\t\t{}\n", created.0, created.1));
    }
    let mut stopped = false;
    for node in &mut *nodes {
        if stopped {
            node.status = "not-written";
            continue;
        }
        stopped = export_node(kernel, destination, name, node).await?;
    }
    for node in &mut *nodes {
        if matches!(node.status, "written" | "unsupported" | "unavailable") {
            validate(kernel, node).await?;
        }
    }
    Ok(report(nodes))
}
async fn export_node(
    kernel: &Connection,
    destination: ResourceId,
    name: &str,
    node: &mut Node,
) -> Result<bool, RequestError> {
    let Some(data) = &node.data else { return Ok(false) };
    let mut input = format!("{name}/{}value.bin\0", node.path).into_bytes();
    input.extend_from_slice(data);
    let (status, detail) =
        write(kernel, destination, "host.export.file", &input, data.len())
            .await?;
    node.status = status;
    node.detail = detail;
    Ok(status != "written")
}
async fn validate(
    kernel: &Connection,
    node: &mut Node,
) -> Result<(), RequestError> {
    if node.data.is_some() {
        validate_value(kernel, node).await?;
    }
    match kernel.children(node.subject).await {
        Ok(children) if children == node.children => (),
        Ok(_) => node.status = "changed",
        Err(error @ RequestError::Unknown { .. }) => return Err(error),
        Err(_) => node.status = "unverified",
    }
    Ok(())
}
async fn validate_value(
    kernel: &Connection,
    node: &mut Node,
) -> Result<(), RequestError> {
    let current = capture::bytes(kernel, node.subject).await;
    match current {
        Ok(data) if Some(&data) == node.data.as_ref() => (),
        Ok(_) => node.status = "changed",
        Err(error @ RequestError::Unknown { .. }) => return Err(error),
        Err(_) => node.status = "unverified",
    }
    Ok(())
}

async fn write(
    kernel: &Connection,
    destination: ResourceId,
    contract: &str,
    data: &[u8],
    expected: usize,
) -> Result<(&'static str, String), RequestError> {
    let result = kernel
        .write(destination, &Message::new(Contract::new(contract, 1), data))
        .await;
    let reply = match result {
        Ok(reply) => reply,
        Err(error @ RequestError::Unknown { .. }) => return Err(error),
        Err(RequestError::Failed(error) | RequestError::Rejected(error)) => {
            return Ok(("incomplete", format!("error={}", error.code.0)));
        }
    };
    if reply.contract != Contract::new(contract, 1)
        || reply.data.len() != 17
        || !reply.resources.is_empty()
    {
        return Ok(("unknown", "invalid Host result".into()));
    }
    let written = integer(&reply.data[1..9]);
    let error = integer(&reply.data[9..]);
    let status = match reply.data[0] {
        0 if written == expected as u64 && error == 0 => "written",
        1 if written <= expected as u64 && error != 0 => "incomplete",
        _ => "unknown",
    };
    Ok((status, format!("bytes={written} error={error}")))
}
fn integer(data: &[u8]) -> u64 {
    u64::from_le_bytes(data.try_into().expect("validated Host outcome"))
}
fn report(nodes: &[Node]) -> String {
    let complete = nodes.iter().all(|node| node.status == "written");
    let mut output =
        String::from(if complete { "complete\n" } else { "partial\n" });
    for node in nodes {
        writeln!(
            output,
            "{}\t{}\t{}",
            node.status,
            capture::escape(&node.path),
            node.detail
        )
        .expect("String formatting");
    }
    output
}
