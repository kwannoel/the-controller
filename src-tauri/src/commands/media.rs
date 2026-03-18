use tauri::AppHandle;
#[cfg(target_os = "macos")]
use tauri::Manager;

/// Read an image file from disk and copy it to the system clipboard.
/// Used by the drag-and-drop handler to put the dropped image on the clipboard
/// so Claude Code can read it via its standard clipboard image detection.
pub(crate) async fn copy_image_file_to_clipboard(
    app: AppHandle,
    path: String,
) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;

    tracing::debug!(path, "opening image file for clipboard copy");
    let image_data = tokio::task::spawn_blocking(move || {
        let img = image::open(&path).map_err(|e| {
            tracing::error!(path, %e, "failed to open image");
            format!("Failed to open image: {e}")
        })?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        tracing::debug!(width, height, "decoded image for clipboard");
        Ok::<_, String>((rgba.into_raw(), width, height))
    })
    .await
    .map_err(|e| {
        tracing::error!(%e, "image clipboard task panicked");
        format!("Task failed: {e}")
    })??;

    let (bytes, width, height) = image_data;
    let img = tauri::image::Image::new_owned(bytes, width, height);
    app.clipboard().write_image(&img).map_err(|e| {
        tracing::error!(%e, "failed to write image to clipboard");
        format!("Failed to write image to clipboard: {e}")
    })
}

/// Capture a screenshot and save it to a temporary file.
/// When `cropped` is false, captures the app window using the window ID.
/// When `cropped` is true, launches interactive crop selection via `screencapture -i`.
/// Returns the path to the screenshot file so the caller can pass it to `initial_prompt`.
#[allow(unused_variables)]
pub(crate) async fn capture_app_screenshot(
    app: AppHandle,
    cropped: bool,
) -> Result<String, String> {
    #[cfg(not(target_os = "macos"))]
    return Err("Screenshot capture is only supported on macOS".into());

    #[cfg(target_os = "macos")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};

        tracing::info!(cropped, "capturing screenshot");

        let tmp_path = std::env::temp_dir().join("the-controller-screenshot.png");
        let tmp_str = tmp_path.to_str().ok_or("Invalid temp path")?.to_string();

        let status = if cropped {
            tracing::debug!("launching interactive crop selection");
            tokio::task::spawn_blocking({
                let tmp_str = tmp_str.clone();
                move || {
                    std::process::Command::new("screencapture")
                        .arg("-i")
                        .arg(&tmp_str)
                        .status()
                }
            })
        } else {
            let window = app
                .get_webview_window("main")
                .ok_or("No main window found")?;

            let window_id = {
                let handle = window.window_handle().map_err(|e| e.to_string())?;
                match handle.as_raw() {
                    RawWindowHandle::AppKit(appkit) => {
                        let ns_view = appkit.ns_view.as_ptr();
                        unsafe { macos_window_number(ns_view) }
                    }
                    _ => return Err("Not a macOS window".into()),
                }
            };

            tracing::debug!(window_id, "capturing window by ID");
            tokio::task::spawn_blocking({
                let tmp_str = tmp_str.clone();
                move || {
                    std::process::Command::new("screencapture")
                        .arg("-x")
                        .arg(format!("-l{}", window_id))
                        .arg(&tmp_str)
                        .status()
                }
            })
        }
        .await
        .map_err(|e| {
            tracing::error!(%e, "screenshot task panicked");
            format!("Task failed: {e}")
        })?
        .map_err(|e| {
            tracing::error!(%e, "failed to run screencapture");
            format!("Failed to run screencapture: {e}")
        })?;

        if !status.success() {
            tracing::error!(code = ?status.code(), "screencapture exited with failure");
            return Err(
                "screencapture failed (screen recording permission may be required)".into(),
            );
        }

        tracing::info!(path = %tmp_str, "screenshot saved");
        Ok(tmp_str)
    }
}

/// Get the macOS CGWindowID from an NSView pointer.
/// Uses the Objective-C runtime to call [NSView window] then [NSWindow windowNumber].
#[cfg(target_os = "macos")]
unsafe fn macos_window_number(ns_view: *mut std::ffi::c_void) -> isize {
    extern "C" {
        fn sel_registerName(name: *const u8) -> *mut std::ffi::c_void;
        fn objc_msgSend(
            obj: *mut std::ffi::c_void,
            sel: *mut std::ffi::c_void,
            ...
        ) -> *mut std::ffi::c_void;
    }

    let sel_window = sel_registerName(c"window".as_ptr().cast());
    let ns_window = objc_msgSend(ns_view, sel_window);

    let sel_number = sel_registerName(c"windowNumber".as_ptr().cast());
    objc_msgSend(ns_window, sel_number) as isize
}
