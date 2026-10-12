use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::future::poll_fn;
use std::pin::pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Poll, Waker};
use std::time::Duration;

use gpui_kit::{Context, SharedString, Task};
use top_metrics::{ChipInfo, Sampler, Signal, SignalError, Snapshot};

/// Samples kept for the sparklines and power bars.
pub const HISTORY_LEN: usize = 48;

/// Processes ranked by CPU for the menu bar popover.
pub const BUSIEST_LEN: usize = 5;

/// How long a process gets to exit after SIGTERM before SIGKILL.
const QUIT_GRACE: Duration = Duration::from_secs(3);

/// How long process rates accumulate after listing resumes, before the
/// sample that shows them.
const PROCESS_BASELINE: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interval {
    HalfSecond,
    Second,
    TwoSeconds,
}

impl Interval {
    pub const ALL: [Self; 3] = [Self::HalfSecond, Self::Second, Self::TwoSeconds];

    pub fn duration(self) -> Duration {
        Duration::from_millis(self.millis())
    }

    pub fn millis(self) -> u64 {
        match self {
            Self::HalfSecond => 500,
            Self::Second => 1000,
            Self::TwoSeconds => 2000,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::HalfSecond => "0.5s",
            Self::Second => "1s",
            Self::TwoSeconds => "2s",
        }
    }
}

/// Recent values, oldest first, each 0–100 except `power` in watts.
#[derive(Default)]
pub struct History {
    pub cpu: VecDeque<f32>,
    pub gpu: VecDeque<f32>,
    pub ane: VecDeque<f32>,
    pub memory: VecDeque<f32>,
    pub power: VecDeque<f32>,
}

impl History {
    fn push(&mut self, snapshot: &Snapshot) {
        let memory = if snapshot.memory.total > 0 {
            snapshot.memory.used as f32 / snapshot.memory.total as f32 * 100.0
        } else {
            0.0
        };
        for (series, value) in [
            (&mut self.cpu, snapshot.cpu.usage),
            (&mut self.gpu, snapshot.gpu.usage),
            (&mut self.ane, snapshot.ane.unwrap_or(0.0)),
            (&mut self.memory, memory),
            (&mut self.power, snapshot.power.system),
        ] {
            if series.len() == HISTORY_LEN {
                series.pop_front();
            }
            series.push_back(value);
        }
    }
}

/// The live measurements every surface shows: one sampler, one history, one
/// refresh policy. Views observe it and never sample on their own.
pub struct Monitor {
    chip: Option<ChipInfo>,
    /// Shared with views by reference count, so a sample is never copied.
    snapshot: Arc<Snapshot>,
    /// Indices into `snapshot.processes`, busiest first.
    busiest: Vec<usize>,
    history: History,
    power_peak: f32,
    interval: Interval,
    frozen: bool,
    notice: Option<SharedString>,
    process_views: Rc<ProcessViews>,
    _sampling: Task<()>,
}

