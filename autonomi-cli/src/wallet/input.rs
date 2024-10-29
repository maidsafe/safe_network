pub(crate) fn get_wallet_selection_input(prompt: &str) -> String {
    println!("{prompt}");

    let mut buffer = String::new();
    let stdin = std::io::stdin();

    if stdin.read_line(&mut buffer).is_err() {
        // consider if error should process::exit(1) here
        return "".to_string();
    };

    // Remove leading and trailing whitespace
    buffer.trim().to_owned()
}

pub(crate) fn get_password_input(prompt: &str) -> String {
    rpassword::prompt_password(prompt)
        .map(|str| str.trim().into())
        .unwrap_or_default()
}

pub(crate) fn confirm_password(password: &str) -> bool {
    const MAX_RETRIES: u8 = 2;

    for _ in 0..MAX_RETRIES {
        if get_password_input("Repeat password: ") == password {
            return true;
        }
        println!("Passwords do not match.");
    }

    false
}

pub(crate) fn request_password(required: bool) -> Option<String> {
    let prompt = if required {
        "Enter password: "
    } else {
        "Enter password (leave empty for none): "
    };

    loop {
        let password = get_password_input(prompt);

        if password.is_empty() {
            if required {
                println!("Password is required.");
                continue;
            }

            return None;
        }

        if confirm_password(&password) {
            return Some(password);
        }

        println!("Please set a new password.");
    }
}
