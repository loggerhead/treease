#[cfg(target_arch = "wasm32")]
use std::{
    alloc::{Layout, alloc, dealloc},
    ffi::c_void,
    ptr,
    sync::Once,
};

#[cfg(target_arch = "wasm32")]
const ALIGN: usize = 16;
#[cfg(target_arch = "wasm32")]
const HEADER_SIZE: usize = std::mem::size_of::<usize>();
#[cfg(target_arch = "wasm32")]
const HEADER_LEN: usize = ((HEADER_SIZE + ALIGN - 1) / ALIGN) * ALIGN;

#[cfg(target_arch = "wasm32")]
pub(crate) fn install_tree_sitter_allocator() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| unsafe {
        tree_sitter::set_allocator(
            Some(ts_malloc),
            Some(ts_calloc),
            Some(ts_realloc),
            Some(ts_free),
        );
    });
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn install_tree_sitter_allocator() {}

#[cfg(target_arch = "wasm32")]
fn allocation_layout(size: usize) -> Option<Layout> {
    let payload_size = size.max(1);
    let total_size = HEADER_LEN.checked_add(payload_size)?;
    Layout::from_size_align(total_size, ALIGN).ok()
}

#[cfg(target_arch = "wasm32")]
unsafe extern "C" fn ts_malloc(size: usize) -> *mut c_void {
    let Some(layout) = allocation_layout(size) else {
        return ptr::null_mut();
    };

    let base = unsafe { alloc(layout) };
    if base.is_null() {
        return ptr::null_mut();
    }

    unsafe {
        (base as *mut usize).write(size);
        base.add(HEADER_LEN).cast::<c_void>()
    }
}

#[cfg(target_arch = "wasm32")]
unsafe extern "C" fn ts_calloc(nmemb: usize, size: usize) -> *mut c_void {
    let Some(total) = nmemb.checked_mul(size) else {
        return ptr::null_mut();
    };

    let payload = unsafe { ts_malloc(total) };
    if !payload.is_null() && total != 0 {
        unsafe {
            ptr::write_bytes(payload.cast::<u8>(), 0, total);
        }
    }
    payload
}

#[cfg(target_arch = "wasm32")]
unsafe extern "C" fn ts_realloc(old_ptr: *mut c_void, size: usize) -> *mut c_void {
    if old_ptr.is_null() {
        return unsafe { ts_malloc(size) };
    }
    if size == 0 {
        unsafe { ts_free(old_ptr) };
        return ptr::null_mut();
    }

    let old_size = unsafe { allocation_size(old_ptr) };
    let new_ptr = unsafe { ts_malloc(size) };
    if new_ptr.is_null() {
        return ptr::null_mut();
    }

    let copy_len = old_size.min(size);
    if copy_len != 0 {
        unsafe {
            ptr::copy_nonoverlapping(old_ptr.cast::<u8>(), new_ptr.cast::<u8>(), copy_len);
        }
    }
    unsafe { ts_free(old_ptr) };
    new_ptr
}

#[cfg(target_arch = "wasm32")]
unsafe extern "C" fn ts_free(allocation: *mut c_void) {
    if allocation.is_null() {
        return;
    }

    let size = unsafe { allocation_size(allocation) };
    let Some(layout) = allocation_layout(size) else {
        return;
    };
    let base = unsafe { allocation.cast::<u8>().sub(HEADER_LEN) };
    unsafe { dealloc(base, layout) };
}

#[cfg(target_arch = "wasm32")]
unsafe fn allocation_size(allocation: *mut c_void) -> usize {
    let base = unsafe { allocation.cast::<u8>().sub(HEADER_LEN) };
    unsafe { (base as *const usize).read() }
}
