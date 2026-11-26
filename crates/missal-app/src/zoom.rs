//! App-wide interface zoom, the way a browser zooms a page.
//!
//! GPUI lays everything out in logical pixels and multiplies by the display's
//! scale factor when it paints. Chromium's page zoom works the same way: it
//! multiplies the device scale factor and divides the viewport. [`ZoomPlatform`]
//! wraps the real platform and does exactly that for every window it opens, so
//! text, controls, spacing, hit testing and the editor all scale together and
//! stay crisp (glyphs are rasterised at the zoomed scale, not resampled).
//!
//! Screen coordinates (window position and size, displays) are untouched: only
//! what happens inside a window is zoomed.
//!
//! Keep the delegation complete. Every trait method with a default body that a
//! platform overrides must be forwarded here, or that platform loses it.

use std::cell::Cell;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use futures::channel::oneshot;
use gpui_kit::{
    A11yCallbacks, Action, ActivationPolicy, ActivityGuard, AnyWindowHandle, App,
    AppLifecyclePhase, BackgroundExecutor, Bounds, Capslock, ClipboardItem, ClipboardReadError,
    Corners, CursorStyle, Decorations, DispatchEventResult, ExternalDragPayload, FileDropEvent,
    ForegroundExecutor, GpuSpecs, GraphicalEnvironment, Keymap, Menu, MenuItem, Modifiers,
    OwnedMenu, PathPromptOptions, Pixels, Platform, PlatformAtlas, PlatformDisplay,
    PlatformGestures, PlatformInfo, PlatformInput, PlatformInputHandler, PlatformKeyboardLayout,
    PlatformKeyboardMapper, PlatformTextSystem, PlatformWindow, Point, PowerMode, PromptButton,
    PromptLevel, RequestFrameOptions, ResizeEdge, Scene, ScreenCaptureSource, ScrollDelta, Size,
    SystemNotification, SystemNotificationResponse, SystemWindowTab, Task, TextInputConfiguration,
    TextInputStateChange, ThermalState, Window, WindowAppearance, WindowBackgroundAppearance,
    WindowBounds, WindowButtonLayout, WindowControlArea, WindowControls, WindowDecorations,
    WindowInsets, WindowParams, WindowVisibility, WindowingRequest,
};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};
use smallvec::SmallVec;

use crate::prefs::{DEFAULT_WINDOW, InterfaceSize};
use crate::state::Models;

/// The smallest and largest zoom the app allows.
pub const MIN: f32 = 0.7;
pub const MAX: f32 = 1.5;

thread_local! {
    static ZOOM: Cell<f32> = const { Cell::new(1.0) };
}

/// The zoom every window is drawn at right now.
pub fn current() -> f32 {
    ZOOM.with(Cell::get)
}

/// Sets the zoom. Returns whether it changed. Windows pick it up on their next
/// `Window::bounds_changed`, which [`apply`] calls for each one.
pub fn set(zoom: f32) -> bool {
    let zoom = zoom.clamp(MIN, MAX);
    ZOOM.with(|cell| {
        let changed = (cell.get() - zoom).abs() > f32::EPSILON;
        cell.set(zoom);
        changed
    })
}

/// The zoom Auto picks for a screen whose usable area (without the taskbar or
/// dock) is `visible`, in the operating system's scaled pixels.
///
/// The layout is designed for a 1280 × 800 window. On a screen with less room
/// than that, such as a 1366 × 768 laptop or a 1080p one at 150 %, everything
/// is drawn smaller, in 5 % steps and never below 80 %, so the same layout fits.
pub fn auto_for(visible: Size<Pixels>) -> f32 {
    let width = f32::from(visible.width) / DEFAULT_WINDOW.0;
    let height = f32::from(visible.height) / DEFAULT_WINDOW.1;
    let percent = (width.min(height) * 20.0).floor() * 5.0;
    percent.clamp(80.0, 100.0) / 100.0
}

