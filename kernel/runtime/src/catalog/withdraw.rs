use alloc::rc::Rc;
use core::cell::RefCell;
use reovim_kernel_interface::Error;

use super::Catalog;
use crate::{Runtime, namespace::Kind};

impl Runtime {
    pub(super) fn withdraw_catalog(
        &self,
        key: u64,
        catalog: &Rc<RefCell<Catalog>>,
    ) -> Result<(), Error> {
        if catalog.borrow().factory.is_none() {
            // An already admitted withdrawal joins the same retired entry.
            return Ok(());
        }
        let selected = catalog.borrow().selected.as_ref().map(|(id, _)| *id);
        if selected.is_some_and(|id| self.module_live(id)) {
            return Err(Error::BUSY);
        }
        let mut shared = self.shared.borrow_mut();
        let index = shared.entry_index(key)?;
        if !matches!(&shared.entries[index].kind,
            Kind::Catalog(entry) if Rc::ptr_eq(entry, catalog))
        {
            return Err(Error::UNAVAILABLE);
        }
        let entry = shared.entries.remove(index);
        drop(shared);
        // Captured requests retain passive metadata, never the replacement.
        // Release imports/code only after all Runtime/catalog guards are gone.
        let factory = catalog.borrow_mut().factory.take();
        drop(entry);
        drop(factory);
        Ok(())
    }
}
