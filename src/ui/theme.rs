use super::layout::{fit, list_row_budget, terminal_width};
use cliclack::{Theme, ThemeState};

const BAR: &str = "│";
const HINT_INDENT: &str = "    ";

struct DietTheme;

fn is_final(state: &ThemeState) -> bool {
    matches!(state, ThemeState::Submit | ThemeState::Cancel)
}

impl Theme for DietTheme {
    fn checkbox_item(
        &self,
        state: &ThemeState,
        selected: bool,
        active: bool,
        label: &str,
        hint: &str,
    ) -> String {
        if is_final(state) && !selected {
            return String::new();
        }
        let budget = list_row_budget(terminal_width());
        let checkbox = self.checkbox_symbol(state, selected, active);
        let styled = self
            .checkbox_style(state, selected, active)
            .apply_to(fit(label, budget));
        let head = format!("{checkbox} {styled}");
        if !active || hint.is_empty() || is_final(state) {
            return head;
        }
        let bar = self.bar_color(state).apply_to(BAR);
        let detail = self.placeholder_style(state).apply_to(fit(hint, budget));
        format!("{head}\n{bar}{HINT_INDENT}{detail}")
    }
}

pub(super) fn install() {
    cliclack::set_theme(DietTheme);
}
