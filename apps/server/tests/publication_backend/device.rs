use super::*;
use std::{
    cell::{Cell, RefCell},
    future::poll_fn,
    task::{Poll, Waker},
};

#[derive(Debug, Eq, PartialEq)]
pub struct Version {
    pub generation: u64,
    pub active: bool,
}

pub struct Device {
    pub current: RefCell<Rc<Version>>,
    pub ready: Cell<bool>,
    pub wake: RefCell<Option<Waker>>,
    pub uncertain: Cell<bool>,
    pub reconcile: Cell<bool>,
}

impl Device {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            current: RefCell::new(Rc::new(Version {
                generation: 0,
                active: false,
            })),
            ready: Cell::new(true),
            wake: RefCell::new(None),
            uncertain: Cell::new(false),
            reconcile: Cell::new(true),
        })
    }
}

struct Evidence {
    reported: Cell<protocol::Status>,
    actual: Cell<protocol::Status>,
    committed: RefCell<Option<Rc<Version>>>,
    device: Rc<Device>,
}
struct Record {
    name: String,
    resource: ResourceId,
    evidence: Rc<Evidence>,
}
struct Backend {
    kernel: Connection,
    device: Rc<Device>,
    records: Rc<RefCell<Vec<Record>>>,
    next: u64,
}

pub fn provider(kernel: Connection, device: Rc<Device>) -> ResourceProvider {
    ResourceProvider::new(
        Backend { kernel, device, records: Rc::default(), next: 0 },
        ResourceOps {
            unload: Some(|backend| {
                backend.records.borrow().iter().try_for_each(|record| {
                    settled(record.evidence.reported.get())
                })
            }),
            read: |_, _, _| Box::pin(async { Err(Error::UNSUPPORTED) }),
            write: |backend, target, input| {
                if input.contract.name.as_ref() == "resource.effect.release" {
                    return Box::pin(std::future::ready(release(
                        backend, input,
                    )));
                }
                publish(backend, target, input)
            },
        },
        Box::new([
            Surface {
                contract: Contract::new("resource.publish", 1),
                methods: Permissions::WRITE,
                input: Box::new([Permissions::READ, Permissions::READ]),
                output: Box::new([Permissions::ALL]),
            },
            surface("resource.effect.release", Permissions::WRITE),
        ]),
        Rc::new(()),
    )
}

fn publish(
    backend: &mut Backend,
    target: ResourceId,
    input: Message,
) -> reovim_kernel_interface::provider::ProviderReply {
    let selected = backend.kernel.read_view(input.resources[0]);
    let base = backend.kernel.read_view(input.resources[1]);
    backend.next += 1;
    let name = format!("effect-{}", backend.next);
    let kernel = backend.kernel.clone();
    let device = backend.device.clone();
    let records = backend.records.clone();
    Box::pin(async move {
        if !input.data.is_empty() {
            return Err(Error::INVALID);
        }
        let (selected, base) = (selected?, base?);
        wait(&device).await;
        let value = read(&selected, "logical.value").await?;
        let expected = read(&base, "resource.base").await?;
        let active = match value.data.as_ref() {
            [0] => false,
            [1] => true,
            _ => return Err(Error::INVALID),
        };
        let expected = u64::from_le_bytes(
            expected.data.as_ref().try_into().map_err(|_| Error::INVALID)?,
        );
        let candidate = Rc::new(Version {
            generation: expected.checked_add(1).ok_or(Error::PRESSURE)?,
            active,
        });
        let evidence = reserve(&kernel, target, &records, &name, &device)?;
        let id = records.borrow().last().unwrap().resource;
        let mut reply = Message::reference(input.contract, id);
        reply.data = name.as_bytes().into();
        apply(&evidence, expected, candidate);
        Ok(reply)
    })
}

#[allow(clippy::future_not_send)] // Runtime drives this local provider.
async fn wait(device: &Device) {
    poll_fn(|context| {
        if device.ready.get() {
            return Poll::Ready(());
        }
        *device.wake.borrow_mut() = Some(context.waker().clone());
        Poll::Pending
    })
    .await;
}

#[allow(clippy::future_not_send)] // Connections belong to one executor.
async fn read(kernel: &Connection, name: &str) -> Result<Message, Error> {
    let root = kernel.root().into_inner().map_err(|error| code(&error))?;
    kernel
        .read(root, &Message::new(Contract::new(name, 1), &[]))
        .await
        .map_err(|error| code(&error))
}

