use std::cell::RefCell;
use std::io::{Result, Write};
use std::sync::Once;

thread_local! {
    static CAPTURED: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
}

struct ThreadSink;

impl Write for ThreadSink {
    fn write(&mut self, bytes: &[u8]) -> Result<usize> {
        CAPTURED.with(|captured| {
            if let Some(buffer) = captured.borrow_mut().as_mut() {
                buffer.extend_from_slice(bytes);
            }
        });
        Ok(bytes.len())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

pub fn install() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        tracing::subscriber::set_global_default(
            tracing_subscriber::fmt()
                .with_writer(|| ThreadSink)
                .with_ansi(false)
                .finish(),
        )
        .unwrap();
    });
}

pub fn capture_this_thread() {
    install();
    CAPTURED.with(|captured| *captured.borrow_mut() = Some(Vec::new()));
}

pub fn captured_on_this_thread() -> String {
    CAPTURED.with(|captured| String::from_utf8(captured.borrow_mut().take().unwrap()).unwrap())
}
