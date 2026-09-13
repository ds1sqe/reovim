mod checkpoint;
mod input;
mod journal;
mod remote;
mod session;
mod terminal;
#[cfg(test)]
mod tests;
mod worker;

use crossterm::event;
use input::Event;
use reovim_client_tui::context::Window;
use reovim_client_tui::input::{Input, Key};
use reovim_client_tui::operation::Operation;
use reovim_client_tui::view::View;
use reovim_client_tui::{editing, input::Action, viewport};
use reovim_kernel_interface::buffer::BufferState;
use std::{env, io, sync::mpsc, thread, time::Duration};

struct Update {
    window: Window,
    accepted: Option<BufferState>,
}

struct Response {
    result: io::Result<Update>,
    receipt: Option<u64>,
}

enum Notice {
    Outcome(Response),
    Capture(
        mpsc::Sender<io::Result<reovim_client_tui::custody::RetainedInput>>,
    ),
}

fn main() -> io::Result<()> {
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut args: Vec<_> = env::args().skip(1).collect();
    if let [flag, path] = args.as_slice()
        && flag == "--inspect-checkpoint"
    {
        return checkpoint::inspect(path);
    }
    let journal_path = if args.len() == 6 && args[4] == "--journal" {
        let path = args.remove(5);
        args.pop();
        Some(path)
    } else {
        None
    };
    let save = checkpoint::argument(&mut args)?;
    let [address, client, buffer, factory] = args.as_slice() else {
        return Err(io::Error::other(
            "usage: reovim-tui ADDRESS CLIENT @BUFFER @TEXT_FACTORY \
             [--checkpoint FILE | --journal NEW_DIRECTORY]",
        ));
    };
    let mut session = session::Session {
        remote: remote::Remote {
            address: address.clone(),
            client: client.clone(),
            pending: None,
            reserved: None,
        },
        buffer: remote::resource(buffer)?,
        access: None,
        operation: Operation::Idle,
        observed: None,
        retiring: Vec::new(),
        accepted: None,
        cancel: cancel.clone(),
        journal: None,
    };
    let factory = remote::resource(factory)?;
    let (send, receive) = mpsc::sync_channel(1);
    let (reply, results) = mpsc::channel();
    if let Some(path) = journal_path {
        session.journal =
            Some(journal::Journal::create(&path, factory, reply.clone())?);
    }
    let worker = thread::spawn(move || {
        let result = worker::run(&mut session, factory, &receive, &reply);
        (session, result)
    });
    let mut view = View::new();
    let result = run(&mut view, &send, &results);
    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = send.send(Action::Close);
    drop(send);
    while !worker.is_finished() {
        if let Ok(notice) = results.recv_timeout(Duration::from_millis(20)) {
            process(&mut view, notice);
        }
    }
    let (session, cleanup) =
        worker.join().map_err(|_| io::Error::other("TUI worker failed"))?;
    if (result.is_err() || cleanup.is_err())
        && let Some(path) = save
    {
        checkpoint::save(&path, &session, factory, &view)?;
    }
    result.and(cleanup)
}

fn accept(view: &mut View, result: io::Result<Update>) {
    match result {
        Ok(update) => view.accept(update.window, update.accepted),
        Err(error) => view.failed(&error),
    }
}

