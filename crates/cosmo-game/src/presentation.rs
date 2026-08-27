//! The virtual screen: everything is drawn into one 320x200 offscreen
//! texture at the original's exact resolution, and that texture is scaled to
//! the window exactly once.
//!
//! This replaces having each camera scale itself to the window
//! independently. That approach put source pixels on fractional boundaries -
//! measured at one common window size, a single source pixel covered 11.33
//! output pixels, so rows came out alternately 11 and 12 tall. The
//! unevenness is subtle but it is on every edge in the game.
//!
//! Rendering to a fixed 320x200 buffer also lets the layout match the
//! original's screen exactly, which the window-relative version could not.
//! `DrawStaticGameScreen` (game2.c:3590-3610) blits the status bar into
//! screen tiles x 1..38, y 19..24, and `DrawMapRegion` (game1.c:885-901)
//! draws the play area from tile (1,1) - so there is an 8px black border
//! down the left, across the top and down the right that we were not
//! reproducing at all.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::camera::RenderTarget;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, ShaderRef, ShaderType, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::render::view::RenderLayers;
use bevy::sprite::{AlphaMode2d, Material2d, Material2dPlugin};

/// The original's screen, in pixels.
pub const SCREEN_W: u32 = 320;
pub const SCREEN_H: u32 = 200;

/// The play area within it: tiles (1,1)..(38,18).
pub const PLAY_X: u32 = 8;
pub const PLAY_Y: u32 = 8;
pub const PLAY_W: u32 = 304;
pub const PLAY_H: u32 = 144;

/// The status bar: tiles x 1..38, y 19..24.
pub const BAR_X: u32 = 8;
pub const BAR_Y: u32 = 152;
pub const BAR_W: u32 = 304;
pub const BAR_H: u32 = 48;

/// The layer the fullscreen present quad lives on, so no other camera draws
/// it and it draws nothing else.
pub const PRESENT_LAYER: usize = 2;

#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub enum PresentationMode {
    /// Integer scaling, letterboxed, no filtering or effects - what the
    /// original put on a VGA monitor, pixels and all.
    Authentic,
    /// Fills the window with sharp-bilinear scaling plus a restrained CRT
    /// treatment.
    Remaster,
}

impl PresentationMode {
    pub fn from_env() -> Self {
        match std::env::var("COSMO_PRESENT").as_deref() {
            Ok("authentic") => PresentationMode::Authentic,
            _ => PresentationMode::Remaster,
        }
    }

    pub fn toggled(self) -> Self {
        match self {
            PresentationMode::Authentic => PresentationMode::Remaster,
            PresentationMode::Remaster => PresentationMode::Authentic,
        }
    }

    fn settings(self) -> PresentSettings {
        match self {
            PresentationMode::Authentic => PresentSettings {
                source_size: Vec2::new(SCREEN_W as f32, SCREEN_H as f32),
                output_size: Vec2::new(SCREEN_W as f32, SCREEN_H as f32),
                sharp: 0.0,
                scanline: 0.0,
                bloom: 0.0,
                vignette: 0.0,
                curvature: 0.0,
                _pad: 0.0,
            },
            PresentationMode::Remaster => PresentSettings {
                source_size: Vec2::new(SCREEN_W as f32, SCREEN_H as f32),
                output_size: Vec2::new(SCREEN_W as f32, SCREEN_H as f32),
                sharp: 1.0,
                // Deliberately mild. A heavy treatment reads as a filter
                // sitting on top of the game; the point is to make the art
                // look intentional, not costumed.
                scanline: 0.18,
                // Bloom is thresholded in the shader so it glows on EGA's
                // saturated primaries rather than smearing every edge. At
                // 0.35 flat it just read as "out of focus".
                bloom: 0.16,
                vignette: 0.18,
                // Off. Barrel distortion samples outside the texture at the
                // corners, so any visible amount crops the picture - it ate
                // the corner of the status bar. The knob stays for taste.
                curvature: 0.0,
                _pad: 0.0,
            },
        }
    }
}

