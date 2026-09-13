use alloc::{boxed::Box, format, rc::Rc, sync::Arc, vec::Vec};
use core::{cell::RefCell, sync::atomic::AtomicBool};
use reovim_kernel_interface::{
    Connection, Error, Message, ResourceId, provider::ProviderReply,
};
use std::{fs::File, os::unix::fs::MetadataExt};

struct Entry {
    name: Box<str>,
    identity: (u64, u64),
    resource: ResourceId,
    source: Rc<super::source::Source>,
}

#[derive(Clone, Default)]
pub(super) struct Selection(Rc<RefCell<Vec<Entry>>>);

struct Captured {
    file: File,
    snapshot: File,
    length: u64,
    identity: (u64, u64),
}

impl Selection {
    pub(super) fn release(
        &self,
        kernel: &Connection,
        input: Message,
    ) -> Result<Message, Error> {
        if !input.resources.is_empty() {
            return Err(Error::INVALID);
        }
        let name =
            core::str::from_utf8(&input.data).map_err(|_| Error::INVALID)?;
        let mut entries = self.0.borrow_mut();
        let index = entries
            .iter()
            .position(|entry| entry.name.as_ref() == name)
            .ok_or(Error::UNAVAILABLE)?;
        kernel.withdraw(entries[index].resource)?;
        entries.swap_remove(index);
        Ok(Message::new(input.contract, &[]))
    }

    fn install(
        &self,
        kernel: &Connection,
        parent: ResourceId,
        name: &str,
        scratch: Arc<File>,
        captured: Captured,
    ) -> Result<(ResourceId, Box<str>), Error> {
        let mut entries = self.0.borrow_mut();
        let existing = entries.iter().find(|e| e.identity == captured.identity);
        if let Some(entry) = existing {
            entry.source.select(captured.snapshot, captured.length);
            return Ok((entry.resource, entry.name.clone()));
        }
        entries.try_reserve(1).map_err(|_| Error::PRESSURE)?;
        let source = super::source::Source::new(captured.file, scratch);
        source.select(captured.snapshot, captured.length);
        let resource = kernel
            .publish(parent, name, source.clone().provider())
            .map_err(|(error, _)| error)?;
        entries.push(Entry {
            name: name.into(),
            identity: captured.identity,
            resource,
            source,
        });
        Ok((resource, name.into()))
    }
}

pub(super) fn select(
    directory: &super::directory::Directory,
    selection: Selection,
    kernel: Connection,
    parent: ResourceId,
    next: u64,
    busy: &Arc<AtomicBool>,
    input: Message,
) -> ProviderReply {
    let root = directory.root.clone();
    let scratch = directory.scratch.clone();
    let storage = scratch.clone();
    let contract = input.contract.clone();
    let work = crate::worker::start(busy, move || {
        let path =
            core::str::from_utf8(&input.data).map_err(|_| Error::INVALID)?;
        let file = crate::path::read(&root, path)
            .map_err(|error| super::directory::native(&error))?;
        capture(file, &storage)
    });
    Box::pin(async move {
        let captured = work.await?;
        let name = format!("source-{next}");
        let (resource, name) =
            selection.install(&kernel, parent, &name, scratch, captured)?;
        let mut reply = Message::reference(contract, resource);
        reply.data = name.as_bytes().into();
        Ok(reply)
    })
}

fn capture(file: File, scratch: &File) -> Result<Captured, Error> {
    let metadata =
        file.metadata().map_err(|error| super::directory::native(&error))?;
    let (snapshot, length) = super::clone(&file, scratch)
        .map_err(|error| super::directory::native(&error))?;
    Ok(Captured {
        file,
        snapshot,
        length,
        identity: (metadata.dev(), metadata.ino()),
    })
}
