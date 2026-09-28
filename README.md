# csldr-rs

fork of https://github.com/mikkokko/csldr/ but with new features (just rust rewrite)

csldr is a custom client library (`client.dll` / `client_i386.so`) implementation for counter-strike 1.6 and half-life (goldsrc engine) written in rust.

Supports **Linux** (`i686-unknown-linux-gnu`) and **Windows** (`i686-pc-windows-gnu` / `i686-pc-windows-msvc`).

---

## features

- **!!3d hrtf & acoustic ray tracing!!**:
  - binaural audio spatialization using kemar hrir datasets with interaural time delay (itd) and head shadowing.
  - real-time 3d ray tracing for wall occlusion, material absorption, geometry reflections, and reverberation calculations.
  - debug overlay for acoustic state and 3d sound ray visualization in-game.
  - freecam spectator mode for 3d acoustic inspection (`hrtf_freecam`).

check the youtube videos for more demonstrations:  
https://www.youtube.com/watch?v=2PTGzt5GvBQ  
https://www.youtube.com/watch?v=4bgmYFFC76I

- **viewmodel manipulation**: configurable viewmodel fov, 3d offsets, origin offsets, rotation (pitch/yaw/roll), sway, lag, bobbing styles, and shift animations.
- **hor+ widescreen fov**: scaling logic (`fov_horplus`) to maintain correct vertical field of view across widescreen aspect ratios (16:9, 16:10, 21:9).
- **customizable 2d crosshair**: 2d opengl crosshair rendering with configurable size, thickness, gap, outline, center dot, rgba color, and dynamic movement/firing spread.
- **movement hud overlay**: on-screen keypress indicator (wasd, jump, duck) and player movement velocity stats.
- **motion blur**: camera rotation motion blur post-processing with configurable intensity and sample count.
- **weapon inspection** (not working): weapon inspect animation trigger via command (`+inspect`).
- **studio model cache**: configurable multi-entry caching system for studio models (`r_studio_cache`).
- **shell casings persistence**: guaranteed ejection and persistence of ejected shell casings (`shell_guarantee`).
- **command filtering**: auto-prefacing raw ip addresses with `connect` command in console.

---

## console commands

| command               | description                                                  |
| --------------------- | ------------------------------------------------------------ |
| `+inspect`            | triggers weapon inspection animation for the active weapon.  |
| `r_studio_flush`      | flushes cached studio model data.                            |
| `hrtf_status`         | prints audio hook status and active hrtf configuration.      |
| `hrtf_rays`           | prints acoustic ray tracing state and reflection statistics. |
| `hrtf_freecam_toggle` | toggles freecam spectator mode for 3d acoustic debugging.    |

---

## cvars reference

### viewmodel (`viewmodel_*`)

| cvar                    | default | description                                       |
| ----------------------- | ------- | ------------------------------------------------- |
| `viewmodel_fov`         | `0`     | viewmodel fov override (`0` uses engine default). |
| `viewmodel_offset_x`    | `0`     | horizontal viewmodel offset (x-axis).             |
| `viewmodel_offset_y`    | `0`     | forward/backward viewmodel offset (y-axis).       |
| `viewmodel_offset_z`    | `0`     | vertical viewmodel offset (z-axis).               |
| `viewmodel_origin_x`    | `0`     | viewmodel origin offset x.                        |
| `viewmodel_origin_y`    | `0`     | viewmodel origin offset y.                        |
| `viewmodel_origin_z`    | `0`     | viewmodel origin offset z.                        |
| `viewmodel_roll`        | `0`     | viewmodel roll rotation angle.                    |
| `viewmodel_pitch`       | `0`     | viewmodel pitch rotation angle.                   |
| `viewmodel_yaw`         | `0`     | viewmodel yaw rotation angle.                     |
| `viewmodel_bob`         | `0`     | viewmodel bobbing intensity multiplier.           |
| `viewmodel_bob_style`   | `0`     | viewmodel bobbing style mode.                     |
| `viewmodel_sway`        | `0`     | viewmodel movement sway intensity.                |
| `viewmodel_lag`         | `0`     | viewmodel rotational movement lag intensity.      |
| `viewmodel_shift`       | `0`     | viewmodel shift animation intensity.              |
| `viewmodel_shift_style` | `0`     | viewmodel shift animation style.                  |

