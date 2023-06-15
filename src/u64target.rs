use std::marker::PhantomData;

use plonky2::{
    field::extension::Extendable,
    hash::hash_types::RichField,
    iop::{
        target::BoolTarget,
        witness::{PartialWitness, WitnessWrite},
    },
    plonk::circuit_builder::CircuitBuilder,
};

#[derive(Clone, Debug)]
pub struct U64Target<F, const D: usize> {
    pub bits: Vec<BoolTarget>,
    _phantom: PhantomData<F>,
}

impl<F, const D: usize> U64Target<F, D>
where
    F: RichField + Extendable<D>,
{
    pub fn new(builder: &mut CircuitBuilder<F, D>) -> Self {
        let mut result = vec![];
        for _ in 0..64 {
            result.push(builder.add_virtual_bool_target_safe());
        }
        Self {
            bits: result,
            _phantom: PhantomData,
        }
    }

    pub fn from(bits: Vec<BoolTarget>) -> Self {
        assert_eq!(bits.len(), 64);
        Self {
            bits,
            _phantom: PhantomData,
        }
    }

    pub fn set_witness(&self, bits: Vec<bool>, pw: &mut PartialWitness<F>) {
        for i in 0..64 {
            pw.set_bool_target(self.bits[i], bits[i]);
        }
    }

    pub fn constant(x: u64, builder: &mut CircuitBuilder<F, D>) -> Self {
        let mut result = vec![];
        let x_bits = u64_to_bits(x);
        for i in 0..64 {
            result.push(builder.constant_bool(x_bits[i]));
        }
        Self {
            bits: result,
            _phantom: PhantomData,
        }
    }

    pub fn connect(&self, other: &Self, builder: &mut CircuitBuilder<F, D>) {
        for i in 0..64 {
            builder.connect(self.bits[i].target, other.bits[i].target);
        }
    }

    pub fn to_bits(&self, builder: &mut CircuitBuilder<F, D>) -> Vec<BoolTarget> {
        let output = Self::new(builder);
        self.connect(&output, builder);
        output.bits
    }

    pub fn xor(&self, other: &Self, builder: &mut CircuitBuilder<F, D>) -> Self {
        let mut result = vec![];
        for i in 0..64 {
            let xor_target = xor_circuit(self.bits[i], other.bits[i], builder);
            result.push(xor_target);
        }
        Self {
            bits: result,
            _phantom: PhantomData,
        }
    }

    pub fn xor_const(&self, other: u64, builder: &mut CircuitBuilder<F, D>) -> Self {
        let other_bits = u64_to_bits(other);
        let mut result = vec![];
        for i in 0..64 {
            let xor_target = xor_const_circuit(self.bits[i], other_bits[i], builder);
            result.push(xor_target);
        }
        Self {
            bits: result,
            _phantom: PhantomData,
        }
    }

    /* Rotate left by n
     * Note that the input parameter n is constant. It is not necessary to make n a constant target or public input,
     * because different n generates a different circuit.
     */
    pub fn rotl(&self, n: usize) -> Self {
        let rotate = rotate_u64(n);
        let mut output = vec![];
        for i in 0..64 {
            output.push(self.bits[rotate[i]]);
        }

        Self {
            bits: output,
            _phantom: PhantomData,
        }
    }

    pub fn and(&self, other: &Self, builder: &mut CircuitBuilder<F, D>) -> Self {
        let mut result = vec![];
        for i in 0..64 {
            result.push(builder.and(self.bits[i], other.bits[i]));
        }
        Self {
            bits: result,
            _phantom: PhantomData,
        }
    }

    pub fn not(&self, builder: &mut CircuitBuilder<F, D>) -> Self {
        let mut result = vec![];
        for i in 0..64 {
            result.push(builder.not(self.bits[i]));
        }
        Self {
            bits: result,
            _phantom: PhantomData,
        }
    }

    /// Calculate `self & !other`.
    pub fn and_not(&self, other: &Self, builder: &mut CircuitBuilder<F, D>) -> Self {
        let mut result = vec![];
        for i in 0..64 {
            // x(1 - y) = x - xy
            result.push(BoolTarget::new_unsafe(builder.arithmetic(
                F::NEG_ONE,
                F::ONE,
                self.bits[i].target,
                other.bits[i].target,
                self.bits[i].target,
            )));
        }
        Self {
            bits: result,
            _phantom: PhantomData,
        }
    }
}

