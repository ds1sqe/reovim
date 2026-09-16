use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use reovim_kernel_interface::{
    Error, Outcome, Value, call::Cancellation, caller::Status,
};

use super::{Action, Frame, Reply};

pub(super) fn frame(frame: &Frame) -> Result<String, Error> {
    let body = match &frame.action {
        Action::Reserve(sequence) => control("reserve", *sequence)?,
        Action::Call(call) => {
            call.validate()?;
            let expected = call.expected.map_or_else(
                || String::from("-\t0\t0"),
                |value| {
                    format!(
                        "{}\t{}\t{}",
                        crate::hex(&value.subject.epoch),
                        value.subject.serial,
                        value.revision
                    )
                },
            );
            format!(
                "call\t{}\t{}\t{}\t{}\t{}\t{}",
                call.request,
                call.operation as u32,
                call.subject.0,
                call.command,
                expected,
                value(&call.input)?
            )
        }
        Action::Result(id) => control("result", *id)?,
        Action::Cancel(id) => control("cancel", *id)?,
        Action::Reconcile(id) => control("reconcile", *id)?,
        Action::Adopt(id) => control("adopt", *id)?,
        Action::Release(id) => control("release", *id)?,
    };
    bounded(format!("v1\t{}\t{body}", crate::hex(&frame.caller)))
}

pub(super) fn reply(reply: &Reply) -> Result<String, Error> {
    let line = match reply {
        Reply::Reserved(id) => control("reserved", *id)?,
        Reply::Accepted => String::from("accepted"),
        Reply::Cancelled(Cancellation::Accepted) => {
            String::from("cancelled\t0")
        }
        Reply::Cancelled(Cancellation::TooLate) => String::from("cancelled\t1"),
        Reply::Status(Status::Reserved) => String::from("reserved"),
        Reply::Status(Status::Pending) => String::from("pending"),
        Reply::Status(Status::Outcome(outcome)) => outcome_value(outcome)?,
        Reply::Error(cause) => format!("error\t{}", error(*cause)?),
    };
    bounded(line)
}

fn outcome_value(outcome: &Outcome) -> Result<String, Error> {
    match outcome {
        Outcome::Ok(output) => Ok(format!("ok\t{}", value(output)?)),
        Outcome::Failed(cause) => Ok(format!("failed\t{}", error(*cause)?)),
        Outcome::Unknown { error: cause, progress } => {
            let progress = progress
                .as_ref()
                .map_or_else(|| Ok(String::from("0\t-")), evidence)?;
            Ok(format!("unknown\t{}\t{progress}", error(*cause)?))
        }
    }
}

fn evidence(value: &Value) -> Result<String, Error> {
    if !value.references.is_empty() {
        return Err(Error::INVALID);
    }
    Ok(format!("1\t{}", data(&value.data)?))
}

fn value(value: &Value) -> Result<String, Error> {
    if value.references.len() > crate::MAX_LINE / 2 {
        return Err(Error::PRESSURE);
    }
    if value.references.iter().any(|token| token.0 == 0) {
        return Err(Error::INVALID);
    }
    let refs = if value.references.is_empty() {
        String::from("-")
    } else {
        value
            .references
            .iter()
            .map(|token| token.0.to_string())
            .collect::<Vec<_>>()
            .join(",")
    };
    Ok(format!("{}\t{refs}", data(&value.data)?))
}

fn data(bytes: &[u8]) -> Result<String, Error> {
    if bytes.len() > crate::MAX_LINE / 2 {
        return Err(Error::PRESSURE);
    }
    Ok(if bytes.is_empty() { String::from("-") } else { crate::hex(bytes) })
}

fn error(error: Error) -> Result<String, Error> {
    if error.code.0 == 0 || error.id.is_some_and(|id| id.serial == 0) {
        return Err(Error::INVALID);
    }
    Ok(error.id.map_or_else(
        || format!("{}\t-\t0", error.code.0),
        |id| {
            format!(
                "{}\t{}\t{}",
                error.code.0,
                crate::hex(&id.epoch),
                id.serial
            )
        },
    ))
}

fn control(action: &str, id: u64) -> Result<String, Error> {
    if id == 0 {
        return Err(Error::INVALID);
    }
    Ok(format!("{action}\t{id}"))
}

fn bounded(line: String) -> Result<String, Error> {
    if line.len() >= crate::MAX_LINE { Err(Error::PRESSURE) } else { Ok(line) }
}
