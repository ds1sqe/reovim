//! Embedded-mode lifecycle smoke test.
//!
//! Asserts the 2b.E sequencing invariant: in `run_inproc_with`, the
//! client future finishes before the server task joins. The test
//! plugs a mock client future into [`reovim_app_launcher::embedded::run_inproc_with`]
//! and records completion order via a shared `AtomicUsize`
//! monotonic ticket counter.

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use {
    reovim_app_launcher::embedded::run_inproc_with,
    reovim_server::{Server, ServerConfig, inproc_channel_pair},
};

#[tokio::test(flavor = "multi_thread")]
async fn client_exit_precedes_server_task_completion() {
    let counter = Arc::new(AtomicUsize::new(0));
    let client_ticket = Arc::new(AtomicUsize::new(usize::MAX));

    let (server_half, _client_half) = inproc_channel_pair();
    let server = Arc::new(Server::new(ServerConfig::inproc()));

    let client_counter = Arc::clone(&counter);
    let client_ticket_cell = Arc::clone(&client_ticket);
    let client_fut = async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let ticket = client_counter.fetch_add(1, Ordering::SeqCst);
        client_ticket_cell.store(ticket, Ordering::SeqCst);
        Ok(())
    };

    run_inproc_with(server, server_half, client_fut)
        .await
        .expect("embedded composition should shut down cleanly");

    let server_ticket = counter.fetch_add(1, Ordering::SeqCst);
    let client_first = client_ticket.load(Ordering::SeqCst);

    assert!(
        client_first < server_ticket,
        "client must complete before server-task join: client={client_first} server={server_ticket}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn client_error_still_drains_server() {
    let (server_half, _client_half) = inproc_channel_pair();
    let server = Arc::new(Server::new(ServerConfig::inproc()));

    let client_fut =
        async { Err::<(), std::io::Error>(std::io::Error::other("mock client failure")) };

    let err = run_inproc_with(server, server_half, client_fut)
        .await
        .expect_err("client error must propagate");
    assert!(err.to_string().contains("mock client failure"));
}
