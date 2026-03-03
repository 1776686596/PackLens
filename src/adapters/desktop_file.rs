use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Instant;

use freedesktop_desktop_entry::DesktopEntry;
use walkdir::WalkDir;

use crate::adapters::util::*;
use crate::adapters::PackageAdapter;
use crate::models::{detect_install_method, make_canonical_id, AdapterResult, Package};
use crate::subprocess::run_command;

pub struct DesktopFileAdapter;

impl PackageAdapter for DesktopFileAdapter {
    fn name(&self) -> &str {
        "desktop-file"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn list_packages(&self) -> impl Future<Output = AdapterResult<Package>> + Send {
        async move {
            let started = Instant::now();
            let mut items = Vec::new();
            let mut warnings = Vec::new();
            let mut resolve_cache: HashMap<String, Option<String>> = HashMap::new();
            let mut dpkg_owner_cache: HashMap<String, Option<String>> = HashMap::new();

            for dir in desktop_dirs() {
                if !dir.exists() {
                    continue;
                }

                for entry in WalkDir::new(&dir).follow_links(false).into_iter().flatten() {
                    if !entry.file_type().is_file() {
                        continue;
                    }
                    let path = entry.path();
                    let is_desktop = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| e.eq_ignore_ascii_case("desktop"));
                    if !is_desktop {
                        continue;
                    }

                    let de = match DesktopEntry::from_path(path, None::<&[&str]>) {
                        Ok(e) => e,
                        Err(e) => {
                            warnings.push(format!(
                                "desktop-file: failed to parse {}: {e}",
                                path.display()
                            ));
                            continue;
                        }
                    };

                    if de.no_display() || de.hidden() {
                        continue;
                    }

                    let exec_raw = de.exec().unwrap_or_default();
                    let exec_path = extract_exec_path(exec_raw);
                    let resolved_exec_path =
                        resolve_exec_path(exec_path.clone(), &mut resolve_cache).await;
                    let install_method =
                        detect_install_method_for_desktop(exec_raw, resolved_exec_path.as_deref());
                    let source = source_from_install_method(install_method);

                    let fallback = path
                        .file_stem()
                        .and_then(|v| v.to_str())
                        .filter(|v| !v.is_empty())
                        .unwrap_or("unknown")
                        .to_string();

                    let mut key =
                        exec_basename(exec_path.as_deref()).unwrap_or_else(|| fallback.clone());
                    if install_method == "flatpak" {
                        if let Some(app_id) = extract_flatpak_app_id(exec_raw) {
                            key = app_id;
                        }
                    }
                    if install_method == "wine" {
                        if let Some(wine_target_key) = extract_wine_target_key(exec_raw) {
                            key = wine_target_key;
                        }
                    }
                    if install_method == "apt" {
                        if let Some(owner) =
                            resolve_dpkg_owner(resolved_exec_path.as_deref(), &mut dpkg_owner_cache)
                                .await
                        {
                            key = owner;
                        }
                    }

                    let name = de
                        .name::<&str>(&[])
                        .map(|v| v.into_owned())
                        .filter(|v| !v.is_empty())
                        .unwrap_or_else(|| fallback.clone());

                    let description = de
                        .comment::<&str>(&[])
                        .map(|v| v.into_owned())
                        .unwrap_or_default();

                    let desktop_file_path = path.to_string_lossy().into_owned();

                    items.push(Package {
                        canonical_id: make_canonical_id(source, &key),
                        name,
                        version: String::new(),
                        source: source.to_string(),
                        install_method: install_method.to_string(),
                        install_path: resolved_exec_path
                            .clone()
                            .or_else(|| Some(path.to_string_lossy().into_owned())),
                        uninstall_command: uninstall_hint(
                            install_method,
                            &key,
                            resolved_exec_path.as_deref(),
                            exec_raw,
                            Some(&desktop_file_path),
                        ),
                        size: resolved_exec_path.as_deref().and_then(file_size_if_regular),
                        description,
                        icon_name: de.icon().map(ToString::to_string),
                        desktop_file: Some(desktop_file_path),
                    });
                }
            }

            AdapterResult {
                items,
                warnings,
                duration_ms: elapsed_ms(&started),
                timestamp: now_timestamp(),
            }
        }
    }
}

fn desktop_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(Path::new(&home).join(".local/share/applications"));
    }
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    dirs
}

