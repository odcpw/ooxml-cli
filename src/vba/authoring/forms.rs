use std::collections::BTreeMap;

use super::model::VbaModuleModel;

const USERFORM_CLSID: &str = "{C62A69F0-16DC-11CE-9E98-00AA00574A4F}";

pub(super) fn render_user_form_storage_streams(
    module: &VbaModuleModel,
) -> BTreeMap<String, Vec<u8>> {
    let mut streams = BTreeMap::new();
    let storage = &module.stream_name;
    streams.insert(
        format!("{storage}/\u{0001}CompObj"),
        render_comp_obj_stream(),
    );
    streams.insert(
        format!("{storage}/\u{0003}VBFrame"),
        render_vb_frame_stream(module),
    );
    streams.insert(format!("{storage}/f"), render_form_control_stream(module));
    streams.insert(format!("{storage}/o"), Vec::new());
    streams
}

fn render_comp_obj_stream() -> Vec<u8> {
    vec![
        0x01, 0x00, 0xFE, 0xFF, 0x03, 0x0A, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x19, 0x00,
        0x00, 0x00, b'M', b'i', b'c', b'r', b'o', b's', b'o', b'f', b't', b' ', b'F', b'o', b'r',
        b'm', b's', b' ', b'2', b'.', b'0', b' ', b'F', b'o', b'r', b'm', 0x00, 0x10, 0x00, 0x00,
        0x00, b'E', b'm', b'b', b'e', b'd', b'd', b'e', b'd', b' ', b'O', b'b', b'j', b'e', b'c',
        b't', 0x00, 0x00, 0x00, 0x00, 0x00, 0xF4, 0x39, 0xB2, 0x71, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]
}

fn render_vb_frame_stream(module: &VbaModuleModel) -> Vec<u8> {
    let caption = module
        .user_form
        .as_ref()
        .map(|form| form.caption.as_str())
        .unwrap_or(module.name.as_str());
    format!(
        "VERSION 5.00\r\n\
Begin {USERFORM_CLSID} {} \r\n\
   Caption         =   \"{}\"\r\n\
   ClientHeight    =   3015\r\n\
   ClientLeft      =   120\r\n\
   ClientTop       =   465\r\n\
   ClientWidth     =   4560\r\n\
   StartUpPosition =   1  'CenterOwner\r\n\
   TypeInfoVer     =   2\r\n\
End\r\n",
        module.name,
        escape_frame_string(caption)
    )
    .into_bytes()
}

fn render_form_control_stream(_module: &VbaModuleModel) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(0x00);
    out.push(0x04);
    let cb_form_pos = out.len();
    out.extend([0x00, 0x00]);

    let prop_mask = (1_u32 << 3) | (1_u32 << 10) | (1_u32 << 11) | (1_u32 << 26) | (1_u32 << 27);
    out.extend(prop_mask.to_le_bytes());
    out.extend(1_u32.to_le_bytes());
    out.extend(0_u32.to_le_bytes());
    out.extend(32_000_u32.to_le_bytes());
    out.extend(5_720_u32.to_le_bytes());
    out.extend(3_640_u32.to_le_bytes());
    out.extend(0_u32.to_le_bytes());
    out.extend(0_u32.to_le_bytes());

    let cb_form = (out.len() - 4) as u16;
    out[cb_form_pos..cb_form_pos + 2].copy_from_slice(&cb_form.to_le_bytes());

    out.extend(0_u16.to_le_bytes());
    out.extend(0_u32.to_le_bytes());
    out.extend(0_u32.to_le_bytes());
    out
}

