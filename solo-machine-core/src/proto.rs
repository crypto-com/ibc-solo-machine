#![allow(missing_docs)]
// pub mod cosmos {
//     pub mod crypto {
//         pub mod ed25519 {
//             tonic::include_proto!("cosmos.crypto.ed25519");
//         }

//         pub mod multisig {
//             tonic::include_proto!("cosmos.crypto.multisig");

//             pub mod v1beta1 {
//                 tonic::include_proto!("cosmos.crypto.multisig.v1beta1");
//             }
//         }

//         pub mod secp256k1 {
//             tonic::include_proto!("cosmos.crypto.secp256k1");
//         }

//         pub mod secp256r1 {
//             tonic::include_proto!("cosmos.crypto.secp256r1");
//         }
//     }
// }

#[cfg(feature = "ethermint")]
pub mod ethermint {
    pub mod types {
        pub mod v1 {
            tonic::include_proto!("ethermint.types.v1");
        }
    }

    pub mod crypto {
        pub mod v1 {
            pub mod ethsecp256k1 {
                tonic::include_proto!("ethermint.crypto.v1.ethsecp256k1");
            }
        }
    }
}

// #[cfg(feature = "solomachine-v2")]
// pub mod ibc {
//     pub mod lightclients {
//         pub mod solomachine {
//             pub mod v2 {
//                 tonic::include_proto!("ibc.lightclients.solomachine.v2");
//             }
//         }
//     }
// }

use anyhow::{Context, Result};
use ibc_proto::google::protobuf::Any;
use prost::Message;

pub fn proto_encode<M: Message>(message: &M) -> Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(message.encoded_len());
    message
        .encode(&mut buf)
        .context("unable to encode protobuf message")?;
    Ok(buf)
}

pub trait AnyConvert: Sized {
    fn from_any(value: &Any) -> Result<Self>;

    fn to_any(&self) -> Result<Any>;
}

macro_rules! impl_any_conversion {
    ($type: ty, $type_url: ident) => {
        impl $crate::proto::AnyConvert for $type {
            fn from_any(value: &::ibc_proto::google::protobuf::Any) -> ::anyhow::Result<Self> {
                ::anyhow::ensure!(
                    value.type_url == $type_url,
                    "invalid type url for `Any` type: expected `{}` and found `{}`",
                    $type_url,
                    value.type_url
                );

                <Self as ::prost::Message>::decode(value.value.as_slice()).map_err(Into::into)
            }

            fn to_any(&self) -> ::anyhow::Result<::ibc_proto::google::protobuf::Any> {
                Ok(::ibc_proto::google::protobuf::Any {
                    type_url: $type_url.to_owned(),
                    value: $crate::proto::proto_encode(self)?,
                })
            }
        }
    };
}
