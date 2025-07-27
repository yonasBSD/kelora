use rhai::{Engine, EvalAltResult};
use std::sync::Mutex;

lazy_static::lazy_static! {
    static ref RNG: Mutex<fastrand::Rng> = Mutex::new(fastrand::Rng::new());
}

fn rand_float() -> Result<f64, Box<EvalAltResult>> {
    let mut rng = RNG.lock().unwrap();
    Ok(rng.f64())
}

fn rand_int_range(min: i64, max: i64) -> Result<i64, Box<EvalAltResult>> {
    if min > max {
        return Err(format!(
            "rand_int: min ({}) cannot be greater than max ({})",
            min, max
        )
        .into());
    }

    let mut rng = RNG.lock().unwrap();
    Ok(rng.i64(min..=max))
}

pub fn register_functions(engine: &mut Engine) {
    engine.register_fn("rand", rand_float);
    engine.register_fn("rand_int", rand_int_range);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rand_float() {
        for _ in 0..100 {
            let val = rand_float().unwrap();
            assert!(
                (0.0..1.0).contains(&val),
                "rand() should return value in [0.0, 1.0), got {}",
                val
            );
        }
    }

    #[test]
    fn test_rand_int_range() {
        // Test basic range
        for _ in 0..100 {
            let val = rand_int_range(1, 10).unwrap();
            assert!(
                (1..=10).contains(&val),
                "rand_int(1, 10) should return value in [1, 10], got {}",
                val
            );
        }

        // Test single value range
        let val = rand_int_range(5, 5).unwrap();
        assert_eq!(val, 5);

        // Test negative range
        for _ in 0..100 {
            let val = rand_int_range(-10, -1).unwrap();
            assert!(
                (-10..=-1).contains(&val),
                "rand_int(-10, -1) should return value in [-10, -1], got {}",
                val
            );
        }
    }

    #[test]
    fn test_rand_int_invalid_range() {
        let result = rand_int_range(10, 5);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("min (10) cannot be greater than max (5)"));
    }
}
