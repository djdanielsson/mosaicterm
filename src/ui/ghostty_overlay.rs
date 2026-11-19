//! Optional libghostty-vt 1.x renderer for the fullscreen overlay.

use std::ffi::{c_char, c_int, c_void, CStr};
use std::path::Path;
use std::ptr;

type Handle = *mut c_void;
type Release = unsafe extern "C" fn(Handle);
type Create = unsafe extern "C" fn(*const c_void, *mut Handle, u16, u16) -> c_int;
type Write = unsafe extern "C" fn(Handle, *const u8, usize);
type Resize = unsafe extern "C" fn(Handle, u16, u16, u32, u32) -> c_int;
type Set = unsafe extern "C" fn(Handle, c_int, *const c_void) -> c_int;
type FormatterCreate = unsafe extern "C" fn(*const c_void, *mut Handle, Handle) -> c_int;
type Format = unsafe extern "C" fn(Handle, *mut u8, usize, *mut usize) -> c_int;

const MAX_REPLY: usize = 8 * 1024;
const MAX_FRAME: usize = 16 * 1024 * 1024;

#[derive(Default)]
struct Replies(Vec<u8>);

pub(super) struct GhosttyOverlay {
    terminal: Handle,
    formatter: Handle,
    terminal_free: Release,
    formatter_free: Release,
    feed: Write,
    resize: Resize,
    format: Format,
    // The heap address remains stable across moves while the terminal holds it.
    replies: Box<Replies>,
    _library: libloading::Library,
}

impl GhosttyOverlay {
    pub fn load(path: &Path, rows: u16, cols: u16) -> anyhow::Result<Self> {
        anyhow::ensure!(path.is_absolute(), "libghostty-vt path must be absolute");
        // SAFETY: The caller selects a trusted Ghostty 1.x library. This wrapper
        // uses the published 1.0.0 C ABI. The library is held until both handles
        // are freed, and the overlay owns every call on the UI thread.
        unsafe {
            let library = libloading::Library::new(path)?;
            let version =
                library.get::<unsafe extern "C" fn() -> *const c_char>(b"ghostty_version")?;
            let version = CStr::from_ptr(version()).to_str()?;
            anyhow::ensure!(
                version.starts_with("1."),
                "Expected libghostty-vt 1.x, found {version}"
            );

            let create = *library.get::<Create>(b"ghostty_terminal_new")?;
            let set = *library.get::<Set>(b"ghostty_terminal_set")?;
            let format_create =
                *library.get::<FormatterCreate>(b"ghostty_formatter_terminal_new")?;
            let format_set = *library.get::<Set>(b"ghostty_formatter_set")?;
            let mut overlay = Self {
                terminal: ptr::null_mut(),
                formatter: ptr::null_mut(),
                terminal_free: *library.get(b"ghostty_terminal_free")?,
                formatter_free: *library.get(b"ghostty_formatter_free")?,
                feed: *library.get(b"ghostty_terminal_vt_write")?,
                resize: *library.get(b"ghostty_terminal_resize")?,
                format: *library.get(b"ghostty_formatter_format_buf")?,
                replies: Box::default(),
                _library: library,
            };
            check(create(ptr::null(), &mut overlay.terminal, cols, rows))?;
            let userdata = std::ptr::from_mut(overlay.replies.as_mut()).cast();
            check(set(overlay.terminal, 1, userdata))?; // USERDATA
            check(set(overlay.terminal, 2, write_pty as *const c_void))?; // WRITE_PTY
            check(format_create(
                ptr::null(),
                &mut overlay.formatter,
                overlay.terminal,
            ))?;

            let vt: c_int = 1;
            let trim = false;
            let extras: u32 = 1 << 6; // Include cursor position in the snapshot.
            check(format_set(overlay.formatter, 1, ptr::from_ref(&vt).cast()))?; // EMIT
            check(format_set(
                overlay.formatter,
                3,
                ptr::from_ref(&trim).cast(),
            ))?; // TRIM
            check(format_set(
                overlay.formatter,
                5,
                ptr::from_ref(&extras).cast(),
            ))?; // EXTRAS
            Ok(overlay)
        }
    }

    pub fn write(&mut self, bytes: &[u8]) {
        // SAFETY: The terminal and source buffer are live for the call.
        unsafe { (self.feed)(self.terminal, bytes.as_ptr(), bytes.len()) };
    }

    pub fn resize(&mut self, rows: u16, cols: u16, width: u32, height: u32) -> anyhow::Result<()> {
        // SAFETY: This overlay owns the live terminal handle.
        check(unsafe { (self.resize)(self.terminal, cols, rows, width, height) })
    }

    pub fn snapshot(&self) -> anyhow::Result<Vec<u8>> {
        let mut len = 0;
        // SAFETY: The library documents the null-buffer size query. The second
        // call receives a live allocation and cannot write beyond that size.
        unsafe {
            (self.format)(self.formatter, ptr::null_mut(), 0, &mut len);
            anyhow::ensure!(len <= MAX_FRAME, "libghostty-vt screen is too large");
            let mut bytes = vec![0; len];
            if len != 0 {
                check((self.format)(
                    self.formatter,
                    bytes.as_mut_ptr(),
                    bytes.len(),
                    &mut len,
                ))?;
                bytes.truncate(len);
            }
            Ok(bytes)
        }
    }

    pub fn take_replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.replies.0)
    }
}

unsafe extern "C" fn write_pty(_terminal: Handle, data: *mut c_void, bytes: *const u8, len: usize) {
    if data.is_null() || bytes.is_null() || len == 0 {
        return;
    }
    // SAFETY: This callback runs synchronously while Ghostty owns the source
    // buffer. Userdata points to the boxed reply queue installed by load().
    let queue = unsafe { &mut *data.cast::<Replies>() };
    if len <= MAX_REPLY.saturating_sub(queue.0.len()) {
        queue
            .0
            .extend_from_slice(unsafe { std::slice::from_raw_parts(bytes, len) });
    }
}

impl Drop for GhosttyOverlay {
    fn drop(&mut self) {
        // SAFETY: These handles are owned by this overlay. The borrowed
        // formatter is released before the terminal and shared library.
        unsafe {
            if !self.formatter.is_null() {
                (self.formatter_free)(self.formatter);
            }
            if !self.terminal.is_null() {
                (self.terminal_free)(self.terminal);
            }
        }
    }
}

fn check(result: c_int) -> anyhow::Result<()> {
    anyhow::ensure!(result == 0, "libghostty-vt returned error {result}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ghostty_screen_preserves_alternate_screen_unicode_and_color() {
        let Some(path) = std::env::var_os("MOSAICTERM_GHOSTTY_VT") else {
            return;
        };
        let mut overlay = GhosttyOverlay::load(Path::new(&path), 24, 80).unwrap();
        overlay.write(b"primary");
        overlay.write(b"\x1b[?1049h\x1b[31m");
        let text = "界 e\u{301}".as_bytes();
        overlay.write(&text[..2]);
        overlay.write(&text[2..]);
        let frame = String::from_utf8(overlay.snapshot().unwrap()).unwrap();
        assert!(frame.contains("界 e\u{301}"));
        assert!(frame.contains("\x1b[38;5;1m"), "{frame:?}");
        assert!(!frame.contains("primary"));
        overlay.resize(30, 100, 8, 16).unwrap();
        overlay.write(b"\x1b[6n");
        assert!(!overlay.take_replies().is_empty());
        overlay.write(b"\x1b[?1049l");
        let frame = String::from_utf8(overlay.snapshot().unwrap()).unwrap();
        assert!(frame.contains("primary"));
        assert!(!frame.contains('界'));
    }
}
