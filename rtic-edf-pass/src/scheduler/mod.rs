use crate::{
    task::{EdfTaskBinding, ScheduledTask, Task},
    types::Timestamp,
};

mod run_queue;
pub use run_queue::RunQueue;

pub type WaitQueue<const N: usize> = priority_queue::PriorityQueue<ScheduledTask, N>;

mod system_deadline;
pub use system_deadline::SystemDeadline;

pub use critical_section::CriticalSection;

#[cfg(feature = "benchmark")]
pub mod benchmark;

/// EDF scheduler. This trait is implemented at the `rtic-edf-pass` codegen
/// step.
pub trait Scheduler<const NUM_DISPATCH_PRIOS: usize, const Q_LEN: usize>: Sized {
    fn now() -> Timestamp;
    fn pend_dispatcher(idx: u16);

    fn run_queue(&self) -> &RunQueue<NUM_DISPATCH_PRIOS>;
    fn system_deadline(&self) -> &SystemDeadline;
    fn wait_queue(&self) -> &WaitQueue<Q_LEN>;

    /// Signal to the scheduler that a task wants to run.
    ///
    /// This function must be run either inside a critical section, or at the
    /// highest interrupt priority on the system.
    fn schedule(&self, cs: CriticalSection<'_>, task: Task) {
        #[cfg(feature = "defmt")]
        let rel_dl = task.rel_deadline();

        let now = Self::now();
        let task = task.into_scheduled(now);
        let sys_dl = self.system_deadline().load();

        #[cfg(feature = "defmt")]
        defmt::trace!(
            "[SCHEDULE] now: {}, rel dl: {}, abs dl: {}, sys dl: {}, dispatcher idx: {}, run queue idx: {}",
            now,
            rel_dl,
            task.abs_deadline(),
            sys_dl,
            task.dispatcher_index(),
            task.rq_index(),
        );

        // We can bypass the queue once per dispatcher priority level (even if the
        // task's deadline is farther in the future than the system deadline), by
        // directly pending the task in its dispatcher if the slot is empty. The
        // scheduling will then happen according to the NVIC's priority
        // configurations, which is what we want (ie, a tiny bit of hardware
        // acceleration to the rescue).
        //
        // This only works because every priority level only has one unique relative
        // deadline, such that no task can ever preempt another with the same deadline
        if task.abs_deadline() < sys_dl {
            #[cfg(feature = "defmt")]
            defmt::trace!("[DIRECT EXECUTE]");
            execute(self, &cs, task);
        } else {
            {
                #[cfg(feature = "defmt")]
                defmt::trace!("[ENQUEUE]");

                self.wait_queue()
                    .insert(task)
                    .expect("Queue ran out of space");
            }
        }
    }

    /// Dispatcher entry
    ///
    /// This function must be called at the top of a dispatcher, before the task
    /// executes.
    ///
    /// # Returns
    ///
    /// The previous timeline, which must be restored when the task completes.
    ///
    /// Each dispatcher should call these functions as follows:
    ///
    /// 1. dispatcher_entry()
    /// 2. Execute its task
    /// 3. dispatcher_exit()
    #[inline]
    fn dispatcher_entry(&self, rq_idx: u16) -> Timestamp {
        // The dispatcher runs at its own priority (lower than the timestamper prio).
        // Therefore we need to make sure the task DL taken from the RQ and the system
        // DL are in sync.
        critical_section::with(|_| {
            let prev_deadline = self.run_queue().get(rq_idx);
            let _abs_dl = self.system_deadline().load();

            #[cfg(feature = "defmt")]
            defmt::trace!(
                "[DISPATCHER ENTRY] sys dl: {}, task dl: {}",
                _abs_dl,
                prev_deadline
            );

            // Optionally assert that the deadline hasn't been missed
            #[cfg(all(feature = "defmt", feature = "check-missed-deadlines"))]
            {
                // TODO: cortex-m leaking here
                use cortex_m::peripheral::scb::VectActive;

                let now = Self::now();

                let vect_active = cortex_m::peripheral::SCB::vect_active();
                let irqn = match vect_active {
                    VectActive::Interrupt { irqn } => Some(irqn),
                    _ => None,
                };

                defmt::assert!(
                    now <= _abs_dl,
                    "Missed deadline. \n\tnow: {}\n\tDeadline: {}\n\tdiff: {}\n\tRun queue idx: {}\n\tISR: {}",
                    now,
                    _abs_dl,
                    now - _abs_dl,
                    rq_idx,
                    irqn,
                );
            }

            #[cfg(all(not(feature = "defmt"), feature = "check-missed-deadlines"))]
            assert!(Self::now() <= _abs_dl, "Missed deadline");

            prev_deadline
        })
    }

