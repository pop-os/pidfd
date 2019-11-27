//! Demonstration proving that children are awaited concurrently.
//!
//! Children will be spawned from first to last, and shall return from last to first --
//! proving that processes are awaited concurrently.

use pidfd::PidFd;
use std::{io, process::Command};

fn main() {
    futures::executor::block_on(async move {
        futures::try_join!(
            spawn_sleeper("1", "5"),
            spawn_sleeper("2", "4"),
            spawn_sleeper("3", "3"),
            spawn_sleeper("4", "2"),
            spawn_sleeper("5", "1"),
        )
        .unwrap();
    })
}

async fn spawn_sleeper(id: &str, timeout: &str) -> io::Result<()> {
    println!("started job {}", id);
    let child = Command::new("/bin/sleep").arg(timeout).spawn().unwrap();
    PidFd::from(child).await?;
    println!("finished job {}", id);
    Ok(())
}
