//! Updates from GitHub Releases.
//!
//! Each release (tag `vX.Y.Z`) carries:
//! - `Abode-X.Y.Z-mac.zip`: the universal `Abode.app`, used by the updater
//! - `Abode-X.Y.Z-mac.dmg`: the same app for people downloading by hand
//! - `Abode-X.Y.Z-windows-setup.exe`: the Windows installer
//! - `Abode-X.Y.Z-linux-<arch>.tar.gz`: the Linux app folder (`x86_64`, `aarch64`)
//!
//! Checking and downloading run on a background thread; the UI polls
//! [`Updater::status`]. Installing:
//! - **macOS**: unzip the new `Abode.app` next to the running one, swap them,
//!   and relaunch.
//! - **Windows**: run the new installer silently (it waits for us to quit,
//!   installs over the old version and starts Abode again).
//! - **Linux**: unpack the new app folder next to the running one, move its
//!   files over the old ones, and relaunch.

use anyhow::{Context, Result, anyhow, bail};
use eframe::egui;
use std::io::{Read, Write};
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Where releases come from (override at build time with `ABODE_UPDATE_REPO`).
pub const REPO: &str = match option_env!("ABODE_UPDATE_REPO") {
    Some(r) => r,
    None => "asmorris/abode",
};
/// This build's version. `ABODE_PRETEND_VERSION` (at build time) makes a build
/// think it's older, for testing updates end to end.
pub const VERSION: &str = match option_env!("ABODE_PRETEND_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: String,
    pub notes: String,
    /// The release page, for downloading by hand.
    pub page: String,
    /// The file this platform's updater installs.
    pub asset: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Status {
    Idle,
    Checking,
    UpToDate,
    Available(Release),
    /// Downloading/installing; progress 0..1 (or None if unknown).
    Installing { release: Release, progress: Option<f32>, step: &'static str },
    /// Installed; quit and run this to start the new version.
    Ready,
    Failed { message: String, page: Option<String> },
}

#[derive(Clone)]
pub struct Updater {
    status: Arc<Mutex<Status>>,
    relaunch: Arc<Mutex<Option<Command>>>,
}

impl Default for Updater {
    fn default() -> Self {
        Self { status: Arc::new(Mutex::new(Status::Idle)), relaunch: Arc::new(Mutex::new(None)) }
    }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(60))
        .user_agent(&format!("Abode/{VERSION} (updater)"))
        .build()
}

/// `"v1.2.3"` → `(1, 2, 3)`. Anything after `-` or `+` is ignored.
pub fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches('v');
    let core = v.split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    Some((parts.next()??, parts.next().flatten().unwrap_or(0), parts.next().flatten().unwrap_or(0)))
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

fn asset_suffix() -> Option<&'static str> {
    if cfg!(target_os = "macos") {
        Some("-mac.zip")
    } else if cfg!(target_os = "windows") {
        Some("-windows-setup.exe")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("-linux-x86_64.tar.gz")
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        Some("-linux-aarch64.tar.gz")
    } else {
        None
    }
}

fn fetch_latest() -> Result<Release> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let resp = agent()
        .get(&url)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(404, _) => anyhow!("no releases have been published yet"),
            other => anyhow!("couldn't reach GitHub ({other})"),
        })?;
    let json: serde_json::Value = serde_json::from_reader(resp.into_reader())?;
    let tag = json["tag_name"].as_str().ok_or_else(|| anyhow!("the release has no version tag"))?;
    let asset = asset_suffix().and_then(|suffix| {
        json["assets"].as_array()?.iter().find_map(|a| {
            let name = a["name"].as_str()?;
            name.ends_with(suffix).then(|| a["browser_download_url"].as_str().map(String::from)).flatten()
        })
    });
    Ok(Release {
        version: tag.trim_start_matches('v').to_string(),
        notes: json["body"].as_str().unwrap_or("").trim().to_string(),
        page: json["html_url"].as_str().unwrap_or("").to_string(),
        asset,
    })
}

impl Updater {
    /// Screenshots only: show an update as available without checking.
    pub fn pretend_available(&self, release: Release) {
        if let Ok(mut g) = self.status.lock() {
            *g = Status::Available(release);
        }
    }

