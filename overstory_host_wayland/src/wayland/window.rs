// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Wayland windowing, `xdg-shell` integration, and `wgpu` surface creation.

#![expect(
    unsafe_code,
    reason = "wgpu surface creation from raw Wayland handles requires unsafe"
)]

use alloc::boxed::Box;
use alloc::string::ToString;
use core::ffi::c_void;
use core::ptr::NonNull;

use frameclock::scheduler::DegradationPolicy;
use frameclock::{
    Duration as FrameDuration, FrameDemand, FrameOpportunity, FrameSubmission, HostTime, OutputId,
    SchedulerConfig,
};
use frameclock_wayland::{
    Clock, PresentEvent, PresentEventQueue, SubmissionId, TickerState,
    presentation_time_to_host_time,
};
use overstory::{CursorIcon, Ui};
use overstory_host::{FrameViewport, UiHost};
use overstory_presenter_wgpu::{WgpuPresentResult, WgpuPresenter, WgpuPresenterDescriptor};
use rustix::event::{PollFd, PollFlags, Timespec};
use rustix::io::Errno;
use ui_events_wayland::seat::SeatReducer;
use wayland_client::protocol::{
    wl_callback, wl_compositor, wl_keyboard, wl_output, wl_pointer, wl_registry, wl_seat,
    wl_surface, wl_touch,
};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum};
use wayland_protocols::wp::cursor_shape::v1::client::{
    wp_cursor_shape_device_v1, wp_cursor_shape_manager_v1,
};
use wayland_protocols::wp::presentation_time::client::{wp_presentation, wp_presentation_feedback};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

use super::{HostResult, WaylandHostError, WindowOptions};

/// Stable application identifier reported to the compositor.
const APP_ID: &str = "overstory_host_wayland";

/// Display cadence assumed before the compositor reports a real refresh
/// interval (60 Hz, in host-time nanoseconds).
const FALLBACK_REFRESH_INTERVAL: FrameDuration = FrameDuration(16_666_667);

/// Submission identity stamped on every presentation-feedback event.
///
/// `frameclock_wayland` requires each [`PresentEvent`] to carry a
/// [`SubmissionId`] but never matches on it: feedback for one surface arrives in
/// submission order, so the FIFO [`PresentEventQueue`] already preserves the
/// only ordering the ticker needs. Per-commit identities would matter only when
/// multiplexing several surfaces on one queue, which this single-surface host
/// never does, so every event shares this one id.
const SUBMISSION_ID: SubmissionId = SubmissionId(0);

