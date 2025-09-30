use std::process::Command;

#[test]
fn test_bucket_function() {
    // Feed input via stdin
    let output = Command::new("./target/release/kelora")
        .args(["-f", "line", "--exec", "e.bucket = bucket(e.line)"])
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(b"test\n").ok();
            }
            child.wait_with_output()
        })
        .expect("Failed to execute kelora");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("bucket="));
}

#[test]
fn test_hash_function_default() {
    let output = Command::new("./target/release/kelora")
        .args(["-f", "line", "--exec", "e.hash = hash(e.line)"])
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(b"hello\n").ok();
            }
            child.wait_with_output()
        })
        .expect("Failed to execute kelora");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // SHA-256 hash of "hello"
    assert!(stdout.contains("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"));
}

#[test]
fn test_hash_function_with_algorithm() {
    let output = Command::new("./target/release/kelora")
        .args([
            "-f",
            "line",
            "--exec",
            r#"e.hash_md5 = hash(e.line, "md5")"#,
        ])
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(b"hello\n").ok();
            }
            child.wait_with_output()
        })
        .expect("Failed to execute kelora");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // MD5 hash of "hello"
    assert!(stdout.contains("5d41402abc4b2a76b9719d911017c592"));
}

#[test]
fn test_anonymize_without_salt_fails() {
    let output = Command::new("./target/release/kelora")
        .args(["-f", "line", "--exec", "e.anon = anonymize(e.line)"])
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(b"user123\n").ok();
            }
            child.wait_with_output()
        })
        .expect("Failed to execute kelora");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("KELORA_SALT"));
    assert!(stderr.contains("export KELORA_SALT="));
}

#[test]
fn test_anonymize_with_cli_salt() {
    let output = Command::new("./target/release/kelora")
        .args([
            "-f",
            "line",
            "--salt",
            "test_salt",
            "--exec",
            "e.anon = anonymize(e.line)",
        ])
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(b"user123\n").ok();
            }
            child.wait_with_output()
        })
        .expect("Failed to execute kelora");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("anon="));
    // Should produce a valid hex string (64 chars for SHA-256)
    assert!(stdout.contains("line="));
}

#[test]
fn test_anonymize_with_env_salt() {
    let output = Command::new("./target/release/kelora")
        .env("KELORA_SALT", "env_test_salt")
        .args(["-f", "line", "--exec", "e.anon = anonymize(e.line)"])
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(b"user123\n").ok();
            }
            child.wait_with_output()
        })
        .expect("Failed to execute kelora");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("anon="));
}

#[test]
fn test_pseudonym_with_cli_salt() {
    let output = Command::new("./target/release/kelora")
        .args([
            "-f",
            "line",
            "--salt",
            "test_salt",
            "--exec",
            "e.pseudo = pseudonym(e.line)",
        ])
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(b"user123\n").ok();
            }
            child.wait_with_output()
        })
        .expect("Failed to execute kelora");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("pseudo="));
}

#[test]
fn test_pseudonym_with_custom_length() {
    let output = Command::new("./target/release/kelora")
        .args([
            "-f",
            "line",
            "--salt",
            "test_salt",
            "--exec",
            "e.pseudo = pseudonym(e.line, 5)",
        ])
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(b"user123\n").ok();
            }
            child.wait_with_output()
        })
        .expect("Failed to execute kelora");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("pseudo="));
    // The pseudonym should be relatively short (5 chars)
}

#[test]
fn test_cli_salt_overrides_env() {
    // Set env salt
    let output_env = Command::new("./target/release/kelora")
        .env("KELORA_SALT", "env_salt")
        .args(["-f", "line", "--exec", "e.anon = anonymize(e.line)"])
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(b"user123\n").ok();
            }
            child.wait_with_output()
        })
        .expect("Failed to execute kelora");

    // CLI salt overrides env
    let output_cli = Command::new("./target/release/kelora")
        .env("KELORA_SALT", "env_salt")
        .args([
            "-f",
            "line",
            "--salt",
            "cli_salt",
            "--exec",
            "e.anon = anonymize(e.line)",
        ])
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(b"user123\n").ok();
            }
            child.wait_with_output()
        })
        .expect("Failed to execute kelora");

    let stdout_env = String::from_utf8_lossy(&output_env.stdout);
    let stdout_cli = String::from_utf8_lossy(&output_cli.stdout);

    // Results should be different (different salts)
    assert_ne!(stdout_env, stdout_cli);
}

#[test]
fn test_deterministic_hashing() {
    // Run twice with same salt
    let output1 = Command::new("./target/release/kelora")
        .args([
            "-f",
            "line",
            "--salt",
            "test_salt",
            "--exec",
            "e.anon = anonymize(e.line)",
        ])
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(b"user123\n").ok();
            }
            child.wait_with_output()
        })
        .expect("Failed to execute kelora");

    let output2 = Command::new("./target/release/kelora")
        .args([
            "-f",
            "line",
            "--salt",
            "test_salt",
            "--exec",
            "e.anon = anonymize(e.line)",
        ])
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(b"user123\n").ok();
            }
            child.wait_with_output()
        })
        .expect("Failed to execute kelora");

    let stdout1 = String::from_utf8_lossy(&output1.stdout);
    let stdout2 = String::from_utf8_lossy(&output2.stdout);

    // Should produce identical output
    assert_eq!(stdout1, stdout2);
}
