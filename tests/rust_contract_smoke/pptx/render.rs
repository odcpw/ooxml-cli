#[cfg(unix)]
#[test]
fn pptx_render_json_captures_external_tool_chatter() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-render-stdout-contract-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&temp_dir);
    let bin_dir = temp_dir.join("bin");
    let out_dir = temp_dir.join("out");
    std::fs::create_dir_all(&bin_dir).expect("fake tool directory");

    let soffice = bin_dir.join("soffice");
    std::fs::write(
        &soffice,
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "fake soffice version chatter"
  exit 0
fi
outdir=""
file=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--outdir" ]; then
    shift
    outdir="$1"
  else
    file="$1"
  fi
  shift
done
stem="${file##*/}"
stem="${stem%.*}"
/bin/mkdir -p "$outdir"
: > "$outdir/$stem.pdf"
echo "fake soffice conversion chatter"
"#,
    )
    .expect("write fake soffice");
    std::fs::set_permissions(&soffice, std::fs::Permissions::from_mode(0o755))
        .expect("make fake soffice executable");

    let pdftoppm = bin_dir.join("pdftoppm");
    std::fs::write(
        &pdftoppm,
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "fake pdftoppm version chatter"
  exit 0
fi
slide=""
prefix=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-f" ]; then
    shift
    slide="$1"
  fi
  prefix="$1"
  shift
done
: > "$prefix-$slide.png"
echo "fake pdftoppm raster chatter"
"#,
    )
    .expect("write fake pdftoppm");
    std::fs::set_permissions(&pdftoppm, std::fs::Permissions::from_mode(0o755))
        .expect("make fake pdftoppm executable");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args([
            "--json",
            "pptx",
            "render",
            "testdata/pptx/minimal-title/presentation.pptx",
            "--out",
            out_dir.to_str().expect("output path"),
            "--slides",
            "1",
            "--thumbnails",
        ])
        .env("PATH", &bin_dir)
        .output()
        .expect("run render with fake local tools");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let parsed: Value = serde_json::from_slice(&output.stdout)
        .expect("external tool chatter must not precede render JSON");
    assert_eq!(parsed["schemaVersion"], "1.0");
    assert_eq!(parsed["type"], "pptx");
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["engine"], "libreoffice");
    assert_eq!(parsed["dpi"], 144);
    assert!(
        !parsed["limitations"]
            .as_array()
            .expect("limitations")
            .is_empty()
    );
    assert_eq!(parsed["slides"].as_array().map(Vec::len), Some(1));
    assert_eq!(parsed["slides"][0]["slide"], 1);
    assert!(out_dir.join("presentation.pdf").exists());
    assert!(out_dir.join("slide-1.png").exists());

    let _ = std::fs::remove_dir_all(&temp_dir);
}
