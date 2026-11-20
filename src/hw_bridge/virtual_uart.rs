//! A local UART endpoint created by Capybara itself, with no external pairing
//! utility. The external tool opens the slave, while Capybara owns the master.
//!
//! PTYs are usable by native serial tools. Browser Web Serial enumeration is a
//! separate OS service and does not necessarily list pseudoterminals.

#[cfg(unix)]
mod unix {
    use std::ffi::CStr;
    use std::fs::File;
    use std::io::{self, Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd};

    pub struct VirtualUart {
        master: File,
        // Retain one slave descriptor: closing a client must not tear down the
        // endpoint or make the master spin on EOF/EIO before another connects.
        _slave: File,
        path: String,
    }

    impl VirtualUart {
        pub fn create() -> io::Result<Self> {
            let mut master = -1;
            let mut slave = -1;
            let mut name = [0 as libc::c_char; 256];
            // openpty initializes both descriptors and writes a NUL-terminated
            // name. Adopt them immediately so all later error paths close both.
            if unsafe { libc::openpty(&mut master, &mut slave, name.as_mut_ptr(), std::ptr::null_mut(), std::ptr::null_mut()) } != 0 {
                return Err(io::Error::last_os_error());
            }
            let master = unsafe { File::from_raw_fd(master) };
            let slave = unsafe { File::from_raw_fd(slave) };
            let path = unsafe { CStr::from_ptr(name.as_ptr()) }.to_string_lossy().into_owned();
            let mut settings = std::mem::MaybeUninit::<libc::termios>::uninit();
            if unsafe { libc::tcgetattr(slave.as_raw_fd(), settings.as_mut_ptr()) } != 0 {
                return Err(io::Error::last_os_error());
            }
            let mut settings = unsafe { settings.assume_init() };
            unsafe { libc::cfmakeraw(&mut settings) };
            settings.c_cflag |= libc::CLOCAL | libc::CREAD;
            settings.c_cc[libc::VMIN] = 0;
            settings.c_cc[libc::VTIME] = 0;
            if unsafe { libc::tcsetattr(slave.as_raw_fd(), libc::TCSANOW, &settings) } != 0 {
                return Err(io::Error::last_os_error());
            }
            let flags = unsafe { libc::fcntl(master.as_raw_fd(), libc::F_GETFL) };
            if flags < 0 || unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
                return Err(io::Error::last_os_error());
            }
            for fd in [master.as_raw_fd(), slave.as_raw_fd()] {
                let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
                if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(Self { master, _slave: slave, path })
        }

        pub fn path(&self) -> &str { &self.path }
    }

    impl Read for VirtualUart {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> { self.master.read(bytes) }
    }

    impl Write for VirtualUart {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { self.master.write(bytes) }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
}

#[cfg(unix)]
pub use unix::VirtualUart;

#[cfg(all(test, unix))]
mod tests {
    use super::VirtualUart;
    use std::fs::OpenOptions;
    use std::io::{Read, Write};
    use std::os::unix::fs::OpenOptionsExt;

    #[test]
    fn pty_is_binary_safe_and_survives_client_reconnects() {
        let mut port = VirtualUart::create().unwrap();
        let mut client = OpenOptions::new().read(true).write(true)
            .custom_flags(libc::O_NONBLOCK).open(port.path()).unwrap();
        let data: Vec<u8> = (0..=255).collect();
        client.write_all(&data).unwrap();
        let mut received = Vec::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while received.len() < data.len() && std::time::Instant::now() < deadline {
            let mut buf = [0; 512];
            match port.read(&mut buf) {
                Ok(n) => received.extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {},
                other => panic!("PTY read failed: {other:?}"),
            }
            std::thread::yield_now();
        }
        assert_eq!(received, data);
        port.write_all(&data).unwrap();
        received.clear();
        while received.len() < data.len() && std::time::Instant::now() < deadline {
            let mut buf = [0; 512];
            match client.read(&mut buf) {
                Ok(n) => received.extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {},
                other => panic!("Client read failed: {other:?}"),
            }
            std::thread::yield_now();
        }
        assert_eq!(received, data);
        drop(client);
        let mut client = OpenOptions::new().read(true).write(true).open(port.path()).unwrap();
        client.write_all(b"reconnected").unwrap();
        let mut buf = [0; 64];
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let count = loop {
            match port.read(&mut buf) {
                Ok(n) if n > 0 => break n,
                Ok(_) => {},
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {},
                other => panic!("Reconnect read failed: {other:?}"),
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        };
        assert_eq!(&buf[..count], b"reconnected");
    }
}
