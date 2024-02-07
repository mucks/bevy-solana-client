#[cfg(feature = "wasm")]
mod wasm_client;

mod rpc_client;

use bevy::prelude::*;

pub struct BevySolanaClient;

impl Plugin for BevySolanaClient {
    fn build(&self, app: &mut App) {
        #[cfg(feature = "wasm")]
        app.add_plugins(wasm_client::WasmClient);
    }
}