/// The zoom a preference resolves to on a screen with `visible` room.
pub fn resolve(size: InterfaceSize, visible: Option<Size<Pixels>>) -> f32 {
    match size {
        InterfaceSize::Auto => visible.map_or(1.0, auto_for),
        InterfaceSize::Percent(percent) => f32::from(percent) / 100.0,
    }
}

/// Puts the saved interface size into effect for `window`'s screen and
/// relayouts every window. Call it when the preference changes or a window
/// moves to another screen.
pub fn apply(window: &mut Window, cx: &mut App) {
    let size = Models::get(cx).prefs.read(cx).read().interface_size;
    let visible = window
        .display(cx)
        .map(|display| display.visible_bounds().size);
    if !set(resolve(size, visible)) {
        return;
    }
    window.bounds_changed(cx);
    let current = Window::window_handle(window);
    for handle in cx.windows() {
        if handle != current {
            let _ = handle.update(cx, |_, window, cx| window.bounds_changed(cx));
        }
    }
}

fn inward(value: Pixels) -> Pixels {
    value / current()
}

fn outward(value: Pixels) -> Pixels {
    value * current()
}

fn point_in(point: Point<Pixels>) -> Point<Pixels> {
    point.map(inward)
}

fn size_in(size: Size<Pixels>) -> Size<Pixels> {
    size.map(inward)
}

fn bounds_in(bounds: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds {
        origin: point_in(bounds.origin),
        size: size_in(bounds.size),
    }
}

fn bounds_out(bounds: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds {
        origin: bounds.origin.map(outward),
        size: bounds.size.map(outward),
    }
}

/// Converts an event from the platform's window pixels to zoomed pixels.
pub fn input_in(input: PlatformInput) -> PlatformInput {
    match input {
        PlatformInput::MouseDown(mut event) => {
            event.position = point_in(event.position);
            PlatformInput::MouseDown(event)
        }
        PlatformInput::MouseUp(mut event) => {
            event.position = point_in(event.position);
            PlatformInput::MouseUp(event)
        }
        PlatformInput::MousePressure(mut event) => {
            event.position = point_in(event.position);
            PlatformInput::MousePressure(event)
        }
        PlatformInput::MouseMove(mut event) => {
            event.position = point_in(event.position);
            PlatformInput::MouseMove(event)
        }
        PlatformInput::MouseExited(mut event) => {
            event.position = point_in(event.position);
            PlatformInput::MouseExited(event)
        }
        PlatformInput::Scroll(mut event) => {
            event.position = point_in(event.position);
            if let ScrollDelta::Pixels(delta) = event.delta {
                event.delta = ScrollDelta::Pixels(point_in(delta));
            }
            PlatformInput::Scroll(event)
        }
        PlatformInput::Pinch(mut event) => {
            event.position = point_in(event.position);
            PlatformInput::Pinch(event)
        }
        PlatformInput::LongPress(mut event) => {
            event.start_position = point_in(event.start_position);
            event.position = point_in(event.position);
            PlatformInput::LongPress(event)
        }
        PlatformInput::TouchDrag(mut event) => {
            event.start_position = point_in(event.start_position);
            event.position = point_in(event.position);
            PlatformInput::TouchDrag(event)
        }
        PlatformInput::BackGesture(mut event) => {
            event.start_position = point_in(event.start_position);
            PlatformInput::BackGesture(event)
        }
        PlatformInput::Touch(mut event) => {
            event.position = point_in(event.position);
            PlatformInput::Touch(event)
        }
        PlatformInput::FileDrop(event) => PlatformInput::FileDrop(match event {
            FileDropEvent::Entered { position, paths } => FileDropEvent::Entered {
                position: point_in(position),
                paths,
            },
            FileDropEvent::Pending { position } => FileDropEvent::Pending {
                position: point_in(position),
            },
            FileDropEvent::Submit { position } => FileDropEvent::Submit {
                position: point_in(position),
            },
            other => other,
        }),
        other => other,
    }
}

/// The real platform, with every window it opens zoomed.
pub struct ZoomPlatform {
    inner: Rc<dyn Platform>,
}

impl ZoomPlatform {
    pub fn new(inner: Rc<dyn Platform>) -> ZoomPlatform {
        ZoomPlatform { inner }
    }
}

