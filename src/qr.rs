use crate::parser::PaymentInfo;
use image::Luma;
use qrcode::QrCode;
use std::path::Path;

pub fn build_ips_string(info: &PaymentInfo) -> String {
    let mut parts: Vec<String> = vec![
        "K:PR".to_string(),
        "V:01".to_string(),
        "C:1".to_string(),
        format!("R:{}", info.account),
        format!("N:{}", info.name),
    ];

    // Amount: ensure RSD prefix, comma decimal
    let amount = if info.amount.starts_with("RSD") {
        info.amount.clone()
    } else {
        format!("RSD{}", info.amount)
    };
    parts.push(format!("I:{}", amount));

    parts.push(format!("SF:{}", info.code));
    parts.push(format!("S:{}", info.purpose));

    if let Some(ref reference) = info.reference {
        parts.push(format!("RO:{}", reference));
    }

    parts.join("|")
}

pub fn generate_qr_image(payload: &str, output: &Path, size: u32) -> Result<(), String> {
    let code = QrCode::new(payload.as_bytes())
        .map_err(|e| format!("QR encode error: {}", e))?;

    let img = code
        .render::<Luma<u8>>()
        .quiet_zone(true)
        .min_dimensions(size, size)
        .build();

    img.save(output)
        .map_err(|e| format!("Image save error: {}", e))?;

    Ok(())
}

pub fn generate_qr_bytes(payload: &str, size: u32) -> Result<Vec<u8>, String> {
    let code = QrCode::new(payload.as_bytes())
        .map_err(|e| format!("QR encode error: {}", e))?;

    let img = code
        .render::<Luma<u8>>()
        .quiet_zone(true)
        .min_dimensions(size, size)
        .build();

    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| format!("PNG encode error: {}", e))?;

    Ok(buf.into_inner())
}
