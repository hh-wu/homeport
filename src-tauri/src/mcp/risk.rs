#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Risk {
    Read,
    Write,
    Dangerous,
}

pub fn classify_command(command: &str) -> Risk {
    let lowered = command.to_lowercase();
    if lowered.contains(":|:") || lowered.contains("|:&") {
        return Risk::Dangerous;
    }
    let mut overall = Risk::Read;
    for segment in split_segments(command) {
        match classify_segment(segment) {
            Risk::Dangerous => return Risk::Dangerous,
            Risk::Write => overall = Risk::Write,
            Risk::Read => {}
        }
    }
    overall
}

pub fn is_download_pipe(command: &str) -> bool {
    let lowered = command.to_lowercase();
    if !lowered.contains('|') {
        return false;
    }
    const SOURCES: [&str; 3] = ["curl", "wget", "fetch"];
    const SINKS: [&str; 8] = [
        "| sh",
        "|sh",
        "| bash",
        "|bash",
        "| zsh",
        "|zsh",
        "| sudo sh",
        "| sudo bash",
    ];
    SOURCES.iter().any(|source| lowered.contains(source))
        && SINKS.iter().any(|sink| lowered.contains(sink))
}

fn split_segments(command: &str) -> Vec<&str> {
    command
        .split([';', '\n', '|'])
        .filter(|segment| !segment.trim().is_empty())
        .collect()
}

fn classify_segment(segment: &str) -> Risk {
    let segment = segment.trim();
    if segment.is_empty() {
        return Risk::Read;
    }
    let mut words: Vec<&str> = segment.split_whitespace().collect();
    let mut head = words[0].to_lowercase();
    while is_wrapper(&head) || (head.contains('=') && words.len() > 1) {
        words.remove(0);
        head = words[0].to_lowercase();
    }
    let joined = segment.to_lowercase();

    if joined.contains(":|:") || joined.contains("|:&") {
        return Risk::Dangerous;
    }
    if matches_dangerous(&head, &words, &joined) {
        return Risk::Dangerous;
    }

    let risk = grade_write(&head, &words, &joined);

    for path in DANGEROUS_PATHS {
        let touches = joined.contains(&format!(">{path}"))
            || joined.contains(&format!(">>{path}"))
            || joined.contains(&format!("> {path}"))
            || joined.contains(&format!(">> {path}"));
        if touches {
            if path == "/dev/" && joined.contains("/dev/null") {
                continue;
            }
            return Risk::Dangerous;
        }
        if is_write_command(&head) && joined.contains(&format!(" {path}")) {
            return Risk::Dangerous;
        }
    }
    risk
}

fn is_write_command(head: &str) -> bool {
    matches!(
        head,
        "cp" | "mv" | "tee" | "install" | "rsync" | "dd" | "truncate"
    )
}

const DANGER_HEADS: [&str; 10] = [
    "mkfs", "shutdown", "reboot", "halt", "poweroff", "init", "fdisk", "parted", "wipefs", "shred",
];

const DANGEROUS_PATHS: [&str; 5] = ["/etc/", "/boot/", "/sys/", "/dev/", "/proc/sys/"];

const PACKAGE_MANAGERS: [&str; 13] = [
    "apt", "apt-get", "yum", "dnf", "pacman", "zypper", "brew", "pip", "pip3", "npm", "yarn",
    "pnpm", "gem",
];

fn matches_dangerous(head: &str, words: &[&str], joined: &str) -> bool {
    if head == "rm" && (contains_any_flag(words, &["-rf", "-fr"]) || joined.contains("rm -rf")) {
        return true;
    }
    if head.starts_with("mkfs") {
        return true;
    }
    if head == "dd" && joined.contains("of=/dev/") {
        return true;
    }
    DANGER_HEADS.contains(&head)
}

