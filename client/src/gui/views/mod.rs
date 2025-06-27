use crate::profiles::Profile;

pub mod default;
#[cfg(windows)]
pub mod update;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum View {
    Default,
    #[cfg(windows)]
    Update,
}

impl Default for View {
    fn default() -> Self {
        Self::Default
    }
}

/// An action requested by the current view
#[derive(Debug, Clone)]
pub enum Action {
    UpdateProfile(Profile),
    #[cfg(windows)] // for now
    SwitchView(View),
    #[cfg(windows)]
    LauncherUpdate(self_update::update::Release),
}
