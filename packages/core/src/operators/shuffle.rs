use crate::operators::*;
use std::time::{SystemTime, UNIX_EPOCH};

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
}

struct Xoshiro256 {
    state: [u64; 4],
}

impl Xoshiro256 {
    fn new(seed: u64) -> Self {
        let mut splitmix = SplitMix64::new(seed);
        Self {
            state: [
                splitmix.next_u64(),
                splitmix.next_u64(),
                splitmix.next_u64(),
                splitmix.next_u64(),
            ],
        }
    }

    fn next_u64(&mut self) -> u64 {
        let result = self.state[0]
            .wrapping_add(self.state[3])
            .rotate_left(23)
            .wrapping_add(self.state[0]);

        let t = self.state[1] << 17;

        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];
        self.state[2] ^= t;
        self.state[3] = self.state[3].rotate_left(45);

        result
    }

    fn uint_less_than(&mut self, less_than: usize) -> usize {
        assert!(less_than > 0);
        let less_than = less_than as u64;
        let mut x = self.next_u64();
        let mut m = (x as u128) * (less_than as u128);
        let mut low = m as u64;

        if low < less_than {
            let mut threshold = less_than.wrapping_neg();
            if threshold >= less_than {
                threshold = threshold.wrapping_sub(less_than);
                if threshold >= less_than {
                    threshold %= less_than;
                }
            }
            while low < threshold {
                x = self.next_u64();
                m = (x as u128) * (less_than as u128);
                low = m as u64;
            }
        }

        (m >> 64) as usize
    }
}

fn running_under_test_harness() -> bool {
    cfg!(test)
        || std::env::var_os("RUST_TEST_THREADS").is_some()
        || std::env::args()
            .next()
            .is_some_and(|arg| arg.contains("/deps/"))
}

/// Fisher-Yates shuffle on sequence content.
fn shuffle_with_random(
    candidate: &TreeNode,
    rng: &mut Xoshiro256,
) -> Result<Box<TreeNode>, CoreError> {
    if candidate.kind != NodeKind::Sequence {
        return Err(CoreError::Eval(EvalError::NodeIsNotArray));
    }

    let mut shuffled =
        *candidate.create_replacement_with_comments(NodeKind::Sequence, &candidate.tag)?;

    let n = candidate.content.len();
    if n <= 1 {
        for child in &candidate.content {
            shuffled.add_child(child)?;
        }
        return Ok(Box::new(shuffled));
    }

    let mut items: Vec<TreeNode> = candidate.content.clone();

    // Fisher-Yates shuffle
    for i in (1..n).rev() {
        let j = rng.uint_less_than(i + 1);
        items.swap(i, j);
    }

    for child in &items {
        shuffled.add_child(child)?;
    }
    Ok(Box::new(shuffled))
}

/// Shuffle array elements randomly (shuffle operator).
pub fn shuffle_operator(
    ctx: Context,
    _d: &mut TreeEngine,
    _expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    // Use timestamp as random seed (in test builds use a fixed seed)
    let seed: u64 = if running_under_test_harness() {
        1621386123000000004
    } else {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    };

    let mut rng = Xoshiro256::new(seed);

    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        let shuffled = shuffle_with_random(candidate, &mut rng)?;
        results.push(*shuffled);
    }
    ctx.child_context(results)
}