impl Platform for ZoomPlatform {
    fn platform_info(&self) -> PlatformInfo {
        self.inner.platform_info()
    }
    fn background_executor(&self) -> BackgroundExecutor {
        self.inner.background_executor()
    }
    fn foreground_executor(&self) -> ForegroundExecutor {
        self.inner.foreground_executor()
    }
    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.inner.text_system()
    }
    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>) {
        self.inner.run(on_finish_launching)
    }
    fn quit(&self) {
        self.inner.quit()
    }
    fn set_initial_windowing(&self, request: WindowingRequest) {
        self.inner.set_initial_windowing(request)
    }
    fn request_windowing(&self, request: WindowingRequest) -> Task<anyhow::Result<()>> {
        self.inner.request_windowing(request)
    }
    fn restart(&self, binary_path: Option<PathBuf>, arguments: Vec<OsString>) {
        self.inner.restart(binary_path, arguments)
    }
    fn activate(&self, ignoring_other_apps: bool) {
        self.inner.activate(ignoring_other_apps)
    }
    fn set_activation_policy(&self, policy: ActivationPolicy) {
        self.inner.set_activation_policy(policy)
    }
    fn hide(&self) {
        self.inner.hide()
    }
    fn hide_other_apps(&self) {
        self.inner.hide_other_apps()
    }
    fn unhide_other_apps(&self) {
        self.inner.unhide_other_apps()
    }
    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        self.inner.displays()
    }
    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.inner.primary_display()
    }
    fn on_displays_changed(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_displays_changed(callback)
    }
    fn active_window(&self) -> Option<AnyWindowHandle> {
        self.inner.active_window()
    }
    fn window_stack(&self) -> Option<Vec<AnyWindowHandle>> {
        self.inner.window_stack()
    }
    fn is_screen_capture_supported(&self) -> bool {
        self.inner.is_screen_capture_supported()
    }
    fn screen_capture_sources(
        &self,
    ) -> oneshot::Receiver<anyhow::Result<Vec<Rc<dyn ScreenCaptureSource>>>> {
        self.inner.screen_capture_sources()
    }
    fn open_window(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
    ) -> anyhow::Result<Box<dyn PlatformWindow>> {
        let inner = self.inner.open_window(handle, options)?;
        Ok(Box::new(ZoomWindow { inner }))
    }
    fn window_appearance(&self) -> WindowAppearance {
        self.inner.window_appearance()
    }
    fn set_window_appearance(&self, appearance: Option<WindowAppearance>) {
        self.inner.set_window_appearance(appearance)
    }
    fn button_layout(&self) -> Option<WindowButtonLayout> {
        self.inner.button_layout()
    }
    fn open_url(&self, url: &str) {
        self.inner.open_url(url)
    }
    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>) {
        self.inner.on_open_urls(callback)
    }
    fn register_url_scheme(&self, url: &str) -> Task<anyhow::Result<()>> {
        self.inner.register_url_scheme(url)
    }
    fn prompt_for_paths(
        &self,
        options: PathPromptOptions,
    ) -> oneshot::Receiver<anyhow::Result<Option<Vec<PathBuf>>>> {
        self.inner.prompt_for_paths(options)
    }
    fn prompt_for_new_path(
        &self,
        directory: &Path,
        suggested_name: Option<&str>,
    ) -> oneshot::Receiver<anyhow::Result<Option<PathBuf>>> {
        self.inner.prompt_for_new_path(directory, suggested_name)
    }
    fn can_select_mixed_files_and_dirs(&self) -> bool {
        self.inner.can_select_mixed_files_and_dirs()
    }
    fn reveal_path(&self, path: &Path) {
        self.inner.reveal_path(path)
    }
    fn open_with_system(&self, path: &Path) {
        self.inner.open_with_system(path)
    }
    fn on_quit(&self, callback: Box<dyn FnMut() -> bool>) {
        self.inner.on_quit(callback)
    }
    fn on_reopen(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_reopen(callback)
    }
    fn on_system_sleep(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_system_sleep(callback)
    }
    fn on_system_wake(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_system_wake(callback)
    }
    fn on_app_lifecycle(&self, callback: Box<dyn FnMut(AppLifecyclePhase)>) {
        self.inner.on_app_lifecycle(callback)
    }
    fn on_memory_warning(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_memory_warning(callback)
    }
    fn gestures(&self) -> Option<Rc<dyn PlatformGestures>> {
        self.inner.gestures()
    }
    fn set_menus(&self, menus: Vec<Menu>, keymap: &Keymap) {
        self.inner.set_menus(menus, keymap)
    }
    fn get_menus(&self) -> Option<Vec<OwnedMenu>> {
        self.inner.get_menus()
    }
    fn set_dock_menu(&self, menu: Vec<MenuItem>, keymap: &Keymap) {
        self.inner.set_dock_menu(menu, keymap)
    }
    fn perform_dock_menu_action(&self, action: usize) {
        self.inner.perform_dock_menu_action(action)
    }
    fn add_recent_document(&self, path: &Path) {
        self.inner.add_recent_document(path)
    }
    fn update_jump_list(
        &self,
        menus: Vec<MenuItem>,
        entries: Vec<SmallVec<[PathBuf; 2]>>,
    ) -> Task<Vec<SmallVec<[PathBuf; 2]>>> {
        self.inner.update_jump_list(menus, entries)
    }
    fn on_app_menu_action(&self, callback: Box<dyn FnMut(&dyn Action)>) {
        self.inner.on_app_menu_action(callback)
    }
    fn on_will_open_app_menu(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_will_open_app_menu(callback)
    }
    fn on_validate_app_menu_command(&self, callback: Box<dyn FnMut(&dyn Action) -> bool>) {
        self.inner.on_validate_app_menu_command(callback)
    }
    fn thermal_state(&self) -> ThermalState {
        self.inner.thermal_state()
    }
    fn on_thermal_state_change(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_thermal_state_change(callback)
    }
    fn prevent_idle_sleep(&self, reason: &str) -> Task<anyhow::Result<ActivityGuard>> {
        self.inner.prevent_idle_sleep(reason)
    }
    fn power_mode(&self) -> PowerMode {
        self.inner.power_mode()
    }
    fn on_power_mode_change(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_power_mode_change(callback)
    }
    fn set_app_identity(&self, identifier: &str, name: &str) {
        self.inner.set_app_identity(identifier, name)
    }
    fn show_system_notification(&self, notification: SystemNotification) {
        self.inner.show_system_notification(notification)
    }
    fn dismiss_system_notification(&self, tag: &str) {
        self.inner.dismiss_system_notification(tag)
    }
    fn on_system_notification_response(
        &self,
        callback: Box<dyn FnMut(SystemNotificationResponse)>,
    ) {
        self.inner.on_system_notification_response(callback)
    }
    fn compositor_name(&self) -> &'static str {
        self.inner.compositor_name()
    }
    fn graphical_environment(&self) -> Option<GraphicalEnvironment> {
        self.inner.graphical_environment()
    }
    fn app_path(&self) -> anyhow::Result<PathBuf> {
        self.inner.app_path()
    }
    fn path_for_auxiliary_executable(&self, name: &str) -> anyhow::Result<PathBuf> {
        self.inner.path_for_auxiliary_executable(name)
    }
    fn set_cursor_style(&self, style: CursorStyle) {
        self.inner.set_cursor_style(style)
    }
    fn hide_cursor_until_mouse_moves(&self) {
        self.inner.hide_cursor_until_mouse_moves()
    }
    fn is_cursor_visible(&self) -> bool {
        self.inner.is_cursor_visible()
    }
    fn should_auto_hide_scrollbars(&self) -> bool {
        self.inner.should_auto_hide_scrollbars()
    }
    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        self.inner.read_from_clipboard()
    }
    fn write_to_clipboard(&self, item: ClipboardItem) {
        self.inner.write_to_clipboard(item)
    }
    fn read_from_clipboard_async(&self) -> Task<Result<Option<ClipboardItem>, ClipboardReadError>> {
        self.inner.read_from_clipboard_async()
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn read_from_primary(&self) -> Option<ClipboardItem> {
        self.inner.read_from_primary()
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn write_to_primary(&self, item: ClipboardItem) {
        self.inner.write_to_primary(item)
    }
    #[cfg(target_os = "macos")]
    fn read_from_find_pasteboard(&self) -> Option<ClipboardItem> {
        self.inner.read_from_find_pasteboard()
    }
    #[cfg(target_os = "macos")]
    fn write_to_find_pasteboard(&self, item: ClipboardItem) {
        self.inner.write_to_find_pasteboard(item)
    }
    fn write_credentials(
        &self,
        url: &str,
        username: &str,
        password: &[u8],
    ) -> Task<anyhow::Result<()>> {
        self.inner.write_credentials(url, username, password)
    }
    #[cfg(target_os = "macos")]
    fn write_credentials_if_not_exists(
        &self,
        url: &str,
        username: &str,
        password: &[u8],
    ) -> Task<anyhow::Result<bool>> {
        self.inner
            .write_credentials_if_not_exists(url, username, password)
    }
    fn read_credentials(&self, url: &str) -> Task<anyhow::Result<Option<(String, Vec<u8>)>>> {
        self.inner.read_credentials(url)
    }
    fn delete_credentials(&self, url: &str) -> Task<anyhow::Result<()>> {
        self.inner.delete_credentials(url)
    }
    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        self.inner.keyboard_layout()
    }
    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        self.inner.keyboard_mapper()
    }
    fn on_keyboard_layout_change(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_keyboard_layout_change(callback)
    }
}

