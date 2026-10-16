//! Fixed dense-network inference shared by native and WASM game policies.
//! `FixedPolicy<N>` supports versioned game contracts; the 32-input `Policy` alias
//! remains a synthetic fixture used by cross-platform inference tests.

use safetensors::{Dtype, SafeTensors};
use std::{error::Error, fmt};

pub const INPUTS: usize = 32;
pub const HIDDEN: usize = 64;
pub const OUTPUTS: usize = 10;
pub const ARCHITECTURE: &str = "prototype-mlp-32-64-64-10-v1";
pub const MAX_WEIGHT_BYTES: usize = 128 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub struct PolicyError(pub String);
impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for PolicyError {}

#[derive(Clone)]
pub struct FixedPolicy<const N: usize> {
    layers: [Layer; 3],
}
#[derive(Clone)]
struct Layer {
    weights: Vec<f32>,
    biases: Vec<f32>,
    inputs: usize,
}

pub type Policy = FixedPolicy<INPUTS>;

impl<const N: usize> FixedPolicy<N> {
    pub fn load(bytes: &[u8]) -> Result<Self, PolicyError> {
        if bytes.len() > MAX_WEIGHT_BYTES {
            return Err(PolicyError("weight file exceeds limit".into()));
        }
        let tensors = SafeTensors::deserialize(bytes).map_err(|e| PolicyError(e.to_string()))?;
        if tensors.len() != 6 {
            return Err(PolicyError("expected exactly six tensors".into()));
        }
        let load = |name: &str, shape: &[usize]| -> Result<Vec<f32>, PolicyError> {
            let t = tensors
                .tensor(name)
                .map_err(|e| PolicyError(e.to_string()))?;
            if t.dtype() != Dtype::F32 || t.shape() != shape {
                return Err(PolicyError(format!("invalid dtype or shape for {name}")));
            }
            let data: Vec<f32> = t
                .data()
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes(b.try_into().expect("four-byte chunk")))
                .collect();
            if data.iter().any(|v| !v.is_finite()) {
                return Err(PolicyError(format!("nonfinite tensor {name}")));
            }
            Ok(data)
        };
        let layer = |index: usize, input: usize, output: usize| -> Result<Layer, PolicyError> {
            Ok(Layer {
                weights: load(&format!("layers.{index}.weight"), &[output, input])?,
                biases: load(&format!("layers.{index}.bias"), &[output])?,
                inputs: input,
            })
        };
        Ok(Self {
            layers: [
                layer(0, N, HIDDEN)?,
                layer(1, HIDDEN, HIDDEN)?,
                layer(2, HIDDEN, OUTPUTS)?,
            ],
        })
    }

    pub fn forward(&self, observation: &[f32]) -> Result<[f32; OUTPUTS], PolicyError> {
        if observation.len() != N || observation.iter().any(|v| !v.is_finite()) {
            return Err(PolicyError(format!(
                "expected {N} finite observation values"
            )));
        }
        let mut first = [0.; HIDDEN];
        let mut second = [0.; HIDDEN];
        let mut output = [0.; OUTPUTS];
        self.layers[0].forward(observation, &mut first, true)?;
        self.layers[1].forward(&first, &mut second, true)?;
        self.layers[2].forward(&second, &mut output, false)?;
        if output.iter().any(|v| !v.is_finite()) {
            return Err(PolicyError("inference overflow".into()));
        }
        Ok(output)
    }
}

impl Layer {
    fn forward(&self, input: &[f32], output: &mut [f32], relu: bool) -> Result<(), PolicyError> {
        for (i, out) in output.iter_mut().enumerate() {
            let row = &self.weights[i * self.inputs..(i + 1) * self.inputs];
            let value = row
                .iter()
                .zip(input)
                .fold(self.biases[i], |sum, (w, x)| sum + w * x);
            // f32::max maps NaN to its other operand. Reject before ReLU can
            // conceal intermediate overflow from otherwise finite inputs.
            if !value.is_finite() {
                return Err(PolicyError("inference overflow".into()));
            }
            *out = if relu { value.max(0.) } else { value };
        }
        Ok(())
    }
}

/// NES button bits: A, B, Select, Start, Up, Down, Left, Right.
/// Factors are horizontal(3), vertical(3), A(2), B(2); ties choose neutral/off.
pub fn decode(logits: &[f32]) -> Result<u8, PolicyError> {
    if logits.len() != OUTPUTS || logits.iter().any(|v| !v.is_finite()) {
        return Err(PolicyError("expected ten finite action logits".into()));
    }
    fn argmax(values: &[f32]) -> usize {
        (1..values.len()).fold(0, |best, i| if values[i] > values[best] { i } else { best })
    }
    Ok([0, 0x40, 0x80][argmax(&logits[..3])]
        | [0, 0x10, 0x20][argmax(&logits[3..6])]
        | [0, 1][argmax(&logits[6..8])]
        | [0, 2][argmax(&logits[8..10])])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ties_are_neutral_and_directions_never_conflict() {
        assert_eq!(decode(&[0.; 10]).unwrap(), 0);
        assert_eq!(
            decode(&[0., 1., 2., 0., 2., 1., 0., 1., 0., 1.]).unwrap(),
            0x93
        );
        assert!(decode(&[f32::NAN; 10]).is_err());
    }
    #[test]
    fn rejects_bad_containers_and_oversize_before_parsing() {
        assert!(Policy::load(b"not tensors").is_err());
        assert!(Policy::load(&vec![0; MAX_WEIGHT_BYTES + 1]).is_err());
    }

    #[test]
    fn relu_cannot_hide_nonfinite_intermediate_results() {
        let layer = Layer {
            weights: vec![f32::MAX, -f32::MAX],
            biases: vec![0.],
            inputs: 2,
        };
        assert!(
            layer
                .forward(&[f32::MAX, f32::MAX], &mut [0.], true)
                .is_err()
        );
    }
}
