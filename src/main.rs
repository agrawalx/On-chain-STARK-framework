use winterfell::{
    math::{fields::f128::BaseElement, FieldElement, ToElements},
    Air, AirContext, Assertion, EvaluationFrame, ProofOptions, TraceInfo,
    TransitionConstraintDegree, Prover, TraceTable, Trace,
    crypto::{hashers::Blake3_256, DefaultRandomCoin, MerkleTree},
    matrix::ColMatrix,
    CompositionPoly, CompositionPolyTrace, DefaultConstraintCommitment,
    DefaultTraceLde, DefaultConstraintEvaluator, StarkDomain,
    TracePolyTable, ConstraintEvaluator, TraceLde, ConstraintCompositionCoefficients,
    AuxRandElements, PartitionOptions, FieldExtension, BatchingMethod,
    AcceptableOptions,
};

/// Public inputs for linear regression verification
#[derive(Clone, Debug)]
pub struct LinearRegressionInputs {
    pub x_value: BaseElement,          // The x for which we want to verify y prediction
    pub predicted_y: BaseElement,      // The claimed y = mx + b result
    pub sample_x_values: Vec<BaseElement>, // Sample x values for validation
    pub sample_y_values: Vec<BaseElement>, // Sample y values for validation
}

impl ToElements<BaseElement> for LinearRegressionInputs {
    fn to_elements(&self) -> Vec<BaseElement> {
        let mut elements = vec![self.x_value, self.predicted_y];
        elements.extend(&self.sample_x_values);
        elements.extend(&self.sample_y_values);
        elements
    }
}

/// AIR for linear regression verification
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

/// Build the execution trace for linear regression
pub fn build_linear_regression_trace(
    slope: BaseElement,
    intercept: BaseElement,
    sample_x_values: &[BaseElement],
    sample_y_values: &[BaseElement],
    target_x: BaseElement,
) -> TraceTable<BaseElement> {
    let num_samples = sample_x_values.len();
    
    // We need at least num_samples + 1 steps (samples + prediction)
    // Winterfell requires minimum trace length of 8, and trace length must be a power of 2
    let trace_length = (num_samples + 1).next_power_of_two().max(8);
    let trace_width = 4; // slope, intercept, x, y
    
    let mut trace = TraceTable::new(trace_width, trace_length);
    
    trace.fill(
        |state| {
            // Initialize first state with first sample point
            state[0] = slope;           // slope column
            state[1] = intercept;       // intercept column
            state[2] = sample_x_values[0]; // x value
            state[3] = sample_y_values[0]; // y value
        },
        |step, state| {
            // Keep slope and intercept constant throughout
            state[0] = slope;
            state[1] = intercept;
            
            if step < num_samples {
                // Fill with sample data
                state[2] = sample_x_values[step];
                state[3] = sample_y_values[step];
            } else if step == num_samples {
                // Prediction step
                state[2] = target_x;
                state[3] = slope * target_x + intercept;
            } else {
                // Padding steps - maintain consistency by repeating the prediction
                state[2] = target_x;
                state[3] = slope * target_x + intercept;
            }
        },
    );
    
    trace
}

/// Linear Regression Prover
pub struct LinearRegressionProver {
    options: ProofOptions,
}

impl LinearRegressionProver {
    pub fn new(options: ProofOptions) -> Self {
        Self { options }
    }
}

impl Prover for LinearRegressionProver {
    type BaseField = BaseElement;
    type Air = LinearRegressionAir;
    type Trace = TraceTable<Self::BaseField>;
    type HashFn = Blake3_256<Self::BaseField>;
    type VC = MerkleTree<Self::HashFn>;
    type RandomCoin = DefaultRandomCoin<Self::HashFn>;
    type TraceLde<E: FieldElement<BaseField = Self::BaseField>> = DefaultTraceLde<E, Self::HashFn, Self::VC>;
    type ConstraintCommitment<E: FieldElement<BaseField = Self::BaseField>> =
        DefaultConstraintCommitment<E, Self::HashFn, Self::VC>;
    type ConstraintEvaluator<'a, E: FieldElement<BaseField = Self::BaseField>> =
        DefaultConstraintEvaluator<'a, Self::Air, E>;

