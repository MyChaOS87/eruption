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
*/

use std::fs;

use crate::constants;

pub fn is_eruption_daemon_running() -> bool {
    let result = fs::read_to_string(&constants::PID_FILE);

    // .map_err(|e| {
    //     eprintln!(
    //         "Could not determine whether the Eruption daemon is running: {}",
    //         e
    //     )
    // });

    result.is_ok()
}
