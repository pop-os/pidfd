#[cfg(not(target_os = "linux"))]
compile_error!("PidFd is only supported on Linux >= 5.3");

use std::{
    convert::TryInto,
    future::Future,
    io,
    mem::MaybeUninit,
    os::unix::{
        io::{AsRawFd, RawFd},
        process::ExitStatusExt,
    },
    pin::Pin,
    process::ExitStatus,
    task::{Context, Poll},
};

const PIDFD_OPEN: libc::c_int = 434;
const PID_SEND: libc::c_int = 424;
const P_PIDFD: libc::idtype_t = 3;

/// A file descriptor which refers to a process
pub struct PidFd(RawFd);

impl PidFd {
    /// Converts a `Child` into a `PidFd`; validating if the PID is in range
    pub fn from_std_checked(child: std::process::Child) -> io::Result<Self> {
        child
            .id()
            .try_into()
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::Other,
                    "child process ID is outside the range of libc::pid_t",
                )
            })
            .and_then(|pid| unsafe { Self::open(pid, 0) })
    }

    /// Creates a pidfd from a PID
    pub unsafe fn open(pid: libc::pid_t, flags: libc::c_uint) -> io::Result<Self> {
        let pidfd = pidfd_create(pid, flags);
        if -1 == pidfd {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(pidfd))
        }
    }

    /// Sends a signal to the process owned by this pidfd
    pub unsafe fn send_raw_signal(
        &self,
        sig: libc::c_int,
        info: *const libc::siginfo_t,
        flags: libc::c_uint,
    ) -> io::Result<()> {
        if -1 == pidfd_send_signal(self.0, sig, info, flags) {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

impl Future for PidFd {
    type Output = io::Result<ExitStatus>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let poll_fds = &mut [libc::pollfd {
            fd: self.0,
            events: libc::POLLIN,
            revents: 0,
        }][..];

        let returned = unsafe { libc::poll(poll_fds as *mut _ as *mut libc::pollfd, 1, 0) };

        if 0 == returned {
            ctx.waker().wake_by_ref();
            Poll::Pending
        } else if -1 == returned {
            Poll::Ready(Err(io::Error::last_os_error()))
        } else {
            #[cfg(feature = "waitid")]
            {
                Poll::Ready(waitid(self.0))
            }

            #[cfg(not(feature = "waitid"))]
            {
                Poll::Ready(Ok(ExitStatus::from_raw(0)))
            }
        }
    }
}

impl AsRawFd for PidFd {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

impl Drop for PidFd {
    fn drop(&mut self) {
        let _ = unsafe { libc::close(self.0) };
    }
}

impl From<std::process::Child> for PidFd {
    fn from(child: std::process::Child) -> Self {
        Self::from_std_checked(child).unwrap()
    }
}

extern "C" {
    fn syscall(num: libc::c_int, ...) -> libc::c_int;
}

unsafe fn pidfd_create(pid: libc::pid_t, flags: libc::c_uint) -> libc::c_int {
    syscall(PIDFD_OPEN, pid, flags)
}

unsafe fn pidfd_send_signal(
    pidfd: libc::c_int,
    sig: libc::c_int,
    info: *const libc::siginfo_t,
    flags: libc::c_uint,
) -> libc::c_int {
    syscall(PID_SEND, pidfd, sig, info, flags)
}

#[cfg(feature = "waitid")]
fn waitid(pidfd: RawFd) -> io::Result<ExitStatus> {
    unsafe {
        let mut info = MaybeUninit::<libc::siginfo_t>::uninit();
        let exit_status = libc::waitid(P_PIDFD, pidfd as u32, info.as_mut_ptr(), libc::WEXITED);
        if -1 == exit_status {
            Err(io::Error::last_os_error())
        } else {
            Ok(ExitStatus::from_raw(info.assume_init().si_errno))
        }
    }
}
