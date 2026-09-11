use reovim_kernel_interface::{Error, ResourceId, driver::Lifecycle};

use crate::Runtime;

impl Runtime {
    pub(in crate::modules) fn module_has_dependants(
        &self,
        id: ResourceId,
    ) -> bool {
        self.modules.iter().any(|m| {
            (m.module.is_some() || m.factory.is_some())
                && m.dependencies
                    .iter()
                    .any(|(dependency, _)| *dependency == id)
        })
    }

    pub(crate) fn module_live(&self, id: ResourceId) -> bool {
        self.modules
            .iter()
            .any(|m| m.id == id && (m.module.is_some() || m.factory.is_some()))
    }

    pub(crate) fn retain_module(&mut self, id: ResourceId) {
        self.modules
            .iter_mut()
            .find(|m| m.id == id)
            .expect("selected module")
            .retained = true;
    }

    pub(in crate::modules) fn settle_module_ownership(&mut self) -> bool {
        // Selection appends dependencies before dependants. Reverse order
        // commits a successful root's entire graph before reclaiming orphans.
        for index in (0..self.modules.len()).rev() {
            self.retain_ready_dependencies(index);
        }
        let mut more = false;
        for index in (0..self.modules.len()).rev() {
            more |= self.close_provisional(index);
        }
        more
    }

    fn retain_ready_dependencies(&mut self, index: usize) {
        let instance = &self.modules[index];
        if !instance.retained || instance.status.get() != Lifecycle::Ready {
            return;
        }
        for dependency in 0..instance.dependencies.len() {
            let id = self.modules[index].dependencies[dependency].0;
            self.retain_module(id);
        }
    }

    fn close_provisional(&mut self, index: usize) -> bool {
        let instance = &self.modules[index];
        if instance.retained
            || instance.closing
            || (instance.module.is_none() && instance.factory.is_none())
            || self.module_has_dependants(instance.id)
        {
            return false;
        }
        let mut instance = self.modules.remove(index);
        let previous = instance.status.get();
        let result = instance.close(self);
        if let Err(error) = result {
            record_cleanup_error(&mut instance, error);
        }
        let changed = instance.closing || instance.status.get() != previous;
        self.modules.insert(index, instance);
        changed
    }
}

fn record_cleanup_error(instance: &mut super::Instance, error: Error) {
    if error != Error::BUSY {
        instance.fail(error);
    }
}
