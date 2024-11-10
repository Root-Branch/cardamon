use anyhow::Context;
use chrono::{DateTime, Datelike, Months, Utc};
use phf::phf_map;
use serde_json::Value;

pub const GLOBAL_CI: f64 = 0.494; // g/Wh

static ISO_3166: phf::Map<&'static str, &'static str> = phf_map! {
    "AF" => "AFG", "AX" => "ALA", "AL" => "ALB", "DZ" => "DZA", "AS" => "ASM", "AD" => "AND",
    "AO" => "AGO", "AI" => "AIA", "AQ" => "ATA", "AG" => "ATG", "AR" => "ARG", "AM" => "ARM",
    "AW" => "ABW", "AU" => "AUS", "AT" => "AUT", "AZ" => "AZE", "BS" => "BHS", "BH" => "BHR",
    "BD" => "BGD", "BB" => "BRB", "BY" => "BLR", "BE" => "BEL", "BZ" => "BLZ", "BJ" => "BEN",
    "BM" => "BMU", "BT" => "BTN", "BO" => "BOL", "BQ" => "BES", "BA" => "BIH", "BW" => "BWA",
    "BV" => "BVT", "BR" => "BRA", "IO" => "IOT", "BN" => "BRN", "BG" => "BGR", "BF" => "BFA",
    "BI" => "BDI", "CV" => "CPV", "KH" => "KHM", "CM" => "CMR", "CA" => "CAN", "KY" => "CYM",
    "CF" => "CAF", "TD" => "TCD", "CL" => "CHL", "CN" => "CHN", "CX" => "CXR", "CC" => "CCK",
    "CO" => "COL", "KM" => "COM", "CG" => "COG", "CD" => "COD", "CK" => "COK", "CR" => "CRI",
    "CI" => "CIV", "HR" => "HRV", "CU" => "CUB", "CW" => "CUW", "CY" => "CYP", "CZ" => "CZE",
    "DK" => "DNK", "DJ" => "DJI", "DM" => "DMA", "DO" => "DOM", "EC" => "ECU", "EG" => "EGY",
    "SV" => "SLV", "GQ" => "GNQ", "ER" => "ERI", "EE" => "EST", "SZ" => "SWZ", "ET" => "ETH",
    "FK" => "FLK", "FO" => "FRO", "FJ" => "FJI", "FI" => "FIN", "FR" => "FRA", "GF" => "GUF",
    "PF" => "PYF", "TF" => "ATF", "GA" => "GAB", "GM" => "GMB", "GE" => "GEO", "DE" => "DEU",
    "GH" => "GHA", "GI" => "GIB", "GR" => "GRC", "GL" => "GRL", "GD" => "GRD", "GP" => "GLP",
    "GU" => "GUM", "GT" => "GTM", "GG" => "GGY", "GN" => "GIN", "GW" => "GNB", "GY" => "GUY",
    "HT" => "HTI", "HM" => "HMD", "VA" => "VAT", "HN" => "HND", "HK" => "HKG", "HU" => "HUN",
    "IS" => "ISL", "IN" => "IND", "ID" => "IDN", "IR" => "IRN", "IQ" => "IRQ", "IE" => "IRL",
    "IM" => "IMN", "IL" => "ISR", "IT" => "ITA", "JM" => "JAM", "JP" => "JPN", "JE" => "JEY",
    "JO" => "JOR", "KZ" => "KAZ", "KE" => "KEN", "KI" => "KIR", "KP" => "PRK", "KR" => "KOR",
    "KW" => "KWT", "KG" => "KGZ", "LA" => "LAO", "LV" => "LVA", "LB" => "LBN", "LS" => "LSO",
    "LR" => "LBR", "LY" => "LBY", "LI" => "LIE", "LT" => "LTU", "LU" => "LUX", "MO" => "MAC",
    "MG" => "MDG", "MW" => "MWI", "MY" => "MYS", "MV" => "MDV", "ML" => "MLI", "MT" => "MLT",
    "MH" => "MHL", "MQ" => "MTQ", "MR" => "MRT", "MU" => "MUS", "YT" => "MYT", "MX" => "MEX",
    "FM" => "FSM", "MD" => "MDA", "MC" => "MCO", "MN" => "MNG", "ME" => "MNE", "MS" => "MSR",
    "MA" => "MAR", "MZ" => "MOZ", "MM" => "MMR", "NA" => "NAM", "NR" => "NRU", "NP" => "NPL",
    "NL" => "NLD", "NC" => "NCL", "NZ" => "NZL", "NI" => "NIC", "NE" => "NER", "NG" => "NGA",
    "NU" => "NIU", "NF" => "NFK", "MK" => "MKD", "MP" => "MNP", "NO" => "NOR", "OM" => "OMN",
    "PK" => "PAK", "PW" => "PLW", "PS" => "PSE", "PA" => "PAN", "PG" => "PNG", "PY" => "PRY",
    "PE" => "PER", "PH" => "PHL", "PN" => "PCN", "PL" => "POL", "PT" => "PRT", "PR" => "PRI",
    "QA" => "QAT", "RE" => "REU", "RO" => "ROU", "RU" => "RUS", "RW" => "RWA", "BL" => "BLM",
    "SH" => "SHN", "KN" => "KNA", "LC" => "LCA", "MF" => "MAF", "PM" => "SPM", "VC" => "VCT",
    "WS" => "WSM", "SM" => "SMR", "ST" => "STP", "SA" => "SAU", "SN" => "SEN", "RS" => "SRB",
    "SC" => "SYC", "SL" => "SLE", "SG" => "SGP", "SX" => "SXM", "SK" => "SVK", "SI" => "SVN",
    "SB" => "SLB", "SO" => "SOM", "ZA" => "ZAF", "GS" => "SGS", "SS" => "SSD", "ES" => "ESP",
    "LK" => "LKA", "SD" => "SDN", "SR" => "SUR", "SJ" => "SJM", "SE" => "SWE", "CH" => "CHE",
    "SY" => "SYR", "TW" => "TWN", "TJ" => "TJK", "TZ" => "TZA", "TH" => "THA", "TL" => "TLS",
    "TG" => "TGO", "TK" => "TKL", "TO" => "TON", "TT" => "TTO", "TN" => "TUN", "TR" => "TUR",
    "TM" => "TKM", "TC" => "TCA", "TV" => "TUV", "UG" => "UGA", "UA" => "UKR", "AE" => "ARE",
    "GB" => "GBR", "US" => "USA", "UM" => "UMI", "UY" => "URY", "UZ" => "UZB", "VU" => "VUT",
    "VE" => "VEN", "VN" => "VNM", "VG" => "VGB", "VI" => "VIR", "WF" => "WLF", "EH" => "ESH",
    "YE" => "YEM", "ZM" => "ZMB", "ZW" => "ZWE",
};

