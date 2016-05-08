//
// This file is part of tak-rs.
//
// tak-rs is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// tak-rs is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with tak-rs. If not, see <http://www.gnu.org/licenses/>.
//
// Copyright 2016 Chris Foster
//

use sdl2;
use sdl2::pixels::Color as SdlColor;
use sdl2::rect::Rect as SdlRectangle;
use sdl2_gfx::primitives::DrawRenderer;

use tak::{Board, Color, Piece};

pub struct RenderContext {
    sdl_renderer: sdl2::render::Renderer<'static>,
}

pub fn initialize(sdl_context: &sdl2::Sdl, (width, height): (u32, u32), fullscreen: bool) -> RenderContext {
    let sdl_renderer = {
        let mut builder = sdl_context.video().unwrap().window("The Reaping", width, height);

        builder.opengl();

        if fullscreen { builder.fullscreen(); }

        let sdl_window = builder.build().unwrap();
        let mut sdl_renderer = sdl_window.renderer().accelerated().build().unwrap();
        sdl_renderer.set_logical_size(width, height).ok();
        sdl_renderer
    };

    RenderContext {
        sdl_renderer: sdl_renderer,
    }
}

pub fn clear(context: &mut RenderContext) {
    context.sdl_renderer.set_draw_color(SdlColor::RGB(60, 60, 60));
    context.sdl_renderer.clear();
}

pub fn present(context: &mut RenderContext) {
    context.sdl_renderer.present();
}

pub trait Renderable {
    fn render(&self, context: &mut RenderContext);
}

const TILE: SdlColor = SdlColor::RGB(224, 209, 155);
const ROAD_WHITE: SdlColor = SdlColor::RGB(244, 234, 201);
const ROAD_BLACK: SdlColor = SdlColor::RGB(184, 176, 152);

const WHITE: SdlColor = SdlColor::RGB(255, 250, 250);
const BLACK: SdlColor = SdlColor::RGB(70, 60, 60);

impl Renderable for Board {
    fn render(&self, context: &mut RenderContext) {
        let board_size = self.spaces.len();

        let (size_x, size_y) = {
            let (width, height) = context.sdl_renderer.logical_size();
            (width as f32, height as f32)
        };

        let (center_x, center_y) = (size_x / 2.0, size_y / 2.0);

        let tile_size = size_y * 0.8 / board_size as f32;
        let board_width = tile_size * board_size as f32;

        for (x, column) in self.spaces.iter().enumerate() {
            for (y, stack) in column.iter().enumerate() {
                match stack.last() {
                    Some(&Piece::Flatstone(color)) |
                    Some(&Piece::Capstone(color)) => match color {
                        Color::White => context.sdl_renderer.set_draw_color(ROAD_WHITE),
                        Color::Black => context.sdl_renderer.set_draw_color(ROAD_BLACK),
                    },
                    _ => context.sdl_renderer.set_draw_color(TILE),
                }

                let (tile_ll_corner_x, tile_ll_corner_y) = (
                    center_x - board_width / 2.0 + (x as f32) * tile_size,
                    center_y + board_width / 2.0 - (y as f32) * tile_size,
                );

                context.sdl_renderer.fill_rect(
                    SdlRectangle::new(
                        (tile_ll_corner_x + 0.05 * tile_size).round() as i32,
                        (tile_ll_corner_y - (1.0 - 0.05) * tile_size).round() as i32,
                        (0.9 * tile_size).round() as u32,
                        (0.9 * tile_size).round() as u32,
                    )
                ).ok();

                let (width, height) = (tile_size * 0.75, tile_size * 0.15);

                for (z, piece) in stack.iter().enumerate() {
                    let board_piece = BoardPiece {
                        board: &self,
                        piece: piece,
                        x: match piece {
                            &Piece::Flatstone(color) => match color {
                                Color::White => (0.05 + 0.05) * tile_size + tile_ll_corner_x + width / 2.0,
                                Color::Black => (0.1 + 0.05) * tile_size + tile_ll_corner_x + width / 2.0,
                            },
                            &Piece::StandingStone(_) |
                            &Piece::Capstone(_) => 0.5 * tile_size + tile_ll_corner_x,
                        },
                        y: tile_ll_corner_y - (0.075 * z as f32 + 0.075) * tile_size - height / 2.0,
                    };

                    board_piece.render(context);
                }

            }
        }
    }
}

