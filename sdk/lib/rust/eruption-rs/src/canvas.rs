/*
    This file is part of Eruption.

    Eruption is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    Eruption is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with Eruption.  If not, see <http://www.gnu.org/licenses/>.

    Copyright (c) 2019-2022, The Eruption Development Team
*/

use crate::color::Color;
use std::ops;

const CANVAS_SIZE: usize = 144 + 36;

#[derive(Debug, Default, Clone)]
pub struct Canvas {
    pub(crate) data: Vec<Color>,
}

impl Canvas {
    pub fn new() -> Self {
        Self {
            data: vec![Color::default(); CANVAS_SIZE],
        }
    }

    /// Paint the canvas with the specified color
    pub fn fill(&mut self, color: Color) {
        self.data.fill(color);
    }
}

impl ops::Index<usize> for Canvas {
    type Output = Color;

    fn index(&self, index: usize) -> &Self::Output {
        &self.data[index]
    }
}

impl ops::IndexMut<usize> for Canvas {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.data[index]
    }
}
