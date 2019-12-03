# pidfd

This Rust crate provides Linux (>= 5.3) PID file descriptor support. PID file descriptors are created from PIDs of processes, and are guaranteed to always reference the process from which the PID FD was created.

One of the benefits of using a PID FD is the ability to use `poll()`, `select()`, and `epoll()` to monitor when the process has terminated. This makes it ideal for use in asynchronous programming. This crate implements `std::future::Future` on the `PidFd` type so that multiple processes can be awaited concurrently.

> Linux 5.4 is required to use the `waitid` feature, which enables fetching the exit status of a pidfd.

```rust
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

    let exit_status = Command::new("/bin/sleep")
        .arg(timeout)
        .spawn()
        .map(|child| PidFd::from(&child))
        .unwrap()
        .into_future()
        .await?;

    println!("finished job {}: {}", id, exit_status);
    Ok(())
}

```

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

#### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
