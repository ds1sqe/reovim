//! Caller-owned delivery custody, separate from Resource ownership.

use alloc::{format, string::String};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Invalid,
    State,
    Unknown,
}

impl core::fmt::Display for Error {
    fn fmt(&self, output: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(output, "service receipt: {self:?}")
    }
}
impl core::error::Error for Error {}

/// A reservation is transmitted once; uncertain delivery recovers its result.
///
/// The owner persists transitions before performing their returned I/O.
/// This value performs no I/O and does not close returned Resource access.
pub struct Request {
    client: String,
    id: u64,
    frame: String,
    sent: bool,
    response: Option<String>,
    adopted: bool,
    released: bool,
}

/// Local checkpoint data; credentials and result bytes require private storage.
/// Restoration validates transitions and the expected caller scope.
pub struct Checkpoint {
    pub frame: String,
    pub sent: bool,
    pub response: Option<String>,
    pub adopted: bool,
    pub released: bool,
}

impl Request {
    #[must_use]
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            frame: self.frame.clone(),
            sent: self.sent,
            response: self.response.clone(),
            adopted: self.adopted,
            released: self.released,
        }
    }

    /// Restores local custody; this neither reconnects nor sends a request.
    /// # Errors
    /// Rejects foreign scope, malformed frames and impossible transitions.
    pub fn restore(client: &str, saved: Checkpoint) -> Result<Self, Error> {
        if saved.frame.len() >= crate::MAX_LINE {
            return Err(Error::Invalid);
        }
        let parts: alloc::vec::Vec<_> = saved.frame.split('\t').collect();
        let ["v1", owner, "call", id, fields @ ..] = parts.as_slice() else {
            return Err(Error::Invalid);
        };
        if *owner != client {
            return Err(Error::Invalid);
        }
        let fields: alloc::vec::Vec<_> =
            fields.iter().map(|value| String::from(*value)).collect();
        let mut value = Self::new(
            client,
            id.parse().map_err(|_| Error::Invalid)?,
            &fields,
        )?;
        if value.frame != saved.frame {
            return Err(Error::Invalid);
        }
        if saved.sent {
            value.send()?;
        }
        if let Some(response) = saved.response {
            value.record(response)?;
        }
        if saved.adopted {
            value.adopt()?;
        }
        if saved.released {
            value.released("ok")?;
        }
        Ok(value)
    }
    /// Takes custody of a known reservation; invalid input leaves it to caller.
    /// # Errors
    /// Rejects invalid identity, fields or an oversized encoded frame.
    pub fn new(
        client: &str,
        id: u64,
        fields: &[String],
    ) -> Result<Self, Error> {
        if client.len() != 32
            || !client.bytes().all(|byte| byte.is_ascii_hexdigit())
            || fields.is_empty()
            || fields.iter().any(|field| field.contains(['\t', '\r', '\n']))
        {
            return Err(Error::Invalid);
        }
        let frame = format!("v1\t{client}\tcall\t{id}\t{}", fields.join("\t"));
        if frame.len() >= crate::MAX_LINE {
            return Err(Error::Invalid);
        }
        Ok(Self {
            client: client.into(),
            id,
            frame,
            sent: false,
            response: None,
            adopted: false,
            released: false,
        })
    }

    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    /// Marks transmission before returning the exact immutable call frame.
    /// # Errors
    /// Rejects replay; use recovery after any attempted transmission.
    pub fn send(&mut self) -> Result<&str, Error> {
        if self.sent {
            return Err(Error::State);
        }
        self.sent = true;
        Ok(&self.frame)
    }

    /// Reads the original result without executing another call.
    /// # Errors
    /// Rejects an unsent or already released reservation.
    pub fn recover(&self) -> Result<String, Error> {
        if !self.sent || self.released {
            return Err(Error::State);
        }
        Ok(self.control("result"))
    }

    /// Retains a complete outcome before the caller adopts its meaning.
    /// # Errors
    /// Invalid/control replies preserve custody and require explicit recovery.
    pub fn record(&mut self, reply: String) -> Result<(), Error> {
        if !self.sent || self.adopted || self.released {
            return Err(Error::State);
        }
        outcome(&reply)?;
        if self.response.as_ref().is_some_and(|old| old != &reply) {
            return Err(Error::Invalid);
        }
        self.response = Some(reply);
        Ok(())
    }

    #[must_use]
    pub fn response(&self) -> Option<&str> {
        self.response.as_deref()
    }

    /// Caller has adopted the known outcome and its separate Resource owners.
    /// # Errors
    /// Unknown outcomes cannot be forgotten as failed effects.
    pub fn adopt(&mut self) -> Result<(), Error> {
        if self.released {
            return Err(Error::State);
        }
        if outcome(self.response.as_deref().ok_or(Error::State)?)? {
            return Err(Error::Unknown);
        }
        self.adopted = true;
        Ok(())
    }

    /// Retains the outcome while receipt cleanup is attempted or retried.
    /// # Errors
    /// Requires prior explicit adoption.
    pub fn release(&self) -> Result<String, Error> {
        if !self.adopted {
            return Err(Error::State);
        }
        Ok(self.control("release"))
    }

    /// Records confirmed cleanup; a lost reply leaves release retryable.
    /// # Errors
    /// Rejects non-success without dropping the retained outcome.
    pub fn released(&mut self, reply: &str) -> Result<(), Error> {
        if !self.adopted || reply != "ok" {
            return Err(Error::State);
        }
        self.released = true;
        Ok(())
    }

    fn control(&self, action: &str) -> String {
        format!("v1\t{}\t{action}\t{}", self.client, self.id)
    }
}

fn outcome(reply: &str) -> Result<bool, Error> {
    if reply.len() >= crate::MAX_LINE || reply.contains(['\r', '\n']) {
        return Err(Error::Invalid);
    }
    if reply == "ok" || reply.starts_with("ok\t") {
        return Ok(false);
    }
    let fields: alloc::vec::Vec<_> = reply.split('\t').collect();
    let ["error", kind, code, resource, recovery] = fields.as_slice() else {
        return Err(Error::Invalid);
    };
    if !matches!(*kind, "rejected" | "failed" | "unknown")
        || code.parse::<u64>().is_err()
        || (!resource.is_empty()
            && resource
                .strip_prefix('@')
                .and_then(|id| id.parse::<u64>().ok())
                .is_none())
        || crate::unhex(recovery).is_none()
    {
        return Err(Error::Invalid);
    }
    Ok(*kind == "unknown")
}
