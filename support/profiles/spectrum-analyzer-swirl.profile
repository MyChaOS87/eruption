#    This file is part of Eruption.
#
#    Eruption is free software: you can redistribute it and/or modify
#    it under the terms of the GNU General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
#
#    Eruption is distributed in the hope that it will be useful,
#    but WITHOUT ANY WARRANTY; without even the implied warranty of
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
#    GNU General Public License for more details.
#
#    You should have received a copy of the GNU General Public License
#    along with Eruption.  If not, see <http://www.gnu.org/licenses/>.
#
#    Copyright (c) 2019-2022, The Eruption Development Team


id = '5dc62fa6-e965-45cb-a0da-e87d29713101'
name = "Spectrum Analyzer + Color Swirls (Perlin)"
description = "Spectrum Analyzer"
active_scripts = [
	'swirl-perlin.lua',
	'audioviz3.lua',
    'halo.lua',
    'shockwave.lua',
	'batique.lua',
#   'dim-zone.lua',
 	'macros.lua',
#	'stats.lua',
]

[[config."Perlin Swirl"]]
type = 'float'
name = 'color_divisor'
value = 4.0
default = 4.0

[[config."Perlin Swirl"]]
type = 'float'
name = 'color_offset'
value = -110.0
default = -110.0

[[config."Perlin Swirl"]]
type = 'float'
name = 'time_scale'
value = 250.0
default = 250.0

[[config."Perlin Swirl"]]
type = 'float'
name = 'coord_scale'
value = 15.0
default = 15.0

[[config.Shockwave]]
type = 'color'
name = 'color_step_shockwave'
value = 0x05010000
default = 0x05010000

[[config.Shockwave]]
type = 'bool'
name = 'mouse_events'
value = true
default = true

[[config.Shockwave]]
type = 'color'
name = 'color_mouse_click_flash'
value = 0xa0ff0000
default = 0xa0ff0000

[[config.Shockwave]]
type = 'color'
name = 'color_mouse_wheel_flash'
value = 0xd0ff0000
default = 0xd0ff0000

[[config."Audio Visualizer #3 (Spectrum Analyzer)"]]
type = 'float'
name = 'opacity'
value = 0.85
default = 0.85

# mouse support
[[config.Batique]]
type = 'int'
name = 'zone_start'
value = 144
default = 144

[[config.Batique]]
type = 'int'
name = 'zone_end'
value = 180
default = 180

[[config.Batique]]
type = 'float'
name = 'coord_scale'
value = 2.5
default = 2.5

[[config.Batique]]
type = 'float'
name = 'opacity'
value = 0.025
default = 0.025

# dim a specific zone, e.g. if the mouse LEDs are too bright
[[config."Dim Zone"]]
type = 'int'
name = 'zone_start'
value = 144
default = 144

[[config."Dim Zone"]]
type = 'int'
name = 'zone_end'
value = 180
default = 180

[[config."Dim Zone"]]
type = 'float'
name = 'opacity'
value = 0.95
default = 0.95
