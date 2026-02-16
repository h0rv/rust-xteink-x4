use esp_idf_svc::hal::task::thread::ThreadSpawnConfiguration;
use esp_idf_svc::sys;
use std::fs;
use std::path::Path;

const EPUB_WORKER_THREAD_STACK_BYTES: usize = 72 * 1024;

/// Log heap usage statistics and current task stack headroom.
pub fn log_heap(label: &str) {
    let free_heap = unsafe { sys::esp_get_free_heap_size() };
    let min_free = unsafe { sys::esp_get_minimum_free_heap_size() };
    let free_8bit = unsafe { sys::heap_caps_get_free_size(sys::MALLOC_CAP_8BIT) };
    let largest_8bit = unsafe { sys::heap_caps_get_largest_free_block(sys::MALLOC_CAP_8BIT) };
    let stack_hwm_words = unsafe { sys::uxTaskGetStackHighWaterMark(core::ptr::null_mut()) };
    let stack_hwm_bytes = (stack_hwm_words as usize) * core::mem::size_of::<sys::StackType_t>();
    log::info!(
        "[MEM] {}: free={} min_free={} free_8bit={} largest_8bit={} stack_hwm={}B",
        label,
        free_heap,
        min_free,
        free_8bit,
        largest_8bit,
        stack_hwm_bytes
    );
}

/// Configure pthread defaults used by `std::thread` workers on ESP-IDF.
pub fn configure_pthread_defaults() {
    let mut config = ThreadSpawnConfiguration::default();
    config.stack_size = EPUB_WORKER_THREAD_STACK_BYTES;
    config.priority = 1;
    config.inherit = false;

    if let Err(err) = config.set() {
        log::warn!("Failed to configure pthread defaults: {}", err);
    } else {
        log::info!(
            "Configured pthread defaults: stack_size={} priority={}",
            config.stack_size,
            config.priority
        );
    }
}

const DIAG_LOG_PATH: &str = "/sd/.xteink/logs/runtime.log";
const DIAG_LOG_MAX_BYTES: usize = 32 * 1024;
const DIAG_LOG_KEEP_TAIL_BYTES: usize = 16 * 1024;

pub fn append_diag(event: &str) {
    let path = Path::new(DIAG_LOG_PATH);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let now_ms = unsafe { sys::esp_timer_get_time() / 1_000 };
    let mut line = String::new();
    line.push_str(&now_ms.to_string());
    line.push('\t');
    line.push_str(event);
    line.push('\n');

    let mut content = fs::read(path).unwrap_or_default();
    content.extend_from_slice(line.as_bytes());
    if content.len() > DIAG_LOG_MAX_BYTES {
        let keep = DIAG_LOG_KEEP_TAIL_BYTES.min(content.len());
        let start = content.len().saturating_sub(keep);
        content = content[start..].to_vec();
    }
    let _ = fs::write(path, content);
}
