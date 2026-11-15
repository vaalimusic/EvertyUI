//! Application runner integrating `winit` 0.30 and `softbuffer` 0.4.
//!
//! Provides a reactive event loop that sleeps with zero CPU usage until woken
//! by OS window events or background thread events (via `EventLoopProxy`).

use std::sync::Arc;
use std::time::Instant;

use crate::{
    renderer::Renderer, CursorIcon, DpiScale, HitRegion, InputEvent, LogicalPoint, LogicalRect,
    PhysicalSize, Rgba8Renderer, TextSystem, Theme, WindowCloseHandler, WindowState,
};
use winit::application::ApplicationHandler;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowAttributes, WindowId};

/// Configuration parameters for initializing a native window.
#[derive(Clone, Debug)]
pub struct WindowConfig {
    pub title: String,
    pub width: f64,
    pub height: f64,
    pub resizable: bool,
    pub always_on_top: bool,
    pub decorations: bool,
    pub transparent: bool,
    pub background: [u8; 3],
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "EvertyUI App".to_string(),
            width: 800.0,
            height: 600.0,
            resizable: true,
            always_on_top: false,
            decorations: true,
            transparent: false,
            background: [246, 247, 249],
        }
    }
}

/// Composes straight-alpha RGBA8 pixels over an opaque RGB background into softbuffer's `0x00RRGGBB` format.
#[inline]
pub fn compose_rgba_over_rgb(rgba: &[u8], bg: [u8; 3], out: &mut [u32]) {
    for (pixel, target) in rgba.chunks_exact(4).zip(out.iter_mut()) {
        let a = u32::from(pixel[3]);
        if a == 255 {
            *target = (u32::from(pixel[0]) << 16) | (u32::from(pixel[1]) << 8) | u32::from(pixel[2]);
        } else if a == 0 {
            *target = (u32::from(bg[0]) << 16) | (u32::from(bg[1]) << 8) | u32::from(bg[2]);
        } else {
            let inv_a = 255 - a;
            let r = (u32::from(pixel[0]) * a + u32::from(bg[0]) * inv_a + 127) / 255;
            let g = (u32::from(pixel[1]) * a + u32::from(bg[1]) * inv_a + 127) / 255;
            let b = (u32::from(pixel[2]) * a + u32::from(bg[2]) * inv_a + 127) / 255;
            *target = (r << 16) | (g << 8) | b;
        }
    }
}

/// Software presenter using `softbuffer` for CPU-based rendering without GPU dependencies.
pub struct SoftPresenter {
    surface: softbuffer::Surface<winit::event_loop::OwnedDisplayHandle, Arc<Window>>,
    background: [u8; 3],
}

impl SoftPresenter {
    pub fn new(
        display: winit::event_loop::OwnedDisplayHandle,
        window: Arc<Window>,
        background: [u8; 3],
    ) -> Result<Self, String> {
        let context = softbuffer::Context::new(display).map_err(|e| e.to_string())?;
        let surface = softbuffer::Surface::new(&context, window).map_err(|e| e.to_string())?;
        Ok(Self { surface, background })
    }

    pub fn set_background(&mut self, background: [u8; 3]) {
        self.background = background;
    }

    pub fn present(&mut self, rgba: &[u8], width: u32, height: u32) -> Result<(), String> {
        let (Some(w), Some(h)) = (
            std::num::NonZeroU32::new(width),
            std::num::NonZeroU32::new(height),
        ) else {
            return Ok(());
        };
        if rgba.len() != (width as usize) * (height as usize) * 4 {
            return Err("pixel buffer length does not match width * height * 4".into());
        }
        self.surface.resize(w, h).map_err(|e| e.to_string())?;
        let mut buffer = self.surface.buffer_mut().map_err(|e| e.to_string())?;
        compose_rgba_over_rgb(rgba, self.background, &mut buffer);
        buffer.present().map_err(|e| e.to_string())
    }
}

/// Context provided to the application handler during event processing.
pub struct AppWindowContext<'a> {
    pub window: &'a Arc<Window>,
    pub state: &'a mut WindowState,
    pub input: &'a mut crate::InputState,
    pub close_handler: &'a mut WindowCloseHandler,
}

impl<'a> AppWindowContext<'a> {
    pub fn request_redraw(&mut self) {
        self.state.request_redraw();
        self.window.request_redraw();
    }

    pub fn set_title(&self, title: &str) {
        self.window.set_title(title);
    }

    pub fn set_minimized(&self, minimized: bool) {
        self.window.set_minimized(minimized);
    }

    pub fn focus_window(&self) {
        self.window.focus_window();
    }
}

