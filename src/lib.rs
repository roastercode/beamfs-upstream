// SPDX-License-Identifier: GPL-2.0-only
//! What a kernel patch series has to pass before it is mailed, as one
//! crate beamfs-bench and beamfs-xfstests both run, so the two tools
//! control the same code by the same rules at the same version.
#![deny(warnings)]

pub mod check;
pub mod gate;
pub mod kernelorg;
pub mod series;
pub mod session;
pub mod sign;
pub mod text;
