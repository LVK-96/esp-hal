//% CHIP_FILTER: esp32
//% FEATURES: unstable esp-alloc/nightly esp-println/uart

#![no_std]
#![no_main]

use esp_alloc as _;
use esp_hal::{
    Blocking,
    rsa::{
        Rsa, RsaMode, RsaModularExponentiation, RsaModularMultiplication, RsaMultiplication,
        operand_sizes::*,
    },
};
use esp_println::{print, println};

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("PANIC! {}", info);
    loop {}
}

esp_bootloader_esp_idf::esp_app_desc!();

fn dump_words(label: &str, words: &[u32]) {
    print!("{}:", label);
    for (i, w) in words.iter().enumerate() {
        if i % 8 == 0 {
            print!("\n[{:3}] ", i);
        }
        print!(" 0x{:08x}", w);
    }
    println!("\n");
}

struct RsaMemoryWindowResult {
    bits: usize,
    passed: bool,
}

fn set_base_with_distinct_words(words: &mut [u32]) {
    for (i, w) in words.iter_mut().enumerate() {
        *w = (i as u32)
            .wrapping_mul(0x1111_1111)
            .wrapping_add(0x0123_4567);
    }
    words[0] = 0xface_b00c;
    words[words.len() - 1] = 0xbabe_cafe;
}

fn modular_exponentiation_uses_full_memory_window<T, const N: usize>(
    rsa: &mut Rsa<'_, Blocking>,
) -> RsaMemoryWindowResult
where
    T: RsaMode<InputType = [u32; N]>,
{
    // X^e mod M
    // For M = 2^(32 * N) - 1, e = 1, result should be X
    let mut exponent = [0; N];
    exponent[0] = 1;
    let modulus = [u32::MAX; N];
    let mut r_squared = [0; N];
    r_squared[0] = 1;
    let mut operand_x = [0; N];
    set_base_with_distinct_words(&mut operand_x);
    let mut outbuf = [0xdead_beef; N];

    println!("modexp {} bit", N * 32);
    dump_words("(M)", &modulus);
    dump_words("(e)", &exponent);
    dump_words("(X)", &operand_x);

    let mut mod_exp = RsaModularExponentiation::<T, _>::new(rsa, &exponent, &modulus, 1);
    mod_exp.start_exponentiation(&operand_x, &r_squared);
    mod_exp.read_results(&mut outbuf);

    dump_words("(X^e mod M)", &outbuf);

    RsaMemoryWindowResult {
        bits: (N * 32),
        passed: operand_x == outbuf,
    }
}

fn modular_multiplication_uses_full_memory_window<T, const N: usize>(
    rsa: &mut Rsa<'_, Blocking>,
) -> RsaMemoryWindowResult
where
    T: RsaMode<InputType = [u32; N]>,
{
    // X * Y  mod M
    // For M = 2^(32 * N) - 1, Y = 1, result should be X
    let modulus = [u32::MAX; N];
    let mut r_squared = [0; N];
    r_squared[0] = 1;
    let mut operand_x = [0; N];
    set_base_with_distinct_words(&mut operand_x);
    let mut operand_y = [0; N];
    operand_y[0] = 1;
    let mut outbuf = [0xdead_beef; N];

    println!("modmult {} bit", N * 32);
    dump_words("(M)", &modulus);
    dump_words("(X)", &operand_x);
    dump_words("(Y)", &operand_y);

    let mut mod_multi =
        RsaModularMultiplication::<T, _>::new(rsa, &operand_x, &modulus, &r_squared, 1);
    mod_multi.start_modular_multiplication(&operand_y);
    mod_multi.read_results(&mut outbuf);

    dump_words("(X*Y mod M)", &outbuf);

    RsaMemoryWindowResult {
        bits: (N * 32),
        passed: operand_x == outbuf,
    }
}