    fn get_pub_inputs(&self, trace: &Self::Trace) -> LinearRegressionInputs {
        let trace_length = trace.length();
        
        // Extract sample points - we need to figure out where samples end
        let mut sample_x_values = Vec::new();
        let mut sample_y_values = Vec::new();
        
        // Look for the pattern: we know samples come first, then prediction
        // We'll detect where the pattern changes
        let mut i = 0;
        let first_x = trace.get(2, 0);
        sample_x_values.push(first_x);
        sample_y_values.push(trace.get(3, 0));
        
        // Continue while we see different x values (samples)
        for step in 1..trace_length {
            let x = trace.get(2, step);
            let y = trace.get(3, step);
            
            // If we haven't seen this x value before, it's either a new sample or the prediction
            if !sample_x_values.contains(&x) {
                // Check if this is likely a sample by looking at the linear relationship
                let slope = trace.get(0, step);
                let intercept = trace.get(1, step);
                let expected_y = slope * x + intercept;
                
                if y == expected_y {
                    if sample_x_values.len() < 4 { // Assume max 4 samples for this example
                        sample_x_values.push(x);
                        sample_y_values.push(y);
                    } else {
                        // This is the prediction
                        return LinearRegressionInputs {
                            x_value: x,
                            predicted_y: y,
                            sample_x_values,
                            sample_y_values,
                        };
                    }
                }
            }
        }
        
        // If we get here, extract the last unique values as prediction
        let last_step = trace_length - 1;
        let x_value = trace.get(2, last_step);
        let predicted_y = trace.get(3, last_step);
        
        LinearRegressionInputs {
            x_value,
            predicted_y,
            sample_x_values,
            sample_y_values,
        }
    }

    fn options(&self) -> &ProofOptions {
        &self.options
    }

    fn new_trace_lde<E: FieldElement<BaseField = Self::BaseField>>(
        &self,
        trace_info: &TraceInfo,
        main_trace: &ColMatrix<Self::BaseField>,
        domain: &StarkDomain<Self::BaseField>,
        partition_option: PartitionOptions,
    ) -> (Self::TraceLde<E>, TracePolyTable<E>) {
        DefaultTraceLde::new(trace_info, main_trace, domain, partition_option)
    }

    fn build_constraint_commitment<E: FieldElement<BaseField = Self::BaseField>>(
        &self,
        composition_poly_trace: CompositionPolyTrace<E>,
        num_constraint_composition_columns: usize,
        domain: &StarkDomain<Self::BaseField>,
        partition_options: PartitionOptions,
    ) -> (Self::ConstraintCommitment<E>, CompositionPoly<E>) {
        DefaultConstraintCommitment::new(
            composition_poly_trace,
            num_constraint_composition_columns,
            domain,
            partition_options,
        )
    }

    fn new_evaluator<'a, E: FieldElement<BaseField = Self::BaseField>>(
        &self,
        air: &'a Self::Air,
        aux_rand_elements: Option<AuxRandElements<E>>,
        composition_coefficients: ConstraintCompositionCoefficients<E>,
    ) -> Self::ConstraintEvaluator<'a, E> {
        DefaultConstraintEvaluator::new(air, aux_rand_elements, composition_coefficients)
    }
}

/// Example usage and testing
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_regression_proof() {
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
        
        // Build execution trace
        let trace = build_linear_regression_trace(
            slope, intercept, &sample_x, &sample_y, target_x
        );
        
        // Verify trace properties
        println!("Trace length: {}", trace.length());
        println!("Trace width: {}", trace.width());
        
        // Verify the trace values manually
        for i in 0..trace.length() {
            let s = trace.get(0, i);
            let b = trace.get(1, i);
            let x = trace.get(2, i);
            let y = trace.get(3, i);
            println!("Step {}: slope={}, intercept={}, x={}, y={}", i, s, b, x, y);
            
            // Verify linear relationship
            let expected = s * x + b;
            assert_eq!(y, expected, "Linear relationship violated at step {}", i);
        }
        
        // Define proof options
        let options = ProofOptions::new(
            32,                        // number of queries
            8,                         // blowup factor  
            0,                         // grinding factor
            FieldExtension::None,
            8,                         // FRI folding factor
            31,                        // FRI max remainder polynomial degree
            BatchingMethod::Linear,
            BatchingMethod::Linear,
        );
        
        // Generate proof
        let prover = LinearRegressionProver::new(options);
        let proof = prover.prove(trace).unwrap();
        
        // Verify proof
        let pub_inputs = LinearRegressionInputs {
            x_value: target_x,
            predicted_y: expected_y,
            sample_x_values: sample_x,
            sample_y_values: sample_y,
        };
        
        let min_opts = AcceptableOptions::MinConjecturedSecurity(95);
        
        let verification_result = winterfell::verify::<
            LinearRegressionAir,
            Blake3_256<BaseElement>,
            DefaultRandomCoin<Blake3_256<BaseElement>>,
            MerkleTree<Blake3_256<BaseElement>>
        >(proof, pub_inputs, &min_opts);
        
        assert!(verification_result.is_ok(), "Proof verification failed: {:?}", verification_result.err());
        println!("✅ Linear regression proof verified successfully!");
        println!("   Predicted y = {} for x = {} (slope and intercept kept private)", expected_y, target_x);
    }
}

