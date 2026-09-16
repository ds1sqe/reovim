use super::{Imported, complete_begin, packet, raw};
use core::ptr;
use reovim_kernel_interface::{
    Error, Pending, abi,
    provider::{IoInfo, IoOps, Read, ReceiveOps, SendOps, Write, WriteFn},
};

pub(super) unsafe fn operations(
    table: abi::IoOps,
) -> Result<IoOps<Imported>, Error> {
    // SAFETY: Import's caller retains readable packet Type spans until these
    // bounded copies finish; rejection transfers no native ownership.
    let receive = unsafe {
        packet::descriptor(table.receive_type, table.receive.is_some())?
    };
    // SAFETY: The send Type follows the same retained native storage contract.
    let send =
        unsafe { packet::descriptor(table.send_type, table.send.is_some())? };
    Ok(IoOps {
        receive: receive
            .map(|packet| ReceiveOps { packet, begin: packet::receive }),
        send: send.map(|packet| SendOps { packet, begin: packet::send }),
        info: IoInfo {
            read_modes: table.read_modes,
            write_modes: table.write_modes,
            alignment: table.alignment,
            transfer_limit: table.transfer_limit,
        },
        read: table.read.map(|_| read as _),
        write: table.write.map(|_| {
            (|state, request| write(state, &request)) as WriteFn<Imported>
        }),
    })
}

fn read(state: &mut Imported, request: Read) -> Result<Pending, Error> {
    if state.instance.stopped.get() {
        return Err(Error::CLOSED);
    }
    let callback = state.io.and_then(|io| io.read).ok_or(Error::UNSUPPORTED)?;
    let input = abi::Read {
        positional: u32::from(request.offset.is_some()),
        offset: request.offset.unwrap_or(0),
        maximum: request.maximum,
    };
    let mut pending =
        abi::Pending { context: ptr::null_mut(), ops: ptr::null() };
    let mut error = abi::Error::default();
    // SAFETY: Import checked the table and retains its context/code. The input
    // and initialized outputs live through this serialized call. Native read
    // must copy its borrowed request before returning pending work.
    let result = unsafe {
        callback(
            state.instance.provider.context,
            &raw const input,
            &raw mut pending,
            &raw mut error,
        )
    };
    complete_begin(state, pending, result, error, None)
}

fn write(state: &Imported, request: &Write<'_>) -> Result<Pending, Error> {
    if state.instance.stopped.get() {
        return Err(Error::CLOSED);
    }
    let callback =
        state.io.and_then(|io| io.write).ok_or(Error::UNSUPPORTED)?;
    let input = abi::Write {
        positional: u32::from(request.offset.is_some()),
        offset: request.offset.unwrap_or(0),
        data: raw::bytes(request.data),
    };
    let mut pending =
        abi::Pending { context: ptr::null_mut(), ops: ptr::null() };
    let mut error = abi::Error::default();
    // SAFETY: The checked callback and context remain under the instance's
    // code lease. Input bytes and writable outputs live through this call;
    // native write must capture borrowed bytes before returning pending work.
    let result = unsafe {
        callback(
            state.instance.provider.context,
            &raw const input,
            &raw mut pending,
            &raw mut error,
        )
    };
    complete_begin(state, pending, result, error, None)
}
