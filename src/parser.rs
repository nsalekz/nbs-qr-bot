use regex::Regex;

#[derive(Debug, Clone)]
pub struct PaymentInfo {
    pub account: String,  // 18 digits, no hyphens
    pub name: String,     // payee name + address, \r\n separated
    pub amount: String,   // numeric, comma decimal (e.g. "1300," or "1300,50")
    pub purpose: String,  // max 35 chars
    pub code: String,     // 3-digit payment code
    pub reference: Option<String>,
}

pub fn parse_payment(text: &str) -> Result<PaymentInfo, String> {
    let account = extract_account(text)?;
    let amount = extract_amount(text)?;
    let name = extract_name(text, &account);
    let purpose = extract_purpose(text);

    Ok(PaymentInfo {
        account,
        name,
        amount,
        purpose,
        code: "289".to_string(),
        reference: None,
    })
}

/// Extract Serbian bank account number.
/// Formats: "160-445519-82", "160-0000000445519-82", "160445519 82", etc.
fn extract_account(text: &str) -> Result<String, String> {
    // Pattern 1: BBB-NNNNN...-CC (standard hyphenated format, variable middle length)
    let re_hyph = Regex::new(r"(\d{3})-(\d{1,13})-(\d{2})").unwrap();
    if let Some(cap) = re_hyph.find_iter(text).next() {
        let caps = re_hyph.captures(cap.as_str()).unwrap();
        let bank = &caps[1];
        let mid = &caps[2];
        let check = &caps[3];
        // Pad middle to 13 digits
        let padded = format!("{}{:0>13}{}", bank, mid, check);
        return Ok(padded);
    }

    // Pattern 2: 18 consecutive digits
    let re_flat = Regex::new(r"\b(\d{18})\b").unwrap();
    if let Some(cap) = re_flat.captures(text) {
        return Ok(cap[1].to_string());
    }

    Err("Nije pronadjen broj racuna. Ocekujem format: 160-445519-82".to_string())
}

/// Extract amount from text.
/// Handles: "1.300 din", "1300 RSD", "iznos 1.300,00", "1,300.00 dinara", etc.
fn extract_amount(text: &str) -> Result<String, String> {
    let lower = text.to_lowercase();

    // Try patterns from most specific to least
    let patterns: &[&str] = &[
        // "1.300,50 din/rsd/dinara" or "1.300 din"
        r"(\d{1,3}(?:\.\d{3})*(?:,\d{1,2})?)\s*(?:din(?:ara)?|rsd)",
        // "iznos/uplat/cena/cen...  1.300,50" or "iznos: 1300"
        r"(?:iznos|uplat|cena|ukupno|svega|za\s+uplatu)[:\s]+(\d{1,3}(?:\.\d{3})*(?:,\d{1,2})?)",
        // Standalone amount with RSD prefix: "RSD 1300" or "RSD1.300,50"
        r"RSD\s*(\d{1,3}(?:\.\d{3})*(?:,\d{1,2})?)",
        // Just a number near "din" or currency context (looser)
        r"(\d+(?:,\d{1,2})?)\s*(?:din(?:ara)?|rsd)",
    ];

    for pat in patterns {
        let re = Regex::new(&format!("(?i){}", pat)).unwrap();
        if let Some(caps) = re.captures(&lower) {
            let raw = caps[1].to_string();
            return Ok(normalize_amount(&raw));
        }
    }

    // Fallback: look for the text around "iznos" more broadly
    let re_broad = Regex::new(r"(?i)(\d[\d\.,]*\d)\s*(?:din|rsd)").unwrap();
    if let Some(caps) = re_broad.captures(text) {
        return Ok(normalize_amount(&caps[1].to_string()));
    }

    Err("Nije pronadjen iznos. Ocekujem npr: '1.300 din' ili 'iznos 1300 RSD'".to_string())
}

/// Normalize amount string to NBS format: no dots, comma as decimal, trailing comma if needed.
/// "1.300" -> "1300,"  |  "1.300,50" -> "1300,50"  |  "1300" -> "1300,"
fn normalize_amount(raw: &str) -> String {
    // Remove thousand-separator dots
    let s = raw.replace('.', "");
    // Ensure there's a comma
    if s.contains(',') {
        s
    } else {
        format!("{},", s)
    }
}

