use crate::model::ParsingContext;
use crate::pb::NodeProto;
use tract_core::ops::cnn::KernelFormat;
use tract_hir::internal::*;
use tract_hir::ops::cnn::PaddingSpec;
use tract_hir::ops::nn::DataFormat;

pub fn conv_transpose(
    _ctx: &ParsingContext,
    node: &NodeProto,
) -> TractResult<(Box<dyn InferenceOp>, Vec<String>)> {
    let padding_spec = super::pad(node)?;
    Ok((expand(ConvTranspose::new(padding_spec)), vec![]))
}

#[derive(Debug, Clone, new, Default, Hash)]
pub struct ConvTranspose {
    padding_spec: PaddingSpec,
}

impl_dyn_hash!(ConvTranspose);

impl Expansion for ConvTranspose {
    fn name(&self) -> Cow<str> {
        "ConvTranspose".into()
    }

    op_onnx!();

    fn rules<'r, 'p: 'r, 's: 'r>(
        &'s self,
        s: &mut Solver<'r>,
        inputs: &'p [TensorProxy],
        outputs: &'p [TensorProxy],
    ) -> InferenceResult {
        check_input_arity(inputs, 2)?;
        check_output_arity(outputs, 1)?;
        s.equals(&inputs[0].datum_type, &outputs[0].datum_type)?;
        s.equals(&inputs[0].datum_type, &inputs[1].datum_type)?;
        s.equals(&inputs[0].rank, &inputs[1].rank)?;
        s.equals(&inputs[0].rank, &outputs[0].rank)?;
        s.equals(&outputs[0].shape[0], &inputs[0].shape[0])?; // N
        s.equals(&inputs[0].shape[1], &inputs[1].shape[0])?; // O
        s.equals(&outputs[0].shape[1], &inputs[1].shape[1])?; // I

        s.given_2(&inputs[0].shape, &inputs[1].shape, move |s, x_shape, w_shape| {
            if let Ok(w_shape) =
                w_shape.iter().map(|d| d.to_usize()).collect::<TractResult<TVec<usize>>>()
            {
                let y_shape = tract_core::ops::cnn::deconv::output_shape(
                    &DataFormat::NCHW,
                    &KernelFormat::OIHW,
                    &self.padding_spec,
                    &*w_shape,
                    &x_shape,
                )?;
                s.equals(&outputs[0].shape, y_shape)?;
            }
            Ok(())
        })?;
        Ok(())
    }

    fn wire(
        &self,
        prefix: &str,
        target: &mut TypedModel,
        inputs: &[OutletId],
    ) -> TractResult<TVec<OutletId>> {
        if let Some(k) = &target.outlet_fact(inputs[1])?.konst {
            target.wire_node(
                prefix,
                tract_core::ops::cnn::DeconvUnary::new(
                    DataFormat::NCHW,
                    KernelFormat::OIHW,
                    self.padding_spec.clone(),
                    k.clone(),
                ),
                &[inputs[0]],
            )
        } else {
            bail!("Kernel values are expected to be constant.")
        }
    }
}
