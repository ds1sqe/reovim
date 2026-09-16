use reovim_kernel_interface::{
    Error, Outcome, Pending, Value,
    call::Completion,
    caller::{Call, Operation},
    declaration::ValueLimits,
    provider::{IoInfo, Read, ResourceProvider, Write},
};

use super::{access::Scope, codec, execution::Task};

pub(super) enum Transfer {
    Read(Read),
    Write { offset: Option<u64>, start: usize, length: u64 },
}

pub(super) struct Io {
    transfer: Transfer,
    pub immediate: Option<Value>,
    known: u64,
    progress: Option<Value>,
}

pub(super) fn admit(scope: &Scope, call: &Call) -> Result<Task, Error> {
    let handle = scope.live(call.subject)?;
    let info = handle.implementation.io.ok_or(Error::UNSUPPORTED)?;
    let input = codec::no_refs(&call.input)?;
    let transfer = parse(call.operation, input)?;
    let rights = match transfer {
        Transfer::Read(_) => 1,
        Transfer::Write { .. } => 2,
    };
    if handle.opened.borrow().held & rights != rights {
        return Err(Error::DENIED);
    }
    check(&transfer, input, info, scope.limits.values)?;
    let immediate = empty(&transfer)?;
    let io = Io {
        transfer,
        immediate,
        known: 0,
        progress: Some(Value::copy(&[0; 8], &[])?),
    };
    Ok(Task::io(handle.entry.clone(), io))
}

fn parse(operation: Operation, input: &[u8]) -> Result<Transfer, Error> {
    let positional =
        matches!(operation, Operation::ReadAt | Operation::WriteAt);
    let (offset, body, prefix) = if positional {
        let (offset, body) = input.split_at_checked(8).ok_or(Error::INVALID)?;
        (Some(number(offset)?), body, 8)
    } else {
        (None, input, 0)
    };
    match operation {
        Operation::Read | Operation::ReadAt => {
            Ok(Transfer::Read(Read { offset, maximum: number(body)? }))
        }
        Operation::Write | Operation::WriteAt => {
            let bytes = codec::bytes(body, usize::MAX)?;
            Ok(Transfer::Write {
                offset,
                start: prefix + 4,
                length: u64::try_from(bytes.len())
                    .map_err(|_| Error::PRESSURE)?,
            })
        }
        _ => Err(Error::UNSUPPORTED),
    }
}

fn check(
    transfer: &Transfer,
    input: &[u8],
    info: IoInfo,
    limits: ValueLimits,
) -> Result<(), Error> {
    match transfer {
        Transfer::Read(read) => {
            info.read(*read)?;
            let maximum =
                usize::try_from(read.maximum).map_err(|_| Error::PRESSURE)?;
            if maximum.checked_add(5).is_none_or(|size| size > limits.bytes) {
                return Err(Error::PRESSURE);
            }
        }
        Transfer::Write { offset, start, .. } => {
            info.write(&Write { offset: *offset, data: &input[*start..] })?;
            if limits.bytes < 8 {
                return Err(Error::PRESSURE);
            }
        }
    }
    Ok(())
}

fn empty(transfer: &Transfer) -> Result<Option<Value>, Error> {
    match transfer {
        Transfer::Read(Read { maximum: 0, .. }) => {
            Value::copy(&[0; 5], &[]).map(Some)
        }
        Transfer::Write { length: 0, .. } => {
            Value::copy(&[0; 8], &[]).map(Some)
        }
        _ => Ok(None),
    }
}

impl Io {
    pub fn begin(
        &self,
        provider: &mut ResourceProvider,
        input: &Value,
    ) -> Result<Pending, Error> {
        match self.transfer {
            Transfer::Read(read) => provider.read(read),
            Transfer::Write { offset, start, .. } => {
                provider.write(Write { offset, data: &input.data[start..] })
            }
        }
    }

    pub fn validate(
        &mut self,
        completion: &Completion,
        limits: ValueLimits,
    ) -> Result<(), Error> {
        let allowed = match self.transfer {
            Transfer::Read(_) => 1,
            Transfer::Write { .. } => 2,
        };
        if completion.activity & !allowed != 0 {
            return Err(Error::PROVIDER);
        }
        let count = match &completion.outcome {
            Outcome::Ok(value) => self.count(value, limits)?,
            Outcome::Unknown { progress: Some(value), .. } => {
                no_refs(value, limits)?;
                number(&value.data)?
            }
            Outcome::Unknown { progress: None, .. } | Outcome::Failed(_) => 0,
        };
        let limit = match self.transfer {
            Transfer::Read(read) => read.maximum,
            Transfer::Write { length, .. } => length,
        };
        if count < self.known || count > limit {
            return Err(Error::PROVIDER);
        }
        self.known = count;
        self.progress
            .as_mut()
            .unwrap()
            .data
            .copy_from_slice(&count.to_le_bytes());
        Ok(())
    }

    fn count(&self, value: &Value, limits: ValueLimits) -> Result<u64, Error> {
        no_refs(value, limits)?;
        match self.transfer {
            Transfer::Read(_) => read_count(value, limits),
            Transfer::Write { .. } => number(&value.data),
        }
    }

    pub const fn fault_progress(&mut self) -> Option<Value> {
        if self.known == 0 { None } else { self.progress.take() }
    }
}

fn no_refs(value: &Value, limits: ValueLimits) -> Result<(), Error> {
    if !value.references.is_empty() || value.data.len() > limits.bytes {
        return Err(Error::PROVIDER);
    }
    Ok(())
}

fn number(bytes: &[u8]) -> Result<u64, Error> {
    Ok(u64::from_le_bytes(bytes.try_into().map_err(|_| Error::INVALID)?))
}

fn read_count(value: &Value, limits: ValueLimits) -> Result<u64, Error> {
    let (eof, data) = value.data.split_last().ok_or(Error::INVALID)?;
    if *eof > 1 {
        return Err(Error::INVALID);
    }
    let data = codec::bytes(data, limits.bytes)?;
    u64::try_from(data.len()).map_err(|_| Error::PROVIDER)
}
