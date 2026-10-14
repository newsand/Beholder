use egui::{InputState, Key};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlotCommand {
    Recenter,
    Refresh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandEffects {
    pub clear_buffer: bool,
    pub reset_plot_view: bool,
}

impl PlotCommand {
    pub fn from_key(key: Key) -> Option<Self> {
        match key {
            Key::F4 => Some(Self::Recenter),
            Key::F5 => Some(Self::Refresh),
            _ => None,
        }
    }

    pub fn detect(input: &InputState) -> Option<Self> {
        if input.key_pressed(Key::F4) {
            Some(Self::Recenter)
        } else if input.key_pressed(Key::F5) {
            Some(Self::Refresh)
        } else {
            None
        }
    }

    pub fn effects(self) -> CommandEffects {
        match self {
            Self::Recenter => CommandEffects {
                clear_buffer: false,
                reset_plot_view: true,
            },
            Self::Refresh => CommandEffects {
                clear_buffer: true,
                reset_plot_view: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_key_maps_f4_and_f5() {
        assert_eq!(PlotCommand::from_key(Key::F4), Some(PlotCommand::Recenter));
        assert_eq!(PlotCommand::from_key(Key::F5), Some(PlotCommand::Refresh));
        assert_eq!(PlotCommand::from_key(Key::A), None);
    }

    #[test]
    fn recenter_only_resets_view() {
        assert_eq!(
            PlotCommand::Recenter.effects(),
            CommandEffects {
                clear_buffer: false,
                reset_plot_view: true,
            }
        );
    }

    #[test]
    fn refresh_clears_and_resets_view() {
        assert_eq!(
            PlotCommand::Refresh.effects(),
            CommandEffects {
                clear_buffer: true,
                reset_plot_view: true,
            }
        );
    }
}