const EMBER_API_BASE_URL: &str = "https://api.ember-energy.org/v1/carbon-intensity";
const EMBER_KEY: &str = "c5e07f2c-5d07-4b99-a78e-661097d874e6";

pub fn valid_region_code(code: &str) -> bool {
    ISO_3166.get_key(code).is_some()
}

fn try_parse_region(json_obj: Value) -> Option<String> {
    json_obj.get("country")?.as_str().map(|str| str.to_string())
}

pub async fn fetch_region_code() -> anyhow::Result<String> {
    let client = reqwest::Client::new();

    let resp = client
        .get("https://api.country.is/")
        .header("Content-Type", "application/json")
        .send()
        .await?;

    let json_obj = resp.json().await?;
    try_parse_region(json_obj).context("Error fetching region from IP")
}

fn try_parse_ci(json_obj: &Value) -> Option<f64> {
    json_obj
        .get("stats")?
        .get("query_value_range")?
        .get("emissions_intensity_gco2_per_kwh")?
        .get("max")?
        .as_f64()
        .map(|ci| ci / 1000.0) // g/kWh -> g/Wh
}

/// Attempts to fetch carbon intensity for the given region from Ember.
pub async fn fetch_ci(code: &str, date: &DateTime<Utc>) -> anyhow::Result<f64> {
    let code = ISO_3166.get(code).context("Incorrect ISO 3166 code")?;

    let client = reqwest::Client::new();

    let end = date;
    let start = end
        .checked_sub_months(Months::new(1))
        .context("Error parsing month")?;

    let start_date = format!("{}-{}", start.year(), start.month());
    let end_date = format!("{}-{}", end.year(), end.month());

    let url = format!(
        "{}/monthly?entity_code={}&start_date={}&end_date={}&api_key={}",
        EMBER_API_BASE_URL, code, start_date, end_date, EMBER_KEY
    );

    let resp = client
        .get(url)
        .header("Content-Type", "application/json")
        .send()
        .await?;

    let json_obj = resp.json().await?;
    try_parse_ci(&json_obj).context("Error parsing carbon intensity")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn can_fetch_region_ci() -> anyhow::Result<()> {
        let now = Utc::now();
        let ci = fetch_ci("GB", &now).await?;
        assert!(ci > 0.0);
        Ok(())
    }

    #[tokio::test]
    async fn incorrect_region_should_cause_error() -> anyhow::Result<()> {
        let now = Utc::now();
        let ci = fetch_ci("ZZ", &now).await;
        assert!(ci.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn can_fetch_ip() -> anyhow::Result<()> {
        let region = fetch_region_code().await?;
        assert!(!region.is_empty());

        let now = Utc::now();
        let ci = fetch_ci(&region, &now).await?;
        assert!(ci > 0.0);

        Ok(())
    }
}