/// Single-threaded owner of every Wayland object and the GPU presenter.
///
/// One `App` value is the [`Dispatch`] target for all of the host's `wl_*`
/// objects, so each protocol callback receives `&mut App` and can update window
/// state directly. It also owns the UI host, the frame ticker, the clock that
/// stamps every frame tick, input event, and timer, and the seat reducer plus
/// per-device objects whose events it normalizes into `ui-events` for the host.
/// When the compositor offers `wp_presentation` it adopts that clock and folds
/// each frame's presentation feedback back into the ticker, sharpening pacing
/// from timing-blind to vsync-aligned. Input dispatch lives in the sibling
/// `input` module, so the fields it touches are visible to it.
pub(super) struct App {
    connection: Connection,
    compositor: Option<wl_compositor::WlCompositor>,
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    seat: Option<wl_seat::WlSeat>,
    output: Option<wl_output::WlOutput>,
    /// `wp_presentation` global, bound when the compositor offers it. Its clock
    /// and per-frame feedback are what lift pacing to vsync-aligned prediction;
    /// `None` keeps the host on monotonic, pacing-only timing.
    presentation: Option<wp_presentation::WpPresentation>,
    wl_surface: Option<wl_surface::WlSurface>,
    xdg_surface: Option<xdg_surface::XdgSurface>,
    xdg_toplevel: Option<xdg_toplevel::XdgToplevel>,
    configured: bool,
    closed: bool,
    logical_size: (u32, u32),
    scale: i32,
    pub(super) seat_reducer: SeatReducer,
    pub(super) pointer: Option<wl_pointer::WlPointer>,
    pub(super) keyboard: Option<wl_keyboard::WlKeyboard>,
    pub(super) touch: Option<wl_touch::WlTouch>,
    /// `cursor-shape-v1` global, bound when the compositor offers it. Without
    /// it no cursor is ever set — there is no themed-cursor
    /// (`wl_pointer.set_cursor`) fallback yet — so the pointer keeps whatever
    /// image it entered the surface with.
    pub(super) cursor_shape_manager: Option<wp_cursor_shape_manager_v1::WpCursorShapeManagerV1>,
    /// Shape device for the seat's pointer, created and destroyed with it as
    /// the seat's pointer capability comes and goes.
    pub(super) cursor_shape_device: Option<wp_cursor_shape_device_v1::WpCursorShapeDeviceV1>,
    /// The UI's current cursor intent, applied through `cursor-shape-v1` when
    /// it changes and re-applied on every pointer enter.
    pub(super) cursor_icon: CursorIcon,
    presenter: Option<WgpuPresenter>,
    pub(super) host: Option<UiHost>,
    ticker: TickerState,
    /// Clock every host time is read from. Starts at `CLOCK_MONOTONIC` and
    /// switches to the compositor's presentation clock once `wp_presentation`
    /// reports it, keeping ticks, input, timers, and feedback in one domain.
    clock: Clock,
    /// Presentation feedback awaiting folding into the ticker by
    /// [`App::drain_present_feedback`].
    present_events: PresentEventQueue,
}

impl App {
    /// Creates empty window state bound to `connection`.
    fn new(connection: Connection, logical_size: (u32, u32)) -> Self {
        Self {
            connection,
            compositor: None,
            wm_base: None,
            seat: None,
            output: None,
            presentation: None,
            wl_surface: None,
            xdg_surface: None,
            xdg_toplevel: None,
            configured: false,
            closed: false,
            logical_size,
            scale: 1,
            seat_reducer: SeatReducer::default(),
            pointer: None,
            keyboard: None,
            touch: None,
            cursor_shape_manager: None,
            cursor_shape_device: None,
            cursor_icon: CursorIcon::Default,
            presenter: None,
            host: None,
            ticker: TickerState::new(),
            clock: Clock::Monotonic,
            present_events: PresentEventQueue::default(),
        }
    }

    /// Returns the output scale as a positive integer.
    ///
    /// `scale` is stored in the protocol's `i32` form but is only ever written
    /// by the `wl_output` handler, which keeps it `>= 1`. Clamping here means the
    /// size and device-pixel-ratio readers below can treat it as a plain `u32`
    /// without each re-asserting that invariant.
    fn integer_scale(&self) -> u32 {
        u32::try_from(self.scale).unwrap_or(1).max(1)
    }

    /// Returns the physical surface size in device pixels.
    fn physical_size(&self) -> (u32, u32) {
        let scale = self.integer_scale();
        (
            self.logical_size.0.saturating_mul(scale),
            self.logical_size.1.saturating_mul(scale),
        )
    }

    /// Returns the integer device-pixel ratio applied to the UI.
    pub(super) fn scale_factor(&self) -> f64 {
        f64::from(self.integer_scale())
    }

    /// Reads the current host time from the active clock domain.
    ///
    /// Before the compositor reports a `wp_presentation` clock this is
    /// `CLOCK_MONOTONIC`; afterwards it is the clock the compositor stamps
    /// presentation feedback in. Every input event, timer, frame tick, and
    /// submission time is read from this one clock so the host's whole timeline
    /// stays in a single domain — the one feedback timestamps arrive in.
    pub(super) fn now(&self) -> HostTime {
        self.clock.now()
    }