### field of view (`fov_*`)

| cvar          | default | description                                         |
| ------------- | ------- | --------------------------------------------------- |
| `fov_horplus` | `1`     | enables hor+ fov scaling on widescreen resolutions. |

### crosshair (`crosshair_*`)

| cvar                  | default | description                                              |
| --------------------- | ------- | -------------------------------------------------------- |
| `crosshair_enabled`   | `0`     | enables custom 2d crosshair rendering (`1` to enable).   |
| `crosshair_size`      | `6`     | length of crosshair lines.                               |
| `crosshair_thickness` | `1.5`   | thickness of crosshair lines in pixels.                  |
| `crosshair_gap`       | `3`     | gap distance from center.                                |
| `crosshair_outline`   | `1`     | enables black outline around crosshair lines.            |
| `crosshair_dot`       | `0`     | enables center dot.                                      |
| `crosshair_color_r`   | `0`     | red color component (0–255).                             |
| `crosshair_color_g`   | `255`   | green color component (0–255).                           |
| `crosshair_color_b`   | `0`     | blue color component (0–255).                            |
| `crosshair_color_a`   | `255`   | alpha / transparency component (0–255).                  |
| `crosshair_dynamic`   | `1`     | enables dynamic crosshair spread during movement/firing. |

### movement hud (`hud_movement*`)

| cvar                 | default | description                                                 |
| -------------------- | ------- | ----------------------------------------------------------- |
| `hud_movement`       | `0`     | enables movement keys overlay & velocity display on screen. |
| `hud_movement_scale` | `1.0`   | scale multiplier for movement hud elements.                 |

### motion blur (`mblur_*`)

| cvar              | default | description                                          |
| ----------------- | ------- | ---------------------------------------------------- |
| `mblur_enable`    | `0`     | enables camera rotation motion blur post-processing. |
| `mblur_intensity` | `0.5`   | strength of motion blur effect.                      |
| `mblur_samples`   | `8`     | number of blur accumulation samples.                 |

### hrtf & acoustics (`hrtf_*`)

| cvar                 | default | description                                        |
| -------------------- | ------- | -------------------------------------------------- |
| `hrtf_enable`        | `0`     | enables 3d binaural hrtf sound processing.         |
| `hrtf_volume`        | `1.0`   | output volume scaling for hrtf audio.              |
| `hrtf_occlusion`     | `1`     | enables wall sound occlusion calculation.          |
| `hrtf_reverb`        | `1`     | enables room acoustic reverberation simulation.    |
| `hrtf_debug`         | `0`     | renders acoustics status hud overlay.              |
| `hrtf_debug_rays`    | `0`     | renders 3d acoustic sound reflection rays in-game. |
| `hrtf_freecam`       | `0`     | enables acoustics freecam spectator mode.          |
| `hrtf_freecam_speed` | `500`   | movement speed of the acoustics freecam.           |

### rendering & gameplay (`r_*` / `shell_*`)

| cvar                  | default | description                                                |
| --------------------- | ------- | ---------------------------------------------------------- |
| `r_studio_cache`      | `1`     | enables caching for studio models.                         |
| `r_studio_cache_size` | `64`    | maximum capacity for studio model cache.                   |
| `shell_guarantee`     | `1`     | ensures shell casing ejection and prevents early despawns. |

---

## building

### requirements

- [rust toolchain](https://rustup.rs/) (cargo, rustc)
- 32-bit compilation target (`i686` architecture required for goldsrc engine):
  - linux: `i686-unknown-linux-gnu`
  - windows: `i686-pc-windows-msvc` or `i686-pc-windows-gnu`

### build commands

```bash
# add 32-bit target (linux example)
rustup target add i686-unknown-linux-gnu

# build release binary
cargo build --target i686-unknown-linux-gnu --release
```

the compiled dynamic library binary path:

- **linux**: `target/i686-unknown-linux-gnu/release/libclient.so`
- **windows**: `target/i686-pc-windows-msvc/release/client.dll`

---

## installation

1. copy the compiled library (`client_i386.so` / `client.so` / `client.dll`) to your game directory under `cstrike/cl_dlls/` or `valve/cl_dlls/`.
2. launch counter-strike 1.6 or half-life.
3. configure cvars via console or bind weapon inspection:
   ```cfg
   bind f +inspect
   ```
