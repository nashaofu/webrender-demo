extern crate env_logger;

use gleam::gl::{GlFns, GlesFns};
use glutin::{
    event,
    event_loop::{self},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
    Api, ContextBuilder,
};
use webrender::{
    api::{
        units::{DeviceIntSize, LayoutRect},
        ColorF, CommonItemProperties, DisplayListBuilder, DocumentId, Epoch, PipelineId,
        RenderNotifier, RenderReasons, SpaceAndClipInfo,
    },
    euclid::{Point2D, Scale},
    RenderApi, Renderer, RendererOptions, Transaction,
};

struct Notifier {
    events_proxy: event_loop::EventLoopProxy<()>,
}

impl Notifier {
    fn new(events_proxy: event_loop::EventLoopProxy<()>) -> Notifier {
        Notifier { events_proxy }
    }
}

impl RenderNotifier for Notifier {
    fn clone(&self) -> Box<dyn RenderNotifier> {
        Box::new(Notifier {
            events_proxy: self.events_proxy.clone(),
        })
    }

    fn wake_up(&self, _composite_needed: bool) {
        #[cfg(not(target_os = "android"))]
        let _ = self.events_proxy.send_event(());
    }

    fn new_frame_ready(&self, _: DocumentId, _scrolled: bool, composite_needed: bool) {
        self.wake_up(composite_needed);
    }
}

pub fn main() {
    env_logger::init();

    let mut events_loop = event_loop::EventLoop::new();
    let window_builder = WindowBuilder::new()
        .with_visible(false)
        .with_transparent(true);

    let context = ContextBuilder::new()
        .build_windowed(window_builder, &events_loop)
        .unwrap();

    let windowed_context = unsafe { context.make_current().unwrap() };

    let notifier = Box::new(Notifier::new(events_loop.create_proxy()));

    let gl = match windowed_context.get_api() {
        Api::OpenGl => unsafe {
            GlFns::load_with(|symbol| windowed_context.get_proc_address(symbol))
        },
        Api::OpenGlEs => unsafe {
            GlesFns::load_with(|symbol| windowed_context.get_proc_address(symbol))
        },
        Api::WebGl => unimplemented!(),
    };

    let (mut renderer, sender) =
        Renderer::new(gl.clone(), notifier, RendererOptions::default(), None).unwrap();

    let device_size = {
        let size = windowed_context.window().inner_size();
        DeviceIntSize::new(size.width as i32, size.height as i32)
    };

    let pipeline_id = PipelineId(0, 0);
    let mut builder = DisplayListBuilder::new(pipeline_id);
    let mut txn = Transaction::new();
    let epoch = Epoch(0);

    builder.begin();

    let mut api = sender.create_api();
    let document_id = api.add_document(device_size);

    render(
        &mut api,
        &mut builder,
        &mut txn,
        device_size,
        pipeline_id,
        document_id,
    );

    let device_pixel_ratio = windowed_context.window().scale_factor();
    let layout_size = device_size.to_f32() / Scale::new(device_pixel_ratio as f32);

    txn.set_display_list(
        epoch,
        Some(ColorF::new(1.0, 0.0, 0.0, 1.0)),
        layout_size,
        builder.end(),
    );
    txn.set_root_pipeline(pipeline_id);
    txn.generate_frame(0, RenderReasons::empty());
    api.send_transaction(document_id, txn);

    events_loop.run_return(|global_event, _, control_flow| {
        *control_flow = event_loop::ControlFlow::Wait;
        let window = windowed_context.window();
        let txn = Transaction::new();

        match global_event {
            event::Event::WindowEvent { event, .. } => match event {
                event::WindowEvent::CloseRequested => control_flow.set_exit(),
                event::WindowEvent::KeyboardInput { input, .. } => {
                    if event::VirtualKeyCode::Escape == input.virtual_keycode.unwrap() {
                        control_flow.set_exit()
                    }
                }
                _ => (),
            },
            event::Event::Resumed => {
                window.set_visible(true);
                window.focus_window();
            }
            _ => (),
        }

        api.send_transaction(document_id, txn);
        renderer.update();
        renderer.render(device_size, 0).unwrap();
        let _ = renderer.flush_pipeline_info();
        windowed_context.swap_buffers().unwrap();
    });
    renderer.deinit();
}

fn render(
    _api: &mut RenderApi,
    builder: &mut DisplayListBuilder,
    _txn: &mut Transaction,
    device_size: DeviceIntSize,
    pipeline_id: PipelineId,
    _document_id: DocumentId,
) {
    let width = device_size.width as f32;
    let height = device_size.height as f32;

    let bounds = LayoutRect::new(Point2D::zero(), Point2D::new(width * 0.5, height * 0.5));

    builder.push_rect(
        &CommonItemProperties::new(bounds, SpaceAndClipInfo::root_scroll(pipeline_id)),
        bounds,
        ColorF::new(1.0, 0.0, 0.0, 1.0),
    );

    let bounds = LayoutRect::new(
        Point2D::new(width * 0.25, height * 0.25),
        Point2D::new(width * 0.75, height * 0.75),
    );

    builder.push_rect(
        &CommonItemProperties::new(bounds, SpaceAndClipInfo::root_scroll(pipeline_id)),
        bounds,
        ColorF::new(0.0, 1.0, 0.0, 1.0),
    );

    let bounds = LayoutRect::new(
        Point2D::new(width * 0.5, height * 0.5),
        Point2D::new(width, height),
    );

    builder.push_rect(
        &CommonItemProperties::new(bounds, SpaceAndClipInfo::root_scroll(pipeline_id)),
        bounds,
        ColorF::new(0.0, 0.0, 1.0, 1.0),
    );
}
