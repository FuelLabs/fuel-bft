use fuel_pbft::*;

use curve25519_dalek::ristretto::RistrettoPoint;
use curve25519_dalek::scalar::Scalar;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdKey(Scalar);
impl Key for EdKey {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RistrettoPeerId(RistrettoPoint);
impl PeerId for RistrettoPeerId {}

fn main() {}