    /// Dispatcher exit
    ///
    /// This function must be called immediately after a task has completed in a
    /// dispatcher.
    ///
    /// 1. Restores the previous deadline
    /// 2. Looks in the global queue, and pends the shortest deadline task if it
    ///    has a shorter deadline than the currently running task, or if its
    ///    dispatcher is ready to accept a new task.
    ///
    /// The `prev_deadline` argument must be the timestamp returned by the
    /// `dispatcher_entry` call that happened immediately before the task
    /// was run.
    ///
    /// Unfortunately has to be generic over [`EdfTaskBinding`] because of the
    /// interrupt unmasking associated function, which means it will get
    /// monomorphized.
    #[inline]
    fn dispatcher_exit<T: EdfTaskBinding>(&self, prev_deadline: Timestamp) {
        let wq = self.wait_queue();

        #[cfg(feature = "defmt")]
        defmt::trace!(
            "[COMPLETE TASK] new dl: {}, dispatcher idx: {}, run queue idx: {}",
            prev_deadline,
            T::DISPATCHER_IDX,
            T::RUN_QUEUE_IDX,
        );

        // Restore previous deadline
        self.system_deadline().store(prev_deadline);

        // The timestamper -> scheduler jump means that we will have exited the
        // timestamper interrupt while the interrupt source is still pending (because
        // the task itself -ie, the user code- must act upon it to clear the interrupt
        // flag - think, for example, of a UART that must read its data register to
        // clear the flag). Therefore the timestamper interrupt will have been
        // erroneously re-pended as soon as it is exited, which would lead to the task
        // being scheduled+executed twice if we didn't manually unpend it.
        T::unpend_timestamper_interrupt();

        // It's possible that a task showed up in the queue as the previous (just
        // completed) task was running. So we need to check if it would preempt
        // the next task in line to run, which would start as soon as the
        // critical section exits.
        //
        // This works because all tasks that share a dispatcher run queue slot have the
        // same deadline, therefore they will never try to preempt each other. Therefore
        // if the run queue's slot is already full for this priority level, it
        // is guaranteed to have a shorter deadline than any dequeued task on
        // the same prio level.
        let next_task = wq.pop();

        critical_section::with(|cs| {
            if let Some(task) = next_task {
                let sys_dl = self.system_deadline().load();

                if task.abs_deadline() < sys_dl {
                    #[cfg(feature = "defmt")]
                    defmt::trace!(
                        "[DEQUEUE TASK] now: {}, sys dl: {}, task dispatcher: {}, task run queue idx: {}, task dl: {}",
                        Self::now(),
                        sys_dl,
                        task.dispatcher_index(),
                        task.rq_index(),
                        task.abs_deadline(),
                    );

                    execute(self, &cs, task);
                } else {
                    // Task isn't ready to run. Put it back into queue.
                    wq.insert(task).expect("Queue ran out of space");
                }
            }

            // CAUTION: This must be the last thing that is called in the function, just
            // before exiting the critical section.
            unsafe {
                T::unmask_timestamper_interrupt();
            }
        });
    }
}

/// Execute a task
///
/// This function performs the follwing:
///
/// 1. (unconditionnally) sets the system deadline to the task slated for
///    execution's deadline
/// 2. Add the task its dispatcher's queue
/// 3. Pend the dispatcher interrupt, which will run as soon as there are no
///    higher priority interrupts running
///
/// **Note**: This function is excluded from the [`Scheduler`] trait in order to
/// avoid it being callable from within an RTIC app.
#[inline]
fn execute<S, const D_LEN: usize, const Q_LEN: usize>(
    scheduler: &S,
    _cs: &critical_section::CriticalSection<'_>,
    task: ScheduledTask,
) where
    S: Scheduler<D_LEN, Q_LEN>,
{
    let rq_idx = task.rq_index();
    let dispatcher_idx = task.dispatcher_index();

    let prev_dl = scheduler.system_deadline().swap(task.abs_deadline());

    #[cfg(feature = "defmt")]
    defmt::trace!(
        "[EXEC] new dl: {}, prev dl: {}",
        scheduler.system_deadline().load(),
        prev_dl
    );

    scheduler.run_queue().insert(prev_dl, rq_idx);
    S::pend_dispatcher(dispatcher_idx);
}
