use zkvm::{Contract, Program, Predicate, Commitment};
use crate::sidecoin_flavor;

pub fn pay_from_addr_to_addrs<I, O>(inputs: I, outputs: O) -> Program
where
    I: IntoIterator<Item = Contract> + ExactSizeIterator,
    O: IntoIterator<Item = (u64, Predicate)> + ExactSizeIterator,
{
    let inputs_len = inputs.len();
    let outputs_lest = outputs.len();
    let (qtys, predicates): (Vec<_>, Vec<_>) = outputs.into_iter().unzip();

    Program::build(|b| {
        for contract in inputs {
            b.push(contract)
                .input()
                .signtx();
        }
        for qty in qtys {
            b.push(Commitment::blinded(qty))
                .push(Commitment::blinded(sidecoin_flavor()));
        }
        b.cloak(inputs_len, outputs_lest);
        for predicate in predicates {
            b.push(predicate)
                .output(1);
        }
    })
}