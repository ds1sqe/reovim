use reovim_kernel_interface::{Contract, Message, ResourceId};
use reovim_service::{hex, request::Request, transport, unhex};
use std::{
    io::{self, BufReader},
    net::TcpStream,
    time::Duration,
};

/// Hosted worker-owned I/O; the borrowed Client remains recoverable externally.
pub struct Remote {
    pub address: String,
    pub client: String,
    pub pending: Option<Request>,
    pub reserved: Option<u64>,
}

impl Remote {
    pub fn exchange(&self, fields: &str) -> io::Result<String> {
        self.frame(&format!("v1\t{}\t{fields}", self.client))
    }

    fn frame(&self, frame: &str) -> io::Result<String> {
        let mut stream = TcpStream::connect(&self.address)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        transport::send(&mut stream, frame)?;
        transport::response(&mut BufReader::new(stream))
    }

    #[cfg(test)]
    pub fn call<T>(
        &mut self,
        fields: &[String],
        decode: impl FnOnce(&str) -> io::Result<T>,
    ) -> io::Result<T> {
        let frame = self.prepare(fields)?;
        let reply = self.transmit(&frame)?;
        if reply.starts_with("error\t") {
            self.finish()?;
            return Err(io::Error::other(reply));
        }
        decode(&reply)
    }

    pub fn prepare(&mut self, fields: &[String]) -> io::Result<String> {
        if self.pending.is_some() || self.reserved.is_some() {
            return Err(io::Error::other("recover the retained receipt first"));
        }
        let reserved = self.exchange("reserve")?;
        let id = reserved
            .strip_prefix("ok\t")
            .ok_or_else(|| io::Error::other(&*reserved))?
            .parse()
            .map_err(io::Error::other)?;
        self.reserved = Some(id);
        self.pending = Some(
            Request::new(&self.client, id, fields).map_err(io::Error::other)?,
        );
        self.reserved = None;
        let frame = self
            .pending
            .as_mut()
            .unwrap()
            .send()
            .map_err(io::Error::other)?
            .to_owned();
        Ok(frame)
    }

    pub fn transmit(&mut self, frame: &str) -> io::Result<String> {
        let reply = self.frame(frame)?;
        self.pending
            .as_mut()
            .unwrap()
            .record(reply.clone())
            .map_err(io::Error::other)?;
        Ok(reply)
    }

    /// Only for a prepared frame that has not reached transmit.
    pub fn abandon_prepared(&mut self) {
        if let Some(request) = self.pending.take() {
            self.reserved = Some(request.id());
        }
    }

    /// Retrieves and decodes the original result without resending its call.
    pub fn recover_response(&mut self) -> io::Result<String> {
        let pending = self.pending.as_ref().ok_or_else(|| {
            io::Error::other("no retained request to recover")
        })?;
        if pending.release().is_ok() {
            return Err(io::Error::other(
                "outcome already adopted; retry cleanup",
            ));
        }
        if pending.response().is_none() {
            let frame = pending.recover().map_err(io::Error::other)?;
            let reply = self.frame(&frame)?;
            self.pending
                .as_mut()
                .unwrap()
                .record(reply)
                .map_err(io::Error::other)?;
        }
        let reply =
            self.pending.as_ref().unwrap().response().unwrap().to_owned();
        Ok(reply)
    }

    /// Called only after the consumer has stored the decoded Resource owners.
    #[cfg(test)]
    pub fn finish(&mut self) -> io::Result<()> {
        self.pending
            .as_mut()
            .ok_or_else(|| io::Error::other("no outcome to adopt"))?
            .adopt()
            .map_err(io::Error::other)?;
        self.cleanup()
    }

    /// Retries only adopted receipt cleanup or a known unsent reservation.
    /// An undecoded/Unknown outcome cannot authorize teardown of its owners.
    pub fn cleanup(&mut self) -> io::Result<()> {
        if let Some(id) = self.reserved {
            let reply = self.exchange(&format!("release\t{id}"))?;
            closed(&reply)?;
            self.reserved = None;
        }
        let Some(pending) = &self.pending else { return Ok(()) };
        let release = pending.release().map_err(io::Error::other)?;
        let released = self.frame(&release)?;
        self.pending
            .as_mut()
            .unwrap()
            .released(&released)
            .map_err(io::Error::other)?;
        self.pending = None;
        Ok(())
    }
}

pub fn closed(reply: &str) -> io::Result<()> {
    if reply == "ok" { Ok(()) } else { Err(io::Error::other(reply)) }
}

pub fn fields(
    method: &str,
    subject: ResourceId,
    input: &Message,
) -> Vec<String> {
    let mut fields = vec![
        method.into(),
        format!("@{}", subject.0),
        input.contract.name.to_string(),
        input.contract.version.to_string(),
        hex(&input.data),
    ];
    if !input.resources.is_empty() {
        fields.push(
            input
                .resources
                .iter()
                .map(|id| format!("@{}", id.0))
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    fields
}

pub fn message(reply: &str) -> io::Result<Message> {
    let parts: Vec<_> = reply.split('\t').collect();
    let ["ok", contract, version, data, references] = parts.as_slice() else {
        return Err(io::Error::other("invalid Message response"));
    };
    let resources = if references.is_empty() {
        Vec::new()
    } else {
        references.split(',').map(resource).collect::<io::Result<Vec<_>>>()?
    };
    Ok(Message {
        contract: Contract::new(
            contract,
            version.parse().map_err(io::Error::other)?,
        ),
        data: unhex(data)
            .ok_or_else(|| io::Error::other("invalid data"))?
            .into(),
        resources: resources.into(),
    })
}

pub fn resource(text: &str) -> io::Result<ResourceId> {
    text.strip_prefix('@')
        .ok_or_else(|| io::Error::other("expected @ID"))?
        .parse()
        .map(ResourceId)
        .map_err(io::Error::other)
}
