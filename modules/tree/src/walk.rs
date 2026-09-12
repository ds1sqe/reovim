#![allow(clippy::future_not_send)]

use alloc::{format, string::String, vec, vec::Vec};
use core::fmt::Write as _;
use reovim_kernel_interface::{Connection, Error, RequestError, ResourceId};

const NODES: usize = 256;
const DEPTH: usize = 16;
const BYTES: usize = 16 * 1024;

struct Visit {
    resource: ResourceId,
    label: String,
    indent: String,
    branch: &'static str,
    last: bool,
    ancestors: Vec<ResourceId>,
}

pub async fn tree(
    kernel: &Connection,
    root: ResourceId,
    label: &str,
) -> Result<String, RequestError> {
    let mut pending = vec![Visit {
        resource: root,
        label: label.into(),
        indent: String::new(),
        branch: "",
        last: true,
        ancestors: Vec::new(),
    }];
    let mut output = String::new();
    let mut visited = 0;
    while let Some(visit) = pending.pop() {
        let line = format!(
            "{}{}{}\n",
            visit.indent,
            visit.branch,
            visit.label.escape_debug()
        );
        if visited == NODES || output.len() + line.len() > BYTES - 256 {
            output.push_str("... [tree limit reached]\n");
            break;
        }
        output.push_str(&line);
        visited += 1;
        descend(kernel, visit, &mut pending, &mut output).await?;
    }
    Ok(output)
}

async fn descend(
    kernel: &Connection,
    visit: Visit,
    pending: &mut Vec<Visit>,
    output: &mut String,
) -> Result<(), RequestError> {
    if visit.ancestors.contains(&visit.resource) {
        note(output, &visit, "[cycle; not expanded]");
        return Ok(());
    }
    let children = match kernel.children(visit.resource).await {
        Ok(children) => children,
        Err(
            RequestError::Rejected(Error::UNSUPPORTED)
            | RequestError::Failed(Error::UNSUPPORTED),
        ) => return Ok(()),
        Err(error @ RequestError::Unknown { .. }) => return Err(error),
        Err(RequestError::Rejected(error) | RequestError::Failed(error)) => {
            note(
                output,
                &visit,
                &format!("[children unavailable: error {}]", error.code.0),
            );
            return Ok(());
        }
    };
    if children.is_empty() {
        return Ok(());
    }
    if visit.ancestors.len() == DEPTH || pending.len() + children.len() > NODES
    {
        note(output, &visit, "[tree limit reached; children not expanded]");
        return Ok(());
    }
    let indent = child_indent(&visit);
    let mut ancestors = visit.ancestors;
    ancestors.push(visit.resource);
    for (index, child) in children.into_iter().rev().enumerate() {
        pending.push(Visit {
            resource: child.resource,
            label: child.name.into(),
            indent: indent.clone(),
            branch: if index == 0 { "└── " } else { "├── " },
            last: index == 0,
            ancestors: ancestors.clone(),
        });
    }
    Ok(())
}

fn child_indent(visit: &Visit) -> String {
    if visit.branch.is_empty() {
        return String::new();
    }
    format!("{}{}", visit.indent, if visit.last { "    " } else { "│   " })
}

fn note(output: &mut String, visit: &Visit, text: &str) {
    writeln!(output, "{}└── {text}", child_indent(visit))
        .expect("String formatting");
}
