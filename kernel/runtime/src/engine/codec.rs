use reovim_kernel_interface::{Error, ResourceId, Value, resource::AccessId};

pub(super) fn name(value: &[u8], non_ascii: bool) -> Result<(), Error> {
    if value.is_empty()
        || value == b"."
        || value == b".."
        || value.len() > 4096
        || value.contains(&0)
        || value.contains(&b'/')
        || (!non_ascii && !value.is_ascii())
    {
        return Err(Error::INVALID);
    }
    Ok(())
}

pub(super) fn path(value: &[u8], non_ascii: bool) -> Result<(), Error> {
    if value == b"/" {
        return Ok(());
    }
    if !value.starts_with(b"/")
        || (!non_ascii && !value.is_ascii())
        || value.len() > 4096
        || value[1..]
            .split(|byte| *byte == b'/')
            .any(|part| part.is_empty() || part == b"." || part == b"..")
        || value.contains(&0)
    {
        return Err(Error::INVALID);
    }
    Ok(())
}

pub(super) fn no_refs(value: &Value) -> Result<&[u8], Error> {
    if !value.references.is_empty() {
        return Err(Error::INVALID);
    }
    Ok(&value.data)
}

pub(super) fn u64_value(data: &[u8]) -> Result<u64, Error> {
    Ok(u64::from_le_bytes(data.try_into().map_err(|_| Error::INVALID)?))
}

pub(super) fn u32_value(data: &[u8]) -> Result<u32, Error> {
    Ok(u32::from_le_bytes(data.try_into().map_err(|_| Error::INVALID)?))
}

pub(super) fn bytes(data: &[u8], bound: usize) -> Result<&[u8], Error> {
    let (length, rest) = data.split_at_checked(4).ok_or(Error::INVALID)?;
    let length =
        usize::try_from(u32_value(length)?).map_err(|_| Error::PRESSURE)?;
    if length > bound {
        return Err(Error::PRESSURE);
    }
    if rest.len() != length {
        return Err(Error::INVALID);
    }
    Ok(rest)
}

pub(super) fn identity(data: &[u8]) -> Result<ResourceId, Error> {
    let (epoch, serial) = data.split_at_checked(16).ok_or(Error::INVALID)?;
    let identity = ResourceId {
        epoch: epoch.try_into().map_err(|_| Error::INVALID)?,
        serial: u64::from_le_bytes(
            serial.try_into().map_err(|_| Error::INVALID)?,
        ),
    };
    if identity.serial == 0 {
        return Err(Error::INVALID);
    }
    Ok(identity)
}

pub(super) fn acquired(
    identity: ResourceId,
    token: AccessId,
    limits: reovim_kernel_interface::declaration::ValueLimits,
) -> Result<Value, Error> {
    if limits.bytes < 32 || limits.references == 0 {
        return Err(Error::PRESSURE);
    }
    let mut data = [0; 32];
    data[..16].copy_from_slice(&identity.epoch);
    data[16..24].copy_from_slice(&identity.serial.to_le_bytes());
    Value::copy(&data, &[token])
}
