use ark_ec::{ModelParameters, SWModelParameters, TEModelParameters};
use ark_ff::{Field, PrimeField};
use ark_r1cs_std::bits::boolean::Boolean;
use ark_r1cs_std::bits::uint8::UInt8;
use ark_r1cs_std::fields::fp::FpVar;
use ark_r1cs_std::fields::{FieldOpsBounds, FieldVar};
use ark_r1cs_std::groups::curves::short_weierstrass::{
    AffineVar as SWAffineVar, ProjectiveVar as SWProjectiveVar,
};
use ark_r1cs_std::groups::curves::twisted_edwards::AffineVar as TEAffineVar;
use ark_r1cs_std::ToConstraintFieldGadget;
use ark_relations::r1cs::SynthesisError;
use ark_std::vec::Vec;

/// An interface for objects that can be absorbed by a `CryptographicSpongeVar`.
pub trait AbsorbableGadget<F: PrimeField> {
    /// Converts the object into field elements that can be absorbed by a `CryptographicSpongeVar`.
    fn to_sponge_field_elements(&self) -> Result<Vec<FpVar<F>>, SynthesisError>;

    /// Specifies the conversion into a list of field elements for a batch.
    fn batch_to_sponge_field_elements(batch: &[Self]) -> Result<Vec<FpVar<F>>, SynthesisError>
    where
        Self: Sized,
    {
        let mut output = Vec::new();
        for absorbable in batch {
            output.append(&mut absorbable.to_sponge_field_elements()?);
        }

        Ok(output)
    }
}

impl<F: PrimeField> AbsorbableGadget<F> for UInt8<F> {
    fn to_sponge_field_elements(&self) -> Result<Vec<FpVar<F>>, SynthesisError> {
        vec![self.clone()].to_constraint_field()
    }

    fn batch_to_sponge_field_elements(batch: &[Self]) -> Result<Vec<FpVar<F>>, SynthesisError> {
        let mut bytes = UInt8::constant_vec((batch.len() as u64).to_le_bytes().as_ref());
        bytes.extend_from_slice(batch);
        bytes.to_constraint_field()
    }
}

impl<F: PrimeField> AbsorbableGadget<F> for Boolean<F> {
    fn to_sponge_field_elements(&self) -> Result<Vec<FpVar<F>>, SynthesisError> {
        Ok(vec![FpVar::from(self.clone())])
    }
}

impl<F: PrimeField> AbsorbableGadget<F> for FpVar<F> {
    fn to_sponge_field_elements(&self) -> Result<Vec<FpVar<F>>, SynthesisError> {
        Ok(vec![self.clone()])
    }

    fn batch_to_sponge_field_elements(batch: &[Self]) -> Result<Vec<FpVar<F>>, SynthesisError> {
        Ok(batch.to_vec())
    }
}

macro_rules! impl_absorbable_group {
    ($group:ident, $params:ident) => {
        impl<P, F> AbsorbableGadget<<P::BaseField as Field>::BasePrimeField> for $group<P, F>
        where
            P: $params,
            F: FieldVar<P::BaseField, <P::BaseField as Field>::BasePrimeField>,
            for<'a> &'a F: FieldOpsBounds<'a, P::BaseField, F>,
            F: ToConstraintFieldGadget<<P::BaseField as Field>::BasePrimeField>,
        {
            fn to_sponge_field_elements(
                &self,
            ) -> Result<Vec<FpVar<<P::BaseField as Field>::BasePrimeField>>, SynthesisError> {
                self.to_constraint_field()
            }
        }
    };
}

impl_absorbable_group!(TEAffineVar, TEModelParameters);
impl_absorbable_group!(SWAffineVar, SWModelParameters);

impl<P, F> AbsorbableGadget<<P::BaseField as Field>::BasePrimeField> for SWProjectiveVar<P, F>
where
    P: SWModelParameters,
    F: FieldVar<P::BaseField, <P::BaseField as Field>::BasePrimeField>,
    for<'a> &'a F: FieldOpsBounds<'a, P::BaseField, F>,
    F: ToConstraintFieldGadget<<P::BaseField as Field>::BasePrimeField>,
{
    fn to_sponge_field_elements(
        &self,
    ) -> Result<
        Vec<FpVar<<<P as ModelParameters>::BaseField as Field>::BasePrimeField>>,
        SynthesisError,
    > {
        self.to_affine()?.to_sponge_field_elements()
    }
}

impl<F: PrimeField, A: AbsorbableGadget<F>> AbsorbableGadget<F> for &[A] {
    fn to_sponge_field_elements(&self) -> Result<Vec<FpVar<F>>, SynthesisError> {
        A::batch_to_sponge_field_elements(self)
    }
}

impl<F: PrimeField, A: AbsorbableGadget<F>> AbsorbableGadget<F> for Vec<A> {
    fn to_sponge_field_elements(&self) -> Result<Vec<FpVar<F>>, SynthesisError> {
        self.as_slice().to_sponge_field_elements()
    }
}

impl<F: PrimeField, A: AbsorbableGadget<F>> AbsorbableGadget<F> for Option<A> {
    fn to_sponge_field_elements(&self) -> Result<Vec<FpVar<F>>, SynthesisError> {
        let mut output = vec![FpVar::from(Boolean::constant(self.is_some()))];
        if let Some(absorbable) = self.as_ref() {
            output.append(&mut absorbable.to_sponge_field_elements()?);
        }
        Ok(output)
    }
}

impl<F: PrimeField, A: AbsorbableGadget<F>> AbsorbableGadget<F> for &A {
    fn to_sponge_field_elements(&self) -> Result<Vec<FpVar<F>>, SynthesisError> {
        (*self).to_sponge_field_elements()
    }
}

/// Individually absorbs each element in a comma-separated list of [`Absorbable`]s into a sponge.
/// Format is `absorb!(s, a_0, a_1, ..., a_n)`, where `s` is a mutable reference to a sponge
/// and each `a_i` implements `AbsorbableVar`.
#[macro_export]
macro_rules! absorb_gadget {
    ($sponge:expr, $($absorbable:expr),+ ) => {
        $(
            CryptographicSpongeVar::absorb($sponge, &$absorbable)?;
        )+
    };
}

/// Quickly convert a list of different [`Absorbable`]s into sponge field elements.
#[macro_export]
macro_rules! collect_sponge_field_elements_gadget {
    ($head:expr $(, $tail:expr)* ) => {
        {
            let mut output = AbsorbableGadget::to_sponge_field_elements(&$head)?;
            $(
                output.append(&mut AbsorbableGadget::to_sponge_field_elements(&$tail)?);
            )*

            Ok(output)
        }
    };
}
