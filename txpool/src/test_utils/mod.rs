///! FibSmall taken from the `fib_small` example in `winterfell`
use winterfell::{
    crypto::{hashers::Blake3_192, DefaultRandomCoin},
    math::fields::f64::BaseElement,
    math::FieldElement,
    Air, AirContext, Assertion, EvaluationFrame, FieldExtension, ProofOptions, Prover, StarkProof,
    Trace, TraceInfo, TraceTable, TransitionConstraintDegree,
};

const TRACE_WIDTH: usize = 2;

pub fn are_equal<E: FieldElement>(
    a: E,
    b: E,
) -> E {
    a - b
}

pub struct FibSmall {
    context: AirContext<BaseElement>,
    result: BaseElement,
}

impl Air for FibSmall {
    type BaseField = BaseElement;
    type PublicInputs = BaseElement;

    // CONSTRUCTOR
    // --------------------------------------------------------------------------------------------
    fn new(
        trace_info: TraceInfo,
        pub_inputs: Self::BaseField,
        options: ProofOptions,
    ) -> Self {
        let degrees = vec![TransitionConstraintDegree::new(1), TransitionConstraintDegree::new(1)];
        assert_eq!(TRACE_WIDTH, trace_info.width());
        FibSmall {
            context: AirContext::new(trace_info, degrees, 3, options),
            result: pub_inputs,
        }
    }

    fn context(&self) -> &AirContext<Self::BaseField> {
        &self.context
    }

    fn evaluate_transition<E: FieldElement + From<Self::BaseField>>(
        &self,
        frame: &EvaluationFrame<E>,
        _periodic_values: &[E],
        result: &mut [E],
    ) {
        let current = frame.current();
        let next = frame.next();
        // expected state width is 2 field elements
        debug_assert_eq!(TRACE_WIDTH, current.len());
        debug_assert_eq!(TRACE_WIDTH, next.len());

        // constraints of Fibonacci sequence (2 terms per step):
        // s_{0, i+1} = s_{0, i} + s_{1, i}
        // s_{1, i+1} = s_{1, i} + s_{0, i+1}
        result[0] = are_equal(next[0], current[0] + current[1]);
        result[1] = are_equal(next[1], current[1] + next[0]);
    }

    fn get_assertions(&self) -> Vec<Assertion<Self::BaseField>> {
        // a valid Fibonacci sequence should start with two ones and terminate with
        // the expected result
        let last_step = self.trace_length() - 1;
        vec![
            Assertion::single(0, 0, Self::BaseField::ONE),
            Assertion::single(1, 0, Self::BaseField::ONE),
            Assertion::single(1, last_step, self.result),
        ]
    }
}

pub struct DummyProver {
    options: ProofOptions,
}

impl DummyProver {
    pub fn new() -> Self {
        Self {
            options: ProofOptions::new(1, 2, 1, FieldExtension::None, 2, 127),
        }
    }

    /// Builds an execution trace for computing a Fibonacci sequence of the specified length such
    /// that each row advances the sequence by 2 terms.
    pub fn build_trace(
        &self,
        sequence_length: usize,
    ) -> TraceTable<BaseElement> {
        assert!(sequence_length.is_power_of_two(), "sequence length must be a power of 2");

        let mut trace = TraceTable::new(TRACE_WIDTH, sequence_length / 2);
        trace.fill(
            |state| {
                state[0] = BaseElement::ONE;
                state[1] = BaseElement::ONE;
            },
            |_, state| {
                state[0] += state[1];
                state[1] += state[0];
            },
        );

        trace
    }
}

impl Prover for DummyProver {
    type BaseField = BaseElement;
    type Air = FibSmall;
    type Trace = TraceTable<BaseElement>;
    type HashFn = Blake3_192<BaseElement>;
    type RandomCoin = DefaultRandomCoin<Self::HashFn>;

    fn get_pub_inputs(
        &self,
        trace: &Self::Trace,
    ) -> BaseElement {
        let last_step = trace.length() - 1;
        trace.get(1, last_step)
    }

    fn options(&self) -> &ProofOptions {
        &self.options
    }
}

pub fn dummy_stark_proof() -> StarkProof {
    let prover = DummyProver::new();
    prover.prove(prover.build_trace(16)).unwrap()
}
