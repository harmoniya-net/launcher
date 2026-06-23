use std::sync::OnceLock;
use std::time::Duration;

use reqwest::Client;
use tokio::runtime::{Handle, Runtime};

/// Process-wide tokio runtime. reqwest's async client requires a tokio reactor;
/// we keep one multi-thread runtime alive for the life of the process and
/// dispatch every HTTP future onto it via `Handle::spawn`.
fn runtime() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("harmoniya-tokio")
            .build()
            .expect("build tokio runtime")
    })
}

pub fn handle() -> Handle {
    runtime().handle().clone()
}

/// Process-wide async HTTP client. Connection pool is reused across calls.
pub fn client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        let _guard = runtime().enter();
        Client::builder()
            .user_agent(concat!("harmoniya-launcher/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            // Trust the union of the bundled Mozilla roots and the OS
            // certificate store. Native roots cover machines behind an
            // AV/corporate-proxy TLS interceptor (whose root is only in the OS
            // store); webpki roots are the fallback if the OS store fails to
            // load. See the `reqwest` features note in the workspace Cargo.toml.
            .tls_built_in_webpki_certs(true)
            .tls_built_in_native_certs(true)
            .build()
            .expect("reqwest build")
    })
}

/// Run a reqwest future on the tokio runtime, then resume on the caller's executor.
/// Use this from `cx.spawn` blocks so reqwest's async IO has a tokio reactor.
/// A join error means the task panicked or the runtime was dropped — both fatal.
pub async fn on_tokio<F, T>(fut: F) -> T
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    handle().spawn(fut).await.expect("tokio task failed (panic or runtime gone)")
}
