use esp_idf_svc::hal::task::thread::ThreadSpawnConfiguration;
use esp_idf_svc::sys;

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