impl Monitor {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let process_views = Rc::<ProcessViews>::default();
        Self {
            chip: None,
            snapshot: Arc::default(),
            busiest: Vec::new(),
            history: History::default(),
            power_peak: 0.0,
            interval: Interval::Second,
            frozen: false,
            notice: None,
            _sampling: Self::start_sampling(process_views.clone(), cx),
            process_views,
        }
    }

    /// A monitor showing a fixed sample, without a sampler, for tests.
    #[cfg(test)]
    pub fn fixture(chip: ChipInfo, snapshot: Snapshot, cx: &mut Context<Self>) -> Self {
        let mut monitor = Self {
            chip: Some(chip),
            snapshot: Arc::default(),
            busiest: Vec::new(),
            history: History::default(),
            power_peak: 0.0,
            interval: Interval::Second,
            frozen: false,
            notice: None,
            process_views: Rc::default(),
            _sampling: Task::ready(()),
        };
        monitor.apply(Sample::new(snapshot), cx);
        monitor
    }

    /// Samples on a background thread for as long as the monitor lives. While
    /// frozen it keeps sampling, so rates stay fresh on resume, but discards
    /// the results. Processes are listed only while a view shows them.
    fn start_sampling(process_views: Rc<ProcessViews>, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            let executor = cx.background_executor().clone();
            let mut sampler = executor.spawn(async { Sampler::new() }).await;
            let chip = sampler.chip().clone();
            if this
                .update(cx, |monitor, cx| monitor.set_chip(chip, cx))
                .is_err()
            {
                return;
            }
            loop {
                let Ok(interval) = this.read_with(cx, |monitor, _| monitor.interval) else {
                    return;
                };
                process_views
                    .wait(executor.timer(interval.duration()))
                    .await;
                let lists = process_views.count.get() > 0;
                if lists != sampler.lists_processes() {
                    sampler = executor
                        .spawn(async move {
                            sampler.set_lists_processes(lists);
                            sampler
                        })
                        .await;
                    if lists {
                        executor.timer(PROCESS_BASELINE).await;
                    }
                }
                let (returned, sample) = executor
                    .spawn(async move {
                        let sample = Sample::new(sampler.sample());
                        (sampler, sample)
                    })
                    .await;
                sampler = returned;
                if this
                    .update(cx, |monitor, cx| monitor.apply(sample, cx))
                    .is_err()
                {
                    return;
                }
            }
        })
    }

    fn set_chip(&mut self, chip: ChipInfo, cx: &mut Context<Self>) {
        self.chip = Some(chip);
        cx.notify();
    }

    fn apply(&mut self, sample: Sample, cx: &mut Context<Self>) {
        if self.frozen {
            return;
        }
        self.history.push(&sample.snapshot);
        self.power_peak = self.power_peak.max(sample.snapshot.power.system);
        self.snapshot = sample.snapshot;
        self.busiest = sample.busiest;
        cx.notify();
    }

    pub fn chip(&self) -> Option<&ChipInfo> {
        self.chip.as_ref()
    }

    pub fn snapshot(&self) -> &Arc<Snapshot> {
        &self.snapshot
    }

    /// The busiest processes by CPU, most first.
    pub fn busiest(&self) -> impl Iterator<Item = &top_metrics::Process> {
        self.busiest.iter().map(|&ix| &self.snapshot.processes[ix])
    }

    pub fn history(&self) -> &History {
        &self.history
    }

    /// Whether at least one sample has arrived.
    pub fn is_live(&self) -> bool {
        !self.history.cpu.is_empty()
    }

    pub fn power_peak(&self) -> f32 {
        self.power_peak
    }

    pub fn power_average(&self) -> f32 {
        let power = &self.history.power;
        if power.is_empty() {
            0.0
        } else {
            power.iter().sum::<f32>() / power.len() as f32
        }
    }

    /// Keeps processes listed while the returned guard lives; views that
    /// show processes hold one. The first one wakes the sampler at once.
    pub fn demand_processes(&self) -> ProcessDemand {
        let views = &self.process_views;
        views.count.set(views.count.get() + 1);
        if views.count.get() == 1 {
            views.opened.set(true);
            if let Some(waker) = views.waker.take() {
                waker.wake();
            }
        }
        ProcessDemand(views.clone())
    }

    #[cfg(test)]
    pub fn process_views(&self) -> usize {
        self.process_views.count.get()
    }

    pub fn interval(&self) -> Interval {
        self.interval
    }

    pub fn set_interval(&mut self, interval: Interval, cx: &mut Context<Self>) {
        if self.interval != interval {
            self.interval = interval;
            cx.notify();
        }
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    pub fn toggle_freeze(&mut self, cx: &mut Context<Self>) {
        self.frozen = !self.frozen;
        self.notice = None;
        cx.notify();
    }

    /// The result of the last process action, until the next one or a freeze toggle.
    pub fn notice(&self) -> Option<&SharedString> {
        self.notice.as_ref()
    }

    /// Sends SIGTERM, then SIGKILL if the same process (pid and start time) is
    /// still running after a grace period.
    pub fn quit_process(
        &mut self,
        pid: i32,
        started_at: u64,
        command: SharedString,
        cx: &mut Context<Self>,
    ) {
        match top_metrics::send_signal(pid, started_at, Signal::Terminate) {
            Ok(()) => {
                self.notice = Some(format!("Quit {command} ({pid})").into());
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(QUIT_GRACE).await;
                    if top_metrics::send_signal(pid, started_at, Signal::Kill).is_ok() {
                        this.update(cx, |monitor, cx| {
                            monitor.notice = Some(format!("Force quit {command} ({pid})").into());
                            cx.notify();
                        })
                        .ok();
                    }
                })
                .detach();
            }
            Err(error) => {
                let reason = match error {
                    SignalError::NotPermitted => "it belongs to another user or the system",
                    SignalError::NoSuchProcess => "it has already exited",
                    SignalError::Other(_) => "the system refused the signal",
                };
                self.notice = Some(format!("Couldn’t quit {command} ({pid}): {reason}.").into());
            }
        }
        cx.notify();
    }
}