    pub fn status(&self) -> Status {
        self.status.lock().map(|s| s.clone()).unwrap_or(Status::Idle)
    }

    fn set(&self, s: Status) {
        if let Ok(mut g) = self.status.lock() {
            *g = s;
        }
    }

    pub fn dismiss(&self) {
        self.set(Status::Idle);
    }

    pub fn busy(&self) -> bool {
        matches!(self.status(), Status::Checking | Status::Installing { .. })
    }

    /// Check for a newer release in the background.
    pub fn check(&self, ctx: &egui::Context) {
        if self.busy() {
            return;
        }
        self.set(Status::Checking);
        let me = self.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let s = match fetch_latest() {
                Ok(r) if is_newer(&r.version, VERSION) => Status::Available(r),
                Ok(_) => Status::UpToDate,
                Err(e) => Status::Failed { message: format!("Couldn't check for updates: {e:#}"), page: None },
            };
            me.set(s);
            ctx.request_repaint();
        });
    }

    /// Download and install `release` in the background.
    pub fn install(&self, ctx: &egui::Context, release: Release) {
        if self.busy() {
            return;
        }
        self.set(Status::Installing { release: release.clone(), progress: None, step: "Downloading" });
        let me = self.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = me.do_install(&ctx, &release);
            match result {
                Ok(cmd) => {
                    if let Ok(mut r) = me.relaunch.lock() {
                        *r = Some(cmd);
                    }
                    me.set(Status::Ready);
                }
                Err(e) => me.set(Status::Failed {
                    message: format!("The update didn't install: {e:#}"),
                    page: Some(release.page.clone()),
                }),
            }
            ctx.request_repaint();
        });
    }

    /// After [`Status::Ready`]: start the new version. Call right before quitting.
    pub fn relaunch(&self) {
        if let Some(mut cmd) = self.relaunch.lock().ok().and_then(|mut r| r.take()) {
            let _ = cmd.spawn();
        }
    }

    fn progress(&self, ctx: &egui::Context, release: &Release, progress: Option<f32>, step: &'static str) {
        self.set(Status::Installing { release: release.clone(), progress, step });
        ctx.request_repaint();
    }

    fn download(&self, ctx: &egui::Context, release: &Release, url: &str, dest: &Path) -> Result<()> {
        let resp = agent().get(url).call().context("download failed")?;
        let total: Option<u64> = resp.header("Content-Length").and_then(|v| v.parse().ok());
        let mut reader = resp.into_reader();
        let mut file = std::fs::File::create(dest)?;
        let mut buf = vec![0u8; 64 * 1024];
        let mut done: u64 = 0;
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])?;
            done += n as u64;
            let p = total.map(|t| (done as f32 / t.max(1) as f32).min(1.0));
            self.progress(ctx, release, p, "Downloading");
        }
        file.flush()?;
        if let Some(t) = total {
            if done != t {
                bail!("the download was incomplete");
            }
        }
        Ok(())
    }

    fn do_install(&self, ctx: &egui::Context, release: &Release) -> Result<Command> {
        let url = release
            .asset
            .clone()
            .ok_or_else(|| anyhow!("this release has no download for your computer"))?;
        let dir = tempfile::Builder::new().prefix("abode-update-").tempdir()?.keep();
        let name = url.rsplit('/').next().unwrap_or("update").to_string();
        let file = dir.join(&name);
        self.download(ctx, release, &url, &file)?;
        self.progress(ctx, release, None, "Installing");
        install_platform(&file, &dir)
    }
}

/// The `.app` bundle we're running from, if any.
#[cfg(target_os = "macos")]
fn current_bundle() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    let bundle = exe.parent()?.parent()?.parent()?.to_path_buf();
    (bundle.extension().is_some_and(|e| e == "app")).then_some(bundle)
}

