use std::cmp::Ordering;
use std::sync::Arc;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::table::{
    Column, ColumnSort, DataTable, TableDelegate, TableEvent, TableState,
};
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, h_flex};
use gpui_kit::{
    App, AppContext as _, Context, Entity, Focusable as _, FontWeight, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _,
};
use top_metrics::{Process, Snapshot};

use crate::monitor::{Monitor, ProcessDemand};
use crate::widgets::{card, hidden, mono};
use crate::{QuitProcess, format};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Pid,
    User,
    Cpu,
    Gpu,
    Memory,
    Resident,
    Time,
    Command,
    /// The trailing lane holding the selected row's quit button.
    Action,
}

const FIELDS: [Field; 9] = [
    Field::Pid,
    Field::User,
    Field::Cpu,
    Field::Gpu,
    Field::Memory,
    Field::Resident,
    Field::Time,
    Field::Command,
    Field::Action,
];

impl Field {
    fn column(self) -> Column {
        let column = match self {
            Self::Pid => Column::new("pid", "PID").width(72.).text_right(),
            Self::User => Column::new("user", "User").width(104.),
            Self::Cpu => Column::new("cpu", "CPU")
                .width(72.)
                .text_right()
                .descending(),
            Self::Gpu => Column::new("gpu", "GPU").width(72.).text_right(),
            Self::Memory => Column::new("mem", "MEM").width(72.).text_right(),
            Self::Resident => Column::new("res", "RES").width(72.).text_right(),
            Self::Time => Column::new("time", "Time").width(104.).text_right(),
            Self::Command => Column::new("command", "Command").width(512.),
            Self::Action => {
                return Column::new("action", "")
                    .width(112.)
                    .text_right()
                    .resizable(false);
            }
        };
        column.sortable().resizable(false)
    }

    /// Text columns sort A→Z first, numbers largest first.
    fn first_sort(self) -> ColumnSort {
        match self {
            Self::Pid | Self::User | Self::Command => ColumnSort::Ascending,
            _ => ColumnSort::Descending,
        }
    }

    /// Whether ordering or filtering by this field reads the lowercased commands.
    fn needs_lowercase(self) -> bool {
        self == Self::Command
    }
}

/// The process table's rows: indices into the latest shared snapshot,
/// filtered by the search query and sorted by the chosen column. Selection is
/// kept by pid, because rows move on every sample.
pub struct ProcessList {
    snapshot: Arc<Snapshot>,
    /// Lowercased commands, built only while sorting by command or searching.
    lowercase: Option<Vec<String>>,
    rows: Vec<usize>,
    query: String,
    sort_field: Field,
    sort: ColumnSort,
    selected_pid: Option<i32>,
    columns: Vec<Column>,
}

impl ProcessList {
    pub fn new() -> Self {
        Self {
            snapshot: Arc::default(),
            lowercase: None,
            rows: Vec::new(),
            query: String::new(),
            sort_field: Field::Cpu,
            sort: ColumnSort::Descending,
            selected_pid: None,
            columns: FIELDS.iter().map(|f| f.column()).collect(),
        }
    }

    pub fn set_snapshot(&mut self, snapshot: Arc<Snapshot>) {
        if !Arc::ptr_eq(&self.snapshot, &snapshot) {
            self.snapshot = snapshot;
            self.lowercase = None;
            self.refresh_rows();
        }
    }

    pub fn set_query(&mut self, query: &str) {
        self.query = query.trim().to_lowercase();
        self.refresh_rows();
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    pub fn total_count(&self) -> usize {
        self.snapshot.processes.len()
    }

    fn row(&self, row_ix: usize) -> Option<&Process> {
        self.rows
            .get(row_ix)
            .map(|&ix| &self.snapshot.processes[ix])
    }

    pub fn selected(&self) -> Option<&Process> {
        self.row(self.selected_row_ix()?)
    }

    /// Records the selection from a row index the table reported.
    pub fn select_row(&mut self, row_ix: usize) {
        self.selected_pid = self.row(row_ix).map(|p| p.pid);
    }

    pub fn clear_selected(&mut self) {
        self.selected_pid = None;
    }

    /// Where the selected process sits after the latest refresh, if still listed.
    pub fn selected_row_ix(&self) -> Option<usize> {
        let pid = self.selected_pid?;
        self.rows
            .iter()
            .position(|&ix| self.snapshot.processes[ix].pid == pid)
    }

    fn refresh_rows(&mut self) {
        let processes = &self.snapshot.processes;
        if self.lowercase.is_none() && (!self.query.is_empty() || self.sort_field.needs_lowercase())
        {
            self.lowercase = Some(processes.iter().map(|p| p.command.to_lowercase()).collect());
        }
        let lowercase = self.lowercase.as_deref();
        let query = &self.query;
        self.rows.clear();
        self.rows.extend((0..processes.len()).filter(|&ix| {
            let process = &processes[ix];
            query.is_empty()
                || lowercase.is_some_and(|commands| commands[ix].contains(query.as_str()))
                || process.user.to_lowercase().contains(query.as_str())
                || process.pid.to_string().contains(query.as_str())
        }));

        let (field, descending) = (self.sort_field, self.sort == ColumnSort::Descending);
        let compare = |a: &usize, b: &usize| {
            let (pa, pb) = (&processes[*a], &processes[*b]);
            let order = match field {
                Field::Pid => pa.pid.cmp(&pb.pid),
                Field::User => pa.user.cmp(&pb.user),
                Field::Cpu => pa.cpu.total_cmp(&pb.cpu),
                Field::Gpu => pa.gpu.total_cmp(&pb.gpu),
                Field::Memory => pa.memory.total_cmp(&pb.memory),
                Field::Resident => pa.resident.cmp(&pb.resident),
                Field::Time => pa.cpu_time.total_cmp(&pb.cpu_time),
                Field::Command => lowercase.map_or(Ordering::Equal, |c| c[*a].cmp(&c[*b])),
                Field::Action => Ordering::Equal,
            };
            if descending { order.reverse() } else { order }
        };
        self.rows.sort_by(compare);
    }
}

impl TableDelegate for ProcessList {
    fn columns_count(&self, _: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        self.columns[col_ix].clone()
    }

