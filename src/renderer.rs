// pathfinder/demo/common/src/renderer.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Rendering functionality for the demo.

use super::window::{View, Window};
use super::{DemoApp, UIVisibility};
use image::ColorType;
use pathfinder_color::ColorU;
use pathfinder_geometry::rect::RectI;
use pathfinder_geometry::vector::Vector2I;
use pathfinder_gpu::{TextureData, Device, RenderTarget};
use pathfinder_renderer::gpu::options::{DestFramebuffer, RendererOptions};
use std::path::PathBuf;

const GROUND_SOLID_COLOR: ColorU = ColorU {
    r: 80,
    g: 80,
    b: 80,
    a: 255,
};

const GROUND_LINE_COLOR: ColorU = ColorU {
    r: 127,
    g: 127,
    b: 127,
    a: 255,
};

const GRIDLINE_COUNT: i32 = 10;

impl<W> DemoApp<W>
where
    W: Window,
{
    pub fn prepare_frame_rendering(&mut self) -> u32 {
        // Make the context current.
        let view = self.ui_model.mode.view(0);
        self.window.make_current(view);

        // Clear to the appropriate color.
        let mode = self.camera.mode();
        let clear_color = Some(self.ui_model.background_color().to_f32());

        // Set up framebuffers.
        let window_size = self.window_size.device_size();
        let scene_count = {
            *self.renderer.options_mut() = RendererOptions {
                dest: DestFramebuffer::Default {
                    viewport: self.window.viewport(View::Mono),
                    window_size,
                },
                background_color: clear_color,
                show_debug_ui: self.options.ui != UIVisibility::None,
            };
            1
        };
        scene_count
    }

    pub fn draw_scene(&mut self) {
        self.renderer.device().begin_commands();

        let view = self.ui_model.mode.view(0);
        self.window.make_current(view);

        self.draw_environment(0);

        self.renderer.device().end_commands();

        self.render_vector_scene();
    }

    pub fn begin_compositing(&mut self) {
        self.renderer.device().begin_commands();
    }

    #[allow(deprecated)]
    pub fn composite_scene(&mut self, render_scene_index: u32) {}

    // Draws the ground, if applicable.
    fn draw_environment(&self, render_scene_index: u32) {
        return;
    }

    #[allow(deprecated)]
    fn render_vector_scene(&mut self) {
        self.renderer.disable_depth();

        // Issue render commands!
        self.scene_proxy.render(&mut self.renderer);
    }

    pub fn take_raster_screenshot(&mut self, path: PathBuf) {
        let drawable_size = self.window_size.device_size();
        let viewport = RectI::new(Vector2I::default(), drawable_size);
        let texture_data_receiver = self
            .renderer
            .device()
            .read_pixels(&RenderTarget::Default, viewport);
        let pixels = match self
            .renderer
            .device()
            .recv_texture_data(&texture_data_receiver)
        {
            TextureData::U8(pixels) => pixels,
            _ => panic!("Unexpected pixel format for default framebuffer!"),
        };
        image::save_buffer(
            path,
            &pixels,
            drawable_size.x() as u32,
            drawable_size.y() as u32,
            ColorType::Rgba8,
        )
        .unwrap();
    }
}
