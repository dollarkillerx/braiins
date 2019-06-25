//! Definition of all Stratum V1 messages

use bitcoin_hashes::sha256d::Hash;
use failure::ResultExt;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

use super::error::ErrorKind;
use super::framing;
use super::{ExtraNonce1, V1Handler, V1Protocol};
use crate::error::{Error, Result};
use crate::v1::framing::Method;
use crate::v1::{HexBytes, HexU32Le};

#[cfg(test)]
pub mod test;

macro_rules! impl_conversion_request {
    ($request:ty, $method:path, $handler_fn:ident) => {
        impl TryFrom<$request> for framing::RequestPayload {
            type Error = crate::error::Error;

            fn try_from(msg: $request) -> Result<Self> {
                let params = serde_json::to_value(msg).context("Cannot parse request")?;

                Ok(Self {
                    method: $method,
                    params,
                })
            }
        }

        impl TryFrom<framing::Request> for $request {
            type Error = crate::error::Error;

            fn try_from(req: framing::Request) -> std::result::Result<Self, Self::Error> {
                // Invariant: it's caller's responsibility to ensure not to pass wrong request
                // for conversion
                assert_eq!(req.payload.method, $method);

                serde_json::from_value(req.payload.params).map_err(Into::into)
            }
        }

        impl wire::Payload<V1Protocol> for $request {
            fn accept(&self, msg: &wire::Message<V1Protocol>, handler: &mut V1Handler) {
                handler.$handler_fn(msg, self);
            }
        }
    };
}

macro_rules! impl_conversion_response {
    ($response:ty) => {
        impl TryFrom<$response> for framing::ResponsePayload {
            type Error = crate::error::Error;

            fn try_from(resp: $response) -> Result<framing::ResponsePayload> {
                let result = framing::StratumResult(
                    serde_json::to_value(resp).context("Cannot parse response")?,
                );

                Ok(framing::ResponsePayload {
                    result: Some(result),
                    error: None,
                })
            }
        }

        impl TryFrom<framing::Response> for $response {
            type Error = crate::error::Error;

            fn try_from(resp: framing::Response) -> Result<Self> {
                let result = resp
                    .payload
                    .result
                    .ok_or(ErrorKind::Json("No result".into()))?;
                <$response>::try_from(&result)
            }
        }

        impl TryFrom<&framing::StratumResult> for $response {
            type Error = crate::error::Error;

            fn try_from(result: &framing::StratumResult) -> Result<Self> {
                // TODO this is needs to be fixed within the deserialization stack with regards
                // to the visitor pattern. We shouldn't clone any part of the incoming message
                // However, since the result is being passed by reference
                serde_json::from_value(result.0.clone())
                    .context("Failed to parse response")
                    .map_err(Into::into)
            }
        }
    };
}

/// Compounds all data required for mining subscription
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct Subscribe(
    pub Option<String>,
    pub Option<ExtraNonce1>,
    pub Option<String>,
    pub Option<String>,
);

impl Subscribe {
    //    pub fn new(agent_signature: Option<String>, extra_nonce1: ExtraNonce1, url: String, port: String) -> Self {
    //        Self(agent, extra_nonce1, url, port)
    //    }
    pub fn agent_signature(&self) -> Option<&String> {
        self.0.as_ref()
    }
    pub fn extra_nonce1(&self) -> Option<&ExtraNonce1> {
        self.1.as_ref()
    }
    pub fn url(&self) -> Option<&String> {
        self.2.as_ref()
    }
    pub fn port(&self) -> Option<&String> {
        self.3.as_ref()
    }
}

// Subscribe::try_from()
//  FIXME: verify signature, url, and port?
//  let agent_signature =
//      req.param_to_string(0, ErrorKind::Subscribe("Invalid signature".into()))?;
//  let url = req.param_to_string(2, ErrorKind::Subscribe("Invalid pool URL".into()))?;
//  let port = req.param_to_string(3, ErrorKind::Subscribe("Invalid TCP port".into()))?;
impl_conversion_request!(Subscribe, Method::Subscribe, visit_subscribe);

/// Custom subscriptions
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct Subscription(pub String, pub String);

/// Subscription response
/// TODO: Do we need to track any subscription ID's or anyhow validate those fields?
/// see StratumError for reasons why this structure doesn't have named fields
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct SubscribeResult(pub Vec<Subscription>, pub ExtraNonce1, pub usize);

