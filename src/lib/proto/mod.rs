// import tonic-generated proto rust code:
pub mod proto {
    tonic::include_proto!("dataloader"); // The string specified here must match the proto package name
}

// import modules into namespace to be able to import it as 'use crate::proto::{...};
// otherwise it would require to import as 'use crate::proto::proto::{...}';
pub use proto::*;
