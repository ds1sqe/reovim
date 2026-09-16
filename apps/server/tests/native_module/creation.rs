use super::*;
use reovim_kernel_interface::caller::{Call, Status};
use reovim_kernel_runtime::PublicationGrant;
use reovim_server::clients::Clients;
use reovim_service::protocol::{Action, Frame, Reply};

#[path = "opened.rs"]
mod opened;

struct Peer {
    clients: Clients,
    sequence: u64,
}
impl Peer {
    fn send(&self, action: Action) -> Reply {
        let frame = Frame { caller: [0xa7; 16], action };
        let frame = Frame::decode(&frame.encode().unwrap()).unwrap();
        let reply = self.clients.dispatch(frame);
        Reply::decode(&reply.encode().unwrap()).unwrap()
    }
    fn call(
        &mut self,
        runtime: &mut Runtime,
        operation: Operation,
        subject: AccessId,
        command: u32,
        data: &[u8],
    ) -> Value {
        self.sequence += 1;
        let Reply::Reserved(request) =
            self.send(Action::Reserve(self.sequence))
        else {
            panic!("reservation rejected")
        };
        assert_eq!(
            self.send(Action::Call(Call {
                request,
                operation,
                subject,
                command,
                expected: None,
                input: Value::copy(data, &[]).unwrap(),
            })),
            Reply::Accepted
        );
        completed(self, runtime, request)
    }

    fn adopt(&self, request: u64, outcome: &Outcome) -> Value {
        let Outcome::Ok(value) = outcome else { panic!("{outcome:?}") };
        assert_eq!(self.send(Action::Adopt(request)), Reply::Accepted);
        assert_eq!(self.send(Action::Release(request)), Reply::Accepted);
        value.clone()
    }
    fn open(&mut self, runtime: &mut Runtime, path: &[u8]) -> Value {
        let mut data = 0_u32.to_le_bytes().to_vec();
        data.extend_from_slice(
            &u32::try_from(path.len()).unwrap().to_le_bytes(),
        );
        data.extend_from_slice(path);
        self.call(runtime, Operation::Acquire, AccessId(0), 0, &data)
    }
}
fn completed(peer: &Peer, runtime: &mut Runtime, request: u64) -> Value {
    for _ in 0..20 {
        runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
        let Reply::Status(Status::Outcome(outcome)) =
            peer.send(Action::Result(request))
        else {
            continue;
        };
        if let Some(value) = settled(peer, request, &outcome) {
            return value;
        }
    }
    panic!("native creation service request did not settle")
}

fn settled(peer: &Peer, request: u64, outcome: &Outcome) -> Option<Value> {
    if matches!(outcome, Outcome::Unknown { .. }) {
        assert_eq!(peer.send(Action::Reconcile(request)), Reply::Accepted);
        return None;
    }
    Some(peer.adopt(request, outcome))
}
fn bind(peer: &mut Peer, runtime: &mut Runtime, access: AccessId) -> AccessId {
    let mut page = 0_u64.to_le_bytes().to_vec();
    page.extend_from_slice(&4096_u64.to_le_bytes());
    let offered = peer.call(runtime, Operation::Interfaces, access, 0, &page);
    let length =
        u32::from_le_bytes(offered.data[4..8].try_into().unwrap()) as usize;
    peer.call(runtime, Operation::Bind, access, 0, &offered.data[4..8 + length])
        .references[0]
}
fn lifetime(
    peer: &mut Peer,
    runtime: &mut Runtime,
    access: AccessId,
    parent: AccessId,
) {
    let bound = bind(peer, runtime, access);
    peer.call(runtime, Operation::SetAccess, bound, 0, &7_u32.to_le_bytes());
    let written =
        peer.call(runtime, Operation::Exec, bound, 0, &29_u32.to_le_bytes());
    assert_eq!(
        u64::from_le_bytes(written.data.as_ref().try_into().unwrap()),
        29
    );
    peer.call(runtime, Operation::Close, bound, 0, &[]);
    let reopened = peer.open(runtime, b"/mount/directory/new").references[0];
    let bound = bind(peer, runtime, reopened);
    peer.call(runtime, Operation::SetAccess, bound, 0, &1_u32.to_le_bytes());
    let read = peer.call(runtime, Operation::Ioctl, bound, 1, &[]);
    assert_eq!(u64::from_le_bytes(read.data.as_ref().try_into().unwrap()), 29);
    let mut name = 3_u32.to_le_bytes().to_vec();
    name.extend_from_slice(b"new");
    peer.call(runtime, Operation::Delete, parent, 0, &name);
    let replacement = peer.call(runtime, Operation::Create, parent, 0, &name);
    let read = peer.call(runtime, Operation::Ioctl, bound, 1, &[]);
    assert_eq!(u64::from_le_bytes(read.data.as_ref().try_into().unwrap()), 29);
    let fresh = bind(peer, runtime, replacement.references[0]);
    let read = peer.call(runtime, Operation::Ioctl, fresh, 1, &[]);
    assert_eq!(u64::from_le_bytes(read.data.as_ref().try_into().unwrap()), 0);
    peer.call(runtime, Operation::Close, bound, 0, &[]);
    peer.call(runtime, Operation::Close, fresh, 0, &[]);
}

