use crate::{Action, Notice, Response, Update, session::Session};
use reovim_client_tui::operation::Operation;
use reovim_kernel_interface::ResourceId;
use std::{io, sync::mpsc};

pub fn run(
    session: &mut Session,
    factory: ResourceId,
    receive: &mpsc::Receiver<Action>,
    reply: &mpsc::Sender<Notice>,
) -> io::Result<()> {
    let result =
        session.attach(factory).map(|window| Update { window, accepted: None });
    let result = persisted(session, result);
    let _ = reply.send(Notice::Outcome(response(session, result)));
    while let Ok(action) = receive.recv() {
        if matches!(action, Action::Observe(_))
            && session.remote.pending.is_none()
            && !matches!(session.operation, Operation::Change(_))
        {
            session.accepted = None;
        }
        let result = match action {
            Action::Observe(start) => session.refresh(factory, start),
            Action::Insert(start, end, text, origin) => {
                session.replace(start, end, &text, origin)
            }
            Action::Undo => session.history(false),
            Action::Redo => session.history(true),
            Action::RecoverView => session.recover(),
            Action::Close => break,
        };
        let result =
            result.map(|window| Update { window, accepted: session.accepted });
        let result = persisted(session, result);
        if reply.send(Notice::Outcome(response(session, result))).is_err() {
            break;
        }
    }
    let result = session.close();
    let cleanup = persisted(session, result);
    if let Err(error) = &cleanup {
        report(session, error);
    }
    cleanup
}

fn persisted<T>(session: &Session, result: io::Result<T>) -> io::Result<T> {
    session.persist()?;
    result
}

const fn response(session: &Session, result: io::Result<Update>) -> Response {
    Response {
        result,
        receipt: match &session.remote.pending {
            Some(request) => Some(request.id()),
            None => session.remote.reserved,
        },
    }
}

fn report(session: &Session, error: &io::Error) {
    eprintln!("TUI cleanup retained: {}", error.to_string().escape_debug());
    eprintln!(
        "client={} access={:?} retiring={:?} reserved={:?}",
        session.remote.client,
        session.access,
        session.retiring,
        session.remote.reserved
    );
    if let Some(request) = &session.remote.pending {
        eprintln!("request={} response={:?}", request.id(), request.response());
    }
}
