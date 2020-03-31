use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
    dpi::LogicalSize,
};

mod scene;
use scene::Scene;
mod icosphere;

async fn get_device_and_queue() -> (wgpu::Device, wgpu::Queue) {
    let adapter = wgpu::Adapter::request(
        &wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
        },
        wgpu::BackendBit::PRIMARY,
    ).await.unwrap();

    adapter.request_device(&wgpu::DeviceDescriptor {
        extensions: wgpu::Extensions {
            anisotropic_filtering: false,
        },
        limits: wgpu::Limits::default(),
    }).await
}

fn main() {
    let event_loop = EventLoop::new();

    let window = Window::new(&event_loop).unwrap();

    let size = window.inner_size();
    let surface = wgpu::Surface::create(&window);

    let (device, queue) = futures::executor::block_on(get_device_and_queue());

    let mut swapchain_desc = wgpu::SwapChainDescriptor {
        usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::Mailbox,
    };

    let mut swapchain = device.create_swap_chain(&surface, &swapchain_desc);

    let (scene, scene_command_buffer) = Scene::new(&device, &swapchain_desc);

    queue.submit(&[scene_command_buffer]);

    let mut resized = false;
    let mut logical_size: LogicalSize<f32> = window.inner_size().to_logical(window.scale_factor());

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll; // TODO: change this to `Poll`.

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::Resized(new_size) => {
                    logical_size = new_size.to_logical(window.scale_factor());
                    resized = true
                }
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                _ => {},
            },
            Event::MainEventsCleared => window.request_redraw(),
            Event::RedrawRequested(_) => {
                if resized {
                    resized = false;
                    let new_size = window.inner_size();
                    swapchain_desc.width = new_size.width;
                    swapchain_desc.height = new_size.height;

                    swapchain = device.create_swap_chain(&surface, &swapchain_desc)
                }

                let frame = swapchain.get_next_texture()
                    .expect("timeout when acquiring next swapchain texture");

                let mut encoder = device.create_command_encoder(&Default::default());

                // Draw the scene first.
                scene.draw(&mut encoder, &frame.view);

                // Finally, submit everything to the GPU to draw!
                queue.submit(&[encoder.finish()]);
            }
            _ => {}
        }
    })
}
