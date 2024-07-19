use std::sync::{Arc, Condvar, Mutex};

/// Manages a pool of threads used to execute arbitrary tasks in parallel.
/// This is similar to `rayon`'s `ThreadPool`, but provides the following additional functionality:
/// - Tasks are assigned priorities and executed in priority order, rather than FIFO
/// - Tasks that are queued but have not yet started executing can be cancelled
/// NB: When the `Tasks` is dropped, any pending tasks will be cancelled but any currently
/// executing tasks will finish normally
pub struct Tasks {
    /// Objects shared between the `Tasks` and its worker threads
    shared: Arc<TasksShared>,
    /// Number of worker threads in the pool
    thread_count: usize,
    /// Total number of tasks submitted so far, used to assign Task IDs
    total_tasks_submitted: usize,
}

impl Tasks {
    /// Create a new `Tasks` thread pool with the given number of threads
    pub fn new(thread_count: usize) -> Self {
        let shared = Arc::new(TasksShared {
            mutex: Mutex::new(TasksMutex {
                pending_tasks: Vec::new(),
                active_worker_threads: 0,
                terminate: false,
            }),
            pending_task_cond: Condvar::new(),
            finished_task_cond: Condvar::new(),
        });

        // start worker threads.
        // note that we do not need to keep the join handles to the worker threads; they are
        // completely detached
        for _ in 0..thread_count {
            // make a clone of the Arc for the worker thread
            let shared = shared.clone();

            std::thread::spawn(move || Self::worker(shared));
        }

        Self {
            shared,
            thread_count,
            total_tasks_submitted: 0,
        }
    }

    /// Submit a new task to the thread pool
    pub fn submit<TaskFn>(&mut self, priority: TaskPriority, task_fn: TaskFn) -> TaskId
    where
        TaskFn: FnOnce() + Send + Sync + 'static,
    {
        let mut lock = self.shared.mutex.lock().expect("`Tasks` mutex poisoned");

        let task_id = TaskId(self.total_tasks_submitted);
        self.total_tasks_submitted += 1;

        lock.pending_tasks.push((
            task_id,
            PendingTask {
                task_fn: Box::new(task_fn),
                priority,
            },
        ));

        // notify a sleeping worker thread that there is a new task
        self.shared.pending_task_cond.notify_one();

        task_id
    }

    /// Block the calling thread until all tasks have finished
    /// Returns the TaskId of the new task in the thread pool
    pub fn block_until_finished(&self) {
        loop {
            let lock = self.shared.mutex.lock().expect("`Tasks` mutex poisoned");

            if lock.pending_tasks.is_empty() && lock.active_worker_threads == 0 {
                break;
            }

            if let Err(e) = self
                .shared
                .finished_task_cond
                .wait_while(lock, |mutex_data| {
                    mutex_data.pending_tasks.is_empty() && mutex_data.active_worker_threads == 0
                })
            {
                log::error!("error blocking until finished: {}", e);
            }
        }
    }

    /// Attempt to cancel a submitted task if it is still pending execution
    /// Returns true if the task was successfully cancelled
    pub fn cancel_if_pending(&mut self, task_id: TaskId) -> bool {
        let mut lock = self.shared.mutex.lock().expect("`Tasks` mutex poisoned");

        lock.pending_tasks
            .iter()
            .position(|(tid, _)| *tid == task_id)
            .inspect(|&task_index| {
                lock.pending_tasks.remove(task_index);
            })
            .is_some()
    }

    /// Returns the original number of workers in the pool
    pub fn total_worker_count(&mut self) -> usize {
        self.thread_count
    }

    /// Returns the current number of active workers in the pool
    pub fn active_worker_count(&mut self) -> usize {
        let lock = self.shared.mutex.lock().expect("`Tasks` mutex poisoned");

        lock.active_worker_threads
    }

    /// Function run on the worker threads
    fn worker(shared: Arc<TasksShared>) {
        loop {
            let mut lock = shared.mutex.lock().expect("`Tasks` mutex poisoned");

            // wait for a pending task
            lock = shared
                .pending_task_cond
                .wait_while(lock, |info| {
                    info.pending_tasks.is_empty() && !info.terminate
                })
                .expect("`Tasks` mutex poisoned");

            // check if the thread should terminate
            if lock.terminate {
                break;
            }

            // get the task with the lowest priority value and remove it from the list
            let next_task_index = lock
                .pending_tasks
                .iter()
                .enumerate()
                .map(|(index, (_task_id, task))| (index, task))
                .min_by(|(_, task_a), (_, task_b)| task_a.priority.cmp(&task_b.priority))
                .expect("tasks should not be empty")
                .0;
            let next_task = lock.pending_tasks.remove(next_task_index).1;

            lock.active_worker_threads += 1;

            // drop the lock so that other threads can access the mutex while the task is processed
            drop(lock);

            // process the task
            (next_task.task_fn)();

            // re-acquire the lock in order to decrement `active_worker_threads`
            let mut lock = shared.mutex.lock().expect("`Tasks` mutex poisoned");
            lock.active_worker_threads -= 1;
            drop(lock);

            // wake the parent thread if it called `block_until_finished`
            shared.finished_task_cond.notify_all();
        }
    }
}

impl Drop for Tasks {
    fn drop(&mut self) {
        // set the `terminate` flag to true, so that worker threads will terminate after their
        // current task
        let mut lock = self.shared.mutex.lock().expect("`Tasks` mutex poisoned");
        lock.terminate = true;
    }
}

/// Identifier for a task submitted to `Tasks`
/// The IDs monotically increase for each new task, so a task ID will never be reused unless an
/// usize overflow occurs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskId(usize);

/// Priority of a task submitted to `Tasks`
/// As is tradition, smaller priority values represent higher priorities
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct TaskPriority {
    /// The priority of this "class" of tasks. Tasks with a smaller class priority value are always
    /// executed before those with a higher class priority value
    pub class_priority: i32,
    /// Additional priority allowing tasks to be ordered within classes
    pub priority_within_class: i32,
}

/// Struct shared between `Tasks` and the worker threads
struct TasksShared {
    /// Mutex guarding access to the pending tasks list and terminate flag
    mutex: Mutex<TasksMutex>,
    /// Condvar to wake the worker threads when a new task arrives
    pending_task_cond: Condvar,
    /// Condvar to wake the thread calling `block_until_finished` when a task is finished
    finished_task_cond: Condvar,
}

/// Struct shared between `Tasks` and the worker threads, guarded by a mutex
struct TasksMutex {
    /// Vec of tasks waiting to be executed
    pending_tasks: Vec<(TaskId, PendingTask)>,
    /// Flag for the worker threads to terminate themselves after the `Tasks` is dropped
    terminate: bool,
    /// Number of worker threads that are currently executing a task
    active_worker_threads: usize,
}

/// Represents a task that has been submitted to `Tasks` and is waiting to be executed
struct PendingTask {
    task_fn: Box<dyn FnOnce() + Send + Sync>,
    priority: TaskPriority,
}