impl SubscribeResult {
    pub fn subscriptions(&self) -> &Vec<Subscription> {
        &self.0
    }

    pub fn extra_nonce_1(&self) -> &ExtraNonce1 {
        &self.1
    }

    pub fn extra_nonce_2_size(&self) -> usize {
        self.2
    }
}

// TODO write a test case for parsing incorrect response
impl_conversion_response!(SubscribeResult);

/// A boolean result
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct BooleanResult(pub bool);

impl_conversion_response!(BooleanResult);

/// Subscription response
/// TODO: Do we need to track any subscription ID's or anyhow validate those fields?
/// see StratumError for reasons why this structure doesn't have named fields
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct Authorize(pub String, pub String);

impl Authorize {
    pub fn name(&self) -> &String {
        &self.0
    }

    pub fn password(&self) -> &String {
        &self.1
    }
}

impl_conversion_request!(Authorize, Method::Authorize, visit_authorize);

/// Difficulty value set by the upstream stratum server
/// Note, that we explicitly enforce 1 one element array so that serde doesn't flatten the
/// 'params' JSON array to a single value, eliminating the array completely.
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct SetDifficulty(pub [f32; 1]);

impl SetDifficulty {
    pub fn value(&self) -> f32 {
        self.0[0]
    }
}

impl_conversion_request!(SetDifficulty, Method::SetDifficulty, visit_set_difficulty);
//#[derive(Deserialize)]
//struct Helper(#[serde(with = "DurationDef")] Duration);
//
//let dur = serde_json::from_str(j).map(|Helper(dur)| dur)?;

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct JobId(HexBytes);

impl JobId {
    pub fn from_slice(job_id: &[u8]) -> Self {
        Self(HexBytes(Vec::from(job_id)))
    }
}

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct PrevHash(HexBytes);

/// Leading part of the coinbase transaction
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct CoinBase1(HexBytes);

/// Trailing part of the coinbase transaction
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct CoinBase2(HexBytes);

/// Merkle branch of transaction hashes leading to coinbase
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct MerkleBranch(Vec<HexBytes>);

/// Version field of Bitcoin block header
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct Version(HexU32Le);

/// Network difficulty target
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct Bits(HexU32Le);

/// Network time
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct Time(HexU32Le);

/// New mining job notification
/// TODO generate the field accessors
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct Notify(
    JobId,
    PrevHash,
    CoinBase1,
    CoinBase2,
    MerkleBranch,
    Version,
    Bits,
    Time,
    bool,
);

// TODO consider making the attributes return new type references, it would be less prone to typos
impl Notify {
    pub fn job_id(&self) -> &[u8] {
        &((self.0).0).0
    }

    pub fn prev_hash(&self) -> &[u8] {
        &((self.1).0).0
    }

    pub fn coin_base_1(&self) -> &[u8] {
        &((self.2).0).0
    }

    pub fn coin_base_2(&self) -> &[u8] {
        &((self.3).0).0
    }

    pub fn merkle_branch(&self) -> &Vec<HexBytes> {
        &(self.4).0
    }

    pub fn version(&self) -> u32 {
        ((self.5).0).0
    }

    pub fn bits(&self) -> u32 {
        ((self.6).0).0
    }

    pub fn time(&self) -> u32 {
        ((self.7).0).0
    }

    pub fn clean_jobs(&self) -> bool {
        self.8
    }
}

impl_conversion_request!(Notify, Method::Notify, visit_notify);

/// Combined username and worker
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct UserName(String);

/// Extra nonce 2, note the underlying serialization type
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct ExtraNonce2(HexBytes);

/// Nonce for the block
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct Nonce(HexU32Le);

/// New mining job notification
/// TODO generate the field accessors
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct Submit(UserName, JobId, ExtraNonce2, Time, Nonce, Version);

impl Submit {
    pub fn user_name(&self) -> &String {
        &(self.0).0
    }

    pub fn job_id(&self) -> &[u8] {
        &((self.1).0).0
    }

    pub fn extra_nonce_2(&self) -> &[u8] {
        &((self.2).0).0
    }

    pub fn time(&self) -> u32 {
        ((self.3).0).0
    }

    pub fn nonce(&self) -> u32 {
        ((self.4).0).0
    }

    pub fn version(&self) -> u32 {
        ((self.5).0).0
    }
}

impl_conversion_request!(Submit, Method::Submit, visit_submit);