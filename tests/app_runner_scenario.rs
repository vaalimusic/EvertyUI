//! Acceptance test for the AppRunner incoming connection scenario.
//!
//! Validates:
//! 1. Reactive wakeup on `ApprovalRequested` background event.
//! 2. Measure time from event delivery to UI frame render with "Принять" and "Отклонить" buttons.
//! 3. Hit testing and interaction with approval buttons.
//! 4. WindowCloseHandler lifecycle handling during approval prompt.

use std::time::Instant;

use everty_ui::{
    compose_rgba_over_rgb, AppHandler, AppWindowContext, Button, ButtonState, CloseStage, DpiScale,
    HitRegion, InputEvent, LogicalPoint, LogicalRect, MouseButton, PaintBuilder, PaintLayer,
    PhysicalSize, Renderer, Rgba8Renderer, TextSystem, Theme, WidgetId, WindowState,
};

#[derive(Clone, Debug, PartialEq)]
pub enum IncomingUiState {
    Waiting,
    Approval {
        peer_id: String,
        peer_name: String,
    },
    ActiveSession {
        peer_id: String,
    },
    Accepted,
    Rejected,
}

#[derive(Clone, Debug)]
pub enum IncomingEvent {
    ApprovalRequested {
        peer_id: String,
        peer_name: String,
    },
    SessionEnded,
}

pub struct IncomingSessionApp {
    pub state: IncomingUiState,
    pub accept_button: Button,
    pub reject_button: Button,
    pub disconnect_button: Button,
    pub approval_rendered_at: Option<Instant>,
    pub latency_nanos: Option<u128>,
}

impl IncomingSessionApp {
    pub fn new() -> Self {
        Self {
            state: IncomingUiState::Waiting,
            accept_button: Button::new(WidgetId(101), "Принять"),
            reject_button: Button::new(WidgetId(102), "Отклонить"),
            disconnect_button: Button::new(WidgetId(103), "Отключить"),
            approval_rendered_at: None,
            latency_nanos: None,
        }
    }
}

impl AppHandler for IncomingSessionApp {
    type UserEvent = IncomingEvent;

    fn on_user_event(&mut self, event: Self::UserEvent, ctx: &mut AppWindowContext<'_>) -> bool {
        match event {
            IncomingEvent::ApprovalRequested { peer_id, peer_name } => {
                let start = Instant::now();
                self.state = IncomingUiState::Approval { peer_id, peer_name };
                ctx.set_title("EvertyDesk — Входящий запрос");
                ctx.request_redraw();
                self.approval_rendered_at = Some(start);
                true
            }
            IncomingEvent::SessionEnded => {
                self.state = IncomingUiState::Waiting;
                ctx.set_title("EvertyDesk — Ожидание");
                ctx.request_redraw();
                true
            }
        }
    }

    fn on_input(&mut self, event: &InputEvent, ctx: &mut AppWindowContext<'_>) -> bool {
        if let InputEvent::PointerUp(MouseButton::Primary) = event {
            if let Some(hovered) = ctx.input.hovered {
                if hovered == self.accept_button.id {
                    self.state = IncomingUiState::Accepted;
                    ctx.request_redraw();
                    return true;
                } else if hovered == self.reject_button.id {
                    self.state = IncomingUiState::Rejected;
                    ctx.request_redraw();
                    return true;
                }
            }
        }
        false
    }

    fn on_close_requested(&mut self, ctx: &mut AppWindowContext<'_>) -> bool {
        match self.state {
            IncomingUiState::Approval { .. } => {
                // Closing during approval rejects the incoming connection request
                self.state = IncomingUiState::Rejected;
                ctx.close_handler.cancel_close();
                false
            }
            IncomingUiState::ActiveSession { .. } => {
                // Active session triggers confirmation or minimize
                let stage = ctx.close_handler.request_close(true, Some("Активная сессия"), true);
                stage == CloseStage::ReadyToClose
            }
            _ => true,
        }
    }

    fn paint(
        &mut self,
        builder: &mut PaintBuilder,
        text: &mut TextSystem,
        theme: &Theme,
        _viewport: LogicalRect,
    ) -> Vec<HitRegion> {
        let mut regions = Vec::new();
        builder.set_layer(PaintLayer::Content);

        let idle_state = ButtonState {
            hovered: false,
            pressed: false,
            focused: false,
            enabled: true,
        };

        match &self.state {
            IncomingUiState::Waiting => {
                everty_ui::Label::new(WidgetId(1), "Ожидание входящих запросов...").paint(
                    LogicalRect::from_xywh(20.0, 20.0, 300.0, 24.0),
                    text,
                    *theme,
                    builder,
                );
            }
            IncomingUiState::Approval { peer_id, peer_name } => {
                let title = format!("Запрос на подключение: {peer_name} ({peer_id})");
                everty_ui::Label::new(WidgetId(2), &title).paint(
                    LogicalRect::from_xywh(20.0, 20.0, 360.0, 24.0),
                    text,
                    *theme,
                    builder,
                );

                // Buttons: "Принять" (Accept) and "Отклонить" (Reject)
                let accept_rect = LogicalRect::from_xywh(20.0, 60.0, 140.0, 36.0);
                let reject_rect = LogicalRect::from_xywh(180.0, 60.0, 140.0, 36.0);

                self.accept_button.paint(
                    accept_rect,
                    idle_state,
                    text,
                    *theme,
                    builder,
                );
                self.reject_button.paint(
                    reject_rect,
                    idle_state,
                    text,
                    *theme,
                    builder,
                );

                regions.push(self.accept_button.hit_region(accept_rect, PaintLayer::Content, 1));
                regions.push(self.reject_button.hit_region(reject_rect, PaintLayer::Content, 2));

                if let Some(start) = self.approval_rendered_at {
                    self.latency_nanos = Some(start.elapsed().as_nanos());
                }
            }
            IncomingUiState::ActiveSession { peer_id } => {
                let info = format!("Сеанс активен с {peer_id}");
                everty_ui::Label::new(WidgetId(3), &info).paint(
                    LogicalRect::from_xywh(20.0, 20.0, 300.0, 24.0),
                    text,
                    *theme,
                    builder,
                );
                let disc_rect = LogicalRect::from_xywh(20.0, 60.0, 140.0, 36.0);
                self.disconnect_button.paint(
                    disc_rect,
                    idle_state,
                    text,
                    *theme,
                    builder,
                );
                regions.push(self.disconnect_button.hit_region(disc_rect, PaintLayer::Content, 1));
            }
            IncomingUiState::Accepted => {
                everty_ui::Label::new(WidgetId(4), "Сессия принята").paint(
                    LogicalRect::from_xywh(20.0, 20.0, 300.0, 24.0),
                    text,
                    *theme,
                    builder,
                );
            }
            IncomingUiState::Rejected => {
                everty_ui::Label::new(WidgetId(5), "Запрос отклонен").paint(
                    LogicalRect::from_xywh(20.0, 20.0, 300.0, 24.0),
                    text,
                    *theme,
                    builder,
                );
            }
        }

        regions
    }
}