    /// Renders the first host-driven frame and arms the frame-callback chain.
    ///
    /// A Wayland surface only becomes visible once a buffer is committed, and
    /// `wl_surface.frame` callbacks are delivered relative to commits. Drawing
    /// one real frame both maps the window and requests the first callback,
    /// after which completed callbacks drive every subsequent frame. This frame
    /// is pacing-only: nothing has been presented yet, so the ticker has no
    /// observed present time to predict the next vsync from.
    fn present_first_frame(&mut self, qh: &QueueHandle<Self>) -> HostResult<()> {
        let opportunity =
            FrameOpportunity::pacing_only(self.now(), FALLBACK_REFRESH_INTERVAL, OutputId(0));
        self.render(opportunity, qh)
    }

    /// Drains completed frame callbacks and renders any frame they make due.
    fn pump_frames(&mut self, qh: &QueueHandle<Self>) -> HostResult<()> {
        while let Some(tick) = self.ticker.poll_tick() {
            let opportunity = frameclock_wayland::frame_opportunity(
                tick,
                frameclock::DisplayTiming::fixed(FALLBACK_REFRESH_INTERVAL),
            );
            self.render(opportunity, qh)?;
        }
        Ok(())
    }

    /// Folds queued presentation feedback into the ticker's pacing facts.
    ///
    /// A `presented` report tells the ticker when a past frame actually reached
    /// the screen and the cadence between refreshes; once both are known the
    /// ticker predicts the next vsync, upgrading later frame hints from
    /// pacing-only to estimated. `discarded` reports carry no timing and so only
    /// clear the queue.
    fn drain_present_feedback(&mut self) {
        while let Some(event) = self.present_events.pop() {
            if let PresentEvent::Presented {
                actual_present,
                refresh_interval,
                ..
            } = event
            {
                self.ticker.set_last_observed_actual_present(actual_present);
                if let Some(interval) = refresh_interval {
                    self.ticker.set_last_observed_refresh_interval(interval);
                }
            }
        }
    }

    /// Prepares and presents one frame for `opportunity`, chaining the next
    /// callback.
    ///
    /// When the scheduler has no frame due, this returns without presenting.
    /// The event loop keeps callbacks coming while queued work or deferred
    /// feedback needs a tick. A frame that does present also asks
    /// `wp_presentation` (when bound) to report when it reaches the screen, so
    /// later frames pace against real scan-out times.
    fn render(&mut self, opportunity: FrameOpportunity, qh: &QueueHandle<Self>) -> HostResult<()> {
        let scale_factor = self.scale_factor();
        let (Some(host), Some(presenter), Some(wl_surface)) = (
            self.host.as_mut(),
            self.presenter.as_mut(),
            self.wl_surface.as_ref(),
        ) else {
            return Ok(());
        };

        let (width, height) = presenter.size();
        if width == 0 || height == 0 {
            return Ok(());
        }

        let viewport = FrameViewport::from_physical_size(width, height, scale_factor);
        let Some(prepared) = host.prepare_requested_frame(viewport, opportunity) else {
            return Ok(());
        };
        debug_assert!(
            !prepared.demand().is_empty(),
            "requested frames should carry non-empty planned demand"
        );

        // Request the next frame callback and a presentation-feedback object
        // before presenting so both ride the commit `present_scene` issues. The
        // callback is the frame-start wake; the feedback reports the actual
        // scan-out time that sharpens later predictions.
        if self.ticker.mark_callback_requested() {
            wl_surface.frame(qh, ());
        }
        if let Some(presentation) = self.presentation.as_ref() {
            let _feedback = presentation.feedback(wl_surface, qh, ());
        }

        match presenter.present_scene(prepared.scene()) {
            Ok(WgpuPresentResult::Presented(timings)) => {
                host.record_frame_submission(timings, FrameSubmission::deferred(self.clock.now()));
            }
            Ok(WgpuPresentResult::SurfaceReconfigured | WgpuPresentResult::Skipped) => {
                host.discard_prepared_frame();
            }
            Err(err) => {
                host.discard_prepared_frame();
                return Err(Box::new(err));
            }
        }

        self.connection.flush()?;
        Ok(())
    }

