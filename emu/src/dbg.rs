use super::gfx::{GfxBufferMutLE, Rgb888};
use super::hw::glutils::Texture;

use imgui::*;
use imgui_opengl_renderer::Renderer;
use imgui_sdl2::ImguiSdl2;
use sdl2::keyboard::Scancode;
mod uisupport;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

// Views
mod regview;
pub use self::regview::*;
mod disasmview;
pub use self::disasmview::*;
mod decoding;
pub use self::decoding::*;
mod tracer;
pub use self::tracer::*;

pub trait DebuggerModel {
    /// Run a frame with a tracer (debugger).
    ///
    /// The function is expected to respect trace API and call the trait methods at
    /// the correct moments, propagating any error (TraceEvent) generated by them
    /// Failure to do so may impede correct debugger functionality (eg: not calling
    /// trace_cpu() at every emulated frame may cause a breakpoint to be missed.
    ///
    /// After a TraceEvent is returned and processed by the debugger, emulation of the
    /// frame will be resumed by calling run_frame with the same screen buffer.
    fn trace_frame<T: Tracer>(
        &mut self,
        screen: &mut GfxBufferMutLE<Rgb888>,
        tracer: &T,
    ) -> Result<()>;

    fn render_debug<'a, 'ui>(&mut self, dr: &DebuggerRenderer<'a, 'ui>);
}

pub struct DebuggerUI {
    imgui: Rc<RefCell<ImGui>>,
    imgui_sdl2: ImguiSdl2,
    backend: Renderer,
    hidpi_factor: f32,
    tex_screen: Texture,

    pub dbg: Debugger,
    paused: bool,
    last_render: Instant, // last instant the debugger refreshed its UI
}

impl DebuggerUI {
    pub(crate) fn new(video: sdl2::VideoSubsystem) -> Self {
        let hidpi_factor = 1.0;

        let mut imgui = ImGui::init();
        imgui.set_ini_filename(Some(im_str!("debug.ini").to_owned()));

        let imgui_sdl2 = ImguiSdl2::new(&mut imgui);
        let backend = Renderer::new(&mut imgui, move |s| video.gl_get_proc_address(s) as _);

        Self {
            imgui: Rc::new(RefCell::new(imgui)),
            imgui_sdl2,
            backend,
            hidpi_factor,
            tex_screen: Texture::new(),
            dbg: Debugger::new(),
            paused: false,
            last_render: Instant::now(),
        }
    }

    pub(crate) fn handle_event(&mut self, event: &sdl2::event::Event) {
        let imgui = self.imgui.clone();
        let mut imgui = imgui.borrow_mut();
        self.imgui_sdl2.handle_event(&mut imgui, &event);
    }

    /// Run an emulator (DebuggerModel) under the debugger for a little while.
    /// Returns true if during this call the emulator completed a frame, or false otherwise.
    pub(crate) fn trace<T: DebuggerModel>(
        &mut self,
        producer: &mut T,
        screen: &mut GfxBufferMutLE<Rgb888>,
    ) -> bool {
        // If the emulation core is paused, we can simply wait here to avoid hogging CPU.
        // Refresh every 16ms / 60FPS.
        if self.paused {
            match Duration::from_millis(16).checked_sub(self.last_render.elapsed()) {
                Some(d) => std::thread::sleep(d),
                None => {}
            }
            return false;
        }

        // Request a Poll event after 50ms to keep the debugger at least at 20 FPS during emulation.
        let trace_until = self.last_render + Duration::from_millis(50);
        self.dbg.set_poll_event(trace_until);
        loop {
            match producer.trace_frame(screen, &self.dbg) {
                Ok(()) => {
                    // A frame is finished. Copy it into the texture so that it's available
                    // starting from next render().
                    self.tex_screen.copy_from_buffer_mut(screen);
                    return true;
                }
                Err(event) => match *event {
                    TraceEvent::Poll() => return false, // Polling
                    _ => unimplemented!(),
                },
            };
        }
    }

    /// Render the current debugger UI.
    pub(crate) fn render<T: DebuggerModel>(
        &mut self,
        window: &sdl2::video::Window,
        event_pump: &sdl2::EventPump,
        model: &mut T,
    ) {
        let imgui = self.imgui.clone();
        let mut imgui = imgui.borrow_mut();

        // Global key shortcuts
        if imgui.is_key_pressed(Scancode::Space as _) {
            self.paused = !self.paused;
        }

        let ui = self.imgui_sdl2.frame(&window, &mut imgui, &event_pump);

        self.render_main(&ui);
        ui.show_demo_window(&mut true);

        {
            let dr = DebuggerRenderer { ui: &ui };
            model.render_debug(&dr);
        }

        // Actually flush commands batched in imgui to OpenGL
        unsafe {
            gl::ClearColor(0.45, 0.55, 0.60, 0.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }

        self.backend.render(ui);
        self.last_render = Instant::now();
    }

    fn render_main<'ui>(&mut self, ui: &Ui<'ui>) {
        ui.main_menu_bar(|| {
            ui.menu(im_str!("Emulation")).build(|| {
                ui.menu_item(im_str!("Reset")).build();
            })
        });

        ui.window(im_str!("Screen"))
            .size((320.0, 240.0), ImGuiCond::FirstUseEver)
            .build(|| {
                let tsid = self.tex_screen.id();
                let reg = ui.get_content_region_avail();
                let image = Image::new(ui, tsid.into(), reg);
                image.build();
            });

        self.dbg.render_main(ui);
    }
}

pub struct DebuggerRenderer<'a, 'ui> {
    ui: &'a Ui<'ui>,
}

impl<'a, 'ui> DebuggerRenderer<'a, 'ui> {
    pub fn render_regview<V: RegisterView>(&self, v: &mut V) {
        render_regview(self.ui, v)
    }
    pub fn render_disasmview<V: DisasmView>(&self, v: &mut V) {
        render_disasmview(self.ui, v)
    }
}