struct BoardPiece<'a> {
    board: &'a Board,
    piece: &'a Piece,
    x: f32,
    y: f32,
}

impl<'a> Renderable for BoardPiece<'a> {
    fn render(&self, context: &mut RenderContext) {
        let size_y = {
            let (_, height) = context.sdl_renderer.logical_size();
            height as f32
        };

        let board_size = self.board.spaces.len();
        let tile_size = size_y * 0.8 / board_size as f32;

        match self.piece {
            &Piece::Flatstone(color) => {
                let (width, height) = (
                    (tile_size * 0.75).round() as u32,
                    (tile_size * 0.15).round() as u32
                );

                let (x, y) = (
                    (self.x - width as f32 / 2.0).round() as i32,
                    (self.y - height as f32 / 2.0).round() as i32,
                );

                match color {
                    Color::White => context.sdl_renderer.set_draw_color(SdlColor::RGB(0, 0, 0)),
                    Color::Black => context.sdl_renderer.set_draw_color(SdlColor::RGB(255, 255, 255)),
                };

                context.sdl_renderer.draw_rect(
                    SdlRectangle::new(
                        x - 1,
                        y - 1,
                        width + 2,
                        height + 2,
                    )
                ).ok();

                match color {
                    Color::White => context.sdl_renderer.set_draw_color(WHITE),
                    Color::Black => context.sdl_renderer.set_draw_color(BLACK),
                };

                context.sdl_renderer.fill_rect(
                    SdlRectangle::new(
                        if color == Color::White { x } else { x - 1 },
                        y,
                        if color == Color::White { width } else { width + 2 },
                        height,
                    )
                ).ok();
            },
            &Piece::StandingStone(color) => {
                let x = [
                    (self.x - 0.4 * tile_size / 2.0).round() as i16,
                    (self.x + 0.4 * tile_size / 2.0).round() as i16,
                    (self.x + 0.4 * tile_size / 2.0).round() as i16,
                    (self.x - 0.4 * tile_size / 2.0).round() as i16,
                ];

                let y = [
                    (self.y - 0.4 * tile_size).round() as i16,
                    (self.y - 0.3 * tile_size).round() as i16,
                    (self.y + 0.05 * tile_size).round() as i16,
                    (self.y - 0.05 * tile_size).round() as i16,
                ];

                context.sdl_renderer.filled_polygon(&x, &y, match color {
                    Color::White => WHITE,
                    Color::Black => BLACK,
                }).ok();

                context.sdl_renderer.aa_polygon(&x, &y, match color {
                    Color::White => SdlColor::RGB(0, 0, 0),
                    Color::Black => SdlColor::RGB(255, 255, 255),
                }).ok();
            },
            &Piece::Capstone(color) => {
                let x = [
                    (self.x - 0.4 * tile_size / 2.0).round() as i16,
                    (self.x + 0.4 * tile_size / 2.0).round() as i16,
                    (self.x + 0.5 * tile_size / 2.0).round() as i16,
                    (self.x - 0.5 * tile_size / 2.0).round() as i16,
                ];

                let y = [
                    (self.y - 0.4 * tile_size).round() as i16,
                    (self.y - 0.4 * tile_size).round() as i16,
                    (self.y + 0.05 * tile_size).round() as i16,
                    (self.y + 0.05 * tile_size).round() as i16 - 1, // XXX Avoid a perfectly horizontal line here because of a bug in SDL2_gfx
                ];

                context.sdl_renderer.filled_polygon(&x, &y, match color {
                    Color::White => WHITE,
                    Color::Black => BLACK,
                }).ok();

                context.sdl_renderer.aa_polygon(&x, &y, match color {
                    Color::White => SdlColor::RGB(0, 0, 0),
                    Color::Black => SdlColor::RGB(255, 255, 255),
                }).ok();
            },
        }
    }
}
