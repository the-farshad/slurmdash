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
        Self {
            fg: Color::Rgb(0x24, 0x29, 0x2f),
            bg: Color::Reset,
            border: Color::Rgb(0xd0, 0xd7, 0xde),
            accent: Color::Rgb(0x09, 0x69, 0xda),
            muted: Color::Rgb(0x57, 0x60, 0x6a),
            ..Self::dark()
        }
    }

    /// Maximum-contrast palette: bright primaries on the default
    /// terminal background. Useful in projected demos or accessibility
    /// settings where the dark theme's subtle muted greys disappear.
    pub fn high_contrast() -> Self {
        Self {
            fg: Color::White,
            bg: Color::Reset,
            border: Color::Gray,
            accent: Color::LightCyan,
            muted: Color::Gray,

            running: Color::LightGreen,
            pending: Color::LightYellow,
            completing: Color::LightCyan,
            completed: Color::LightBlue,
            failed: Color::LightRed,
            cancelled: Color::LightMagenta,
            preempted: Color::Yellow,
            held: Color::LightMagenta,
            suspended: Color::Gray,

            usage_low: Color::LightGreen,
            usage_med: Color::LightYellow,
            usage_high: Color::Yellow,
            usage_critical: Color::LightRed,

            action_safe: Color::LightCyan,
            action_normal: Color::LightGreen,
            action_warning: Color::LightYellow,
            action_danger: Color::LightRed,
        }
    }

    /// Wong palette — deuteranopia / protanopia safe. Red and green are
    /// avoided as primary differentiators; we lean on blue/orange/yellow
    /// plus reddish-purple. Failure state stays vermillion which reads
    /// distinct from running's bluish-green in every common form of CVD.
    pub fn colorblind_safe() -> Self {
        // Wong (Nature, 2011): vermillion, blue, bluish-green, yellow,
        // sky-blue, orange, reddish-purple, black.
        let vermillion = Color::Rgb(0xd5, 0x5e, 0x00);
        let blue = Color::Rgb(0x00, 0x72, 0xb2);
        let bluish_green = Color::Rgb(0x00, 0x9e, 0x73);
        let yellow = Color::Rgb(0xf0, 0xe4, 0x42);
        let sky_blue = Color::Rgb(0x56, 0xb4, 0xe9);
        let orange = Color::Rgb(0xe6, 0x9f, 0x00);
        let purple = Color::Rgb(0xcc, 0x79, 0xa7);

        Self {
            fg: Color::Rgb(0xea, 0xea, 0xea),
            bg: Color::Reset,
            border: Color::Rgb(0x4a, 0x4a, 0x4a),
            accent: sky_blue,
            muted: Color::Rgb(0x9a, 0x9a, 0x9a),

            running: bluish_green,
            pending: orange,
            completing: sky_blue,
            completed: blue,
            failed: vermillion,
            cancelled: purple,
            preempted: yellow,
            held: purple,
            suspended: Color::Rgb(0x6a, 0x6a, 0x6a),

            usage_low: bluish_green,
            usage_med: yellow,
            usage_high: orange,
            usage_critical: vermillion,

            action_safe: sky_blue,
            action_normal: bluish_green,
            action_warning: orange,
            action_danger: vermillion,
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name {
            "light" => Self::light(),
            "high-contrast" | "high_contrast" | "hc" => Self::high_contrast(),
            "colorblind-safe" | "colorblind" | "cb" => Self::colorblind_safe(),
            _ => Self::dark(),
        }
    }

    /// Ordered list of built-in themes. Used by the `T` keybind to cycle.
    pub const NAMES: &'static [&'static str] =
        &["dark", "light", "high-contrast", "colorblind-safe"];

    /// Return the next theme name in [`Self::NAMES`] after `current`,
    /// wrapping around.
    pub fn next_name(current: &str) -> &'static str {
        let mut hit = false;
        for n in Self::NAMES {
            if hit {
                return n;
            }
            if *n == current {
                hit = true;
            }
        }
        Self::NAMES[0]
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
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    pub fn footer_style(&self) -> Style {
        Style::default().fg(self.muted)
    }

    pub fn border_style(&self) -> Style {
        Style::default().fg(self.border)
    }
}
