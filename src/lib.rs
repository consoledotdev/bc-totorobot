// Copyright 2021, Console Ltd https://console.dev
// SPDX-License-Identifier: AGPL-3.0-or-later

#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_contrib;

use log::info;
use mailchimp::{Lists, MailchimpApi};
use rocket::config::{Config, Environment};
use rocket::http::{ContentType, Status};
use rocket::request::Request;
use rocket::response;
use rocket::response::{Responder, Response};
use rocket_contrib::json::JsonValue;
use std::collections::HashMap;
use std::env;

// AzureResponse
// Function responses are formatted as key/value pairs
// https://docs.microsoft.com/en-us/azure/azure-functions/functions-custom-handlers#response-payload
struct AzureResponse {
    logs: Vec<String>,
    return_value: String,
}

impl AzureResponse {
    fn to_json(&self) -> JsonValue {
        json!({
            "Logs": self.logs,
            "ReturnValue": self.return_value,
        })
    }
}

// ApiResponse
// If we just return JSON then every response will be HTTP 200 OK even if there
// are errors. This allows setting the HTTP status code
// From: https://stackoverflow.com/a/54867136/2183
struct ApiResponse {
    json: JsonValue,
    status: Status,
}

impl<'r> Responder<'r> for ApiResponse {
    fn respond_to(self, req: &Request) -> response::Result<'r> {
        Response::build_from(self.json.respond_to(&req).unwrap())
            .status(self.status)
            .header(ContentType::JSON)
            .ok()
    }
}

#[get("/health_check")]
fn health_check() -> &'static str {
    "OK"
}

#[post("/post_mailchimp_stats", format = "json")]
fn post_mailchimp_stats() -> ApiResponse {
    let mut logs = Vec::new();

    // Create API client
    let api_key = env::var("TOTORO_MAILCHIMP_APIKEY")
        .expect("TOTORO_MAILCHIMP_APIKEY not set");
    let api = MailchimpApi::new(&api_key);

    // Query the specific list
    let lists = Lists::new(api);
    let list_id = env::var("TOTORO_MAILCHIMP_LIST_ID")
        .expect("TOTORO_MAILCHIMP_LIST_ID not set");
    let r_list = lists.get_list_info(&list_id, HashMap::new());

    match r_list {
        Ok(list) => {
            // Get the stats
            let stats = list.stats.as_ref().expect("No stats returned");

            info!("Raw stats: {:?}", stats);

            // Construct the Campfire bot text
            let mut content =
                String::from("<strong>Mailchimp Stats (Rust)</strong><ul>");

            // The number of active members in the list
            if let Some(member_count) = stats.member_count {
                let s = format!(
                    "<li><strong>Active subscribers:</strong> {:.0}</li>",
                    member_count
                );
                content.push_str(&s);
            }

            // The number of members who have subscribed since the last
            // campaign was sent
            if let Some(subscribe_count_since_send) =
                stats.member_count_since_send
            {
                let s = format!(
                    "<li><strong>Subscribes since last send:</strong> {:.0}</li>",
                    subscribe_count_since_send
                );
                content.push_str(&s);
            }

            // The number of members who have unsubscribed since the last
            // campaign was sent
            if let Some(unsubscribe_count_since_send) =
                stats.unsubscribe_count_since_send
            {
                let s = format!(
                    "<li><strong>Unsubscribes since last send:</strong> {:.0}</li>",
                    unsubscribe_count_since_send
                );
                content.push_str(&s);
            }

            // The average number of subscriptions per month for the list
            if let Some(avg_sub_rate) = stats.avg_sub_rate {
                let s = format!(
                    "<li><strong>Subscribe rate:</strong> {:.0}/m</li>",
                    avg_sub_rate
                );
                content.push_str(&s);
            }

            // The average number of unsubscriptions per month for the list
            if let Some(avg_unsub_rate) = stats.avg_unsub_rate {
                let s = format!(
                    "<li><strong>Unsubscribe rate:</strong> {:.0}/m</li>",
                    avg_unsub_rate
                );
                content.push_str(&s);
            }

            // The average click rate (a percentage represented as a number
            // between 0 and 100) per campaign for the list
            if let Some(click_rate) = stats.click_rate {
                let s = format!(
                    "<li><strong>Click rate:</strong> {:.0}%</li>",
                    click_rate
                );
                content.push_str(&s);
            }

            // Only post to Basecamp if we are actually in production
            if env::var("TOTORO_PRODUCTION").is_ok() {
                // Send it over to Basecamp
                // https://github.com/basecamp/bc3-api/blob/master/sections/chatbots.md#create-a-line
                info!("Sending Basecamp: {:?}", content);

                let basecamp_bot_url = env::var("TOTORO_BASECAMP_BOTURL")
                    .expect("TOTORO_BASECAMP_BOTURL not set");

                let mut json_body = HashMap::new();
                json_body.insert("content", content);

                // Use blocking because rocket is itself blocking
                let client = reqwest::blocking::Client::new();
                let resp = client
                    .post(&basecamp_bot_url)
                    .json(&json_body)
                    .send()
                    .expect("Reqwest client error");

                if resp.status().is_success() {
                    // Build response JSON
                    let response = AzureResponse {
                        logs,
                        return_value: String::from("ok"),
                    };

                    // Return response
                    ApiResponse {
                        json: response.to_json(),
                        status: Status::Ok,
                    }
                } else {
                    // Log errors
                    let error = format!(
                        "Error posting to Basecamp: {:?}",
                        resp.status()
                    );
                    logs.push(error);

                    // Build response JSON
                    let response = AzureResponse {
                        logs,
                        return_value: String::from("error"),
                    };

                    // Return response
                    ApiResponse {
                        json: response.to_json(),
                        status: Status::InternalServerError,
                    }
                }
            } else {
                info!("Would have posted to Basecamp: {:?}", content);

                // Build response JSON
                let response = AzureResponse {
                    logs,
                    return_value: String::from("ok"),
                };

                // Return response
                ApiResponse {
                    json: response.to_json(),
                    status: Status::Ok,
                }
            }
        }
        Err(e) => {
            // Log errors
            let error = format!("Error getting Mailchimp list info: {:?}", e);
            logs.push(error);

            // Build response JSON
            let response = AzureResponse {
                logs,
                return_value: String::from("error"),
            };

            // Return response
            ApiResponse {
                json: response.to_json(),
                status: Status::InternalServerError,
            }
        }
    }
}