/// The offscreen 320x200 image every camera renders into.
#[derive(Resource)]
pub struct VirtualScreen(pub Handle<Image>);

impl VirtualScreen {
    pub fn target(&self) -> RenderTarget {
        RenderTarget::Image(self.0.clone().into())
    }
}

#[derive(Component)]
pub struct PresentQuad;

#[derive(Component)]
pub struct PresentCamera;

#[derive(ShaderType, Debug, Clone)]
pub struct PresentSettings {
    pub source_size: Vec2,
    pub output_size: Vec2,
    pub sharp: f32,
    pub scanline: f32,
    pub bloom: f32,
    pub vignette: f32,
    pub curvature: f32,
    pub _pad: f32,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct PresentMaterial {
    #[uniform(0)]
    pub settings: PresentSettings,
    #[texture(1)]
    #[sampler(2)]
    pub screen: Handle<Image>,
}

impl Material2d for PresentMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/present.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Opaque
    }
}

pub struct PresentationPlugin;

impl Plugin for PresentationPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<PresentMaterial>::default())
            .add_systems(Update, (toggle_mode, fit_present_quad).chain());
    }
}

/// Creates the virtual screen and the camera that shows it.
///
/// Runs directly on the world before the app starts, for the same reason the
/// other core resources do: the initial state transition happens before a
/// `Startup` system's deferred commands are applied, so an `OnEnter` system
/// would find these missing.
pub fn insert_virtual_screen(app: &mut App) {
    let mut image = Image::new_fill(
        Extent3d {
            width: SCREEN_W,
            height: SCREEN_H,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    // Linear filtering is what lets the shader blend *across* a texel
    // boundary; the sharp-bilinear UV maths is what keeps the inside of each
    // texel flat. Nearest here would defeat it.
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());

    let handle = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(image.clone());

    let mode = PresentationMode::from_env();
    let mut settings = mode.settings();
    settings.output_size = Vec2::new(SCREEN_W as f32, SCREEN_H as f32);

    let material = app
        .world_mut()
        .resource_mut::<Assets<PresentMaterial>>()
        .add(PresentMaterial {
            settings,
            screen: handle.clone(),
        });
    let mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Rectangle::new(1.0, 1.0));

    app.world_mut().spawn((
        PresentCamera,
        Camera2d,
        Camera {
            // Above every camera that draws into the virtual screen.
            order: 100,
            ..default()
        },
        RenderLayers::layer(PRESENT_LAYER),
    ));
    app.world_mut().spawn((
        PresentQuad,
        Mesh2d(mesh),
        MeshMaterial2d(material),
        Transform::default(),
        RenderLayers::layer(PRESENT_LAYER),
    ));

    app.insert_resource(VirtualScreen(handle));
    app.insert_resource(mode);
}

/// F5 flips between authentic and remastered presentation.
fn toggle_mode(keys: Res<ButtonInput<KeyCode>>, mut mode: ResMut<PresentationMode>) {
    if keys.just_pressed(KeyCode::F5) {
        *mode = mode.toggled();
        info!("presentation: {:?}", *mode);
    }
}

/// Sizes the present quad to the window and keeps the shader's idea of the
/// output size in step with it.
fn fit_present_quad(
    windows: Query<&Window>,
    mode: Res<PresentationMode>,
    mut materials: ResMut<Assets<PresentMaterial>>,
    mut quad: Query<(&mut Transform, &MeshMaterial2d<PresentMaterial>), With<PresentQuad>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((mut transform, handle)) = quad.single_mut() else {
        return;
    };
    let size = window.resolution.size();
    if size.x < 1.0 || size.y < 1.0 {
        return;
    }
    let dpi = window.resolution.scale_factor().max(0.001);
    let scale = scale_for(size.x * dpi, size.y * dpi, *mode);

    // The quad is sized in logical pixels, because that is what a default
    // Camera2d maps one world unit to...
    transform.scale = Vec3::new(
        SCREEN_W as f32 * scale / dpi,
        SCREEN_H as f32 * scale / dpi,
        1.0,
    );

    if let Some(material) = materials.get_mut(&handle.0) {
        let mut settings = mode.settings();
        // ...but the shader reasons in *physical* pixels, because how wide
        // the blend ramp should be is a rasterisation question. Feeding it
        // logical pixels on a HiDPI display made the ramp `scale_factor`
        // times too wide, which is simply blur.
        settings.output_size = Vec2::new(SCREEN_W as f32 * scale, SCREEN_H as f32 * scale);
        material.settings = settings;
    }
}

