use super::{Instance, ResourceId, Runtime};

pub(in crate::engine) fn claim(
    runtime: &mut Runtime,
    id: ResourceId,
    fresh: bool,
) {
    let instance = instance(runtime, id);
    instance.provisional |= fresh;
    instance.probe_uses += 1;
}
pub(in crate::engine) fn commit(runtime: &mut Runtime, id: ResourceId) {
    instance(runtime, id).provisional = false;
}
pub(in crate::engine) fn release(
    runtime: &mut Runtime,
    id: ResourceId,
) -> bool {
    let instance = instance(runtime, id);
    instance.probe_uses -= 1;
    instance.provisional && instance.probe_uses == 0
}
pub(in crate::engine) fn owned(runtime: &Runtime, id: ResourceId) -> bool {
    runtime
        .modules
        .iter()
        .find(|i| i.resource.identity == id)
        .is_some_and(|i| i.provisional && i.probe_uses == 0)
}
pub(in crate::engine) fn stopped(runtime: &Runtime, id: ResourceId) -> bool {
    runtime
        .modules
        .iter()
        .any(|instance| instance.resource.identity == id && instance.stopped())
}
fn instance(runtime: &mut Runtime, id: ResourceId) -> &mut Instance {
    runtime.modules.iter_mut().find(|i| i.resource.identity == id).unwrap()
}
