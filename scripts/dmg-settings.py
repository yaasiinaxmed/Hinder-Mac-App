# dmgbuild settings for the Hinder installer window.
# Used by scripts/bundle.sh:  dmgbuild -s scripts/dmg-settings.py -D app=... -D background=... "Hinder" out.dmg
# The window layout is written straight into the image's .DS_Store, so no
# Finder scripting (or Automation permission) is needed.
import os

app = defines["app"]  # noqa: F821  (provided by dmgbuild)
app_name = os.path.basename(app)

format = "UDZO"
filesystem = "HFS+"
size = None

files = [app]
symlinks = {"Applications": "/Applications"}
hide_extensions = [app_name]

icon = defines.get("volicon")  # noqa: F821
badge_icon = None

# Window: matches assets/dmg/background.svg (660 x 420 points).
background = defines["background"]  # noqa: F821
window_rect = ((200, 120), (660, 420))
default_view = "icon-view"
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
show_icon_preview = False

arrange_by = None
grid_offset = (0, 0)
scroll_position = (0, 0)
label_pos = "bottom"
text_size = 13
icon_size = 128

icon_locations = {
    app_name: (170, 215),
    "Applications": (490, 215),
}