    /// Ensures a `wl_surface.frame` callback is in flight so an idle surface
    /// wakes to draw.
    ///
    /// Input, timers, and resize can make a frame due while the surface sits
    /// idle with no pending callback. Requesting one — and committing so the
    /// request reaches the compositor — schedules the next render without
    /// waiting on unrelated surface activity. When a callback is already
    /// pending the ticker reports so and this is a no-op, so it is safe to call
    /// after every state change that might want a frame.
    pub(super) fn ensure_frame_callback(&mut self, qh: &QueueHandle<Self>) {
        if let Some(wl_surface) = self.wl_surface.as_ref()
            && self.ticker.mark_callback_requested()
        {
            wl_surface.frame(qh, ());
            wl_surface.commit();
        }
    }

    /// Reconciles the GPU surface with the latest configured size and output
    /// scale, presenting a fresh frame whenever either changed.
    ///
    /// `xdg_toplevel` configure events update the logical window size and
    /// `wl_output` updates the integer scale; their product is the physical
    /// surface size. When that size moves the buffer scale is re-applied and
    /// the surface reconfigured, then one frame is drawn so the resized buffer
    /// and its scale reach the compositor in a single commit. Zero-area
    /// configurations are ignored.
    fn reconcile_surface_size(&mut self, qh: &QueueHandle<Self>) -> HostResult<()> {
        let (width, height) = self.physical_size();
        let Some(presenter) = self.presenter.as_mut() else {
            return Ok(());
        };
        if width == 0 || height == 0 || (width, height) == presenter.size() {
            return Ok(());
        }
        presenter.resize(width, height);

        // Re-apply the integer buffer scale as pending surface state; the
        // present below commits it together with the resized buffer so the
        // surface stays logically sized.
        if let Some(wl_surface) = self.wl_surface.as_ref() {
            wl_surface.set_buffer_scale(self.scale);
        }
        if let Some(host) = self.host.as_mut() {
            host.request_frame(FrameDemand::CONTINUOUS_INPUT);
        }

        // Draw at once so the resized buffer and its scale reach the compositor
        // together and the frame-callback chain is re-armed.
        let opportunity =
            FrameOpportunity::pacing_only(self.now(), FALLBACK_REFRESH_INTERVAL, OutputId(0));
        self.render(opportunity, qh)
    }

    /// Pumps UI timers due at `now`.
    ///
    /// The host promotes a due timer to animation demand itself, so the event
    /// loop requests a callback when that work needs a tick. `now` is sampled in
    /// `frameclock_wayland`'s monotonic
    /// domain, the same timeline that stamps input and frame ticks.
    fn pump_due_timers(&mut self, now: u64) {
        if let Some(host) = self.host.as_mut() {
            host.pump_timers(now);
        }
    }

    /// Returns the longest the loop may sleep before pending UI work is due.
    ///
    /// `None` lets the loop block until the Wayland socket is readable; `Some`
    /// bounds the wait so a queued UI timer or the next queued frame start fires
    /// on time. The host is consulted in its own monotonic clock domain.
    fn wakeup_timeout(&self, now: u64) -> Option<Timespec> {
        self.host
            .as_ref()
            .and_then(|host| host.duration_until_next_wakeup_at(now))
            .and_then(|wait| Timespec::try_from(wait).ok())
    }
}

