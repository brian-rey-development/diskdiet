use super::entries::display_label;
use super::layout::{fit, progress_message_budget, terminal_width};
use crate::actions;
use crate::fmt::count_noun;
use crate::model::{Action, Item, Outcome, RunMode};
use crate::scan::caches::{OwnerCheck, owner_check};
use crate::tools::Tools;
use std::sync::atomic::{AtomicBool, Ordering};

const RANK_EMPTY_TRASH: u8 = 0;
const RANK_NORMAL: u8 = 1;
const RANK_VM_TRIM: u8 = 2;
const PROGRESS_TEMPLATE: &str = "{bar:20.magenta} {pos}/{len} [{elapsed_precise}] {msg}";
const INTERRUPT_EXIT_CODE: i32 = 130;
const OWNER_RUNNING: &str = "its app is running, quit it and run diskdiet again";
const OWNER_UNKNOWN: &str =
    "could not check whether its app is running (ps is unavailable), run diskdiet again";

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

pub(crate) struct Executed<'a> {
    pub item: &'a Item,
    pub outcome: Outcome,
}

fn execution_rank(item: &Item) -> u8 {
    match &item.action {
        Action::EmptyTrash => RANK_EMPTY_TRASH,
        action if action.is_vm_trim() => RANK_VM_TRIM,
        _ => RANK_NORMAL,
    }
}

pub fn execution_order<'a>(items: &[&'a Item]) -> Vec<&'a Item> {
    let mut ordered = items.to_vec();
    ordered.sort_by_key(|item| execution_rank(item));
    ordered
}

fn on_interrupt() {
    if INTERRUPTED.swap(true, Ordering::SeqCst) {
        let _ = console::Term::stderr().show_cursor();
        std::process::exit(INTERRUPT_EXIT_CODE);
    }
}

fn run_one(item: &Item, tools: &Tools, mode: RunMode) -> Outcome {
    if INTERRUPTED.load(Ordering::SeqCst) {
        return Outcome::Stopped;
    }
    if mode == RunMode::Live {
        match owner_check(item) {
            OwnerCheck::Running => return Outcome::Skipped(OWNER_RUNNING.to_owned()),
            OwnerCheck::Unknown => return Outcome::Skipped(OWNER_UNKNOWN.to_owned()),
            OwnerCheck::Idle => {}
        }
    }
    actions::execute(item, tools, mode)
}

pub fn processed_summary(item_count: usize) -> String {
    format!("Processed {}", count_noun(item_count, "item", "items"))
}

fn progress_label(mode: RunMode) -> &'static str {
    match mode {
        RunMode::DryRun => "Pretending to trim...",
        RunMode::Live => "Trimming...",
    }
}

pub(crate) fn execute_all<'a>(
    items: &[&'a Item],
    tools: &Tools,
    mode: RunMode,
) -> Vec<Executed<'a>> {
    let _ = ctrlc::set_handler(on_interrupt);
    let bar = cliclack::progress_bar(items.len() as u64).with_template(PROGRESS_TEMPLATE);
    let budget = progress_message_budget(terminal_width());
    bar.start(progress_label(mode));
    let mut executed = Vec::with_capacity(items.len());
    for item in execution_order(items) {
        bar.set_message(fit(&display_label(item), budget));
        let outcome = run_one(item, tools, mode);
        executed.push(Executed { item, outcome });
        bar.inc(1);
    }
    bar.stop(processed_summary(executed.len()));
    executed
}
