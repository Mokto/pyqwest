use std::sync::OnceLock;

use tokio::runtime::{Builder, Runtime};

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

const WORKER_THREADS_VAR: &str = "PYQWEST_WORKER_THREADS";

/// The process-wide multi-threaded tokio runtime, built on first use.
///
/// Worker count follows tokio's default (one per available core) unless
/// `PYQWEST_WORKER_THREADS` overrides it. The default is a poor fit under a
/// container CPU limit: those are enforced by CFS quota rather than by
/// narrowing the affinity mask `available_parallelism` reads, so a process
/// granted a fraction of a large host still gets a worker per host core. Each
/// worker that allocates also keeps a glibc malloc arena alive for the life of
/// the process, so the cost of the extra workers is resident memory and not
/// just idle threads.
pub(crate) fn get_runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        let mut builder = Builder::new_multi_thread();
        builder.enable_all();

        if let Some(worker_threads) = configured_worker_threads(std::env::var(WORKER_THREADS_VAR)) {
            builder.worker_threads(worker_threads);
        }

        builder.build().expect("failed to build the tokio runtime")
    })
}

/// Ignores an unset, unparseable or zero value so the runtime keeps tokio's
/// default rather than failing an import over an environment typo.
fn configured_worker_threads<E>(value: Result<String, E>) -> Option<usize> {
    value
        .ok()?
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|count| *count > 0)
}

#[cfg(test)]
mod tests {
    use super::configured_worker_threads;

    #[derive(Debug)]
    struct NotSet;

    #[test]
    fn uses_a_valid_count() {
        assert_eq!(
            configured_worker_threads(Ok::<_, NotSet>(" 4 ".into())),
            Some(4)
        );
    }

    #[test]
    fn falls_back_to_the_tokio_default() {
        for value in ["", "0", "-1", "two", "4.5"] {
            assert_eq!(
                configured_worker_threads(Ok::<_, NotSet>(value.into())),
                None,
                "{value:?} should leave the default in place"
            );
        }
        assert_eq!(configured_worker_threads(Err(NotSet)), None);
    }
}
