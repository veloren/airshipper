use crate::profiles::Profile;

pub mod default;
#[cfg(windows)]
pub mod update;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub enum View {
    #[default]
    Default,
    #[cfg(windows)]
    Update,
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