fn run(
    view: &mut View,
    send: &mpsc::SyncSender<Action>,
    results: &mpsc::Receiver<Notice>,
) -> io::Result<()> {
    let mut terminal = terminal::Terminal::enter()?;
    view.visible = draw(None, 0, &view.status, &mut view.top)?;
    loop {
        receive(view, results)?;
        let Some(event) = next_event(view)? else { continue };
        if quit(&event) && view.queued.is_empty() && view.submitted.is_none() {
            break;
        }
        if quit(&event) {
            view.status =
                "Retained input: F6 recover or Ctrl+D discard settled work"
                    .into();
            view.visible = draw(
                view.current.as_ref(),
                view.cursor,
                &view.status,
                &mut view.top,
            )?;
            continue;
        }
        if matches!(event, Event::Resize) {
            view.visible = draw(
                view.current.as_ref(),
                view.cursor,
                &view.status,
                &mut view.top,
            )?;
            continue;
        }
        let Event::Input(value) = event else { continue };
        if view.pending {
            view.queued.push_back(value);
            continue;
        }
        if logical_key(&value).is_some_and(editing::recover) {
            send.send(Action::RecoverView).map_err(io::Error::other)?;
            view.pending = true;
            continue;
        }
        if logical_key(&value).is_some_and(editing::refresh) {
            let start = view.current.as_ref().map_or(0, |w| w.range.start);
            send.send(Action::Observe(start)).map_err(io::Error::other)?;
            view.pending = true;
            continue;
        }
        if (view.halted || !view.visible)
            && logical_key(&value).is_some_and(editing::discard)
        {
            view.discard_retained();
            view.visible = draw(
                view.current.as_ref(),
                view.cursor,
                &view.status,
                &mut view.top,
            )?;
            continue;
        }
        if view.halted || !view.visible {
            continue;
        }
        if let Some(window) = &view.current
            && let Some(action) = action(value, window, &mut view.cursor)
        {
            view.begin(&action).map_err(io::Error::other)?;
            send.send(action).map_err(io::Error::other)?;
        }
        view.visible = draw(
            view.current.as_ref(),
            view.cursor,
            if view.pending { "Request pending..." } else { &view.status },
            &mut view.top,
        )?;
    }
    terminal.restore()
}

fn next_event(view: &mut View) -> io::Result<Option<Event>> {
    if !view.pending && !view.halted && view.visible && !view.queued.is_empty()
    {
        return Ok(view.queued.pop_front().map(Event::Input));
    }
    if view.pending && view.queued.len() == 32 {
        thread::sleep(Duration::from_millis(20));
        return Ok(None);
    }
    if !event::poll(Duration::from_millis(20))? {
        return Ok(None);
    }
    event::read().map(input::event)
}

const fn logical_key(input: &Input) -> Option<Key> {
    if let Input::Key(key) = input { Some(*key) } else { None }
}

fn receive(
    view: &mut View,
    results: &mpsc::Receiver<Notice>,
) -> io::Result<()> {
    match results.try_recv() {
        Ok(notice) => {
            if !process(view, notice) {
                return Ok(());
            }
            let visible = draw(
                view.current.as_ref(),
                view.cursor,
                &view.status,
                &mut view.top,
            )?;
            view.visible = visible;
            Ok(())
        }
        Err(mpsc::TryRecvError::Empty) => Ok(()),
        Err(mpsc::TryRecvError::Disconnected) => Err(io::Error::other(
            "TUI worker disconnected; retained state must be recovered",
        )),
    }
}

fn process(view: &mut View, notice: Notice) -> bool {
    match notice {
        Notice::Capture(reply) => {
            let _ = reply.send(view.retained_input().map_err(io::Error::other));
            false
        }
        Notice::Outcome(response) => {
            view.receipt = response.receipt;
            accept(view, response.result);
            true
        }
    }
}

fn quit(event: &Event) -> bool {
    match event {
        Event::Input(value) => logical_key(value).is_some_and(editing::quit),
        Event::Resize => false,
    }
}

fn action(input: Input, window: &Window, cursor: &mut usize) -> Option<Action> {
    editing::action(input, window, cursor)
}

fn draw(
    window: Option<&Window>,
    cursor: usize,
    status: &str,
    top: &mut usize,
) -> io::Result<bool> {
    let (columns, rows) = crossterm::terminal::size()?;
    let Some(window) = window else {
        return terminal::paint(&[], None, status, columns).map(|()| false);
    };
    let frame = match viewport::project(
        window,
        cursor,
        columns,
        rows.saturating_sub(1),
        *top,
    ) {
        Ok(frame) => frame,
        Err(error) => {
            return terminal::paint(&[], None, viewport_error(error), columns)
                .map(|()| false);
        }
    };
    *top = frame.top;
    terminal::paint(&frame.lines, frame.cursor, status, columns).map(|()| true)
}

const fn viewport_error(error: viewport::Error) -> &'static str {
    match error {
        viewport::Error::ColumnContext => {
            "Tab column unavailable after oversized grapheme"
        }
        viewport::Error::TooNarrow => "Widen terminal to display source",
        viewport::Error::InvalidWindow => {
            "Source view unavailable; resize or refresh"
        }
    }
}
