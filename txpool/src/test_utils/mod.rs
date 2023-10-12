use winterfell::{
    crypto::{hashers::Blake3_192, DefaultRandomCoin},
    math::fields::f64::BaseElement,
    math::FieldElement,
    Air, AirContext, Assertion, EvaluationFrame, FieldExtension, ProofOptions, Prover, StarkProof,
    Trace, TraceInfo, TraceTable, TransitionConstraintDegree,
};

pub struct DummyAir {
    context: AirContext<BaseElement>,
}

impl Air for DummyAir {
    type BaseField = BaseElement;
    type PublicInputs = BaseElement;

    // CONSTRUCTOR
    // --------------------------------------------------------------------------------------------
    fn new(
        trace_info: TraceInfo,
        _pub_inputs: Self::BaseField,
        options: ProofOptions,
    ) -> Self {
        let degrees = vec![TransitionConstraintDegree::new(1), TransitionConstraintDegree::new(1)];
        DummyAir {
            context: AirContext::new(trace_info, degrees, 3, options),
        }
    }

    fn context(&self) -> &AirContext<Self::BaseField> {
        &self.context
    }

    fn evaluate_transition<E: FieldElement + From<Self::BaseField>>(
        &self,
        _frame: &EvaluationFrame<E>,
        _periodic_values: &[E],
        result: &mut [E],
    ) {
        // always accept
        result[0] = E::ZERO;
        result[1] = E::ZERO;
    }

    fn get_assertions(&self) -> Vec<Assertion<Self::BaseField>> {
        // nothing
        Vec::new()
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
}

impl Prover for DummyProver {
    type BaseField = BaseElement;
    type Air = DummyAir;
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
    prover.prove(TraceTable::new(2, 8)).unwrap()
}