pub fn rocket() -> rocket::Rocket {
    // Define Rocket routes
    let routes = routes![health_check, post_mailchimp_stats,];

    // Pick up custom port setting for Azure
    // https://docs.microsoft.com/en-us/azure/azure-functions/create-first-function-vs-code-other?tabs=rust%2Clinux#create-and-build-your-function
    let port: u16 = match env::var("FUNCTIONS_CUSTOMHANDLER_PORT") {
        Ok(val) => val.parse().expect("Custom Handler port is not a number!"),
        Err(_) => 3000,
    };

    // Creating a custom config for each environment seems to be the only way
    // to set a custom port on Rocket
    // https://api.rocket.rs/v0.4/rocket/config/struct.ConfigBuilder.html#example-2
    let config;
    if env::var("TOTORO_PRODUCTION").is_ok() {
        config = Config::build(Environment::Production)
            .port(port)
            .log_level(rocket::config::LoggingLevel::Normal)
            .unwrap();
    } else {
        config = Config::build(Environment::Development)
            .address("127.0.0.1")
            .port(port)
            .log_level(rocket::config::LoggingLevel::Debug)
            .unwrap();
    }

    rocket::custom(config).mount("/", routes)
}

#[cfg(test)]
mod test {
    use super::rocket;
    use rocket::http::Status;
    use rocket::local::Client;

    #[test]
    fn health_check_ok() {
        let client = Client::new(rocket()).expect("valid rocket instance");
        let mut response = client.get("/health_check").dispatch();
        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.body_string(), Some("OK".into()));
    }

    // TODO: Test post_mailchimp_stats
}