fn extract_exec_path(exec: &str) -> Option<String> {
    let mut tokens = exec.split_whitespace();
    let mut candidate = tokens.next()?;
    if candidate == "env" || candidate.contains('=') {
        candidate = tokens.find(|t| !t.contains('=') && !t.starts_with('%'))?;
    }
    let cleaned = candidate.trim_matches('"').trim_matches('\'');
    if cleaned.is_empty() || cleaned.starts_with('%') {
        return None;
    }
    Some(cleaned.to_string())
}

fn detect_install_method_for_desktop(exec_raw: &str, exec_path: Option<&str>) -> &'static str {
    if extract_steam_game_id(exec_raw).is_some() {
        return "steam";
    }
    if extract_flatpak_app_id(exec_raw).is_some() {
        return "flatpak";
    }
    if is_wine_exec(exec_raw, exec_path) {
        return "wine";
    }

    match exec_path {
        Some(path) if path.ends_with(".AppImage") => "appimage",
        Some(path) if path.contains("/snap/") => "snap",
        Some(path) if path.contains("/flatpak/") => "flatpak",
        Some(path) => {
            let method = detect_install_method(path);
            if method == "unknown" {
                "manual"
            } else {
                method
            }
        }
        None => "manual",
    }
}

fn source_from_install_method(install_method: &str) -> &str {
    match install_method {
        "apt" | "snap" | "flatpak" | "appimage" | "steam" => install_method,
        _ => "manual",
    }
}

fn exec_basename(exec_path: Option<&str>) -> Option<String> {
    let p = exec_path?;
    let fname = Path::new(p)
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or(p);
    let stem = Path::new(fname)
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or(fname);
    if stem.is_empty() {
        None
    } else {
        Some(stem.to_string())
    }
}

fn uninstall_hint(
    install_method: &str,
    key: &str,
    exec_path: Option<&str>,
    exec_raw: &str,
    desktop_file: Option<&str>,
) -> Option<String> {
    match install_method {
        "apt" => Some(format!("sudo apt remove {}", key)),
        "snap" => Some(format!("sudo snap remove {}", key)),
        "flatpak" => Some(format!("flatpak uninstall {}", key)),
        "npm" => Some(format!("npm uninstall -g {}", key)),
        "cargo" => Some(format!("cargo uninstall {}", key)),
        "uv" => Some(format!("uv tool uninstall {}", key)),
        "pipx" => Some(format!("pipx uninstall {}", key)),
        "steam" => Some(build_steam_uninstall_command(exec_raw)),
        "wine" => Some(build_wine_uninstall_command(exec_raw)),
        "appimage" => exec_path.map(|p| format!("rm -f '{}'", p.replace('\'', "'\\''"))),
        "manual" => Some(build_manual_uninstall_command(exec_path, desktop_file, key)),
        _ => None,
    }
}

fn extract_flatpak_app_id(exec_raw: &str) -> Option<String> {
    let tokens = parse_exec_tokens(exec_raw);
    let flatpak_pos = tokens.iter().position(|token| {
        Path::new(token)
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "flatpak")
    })?;

    if tokens.get(flatpak_pos + 1).is_none_or(|t| t != "run") {
        return None;
    }

    for token in tokens.iter().skip(flatpak_pos + 2) {
        if token.starts_with('-') {
            continue;
        }
        return Some(token.to_string());
    }

    None
}

fn extract_steam_game_id(exec_raw: &str) -> Option<String> {
    let tokens = parse_exec_tokens(exec_raw);
    for token in tokens {
        let Some(rest) = token.strip_prefix("steam://rungameid/") else {
            continue;
        };
        let game_id: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !game_id.is_empty() {
            return Some(game_id);
        }
    }
    None
}

fn is_flatpak_steam_exec(exec_raw: &str) -> bool {
    let tokens = parse_exec_tokens(exec_raw);
    let has_flatpak = tokens.iter().any(|token| {
        Path::new(token)
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "flatpak")
    });
    has_flatpak
        && tokens
            .iter()
            .any(|token| token == "com.valvesoftware.Steam")
}

fn build_steam_uninstall_command(exec_raw: &str) -> String {
    let Some(game_id) = extract_steam_game_id(exec_raw) else {
        return "steam".to_string();
    };

    if is_flatpak_steam_exec(exec_raw) {
        format!("flatpak run com.valvesoftware.Steam steam://uninstall/{game_id}")
    } else {
        format!("steam steam://uninstall/{game_id}")
    }
}

