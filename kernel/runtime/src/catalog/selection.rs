use alloc::{rc::Rc, vec::Vec};
use core::cell::Cell;
use reovim_kernel_interface::{Error, ResourceId, driver::Lifecycle};

use super::resolve::Selection;
use crate::Runtime;

impl Runtime {
    pub(super) fn select_graph(
        &mut self,
        graph: &[Selection],
    ) -> Result<(ResourceId, Rc<Cell<Lifecycle>>), Error> {
        let mut created = crate::reserve(graph.len())?;
        let original_entries = self.shared.borrow().entries.len();
        let result = graph.iter().try_for_each(|(_, entry)| {
            self.select_entry(graph, entry, &mut created)
        });
        if let Err(error) = result {
            self.rollback_selection(&created);
            // Selection only appends entries and executes no module code.
            // Discard the new directories too, without reusing identities.
            self.shared.borrow_mut().entries.truncate(original_entries);
            return Err(error);
        }
        let selected = graph
            .last()
            .and_then(|(_, entry)| entry.borrow().selected.clone())
            .ok_or(Error::INVALID)?;
        self.retain_module(selected.0);
        Ok(selected)
    }

    fn select_entry(
        &mut self,
        graph: &[Selection],
        entry: &Rc<core::cell::RefCell<super::Catalog>>,
        created: &mut Vec<Selection>,
    ) -> Result<(), Error> {
        if entry.borrow().selected.is_some() {
            return Ok(());
        }
        let catalog = entry.borrow();
        let factory = catalog.factory.as_ref().ok_or(Error::UNAVAILABLE)?;
        let mut dependencies = crate::reserve(factory.dependencies.len())?;
        for identity in &factory.dependencies {
            let dependency = graph
                .iter()
                .find(|(_, candidate)| candidate.borrow().identity == *identity)
                .ok_or(Error::INVALID)?;
            dependencies.push(
                dependency
                    .1
                    .borrow()
                    .selected
                    .clone()
                    .ok_or(Error::PROVIDER)?,
            );
        }
        drop(catalog);
        let key = graph
            .iter()
            .find(|(_, candidate)| Rc::ptr_eq(candidate, entry))
            .map(|(key, _)| *key)
            .ok_or(Error::INVALID)?;
        let (id, _) = self.select(&mut entry.borrow_mut(), key)?;
        self.set_dependencies(id, dependencies);
        created.push((key, Rc::clone(entry)));
        Ok(())
    }

    fn rollback_selection(&mut self, created: &[Selection]) {
        for (_, entry) in created.iter().rev() {
            let (id, _) =
                entry.borrow_mut().selected.take().expect("new selection");
            self.discard_unstarted(id);
        }
    }
}
