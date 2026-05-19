//! Application state and event loop.
//!
//! Phase 1: minimal — owns the job list, selected row, view enum, and
//! refresh policy. Built out alongside the TUI widgets in Phase 1.6 / 1.7.

use crate::slurm::model::Job;

#[derive(Debug, Default)]
pub struct AppState {
    pub jobs: Vec<Job>,
    pub selected: usize,
    pub view: View,
    pub last_error: Option<String>,
    pub should_quit: bool,
}

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub enum View {
    #[default]
    Jobs,
    Details,
    Help,
}

impl AppState {
    pub fn select_next(&mut self) {
        if self.jobs.is_empty() {
            self.selected = 0;
            return;
        }
        self.selected = (self.selected + 1).min(self.jobs.len() - 1);
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn selected_job(&self) -> Option<&Job> {
        self.jobs.get(self.selected)
    }
}