    fn perform_sort(
        &mut self,
        col_ix: usize,
        sort: ColumnSort,
        window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) {
        let field = FIELDS[col_ix];
        // The header cycles to Default after two clicks; restart from the field's natural order.
        self.sort = if sort == ColumnSort::Default {
            field.first_sort()
        } else {
            sort
        };
        self.sort_field = field;
        self.refresh_rows();
        // The table keeps its selected index; move it to where the selected pid landed.
        cx.defer_in(window, |table, _, cx| {
            if let Some(row_ix) = table.delegate().selected_row_ix()
                && table.selected_row() != Some(row_ix)
            {
                table.set_selected_row(row_ix, cx);
            }
        });
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let Some(process) = self.row(row_ix) else {
            return div().into_any_element();
        };
        let theme = cx.theme();
        // Task info of other users' processes needs root; their CPU and memory are unknown.
        let readable = process.resident > 0;
        let unknown = || {
            div()
                .text_color(theme.muted_foreground)
                .child("–")
                .into_any_element()
        };
        let percent = |value: f32| SharedString::from(format!("{value:.1}%"));
        match FIELDS[col_ix] {
            Field::Pid => div()
                .text_color(theme.muted_foreground)
                .child(process.pid.to_string())
                .into_any_element(),
            Field::User => div()
                .truncate()
                .child(process.user.clone())
                .into_any_element(),
            Field::Cpu if readable => div()
                .when(process.cpu >= 20.0, |this| this.text_color(theme.primary))
                .child(percent(process.cpu))
                .into_any_element(),
            Field::Gpu => div().child(percent(process.gpu)).into_any_element(),
            Field::Memory if readable => div().child(percent(process.memory)).into_any_element(),
            Field::Resident if readable => div()
                .text_color(theme.muted_foreground)
                .child(format::resident(process.resident))
                .into_any_element(),
            Field::Time if readable => div()
                .text_color(theme.muted_foreground)
                .child(format::cpu_time(process.cpu_time))
                .into_any_element(),
            Field::Cpu | Field::Memory | Field::Resident | Field::Time => unknown(),
            Field::Command => div()
                .truncate()
                .child(process.command.clone())
                .into_any_element(),
            Field::Action => h_flex()
                .w_full()
                .justify_end()
                .when(self.selected_pid == Some(process.pid), |this| {
                    this.child(
                        Button::new(("quit", process.pid as u64))
                            .xsmall()
                            .ghost()
                            .text_color(theme.danger)
                            .label("Quit…")
                            .tooltip("Quit process (⌫)")
                            .on_click(|_, window, cx| {
                                window.dispatch_action(Box::new(QuitProcess), cx)
                            }),
                    )
                })
                .into_any_element(),
        }
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let message = if self.query.is_empty() {
            "Reading processes…".to_owned()
        } else {
            format!("No processes match “{}”", self.query)
        };
        h_flex()
            .size_full()
            .justify_center()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(message)
    }
}

/// The process card: search field and table. Rendered as a cached view, so
/// scrolling or moving the selection repaints only this card.
pub struct ProcessPanel {
    search: Entity<InputState>,
    table: Entity<TableState<ProcessList>>,
    _processes: ProcessDemand,
    _subscriptions: Vec<Subscription>,
}

impl ProcessPanel {
    pub fn new(monitor: Entity<Monitor>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search name, user or PID"));
        let table = cx.new(|cx| {
            TableState::new(ProcessList::new(), window, cx)
                .col_movable(false)
                .col_resizable(false)
                .col_selectable(false)
        });
        table.read(cx).focus_handle(cx).focus(window, cx);

        let subscriptions = vec![
            cx.observe(&monitor, |this, monitor, cx| {
                let snapshot = monitor.read(cx).snapshot().clone();
                this.table
                    .update(cx, |table, _| table.delegate_mut().set_snapshot(snapshot));
                this.sync_selection(cx);
            }),
            cx.subscribe_in(&search, window, |this, state, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    let query = state.read(cx).value().to_string();
                    this.table
                        .update(cx, |table, _| table.delegate_mut().set_query(&query));
                    this.sync_selection(cx);
                }
            }),
            cx.subscribe(&table, |_, table, event, cx| {
                table.update(cx, |table, cx| {
                    match event {
                        TableEvent::SelectRow(row_ix) => table.delegate_mut().select_row(*row_ix),
                        TableEvent::ClearSelection => table.delegate_mut().clear_selected(),
                        _ => return,
                    }
                    cx.notify();
                });
            }),
        ];

