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
    let reference = extract_reference(text);

    Ok(PaymentInfo {
        account,
        name,
        amount,
        purpose,
        code: "289".to_string(),
        reference,
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
/// Handles both comma-decimal ("3.307,78") and dot-decimal ("3307.78") formats.
fn extract_amount(text: &str) -> Result<String, String> {
    // Collect all candidate amounts with their context, pick the best one
    let mut candidates: Vec<(String, usize)> = Vec::new(); // (normalized_amount, priority)

    // Pattern 1: keyword context (highest priority)
    // "iznosu od 3307.78", "iznos: 1300", "za uplatu 1.300 din"
    let kw_re = Regex::new(r"(?i)(?:iznos\w*|uplat\w*|cen\w*|ukupno|svega|za\s+uplatu)\s+(?:od\s+|[:\s])*(\d[\d.,]*)(?:\s*(?:din\w*|rsd))").unwrap();
    if let Some(caps) = kw_re.captures(text) {
        candidates.push((normalize_amount(&caps[1]), 0));
    }

    // Pattern 2: number directly followed by currency ("3307.78 RSD", "1.300 din")
    let currency_re = Regex::new(r"(?i)(\d[\d.,]*)\s*(?:din(?:ara)?|rsd)\b").unwrap();
    for caps in currency_re.captures_iter(text) {
        let raw = caps[1].to_string();
        // Skip if it looks like a phone number or account-adjacent
        if raw.len() > 1 {
            candidates.push((normalize_amount(&raw), 1));
        }
    }

    // Pattern 3: "RSD 3307.78" prefix style
    let prefix_re = Regex::new(r"(?i)RSD\s*(\d[\d.,]*)").unwrap();
    if let Some(caps) = prefix_re.captures(text) {
        candidates.push((normalize_amount(&caps[1]), 1));
    }

    // Among same-priority candidates, prefer the largest amount (avoids partial matches)
    candidates.sort_by(|a, b| {
        a.1.cmp(&b.1).then_with(|| {
            let val_a = amount_value(&a.0);
            let val_b = amount_value(&b.0);
            val_b.partial_cmp(&val_a).unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    if let Some((amount, _)) = candidates.into_iter().next() {
        return Ok(amount);
    }

    Err("Nije pronadjen iznos. Ocekujem npr: '1.300 din' ili 'iznos 1300 RSD'".to_string())
}

/// Parse a normalized amount string to a float for comparison
fn amount_value(s: &str) -> f64 {
    let clean = s.trim_end_matches(',').replace(',', ".");
    clean.parse::<f64>().unwrap_or(0.0)
}

/// Normalize amount string to NBS format.
/// Handles both dot-decimal ("3307.78") and comma-decimal ("3.307,78") inputs.
/// Output always uses comma as decimal separator per NBS spec.
fn normalize_amount(raw: &str) -> String {
    let s = raw.trim();

    // Determine if dot is decimal or thousand separator:
    // - "3307.78" -> dot is decimal (1-2 digits after last dot, no comma present)
    // - "1.300" -> dot is thousand sep (exactly 3 digits after dot)
    // - "1.300,50" -> dot is thousand sep, comma is decimal

    if s.contains(',') {
        // Comma present -> dots are thousand separators, comma is decimal
        let no_thousands = s.replace('.', "");
        return ensure_comma(&no_thousands);
    }

    if let Some(dot_pos) = s.rfind('.') {
        let after_dot = &s[dot_pos + 1..];
        if after_dot.len() <= 2 {
            // Dot is decimal separator (e.g. "3307.78")
            // Convert dot to comma
            let converted = format!("{},{}", &s[..dot_pos], after_dot);
            return converted;
        } else {
            // Dot is thousand separator (e.g. "1.300" -> 3 digits after dot)
            let no_thousands = s.replace('.', "");
            return ensure_comma(&no_thousands);
        }
    }

    // No dot, no comma — whole number
    ensure_comma(s)
}

fn ensure_comma(s: &str) -> String {
    if s.contains(',') {
        s.to_string()
    } else {
        format!("{},", s)
    }
}

/// Extract reference number (model + poziv na broj)
fn extract_reference(text: &str) -> Option<String> {
    // Pattern: "model 97" ... "poziv na broj 60600272972371"
    let model_re = Regex::new(r"(?i)model\s+(\d{2})").unwrap();
    let poziv_re = Regex::new(r"(?i)poziv\s+na\s+broj\s+(\d+)").unwrap();

    let model = model_re.captures(text).map(|c| c[1].to_string());
    let poziv = poziv_re.captures(text).map(|c| c[1].to_string());

    match (model, poziv) {
        (Some(m), Some(p)) => Some(format!("{}{}", m, p)),
        (None, Some(p)) => Some(format!("00{}", p)),
        _ => None,
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

/// Extract address (street + city) from raw text, line by line to avoid cross-line greediness
fn extract_address_from_text(text: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().map(|l| l.trim()).collect();
    // Match street: "Name Name Number" on a single line, must start with a letter
    let street_re = Regex::new(r"(?i)^([A-Za-z\u{0100}-\u{024F}][\w ]+\s+\d+)\s*[,.]?\s*$").unwrap();
    let city_re = Regex::new(r"(?i)^\s*(\d{5}\s+\w+)").unwrap();

    for (i, line) in lines.iter().enumerate() {
        if let Some(caps) = street_re.captures(line) {
            let street = caps[1].trim();
            if street.len() > 5 && street.len() < 40 {
                if i + 1 < lines.len() {
                    if let Some(city_caps) = city_re.captures(lines[i + 1]) {
                        return Some(format!("{}\r\n{}", street, city_caps[1].trim()));
                    }
                }
                return Some(street.to_string());
            }
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
    let city_re = Regex::new(r"(?i)^\s*\d{5}\s+\w+").unwrap();
    let street_re = Regex::new(r"(?i)^[A-Za-z\u{0100}-\u{024F}][\w ]+\s+\d+\s*[,.]?\s*$").unwrap();
    let known_city = Regex::new(r"(?i)\b(?:beograd|novi sad|nis|cacak|kragujevac|subotica|kraljevo|zrenjanin|pancevo|leskovac|valjevo|uzice)\b").unwrap();

    // First pass: find street line
    for (i, line) in lines.iter().enumerate() {
        if street_re.is_match(line) {
            let street = line.trim().trim_end_matches(',').trim_end_matches('.');
            if street.len() > 5 && street.len() < 45
                && !street.to_lowercase().contains("telefon")
                && !street.to_lowercase().contains("pib")
                && !street.to_lowercase().contains("mb:")
            {
                // Check next line for city
                if i + 1 < lines.len() {
                    let next = lines[i + 1].trim().trim_end_matches(',').trim_end_matches('.');
                    if city_re.is_match(next) || known_city.is_match(next) {
                        return Some(format!("{}\r\n{}", street, next));
                    }
                }
                return Some(street.to_string());
            }
        }
    }

    // Second pass: just find a city line
    for line in lines {
        let l = line.trim().trim_end_matches(',').trim_end_matches('.');
        if (city_re.is_match(l) || known_city.is_match(l))
            && l.len() > 3 && l.len() < 50
            && !l.to_lowercase().contains("telefon")
        {
            return Some(l.to_string());
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
    fn test_cartel_shop_no_pib_in_name() {
        let text = "Postovani,\nPrilikom porucivanja odabrali ste opciju \"direktna bankovna transakcija\".\n160-445519-82\nCartel Shop,\nMB: 63587001\nPIB: 108630307\nMilosa Obilica 2,\n32000 Cacak.\nkontakt telefon +381695500557.\nIznos za uplatu 1.300 din";
        let info = parse_payment(text).unwrap();
        assert!(!info.name.contains("108630307"), "PIB leaked into name: {}", info.name);
        assert!(info.name.contains("Cartel Shop"), "Name missing: {}", info.name);
        // Max 3 lines
        let line_count = info.name.split("\r\n").count();
        assert!(line_count <= 3, "Name has {} lines: {}", line_count, info.name);
    }

    #[test]
    fn test_account_padding() {
        assert_eq!(
            extract_account("racun: 160-445519-82").unwrap(),
            "160000000044551982"
        );
    }

    #[test]
    fn test_a1_invoice() {
        let text = "Sutra istice rok za placanje racuna 03/2026. Ako jos niste izmirili svoj racun, dug u iznosu od 3307.78 RSD mozete da uplatite\nna tekuci racun 265-1110312345678-24\nmodel 97\npoziv na broj 60600272972371.\nU svakom trenutku racun mozete da platite online. Vas A1";
        let info = parse_payment(text).unwrap();
        assert_eq!(info.account, "265111031234567824");
        assert_eq!(info.amount, "3307,78", "Amount was: {}", info.amount);
        assert_eq!(info.reference, Some("9760600272972371".to_string()));
    }

    #[test]
    fn test_amount_dot_decimal() {
        assert_eq!(normalize_amount("3307.78"), "3307,78");
    }

    #[test]
    fn test_amount_comma_decimal() {
        assert_eq!(normalize_amount("1.300,50"), "1300,50");
    }

    #[test]
    fn test_amount_without_decimals() {
        assert_eq!(normalize_amount("1300"), "1300,");
    }

    #[test]
    fn test_amount_thousands_only() {
        assert_eq!(normalize_amount("15.000"), "15000,");
    }
}
