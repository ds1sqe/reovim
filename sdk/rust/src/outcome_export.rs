use crate::{caller_export::native_error, raw};
use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::{any::Any, cell::Cell, ffi::c_void};
use reovim_kernel_interface::{
    Error, Outcome, Value, abi, declaration::ValueLimits,
};

struct Taken {
    outcome: Rc<Outcome>,
    references: Vec<u64>,
    outstanding: Rc<Cell<bool>>,
    cleanup: Option<Box<dyn Any>>,
}

impl Drop for Taken {
    fn drop(&mut self) {
        // Keep finish fenced through provider cleanup, including destructors.
        drop(self.cleanup.take());
        self.outstanding.set(false);
    }
}

pub fn export(
    outcome: Rc<Outcome>,
    activity: u8,
    cleanup: &mut Option<Box<dyn Any>>,
    outstanding: Rc<Cell<bool>>,
    limits: ValueLimits,
) -> Result<abi::Outcome, Error> {
    validate(&outcome)?;
    if activity & !7 != 0 {
        return Err(Error::PROVIDER);
    }
    let mut owner = Box::new(Taken {
        outcome,
        references: Vec::new(),
        outstanding,
        cleanup: None,
    });
    let data = output_value(&owner.outcome);
    if let Some(data) = data {
        bounds(data, limits)?;
        owner
            .references
            .try_reserve_exact(data.references.len())
            .map_err(|_| Error::PRESSURE)?;
        owner.references.extend(data.references.iter().map(|token| token.0));
    }
    let (status, error) = output_status(&owner.outcome);
    let value = data.map_or_else(raw::empty, |data| abi::Args {
        data: raw::bytes(&data.data),
        refs: abi::Refs {
            ptr: owner.references.as_ptr(),
            len: owner.references.len(),
        },
    });
    owner.cleanup = cleanup.take();
    owner.outstanding.set(true);
    Ok(abi::Outcome {
        status,
        activity: u32::from(activity),
        error,
        value,
        owner: Box::into_raw(owner).cast(),
        release: Some(release),
    })
}

fn bounds(value: &Value, limits: ValueLimits) -> Result<(), Error> {
    if value.data.len() > limits.bytes
        || value.references.len() > limits.references
    {
        return Err(Error::PRESSURE);
    }
    Ok(())
}

fn validate(outcome: &Outcome) -> Result<(), Error> {
    let cause = match outcome {
        Outcome::Ok(_) => return Ok(()),
        Outcome::Failed(error) => error,
        Outcome::Unknown { error, progress } => {
            if progress
                .as_ref()
                .is_some_and(|value| !value.references.is_empty())
            {
                return Err(Error::PROVIDER);
            }
            error
        }
    };
    if cause.code.0 == 0 || cause.id.is_some_and(|id| id.serial == 0) {
        return Err(Error::PROVIDER);
    }
    Ok(())
}

const fn output_value(outcome: &Outcome) -> Option<&Value> {
    match outcome {
        Outcome::Ok(value) | Outcome::Unknown { progress: Some(value), .. } => {
            Some(value)
        }
        _ => None,
    }
}

fn output_status(outcome: &Outcome) -> (u32, abi::Error) {
    match outcome {
        Outcome::Ok(_) => (0, abi::Error::default()),
        Outcome::Failed(error) => (1, native_error(*error)),
        Outcome::Unknown { error, .. } => (2, native_error(*error)),
    }
}

unsafe extern "C" fn release(context: *mut c_void) {
    // SAFETY: export transferred this exact Box once. The native owner releases
    // it exactly once while its provider/Caller retains this adapter's code.
    drop(unsafe { Box::from_raw(context.cast::<Taken>()) });
}
