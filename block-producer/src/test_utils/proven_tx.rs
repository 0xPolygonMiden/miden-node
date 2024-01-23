//! FibSmall taken from the `fib_small` example in `winterfell`

use std::sync::{Arc, Mutex};

use miden_air::{ExecutionProof, HashFunction};
use miden_crypto::hash::rpo::Rpo256;
use miden_mock::constants::ACCOUNT_ID_REGULAR_ACCOUNT_UPDATABLE_CODE_ON_CHAIN;
use miden_objects::{
    accounts::AccountId,
    notes::{NoteEnvelope, NoteMetadata, Nullifier},
    transaction::{InputNotes, OutputNotes, ProvenTransaction},
    Digest, ONE,
};
use once_cell::sync::Lazy;
use winterfell::{
    crypto::{hashers::Blake3_192, DefaultRandomCoin},
    math::{fields::f64::BaseElement, FieldElement},
    matrix::ColMatrix,
    Air, AirContext, Assertion, AuxTraceRandElements, ConstraintCompositionCoefficients,
    DefaultConstraintEvaluator, DefaultTraceLde, EvaluationFrame, FieldExtension, ProofOptions,
    Prover, StarkDomain, StarkProof, Trace, TraceInfo, TracePolyTable, TraceTable,
    TransitionConstraintDegree,
};

use super::MockPrivateAccount;

/// Keeps track how many accounts were created as a source of randomness
static NUM_ACCOUNTS_CREATED: Lazy<Arc<Mutex<u32>>> = Lazy::new(|| Arc::new(Mutex::new(0)));

/// Keeps track how many accounts were created as a source of randomness
static NUM_NOTES_CREATED: Lazy<Arc<Mutex<u64>>> = Lazy::new(|| Arc::new(Mutex::new(0)));

pub struct MockProvenTxBuilder {
    mock_account: MockPrivateAccount,
    notes_created: Option<Vec<NoteEnvelope>>,
}

impl MockProvenTxBuilder {
    pub fn new() -> Self {
        let account_index: u32 = {
            let mut locked_num_accounts_created = NUM_ACCOUNTS_CREATED.lock().unwrap();

            let account_index = *locked_num_accounts_created;

            *locked_num_accounts_created += 1;

            account_index
        };
        Self {
            mock_account: account_index.into(),
            notes_created: None,
        }
    }

    pub fn num_notes_created(
        mut self,
        num_notes_created_in_tx: u64,
    ) -> Self {
        let mut locked_num_notes_created = NUM_NOTES_CREATED.lock().unwrap();

        let notes_created: Vec<_> = (*locked_num_notes_created
            ..(*locked_num_notes_created + num_notes_created_in_tx))
            .map(|note_index| {
                let note_hash = Rpo256::hash(&note_index.to_be_bytes());

                NoteEnvelope::new(note_hash.into(), NoteMetadata::new(self.mock_account.id, ONE))
            })
            .collect();

        // update state
        self.notes_created = Some(notes_created);
        *locked_num_notes_created += num_notes_created_in_tx;

        self
    }

    pub fn build(self) -> ProvenTransaction {
        ProvenTransaction::new(
            self.mock_account.id,
            self.mock_account.states[0],
            self.mock_account.states[1],
            InputNotes::new(Vec::new()).unwrap(),
            OutputNotes::new(self.notes_created.unwrap_or_default()).unwrap(),
            None,
            Digest::default(),
            ExecutionProof::new(StarkProof::new_dummy(), HashFunction::Blake3_192),
        )
    }
}

impl Default for MockProvenTxBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// We need to generate a new `ProvenTransaction` every time because it doesn't
/// derive `Clone`. Doing it this way allows us to compute the `StarkProof`
/// once, and clone it for each new `ProvenTransaction`.
#[derive(Clone)]
pub struct DummyProvenTxGenerator {
    stark_proof: StarkProof,
}

impl DummyProvenTxGenerator {
    pub fn new() -> Self {
        let prover = DummyProver::new();
        let stark_proof = prover.prove(prover.build_trace(16)).unwrap();
        Self { stark_proof }
    }

    pub fn dummy_proven_tx(&self) -> ProvenTransaction {
        ProvenTransaction::new(
            AccountId::try_from(ACCOUNT_ID_REGULAR_ACCOUNT_UPDATABLE_CODE_ON_CHAIN).unwrap(),
            Digest::default(),
            Digest::default(),
            InputNotes::new(Vec::new()).unwrap(),
            OutputNotes::new(Vec::new()).unwrap(),
            None,
            Digest::default(),
            ExecutionProof::new(self.stark_proof.clone(), HashFunction::Blake3_192),
        )
    }

    pub fn dummy_proven_tx_with_params(
        &self,
        account_id: AccountId,
        initial_account_hash: Digest,
        final_account_hash: Digest,
        input_notes: InputNotes<Nullifier>,
        output_notes: OutputNotes<NoteEnvelope>,
    ) -> ProvenTransaction {
        ProvenTransaction::new(
            account_id,
            initial_account_hash,
            final_account_hash,
            input_notes,
            output_notes,
            None,
            Digest::default(),
            ExecutionProof::new(self.stark_proof.clone(), HashFunction::Blake3_192),
        )
    }
}

impl Default for DummyProvenTxGenerator {
    fn default() -> Self {
        Self::new()
    }
}

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
    type TraceLde<E: FieldElement<BaseField = BaseElement>> =
        DefaultTraceLde<E, Blake3_192<BaseElement>>;
    type ConstraintEvaluator<'a, E: FieldElement<BaseField = BaseElement>> =
        DefaultConstraintEvaluator<'a, FibSmall, E>;

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

    fn new_trace_lde<E: FieldElement<BaseField = BaseElement>>(
        &self,
        trace_info: &TraceInfo,
        main_trace: &ColMatrix<BaseElement>,
        domain: &StarkDomain<BaseElement>,
    ) -> (Self::TraceLde<E>, TracePolyTable<E>) {
        DefaultTraceLde::new(trace_info, main_trace, domain)
    }

    fn new_evaluator<'a, E: FieldElement<BaseField = BaseElement>>(
        &self,
        air: &'a FibSmall,
        aux_rand_elements: AuxTraceRandElements<E>,
        composition_coefficients: ConstraintCompositionCoefficients<E>,
    ) -> Self::ConstraintEvaluator<'a, E> {
        DefaultConstraintEvaluator::new(air, aux_rand_elements, composition_coefficients)
    }
}
