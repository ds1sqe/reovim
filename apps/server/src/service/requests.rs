//! Bounded delivery receipts owned by one retained logical client.

use super::pending::Response;
use alloc::{boxed::Box, format, rc::Rc, string::String, vec::Vec};
use core::{
    cell::RefCell,
    future::poll_fn,
    task::{Context, Poll, Waker},
};
use reovim_kernel_interface::Connection;

const CAPACITY: usize = 64;

#[derive(Default)]
pub(super) struct Requests {
    next: u64,
    entries: Vec<(u64, Rc<RefCell<Receipt>>)>,
}

#[derive(Default)]
struct Receipt {
    input: Option<String>,
    response: Option<Response>,
    output: Option<Result<String, String>>,
    wake: Option<Waker>,
}

impl Requests {
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn list(&self) -> String {
        let entries: Vec<_> = self
            .entries
            .iter()
            .map(|(id, receipt)| format!("{id}={}", receipt.borrow().state()))
            .collect();
        format!("ok\t{}", entries.join("\t"))
    }

    pub fn input(&self, id: u64) -> Result<String, String> {
        let receipt = self.find(id)?;
        let receipt = receipt.borrow();
        let input = receipt.input.as_deref().unwrap_or("");
        Ok(format!("ok\t{}", reovim_service::hex(input.as_bytes())))
    }

    pub fn reserve(&mut self) -> Result<String, String> {
        self.allocate().map(|id| format!("ok\t{id}"))
    }

    pub fn enqueue(
        &mut self,
        input: &str,
        make: impl FnOnce(u64) -> Response,
    ) -> Result<(u64, Response), String> {
        let id = self.allocate()?;
        let receipt = self.find(id)?;
        let mut value = receipt.borrow_mut();
        value.input = Some(input.into());
        value.response = Some(make(id));
        drop(value);
        Ok((id, observe(receipt)))
    }

    fn allocate(&mut self) -> Result<u64, String> {
        if self.entries.len() == CAPACITY {
            return Err("request capacity".into());
        }
        let id =
            self.next.checked_add(1).ok_or("request identity exhausted")?;
        self.entries.try_reserve(1).map_err(|_| "request capacity")?;
        self.entries.push((id, Rc::default()));
        self.next = id;
        Ok(id)
    }

    pub fn call(
        &self,
        id: u64,
        input: &str,
        kernel: &Connection,
    ) -> Result<Response, String> {
        let receipt = self.find(id)?;
        let mut value = receipt.borrow_mut();
        if value.input.as_deref().is_some_and(|prior| prior != input) {
            return Err("request identity has different input".into());
        }
        if value.input.is_none() {
            let mut captured = String::new();
            captured
                .try_reserve(input.len())
                .map_err(|_| "request capacity")?;
            captured.push_str(input);
            value.input = Some(captured);
            install(&mut value, super::dispatch(input, kernel));
        }
        drop(value);
        Ok(observe(receipt))
    }

    pub fn result(&self, id: u64) -> Result<Response, String> {
        let receipt = self.find(id)?;
        if receipt.borrow().input.is_none() {
            return Err("request remains reserved; no result yet".into());
        }
        Ok(observe(receipt))
    }

    pub fn status(&self, id: u64) -> Result<String, String> {
        let receipt = self.find(id)?;
        let value = receipt.borrow();
        Ok(format!("ok\t{}", value.state()))
    }

    pub fn release(&mut self, id: u64) -> Result<String, String> {
        let index = self.entries.iter().position(|(key, _)| *key == id);
        let Some(index) = index else {
            return (id != 0 && id <= self.next)
                .then(|| "ok".into())
                .ok_or_else(|| "unavailable request".into());
        };
        if self.entries[index].1.borrow().response.is_some() {
            return Err("request is still pending".into());
        }
        self.entries.remove(index);
        Ok("ok".into())
    }

    pub fn advance(&self, context: &mut Context<'_>) {
        for (_, receipt) in &self.entries {
            advance(receipt, context);
        }
    }

    fn find(&self, id: u64) -> Result<Rc<RefCell<Receipt>>, String> {
        self.entries
            .iter()
            .find(|(key, _)| *key == id)
            .map(|(_, value)| value.clone())
            .ok_or_else(|| "unavailable request".into())
    }
}

impl Receipt {
    const fn state(&self) -> &'static str {
        if self.output.is_some() {
            "finished"
        } else if self.input.is_some() {
            "pending"
        } else {
            "reserved"
        }
    }
}

fn install(receipt: &mut Receipt, response: Result<Response, String>) {
    match response {
        Ok(response) => receipt.response = Some(response),
        Err(error) => receipt.output = Some(Err(error)),
    }
}
fn observe(receipt: Rc<RefCell<Receipt>>) -> Response {
    Box::pin(poll_fn(move |context| {
        let mut receipt = receipt.borrow_mut();
        if let Some(output) = &receipt.output {
            return Poll::Ready(output.clone());
        }
        receipt.wake = Some(context.waker().clone());
        Poll::Pending
    }))
}
fn advance(receipt: &Rc<RefCell<Receipt>>, context: &mut Context<'_>) {
    let mut receipt = receipt.borrow_mut();
    let Some(response) = receipt.response.as_mut() else { return };
    let Poll::Ready(output) = response.as_mut().poll(context) else { return };
    let retired = receipt.response.take();
    receipt.output = Some(output);
    let wake = receipt.wake.take();
    drop(receipt);
    drop(retired);
    if let Some(wake) = wake {
        wake.wake();
    }
}