/// Extract payee name. Tries several heuristics:
/// 1. Text immediately after account number in the raw string
/// 2. Line right before or after the account number
/// 3. Business name indicators (DOO, doo, SZR, etc.)
fn extract_name(text: &str, _account_raw: &str) -> String {
    // Strategy 1: grab the text right after the account number pattern in raw text.
    // This works even when the whole dump is a single line.
    // Pattern: account number followed by some text until a stop word/pattern
    let after_account = Regex::new(
        r"(?i)\d{3}-\d{1,13}-\d{2}\s*[,.]?\s*(.+?)(?:\s*(?:MB[:\s]|PIB[:\s]|telefon|kontakt|\d{5}\s|\d{1,3}(?:\.\d{3})*\s*(?:din|rsd)|iznos|ukoliko|hvala))"
    ).unwrap();

    if let Some(caps) = after_account.captures(text) {
        let raw_name = caps[1].trim().trim_end_matches(',').trim_end_matches('.').trim();
        if !raw_name.is_empty() && raw_name.len() > 2 {
            // Try to also grab an address (street + city pattern nearby)
            let addr = extract_address_from_text(text);
            if let Some(a) = addr {
                return truncate_name(&format!("{}\r\n{}", raw_name, a));
            }
            return truncate_name(raw_name);
        }
    }

    // Strategy 2: line-based parsing (works when text has newlines)
    let lines: Vec<&str> = text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();

    let re_hyph = Regex::new(r"\d{3}-\d{1,13}-\d{2}").unwrap();

    let mut account_line_idx: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        if re_hyph.is_match(line) {
            account_line_idx = Some(i);
            break;
        }
    }

    // Try to find a business name (company indicators)
    let biz_re = Regex::new(r"(?i)\b(?:d\.?o\.?o\.?|s\.?z\.?r\.?|a\.?d\.?|str|j\.?p\.?|shop|store|market|servis)\b").unwrap();
    for line in &lines {
        if biz_re.is_match(line)
            && !re_hyph.is_match(line)
            && !line.contains("PIB")
            && !line.contains("MB:")
            && !line.to_lowercase().contains("telefon")
        {
            let name = line.trim_end_matches(',').trim();
            let addr = find_address(&lines);
            if let Some(a) = addr {
                return truncate_name(&format!("{}\r\n{}", name, a));
            }
            return truncate_name(name);
        }
    }

    // Line right after or before the account number line
    if let Some(idx) = account_line_idx {
        if idx + 1 < lines.len() {
            let candidate = lines[idx + 1];
            if looks_like_name(candidate) {
                let addr = find_address_near(&lines, idx + 1);
                if let Some(a) = addr {
                    return truncate_name(&format!("{}\r\n{}", candidate.trim_end_matches(','), a));
                }
                return truncate_name(candidate);
            }
        }
        if idx > 0 {
            let candidate = lines[idx - 1];
            if looks_like_name(candidate) {
                return truncate_name(candidate);
            }
        }
    }

    "Primalac".to_string()
}

/// Extract address (street + city) from raw text, even single-line
fn extract_address_from_text(text: &str) -> Option<String> {
    // Look for "Street Name Number, Zip City" or "Street Name Number, City"
    let re = Regex::new(
        r"(?i)([\w\s]+\s+\d+)\s*[,.]?\s*(\d{5}\s+\w+)"
    ).unwrap();
    if let Some(caps) = re.captures(text) {
        let street = caps[1].trim();
        let city = caps[2].trim();
        // Filter out false positives (phone numbers, amounts)
        if !street.contains('+') && street.len() > 5 && street.len() < 40 {
            return Some(format!("{}\r\n{}", street, city));
        }
    }
    None
}

fn looks_like_name(s: &str) -> bool {
    let lower = s.to_lowercase();
    // Not a number line, not metadata
    !s.chars().all(|c| c.is_ascii_digit() || c == '-' || c == ' ')
        && !lower.contains("pib")
        && !lower.contains("mb:")
        && !lower.contains("telefon")
        && !lower.contains("iznos")
        && !lower.contains("uplat")
        && !lower.contains("postovani")
        && !lower.contains("porudzbina")
        && !lower.contains("placanj")
        && !lower.contains("hvala")
        && !lower.contains("ukoliko")
        && !lower.contains("kontakt")
        && !lower.contains("din")
        && s.len() > 3
        && s.len() < 60
}

