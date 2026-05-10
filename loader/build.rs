use std::io;

use winres::WindowsResource;

const APP_MANIFEST: &str = r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <description>Holiton Loader</description>
  <assemblyIdentity type="win32" name="dev.holiton.loader" version="0.5.18.0" />
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
      <security>
          <requestedPrivileges>
              <requestedExecutionLevel level="asInvoker" uiAccess="false" />
          </requestedPrivileges>
      </security>
  </trustInfo>
  <asmv3:application xmlns:asmv3="urn:schemas-microsoft-com:asm.v3">
    <asmv3:windowsSettings xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">
      <dpiAware>True/PM</dpiAware>
    </asmv3:windowsSettings>
  </asmv3:application>
</assembly>
"#;

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=resources/app-icon.ico");
    println!("cargo:rerun-if-changed=build.rs");

    let mut resource = WindowsResource::new();
    resource.set_icon("./resources/app-icon.ico");
    resource.set_manifest(APP_MANIFEST);
    resource.compile()?;
    Ok(())
}
