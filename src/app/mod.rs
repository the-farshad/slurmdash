use crate::actions::ActionKind;
use crate::slurm::model::{Job, JobDetails};

#[derive(Debug, Default)]
pub struct AppState {
    pub jobs: Vec<Job>,
    pub selected: usize,
    pub view: View,
    pub details: Option<JobDetails>,
    pub confirm: Option<Confirm>,
    pub show_help: bool,
    pub last_error: Option<String>,
    pub should_quit: bool,
}

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub enum View {
    #[default]
    Jobs,
    Details,
}

#[derive(Debug, Clone)]
pub struct Confirm {
    pub kind: ActionKind,
    pub job_id: String,
    pub preview: String,
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
