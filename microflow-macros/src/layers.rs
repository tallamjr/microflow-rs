use flatbuffers::{ForwardsUOffset, Vector};
use nalgebra::{convert_ref, DMatrix};
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};

use crate::matrix::TokenMatrix;
use crate::tensor::TokenTensor;
use crate::tflite_flatbuffers::tflite::{
    ActivationFunctionType, Buffer, BuiltinOperator, Operator, Tensor,
};

pub const SUPPORTED_OPS: [BuiltinOperator; 3] = [
    BuiltinOperator::QUANTIZE,
    BuiltinOperator::DEQUANTIZE,
    BuiltinOperator::FULLY_CONNECTED,
];

pub struct FullyConnected {
    pub(crate) weights: TokenTensor<i8>,
    pub(crate) output: TokenTensor<i8>,
    pub(crate) activation: ActivationFunctionType,
    pub(crate) constants: (i8, TokenMatrix<f32>, f32, TokenMatrix<i32>, i32),
}

impl FullyConnected {
    pub fn new(
        operator: Operator,
        tensors: Vector<ForwardsUOffset<Tensor>>,
        buffers: Vector<ForwardsUOffset<Buffer>>,
    ) -> Self {
        let inputs = operator.inputs().unwrap();
        let input = TokenTensor::new_empty(tensors.get(inputs.get(0) as usize));
        let weights = TokenTensor::new_with_data(tensors.get(inputs.get(1) as usize), buffers);
        let biases: TokenTensor<i32> =
            TokenTensor::new_with_data(tensors.get(inputs.get(2) as usize), buffers);
        let output =
            TokenTensor::new_empty(tensors.get(operator.outputs().unwrap().get(0) as usize));
        let activation = operator
            .builtin_options_as_fully_connected_options()
            .unwrap()
            .fused_activation_function();
        let constants = Self::preprocess(&input, &weights, &biases, &output);
        Self {
            weights,
            output,
            activation,
            constants,
        }
    }

    pub fn preprocess(
        input: &TokenTensor<i8>,
        weights: &TokenTensor<i8>,
        biases: &TokenTensor<i32>,
        output: &TokenTensor<i8>,
    ) -> (i8, TokenMatrix<f32>, f32, TokenMatrix<i32>, i32) {
        (
            output.zero_point,
            (biases.scale / output.scale
                * biases.matrix.add_scalar(-biases.zero_point).cast::<f32>())
            .into(),
            input.scale * weights.scale / output.scale,
            DMatrix::from_rows(&[(input.zero_point as i32
                * convert_ref::<DMatrix<i8>, DMatrix<i32>>(&weights.matrix).row_sum())])
            .into(),
            input.matrix.shape().1 as i32 * input.zero_point as i32 * weights.zero_point as i32,
        )
    }
}

impl ToTokens for FullyConnected {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let weights = &self.weights;
        let output_scale = &self.output.scale;
        let output_zero_point = &self.output.zero_point;
        let (constant_0, constant_1, constant_2, constant_3, constant_4) = &self.constants;

        let activation = match self.activation {
            ActivationFunctionType::RELU => quote! { microflow::activations::Activation::RELU },
            ActivationFunctionType::NONE => quote! { microflow::activations::Activation::NONE },
            _ => unimplemented!(),
        };
        let constants = quote! {
            (#constant_0, #constant_1, #constant_2, #constant_3, #constant_4)
        };

        let output = quote! {
            let output = microflow::ops::fully_connected(output, #weights, #output_scale, #output_zero_point, #activation, #constants);
        };
        output.to_tokens(tokens);
    }
}
