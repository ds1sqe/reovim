//! Delivery correlation is separate from the invoked module's contract.

use reovim_service::request::Request;
use std::io;

pub fn call(
    address: &str,
    client: &str,
    fields: &[String],
) -> io::Result<String> {
    let reserved =
        crate::exchange(address, &[client.into(), "reserve".into()])?;
    let id = reserved
        .strip_prefix("ok\t")
        .ok_or_else(|| io::Error::other("invalid request reservation"))?
        .parse::<u64>()
        .map_err(io::Error::other)?
        .to_string();
    let mut request = match Request::new(
        client,
        id.parse().map_err(io::Error::other)?,
        fields,
    ) {
        Ok(request) => request,
        Err(error) => {
            release(address, client, &id);
            return Err(io::Error::other(error));
        }
    };
    let frame = request.send().map_err(io::Error::other)?;
    let reply = crate::send_frame(address, frame).map_err(|error| {
        io::Error::other(format!(
            "outcome unknown; client={client} request={id}: {error}"
        ))
    })?;
    request.record(reply.clone()).map_err(|error| {
        io::Error::other(format!(
            "invalid outcome; client={client} request={id}: {error}"
        ))
    })?;
    if let Err(error) = request.adopt() {
        return Err(io::Error::other(format!(
            "{reply}; client={client} request={id}: {error}"
        )));
    }
    // A complete known response is adopted before its receipt is released.
    let cleanup = crate::send_frame(
        address,
        &request.release().map_err(io::Error::other)?,
    )
    .and_then(crate::accept)
    .and_then(|reply| request.released(&reply).map_err(io::Error::other));
    if let Err(error) = cleanup {
        eprintln!("receipt {id} could not be released: {error}");
    }
    crate::accept(reply)
}

fn release(address: &str, client: &str, id: &str) {
    if let Err(error) =
        crate::exchange(address, &[client.into(), "release".into(), id.into()])
    {
        eprintln!("receipt {id} could not be released: {error}");
    }
}

pub fn control(
    address: &str,
    client: &str,
    fields: &[String],
) -> io::Result<bool> {
    if fields == ["--disconnect"] {
        println!(
            "{}",
            crate::exchange(address, &[client.into(), "disconnect".into()])?
        );
        return Ok(true);
    }
    if fields == ["--requests"] {
        println!(
            "{}",
            crate::exchange(address, &[client.into(), "requests".into()])?
        );
        return Ok(true);
    }
    let [action, id] = fields else { return Ok(false) };
    let action = match action.as_str() {
        "--recover" => "result",
        "--request-status" => "status",
        "--request-info" => "input",
        "--forget" => "release",
        _ => return Ok(false),
    };
    let reply =
        crate::exchange(address, &[client.into(), action.into(), id.clone()])?;
    println!("{reply}");
    Ok(true)
}
