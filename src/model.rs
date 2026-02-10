/// A step: flat list of lines. No round/entry nesting.
pub type Step = Vec<String>;

pub fn new_step() -> Step {
    vec![String::new()]
}

pub fn make_steps(n: usize) -> Vec<Step> {
    (0..n).map(|_| new_step()).collect()
}
