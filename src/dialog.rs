//! Native OS dialog helpers (I/O boundary — no state mutation).

/// Show a native confirmation dialog asking the user whether to quit.
/// Returns `true` if the user confirmed, `false` if cancelled.
#[cfg(target_os = "macos")]
pub fn confirm_quit() -> bool {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSAlertStyle};
    use objc2_foundation::NSString;

    let mtm = unsafe { MainThreadMarker::new_unchecked() };

    let alert = NSAlert::new(mtm);
    alert.setAlertStyle(NSAlertStyle::Informational);
    alert.setMessageText(&NSString::from_str("Quit Awebo?"));
    alert.setInformativeText(&NSString::from_str(
        "This is the last tab. Are you sure you want to quit?",
    ));

    alert.addButtonWithTitle(&NSString::from_str("Quit"));
    alert.addButtonWithTitle(&NSString::from_str("Cancel"));

    let response = alert.runModal();
    response == NSAlertFirstButtonReturn
}

/// Linux: GTK dialog via rfd. Windows: Win32 MessageBoxW via rfd.
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub fn confirm_quit() -> bool {
    use rfd::{MessageButtons, MessageDialog, MessageLevel};

    MessageDialog::new()
        .set_level(MessageLevel::Info)
        .set_title("Quit Awebo?")
        .set_description("This is the last tab. Are you sure you want to quit?")
        .set_buttons(MessageButtons::YesNo)
        .show()
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub fn confirm_quit() -> bool {
    true
}