/// Trait implemented by applications running inside `AppRunner`.
pub trait AppHandler: 'static {
    type UserEvent: 'static + Send;

    /// Called once when the native window has been created.
    fn on_ready(
        &mut self,
        _ctx: &mut AppWindowContext<'_>,
        _proxy: EventLoopProxy<Self::UserEvent>,
    ) {
    }

    /// Handle a background thread user event (e.g. incoming session, IPC, task progress).
    /// Returns `true` if this event requires a redraw.
    fn on_user_event(&mut self, _event: Self::UserEvent, _ctx: &mut AppWindowContext<'_>) -> bool {
        true
    }

    /// Handle a platform-neutral UI input event (pointer, keyboard, ime, wheel).
    /// Returns `true` if this event requires a redraw.
    fn on_input(&mut self, _event: &InputEvent, _ctx: &mut AppWindowContext<'_>) -> bool {
        false
    }

    /// Called when window size or DPI scale factor changes.
    fn on_metrics_changed(&mut self, _ctx: &mut AppWindowContext<'_>) {}

    /// Called when the user or OS requests window closure.
    /// Returns `true` if the window should exit immediately.
    fn on_close_requested(&mut self, ctx: &mut AppWindowContext<'_>) -> bool {
        let stage = ctx.close_handler.request_close(false, None, true);
        stage == crate::CloseStage::ReadyToClose
    }

    /// Returns the next deadline for time-dependent updates (e.g. cursor blink, animations).
    /// Returns `None` if idle, causing the runner to sleep with zero CPU usage.
    fn next_deadline(&self) -> Option<Instant> {
        None
    }

    /// Paints the current frame and returns hit regions for event routing.
    fn paint(
        &mut self,
        builder: &mut crate::PaintBuilder,
        text: &mut TextSystem,
        theme: &Theme,
        viewport: LogicalRect,
    ) -> Vec<HitRegion>;
}

/// Drives the desktop window and reactive event loop.
pub struct AppRunner<H: AppHandler> {
    config: WindowConfig,
    handler: H,
    proxy: Option<EventLoopProxy<H::UserEvent>>,
    window: Option<Arc<Window>>,
    soft: Option<SoftPresenter>,
    raster: Rgba8Renderer,
    text: TextSystem,
    state: WindowState,
    input: crate::InputState,
    close_handler: WindowCloseHandler,
    theme: Theme,
    hit_regions: Vec<HitRegion>,
    cursor: CursorIcon,
    exit_requested: bool,
}

impl<H: AppHandler> AppRunner<H> {
    pub fn new(config: WindowConfig, handler: H) -> Self {
        let initial_size = PhysicalSize::new(config.width, config.height);
        Self {
            config,
            handler,
            proxy: None,
            window: None,
            soft: None,
            raster: Rgba8Renderer::new(),
            text: TextSystem::new(),
            state: WindowState::new(initial_size, DpiScale::ONE),
            input: crate::InputState::default(),
            close_handler: WindowCloseHandler::default(),
            theme: Theme::EVERTY_LIGHT,
            hit_regions: Vec::new(),
            cursor: CursorIcon::Default,
            exit_requested: false,
        }
    }

    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Builds an event loop along with its proxy before running, allowing senders to be established.
    pub fn build_event_loop() -> Result<
        (
            EventLoop<H::UserEvent>,
            EventLoopProxy<H::UserEvent>,
        ),
        String,
    > {
        let mut builder = EventLoop::<H::UserEvent>::with_user_event();
        #[cfg(windows)]
        {
            use winit::platform::windows::EventLoopBuilderExtWindows;
            builder.with_any_thread(true);
        }
        #[cfg(target_os = "linux")]
        {
            use winit::platform::x11::EventLoopBuilderExtX11;
            builder.with_any_thread(true);
        }
        let event_loop = builder.build().map_err(|e| e.to_string())?;
        let proxy = event_loop.create_proxy();
        Ok((event_loop, proxy))
    }

    /// Runs the application using an existing event loop.
    pub fn run_with_event_loop(mut self, event_loop: EventLoop<H::UserEvent>) -> Result<(), String> {
        self.proxy = Some(event_loop.create_proxy());
        event_loop.run_app(&mut self).map_err(|e| e.to_string())
    }

    /// Runs the application, initializing a new event loop.
    pub fn run(self) -> Result<(), String> {
        let (event_loop, _) = Self::build_event_loop()?;
        self.run_with_event_loop(event_loop)
    }