/// The views showing processes, shared by the monitor and its sampling loop.
#[derive(Default)]
struct ProcessViews {
    count: Cell<usize>,
    /// Set when the first view opens; the loop's next wait returns at once.
    opened: Cell<bool>,
    waker: RefCell<Option<Waker>>,
}

impl ProcessViews {
    /// Waits for `timer`, or less if a first view opens meanwhile.
    async fn wait(&self, timer: Task<()>) {
        let mut timer = pin!(timer);
        poll_fn(|cx| {
            if self.opened.replace(false) || timer.as_mut().poll(cx).is_ready() {
                Poll::Ready(())
            } else {
                *self.waker.borrow_mut() = Some(cx.waker().clone());
                Poll::Pending
            }
        })
        .await
    }
}

/// Keeps processes listed while alive; see [`Monitor::demand_processes`].
pub struct ProcessDemand(Rc<ProcessViews>);

impl Drop for ProcessDemand {
    fn drop(&mut self) {
        self.0.count.set(self.0.count.get() - 1);
    }
}

/// A snapshot with what every view derives from it computed once, off the UI thread.
struct Sample {
    snapshot: Arc<Snapshot>,
    busiest: Vec<usize>,
}

impl Sample {
    fn new(snapshot: Snapshot) -> Self {
        let mut busiest: Vec<usize> = (0..snapshot.processes.len()).collect();
        let by_cpu = |a: &usize, b: &usize| {
            snapshot.processes[*b]
                .cpu
                .total_cmp(&snapshot.processes[*a].cpu)
        };
        if busiest.len() > BUSIEST_LEN {
            busiest.select_nth_unstable_by(BUSIEST_LEN, by_cpu);
            busiest.truncate(BUSIEST_LEN);
        }
        busiest.sort_by(by_cpu);
        Self {
            snapshot: Arc::new(snapshot),
            busiest,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::time::Duration;

    use gpui_kit::{AppContext as _, TestAppContext};
    use top_metrics::{ChipInfo, Process, Snapshot};

    use super::{BUSIEST_LEN, Monitor, Sample};

    #[gpui_kit::test]
    fn the_first_process_view_ends_the_wait(cx: &mut TestAppContext) {
        let monitor = cx.new(|cx| Monitor::fixture(ChipInfo::default(), Snapshot::default(), cx));
        let views = cx.read(|cx| monitor.read(cx).process_views.clone());
        let waited = Rc::new(Cell::new(false));
        let timer = cx.executor().timer(Duration::from_secs(60));
        cx.foreground_executor()
            .spawn({
                let waited = waited.clone();
                async move {
                    views.wait(timer).await;
                    waited.set(true);
                }
            })
            .detach();
        cx.run_until_parked();
        assert!(!waited.get(), "waits for the timer");

        let first = cx.read(|cx| monitor.read(cx).demand_processes());
        cx.run_until_parked();
        assert!(waited.get(), "the first process view wakes the sampler");
        assert_eq!(cx.read(|cx| monitor.read(cx).process_views()), 1);
        drop(first);
        assert_eq!(cx.read(|cx| monitor.read(cx).process_views()), 0);
    }

    #[test]
    fn ranks_the_busiest_processes() {
        let mut snapshot = Snapshot::default();
        snapshot.processes = (0..20)
            .map(|ix| {
                let mut process = Process::default();
                process.pid = ix;
                process.cpu = (ix * 7 % 20) as f32;
                process
            })
            .collect();
        let sample = Sample::new(snapshot);
        let cpu: Vec<f32> = sample
            .busiest
            .iter()
            .map(|&ix| sample.snapshot.processes[ix].cpu)
            .collect();
        assert_eq!(cpu, vec![19.0, 18.0, 17.0, 16.0, 15.0]);
        assert_eq!(cpu.len(), BUSIEST_LEN);
    }
}
