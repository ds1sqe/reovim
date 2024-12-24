use {
    futures::{StreamExt, future::FutureExt, select},
    futures_timer::Delay,
    reovim_core::{
        api::{cursor, disable_raw_mode, enable_raw_mode},
        command::terminal::{Clear, ClearType},
        event::{
            DisableMouseCapture, EnableMouseCapture, Event, EventStream,
            KeyCode,
        },
        macros::execute,
    },
    std::{io, io::Write, time::Duration},
};

async fn print_events() {
    let mut reader = EventStream::new();

    loop {
        let mut delay = Delay::new(Duration::from_millis(1_000)).fuse();
        let mut event = reader.next().fuse();

        select! {
            _ = delay => {},
            maybe_event = event => {
                match maybe_event {
                    Some(Ok(event)) => {
                        let mut stdout = io::stdout();

                        if event == Event::Key(KeyCode::Char('c').into()) {
                            print!("\rCursor position: {:?}", cursor::position());
                        } else {
                            print!("\rEvent::{:?}", event);
                        }
                        execute!(stdout, Clear(ClearType::FromCursorDown)).unwrap();
                        stdout.flush().unwrap();

                        if event == Event::Key(KeyCode::Esc.into()) {
                            break;
                        }
                    }
                    Some(Err(e)) => print!("Error: {:?}", e),
                    None => break,
                }
            }
        };
    }
}

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnableMouseCapture)?;
    execute!(stdout, Clear(ClearType::All))?;

    print_events().await;

    let mut stdout = io::stdout();
    execute!(stdout, DisableMouseCapture)?;

    disable_raw_mode()
}