/// A platform window drawn at the app's zoom.
struct ZoomWindow {
    inner: Box<dyn PlatformWindow>,
}

impl HasWindowHandle for ZoomWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.inner.window_handle()
    }
}

impl HasDisplayHandle for ZoomWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.inner.display_handle()
    }
}

impl PlatformWindow for ZoomWindow {
    fn bounds(&self) -> Bounds<Pixels> {
        self.inner.bounds()
    }
    fn is_maximized(&self) -> bool {
        self.inner.is_maximized()
    }
    fn window_bounds(&self) -> WindowBounds {
        self.inner.window_bounds()
    }
    fn content_size(&self) -> Size<Pixels> {
        size_in(self.inner.content_size())
    }
    fn visual_viewport_bounds(&self) -> Bounds<Pixels> {
        bounds_in(self.inner.visual_viewport_bounds())
    }
    fn on_visual_viewport_changed(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_visual_viewport_changed(callback)
    }
    fn prepare_frame(&self) -> bool {
        self.inner.prepare_frame()
    }
    fn resize(&mut self, size: Size<Pixels>) {
        self.inner.resize(size.map(outward))
    }
    fn scale_factor(&self) -> f32 {
        self.inner.scale_factor() * current()
    }
    fn appearance(&self) -> WindowAppearance {
        self.inner.appearance()
    }
    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.inner.display()
    }
    fn mouse_position(&self) -> Point<Pixels> {
        point_in(self.inner.mouse_position())
    }
    fn modifiers(&self) -> Modifiers {
        self.inner.modifiers()
    }
    fn capslock(&self) -> Capslock {
        self.inner.capslock()
    }
    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        self.inner.set_input_handler(input_handler)
    }
    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.inner.take_input_handler()
    }
    fn set_text_input_configuration(&mut self, configuration: TextInputConfiguration) {
        self.inner.set_text_input_configuration(configuration)
    }
    fn prompt(
        &self,
        level: PromptLevel,
        msg: &str,
        detail: Option<&str>,
        answers: &[PromptButton],
    ) -> Option<oneshot::Receiver<usize>> {
        self.inner.prompt(level, msg, detail, answers)
    }
    fn activate(&self) {
        self.inner.activate()
    }
    fn request_attention(&self) {
        self.inner.request_attention()
    }
    fn is_active(&self) -> bool {
        self.inner.is_active()
    }
    fn visibility(&self) -> WindowVisibility {
        self.inner.visibility()
    }
    fn is_hovered(&self) -> bool {
        self.inner.is_hovered()
    }
    fn background_appearance(&self) -> WindowBackgroundAppearance {
        self.inner.background_appearance()
    }
    fn set_title(&mut self, title: &str) {
        self.inner.set_title(title)
    }
    fn set_background_appearance(&self, background_appearance: WindowBackgroundAppearance) {
        self.inner.set_background_appearance(background_appearance)
    }
    fn minimize(&self) {
        self.inner.minimize()
    }
    fn zoom(&self) {
        self.inner.zoom()
    }
    fn toggle_fullscreen(&self) {
        self.inner.toggle_fullscreen()
    }
    fn is_fullscreen(&self) -> bool {
        self.inner.is_fullscreen()
    }
    fn frame_waker(&self) -> Option<Rc<dyn Fn()>> {
        self.inner.frame_waker()
    }
    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>) {
        self.inner.on_request_frame(callback)
    }
    fn on_input(&self, mut callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>) {
        self.inner
            .on_input(Box::new(move |input| callback(input_in(input))))
    }
    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.inner.on_active_status_change(callback)
    }
    fn on_visibility_change(&self, callback: Box<dyn FnMut(WindowVisibility)>) {
        self.inner.on_visibility_change(callback)
    }
    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.inner.on_hover_status_change(callback)
    }
    fn on_resize(&self, mut callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        self.inner.on_resize(Box::new(move |size, scale| {
            callback(size_in(size), scale * current())
        }))
    }
    fn on_moved(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_moved(callback)
    }
    fn on_display_changed(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_display_changed(callback)
    }
    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>) {
        self.inner.on_should_close(callback)
    }
    fn on_hit_test_window_control(&self, callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
        self.inner.on_hit_test_window_control(callback)
    }
    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        self.inner.on_close(callback)
    }
    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_appearance_changed(callback)
    }
    fn on_button_layout_changed(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_button_layout_changed(callback)
    }
    fn draw(&self, scene: &Scene) {
        self.inner.draw(scene)
    }
    fn schedule_frame(&self) {
        self.inner.schedule_frame()
    }
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.inner.sprite_atlas()
    }
    fn is_subpixel_rendering_supported(&self) -> bool {
        self.inner.is_subpixel_rendering_supported()
    }
    fn get_title(&self) -> String {
        self.inner.get_title()
    }
    fn tabbed_windows(&self) -> Option<Vec<SystemWindowTab>> {
        self.inner.tabbed_windows()
    }
    fn tab_bar_visible(&self) -> bool {
        self.inner.tab_bar_visible()
    }
    fn set_edited(&mut self, edited: bool) {
        self.inner.set_edited(edited)
    }
    fn set_document_path(&self, path: Option<&Path>) {
        self.inner.set_document_path(path)
    }
    fn toggle_simple_fullscreen(&self) {
        self.inner.toggle_simple_fullscreen()
    }
    fn is_simple_fullscreen(&self) -> bool {
        self.inner.is_simple_fullscreen()
    }
    #[cfg(target_os = "macos")]
    fn set_traffic_light_position(&self, position: Point<Pixels>) {
        self.inner.set_traffic_light_position(position)
    }
    fn show_character_palette(&self) {
        self.inner.show_character_palette()
    }
    fn titlebar_double_click(&self, is_resizable: bool, is_minimizable: bool) {
        self.inner
            .titlebar_double_click(is_resizable, is_minimizable)
    }
    fn on_move_tab_to_new_window(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_move_tab_to_new_window(callback)
    }
    fn on_merge_all_windows(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_merge_all_windows(callback)
    }
    fn on_select_previous_tab(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_select_previous_tab(callback)
    }
    fn on_select_next_tab(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_select_next_tab(callback)
    }
    fn on_toggle_tab_bar(&self, callback: Box<dyn FnMut()>) {
        self.inner.on_toggle_tab_bar(callback)
    }
    fn merge_all_windows(&self) {
        self.inner.merge_all_windows()
    }
    fn move_tab_to_new_window(&self) {
        self.inner.move_tab_to_new_window()
    }
    fn toggle_window_tab_overview(&self) {
        self.inner.toggle_window_tab_overview()
    }
    fn set_tabbing_identifier(&self, identifier: Option<String>) {
        self.inner.set_tabbing_identifier(identifier)
    }
    fn native_window_state(&self) -> Option<Vec<u8>> {
        self.inner.native_window_state()
    }
    fn restore_native_window_state(&self, state: &[u8]) {
        self.inner.restore_native_window_state(state)
    }
    #[cfg(target_os = "windows")]
    fn get_raw_handle(&self) -> windows::Win32::Foundation::HWND {
        self.inner.get_raw_handle()
    }
    fn inner_window_bounds(&self) -> WindowBounds {
        self.inner.inner_window_bounds()
    }
    fn request_decorations(&self, decorations: WindowDecorations) {
        self.inner.request_decorations(decorations)
    }
    fn show_window_menu(&self, position: Point<Pixels>) {
        self.inner.show_window_menu(position.map(outward))
    }
    fn start_window_move(&self) {
        self.inner.start_window_move()
    }
    fn can_start_external_drag(&self) -> bool {
        self.inner.can_start_external_drag()
    }
    fn start_external_drag(&self, payload: &ExternalDragPayload) -> bool {
        self.inner.start_external_drag(payload)
    }
    fn start_window_resize(&self, edge: ResizeEdge) {
        self.inner.start_window_resize(edge)
    }
    fn set_exclusive_zone(&self, zone: Pixels) {
        self.inner.set_exclusive_zone(outward(zone))
    }
    fn set_input_region(&self, region: Option<&[Bounds<Pixels>]>) {
        let region: Option<Vec<Bounds<Pixels>>> =
            region.map(|region| region.iter().copied().map(bounds_out).collect());
        self.inner.set_input_region(region.as_deref())
    }
    fn window_decorations(&self) -> Decorations {
        self.inner.window_decorations()
    }
    fn set_app_id(&mut self, app_id: &str) {
        self.inner.set_app_id(app_id)
    }
    fn map_window(&mut self) -> anyhow::Result<()> {
        self.inner.map_window()
    }
    fn window_controls(&self) -> WindowControls {
        self.inner.window_controls()
    }
    fn set_client_inset(&self, inset: Pixels) {
        self.inner.set_client_inset(outward(inset))
    }
    fn set_client_corner_radii(&self, radii: Corners<Pixels>) {
        self.inner
            .set_client_corner_radii(radii.map(|radius| outward(*radius)))
    }
    fn gpu_specs(&self) -> Option<GpuSpecs> {
        self.inner.gpu_specs()
    }
    fn update_ime_position(&self, bounds: Bounds<Pixels>) {
        self.inner.update_ime_position(bounds_out(bounds))
    }
    fn insets(&self) -> WindowInsets {
        self.inner.insets()
    }
    fn on_insets_changed(&self, callback: Box<dyn FnMut(WindowInsets)>) {
        self.inner.on_insets_changed(callback)
    }
    fn set_back_handler(&self, callback: Box<dyn FnMut()>) {
        self.inner.set_back_handler(callback)
    }
    fn set_back_enabled(&self, enabled: bool) {
        self.inner.set_back_enabled(enabled)
    }
    fn show_soft_keyboard(&self) {
        self.inner.show_soft_keyboard()
    }
    fn hide_soft_keyboard(&self) {
        self.inner.hide_soft_keyboard()
    }
    fn text_input_state_changed(&self, change: TextInputStateChange) {
        self.inner.text_input_state_changed(change)
    }
    fn play_system_bell(&self) {
        self.inner.play_system_bell()
    }
    fn a11y_init(&self, callbacks: A11yCallbacks) {
        self.inner.a11y_init(callbacks)
    }
    fn a11y_tree_update(&self, tree_update: accesskit::TreeUpdate) {
        self.inner.a11y_tree_update(tree_update)
    }
    fn a11y_update_window_bounds(&self) {
        self.inner.a11y_update_window_bounds()
    }
}