/// How many **physical** pixels one source pixel covers.
///
/// Physical, not logical, is the whole point in authentic mode: the scale
/// has to be a whole number of the pixels actually lit, and a HiDPI display
/// puts a fractional scale factor between the two. Rounding in logical
/// space on a 1.5x display gave 10.5 physical pixels per source pixel -
/// still alternating 10 and 11, which is exactly the artefact the integer
/// mode exists to remove.
///
/// Remaster mode fills the window and leaves the evenness to the shader.
pub fn scale_for(physical_w: f32, physical_h: f32, mode: PresentationMode) -> f32 {
    let scale = (physical_w / SCREEN_W as f32).min(physical_h / SCREEN_H as f32);
    match mode {
        PresentationMode::Authentic => scale.floor().max(1.0),
        PresentationMode::Remaster => scale.max(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authentic_scaling_is_a_whole_number_of_physical_pixels() {
        // A 2552x1432 logical window on a 1.5x display: 3828x2148 physical,
        // which is 10.74 source-pixels-per-pixel. Authentic must land on 10.
        assert_eq!(scale_for(3828.0, 2148.0, PresentationMode::Authentic), 10.0);
        // The bug this replaced rounded in logical space instead, giving
        // floor(7.16) = 7 logical = 10.5 physical.
        assert_ne!(scale_for(3828.0, 2148.0, PresentationMode::Authentic), 10.5);
    }

    #[test]
    fn remaster_scaling_fills_the_window_on_its_tightest_axis() {
        // 2148/200 = 10.74 is tighter than 3828/320 = 11.96.
        let s = scale_for(3828.0, 2148.0, PresentationMode::Remaster);
        assert!((s - 10.74).abs() < 0.001);
        assert!((SCREEN_H as f32 * s - 2148.0).abs() < 0.01);
    }

    #[test]
    fn a_window_smaller_than_the_virtual_screen_still_shows_something() {
        assert_eq!(scale_for(120.0, 90.0, PresentationMode::Authentic), 1.0);
    }

    #[test]
    fn the_two_modes_agree_when_the_scale_is_already_integral() {
        let a = scale_for(1280.0, 800.0, PresentationMode::Authentic);
        let r = scale_for(1280.0, 800.0, PresentationMode::Remaster);
        assert_eq!(a, r);
        assert_eq!(a, 4.0);
    }

    #[test]
    fn the_scale_is_uniform_so_the_aspect_ratio_is_never_distorted() {
        // One scalar for both axes is what guarantees this; the test guards
        // against anyone splitting it into an x and a y scale later.
        for (w, h) in [(1913.0, 733.0), (640.0, 4000.0), (3828.0, 2148.0)] {
            for mode in [PresentationMode::Authentic, PresentationMode::Remaster] {
                let s = scale_for(w, h, mode);
                let (pw, ph) = (SCREEN_W as f32 * s, SCREEN_H as f32 * s);
                let want = SCREEN_W as f32 / SCREEN_H as f32;
                assert!((pw / ph - want).abs() < 0.001, "{mode:?} distorted {w}x{h}");
                assert!(pw <= w.max(SCREEN_W as f32) + 0.01 || s == 1.0);
            }
        }
    }
}
