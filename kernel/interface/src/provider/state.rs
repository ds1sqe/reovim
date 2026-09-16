//! Provider-private immutable state retained by installation and snapshots.

use alloc::rc::Rc;
use core::any::Any;

use super::ResourceProvider;
use crate::Error;

trait Dispatch {
    fn data(&self) -> &dyn Any;
    fn snapshot(&self) -> Result<ResourceProvider, Error>;
}

struct Implementation<T> {
    value: Rc<T>,
    snapshot: fn(Rc<T>) -> Result<ResourceProvider, Error>,
    // State destructors may execute module code, so release this owner last.
    _code: Rc<dyn Any>,
}

impl<T: 'static> Dispatch for Implementation<T> {
    fn data(&self) -> &dyn Any {
        self.value.as_ref()
    }

    fn snapshot(&self) -> Result<ResourceProvider, Error> {
        (self.snapshot)(self.value.clone())
    }
}

/// One retained immutable state owned by its originating provider.
///
/// For example, History can retain a mesh revision without knowing its layout.
/// Callers inspect a snapshot Resource; this value is a provider-side record,
/// not a semantic call argument. Cloning retains the same state and code.
#[derive(Clone)]
pub struct State {
    implementation: Rc<dyn Dispatch>,
    cost: u64,
}

impl State {
    /// Retains private immutable data and its inert snapshot constructor.
    ///
    /// For example, value is one mesh revision and code pins its module.
    /// The provider must preserve that revision despite later live edits.
    /// Snapshot construction performs no external I/O or waiting and returns
    /// a readonly provider; later reads may load deferred data under custody.
    /// Cost is the provider's quota charge, not measured allocation size.
    #[must_use]
    pub fn new<T: 'static>(
        value: Rc<T>,
        cost: u64,
        snapshot: fn(Rc<T>) -> Result<ResourceProvider, Error>,
        code: Rc<dyn Any>,
    ) -> Self {
        Self {
            implementation: Rc::new(Implementation {
                value,
                snapshot,
                _code: code,
            }),
            cost,
        }
    }

    /// Borrows private state in its originating editing binding.
    ///
    /// For example, a mesh binding rejects a foreign state when this returns
    /// None. Matching Rust types alone do not establish provider provenance;
    /// installation must also validate the originating binding. Runtime/Core
    /// retain this value without inspecting or downcasting its contents.
    #[must_use]
    pub fn data<T: 'static>(&self) -> Option<&T> {
        self.implementation.data().downcast_ref()
    }

    /// Returns the declared quota charge, e.g. retained sparse-state cost.
    #[must_use]
    pub const fn cost(&self) -> u64 {
        self.cost
    }

    /// Constructs readonly Resource access over this exact retained state.
    ///
    /// For example, closing a live Buffer cannot release code needed by its
    /// snapshot. The returned provider and its requests retain state/code
    /// independently of this State handle; the installation owner validates
    /// offered operations and owns provider cleanup before publication.
    /// # Errors
    /// Construction failure publishes nothing and leaves state retained.
    /// The constructor owns cleanup of anything it did not return.
    pub fn snapshot(&self) -> Result<ResourceProvider, Error> {
        let mut provider = self.implementation.snapshot()?;
        let code = provider.code.clone();
        provider.code = Rc::new((self.clone(), code));
        Ok(provider)
    }
}