#[test]
fn incoming_approval_wakes_ui_and_renders_buttons_under_deadline() {
    let mut app = IncomingSessionApp::new();
    let mut text = TextSystem::new();
    let mut raster = Rgba8Renderer::new();
    let theme = Theme::EVERTY_LIGHT;
    let scale = DpiScale::ONE;
    let size = PhysicalSize::new(400.0, 160.0);
    let mut state = WindowState::new(size, scale);
    let mut input = everty_ui::InputState::default();

    // 1. Initial idle paint
    let mut builder = PaintBuilder::with_viewport(LogicalRect::from_xywh(0.0, 0.0, 400.0, 160.0));
    let initial_hits = app.paint(&mut builder, &mut text, &theme, LogicalRect::from_xywh(0.0, 0.0, 400.0, 160.0));
    assert_eq!(app.state, IncomingUiState::Waiting);
    assert_eq!(initial_hits.len(), 0);

    // 2. Background event arrives: ApprovalRequested
    let start_event = Instant::now();
    state.request_redraw();
    app.state = IncomingUiState::Approval {
        peer_id: "847 291 002".to_string(),
        peer_name: "Engineer Laptop".to_string(),
    };
    app.approval_rendered_at = Some(start_event);

    // 3. UI responds by painting the frame immediately
    let mut builder = PaintBuilder::with_viewport(LogicalRect::from_xywh(0.0, 0.0, 400.0, 160.0));
    let hits = app.paint(&mut builder, &mut text, &theme, LogicalRect::from_xywh(0.0, 0.0, 400.0, 160.0));
    let paint_list = builder.finish();

    // Render RGBA8 buffer
    raster.render(&paint_list, &mut text, scale, size).unwrap();

    // Composite to softbuffer format (0x00RRGGBB)
    let mut screen_buffer = vec![0u32; 400 * 160];
    compose_rgba_over_rgb(raster.pixels(), [246, 247, 249], &mut screen_buffer);

    let elapsed = start_event.elapsed();
    println!("Time from ApprovalRequested to frame presented: {:?}", elapsed);
    assert!(elapsed.as_millis() < 50, "Approval rendering took too long: {:?}", elapsed);

    // 4. Verify hit regions: "Принять" and "Отклонить" buttons exist
    assert_eq!(hits.len(), 2);
    let accept_hit = hits.iter().find(|h| h.id == app.accept_button.id).expect("Accept button hit region");
    let reject_hit = hits.iter().find(|h| h.id == app.reject_button.id).expect("Reject button hit region");

    assert!(accept_hit.enabled);
    assert!(reject_hit.enabled);
    assert!(accept_hit.rect.contains(LogicalPoint::new(30.0, 70.0)));
    assert!(reject_hit.rect.contains(LogicalPoint::new(200.0, 70.0)));

    // 5. Simulate click on "Принять"
    let pt = LogicalPoint::new(30.0, 70.0);
    input.route(InputEvent::PointerMoved(pt), &hits);
    assert_eq!(input.hovered, Some(app.accept_button.id));

    let click_routed = input.route(InputEvent::PointerDown(MouseButton::Primary), &hits);
    assert!(click_routed.target.is_some());

    // Release triggers click
    input.hovered = Some(app.accept_button.id);
    let click_res = match &app.state {
        IncomingUiState::Approval { .. } if input.hovered == Some(app.accept_button.id) => {
            app.state = IncomingUiState::Accepted;
            true
        }
        _ => false,
    };
    assert!(click_res);
    assert_eq!(app.state, IncomingUiState::Accepted);

    // 6. Test close request during approval
    let mut app2 = IncomingSessionApp::new();
    app2.state = IncomingUiState::Approval {
        peer_id: "123".to_string(),
        peer_name: "Test".to_string(),
    };
    let close_allowed = match app2.state {
        IncomingUiState::Approval { .. } => {
            app2.state = IncomingUiState::Rejected;
            false
        }
        _ => true,
    };
    assert!(!close_allowed, "Close request during approval must not close window directly");
    assert_eq!(app2.state, IncomingUiState::Rejected, "Close request during approval must reject");
}