fn build_manual_uninstall_command(
    exec_path: Option<&str>,
    desktop_file: Option<&str>,
    key: &str,
) -> String {
    let mut files = Vec::<String>::new();
    let mut dirs = Vec::<String>::new();

    if let Some(exec) = exec_path {
        let cleaned = exec.trim();
        if !cleaned.is_empty() && cleaned != "-" {
            let p = Path::new(cleaned);
            if p.is_dir() {
                dirs.push(cleaned.to_string());
            } else {
                files.push(cleaned.to_string());
            }
        }
    }

    if let Some(desktop) = desktop_file {
        let cleaned = desktop.trim();
        if !cleaned.is_empty() && cleaned != "-" && !files.iter().any(|f| f == cleaned) {
            files.push(cleaned.to_string());
        }
    }

    let mut commands = Vec::<String>::new();
    for file in &files {
        let quoted = format!("'{}'", shell_quote_single(file));
        let use_sudo = manual_path_requires_sudo(file);
        let sudo_prefix = if use_sudo { "sudo " } else { "" };
        let msg_remove = format!("'{}'", shell_quote_single(&format!("[manual] remove file: {file}")));
        let msg_skip =
            format!("'{}'", shell_quote_single(&format!("[manual] skip missing file: {file}")));
        commands.push(format!(
            "if {sudo_prefix}test -e {quoted}; then echo {msg_remove} && {sudo_prefix}ls -ld {quoted} && {sudo_prefix}rm -f {quoted}; else echo {msg_skip}; fi"
        ));
    }
    for dir in &dirs {
        let quoted = format!("'{}'", shell_quote_single(dir));
        let use_sudo = manual_path_requires_sudo(dir);
        let sudo_prefix = if use_sudo { "sudo " } else { "" };
        let msg_remove = format!("'{}'", shell_quote_single(&format!("[manual] remove dir: {dir}")));
        let msg_skip =
            format!("'{}'", shell_quote_single(&format!("[manual] skip missing dir: {dir}")));
        commands.push(format!(
            "if {sudo_prefix}test -d {quoted}; then echo {msg_remove} && {sudo_prefix}ls -ld {quoted} && {sudo_prefix}rm -rf {quoted}; else echo {msg_skip}; fi"
        ));
    }

    if commands.is_empty() {
        format!("manual uninstall (remove executable/app dir): {key}")
    } else {
        commands.join("; ")
    }
}

fn manual_path_requires_sudo(path: &str) -> bool {
    let path = path.trim();
    if !path.starts_with('/') {
        return false;
    }

    // 常见用户目录与临时目录通常不需要 sudo。
    if path.starts_with("/home/") || path.starts_with("/run/user/") {
        return false;
    }
    if path.starts_with("/tmp/") || path.starts_with("/var/tmp/") {
        return false;
    }

    // 系统目录一般需要管理员权限删除。
    let system_prefixes = [
        "/usr/",
        "/opt/",
        "/etc/",
        "/var/",
        "/bin/",
        "/sbin/",
        "/lib/",
        "/lib64/",
        "/snap/",
        "/srv/",
        "/root/",
    ];
    system_prefixes.iter().any(|prefix| path.starts_with(prefix))
}

fn parse_exec_tokens(exec_raw: &str) -> Vec<String> {
    let mut tokens = exec_raw
        .split_whitespace()
        .map(|token| token.trim_matches('"').trim_matches('\''))
        .filter(|token| !token.is_empty() && !token.starts_with('%'))
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    if tokens.first().is_some_and(|token| token == "env") {
        tokens.remove(0);
    }
    while tokens.first().is_some_and(|token| token.contains('=')) {
        tokens.remove(0);
    }

    tokens
}

fn is_wine_exec(exec_raw: &str, exec_path: Option<&str>) -> bool {
    if exec_path.is_some_and(is_wine_command_token) {
        return true;
    }
    let tokens = parse_exec_tokens(exec_raw);
    tokens
        .first()
        .is_some_and(|token| is_wine_command_token(token))
}

fn is_wine_command_token(token: &str) -> bool {
    let name = Path::new(token)
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or(token)
        .to_ascii_lowercase();
    matches!(
        name.as_str(),
        "wine" | "wine64" | "wine-stable" | "wine-staging" | "wine-development"
    )
}

