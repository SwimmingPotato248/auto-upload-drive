extern crate google_drive3 as drive3;
use chrono;
use drive3::api::{File, Permission};
use drive3::Error;
use drive3::{hyper, hyper_rustls, oauth2, DriveHub};
use regex::Regex;
use std::env;
use std::fs::{self, DirEntry};

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        panic!("Not enough args")
    }

    let app_secret = oauth2::read_application_secret(&args[1])
        .await
        .expect("json/credentials.json");

    let auth = oauth2::InstalledFlowAuthenticator::builder(
        app_secret,
        oauth2::InstalledFlowReturnMethod::HTTPRedirect,
    )
    .persist_tokens_to_disk("tokencache.json")
    .build()
    .await
    .unwrap();

    let hub = DriveHub::new(
        hyper::Client::builder().build(
            hyper_rustls::HttpsConnectorBuilder::new()
                .with_native_roots()
                .https_or_http()
                .enable_http1()
                .build(),
        ),
        auth,
    );

    let base_dir = args[2].as_str();

    let paths: Vec<_> = fs::read_dir(&base_dir)
        .unwrap()
        .into_iter()
        .map(|res| res.unwrap())
        .collect();
    let email_regex = &args[4];
    let re = Regex::new(&(format!(r"([^-]*@{})", email_regex))).unwrap();

    for path in paths {
        let tmp_path = path.path();
        let str = tmp_path.to_str().unwrap();
        let email = re.find(str).unwrap().as_str();
        let dir = String::from(base_dir);
        let new_dir = dir.clone() + "/" + email;
        fs::create_dir(&new_dir).unwrap_or_else(|_| {});
        let new_path = String::from(&new_dir) + "/" + path.file_name().to_str().unwrap();
        fs::rename(str, new_path).unwrap();
    }

    // Create base directory

    let create_dir_request = File {
        name: Some(
            String::from("Vault export ") + &chrono::offset::Local::now().format("%F").to_string(),
        ),
        mime_type: Some("application/vnd.google-apps.folder".to_owned()),

        ..Default::default()
    };

    let create_dir_results = hub
        .files()
        .create(create_dir_request)
        .upload(
            fs::File::open(args[1].to_owned()).unwrap(),
            "application/vnd.google-apps.folder".parse().unwrap(),
        )
        .await;

    match create_dir_results {
        Err(e) => match e {
            // The Error enum provides details about what exactly happened.
            // You can also just use its `Debug`, `Display` or `Error` traits
            Error::HttpError(_)
            | Error::Io(_)
            | Error::MissingAPIKey
            | Error::MissingToken(_)
            | Error::Cancelled
            | Error::UploadSizeLimitExceeded(_, _)
            | Error::Failure(_)
            | Error::BadRequest(_)
            | Error::FieldClash(_)
            | Error::JsonDecodeError(_, _) => println!("{}", e),
        },
        Ok(res) => {
            let parent_dir = res.1.id.unwrap();

            let dirs: Vec<_> = fs::read_dir(base_dir)
                .unwrap()
                .into_iter()
                .map(|res| res.unwrap())
                .collect();

            for dir in dirs {
                let email = dir.file_name().into_string().unwrap();
                // Create folder
                let create_dir_request = File {
                    name: Some(dir.file_name().into_string().unwrap()),
                    mime_type: Some("application/vnd.google-apps.folder".to_owned()),
                    parents: Some(vec![parent_dir.to_owned()]),
                    ..Default::default()
                };

                let create_dir_results = hub
                    .files()
                    .create(create_dir_request)
                    .upload(
                        fs::File::open(args[1].to_owned()).unwrap(),
                        "application/vnd.google-apps.folder".parse().unwrap(),
                    )
                    .await;

                match create_dir_results {
                    Err(e) => match e {
                        // The Error enum provides details about what exactly happened.
                        // You can also just use its `Debug`, `Display` or `Error` traits
                        Error::HttpError(_)
                        | Error::Io(_)
                        | Error::MissingAPIKey
                        | Error::MissingToken(_)
                        | Error::Cancelled
                        | Error::UploadSizeLimitExceeded(_, _)
                        | Error::Failure(_)
                        | Error::BadRequest(_)
                        | Error::FieldClash(_)
                        | Error::JsonDecodeError(_, _) => println!("{}", e),
                    },
                    Ok(res) => {
                        // Share folder
                        let dir_id = res.1.id.unwrap();

                        let permission_request = Permission {
                            email_address: Some(email),
                            role: Some("reader".to_owned()),
                            type_: Some("user".to_owned()),
                            ..Default::default()
                        };

                        hub.permissions()
                            .create(permission_request, &dir_id)
                            .doit()
                            .await
                            .unwrap();

                        // Upload files

                        let files: Vec<DirEntry> = fs::read_dir(dir.path().to_owned())
                            .unwrap()
                            .into_iter()
                            .map(|e| e.unwrap())
                            .collect();

                        for file in files {
                            let req = File {
                                name: Some(file.file_name().into_string().unwrap()),
                                mime_type: Some("application/octet-stream".to_string()),
                                parents: Some(vec![dir_id.to_owned()]),
                                ..Default::default()
                            };

                            hub.files()
                                .create(req)
                                .upload(
                                    fs::File::open(file.path().to_str().unwrap()).unwrap(),
                                    "application/octet-stream".parse().unwrap(),
                                )
                                .await
                                .unwrap();
                        }
                    }
                }
            }
        }
    }
}
