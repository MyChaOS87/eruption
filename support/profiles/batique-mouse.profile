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


id = '5dc59fa6-e965-25cb-a0da-e87d28713094'
name = 'Batique (Mouse only)'
description = 'The batique profile (Mouse only)'
active_scripts = [
	'batique.lua',
#   'dim-zone.lua',
	'macros.lua',
#	'stats.lua',
]

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
