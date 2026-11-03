use crate::actions;
use crate::model::{Action, Item, Outcome};
use crate::scan::caches::owner_is_running_now;
use crate::tools::Tools;
use std::sync::atomic::{AtomicBool, Ordering};

const FSTRIM: &str = "fstrim";
const RANK_EMPTY_TRASH: u8 = 0;
const RANK_NORMAL: u8 = 1;
const RANK_VM_TRIM: u8 = 2;
const INTERRUPT_EXIT_CODE: i32 = 130;
const OWNER_RUNNING: &str = "its app is running, quit it and run diskdiet again";
pub const STOPPED_REASON: &str = "stopped with Ctrl-C before it started";

static STOP: AtomicBool = AtomicBool::new(false);

pub struct Executed<'a> {
    pub item: &'a Item,
    pub outcome: Outcome,
}

fn rank(item: &Item) -> u8 {
    match &item.action {
        Action::EmptyTrash => RANK_EMPTY_TRASH,
        Action::RunCommand { args, .. } if args.iter().any(|arg| arg == FSTRIM) => RANK_VM_TRIM,
        _ => RANK_NORMAL,
    }
}

pub fn execution_order<'a>(items: &[&'a Item]) -> Vec<&'a Item> {
    let mut ordered = items.to_vec();
    ordered.sort_by_key(|item| rank(item));
    ordered
}

fn on_interrupt() {
    if STOP.swap(true, Ordering::SeqCst) {
        let _ = console::Term::stderr().show_cursor();
        std::process::exit(INTERRUPT_EXIT_CODE);
    }
}

fn run_one(item: &Item, tools: &Tools, dry_run: bool) -> Outcome {
    if STOP.load(Ordering::SeqCst) {
        return Outcome::Skipped(STOPPED_REASON.to_string());
    }
    if !dry_run && owner_is_running_now(item) {
        return Outcome::Skipped(OWNER_RUNNING.to_string());
    }
    actions::execute(item, tools, dry_run)
}

pub fn execute_all<'a>(items: &[&'a Item], tools: &Tools, dry_run: bool) -> Vec<Executed<'a>> {
    let _ = ctrlc::set_handler(on_interrupt);
    let bar = cliclack::progress_bar(items.len() as u64);
    bar.start(if dry_run {
        "Pretending to trim..."
    } else {
        "Trimming..."
    });
    let mut executed = Vec::with_capacity(items.len());
    for item in execution_order(items) {
        bar.set_message(&item.label);
        let outcome = run_one(item, tools, dry_run);
        executed.push(Executed { item, outcome });
        bar.inc(1);
    }
    bar.stop(format!("Processed {} items", executed.len()));
    executed
}