fn escape_frame_string(value: &str) -> String {
    value.replace('"', "\"\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vba::authoring::model::{VbaModuleModel, VbaUserFormModel};

    #[test]
    fn renders_pinned_minimal_userform_storage_streams() {
        let module = VbaModuleModel::user_form(
            "MyForm",
            None::<String>,
            b"Attribute VB_Name = \"MyForm\"\r\n".to_vec(),
            VbaUserFormModel::new("Café \"Dialog\""),
        );
        let streams = render_user_form_storage_streams(&module);

        let expected_comp_obj = vec![
            0x01, 0x00, 0xFE, 0xFF, 0x03, 0x0A, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x19, 0x00, 0x00, 0x00, b'M', b'i', b'c', b'r', b'o', b's', b'o', b'f', b't', b' ',
            b'F', b'o', b'r', b'm', b's', b' ', b'2', b'.', b'0', b' ', b'F', b'o', b'r', b'm',
            0x00, 0x10, 0x00, 0x00, 0x00, b'E', b'm', b'b', b'e', b'd', b'd', b'e', b'd', b' ',
            b'O', b'b', b'j', b'e', b'c', b't', 0x00, 0x00, 0x00, 0x00, 0x00, 0xF4, 0x39, 0xB2,
            0x71, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        assert_eq!(streams["MyForm/\u{0001}CompObj"], expected_comp_obj);

        // MS-OFORMS FormControl layout, pinned byte-for-byte:
        // - 00 04: MinorVersion 0, MajorVersion 4.
        // - 20 00: cbForm = 32 bytes for PropMask + DataBlock + ExtraDataBlock.
        // - 08 0c 00 0c: FormPropMask 0x0c000c08, selecting NextAvailableID,
        //   DisplayedSize, LogicalSize, ShapeCookie, and the required DrawBuffer.
        // - DataBlock: NextAvailableID=1, ShapeCookie=0, DrawBuffer=32000.
        // - ExtraDataBlock: DisplayedSize=(5720,3640), LogicalSize=(0,0).
        // - FormSiteData: zero class-info entries, zero sites, and zero site bytes.
        // fCaption is not selected, so no caption bytes occur in `f`; VBA 5 stores
        // this UserForm caption in the textual VBFrame stream asserted below.
        let expected_f = vec![
            0x00, 0x04, // MinorVersion, MajorVersion
            0x20, 0x00, // cbForm = 32
            0x08, 0x0C, 0x00, 0x0C, // FormPropMask = 0x0c000c08
            0x01, 0x00, 0x00, 0x00, // NextAvailableID = 1
            0x00, 0x00, 0x00, 0x00, // ShapeCookie = 0
            0x00, 0x7D, 0x00, 0x00, // DrawBuffer = 32000
            0x58, 0x16, 0x00, 0x00, // DisplayedSize.cx = 5720
            0x38, 0x0E, 0x00, 0x00, // DisplayedSize.cy = 3640
            0x00, 0x00, 0x00, 0x00, // LogicalSize.cx = 0
            0x00, 0x00, 0x00, 0x00, // LogicalSize.cy = 0
            0x00, 0x00, // CountOfSiteClassInfo = 0
            0x00, 0x00, 0x00, 0x00, // CountOfSites = 0
            0x00, 0x00, 0x00, 0x00, // CountOfBytes = 0
        ];
        assert_eq!(streams["MyForm/f"], expected_f);

        let expected_vb_frame = concat!(
            "VERSION 5.00\r\n",
            "Begin {C62A69F0-16DC-11CE-9E98-00AA00574A4F} MyForm \r\n",
            "Caption         =   \"Café \"\"Dialog\"\"\"\r\n",
            "ClientHeight    =   3015\r\n",
            "ClientLeft      =   120\r\n",
            "ClientTop       =   465\r\n",
            "ClientWidth     =   4560\r\n",
            "StartUpPosition =   1  'CenterOwner\r\n",
            "TypeInfoVer     =   2\r\n",
            "End\r\n",
        );
        assert_eq!(
            streams["MyForm/\u{0003}VBFrame"],
            expected_vb_frame.as_bytes()
        );
        assert_eq!(streams["MyForm/o"], Vec::<u8>::new());
    }
}
