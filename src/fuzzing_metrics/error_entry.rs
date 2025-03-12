use std::cell::UnsafeCell;
use std::sync::atomic::AtomicU64;

#[repr(C)]
pub struct ErrorEntry {
    pub(crate) count: AtomicU64,
    pub(crate) message: UnsafeCell<[u8; 128]>,
    pub(crate) is_active: AtomicU64,
}
