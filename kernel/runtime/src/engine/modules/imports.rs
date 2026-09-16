use super::{Cleanup, Lifecycle, Runtime};

pub(super) fn retire(runtime: &mut Runtime, index: usize) -> bool {
    let instance = &runtime.modules[index];
    if instance.module.is_some() || instance.parent_imports.is_none() {
        return false;
    }
    let scope = instance.imports.clone();
    if let Err(error) = crate::engine::bindings::retire(runtime, &scope) {
        runtime.modules[index].control.cleanup_failed(error);
        return false;
    }
    let instance = &mut runtime.modules[index];
    instance.parent_imports = None;
    instance.cleanup = Cleanup::Stopped;
    if instance.closing {
        instance.status.set(Lifecycle::Closed);
    }
    true
}
