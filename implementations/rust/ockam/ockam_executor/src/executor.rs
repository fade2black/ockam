use core::cell::UnsafeCell;
use core::future::Future;
use core::mem::MaybeUninit;
use core::pin::Pin;
use core::sync::atomic::{self, AtomicBool, AtomicUsize, Ordering};
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use crossbeam_queue::SegQueue;
use ockam_core::compat::boxed::Box;
use ockam_core::compat::collections::BTreeMap;
use ockam_core::compat::sync::{Arc, Mutex, RwLock};
use ockam_core::compat::task::Wake;
use ockam_core::compat::vec::Vec;

use pin_utils::pin_mut;

/// Returns current executor.
/// WARNING: TODO this is not thread-safe
pub fn current() -> &'static Executor<'static> {
    static INIT: AtomicBool = AtomicBool::new(false);
    static mut EXECUTOR: UnsafeCell<MaybeUninit<Executor>> = UnsafeCell::new(MaybeUninit::uninit());

    if INIT.load(Ordering::Relaxed) {
        unsafe { &*(EXECUTOR.get() as *const Executor) }
    } else {
        unsafe {
            let executorp = EXECUTOR.get() as *mut Executor;
            executorp.write(Executor::new());
            atomic::compiler_fence(Ordering::Release);
            INIT.store(true, Ordering::Relaxed);
            &*executorp
        }
    }
}

/// Executor
pub struct Executor<'a> {
    tasks: UnsafeCell<BTreeMap<TaskId, Box<Task>>>,
    waker_cache: UnsafeCell<BTreeMap<TaskId, Waker>>,
    // TODO tasks: Arc<Mutex<BTreeMap<TaskId, Box<Task>>>>,
    // TODO waker_cache: Arc<Mutex<BTreeMap<TaskId, Waker>>>,
    task_queue: Arc<SegQueue<TaskId>>,
    marker: core::marker::PhantomData<&'a ()>,
}

impl<'a> Executor<'a> {
    pub fn new() -> Self {
        Self {
            tasks: UnsafeCell::new(BTreeMap::new()),
            waker_cache: UnsafeCell::new(BTreeMap::new()),
            // TODO tasks: Arc::new(Mutex::new(BTreeMap::new())),
            // TODO waker_cache: Arc::new(Mutex::new(BTreeMap::new())),
            task_queue: Arc::new(SegQueue::new()),
            marker: core::marker::PhantomData,
        }
    }

    pub fn block_on<T>(&self, future: impl Future<Output = T>) -> T {
        let mut node = Node {
            id: TaskId::new(),
            name: "Node",
            future: UnsafeCell::new(future),
        };
        let node_waker = NodeWaker::new(node.id);

        let result = loop {
            // progress on main task
            let mut context = Context::from_waker(&node_waker);
            if let Poll::Ready(result) = node.poll(&mut context) {
                // exit main task
                break result;
            }

            let mut last_task = node.id.0;
            let mut task_budget = self.task_queue.len();

            // task executor loop
            while let Some(task_id) = self.task_queue.pop() {
                // yield to looping tasks
                if (task_id.0) == last_task {
                    self.task_queue.push(task_id);
                    break;
                } else {
                    last_task = task_id.0;
                }

                // poll task
                self.poll_task(task_id);

                // don't loop through all tasks more than once without running main
                if task_budget == 0 {
                    break;
                }
                task_budget -= 1;
            }
            self.sleep_if_idle();
        };
        result
    }

    /// poll_task
    fn poll_task(&self, task_id: TaskId) {
        let tasks = unsafe {
            let tasksp = self.tasks.get() as *mut BTreeMap<TaskId, Box<Task>>;
            &mut (*tasksp)
        };
        let task = match tasks.get_mut(&task_id) {
            Some(task) => {
                //let task_count = NEXT_ID.load(Ordering::Relaxed);
                //trace!("[executor] poll task: {}@{} / {}", task.name, task.id.0, task_count);
                task
            }
            None => {
                warn!("[executor] No task for id: {:?}", task_id);
                return;
            }
        };

        let waker_cache = unsafe {
            let waker_cachep = self.waker_cache.get() as *mut BTreeMap<TaskId, Waker>;
            &mut (*waker_cachep)
        };
        let waker = waker_cache
            .entry(task_id)
            .or_insert_with(|| TaskWaker::new(task_id, self.task_queue.clone()));

        let mut context = Context::from_waker(waker);
        match task.poll(&mut context) {
            Poll::Ready(()) => {
                // task completed, remove it and its cached waker
                if let Some(task) = tasks.remove(&task_id) {
                    drop(task);
                }
                waker_cache.remove(&task_id);
            }
            Poll::Pending => (),
        }
    }