#[cfg(target_os = "macos")]
fn install_platform(zip: &Path, dir: &Path) -> Result<Command> {
    let bundle = current_bundle().ok_or_else(|| {
        anyhow!("this copy of Abode isn't an installed app (it's running from the source code), so it can't update itself")
    })?;
    let run = |cmd: &mut Command| -> Result<()> {
        let out = cmd.output()?;
        if !out.status.success() {
            bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
        }
        Ok(())
    };
    let extract = dir.join("extracted");
    run(Command::new("/usr/bin/ditto").args(["-x", "-k"]).arg(zip).arg(&extract)).context("couldn't unpack the update")?;
    let new_app = std::fs::read_dir(&extract)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| p.extension().is_some_and(|e| e == "app"))
        .ok_or_else(|| anyhow!("the download didn't contain the app"))?;

    let parent = bundle.parent().ok_or_else(|| anyhow!("can't find where Abode is installed"))?;
    let staged = parent.join(".Abode-update.app");
    let old = parent.join(".Abode-old.app");
    let _ = std::fs::remove_dir_all(&staged);
    let _ = std::fs::remove_dir_all(&old);
    run(Command::new("/usr/bin/ditto").arg(&new_app).arg(&staged))
        .with_context(|| format!("couldn't write to {} (is Abode in your Applications folder?)", parent.display()))?;
    std::fs::rename(&bundle, &old).context("couldn't move the old version aside")?;
    if let Err(e) = std::fs::rename(&staged, &bundle) {
        let _ = std::fs::rename(&old, &bundle);
        return Err(e).context("couldn't put the new version in place");
    }
    let _ = std::fs::remove_dir_all(&old);
    let _ = Command::new("/usr/bin/xattr").args(["-dr", "com.apple.quarantine"]).arg(&bundle).output();

    // Relaunch once we've quit.
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c").arg("sleep 1; /usr/bin/open \"$0\"").arg(&bundle);
    Ok(cmd)
}

#[cfg(target_os = "windows")]
fn install_platform(setup: &Path, _dir: &Path) -> Result<Command> {
    // The installer waits a moment for us to exit, installs silently over the
    // current version, then starts Abode again.
    let mut cmd = Command::new(setup);
    cmd.arg("/S");
    Ok(cmd)
}

#[cfg(target_os = "linux")]
fn install_platform(tarball: &Path, dir: &Path) -> Result<Command> {
    // Work out where we're running from before replacing anything: once the
    // binary is replaced, /proc/self/exe points at a deleted file.
    let exe = std::env::current_exe()?.canonicalize()?;
    let install = exe.parent().ok_or_else(|| anyhow!("can't find where Abode is installed"))?;
    // The release folder ships install.sh next to the binary; a build from the
    // source code doesn't have it.
    if !install.join("install.sh").is_file() {
        bail!("this copy of Abode isn't an installed app (it's running from the source code), so it can't update itself");
    }

    let extract = dir.join("extracted");
    std::fs::create_dir_all(&extract)?;
    let out = Command::new("tar").arg("-xzf").arg(tarball).arg("-C").arg(&extract).output()?;
    if !out.status.success() {
        bail!("couldn't unpack the update: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let new_dir = extract.join("Abode");
    if !new_dir.join("abode").is_file() {
        bail!("the download didn't contain the app");
    }

    // Stage next to the install so the final renames stay on one disk, then
    // move each file over the old one. Renaming over the running binary is
    // fine on Linux: we keep running the old copy until we quit.
    let staged = install.join(".abode-update");
    let _ = std::fs::remove_dir_all(&staged);
    std::fs::create_dir(&staged)
        .with_context(|| format!("couldn't write to {} (try reinstalling Abode)", install.display()))?;
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&new_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            let name = entry.file_name();
            std::fs::copy(entry.path(), staged.join(&name)).context("couldn't stage the update")?;
            files.push(name);
        }
    }
    for name in &files {
        std::fs::rename(staged.join(name), install.join(name)).context("couldn't put the new version in place")?;
    }
    let _ = std::fs::remove_dir_all(&staged);

    // Relaunch once we've quit.
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c").arg("sleep 1; exec \"$0\"").arg(&exe);
    Ok(cmd)
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn install_platform(_file: &Path, _dir: &Path) -> Result<Command> {
    bail!("automatic updates aren't supported on this system yet")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare() {
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("0.4"), Some((0, 4, 0)));
        assert!(is_newer("v0.2.0", "0.1.9"));
        assert!(is_newer("1.0.0", "0.9.12"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0-beta", "0.1.0"));
        assert!(!is_newer("nonsense", "0.1.0"));
    }
}
