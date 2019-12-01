use once_cell::sync::Lazy;
use std::{
    fs::File,
    io::{self, Read, Write},
    os::unix::io::{AsRawFd, FromRawFd, RawFd},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    task::Waker,
};

pub(crate) struct ReactorSender {
    fds: Arc<Mutex<Vec<(RawFd, Arc<AtomicBool>, Waker)>>>,
    cancel: File,
}

impl ReactorSender {
    pub fn send(&self, fd: RawFd, completed: Arc<AtomicBool>, waker: Waker) {
        self.fds.lock().unwrap().push((fd, completed, waker));
        let _ = self.cancel.try_clone().unwrap().write_all(b"0");
    }
}

fn create_pipe() -> (File, File) {
    let mut cancel_fds = [0; 2];
    unsafe { libc::pipe(&mut cancel_fds as *mut _ as *mut libc::c_int) };
    let reader = unsafe { File::from_raw_fd(cancel_fds[0]) };
    let writer = unsafe { File::from_raw_fd(cancel_fds[1]) };
    (reader, writer)
}

pub(crate) static REACTOR: Lazy<ReactorSender> = Lazy::new(|| {
    // Create a pipe to use as a cancellation mechanism.
    let (mut reader, writer) = create_pipe();

    let fds: Arc<Mutex<Vec<(RawFd, Arc<AtomicBool>, Waker)>>> = Arc::default();
    let fds_ = fds.clone();

    std::thread::spawn(move || {
        let fds = fds_;
        let mut pollers = Vec::new();
        let mut buffer = [0u8; 1];

        loop {
            pollers.clear();
            pollers.push(libc::pollfd {
                fd: reader.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            });

            for &(fd, _, _) in fds.lock().unwrap().iter() {
                pollers.push(libc::pollfd {
                    fd,
                    events: libc::POLLIN,
                    revents: 0,
                });
            }

            let returned = unsafe {
                let pollers: &mut [libc::pollfd] = &mut pollers;
                libc::poll(
                    pollers as *mut _ as *mut libc::pollfd,
                    pollers.len() as u64,
                    -1,
                )
            };

            if returned == -1 {
                panic!(
                    "fatal error in process reactor: {}",
                    io::Error::last_os_error()
                );
            } else if returned < 1 {
                continue;
            }

            if pollers[0].revents == libc::POLLIN {
                let _ = reader.read(&mut buffer);
                continue;
            }

            let pos = pollers[1..]
                .iter()
                .position(|event| event.revents == libc::POLLIN)
                .expect("unhandled event");

            let value = fds.lock().unwrap().swap_remove(pos);
            value.1.store(true, Ordering::SeqCst);
            value.2.wake();
        }
    });

    ReactorSender {
        fds,
        cancel: writer,
    }
});
