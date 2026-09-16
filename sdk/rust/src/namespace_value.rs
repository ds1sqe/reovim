use alloc::{boxed::Box, vec::Vec};
use reovim_kernel_interface::{Error, Value, provider::Rename};

pub fn encode(request: Rename<'_>, maximum: usize) -> Result<Value, Error> {
    let mut data = Vec::new();
    bytes(&mut data, request.name, maximum)?;
    bytes(&mut data, request.key, maximum)?;
    append(
        &mut data,
        &u32::from(request.destination.is_some()).to_le_bytes(),
        maximum,
    )?;
    if let Some(key) = request.destination {
        bytes(&mut data, key, maximum)?;
    }
    bytes(&mut data, request.new_name, maximum)?;
    Ok(Value { data: data.into_boxed_slice(), references: Box::new([]) })
}
fn bytes(
    data: &mut Vec<u8>,
    value: &[u8],
    maximum: usize,
) -> Result<(), Error> {
    let length = u32::try_from(value.len()).map_err(|_| Error::PRESSURE)?;
    append(data, &length.to_le_bytes(), maximum)?;
    append(data, value, maximum)
}
fn append(
    data: &mut Vec<u8>,
    value: &[u8],
    maximum: usize,
) -> Result<(), Error> {
    if value.len() > maximum.saturating_sub(data.len()) {
        return Err(Error::PRESSURE);
    }
    data.try_reserve(value.len()).map_err(|_| Error::PRESSURE)?;
    data.extend_from_slice(value);
    Ok(())
}
pub fn decode(value: &Value) -> Result<Rename<'_>, Error> {
    if !value.references.is_empty() {
        return Err(Error::INVALID);
    }
    let mut data = &value.data[..];
    let name = span(&mut data)?;
    let key = span(&mut data)?;
    let destination = match integer(&mut data)? {
        0 => None,
        1 => Some(span(&mut data)?),
        _ => return Err(Error::INVALID),
    };
    let new_name = span(&mut data)?;
    if !data.is_empty() {
        return Err(Error::INVALID);
    }
    Ok(Rename { name, key, destination, new_name })
}
fn integer(data: &mut &[u8]) -> Result<u32, Error> {
    let (number, rest) = data.split_at_checked(4).ok_or(Error::INVALID)?;
    *data = rest;
    Ok(u32::from_le_bytes(number.try_into().unwrap()))
}
fn span<'a>(data: &mut &'a [u8]) -> Result<&'a [u8], Error> {
    let size = integer(data)? as usize;
    let (value, rest) = data.split_at_checked(size).ok_or(Error::INVALID)?;
    *data = rest;
    Ok(value)
}
