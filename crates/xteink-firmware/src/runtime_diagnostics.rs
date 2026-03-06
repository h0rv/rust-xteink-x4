use esp_idf_svc::sys;

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