pub fn xor_circuit<F, const D: usize>(
    a: BoolTarget,
    b: BoolTarget,
    builder: &mut CircuitBuilder<F, D>,
) -> BoolTarget
where
    F: RichField + Extendable<D>,
{
    // a = 0, b = 0 => 0
    // a = 1, b = 0 => 1
    // a = 0, b = 1 => 1
    // a = 1, b = 1 => 0
    // xor(a, b) = a*(1-b) + (1-a)*b = a + b - 2*ab
    let b_minus_2ab = builder.arithmetic(-F::TWO, F::ONE, a.target, b.target, b.target);
    let a_plus_b_minus_2ab = builder.add(a.target, b_minus_2ab);
    // let c = builder.add_virtual_bool_target_safe();
    // builder.connect(c.target, a_plus_b_neg_two_ab);

    BoolTarget::new_unsafe(a_plus_b_minus_2ab)
}

pub fn xor_const_circuit<F, const D: usize>(
    a: BoolTarget,
    b: bool,
    builder: &mut CircuitBuilder<F, D>,
) -> BoolTarget
where
    F: RichField + Extendable<D>,
{
    // b = 0 => xor(a, b) = a
    // b = 1 => xor(a, b) = 1 - a = not(a)
    if b {
        builder.not(a)
    } else {
        a
    }
}

// reffered to https://github.com/polymerdao/plonky2-sha256
/// 0 -> [0, 1, 2, ..., 63]
/// 1 -> [63, 0, 1, ..., 62]
/// 63 -> [1, 2, ..., 63, 0]
fn rotate_u64(y: usize) -> Vec<usize> {
    let mut res = Vec::new();
    for i in 64 - y..64 {
        res.push(i);
    }
    for i in 0..64 - y {
        res.push(i);
    }
    res
}

pub fn from_bits_to_u64(bools: &[bool]) -> u64 {
    let mut result: u64 = 0;
    let mut shift = 0;
    for &bit in bools {
        if bit {
            result |= 1 << shift;
        }
        shift += 1;
        if shift == 64 {
            break;
        }
    }
    result
}

pub fn u64_to_bits(num: u64) -> Vec<bool> {
    let mut result = Vec::with_capacity(64);
    let mut n = num;
    for _ in 0..64 {
        result.push(n & 1 == 1);
        n >>= 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use plonky2::{
        field::goldilocks_field::GoldilocksField,
        plonk::{circuit_data::CircuitConfig, config::PoseidonGoldilocksConfig},
    };
    use rand::Rng;

    type F = GoldilocksField;
    type C = PoseidonGoldilocksConfig;
    const D: usize = 2;

    use super::*;

    #[test]
    fn test_xor() -> Result<()> {
        let rng = &mut rand::thread_rng();
        let a: u64 = rng.gen();
        let b: u64 = rng.gen();
        let expected_output = a ^ b;

        let config = CircuitConfig::standard_recursion_config();
        let mut builder = CircuitBuilder::<F, D>::new(config);
        let a_t = U64Target::constant(a, &mut builder);
        let b_t = U64Target::constant(b, &mut builder);
        let a_xor_b_t = a_t.xor(&b_t, &mut builder);

        let mut pw = PartialWitness::<F>::new();
        a_xor_b_t.set_witness(u64_to_bits(expected_output), &mut pw);

        let data = builder.build::<C>();
        let proof = data.prove(pw)?;
        data.verify(proof)?;
        Ok(())
    }

    #[test]
    fn test_rotl() -> Result<()> {
        let rng = &mut rand::thread_rng();
        let a: u64 = rng.gen();
        let n: usize = rng.gen_range(0..=63);
        let expected_output = a.rotate_left(n as u32);

        let config = CircuitConfig::standard_recursion_config();
        let mut builder = CircuitBuilder::<F, D>::new(config);
        let a_t = U64Target::constant(a, &mut builder);
        let a_rotl_t = a_t.rotl(n);

        let mut pw = PartialWitness::<F>::new();
        a_rotl_t.set_witness(u64_to_bits(expected_output), &mut pw);

        let data = builder.build::<C>();
        let proof = data.prove(pw)?;
        data.verify(proof)?;
        Ok(())
    }
}
