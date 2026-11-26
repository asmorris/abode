//! The system's accessibility settings that Abode follows.

/// Has the person asked their computer for more contrast? Increase contrast
/// on a Mac, a High contrast / Contrast theme on Windows, High Contrast on
/// GNOME. Cheap enough to call when the window regains focus.
pub fn system_high_contrast() -> bool {
    imp::high_contrast()
}

#[cfg(target_os = "macos")]
mod imp {
    pub fn high_contrast() -> bool {
        objc2_app_kit::NSWorkspace::sharedWorkspace().accessibilityDisplayShouldIncreaseContrast()
    }
}

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
    use windows_sys::Win32::UI::WindowsAndMessaging::{SPI_GETHIGHCONTRAST, SystemParametersInfoW};

    pub fn high_contrast() -> bool {
        let mut hc = HIGHCONTRASTW { cbSize: size_of::<HIGHCONTRASTW>() as u32, dwFlags: 0, lpszDefaultScheme: std::ptr::null_mut() };
        // SAFETY: SPI_GETHIGHCONTRAST fills in the HIGHCONTRASTW we pass, whose
        // size is given both in cbSize and as uiParam.
        let ok = unsafe { SystemParametersInfoW(SPI_GETHIGHCONTRAST, hc.cbSize, (&raw mut hc).cast(), 0) };
        ok != 0 && hc.dwFlags & HCF_HIGHCONTRASTON != 0
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod imp {
    /// GNOME's (and GNOME-based desktops') High Contrast switch.
    pub fn high_contrast() -> bool {
        std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.a11y.interface", "high-contrast"])
            .output()
            .is_ok_and(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "true")
    }
}
