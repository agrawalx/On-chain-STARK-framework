#![no_main]
#![no_std]
extern crate alloc;
use static_alloc::Bump;

#[global_allocator]
static ALLOCATOR: Bump<[u8; 64 * 1024]> = Bump::uninit();
use uapi::{HostFn, HostFnImpl as api, ReturnFlags};
use winterfell::{verify, AcceptableOptions};
use core::slice;
use core::result::Result::{Ok, Err};
pub use core::{fmt, iter, mem, ops};
use core::panic::PanicInfo;
use alloc::vec::Vec; 
use alloc::vec; 

use winterfell::{
    math::{fields::f128::BaseElement, FieldElement, ToElements},
    crypto::{hashers::Blake3_256, DefaultRandomCoin, MerkleTree}, 
    Air, AirContext, Assertion, EvaluationFrame, ProofOptions, TraceInfo, 
    TransitionConstraintDegree
};

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe {
        core::arch::asm!("unimp");
        core::hint::unreachable_unchecked();
    }
}

#[no_mangle]
#[polkavm_derive::polkavm_export]
pub extern "C" fn deploy() {}

#[no_mangle]
#[polkavm_derive::polkavm_export]
pub extern "C" fn call() {
    // Expected ABI: verify(bytes calldata proof, bytes calldata publicInputs) returns (bool)
    
    // Read input lengths (first 32 bytes = proof length, second 32 = inputs length)
    let mut len_buf = [0u8; 64];
    api::call_data_copy(&mut len_buf, 0);
    
    let proof_len = u32::from_be_bytes(len_buf[0..4].try_into().unwrap()) as usize;
    let inputs_len = u32::from_be_bytes(len_buf[32..36].try_into().unwrap()) as usize;
    
    // Read actual data (starts at offset 64)
    let mut input_data = vec![0u8; proof_len + inputs_len];
    api::call_data_copy(&mut input_data, 64);
    
    let proof_bytes = &input_data[..proof_len];
    let pub_inputs_bytes = &input_data[proof_len..];
    
    // Verification
    let result = verify_stark_proof(proof_bytes, pub_inputs_bytes);
    
    // Return bool (padded to 32 bytes)
    let mut output = [0u8; 32];
    output[31] = result as u8;
    api::return_value(ReturnFlags::empty(), &output);
}

fn verify_stark_proof(proof_bytes: &[u8], pub_inputs_bytes: &[u8]) -> bool {
    let proof = match winterfell::Proof::from_bytes(proof_bytes) {
        Ok(p) => p,
        Err(_) => return false,
    };
    
    // let pub_inputs = match bincode::deserialize(pub_inputs_bytes) {
    //     Ok(i) => i,
    //     Err(_) => return false,
    // };
    // Perform verification
    // Secret parameters (not revealed in proof)
    let slope = BaseElement::new(3);        // m = 3
    let intercept = BaseElement::new(7);    // b = 7
    
    // Public sample data points
    let sample_x = vec![
        BaseElement::new(1),   // x = 1
        BaseElement::new(2),   // x = 2  
        BaseElement::new(4),   // x = 4
        BaseElement::new(5),   // x = 5
    ];
    
    let sample_y = vec![
        BaseElement::new(10),  // y = 3*1 + 7 = 10
        BaseElement::new(13),  // y = 3*2 + 7 = 13
        BaseElement::new(19),  // y = 3*4 + 7 = 19
        BaseElement::new(22),  // y = 3*5 + 7 = 22
    ];
    
    // Target prediction
    let target_x = BaseElement::new(6);
    let expected_y = slope * target_x + intercept; // 3*6 + 7 = 25

    let pub_inputs = LinearRegressionInputs {
        x_value: target_x,
        predicted_y: expected_y,
        sample_x_values: sample_x,
        sample_y_values: sample_y,
    };

    verify::<
        LinearRegressionAir,
        Blake3_256<BaseElement>,
        DefaultRandomCoin<Blake3_256<BaseElement>>,
        MerkleTree<Blake3_256<BaseElement>>
    >(proof, pub_inputs, &AcceptableOptions::MinConjecturedSecurity(95)).is_ok()
}