    /// spawn
    pub fn spawn(&self, future: impl Future + 'static) {
        let task = Task::allocate(future);
        debug!("[executor] spawning task: {}", task.id.0);
        self.task_queue.push(task.id);
        let tasks = unsafe {
            let tasksp = self.tasks.get() as *mut BTreeMap<TaskId, Box<Task>>;
            &mut (*tasksp)
        };
        if tasks.insert(task.id, task).is_some() {
            panic!("[executor] task with same id already exists");
        }
        #[cfg(feature = "debug_alloc")]
        crate::debug_alloc::stats();
    }

    pub fn spawn_with_name(&self, name: &'static str, future: impl Future + 'static) {
        let task = Task::allocate_with_name(name, future);
        debug!("[executor] spawning task: {}@{}", name, task.id.0);
        self.task_queue.push(task.id);
        let tasks = unsafe {
            let tasksp = self.tasks.get() as *mut BTreeMap<TaskId, Box<Task>>;
            &mut (*tasksp)
        };
        if tasks.insert(task.id, task).is_some() {
            panic!("[executor] task with same id already exists");
        }
        #[cfg(feature = "debug_alloc")]
        crate::debug_alloc::stats();
    }

    fn sleep_if_idle(&self) {
        // TODO disable interrupts
        if self.task_queue.is_empty() {
            // TODO sleep
        }
    }
}

impl<'a> Default for Executor<'a> {
    fn default() -> Self {
        Self::new()
    }
}

// - Task ---------------------------------------------------------------------

type Task = Node<dyn Future<Output = ()> + 'static>;

/// Node
pub struct Node<F>
where
    F: ?Sized,
{
    id: TaskId,
    name: &'static str,
    future: UnsafeCell<F>,
    // TODO future: Pin<Box<F>>,
}

impl<F> Drop for Node<F>
where
    F: ?Sized,
{
    fn drop(&mut self) {
        debug!("[executor] dropped task: {}", self.id.0);
    }
}

impl<F, T> Node<F>
where
    F: ?Sized + Future<Output = T>,
{
    fn poll(&mut self, context: &mut Context) -> Poll<T> {
        //self.future.as_mut().poll(context)
        let future = unsafe {
            let futurep = self.future.get() as *mut F;
            &mut (*futurep)
        };
        unsafe { Pin::new_unchecked(future).poll(context) }
    }
}

impl Task {
    fn allocate(future: impl Future + 'static) -> Box<Task> {
        Box::new(Node {
            id: TaskId::new(),
            name: "Task",
            future: UnsafeCell::new(async {
                // task terminating
                future.await;
            }),
            // TODO future: Box::pin(future),
        })
    }

    fn allocate_with_name(name: &'static str, future: impl Future + 'static) -> Box<Task> {
        Box::new(Node {
            id: TaskId::new(),
            name: name,
            future: UnsafeCell::new(async {
                // task terminating
                future.await;
            }),
            // TODO future: Box::pin(future),
        })
    }
}

// - TaskId -------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TaskId(usize);

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

impl TaskId {
    fn new() -> Self {
        TaskId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

// - Waker --------------------------------------------------------------------

struct NodeWaker;
impl NodeWaker {
    #[allow(clippy::new_ret_no_self)]
    fn new(_task_id: TaskId) -> Waker {
        Waker::from(Arc::new(NodeWaker {}))
    }
}

impl Wake for NodeWaker {
    fn wake(self: Arc<Self>) {
        // no-op
    }
}

struct TaskWaker<'a> {
    task_id: TaskId,
    task_queue: Arc<SegQueue<TaskId>>,
    marker: core::marker::PhantomData<&'a ()>,
}

impl<'a> TaskWaker<'a> {
    fn new(task_id: TaskId, task_queue: Arc<SegQueue<TaskId>>) -> Waker {
        Waker::from(Arc::new(TaskWaker {
            task_id,
            task_queue,
            marker: core::marker::PhantomData,
        }))
    }

    fn reschedule_task(&self) {
        self.task_queue.push(self.task_id);
    }
}

impl<'a> Wake for TaskWaker<'a> {
    fn wake(self: Arc<Self>) {
        self.reschedule_task();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.reschedule_task();
    }
}
