#![allow(clippy::future_not_send)]

use alloc::{boxed::Box, format, string::String, vec, vec::Vec};
use core::fmt::Write as _;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    namespace::ResourceEntry,
};

pub struct Node {
    pub subject: ResourceId,
    pub path: String,
    pub data: Option<Box<[u8]>>,
    pub children: Vec<ResourceEntry>,
    pub status: &'static str,
    pub detail: String,
}
struct Visit {
    subject: ResourceId,
    path: String,
    depth: usize,
}

pub async fn tree(
    kernel: &Connection,
    root: ResourceId,
) -> Result<Vec<Node>, RequestError> {
    let mut pending =
        vec![Visit { subject: root, path: String::new(), depth: 0 }];
    let mut nodes = Vec::new();
    let mut paths = 0;
    while let Some(visit) = pending.pop() {
        if nodes.len() == 64 || visit.depth > 16 {
            return Err(Error::PRESSURE.into());
        }
        let node = capture(kernel, visit.subject, visit.path).await?;
        paths += escape(&node.path).len();
        if paths > 8192 {
            return Err(Error::PRESSURE.into());
        }
        descend(&mut pending, &nodes, &node, visit.depth)?;
        nodes.push(node);
    }
    Ok(nodes)
}
fn descend(
    pending: &mut Vec<Visit>,
    nodes: &[Node],
    node: &Node,
    depth: usize,
) -> Result<(), Error> {
    if nodes.len() + pending.len() + node.children.len() > 64 {
        return Err(Error::PRESSURE);
    }
    for child in node.children.iter().rev() {
        if child.name.contains('\0') {
            return Err(Error::INVALID);
        }
        if child.resource == node.subject
            || nodes.iter().any(|n| n.subject == child.resource)
        {
            return Err(Error::INVALID);
        }
        pending.push(Visit {
            subject: child.resource,
            path: format!("{}children/{}/", node.path, child.name),
            depth: depth + 1,
        });
    }
    Ok(())
}
async fn capture(
    kernel: &Connection,
    subject: ResourceId,
    path: String,
) -> Result<Node, RequestError> {
    let children = match kernel.children(subject).await {
        Ok(children) => children,
        Err(RequestError::Rejected(Error::UNSUPPORTED)) => Vec::new(),
        Err(error) => return Err(error),
    };
    let mut node = Node {
        subject,
        path,
        data: None,
        children,
        status: "unsupported",
        detail: String::new(),
    };
    match bytes(kernel, subject).await {
        Ok(data) => {
            node.data = Some(data);
            node.status = "captured";
        }
        Err(error @ RequestError::Unknown { .. }) => return Err(error),
        Err(
            RequestError::Failed(Error::PRESSURE)
            | RequestError::Rejected(Error::PRESSURE),
        ) => {
            return Err(Error::PRESSURE.into());
        }
        Err(RequestError::Rejected(error) | RequestError::Failed(error)) => {
            if error != Error::UNSUPPORTED {
                node.status = "unavailable";
            }
            node.detail = format!("{}", error.code.0);
        }
    }
    Ok(node)
}
pub async fn bytes(
    kernel: &Connection,
    subject: ResourceId,
) -> Result<Box<[u8]>, RequestError> {
    let reply = kernel
        .read(subject, &Message::new(Contract::new("resource.bytes", 1), &[]))
        .await?;
    if reply.contract != Contract::new("resource.bytes", 1)
        || !reply.resources.is_empty()
    {
        return Err(Error::PROVIDER.into());
    }
    if reply.data.len() > 16 * 1024 {
        return Err(Error::PRESSURE.into());
    }
    Ok(reply.data)
}
pub fn escape(path: &str) -> String {
    let mut escaped = String::new();
    for character in path.chars() {
        escape_character(&mut escaped, character);
    }
    escaped
}
fn escape_character(escaped: &mut String, character: char) {
    if character != '%' && !character.is_control() {
        escaped.push(character);
        return;
    }
    let mut bytes = [0; 4];
    for byte in character.encode_utf8(&mut bytes).bytes() {
        write!(escaped, "%{byte:02X}").expect("String formatting");
    }
}
