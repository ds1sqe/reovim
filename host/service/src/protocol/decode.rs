use alloc::vec::Vec;
use reovim_kernel_interface::{
    Error, Outcome, ResourceId, Value,
    call::Cancellation,
    caller::{Call, Expected, Operation, Status},
    resource::AccessId,
};

use super::{Action, Frame, Reply, outcome};

pub(super) fn frame(line: &str) -> Result<Frame, Error> {
    let fields = fields(line)?;
    let ["v1", token, action, rest @ ..] = fields.as_slice() else {
        return Err(Error::INVALID);
    };
    let caller = epoch(token)?;
    let action = match (*action, rest) {
        ("reserve", [sequence]) => Action::Reserve(nonzero(sequence)?),
        ("call", rest) => Action::Call(call(rest)?),
        ("result", [id]) => Action::Result(nonzero(id)?),
        ("cancel", [id]) => Action::Cancel(nonzero(id)?),
        ("reconcile", [id]) => Action::Reconcile(nonzero(id)?),
        ("adopt", [id]) => Action::Adopt(nonzero(id)?),
        ("release", [id]) => Action::Release(nonzero(id)?),
        _ => return Err(Error::INVALID),
    };
    Ok(Frame { caller, action })
}

fn call(fields: &[&str]) -> Result<Call, Error> {
    let [
        request,
        operation,
        subject,
        command,
        epoch,
        serial,
        revision,
        data,
        refs,
    ] = fields
    else {
        return Err(Error::INVALID);
    };
    let expected = match (*epoch, *serial, *revision) {
        ("-", "0", "0") => None,
        _ => Some(Expected {
            subject: ResourceId {
                epoch: self::epoch(epoch)?,
                serial: nonzero(serial)?,
            },
            revision: number(revision)?,
        }),
    };
    let call = Call {
        request: nonzero(request)?,
        operation: Operation::try_from(u32_number(operation)?)?,
        subject: AccessId(number(subject)?),
        command: u32_number(command)?,
        expected,
        input: value(data, refs)?,
    };
    call.validate()?;
    Ok(call)
}

pub(super) fn reply(line: &str) -> Result<Reply, Error> {
    let fields = fields(line)?;
    match fields.as_slice() {
        ["reserved", id] => Ok(Reply::Reserved(nonzero(id)?)),
        ["accepted"] => Ok(Reply::Accepted),
        ["reserved"] => Ok(Reply::Status(Status::Reserved)),
        ["pending"] => Ok(Reply::Status(Status::Pending)),
        ["cancelled", "0"] => Ok(Reply::Cancelled(Cancellation::Accepted)),
        ["cancelled", "1"] => Ok(Reply::Cancelled(Cancellation::TooLate)),
        ["ok", data, refs] => Ok(outcome(Outcome::Ok(value(data, refs)?))),
        ["failed", code, epoch, serial] => {
            Ok(outcome(Outcome::Failed(error(code, epoch, serial)?)))
        }
        ["error", code, epoch, serial] => {
            Ok(Reply::Error(error(code, epoch, serial)?))
        }
        ["unknown", code, epoch, serial, present, data] => {
            unknown(code, epoch, serial, present, data)
        }
        _ => Err(Error::INVALID),
    }
}

fn unknown(
    code: &str,
    epoch: &str,
    serial: &str,
    present: &str,
    data: &str,
) -> Result<Reply, Error> {
    let progress = match (present, data) {
        ("0", "-") => None,
        ("1", data) => Some(value(data, "-")?),
        _ => return Err(Error::INVALID),
    };
    Ok(outcome(Outcome::Unknown {
        error: error(code, epoch, serial)?,
        progress,
    }))
}

fn error(code: &str, epoch: &str, serial: &str) -> Result<Error, Error> {
    let code = u32_number(code)?;
    if code == 0 {
        return Err(Error::INVALID);
    }
    match (epoch, serial) {
        ("-", "0") => Ok(Error::new(code)),
        _ => Ok(Error::with_id(
            code,
            ResourceId { epoch: self::epoch(epoch)?, serial: nonzero(serial)? },
        )),
    }
}

fn value(data: &str, refs: &str) -> Result<Value, Error> {
    let data = if data == "-" { Vec::new() } else { hex(data)? };
    let mut references = Vec::new();
    if refs != "-" {
        for token in refs.split(',') {
            references.push(AccessId(nonzero(token)?));
        }
    }
    Value::copy(&data, &references)
}

fn fields(line: &str) -> Result<Vec<&str>, Error> {
    if line.len() >= crate::MAX_LINE {
        return Err(Error::PRESSURE);
    }
    if line.contains(['\r', '\n'])
        || line.bytes().filter(|byte| *byte == b'\t').count() > 11
    {
        return Err(Error::INVALID);
    }
    Ok(line.split('\t').collect())
}

fn hex(text: &str) -> Result<Vec<u8>, Error> {
    if text.is_empty()
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(Error::INVALID);
    }
    crate::unhex(text).ok_or(Error::INVALID)
}

fn epoch(text: &str) -> Result<[u8; 16], Error> {
    if text.len() != 32 {
        return Err(Error::INVALID);
    }
    hex(text)?.try_into().map_err(|_| Error::INVALID)
}

fn number(text: &str) -> Result<u64, Error> {
    if text.is_empty()
        || !text.bytes().all(|byte| byte.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0'))
    {
        return Err(Error::INVALID);
    }
    text.parse().map_err(|_| Error::INVALID)
}

fn nonzero(text: &str) -> Result<u64, Error> {
    let value = number(text)?;
    if value == 0 { Err(Error::INVALID) } else { Ok(value) }
}

fn u32_number(text: &str) -> Result<u32, Error> {
    u32::try_from(number(text)?).map_err(|_| Error::INVALID)
}
