pub mod board;
pub mod env;
pub mod game_config;
pub mod nn;
pub mod mcts;
pub mod alphazero;
pub mod generator;
pub mod quest_manager;
pub mod score_manager;
pub mod tile;
pub mod unity_random;
pub mod gpu_engine;
pub mod gpu_nn;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;