        let (snapshot, processes) = {
            let monitor = monitor.read(cx);
            (monitor.snapshot().clone(), monitor.demand_processes())
        };
        table.update(cx, |table, _| table.delegate_mut().set_snapshot(snapshot));
        Self {
            search,
            table,
            _processes: processes,
            _subscriptions: subscriptions,
        }
    }

    #[cfg(test)]
    pub fn table(&self) -> &Entity<TableState<ProcessList>> {
        &self.table
    }

    pub fn selected(&self, cx: &App) -> Option<Process> {
        self.table.read(cx).delegate().selected().cloned()
    }

    pub fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search
            .update(cx, |search, cx| search.focus(window, cx));
    }

    pub fn focus_table(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = self.table.read(cx).focus_handle(cx);
        handle.focus(window, cx);
    }

    /// Rows move on every sample; keep the table's selected row on the selected pid.
    fn sync_selection(&mut self, cx: &mut Context<Self>) {
        self.table
            .update(cx, |table, cx| match table.delegate().selected_row_ix() {
                Some(row_ix) if table.selected_row() != Some(row_ix) => {
                    table.set_selected_row(row_ix, cx)
                }
                None if table.selected_row().is_some() => table.clear_selection(cx),
                _ => cx.notify(),
            });
        cx.notify();
    }
}

impl Render for ProcessPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let list = self.table.read(cx).delegate();
        card(cx)
            .size_full()
            .overflow_hidden()
            .child(
                h_flex()
                    .gap_3()
                    .px_3()
                    .py_2p5()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .font_family(theme.font_family.clone())
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Processes"),
                    )
                    .child(
                        mono(cx)
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!("{} of {}", list.row_count(), list.total_count())),
                    )
                    .child(div().flex_1())
                    .when(!hidden("search"), |this| {
                        this.child(
                            Input::new(&self.search)
                                .id("search")
                                .small()
                                .w_72()
                                .cleanable(true)
                                .prefix(
                                    Icon::new(IconName::Search)
                                        .small()
                                        .text_color(theme.muted_foreground),
                                ),
                        )
                    }),
            )
            .child(
                mono(cx).flex_1().min_h_0().text_xs().child(
                    DataTable::new(&self.table)
                        .bordered(false)
                        .stripe(false)
                        .small(),
                ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(processes: Vec<Process>) -> Arc<Snapshot> {
        let mut snapshot = Snapshot::default();
        snapshot.processes = processes;
        Arc::new(snapshot)
    }

    fn process(pid: i32, command: &str, cpu: f32) -> Process {
        let mut process = Process::default();
        process.pid = pid;
        process.command = command.into();
        process.user = "akira".into();
        process.cpu = cpu;
        process
    }

    #[test]
    fn keeps_selection_by_pid_when_rows_reorder() {
        let mut list = ProcessList::new();
        list.set_snapshot(snapshot(vec![process(1, "a", 10.0), process(2, "b", 5.0)]));
        list.select_row(1);
        assert_eq!(list.selected().map(|p| p.pid), Some(2));

        list.set_snapshot(snapshot(vec![process(1, "a", 1.0), process(2, "b", 50.0)]));
        assert_eq!(list.selected_row_ix(), Some(0));
    }

    #[test]
    fn filters_by_command_user_or_pid() {
        let mut list = ProcessList::new();
        list.set_snapshot(snapshot(vec![
            process(454, "WindowServer", 1.0),
            process(1416, "ghostty", 2.0),
        ]));
        list.set_query("window");
        assert_eq!(list.row_count(), 1);
        list.set_query("1416");
        assert_eq!(list.selected_row_ix(), None);
        assert_eq!(list.row(0).map(|p| p.command.as_str()), Some("ghostty"));
        list.set_query("AKIRA");
        assert_eq!(list.row_count(), 2);
    }

    #[test]
    fn sorts_commands_case_insensitively() {
        let mut list = ProcessList::new();
        list.set_snapshot(snapshot(vec![
            process(1, "zsh", 1.0),
            process(2, "Arc", 1.0),
            process(3, "bash", 1.0),
        ]));
        list.sort_field = Field::Command;
        list.sort = ColumnSort::Ascending;
        list.lowercase = None;
        list.refresh_rows();
        let commands: Vec<&str> = (0..3)
            .filter_map(|ix| list.row(ix).map(|p| p.command.as_str()))
            .collect();
        assert_eq!(commands, vec!["Arc", "bash", "zsh"]);
    }
}
