//! Color and style palette for the TUI.
//!
//! Phase 1.9: one default `dark` theme plus a `light` variant. Job-state
//! colors and resource-usage gradients live here so widgets stay free of
//! literal color values. More themes (`high-contrast`, `colorblind-safe`)
//! are listed in the CLI but fall back to `dark` until they ship.

use ratatui::style::{Color, Modifier, Style};

use crate::slurm::state::JobState;

#[derive(Debug, Clone)]
pub struct Theme {
    pub fg: Color,
    pub bg: Color,
    pub border: Color,
    pub accent: Color,
    pub muted: Color,

    pub running: Color,
    pub pending: Color,
    pub completing: Color,
    pub completed: Color,
    pub failed: Color,
    pub cancelled: Color,
    pub preempted: Color,
    pub held: Color,
    pub suspended: Color,

    pub usage_low: Color,
    pub usage_med: Color,
    pub usage_high: Color,
    pub usage_critical: Color,

    pub action_safe: Color,
    pub action_normal: Color,
    pub action_warning: Color,
    pub action_danger: Color,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            fg: Color::Rgb(0xc9, 0xd1, 0xd9),
            bg: Color::Reset,
            border: Color::Rgb(0x30, 0x36, 0x3d),
            accent: Color::Rgb(0x58, 0xa6, 0xff),
            muted: Color::Rgb(0x8b, 0x94, 0x9e),

            running: Color::Rgb(0x3f, 0xb9, 0x50),
            pending: Color::Rgb(0xd2, 0x99, 0x22),
            completing: Color::Cyan,
            completed: Color::Rgb(0x58, 0xa6, 0xff),
            failed: Color::Rgb(0xf8, 0x51, 0x49),
            cancelled: Color::Rgb(0xbc, 0x8c, 0xff),
            preempted: Color::Rgb(0xff, 0x8c, 0x00),
            held: Color::Rgb(0xa3, 0x71, 0xf7),
            suspended: Color::DarkGray,

            usage_low: Color::Rgb(0x3f, 0xb9, 0x50),
            usage_med: Color::Rgb(0xd2, 0x99, 0x22),
            usage_high: Color::Rgb(0xff, 0x8c, 0x00),
            usage_critical: Color::Rgb(0xf8, 0x51, 0x49),

            action_safe: Color::Rgb(0x58, 0xa6, 0xff),
            action_normal: Color::Rgb(0x3f, 0xb9, 0x50),
            action_warning: Color::Rgb(0xd2, 0x99, 0x22),
            action_danger: Color::Rgb(0xf8, 0x51, 0x49),
        }
    }

    pub fn light() -> Self {
        // Conservative inversion — refine in a later pass.
        Self {
            fg: Color::Rgb(0x24, 0x29, 0x2f),
            bg: Color::Reset,
            border: Color::Rgb(0xd0, 0xd7, 0xde),
            accent: Color::Rgb(0x09, 0x69, 0xda),
            muted: Color::Rgb(0x57, 0x60, 0x6a),
            ..Self::dark()
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name {
            "light" => Self::light(),
            _ => Self::dark(),
        }
    }

    pub fn job_state_style(&self, state: &JobState) -> Style {
        let color = match state {
            JobState::Running => self.running,
            JobState::Pending => self.pending,
            JobState::Completing => self.completing,
            JobState::Completed => self.completed,
            JobState::Cancelled => self.cancelled,
            JobState::Failed
            | JobState::Timeout
            | JobState::NodeFail
            | JobState::BootFail
            | JobState::Deadline
            | JobState::OutOfMemory => self.failed,
            JobState::Preempted => self.preempted,
            JobState::Held => self.held,
            JobState::Suspended => self.suspended,
            JobState::Other(_) => self.fg,
        };
        Style::default().fg(color).add_modifier(Modifier::BOLD)
    }

    pub fn header_style(&self) -> Style {
        Style::default().fg(self.accent).add_modifier(Modifier::BOLD)
    }

    pub fn footer_style(&self) -> Style {
        Style::default().fg(self.muted)
    }

    pub fn border_style(&self) -> Style {
        Style::default().fg(self.border)
    }
}