#[derive(Clone, Debug)]
pub struct LinearRegressionInputs {
    pub x_value: BaseElement,
    pub predicted_y: BaseElement,
    pub sample_x_values: Vec<BaseElement>,
    pub sample_y_values: Vec<BaseElement>,
}

impl ToElements<BaseElement> for LinearRegressionInputs {
    fn to_elements(&self) -> Vec<BaseElement> {
        let mut elements = vec![self.x_value, self.predicted_y];
        elements.extend(&self.sample_x_values);
        elements.extend(&self.sample_y_values);
        elements
    }
}

pub struct LinearRegressionAir {
    context: AirContext<BaseElement>,
    x_value: BaseElement,
    predicted_y: BaseElement,
    sample_x_values: Vec<BaseElement>,
    sample_y_values: Vec<BaseElement>,
    num_samples: usize,
}

impl Air for LinearRegressionAir {
    type BaseField = BaseElement;
    type PublicInputs = LinearRegressionInputs;

    fn new(trace_info: TraceInfo, pub_inputs: LinearRegressionInputs, options: ProofOptions) -> Self {
        // Our trace has 4 columns: slope (m), intercept (b), x_input, y_output
        assert_eq!(4, trace_info.width());
        
        let num_samples = pub_inputs.sample_x_values.len();
        assert_eq!(num_samples, pub_inputs.sample_y_values.len(), "Sample arrays must have equal length");
        
        // Constraints:
        // 1. Linear relationship: y = mx + b (degree 2: multiplication of slope * x)
        // 2. Slope consistency (degree 1: next_slope - slope = 0)
        // 3. Intercept consistency (degree 1: next_intercept - intercept = 0)
        let degrees = vec![
            TransitionConstraintDegree::new(2), // Linear constraint: y - mx - b = 0
            TransitionConstraintDegree::new(1), // Slope consistency
            TransitionConstraintDegree::new(1), // Intercept consistency
        ];
        
        // Assertions for sample points and prediction
        let num_assertions = 2 * num_samples + 2; // x,y pairs for samples + prediction x,y
        
        LinearRegressionAir {
            context: AirContext::new(trace_info, degrees, num_assertions, options),
            x_value: pub_inputs.x_value,
            predicted_y: pub_inputs.predicted_y,
            sample_x_values: pub_inputs.sample_x_values,
            sample_y_values: pub_inputs.sample_y_values,
            num_samples,
        }
    }

    fn evaluate_transition<E: FieldElement + From<Self::BaseField>>(
        &self,
        frame: &EvaluationFrame<E>,
        _periodic_values: &[E],
        result: &mut [E],
    ) {
        // Extract current state: [slope, intercept, x, y]
        let slope = frame.current()[0];
        let intercept = frame.current()[1];
        let x = frame.current()[2];
        let y = frame.current()[3];
        
        // Extract next state
        let next_slope = frame.next()[0];
        let next_intercept = frame.next()[1];
        
        // Constraint 1: Linear relationship y = mx + b
        // This ensures y - mx - b = 0
        result[0] = y - slope * x - intercept;
        
        // Constraint 2: Slope must remain constant across all steps
        result[1] = next_slope - slope;
        
        // Constraint 3: Intercept must remain constant across all steps  
        result[2] = next_intercept - intercept;
    }

    fn get_assertions(&self) -> Vec<Assertion<Self::BaseField>> {
        let mut assertions = Vec::new();
        
        // Assert that each sample point is correctly represented in the trace
        for i in 0..self.num_samples {
            // Assert x value at step i
            assertions.push(Assertion::single(2, i, self.sample_x_values[i]));
            // Assert y value at step i  
            assertions.push(Assertion::single(3, i, self.sample_y_values[i]));
        }
        
        // Assert the final prediction at the prediction step
        let prediction_step = self.num_samples;
        assertions.push(Assertion::single(2, prediction_step, self.x_value));
        assertions.push(Assertion::single(3, prediction_step, self.predicted_y));
        
        assertions
    }

    fn context(&self) -> &AirContext<Self::BaseField> {
        &self.context
    }
}

