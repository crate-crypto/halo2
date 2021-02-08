//! The Grain LFSR in self-shrinking mode, as used by Poseidon.

use std::marker::PhantomData;

use bitvec::prelude::*;

use crate::arithmetic::FieldExt;

const STATE: usize = 80;

#[derive(Debug, Clone, Copy)]
pub(super) enum FieldType {
    /// GF(2^n)
    #[allow(dead_code)]
    Binary,
    /// GF(p)
    PrimeOrder,
}

impl FieldType {
    fn tag(&self) -> u8 {
        match self {
            FieldType::Binary => 0,
            FieldType::PrimeOrder => 1,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum SboxType {
    /// x^alpha
    Pow,
    /// x^(-1)
    #[allow(dead_code)]
    Inv,
}

impl SboxType {
    fn tag(&self) -> u8 {
        match self {
            SboxType::Pow => 0,
            SboxType::Inv => 1,
        }
    }
}

pub(super) struct Grain<F: FieldExt> {
    state: bitarr!(for 80, in Msb0, u8),
    next_bit: usize,
    _field: PhantomData<F>,
}

impl<F: FieldExt> Grain<F> {
    pub(super) fn new(sbox: SboxType, t: u16, r_f: u16, r_p: u16) -> Self {
        // Initialize the LFSR state.
        let mut state = bitarr![Msb0, u8; 1; STATE];
        let mut set_bits = |offset: usize, len, value| {
            // Poseidon sets initial state bits in MSB order.
            for i in (0..len).rev() {
                *state.get_mut(offset + i).unwrap() = (value >> i) & 1 != 0;
            }
        };
        set_bits(0, 2, FieldType::PrimeOrder.tag() as u16);
        set_bits(2, 4, sbox.tag() as u16);
        set_bits(6, 12, F::NUM_BITS as u16);
        set_bits(18, 12, t);
        set_bits(30, 10, r_f);
        set_bits(40, 10, r_p);

        let mut grain = Grain {
            state,
            next_bit: STATE,
            _field: PhantomData::default(),
        };

        // Discard the first 160 bits.
        for _ in 0..20 {
            grain.load_next_8_bits();
            grain.next_bit = STATE;
        }

        grain
    }

    fn load_next_8_bits(&mut self) {
        let mut new_bits = 0u8;
        for i in 0..8 {
            new_bits |= ((self.state[i + 62]
                ^ self.state[i + 51]
                ^ self.state[i + 38]
                ^ self.state[i + 23]
                ^ self.state[i + 13]
                ^ self.state[i]) as u8)
                << i;
        }
        self.state.rotate_right(8);
        self.next_bit -= 8;
        for i in 0..8 {
            *self.state.get_mut(i + STATE - 8).unwrap() = (new_bits >> i) & 1 != 0;
        }
    }

    fn get_next_bit(&mut self) -> bool {
        if self.next_bit == STATE {
            self.load_next_8_bits();
        }
        let ret = self.state[self.next_bit];
        self.next_bit += 1;
        ret
    }

    /// Returns the next field element from this Grain instantiation.
    pub(super) fn next_field_element(&mut self) -> F {
        // Loop until we get an element in the field.
        loop {
            let mut bytes = F::Repr::default();

            // Fill the repr with bits in little-endian order.
            let view = bytes.as_mut();
            for (i, bit) in self.take(F::NUM_BITS as usize).enumerate() {
                view[i / 8] |= if bit { 1 << (i % 8) } else { 0 };
            }

            if let Some(f) = F::from_repr(bytes) {
                break f;
            }
        }
    }
}

impl<F: FieldExt> Iterator for Grain<F> {
    type Item = bool;

    fn next(&mut self) -> Option<Self::Item> {
        // Evaluate bits in pairs:
        // - If the first bit is a 1, output the second bit.
        // - If the first bit is a 0, discard the second bit.
        while !self.get_next_bit() {
            self.get_next_bit();
        }
        Some(self.get_next_bit())
    }
}

#[cfg(test)]
mod tests {
    use super::{Grain, SboxType};
    use crate::pasta::Fp;

    #[test]
    fn grain() {
        let mut grain = Grain::<Fp>::new(SboxType::Pow, 3, 8, 56);
        let f = grain.next_field_element();
    }
}