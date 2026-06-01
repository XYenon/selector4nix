use fastrand::Rng;

pub fn generate_test_contents(count: usize, seed: u64) -> Vec<Vec<u8>> {
    let mut rng = Rng::with_seed(seed);
    (0..count)
        .map(|i| generate_content(i, count, &mut rng))
        .collect()
}

fn generate_content(index: usize, total: usize, rng: &mut Rng) -> Vec<u8> {
    if index == 0 {
        return Vec::new();
    }
    if index == 1 {
        return b"\x00\x01\x02\x03\x04\x05\x06\x07".to_vec();
    }

    let size_bucket = index * 4 / total;
    match size_bucket {
        0 => generate_random_bytes(rng.usize(1..100), rng),
        1 => generate_random_bytes(rng.usize(100..1_000), rng),
        2 => generate_random_bytes(rng.usize(1_000..50_000), rng),
        _ => generate_random_bytes(rng.usize(50_000..500_000), rng),
    }
}

fn generate_random_bytes(len: usize, rng: &mut Rng) -> Vec<u8> {
    (0..len).map(|_| rng.u8(..)).collect()
}

pub fn generate_hash(rng: &mut Rng) -> String {
    const NIX_BASE32_CHARSET: &[u8] = b"0123456789abcdfghijklmnpqrsvwxyz";
    rng.choose_multiple(NIX_BASE32_CHARSET.iter(), 32)
        .iter()
        .map(|c| **c as char)
        .collect()
}
