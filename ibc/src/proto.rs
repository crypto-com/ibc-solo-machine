pub mod cosmos {
    pub mod crypto {
        pub mod ed25519 {
            tonic::include_proto!("cosmos.crypto.ed25519");
        }

        pub mod multisig {
            tonic::include_proto!("cosmos.crypto.multisig");

            pub mod v1beta1 {
                tonic::include_proto!("cosmos.crypto.multisig.v1beta1");
            }
        }

        pub mod secp256k1 {
            tonic::include_proto!("cosmos.crypto.secp256k1");
        }

        pub mod secp256r1 {
            tonic::include_proto!("cosmos.crypto.secp256r1");
        }
    }
}

use anyhow::{Context, Result};
use prost::Message;
use prost_types::Any;

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
            fn from_any(value: &::prost_types::Any) -> ::anyhow::Result<Self> {
                ::anyhow::ensure!(
                    value.type_url == $type_url,
                    "invalid type url for `Any` type: expected `{}` and found `{}`",
                    $type_url,
                    value.type_url
                );

                <Self as ::prost::Message>::decode(value.value.as_slice()).map_err(Into::into)
            }

            fn to_any(&self) -> ::anyhow::Result<::prost_types::Any> {
                Ok(::prost_types::Any {
                    type_url: $type_url.to_owned(),
                    value: $crate::proto::proto_encode(self)?,
                })
            }
        }
    };
}

// macro_rules! impl_any_conversion {
//     ($type: ty, $type_url: ident) => {
//         impl ::std::convert::TryFrom<&$type> for ::prost_types::Any {
//             type Error = ::anyhow::Error;

//             fn try_from(value: &$type) -> ::std::result::Result<Self, Self::Error> {
//                 Ok(Self {
//                     type_url: $type_url.to_owned(),
//                     value: $crate::proto::proto_encode(value)?,
//                 })
//             }
//         }

//         impl ::std::convert::TryFrom<&::prost_types::Any> for $type {
//             type Error = ::anyhow::Error;

//             fn try_from(value: &::prost_types::Any) -> ::std::result::Result<Self, Self::Error> {
//                 ::anyhow::ensure!(
//                     value.type_url == $type_url,
//                     "invalid type url for `Any` type: expected `{}` and found `{}`",
//                     $type_url,
//                     value.type_url
//                 );

//                 <Self as ::prost::Message>::decode(value.value.as_slice())
//             }
//         }
//     };
// }