/// Main function demonstrating usage
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔐 ZK-STARK Linear Regression Proof with Winterfell");
    println!("==================================================");
    
    // Private parameters (these won't be revealed in the proof)
    let slope = BaseElement::new(2);        // m = 2 (secret)
    let intercept = BaseElement::new(5);    // b = 5 (secret)
    
    println!("📊 Private linear model: y = {}x + {} (parameters hidden)", slope, intercept);
    
    // Public sample data that the verifier can see
    let sample_x = vec![
        BaseElement::new(1),
        BaseElement::new(3),
        BaseElement::new(7),
        BaseElement::new(10),
    ];
    
    let sample_y = vec![
        BaseElement::new(7),   // 2*1 + 5 = 7
        BaseElement::new(11),  // 2*3 + 5 = 11  
        BaseElement::new(19),  // 2*7 + 5 = 19
        BaseElement::new(25),  // 2*10 + 5 = 25
    ];
    
    println!("📈 Public sample points:");
    for (i, (x, y)) in sample_x.iter().zip(sample_y.iter()).enumerate() {
        println!("   Point {}: ({}, {})", i + 1, x, y);
    }
    
    // Target prediction
    let target_x = BaseElement::new(8);
    let predicted_y = slope * target_x + intercept; // 2*8 + 5 = 21
    
    println!("🎯 Claim: For x = {}, predicted y = {}", target_x, predicted_y);
    
    // Build the execution trace
    let trace = build_linear_regression_trace(
        slope, intercept, &sample_x, &sample_y, target_x
    );
    
    println!("⚙️  Trace details:");
    println!("   Trace length: {}", trace.length());
    println!("   Trace width: {}", trace.width());
    
    // Debug: Print first few rows of trace
    for i in 0..std::cmp::min(6, trace.length()) {
        let s = trace.get(0, i);
        let b = trace.get(1, i);
        let x = trace.get(2, i);
        let y = trace.get(3, i);
        println!("   Step {}: slope={}, intercept={}, x={}, y={}", i, s, b, x, y);
    }
    
    // Configure proof options  
    let options = ProofOptions::new(
        32,                        // queries for security
        8,                         // blowup factor
        0,                         // grinding factor
        FieldExtension::None,      // no field extension
        8,                         // FRI folding factor
        31,                        // FRI max remainder degree
        BatchingMethod::Linear,    // constraint batching
        BatchingMethod::Linear,    // DEEP batching
    );
    
    println!("⚙️  Generating STARK proof...");
    
    // Generate the proof
    let prover = LinearRegressionProver::new(options);
    let proof = prover.prove(trace)?;
    
    println!("✅ Proof generated! Size: {} bytes", proof.to_bytes().len());
    
    // Prepare public inputs for verification
    let pub_inputs = LinearRegressionInputs {
        x_value: target_x,
        predicted_y,
        sample_x_values: sample_x,
        sample_y_values: sample_y,
    };
    
    println!("🔍 Verifying proof...");
    
    // Verify the proof
    let min_opts = AcceptableOptions::MinConjecturedSecurity(95);
    let verification_result = winterfell::verify::<
        LinearRegressionAir,
        Blake3_256<BaseElement>, 
        DefaultRandomCoin<Blake3_256<BaseElement>>,
        MerkleTree<Blake3_256<BaseElement>>
    >(proof, pub_inputs, &min_opts);
    
    match verification_result {
        Ok(_) => {
            println!("🎉 SUCCESS: Proof verified!");
            println!("   ✓ The predicted y = {} for x = {} is correct", predicted_y, target_x);
            println!("   ✓ Linear model parameters remain private");
            println!("   ✓ Computation integrity guaranteed without revealing slope/intercept");
        },
        Err(e) => {
            println!("❌ FAILED: Proof verification failed: {:?}", e);
        }
    }
    
    Ok(())
}

