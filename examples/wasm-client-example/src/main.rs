use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(bevy_solana_client::BevySolanaClient)
        .run();
}
