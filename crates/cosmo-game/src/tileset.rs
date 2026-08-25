use crate::data::GameData;
use bevy::prelude::*;

pub const TILE_PX: f32 = 8.0;

#[derive(Resource)]
pub struct TilesetAssets {
    pub solid_image: Handle<Image>,
    pub solid_layout: Handle<TextureAtlasLayout>,
    pub masked_image: Handle<Image>,
    pub masked_layout: Handle<TextureAtlasLayout>,
}

pub fn load_tileset(
    asset_server: &AssetServer,
    layouts: &mut Assets<TextureAtlasLayout>,
    data: &GameData,
) -> TilesetAssets {
    let cols = data.tileset.atlas_cols;
    let solid_rows = (data.tileset.solid_tile_count as u32).div_ceil(cols).max(1);
    let masked_rows = (data.tileset.masked_tile_count as u32).div_ceil(cols).max(1);

    let solid_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(TILE_PX as u32),
        cols,
        solid_rows,
        None,
        None,
    ));
    let masked_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(TILE_PX as u32),
        cols,
        masked_rows,
        None,
        None,
    ));

    TilesetAssets {
        solid_image: asset_server.load("generated/tileset_solid.png"),
        solid_layout,
        masked_image: asset_server.load("generated/tileset_masked.png"),
        masked_layout,
    }
}
