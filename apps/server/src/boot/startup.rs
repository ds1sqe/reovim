//! Required activation retains its original Caller receipts.

use alloc::{rc::Rc, vec::Vec};
use core::task::Context;
use reovim_kernel_interface::{
    Error, Outcome, ResourceId, Value,
    caller::{Caller, Operation, Status},
    driver::Progress,
    module::activation,
    resource::AccessId,
};
use reovim_sdk::Request;

#[derive(Clone, Copy)]
enum Step {
    Open,
    Access,
    Bind,
    Activate,
    Info,
    CloseControl,
    CloseBound,
    Done,
}

/// Required startup through ordinary package activation, e.g. before listening.
///
/// Composition drives this owner beside Runtime; no direct constructor route.
/// Keep it and its Caller on error so original work remains recoverable.
pub struct Startup {
    caller: Caller,
    package: ResourceId,
    declaration: Vec<u8>,
    step: Step,
    request: Option<Request>,
    opened: AccessId,
    bound: AccessId,
    control: AccessId,
    instance: Option<ResourceId>,
    failure: Option<Error>,
}

impl Startup {
    /// Captures the known package activation API without submitting work.
    /// The supplied Caller needs READ|EXEC on the exact admitted package.
    /// # Errors
    /// Declaration allocation failure rejects before acquiring any access.
    pub fn new(caller: Caller, package: ResourceId) -> Result<Self, Error> {
        let api = activation::declaration()?;
        let mut declaration = Vec::new();
        declaration
            .try_reserve_exact(4 + api.bytes().len())
            .map_err(|_| Error::PRESSURE)?;
        let size =
            u32::try_from(api.bytes().len()).map_err(|_| Error::INVALID)?;
        declaration.extend_from_slice(&size.to_le_bytes());
        declaration.extend_from_slice(api.bytes());
        Ok(Self {
            caller,
            package,
            declaration,
            step: Step::Open,
            request: None,
            opened: AccessId(0),
            bound: AccessId(0),
            control: AccessId(0),
            instance: None,
            failure: None,
        })
    }

    /// Returns Ready identity only after temporary startup accesses are closed.
    /// Example: grant its published exports before admitting service clients.
    /// # Errors
    /// A settled startup failure retains its original diagnostic.
    pub const fn result(&self) -> Result<Option<ResourceId>, Error> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        Ok(if matches!(self.step, Step::Done) { self.instance } else { None })
    }

    /// Exposes original custody for cancellation/reconciliation, e.g. UNKNOWN.
    #[must_use]
    pub const fn request(&self) -> Option<&Request> {
        self.request.as_ref()
    }

    /// Advances one receipt transition; Runtime driving stays with composition.
    /// # Errors
    /// Submission, observation and cleanup failure retain the exact request.
    /// UNKNOWN
    /// reports Incomplete without releasing or replaying its activation effect.
    pub fn advance(&mut self, cx: &mut Context<'_>) -> Result<Progress, Error> {
        if self.result()?.is_some() {
            return Ok(Progress::Idle);
        }
        if self.request.is_none() {
            let (operation, subject, input) = self.call()?;
            self.request = Some(Request::reserve(
                &self.caller,
                operation,
                subject,
                0,
                None,
                input,
            )?);
        }
        let request = self.request.as_ref().ok_or(Error::PROVIDER)?;
        match request.status(cx)? {
            Status::Reserved => {
                request.submit()?;
                Ok(Progress::More)
            }
            Status::Pending => Ok(Progress::Idle),
            Status::Outcome(outcome) => self.complete(&outcome),
        }
    }

    fn complete(&mut self, outcome: &Rc<Outcome>) -> Result<Progress, Error> {
        let value = match outcome.as_ref() {
            Outcome::Ok(value) => value,
            Outcome::Failed(error) => return self.failed(*error),
            Outcome::Unknown { .. } => return Err(Error::INCOMPLETE),
        };
        self.capture(value)?;
        let request = self.request.as_ref().ok_or(Error::PROVIDER)?;
        request.adopt()?;
        request.release()?;
        self.request = None;
        self.step = next(self.step);
        Ok(Progress::More)
    }

    fn failed(&mut self, error: Error) -> Result<Progress, Error> {
        self.request.as_ref().ok_or(Error::PROVIDER)?.release()?;
        self.request = None;
        self.failure = Some(error);
        Err(error)
    }

    fn capture(&mut self, value: &Value) -> Result<(), Error> {
        match self.step {
            Step::Open => self.opened = reference(value)?,
            Step::Bind => self.bound = reference(value)?,
            Step::Activate => self.control = reference(value)?,
            Step::Info => self.instance = Some(identity(value)?),
            _ => {}
        }
        Ok(())
    }

    fn call(&self) -> Result<(Operation, AccessId, Value), Error> {
        let (operation, subject, bytes) = match self.step {
            Step::Open => {
                let mut locator = [0; 28];
                locator[..4].copy_from_slice(&1_u32.to_le_bytes());
                locator[4..20].copy_from_slice(&self.package.epoch);
                locator[20..]
                    .copy_from_slice(&self.package.serial.to_le_bytes());
                return Ok((
                    Operation::Acquire,
                    AccessId(0),
                    Value::copy(&locator, &[])?,
                ));
            }
            Step::Access => {
                (Operation::SetAccess, self.opened, &[5, 0, 0, 0][..])
            }
            Step::Bind => {
                (Operation::Bind, self.opened, self.declaration.as_slice())
            }
            Step::Activate => (Operation::Exec, self.bound, &[][..]),
            Step::Info => (Operation::Info, self.control, &[][..]),
            Step::CloseControl => (Operation::Close, self.control, &[][..]),
            Step::CloseBound => (Operation::Close, self.bound, &[][..]),
            Step::Done => return Err(Error::CLOSED),
        };
        Ok((operation, subject, Value::copy(bytes, &[])?))
    }
}

const fn next(step: Step) -> Step {
    match step {
        Step::Open => Step::Access,
        Step::Access => Step::Bind,
        Step::Bind => Step::Activate,
        Step::Activate => Step::Info,
        Step::Info => Step::CloseControl,
        Step::CloseControl => Step::CloseBound,
        // Bind shares opened state; closing it also removes the base handle.
        Step::CloseBound | Step::Done => Step::Done,
    }
}

fn reference(value: &Value) -> Result<AccessId, Error> {
    value.references.first().copied().ok_or(Error::PROVIDER)
}
fn identity(value: &Value) -> Result<ResourceId, Error> {
    let bytes = value.data.get(..24).ok_or(Error::PROVIDER)?;
    Ok(ResourceId {
        epoch: bytes[..16].try_into().map_err(|_| Error::PROVIDER)?,
        serial: u64::from_le_bytes(
            bytes[16..].try_into().map_err(|_| Error::PROVIDER)?,
        ),
    })
}
