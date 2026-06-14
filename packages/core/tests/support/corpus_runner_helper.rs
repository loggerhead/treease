#[allow(dead_code)]
#[path = "../corpus_runner.rs"]
mod corpus_runner;

fn main() {
    std::process::exit(corpus_runner::corpus_runner_helper_main());
}
