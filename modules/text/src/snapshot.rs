#![allow(clippy::future_not_send)]
use crate::{api, scan::Scan};
use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::cell::Cell;
use core::ops::Range;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    buffer::{self, RetainedState},
    provider::{Permissions, ResourceOps, ResourceProvider, Surface},
};

pub struct Snapshot {
    pub kernel: Connection,
    pub source: RetainedState,
    pub length: u64,
    pub id: Cell<ResourceId>,
    pub closed: Cell<bool>,
    pub active: Cell<usize>,
}

pub struct Use(Rc<Snapshot>);
impl Drop for Use {
    fn drop(&mut self) {
        self.0.active.set(self.0.active.get() - 1);
    }
}

impl Snapshot {
    pub fn acquire(self: &Rc<Self>) -> Result<Use, RequestError> {
        if self.closed.get() {
            return Err(Error::CLOSED.into());
        }
        if self.active.get() == 32 {
            return Err(Error::PRESSURE.into());
        }
        self.active.set(self.active.get() + 1);
        Ok(Use(self.clone()))
    }

    pub fn close(&self) -> Result<(), Error> {
        self.closed.set(true);
        if self.active.get() != 0 {
            return Err(Error::BUSY);
        }
        self.kernel
            .close(self.source.id)
            .into_inner()
            .map_err(crate::view::cause)
    }

    pub async fn range(
        &self,
        wanted: Range<u64>,
    ) -> Result<Range<u64>, RequestError> {
        if wanted.start > wanted.end {
            return Err(Error::INVALID.into());
        }
        let mut scan = Scan::new(&self.kernel, &self.source, self.length);
        for _ in 0..wanted.start {
            scan.next().await?.ok_or(Error::INVALID)?;
        }
        let start = scan.offset;
        for _ in wanted {
            scan.next().await?.ok_or(Error::INVALID)?;
        }
        Ok(start..scan.offset)
    }

    async fn window(&self, input: Message) -> Result<Message, RequestError> {
        if input.data.len() != 16 {
            return Err(Error::INVALID.into());
        }
        let start = buffer::integer(&input.data[..8])?;
        let count = buffer::integer(&input.data[8..])?;
        if count > 1024 {
            return Err(Error::PRESSURE.into());
        }
        let mut scan = Scan::new(&self.kernel, &self.source, self.length);
        for _ in 0..start {
            scan.next().await?.ok_or(Error::INVALID)?;
        }
        let (units, consumed) = collect(&mut scan, count).await?;
        let end = start.checked_add(consumed).ok_or(Error::PRESSURE)?;
        let mut data = Vec::new();
        data.push(u8::from(scan.offset == self.length));
        data.extend_from_slice(&start.to_le_bytes());
        data.extend_from_slice(&end.to_le_bytes());
        data.extend_from_slice(&units);
        Ok(Message {
            contract: input.contract,
            data: data.into(),
            resources: Box::new([]),
        })
    }
}

async fn collect(
    scan: &mut Scan<'_>,
    count: u64,
) -> Result<(Vec<u8>, u64), RequestError> {
    let mut units = Vec::new();
    let mut consumed = 0;
    for _ in 0..count {
        let Some(unit) = scan.next().await? else {
            break;
        };
        api::encode(unit, &mut units);
        consumed += 1;
    }
    Ok((units, consumed))
}

pub fn provider(snapshot: Rc<Snapshot>) -> ResourceProvider {
    let closing = snapshot.clone();
    ResourceProvider::new(
        snapshot,
        ResourceOps {
            unload: None,
            read: |snapshot, _, input| {
                let active = snapshot.acquire();
                let snapshot = snapshot.clone();
                Box::pin(async move {
                    let _active = active?;
                    snapshot.window(input).await
                })
            },
            write: |_, _, _| Box::pin(async { Err(Error::UNSUPPORTED.into()) }),
        },
        Box::new([Surface {
            contract: Contract::new("text.window", 1),
            methods: Permissions::READ,
            input: Box::new([]),
            output: Box::new([]),
        }]),
        Rc::new(()),
    )
    .try_owned(move || closing.close())
}