/// Connects to the compositor, opens an `xdg-shell` window, creates a `wgpu`
/// surface, and drives the UI until the window is closed.
pub(super) fn run<F>(options: WindowOptions, build_ui: F) -> HostResult<()>
where
    F: FnOnce() -> Ui + 'static,
{
    let connection = Connection::connect_to_env()?;
    let mut event_queue: EventQueue<App> = connection.new_event_queue();
    let qh = event_queue.handle();
    let _registry = connection.display().get_registry(&qh, ());

    let mut app = App::new(connection.clone(), options.size());

    // Bind the compositor, xdg-shell, seat, and output globals.
    event_queue.roundtrip(&mut app)?;

    let compositor = app
        .compositor
        .clone()
        .ok_or(WaylandHostError::MissingGlobal("wl_compositor"))?;
    let wm_base = app
        .wm_base
        .clone()
        .ok_or(WaylandHostError::MissingGlobal("xdg_wm_base"))?;

    // Create the surface, xdg surface, and toplevel window.
    let wl_surface = compositor.create_surface(&qh, ());
    let xdg_surface = wm_base.get_xdg_surface(&wl_surface, &qh, ());
    let xdg_toplevel = xdg_surface.get_toplevel(&qh, ());
    xdg_toplevel.set_title(options.title().to_string());
    xdg_toplevel.set_app_id(APP_ID.to_string());
    wl_surface.commit();

    app.wl_surface = Some(wl_surface.clone());
    app.xdg_surface = Some(xdg_surface);
    app.xdg_toplevel = Some(xdg_toplevel);

    // Roundtrip until the compositor configures the surface.
    while !app.configured {
        event_queue.roundtrip(&mut app)?;
    }

    // Apply the integer buffer scale (defaults to 1 until an output reports it).
    wl_surface.set_buffer_scale(app.scale);
    wl_surface.commit();

    // Build a Vulkan-backed wgpu instance and a surface from the raw Wayland
    // handles, then hand the surface to the shared presenter.
    let mut instance_descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    instance_descriptor.backends = wgpu::Backends::VULKAN;
    let instance = wgpu::Instance::new(instance_descriptor);
    let surface = create_surface(&instance, &connection, &wl_surface)?;

    let (physical_width, physical_height) = app.physical_size();
    let presenter = WgpuPresenter::new(
        &instance,
        surface,
        physical_width,
        physical_height,
        WgpuPresenterDescriptor::new("overstory_host_wayland"),
    )?;
    app.presenter = Some(presenter);

    // Build the UI host now that the window and GPU surface exist, then ask for
    // an initial animation frame so the first callback has work to draw.
    let mut host = UiHost::with_scheduler_config(build_ui(), wayland_scheduler_config());
    host.request_frame(FrameDemand::ANIMATION);
    app.host = Some(host);

    // Draw the first frame to map the surface and arm the callback chain.
    app.present_first_frame(&qh)?;

    // Drive the window with a timer-aware poll loop. Each turn dispatches the
    // events already queued, reconciles a pending resize, renders any frame a
    // completed callback made due, fires due UI timers, then parks on the
    // Wayland socket until the next event arrives or the next UI wakeup falls
    // due — whichever comes first.
    while !app.closed {
        event_queue.dispatch_pending(&mut app)?;
        if app.closed {
            break;
        }

        // Fold any presentation feedback that arrived into the ticker so the
        // next frames pace against the compositor's real scan-out times.
        app.drain_present_feedback();
        app.reconcile_surface_size(&qh)?;
        app.pump_frames(&qh)?;

        let now = app.now().ticks();
        app.pump_due_timers(now);
        if app.host.as_ref().is_some_and(UiHost::needs_frame_tick) {
            app.ensure_frame_callback(&qh);
        }

        // Flush every request queued above — surface acks, a resize commit, a
        // woken frame callback — before parking on the socket.
        connection.flush()?;
        let Some(guard) = event_queue.prepare_read() else {
            // Events arrived between dispatching and preparing the read; loop
            // back to dispatch them.
            continue;
        };
        let timeout = app.wakeup_timeout(now);
        let mut fds = [PollFd::new(&connection, PollFlags::IN)];
        match rustix::event::poll(&mut fds, timeout.as_ref()) {
            // Socket readable: ingest the events for the next turn's dispatch.
            Ok(readable) if readable > 0 => {
                guard.read()?;
            }
            // Timed out before the wakeup, or interrupted by a signal: drop the
            // read intent and re-evaluate from the top.
            Ok(_) | Err(Errno::INTR) => drop(guard),
            Err(err) => return Err(Box::new(err)),
        }
    }

    Ok(())
}

