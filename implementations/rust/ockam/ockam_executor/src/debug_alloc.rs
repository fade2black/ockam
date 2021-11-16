use alloc_cortex_m::CortexMHeap;
use core::alloc::Layout;

use crate::debug;

// - heap ---------------------------------------------------------------------

#[global_allocator]
pub static ALLOCATOR: CortexMHeap = CortexMHeap::empty();

static mut HEAP_SIZE: usize = 0;

// - initialization -----------------------------------------------------------

pub fn init(heap_size: usize) {
    unsafe {
        HEAP_SIZE = heap_size;
        ALLOCATOR.init(cortex_m_rt::heap_start() as usize, heap_size)
    }
}

// - stats --------------------------------------------------------------------

pub fn stats() {
    debug!(
        "[executor] Heap usage: {} / {}  free: {}",
        ALLOCATOR.used(),
        unsafe { HEAP_SIZE },
        ALLOCATOR.free()
    );
}

// - debug allocator ----------------------------------------------------------

/// Debug allocator for tracking allocation stats.
///
/// Based on the `alloc-cortex-m` crate.
mod alloc_cortex_m {
    use core::alloc::{GlobalAlloc, Layout};
    use core::cell::RefCell;
    use core::ptr::{self, NonNull};

    use cortex_m::interrupt::Mutex;
    use linked_list_allocator::Heap;

    use crate::trace;

    pub struct CortexMHeap {
        heap: Mutex<RefCell<Heap>>,
    }

    impl CortexMHeap {
        /// Crate a new UNINITIALIZED heap allocator
        pub const fn empty() -> CortexMHeap {
            CortexMHeap {
                heap: Mutex::new(RefCell::new(Heap::empty())),
            }
        }

        /// Initializes the heap
        pub unsafe fn init(&self, start_addr: usize, size: usize) {
            cortex_m::interrupt::free(|cs| {
                self.heap.borrow(cs).borrow_mut().init(start_addr, size);
            });
        }

        /// Returns an estimate of the amount of bytes in use.
        pub fn used(&self) -> usize {
            cortex_m::interrupt::free(|cs| self.heap.borrow(cs).borrow_mut().used())
        }

        /// Returns an estimate of the amount of bytes available.
        pub fn free(&self) -> usize {
            cortex_m::interrupt::free(|cs| self.heap.borrow(cs).borrow_mut().free())
        }
    }

    unsafe impl GlobalAlloc for CortexMHeap {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let p = cortex_m::interrupt::free(|cs| {
                self.heap
                    .borrow(cs)
                    .borrow_mut()
                    .allocate_first_fit(layout)
                    .ok()
                    .map_or(ptr::null_mut(), |allocation| allocation.as_ptr())
            });
            trace!(
                "alloc -> {}\tused: {}\tfree: {}",
                layout.size(),
                self.used(),
                self.free()
            );
            p
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            cortex_m::interrupt::free(|cs| {
                self.heap
                    .borrow(cs)
                    .borrow_mut()
                    .deallocate(NonNull::new_unchecked(ptr), layout)
            });
            trace!(
                "dealloc -> {}\tused: {}\tfree: {}",
                layout.size(),
                self.used(),
                self.free()
            );
        }
    }
}
