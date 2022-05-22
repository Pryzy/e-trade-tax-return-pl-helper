use serde::{Deserialize, Serialize};

pub use crate::logging::ResultExt;

pub struct PL {}

type ReqwestClient = reqwest::blocking::Client;

// Example response: {"table":"A",
//                    "currency":"dolar amerykański",
//                    "code":"USD",
//                    "rates":[{"no":"039/A/NBP/2021",
//                              "effectiveDate":"2021-02-26",
//                              "mid":3.7247}]}

#[derive(Debug, Deserialize, Serialize)]
struct NBPResponse<T> {
    table: String,
    currency: String,
    code: String,
    rates: Vec<T>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ExchangeRate {
    no: String,
    effectiveDate: String,
    mid: f32,
}

impl etradeTaxReturnHelper::Residency for PL {
    fn get_exchange_rates(
        &self,
        dates: &mut std::collections::HashMap<String, Option<(String, f32)>>
    ) -> Result<(), String>{

        // proxies are taken from env vars: http_proxy and https_proxy
        let http_proxy = std::env::var("http_proxy");
        let https_proxy = std::env::var("https_proxy");

        // If there is proxy then pick first URL
        let base_client = ReqwestClient::builder();
        let client = match &http_proxy {
            Ok(proxy) => base_client
                .proxy(reqwest::Proxy::http(proxy).expect_and_log("Error setting HTTP proxy")),
            Err(_) => base_client,
        };
        let client = match &https_proxy {
            Ok(proxy) => client
                .proxy(reqwest::Proxy::https(proxy).expect_and_log("Error setting HTTP proxy")),
            Err(_) => client,
        };
        let client = client
            .build()
            .expect_and_log("Could not create REST API client");

        let base_exchange_rate_url = "http://api.nbp.pl/api/exchangerates/rates/a/";

        dates.iter_mut().for_each(|(date, val)| {
            let mut converted_date =
                chrono::NaiveDate::parse_from_str(&date, "%m/%d/%y").unwrap();

            // Try to get exchange rate going backwards with dates till success
            let mut is_success = false;
            while is_success == false {
                converted_date = converted_date
                    .checked_sub_signed(chrono::Duration::days(1))
                    .expect_and_log("Error traversing date");

                let exchange_rate_url: String = base_exchange_rate_url.to_string()
                    + &format!("usd/{}", converted_date.format("%Y-%m-%d"))
                    + "/?format=json";

                let body = client.get(&(exchange_rate_url)).send();
                let actual_body = body.expect_and_log(&format!(
                    "Getting Exchange Rate from NBP ({}) failed",
                    exchange_rate_url
                ));
                is_success = actual_body.status().is_success();
                if is_success == true {
                    log::info!("RESPONSE {:#?}", actual_body);

                    let nbp_response = actual_body
                        .json::<NBPResponse<ExchangeRate>>()
                        .expect_and_log("Error converting response to JSON");
                    log::info!("body of exchange_rate = {:#?}", nbp_response);
                    let exchange_rate = nbp_response.rates[0].mid;
                    let exchange_rate_date = format!("{}", converted_date.format("%Y-%m-%d"));
                        *val = Some((exchange_rate_date, exchange_rate));
                    };
                }
        });
        Ok(())
    }

    fn present_result(&self, gross_us_pl: f32, tax_us_pl: f32) {
        println!("===> PRZYCHOD Z ZAGRANICY: {} PLN", gross_us_pl);
        println!("===> PODATEK ZAPLACONY ZAGRANICA: {} PLN", tax_us_pl);
        // Expected full TAX in Poland
        let full_tax_pl = gross_us_pl * 19.0 / 100.0;
        // Normally you pay 15% in US, but if you made wrong
        // choices in your residency application you may be charged 30%
        // in that case you do not pay anything in Poland because you paid
        // 30% alrady in US
        let tax_diff_to_pay_pl = if full_tax_pl > tax_us_pl {
            full_tax_pl - tax_us_pl
        } else {
            0.0
        };
        println!("DOPLATA: {} PLN", tax_diff_to_pay_pl);
    }
}
