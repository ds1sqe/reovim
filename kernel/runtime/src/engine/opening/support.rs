use super::{Implementation, ResourceProvider};
use reovim_kernel_interface::provider::{IoInfo, NamespaceInfo};

pub(super) fn compatible(
    subject: &Implementation,
    provider: &ResourceProvider,
) -> bool {
    provider.interfaces().iter().all(|api| subject.interfaces.contains(api))
        && io(subject.io, provider.io())
        && provider
            .receive_type()
            .is_none_or(|packet| subject.receive_type.as_ref() == Some(packet))
        && provider
            .send_type()
            .is_none_or(|packet| subject.send_type.as_ref() == Some(packet))
        && namespace(subject.namespace, provider.namespace())
}

const fn io(subject: Option<IoInfo>, opened: Option<IoInfo>) -> bool {
    let Some(opened) = opened else { return true };
    let Some(subject) = subject else { return false };
    opened.read_modes & !subject.read_modes == 0
        && opened.write_modes & !subject.write_modes == 0
}

const fn namespace(
    subject: Option<NamespaceInfo>,
    opened: Option<NamespaceInfo>,
) -> bool {
    let Some(opened) = opened else { return true };
    let Some(subject) = subject else { return false };
    (!opened.lookup || subject.lookup)
        && (!opened.create || subject.create)
        && (!opened.list || subject.list)
        && (!opened.materialize || subject.materialize)
        && (!opened.remove || subject.remove)
        && (!opened.link || subject.link)
        && (!opened.rename || subject.rename)
}