fn loaded(source: &str, create_access: u32) -> (Runtime, Peer) {
    let mut runtime =
        reovim_server::assemble([0xa7; 16], Limits::default()).unwrap();
    let namespace = runtime.register_namespace(b"/mount").unwrap();
    let binding = Rc::new(runtime.bind(&[]).unwrap());
    runtime
        .grant_publication(
            &binding,
            PublicationGrant {
                namespace,
                scope: ModuleScope::Global,
                access: 3,
                create_access,
                retain_loans: false,
            },
        )
        .unwrap();
    let mut clients = Clients::new(1).unwrap();
    clients.admit([0xa7; 16], &binding).unwrap();
    let peer = Peer { clients, sequence: 0 };
    let artifact = Artifact::compile(source);
    let module = unsafe { native::load(&artifact.library) }.unwrap();
    let paths: Vec<&[u8]> =
        module.required().iter().map(AsRef::as_ref).collect();
    runtime
        .activate(
            Activation {
                namespace,
                scope: ModuleScope::Global,
                required: &paths,
                exports: module.export_limit(),
            },
            module_implementation(),
            module.imports(
                binding.caller(),
                ValueLimits { bytes: 8192, elements: 256, references: 8 },
                16,
            ),
            &binding,
            Rc::new(module.clone()),
        )
        .unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    drop(module);
    drop(artifact);
    (runtime, peer)
}

#[test]
fn service_create_and_reopen_share_identity_through_a_late_native_module() {
    let (mut runtime, mut peer) = loaded("namespace_module.c", 7);
    let cx = &mut Context::from_waker(Waker::noop());
    let parent = peer.open(&mut runtime, b"/mount/directory").references[0];
    peer.call(
        &mut runtime,
        Operation::SetAccess,
        parent,
        0,
        &3_u32.to_le_bytes(),
    );
    let mut name = 3_u32.to_le_bytes().to_vec();
    name.extend_from_slice(b"new");
    let created = peer.call(&mut runtime, Operation::Create, parent, 0, &name);
    let reopened = peer.open(&mut runtime, b"/mount/directory/new");
    assert_eq!(created.data[..24], reopened.data[..24]);
    assert_ne!(created.references[0], reopened.references[0]);
    let info =
        peer.call(&mut runtime, Operation::Info, created.references[0], 0, &[]);
    assert_eq!(u32::from_le_bytes(info.data[24..28].try_into().unwrap()), 3);
    let mut page = 0_u64.to_le_bytes().to_vec();
    page.extend_from_slice(&4096_u64.to_le_bytes());
    let rows = peer.call(&mut runtime, Operation::Entries, parent, 0, &page);
    assert!(listed(&rows).iter().any(
        |(name, id)| name == b"new" && id.as_slice() == &created.data[..24]
    ));
    peer.call(&mut runtime, Operation::Close, reopened.references[0], 0, &[]);
    lifetime(&mut peer, &mut runtime, created.references[0], parent);
    peer.call(&mut runtime, Operation::Close, parent, 0, &[]);
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
}
fn listed(value: &Value) -> Vec<(Vec<u8>, Vec<u8>)> {
    let count = u32::from_le_bytes(value.data[..4].try_into().unwrap());
    let mut data = &value.data[4..];
    let mut rows = Vec::new();
    for _ in 0..count {
        let size = u32::from_le_bytes(data[..4].try_into().unwrap()) as usize;
        let name = data[4..4 + size].to_vec();
        let id = data[4 + size..28 + size].to_vec();
        rows.push((name, id));
        data = &data[28 + size..];
    }
    rows
}
