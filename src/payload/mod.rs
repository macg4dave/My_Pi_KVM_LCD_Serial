mod icons;
mod parser;
mod schema;

pub use icons::{DisplayMode, Icon};
pub use parser::{
    decode_command_frame, encode_command_frame, CommandMessage, CommandStream, Defaults, Payload,
    RenderFrame, COMMAND_MAX_CHUNK_BYTES, COMMAND_MAX_COMMAND_CHARS, COMMAND_MAX_FRAME_BYTES,
    COMMAND_MAX_SCRATCH_PATH_BYTES, COMMAND_SCHEMA_VERSION,
};
pub use schema::{
    decode_tunnel_frame, encode_tunnel_msg, TunnelMsg, TunnelMsgOwned, TUNNEL_MAX_FRAME_BYTES,
};

pub const DEFAULT_SCROLL_MS: u64 = 250;
pub const DEFAULT_PAGE_TIMEOUT_MS: u64 = 4000;