fn extract_wine_target_exec(exec_raw: &str) -> Option<String> {
    let tokens = parse_exec_tokens(exec_raw);
    let wine_pos = tokens
        .iter()
        .position(|token| is_wine_command_token(token))?;

    let args: Vec<&str> = tokens
        .iter()
        .skip(wine_pos + 1)
        .map(String::as_str)
        .filter(|token| !token.starts_with('-') && *token != "start" && *token != "/unix")
        .collect();

    for (index, token) in args.iter().enumerate() {
        if !token.contains("\\") && !token.contains(':') {
            continue;
        }

        let mut combined = (*token).to_string();
        let lower = combined.to_ascii_lowercase();
        if lower.ends_with(".exe") || lower.ends_with(".msi") {
            return Some(combined);
        }

        for suffix in args.iter().skip(index + 1) {
            combined.push(' ');
            combined.push_str(suffix);
            let lower = combined.to_ascii_lowercase();
            if lower.ends_with(".exe") || lower.ends_with(".msi") {
                return Some(combined);
            }
        }
    }

    args.iter()
        .rev()
        .find(|token| {
            let lower = token.to_ascii_lowercase();
            lower.ends_with(".exe") || lower.ends_with(".msi")
        })
        .map(|token| (*token).to_string())
}

fn extract_wine_target_key(exec_raw: &str) -> Option<String> {
    let target = extract_wine_target_exec(exec_raw)?;
    let normalized = target.replace('\\', "/");
    Path::new(&normalized)
        .file_stem()
        .and_then(|v| v.to_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
}

fn extract_env_assignment(exec_raw: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    exec_raw
        .split_whitespace()
        .map(|token| token.trim_matches('"').trim_matches('\''))
        .find_map(|token| token.strip_prefix(&prefix).map(ToString::to_string))
}

fn shell_quote_single(text: &str) -> String {
    text.replace('\'', "'\\''")
}

fn build_wine_uninstall_command(exec_raw: &str) -> String {
    let prefix_part = extract_env_assignment(exec_raw, "WINEPREFIX")
        .map(|prefix| format!("WINEPREFIX='{}' ", shell_quote_single(&prefix)))
        .unwrap_or_default();

    if let Some(target_exec) = extract_wine_target_exec(exec_raw) {
        format!(
            "{}wine '{}' /uninstall || {}wine uninstaller",
            prefix_part,
            shell_quote_single(&target_exec),
            prefix_part
        )
    } else {
        format!("{}wine uninstaller", prefix_part)
    }
}

async fn resolve_dpkg_owner(
    exec_path: Option<&str>,
    cache: &mut HashMap<String, Option<String>>,
) -> Option<String> {
    let path = exec_path?;
    if !path.starts_with('/') || !command_exists("dpkg-query") {
        return None;
    }

    if let Some(cached) = cache.get(path) {
        return cached.clone();
    }

    let owner = run_command("dpkg-query", &["-S", path], 8)
        .await
        .ok()
        .and_then(|output| parse_dpkg_owner(&output.stdout));

    cache.insert(path.to_string(), owner.clone());
    owner
}

fn parse_dpkg_owner(stdout: &str) -> Option<String> {
    let line = stdout
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && line.contains(": "))?;
    let (owners, _) = line.split_once(": ")?;
    let owner = owners.split(',').next()?.trim();
    if owner.is_empty() {
        None
    } else {
        Some(owner.to_string())
    }
}

