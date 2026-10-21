//! AppKit calls on the main thread. The unsafe bridges read the view the
//! window handle points at while the window that owns it is alive, and two
//! constant appearance names AppKit exports.
#![allow(unsafe_code)]

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    NSAutoresizingMaskOptions, NSUserInterfaceItemIdentification, NSView,
    NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
    NSWindowOrderingMode,
};
use objc2_foundation::NSString;
use raw_window_handle::RawWindowHandle;

/// Marks the backdrop view, so it is found again and never added twice.
const IDENTIFIER: &str = "lumilio-backdrop";

pub(super) fn set_blur(handle: RawWindowHandle, on: bool, dark: bool) -> bool {
    let RawWindowHandle::AppKit(handle) = handle else {
        return false;
    };
    let Some(main_thread) = MainThreadMarker::new() else {
        return false;
    };
    // SAFETY: the handle comes from a live window on this, the main thread;
    // its view is owned by that window for the duration of this call.
    let view: &NSView = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    let Some(content) = view.window().and_then(|window| window.contentView()) else {
        return false;
    };
    let identifier = NSString::from_str(IDENTIFIER);
    let existing: Option<Retained<NSView>> = content
        .subviews()
        .iter()
        .find(|child| child.identifier().is_some_and(|id| *id == *identifier));
    if !on {
        if let Some(backdrop) = existing {
            backdrop.removeFromSuperview();
        }
        return true;
    }
    let backdrop = existing.unwrap_or_else(|| {
        let backdrop = NSVisualEffectView::initWithFrame(
            main_thread.alloc::<NSVisualEffectView>(),
            content.bounds(),
        );
        // The lightest of the system materials: the desktop's colour comes
        // through, and the theme's own page colour does the tinting.
        backdrop.setMaterial(NSVisualEffectMaterial::FullScreenUI);
        backdrop.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        backdrop.setState(NSVisualEffectState::Active);
        backdrop.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        backdrop.setIdentifier(Some(&identifier));
        content.addSubview_positioned_relativeTo(&backdrop, NSWindowOrderingMode::Below, None);
        Retained::into_super(backdrop)
    });
    // The material follows the theme's tone, not the system's: a dark theme
    // over a light material turns grey.
    // SAFETY: AppKit's appearance names are immutable constants.
    let name = unsafe {
        if dark {
            NSAppearanceNameDarkAqua
        } else {
            NSAppearanceNameAqua
        }
    };
    backdrop.setAppearance(NSAppearance::appearanceNamed(name).as_deref());
    true
}