fn multiplication_uses_full_memory_window<T, const N: usize, const O: usize>(
    rsa: &mut Rsa<'_, Blocking>,
) -> RsaMemoryWindowResult
where
    T: RsaMode<InputType = [u32; N]> + esp_hal::rsa::Multi<OutputType = [u32; O]>,
{
    // X * Y
    // For Y = 1, result should be X
    let mut operand_x = [0; N];
    set_base_with_distinct_words(&mut operand_x);
    let mut operand_y = [0; N];
    operand_y[0] = 1;
    let mut expected = [0; O];
    expected[..N].copy_from_slice(&operand_x);
    let mut outbuf = [0xdead_beef; O];

    println!("multiply {} bit", N * 32);
    dump_words("(X)", &operand_x);
    dump_words("(Y)", &operand_y);

    let mut multi = RsaMultiplication::<T, _>::new(rsa, &operand_x);
    multi.start_multiplication(&operand_y);
    multi.read_results(&mut outbuf);

    dump_words("(X * Y)", &outbuf);

    RsaMemoryWindowResult {
        bits: (N * 32),
        passed: expected == outbuf,
    }
}

type RsaMemoryWindowCheck = for<'a, 'd> fn(&'a mut Rsa<'d, Blocking>) -> RsaMemoryWindowResult;

const RSA_MODEXP_MEMORY_WINDOW_CHECKS: &[RsaMemoryWindowCheck] = &[
    modular_exponentiation_uses_full_memory_window::<Op512, { 512 / 32 }>,
    modular_exponentiation_uses_full_memory_window::<Op1024, { 1024 / 32 }>,
    modular_exponentiation_uses_full_memory_window::<Op1536, { 1536 / 32 }>,
    modular_exponentiation_uses_full_memory_window::<Op2048, { 2048 / 32 }>,
    modular_exponentiation_uses_full_memory_window::<Op2560, { 2560 / 32 }>,
    modular_exponentiation_uses_full_memory_window::<Op3072, { 3072 / 32 }>,
    modular_exponentiation_uses_full_memory_window::<Op3584, { 3584 / 32 }>,
    modular_exponentiation_uses_full_memory_window::<Op4096, { 4096 / 32 }>,
];

const RSA_MODMULT_MEMORY_WINDOW_CHECKS: &[RsaMemoryWindowCheck] = &[
    modular_multiplication_uses_full_memory_window::<Op512, { 512 / 32 }>,
    modular_multiplication_uses_full_memory_window::<Op1024, { 1024 / 32 }>,
    modular_multiplication_uses_full_memory_window::<Op1536, { 1536 / 32 }>,
    modular_multiplication_uses_full_memory_window::<Op2048, { 2048 / 32 }>,
];

const RSA_MULT_MEMORY_WINDOW_CHECKS: &[RsaMemoryWindowCheck] = &[
    multiplication_uses_full_memory_window::<Op512, { 512 / 32 }, { 2 * 512 / 32 }>,
    multiplication_uses_full_memory_window::<Op1024, { 1024 / 32 }, { 2 * 1024 / 32 }>,
    multiplication_uses_full_memory_window::<Op1536, { 1536 / 32 }, { 2 * 1536 / 32 }>,
    multiplication_uses_full_memory_window::<Op2048, { 2048 / 32 }, { 2 * 2048 / 32 }>,
];

struct TestCounter {
    passed: u32,
    failed: u32,
}

impl TestCounter {
    const fn new() -> Self {
        Self {
            passed: 0,
            failed: 0,
        }
    }

    fn report(&mut self, result: RsaMemoryWindowResult) {
        if result.passed {
            self.passed += 1;
            println!("[PASS] {} bit", result.bits);
        } else {
            self.failed += 1;
            println!("[FAIL] {} bit", result.bits);
        }
    }
}

#[esp_hal::main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default());

    let mut counter = TestCounter::new();
    let mut rsa = Rsa::new(peripherals.RSA);

    println!("--- modexp tests ---");
    for check in RSA_MODEXP_MEMORY_WINDOW_CHECKS {
        counter.report(check(&mut rsa));
    }

    println!("\n--- modmult tests ---");
    for check in RSA_MODMULT_MEMORY_WINDOW_CHECKS {
        counter.report(check(&mut rsa));
    }

    println!("\n--- multiply tests ---");
    for check in RSA_MULT_MEMORY_WINDOW_CHECKS {
        counter.report(check(&mut rsa));
    }

    println!();
    println!(
        "=== {} passed, {} failed ===",
        counter.passed, counter.failed
    );
    if counter.failed > 0 {
        println!("SOME TESTS FAILED!");
    } else {
        println!("ALL TESTS PASSED!");
    }

    loop {}
}