fn grade_write(head: &str, words: &[&str], joined: &str) -> Risk {
    match head {
        "mv" | "truncate" | "chmod" | "chown" | "kill" | "pkill" | "killall" | "tee"
        | "userdel" | "usermod" | "passwd" | "visudo" | "rm" => return Risk::Write,
        "cp" if contains_any_flag(words, &["-f", "--force"]) => return Risk::Write,
        _ => {}
    }
    if head == "systemctl" || head == "service" {
        for sub in [
            "restart", "stop", "disable", "mask", "kill", "start", "enable",
        ] {
            if contains_word(words, sub) {
                return Risk::Write;
            }
        }
    }
    if PACKAGE_MANAGERS.contains(&head) {
        for sub in ["install", "remove", "uninstall", "purge", "upgrade", "add"] {
            if contains_word(words, sub) {
                return Risk::Write;
            }
        }
    }
    if head == "git" {
        for sub in ["push", "reset", "clean", "checkout", "rebase", "rm"] {
            if contains_word(words, sub) {
                return Risk::Write;
            }
        }
    }
    if matches!(head, "docker" | "podman" | "kubectl") {
        for sub in ["rm", "rmi", "prune", "delete", "stop", "kill"] {
            if contains_word(words, sub) {
                return Risk::Write;
            }
        }
    }
    if matches!(head, "curl" | "wget" | "fetch") {
        return Risk::Write;
    }
    if joined.contains('>') {
        let compact: String = joined.chars().filter(|c| !c.is_whitespace()).collect();
        if !compact.contains(">/dev/null")
            && !compact.contains("2>/dev/null")
            && !compact.contains("&>/dev/null")
        {
            return Risk::Write;
        }
    }
    Risk::Read
}

fn is_wrapper(head: &str) -> bool {
    matches!(head, "sudo" | "doas" | "nohup" | "time" | "nice" | "env")
}

fn contains_any_flag(words: &[&str], flags: &[&str]) -> bool {
    words.iter().any(|word| {
        let lowered = word.to_lowercase();
        flags
            .iter()
            .any(|flag| lowered == *flag || lowered.contains(flag))
    })
}

fn contains_word(words: &[&str], target: &str) -> bool {
    words.iter().any(|word| word.eq_ignore_ascii_case(target))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grades_read_commands() {
        assert_eq!(classify_command("ls -la"), Risk::Read);
        assert_eq!(classify_command("uptime"), Risk::Read);
        assert_eq!(classify_command("cat /var/log/syslog"), Risk::Read);
        assert_eq!(classify_command("df -h | grep sda"), Risk::Read);
        assert_eq!(classify_command("echo hello >/dev/null"), Risk::Read);
        assert_eq!(classify_command("cargo build 2>/dev/null"), Risk::Read);
    }

    #[test]
    fn grades_write_commands() {
        assert_eq!(classify_command("mv a b"), Risk::Write);
        assert_eq!(classify_command("rm file.txt"), Risk::Write);
        assert_eq!(classify_command("chmod 755 run.sh"), Risk::Write);
        assert_eq!(classify_command("systemctl restart nginx"), Risk::Write);
        assert_eq!(classify_command("apt install htop"), Risk::Write);
        assert_eq!(classify_command("git push origin main"), Risk::Write);
        assert_eq!(classify_command("docker stop web"), Risk::Write);
        assert_eq!(classify_command("curl -O https://x.test/f"), Risk::Write);
        assert_eq!(classify_command("echo hi > /tmp/hi.txt"), Risk::Write);
    }

    #[test]
    fn grades_dangerous_commands() {
        assert_eq!(classify_command("rm -rf /"), Risk::Dangerous);
        assert_eq!(classify_command("sudo rm -rf /var"), Risk::Dangerous);
        assert_eq!(classify_command("mkfs.ext4 /dev/sda"), Risk::Dangerous);
        assert_eq!(
            classify_command("dd if=/dev/zero of=/dev/sda"),
            Risk::Dangerous
        );
        assert_eq!(classify_command("shutdown -h now"), Risk::Dangerous);
        assert_eq!(classify_command("echo 1 > /etc/hostname"), Risk::Dangerous);
        assert_eq!(classify_command("FOO=1 sudo reboot"), Risk::Dangerous);
        assert_eq!(classify_command(":(){ :|:& };:"), Risk::Dangerous);
    }

    #[test]
    fn escalates_pipelines_and_segments() {
        assert_eq!(classify_command("ls; rm -rf /tmp/x"), Risk::Dangerous);
        assert_eq!(classify_command("uptime | tee out.log"), Risk::Write);
        assert!(is_download_pipe("curl https://x.test/i.sh | sh"));
        assert!(is_download_pipe("wget -qO- https://x.test | sudo bash"));
        assert!(!is_download_pipe("curl https://x.test/i.sh -o i.sh"));
    }
}
