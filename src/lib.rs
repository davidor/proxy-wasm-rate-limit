use limitador::counter::Counter;
use limitador::limit::Limit;
use limitador::storage::wasm::{Clock, WasmStorage};
use limitador::storage::Storage;
use limitador::RateLimiter;
use proxy_wasm::hostcalls::get_current_time;
use proxy_wasm::traits::*;
use proxy_wasm::types::*;
use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

// Assume that everything belongs to the same namespace for now
const NAMESPACE: &str = "proxy_wasm";

const SHARED_DATA_KEY: &str = "counters";

#[no_mangle]
pub fn _start() {
    proxy_wasm::set_log_level(LogLevel::Trace);
    proxy_wasm::set_http_context(|_context_id, _| -> Box<dyn HttpContext> { Box::new(Limiter {}) })
}

struct WasmClock {}
impl Clock for WasmClock {
    fn get_current_time(&self) -> SystemTime {
        get_current_time().unwrap()
    }
}

struct Limiter {}

impl Context for Limiter {}

impl Limiter {
    // TODO: Notice that the counters are stored in the "shared_data" space.
    // This is not efficient, we need to find a way to keep this state across
    // requests without having to serialize/deserialize everything.

    pub fn get_stored_counters(&self) -> Option<HashMap<Counter, SystemTime>> {
        let (stored_data, _) = self.get_shared_data(SHARED_DATA_KEY);

        match stored_data {
            Some(data) => {
                Some(bincode::deserialize::<HashMap<Counter, SystemTime>>(&data[..]).unwrap())
            }
            None => None,
        }
    }

    pub fn store_counters(&self, counters: HashSet<Counter>) {
        let mut counters_to_store: HashMap<Counter, SystemTime> = HashMap::new();

        for counter in counters {
            counters_to_store.insert(
                counter.clone(),
                // We can unwrap() expires_in() because the call to
                // get_counters() always sets it.
                get_current_time().unwrap() + counter.expires_in().unwrap(),
            );
        }

        self.set_shared_data(
            SHARED_DATA_KEY,
            Some(&bincode::serialize(&counters_to_store).unwrap()),
            None,
        )
        .unwrap();
    }
}

impl HttpContext for Limiter {
    fn on_http_request_headers(&mut self, _: usize) -> Action {
        let clock = Box::new(WasmClock {});
        let storage = WasmStorage::new(clock);

        let limiter = match self.get_stored_counters() {
            Some(stored_counters) => {
                stored_counters.iter().for_each(|(counter, expires_at)| {
                    // We know that counter.remaining() has been set because
                    // get_counters() always set it, so we can unwrap().
                    storage.add_counter(counter, counter.remaining().unwrap(), *expires_at);
                });

                new_limiter(storage)
            }
            None => new_limiter(storage),
        };

        let kv_for_auth = key_vals_for_authorizing(&self.get_http_request_headers());

        match limiter.is_rate_limited(NAMESPACE, &kv_for_auth, 1) {
            Ok(is_limited) => {
                if is_limited {
                    self.send_http_response(429, vec![], Some(b"Too many requests.\n"));
                    Action::Pause
                } else {
                    limiter.update_counters(NAMESPACE, &kv_for_auth, 1).unwrap();
                    self.store_counters(limiter.get_counters(NAMESPACE).unwrap());
                    Action::Continue
                }
            }
            Err(_) => {
                self.send_http_response(403, vec![], Some(b"Access forbidden.\n"));
                Action::Pause
            }
        }
    }
}

/// Returns the key-values used for authorizing. The keys have the following
/// format:
/// - Request path: req.path
/// - Request method: req.method
/// - Request headers: req.headers._name_of_the_header_
fn key_vals_for_authorizing(request_headers: &[(String, String)]) -> HashMap<String, String> {
    let mut result: HashMap<String, String> = HashMap::new();

    for (header_name, header_val) in request_headers {
        if header_name.starts_with(':') {
            if *header_name == ":path" {
                result.insert("req.path".to_string(), header_val.to_string());
            } else if *header_name == ":method" {
                result.insert("req.method".to_string(), header_val.to_string());
            } // Ignore :authority
        } else {
            result.insert(
                format!("req.headers.{}", header_name.to_lowercase()), // Case-insensitive
                header_val.to_string(),
            );
        }
    }

    result
}

fn new_limiter(storage: WasmStorage) -> RateLimiter {
    // TODO: for now the limits are in limits.rs. That's because we can't read a
    // file in WASM. Of course, this is not ideal because the program needs to
    // be recompiled every time there's a change in the limits.
    let limits = include!("limits.rs");

    let b: Box<dyn Storage> = Box::new(storage);
    let limiter = RateLimiter::new_with_storage(b);

    for limit in limits {
        limiter.add_limit(&limit).unwrap();
    }

    limiter
}