    fn draw(&mut self) {
        let Some(window) = self.window.as_ref() else { return };
        let physical = window.inner_size();
        if physical.width == 0 || physical.height == 0 {
            return;
        }
        let scale = self.state.scale;
        let viewport = LogicalRect::from_xywh(
            0.0,
            0.0,
            f64::from(physical.width) / scale.get(),
            f64::from(physical.height) / scale.get(),
        );

        let mut builder = crate::PaintBuilder::with_viewport(viewport);
        self.hit_regions = self.handler.paint(&mut builder, &mut self.text, &self.theme, viewport);

        let paint = builder.finish();
        let p_size = PhysicalSize::new(f64::from(physical.width), f64::from(physical.height));
        if let Err(e) = self.raster.render(&paint, &mut self.text, scale, p_size) {
            eprintln!("EvertyUI raster render error: {e}");
            return;
        }

        if let Some(soft) = self.soft.as_mut() {
            if let Err(e) = soft.present(self.raster.pixels(), physical.width, physical.height) {
                eprintln!("EvertyUI softbuffer presentation error: {e}");
            }
        }
    }
}

impl<H: AppHandler> ApplicationHandler<H::UserEvent> for AppRunner<H> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attributes = WindowAttributes::default()
            .with_title(&self.config.title)
            .with_inner_size(winit::dpi::LogicalSize::new(self.config.width, self.config.height))
            .with_resizable(self.config.resizable)
            .with_decorations(self.config.decorations)
            .with_transparent(self.config.transparent)
            .with_window_level(if self.config.always_on_top {
                winit::window::WindowLevel::AlwaysOnTop
            } else {
                winit::window::WindowLevel::Normal
            });

        let window = match event_loop.create_window(attributes) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("Failed to create native window: {e}");
                event_loop.exit();
                return;
            }
        };

        let soft = match SoftPresenter::new(
            event_loop.owned_display_handle(),
            window.clone(),
            self.config.background,
        ) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to create software presenter: {e}");
                event_loop.exit();
                return;
            }
        };

        let scale = DpiScale::new(window.scale_factor()).unwrap_or(DpiScale::ONE);
        let physical = window.inner_size();
        self.state.update_metrics(
            PhysicalSize::new(f64::from(physical.width), f64::from(physical.height)),
            scale,
        );

        self.window = Some(window.clone());
        self.soft = Some(soft);

        if let Some(proxy) = self.proxy.clone() {
            let mut ctx = AppWindowContext {
                window: &window,
                state: &mut self.state,
                input: &mut self.input,
                close_handler: &mut self.close_handler,
            };
            self.handler.on_ready(&mut ctx, proxy);
        }

        window.request_redraw();
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: H::UserEvent) {
        let Some(window) = self.window.as_ref() else { return };
        let mut ctx = AppWindowContext {
            window,
            state: &mut self.state,
            input: &mut self.input,
            close_handler: &mut self.close_handler,
        };
        if self.handler.on_user_event(event, &mut ctx) {
            self.state.request_redraw();
            window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: winit::event::WindowEvent,
    ) {
        let Some(window) = self.window.as_ref() else { return };
        if window.id() != window_id {
            return;
        }

        match event {
            winit::event::WindowEvent::CloseRequested => {
                let mut ctx = AppWindowContext {
                    window,
                    state: &mut self.state,
                    input: &mut self.input,
                    close_handler: &mut self.close_handler,
                };
                if self.handler.on_close_requested(&mut ctx) {
                    self.exit_requested = true;
                    event_loop.exit();
                }
            }
            winit::event::WindowEvent::Resized(size) => {
                let scale = DpiScale::new(window.scale_factor()).unwrap_or(DpiScale::ONE);
                let p_size = PhysicalSize::new(f64::from(size.width), f64::from(size.height));
                let change = self.state.update_metrics(p_size, scale);
                if change.size_changed || change.scale_changed {
                    let mut ctx = AppWindowContext {
                        window,
                        state: &mut self.state,
                        input: &mut self.input,
                        close_handler: &mut self.close_handler,
                    };
                    self.handler.on_metrics_changed(&mut ctx);
                    window.request_redraw();
                }
            }
            winit::event::WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let scale = DpiScale::new(scale_factor).unwrap_or(DpiScale::ONE);
                let physical = window.inner_size();
                let p_size = PhysicalSize::new(f64::from(physical.width), f64::from(physical.height));
                self.state.update_metrics(p_size, scale);
                let mut ctx = AppWindowContext {
                    window,
                    state: &mut self.state,
                    input: &mut self.input,
                    close_handler: &mut self.close_handler,
                };
                self.handler.on_metrics_changed(&mut ctx);
                window.request_redraw();
            }
            winit::event::WindowEvent::RedrawRequested => {
                self.draw();
            }
            winit::event::WindowEvent::CursorMoved { position, .. } => {
                let scale = self.state.scale;
                let logical = crate::platform::winit::pointer_position(position, scale);
                let event = InputEvent::PointerMoved(logical);
                let routed = self.input.route(event.clone(), &self.hit_regions);

                let icon = crate::resolve_cursor(logical, &self.hit_regions);
                if icon != self.cursor {
                    self.cursor = icon;
                    window.set_cursor(crate::platform::winit::cursor_icon(icon));
                }

                let mut ctx = AppWindowContext {
                    window,
                    state: &mut self.state,
                    input: &mut self.input,
                    close_handler: &mut self.close_handler,
                };
                if self.handler.on_input(&event, &mut ctx) || routed.target.is_some() {
                    window.request_redraw();
                }
            }
            winit::event::WindowEvent::CursorLeft { .. } => {
                self.input.hovered = None;
                let mut ctx = AppWindowContext {
                    window,
                    state: &mut self.state,
                    input: &mut self.input,
                    close_handler: &mut self.close_handler,
                };
                let event = InputEvent::PointerMoved(LogicalPoint::new(-10000.0, -10000.0));
                let _ = self.handler.on_input(&event, &mut ctx);
                window.request_redraw();
            }
            winit::event::WindowEvent::MouseInput { state, button, .. } => {
                let event = crate::platform::winit::pointer_button_event(state, button);
                let routed = self.input.route(event.clone(), &self.hit_regions);
                let mut ctx = AppWindowContext {
                    window,
                    state: &mut self.state,
                    input: &mut self.input,
                    close_handler: &mut self.close_handler,
                };
                if self.handler.on_input(&event, &mut ctx) || routed.target.is_some() {
                    window.request_redraw();
                }
            }
            winit::event::WindowEvent::MouseWheel { delta, .. } => {
                let event = crate::platform::winit::wheel_event(delta, self.state.scale);
                let routed = self.input.route(event.clone(), &self.hit_regions);
                let mut ctx = AppWindowContext {
                    window,
                    state: &mut self.state,
                    input: &mut self.input,
                    close_handler: &mut self.close_handler,
                };
                if self.handler.on_input(&event, &mut ctx) || routed.target.is_some() {
                    window.request_redraw();
                }
            }
            winit::event::WindowEvent::ModifiersChanged(modifiers) => {
                self.input.modifiers = crate::platform::winit::modifiers(modifiers.state());
            }
            winit::event::WindowEvent::KeyboardInput { event, .. } => {
                let mods = self.input.modifiers;
                if let Some(input_event) = crate::platform::winit::key_event(
                    &event.logical_key,
                    event.physical_key,
                    event.state,
                    mods,
                ) {
                    let routed = self.input.route(input_event.clone(), &self.hit_regions);
                    let mut ctx = AppWindowContext {
                        window,
                        state: &mut self.state,
                        input: &mut self.input,
                        close_handler: &mut self.close_handler,
                    };
                    if self.handler.on_input(&input_event, &mut ctx) || routed.target.is_some() {
                        window.request_redraw();
                    }
                }
            }
            winit::event::WindowEvent::Ime(ime) => {
                let event = crate::platform::winit::ime_event(ime);
                let routed = self.input.route(event.clone(), &self.hit_regions);
                let mut ctx = AppWindowContext {
                    window,
                    state: &mut self.state,
                    input: &mut self.input,
                    close_handler: &mut self.close_handler,
                };
                if self.handler.on_input(&event, &mut ctx) || routed.target.is_some() {
                    window.request_redraw();
                }
            }
            winit::event::WindowEvent::Focused(focused) => {
                self.state.set_focused(focused);
                let event = InputEvent::WindowFocused(focused);
                self.input.route(event.clone(), &[]);
                let mut ctx = AppWindowContext {
                    window,
                    state: &mut self.state,
                    input: &mut self.input,
                    close_handler: &mut self.close_handler,
                };
                let _ = self.handler.on_input(&event, &mut ctx);
                window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.exit_requested {
            event_loop.exit();
            return;
        }

        if let Some(deadline) = self.handler.next_deadline() {
            let now = Instant::now();
            if deadline <= now {
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            }
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_rgba_over_rgb_handles_alpha_extremes_and_blending() {
        let bg = [200, 200, 200];
        // 3 pixels: fully transparent, fully opaque green, 50% alpha white
        let rgba = [
            0, 0, 0, 0,
            0, 255, 0, 255,
            255, 255, 255, 128,
        ];
        let mut out = [0u32; 3];
        compose_rgba_over_rgb(&rgba, bg, &mut out);

        // Pixel 0: background untouched (200, 200, 200) -> 0x00C8C8C8
        assert_eq!(out[0], (200 << 16) | (200 << 8) | 200);

        // Pixel 1: opaque green (0, 255, 0) -> 0x0000FF00
        assert_eq!(out[1], (0 << 16) | (255 << 8) | 0);

        // Pixel 2: 50% blend of 255 and 200 -> approx 228
        let r = (out[2] >> 16) & 0xFF;
        let g = (out[2] >> 8) & 0xFF;
        let b = out[2] & 0xFF;
        assert_eq!(r, g);
        assert_eq!(g, b);
        assert!((226..=230).contains(&r));
    }
}