fn find_address(lines: &[&str]) -> Option<String> {
    let addr_re = Regex::new(r"(?i)\d{5}\s+\w+|(?:ulica|br\.?|bb)\s|(?:beograd|novi sad|nis|cacak|kragujevac|subotica|kraljevo|zrenjanin|pancevo|leskovac|valjevo|uzice)").unwrap();
    let street_re = Regex::new(r"(?i)^\s*[\w\s]+(?: \d+| bb)").unwrap();
    for line in lines {
        if addr_re.is_match(line) || street_re.is_match(line) {
            let l = line.trim().trim_end_matches(',').trim_end_matches('.');
            if l.len() > 3 && l.len() < 50 && !l.to_lowercase().contains("telefon") {
                return Some(l.to_string());
            }
        }
    }
    None
}

fn find_address_near(lines: &[&str], name_idx: usize) -> Option<String> {
    // Look at next 2 lines for address-like content
    for i in (name_idx + 1)..std::cmp::min(name_idx + 3, lines.len()) {
        let line = lines[i].trim();
        let addr_re = Regex::new(r"(?i)\d{5}|ulica|br\.?|bb|beograd|cacak|novi sad|nis|kragujevac").unwrap();
        if addr_re.is_match(line) && !line.to_lowercase().contains("telefon") && line.len() < 50 {
            return Some(line.trim_end_matches(',').trim_end_matches('.').to_string());
        }
    }
    None
}

fn extract_purpose(text: &str) -> String {
    let lower = text.to_lowercase();

    // Look for "svrha" field
    let svrha_re = Regex::new(r"(?i)svrha[:\s]+(.+)").unwrap();
    if let Some(caps) = svrha_re.captures(text) {
        return truncate_purpose(&caps[1]);
    }

    // Infer from context
    if lower.contains("porudzbina") || lower.contains("porucivanj") {
        return "Placanje porudzbine".to_string();
    }
    if lower.contains("faktur") {
        return "Placanje fakture".to_string();
    }
    if lower.contains("clanan") || lower.contains("clanarin") {
        return "Placanje clanarine".to_string();
    }
    if lower.contains("kirij") || lower.contains("zakup") {
        return "Placanje zakupa".to_string();
    }
    if lower.contains("elektricn") || lower.contains("eps") || lower.contains("struj") {
        return "Elektricna energija".to_string();
    }
    if lower.contains("infostud") || lower.contains("poslovi") {
        return "Placanje oglasa".to_string();
    }

    "Uplata".to_string()
}

fn truncate_purpose(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.len() > 35 {
        trimmed[..35].to_string()
    } else {
        trimmed.to_string()
    }
}

fn truncate_name(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.len() > 70 {
        trimmed[..70].to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cartel_shop() {
        let text = r#"Postovani,
Prilikom porucivanja odabrali ste opciju "direktna bankovna transakcija". Vasa porudzbina je u obradi, kako bi bila realizovana potrebno je da izvrsite uplatu na nas racun u Intesa Banci:
160-445519-82
Cartel Shop,
MB: 63587001
PIB: 108630307
Milosa Obilica 2,
32000 Cacak.
kontakt telefon +381695500557.
Ukoliko ipak zelite placanje prilikom preuzimanja paketa, obavestite nas o tome.
Hvala Vam na ukazanom poverenju Vas Cartel Shop & praznecigarete.com
Iznos za uplatu 1.300 din"#;

        let info = parse_payment(text).unwrap();
        assert_eq!(info.account, "160000000044551982");
        assert_eq!(info.amount, "1300,");
        assert!(info.name.contains("Cartel Shop"));
        assert_eq!(info.purpose, "Placanje porudzbine");
    }

    #[test]
    fn test_account_padding() {
        assert_eq!(
            extract_account("racun: 160-445519-82").unwrap(),
            "160000000044551982"
        );
    }

    #[test]
    fn test_amount_with_decimals() {
        assert_eq!(normalize_amount("1.300,50"), "1300,50");
    }

    #[test]
    fn test_amount_without_decimals() {
        assert_eq!(normalize_amount("1300"), "1300,");
    }

    #[test]
    fn test_amount_thousands() {
        assert_eq!(normalize_amount("15.000"), "15000,");
    }
}
