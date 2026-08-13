//! Embed Windows PE VERSIONINFO so Explorer / SmartScreen show a real publisher/product.
fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }

    // Cross-build on Linux uses the MinGW windres.
    if std::env::var_os("HOST").is_some() {
        if let Ok(host) = std::env::var("HOST") {
            if !host.contains("windows") {
                std::env::set_var("WINDRES", "x86_64-w64-mingw32-windres");
            }
        }
    }

    let ver = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".into());
    let mut parts = ver.split('.');
    let major: u64 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor: u64 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch: u64 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let packed = (major << 48) | (minor << 32) | (patch << 16);

    let mut res = winres::WindowsResource::new();
    res.set("FileDescription", "Njörðr Seas' CYD miner — USB SHA-256 mining companion");
    res.set("ProductName", "Njörðr Seas' CYD miner");
    res.set("CompanyName", "GutFarms");
    res.set("LegalCopyright", "Copyright (c) GutFarms");
    res.set("OriginalFilename", "cyd-companion.exe");
    res.set("InternalName", "cyd-companion");
    res.set("FileVersion", &ver);
    res.set("ProductVersion", &ver);
    res.set_version_info(winres::VersionInfo::FILEVERSION, packed);
    res.set_version_info(winres::VersionInfo::PRODUCTVERSION, packed);
    // App / Desktop / Start Menu icon (multi-size ICO).
    let icon = std::path::Path::new("assets").join("cyd-miner.ico");
    if icon.is_file() {
        res.set_icon(icon.to_str().unwrap_or("assets/cyd-miner.ico"));
        println!("cargo:rerun-if-changed=assets/cyd-miner.ico");
    } else {
        println!("cargo:warning=missing assets/cyd-miner.ico — PE icon not embedded");
    }
    // asInvoker — no UAC prompt for normal use; Update board elevates separately if needed.
    res.set_manifest(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity version="1.0.0.0" processorArchitecture="*" name="GutFarms.CYDCompanion" type="win32"/>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
      <supportedOS Id="{1f676c76-80e1-4239-95bb-83d0f6d0da78}"/>
      <supportedOS Id="{4a2f28e3-53b9-4441-ba9c-d69d4a4a6e38}"/>
      <supportedOS Id="{35138b9a-5d96-4fbd-8e2d-a2440225f93a}"/>
      <supportedOS Id="{e2011457-1546-43c5-a5fe-008deee3d3f0}"/>
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
    </windowsSettings>
  </application>
</assembly>"#,
    );
    if let Err(e) = res.compile() {
        println!("cargo:warning=winres compile failed: {e}");
    }
}