/// Scheduler configuration for the frame-callback-driven Wayland host.
///
/// Use the adapter's estimated timing with the existing one-deep pipeline and
/// fallback build margin. Native callback hints release due frames at ticks;
/// cadence-held frames keep the callback chain running until they are ready.
fn wayland_scheduler_config() -> SchedulerConfig {
    let mut config = frameclock_wayland::scheduler_config();
    config.min_depth = 1;
    config.max_depth = 1;
    config.initial_depth = 1;
    config.degradation_policy = DegradationPolicy::Fixed;
    config.minimum_frame_start_margin = FALLBACK_REFRESH_INTERVAL;
    config
}

/// Creates a `wgpu` surface from the raw `wl_display` and `wl_surface` handles.
fn create_surface(
    instance: &wgpu::Instance,
    connection: &Connection,
    wl_surface: &wl_surface::WlSurface,
) -> HostResult<wgpu::Surface<'static>> {
    let display_ptr = connection.backend().display_ptr().cast::<c_void>();
    let surface_ptr = Proxy::id(wl_surface).as_ptr().cast::<c_void>();
    let target = wgpu::SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: Some(wgpu::rwh::RawDisplayHandle::Wayland(
            wgpu::rwh::WaylandDisplayHandle::new(
                NonNull::new(display_ptr).expect("wayland display pointer should be non-null"),
            ),
        )),
        raw_window_handle: wgpu::rwh::RawWindowHandle::Wayland(
            wgpu::rwh::WaylandWindowHandle::new(
                NonNull::new(surface_ptr).expect("wl_surface pointer should be non-null"),
            ),
        ),
    };
    // SAFETY: `display_ptr` and `surface_ptr` come from the live `Connection`
    // and `wl_surface`, which the caller keeps alive for at least as long as the
    // returned surface.
    let surface = unsafe { instance.create_surface_unsafe(target) }?;
    Ok(surface)
}