fn reserve(
    kernel: &Connection,
    target: ResourceId,
    records: &RefCell<Vec<Record>>,
    name: &str,
    device: &Rc<Device>,
) -> Result<Rc<Evidence>, Error> {
    records.borrow_mut().try_reserve(1).map_err(|_| Error::PRESSURE)?;
    let evidence = Rc::new(Evidence {
        reported: Cell::new(protocol::Status::OutcomeUnknown(
            Error::NOT_READY.code,
        )),
        actual: Cell::new(protocol::Status::OutcomeUnknown(
            Error::NOT_READY.code,
        )),
        committed: RefCell::new(None),
        device: device.clone(),
    });
    let resource = kernel
        .publish(target, name, record(evidence.clone()))
        .map_err(|(error, _)| error)?;
    records.borrow_mut().push(Record {
        name: name.into(),
        resource,
        evidence: evidence.clone(),
    });
    Ok(evidence)
}

fn apply(evidence: &Evidence, expected: u64, candidate: Rc<Version>) {
    let mut current = evidence.device.current.borrow_mut();
    let status = if current.generation == expected {
        *current = candidate.clone();
        *evidence.committed.borrow_mut() = Some(candidate);
        protocol::Status::Applied
    } else {
        protocol::Status::DefinitelyNotApplied(Error::STALE.code)
    };
    evidence.actual.set(status);
    evidence.reported.set(if evidence.device.uncertain.replace(false) {
        protocol::Status::OutcomeUnknown(Error::UNAVAILABLE.code)
    } else {
        status
    });
}

fn record(evidence: Rc<Evidence>) -> ResourceProvider {
    let views = evidence.clone();
    ResourceProvider::new(
        evidence,
        ResourceOps {
            unload: Some(|evidence| settled(evidence.reported.get())),
            read: |evidence, _, input| {
                Box::pin(std::future::ready(report(evidence, input)))
            },
            write: |evidence, _, input| {
                let result = reconcile(evidence, input);
                Box::pin(std::future::ready(result))
            },
        },
        Box::new([
            surface("resource.effect", Permissions::READ),
            surface("resource.effect.reconcile", Permissions::WRITE),
        ]),
        Rc::new(()),
    )
    .with_read_view(move || {
        if views.reported.get() != protocol::Status::Applied {
            return Err(Error::NOT_READY);
        }
        Ok(view(views.committed.borrow().clone().ok_or(Error::NOT_READY)?))
    })
}

fn reconcile(evidence: &Evidence, input: Message) -> Result<Message, Error> {
    empty(&input)?;
    if !evidence.device.reconcile.get() {
        return Err(Error::UNSUPPORTED);
    }
    evidence.reported.set(evidence.actual.get());
    protocol::encode(input.contract, evidence.reported.get())
}

fn report(evidence: &Evidence, input: Message) -> Result<Message, Error> {
    empty(&input)?;
    protocol::encode(input.contract, evidence.reported.get())
}

fn empty(input: &Message) -> Result<(), Error> {
    if input.data.is_empty() && input.resources.is_empty() {
        Ok(())
    } else {
        Err(Error::INVALID)
    }
}

fn release(backend: &Backend, input: Message) -> Result<Message, Error> {
    let mut records = backend.records.borrow_mut();
    let index = records
        .iter()
        .position(|r| r.name.as_bytes() == input.data.as_ref())
        .ok_or(Error::UNAVAILABLE)?;
    settled(records[index].evidence.reported.get())?;
    backend.kernel.withdraw(records[index].resource)?;
    records.remove(index);
    Ok(Message::new(input.contract, &[]))
}

const fn settled(status: protocol::Status) -> Result<(), Error> {
    match status {
        protocol::Status::Applied
        | protocol::Status::DefinitelyNotApplied(_) => Ok(()),
        _ => Err(Error::BUSY),
    }
}

pub fn view(version: Rc<Version>) -> ResourceProvider {
    let again = version.clone();
    ResourceProvider::new(
        version,
        ResourceOps {
            unload: None,
            read: |version, _, input| {
                let value = if input.contract.name.as_ref() == "resource.base" {
                    version.generation.to_le_bytes().to_vec()
                } else {
                    vec![u8::from(version.active)]
                };
                Box::pin(
                    async move { Ok(Message::new(input.contract, &value)) },
                )
            },
            write: |_, _, _| Box::pin(async { Err(Error::UNSUPPORTED) }),
        },
        Box::new([
            surface("resource.base", Permissions::READ),
            surface("logical.value", Permissions::READ),
        ]),
        Rc::new(()),
    )
    .with_read_view(move || Ok(view(again.clone())))
}
