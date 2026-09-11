//! Loaded read/write modules forward complete values under their import grants.

use alloc::{string::String, vec::Vec};
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    access::Method,
};

#[allow(clippy::future_not_send)]
/// # Errors
/// Rejects invalid arguments or unavailable granted Resource access.
pub async fn invoke(
    kernel: &Connection,
    method: Method,
    input: Message,
) -> Result<Message, RequestError> {
    let args = arguments(&input.data).map_err(RequestError::Rejected)?;
    let (path, rest) =
        args.split_first().ok_or(RequestError::Rejected(Error::INVALID))?;
    let mut resources = input.resources.to_vec();
    let subject = subject(kernel, path, &mut resources).await?;
    let (contract, data) = parameters(kernel, subject, method, rest).await?;
    let mut request = Message::new(contract, &data);
    request.resources = resources.into();
    match method {
        Method::Read => kernel.read(subject, &request).await,
        Method::Write => kernel.write(subject, &request).await,
    }
}

/// # Errors
/// Rejects invalid arguments or unavailable granted Resource access.
pub fn arguments(data: &[u8]) -> Result<Vec<&str>, Error> {
    if data.is_empty() {
        return Ok(Vec::new());
    }
    let text = core::str::from_utf8(data).map_err(|_| Error::INVALID)?;
    Ok(text.split('\0').collect())
}

#[allow(clippy::future_not_send)]
/// # Errors
/// Rejects invalid arguments or unavailable granted Resource access.
pub async fn cwd(kernel: &Connection) -> Result<String, RequestError> {
    let subject = match kernel.resolve("env/CWD").await {
        Ok(subject) => subject,
        Err(RequestError::Rejected(error)) if error == Error::UNAVAILABLE => {
            return Ok(String::new());
        }
        Err(error) => return Err(error),
    };
    let reply = kernel
        .read(subject, &Message::new(Contract::new("resource.bytes", 1), &[]))
        .await?;
    String::from_utf8(reply.data.into())
        .map_err(|_| RequestError::Failed(Error::INVALID))
}

#[allow(clippy::future_not_send)]
/// # Errors
/// Rejects invalid arguments or unavailable granted Resource access.
pub async fn subject(
    kernel: &Connection,
    path: &str,
    resources: &mut Vec<ResourceId>,
) -> Result<ResourceId, RequestError> {
    if let Some(index) = path.strip_prefix('@') {
        let index: usize = index
            .parse()
            .map_err(|_| RequestError::Rejected(Error::INVALID))?;
        if index >= resources.len() {
            return Err(RequestError::Rejected(Error::INVALID));
        }
        return Ok(resources.remove(index));
    }
    let cwd =
        if path.starts_with('/') { String::new() } else { cwd(kernel).await? };
    kernel.resolve(&crate::path(&cwd, path)).await
}

#[allow(clippy::future_not_send)]
async fn parameters(
    kernel: &Connection,
    subject: ResourceId,
    method: Method,
    args: &[&str],
) -> Result<(Contract, Vec<u8>), RequestError> {
    if args.first() == Some(&"--contract") {
        return explicit(args).map_err(RequestError::Rejected);
    }
    if method == Method::Read {
        return if args.is_empty() {
            Ok((Contract::new("resource.bytes", 1), Vec::new()))
        } else {
            Err(RequestError::Rejected(Error::INVALID))
        };
    }
    let call = crate::discover(kernel, subject).await?;
    let data = call.data(args).map_err(RequestError::Rejected)?;
    Ok((call.contract, data.into()))
}

fn explicit(args: &[&str]) -> Result<(Contract, Vec<u8>), Error> {
    let ["--contract", name, version, rest @ ..] = args else {
        return Err(Error::INVALID);
    };
    let data = match rest {
        [] => Vec::new(),
        ["--hex", value] => {
            reovim_service::unhex(value).ok_or(Error::INVALID)?
        }
        _ => return Err(Error::INVALID),
    };
    Ok((
        Contract::new(name, version.parse().map_err(|_| Error::INVALID)?),
        data,
    ))
}

/// # Errors
/// Rejects invalid arguments or unavailable granted Resource access.
pub fn optional(data: &[u8], default: &str) -> Result<String, Error> {
    let args = arguments(data)?;
    match args.as_slice() {
        [] => Ok(default.into()),
        [value] => Ok((*value).into()),
        _ => Err(Error::INVALID),
    }
}

/// # Errors
/// Rejects invalid arguments or unavailable granted Resource access.
pub fn empty(input: &Message) -> Result<(), RequestError> {
    if input.data.is_empty() && input.resources.is_empty() {
        Ok(())
    } else {
        Err(RequestError::Rejected(Error::INVALID))
    }
}