impl Dispatch<wl_registry::WlRegistry, ()> for App {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        else {
            return;
        };
        if interface.as_str() == wl_compositor::WlCompositor::interface().name {
            let compositor: wl_compositor::WlCompositor =
                registry.bind(name, version.min(6), qh, ());
            state.compositor = Some(compositor);
        } else if interface.as_str() == xdg_wm_base::XdgWmBase::interface().name {
            let wm_base: xdg_wm_base::XdgWmBase = registry.bind(name, version.min(6), qh, ());
            state.wm_base = Some(wm_base);
        } else if interface.as_str() == wl_seat::WlSeat::interface().name {
            let seat_version = version.min(7);
            let seat: wl_seat::WlSeat = registry.bind(name, seat_version, qh, ());
            // The seat's pointer inherits its version; tell the reducer so it
            // batches scroll axes by `frame` on pointers that support it.
            state.seat_reducer = SeatReducer::for_version(seat_version);
            state.seat = Some(seat);
        } else if interface.as_str() == wl_output::WlOutput::interface().name {
            let output: wl_output::WlOutput = registry.bind(name, version.min(4), qh, ());
            state.output = Some(output);
        } else if interface.as_str() == wp_presentation::WpPresentation::interface().name {
            let presentation: wp_presentation::WpPresentation =
                registry.bind(name, version.min(1), qh, ());
            state.presentation = Some(presentation);
        } else if interface.as_str()
            == wp_cursor_shape_manager_v1::WpCursorShapeManagerV1::interface().name
        {
            // Version 2 adds the non-CSS shapes (`dnd_ask`, `all_resize`);
            // shape mapping downgrades them on a version-1 device.
            let manager: wp_cursor_shape_manager_v1::WpCursorShapeManagerV1 =
                registry.bind(name, version.min(2), qh, ());
            state.cursor_shape_manager = Some(manager);
        }
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for App {
    fn event(
        _state: &mut Self,
        _proxy: &wl_compositor::WlCompositor,
        _event: wl_compositor::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // wl_compositor has no events.
    }
}

impl Dispatch<wl_surface::WlSurface, ()> for App {
    fn event(
        _state: &mut Self,
        _proxy: &wl_surface::WlSurface,
        _event: wl_surface::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Surface enter/leave and preferred-scale events are not handled yet.
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for App {
    fn event(
        _state: &mut Self,
        wm_base: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            wm_base.pong(serial);
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for App {
    fn event(
        state: &mut Self,
        surface: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            surface.ack_configure(serial);
            state.configured = true;
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for App {
    fn event(
        state: &mut Self,
        _toplevel: &xdg_toplevel::XdgToplevel,
        event: xdg_toplevel::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            xdg_toplevel::Event::Configure { width, height, .. } => {
                if let (Ok(width), Ok(height)) = (u32::try_from(width), u32::try_from(height))
                    && width != 0
                    && height != 0
                {
                    state.logical_size = (width, height);
                }
            }
            xdg_toplevel::Event::Close => {
                state.closed = true;
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_output::WlOutput, ()> for App {
    fn event(
        state: &mut Self,
        _output: &wl_output::WlOutput,
        event: wl_output::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Scale { factor } = event
            && factor >= 1
        {
            state.scale = factor;
        }
    }
}

impl Dispatch<wl_callback::WlCallback, ()> for App {
    fn event(
        state: &mut Self,
        _callback: &wl_callback::WlCallback,
        event: wl_callback::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // A completed frame callback queues one tick; `pump_frames` drains it
        // after dispatch returns.
        if let wl_callback::Event::Done { .. } = event {
            state.ticker.on_callback_done(state.clock, OutputId(0));
        }
    }
}

impl Dispatch<wp_presentation::WpPresentation, ()> for App {
    fn event(
        state: &mut Self,
        _presentation: &wp_presentation::WpPresentation,
        event: wp_presentation::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // The compositor names the clock its presentation timestamps use exactly
        // once. Adopt it so frame ticks, input, timers, and feedback all read one
        // clock; an unrecognized id leaves the monotonic fallback in place.
        if let wp_presentation::Event::ClockId { clk_id } = event
            && let Some(clock) = Clock::from_presentation_clock_id(clk_id)
        {
            state.clock = clock;
        }
    }
}

impl Dispatch<wp_presentation_feedback::WpPresentationFeedback, ()> for App {
    fn event(
        state: &mut Self,
        _feedback: &wp_presentation_feedback::WpPresentationFeedback,
        event: wp_presentation_feedback::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Each feedback object delivers exactly one terminal event; record it so
        // `drain_present_feedback` can fold its timing into the ticker.
        match event {
            wp_presentation_feedback::Event::Presented {
                tv_sec_hi,
                tv_sec_lo,
                tv_nsec,
                refresh,
                flags,
                ..
            } => {
                let actual_present = presentation_time_to_host_time(tv_sec_hi, tv_sec_lo, tv_nsec);
                let refresh_interval = if refresh == 0 {
                    None
                } else {
                    Some(FrameDuration(u64::from(refresh)))
                };
                let flags = match flags {
                    WEnum::Value(kind) => kind.bits(),
                    WEnum::Unknown(bits) => bits,
                };
                state.present_events.push(PresentEvent::Presented {
                    id: SUBMISSION_ID,
                    actual_present,
                    refresh_interval,
                    output: None,
                    flags,
                });
            }
            wp_presentation_feedback::Event::Discarded => {
                state
                    .present_events
                    .push(PresentEvent::Discarded { id: SUBMISSION_ID });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FALLBACK_REFRESH_INTERVAL, wayland_scheduler_config};
    use frameclock::scheduler::DegradationPolicy;

    #[test]
    fn wayland_scheduler_uses_frame_callback_as_frame_start_wake() {
        let config = wayland_scheduler_config();

        assert_eq!(config.minimum_frame_start_margin, FALLBACK_REFRESH_INTERVAL);
        assert_eq!(config.min_depth, 1);
        assert_eq!(config.max_depth, 1);
        assert_eq!(config.initial_depth, 1);
        assert_eq!(config.degradation_policy, DegradationPolicy::Fixed);
    }
}