async fn resolve_exec_path(
    exec_path: Option<String>,
    cache: &mut HashMap<String, Option<String>>,
) -> Option<String> {
    let path = exec_path?;
    if path.contains('/') {
        return Some(path);
    }

    if let Some(cached) = cache.get(&path) {
        return cached.clone();
    }

    let resolved = resolve_path(&path).await;
    cache.insert(path, resolved.clone());
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_flatpak_install_method_from_exec_raw() {
        assert_eq!(
            detect_install_method_for_desktop(
                "/usr/bin/flatpak run org.mozilla.firefox",
                Some("/usr/bin/flatpak")
            ),
            "flatpak"
        );
        assert_eq!(
            detect_install_method_for_desktop(
                "env FOO=bar flatpak run --branch=stable org.gnome.Calculator",
                Some("/usr/bin/flatpak")
            ),
            "flatpak"
        );
    }

    #[test]
    fn detect_install_method_from_exec_path() {
        assert_eq!(
            detect_install_method_for_desktop("/snap/bin/firefox %u", Some("/snap/bin/firefox")),
            "snap"
        );
        assert_eq!(
            detect_install_method_for_desktop(
                "/home/u/Tool.AppImage",
                Some("/home/u/Tool.AppImage")
            ),
            "appimage"
        );
        assert_eq!(
            detect_install_method_for_desktop("/usr/bin/gedit", Some("/usr/bin/gedit")),
            "apt"
        );
    }

    #[test]
    fn extract_flatpak_app_id_from_exec() {
        assert_eq!(
            extract_flatpak_app_id("flatpak run org.mozilla.firefox %u"),
            Some("org.mozilla.firefox".to_string())
        );
        assert_eq!(
            extract_flatpak_app_id(
                "env BAMF_DESKTOP_FILE_HINT=/var/lib/flatpak/app/org.mozilla.firefox foo=bar flatpak run --arch=x86_64 org.mozilla.firefox"
            ),
            Some("org.mozilla.firefox".to_string())
        );
    }

    #[test]
    fn parse_dpkg_owner_with_arch_and_multi_owner() {
        assert_eq!(
            parse_dpkg_owner("libc6:amd64: /usr/lib/x86_64-linux-gnu/libc.so.6\n"),
            Some("libc6:amd64".to_string())
        );
        assert_eq!(
            parse_dpkg_owner("python3, python3-minimal: /usr/bin/python3\n"),
            Some("python3".to_string())
        );
    }

    #[test]
    fn uninstall_hint_uses_install_method() {
        assert_eq!(
            uninstall_hint("apt", "firefox", None, "", None),
            Some("sudo apt remove firefox".to_string())
        );
        assert_eq!(
            uninstall_hint("flatpak", "org.mozilla.firefox", None, "", None),
            Some("flatpak uninstall org.mozilla.firefox".to_string())
        );
    }

    #[test]
    fn manual_uninstall_prefers_real_paths() {
        let cmd = uninstall_hint(
            "manual",
            "foo",
            Some("/opt/foo/foo"),
            "",
            Some("/home/u/.local/share/applications/foo.desktop"),
        )
        .expect("manual uninstall command");
        assert!(cmd.contains("[manual] remove file:"));
        assert!(cmd.contains("sudo ls -ld '/opt/foo/foo'"));
        assert!(cmd.contains("sudo rm -f '/opt/foo/foo'"));
        assert!(cmd.contains("'/home/u/.local/share/applications/foo.desktop'"));
    }

    #[test]
    fn detect_wine_install_method() {
        assert_eq!(
            detect_install_method_for_desktop(
                "env WINEPREFIX=/home/u/.wine-game wine C:\\\\Games\\\\foo\\\\foo.exe",
                Some("/usr/bin/wine")
            ),
            "wine"
        );
    }

    #[test]
    fn extract_wine_target_key_from_exec() {
        assert_eq!(
            extract_wine_target_key("wine C:\\\\Program Files\\\\FooGame\\\\foo.exe"),
            Some("foo".to_string())
        );
    }

    #[test]
    fn build_wine_uninstall_command_with_prefix() {
        let cmd = build_wine_uninstall_command(
            "env WINEPREFIX=/home/u/.wine-foo wine C:\\\\Games\\\\foo\\\\foo.exe",
        );
        assert!(cmd.contains("WINEPREFIX='/home/u/.wine-foo'"));
        assert!(cmd.contains("wine 'C:\\\\Games\\\\foo\\\\foo.exe' /uninstall"));
        assert!(cmd.contains("wine uninstaller"));
    }

    #[test]
    fn detect_steam_install_method() {
        assert_eq!(
            detect_install_method_for_desktop(
                "steam steam://rungameid/570",
                Some("/usr/games/steam")
            ),
            "steam"
        );
    }

    #[test]
    fn extract_steam_game_id_from_exec() {
        assert_eq!(
            extract_steam_game_id("steam steam://rungameid/730 -silent"),
            Some("730".to_string())
        );
    }

    #[test]
    fn build_steam_uninstall_command_from_exec() {
        assert_eq!(
            build_steam_uninstall_command("steam steam://rungameid/570"),
            "steam steam://uninstall/570"
        );
        assert_eq!(
            build_steam_uninstall_command(
                "flatpak run com.valvesoftware.Steam steam://rungameid/730"
            ),
            "flatpak run com.valvesoftware.Steam steam://uninstall/730"
        );
    }
}
